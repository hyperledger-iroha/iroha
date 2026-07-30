---- MODULE SumeragiV2AsyncNetwork ----
EXTENDS SumeragiV2Inductive, Sequences, FiniteSets, Naturals, Functions

(***************************************************************************
Production-coupled asynchronous execution for Sumeragi v2.

There is one reducer scheduler per validator.  The validators are independent
processes: post-GST weak fairness is attached to each responsive validator's
run-loop action, never to a favorable global interleaving and never to an
individual protocol command.  Each runtime owns the same single bounded queue
as `BoundedIngress`; normal, progress, and completion are admission classes
over its total length.  Dispatch first selects the least immutable lifecycle
ordinal across all classes; the production class cursor
(Completion -> Progress -> Normal) breaks ties, and physical FIFO order breaks
same-class ties.  Causal successors inherit the root ordinal, so later
Completion, Control, and timeout work cannot jump an admitted lifecycle.
TimeoutElapsed and RetransmitElapsed are local scheduler inputs.  Timeout work
remains behind the reducer Busy fence.  RetransmitElapsed instead names the
bounded run-loop program point immediately before production's unconditional
`drive_block_sync` call: once armed, the next Runtime visit emits retained
network work even while the reducer executor is busy.

Queued stale I/O is deliberately retained in this model as a conservative
service burden.  Production cancels queued certified-stale Sign/Store/Validate
work by exact identity and coalesces exact retries until the completion is
acknowledged; non-stale admission remains lossless.  Thus a full model queue
leaves its causal producer head pending until weakly fair class-aware service
creates capacity, over-approximating rather than hiding stale work.  Likewise all
validators in one instance use the same view-indexed pacemaker rule; mixed
fixed/adaptive binaries require a version or configuration-fingerprint gate
before the temporal theorem applies to a deployment.

The hidden transport carries actual Core network envelopes into a distinct
model of `FairV2Ingress`.  `ValidatorIds` is the finite configured universe of
authenticated peers and ingress sources; `CurrentVoters` is only its
positive-power voting subset.  Every recipient has one lane for each configured
authenticated peer plus one aggregate untrusted lane.  Admission is bounded by
one total capacity while preserving the empty-lane, Progress, TimeoutVote,
shared physical completion, and post-service continuation potential; non-empty
lanes are serviced by the exact ready-queue rotation used in Rust.  `Chunk` and
`CertifiedResponse` have distinct logical classes but share one physical
completion owner per lane.  Every preauthenticated certified response is
charged to the aggregate untrusted lane regardless of its relay hop.  A
zero-power or non-roster authenticated hop otherwise spends one of the same N
bounded source lanes and acquires no consensus authority.  An unattributed
relay may also use the aggregate untrusted lane, whose completion slot and
generic continuation never borrow an authenticated peer's owners.  A source
may borrow idle message capacity but cannot consume another source's
reservations.  Each authenticated validator source also isolates the fixed
valid-timeout-vote byte reserve from all other wire traffic.
The full canonical-wire TransportCompletion byte ceiling is intentionally
abstract here and is linked to the exact count/byte mutation refinement in
`SumeragiV2EffectCapacityOuterTransportMutation`; this module makes no byte
claim for that class.  `Chunk`/`CertifiedResponse` and `ProgressJunk` are also
the finite representatives for production lane-local completion and progress
traffic, respectively.  The production path admits both V2 and lane-local
messages through the same owner and separately enforces one- and four-MiB
lane-local wire ceilings; that class/byte correspondence is an explicit
production-refinement premise rather than a theorem of this byte-abstract
module.

`AsyncNetworkItem.source` is the authenticated transport/relay hop.  A
historical response separately carries the authenticated archive server, the
exact signed-request hash, and one frozen certificate signer citation.  Every
fully authenticated certified response is charged to the aggregate untrusted
physical ingress lane independently of that relay hop; all other traffic is
charged to `item.source`.  The outer response remains the reducer candidate's
protected evidence even though the payload delivered to Core is the certified
body.  Canonical request identity excludes the response hop and every
process-local route ordinal.
`SumeragiV2AsyncNetworkReplyRoutes!AsyncProductionSpec`
composes a bounded per-authenticated-source ownership machine with this
consensus scheduler: each source has an
independent message/chunk cursor and tenure-bound ticket, while immutable
payload materialization is shared by semantic identity.  Exact retry, a later
delivery on the same tenure, and reconnect preserve progress; only a newly
observed alternate source starts at item zero.  The source capacity is derived
from the validator/source geometry and the exact-output corridor fails
configuration when it cannot reserve every source.

An emitted Core envelope remains in immutable authentication history when a
hidden packet is lost before GST.  Retransmission scans only the reducer's
bounded per-class retained controls and active certified-body requests.  Packet
publication is atomic here; production's actor admission, encode, frame, batch,
write, and flush stages and its remaining broadcast cursor must refine that
single action while retaining the exact occurrence until the matching flush
acknowledgement.  This is another unassigned production-refinement proposition,
not a consequence of the abstract packet fairness actions.

Post-GST historical catch-up is also exact scheduler ownership.  A responsive
peer with no local Decision may be opened as one explicit recovery target only
when a responsive authenticated archive already holds the applied Commit
receipt.  The target then uses its own fair runner, certificate discovery, I/O
worker, and bidirectional bounded packet corridor; the exact Apply command
retires that ownership atomically.  Applied zero-power peers may continue to
drain authenticated historical requests and run the bounded Serve I/O lane,
but they never enter a current-voter RunNode, vote, or QC domain.

Most importantly, this module has no shadow decision, application, vote,
view-change, or chain-rollover transition.  A serviced reducer command invokes
exactly one Core action, while scheduler, timer-admission, chunk, and hidden-
transport bookkeeping are Core stutters.  The module describes one arbitrary
frozen height context; chain-height composition is proved separately over the
canonical ChainEpoch successor relation.
***************************************************************************)

CONSTANTS
  AsyncQueueCapacity,
  AsyncProgressReserve,
  AsyncCompletionReserve,
  AsyncIngressCapacity,
  AsyncIoAuxCapacity,
  AsyncIoWorkCapacity,
  AsyncDeferredNormalCapacity,
  AsyncDeferredProgressCapacity,
  AsyncDeliveryBound,
  AsyncRetransmitPeriod,
  AsyncRoundTimeout,
  AsyncMaximumRoundTimeout,
  AsyncMaximumView,
  AsyncChunkCount

AsyncCompletionTags == {"TimeoutElapsed", "RetransmitElapsed"}
AsyncNetworkKinds ==
  {"Proposal", "PrepareVote", "CommitVote", "PrepareQC", "CommitQC",
   "TimeoutVote", "TimeoutCertificate", "Chunk",
   "CertifiedRequest", "CertifiedResponse",
   "CommitCertificateRequest", "CommitCertificateResponse",
   "NormalJunk", "ProgressJunk", "Noise"}
AsyncDeliveryKinds ==
  {"DeliverProposal", "DeliverVote", "DeliverQC", "DeliverTimeout",
   "DeliverTC", "DeliverChunk",
   "AcceptCertifiedRequest", "AcceptCertifiedResponse",
   "RejectNormal", "RejectProgress"}
AsyncReducerKinds ==
  {"AssembleBody", "BeginProposal", "PersistProposal", "SignProposal",
   "FetchBody", "RebindRetainedBody", "StoreBody", "ValidateBody",
   "BeginPrepare",
   "PersistPrepare", "SignVote", "FormPrepareQC", "BeginObservePrepare",
   "PersistObservePrepare", "BeginLockCommit", "PersistLockCommit",
   "FormCommitQC", "BeginDecision", "PersistDecision", "BeginTimeout",
   "PersistTimeout", "SignTimeout", "BeginInstallTC",
   "PersistInstallTC", "RequestCertifiedBody", "FetchCertifiedBody", "Apply"}
AsyncWorkKinds == AsyncCompletionTags \cup AsyncDeliveryKinds \cup AsyncReducerKinds

\* Rust retains service identities for exactly these eleven reducer input
\* classes.  The TLA command graph is more detailed, so the tracked boundary
\* kinds below project onto the adapter classes rather than pretending that
\* every internal Persist/Sign continuation is another Rust event class.
AsyncCandidateServiceStageClasses ==
  {"LocalProposalReady", "ProposalReceived", "VoteReceived",
   "QuorumCertificateReceived", "TimeoutVoteReceived",
   "TimeoutCertificateReceived", "TimeoutElapsed", "BodyAvailable",
   "BodyStored", "ValidationCompleted", "ApplicationCompleted"}

AsyncCandidateServiceTrackedKinds ==
  {"AssembleBody", "DeliverProposal", "DeliverVote", "DeliverQC",
   "DeliverTimeout", "DeliverTC", "TimeoutElapsed", "FetchBody",
   "RebindRetainedBody", "FetchCertifiedBody", "StoreBody",
   "ValidateBody", "Apply"}

NoAsyncCandidateServiceStage == "NoCandidateServiceStage"

AsyncCandidateServiceStageForKind(kind) ==
  CASE kind = "AssembleBody" -> "LocalProposalReady"
    [] kind = "DeliverProposal" -> "ProposalReceived"
    [] kind = "DeliverVote" -> "VoteReceived"
    [] kind = "DeliverQC" -> "QuorumCertificateReceived"
    [] kind = "DeliverTimeout" -> "TimeoutVoteReceived"
    [] kind = "DeliverTC" -> "TimeoutCertificateReceived"
    [] kind = "TimeoutElapsed" -> "TimeoutElapsed"
    [] kind \in
         {"FetchBody", "RebindRetainedBody", "FetchCertifiedBody"} ->
         "BodyAvailable"
    [] kind = "StoreBody" -> "BodyStored"
    [] kind = "ValidateBody" -> "ValidationCompleted"
    [] kind = "Apply" -> "ApplicationCompleted"
    [] OTHER -> NoAsyncCandidateServiceStage

AsyncCommandClasses == {"Normal", "Progress", "Completion"}
AsyncIoCommandClasses == {"Serve", "Consensus", "Control"}
AsyncControlKinds ==
  {"Proposal", "PrepareVote", "CommitVote", "PrepareQC", "CommitQC",
   "TimeoutVote", "TimeoutCertificate"}
AsyncLeaderWireKinds ==
  AsyncControlKinds \cup {"Chunk", "CertifiedResponse"}
AsyncInstallRetainedControlKinds ==
  {"CommitVote", "PrepareQC", "CommitQC", "TimeoutCertificate"}

NoAsyncChunk == 0
AsyncChunks == 1..AsyncChunkCount
AsyncHeartbeatSubject == CHOOSE subject \in ValidSubjects: TRUE
AsyncUntrustedSource == N

\* `N` counts every configured authenticated peer, not only the current
\* positive-power roster.  Archive servers consume those same finite source
\* identities.  Only malformed or unattributed relay traffic may name the one
\* aggregate untrusted signature owner.
AsyncArchiveServerIds == ValidatorIds
AsyncCertifiedResponseSignatureOwners ==
  AsyncArchiveServerIds \cup {AsyncUntrustedSource}
AsyncIngressSources ==
  AsyncArchiveServerIds \cup {AsyncUntrustedSource}

AsyncReplyRequestKinds ==
  {"CertifiedRequest", "CommitCertificateRequest"}

AsyncReplySourceOrder == [index \in 1..N |-> index - 1]
AsyncReplySourceCapacity == Cardinality(AsyncArchiveServerIds)

\* The same 4N+2 ingress geometry which protects every authenticated source's
\* exact-output occurrence must contain at least one route owner per source.
\* CurrentVoters may be a strict subset of those N peers; zero-power archives
\* do not add another lane or an unbounded source class.  This is a
\* configuration equation, not an eviction fallback.
AsyncReplyExactOutputCorridorCapacity ==
  (AsyncIngressCapacity - 2) \div 4

\* The Rust/Norito maximal structural fixture covers 128 signer indices, a
\* maximum-size PrepareQC aggregate signature, and a maximum-size timeout-vote
\* signature.  Ordinary traffic cannot borrow the separate 64 KiB validator-
\* source reserve, so every valid timeout vote passes the byte gate when that
\* source has no earlier distinct timeout vote queued.  Exact retransmissions
\* coalesce; a newer view waits for fair service to release the sole critical
\* byte owner instead of sharing one logical reserve with unbounded votes.
AsyncValidTimeoutVoteWireByteBound == 4 * 1024
AsyncTimeoutVoteByteReserve == 64 * 1024

AsyncChunkReceipt(node, roundView, subject, chunk) ==
  [node |-> node, view |-> roundView, subject |-> subject, chunk |-> chunk]

AsyncChunkReceiptSet ==
  [node: ValidatorIds, view: Views, subject: Subjects,
   chunk: AsyncChunks]

AsyncHistoricalLockRestartAuthority(node, qc) ==
  [node |-> node, context |-> qc.context,
   view |-> qc.view, subject |-> qc.subject]

AsyncHistoricalLockRestartAuthoritySet ==
  [node: ValidatorIds, context: ContextRecords,
   view: Views, subject: Subjects]

AsyncRunnerCycleBudget ==
  AsyncQueueCapacity + 2 * AsyncIngressCapacity + 3

\* A protected occurrence has at most AsyncQueueCapacity same-class positions
\* ahead of or equal to it.  The cyclic cursor needs at most three serialized
\* dispatches per such position.  The runner-cycle term conservatively uses
\* the scalar invariant's coarse Q+I budget even in Local phase, giving
\* Q+2I+3 through the subsequent Ingress phase and serialized dispatch.
AsyncRuntimeCycleBudget ==
  3 * AsyncQueueCapacity * AsyncRunnerCycleBudget

AsyncIoDrainBudget ==
  AsyncIoAuxCapacity + AsyncIoWorkCapacity + 1

AsyncDeferredDrainBudget ==
  AsyncDeferredNormalCapacity + AsyncDeferredProgressCapacity
    + AsyncCompletionReserve

AsyncRetainedControlBudget == 7 * N

AsyncRetainedProposalChunkBudget == N * AsyncChunkCount

AsyncActiveCertifiedRequestBudget == N

AsyncActiveCommitRequestBudget == N

AsyncActiveRequestBudget ==
  AsyncActiveCertifiedRequestBudget + AsyncActiveCommitRequestBudget

AsyncServeLifecycleFamilyBudget ==
  AsyncActiveRequestBudget * Cardinality(Phases)

AsyncRetransmitEmissionBudget ==
  AsyncRetainedControlBudget
    + AsyncRetainedProposalChunkBudget
    + AsyncActiveRequestBudget

AsyncOneWayTransportBudget ==
  AsyncDeliveryBound
    * (AsyncIngressCapacity + AsyncRuntimeCycleBudget
         + AsyncRetransmitEmissionBudget + 1)

AsyncProposalPipelineBudget ==
  4 * N
    * (AsyncRuntimeCycleBudget + AsyncIoDrainBudget
         + AsyncDeferredDrainBudget + AsyncChunkCount + 8)

AsyncCertifiedRecoveryBudget ==
  2 * AsyncOneWayTransportBudget
    + 2 * AsyncIoDrainBudget * AsyncDeliveryBound
    + 3 * AsyncRuntimeCycleBudget * AsyncDeliveryBound

AsyncWorstCaseServiceBudget ==
  AsyncProposalPipelineBudget * AsyncDeliveryBound
    + AsyncCertifiedRecoveryBudget
    + 4 * AsyncRetransmitPeriod
    + AsyncProgressReserve + AsyncCompletionReserve

(***************************************************************************
The lifecycle table is scheduler-local bookkeeping, so its capacity is
derived entirely from the configured roster and the queues which can retain
an admitted candidate.  The deliberately coarse causal term covers the
largest three-child reducer continuation plus one pending root for every
ordinary runtime position.  A retired entry remains reserved exactly while a
transient marker, terminal tombstone, or durable replay source owns it.  It is
compacted after strict view/context/height/Decision obsolescence, loss of that
durable source, or an ignored no-state-change producer episode leaves no such
owner.  Thus both serviced and restart-replayable roots remain dormant with
their ordinal instead of relying on eviction.  The reviewed bound is derived
from configured queue
and roster geometry.  The adequate-leader proof uses the concrete constructor
arms only to freeze causal identity; capacity is established independently by
the physical/service/durable owner-token injection below, not by assuming a
cardinality bound for the constructor universe.
***************************************************************************)
AsyncSemanticIngressLifecycleCapacity ==
  AsyncIngressCapacity + 2 * N

AsyncServicedCandidateLifecycleCapacity ==
  AsyncSemanticIngressLifecycleCapacity
    + 2 * AsyncDeferredNormalCapacity
    + AsyncDeferredProgressCapacity

AsyncCausalCandidateLifecycleCapacity ==
  3 * AsyncQueueCapacity

AsyncDormantDurableLifecycleCapacity == 8

AsyncActiveCandidateLifecycleCapacity ==
  AsyncQueueCapacity
    + AsyncCausalCandidateLifecycleCapacity
    + AsyncIoWorkCapacity
    + AsyncDormantDurableLifecycleCapacity

AsyncReviewedActiveCandidateLifecycleCapacity ==
  AsyncActiveCandidateLifecycleCapacity

\* Ordinary roots consume the reviewed queue/service store.  One additional
\* slot is reserved exclusively for the clock owner which crosses its
\* deadline; when BeginTimeout is materialized, that same reservation becomes
\* its candidate record.  Ordinary ingress can therefore never fill the
\* table in a way which prevents an already-due timeout from freezing its
\* immutable position.
AsyncCandidateLifecycleOrdinaryCapacity ==
  AsyncServicedCandidateLifecycleCapacity
    + AsyncActiveCandidateLifecycleCapacity

AsyncCandidateLifecyclePerNodeCapacity ==
  AsyncCandidateLifecycleOrdinaryCapacity + 1

AsyncCandidateLifecycleCapacity ==
  N * AsyncCandidateLifecyclePerNodeCapacity

AsyncTerminalCandidateLifecycleCapacity ==
  AsyncSemanticIngressLifecycleCapacity

\* Each immutable lifecycle slot can retain at most one serviced identity for
\* each projected adapter event class.  The source-level coverage and
\* collision theorems below connect the model's detailed boundary commands to
\* Rust's exact eleven-class `ServicedCandidateStage` carrier.  Neither side
\* introduces a wire field or deployment knob.
AsyncCandidateServiceStageCapacity ==
  Cardinality(AsyncCandidateServiceStageClasses)

AsyncCandidateServiceRecordCapacity ==
  N * AsyncCandidateLifecyclePerNodeCapacity
    * AsyncCandidateServiceStageCapacity

THEOREM AsyncCandidateServiceStageCarrierHasExactlyElevenClasses ==
  AsyncCandidateServiceStageCapacity = 11
BY SMT DEF AsyncCandidateServiceStageCapacity,
           AsyncCandidateServiceStageClasses

THEOREM AsyncCandidateServiceTrackedKindProjectionIsCovered ==
  \A kind \in AsyncCandidateServiceTrackedKinds:
    AsyncCandidateServiceStageForKind(kind)
      \in AsyncCandidateServiceStageClasses
BY SMT DEF AsyncCandidateServiceTrackedKinds,
           AsyncCandidateServiceStageForKind,
           AsyncCandidateServiceStageClasses,
           NoAsyncCandidateServiceStage

THEOREM AsyncCandidateLifecycleCapacityDerivesFromReviewedOwners ==
  AsyncConfiguration
    => /\ AsyncTerminalCandidateLifecycleCapacity
             = AsyncSemanticIngressLifecycleCapacity
       /\ AsyncCandidateLifecycleOrdinaryCapacity
            = AsyncServicedCandidateLifecycleCapacity
                + AsyncActiveCandidateLifecycleCapacity
       /\ AsyncServicedCandidateLifecycleCapacity
            = AsyncTerminalCandidateLifecycleCapacity
                + 2 * AsyncDeferredNormalCapacity
                + AsyncDeferredProgressCapacity
       /\ AsyncCandidateLifecyclePerNodeCapacity
            = AsyncCandidateLifecycleOrdinaryCapacity + 1
BY SMT DEF AsyncCandidateLifecyclePerNodeCapacity,
           AsyncCandidateLifecycleOrdinaryCapacity,
           AsyncTerminalCandidateLifecycleCapacity,
           AsyncSemanticIngressLifecycleCapacity,
           AsyncServicedCandidateLifecycleCapacity,
           AsyncActiveCandidateLifecycleCapacity,
           AsyncReviewedActiveCandidateLifecycleCapacity

THEOREM AsyncCandidateServiceRecordCapacityMatchesConfiguredGeometry ==
  AsyncConfiguration
    => AsyncCandidateServiceRecordCapacity
         = N
             * (AsyncSemanticIngressLifecycleCapacity
                  + 2 * AsyncDeferredNormalCapacity
                  + AsyncDeferredProgressCapacity
                  + 4 * AsyncQueueCapacity
                  + AsyncIoWorkCapacity
                  + AsyncDormantDurableLifecycleCapacity
                  + 1)
             * 11
BY AsyncCandidateServiceStageCarrierHasExactlyElevenClasses,
   SMT
   DEF AsyncCandidateServiceRecordCapacity,
       AsyncCandidateServiceStageCapacity,
       AsyncCandidateLifecyclePerNodeCapacity,
       AsyncCandidateLifecycleOrdinaryCapacity,
       AsyncServicedCandidateLifecycleCapacity,
       AsyncActiveCandidateLifecycleCapacity,
       AsyncCausalCandidateLifecycleCapacity

\* Production uses a linearly growing timeout.  The arithmetic is saturated
\* only where the implementation's duration representation saturates; the
\* liveness configuration below requires the complete post-GST service budget
\* to remain strictly below that representational ceiling.
AsyncLinearViewTimeout(roundView) ==
  AsyncRoundTimeout * (roundView + 1)

AsyncViewTimeout(roundView) ==
  IF AsyncLinearViewTimeout(roundView) <= AsyncMaximumRoundTimeout
  THEN AsyncLinearViewTimeout(roundView)
  ELSE AsyncMaximumRoundTimeout

AsyncServiceBoundRepresentable ==
  /\ AsyncWorstCaseServiceBudget < AsyncMaximumRoundTimeout
  /\ AsyncWorstCaseServiceBudget <= AsyncMaximumView

AsyncConfiguration ==
  /\ AsyncQueueCapacity \in Nat \ {0}
  /\ AsyncProgressReserve \in Nat \ {0}
  /\ AsyncCompletionReserve \in Nat \ {0}
  /\ AsyncProgressReserve + AsyncCompletionReserve < AsyncQueueCapacity
  /\ AsyncCompletionReserve >= 1
  /\ AsyncIngressCapacity \in Nat \ {0}
  /\ AsyncIngressCapacity >= 4 * N + 2
  /\ AsyncReplySourceCapacity = N
  /\ AsyncReplyExactOutputCorridorCapacity >=
       AsyncReplySourceCapacity
  /\ AsyncValidTimeoutVoteWireByteBound <= AsyncTimeoutVoteByteReserve
  /\ AsyncIoAuxCapacity \in Nat \ {0}
  /\ AsyncIoWorkCapacity \in Nat \ {0}
  /\ AsyncIoWorkCapacity <= AsyncCompletionReserve
  /\ AsyncDeferredNormalCapacity \in Nat \ {0}
  /\ AsyncDeferredProgressCapacity \in Nat \ {0}
  /\ AsyncDeferredProgressCapacity >= 2 * N + 3
  /\ AsyncDeliveryBound \in Nat \ {0}
  /\ AsyncRetransmitPeriod \in Nat \ {0}
  /\ AsyncRoundTimeout \in Nat \ {0}
  /\ AsyncRoundTimeout >= 5
  /\ AsyncRetransmitPeriod = AsyncRoundTimeout \div 5
  /\ AsyncMaximumRoundTimeout \in Nat \ {0}
  /\ AsyncRoundTimeout <= AsyncMaximumRoundTimeout
  /\ AsyncMaximumView \in Nat
  /\ AsyncChunkCount \in Nat \ {0}
  /\ AsyncServiceBoundRepresentable

AsyncBodyEnvelope(recipient, blockHeight, roundView, subject, chunk, nonce) ==
  [recipient |-> recipient, height |-> blockHeight, view |-> roundView,
   subject |-> subject, chunk |-> chunk, nonce |-> nonce]

NoAsyncItem ==
  [kind |-> "NoItem", source |-> 0,
   envelope |-> AsyncBodyEnvelope(0, 0, 0, AsyncHeartbeatSubject,
                                  NoAsyncChunk, 0)]

AsyncBodyEnvelopeSet ==
  [recipient: ValidatorIds, height: Heights, view: Views,
   subject: Subjects, chunk: 0..AsyncChunkCount,
   nonce: 0..(AsyncIngressCapacity - 1)]

AsyncReplyRequestEnvelopeSet ==
  [recipient: ValidatorIds, height: Heights, view: Views,
   subject: Subjects, chunk: {NoAsyncChunk},
   nonce: 0..(AsyncIngressCapacity - 1)]

\* Commit-certificate discovery emits one canonical request per requester;
\* fanout routing is outside its signed identity.  The heartbeat subject and
\* zero nonce are the finite stand-ins for that exact wire request.
AsyncCommitCertificateRequestEnvelopeSet ==
  [recipient: ValidatorIds, height: Heights, view: Views,
   subject: {AsyncHeartbeatSubject}, chunk: {NoAsyncChunk}, nonce: {0}]

AsyncNetworkItem(kind, source, envelope) ==
  [kind |-> kind, source |-> source, envelope |-> envelope]

AsyncCertifiedRequestPreimage(requester, qc) ==
  [round |-> [height |-> qc.context.height, view |-> qc.view],
   subject |-> qc.subject,
   certificate |-> qc,
   requester |-> requester]

AsyncCertifiedRequestSignature(requester, qc, signatureNonce) ==
  [signer |-> requester,
   preimage |-> AsyncCertifiedRequestPreimage(requester, qc),
   nonce |-> signatureNonce]

AsyncCertifiedSignedRequest(requester, qc, signatureNonce) ==
  [preimage |-> AsyncCertifiedRequestPreimage(requester, qc),
   signature |->
     AsyncCertifiedRequestSignature(requester, qc, signatureNonce)]

AsyncCertifiedRequestHashOf(requester, qc, signatureNonce) ==
  [exactSignedRequest |->
     AsyncCertifiedSignedRequest(requester, qc, signatureNonce)]

AsyncCertifiedRequestEnvelope(route, requester, qc, signatureNonce) ==
  [recipient |-> route,
   height |-> qc.context.height,
   view |-> qc.view,
   subject |-> qc.subject,
   requester |-> requester,
   certificate |-> qc,
   signatureNonce |-> signatureNonce]

AsyncCertifiedRequestHash(request) ==
  AsyncCertifiedRequestHashOf(
    request.envelope.requester,
    request.envelope.certificate,
    request.envelope.signatureNonce)

AsyncCertifiedRequestItems ==
  {AsyncNetworkItem(
     "CertifiedRequest", requester,
     AsyncCertifiedRequestEnvelope(route, requester, qc, signatureNonce)):
     requester \in ValidatorIds,
     route \in AsyncArchiveServerIds,
     qc \in QcRecordSet,
     signatureNonce \in 0..(AsyncIngressCapacity - 1)}

AsyncCertifiedRequestHashes ==
  {AsyncCertifiedRequestHash(request):
     request \in AsyncCertifiedRequestItems}

AsyncCommitCertificateRequestItems ==
  {AsyncNetworkItem("CommitCertificateRequest", source, envelope):
     source \in ValidatorIds,
     envelope \in AsyncCommitCertificateRequestEnvelopeSet}

(***************************************************************************
Reply-route ownership keeps the physical recipient as its separate `owner`.
Certified-body request semantics therefore project only the exact signed
request hash, so every physical archive fanout occurrence competes for the
same semantic attempt.  Commit-certificate discovery retains its established
full-envelope semantics.
***************************************************************************)
AsyncReplySemanticIdentity(kind, envelope) ==
  IF kind = "CertifiedRequest"
  THEN [kind |-> kind,
        requestHash |->
          AsyncCertifiedRequestHashOf(
            envelope.requester, envelope.certificate,
            envelope.signatureNonce)]
  ELSE [kind |-> kind, envelope |-> envelope]

AsyncReplySemanticIdentities ==
  {AsyncReplySemanticIdentity(
     "CertifiedRequest", request.envelope):
     request \in AsyncCertifiedRequestItems}
  \cup
  {AsyncReplySemanticIdentity("CommitCertificateRequest", envelope):
     envelope \in AsyncCommitCertificateRequestEnvelopeSet}

(***************************************************************************
Serve ownership is internal to one archive process.  The exact signed request
identity is route-neutral for certified-body recovery, while the archive
process remains an explicit owner coordinate.  Consequently retransmission to
the same archive retains one logical identity, but the same fanout request at
two different archives owns two independent bounded Serve lifecycles.
***************************************************************************)
AsyncServeLogicalRequestIdentity(node, request) ==
  [owner |-> node,
   request |->
     AsyncReplySemanticIdentity(request.kind, request.envelope)]

AsyncServeLogicalRequestIdentities ==
  {[owner |-> node, request |-> requestIdentity]:
     node \in ValidatorIds,
     requestIdentity \in AsyncReplySemanticIdentities}

AsyncServeLifecycleFamily(node, request) ==
  IF request.kind = "CertifiedRequest"
  THEN [owner |-> node,
        kind |-> request.kind,
        context |-> request.envelope.certificate.context,
        requester |-> request.source,
        phase |-> request.envelope.certificate.phase]
  ELSE [owner |-> node,
        kind |-> request.kind,
        context |-> context,
        requester |-> request.source,
        phase |-> "Commit"]

AsyncServeLifecycleFamilies ==
  [owner: ValidatorIds,
   kind: AsyncReplyRequestKinds,
   context: ContextRecords,
   requester: ValidatorIds,
   phase: Phases]

AsyncServeRequestView(request) == request.envelope.view

AsyncServeLifecycleCoordinates(node, identity, family, roundView) ==
  \E request \in
       AsyncCertifiedRequestItems \cup
         AsyncCommitCertificateRequestItems:
    /\ identity = AsyncServeLogicalRequestIdentity(node, request)
    /\ family = AsyncServeLifecycleFamily(node, request)
    /\ roundView = AsyncServeRequestView(request)

(***************************************************************************
The production request hash covers the exact signed request: round, subject,
the full frozen QC, requester, and signature.  The physical archive route is
an outer fanout coordinate and is deliberately absent from that projection.
The finite signature nonce distinguishes arbitrary signed-request witnesses;
honest constructors use zero.  The logical registration key below remains the
separate Rust conflict index `(round, subject, requester)`.
***************************************************************************)
AsyncCertifiedRequestRegistrationIdentity(request) ==
  [context |-> request.envelope.certificate.context,
   requester |-> request.source,
   height |-> request.envelope.height,
   view |-> request.envelope.view,
   subject |-> request.envelope.subject]

(***************************************************************************
Rust keeps a logical conflict index beside the exact-hash lookup: one logical
`(round, subject, requester)` key may own several physical route aliases, but
all such aliases must name the same exact signed request hash.  These
set-level predicates make that constraint proof-visible without adding state.
***************************************************************************)
AsyncCertifiedRequestsIn(items) ==
  {request \in items: request.kind = "CertifiedRequest"}

AsyncCertifiedRequestAliasesCompatible(left, right) ==
  \/ AsyncCertifiedRequestRegistrationIdentity(left)
       # AsyncCertifiedRequestRegistrationIdentity(right)
  \/ AsyncCertifiedRequestHash(left) = AsyncCertifiedRequestHash(right)

AsyncCertifiedRequestLogicalIndexConsistent(items) ==
  \A left, right \in AsyncCertifiedRequestsIn(items):
    AsyncCertifiedRequestAliasesCompatible(left, right)

AsyncCertifiedRequestSetsCompatible(existing, incoming) ==
  \A left \in AsyncCertifiedRequestsIn(existing),
     right \in AsyncCertifiedRequestsIn(incoming):
    AsyncCertifiedRequestAliasesCompatible(left, right)

AsyncCommitCertificateRequestRegistrationIdentity(request) ==
  [context |-> context,
   requester |-> request.source,
   height |-> request.envelope.height]

AsyncCertifiedCitedResponder(request) ==
  IF request.envelope.certificate.signers # {}
  THEN CHOOSE signer \in request.envelope.certificate.signers: TRUE
  ELSE request.envelope.requester

AsyncCertifiedResponseEnvelope(
    request, archiveServer, citedResponder, signatureOwner) ==
  [recipient |-> request.envelope.requester,
   height |-> request.envelope.height,
   view |-> request.envelope.view,
   subject |-> request.envelope.subject,
   requestHash |-> AsyncCertifiedRequestHash(request),
   archiveServer |-> archiveServer,
   citedResponder |-> citedResponder,
   signatureOwner |-> signatureOwner]

AsyncCommitCertificateResponseEnvelope(request, qc) ==
  [recipient |-> request.source, request |-> request, qc |-> qc]

(***************************************************************************
The aggregate untrusted ingress lane owns a TransportCompletion slot.  Its
synthetic certified response remains structurally typed, but its verified
signature owner is the untrusted aggregate rather than the claimed archive
server.  It therefore cannot authenticate an archive claim even if its exact
request hash happens to match an outstanding request.
***************************************************************************)
AsyncUntrustedCompletionQcWitness ==
  CHOOSE qc \in QcRecordSet: TRUE

AsyncUntrustedCompletionRequestWitness(recipient, nonce) ==
  AsyncNetworkItem(
    "CertifiedRequest", recipient,
    AsyncCertifiedRequestEnvelope(
      recipient, recipient, AsyncUntrustedCompletionQcWitness, nonce))

AsyncUntrustedTransportCompletionItem(kind, recipient, nonce) ==
  LET bodyEnvelope ==
        AsyncBodyEnvelope(recipient, context.height, nodeView[recipient],
                          AsyncHeartbeatSubject, NoAsyncChunk, nonce)
      request ==
        AsyncUntrustedCompletionRequestWitness(recipient, nonce)
  IN IF kind = "CertifiedResponse"
     THEN AsyncNetworkItem(
            "CertifiedResponse", AsyncUntrustedSource,
            AsyncCertifiedResponseEnvelope(
              request, recipient, recipient, AsyncUntrustedSource))
     ELSE AsyncNetworkItem(kind, AsyncUntrustedSource, bodyEnvelope)

AsyncNetworkItems ==
  {AsyncNetworkItem("Proposal", envelope.proposal.proposer, envelope):
     envelope \in ProposalEnvelopeSet}
  \cup {AsyncNetworkItem(
           IF envelope.vote.phase = "Prepare" THEN "PrepareVote"
           ELSE "CommitVote",
           envelope.vote.signer, envelope): envelope \in VoteEnvelopeSet}
  \cup {AsyncNetworkItem(
           IF envelope.qc.phase = "Prepare" THEN "PrepareQC" ELSE "CommitQC",
           source, envelope):
          source \in ValidatorIds, envelope \in QcEnvelopeSet}
  \cup {AsyncNetworkItem("TimeoutVote", envelope.vote.signer, envelope):
          envelope \in TimeoutEnvelopeSet}
  \cup {AsyncNetworkItem("TimeoutCertificate", source, envelope):
          source \in ValidatorIds, envelope \in TcEnvelopeSet}
  \cup AsyncCertifiedRequestItems
  \cup AsyncCommitCertificateRequestItems
  \cup {AsyncNetworkItem(kind, source, envelope):
          kind \in {"NormalJunk", "ProgressJunk"},
          source \in ValidatorIds, envelope \in AsyncBodyEnvelopeSet}
  \cup {AsyncNetworkItem("Chunk", source, envelope):
          source \in AsyncIngressSources,
          envelope \in AsyncBodyEnvelopeSet}
  \cup {AsyncNetworkItem(
           "CertifiedResponse", source,
           AsyncCertifiedResponseEnvelope(
             request, archiveServer,
             AsyncCertifiedCitedResponder(request),
             archiveServer)):
          source \in AsyncIngressSources,
          archiveServer \in AsyncArchiveServerIds,
          request \in AsyncCertifiedRequestItems}
  \cup {AsyncUntrustedTransportCompletionItem(
           "CertifiedResponse", recipient, nonce):
          recipient \in ValidatorIds,
          nonce \in 0..(AsyncIngressCapacity - 1)}
  \cup {AsyncNetworkItem(
          "CommitCertificateResponse", source,
           AsyncCommitCertificateResponseEnvelope(request, qc)):
          source \in AsyncIngressSources,
          request \in AsyncCommitCertificateRequestItems,
          qc \in QcRecordSet}
  \cup {AsyncNetworkItem("Noise", source, envelope):
          source \in AsyncIngressSources, envelope \in AsyncBodyEnvelopeSet}

(***************************************************************************
Bounded internal service identity for retained control traffic.

The exact authenticated wire tuple remains the retry identity, but production
must not retain an ever-growing set of those tuples.  Instead every recipient,
authenticated source, and protocol-owner class has one fixed service slot.
Vote and timeout-vote slots name their signer; the other classes name their
kind.  Cross-source relays deliberately remain distinct slots because the
physical scheduler charges those source lanes separately.

A slot stores the first immutable local admission ordinal for one
context/height/view owner.  A strictly newer view may replace the slot.  An
exact retry coalesces with its owner, and every later same/lower-view identity
drains without replacing it.  Successful reducer service flips one monotone
`consumed` bit.  A fresh successor-height Async instance starts with an empty
table, so storage is roster/class bounded rather than history bounded.
***************************************************************************)
AsyncLeaderWireServiceIdentity(item) ==
  [kind |-> item.kind,
   source |-> item.source,
   envelope |-> item.envelope]

AsyncControlServiceProtocolOwner(kind, source) ==
  IF kind \in {"PrepareVote", "CommitVote", "TimeoutVote"}
  THEN source
  ELSE kind

AsyncControlServiceSlot(recipient, source, kind) ==
  [recipient |-> recipient,
   source |-> source,
   kind |-> kind,
   owner |-> AsyncControlServiceProtocolOwner(kind, source)]

AsyncControlServiceSlotSet ==
  {AsyncControlServiceSlot(recipient, source, kind):
     recipient \in ValidatorIds,
     source \in ValidatorIds,
     kind \in AsyncControlKinds}

AsyncControlItemContext(item) ==
  CASE item.kind = "Proposal" -> item.envelope.proposal.context
    [] item.kind \in {"PrepareVote", "CommitVote", "TimeoutVote"} ->
         item.envelope.vote.context
    [] item.kind \in {"PrepareQC", "CommitQC"} ->
         item.envelope.qc.context
    [] item.kind = "TimeoutCertificate" -> item.envelope.tc.context

AsyncControlItemHeight(item) == AsyncControlItemContext(item).height

AsyncControlItemView(item) ==
  CASE item.kind = "Proposal" -> item.envelope.proposal.view
    [] item.kind \in {"PrepareVote", "CommitVote", "TimeoutVote"} ->
         item.envelope.vote.view
    [] item.kind \in {"PrepareQC", "CommitQC"} -> item.envelope.qc.view
    [] item.kind = "TimeoutCertificate" -> item.envelope.tc.view

AsyncControlItemSubject(item) ==
  CASE item.kind = "Proposal" -> item.envelope.proposal.subject
    [] item.kind \in {"PrepareVote", "CommitVote"} ->
         item.envelope.vote.subject
    [] item.kind = "TimeoutVote" -> item.envelope.vote.highSubject
    [] item.kind \in {"PrepareQC", "CommitQC"} -> item.envelope.qc.subject
    [] item.kind = "TimeoutCertificate" -> NoSubject

AsyncControlServiceRecord(item, ordinal, consumed) ==
  [slot |->
     AsyncControlServiceSlot(
       item.envelope.recipient, item.source, item.kind),
   context |-> AsyncControlItemContext(item),
   height |-> AsyncControlItemHeight(item),
   view |-> AsyncControlItemView(item),
   subject |-> AsyncControlItemSubject(item),
   phase |-> item.kind,
   identity |-> AsyncLeaderWireServiceIdentity(item),
   ordinal |-> ordinal,
   consumed |-> consumed]

AsyncControlServiceRecordSet ==
  {AsyncControlServiceRecord(item, ordinal, consumed):
     item \in {wire \in AsyncNetworkItems:
                 wire.kind \in AsyncControlKinds},
     ordinal \in Nat \ {0},
     consumed \in BOOLEAN}

THEOREM AsyncLeaderWireExactRetryRetainsServiceIdentity ==
  \A left, right:
    /\ left.kind = right.kind
    /\ left.source = right.source
    /\ left.envelope = right.envelope
    => AsyncLeaderWireServiceIdentity(left)
         = AsyncLeaderWireServiceIdentity(right)
BY DEF AsyncLeaderWireServiceIdentity

THEOREM AsyncControlServiceSlotCarrierIsRosterClassBounded ==
  Cardinality(AsyncControlServiceSlotSet)
    <= Cardinality(ValidatorIds)
         * Cardinality(ValidatorIds)
         * Cardinality(AsyncControlKinds)
BY FS_Product, FS_Image, FS_Subset, Isa
   DEF AsyncControlServiceSlotSet

(***************************************************************************
Route-neutral command payloads are defined before candidate construction so
the first local admission can freeze one normalized causal origin.  The
normalization is semantic, not a wire rewrite: authenticated bytes remain the
ingress authority, while aggregate relay choice and quorum-superset encoding
cannot manufacture a new logical origin after admission.
***************************************************************************)
AsyncCandidateQcSemanticPayload(qc) ==
  CertificateRefOf(qc)

AsyncCandidatePrepareQcSemanticPayload(qc) ==
  IF qc = NoPrepareQC
  THEN NoPrepareQC
  ELSE AsyncCandidateQcSemanticPayload(qc)

AsyncCandidateVoteSemanticPayload(vote) ==
  [context |-> vote.context,
   height |-> vote.height,
   view |-> vote.view,
   phase |-> vote.phase,
   subject |-> vote.subject,
   signer |-> vote.signer]

AsyncCandidateTimeoutVoteSemanticPayload(vote) ==
  [context |-> vote.context,
   height |-> vote.height,
   view |-> vote.view,
   signer |-> vote.signer,
   highestPrepareQc |->
     AsyncCandidatePrepareQcSemanticPayload(vote.highestPrepareQc),
   highRank |-> vote.highRank,
   highSubject |-> vote.highSubject]

AsyncCandidateTcSemanticPayload(tc) ==
  IF tc = NoTimeoutCertificate
  THEN NoTimeoutCertificate
  ELSE [context |-> tc.context,
        height |-> tc.height,
        view |-> tc.view,
        highestPrepareQc |->
          AsyncCandidatePrepareQcSemanticPayload(
            tc.highestPrepareQc)]

AsyncCandidateProposalSemanticPayload(proposal) ==
  [context |-> proposal.context,
   height |-> proposal.height,
   view |-> proposal.view,
   subject |-> proposal.subject,
   proposer |-> proposal.proposer,
   timeoutCertificate |->
     AsyncCandidateTcSemanticPayload(proposal.timeoutCertificate),
   highestPrepareQc |->
     AsyncCandidatePrepareQcSemanticPayload(
       proposal.highestPrepareQc),
   justifyRank |-> proposal.justifyRank,
   justifySubject |-> proposal.justifySubject]

AsyncCandidateCertifiedRequestHashSemanticPayload(requestHash) ==
  LET signed == requestHash.exactSignedRequest
      preimage == signed.preimage
      signature == signed.signature
  IN [round |-> preimage.round,
      subject |-> preimage.subject,
      certificate |->
        AsyncCandidateQcSemanticPayload(preimage.certificate),
      requester |-> preimage.requester,
      signer |-> signature.signer,
      signatureNonce |-> signature.nonce]

AsyncCandidateCertifiedRequestItemSemanticPayload(item) ==
  [recipient |-> item.envelope.recipient,
   height |-> item.envelope.height,
   view |-> item.envelope.view,
   subject |-> item.envelope.subject,
   requester |-> item.envelope.requester,
   certificate |->
     AsyncCandidateQcSemanticPayload(
       item.envelope.certificate),
   signatureNonce |-> item.envelope.signatureNonce]

AsyncCandidateCommitRequestItemSemanticPayload(item) ==
  [kind |-> item.kind,
   source |-> item.source,
   recipient |-> item.envelope.recipient,
   height |-> item.envelope.height,
   view |-> item.envelope.view,
   subject |-> item.envelope.subject,
   chunk |-> item.envelope.chunk,
   nonce |-> item.envelope.nonce]

AsyncRouteNeutralCandidateItem(item) ==
  IF item = NoAsyncItem
  THEN [kind |-> "NoItem",
        source |-> 0,
        payload |-> NoAsyncItem]
  ELSE [kind |-> item.kind,
        \* Aggregate certificate relays and recovery responses name one
        \* semantic occurrence independently of their physical relay.
        source |->
          IF item.kind
               \in {"PrepareQC", "CommitQC", "TimeoutCertificate",
                    "CertifiedResponse", "CommitCertificateResponse"}
          THEN AsyncUntrustedSource
          ELSE item.source,
        payload |->
          CASE item.kind = "Proposal" ->
                 [recipient |-> item.envelope.recipient,
                  proposal |->
                    AsyncCandidateProposalSemanticPayload(
                      item.envelope.proposal)]
            [] item.kind \in {"PrepareVote", "CommitVote"} ->
                 [recipient |-> item.envelope.recipient,
                  vote |->
                    AsyncCandidateVoteSemanticPayload(
                      item.envelope.vote)]
            [] item.kind \in {"PrepareQC", "CommitQC"} ->
                 [recipient |-> item.envelope.recipient,
                  qc |->
                    AsyncCandidateQcSemanticPayload(
                      item.envelope.qc)]
            [] item.kind = "TimeoutVote" ->
                 [recipient |-> item.envelope.recipient,
                  vote |->
                    AsyncCandidateTimeoutVoteSemanticPayload(
                      item.envelope.vote)]
            [] item.kind = "TimeoutCertificate" ->
                 [recipient |-> item.envelope.recipient,
                  tc |->
                    AsyncCandidateTcSemanticPayload(
                      item.envelope.tc)]
            [] item.kind = "CertifiedRequest" ->
                 AsyncCandidateCertifiedRequestItemSemanticPayload(item)
            [] item.kind = "CommitCertificateRequest" ->
                 AsyncCandidateCommitRequestItemSemanticPayload(item)
            [] item.kind = "CertifiedResponse" ->
                 [recipient |-> item.envelope.recipient,
                  height |-> item.envelope.height,
                  view |-> item.envelope.view,
                  subject |-> item.envelope.subject,
                  requestHash |->
                    AsyncCandidateCertifiedRequestHashSemanticPayload(
                      item.envelope.requestHash),
                  archiveServer |-> item.envelope.archiveServer,
                  citedResponder |-> item.envelope.citedResponder,
                  signatureOwner |-> item.envelope.signatureOwner]
            [] item.kind = "CommitCertificateResponse" ->
                 [recipient |-> item.envelope.recipient,
                  request |->
                    AsyncCandidateCommitRequestItemSemanticPayload(
                      item.envelope.request),
                  qc |->
                    AsyncCandidateQcSemanticPayload(
                      item.envelope.qc)]
            [] item.kind \in {"NormalJunk", "ProgressJunk"} ->
                 \* Authenticated junk is an ignored producer episode, not
                 \* one logical lifecycle per attacker-chosen nonce.  The
                 \* outer source and kind still isolate the finite roster
                 \* owners; envelope bytes remain available on candidate.item
                 \* to the rejecting reducer but cannot replenish the service
                 \* marker or lifecycle tables.
                 [recipient |-> item.envelope.recipient]
            [] OTHER -> item.envelope]

AsyncRouteNeutralCandidateEvidence(evidence) ==
  IF evidence = NoAsyncItem
  THEN [kind |-> "NoEvidence", payload |-> NoAsyncItem]
  ELSE IF evidence \in AsyncNetworkItems
       THEN [kind |-> "NetworkItem",
             payload |-> AsyncRouteNeutralCandidateItem(evidence)]
       ELSE IF evidence \in ProposalRecordSet
            THEN [kind |-> "Proposal",
                  payload |->
                    AsyncCandidateProposalSemanticPayload(evidence)]
            ELSE IF evidence \in VoteRecordSet
                 THEN [kind |-> "Vote",
                       payload |->
                         AsyncCandidateVoteSemanticPayload(evidence)]
                 ELSE IF evidence \in TimeoutVoteRecordSet
                      THEN [kind |-> "TimeoutVote",
                            payload |->
                              AsyncCandidateTimeoutVoteSemanticPayload(
                                evidence)]
                      ELSE IF evidence \in QcRecordSet
                           THEN [kind |-> "QC",
                                 payload |->
                                   AsyncCandidateQcSemanticPayload(
                                     evidence)]
                           ELSE IF evidence \in TcRecordSet
                                THEN [kind |-> "TC",
                                      payload |->
                                        AsyncCandidateTcSemanticPayload(
                                          evidence)]
                                ELSE [kind |-> "Body",
                                      payload |-> evidence]

\* Exact evidence survives queue/pool epochs independently of the delivery
\* envelope.  Durable restart replay therefore names the authenticated Core
\* record which caused the work, while ordinary ingress keeps the exact wire
\* item as both its payload and evidence.
AsyncEvidenceSet ==
  AsyncNetworkItems \cup {NoAsyncItem}
    \cup ProposalRecordSet \cup VoteRecordSet \cup TimeoutVoteRecordSet
    \cup QcRecordSet \cup TcRecordSet \cup BodyRecordSet

AsyncRouteNeutralCandidateItemSet ==
  {AsyncRouteNeutralCandidateItem(item):
     item \in AsyncNetworkItems \cup {NoAsyncItem}}

AsyncRouteNeutralCandidateEvidenceSet ==
  {AsyncRouteNeutralCandidateEvidence(evidence):
     evidence \in AsyncEvidenceSet}

(***************************************************************************
Immutable internal causal origin.

The value is minted deterministically by the first local candidate admission.
It freezes the normalized root packet/authority and the root
target/context/height/leader/view/subject/phase coordinates.  A successor may
rewrite its mutable work view, evidence, class, body stage, or consumer
generation, but it must carry this record unchanged.  Transport retries
therefore reproduce one origin, while a later unrelated packet cannot splice
itself into the admitted lifecycle.  This record is scheduler-local ghost
metadata: it is neither a wire field nor a Norito payload.
***************************************************************************)
AsyncCandidateCausalOrigin(
    kind, node, blockHeight, roundView, subject, item,
    consumerContext, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity) ==
  [target |-> node,
   context |-> consumerContext,
   height |-> blockHeight,
   leader |-> Leader(consumerContext, roundView),
   view |-> roundView,
   subject |-> subject,
   phase |-> kind,
   owner |-> node,
   kind |-> "CausalOrigin",
   payload |->
     [workKind |-> kind,
      item |-> AsyncRouteNeutralCandidateItem(item),
      authority |-> AsyncRouteNeutralCandidateEvidence(evidence),
      body |-> bodyIdentity,
      manifest |-> manifestIdentity,
      commitment |-> commitmentIdentity]]

AsyncCandidateCausalOriginSet ==
  [target: ValidatorIds,
   context: ContextRecords,
   height: Heights,
   leader: ValidatorIds,
   view: Views,
   subject: SubjectOrNone,
   phase: AsyncWorkKinds,
   owner: ValidatorIds,
   kind: {"CausalOrigin"},
   payload:
     [workKind: AsyncWorkKinds,
      item: AsyncRouteNeutralCandidateItemSet,
      authority: AsyncRouteNeutralCandidateEvidenceSet,
      body: SubjectOrNone,
      manifest: SubjectOrNone,
      commitment: SubjectOrNone]]

\* The timeout clock stores its complete immutable origin together with its
\* ordinal.  A sentinel is used only while no timeout lifecycle is owned; it
\* is deliberately outside `AsyncCandidateCausalOriginSet`.
NoAsyncCandidateLifecycleOrigin ==
  [kind |-> "NoCandidateLifecycleOrigin"]

AsyncCandidateWithIdentityAndOrigin(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    consumerContext, consumerView, consumerGeneration, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity, causalOrigin) ==
  [class |-> commandClass, kind |-> kind, node |-> node,
   height |-> blockHeight, view |-> roundView, subject |-> subject,
   item |-> item, consumerContext |-> consumerContext,
   consumerView |-> consumerView,
   consumerGeneration |-> consumerGeneration,
   evidence |-> evidence, bodyIdentity |-> bodyIdentity,
   manifestIdentity |-> manifestIdentity,
   commitmentIdentity |-> commitmentIdentity,
   causalOrigin |-> causalOrigin]

AsyncCandidateWithIdentity(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    consumerContext, consumerView, consumerGeneration, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity) ==
  AsyncCandidateWithIdentityAndOrigin(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    consumerContext, consumerView, consumerGeneration, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity,
    AsyncCandidateCausalOrigin(
      kind, node, blockHeight, roundView, subject, item,
      consumerContext, evidence,
      bodyIdentity, manifestIdentity, commitmentIdentity))

AsyncCandidate(commandClass, kind, node, blockHeight, roundView, subject,
               item) ==
  AsyncCandidateWithIdentity(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    context, nodeView[node], generation[node], item,
    subject, subject, subject)

AsyncCandidateFrom(commandClass, kind, command) ==
  AsyncCandidateWithIdentityAndOrigin(
    commandClass, kind, command.node, context.height, command.view,
    command.subject, NoAsyncItem,
    command.consumerContext, command.consumerView,
    command.consumerGeneration, command.evidence,
    command.bodyIdentity, command.manifestIdentity,
    command.commitmentIdentity, command.causalOrigin)

AsyncCandidateAtConsumer(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    consumerView, consumerGeneration, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity) ==
  AsyncCandidateWithIdentity(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    context, consumerView, consumerGeneration, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity)

AsyncCandidateAtConsumerWithOrigin(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    consumerView, consumerGeneration, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity, causalOrigin) ==
  AsyncCandidateWithIdentityAndOrigin(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    context, consumerView, consumerGeneration, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity, causalOrigin)

AsyncConsumerEventTag(candidate) ==
  [context |-> candidate.consumerContext,
   height |-> candidate.consumerContext.height,
   node |-> candidate.node,
   view |-> candidate.consumerView,
   generation |-> candidate.consumerGeneration]

AsyncWorkIdentity(candidate) ==
  [class |-> candidate.class, kind |-> candidate.kind,
   node |-> candidate.node, height |-> candidate.height,
   view |-> candidate.view, subject |-> candidate.subject]

ExactAsyncCandidateIdentity(candidate) ==
  [consumer |-> AsyncConsumerEventTag(candidate),
   payload |-> candidate.item,
   evidence |-> candidate.evidence,
   causalOrigin |-> candidate.causalOrigin,
   work |-> AsyncWorkIdentity(candidate),
   body |-> candidate.bodyIdentity,
   manifest |-> candidate.manifestIdentity,
   commitment |-> candidate.commitmentIdentity]

CandidateConsumerCurrent(candidate) ==
  /\ candidate.consumerContext = context
  /\ candidate.consumerView = nodeView[candidate.node]
  /\ candidate.consumerGeneration = generation[candidate.node]

AsyncCandidateSet ==
  [class: AsyncCommandClasses, kind: AsyncWorkKinds, node: ValidatorIds,
   height: Heights, view: Views, subject: SubjectOrNone,
   item: AsyncNetworkItems \cup {NoAsyncItem},
   consumerContext: ContextRecords, consumerView: Views,
   consumerGeneration: Generations, evidence: AsyncEvidenceSet,
   bodyIdentity: SubjectOrNone, manifestIdentity: SubjectOrNone,
   commitmentIdentity: SubjectOrNone,
   causalOrigin: AsyncCandidateCausalOriginSet]

AsyncCandidateDomain ==
  {"class", "kind", "node", "height", "view", "subject", "item",
   "consumerContext", "consumerView", "consumerGeneration",
   "evidence", "bodyIdentity", "manifestIdentity", "commitmentIdentity",
   "causalOrigin"}

NoAsyncCandidate ==
  AsyncCandidateWithIdentity(
    "Normal", "AssembleBody", 0, 0, 0,
    AsyncHeartbeatSubject, NoAsyncItem, context, 0, 0, NoAsyncItem,
    AsyncHeartbeatSubject, AsyncHeartbeatSubject, AsyncHeartbeatSubject)

(***************************************************************************
An adapter Busy result retains one exact deferred reducer input independently
of the cyclic class cursor.  The full immutable candidate is carried alongside
its canonical semantic identity; neither a later same-class input nor a cursor
advance can manufacture an equal handoff.  The candidate remains at the head
of its class queue, so the ordinary bounded queues continue to own capacity and
the handoff is an exact dispatch capability rather than a second queue owner.
***************************************************************************)
NoAsyncDeferredHandoff == [active |-> FALSE]

AsyncDeferredHandoff(candidate) ==
  [active |-> TRUE,
   candidate |-> candidate,
   identity |-> ExactAsyncCandidateIdentity(candidate)]

AsyncDeferredHandoffSet ==
  {NoAsyncDeferredHandoff}
    \cup {AsyncDeferredHandoff(candidate): candidate \in AsyncCandidateSet}

AsyncIoCapacity == AsyncIoAuxCapacity + AsyncIoWorkCapacity + 1

AsyncIoJob(commandClass, candidate, nonce) ==
  [class |-> commandClass, candidate |-> candidate, nonce |-> nonce]

AsyncIoConsensusJob(candidate) == AsyncIoJob("Consensus", candidate, 0)
AsyncIoControlJob == AsyncIoJob("Control", NoAsyncCandidate, 0)

AsyncServeAdmission(node, identity, family, roundView, ordinal) ==
  [node |-> node, identity |-> identity, family |-> family,
   view |-> roundView, ordinal |-> ordinal]

AsyncServeIngressAdmission(
    node, identity, ordinal, schedulerOrdinal, ingressPredecessors) ==
  [node |-> node, identity |-> identity, ordinal |-> ordinal,
   schedulerOrdinal |-> schedulerOrdinal,
   ingressPredecessors |-> ingressPredecessors]

AsyncServeReservation(
    node, identity, family, roundView, ordinal,
    predecessors, ingressPredecessors, rollbackTombstones) ==
  [node |-> node, identity |-> identity, family |-> family,
   view |-> roundView, ordinal |-> ordinal,
   predecessors |-> predecessors,
   ingressPredecessors |-> ingressPredecessors,
   rollbackTombstones |-> rollbackTombstones]

AsyncServeTombstone(
    node, identity, family, roundView, ordinal, outputs) ==
  [node |-> node, identity |-> identity, family |-> family,
   view |-> roundView, ordinal |-> ordinal, outputs |-> outputs]

AsyncPacket(item, sentAt, deadline) ==
  [item |-> item, sentAt |-> sentAt, deadline |-> deadline]

AsyncPacketSet ==
  [item: AsyncNetworkItems, sentAt: Nat, deadline: Nat]

AsyncBodyEnvelopeTyped(envelope) ==
  /\ DOMAIN envelope =
       {"recipient", "height", "view", "subject", "chunk", "nonce"}
  /\ envelope.recipient \in ValidatorIds
  /\ envelope.height \in Heights
  /\ envelope.view \in Views
  /\ envelope.subject \in Subjects
  /\ envelope.chunk \in 0..AsyncChunkCount
  /\ envelope.nonce \in 0..(AsyncIngressCapacity - 1)

(***************************************************************************
Runtime typing must inspect the actual finite evidence value, never enumerate
the powerset-valued `TcRecordSet` carrier.  `TcEnvelopeSet` is useful as a
mathematical universe, but constructing it for one membership check exceeds
the pinned TLC set cap even in the initial state.  These predicates are the
structural membership expansion for the only unbounded record branch.
***************************************************************************)
AsyncTcRecordTyped(tc) ==
  /\ DOMAIN tc =
       {"context", "height", "view", "votes", "highestPrepareQc"}
  /\ tc.context \in ContextRecords
  /\ tc.height \in Heights
  /\ tc.view \in Views
  /\ tc.votes \subseteq TimeoutVoteRecordSet
  /\ tc.highestPrepareQc \in PrepareQcOptionSet

AsyncTcEnvelopeTyped(envelope) ==
  /\ DOMAIN envelope = {"recipient", "tc"}
  /\ envelope.recipient \in ValidatorIds
  /\ AsyncTcRecordTyped(envelope.tc)

(***************************************************************************
The commit-certificate request has a canonical route-independent wire shape:
the heartbeat subject, zero chunk, and zero nonce are part of the signed
request identity.  Generic body-envelope typing is therefore insufficient for
this request kind even though it has the same record domain.
***************************************************************************)
AsyncCommitCertificateRequestEnvelopeTyped(envelope) ==
  /\ AsyncBodyEnvelopeTyped(envelope)
  /\ envelope.subject = AsyncHeartbeatSubject
  /\ envelope.chunk = NoAsyncChunk
  /\ envelope.nonce = 0

AsyncReplyRequestItemTyped(item, kind) ==
  /\ DOMAIN item = {"kind", "source", "envelope"}
  /\ kind \in AsyncReplyRequestKinds
  /\ item.kind = kind
  /\ item.source \in ValidatorIds
  /\ IF kind = "CertifiedRequest"
     THEN /\ DOMAIN item.envelope =
               {"recipient", "height", "view", "subject", "requester",
                "certificate", "signatureNonce"}
          /\ item.envelope.recipient \in AsyncArchiveServerIds
          /\ item.envelope.requester = item.source
          /\ item.envelope.certificate \in QcRecordSet
          /\ item.envelope.height =
               item.envelope.certificate.context.height
          /\ item.envelope.view = item.envelope.certificate.view
          /\ item.envelope.subject = item.envelope.certificate.subject
          /\ item.envelope.signatureNonce
               \in 0..(AsyncIngressCapacity - 1)
     ELSE AsyncCommitCertificateRequestEnvelopeTyped(item.envelope)

AsyncCertifiedResponseEnvelopeTyped(envelope) ==
  /\ DOMAIN envelope =
       {"recipient", "height", "view", "subject", "requestHash",
        "archiveServer", "citedResponder", "signatureOwner"}
  /\ envelope.recipient \in ValidatorIds
  /\ envelope.height \in Heights
  /\ envelope.view \in Views
  /\ envelope.subject \in Subjects
  /\ envelope.requestHash \in AsyncCertifiedRequestHashes
  /\ envelope.archiveServer \in AsyncArchiveServerIds
  /\ envelope.citedResponder \in ValidatorIds
  /\ envelope.signatureOwner
       \in AsyncCertifiedResponseSignatureOwners

AsyncCommitCertificateResponseEnvelopeTyped(envelope) ==
  /\ DOMAIN envelope = {"recipient", "request", "qc"}
  /\ AsyncReplyRequestItemTyped(
       envelope.request, "CommitCertificateRequest")
  /\ envelope.recipient = envelope.request.source
  /\ envelope.qc \in QcRecordSet

AsyncItemTyped(item) ==
  /\ DOMAIN item = {"kind", "source", "envelope"}
  /\ item.kind \in AsyncNetworkKinds
  /\ item.source \in AsyncIngressSources
  /\ (item.kind \notin
        {"Noise", "Chunk", "CertifiedResponse",
         "CommitCertificateResponse"}
        => item.source \in ValidatorIds)
  /\ item.envelope.recipient \in ValidatorIds
  /\ CASE item.kind = "Proposal" ->
            /\ item.envelope \in ProposalEnvelopeSet
            /\ item.source = item.envelope.proposal.proposer
       [] item.kind \in {"PrepareVote", "CommitVote"} ->
            /\ item.envelope \in VoteEnvelopeSet
            /\ item.source = item.envelope.vote.signer
            /\ item.kind =
                 IF item.envelope.vote.phase = "Prepare"
                 THEN "PrepareVote"
                 ELSE "CommitVote"
       [] item.kind \in {"PrepareQC", "CommitQC"} ->
            /\ item.envelope \in QcEnvelopeSet
            /\ item.kind =
                 IF item.envelope.qc.phase = "Prepare"
                 THEN "PrepareQC"
                 ELSE "CommitQC"
       [] item.kind = "TimeoutVote" ->
            /\ item.envelope \in TimeoutEnvelopeSet
            /\ item.source = item.envelope.vote.signer
       [] item.kind = "TimeoutCertificate" ->
            AsyncTcEnvelopeTyped(item.envelope)
       [] item.kind \in AsyncReplyRequestKinds ->
            AsyncReplyRequestItemTyped(item, item.kind)
       [] item.kind = "CertifiedResponse" ->
            AsyncCertifiedResponseEnvelopeTyped(item.envelope)
       [] item.kind = "CommitCertificateResponse" ->
            AsyncCommitCertificateResponseEnvelopeTyped(item.envelope)
       [] OTHER ->
            /\ item.kind \in {"Chunk", "NormalJunk", "ProgressJunk", "Noise"}
            /\ AsyncBodyEnvelopeTyped(item.envelope)

AsyncEvidenceTyped(evidence) ==
  \/ evidence = NoAsyncItem
  \/ AsyncItemTyped(evidence)
  \/ evidence \in ProposalRecordSet
  \/ evidence \in VoteRecordSet
  \/ evidence \in TimeoutVoteRecordSet
  \/ evidence \in QcRecordSet
  \/ AsyncTcRecordTyped(evidence)
  \/ evidence \in BodyRecordSet

AsyncCandidateCausalOriginTyped(origin) ==
  /\ origin \in AsyncCandidateCausalOriginSet
  /\ origin.owner = origin.target
  /\ origin.leader = Leader(origin.context, origin.view)
  /\ origin.payload.workKind = origin.phase

AsyncCandidateTyped(candidate) ==
  /\ DOMAIN candidate = AsyncCandidateDomain
  /\ candidate.class \in AsyncCommandClasses
  /\ candidate.kind \in AsyncWorkKinds
  /\ candidate.node \in ValidatorIds
  /\ candidate.height \in Heights
  /\ candidate.view \in Views
  /\ candidate.subject \in SubjectOrNone
  /\ (candidate.item = NoAsyncItem \/ AsyncItemTyped(candidate.item))
  /\ candidate.consumerContext \in ContextRecords
  /\ candidate.consumerView \in Views
  /\ candidate.consumerGeneration \in Generations
  /\ AsyncEvidenceTyped(candidate.evidence)
  /\ candidate.bodyIdentity \in SubjectOrNone
  /\ candidate.manifestIdentity \in SubjectOrNone
  /\ candidate.commitmentIdentity \in SubjectOrNone
  /\ AsyncCandidateCausalOriginTyped(candidate.causalOrigin)

(***************************************************************************
The service identity below is the route-neutral lifecycle key for one logical
reducer occurrence.  Unlike `ExactAsyncCandidateIdentity`, it deliberately
excludes the process-local consumer view and generation.  Coalescing state is
split around the durability boundary:

* every successful service installs a transient marker carrying the exact
  consumer generation.  It prevents same-generation reactivation but becomes
  inactive when Restart advances the generation and is deleted by the
  responsive replay reset;
* only a terminal nondispatchable discard installs a restart-durable
  tombstone.  Such a tombstone remains route-neutral and cannot be resurrected
  at the retired stage.

The semantic round/view, frozen context, derived leader, subject, work phase,
and immutable payload remain part of both keys.  Both record classes allocate
from one immutable per-node ordinal high-watermark, so the fixed-GST producer
episode remains finite without treating a pre-GST restart as progress.

Authenticated aggregate certificates have one semantic reducer occurrence
even when another valid quorum superset or aggregate encoding carries them.
Their service key therefore retains the certificate reference
`(context,height,view,phase,subject)`, and a TC additionally retains its
selected highest-Prepare reference, but deliberately omits QC signer sets and
TC timeout-vote/share composition.  Proposal and timeout-vote payloads apply
the same projection to nested certificate evidence.  Exact wire
authentication remains a delivery precondition; this projection is used only
after that boundary to bound the transient and terminal service-owner tables.

Certified responses also normalize their relay hop to the
aggregate-untrusted source used by the physical ingress owner.  Retried
delivery through another relay consequently retains one service identity
without weakening the signed request/body/certificate authority checked
before service.
***************************************************************************)
AsyncCandidateServicePayload(candidate) ==
  [workKind |-> candidate.kind,
   causalOrigin |-> candidate.causalOrigin,
   item |-> AsyncRouteNeutralCandidateItem(candidate.item),
   evidence |-> AsyncRouteNeutralCandidateEvidence(candidate.evidence),
   body |-> candidate.bodyIdentity,
   manifest |-> candidate.manifestIdentity,
   commitment |-> candidate.commitmentIdentity]

AsyncCandidateServiceIdentity(candidate) ==
  [target |-> candidate.node,
   context |-> candidate.consumerContext,
   height |-> candidate.height,
   leader |-> Leader(candidate.consumerContext, candidate.view),
   view |-> candidate.view,
   subject |-> candidate.subject,
   phase |-> candidate.kind,
   owner |-> candidate.node,
   kind |-> "Candidate",
   payload |-> AsyncCandidateServicePayload(candidate)]

AsyncCandidateAdmissionIdentity(candidate) ==
  [service |-> AsyncCandidateServiceIdentity(candidate),
   consumer |-> AsyncConsumerEventTag(candidate)]

AsyncCandidateAdmissionIdentitySet ==
  {AsyncCandidateAdmissionIdentity(candidate):
     candidate \in AsyncCandidateSet}

AsyncRestartScopedCandidateServiceKinds ==
  {"SignProposal", "SignVote", "SignTimeout"}

AsyncCandidateServiceMarker(
    candidate, episodeView, episodeGeneration, ordinal) ==
  [identity |-> AsyncCandidateServiceIdentity(candidate),
   node |-> candidate.node,
   context |-> candidate.consumerContext,
   height |-> candidate.height,
   view |-> candidate.view,
   episodeView |-> episodeView,
   generation |-> episodeGeneration,
   subject |-> candidate.subject,
   phase |-> candidate.kind,
   ordinal |-> ordinal]

AsyncCandidateServiceMarkerSet ==
  {AsyncCandidateServiceMarker(
     candidate, episodeView, episodeGeneration, ordinal):
     candidate \in AsyncCandidateSet,
     episodeView \in Views,
     episodeGeneration \in Generations,
     ordinal \in Nat \ {0}}

AsyncCandidateServiceTombstone(candidate, episodeView, ordinal) ==
  [identity |-> AsyncCandidateServiceIdentity(candidate),
   node |-> candidate.node,
   context |-> candidate.consumerContext,
   height |-> candidate.height,
   view |-> candidate.view,
   episodeView |-> episodeView,
   subject |-> candidate.subject,
   phase |-> candidate.kind,
   ordinal |-> ordinal]

AsyncCandidateServiceTombstoneSet ==
  {AsyncCandidateServiceTombstone(candidate, episodeView, ordinal):
     candidate \in AsyncCandidateSet,
     episodeView \in Views,
     ordinal \in Nat \ {0}}

(***************************************************************************
One immutable local admission ordinal follows a logical reducer lifecycle.
The normalized causal origin is the key: causal successors, adapter deferral,
I/O completion, transport retry, and same-height replay all retain it.  A
record may become restart-dormant (`retired = TRUE`) while no physical carrier
is scheduled.  Reactivation reuses the same record and ordinal.  A transient
marker, terminal tombstone, or exact durable replay source keeps the dormant
reservation live.  Only after that semantic owner disappears (or a strict
context/view/height/Decision guard makes it obsolete) is the slot compacted.
An ignored no-state-change episode has no such owner and is released in the
same transition.
***************************************************************************)
AsyncCandidateLifecycleServicedSlots ==
  1..AsyncServicedCandidateLifecycleCapacity

AsyncCandidateLifecycleActiveSlots ==
  (AsyncServicedCandidateLifecycleCapacity + 1)
    ..AsyncCandidateLifecycleOrdinaryCapacity

AsyncCandidateLifecycleOrdinarySlots ==
  AsyncCandidateLifecycleServicedSlots
    \cup AsyncCandidateLifecycleActiveSlots

AsyncCandidateLifecycleClockSlot ==
  AsyncCandidateLifecyclePerNodeCapacity

AsyncCandidateLifecycleSlots ==
  AsyncCandidateLifecycleOrdinarySlots
    \cup {AsyncCandidateLifecycleClockSlot}

AsyncCandidateLifecycleAdmission(node, origin, ordinal, slot, retired) ==
  [node |-> node, origin |-> origin,
   ordinal |-> ordinal, slot |-> slot, retired |-> retired]

AsyncCandidateLifecycleAdmissionSet ==
  {AsyncCandidateLifecycleAdmission(node, origin, ordinal, slot, retired):
     node \in ValidatorIds,
     origin \in AsyncCandidateCausalOriginSet,
     ordinal \in Nat \ {0},
     slot \in AsyncCandidateLifecycleSlots,
     retired \in BOOLEAN}

AsyncQueueTyped(queue) ==
  /\ queue \in Seq(Range(queue))
  /\ DOMAIN queue = 1..Len(queue)
  /\ \A index \in 1..Len(queue): AsyncCandidateTyped(queue[index])

AsyncPacketTyped(packet) ==
  /\ DOMAIN packet = {"item", "sentAt", "deadline"}
  /\ AsyncItemTyped(packet.item)
  /\ packet.sentAt \in Nat
  /\ packet.deadline \in Nat

AsyncIoJobTyped(job) ==
  /\ DOMAIN job = {"class", "candidate", "nonce"}
  /\ job.class \in AsyncIoCommandClasses
  /\ job.nonce \in 0..AsyncIoAuxCapacity
  /\ IF job.class = "Consensus"
     THEN /\ AsyncCandidateTyped(job.candidate)
          /\ job.candidate.class = "Completion"
     ELSE IF job.class = "Serve"
          THEN /\ AsyncCandidateTyped(job.candidate)
               /\ job.candidate.kind = "AcceptCertifiedRequest"
               /\ job.candidate.item.kind
                    \in {"CertifiedRequest", "CommitCertificateRequest"}
          ELSE job.candidate = NoAsyncCandidate

VARIABLES
  asyncNow,
  asyncCommandQueues,
  asyncNextCommandClass,
  asyncFifoOwed,
  asyncTimeoutEmitted,
  asyncRunnerPhase,
  asyncRunnerBudget,
  asyncCausalAdmissionOwed,
  asyncNextLocalSource,
  asyncIoQueues,
  asyncNextServeAdmissionOrdinal,
  asyncNextServeIngressOrdinal,
  asyncServeIngressAdmissions,
  asyncServeAdmissions,
  asyncServeReservations,
  asyncServeTombstones,
  asyncOutstandingWork,
  asyncIoReadyCompletions,
  asyncLocalReadyCompletions,
  asyncNextCompletionSource,
  asyncIoControlAvailable,
  asyncDeferredCompletionQueues,
  asyncDeferredProgressQueues,
  asyncDeferredNormalQueues,
  asyncDeferredHandoffs,
  asyncNextDeferredClass,
  asyncDeferredDrainOwed,
  asyncCausalQueues,
  asyncOutstandingTags,
  asyncNodeDeadlines,
  asyncRetransmitDeadlines,
  asyncNodeServiceDeadlines,
  asyncIoServiceDeadlines,
  asyncSentItems, asyncRetainedControl, asyncActiveRequests,
  asyncCertifiedResponseClaim,
  asyncTransport,
  asyncIngressLanes,
  asyncIngressReady,
  asyncHeldChunks,
  asyncHistoricalRecoveryTargets,
  asyncControlServiceState,
  asyncServiceActivationState,
  asyncRecoveryPhase,
  asyncRecoveryNode,
  asyncRecoveryGeneration,
  asyncRecoveryReplayQueue,
  asyncHistoricalLockRestartAuthorities

AsyncSchedulerVars ==
  <<asyncNow, asyncCommandQueues, asyncNextCommandClass,
    asyncFifoOwed, asyncTimeoutEmitted,
    asyncRunnerPhase, asyncRunnerBudget,
    asyncCausalAdmissionOwed, asyncNextLocalSource, asyncIoQueues,
    asyncNextServeAdmissionOrdinal, asyncNextServeIngressOrdinal,
    asyncServeIngressAdmissions, asyncServeAdmissions,
    asyncServeReservations, asyncServeTombstones,
    asyncOutstandingWork, asyncIoReadyCompletions,
    asyncLocalReadyCompletions, asyncNextCompletionSource,
    asyncIoControlAvailable, asyncDeferredCompletionQueues,
    asyncDeferredProgressQueues, asyncDeferredNormalQueues,
    asyncDeferredHandoffs,
    asyncNextDeferredClass, asyncDeferredDrainOwed,
    asyncCausalQueues, asyncOutstandingTags,
    asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
    asyncSentItems, asyncRetainedControl, asyncActiveRequests,
    asyncCertifiedResponseClaim, asyncTransport,
    asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
    asyncHistoricalRecoveryTargets,
    asyncControlServiceState, asyncServiceActivationState>>

AsyncSchedulerExceptHistoricalRecoveryTargets ==
  <<asyncNow, asyncCommandQueues, asyncNextCommandClass,
    asyncFifoOwed, asyncTimeoutEmitted,
    asyncRunnerPhase, asyncRunnerBudget,
    asyncCausalAdmissionOwed, asyncNextLocalSource, asyncIoQueues,
    asyncNextServeAdmissionOrdinal, asyncNextServeIngressOrdinal,
    asyncServeIngressAdmissions, asyncServeAdmissions,
    asyncServeReservations, asyncServeTombstones,
    asyncOutstandingWork, asyncIoReadyCompletions,
    asyncLocalReadyCompletions, asyncNextCompletionSource,
    asyncIoControlAvailable, asyncDeferredCompletionQueues,
    asyncDeferredProgressQueues, asyncDeferredNormalQueues,
    asyncDeferredHandoffs,
    asyncNextDeferredClass, asyncDeferredDrainOwed,
    asyncCausalQueues, asyncOutstandingTags,
    asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
    asyncSentItems, asyncRetainedControl, asyncActiveRequests,
    asyncCertifiedResponseClaim, asyncTransport,
    asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
    asyncControlServiceState, asyncServiceActivationState>>

AsyncSchedulerExceptServiceActivation ==
  <<asyncNow, asyncCommandQueues, asyncNextCommandClass,
    asyncFifoOwed, asyncTimeoutEmitted,
    asyncRunnerPhase, asyncRunnerBudget,
    asyncCausalAdmissionOwed, asyncNextLocalSource, asyncIoQueues,
    asyncNextServeAdmissionOrdinal, asyncNextServeIngressOrdinal,
    asyncServeIngressAdmissions, asyncServeAdmissions,
    asyncServeReservations, asyncServeTombstones,
    asyncOutstandingWork, asyncIoReadyCompletions,
    asyncLocalReadyCompletions, asyncNextCompletionSource,
    asyncIoControlAvailable, asyncDeferredCompletionQueues,
    asyncDeferredProgressQueues, asyncDeferredNormalQueues,
    asyncDeferredHandoffs,
    asyncNextDeferredClass, asyncDeferredDrainOwed,
    asyncCausalQueues, asyncOutstandingTags,
    asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncSentItems, asyncRetainedControl, asyncActiveRequests,
    asyncCertifiedResponseClaim, asyncTransport,
    asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
    asyncHistoricalRecoveryTargets, asyncControlServiceState>>

AsyncRecoveryLifecycleVars ==
  <<asyncRecoveryPhase, asyncRecoveryNode, asyncRecoveryGeneration>>

AsyncRecoveryControlVars ==
  <<asyncRecoveryPhase, asyncRecoveryNode, asyncRecoveryGeneration,
    asyncRecoveryReplayQueue>>

AsyncRecoveryVars ==
  <<asyncRecoveryPhase, asyncRecoveryNode, asyncRecoveryGeneration,
    asyncRecoveryReplayQueue, asyncHistoricalLockRestartAuthorities>>

AsyncAllVars == <<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars>>

AsyncServiceActivationFrameVars ==
  <<gst, vars, AsyncSchedulerExceptServiceActivation, AsyncRecoveryVars>>

(***************************************************************************
The control service table is one bounded per-height structure.  Its
`nextOrdinal` function never reuses a first-owner ordinal during the height;
same-height restart retains that high-watermark, while `slots` contains at
most one record for each fixed recipient/source/protocol-owner slot.
***************************************************************************)
AsyncControlServiceSlots == asyncControlServiceState.slots

AsyncNextControlServiceOrdinal(node) ==
  asyncControlServiceState.nextOrdinal[node]

AsyncCertifiedResponseClaimRecords ==
  asyncControlServiceState.certifiedResponseClaims

AsyncNextCertifiedResponseClaimOrdinal(node) ==
  asyncControlServiceState.certifiedResponseNextOrdinal[node]

AsyncCandidateServiceMarkers ==
  asyncControlServiceState.candidateServiceMarkers

AsyncCandidateTerminalTombstones ==
  asyncControlServiceState.candidateTerminalTombstones

\* Compatibility projection for finite-owner proofs shared by the adequate
\* leader and historical-recovery modules.  The two state classes remain
\* explicit below; this union must never be used to suppress restart replay.
AsyncCandidateServiceTombstones ==
  AsyncCandidateServiceMarkers \cup AsyncCandidateTerminalTombstones

AsyncNextCandidateServiceOrdinal(node) ==
  asyncControlServiceState.candidateServiceNextOrdinal[node]

AsyncCandidateLifecycleAdmissions ==
  asyncControlServiceState.candidateLifecycleAdmissions

AsyncNextCandidateLifecycleOrdinal(node) ==
  asyncControlServiceState.candidateLifecycleNextOrdinal[node]

AsyncTimeoutLifecycleOrdinal(node) ==
  asyncControlServiceState.timeoutLifecycleOrdinal[node]

AsyncTimeoutLifecycleOrigin(node) ==
  asyncControlServiceState.timeoutLifecycleOrigin[node]

AsyncCandidateLifecycleRecordsFor(node, origin) ==
  {record \in AsyncCandidateLifecycleAdmissions:
     /\ record.node = node
     /\ record.origin = origin}

AsyncCandidateLifecycleRecorded(node, origin) ==
  AsyncCandidateLifecycleRecordsFor(node, origin) # {}

AsyncCandidateLifecycleRecordFor(node, origin) ==
  CHOOSE record \in AsyncCandidateLifecycleRecordsFor(node, origin): TRUE

AsyncCandidateLifecycleOrdinal(candidate) ==
  LET records ==
        AsyncCandidateLifecycleRecordsFor(
          candidate.node, candidate.causalOrigin)
  IN IF records = {}
     THEN AsyncNextCandidateLifecycleOrdinal(candidate.node)
     ELSE (CHOOSE record \in records: TRUE).ordinal

AsyncCandidateLifecycleOriginDormant(node, origin) ==
  /\ AsyncCandidateLifecycleRecorded(node, origin)
  /\ AsyncCandidateLifecycleRecordFor(node, origin).retired

AsyncCandidateTransientServiceRecordsForIdentity(identity) ==
  {record \in AsyncCandidateServiceMarkers:
     record.identity = identity}

AsyncCandidateTerminalRecordsForIdentity(identity) ==
  {record \in AsyncCandidateTerminalTombstones:
     record.identity = identity}

AsyncCandidateTransientServiceIdentityMarked(identity) ==
  AsyncCandidateTransientServiceRecordsForIdentity(identity) # {}

AsyncCandidateTerminalIdentityTombstoned(identity) ==
  AsyncCandidateTerminalRecordsForIdentity(identity) # {}

AsyncCandidateServiceRecordsForIdentity(identity) ==
  AsyncCandidateTransientServiceRecordsForIdentity(identity)
    \cup AsyncCandidateTerminalRecordsForIdentity(identity)

\* Compatibility name: in a live generation either record class coalesces an
\* exact retry.  Restart-specific code must use the terminal predicate above.
AsyncCandidateServiceIdentityTombstoned(identity) ==
  AsyncCandidateServiceRecordsForIdentity(identity) # {}

AsyncCandidateTransientServiceRecordsFor(candidate) ==
  AsyncCandidateTransientServiceRecordsForIdentity(
    AsyncCandidateServiceIdentity(candidate))

AsyncCandidateTerminalRecordsFor(candidate) ==
  AsyncCandidateTerminalRecordsForIdentity(
    AsyncCandidateServiceIdentity(candidate))

AsyncCandidateServiceRecordsFor(candidate) ==
  AsyncCandidateServiceRecordsForIdentity(
    AsyncCandidateServiceIdentity(candidate))

AsyncCandidateTransientServiceMarked(candidate) ==
  AsyncCandidateTransientServiceRecordsFor(candidate) # {}

AsyncCandidateTerminalTombstoned(candidate) ==
  AsyncCandidateTerminalRecordsFor(candidate) # {}

AsyncCandidateServiceCoalesced(candidate) ==
  \/ AsyncCandidateTransientServiceMarked(candidate)
  \/ AsyncCandidateTerminalTombstoned(candidate)

AsyncCandidateServiceTombstoned(candidate) ==
  AsyncCandidateServiceCoalesced(candidate)

AsyncCandidateServiceRecordsForNodeIn(state, node) ==
  {record \in
     state.candidateServiceMarkers
       \cup state.candidateTerminalTombstones:
     record.node = node}

AsyncCandidateServiceOwnerPartitionInvariantIn(state) ==
  /\ \A serviced \in
       state.candidateServiceMarkers
         \cup state.candidateTerminalTombstones:
       /\ serviced.phase \in AsyncCandidateServiceTrackedKinds
       /\ \E lifecycle \in state.candidateLifecycleAdmissions:
            /\ lifecycle.node = serviced.node
            /\ lifecycle.origin = serviced.identity.payload.causalOrigin
            /\ IF lifecycle.origin.phase = "BeginTimeout"
               THEN lifecycle.slot = AsyncCandidateLifecycleClockSlot
               ELSE IF lifecycle.retired
                    THEN lifecycle.slot
                           \in AsyncCandidateLifecycleServicedSlots
                    ELSE lifecycle.slot
                           \in AsyncCandidateLifecycleActiveSlots
  /\ \A left, right \in
       state.candidateServiceMarkers
         \cup state.candidateTerminalTombstones:
       /\ (left.identity = right.identity => left = right)
       /\ ((left.node = right.node /\ left.ordinal = right.ordinal)
             => left = right)
       /\ ((left.node = right.node
             /\ left.identity.payload.causalOrigin
                  = right.identity.payload.causalOrigin
             /\ AsyncCandidateServiceStageForKind(left.phase)
                  = AsyncCandidateServiceStageForKind(right.phase))
             => left = right)

AsyncCandidateLifecycleRecordForServiceIn(state, serviced) ==
  CHOOSE lifecycle \in state.candidateLifecycleAdmissions:
    /\ lifecycle.node = serviced.node
    /\ lifecycle.origin = serviced.identity.payload.causalOrigin

AsyncCandidateServiceStageOwnerAddresses ==
  [node: ValidatorIds,
   slot: AsyncCandidateLifecycleSlots,
   stage: AsyncCandidateServiceStageClasses]

AsyncCandidateServiceStageOwnerProjectionIn(state) ==
  [serviced \in
     state.candidateServiceMarkers
       \cup state.candidateTerminalTombstones
     |-> [node |-> serviced.node,
          slot |->
            (AsyncCandidateLifecycleRecordForServiceIn(
               state, serviced)).slot,
          stage |-> AsyncCandidateServiceStageForKind(serviced.phase)]]

AsyncControlServiceRecordsForSlot(slot) ==
  {record \in AsyncControlServiceSlots: record.slot = slot}

AsyncControlServiceRecordsForItem(item) ==
  AsyncControlServiceRecordsForSlot(
    AsyncControlServiceSlot(
      item.envelope.recipient, item.source, item.kind))

AsyncControlServiceRecordsForItemIn(state, item) ==
  {record \in state.slots:
    record.slot =
      AsyncControlServiceSlot(
        item.envelope.recipient, item.source, item.kind)}

AsyncControlServiceSlotOwnedIn(state, item) ==
  AsyncControlServiceRecordsForItemIn(state, item) # {}

AsyncControlServiceRecordForItemIn(state, item) ==
  CHOOSE record \in AsyncControlServiceRecordsForItemIn(state, item): TRUE

AsyncControlServiceIdentityServicedOrAdvancedIn(state, item) ==
  /\ item.kind \in AsyncControlKinds
  /\ AsyncControlServiceSlotOwnedIn(state, item)
  /\ LET record == AsyncControlServiceRecordForItemIn(state, item)
     IN \/ /\ AsyncControlServiceIdentityMatches(item, record)
              /\ record.consumed
        \/ /\ record.context = AsyncControlItemContext(item)
              /\ record.height = AsyncControlItemHeight(item)
              /\ record.view > AsyncControlItemView(item)

AsyncControlServiceSlotOwned(item) ==
  AsyncControlServiceRecordsForItem(item) # {}

AsyncControlServiceRecordForItem(item) ==
  CHOOSE record \in AsyncControlServiceRecordsForItem(item): TRUE

AsyncControlServiceIdentityMatches(item, record) ==
  record.identity = AsyncLeaderWireServiceIdentity(item)

AsyncControlServiceCurrentHeightItem(item) ==
  /\ item.kind \in AsyncControlKinds
  /\ AsyncControlItemContext(item) = context
  /\ AsyncControlItemHeight(item) = height

AsyncControlServiceStrictlyNewerItem(item) ==
  /\ AsyncControlServiceCurrentHeightItem(item)
  /\ AsyncControlServiceSlotOwned(item)
  /\ LET current == AsyncControlServiceRecordForItem(item)
     IN \/ current.context # context
        \/ current.height < height
        \/ /\ current.context = context
           /\ current.height = height
           /\ AsyncControlItemView(item) > current.view

AsyncControlServiceAdmissionStartsOrReplaces(item) ==
  /\ AsyncControlServiceCurrentHeightItem(item)
  /\ \/ ~AsyncControlServiceSlotOwned(item)
     \/ AsyncControlServiceStrictlyNewerItem(item)

AsyncControlServiceAdmissionCoalesced(item) ==
  /\ AsyncControlServiceCurrentHeightItem(item)
  /\ AsyncControlServiceSlotOwned(item)
  /\ ~AsyncControlServiceStrictlyNewerItem(item)

AsyncControlServiceOccurrenceIsCurrentOwner(item) ==
  /\ item.kind \in AsyncControlKinds
  /\ AsyncControlServiceSlotOwned(item)
  /\ LET record == AsyncControlServiceRecordForItem(item)
     IN /\ AsyncControlServiceIdentityMatches(item, record)
        /\ ~record.consumed

AsyncControlServiceOccurrenceRetired(item) ==
  /\ item.kind \in AsyncControlKinds
  /\ AsyncControlServiceSlotOwned(item)
  /\ LET record == AsyncControlServiceRecordForItem(item)
     IN \/ ~AsyncControlServiceIdentityMatches(item, record)
        \/ record.consumed

AsyncControlServiceConsumed(item) ==
  /\ item.kind \in AsyncControlKinds
  /\ AsyncControlServiceSlotOwned(item)
  /\ LET record == AsyncControlServiceRecordForItem(item)
     IN /\ AsyncControlServiceIdentityMatches(item, record)
        /\ record.consumed

\* Permanent logical retirement is narrower than physical occurrence
\* retirement.  A mismatching lower-view record does not tombstone a strictly
\* newer item because admission may atomically replace that record.  An exact
\* consumed identity, or a same/higher-view identity already owning the slot,
\* can never be replaced by this same/lower retry.
AsyncControlServiceOccurrenceTombstoned(item) ==
  /\ item.kind \in AsyncControlKinds
  /\ AsyncControlServiceSlotOwned(item)
  /\ LET record == AsyncControlServiceRecordForItem(item)
     IN \/ /\ AsyncControlServiceIdentityMatches(item, record)
              /\ record.consumed
        \/ /\ ~AsyncControlServiceIdentityMatches(item, record)
              /\ record.context = AsyncControlItemContext(item)
              /\ record.height = AsyncControlItemHeight(item)
              /\ record.view >= AsyncControlItemView(item)

\* This is the durable service marker used by logical no-resurrection.  A
\* same-view mismatching owner tombstones only that physical occurrence; it
\* does not prove that the shared logical slot has been serviced.  The marker
\* therefore accepts only the exact consumed identity or a strict view
\* high-watermark which makes every retry at the old view permanently stale.
AsyncControlServiceIdentityServicedOrAdvanced(item) ==
  /\ item.kind \in AsyncControlKinds
  /\ AsyncControlServiceSlotOwned(item)
  /\ LET record == AsyncControlServiceRecordForItem(item)
     IN \/ /\ AsyncControlServiceIdentityMatches(item, record)
              /\ record.consumed
        \/ /\ record.context = AsyncControlItemContext(item)
              /\ record.height = AsyncControlItemHeight(item)
              /\ record.view > AsyncControlItemView(item)

AsyncControlServiceStateTypeInvariant ==
  /\ DOMAIN asyncControlServiceState =
       {"nextOrdinal", "slots",
        "certifiedResponseNextOrdinal", "certifiedResponseClaims",
        "candidateServiceNextOrdinal", "candidateServiceMarkers",
        "candidateTerminalTombstones",
        "candidateLifecycleNextOrdinal", "candidateLifecycleAdmissions",
        "timeoutLifecycleOrdinal", "timeoutLifecycleOrigin"}
  /\ asyncControlServiceState.nextOrdinal
       \in [ValidatorIds -> (Nat \ {0})]
  /\ asyncControlServiceState.certifiedResponseNextOrdinal
       \in [ValidatorIds -> (Nat \ {0})]
  /\ asyncControlServiceState.candidateServiceNextOrdinal
       \in [ValidatorIds -> (Nat \ {0})]
  /\ asyncControlServiceState.candidateLifecycleNextOrdinal
       \in [ValidatorIds -> (Nat \ {0})]
  /\ asyncControlServiceState.timeoutLifecycleOrdinal
       \in [ValidatorIds -> Nat]
  /\ asyncControlServiceState.timeoutLifecycleOrigin
       \in [ValidatorIds ->
             AsyncCandidateCausalOriginSet
               \cup {NoAsyncCandidateLifecycleOrigin}]
  /\ IsFiniteSet(AsyncControlServiceSlots)
  /\ IsFiniteSet(AsyncCertifiedResponseClaimRecords)
  /\ IsFiniteSet(AsyncCandidateServiceMarkers)
  /\ IsFiniteSet(AsyncCandidateTerminalTombstones)
  /\ IsFiniteSet(AsyncCandidateLifecycleAdmissions)
  /\ AsyncCandidateServiceOwnerPartitionInvariantIn(
       asyncControlServiceState)
  /\ AsyncCandidateLifecycleReviewedCapacityInvariantIn(
       asyncControlServiceState)
  /\ Cardinality(AsyncCertifiedResponseClaimRecords)
       <= Cardinality(ValidatorIds)
  /\ Cardinality(AsyncCandidateServiceTombstones)
       <= AsyncCandidateServiceRecordCapacity
  /\ AsyncControlServiceSlots \subseteq AsyncControlServiceRecordSet
  /\ AsyncCandidateServiceMarkers
       \subseteq AsyncCandidateServiceMarkerSet
  /\ AsyncCandidateTerminalTombstones
       \subseteq AsyncCandidateServiceTombstoneSet
  /\ AsyncCandidateLifecycleAdmissions
       \subseteq AsyncCandidateLifecycleAdmissionSet
  /\ \A left, right \in AsyncControlServiceSlots:
       left.slot = right.slot => left = right
  /\ \A record \in AsyncControlServiceSlots:
       /\ record.slot \in AsyncControlServiceSlotSet
       /\ record.ordinal
            < AsyncNextControlServiceOrdinal(record.slot.recipient)
  /\ \A record \in AsyncCertifiedResponseClaimRecords:
       /\ DOMAIN record = {"recipient", "family", "identity", "ordinal"}
       /\ record.recipient \in ValidatorIds
       /\ record.family \in AsyncCertifiedRequestHashes
       /\ record.ordinal \in Nat \ {0}
       /\ record.ordinal
            < AsyncNextCertifiedResponseClaimOrdinal(record.recipient)
  /\ \A left, right \in AsyncCandidateServiceTombstones:
       /\ (left.identity = right.identity => left = right)
       /\ ((left.node = right.node /\ left.ordinal = right.ordinal)
             => left = right)
  /\ \A record \in AsyncCandidateServiceMarkers:
       /\ record.generation \in Generations
       /\ record.ordinal \in Nat \ {0}
       /\ record.ordinal < AsyncNextCandidateServiceOrdinal(record.node)
  /\ \A record \in AsyncCandidateTerminalTombstones:
       /\ record.ordinal \in Nat \ {0}
       /\ record.ordinal < AsyncNextCandidateServiceOrdinal(record.node)
  /\ \A left, right \in AsyncCandidateLifecycleAdmissions:
       /\ ((left.node = right.node /\ left.origin = right.origin)
             => left = right)
       /\ ((left.node = right.node /\ left.ordinal = right.ordinal)
             => left.origin = right.origin)
       /\ ((left.node = right.node /\ left.slot = right.slot)
             => left = right)
  /\ \A record \in AsyncCandidateLifecycleAdmissions:
       /\ record.origin.owner = record.node
       /\ record.origin.target = record.node
       /\ (record.origin.phase = "BeginTimeout")
            = (record.slot = AsyncCandidateLifecycleClockSlot)
       /\ record.ordinal
            < AsyncNextCandidateLifecycleOrdinal(record.node)
  /\ \A node \in ValidatorIds:
       AsyncTimeoutLifecycleOrdinal(node)
         < AsyncNextCandidateLifecycleOrdinal(node)
  /\ \A node \in ValidatorIds:
       (AsyncTimeoutLifecycleOrdinal(node) = 0)
         = (AsyncTimeoutLifecycleOrigin(node)
              = NoAsyncCandidateLifecycleOrigin)

THEOREM AsyncCandidateServiceLifecycleStageCollisionCoalesces ==
  \A state, left, right:
    /\ AsyncCandidateServiceOwnerPartitionInvariantIn(state)
    /\ AsyncCandidateLifecycleSlotInjectionInvariantIn(state)
    /\ left \in
         state.candidateServiceMarkers
           \cup state.candidateTerminalTombstones
    /\ right \in
         state.candidateServiceMarkers
           \cup state.candidateTerminalTombstones
    /\ left.node = right.node
    /\ (AsyncCandidateLifecycleRecordForServiceIn(state, left)).slot
         = (AsyncCandidateLifecycleRecordForServiceIn(state, right)).slot
    /\ AsyncCandidateServiceStageForKind(left.phase)
         = AsyncCandidateServiceStageForKind(right.phase)
    => left = right
BY IsaT(300)
   DEF AsyncCandidateServiceOwnerPartitionInvariantIn,
       AsyncCandidateLifecycleSlotInjectionInvariantIn,
       AsyncCandidateLifecycleRecordForServiceIn,
       AsyncCandidateLifecycleRecordsForNodeIn,
       AsyncCandidateLifecycleServiceRecordCoversIn

THEOREM AsyncCandidateServiceRecordsInjectIntoLifecycleStageOwners ==
  AsyncControlServiceStateTypeInvariant
    => /\ AsyncCandidateServiceStageOwnerProjectionIn(
             asyncControlServiceState)
           \in Injection(
                AsyncCandidateServiceTombstones,
                AsyncCandidateServiceStageOwnerAddresses)
       /\ Cardinality(AsyncCandidateServiceTombstones)
            <= AsyncCandidateServiceRecordCapacity
BY AsyncCandidateServiceLifecycleStageCollisionCoalesces,
   AsyncCandidateServiceTrackedKindProjectionIsCovered,
   FS_Injection, FS_Product, FS_Interval, FS_CardinalityType, IsaT(300)
   DEF AsyncControlServiceStateTypeInvariant,
       AsyncCandidateServiceOwnerPartitionInvariantIn,
       AsyncCandidateServiceStageOwnerProjectionIn,
       AsyncCandidateLifecycleRecordForServiceIn,
       AsyncCandidateServiceStageOwnerAddresses,
       AsyncCandidateServiceRecordCapacity,
       AsyncCandidateServiceStageCapacity,
       AsyncCandidateServiceTombstones,
       AsyncCandidateLifecycleSlotInjectionInvariantIn,
       AsyncCandidateLifecycleSlots,
       AsyncCandidateLifecycleOrdinarySlots

THEOREM AsyncControlServiceTableCardinalityIsSlotBounded ==
  AsyncControlServiceStateTypeInvariant
    => Cardinality(AsyncControlServiceSlots)
         <= Cardinality(AsyncControlServiceSlotSet)
BY FS_Subset, Isa
   DEF AsyncControlServiceStateTypeInvariant,
       AsyncControlServiceSlots

AsyncServeLifecycleVars ==
  <<asyncNextServeAdmissionOrdinal, asyncServeAdmissions,
    asyncServeReservations, asyncServeTombstones>>

AsyncServeIngressAdmissionVars ==
  <<asyncNextServeIngressOrdinal, asyncServeIngressAdmissions>>

AsyncIoVars ==
  <<asyncIoQueues, AsyncServeLifecycleVars,
    AsyncServeIngressAdmissionVars,
    asyncOutstandingWork, asyncIoReadyCompletions,
    asyncLocalReadyCompletions, asyncNextCompletionSource,
    asyncIoControlAvailable>>

AsyncIoExceptServeReservationsVars ==
  <<asyncIoQueues, asyncNextServeAdmissionOrdinal,
    asyncServeAdmissions, asyncServeTombstones,
    asyncOutstandingWork, asyncIoReadyCompletions,
    asyncLocalReadyCompletions, asyncNextCompletionSource,
    asyncIoControlAvailable>>

AsyncDeferredVars ==
  <<asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncDeferredHandoffs, asyncNextDeferredClass,
    asyncDeferredDrainOwed>>

AsyncLocalSources == {"Producer", "Causal"}

AsyncLocalAdmissionVars ==
  <<asyncCausalAdmissionOwed, asyncNextLocalSource>>

ResponsiveReplayQuarantined(node) ==
  /\ node = asyncRecoveryNode
  /\ asyncRecoveryPhase \in {"ReplayRequired", "Replaying"}

ResponsiveReplayDraining(node) ==
  node = asyncRecoveryNode /\ asyncRecoveryPhase = "Replaying"

ResponsiveReplayExecutorAllowed(node) ==
  ~ResponsiveReplayQuarantined(node) \/ ResponsiveReplayDraining(node)

HeldChunksFor(node, roundView, subject) ==
  {receipt.chunk:
     receipt \in {entry \in asyncHeldChunks:
       /\ entry.node = node
       /\ entry.view = roundView
       /\ entry.subject = subject}}

AsyncVotersAt(initialContext) ==
  Responsive \cap VotingRoster(initialContext.epoch)

AsyncCurrentResponsiveVoters == Responsive \cap CurrentVoters

(***************************************************************************
Successor-height composition starts from the exact standalone initialization
and may then restrict local service to the first independently joined node.
The `restricted` tombstone is irreversible: without it, reactivating every
node while the local clock remains zero could re-enable the restriction step
and create an unbounded deactivate/reactivate episode.  Active membership is
internal scheduler metadata; it is neither a wire field nor a configuration
toggle.  The paired nonzero deadlines are its exact executable projection.
***************************************************************************)
AsyncServiceActivationStateSet ==
  [restricted: BOOLEAN, activeNodes: SUBSET ValidatorIds]

AsyncServiceActivationRestricted ==
  asyncServiceActivationState.restricted

AsyncActiveServiceNodes ==
  asyncServiceActivationState.activeNodes

AsyncServiceActivationClockPristine ==
  /\ ~gst
  /\ asyncNow = 0
  /\ \A node \in ValidatorIds:
       /\ asyncNodeDeadlines[node] = AsyncViewTimeout(nodeView[node])
       /\ asyncRetransmitDeadlines[node] = AsyncRetransmitPeriod
       /\ asyncNodeServiceDeadlines[node] = AsyncDeliveryBound
       /\ asyncIoServiceDeadlines[node] = AsyncDeliveryBound

AsyncServiceActivationPairInvariant ==
  /\ asyncServiceActivationState \in AsyncServiceActivationStateSet
  /\ \A node \in ValidatorIds:
       /\ (node \in AsyncActiveServiceNodes
             <=> asyncNodeServiceDeadlines[node] # 0)
       /\ (node \in AsyncActiveServiceNodes
             <=> asyncIoServiceDeadlines[node] # 0)

(***************************************************************************
`asyncNodeServiceDeadlines` is ghost bookkeeping for the reviewed
`runtime-after-gst` contract, not a shared scheduler or a value stored by the
production nodes. Each member below denotes one independent validator process
whose serialized height runner must regain a finite local service turn. The
product model advances its proof clock only while every such local contract is
current; no production process reads or updates another process's deadline.
***************************************************************************)
LocalRunnerServiceOwners ==
  AsyncActiveServiceNodes
    \cap (AsyncCurrentResponsiveVoters \cup asyncHistoricalRecoveryTargets)

HistoricalRecoveryTarget(node) ==
  node \in asyncHistoricalRecoveryTargets

AsyncGenesisResponsiveVoters ==
  AsyncVotersAt(ContextRecord(0, <<>>))

AsyncNormalLimit ==
  AsyncQueueCapacity - AsyncProgressReserve - AsyncCompletionReserve
AsyncProgressLimit == AsyncQueueCapacity - AsyncCompletionReserve
AsyncOrdinaryCompletionLimit == AsyncQueueCapacity - 1

AsyncQueueDepth(node) == Len(asyncCommandQueues[node])

AsyncIoQueueDepth(node) == Len(asyncIoQueues[node])

AsyncIoAdmissionLimit(commandClass) ==
  CASE commandClass = "Serve" -> AsyncIoAuxCapacity
    [] commandClass = "Consensus" ->
         AsyncIoAuxCapacity + AsyncIoWorkCapacity
    [] commandClass = "Control" -> AsyncIoCapacity

AsyncIoServeIndices(queue) ==
  {index \in 1..Len(queue): queue[index].class = "Serve"}

AsyncIoServeJobIdentity(node, job) ==
  AsyncServeLogicalRequestIdentity(node, job.candidate.item)

AsyncIoServeIdentities(node) ==
  {AsyncIoServeJobIdentity(node, asyncIoQueues[node][index]):
     index \in AsyncIoServeIndices(asyncIoQueues[node])}

AsyncIoServeIdentityIndices(node, identity) ==
  {index \in AsyncIoServeIndices(asyncIoQueues[node]):
     AsyncIoServeJobIdentity(
       node, asyncIoQueues[node][index]) = identity}

AsyncIoJobsBeforeServeIdentity(node, identity) ==
  {asyncIoQueues[node][index]:
     index \in
       {position \in 1..Len(asyncIoQueues[node]):
          \E targetIndex \in
               AsyncIoServeIdentityIndices(node, identity):
            position < targetIndex}}

AsyncServeIngressAdmissionRecords(node, identity) ==
  {admission \in asyncServeIngressAdmissions:
     /\ admission.node = node
     /\ admission.identity = identity}

AsyncServeIngressAdmissionOwned(node, identity) ==
  AsyncServeIngressAdmissionRecords(node, identity) # {}

AsyncServeIngressAdmissionRecord(node, identity) ==
  CHOOSE admission \in
    AsyncServeIngressAdmissionRecords(node, identity): TRUE

AsyncServeIngressAdmissionOrdinal(node, identity) ==
  AsyncServeIngressAdmissionRecord(node, identity).ordinal

AsyncServeIngressAdmissionSchedulerOrdinal(node, identity) ==
  AsyncServeIngressAdmissionRecord(node, identity).schedulerOrdinal

AsyncServeIngressAdmissionPredecessorCounts(node, identity) ==
  IF AsyncServeIngressAdmissionOwned(node, identity)
  THEN AsyncServeIngressAdmissionRecord(
         node, identity).ingressPredecessors
  ELSE [source \in AsyncIngressSources |-> 0]

(***************************************************************************
Production may pre-reserve several bounded `serve_ingress_waiters` before the
selected barrier drains.  The model refines those physical waiter records as
this ticket's frozen per-source ingress prefix, not as an unbounded sequence
of independently privileged target turns.  Each waiter still receives its
Rust shared ordinal; for proof ranking, every waiter admitted before the
timeout cut is charged once to this finite prefix.  A packet admitted after
the cut is appended behind the snapshot and, if it later becomes a distinct
ticket, receives a strictly later shared scheduler ordinal.
***************************************************************************)
AsyncServeIngressAdmissionPredecessorDebtSlots(node, identity) ==
  UNION {
    {[source |-> source, slot |-> slot]:
       slot \in
         1..AsyncServeIngressAdmissionPredecessorCounts(
              node, identity)[source]}:
    source \in AsyncIngressSources}

AsyncServeIngressAdmissionIdentities(node) ==
  {admission.identity:
     admission \in
       {owned \in asyncServeIngressAdmissions:
          owned.node = node}}

AsyncServeIngressLifecycleOwnerIdentities(node) ==
  AsyncServeIngressAdmissionIdentities(node)

AsyncServeEarliestIngressLifecycleOwnerIdentity(node) ==
  CHOOSE identity \in AsyncServeIngressLifecycleOwnerIdentities(node):
    \A other \in AsyncServeIngressLifecycleOwnerIdentities(node):
      AsyncServeIngressAdmissionOrdinal(node, identity)
        <= AsyncServeIngressAdmissionOrdinal(node, other)

(***************************************************************************
The ingress ordinal orders exact requests inside the physical Serve barrier.
The scheduler ordinal instead joins that barrier to the one process-local
Candidate/timeout lifecycle order.  The two values are deliberately distinct:
retries may receive another ingress turn, but every turn is born after every
already admitted Runtime root and can therefore interleave, rather than erase,
an older timeout episode.
***************************************************************************)
AsyncServeEarliestIngressSchedulerOwnerIdentity(node) ==
  CHOOSE identity \in AsyncServeIngressLifecycleOwnerIdentities(node):
    \A other \in AsyncServeIngressLifecycleOwnerIdentities(node):
      AsyncServeIngressAdmissionSchedulerOrdinal(node, identity)
        <= AsyncServeIngressAdmissionSchedulerOrdinal(node, other)

AsyncServeEarliestIngressSchedulerOrdinal(node) ==
  IF AsyncServeIngressLifecycleOwnerIdentities(node) = {}
  THEN AsyncNextCandidateLifecycleOrdinal(node)
  ELSE AsyncServeIngressAdmissionSchedulerOrdinal(
         node, AsyncServeEarliestIngressSchedulerOwnerIdentity(node))

AsyncServeAdmissionRecords(node, identity) ==
  {admission \in asyncServeAdmissions:
     /\ admission.node = node
     /\ admission.identity = identity}

AsyncServeReservationRecords(node, identity) ==
  {reservation \in asyncServeReservations:
     /\ reservation.node = node
     /\ reservation.identity = identity}

AsyncServeTombstoneRecords(node, identity) ==
  {tombstone \in asyncServeTombstones:
     /\ tombstone.node = node
     /\ tombstone.identity = identity}

AsyncServeFamilyReservationRecords(node, family) ==
  {reservation \in asyncServeReservations:
     /\ reservation.node = node
     /\ reservation.family = family}

AsyncServeFamilyTombstoneRecords(node, family) ==
  {tombstone \in asyncServeTombstones:
     /\ tombstone.node = node
     /\ tombstone.family = family}

AsyncServeLifecycleOwned(node, identity) ==
  \/ AsyncServeReservationRecords(node, identity) # {}
  \/ AsyncServeTombstoneRecords(node, identity) # {}

AsyncServeLifecycleFamilyOwned(node, family) ==
  \/ AsyncServeFamilyReservationRecords(node, family) # {}
  \/ AsyncServeFamilyTombstoneRecords(node, family) # {}

AsyncServeLifecycleTombstone(node, identity) ==
  AsyncServeTombstoneRecords(node, identity) # {}

AsyncServeLogicalIdentityRequests(node, identity) ==
  {request \in
     AsyncCertifiedRequestItems \cup
       AsyncCommitCertificateRequestItems:
     AsyncServeLogicalRequestIdentity(node, request) = identity}

AsyncServeLiveReservationOwned(node, identity) ==
  AsyncServeReservationRecords(node, identity) # {}

AsyncServeAdmissionRecord(node, identity) ==
  CHOOSE admission \in AsyncServeAdmissionRecords(node, identity): TRUE

AsyncServeReservationRecord(node, identity) ==
  CHOOSE reservation \in AsyncServeReservationRecords(node, identity): TRUE

AsyncServeRollbackTombstones(node, identity) ==
  IF AsyncServeLiveReservationOwned(node, identity)
  THEN AsyncServeReservationRecord(
         node, identity).rollbackTombstones
  ELSE {}

AsyncServeTombstoneRecord(node, identity) ==
  CHOOSE tombstone \in AsyncServeTombstoneRecords(node, identity): TRUE

AsyncServeAdmissionOrdinal(node, identity) ==
  IF AsyncServeLiveReservationOwned(node, identity)
  THEN AsyncServeReservationRecord(node, identity).ordinal
  ELSE AsyncServeTombstoneRecord(node, identity).ordinal

AsyncServeFrozenPredecessorSet(node, identity) ==
  IF AsyncServeLiveReservationOwned(node, identity)
  THEN AsyncServeReservationRecord(node, identity).predecessors
  ELSE {}

AsyncServeFrozenIngressPredecessorCounts(node, identity) ==
  IF AsyncServeLiveReservationOwned(node, identity)
  THEN AsyncServeReservationRecord(
         node, identity).ingressPredecessors
  ELSE [source \in AsyncIngressSources |-> 0]

AsyncServeFrozenIngressPredecessorSet(node, identity) ==
  UNION {
    {[
       source |-> source,
       index |-> index,
       item |-> asyncIngressLanes[node][source][index]
     ]:
       index \in
         1..AsyncServeFrozenIngressPredecessorCounts(
              node, identity)[source]}:
    source \in AsyncIngressSources}

(***************************************************************************
The item-bearing frozen-prefix set above is used to recognize the concrete
runner action.  It is not a stable rank carrier: after draining slot one,
the item formerly at slot two is re-indexed as slot one.  The count-slot set
below deliberately omits the mutable item and index.  A frozen-prefix drain
changes `1..n` to `1..(n - 1)`, so its rank carrier is a genuine subset and
cannot be replenished by re-indexing the surviving items.
***************************************************************************)
AsyncServeFrozenIngressPredecessorDebtSlots(node, identity) ==
  UNION {
    {[source |-> source, slot |-> slot]:
       slot \in
         1..AsyncServeFrozenIngressPredecessorCounts(
              node, identity)[source]}:
    source \in AsyncIngressSources}

AsyncServeTombstoneOutputs(node, identity) ==
  IF AsyncServeLifecycleTombstone(node, identity)
  THEN AsyncServeTombstoneRecord(node, identity).outputs
  ELSE {}

AsyncServeJobQueued(node, identity) ==
  identity \in AsyncIoServeIdentities(node)

AsyncServeOffQueueReservations(node) ==
  {reservation \in asyncServeReservations:
     /\ reservation.node = node
     /\ ~AsyncServeJobQueued(node, reservation.identity)}

AsyncIoEffectiveQueueDepth(node) ==
  IF AsyncServeOffQueueReservations(node) # {}
  THEN AsyncIoCapacity
  ELSE AsyncIoQueueDepth(node)

CanEnqueueIoClass(node, commandClass) ==
  AsyncIoEffectiveQueueDepth(node) <
    AsyncIoAdmissionLimit(commandClass)

CanResumeExactServeCapacity(node, identity) ==
  /\ AsyncServeLiveReservationOwned(node, identity)
  /\ ~AsyncServeJobQueued(node, identity)
  /\ AsyncIoQueueDepth(node) < AsyncIoCapacity

AsyncServeFamilyReservationRecord(node, family) ==
  CHOOSE reservation \in
    AsyncServeFamilyReservationRecords(node, family): TRUE

AsyncServeFamilyTombstoneRecord(node, family) ==
  CHOOSE tombstone \in
    AsyncServeFamilyTombstoneRecords(node, family): TRUE

AsyncServeFamilyOwnerIdentity(node, family) ==
  IF AsyncServeFamilyReservationRecords(node, family) # {}
  THEN AsyncServeFamilyReservationRecord(node, family).identity
  ELSE AsyncServeFamilyTombstoneRecord(node, family).identity

AsyncServeFamilyHighWatermark(node, family) ==
  IF AsyncServeFamilyReservationRecords(node, family) # {}
  THEN AsyncServeFamilyReservationRecord(node, family).view
  ELSE AsyncServeFamilyTombstoneRecord(node, family).view

AsyncServeLogicalIdentityRetiredOrSuperseded(node, identity) ==
  \/ AsyncServeLifecycleTombstone(node, identity)
  \/ \E request \in AsyncServeLogicalIdentityRequests(node, identity):
       LET family == AsyncServeLifecycleFamily(node, request)
       IN /\ AsyncServeLifecycleFamilyOwned(node, family)
          /\ AsyncServeFamilyOwnerIdentity(node, family) # identity
          /\ AsyncServeFamilyHighWatermark(node, family)
               > AsyncServeRequestView(request)

AsyncServeActiveFamilyRequests(node, family) ==
  {request \in asyncActiveRequests:
     /\ request.kind \in AsyncReplyRequestKinds
     /\ request.envelope.recipient = node
     /\ AsyncServeLifecycleFamily(node, request) = family}

AsyncServeFamilyAdvanceRetiresPriorRequests(node, request) ==
  LET identity == AsyncServeLogicalRequestIdentity(node, request)
      family == AsyncServeLifecycleFamily(node, request)
  IN \A active \in AsyncServeActiveFamilyRequests(node, family):
       \/ AsyncServeRequestView(active) >=
            AsyncServeRequestView(request)
       \/ AsyncServeLogicalRequestIdentity(node, active) = identity

AsyncServeRequestRespectsHighWatermark(request) ==
  LET node == request.envelope.recipient
      family == AsyncServeLifecycleFamily(node, request)
  IN IF AsyncServeLifecycleFamilyOwned(node, family)
     THEN AsyncServeRequestView(request) >=
            AsyncServeFamilyHighWatermark(node, family)
     ELSE TRUE

AsyncServePublicationRespectsHighWatermarks(items) ==
  \A request \in items:
    AsyncServeRequestRespectsHighWatermark(request)

AsyncServeIngressPredecessorCounts(node) ==
  [source \in AsyncIngressSources |->
     Len(asyncIngressLanes[node][source])]

AsyncIoServeNonces(node) ==
  {asyncIoQueues[node][index].nonce:
     index \in AsyncIoServeIndices(asyncIoQueues[node])}

FreshAsyncIoServeNonce(node) ==
  CHOOSE nonce \in 0..AsyncIoAuxCapacity:
    nonce \notin AsyncIoServeNonces(node)

AsyncIoCertifiedServeJob(node, candidate) ==
  AsyncIoJob("Serve", candidate, FreshAsyncIoServeNonce(node))

AsyncServeReservationsWithoutFrom(reservations, node, identity) ==
  {reservation \in reservations:
     \/ reservation.node # node
     \/ reservation.identity # identity}

AsyncServeReservationsWithout(node, identity) ==
  AsyncServeReservationsWithoutFrom(
    asyncServeReservations, node, identity)

AsyncServeIngressAdmissionsWithout(node, identity) ==
  {admission \in asyncServeIngressAdmissions:
     \/ admission.node # node
     \/ admission.identity # identity}

AsyncServeAdmissionsWithout(node, identity) ==
  {admission \in asyncServeAdmissions:
     \/ admission.node # node
     \/ admission.identity # identity}

AsyncServeTombstonesWithoutFamily(node, family) ==
  {tombstone \in asyncServeTombstones:
     \/ tombstone.node # node
     \/ tombstone.family # family}

AsyncServeAdmissionsWithoutNode(node) ==
  {admission \in asyncServeAdmissions:
     admission.node # node}

AsyncServeIngressAdmissionsWithoutNode(node) ==
  {admission \in asyncServeIngressAdmissions:
     admission.node # node}

AsyncServeReservationsWithoutNode(node) ==
  {reservation \in asyncServeReservations:
     reservation.node # node}

AsyncServeTombstonesWithoutNode(node) ==
  {tombstone \in asyncServeTombstones:
     tombstone.node # node}

AsyncServeRemainingPredecessorIndices(node, identity) ==
  {index \in 1..Len(asyncIoQueues[node]):
     asyncIoQueues[node][index]
       \in AsyncServeFrozenPredecessorSet(node, identity)}

AsyncServeEarlierOrdinalIndices(node, identity) ==
  {index \in AsyncIoServeIndices(asyncIoQueues[node]):
     LET earlierIdentity ==
           AsyncIoServeJobIdentity(
             node, asyncIoQueues[node][index])
     IN AsyncServeAdmissionOrdinal(node, earlierIdentity)
          < AsyncServeAdmissionOrdinal(node, identity)}

AsyncServeEarlierLiveReservationIdentities(node, identity) ==
  IF AsyncServeLifecycleOwned(node, identity)
  THEN {reservation.identity:
          reservation \in
            {owned \in asyncServeReservations:
               /\ owned.node = node
               /\ owned.ordinal
                    < AsyncServeAdmissionOrdinal(node, identity)
               \* Once an earlier owner materializes, its physical I/O job is
               \* charged to the ordinary capacity/FIFO component.  Keeping
               \* both identities here would count the same finite owner
               \* twice and turn materialization into a replenishment ascent.
               /\ ~AsyncServeJobQueued(node, owned.identity)}}
  ELSE {}

AsyncServeMaterializationPredecessorIndices(node, identity) ==
  AsyncServeRemainingPredecessorIndices(node, identity)
    \cup AsyncServeEarlierOrdinalIndices(node, identity)

AsyncServeMaterializationPredecessorJobs(node, identity) ==
  {asyncIoQueues[node][index]:
     index \in
       AsyncServeMaterializationPredecessorIndices(node, identity)}

AsyncServeResumeInsertionIndex(node, identity) ==
  Cardinality(
    AsyncServeMaterializationPredecessorIndices(
      node, identity)) + 1

AsyncIoQueueWithResumedServe(node, identity, job) ==
  LET insertionIndex ==
        AsyncServeResumeInsertionIndex(node, identity)
  IN SubSeq(asyncIoQueues[node], 1, insertionIndex - 1)
       \o <<job>>
       \o SubSeq(
            asyncIoQueues[node], insertionIndex,
            Len(asyncIoQueues[node]))

AsyncServeReservationsAfterIoService(node, job, completedIdentity) ==
  {AsyncServeReservation(
     reservation.node,
     reservation.identity,
     reservation.family,
     reservation.view,
     reservation.ordinal,
     IF reservation.node = node
     THEN reservation.predecessors \ {job}
     ELSE reservation.predecessors,
     reservation.ingressPredecessors,
     reservation.rollbackTombstones):
     reservation \in
       {owned \in asyncServeReservations:
          \/ completedIdentity = NoAsyncItem
          \/ owned.node # node
          \/ owned.identity # completedIdentity}}

AsyncServeReservationsAfterIngressDrain(node, source, laneIndex) ==
  {AsyncServeReservation(
     reservation.node,
     reservation.identity,
     reservation.family,
     reservation.view,
     reservation.ordinal,
     reservation.predecessors,
     IF /\ reservation.node = node
           /\ laneIndex <=
                reservation.ingressPredecessors[source]
     THEN [reservation.ingressPredecessors EXCEPT ![source] = @ - 1]
     ELSE reservation.ingressPredecessors,
     reservation.rollbackTombstones):
     reservation \in asyncServeReservations}

AsyncServeIngressAdmissionsAfterIngressDrain(node, source, laneIndex) ==
  LET item == asyncIngressLanes[node][source][laneIndex]
      drained ==
        {AsyncServeIngressAdmission(
           admission.node,
           admission.identity,
           admission.ordinal,
           admission.schedulerOrdinal,
           IF /\ admission.node = node
                 /\ laneIndex <=
                      admission.ingressPredecessors[source]
           THEN [admission.ingressPredecessors
                   EXCEPT ![source] = @ - 1]
           ELSE admission.ingressPredecessors):
           admission \in asyncServeIngressAdmissions}
  IN IF item.kind \in AsyncReplyRequestKinds
     THEN {admission \in drained:
             \/ admission.node # node
             \/ admission.identity #
                  AsyncServeLogicalRequestIdentity(node, item)}
     ELSE drained

AsyncIoServeNonceOwnership(queue) ==
  \A left, right \in AsyncIoServeIndices(queue):
    queue[left].nonce = queue[right].nonce => left = right

AsyncIoSequenceTyped(queue) ==
  /\ queue \in Seq(Range(queue))
  /\ DOMAIN queue = 1..Len(queue)
  /\ \A index \in 1..Len(queue): AsyncIoJobTyped(queue[index])

(***************************************************************************
Hidden-ingress acceptance and the auxiliary-I/O reservation are one action.  A
new exact request receives the next immutable per-height admission ordinal.
That high-watermark is retained across same-height restart and reconstructed
with the lifecycle tombstone before replay; only successor-height
`AsyncTransportInit` resets it.  Admission freezes the complete current I/O
FIFO and each current ingress-lane prefix before the request becomes visible
to the fair ingress runner.  The reservation is a
logical future-slot ticket even when the physical I/O queue is full.  The
production receiver has one uncommitted Serve barrier, so a new lifecycle
cannot reserve another future slot until that barrier materializes or rolls
back.  A queued exact ingress owner also excludes a later reservation until
the earlier ordinal drains; this keeps the Rust barrier filter aligned with
the cross-source ingress selector.  While the barrier remains off-queue,
ordinary producers observe full effective capacity, and its next physical
slot is exclusive.
Later causal, Consensus, Control, Completion, priority, and Serve work
therefore cannot acquire a position ahead of it.  A duplicate while queued
coalesces behind the existing ingress record and therefore reuses that exact
record's scheduler ordinal.  A post-drain retransmission (including one which
observes a retained tombstone) creates a fresh ingress record and consumes a
fresh shared scheduler ordinal, while retaining the same logical Serve
identity.  It never reacquires the old priority position or allocates another
Serve lifecycle ordinal/stage.
***************************************************************************)
ReserveExactServeCapacity(node, candidate) ==
  LET identity ==
        AsyncServeLogicalRequestIdentity(node, candidate.item)
      family ==
        AsyncServeLifecycleFamily(node, candidate.item)
      roundView == AsyncServeRequestView(candidate.item)
      ordinal == asyncNextServeAdmissionOrdinal[node]
      ingressOrdinal == asyncNextServeIngressOrdinal[node]
      schedulerOrdinal == AsyncNextCandidateLifecycleOrdinal(node)
  IN /\ candidate.kind = "AcceptCertifiedRequest"
     /\ candidate.item.kind \in AsyncReplyRequestKinds
     /\ ~AsyncServeIngressAdmissionOwned(node, identity)
     /\ AsyncServeIngressLifecycleOwnerIdentities(node) = {}
     /\ AsyncServeOffQueueReservations(node) = {}
     /\ ~AsyncServeLifecycleFamilyOwned(node, family)
     /\ AsyncServeFamilyAdvanceRetiresPriorRequests(
          node, candidate.item)
     /\ UNCHANGED asyncIoQueues
     /\ asyncNextServeAdmissionOrdinal' =
          [asyncNextServeAdmissionOrdinal EXCEPT ![node] = @ + 1]
     /\ asyncNextServeIngressOrdinal' =
          [asyncNextServeIngressOrdinal EXCEPT ![node] = @ + 1]
     /\ asyncServeIngressAdmissions' =
          asyncServeIngressAdmissions
            \cup {AsyncServeIngressAdmission(
                    node, identity, ingressOrdinal, schedulerOrdinal,
                    AsyncServeIngressPredecessorCounts(node))}
     /\ asyncServeAdmissions' =
          asyncServeAdmissions
            \cup {AsyncServeAdmission(
                    node, identity, family, roundView, ordinal)}
     /\ asyncServeReservations' =
          asyncServeReservations
            \cup {AsyncServeReservation(
                    node, identity, family, roundView, ordinal,
                    {asyncIoQueues[node][index]:
                       index \in 1..Len(asyncIoQueues[node])},
                    AsyncServeIngressPredecessorCounts(node), {})}
     /\ UNCHANGED asyncServeTombstones

AdvanceExactServeCapacity(node, candidate) ==
  LET identity ==
        AsyncServeLogicalRequestIdentity(node, candidate.item)
      family ==
        AsyncServeLifecycleFamily(node, candidate.item)
      roundView == AsyncServeRequestView(candidate.item)
      ordinal == asyncNextServeAdmissionOrdinal[node]
      ingressOrdinal == asyncNextServeIngressOrdinal[node]
      schedulerOrdinal == AsyncNextCandidateLifecycleOrdinal(node)
  IN /\ candidate.kind = "AcceptCertifiedRequest"
     /\ candidate.item.kind \in AsyncReplyRequestKinds
     /\ ~AsyncServeIngressAdmissionOwned(node, identity)
     /\ AsyncServeIngressLifecycleOwnerIdentities(node) = {}
     /\ AsyncServeOffQueueReservations(node) = {}
     /\ ~AsyncServeLifecycleOwned(node, identity)
     /\ AsyncServeFamilyTombstoneRecords(node, family) # {}
     /\ roundView > AsyncServeFamilyHighWatermark(node, family)
     /\ AsyncServeFamilyAdvanceRetiresPriorRequests(
          node, candidate.item)
     /\ UNCHANGED asyncIoQueues
     /\ asyncNextServeAdmissionOrdinal' =
          [asyncNextServeAdmissionOrdinal EXCEPT ![node] = @ + 1]
     /\ asyncNextServeIngressOrdinal' =
          [asyncNextServeIngressOrdinal EXCEPT ![node] = @ + 1]
     /\ asyncServeIngressAdmissions' =
          asyncServeIngressAdmissions
            \cup {AsyncServeIngressAdmission(
                    node, identity, ingressOrdinal, schedulerOrdinal,
                    AsyncServeIngressPredecessorCounts(node))}
     /\ asyncServeAdmissions' =
          asyncServeAdmissions
            \cup {AsyncServeAdmission(
                    node, identity, family, roundView, ordinal)}
     /\ asyncServeReservations' =
          asyncServeReservations
            \cup {AsyncServeReservation(
                    node, identity, family, roundView, ordinal,
                    {asyncIoQueues[node][index]:
                       index \in 1..Len(asyncIoQueues[node])},
                    AsyncServeIngressPredecessorCounts(node),
                    AsyncServeFamilyTombstoneRecords(node, family))}
     /\ asyncServeTombstones' =
          AsyncServeTombstonesWithoutFamily(node, family)

CoalesceExactServeIngressCapacity(node, candidate) ==
  LET identity ==
        AsyncServeLogicalRequestIdentity(node, candidate.item)
      family ==
        AsyncServeLifecycleFamily(node, candidate.item)
      roundView == AsyncServeRequestView(candidate.item)
      ingressOrdinal == asyncNextServeIngressOrdinal[node]
      schedulerOrdinal == AsyncNextCandidateLifecycleOrdinal(node)
  IN /\ candidate.kind = "AcceptCertifiedRequest"
     /\ candidate.item.kind \in AsyncReplyRequestKinds
     /\ ~AsyncServeIngressAdmissionOwned(node, identity)
     /\ \/ AsyncServeLifecycleOwned(node, identity)
        \/ /\ AsyncServeLifecycleFamilyOwned(node, family)
              /\ roundView <=
                   AsyncServeFamilyHighWatermark(node, family)
     /\ asyncNextServeIngressOrdinal' =
          [asyncNextServeIngressOrdinal EXCEPT ![node] = @ + 1]
     /\ asyncServeIngressAdmissions' =
          asyncServeIngressAdmissions
            \cup {AsyncServeIngressAdmission(
                    node, identity, ingressOrdinal, schedulerOrdinal,
                    AsyncServeIngressPredecessorCounts(node))}
     /\ UNCHANGED <<asyncIoQueues, AsyncServeLifecycleVars>>

AcceptOrReserveExactServeIngress(node, candidate) ==
  \/ ReserveExactServeCapacity(node, candidate)
  \/ AdvanceExactServeCapacity(node, candidate)
  \/ CoalesceExactServeIngressCapacity(node, candidate)

ResumeExactServeCapacity(node, candidate) ==
  LET identity ==
        AsyncServeLogicalRequestIdentity(node, candidate.item)
      job == AsyncIoCertifiedServeJob(node, candidate)
  IN /\ candidate.kind = "AcceptCertifiedRequest"
     /\ candidate.item.kind \in AsyncReplyRequestKinds
     /\ CanResumeExactServeCapacity(node, identity)
     /\ asyncIoQueues' =
          [asyncIoQueues EXCEPT
             ![node] =
               AsyncIoQueueWithResumedServe(node, identity, job)]
     /\ UNCHANGED <<asyncNextServeAdmissionOrdinal,
                     asyncServeAdmissions, asyncServeTombstones>>

CoalesceExactServeCapacity(node, candidate) ==
  LET identity ==
        AsyncServeLogicalRequestIdentity(node, candidate.item)
  IN /\ candidate.kind = "AcceptCertifiedRequest"
     /\ candidate.item.kind \in AsyncReplyRequestKinds
     /\ AsyncServeLifecycleOwned(node, identity)
     /\ \/ AsyncServeLifecycleTombstone(node, identity)
        \/ AsyncServeJobQueued(node, identity)
     /\ UNCHANGED <<asyncIoQueues, asyncNextServeAdmissionOrdinal,
                     asyncServeAdmissions, asyncServeTombstones>>

AsyncCompletionSequenceTyped(queue) ==
  /\ queue \in Seq(Range(queue))
  /\ DOMAIN queue = 1..Len(queue)
  /\ \A index \in 1..Len(queue):
       /\ AsyncCandidateTyped(queue[index])
       /\ queue[index].class = "Completion"

QueuedCompletionIndices(node) ==
  {index \in 1..Len(asyncCommandQueues[node]):
     asyncCommandQueues[node][index].class = "Completion"}

QueuedCompletionCount(node) == Cardinality(QueuedCompletionIndices(node))

DeferredCompletionCount(node) ==
  Len(asyncDeferredCompletionQueues[node])

AsyncOutstandingWorkCount(node) ==
  Cardinality(asyncOutstandingWork[node])

(***************************************************************************
`AsyncOutstandingWorkCount` is the executor's effect-work ownership: it is
released only when a producer completion enters the serialized runtime queue.
Queued and Busy-deferred completions consume the independent runtime/adapter
lanes, not another executor work slot.  `AsyncCompletionLoad` remains the
total diagnostic ownership count used by service ranks; it must not be used
as the effect-work admission limit.  Conflating these capacities lets a stale
Busy-deferred completion block the exact persistence completion which clears
Busy, even though the production executor has already released that work
slot.
***************************************************************************)
AsyncCompletionLoad(node) ==
  AsyncOutstandingWorkCount(node) + QueuedCompletionCount(node)
    + DeferredCompletionCount(node)

CanEnqueueClass(node, commandClass) ==
  CASE commandClass = "Normal" ->
         AsyncQueueDepth(node) < AsyncNormalLimit
    [] commandClass = "Progress" ->
         AsyncQueueDepth(node) < AsyncProgressLimit
    [] commandClass = "Completion" ->
         AsyncQueueDepth(node) < AsyncOrdinaryCompletionLimit

CanEnqueueCertifiedResponse(node) ==
  AsyncQueueDepth(node) < AsyncQueueCapacity

SequenceSet(sequence) == {sequence[index]: index \in 1..Len(sequence)}

SequenceHasUniqueValues(sequence) ==
  Len(sequence) = Cardinality(SequenceSet(sequence))

QueuedCandidates ==
  UNION {SequenceSet(asyncCommandQueues[node]): node \in ValidatorIds}

DeferredCandidates ==
  UNION
    {SequenceSet(asyncDeferredCompletionQueues[node])
       \cup SequenceSet(asyncDeferredProgressQueues[node])
       \cup SequenceSet(asyncDeferredNormalQueues[node]):
       node \in ValidatorIds}

CausalCandidates ==
  UNION {SequenceSet(asyncCausalQueues[node]): node \in ValidatorIds}

TrackedWorkCandidates ==
  UNION {asyncOutstandingWork[node]: node \in ValidatorIds}

ResponsiveReplayScheduledCandidates(node) ==
  {candidate \in QueuedCandidates \cup DeferredCandidates
                    \cup CausalCandidates \cup TrackedWorkCandidates:
     candidate.node = node}

CandidateScheduledIn(candidate, commandQueues,
                     deferredCompletionQueues, deferredProgressQueues,
                     deferredNormalQueues, causalQueues, outstandingWork) ==
  \* Keep every prefix UNION parenthesized.  Without these parentheses TLA+
  \* parses the old surface form as UNION({command queue sets} \cup
  \* UNION(...)), rather than as the union of the four candidate carriers.
  candidate \in
    (UNION {SequenceSet(commandQueues[node]): node \in ValidatorIds})
      \cup (UNION
            {SequenceSet(deferredCompletionQueues[node])
               \cup SequenceSet(deferredProgressQueues[node])
               \cup SequenceSet(deferredNormalQueues[node]):
               node \in ValidatorIds})
      \cup (UNION {SequenceSet(causalQueues[node]): node \in ValidatorIds})
      \cup (UNION {outstandingWork[node]: node \in ValidatorIds})

CandidateScheduled(candidate) ==
  CandidateScheduledIn(
    candidate, asyncCommandQueues,
    asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncCausalQueues, asyncOutstandingWork)

CandidateScheduledAfter(candidate) ==
  CandidateScheduledIn(
    candidate, asyncCommandQueues',
    asyncDeferredCompletionQueues', asyncDeferredProgressQueues',
    asyncDeferredNormalQueues', asyncCausalQueues', asyncOutstandingWork')

AsyncScheduledCandidateOriginsForNodeIn(
    node, commandQueues, deferredCompletionQueues,
    deferredProgressQueues, deferredNormalQueues,
    causalQueues, outstandingWork) ==
  LET scheduled ==
        (UNION {SequenceSet(commandQueues[owner]):
                  owner \in ValidatorIds})
          \cup (UNION
                {SequenceSet(deferredCompletionQueues[owner])
                   \cup SequenceSet(deferredProgressQueues[owner])
                   \cup SequenceSet(deferredNormalQueues[owner]):
                   owner \in ValidatorIds})
          \cup (UNION {SequenceSet(causalQueues[owner]):
                        owner \in ValidatorIds})
          \cup (UNION {outstandingWork[owner]:
                        owner \in ValidatorIds})
      owned == {candidate \in scheduled: candidate.node = node}
  IN {candidate.causalOrigin: candidate \in owned}

AsyncScheduledCandidateOriginsForNode(node) ==
  AsyncScheduledCandidateOriginsForNodeIn(
    node, asyncCommandQueues,
    asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncCausalQueues, asyncOutstandingWork)

AsyncScheduledCandidateOriginsForNodeAfter(node) ==
  AsyncScheduledCandidateOriginsForNodeIn(
    node, asyncCommandQueues',
    asyncDeferredCompletionQueues', asyncDeferredProgressQueues',
    asyncDeferredNormalQueues', asyncCausalQueues', asyncOutstandingWork')

AsyncCandidateLifecycleRecordCoversScheduledOrigin(record) ==
  record.origin \in AsyncScheduledCandidateOriginsForNode(record.node)

AsyncCandidateLifecycleActiveRecords ==
  {record \in AsyncCandidateLifecycleAdmissions: ~record.retired}

AsyncCandidateLifecycleSchedulerCoverageInvariant ==
  /\ \A node \in ValidatorIds:
       \A origin \in AsyncScheduledCandidateOriginsForNode(node):
         /\ AsyncCandidateLifecycleRecorded(node, origin)
         /\ ~AsyncCandidateLifecycleRecordFor(node, origin).retired
  /\ \A record \in AsyncCandidateLifecycleActiveRecords:
       AsyncCandidateLifecycleRecordCoversScheduledOrigin(record)

AsyncScheduledCandidateServiceIdentities ==
  {AsyncCandidateServiceIdentity(candidate):
     candidate \in
       QueuedCandidates \cup DeferredCandidates
         \cup CausalCandidates \cup TrackedWorkCandidates}

AsyncScheduledCandidateAdmissionIdentities ==
  {AsyncCandidateAdmissionIdentity(candidate):
     candidate \in
       QueuedCandidates \cup DeferredCandidates
         \cup CausalCandidates \cup TrackedWorkCandidates}

AsyncCandidateServiceIdentityScheduled(candidate) ==
  AsyncCandidateServiceIdentity(candidate)
    \in AsyncScheduledCandidateServiceIdentities

CandidateAdmissionCoalesced(candidate) ==
  AsyncCandidateServiceIdentityScheduled(candidate)
    \/ AsyncCandidateServiceCoalesced(candidate)

(***************************************************************************
Responsive restart does not create a second durable owner.  The authority
below is a generation-free ghost projection of the exact PrepareQC already
retained by `DurableState::locked()` before a crash.  It records only the
semantic lock identity needed while process-local scheduler ownership is
rebuilt.  The projection is registered atomically when the responsive-crash
lifecycle starts, survives the generation change and replay reset, and is
retired only when either the durable source is legitimately gone or an exact
current-generation FetchBody owner has been materialized.
***************************************************************************)

HistoricalLockRestartAuthoritySourceKernel(
    authority, qc, currentContext, currentNodeView,
    currentLockRank, currentLockSubject, currentInstalledTCs,
    currentCommitIntents, currentDecisions) ==
  /\ authority =
       AsyncHistoricalLockRestartAuthority(authority.node, qc)
  /\ qc.context = currentContext
  /\ qc.phase = "Prepare"
  /\ qc.view < currentNodeView[authority.node]
  /\ qc.view = currentLockRank[authority.node]
  /\ qc.subject = currentLockSubject[authority.node]
  /\ \/ \E installed \in currentInstalledTCs:
          /\ installed.node = authority.node
          /\ installed.tc.context = qc.context
          /\ installed.tc.view >= qc.view
          /\ installed.tc.highestPrepareQc = qc
     \/ \E vote \in currentCommitIntents:
          /\ vote.signer = authority.node
          /\ vote.context = currentContext
          /\ vote.phase = "Commit"
          /\ vote.view = qc.view
          /\ vote.subject = qc.subject
  /\ ~\E decision \in currentDecisions:
       /\ decision.node = authority.node
       /\ decision.qc.context = currentContext

HistoricalLockRestartAuthoritySource(authority) ==
  \E qc \in prepareQCs:
    HistoricalLockRestartAuthoritySourceKernel(
      authority, qc, context, nodeView, lockRank, lockSubject,
      installedTCs, commitIntents, decisions)

HistoricalLockRestartAuthoritySourceAfter(authority) ==
  \E qc \in prepareQCs':
    HistoricalLockRestartAuthoritySourceKernel(
      authority, qc, context', nodeView', lockRank', lockSubject',
      installedTCs', commitIntents', decisions')

HistoricalLockRestartExactCurrentFetchKernel(
    authority, qc, candidate, currentContext, currentNodeView,
    currentGeneration, commandQueues, deferredCompletionQueues,
    deferredProgressQueues, deferredNormalQueues, causalQueues,
    outstandingWork) ==
  /\ candidate.class = "Completion"
  /\ candidate.kind = "FetchBody"
  /\ candidate.node = authority.node
  /\ candidate.height = authority.context.height
  /\ candidate.view = authority.view
  /\ candidate.subject = authority.subject
  /\ candidate.item = NoAsyncItem
  /\ candidate.consumerContext = authority.context
  /\ candidate.consumerContext = currentContext
  /\ candidate.consumerView = currentNodeView[authority.node]
  /\ candidate.consumerGeneration = currentGeneration[authority.node]
  /\ candidate.evidence = qc
  /\ candidate.bodyIdentity = authority.subject
  /\ candidate.manifestIdentity = authority.subject
  /\ candidate.commitmentIdentity = authority.subject
  /\ CandidateScheduledIn(
       candidate, commandQueues,
       deferredCompletionQueues, deferredProgressQueues,
       deferredNormalQueues, causalQueues, outstandingWork)

HistoricalLockRestartExactCurrentFetchOwner(authority) ==
  \E qc \in prepareQCs, candidate \in AsyncCandidateSet:
    /\ HistoricalLockRestartAuthoritySourceKernel(
         authority, qc, context, nodeView, lockRank, lockSubject,
         installedTCs, commitIntents, decisions)
    /\ HistoricalLockRestartExactCurrentFetchKernel(
         authority, qc, candidate, context, nodeView, generation,
         asyncCommandQueues, asyncDeferredCompletionQueues,
         asyncDeferredProgressQueues, asyncDeferredNormalQueues,
         asyncCausalQueues, asyncOutstandingWork)

HistoricalLockRestartExactCurrentFetchOwnerAfter(authority) ==
  \E qc \in prepareQCs', candidate \in AsyncCandidateSet:
    /\ HistoricalLockRestartAuthoritySourceKernel(
         authority, qc, context', nodeView', lockRank', lockSubject',
         installedTCs', commitIntents', decisions')
    /\ HistoricalLockRestartExactCurrentFetchKernel(
         authority, qc, candidate, context', nodeView', generation',
         asyncCommandQueues', asyncDeferredCompletionQueues',
         asyncDeferredProgressQueues', asyncDeferredNormalQueues',
         asyncCausalQueues', asyncOutstandingWork')

ResponsiveCrashRecoveryRegistration(node) ==
  /\ asyncRecoveryPhase = "Eligible"
  /\ asyncRecoveryPhase' = "RestartRequired"
  /\ asyncRecoveryNode' = node
  /\ node \in Responsive

ResponsiveCrashHistoricalLockRestartAuthorities(node) ==
  {AsyncHistoricalLockRestartAuthority(node, qc):
     qc \in {candidateQc \in prepareQCs:
       HistoricalLockRestartAuthoritySourceKernel(
         AsyncHistoricalLockRestartAuthority(node, candidateQc), candidateQc,
         context, nodeView, lockRank, lockSubject,
         installedTCs, commitIntents, decisions)}}

AsyncHistoricalLockRestartAuthorityTransition ==
  \/ \E node \in ValidatorIds:
       /\ ResponsiveCrashRecoveryRegistration(node)
       /\ asyncHistoricalLockRestartAuthorities' =
            asyncHistoricalLockRestartAuthorities
              \cup ResponsiveCrashHistoricalLockRestartAuthorities(node)
  \/ /\ ~\E node \in ValidatorIds:
             ResponsiveCrashRecoveryRegistration(node)
     /\ asyncHistoricalLockRestartAuthorities' =
          {authority \in asyncHistoricalLockRestartAuthorities:
             /\ HistoricalLockRestartAuthoritySourceAfter(authority)
             /\ ~HistoricalLockRestartExactCurrentFetchOwnerAfter(authority)}

\* Every action named by weak fairness is the exact fully framed AsyncNext arm,
\* not only its inner scheduler or reducer component.  These suffixes bind
\* every otherwise-outer primed variable before TLC evaluates ENABLED.  Do not
\* conjoin the complete Core `Next` relation here: the action itself already
\* supplies an exact Core transition or `UNCHANGED vars`, and redundantly
\* searching every Core branch makes ENABLED both noisy and needlessly costly.
\* `AsyncFairActionsRefineAsyncNext` states the typed executable-relation
\* claim once, outside the fairness queries;
\* `SumeragiV2AsyncFairnessRefinementProofs` owns its deductive discharge
\* without changing this executable action relation.
AsyncCoreOuterFrame ==
  UNCHANGED <<height, context>>

AsyncNonCrashOuterFrame ==
  /\ UNCHANGED up
  /\ UNCHANGED asyncServiceActivationState
  /\ UNCHANGED AsyncRecoveryControlVars
  /\ AsyncHistoricalLockRestartAuthorityTransition
  /\ AsyncCoreOuterFrame

AsyncNonRunnerOuterFrame ==
  /\ UNCHANGED asyncNodeServiceDeadlines
  /\ AsyncNonCrashOuterFrame

AsyncRecoveryOuterFrame ==
  /\ UNCHANGED up
  /\ UNCHANGED asyncServiceActivationState
  /\ AsyncHistoricalLockRestartAuthorityTransition
  /\ AsyncCoreOuterFrame

(***************************************************************************
Every logical reducer candidate has one scheduler owner.  This is stronger
than the individual FIFO typing facts: an exact candidate may not occur in a
second queue, move into executor work while retaining its old owner, or be
re-created behind itself by a causal successor batch.  The invariant follows
the production idempotent reducer, serialized WAL/signing and Busy-deferred
owners, plus the executor's exact round/subject and work-ID coalescing
boundaries.
***************************************************************************)
AsyncLogicalCandidateOwnershipInvariant ==
  /\ \A node \in ValidatorIds:
       /\ SequenceHasUniqueValues(asyncCommandQueues[node])
       /\ SequenceHasUniqueValues(asyncCausalQueues[node])
       /\ SequenceHasUniqueValues(asyncDeferredCompletionQueues[node])
       /\ SequenceHasUniqueValues(asyncDeferredProgressQueues[node])
       /\ SequenceHasUniqueValues(asyncDeferredNormalQueues[node])
  /\ QueuedCandidates \cap DeferredCandidates = {}
  /\ QueuedCandidates \cap CausalCandidates = {}
  /\ QueuedCandidates \cap TrackedWorkCandidates = {}
  /\ DeferredCandidates \cap CausalCandidates = {}
  /\ DeferredCandidates \cap TrackedWorkCandidates = {}
  /\ CausalCandidates \cap TrackedWorkCandidates = {}

EnqueueCandidate(candidate) ==
  LET node == candidate.node
  IN /\ ~AsyncCandidateServiceCoalesced(candidate)
     /\ asyncCommandQueues' =
          [asyncCommandQueues EXCEPT ![node] = Append(@, candidate)]
     /\ UNCHANGED asyncNextCommandClass

NodeQueueNonempty(node) == Len(asyncCommandQueues[node]) > 0

(***************************************************************************
The production bounded ingress retains its Completion/Progress/Normal cursor
as the tie breaker among equal lifecycle ordinals.  Across classes it first
selects the oldest immutable causal lifecycle and removes that lifecycle's
first command in the selected class.  Physical enqueue ordinals remain unique
diagnostics; causal children keep the parent's lifecycle ordinal, so a later
Completion or Control root cannot jump ahead of an admitted Normal target.
***************************************************************************)
NextCommandClass(commandClass) ==
  CASE commandClass = "Completion" -> "Progress"
    [] commandClass = "Progress" -> "Normal"
    [] OTHER -> "Completion"

CommandClassIndices(node, commandClass) ==
  {index \in 1..Len(asyncCommandQueues[node]):
     asyncCommandQueues[node][index].class = commandClass}

NodeCommandLifecycleOrdinals(node) ==
  {AsyncCandidateLifecycleOrdinal(asyncCommandQueues[node][index]):
     index \in 1..Len(asyncCommandQueues[node])}

OldestNodeCommandLifecycleOrdinal(node) ==
  CHOOSE ordinal \in NodeCommandLifecycleOrdinals(node):
    \A other \in NodeCommandLifecycleOrdinals(node): ordinal <= other

OldestCommandClassIndices(node, commandClass) ==
  {index \in CommandClassIndices(node, commandClass):
     AsyncCandidateLifecycleOrdinal(asyncCommandQueues[node][index])
       = OldestNodeCommandLifecycleOrdinal(node)}

FirstOldestCommandClassIndex(node, commandClass) ==
  CHOOSE index \in OldestCommandClassIndices(node, commandClass):
    \A other \in OldestCommandClassIndices(node, commandClass):
      index <= other

CommandClassOwnsOldestLifecycle(node, commandClass) ==
  OldestCommandClassIndices(node, commandClass) # {}

SelectedCommandClass(node) ==
  LET first == asyncNextCommandClass[node]
      second == NextCommandClass(first)
      third == NextCommandClass(second)
  IN IF CommandClassOwnsOldestLifecycle(node, first)
     THEN first
     ELSE IF CommandClassOwnsOldestLifecycle(node, second)
          THEN second
          ELSE third

NextNodeCommandIndex(node) ==
  FirstOldestCommandClassIndex(node, SelectedCommandClass(node))

NextNodeCommand(node) ==
  asyncCommandQueues[node][NextNodeCommandIndex(node)]

SequenceWithoutIndex(sequence, index) ==
  SubSeq(sequence, 1, index - 1)
    \o SubSeq(sequence, index + 1, Len(sequence))

RemoveNextNodeCommand(node) ==
  /\ asyncCommandQueues' =
       [asyncCommandQueues EXCEPT
          ![node] = SequenceWithoutIndex(@, NextNodeCommandIndex(node))]
  /\ asyncNextCommandClass' =
       [asyncNextCommandClass EXCEPT
          ![node] = NextCommandClass(SelectedCommandClass(node))]

NodeHasDecision(node) ==
  \E decision \in decisions:
    /\ decision.node = node
    /\ decision.qc.context = context
    /\ decision.qc.phase = "Commit"

AsyncCandidateAdmissionIdentityObsolete(identity) ==
  LET consumer == identity.consumer
      item == identity.service.payload.item
  IN \/ consumer.context # context
     \/ consumer.height # height
     \/ identity.service.height # height
     \/ consumer.view < nodeView[consumer.node]
     \/ consumer.generation # generation[consumer.node]
     \/ /\ identity.service.phase = "DeliverChunk"
           /\ \/ NodeHasDecision(consumer.node)
              \/ /\ item.kind = "Chunk"
                    /\ AsyncChunkReceipt(
                         consumer.node, identity.service.view,
                         identity.service.subject,
                         item.envelope.chunk) \in asyncHeldChunks

AsyncCandidateAdmissionIdentityTerminallyCovered(identity) ==
  \/ AsyncCandidateTerminalIdentityTombstoned(identity.service)
  \/ AsyncCandidateAdmissionIdentityObsolete(identity)

AsyncCandidateAdmissionIdentityLifecycleCovered(identity) ==
  \/ AsyncCandidateTransientServiceIdentityMarked(identity.service)
  \/ AsyncCandidateAdmissionIdentityTerminallyCovered(identity)

NodeHasApplication(node) ==
  \E application \in applied:
    /\ application.node = node
    /\ application.qc.context = context
    /\ application.qc.phase = "Commit"

(***************************************************************************
Archive-service identities are deliberately separate from voting identities.
The production archive snapshot may contain authenticated zero-power or
non-roster peers.  This model relies for post-GST service only on the
Responsive members which are up and already hold the exact applied receipt.
Neither set is used by any vote, QC, pacemaker, or ordinary RunNode action.
***************************************************************************)
AsyncResponsiveArchiveServers ==
  Responsive \cap AsyncArchiveServerIds

AsyncResponsiveOnlineArchiveServers ==
  AsyncResponsiveArchiveServers \cap up

AsyncResponsiveAppliedArchiveServers ==
  {node \in AsyncResponsiveOnlineArchiveServers:
     NodeHasApplication(node)}

AsyncArchiveIoServiceNodes ==
  AsyncActiveServiceNodes
    \cap (AsyncCurrentResponsiveVoters
           \cup AsyncResponsiveAppliedArchiveServers)

AsyncTimedServiceNodes ==
  AsyncArchiveIoServiceNodes
    \cup (asyncHistoricalRecoveryTargets \cap AsyncActiveServiceNodes)

ConcreteDecisionNodes ==
  {node \in AsyncCurrentResponsiveVoters: NodeHasDecision(node)}

ConcreteAppliedNodes ==
  {node \in AsyncCurrentResponsiveVoters: NodeHasApplication(node)}

AsyncProposalSubject(node) ==
  IF highestRank[node] = NoRank
  THEN AsyncHeartbeatSubject
  ELSE highestSubject[node]

AsyncActiveViews ==
  {nodeView[node]: node \in ValidatorIds}
    \cup {entry.vote.view: entry \in receivedVotes}
    \cup {entry.vote.view: entry \in receivedTimeoutVotes}

CandidateForRequest(commandClass, kind, request, roundView, subject) ==
  AsyncCandidate(commandClass, kind, request.node, request.vote.context.height,
                 roundView, subject, NoAsyncItem)

AsyncNoItemCandidateCausalOriginAt(
    commandClass, kind, node, ownerContext, roundView, subject) ==
  AsyncCandidateCausalOrigin(
    kind, node, ownerContext.height, roundView,
    subject, NoAsyncItem, ownerContext, NoAsyncItem,
    subject, subject, subject)

NoItemCandidate(commandClass, kind, node, roundView, subject) ==
  AsyncCandidateWithIdentityAndOrigin(
    commandClass, kind, node, context.height, roundView,
    subject, NoAsyncItem, context, nodeView[node], generation[node],
    NoAsyncItem, subject, subject, subject,
    AsyncNoItemCandidateCausalOriginAt(
      commandClass, kind, node, context, roundView, subject))

(***************************************************************************
Causal adapter work.  A candidate exists only because the immediately
preceding serialized command emitted it.  There is no global ENABLED scan and
no favorable command ranking.  The sequence order below is the adapter effect
order; stale speculative continuations are still consumed FIFO and discarded
by the exact Core guard.  Proposal delivery first schedules
`RebindRetainedBody`.  That completion uses the retained locked-body authority
through the exact target-round `RebindRetainedBody`/Available boundary, then
follows the ordinary StoreBody -> ValidateBody chain.  The target round still
mints its own durable body receipt and generation-bound validation marker.  If
the exact canonical bytes already have a durable validation witness in an
earlier view of the same height context, production reuses that witness's
deterministic execution commitment instead of executing the body again; this
model's `ValidateBody` action abstracts both paths.  The direct `BeginPrepare`
successor remains the fast path when current-round evidence is already present,
and successful validation schedules a second prepare attempt.
***************************************************************************)

CausalCandidate(commandClass, kind, command) ==
  AsyncCandidateFrom(commandClass, kind, command)

CausalCandidateWithEvidence(commandClass, kind, command, evidence) ==
  AsyncCandidateWithIdentityAndOrigin(
    commandClass, kind, command.node, context.height, command.view,
    command.subject, NoAsyncItem,
    command.consumerContext, command.consumerView,
    command.consumerGeneration, evidence,
    command.bodyIdentity, command.manifestIdentity,
    command.commitmentIdentity, command.causalOrigin)

RetainedBodyRebindCandidate(command) ==
  CausalCandidate("Completion", "RebindRetainedBody", command)

InstallTcEvidenceMatches(command, tc) ==
  \/ command.evidence = tc
  \/ \E item \in AsyncNetworkItems:
       /\ command.evidence = item
       /\ item.kind = "TimeoutCertificate"
       /\ item.envelope.tc = tc

InstallTcFromEvidence(command) ==
  IF command.evidence \in TcRecordSet
  THEN command.evidence
  ELSE command.evidence.envelope.tc

InstallRequests(command) ==
  {installRequest \in pendingInstallTC:
    /\ command.node = installRequest.node
    /\ command.view = installRequest.tc.view
    /\ InstallTcEvidenceMatches(command, installRequest.tc)}

InstallCommitSignRequests(command) ==
  {signRequest \in VoteSignSet:
    \E installRequest \in InstallRequests(command):
      signRequest \in
        ActiveLockedCommitSignRequestsAfterInstall(
          installRequest.node, installRequest.tc)}

InstallGenerationAfter(command) ==
  LET requests == InstallRequests(command)
  IN IF requests = {}
     THEN generation[command.node]
     ELSE LET request == CHOOSE pending \in requests: TRUE
          IN IF StrictSameRoundTcUpgrade(request.node, request.tc)
             THEN generation[request.node] + 1
             ELSE 0

InstallCommitSignSuccessor(command) ==
  LET signRequest ==
        CHOOSE request \in InstallCommitSignRequests(command): TRUE
  IN AsyncCandidateAtConsumerWithOrigin(
       "Completion", "SignVote", signRequest.node,
       signRequest.vote.context.height, signRequest.vote.view,
       signRequest.vote.subject, NoAsyncItem, command.view + 1,
       InstallGenerationAfter(command),
       signRequest.vote, signRequest.vote.subject,
       signRequest.vote.subject, signRequest.vote.subject,
       command.causalOrigin)

(***************************************************************************
Production retains the full `durable.locked()` PrepareQC.  The TC and durable
lock now carry that same QcRecord, so recovery takes the exact resulting value
directly.  No rank/subject search or signer-set tie breaker is admissible.
***************************************************************************)

InstallResultingLockedPrepareQCs(command) ==
  LET qc ==
        ResultingInstallLockPrepareQc(
          command.node, InstallTcFromEvidence(command))
  IN IF InstallRequests(command) = {} \/ qc = NoPrepareQC
     THEN {}
     ELSE {qc}

InstallLockedFetchSuccessor(command) ==
  LET qc ==
        ResultingInstallLockPrepareQc(
          command.node, InstallTcFromEvidence(command))
  IN AsyncCandidateAtConsumerWithOrigin(
       "Completion", "FetchBody", command.node, qc.context.height,
       qc.view, qc.subject, NoAsyncItem, command.view + 1,
       InstallGenerationAfter(command),
       qc, qc.subject, qc.subject, qc.subject, command.causalOrigin)

InstallLockedFetchSuccessors(command) ==
  IF InstallResultingLockedPrepareQCs(command) = {}
  THEN <<>>
  ELSE <<InstallLockedFetchSuccessor(command)>>

InstallCommitSignSuccessors(command) ==
  IF InstallCommitSignRequests(command) = {}
  THEN <<>>
  ELSE <<InstallCommitSignSuccessor(command)>>

(***************************************************************************
`AppendCausalSuccessors` is conjoined with `PersistInstallTC`, so its
constructors evaluate in the pre-state.  Derive the AssembleBody subject from
the exact pending TC to predict the post-install high reference; using the
unprimed `AsyncProposalSubject` here would freeze the superseded subject into
the new-generation candidate identity.
***************************************************************************)
InstallProposalSubject(command) ==
  LET requests == InstallRequests(command)
  IN IF requests = {}
     THEN AsyncProposalSubject(command.node)
     ELSE LET tc == InstallTcFromEvidence(command)
              selectedRank == TcHighRank(tc)
          IN IF selectedRank > highestRank[command.node]
             THEN TcHighSubject(tc)
             ELSE AsyncProposalSubject(command.node)

InstallProposalSuccessor(command) ==
  LET subject == InstallProposalSubject(command)
  IN AsyncCandidateAtConsumerWithOrigin(
       "Normal", "AssembleBody", command.node, context.height,
       command.view + 1, subject, NoAsyncItem, command.view + 1,
       InstallGenerationAfter(command),
       command.evidence, subject, subject, subject, command.causalOrigin)

(***************************************************************************
TC acknowledgement clears production `body_work`, emits exact locked-body
Fetch before driving the active signature queue, and then resumes ordinary
next-view proposal work.  The Fetch is emitted for every resulting lock,
including one which already has a durable Commit intent; exact candidate
coalescing prevents duplicate scheduler ownership.  A lock without an intent
passes Fetch/Store/Validate before the ordinary WAL-backed BeginLockCommit.
***************************************************************************)
InstallCommandSuccessors(command) ==
  InstallLockedFetchSuccessors(command)
    \o InstallCommitSignSuccessors(command)
    \o <<InstallProposalSuccessor(command)>>

DecisionFetchFrontier(command) ==
  \E qc \in DecisionQcValues:
    /\ command.kind = "FetchBody"
    /\ command.node \in ValidatorIds
    /\ command.height = qc.context.height
    /\ command.view = qc.view
    /\ command.subject = qc.subject
    /\ command.evidence = qc
    /\ DecisionCertifiedBodyRecoveryAuthority(command.node, qc)

LockedPrepareFetchFrontier(command) ==
  \E qc \in prepareQCs:
    /\ command.kind = "FetchBody"
    /\ command.node \in ValidatorIds
    /\ command.height = qc.context.height
    /\ command.view = qc.view
    /\ command.subject = qc.subject
    /\ command.evidence = qc
    /\ LockedPrepareRecoverySource(command.node, qc)

CertifiedRecoveryFetchFrontier(command) ==
  \/ DecisionFetchFrontier(command)
  \/ LockedPrepareFetchFrontier(command)

PersistDecisionRequests(command) ==
  {request \in pendingDecision:
    /\ request.node = command.node
    /\ request.qc.context.height = command.height
    /\ request.qc.view = command.view
    /\ request.qc.subject = command.subject}

PersistDecisionRequest(command) ==
  CHOOSE request \in PersistDecisionRequests(command): TRUE

PersistDecisionBody(command) ==
  LET request == PersistDecisionRequest(command)
      qc == request.qc
  IN BodyRecord(request.node, qc.context, qc.view, qc.subject)

PersistDecisionValidationHeld(command) ==
  LET request == PersistDecisionRequest(command)
      qc == request.qc
  IN \E validation \in validatedBodies:
       /\ validation.node = request.node
       /\ validation.context = qc.context
       /\ validation.view = qc.view
       /\ validation.subject = qc.subject

(***************************************************************************
Decision persistence acknowledges the exact durable-body state observed by
the reducer.  Missing, adapter-available, durable, and already-validated bodies
continue respectively with Fetch, Store, Validate, and Apply.  Classifying the
successor here is essential: sending a held-and-validated body through Fetch
would stutter, enqueue a disabled Validate, and lose the Apply frontier.
***************************************************************************)
PersistDecisionRecoveryKind(command) ==
  LET request == PersistDecisionRequest(command)
      qc == request.qc
  IN IF BodyHeldBy(durableBodies, request.node, qc.context,
                   qc.view, qc.subject)
     THEN IF PersistDecisionValidationHeld(command)
          THEN "Apply"
          ELSE "ValidateBody"
     ELSE IF PersistDecisionBody(command) \in availableBodies
          THEN "StoreBody"
          ELSE "FetchBody"

PersistDecisionRecoverySuccessor(command) ==
  LET request == PersistDecisionRequest(command)
      qc == request.qc
  IN AsyncCandidateAtConsumerWithOrigin(
       "Completion", PersistDecisionRecoveryKind(command),
       request.node, qc.context.height, qc.view, qc.subject,
       NoAsyncItem, command.consumerView, command.consumerGeneration, qc,
       qc.subject, qc.subject, qc.subject, command.causalOrigin)

(***************************************************************************
Closed inventory of reducer parents which can emit a causal successor.

Keeping this set next to the CASE relation is intentional: source-fidelity
checks compare every CASE label with this inventory, so a newly modelled WAL,
signing, QC, timeout, Decision, or body-pipeline continuation cannot silently
bypass scheduler-wide exact-child coalescing.
***************************************************************************)
CausalSuccessorParentKinds ==
  {"AssembleBody", "BeginProposal", "PersistProposal",
   "DeliverProposal", "DeliverChunk", "FetchBody",
   "RebindRetainedBody", "FetchCertifiedBody", "StoreBody",
   "ValidateBody", "BeginPrepare", "PersistPrepare", "DeliverVote",
   "DeliverQC", "BeginObservePrepare", "PersistObservePrepare",
   "BeginLockCommit", "PersistLockCommit", "FormCommitQC",
   "BeginDecision", "PersistDecision", "BeginTimeout",
   "PersistTimeout", "SignTimeout", "DeliverTimeout", "DeliverTC",
   "BeginInstallTC", "PersistInstallTC"}

SignTimeoutRequests(command) ==
  {request \in signTimeouts:
    /\ command.node = request.node
    /\ command.height = context.height
    /\ command.view = request.vote.view
    /\ command.subject = request.vote.highSubject}

SignTimeoutFormsTC(command) ==
  \E request \in SignTimeoutRequests(command):
    /\ LocalTimeoutCompletionGuard(request)
    /\ TimeoutReceiptFormsTC(request.node, request.vote)

DeliverTimeoutFormsTC(command) ==
  /\ command.kind = "DeliverTimeout"
  /\ command.item.kind = "TimeoutVote"
  /\ command.node = command.item.envelope.recipient
  /\ TimeoutReceiptFormsTC(
       command.node, command.item.envelope.vote)

ExactFormedTcForTimeoutCommand(command) ==
  IF command.kind = "SignTimeout"
  THEN TimeoutCertificateAfterReceipt(
         command.node, LocalTimeoutVoteFor(command.node))
  ELSE TimeoutCertificateAfterReceipt(
         command.node, command.item.envelope.vote)

CommandSuccessors(command) ==
  CASE command.kind = "AssembleBody" ->
         IF ExactDecidedLocalBody(command.node, command.view,
                                  command.subject)
         THEN <<CausalCandidate("Completion", "Apply", command)>>
         ELSE <<CausalCandidate("Completion", "BeginProposal", command)>>
    [] command.kind = "BeginProposal" ->
         <<CausalCandidate("Completion", "PersistProposal", command)>>
    [] command.kind = "PersistProposal" ->
         <<CausalCandidate("Completion", "SignProposal", command)>>
    [] command.kind = "DeliverProposal" ->
         <<RetainedBodyRebindCandidate(command),
           CausalCandidate("Normal", "BeginPrepare", command)>>
    [] command.kind = "DeliverChunk" ->
         <<CausalCandidate("Completion", "FetchBody", command)>>
    [] command.kind = "FetchBody" ->
         IF CertifiedRecoveryFetchFrontier(command)
         THEN IF BodyHeldBy(durableBodies, command.node, context,
                            command.view, command.subject)
              THEN <<CausalCandidate("Completion", "ValidateBody", command)>>
              ELSE <<>>
         ELSE <<CausalCandidate("Completion", "StoreBody", command)>>
    [] command.kind = "RebindRetainedBody" ->
         <<CausalCandidate("Completion", "StoreBody", command)>>
    [] command.kind = "FetchCertifiedBody" ->
         <<CausalCandidate("Completion", "StoreBody", command)>>
    [] command.kind = "StoreBody" ->
         <<CausalCandidate("Completion", "ValidateBody", command)>>
    [] command.kind = "ValidateBody" ->
         <<CausalCandidate("Normal", "BeginPrepare", command),
           CausalCandidate("Completion", "BeginLockCommit", command),
           CausalCandidate("Completion", "Apply", command)>>
    [] command.kind = "BeginPrepare" ->
         <<CausalCandidate("Completion", "PersistPrepare", command)>>
    [] command.kind = "PersistPrepare" ->
         <<CausalCandidate("Completion", "SignVote", command)>>
    [] command.kind = "DeliverVote" ->
         <<CausalCandidate("Progress",
              IF command.item.envelope.vote.phase = "Prepare"
              THEN "FormPrepareQC" ELSE "FormCommitQC", command)>>
    [] command.kind = "DeliverQC" ->
         IF command.item.envelope.qc.phase = "Prepare"
         THEN IF QcDeliveryCreatesReceipt(
                   command.node, command.item.envelope.qc)
              THEN <<CausalCandidate(
                        "Progress", "BeginObservePrepare", command),
                     CausalCandidate(
                        "Completion", "BeginLockCommit", command)>>
              ELSE <<>>
         ELSE <<CausalCandidate("Progress", "BeginDecision", command)>>
    [] command.kind = "BeginObservePrepare" ->
         <<CausalCandidate("Completion", "PersistObservePrepare", command)>>
    [] command.kind = "PersistObservePrepare" ->
         <<CausalCandidate("Completion", "BeginLockCommit", command)>>
    [] command.kind = "BeginLockCommit" ->
         <<CausalCandidate("Completion", "PersistLockCommit", command)>>
    [] command.kind = "PersistLockCommit" ->
         <<CausalCandidate("Completion", "SignVote", command)>>
    [] command.kind = "FormCommitQC" ->
         <<CausalCandidate("Completion", "PersistDecision", command)>>
    [] command.kind = "BeginDecision" ->
         <<CausalCandidate("Completion", "PersistDecision", command)>>
    [] command.kind = "PersistDecision" ->
         IF PersistDecisionRequests(command) = {}
         THEN <<>>
         ELSE <<PersistDecisionRecoverySuccessor(command)>>
    [] command.kind = "BeginTimeout" ->
         <<CausalCandidate("Completion", "PersistTimeout", command)>>
    [] command.kind = "PersistTimeout" ->
         <<CausalCandidate("Completion", "SignTimeout", command)>>
    [] command.kind = "SignTimeout" ->
         IF SignTimeoutFormsTC(command)
         THEN <<CausalCandidateWithEvidence(
                   "Completion", "PersistInstallTC", command,
                   ExactFormedTcForTimeoutCommand(command))>>
         ELSE <<>>
    [] command.kind = "DeliverTimeout" ->
         IF DeliverTimeoutFormsTC(command)
         THEN <<CausalCandidateWithEvidence(
                   "Completion", "PersistInstallTC", command,
                   ExactFormedTcForTimeoutCommand(command))>>
         ELSE <<>>
    [] command.kind = "DeliverTC" ->
         IF NoDecisionForNode(command.node)
         THEN <<CausalCandidate("Progress", "BeginInstallTC", command)>>
         ELSE <<>>
    [] command.kind = "BeginInstallTC" ->
         <<CausalCandidate("Completion", "PersistInstallTC", command)>>
    [] command.kind = "PersistInstallTC" ->
         InstallCommandSuccessors(command)
    [] OTHER -> <<>>

(***************************************************************************
Closed constructor audit for immutable causal lineage.  Every causal child,
including the four constructors which deliberately replace evidence, view,
generation, or body stage, retains the parent's first-admission origin.
Keeping this theorem adjacent to the complete CASE inventory lets the source
checker reject any new successor branch which omits the internal metadata.
***************************************************************************)
THEOREM CommandSuccessorsRetainCausalOrigin ==
  \A command:
    \A successor \in SequenceSet(CommandSuccessors(command)):
      successor.causalOrigin = command.causalOrigin
BY SMTT(60)
   DEF CommandSuccessors, CausalCandidate,
       CausalCandidateWithEvidence, RetainedBodyRebindCandidate,
       PersistDecisionRecoverySuccessor, InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallLockedFetchSuccessor,
       InstallCommitSignSuccessors, InstallCommitSignSuccessor,
       InstallProposalSuccessor, AsyncCandidateFrom,
       AsyncCandidateAtConsumerWithOrigin,
       AsyncCandidateWithIdentityAndOrigin, SequenceSet

(***************************************************************************
Reducer effects preserve their declared order, but an exact successor which
already has any scheduler owner is coalesced.  The internal BodyAvailable
adapter preflight also coalesces FetchBody, RebindRetainedBody, and
FetchCertifiedBody after their exact body has left Missing.  This mirrors the
production reducer-state check before an internal callback receives an
admission ordinal.  In particular, a historical FetchBody whose response has
already staged the body cannot be recreated between that response and the
later Validate turn, excluding the old A -> B -> A occurrence lasso without
assuming favorable runner scheduling.
***************************************************************************)
AsyncCandidateInternalBodyAvailableStageRetired(candidate) ==
  /\ candidate.item = NoAsyncItem
  /\ candidate.kind
       \in {"FetchBody", "RebindRetainedBody", "FetchCertifiedBody"}
  /\ LET body ==
           BodyRecord(candidate.node, candidate.consumerContext,
                      candidate.view, candidate.subject)
     IN \/ body \in availableBodies
        \/ BodyHeldBy(durableBodies, candidate.node,
                       candidate.consumerContext,
                       candidate.view, candidate.subject)

FreshCandidateSequence(candidate) ==
  IF CandidateAdmissionCoalesced(candidate)
       \/ AsyncCandidateInternalBodyAvailableStageRetired(candidate)
  THEN <<>>
  ELSE <<candidate>>

FreshCommandSuccessors(command) ==
  LET successors == CommandSuccessors(command)
  IN CASE Len(successors) = 0 -> <<>>
       [] Len(successors) = 1 -> FreshCandidateSequence(successors[1])
       [] Len(successors) = 2 ->
            FreshCandidateSequence(successors[1])
              \o FreshCandidateSequence(successors[2])
       [] Len(successors) = 3 ->
            FreshCandidateSequence(successors[1])
              \o FreshCandidateSequence(successors[2])
              \o FreshCandidateSequence(successors[3])
       [] OTHER -> <<>>

AppendCausalSuccessors(command) ==
  asyncCausalQueues' =
    [asyncCausalQueues EXCEPT
       ![command.node] = @ \o FreshCommandSuccessors(command)]

LeaveCausalQueues == UNCHANGED asyncCausalQueues

(***************************************************************************
Authenticated sent history and bounded retransmission retention.

`asyncSentItems` is monotonic authenticity history.  It permits an old packet
that was already emitted to remain authentic after its control class is
replaced.  It is never scanned by retransmission.  `asyncRetainedControl`
models the reducer's seven-entry `outbound_control` map: at most one complete
broadcast batch is retained per source and control class, and only a strictly
higher-view message (or the exact duplicate) replaces that class.

Certified-body and commit-certificate discovery requests share an independent
bounded request lifecycle.  They remain in `asyncActiveRequests` until a
matching authenticated response completes the outer ingress handoff and are
retried beside, but never stored in, the control map.  A certified request is
Ready until one exact route-neutral response projection acquires the global
claim; the claim survives physical backpressure and retires with that request.
Chunks, responses, and adversarial junk are one-shot authenticated emissions.
***************************************************************************)

ProposalOutbox(request) ==
  {AsyncNetworkItem("Proposal", request.node,
                    ProposalEnvelope(recipient, request.proposal)):
     recipient \in CurrentVoters}

VoteOutbox(request) ==
  {AsyncNetworkItem(
       IF request.vote.phase = "Prepare" THEN "PrepareVote" ELSE "CommitVote",
       request.node, VoteEnvelope(recipient, request.vote)):
     recipient \in CurrentVoters \ {request.node}}

QcOutbox(node, qc) ==
  {AsyncNetworkItem(
       IF qc.phase = "Prepare" THEN "PrepareQC" ELSE "CommitQC",
       node, QcEnvelope(recipient, qc)):
     recipient \in CurrentVoters}

TimeoutOutbox(request) ==
  {AsyncNetworkItem("TimeoutVote", request.node,
                    TimeoutEnvelope(recipient, request.vote)):
     recipient \in CurrentVoters}

ByzantineProposalOutbox(signer, proposal) ==
  {AsyncNetworkItem("Proposal", signer,
                    ProposalEnvelope(recipient, proposal)):
     recipient \in CurrentVoters}

ByzantineVoteOutbox(signer, vote) ==
  {AsyncNetworkItem(
     IF vote.phase = "Prepare" THEN "PrepareVote" ELSE "CommitVote",
     signer, VoteEnvelope(recipient, vote)):
       recipient \in CurrentVoters}

ByzantineTimeoutOutbox(signer, vote) ==
  {AsyncNetworkItem("TimeoutVote", signer,
                    TimeoutEnvelope(recipient, vote)):
     recipient \in CurrentVoters}

TcOutbox(node, tc) ==
  {AsyncNetworkItem("TimeoutCertificate", node,
                    TcEnvelope(recipient, tc)):
     recipient \in CurrentVoters}

(***************************************************************************
`Responsive` is the static authenticated peer set guaranteed online after
GST.  It may strictly contain the positive-power roster, so the preferred
signed archive fanout always includes zero-power/non-roster peers without
granting them a vote.  The fanout identity is stable across availability
changes; only the service proof relies on Responsive peers being online after
GST.  The old QC signer set does not restrict which authenticated applied
archive may answer.  It is used only as a total-route fallback when the static
responsive union is empty in a degenerate model instance.
***************************************************************************)
CertifiedArchiveRoutes(node, qc) ==
  LET postGstRoutes ==
        (CurrentVoters
           \cup AsyncResponsiveArchiveServers) \ {node}
      frozenQcFallback ==
        (qc.signers \cap AsyncArchiveServerIds) \ {node}
  IN IF postGstRoutes # {}
     THEN postGstRoutes
     ELSE frozenQcFallback

CertifiedRequestOutbox(node, qc) ==
  {AsyncNetworkItem(
     "CertifiedRequest", node,
     AsyncCertifiedRequestEnvelope(server, node, qc, 0)):
       server \in CertifiedArchiveRoutes(node, qc)}

CommitCertificateRequestOutbox(node) ==
  {AsyncNetworkItem(
     "CommitCertificateRequest", node,
     AsyncBodyEnvelope(server, context.height, nodeView[node],
                       AsyncHeartbeatSubject, NoAsyncChunk, 0)):
       server \in CurrentVoters \ {node}}

CertifiedResponseItem(via, archiveServer, request) ==
  AsyncNetworkItem(
    "CertifiedResponse", via,
    AsyncCertifiedResponseEnvelope(
      request, archiveServer, AsyncCertifiedCitedResponder(request),
      archiveServer))

(***************************************************************************
Response authentication binds the canonical signed wire identity, not its
transport relay lane.  A production CertifiedBodyResponse has one canonical
resultless body, a manifest rederived from those exact bytes, and a
deterministic archive signature.  The collision-resistant subject and exact
signed request hash therefore abstract the omitted byte/signature fields, so
the record below is the model's full canonical-wire identity rather than a
coarse routing projection.  The occurrence remains in append-only sent
history after ingress retires every active request alias, so reducer execution
can recheck the same production authentication token without reintroducing
route authority.
***************************************************************************)
AsyncCertifiedResponseCanonicalWireIdentity(item) ==
  [kind |-> item.kind,
   envelope |-> item.envelope]

\* The model omits the response body bytes because `subject` is their
\* collision-resistant content identity.  The complete canonical signed-wire
\* record is therefore the exact response hash in this abstraction.
AsyncCertifiedResponseHash(item) ==
  AsyncCertifiedResponseCanonicalWireIdentity(item)

AsyncCertifiedResponseWaiterFamily(item) ==
  item.envelope.requestHash

AsyncCertifiedResponseOccurrenceIdentity(item) ==
  [requestHash |-> item.envelope.requestHash,
   authenticatedResponder |-> item.envelope.archiveServer,
   responseHash |-> AsyncCertifiedResponseHash(item)]

AsyncCertifiedResponseClaimRecord(item, ordinal) ==
  [recipient |-> item.envelope.recipient,
   family |-> AsyncCertifiedResponseWaiterFamily(item),
   identity |-> AsyncCertifiedResponseOccurrenceIdentity(item),
   ordinal |-> ordinal]

\* Compatibility name retained for proof-ledger and mutation artifacts.
AsyncCertifiedResponseAuthProjection(item) ==
  AsyncCertifiedResponseCanonicalWireIdentity(item)

CertifiedResponseAuthenticatedOccurrence(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ item.envelope.signatureOwner = item.envelope.archiveServer
  /\ \E sent \in asyncSentItems:
       /\ sent.kind = "CertifiedResponse"
       /\ AsyncCertifiedResponseCanonicalWireIdentity(sent)
            = AsyncCertifiedResponseCanonicalWireIdentity(item)
       /\ sent.envelope.signatureOwner = sent.envelope.archiveServer

(***************************************************************************
The active exact signed request is the per-request response authority.  Its
hash is `Ready` until one fully authenticated response acquires the recipient-
local route-neutral claim.  Each process owns an independent claim, matching
the Rust ingress instance.  The claim stores the canonical signed-wire
identity, so an exact response relayed through another transport hop coalesces
with the same owner while a distinct archive response at that recipient does
not.
***************************************************************************)
CertifiedRequestAuthority(item) ==
  /\ item.kind = "CertifiedRequest"
  /\ item.source = item.envelope.requester
  /\ LET qc == item.envelope.certificate
     IN /\ qc \in DecisionQcValues \cup prepareQCs
        /\ CertifiedBodyRecoveryAuthority(item.envelope.requester, qc)
        /\ item.envelope.height = qc.context.height
        /\ item.envelope.view = qc.view
        /\ item.envelope.subject = qc.subject

CertifiedRequestAuthorized(item) ==
  /\ CertifiedRequestAuthority(item)
  /\ item.envelope.recipient
       \in CertifiedArchiveRoutes(
            item.envelope.requester, item.envelope.certificate)

MatchingCertifiedRequests(response) ==
  {request \in asyncActiveRequests:
     /\ request.kind = "CertifiedRequest"
     /\ AsyncCertifiedRequestHash(request) =
          response.envelope.requestHash}

MatchingSentCertifiedRequests(response) ==
  {request \in asyncSentItems:
     /\ request.kind = "CertifiedRequest"
     /\ AsyncCertifiedRequestHash(request) =
          response.envelope.requestHash}

(***************************************************************************
Publication is the one mutable recovery-authority check.  Once published, the
exact signed request in append-only history is a frozen registration: later
response admission/consumption checks its immutable certificate and signed
bindings, not membership in a mutable lock/Decision pool.  View/Decision/Apply
lifecycle actions explicitly retain, rebind, or retire the active
registration and its linear claim.
***************************************************************************)
FrozenCertifiedRequestRegistration(request) ==
  /\ request.kind = "CertifiedRequest"
  /\ request \in asyncSentItems
  /\ request.source = request.envelope.requester
  /\ LET qc == request.envelope.certificate
     IN /\ request.envelope.height = qc.context.height
        /\ request.envelope.view = qc.view
        /\ request.envelope.subject = qc.subject

FrozenCertifiedResponseBinding(item, request) ==
  /\ item.kind = "CertifiedResponse"
  /\ FrozenCertifiedRequestRegistration(request)
  /\ CertifiedResponseAuthenticatedOccurrence(item)
  /\ item.envelope.archiveServer \in AsyncArchiveServerIds
  /\ AsyncCertifiedRequestHash(request) = item.envelope.requestHash
  /\ request.envelope.requester = item.envelope.recipient
  /\ request.envelope.height = item.envelope.height
  /\ request.envelope.view = item.envelope.view
  /\ request.envelope.subject = item.envelope.subject
  /\ item.envelope.citedResponder
       \in request.envelope.certificate.signers

CertifiedResponseCapabilityAuthorized(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ \E request \in MatchingSentCertifiedRequests(item):
       FrozenCertifiedResponseBinding(item, request)

IngressItemHasAuthenticatedHistory(item) ==
  IF item.kind = "CertifiedResponse"
  THEN CertifiedResponseAuthenticatedOccurrence(item)
  ELSE item \in asyncSentItems

CertifiedResponseAuthorized(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ CertifiedResponseAuthenticatedOccurrence(item)
  /\ item.envelope.archiveServer \in AsyncArchiveServerIds
  /\ MatchingCertifiedRequests(item) # {}
  /\ \E request \in MatchingCertifiedRequests(item):
       FrozenCertifiedResponseBinding(item, request)

AsyncCertifiedResponseItems ==
  {item \in AsyncNetworkItems: item.kind = "CertifiedResponse"}

AsyncCertifiedResponseClaimValues ==
  {AsyncCertifiedResponseCanonicalWireIdentity(item):
     item \in AsyncCertifiedResponseItems}

ActiveCertifiedRequestHashesIn(requests) ==
  {AsyncCertifiedRequestHash(request):
     request \in
       {candidate \in requests: candidate.kind = "CertifiedRequest"}}

ActiveCertifiedRequestHashes ==
  ActiveCertifiedRequestHashesIn(asyncActiveRequests)

(***************************************************************************
Each process owns an independent `FairV2Ingress` instance and registers only
the exact requests that it issued.  The generic aggregate-untrusted completion
fence must therefore inspect the destination process's authorities, not the
global union across validators.  Otherwise an unrelated request at one node
would reject ordinary completion traffic at every other node.
***************************************************************************)
ActiveCertifiedRequestHashesAt(recipient) ==
  {AsyncCertifiedRequestHash(request):
     request \in
       {candidate \in asyncActiveRequests:
          /\ candidate.kind = "CertifiedRequest"
          /\ candidate.envelope.requester = recipient}}

CertifiedResponseClaimRecordsForFamily(requestHash) ==
  {record \in AsyncCertifiedResponseClaimRecords:
     record.family = requestHash}

CertifiedResponseClaimRecordsAt(recipient) ==
  {record \in AsyncCertifiedResponseClaimRecords:
     record.recipient = recipient}

CertifiedResponseClaimRecordsForIdentity(identity) ==
  {record \in AsyncCertifiedResponseClaimRecords:
     record.identity = identity}

CertifiedResponseClaimRecordForItem(item) ==
  CHOOSE record \in
    CertifiedResponseClaimRecordsForIdentity(
      AsyncCertifiedResponseOccurrenceIdentity(item)): TRUE

CertifiedResponseClaimAdmissionOrdinal(item) ==
  CertifiedResponseClaimRecordForItem(item).ordinal

CertifiedResponseAuthorityClaimed(requestHash) ==
  \E projection \in asyncCertifiedResponseClaim:
    projection.envelope.requestHash = requestHash

CertifiedResponseAuthorityReady(requestHash) ==
  /\ requestHash \in ActiveCertifiedRequestHashes
  /\ ~CertifiedResponseAuthorityClaimed(requestHash)

CertifiedResponseClaimsAt(recipient) ==
  {projection \in asyncCertifiedResponseClaim:
     projection.envelope.recipient = recipient}

CertifiedResponseClaimsForFamilyAt(recipient, requestHash) ==
  {projection \in CertifiedResponseClaimsAt(recipient):
     projection.envelope.requestHash = requestHash}

CertifiedResponseClaimRecordsForFamilyAt(recipient, requestHash) ==
  {record \in CertifiedResponseClaimRecordsAt(recipient):
     record.family = requestHash}

\* Rust keys the response waiter by request hash.  Claim availability is
\* therefore family-local even though every admitted response shares the
\* recipient's normalized physical-completion lane.  The physical-owner gate
\* below still serializes actual reducer ownership; an unrelated logical
\* family must not steal or indefinitely postpone this family's claim.
CertifiedResponseRecipientClaimAvailable(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ CertifiedResponseClaimsForFamilyAt(
       item.envelope.recipient, item.envelope.requestHash) = {}
  /\ CertifiedResponseClaimRecordsForFamilyAt(
       item.envelope.recipient, item.envelope.requestHash) = {}

CertifiedResponseClaimMetadataMatches(item) ==
  \E record \in AsyncCertifiedResponseClaimRecords:
    /\ record.recipient = item.envelope.recipient
    /\ record.family = AsyncCertifiedResponseWaiterFamily(item)
    /\ record.identity = AsyncCertifiedResponseOccurrenceIdentity(item)

CertifiedResponseClaimMatches(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ AsyncCertifiedResponseCanonicalWireIdentity(item)
       \in asyncCertifiedResponseClaim

(***************************************************************************
Admission performs the full mutable authority check exactly once and stores
the authenticated signed-envelope projection as an opaque linear capability,
matching Rust's `AuthenticatedCertifiedBodyResponse`.  Ingress handoff
therefore rechecks the immutable signature/request binding and the still-live
exact request, but does not re-evaluate lock/Decision authority which may
advance while the token is queued.  The handoff atomically retires that request
and physical claim, while the delayed candidate retains the frozen signed
capability needed by `AcceptCertifiedResponseCapability`.  Other lifecycle
retirement clears or filters the claim atomically, so no physical claim can
outlive its exact active request authority.
***************************************************************************)
CertifiedResponseClaimAuthorized(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ CertifiedResponseClaimMatches(item)
  /\ CertifiedResponseAuthenticatedOccurrence(item)
  /\ item.envelope.archiveServer \in AsyncArchiveServerIds
  /\ MatchingCertifiedRequests(item) # {}
  /\ \E request \in MatchingCertifiedRequests(item):
       FrozenCertifiedResponseBinding(item, request)

CertifiedResponseClaimProjectionAuthenticated(projection) ==
  \E item \in AsyncCertifiedResponseItems:
    /\ AsyncCertifiedResponseCanonicalWireIdentity(item) = projection
    /\ CertifiedResponseAuthenticatedOccurrence(item)
    /\ item.envelope.archiveServer \in AsyncArchiveServerIds
    /\ MatchingCertifiedRequests(item) # {}
    /\ \E request \in MatchingCertifiedRequests(item):
         FrozenCertifiedResponseBinding(item, request)

CertifiedResponseClaimForRequests(requests) ==
  {projection \in asyncCertifiedResponseClaim:
     projection.envelope.requestHash
       \in ActiveCertifiedRequestHashesIn(requests)}

CertifiedResponseClaimForRequestsExceptRecipient(
    requests, recipient) ==
  {projection \in CertifiedResponseClaimForRequests(requests):
     projection.envelope.recipient # recipient}

CertifiedResponseClaimRecordsFor(
    records, requests, projections) ==
  {record \in records:
     /\ record.family \in ActiveCertifiedRequestHashesIn(requests)
     /\ record.identity.responseHash \in projections}

ActiveRequestsWithoutNode(node) ==
  {item \in asyncActiveRequests: item.source # node}

FilterCertifiedResponseAuthority(nextRequests) ==
  /\ asyncActiveRequests' = nextRequests
  /\ asyncCertifiedResponseClaim' =
       CertifiedResponseClaimForRequests(nextRequests)

RetireNodeCertifiedResponseAuthority(node) ==
  FilterCertifiedResponseAuthority(ActiveRequestsWithoutNode(node))

CertifiedRequestSurvivesBodyCompletion(item, command) ==
  \/ item.kind # "CertifiedRequest"
  \/ item.source # command.node
  \/ item.envelope.height # command.height
  \/ item.envelope.view # command.view
  \/ item.envelope.subject # command.subject

RetireCompletedBodyCertifiedResponseAuthority(command) ==
  FilterCertifiedResponseAuthority(
    {item \in asyncActiveRequests:
       CertifiedRequestSurvivesBodyCompletion(item, command)})

CommitCertificateResponseItem(request, qc) ==
  AsyncNetworkItem(
    "CommitCertificateResponse", AsyncUntrustedSource,
    AsyncCommitCertificateResponseEnvelope(request, qc))

(***************************************************************************
Production serves a certified request from the retained canonical body held
by its addressed authenticated archive server.  The server may be zero-power,
outside both the current voting roster and the frozen QC signer set.  The
validation cache is consumer-local pipeline state, not serving authority;
requiring it here would suppress a response that `serve_certified_body` emits
from the durable body store.  Addressing constrains this honest constructor,
not response acceptance: another archive's independently authenticated
content-addressed response remains safe.
***************************************************************************)
CertifiedServeCanRespond(server, request) ==
  /\ request.kind = "CertifiedRequest"
  /\ request.envelope.recipient = server
  /\ BodyHeldBy(durableBodies, server, request.envelope.certificate.context,
                request.envelope.view, request.envelope.subject)

CommitCertificateServeCanRespond(request) ==
  /\ request.kind = "CommitCertificateRequest"
  /\ \E application \in applied:
       /\ application.node = request.envelope.recipient
       /\ application.qc.context = context
       /\ application.qc.phase = "Commit"

CommitCertificateServiceApplication(request) ==
  CHOOSE application \in applied:
    /\ application.node = request.envelope.recipient
    /\ application.qc.context = context
    /\ application.qc.phase = "Commit"

CommitCertificateResponseItems(request) ==
  IF CommitCertificateServeCanRespond(request)
  THEN {CommitCertificateResponseItem(
          request, CommitCertificateServiceApplication(request).qc)}
  ELSE {}

ChunkOutbox(node, source, roundView, subject) ==
  {AsyncNetworkItem(
     "Chunk", source,
     AsyncBodyEnvelope(node, context.height, roundView, subject, chunk, 0)):
       chunk \in AsyncChunks}

BroadcastChunkOutbox(source, roundView, subject) ==
  UNION {ChunkOutbox(recipient, source, roundView, subject):
           recipient \in CurrentVoters}

(***************************************************************************
Production records an authenticated delivery while any reducer/adapter owner
retains it for the current consumer epoch.  Model retransmission suppression
over the same complete scheduler ownership set, but retire that authority as
soon as context, view, or generation changes.  A deferred or causal occurrence
therefore cannot acquire an exact replacement while current, and a stale
pre-TC occurrence cannot suppress the locked Commit reconstruction which
belongs to the new pool.
***************************************************************************)
ItemInScheduledDelivery(item) ==
  \E candidate \in QueuedCandidates \cup DeferredCandidates
                      \cup CausalCandidates \cup TrackedWorkCandidates:
    /\ candidate.item = item
    /\ CandidateConsumerCurrent(candidate)

IngressResourceSource(item) ==
  IF item.kind = "CertifiedResponse"
  THEN AsyncUntrustedSource
  ELSE item.source

IngressLane(recipient, source) == asyncIngressLanes[recipient][source]

IngressLaneDepth(recipient, source) == Len(IngressLane(recipient, source))

IngressDepth(recipient) ==
  Cardinality(
    {pair \in AsyncIngressSources \X (1..AsyncIngressCapacity):
       pair[2] <= IngressLaneDepth(recipient, pair[1])})

(*
The transport ingress class is deliberately broader than reducer delivery
priority.  It is computed before payload authentication, so a Byzantine
validator may occupy only its own source-scoped non-timeout Progress,
TimeoutVote, and shared physical completion reservations; the authenticated
reducer still decides whether a Commit vote is the exact locked-round
reconstruction witness.  Body and certificate recovery requests remain
Progress.  `CertifiedResponse` has a distinct logical class from generic
`Chunk`, while both consume the same one physical completion owner.
*)
IngressTransportCompletionKinds == {"Chunk", "CertifiedResponse"}

IngressProgressKinds ==
  {"CommitVote", "PrepareQC", "CommitQC", "TimeoutVote",
   "TimeoutCertificate", "CertifiedRequest", "CommitCertificateRequest",
   "CommitCertificateResponse"}

IngressAdmissionClass(item) ==
  IF item.kind = "CertifiedResponse"
  THEN "CertifiedResponse"
  ELSE IF item.kind \in IngressTransportCompletionKinds
       THEN "TransportCompletion"
  ELSE IF item.kind \in IngressProgressKinds THEN "Progress" ELSE "Auxiliary"

IngressUsesPhysicalCompletionOwner(item) ==
  IngressAdmissionClass(item)
    \in {"CertifiedResponse", "TransportCompletion"}

IngressCoalescingIdentity(item) ==
  IF item.kind \in AsyncReplyRequestKinds
  THEN AsyncServeLogicalRequestIdentity(
         item.envelope.recipient, item)
  ELSE IF item.kind = "CertifiedResponse"
       THEN AsyncCertifiedResponseAuthProjection(item)
       ELSE item

IngressHasCoalescingOwner(item) ==
  \E queued \in SequenceSet(
       IngressLane(
         item.envelope.recipient, IngressResourceSource(item))):
    IngressCoalescingIdentity(queued) = IngressCoalescingIdentity(item)

IngressLaneHasNonTimeoutProgressIn(lanes, recipient, source) ==
  \E queued \in SequenceSet(lanes[recipient][source]):
    /\ IngressAdmissionClass(queued) = "Progress"
    /\ queued.kind # "TimeoutVote"

IngressLaneHasTimeoutVoteIn(lanes, recipient, source) ==
  \E queued \in SequenceSet(lanes[recipient][source]):
    queued.kind = "TimeoutVote"

IngressLaneHasTransportCompletionIn(lanes, recipient, source) ==
  \E queued \in SequenceSet(lanes[recipient][source]):
    IngressUsesPhysicalCompletionOwner(queued)

AsyncTimeoutVoteByteGateAllows(item) ==
  \/ item.kind # "TimeoutVote"
  \/ IngressResourceSource(item) \notin ValidatorIds
  \/ /\ AsyncValidTimeoutVoteWireByteBound <= AsyncTimeoutVoteByteReserve
     /\ ~IngressLaneHasTimeoutVoteIn(asyncIngressLanes,
                                      item.envelope.recipient,
                                      IngressResourceSource(item))

AsyncTransportCompletionOwnerGateAllows(item) ==
  \/ ~IngressUsesPhysicalCompletionOwner(item)
  \/ ~IngressLaneHasTransportCompletionIn(
       asyncIngressLanes, item.envelope.recipient,
       IngressResourceSource(item))

CertifiedResponseFreshClaimGateAllows(item) ==
  \/ item.kind # "CertifiedResponse"
  \/ /\ CertifiedResponseAuthorized(item)
     /\ CertifiedResponseAuthorityReady(item.envelope.requestHash)
     /\ CertifiedResponseRecipientClaimAvailable(item)

AsyncUntrustedGenericCompletionGateAllows(item) ==
  \/ IngressAdmissionClass(item) # "TransportCompletion"
  \/ IngressResourceSource(item) # AsyncUntrustedSource
  \/ ActiveCertifiedRequestHashesAt(item.envelope.recipient) = {}

(*
An empty source needs a first-message slot.  A validator separately reserves a
missing non-timeout Progress item, a missing TimeoutVote, and one missing
TransportCompletion item shared by Chunk and CertifiedResponse.  The
continuation term covers the depth-one through depth-three combinations whose
removal would recreate one of those reservations.  The aggregate untrusted
source reserves `max(2 - depth, missing_transport_completion)`: at depth zero
the empty-source and missing-completion terms are its two owners, while at
depth one a present completion receives the separate generic continuation.
Together these potentials match the production `4N+2` count gate for N >= 1.
*)
IngressProtectedSourcesFor(lanes, recipient) ==
  {source \in AsyncIngressSources:
     \/ Len(lanes[recipient][source]) = 0
     \/ /\ source \in ValidatorIds
           /\ ~IngressLaneHasNonTimeoutProgressIn(
                 lanes, recipient, source)}

IngressTimeoutVoteProtectedSourcesFor(lanes, recipient) ==
  {source \in ValidatorIds:
     ~IngressLaneHasTimeoutVoteIn(lanes, recipient, source)}

IngressTransportCompletionProtectedSourcesFor(lanes, recipient) ==
  {source \in AsyncIngressSources:
     ~IngressLaneHasTransportCompletionIn(lanes, recipient, source)}

IngressContinuationProtectedSourcesFor(lanes, recipient) ==
  {source \in AsyncIngressSources:
     \/ /\ source \in ValidatorIds
           /\ \/ Len(lanes[recipient][source]) = 0
              \/ /\ Len(lanes[recipient][source]) = 1
                    /\ (IngressLaneHasNonTimeoutProgressIn(
                           lanes, recipient, source)
                         \/ IngressLaneHasTimeoutVoteIn(
                              lanes, recipient, source)
                         \/ IngressLaneHasTransportCompletionIn(
                              lanes, recipient, source))
              \/ /\ Len(lanes[recipient][source]) = 2
                    /\ \/ /\ IngressLaneHasNonTimeoutProgressIn(
                                  lanes, recipient, source)
                               /\ IngressLaneHasTimeoutVoteIn(
                                    lanes, recipient, source)
                       \/ /\ IngressLaneHasNonTimeoutProgressIn(
                                  lanes, recipient, source)
                               /\ IngressLaneHasTransportCompletionIn(
                                    lanes, recipient, source)
                       \/ /\ IngressLaneHasTimeoutVoteIn(
                                  lanes, recipient, source)
                               /\ IngressLaneHasTransportCompletionIn(
                                    lanes, recipient, source)
              \/ /\ Len(lanes[recipient][source]) = 3
                    /\ IngressLaneHasNonTimeoutProgressIn(
                         lanes, recipient, source)
                    /\ IngressLaneHasTimeoutVoteIn(
                         lanes, recipient, source)
                    /\ IngressLaneHasTransportCompletionIn(
                         lanes, recipient, source)
     \/ /\ source \notin ValidatorIds
           /\ Len(lanes[recipient][source]) = 1
           /\ IngressLaneHasTransportCompletionIn(
                lanes, recipient, source)}

IngressProtectedSlotCountFor(lanes, recipient) ==
  Cardinality(IngressProtectedSourcesFor(lanes, recipient))
    + Cardinality(
        IngressTimeoutVoteProtectedSourcesFor(lanes, recipient))
    + Cardinality(
        IngressTransportCompletionProtectedSourcesFor(lanes, recipient))
    + Cardinality(IngressContinuationProtectedSourcesFor(lanes, recipient))

IngressLanesAfterAdmission(item) ==
  [asyncIngressLanes EXCEPT
     ![item.envelope.recipient][IngressResourceSource(item)] =
       Append(@, item)]

IngressProtectedSourcesAfterAdmission(item) ==
  IngressProtectedSourcesFor(
    IngressLanesAfterAdmission(item), item.envelope.recipient)

IngressProtectedSlotCountAfterAdmission(item) ==
  IngressProtectedSlotCountFor(IngressLanesAfterAdmission(item),
                               item.envelope.recipient)

IngressUsableCapacityAfterAdmission(item) ==
  AsyncIngressCapacity
    - IngressProtectedSlotCountAfterAdmission(item)

ItemInIngress(item) ==
  \E recipient \in ValidatorIds, source \in AsyncIngressSources:
    item \in SequenceSet(IngressLane(recipient, source))

ItemHasPacket(item) ==
  \E packet \in asyncTransport: packet.item = item

ItemInIoServe(item) ==
  \E node \in ValidatorIds:
    \E job \in SequenceSet(asyncIoQueues[node]):
      /\ job.class = "Serve"
      /\ job.candidate # NoAsyncCandidate
      /\ job.candidate.item = item
      /\ CandidateConsumerCurrent(job.candidate)

ItemInLocalCompletion(item) ==
  \E node \in ValidatorIds:
    \E candidate \in SequenceSet(asyncLocalReadyCompletions[node]):
      /\ candidate.item = item
      /\ CandidateConsumerCurrent(candidate)

ItemScheduled(item) ==
  ItemInScheduledDelivery(item) \/ ItemInIngress(item) \/ ItemHasPacket(item)
    \/ ItemInIoServe(item) \/ ItemInLocalCompletion(item)

SendableItems(source) ==
  {item \in asyncRetainedControl: item.source = source}

ActiveRequestItems(source) ==
  {item \in asyncActiveRequests: item.source = source}

ControlClass(item) == item.kind

ControlRecipients(source, controlClass, voters) ==
  IF controlClass \in {"PrepareVote", "CommitVote"}
  THEN voters \ {source}
  ELSE voters

ControlView(item) ==
  CASE item.kind = "Proposal" -> item.envelope.proposal.view
    [] item.kind \in {"PrepareVote", "CommitVote", "TimeoutVote"} ->
         item.envelope.vote.view
    [] item.kind \in {"PrepareQC", "CommitQC"} -> item.envelope.qc.view
    [] item.kind = "TimeoutCertificate" -> item.envelope.tc.view

RetainedClassItems(retained, source, controlClass) ==
  {item \in retained:
     /\ item.source = source
     /\ ControlClass(item) = controlClass}

RememberedControl(retained, items) ==
  IF items = {}
  THEN retained
  ELSE LET fresh == CHOOSE item \in items: TRUE
           existing ==
             RetainedClassItems(retained, fresh.source, ControlClass(fresh))
       IN IF existing = {}
                \/ ControlView(fresh)
                     > ControlView(CHOOSE item \in existing: TRUE)
                \/ items = existing
          THEN (retained \ existing) \cup items
          ELSE retained

InstalledControl(retained, node, items) ==
  {item \in RememberedControl(retained, items):
     item.source # node
       \/ ControlClass(item) \in AsyncInstallRetainedControlKinds}

CurrentTimeoutControlFor(items, node) ==
  LET currentClass == RetainedClassItems(items, node, "TimeoutVote")
      exactCurrentClass ==
        \A item \in currentClass:
          /\ item.envelope.vote.context = context
          /\ item.envelope.vote.height = height
          /\ item.envelope.vote.view = nodeView[node]
          /\ item.envelope.vote.signer = node
  IN IF exactCurrentClass THEN currentClass ELSE {}

ReseedExactHighestPrepareControl(retained, node, tc) ==
  LET withoutOwnPrepare ==
        retained \ RetainedClassItems(retained, node, "PrepareQC")
      highestPrepare == ResultingInstallHighestPrepareQc(node, tc)
  IN IF highestPrepare = NoPrepareQC
     THEN withoutOwnPrepare
     ELSE withoutOwnPrepare \cup QcOutbox(node, highestPrepare)

(***************************************************************************
InstallTimeout clears volatile local control, except that a strict same-round
upgrade keeps the exact current TimeoutVote.  It atomically replaces the
source's retained TimeoutCertificate class with the exact newly installed TC
batch: a same-view TC may carry different authenticated evidence, so the
generic view-only `RememberedControl` replacement rule is insufficient here.
It then restores the complete durable highest PrepareQC object, replacing any
equal-view/different-evidence occurrence rather than reconstructing it from
rank and subject.
***************************************************************************)
InstalledControlAfterTC(retained, node, tc, items) ==
  LET withoutOwnTc ==
        retained \ RetainedClassItems(retained, node, "TimeoutCertificate")
      remembered == RememberedControl(withoutOwnTc, items)
      installed ==
        {item \in remembered:
          item.source # node
            \/ ControlClass(item) \in AsyncInstallRetainedControlKinds}
      withCurrentTimeout ==
        installed
          \cup (IF StrictSameRoundTcUpgrade(node, tc)
                THEN CurrentTimeoutControlFor(remembered, node)
                ELSE {})
  IN ReseedExactHighestPrepareControl(withCurrentTimeout, node, tc)

(***************************************************************************
Transport publication preserves the liveness consequence of the production
per-source FIFO while an overdue head stops `asyncNow`.  The configured
delivery bound is strictly positive, so every newly published packet has a
deadline strictly after the current clock.  It therefore cannot join or
recreate the current finite due prefix while that prefix keeps the clock
fixed.  Packets published at the same clock value may be reordered
nondeterministically inside their finite batch; this is conservative for the
production FIFO, and the next clock advance makes them due normally.
***************************************************************************)
PacketForItem(item) ==
  AsyncPacket(item, asyncNow, asyncNow + AsyncDeliveryBound)

PacketsForItems(items) ==
  {PacketForItem(item): item \in items}

PublishControlItems(items) ==
  /\ items \subseteq {item \in AsyncNetworkItems:
                        item.kind \in AsyncControlKinds}
  /\ asyncRetainedControl' =
       RememberedControl(asyncRetainedControl, items)
  /\ asyncSentItems' = asyncSentItems \cup items
  /\ asyncTransport' = asyncTransport \cup PacketsForItems(items)
  /\ UNCHANGED <<asyncActiveRequests, asyncCertifiedResponseClaim>>

PublishEphemeralItems(items) ==
  /\ asyncSentItems' = asyncSentItems \cup items
  /\ asyncTransport' = asyncTransport \cup PacketsForItems(items)
  /\ UNCHANGED <<asyncRetainedControl, asyncActiveRequests,
                  asyncCertifiedResponseClaim>>

PublishControlAndEphemeralItems(controlItems, ephemeralItems) ==
  /\ controlItems \subseteq {item \in AsyncNetworkItems:
                               item.kind \in AsyncControlKinds}
  /\ asyncRetainedControl' =
       RememberedControl(asyncRetainedControl, controlItems)
  /\ asyncSentItems' =
       asyncSentItems \cup controlItems \cup ephemeralItems
  /\ asyncTransport' =
       asyncTransport
         \cup PacketsForItems(controlItems \cup ephemeralItems)
  /\ UNCHANGED <<asyncActiveRequests, asyncCertifiedResponseClaim>>

PublishCertifiedRequests(items) ==
  /\ \A item \in items: item.kind = "CertifiedRequest"
  \* Fail closed on a pre-existing conflict, an incoming conflict, or a
  \* conflict between an active exact registration and the new fanout.
  /\ AsyncCertifiedRequestLogicalIndexConsistent(asyncActiveRequests)
  /\ AsyncCertifiedRequestLogicalIndexConsistent(items)
  /\ AsyncCertifiedRequestSetsCompatible(asyncActiveRequests, items)
  /\ AsyncServePublicationRespectsHighWatermarks(items)
  /\ asyncActiveRequests' = asyncActiveRequests \cup items
  /\ asyncSentItems' = asyncSentItems \cup items
  /\ asyncTransport' = asyncTransport \cup PacketsForItems(items)
  /\ UNCHANGED asyncCertifiedResponseClaim
  /\ UNCHANGED asyncRetainedControl

PublishCommitCertificateRequests(items) ==
  /\ \A item \in items: item.kind = "CommitCertificateRequest"
  /\ AsyncServePublicationRespectsHighWatermarks(items)
  /\ asyncActiveRequests' = asyncActiveRequests \cup items
  /\ asyncSentItems' = asyncSentItems \cup items
  /\ asyncTransport' = asyncTransport \cup PacketsForItems(items)
  /\ UNCHANGED asyncCertifiedResponseClaim
  /\ UNCHANGED asyncRetainedControl

CertifiedRequestSurvivesInstall(item, node, tc) ==
  IF item.kind # "CertifiedRequest" \/ item.source # node
  THEN TRUE
  ELSE LET certificateRef ==
             [context |-> item.envelope.certificate.context,
              phase |-> item.envelope.certificate.phase,
              view |-> item.envelope.certificate.view,
              subject |-> item.envelope.certificate.subject]
           resultingRef ==
             [context |-> context,
              phase |-> "Prepare",
              view |-> ResultingInstallLockRank(node, tc),
              subject |-> ResultingInstallLockSubject(node, tc)]
       IN /\ ResultingInstallLockRank(node, tc) # NoRank
          /\ item.envelope.height = context.height
          /\ certificateRef = resultingRef

PersistInstalledControl(node, items, broadcast) ==
  /\ asyncRetainedControl' =
       InstalledControl(asyncRetainedControl, node, items)
  /\ asyncSentItems' =
       IF broadcast THEN asyncSentItems \cup items ELSE asyncSentItems
  /\ asyncTransport' =
       IF broadcast
       THEN asyncTransport \cup PacketsForItems(items)
       ELSE asyncTransport
  /\ UNCHANGED <<asyncActiveRequests, asyncCertifiedResponseClaim>>

(***************************************************************************
Installing a TC replaces production's volatile body-work owner.  Retire only
this node's certified requests for superseded lock identities; immutable sent
history and already in-flight packet occurrences remain authentication facts
and are harmless because response admission rechecks the current authority.

Keep PersistInstalledControl as the generic retained-control helper used by
existing proof decompositions; this install-specific wrapper owns the request
lifecycle change without altering that helper's arity or meaning.
***************************************************************************)
PersistInstalledControlAfterInstall(node, tc, items, broadcast) ==
  /\ asyncRetainedControl' =
       InstalledControlAfterTC(asyncRetainedControl, node, tc, items)
  /\ asyncSentItems' =
       IF broadcast THEN asyncSentItems \cup items ELSE asyncSentItems
  /\ asyncTransport' =
       IF broadcast
       THEN asyncTransport \cup PacketsForItems(items)
       ELSE asyncTransport
  /\ asyncActiveRequests' =
       {item \in asyncActiveRequests:
          CertifiedRequestSurvivesInstall(item, node, tc)}
  /\ asyncCertifiedResponseClaim' =
       CertifiedResponseClaimForRequests(asyncActiveRequests')

CertifiedRequestSurvivesDecision(item, node, qc) ==
  IF item.source # node
  THEN TRUE
  ELSE /\ item.kind = "CertifiedRequest"
       /\ item.envelope.height = qc.context.height
       /\ item.envelope.view = qc.view
       /\ item.envelope.subject = qc.subject

PersistDecisionControl(node, qc, items, broadcast) ==
  /\ asyncRetainedControl' =
       RememberedControl(asyncRetainedControl, items)
  /\ asyncSentItems' =
       IF broadcast THEN asyncSentItems \cup items ELSE asyncSentItems
  /\ asyncTransport' =
       IF broadcast
       THEN asyncTransport \cup PacketsForItems(items)
       ELSE asyncTransport
  /\ FilterCertifiedResponseAuthority(
       {item \in asyncActiveRequests:
          CertifiedRequestSurvivesDecision(item, node, qc)})

TimeoutTagPresent(node) ==
  "TimeoutElapsed" \notin asyncOutstandingTags[node]

ActiveCommitCertificateRequests(node) ==
  {item \in asyncActiveRequests:
     item.source = node /\ item.kind = "CommitCertificateRequest"}

(***************************************************************************
Commit-certificate discovery is recurring auxiliary work which production
runs before the outer-loop executor turn.  It is therefore modelled as a
non-runner action: taking discovery does not satisfy `RunNode` fairness, and
the fair serialized runtime turn remains continuously enabled.  Folding this
prefix into `RuntimeStep` would let repeated discovery satisfy weak fairness
while an already queued reducer command never executes.
***************************************************************************)
CommitCertificateDiscoveryReady(node) ==
  /\ node \in AsyncActiveServiceNodes
  /\ ~ResponsiveReplayQuarantined(node)
  /\ asyncNow >= AsyncRoundTimeout
  /\ ~NodeHasDecision(node)
  /\ ActiveCommitCertificateRequests(node) = {}
  /\ CommitCertificateRequestOutbox(node) # {}

CommitCertificateDiscoveryDue(node) ==
  /\ node \in AsyncCurrentResponsiveVoters
  /\ CommitCertificateDiscoveryReady(node)

HistoricalCommitCertificateDiscoveryDue(node) ==
  /\ HistoricalRecoveryTarget(node)
  /\ CommitCertificateDiscoveryReady(node)

(***************************************************************************
An old-height recovery target is explicit scheduler ownership, not a chain-
wrapper exception to the exact Async transition relation.  The target may be
outside the frozen voting roster, but it must be a responsive live validator
and a current responsive server must already hold the exact applied Commit
receipt.  Opening is post-GST and only precedes local Decision installation;
the exact Apply command below retires the target atomically.
***************************************************************************)
HistoricalRecoverySourceReady(node) ==
  /\ node \in AsyncActiveServiceNodes
  /\ node \in Responsive \cap up
  /\ ~NodeHasDecision(node)
  /\ ~NodeHasApplication(node)
  /\ (AsyncResponsiveAppliedArchiveServers \ {node}) # {}

OpenHistoricalRecovery(node) ==
  /\ gst
  /\ HistoricalRecoverySourceReady(node)
  /\ ~HistoricalRecoveryTarget(node)
  /\ asyncHistoricalRecoveryTargets' =
       asyncHistoricalRecoveryTargets \cup {node}
  /\ UNCHANGED <<vars, AsyncSchedulerExceptHistoricalRecoveryTargets,
                 AsyncRecoveryVars>>

AsyncNodeHasDecisionIn(node, currentContext, currentDecisions) ==
  \E decision \in currentDecisions:
    /\ decision.node = node
    /\ decision.qc.context = currentContext
    /\ decision.qc.phase = "Commit"

AsyncNodeTimedOutIn(
    node, roundView, currentContext, currentTimeoutIntents) ==
  \E vote \in currentTimeoutIntents:
    /\ vote.signer = node
    /\ vote.context = currentContext
    /\ vote.view = roundView

ResponsiveReplayQuarantinedIn(node, recoveryNode, recoveryPhase) ==
  /\ node = recoveryNode
  /\ recoveryPhase \in {"ReplayRequired", "Replaying"}

TimeoutTagPresentIn(node, outstandingTags) ==
  "TimeoutElapsed" \notin outstandingTags[node]

AsyncTimeoutClockDueIn(
    node, currentContext, currentNodeView, currentNow,
    currentNodeDeadlines, currentDecisions, currentTimeoutIntents,
    currentTimeoutEmitted, currentOutstandingTags,
    recoveryNode, recoveryPhase) ==
  /\ node \in Responsive \cap VotingRoster(currentContext.epoch)
  /\ ~ResponsiveReplayQuarantinedIn(
       node, recoveryNode, recoveryPhase)
  /\ currentNow >= currentNodeDeadlines[node]
  /\ ~AsyncNodeHasDecisionIn(node, currentContext, currentDecisions)
  /\ ~AsyncNodeTimedOutIn(
       node, currentNodeView[node], currentContext,
       currentTimeoutIntents)
  /\ ~currentTimeoutEmitted[node]
  /\ TimeoutTagPresentIn(node, currentOutstandingTags)

AsyncTimeoutClockDue(node) ==
  AsyncTimeoutClockDueIn(
    node, context, nodeView, asyncNow, asyncNodeDeadlines,
    decisions, timeoutIntents, asyncTimeoutEmitted,
    asyncOutstandingTags, asyncRecoveryNode, asyncRecoveryPhase)

AsyncTimeoutClockDueAfter(node) ==
  AsyncTimeoutClockDueIn(
    node, context', nodeView', asyncNow', asyncNodeDeadlines',
    decisions', timeoutIntents', asyncTimeoutEmitted',
    asyncOutstandingTags', asyncRecoveryNode', asyncRecoveryPhase')

AsyncTimeoutLifecycleOwned(node) ==
  /\ AsyncTimeoutLifecycleOrdinal(node) # 0
  /\ AsyncTimeoutLifecycleOrigin(node)
       # NoAsyncCandidateLifecycleOrigin

AsyncEffectiveTimeoutLifecycleOrigin(node) ==
  IF AsyncTimeoutLifecycleOwned(node)
  THEN AsyncTimeoutLifecycleOrigin(node)
  ELSE AsyncProposedTimeoutCausalOrigin(node)

AsyncEffectiveTimeoutLifecycleOrdinal(node) ==
  IF AsyncTimeoutLifecycleOwned(node)
  THEN AsyncTimeoutLifecycleOrdinal(node)
  ELSE AsyncNextCandidateLifecycleOrdinal(node)

AsyncOlderCandidateLifecycleBlocksTimeout(node) ==
  \E candidate \in
       QueuedCandidates \cup DeferredCandidates
         \cup CausalCandidates \cup TrackedWorkCandidates:
    /\ candidate.node = node
    /\ AsyncCandidateLifecycleOrdinal(candidate)
         < AsyncEffectiveTimeoutLifecycleOrdinal(node)

AsyncOlderRunnableCandidateLifecyclePrecedesServeIngress(node) ==
  \E candidate \in QueuedCandidates \cup DeferredCandidates:
    /\ candidate.node = node
    /\ AsyncCandidateLifecycleOrdinal(candidate)
         < AsyncServeEarliestIngressSchedulerOrdinal(node)

AsyncOlderFrozenTimeoutLifecyclePrecedesServeIngress(node) ==
  /\ AsyncTimeoutLifecycleOwned(node)
  /\ AsyncTimeoutLifecycleOrdinal(node)
       < AsyncServeEarliestIngressSchedulerOrdinal(node)

(***************************************************************************
Only an already physical Runtime candidate or the frozen timeout lifecycle
can take the Runtime predecessor macro-step.  Local/causal owners are excluded
from this predicate because the Local phase compares its selected immutable
owner separately in `AsyncOlderLocalLifecyclePrecedesServeIngress`; neither
predicate admits work whose ordinal was allocated after the Serve ticket.
***************************************************************************)
AsyncOlderRuntimeLifecyclePrecedesServeIngress(node) ==
  /\ AsyncServeIngressLifecycleOwnerIdentities(node) # {}
  /\ \/ AsyncOlderRunnableCandidateLifecyclePrecedesServeIngress(node)
     \/ AsyncOlderFrozenTimeoutLifecyclePrecedesServeIngress(node)

\* Neutral fixed-corridor clock freshness shared by adequate-leader and
\* retained-lock proofs.  It lives below either temporal composition module
\* to avoid a dependency cycle.  An owned clock is relevant only when its
\* frozen immutable origin belongs to this context and is no later than the
\* requested corridor view.
AsyncTimeoutLifecycleKinds ==
  {"BeginTimeout", "PersistTimeout", "SignTimeout",
   "DeliverTimeout", "FormTC", "DeliverTC",
   "BeginInstallTC", "PersistInstallTC"}

AsyncOlderOrEqualTimeoutLifecycleOwned(
    node, currentContext, roundView) ==
  \/ IF AsyncTimeoutLifecycleOwned(node)
     THEN /\ AsyncTimeoutLifecycleOrigin(node).context = currentContext
          /\ AsyncTimeoutLifecycleOrigin(node).view \in 0..roundView
     ELSE FALSE
  \/ \E candidate \in
       QueuedCandidates \cup DeferredCandidates
         \cup CausalCandidates \cup TrackedWorkCandidates:
       /\ candidate.node = node
       /\ candidate.consumerContext = currentContext
       /\ candidate.causalOrigin.context = currentContext
       /\ candidate.causalOrigin.view \in 0..roundView
       /\ candidate.causalOrigin.phase \in AsyncTimeoutLifecycleKinds

AsyncFreshNodeServiceWindow(node, currentContext, roundView) ==
  /\ node \in ValidatorIds
  /\ currentContext = context
  /\ roundView \in Views
  /\ nodeView[node] = roundView
  /\ asyncNow + AsyncWorstCaseServiceBudget
       < asyncNodeDeadlines[node]
  /\ ~NodeTimedOut(node, roundView)
  /\ ~asyncTimeoutEmitted[node]
  /\ "TimeoutElapsed" \notin asyncOutstandingTags[node]
  /\ ~AsyncOlderOrEqualTimeoutLifecycleOwned(
       node, currentContext, roundView)

TimeoutDue(node) ==
  /\ AsyncTimeoutClockDue(node)
  /\ ~AsyncOlderCandidateLifecycleBlocksTimeout(node)

RetransmitTagPresent(node) ==
  "RetransmitElapsed" \notin asyncOutstandingTags[node]

RetransmitDue(node) ==
  /\ ~ResponsiveReplayQuarantined(node)
  /\ asyncNow >= asyncRetransmitDeadlines[node]
  /\ RetransmitTagPresent(node)
  /\ ~AsyncTimeoutClockDue(node)

AsyncRetransmitProgramCounterStates == {"AwaitDue", "DriveDue"}

AsyncRetransmitProgramCounter(node) ==
  IF "RetransmitElapsed" \in asyncOutstandingTags[node]
  THEN "DriveDue"
  ELSE "AwaitDue"

(***************************************************************************
Exact Core reducer command execution.
***************************************************************************)

CommandMatches(command, node, roundView, subject) ==
  /\ command.node = node
  /\ command.height = context.height
  /\ command.view = roundView
  /\ command.subject = subject

AssembleLocalBodyReady(node, subject) ==
  LET roundView == nodeView[node]
      body == BodyRecord(node, context, roundView, subject)
      validation == ValidationRecord(node, context, roundView,
                                      generation[node], subject)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ node = Leader(context, nodeView[node])
     /\ subject \in ValidSubjects
     /\ LocalBodyNotSupersededByDecision(node, roundView, subject)
     /\ body \in BodyRecordSet
     /\ validation \in ValidationRecordSet
     /\ ~BodyHeldBy(durableBodies, node, context, roundView, subject)

BeginLocalProposalReady(node, subject) ==
  LET roundView == nodeView[node]
      proposal == LocalProposalFor(node, subject)
      request == ProposalWal(node, proposal)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ node = Leader(context, roundView)
     /\ NodeIdle(node)
     /\ (roundView = 0 \/ NodeInstalledTC(node, roundView - 1))
     /\ BodyHeldBy(durableBodies, node, context, roundView, subject)
     /\ BodyValidatedBy(validatedBodies, node, context, roundView,
                        generation[node], subject)
     /\ ProposalWireValidFor(node, proposal)
     /\ LocalProposalReproposesJustifiedHigh(proposal)
     /\ ~\E prior \in proposalIntents:
           /\ prior.proposer = node
           /\ prior.context = context
           /\ prior.view = roundView
     /\ proposal \notin proposalIntents
     /\ request \in ProposalWalSet
     /\ request \notin pendingProposal

PersistProposalReady(request) ==
  /\ request \in pendingProposal
  /\ request.proposal \notin proposalIntents

FetchBodyReady(node, proposal) ==
  LET body == BodyRecord(node, context, proposal.view, proposal.subject)
  IN /\ ProposalAt(node, proposal) \in seenProposals
     /\ body \notin availableBodies
     /\ body \in BodyRecordSet

RebindRetainedBodyReady(node, proposal) ==
  LET body == BodyRecord(node, context, proposal.view, proposal.subject)
  IN /\ ProposalAt(node, proposal) \in seenProposals
     /\ lockRank[node] # NoRank
     /\ lockSubject[node] = proposal.subject
     /\ RetainedLockedBodyHeldBy(retainedLockedBodies, node, context,
                                  proposal.subject)
     /\ body \notin availableBodies
     /\ body \in BodyRecordSet

StoreBodyReady(node, roundView, subject) ==
  BodyRecord(node, context, roundView, subject) \in availableBodies

ValidateBodyReady(node, proposal) ==
  LET validation == ValidationRecord(node, context, proposal.view,
                                      generation[node], proposal.subject)
  IN /\ ProposalAt(node, proposal) \in seenProposals
     /\ BodyHeldBy(durableBodies, node, context, proposal.view,
                    proposal.subject)
     /\ proposal.subject \in ValidSubjects
     /\ validation \notin validatedBodies
     /\ validation \in ValidationRecordSet

ValidateDecidedBodyReady(node, qc) ==
  LET validation == ValidationRecord(node, context, qc.view,
                                      generation[node], qc.subject)
      decision == [node |-> node, qc |-> qc]
  IN /\ decision \in decisions
     /\ qc.phase = "Commit"
     /\ qc.context = context
     /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
     /\ qc.subject \in ValidSubjects
     /\ validation \notin validatedBodies
     /\ validation \in ValidationRecordSet

ValidateLockedBodyReady(node, qc) ==
  LET validation == ValidationRecord(node, context, qc.view,
                                      generation[node], qc.subject)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ HistoricalLockedPrepareSource(node, qc)
     /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
     /\ qc.subject \in ValidSubjects
     /\ validation \notin validatedBodies
     /\ validation \in ValidationRecordSet

RejectBodyReady(node, proposal) ==
  LET body == BodyRecord(node, context, proposal.view, proposal.subject)
  IN /\ ProposalAt(node, proposal) \in seenProposals
     /\ BodyHeldBy(durableBodies, node, context, proposal.view,
                    proposal.subject)
     /\ proposal.subject \notin ValidSubjects
     /\ body \in BodyRecordSet

BeginPrepareReady(node, proposal) ==
  LET request == PrepareRequestFor(node, proposal)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ NodeIdle(node)
     /\ ProposalAt(node, proposal) \in seenProposals
     /\ ProposalWireValidFor(node, proposal)
     /\ PrepareSignerAvailability(durableBodies, validatedBodies, context,
                                  proposal.view, generation,
                                  proposal.subject, node)
     /\ lockRank[node] < proposal.view
     /\ ~NodeTimedOut(node, proposal.view)
     /\ ~\E prior \in prepareIntents:
           /\ prior.signer = node
           /\ prior.context = context
           /\ prior.view = proposal.view
     /\ request \in PrepareWalSet

PersistPrepareReady(request) ==
  /\ request \in pendingPrepare
  /\ request.vote \notin prepareIntents

BeginObservePrepareReady(node, qc) ==
  /\ QcAt(node, qc) \in receivedQCs
  /\ qc.context = context
  /\ qc.phase = "Prepare"
  /\ qc.view <= nodeView[node]
  /\ qc.view > highestRank[node]
  /\ NodeIdle(node)

PersistObservePrepareReady(request) ==
  request \in pendingObservePrepare

BeginLockCommitReady(node, qc) ==
  LET vote == Vote(context, qc.view, "Commit", qc.subject, node)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ qc.context = context
     /\ qc.phase = "Prepare"
     /\ CurrentOpenPrepareForCommit(node, qc)
     /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
     /\ BodyValidatedBy(validatedBodies, node, context, qc.view,
                        generation[node], qc.subject)
     /\ NodeIdle(node)
     /\ qc.view >= lockRank[node]
     /\ (qc.view = lockRank[node] => qc.subject = lockSubject[node])
     /\ vote \notin commitIntents

PersistLockCommitReady(request) ==
  LET retained == RetainedLockedBodyRecord(
                    request.node, request.qc.context, request.qc.subject)
  IN /\ request \in pendingLockCommit
     /\ request.vote \notin commitIntents
     /\ BodyHeldBy(durableBodies, request.node, request.qc.context,
                    request.qc.view, request.qc.subject)
     /\ retained \in RetainedLockedBodyRecordSet

FormCommitQCReady(node, roundView, subject) ==
  LET signers == VoteSignersAt(node, roundView, "Commit", subject)
      qc == QC(context, roundView, "Commit", subject, signers)
  IN /\ node \in up
     /\ CommitRoundAdmissible(node, roundView, subject)
     /\ QcWireValid(qc)
     /\ qc \in QcRecordSet
     /\ NodeIdle(node)
     /\ ~\E decision \in decisions:
           /\ decision.node = node
           /\ decision.qc.context = context

BeginDecisionReady(node, qc) ==
  /\ node \in ValidatorIds
  /\ QcAt(node, qc) \in receivedQCs
  /\ qc.context = context
  /\ qc.phase = "Commit"
  /\ NodeIdle(node)
  /\ ~\E decision \in decisions:
       /\ decision.node = node
       /\ decision.qc.context = context

PersistTimeoutReady(request) ==
  /\ request \in pendingTimeout
  /\ request.vote \notin timeoutIntents

BeginInstallTCReady(node, tc) ==
  /\ TcAt(node, tc) \in receivedTCs
  /\ tc.view + 1 \in Views
  /\ \/ tc.view >= nodeView[node]
     \/ StrictSameRoundTcUpgrade(node, tc)
  /\ NodeIdle(node)
  /\ NoDecisionForNode(node)

PersistInstallTCReady(request) ==
  /\ request \in pendingInstallTC
  /\ \/ request.tc.view >= nodeView[request.node]
     \/ StrictSameRoundTcUpgrade(request.node, request.tc)
  /\ (StrictSameRoundTcUpgrade(request.node, request.tc)
        => GenerationCanIncrement(generation[request.node]))

CompleteProposalSignatureReady(request) ==
  /\ request \in signProposals
  /\ request.proposal.proposer = request.node
  /\ request.proposal \in proposalIntents

CompleteVoteSignatureReady(request) ==
  /\ request \in signVotes
  /\ request.vote.signer = request.node
  /\ (request.vote \in prepareIntents \/ request.vote \in commitIntents)
  /\ VoteRoundAdmissible(request.node, request.vote)

FormPrepareQCReady(node, roundView, subject) ==
  LET signers == VoteSignersAt(node, roundView, "Prepare", subject)
      qc == QC(context, roundView, "Prepare", subject, signers)
  IN /\ node \in up
     /\ roundView = nodeView[node]
     /\ QcWireValid(qc)
     /\ qc \in QcRecordSet

CompleteTimeoutSignatureReady(request) ==
  LocalTimeoutCompletionGuard(request)

PersistDecisionReady(request) == request \in pendingDecision

FetchCertifiedBodyReady(node, qc) ==
  LET body == BodyRecord(node, context, qc.view, qc.subject)
  IN /\ CertifiedBodyRecoveryAuthority(node, qc)
     /\ ~BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
     /\ body \in BodyRecordSet

ApplyDecisionReady(node, qc) ==
  LET application == [node |-> node, qc |-> qc]
  \* Keep readiness on the same exact current-context Commit authority as the
  \* state-changing action; command evidence is independent provenance.
  IN /\ DecisionCertifiedBodyRecoveryAuthority(node, qc)
     /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
     /\ \E validation \in validatedBodies:
           /\ validation.node = node
           /\ validation.context = context
           /\ validation.view = qc.view
           /\ validation.subject = qc.subject
     /\ application \notin applied

DeliverProposalReady(envelope) ==
  /\ envelope \in proposalNetwork
  /\ envelope.recipient \in up
  /\ ProposalWireValidFor(envelope.recipient, envelope.proposal)

DeliverVoteReady(envelope) ==
  LET received == VoteAt(envelope.recipient, envelope.vote)
  IN /\ envelope \in voteNetwork
     /\ envelope.recipient \in up
     /\ envelope.vote.context = context
     /\ envelope.vote.signer \in CurrentVoters
     /\ VoteRoundAdmissible(envelope.recipient, envelope.vote)
     /\ received \notin receivedVotes

DeliverQCReady(envelope) ==
  /\ envelope \in qcNetwork
  /\ envelope.recipient \in up
  /\ QcWireValid(envelope.qc)

DeliverTimeoutReady(envelope) ==
  TimeoutDeliveryGuard(envelope)

DeliverTCReady(envelope) ==
  /\ envelope \in tcNetwork
  /\ envelope.recipient \in up
  /\ TCValid(envelope.tc)

(***************************************************************************
BeginLockCommit consumes the exact PrepareQC which caused its causal work.
Rank/subject equality is only a scheduling projection and cannot identify a
certificate: different signer sets can certify the same projection.  Ordinary
QC delivery retains the authenticated QC item, restart/install recovery
retains the QcRecord itself, and certified-body recovery retains a response
bound to the exact signed request whose certificate is that QcRecord.
***************************************************************************)

BeginLockCommandEvidenceMatches(command, qc) ==
  \/ command.evidence = qc
  \/ \E item \in AsyncNetworkItems:
       /\ command.evidence = item
       /\ \/ /\ item.kind = "PrepareQC"
             /\ item.envelope.qc = qc
          \/ /\ item.kind = "CertifiedResponse"
             /\ CertifiedResponseCapabilityAuthorized(item)
             /\ \E request \in MatchingSentCertifiedRequests(item):
                  /\ FrozenCertifiedResponseBinding(item, request)
                  /\ request.envelope.certificate = qc

RegularCoreCommand(command) ==
  \/ /\ command.kind = "AssembleBody"
     /\ CommandMatches(command, command.node, nodeView[command.node],
                       command.subject)
     /\ AssembleLocalBody(command.node, command.subject)
  \/ /\ command.kind = "BeginProposal"
     /\ BeginLocalProposal(command.node, command.subject)
  \/ /\ command.kind = "PersistProposal"
     /\ \E request \in pendingProposal:
          /\ CommandMatches(command, request.node, request.proposal.view,
                            request.proposal.subject)
          /\ PersistProposal(request)
  \/ /\ command.kind = "FetchBody"
     /\ ~CertifiedRecoveryFetchFrontier(command)
     /\ HeldChunksFor(command.node, command.view, command.subject) =
          AsyncChunks
     /\ ~BodyHeldBy(durableBodies, command.node, context,
                     command.view, command.subject)
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ FetchBody(command.node, proposal)
  \/ /\ command.kind = "RebindRetainedBody"
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ RebindRetainedBody(command.node, proposal)
  \/ /\ command.kind = "StoreBody"
     /\ StoreBody(command.node, command.view, command.subject)
  \/ /\ command.kind = "ValidateBody"
     /\ \/ \E proposal \in SeenProposalValues:
               /\ CommandMatches(command, command.node, proposal.view,
                                 proposal.subject)
               /\ (ValidateBody(command.node, proposal)
                     \/ RejectBody(command.node, proposal))
        \/ \E qc \in DecisionQcValues:
             /\ CommandMatches(command, command.node, qc.view, qc.subject)
             /\ ValidateDecidedBody(command.node, qc)
        \/ \E qc \in prepareQCs:
             /\ CommandMatches(command, command.node, qc.view, qc.subject)
             /\ ValidateLockedBody(command.node, qc)
  \/ /\ command.kind = "BeginPrepare"
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ BeginPrepare(command.node, proposal)
  \/ /\ command.kind = "PersistPrepare"
     /\ \E request \in pendingPrepare:
          /\ CommandMatches(command, request.node, request.vote.view,
                            request.vote.subject)
          /\ PersistPrepare(request)
  \/ /\ command.kind = "BeginObservePrepare"
     /\ \E qc \in ReceivedQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ BeginObservePrepare(command.node, qc)
  \/ /\ command.kind = "PersistObservePrepare"
     /\ \E request \in pendingObservePrepare:
          /\ CommandMatches(command, request.node, request.qc.view,
                            request.qc.subject)
          /\ PersistObservePrepare(request)
  \/ /\ command.kind = "BeginLockCommit"
     /\ \E qc \in LockCommitQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ BeginLockCommandEvidenceMatches(command, qc)
          /\ BeginLockCommit(command.node, qc)
  \/ /\ command.kind = "PersistLockCommit"
     /\ \E request \in pendingLockCommit:
          /\ CommandMatches(command, request.node, request.qc.view,
                            request.qc.subject)
          /\ PersistLockCommit(request)
  \/ /\ command.kind = "FormCommitQC"
     /\ FormCommitQC(command.node, command.view, command.subject)
  \/ /\ command.kind = "BeginDecision"
     /\ \E qc \in ReceivedQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ BeginDecision(command.node, qc)
  \/ /\ command.kind = "PersistTimeout"
     /\ \E request \in pendingTimeout:
          /\ CommandMatches(command, request.node, request.vote.view,
                            request.vote.highSubject)
          /\ PersistTimeout(request)
  \/ /\ command.kind = "BeginInstallTC"
     /\ \E tc \in ReceivedTcValues:
          /\ command.view = tc.view
          /\ InstallTcEvidenceMatches(command, tc)
          /\ BeginInstallTC(command.node, tc)
  \/ /\ command.kind = "FetchCertifiedBody"
     /\ command.item.kind = "CertifiedResponse"
     /\ command.item.envelope.recipient = command.node
     /\ command.item.envelope.view = command.view
     /\ command.item.envelope.subject = command.subject
     /\ CertifiedResponseCapabilityAuthorized(command.item)
     /\ AcceptCertifiedResponseCapability(
          command.node, command.view, command.subject)

RegularCoreCommandReady(command) ==
  \/ /\ command.kind = "AssembleBody"
     /\ CommandMatches(command, command.node, nodeView[command.node],
                       command.subject)
     /\ AssembleLocalBodyReady(command.node, command.subject)
  \/ /\ command.kind = "BeginProposal"
     /\ BeginLocalProposalReady(command.node, command.subject)
  \/ /\ command.kind = "PersistProposal"
     /\ \E request \in pendingProposal:
          /\ CommandMatches(command, request.node, request.proposal.view,
                            request.proposal.subject)
          /\ PersistProposalReady(request)
  \/ /\ command.kind = "FetchBody"
     /\ ~CertifiedRecoveryFetchFrontier(command)
     /\ HeldChunksFor(command.node, command.view, command.subject) =
          AsyncChunks
     /\ ~BodyHeldBy(durableBodies, command.node, context,
                     command.view, command.subject)
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ FetchBodyReady(command.node, proposal)
  \/ /\ command.kind = "RebindRetainedBody"
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ RebindRetainedBodyReady(command.node, proposal)
  \/ /\ command.kind = "StoreBody"
     /\ StoreBodyReady(command.node, command.view, command.subject)
  \/ /\ command.kind = "ValidateBody"
     /\ \/ \E proposal \in SeenProposalValues:
               /\ CommandMatches(command, command.node, proposal.view,
                                 proposal.subject)
               /\ (ValidateBodyReady(command.node, proposal)
                     \/ RejectBodyReady(command.node, proposal))
        \/ \E qc \in DecisionQcValues:
             /\ CommandMatches(command, command.node, qc.view, qc.subject)
             /\ ValidateDecidedBodyReady(command.node, qc)
        \/ \E qc \in prepareQCs:
             /\ CommandMatches(command, command.node, qc.view, qc.subject)
             /\ ValidateLockedBodyReady(command.node, qc)
  \/ /\ command.kind = "BeginPrepare"
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ BeginPrepareReady(command.node, proposal)
  \/ /\ command.kind = "PersistPrepare"
     /\ \E request \in pendingPrepare:
          /\ CommandMatches(command, request.node, request.vote.view,
                            request.vote.subject)
          /\ PersistPrepareReady(request)
  \/ /\ command.kind = "BeginObservePrepare"
     /\ \E qc \in ReceivedQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ BeginObservePrepareReady(command.node, qc)
  \/ /\ command.kind = "PersistObservePrepare"
     /\ \E request \in pendingObservePrepare:
          /\ CommandMatches(command, request.node, request.qc.view,
                            request.qc.subject)
          /\ PersistObservePrepareReady(request)
  \/ /\ command.kind = "BeginLockCommit"
     /\ \E qc \in LockCommitQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ BeginLockCommandEvidenceMatches(command, qc)
          /\ BeginLockCommitReady(command.node, qc)
  \/ /\ command.kind = "PersistLockCommit"
     /\ \E request \in pendingLockCommit:
          /\ CommandMatches(command, request.node, request.qc.view,
                            request.qc.subject)
          /\ PersistLockCommitReady(request)
  \/ /\ command.kind = "FormCommitQC"
     /\ FormCommitQCReady(command.node, command.view, command.subject)
  \/ /\ command.kind = "BeginDecision"
     /\ \E qc \in ReceivedQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ BeginDecisionReady(command.node, qc)
  \/ /\ command.kind = "PersistTimeout"
     /\ \E request \in pendingTimeout:
          /\ CommandMatches(command, request.node, request.vote.view,
                            request.vote.highSubject)
          /\ PersistTimeoutReady(request)
  \/ /\ command.kind = "BeginInstallTC"
     /\ \E tc \in ReceivedTcValues:
          /\ command.view = tc.view
          /\ InstallTcEvidenceMatches(command, tc)
          /\ BeginInstallTCReady(command.node, tc)
  \/ /\ command.kind = "FetchCertifiedBody"
     /\ command.item.kind = "CertifiedResponse"
     /\ command.item.envelope.recipient = command.node
     /\ command.item.envelope.view = command.view
     /\ command.item.envelope.subject = command.subject
     /\ CertifiedResponseCapabilityAuthorized(command.item)
     /\ InstallCertifiedBodyEffectReady(
          command.node, command.view, command.subject)

AsyncAuxVars ==
  <<asyncOutstandingTags, asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncSentItems, asyncRetainedControl, asyncActiveRequests,
    asyncCertifiedResponseClaim, asyncTransport,
    asyncIngressLanes, asyncIngressReady,
    asyncHeldChunks, asyncHistoricalRecoveryTargets
    >>

ExecuteRegularCommand(command) ==
  /\ RegularCoreCommand(command)
  /\ IF command.kind \in {"FetchBody", "RebindRetainedBody"}
     THEN RetireCompletedBodyCertifiedResponseAuthority(command)
     ELSE UNCHANGED <<asyncActiveRequests,
                      asyncCertifiedResponseClaim>>
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems,
                 asyncRetainedControl, asyncTransport,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

ExecuteSignProposal(command) ==
  /\ command.kind = "SignProposal"
  /\ \E request \in signProposals:
       LET controlItems == ProposalOutbox(request)
           chunkItems ==
             BroadcastChunkOutbox(request.node,
               request.proposal.view, request.proposal.subject)
       IN /\ CommandMatches(command, request.node, request.proposal.view,
                             request.proposal.subject)
          /\ CompleteProposalSignature(request)
          /\ PublishControlAndEphemeralItems(controlItems, chunkItems)
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

ExecuteSignVote(command) ==
  /\ command.kind = "SignVote"
  /\ \E request \in signVotes:
       /\ CommandMatches(command, request.node, request.vote.view,
                         request.vote.subject)
       /\ CompleteVoteSignature(request)
       /\ PublishControlItems(VoteOutbox(request))
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

ExecuteFormPrepareQC(command) ==
  LET signers == VoteSignersAt(command.node, command.view, "Prepare",
                               command.subject)
      qc == QC(context, command.view, "Prepare", command.subject, signers)
      items == QcOutbox(command.node, qc)
  IN /\ command.kind = "FormPrepareQC"
     /\ FormPrepareQC(command.node, command.view, command.subject)
     /\ PublishControlItems(items)
     /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets
                    >>

ExecuteSignTimeout(command) ==
  /\ command.kind = "SignTimeout"
  /\ \E request \in signTimeouts:
       /\ CommandMatches(command, request.node, request.vote.view,
                         request.vote.highSubject)
       /\ CompleteTimeoutSignature(request)
       /\ PublishControlItems(TimeoutOutbox(request))
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

ExecutePersistInstall(command) ==
  /\ command.kind = "PersistInstallTC"
  /\ \E request \in pendingInstallTC:
       /\ command.node = request.node
       /\ command.view = request.tc.view
       /\ InstallTcEvidenceMatches(command, request.tc)
       /\ PersistInstallTC(request)
       /\ PersistInstalledControlAfterInstall(
            request.node, request.tc,
            TcOutbox(request.node, request.tc),
            request.rebroadcast)
  /\ asyncNodeDeadlines' =
       [asyncNodeDeadlines EXCEPT
          ![command.node] =
            asyncNow + AsyncViewTimeout(command.view + 1)]
  /\ asyncRetransmitDeadlines' =
       [asyncRetransmitDeadlines EXCEPT
          ![command.node] = asyncNow + AsyncRetransmitPeriod]
  /\ UNCHANGED <<asyncOutstandingTags,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

ExecutePersistDecision(command) ==
  /\ command.kind = "PersistDecision"
  /\ \E request \in pendingDecision:
       /\ CommandMatches(command, request.node, request.qc.view,
                         request.qc.subject)
       /\ PersistDecision(request)
       /\ PersistDecisionControl(
            request.node, request.qc,
            QcOutbox(request.node, request.qc),
            request.rebroadcast)
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady,
                 asyncHeldChunks, asyncHistoricalRecoveryTargets>>

ExecuteRequestCertifiedBody(command) ==
  /\ command.kind = "RequestCertifiedBody"
  /\ ~BodyHeldBy(durableBodies, command.node, context, command.view,
                  command.subject)
  /\ \E qc \in DecisionQcValues \cup prepareQCs:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ command.evidence = qc
       /\ CertifiedBodyRecoveryAuthority(command.node, qc)
       /\ UNCHANGED vars
       /\ PublishCertifiedRequests(
            CertifiedRequestOutbox(command.node, qc))
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

(***************************************************************************
The reducer owns one certificate-backed `FetchBody` frontier, authorized by
either a durable Commit Decision or the exact TC-installed locked PrepareQC.
The adapter resolves it from the reopened durable catalog when possible;
otherwise the same frontier opens the certified request lifecycle.  Later
Store/Validate/Apply-or-Commit work is emitted only by body-state transitions.
***************************************************************************)
ExecuteDecisionFetch(command) ==
  /\ CertifiedRecoveryFetchFrontier(command)
  /\ IF BodyHeldBy(durableBodies, command.node, context, command.view,
                    command.subject)
     THEN /\ UNCHANGED vars
          /\ UNCHANGED <<asyncSentItems, asyncRetainedControl,
                          asyncActiveRequests,
                          asyncCertifiedResponseClaim, asyncTransport>>
     ELSE \E qc \in DecisionQcValues \cup prepareQCs:
            /\ CommandMatches(command, command.node, qc.view, qc.subject)
            /\ command.evidence = qc
            /\ CertifiedBodyRecoveryAuthority(command.node, qc)
            /\ UNCHANGED vars
            /\ PublishCertifiedRequests(
                 CertifiedRequestOutbox(command.node, qc))
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                  asyncRetransmitDeadlines,
                  asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                  asyncHistoricalRecoveryTargets>>

ExecuteApply(command) ==
  /\ command.kind = "Apply"
  /\ \E qc \in DecisionQcValues:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ ApplyDecision(command.node, qc)
  /\ asyncHistoricalRecoveryTargets' =
       asyncHistoricalRecoveryTargets \ {command.node}
  /\ RetireNodeCertifiedResponseAuthority(command.node)
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems,
                 asyncRetainedControl, asyncTransport,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>

ExecuteCoreDelivery(command) ==
  LET item == command.item
  IN /\ item \in asyncSentItems
     /\ command.node = item.envelope.recipient
     /\ \/ /\ command.kind = "DeliverProposal"
            /\ item.kind = "Proposal"
            /\ DeliverProposal(item.envelope)
        \/ /\ command.kind = "DeliverVote"
            /\ item.kind \in {"PrepareVote", "CommitVote"}
            /\ DeliverVote(item.envelope)
        \/ /\ command.kind = "DeliverQC"
            /\ item.kind \in {"PrepareQC", "CommitQC"}
            /\ DeliverQC(item.envelope)
        \/ /\ command.kind = "DeliverTimeout"
            /\ item.kind = "TimeoutVote"
            /\ DeliverTimeout(item.envelope)
        \/ /\ command.kind = "DeliverTC"
            /\ item.kind = "TimeoutCertificate"
            /\ DeliverTC(item.envelope)
     /\ asyncRetainedControl' =
          IF /\ item.kind = "PrepareQC"
             /\ QcDeliveryCreatesReceipt(command.node, item.envelope.qc)
          THEN RememberedControl(
                 asyncRetainedControl,
                 QcOutbox(command.node, item.envelope.qc))
          ELSE asyncRetainedControl
     /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines, asyncSentItems,
                    asyncActiveRequests, asyncCertifiedResponseClaim,
                    asyncTransport,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets
                    >>

ExecuteChunkDelivery(command) ==
  LET item == command.item
  IN /\ command.kind = "DeliverChunk"
     /\ item \in asyncSentItems
     /\ item.kind = "Chunk"
     /\ item.envelope.recipient = command.node
     /\ item.envelope.chunk \in AsyncChunks
     /\ UNCHANGED vars
     /\ UNCHANGED <<asyncSentItems, asyncRetainedControl,
                    asyncActiveRequests, asyncCertifiedResponseClaim>>
     /\ asyncHeldChunks' =
          asyncHeldChunks \cup
            {AsyncChunkReceipt(command.node, item.envelope.view,
                               item.envelope.subject,
                               item.envelope.chunk)}
     /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines, asyncTransport,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHistoricalRecoveryTargets
                    >>

ExecuteRejectAuthenticatedJunk(command) ==
  LET item == command.item
  IN /\ \/ /\ command.kind = "RejectNormal"
             /\ item.kind = "NormalJunk"
        \/ /\ command.kind = "RejectProgress"
             /\ item.kind = "ProgressJunk"
     /\ item \in asyncSentItems
     /\ item.envelope.recipient = command.node
     /\ UNCHANGED vars
     /\ UNCHANGED <<asyncSentItems, asyncRetainedControl,
                    asyncActiveRequests, asyncCertifiedResponseClaim>>
     /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines, asyncTransport,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets
                    >>

ExecuteCommand(command) ==
  \/ ExecuteRegularCommand(command)
  \/ ExecuteDecisionFetch(command)
  \/ ExecuteSignProposal(command)
  \/ ExecuteSignVote(command)
  \/ ExecuteFormPrepareQC(command)
  \/ ExecuteSignTimeout(command)
  \/ ExecutePersistInstall(command)
  \/ ExecutePersistDecision(command)
  \/ ExecuteRequestCertifiedBody(command)
  \/ ExecuteApply(command)
  \/ ExecuteCoreDelivery(command)
  \/ ExecuteChunkDelivery(command)
  \/ ExecuteRejectAuthenticatedJunk(command)

CausalQueueNonempty(node) == Len(asyncCausalQueues[node]) > 0

CausalQueueLifecycleOrdinals(node) ==
  {AsyncCandidateLifecycleOrdinal(asyncCausalQueues[node][index]):
     index \in 1..Len(asyncCausalQueues[node])}

OldestCausalLifecycleOrdinal(node) ==
  CHOOSE ordinal \in CausalQueueLifecycleOrdinals(node):
    \A other \in CausalQueueLifecycleOrdinals(node): ordinal <= other

OldestCausalCandidateIndices(node) ==
  {index \in 1..Len(asyncCausalQueues[node]):
     AsyncCandidateLifecycleOrdinal(asyncCausalQueues[node][index])
       = OldestCausalLifecycleOrdinal(node)}

NextCausalCandidateIndex(node) ==
  CHOOSE index \in OldestCausalCandidateIndices(node):
    \A other \in OldestCausalCandidateIndices(node): index <= other

HeadCausalCandidate(node) ==
  asyncCausalQueues[node][NextCausalCandidateIndex(node)]

CandidateInFlight(candidate) ==
  candidate \in QueuedCandidates \cup DeferredCandidates
    \cup TrackedWorkCandidates

CausalHeadCanAdvance(node) ==
  LET candidate == HeadCausalCandidate(node)
  IN /\ CausalQueueNonempty(node)
     /\ \/ CandidateInFlight(candidate)
        \/ /\ candidate.class = "Completion"
              /\ CanEnqueueIoClass(node, "Consensus")
              /\ AsyncOutstandingWorkCount(node) < AsyncIoWorkCapacity
        \/ /\ candidate.class # "Completion"
              /\ CanEnqueueClass(node, candidate.class)

(***************************************************************************
Once a Local turn observes causal work, the debt remains active until the
exact head is removed.  The class split preserves the production runtime and
I/O reservations while preventing outer producer/ingress work from stealing
an admission window that the serialized Rust continuation consumes before it
returns to outer ingress.
***************************************************************************)
CausalAdmissionDebtActive(node) ==
  /\ asyncCausalAdmissionOwed[node]
  /\ CausalQueueNonempty(node)

NonCompletionCausalAdmissionDebt(node) ==
  /\ CausalAdmissionDebtActive(node)
  /\ HeadCausalCandidate(node).class # "Completion"

CompletionCausalAdmissionDebt(node) ==
  /\ CausalAdmissionDebtActive(node)
  /\ HeadCausalCandidate(node).class = "Completion"

DiscardCommand(command) ==
  /\ UNCHANGED vars
  /\ UNCHANGED <<asyncSentItems, asyncRetainedControl,
                 asyncActiveRequests, asyncCertifiedResponseClaim>>
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncTransport,
                 asyncIngressLanes, asyncIngressReady,
                 asyncHeldChunks, asyncHistoricalRecoveryTargets
                 >>

(***************************************************************************
ENABLED distributes over this exact finite action union.  The singleton
witness binds the selected command rigidly before it appears under ENABLED,
which is required when this module is instantiated by a parameterized chain
proof.  It avoids enumerating the full candidate carrier and leaves the exact
twelve-arm production dispatch surface unchanged.
***************************************************************************)
ExecuteRegularCommandReady(command) == RegularCoreCommandReady(command)

ExecuteDecisionFetchReady(command) ==
  CertifiedRecoveryFetchFrontier(command)

ExecuteSignProposalReady(command) ==
  /\ command.kind = "SignProposal"
  /\ \E request \in signProposals:
       LET controlItems == ProposalOutbox(request)
       IN /\ CommandMatches(command, request.node, request.proposal.view,
                             request.proposal.subject)
          /\ CompleteProposalSignatureReady(request)
          /\ controlItems \subseteq
               {item \in AsyncNetworkItems:
                  item.kind \in AsyncControlKinds}

ExecuteSignVoteReady(command) ==
  /\ command.kind = "SignVote"
  /\ \E request \in signVotes:
       /\ CommandMatches(command, request.node, request.vote.view,
                         request.vote.subject)
       /\ CompleteVoteSignatureReady(request)
       /\ VoteOutbox(request) \subseteq
            {item \in AsyncNetworkItems:
               item.kind \in AsyncControlKinds}

ExecuteFormPrepareQCReady(command) ==
  LET signers == VoteSignersAt(command.node, command.view, "Prepare",
                               command.subject)
      qc == QC(context, command.view, "Prepare", command.subject, signers)
      items == QcOutbox(command.node, qc)
  IN /\ command.kind = "FormPrepareQC"
     /\ FormPrepareQCReady(command.node, command.view, command.subject)
     /\ items \subseteq
          {item \in AsyncNetworkItems:
             item.kind \in AsyncControlKinds}

ExecuteSignTimeoutReady(command) ==
  /\ command.kind = "SignTimeout"
  /\ \E request \in signTimeouts:
       /\ CommandMatches(command, request.node, request.vote.view,
                         request.vote.highSubject)
       /\ CompleteTimeoutSignatureReady(request)
       /\ TimeoutOutbox(request) \subseteq
            {item \in AsyncNetworkItems:
               item.kind \in AsyncControlKinds}

ExecutePersistInstallReady(command) ==
  /\ command.kind = "PersistInstallTC"
  /\ \E request \in pendingInstallTC:
       /\ command.node = request.node
       /\ command.view = request.tc.view
       /\ InstallTcEvidenceMatches(command, request.tc)
       /\ PersistInstallTCReady(request)

ExecutePersistDecisionReady(command) ==
  /\ command.kind = "PersistDecision"
  /\ \E request \in pendingDecision:
       /\ CommandMatches(command, request.node, request.qc.view,
                         request.qc.subject)
       /\ PersistDecisionReady(request)

ExecuteRequestCertifiedBodyReady(command) ==
  /\ command.kind = "RequestCertifiedBody"
  /\ ~BodyHeldBy(durableBodies, command.node, context, command.view,
                  command.subject)
  /\ \E qc \in DecisionQcValues \cup prepareQCs:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ command.evidence = qc
       /\ CertifiedBodyRecoveryAuthority(command.node, qc)
       /\ \A item \in CertifiedRequestOutbox(command.node, qc):
            item.kind = "CertifiedRequest"

ExecuteApplyReady(command) ==
  /\ command.kind = "Apply"
  /\ \E qc \in DecisionQcValues:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ ApplyDecisionReady(command.node, qc)

ExecuteCoreDeliveryReady(command) ==
  LET item == command.item
  IN /\ item \in asyncSentItems
     /\ AsyncControlServiceOccurrenceIsCurrentOwner(item)
     /\ command.node = item.envelope.recipient
     /\ \/ /\ command.kind = "DeliverProposal"
            /\ item.kind = "Proposal"
            /\ DeliverProposalReady(item.envelope)
        \/ /\ command.kind = "DeliverVote"
            /\ item.kind \in {"PrepareVote", "CommitVote"}
            /\ DeliverVoteReady(item.envelope)
        \/ /\ command.kind = "DeliverQC"
            /\ item.kind \in {"PrepareQC", "CommitQC"}
            /\ DeliverQCReady(item.envelope)
        \/ /\ command.kind = "DeliverTimeout"
            /\ item.kind = "TimeoutVote"
            /\ DeliverTimeoutReady(item.envelope)
        \/ /\ command.kind = "DeliverTC"
            /\ item.kind = "TimeoutCertificate"
            /\ DeliverTCReady(item.envelope)

ExecuteChunkDeliveryReady(command) ==
  LET item == command.item
  IN /\ command.kind = "DeliverChunk"
     /\ item \in asyncSentItems
     /\ item.kind = "Chunk"
     /\ item.envelope.recipient = command.node
     /\ item.envelope.chunk \in AsyncChunks

ExecuteRejectAuthenticatedJunkReady(command) ==
  LET item == command.item
  IN /\ \/ /\ command.kind = "RejectNormal"
             /\ item.kind = "NormalJunk"
        \/ /\ command.kind = "RejectProgress"
             /\ item.kind = "ProgressJunk"
     /\ item \in asyncSentItems
     /\ item.envelope.recipient = command.node

CommandExecutionReady(command) ==
  \E selectedCommand \in {command}:
    \/ ExecuteRegularCommandReady(selectedCommand)
    \/ ExecuteDecisionFetchReady(selectedCommand)
    \/ ExecuteSignProposalReady(selectedCommand)
    \/ ExecuteSignVoteReady(selectedCommand)
    \/ ExecuteFormPrepareQCReady(selectedCommand)
    \/ ExecuteSignTimeoutReady(selectedCommand)
    \/ ExecutePersistInstallReady(selectedCommand)
    \/ ExecutePersistDecisionReady(selectedCommand)
    \/ ExecuteRequestCertifiedBodyReady(selectedCommand)
    \/ ExecuteApplyReady(selectedCommand)
    \/ ExecuteCoreDeliveryReady(selectedCommand)
    \/ ExecuteChunkDeliveryReady(selectedCommand)
    \/ ExecuteRejectAuthenticatedJunkReady(selectedCommand)

LocalAssemblyBusyDispatchAllowed(command) ==
  /\ command.class = "Normal"
  /\ command.kind = "AssembleBody"
  /\ command.item = NoAsyncItem

(***************************************************************************
Every scheduler caller obtains the command from an AsyncCandidateTyped queue.
Keep that structural type guard direct: membership in the equivalent finite
Cartesian carrier forces TLC to enumerate millions of irrelevant records.
***************************************************************************)
CommandDispatchable(command) ==
  /\ AsyncCandidateTyped(command)
  /\ CandidateConsumerCurrent(command)
  /\ CommandExecutionReady(command)
  /\ (NodeIdle(command.node)
        \/ command.class = "Completion"
        \/ LocalAssemblyBusyDispatchAllowed(command))

HistoricalLockedCommitItem(item) ==
  IF item.kind = "CommitVote"
  THEN /\ item.envelope.vote.view # nodeView[item.envelope.recipient]
       /\ LockedPrepareRound(item.envelope.recipient,
                             item.envelope.vote.view,
                             item.envelope.vote.subject)
  ELSE FALSE

ProtectedProgressCommand(command) ==
  CASE command.kind = "DeliverVote" ->
         HistoricalLockedCommitItem(command.item)
    [] command.kind = "DeliverTimeout" ->
         command.item.kind = "TimeoutVote"
    [] command.kind = "DeliverQC" ->
         command.item.kind \in {"PrepareQC", "CommitQC"}
    [] command.kind = "DeliverTC" ->
         command.item.kind = "TimeoutCertificate"
    [] OTHER -> FALSE

SameProtectedProgressSlot(left, right) ==
  /\ ProtectedProgressCommand(left)
  /\ ProtectedProgressCommand(right)
  /\ left.node = right.node
  /\ CASE left.kind = "DeliverVote" ->
            /\ right.kind = "DeliverVote"
            /\ left.item.envelope.vote.signer =
                 right.item.envelope.vote.signer
       [] left.kind = "DeliverQC" ->
            /\ right.kind = "DeliverQC"
            /\ left.item.kind = right.item.kind
       [] left.kind = "DeliverTimeout" ->
            /\ right.kind = "DeliverTimeout"
            /\ left.item.envelope.vote.signer =
                 right.item.envelope.vote.signer
       [] OTHER -> right.kind = "DeliverTC"

SameProtectedProgressSlotIndices(node, command) ==
  {index \in 1..Len(asyncDeferredProgressQueues[node]):
     SameProtectedProgressSlot(
       asyncDeferredProgressQueues[node][index], command)}

DeferredProgressAfter(node, command) ==
  LET queue == asyncDeferredProgressQueues[node]
  IN IF command \in SequenceSet(queue)
     THEN queue
     ELSE IF SameProtectedProgressSlotIndices(node, command) # {}
          THEN queue
          ELSE IF Len(queue) < AsyncDeferredProgressCapacity
               THEN Append(queue, command)
               ELSE queue

DeferredCommandAlreadyRepresented(node, command) ==
  CASE command.class = "Completion" ->
         command \in SequenceSet(asyncDeferredCompletionQueues[node])
    [] command.class = "Progress" ->
         \/ command \in SequenceSet(asyncDeferredProgressQueues[node])
         \/ SameProtectedProgressSlotIndices(node, command) # {}
    [] OTHER ->
         command \in SequenceSet(asyncDeferredNormalQueues[node])

DeferredCommandHasCapacity(node, command) ==
  CASE command.class = "Completion" ->
         Len(asyncDeferredCompletionQueues[node])
           < AsyncDeferredNormalCapacity
    [] command.class = "Progress" ->
         Len(asyncDeferredProgressQueues[node])
           < AsyncDeferredProgressCapacity
    [] OTHER ->
         Len(asyncDeferredNormalQueues[node])
           < AsyncDeferredNormalCapacity

DeferredCommandCanAdmit(node, command) ==
  \/ DeferredCommandAlreadyRepresented(node, command)
  \/ DeferredCommandHasCapacity(node, command)

(***************************************************************************
Admitting a Busy-rejected command creates an immediately owned deferred
service turn.  Production tests the adapter queues directly on every runtime
step, so the model debt bit must be armed by the admission itself; inheriting
a stale FALSE bit would strand the new owner after the reducer becomes idle.
***************************************************************************)
DeferCommand(command) ==
  LET node == command.node
  IN /\ UNCHANGED vars
     /\ asyncDeferredCompletionQueues' =
          IF command.class = "Completion"
          THEN [asyncDeferredCompletionQueues EXCEPT
                  ![node] = IF command \in SequenceSet(@)
                           THEN @
                           ELSE IF Len(@) < AsyncDeferredNormalCapacity
                                THEN Append(@, command) ELSE @]
          ELSE asyncDeferredCompletionQueues
     /\ asyncDeferredProgressQueues' =
          IF command.class = "Progress"
          THEN [asyncDeferredProgressQueues EXCEPT
                  ![node] = DeferredProgressAfter(node, command)]
          ELSE asyncDeferredProgressQueues
     /\ asyncDeferredNormalQueues' =
          IF command.class = "Normal"
          THEN [asyncDeferredNormalQueues EXCEPT
                  ![node] = IF command \in SequenceSet(@)
                           THEN @
                           ELSE IF Len(@) < AsyncDeferredNormalCapacity
                                THEN Append(@, command) ELSE @]
          ELSE asyncDeferredNormalQueues
     /\ UNCHANGED <<asyncDeferredHandoffs, asyncNextDeferredClass>>
     /\ asyncDeferredDrainOwed' =
          [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
     /\ UNCHANGED <<asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncSentItems, asyncRetainedControl,
                    asyncActiveRequests, asyncCertifiedResponseClaim,
                    asyncTransport, asyncIngressLanes,
                    asyncIngressReady, asyncHeldChunks,
                    asyncHistoricalRecoveryTargets
                    >>

DeferredQueueNonempty(node) ==
  Len(asyncDeferredCompletionQueues[node]) > 0
    \/ Len(asyncDeferredProgressQueues[node]) > 0
    \/ Len(asyncDeferredNormalQueues[node]) > 0

DeferredWorkServiceable(node) ==
  /\ asyncDeferredDrainOwed[node]
  /\ DeferredQueueNonempty(node)
  /\ NodeIdle(node)

(***************************************************************************
The adapter's deferred reducer inputs first select the least lifecycle ordinal
across all three classes and use their independent cyclic cursor only as an
equal-ordinal tie break.  The production runtime invokes the selector only
after both serialized Busy fences are open.
The exact-handoff state remains part of the refinement carrier so the modular
ownership proofs can distinguish an already selected occurrence, but a Busy
node cannot create a fresh handoff by consuming a deferred-service turn.
***************************************************************************)
DeferredClassQueue(node, commandClass) ==
  CASE commandClass = "Completion" -> asyncDeferredCompletionQueues[node]
    [] commandClass = "Progress" -> asyncDeferredProgressQueues[node]
    [] OTHER -> asyncDeferredNormalQueues[node]

DeferredClassNonempty(node, commandClass) ==
  Len(DeferredClassQueue(node, commandClass)) > 0

DeferredHandoffActive(node) ==
  asyncDeferredHandoffs[node] # NoAsyncDeferredHandoff

DeferredHandoffCandidate(node) ==
  asyncDeferredHandoffs[node].candidate

DeferredHandoffMatches(node, candidate) ==
  /\ DeferredHandoffActive(node)
  /\ asyncDeferredHandoffs[node] = AsyncDeferredHandoff(candidate)

DeferredHandoffQueueHead(node) ==
  IF DeferredHandoffActive(node)
  THEN LET candidate == DeferredHandoffCandidate(node)
       IN /\ DeferredClassNonempty(node, candidate.class)
          /\ candidate \in SequenceSet(
               DeferredClassQueue(node, candidate.class))
  ELSE FALSE

InstallDeferredHandoff(node, candidate) ==
  asyncDeferredHandoffs' =
    [asyncDeferredHandoffs EXCEPT
       ![node] = AsyncDeferredHandoff(candidate)]

RetainDeferredHandoffs ==
  UNCHANGED asyncDeferredHandoffs

ClearDeferredHandoff(node) ==
  asyncDeferredHandoffs' =
    [asyncDeferredHandoffs EXCEPT ![node] = NoAsyncDeferredHandoff]

DeferredHandoffAllowsExecution(node, candidate) ==
  /\ CommandDispatchable(candidate)
  /\ (~DeferredHandoffActive(node)
        \/ DeferredHandoffMatches(node, candidate))

DeferredHandoffBlocksExecution(node, candidate) ==
  /\ DeferredHandoffActive(node)
  /\ ~DeferredHandoffMatches(node, candidate)

DeferredClassLifecycleOrdinals(node, commandClass) ==
  {AsyncCandidateLifecycleOrdinal(
     DeferredClassQueue(node, commandClass)[index]):
     index \in 1..Len(DeferredClassQueue(node, commandClass))}

OldestDeferredClassLifecycleOrdinal(node, commandClass) ==
  CHOOSE ordinal \in DeferredClassLifecycleOrdinals(node, commandClass):
    \A other \in DeferredClassLifecycleOrdinals(node, commandClass):
      ordinal <= other

OldestDeferredClassIndices(node, commandClass) ==
  {index \in 1..Len(DeferredClassQueue(node, commandClass)):
     AsyncCandidateLifecycleOrdinal(
       DeferredClassQueue(node, commandClass)[index])
       = OldestDeferredClassLifecycleOrdinal(node, commandClass)}

FirstOldestDeferredClassIndex(node, commandClass) ==
  CHOOSE index \in OldestDeferredClassIndices(node, commandClass):
    \A other \in OldestDeferredClassIndices(node, commandClass):
      index <= other

OldestDeferredClassCandidate(node, commandClass) ==
  DeferredClassQueue(node, commandClass)[
    FirstOldestDeferredClassIndex(node, commandClass)]

DeferredHeadLifecycleOrdinals(node) ==
  {AsyncCandidateLifecycleOrdinal(
     OldestDeferredClassCandidate(node, commandClass)):
     commandClass \in AsyncCommandClasses,
     DeferredClassNonempty(node, commandClass)}

OldestDeferredHeadLifecycleOrdinal(node) ==
  CHOOSE ordinal \in DeferredHeadLifecycleOrdinals(node):
    \A other \in DeferredHeadLifecycleOrdinals(node): ordinal <= other

DeferredClassOwnsOldestLifecycle(node, commandClass) ==
  /\ DeferredClassNonempty(node, commandClass)
  /\ AsyncCandidateLifecycleOrdinal(
       OldestDeferredClassCandidate(node, commandClass))
       = OldestDeferredHeadLifecycleOrdinal(node)

SelectedDeferredClass(node) ==
  IF DeferredHandoffActive(node)
  THEN DeferredHandoffCandidate(node).class
  ELSE LET first == asyncNextDeferredClass[node]
           second == NextCommandClass(first)
           third == NextCommandClass(second)
       IN IF DeferredClassOwnsOldestLifecycle(node, first)
          THEN first
          ELSE IF DeferredClassOwnsOldestLifecycle(node, second)
               THEN second
               ELSE third

NextDeferredCommandIndex(node) ==
  IF DeferredHandoffActive(node)
  THEN CHOOSE index \in
         1..Len(DeferredClassQueue(node, SelectedDeferredClass(node))):
         DeferredClassQueue(node, SelectedDeferredClass(node))[index]
           = DeferredHandoffCandidate(node)
  ELSE FirstOldestDeferredClassIndex(
         node, SelectedDeferredClass(node))

NextDeferredCommand(node) ==
  DeferredClassQueue(node, SelectedDeferredClass(node))[
    NextDeferredCommandIndex(node)]

AdvanceNextDeferredClass(node) ==
  asyncNextDeferredClass' =
    [asyncNextDeferredClass EXCEPT
       ![node] = NextCommandClass(SelectedDeferredClass(node))]

RemoveNextDeferredCommand(node) ==
  /\ IF SelectedDeferredClass(node) = "Completion"
     THEN /\ asyncDeferredCompletionQueues' =
                [asyncDeferredCompletionQueues EXCEPT
                   ![node] = SequenceWithoutIndex(
                     @, NextDeferredCommandIndex(node))]
          /\ UNCHANGED <<asyncDeferredProgressQueues,
                         asyncDeferredNormalQueues>>
     ELSE IF SelectedDeferredClass(node) = "Progress"
          THEN /\ asyncDeferredProgressQueues' =
                     [asyncDeferredProgressQueues EXCEPT
                        ![node] = SequenceWithoutIndex(
                          @, NextDeferredCommandIndex(node))]
               /\ UNCHANGED <<asyncDeferredCompletionQueues,
                              asyncDeferredNormalQueues>>
          ELSE /\ asyncDeferredNormalQueues' =
                     [asyncDeferredNormalQueues EXCEPT
                        ![node] = SequenceWithoutIndex(
                          @, NextDeferredCommandIndex(node))]
               /\ UNCHANGED <<asyncDeferredCompletionQueues,
                              asyncDeferredProgressQueues>>
  /\ AdvanceNextDeferredClass(node)

(***************************************************************************
Per-recipient, per-source transport admission.
***************************************************************************)

DeliveryKind(item) ==
  CASE item.kind = "Proposal" -> "DeliverProposal"
    [] item.kind \in {"PrepareVote", "CommitVote"} -> "DeliverVote"
    [] item.kind \in {"PrepareQC", "CommitQC"} -> "DeliverQC"
    [] item.kind = "TimeoutVote" -> "DeliverTimeout"
    [] item.kind = "TimeoutCertificate" -> "DeliverTC"
    [] item.kind = "Chunk" -> "DeliverChunk"
    [] item.kind = "CertifiedRequest" -> "AcceptCertifiedRequest"
    [] item.kind = "CertifiedResponse" -> "AcceptCertifiedResponse"
    [] item.kind = "CommitCertificateRequest" -> "AcceptCertifiedRequest"
    [] item.kind = "CommitCertificateResponse" -> "AcceptCertifiedResponse"
    [] item.kind = "NormalJunk" -> "RejectNormal"
    [] item.kind = "ProgressJunk" -> "RejectProgress"
    [] OTHER -> "DeliverChunk"

\* Requests own outer FairV2Ingress Progress slots, but bypass the serialized
\* runtime command queue and enter the independently ranked Serve I/O lane.
\* Their candidate class is therefore not runtime Progress.
DeliveryClass(item) ==
  IF HistoricalLockedCommitItem(item)
       \/ item.kind \in {"PrepareQC", "CommitQC", "TimeoutVote",
                    "TimeoutCertificate", "Chunk", "CertifiedResponse",
                    "CommitCertificateResponse",
                    "ProgressJunk"}
  THEN "Progress"
  ELSE "Normal"

DeliverySubject(item) ==
  CASE item.kind = "Proposal" -> item.envelope.proposal.subject
    [] item.kind \in {"PrepareVote", "CommitVote", "TimeoutVote"} ->
         IF item.kind = "TimeoutVote"
         THEN item.envelope.vote.highSubject
         ELSE item.envelope.vote.subject
    [] item.kind \in {"PrepareQC", "CommitQC"} -> item.envelope.qc.subject
    [] item.kind = "CommitCertificateResponse" -> item.envelope.qc.subject
    [] item.kind = "TimeoutCertificate" -> NoSubject
    [] OTHER -> item.envelope.subject

DeliveryView(item) ==
  CASE item.kind = "Proposal" -> item.envelope.proposal.view
    [] item.kind \in {"PrepareVote", "CommitVote", "TimeoutVote"} ->
         item.envelope.vote.view
    [] item.kind \in {"PrepareQC", "CommitQC"} -> item.envelope.qc.view
    [] item.kind = "CommitCertificateResponse" -> item.envelope.qc.view
    [] item.kind = "TimeoutCertificate" -> item.envelope.tc.view
    [] OTHER -> item.envelope.view

DeliveryHeight(item) ==
  CASE item.kind = "Proposal" -> item.envelope.proposal.context.height
    [] item.kind \in {"PrepareVote", "CommitVote", "TimeoutVote"} ->
         item.envelope.vote.context.height
    [] item.kind \in {"PrepareQC", "CommitQC"} ->
         item.envelope.qc.context.height
    [] item.kind = "CommitCertificateResponse" ->
         item.envelope.qc.context.height
    [] item.kind = "TimeoutCertificate" -> item.envelope.tc.context.height
    [] OTHER -> item.envelope.height

AsyncDeliveryCandidateCausalOriginAt(item, consumerContext) ==
  LET node == item.envelope.recipient
      subject == DeliverySubject(item)
  IN AsyncCandidateCausalOrigin(
       DeliveryKind(item), node,
       DeliveryHeight(item), DeliveryView(item), subject, item,
       consumerContext, item, subject, subject, subject)

DeliveryCandidate(item) ==
  LET node == item.envelope.recipient
      subject == DeliverySubject(item)
  IN AsyncCandidateWithIdentityAndOrigin(
       DeliveryClass(item), DeliveryKind(item), node,
       DeliveryHeight(item), DeliveryView(item), subject, item,
       context, nodeView[node], generation[node], item,
       subject, subject, subject,
       AsyncDeliveryCandidateCausalOriginAt(item, context))

DueSourcePackets(recipient, source) ==
  {packet \in asyncTransport:
     /\ packet.item.envelope.recipient = recipient
     /\ packet.item.source = source
     /\ packet.deadline <= asyncNow}

OldestDueSourcePacket(recipient, source) ==
  CHOOSE packet \in DueSourcePackets(recipient, source):
    \A other \in DueSourcePackets(recipient, source):
      packet.sentAt <= other.sentAt

(***************************************************************************
Policy rejection consumes one delivery attempt without granting queue
ownership.  Invalid/stale responses are rejected before the physical gate.
An authenticated response blocked by another live claim at the same recipient
is finite backpressure, not rejection: its exact transport packet remains
owned and retryable, matching `SumeragiIngressDisposition::Retry` and the
caller-side retained queue.  Claims at other recipients are independent.
Likewise, a live certified request at the packet recipient prevents fresh
generic traffic from repeatedly refilling that process's aggregate untrusted
completion owner.  Each such relayed occurrence is rejected rather than
returned for retry, so an older caller-side per-source FIFO entry cannot
remain ahead of the exact response forever.  Other recipients and direct
validator completion lanes remain admissible, and a fresh relay occurrence
can be admitted after the recipient's request authority retires.  Chunk
service has a separate durable stage marker: an already held receipt, a
strictly advanced consumer view, or durable Decision rejects retry admission.
This permits bounded candidate lifecycle-record reclamation without recreating
the old route-neutral chunk stage.
***************************************************************************)
CertifiedResponsePacketPolicyRejected(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ ~CertifiedResponseAuthorized(item)

UntrustedGenericCompletionPacketPolicyRejected(item) ==
  /\ IngressAdmissionClass(item) = "TransportCompletion"
  /\ IngressResourceSource(item) = AsyncUntrustedSource
  /\ ActiveCertifiedRequestHashesAt(item.envelope.recipient) # {}

AsyncCandidateServicePacketRetired(item) ==
  AsyncCandidateServiceCoalesced(DeliveryCandidate(item))

AsyncChunkIngressStageRetired(item) ==
  LET recipient == item.envelope.recipient
      receipt ==
        AsyncChunkReceipt(
          recipient, item.envelope.view,
          item.envelope.subject, item.envelope.chunk)
  IN \/ receipt \in asyncHeldChunks
     \/ item.envelope.view < nodeView[recipient]
     \/ NodeHasDecision(recipient)

AsyncControlIngressStageRetired(item) ==
  AsyncControlServiceIdentityServicedOrAdvanced(item)

AsyncCertifiedResponseIngressStageRetired(item) ==
  LET recipient == item.envelope.recipient
      body ==
        BodyRecord(recipient, context, item.envelope.view,
                   item.envelope.subject)
  IN \/ item.envelope.requestHash
           \notin ActiveCertifiedRequestHashes
     \/ body \in availableBodies
     \/ BodyHeldBy(durableBodies, recipient, context,
                    item.envelope.view, item.envelope.subject)
     \/ NodeHasDecision(recipient)

AsyncCommitCertificateResponseIngressStageRetired(item) ==
  LET recipient == item.envelope.recipient
      qc == item.envelope.qc
  IN \/ {request \in asyncActiveRequests:
           /\ request.kind = "CommitCertificateRequest"
           /\ AsyncCommitCertificateRequestRegistrationIdentity(request)
                = AsyncCommitCertificateRequestRegistrationIdentity(
                    item.envelope.request)} = {}
     \/ QcAt(recipient, qc) \in receivedQCs
     \/ NodeHasDecision(recipient)

AsyncJunkIngressStageRetired(item) ==
  item.kind \in {"NormalJunk", "ProgressJunk", "Noise"}

AsyncCandidateStageRetired(item) ==
  CASE item.kind = "Chunk" ->
         AsyncChunkIngressStageRetired(item)
    [] item.kind \in AsyncControlKinds ->
         AsyncControlIngressStageRetired(item)
    [] item.kind = "CertifiedResponse" ->
         AsyncCertifiedResponseIngressStageRetired(item)
    [] item.kind = "CommitCertificateResponse" ->
         AsyncCommitCertificateResponseIngressStageRetired(item)
    [] item.kind \in {"NormalJunk", "ProgressJunk", "Noise"} ->
         AsyncJunkIngressStageRetired(item)
    [] OTHER -> FALSE

IngressPacketPolicyRejected(item) ==
  \/ CertifiedResponsePacketPolicyRejected(item)
  \/ UntrustedGenericCompletionPacketPolicyRejected(item)
  \/ AsyncControlServiceAdmissionCoalesced(item)
  \/ AsyncCandidateServicePacketRetired(item)
  \/ AsyncCandidateStageRetired(item)

CommitCertificateRequestAuthorized(item) ==
  /\ item.kind = "CommitCertificateRequest"
  /\ item.source
       \in CurrentVoters \cup asyncHistoricalRecoveryTargets
  /\ item.envelope.recipient \in CurrentVoters
  /\ item.envelope.height = context.height

AsyncServeRequestAuthorized(item) ==
  /\ item.kind \in AsyncReplyRequestKinds
  /\ IngressItemHasAuthenticatedHistory(item)
  /\ IF item.kind = "CertifiedRequest"
     THEN CertifiedRequestAuthorized(item)
     ELSE CommitCertificateRequestAuthorized(item)

AsyncServeRequestServiceable(node, item) ==
  /\ item.kind \in AsyncReplyRequestKinds
  /\ IF item.kind = "CertifiedRequest"
     THEN CertifiedServeCanRespond(node, item)
     ELSE CommitCertificateServeCanRespond(item)

AsyncServeLifecycleAdmissionRequired(node, item) ==
  AsyncServeRequestAuthorized(item)
    /\ AsyncServeRequestServiceable(node, item)

AsyncServeLifecycleDrainRequired(node, item) ==
  LET identity == AsyncServeLogicalRequestIdentity(node, item)
  IN \/ AsyncServeLifecycleOwned(node, identity)
     \/ AsyncServeLifecycleAdmissionRequired(node, item)

AsyncServeRequestLifecycleRetired(item) ==
  /\ item.kind \in AsyncReplyRequestKinds
  /\ \/ item \notin asyncActiveRequests
     \/ /\ item.source \in ValidatorIds
           /\ NodeHasDecision(item.source)

AsyncServeLifecycleSuperseded(node, item) ==
  LET identity == AsyncServeLogicalRequestIdentity(node, item)
      family == AsyncServeLifecycleFamily(node, item)
  IN /\ ~AsyncServeLifecycleOwned(node, identity)
     /\ AsyncServeLifecycleFamilyOwned(node, family)
     /\ AsyncServeRequestView(item) <
          AsyncServeFamilyHighWatermark(node, family)
     /\ AsyncServeRequestLifecycleRetired(item)

AsyncServeLifecycleConflict(node, item) ==
  LET identity == AsyncServeLogicalRequestIdentity(node, item)
      family == AsyncServeLifecycleFamily(node, item)
  IN /\ ~AsyncServeLifecycleOwned(node, identity)
     /\ AsyncServeLifecycleFamilyOwned(node, family)
     /\ AsyncServeRequestView(item) =
          AsyncServeFamilyHighWatermark(node, family)

(***************************************************************************
An authorized exact request without an existing lifecycle does not leave
transport until its immutable local retention owner can answer it.  This
keeps an initially unserviceable request out of the coalescing ingress lanes:
if StoreBody, Apply, or historical recovery later makes it serviceable, the
same retained packet crosses the atomic admission/reservation cut then.
Existing lifecycle, superseded, and same-view conflict identities may still
enter so retries can replay, retire, or be rejected deterministically.
***************************************************************************)
AsyncServeTransportAdmissionGateAllows(node, item) ==
  IF AsyncServeRequestAuthorized(item)
  THEN \/ AsyncServeRequestServiceable(node, item)
       \/ AsyncServeLifecycleOwned(
            node, AsyncServeLogicalRequestIdentity(node, item))
       \/ AsyncServeLifecycleSuperseded(node, item)
       \/ AsyncServeLifecycleConflict(node, item)
  ELSE TRUE

CanAdmitIngressItem(item) ==
  /\ ~AsyncControlServiceAdmissionCoalesced(item)
  /\ ~AsyncCandidateServicePacketRetired(item)
  /\ ~AsyncCandidateStageRetired(item)
  /\ AsyncServeTransportAdmissionGateAllows(
       item.envelope.recipient, item)
  /\ IngressDepth(item.envelope.recipient)
       < IngressUsableCapacityAfterAdmission(item)
  /\ AsyncTimeoutVoteByteGateAllows(item)
  /\ AsyncTransportCompletionOwnerGateAllows(item)
  /\ CertifiedResponseFreshClaimGateAllows(item)
  /\ AsyncUntrustedGenericCompletionGateAllows(item)

ExactServeTransportAdmissionCanAdvance(node, request) ==
  LET identity == AsyncServeLogicalRequestIdentity(node, request)
      family == AsyncServeLifecycleFamily(node, request)
      roundView == AsyncServeRequestView(request)
  IN IF AsyncServeLifecycleOwned(node, identity)
     THEN TRUE
     ELSE IF AsyncServeLifecycleSuperseded(node, request)
          THEN TRUE
     ELSE IF AsyncServeLifecycleConflict(node, request)
          THEN TRUE
          ELSE IF ~AsyncServeLifecycleFamilyOwned(node, family)
                    THEN /\ AsyncServeIngressLifecycleOwnerIdentities(
                               node) = {}
                         /\ AsyncServeOffQueueReservations(node) = {}
                    ELSE /\ AsyncServeFamilyTombstoneRecords(
                                 node, family) # {}
                         /\ AsyncServeIngressLifecycleOwnerIdentities(
                              node) = {}
                         /\ AsyncServeOffQueueReservations(node) = {}
                         /\ roundView >
                              AsyncServeFamilyHighWatermark(node, family)
                         /\ AsyncServeFamilyAdvanceRetiresPriorRequests(
                              node, request)

AdmitHiddenPacket(recipient, source) ==
  LET packet == OldestDueSourcePacket(recipient, source)
      item == packet.item
      resourceSource == IngressResourceSource(item)
      lane == IngressLane(recipient, resourceSource)
      candidate == DeliveryCandidate(item)
  IN /\ recipient \in up
     /\ ~ResponsiveReplayQuarantined(recipient)
     /\ DueSourcePackets(recipient, source) # {}
     /\ ~IngressHasCoalescingOwner(item)
     /\ CanAdmitIngressItem(item)
     /\ IF AsyncServeLifecycleAdmissionRequired(recipient, item)
        THEN ExactServeTransportAdmissionCanAdvance(recipient, item)
        ELSE TRUE
     /\ asyncTransport' = asyncTransport \ {packet}
     /\ asyncIngressLanes' =
          [asyncIngressLanes EXCEPT
             ![recipient][resourceSource] = Append(@, item)]
     /\ asyncIngressReady' =
          IF Len(lane) = 0
          THEN [asyncIngressReady EXCEPT
                  ![recipient] = Append(@, resourceSource)]
          ELSE asyncIngressReady
     /\ asyncCertifiedResponseClaim' =
          IF item.kind = "CertifiedResponse"
          THEN asyncCertifiedResponseClaim
                 \cup {AsyncCertifiedResponseCanonicalWireIdentity(item)}
          ELSE asyncCertifiedResponseClaim
     /\ IF AsyncServeLifecycleAdmissionRequired(recipient, item)
        THEN AcceptOrReserveExactServeIngress(recipient, candidate)
        ELSE UNCHANGED
               <<AsyncServeLifecycleVars,
                 AsyncServeIngressAdmissionVars>>
     /\ UNCHANGED AsyncDeferredVars
     /\ LeaveCausalQueues
     /\ UNCHANGED AsyncLocalAdmissionVars
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, asyncIoQueues,
                    asyncOutstandingWork, asyncIoReadyCompletions,
                    asyncLocalReadyCompletions, asyncNextCompletionSource,
                    asyncIoControlAvailable, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncSentItems, asyncRetainedControl, asyncActiveRequests,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets
                    >>

(*
FairV2Ingress coalesces a reply request by its route-local logical lifecycle
identity while its normalized resource lane owns the first occurrence.
Certified responses use the route-neutral authenticated-envelope projection,
so the same signed response coalesces across relay vias while that recipient's
singleton claim remains unchanged.
*)
CoalesceHiddenPacket(recipient, source) ==
  LET packet == OldestDueSourcePacket(recipient, source)
      item == packet.item
  IN /\ recipient \in up
     /\ ~ResponsiveReplayQuarantined(recipient)
     /\ DueSourcePackets(recipient, source) # {}
     /\ IngressHasCoalescingOwner(item)
     /\ \/ item.kind # "CertifiedResponse"
        \/ CertifiedResponseClaimMatches(item)
     /\ asyncTransport' = asyncTransport \ {packet}
     /\ UNCHANGED <<asyncIngressLanes, asyncIngressReady>>
     /\ UNCHANGED AsyncDeferredVars
     /\ LeaveCausalQueues
     /\ UNCHANGED AsyncLocalAdmissionVars
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncSentItems, asyncRetainedControl,
                    asyncActiveRequests, asyncCertifiedResponseClaim,
                    asyncHeldChunks,
                    asyncHistoricalRecoveryTargets>>

THEOREM AsyncLiveServeIngressDuplicateRetainsSchedulerOrdinal ==
  \A recipient \in ValidatorIds,
     source \in AsyncIngressSources:
    LET item == OldestDueSourcePacket(recipient, source).item
        identity == AsyncServeLogicalRequestIdentity(recipient, item)
    IN /\ item.kind \in AsyncReplyRequestKinds
       /\ AsyncServeIngressAdmissionOwned(recipient, identity)
       /\ CoalesceHiddenPacket(recipient, source)
       => /\ AsyncServeIngressAdmissionOwned(recipient, identity)'
          /\ AsyncServeIngressAdmissionSchedulerOrdinal(
                 recipient, identity)'
               = AsyncServeIngressAdmissionSchedulerOrdinal(
                   recipient, identity)
BY Isa
   DEF CoalesceHiddenPacket, AsyncIoVars,
       AsyncServeIngressAdmissionVars,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionSchedulerOrdinal,
       AsyncServeIngressAdmissionRecord

DropPolicyRejectedHiddenPacket(recipient, source) ==
  LET packet == OldestDueSourcePacket(recipient, source)
      item == packet.item
  IN /\ recipient \in up
     /\ ~ResponsiveReplayQuarantined(recipient)
     /\ DueSourcePackets(recipient, source) # {}
     /\ IngressPacketPolicyRejected(item)
     /\ asyncTransport' = asyncTransport \ {packet}
     /\ UNCHANGED <<asyncIngressLanes, asyncIngressReady>>
     /\ UNCHANGED AsyncDeferredVars
     /\ LeaveCausalQueues
     /\ UNCHANGED AsyncLocalAdmissionVars
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncSentItems, asyncRetainedControl,
                    asyncActiveRequests, asyncCertifiedResponseClaim,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets>>

AdmitFreshHiddenPacket(recipient, source) ==
  AdmitHiddenPacket(recipient, source)

AdmitIngressPacket(recipient, source) ==
  \/ AdmitHiddenPacket(recipient, source)
  \/ CoalesceHiddenPacket(recipient, source)
  \/ DropPolicyRejectedHiddenPacket(recipient, source)

HeadIngressSource(node) == Head(asyncIngressReady[node])

HeadIngressItem(node) ==
  Head(IngressLane(node, HeadIngressSource(node)))

IngressItemAt(node, index) ==
  Head(IngressLane(node, asyncIngressReady[node][index]))

MatchingCommitCertificateRequests(response) ==
  {request \in asyncActiveRequests:
     /\ request.kind = "CommitCertificateRequest"
     /\ AsyncCommitCertificateRequestRegistrationIdentity(request)
          = AsyncCommitCertificateRequestRegistrationIdentity(
              response.envelope.request)}

CommitCertificateResponseAuthorized(item) ==
  /\ item.kind = "CommitCertificateResponse"
  /\ item.source \in AsyncIngressSources
  /\ item.envelope.request \in asyncActiveRequests
  /\ CommitCertificateRequestAuthorized(item.envelope.request)
  /\ item.envelope.qc \in commitQCs
  /\ item.envelope.qc.context = context
  /\ item.envelope.qc.phase = "Commit"
  /\ MatchingCommitCertificateRequests(item) # {}

(***************************************************************************
One finite high-watermark owner represents each
archive/kind/context/requester/phase family.  The certified subject remains in
the full exact identity, while certificate uniqueness rejects a different
same-view subject or request hash instead of allocating another tombstone.  A
delayed lower-view retry is absorbed only after its requester has retired that
exact active request (or already reached Decision); it is therefore an
already-reached lifecycle goal, not a silently dropped live request.  A
higher-view request may replace only a terminal family tombstone and receives
a new immutable admission ordinal.
***************************************************************************)
CoalesceSupersededExactServeRequest(node, candidate) ==
  /\ candidate.kind = "AcceptCertifiedRequest"
  /\ candidate.item.kind \in AsyncReplyRequestKinds
  /\ AsyncServeLifecycleSuperseded(node, candidate.item)
  /\ UNCHANGED <<asyncIoQueues, asyncNextServeAdmissionOrdinal,
                  asyncServeAdmissions, asyncServeTombstones>>

RejectConflictingExactServeRequest(node, candidate) ==
  /\ candidate.kind = "AcceptCertifiedRequest"
  /\ candidate.item.kind \in AsyncReplyRequestKinds
  /\ AsyncServeLifecycleConflict(node, candidate.item)
  /\ UNCHANGED <<asyncIoQueues, asyncNextServeAdmissionOrdinal,
                  asyncServeAdmissions, asyncServeTombstones>>

AcceptOrCoalesceExactServeRequest(node, candidate) ==
  \/ ResumeExactServeCapacity(node, candidate)
  \/ CoalesceExactServeCapacity(node, candidate)
  \/ CoalesceSupersededExactServeRequest(node, candidate)
  \/ RejectConflictingExactServeRequest(node, candidate)

ExactServeIngressCanAdvance(node, request) ==
  LET identity == AsyncServeLogicalRequestIdentity(node, request)
  IN IF AsyncServeLifecycleOwned(node, identity)
     THEN \/ AsyncServeLifecycleTombstone(node, identity)
          \/ AsyncServeJobQueued(node, identity)
          \/ CanResumeExactServeCapacity(node, identity)
     ELSE IF AsyncServeLifecycleSuperseded(node, request)
          THEN TRUE
          ELSE IF AsyncServeLifecycleConflict(node, request)
               THEN TRUE
               ELSE FALSE

ExactServeIngressNeedsQueueSlot(node, request) ==
  LET identity == AsyncServeLogicalRequestIdentity(node, request)
  IN AsyncServeLiveReservationOwned(node, identity)
       /\ ~AsyncServeJobQueued(node, identity)
       /\ ~AsyncServeLifecycleSuperseded(node, request)
       /\ ~AsyncServeLifecycleConflict(node, request)

(***************************************************************************
A terminal lifecycle is a retained response cache, not a request sink.  An
exact retransmission re-emits the cached output into the ordinary bounded
transport corridor while the old ingress/Serve stage remains retired.
***************************************************************************)
AsyncServeCachedReplayItems(node, item) ==
  LET identity == AsyncServeLogicalRequestIdentity(node, item)
  IN IF /\ AsyncServeRequestAuthorized(item)
           /\ AsyncServeLifecycleTombstone(node, identity)
     THEN AsyncServeTombstoneOutputs(node, identity)
     ELSE {}

(***************************************************************************
The authenticated archive hop is not a historical vote authority.  The
synthetic Core envelope cites one frozen CommitQC signer, while the exact
outer CommitCertificateResponse remains immutable candidate evidence.  This
prevents key rotation from either excluding a valid archive server or
retagging that server as a fictitious historical voter.
***************************************************************************)
HistoricalCommitQcSigner(response) ==
  IF response.envelope.qc.signers # {}
  THEN CHOOSE signer \in response.envelope.qc.signers: TRUE
  ELSE 0

DiscoveredCommitQcItem(response) ==
  AsyncNetworkItem(
    "CommitQC", HistoricalCommitQcSigner(response),
    QcEnvelope(response.envelope.recipient, response.envelope.qc))

AsyncCommitCertificateResponseCandidateCausalOriginAt(
    item, consumerContext) ==
  LET discovered == DiscoveredCommitQcItem(item)
      node == discovered.envelope.recipient
      subject == discovered.envelope.qc.subject
  IN AsyncCandidateCausalOrigin(
       DeliveryKind(discovered), node,
       DeliveryHeight(discovered), DeliveryView(discovered), subject,
       discovered, consumerContext, item,
       subject, subject, subject)

CommitCertificateResponseCandidate(item) ==
  LET discovered == DiscoveredCommitQcItem(item)
      node == discovered.envelope.recipient
      subject == discovered.envelope.qc.subject
  IN AsyncCandidateWithIdentityAndOrigin(
       DeliveryClass(discovered), DeliveryKind(discovered), node,
       DeliveryHeight(discovered), DeliveryView(discovered), subject,
       discovered, context, nodeView[node], generation[node], item,
       subject, subject, subject,
       AsyncCommitCertificateResponseCandidateCausalOriginAt(
         item, context))

(***************************************************************************
Exact Commit-import causal provenance.

Production never accepts an asserted causal root for this reducer tail.
Authenticated DeliverQC ingress mints the root from the deeply validated wire
occurrence, and BeginDecision/PersistDecision are enqueued with the immutable
lifecycle owner returned beside the selected parent.  The Rust runtime checks
the complete origin projection and fails closed if a child attempts to replace
that owner.  The model records the same boundary explicitly: a Commit
DeliverQC, every BeginDecision, and an imported PersistDecision may execute
only with the canonical direct-QC or CommitCertificateResponse root.

The local FormCommitQC -> PersistDecision path is intentionally outside this
predicate.  Its evidence is a CommitVote causal root and its WAL has
`rebroadcast = TRUE`; it is not a historical certificate import.
***************************************************************************)

AsyncCommitImportDirectEvidence(candidate, qc) ==
  /\ candidate.evidence \in asyncSentItems
  /\ candidate.evidence.kind = "CommitQC"
  /\ candidate.evidence.envelope = QcEnvelope(candidate.node, qc)
  /\ candidate.causalOrigin =
       AsyncDeliveryCandidateCausalOriginAt(
         candidate.evidence, candidate.consumerContext)

AsyncCommitImportResponseEvidence(candidate, qc) ==
  /\ candidate.evidence \in asyncSentItems
  /\ candidate.evidence.kind = "CommitCertificateResponse"
  /\ candidate.evidence.envelope.recipient = candidate.node
  /\ candidate.evidence.envelope.qc = qc
  /\ CommitCertificateRequestAuthorized(
       candidate.evidence.envelope.request)
  /\ candidate.causalOrigin =
       AsyncCommitCertificateResponseCandidateCausalOriginAt(
         candidate.evidence, candidate.consumerContext)

AsyncCommitImportCandidateLineage(candidate, qc) ==
  /\ candidate \in AsyncCandidateSet
  /\ qc \in commitQCs
  /\ qc.context = candidate.consumerContext
  /\ qc.phase = "Commit"
  /\ candidate.height = qc.context.height
  /\ candidate.view = qc.view
  /\ candidate.subject = qc.subject
  /\ candidate.kind
       \in {"DeliverQC", "BeginDecision", "PersistDecision"}
  /\ CASE candidate.kind \in {"DeliverQC", "BeginDecision"} ->
            candidate.class = "Progress"
       [] candidate.kind = "PersistDecision" ->
            candidate.class = "Completion"
       [] OTHER -> FALSE
  /\ \/ AsyncCommitImportDirectEvidence(candidate, qc)
     \/ AsyncCommitImportResponseEvidence(candidate, qc)
  /\ candidate.item =
       IF candidate.kind = "DeliverQC"
       THEN IF candidate.evidence.kind = "CommitQC"
            THEN candidate.evidence
            ELSE DiscoveredCommitQcItem(candidate.evidence)
       ELSE NoAsyncItem

AsyncCommitImportExecutionNeedsLineage(candidate) ==
  \/ /\ candidate.kind = "DeliverQC"
     /\ candidate.item.kind = "CommitQC"
  \/ candidate.kind = "BeginDecision"
  \/ /\ candidate.kind = "PersistDecision"
     /\ candidate.evidence \in AsyncNetworkItems
     /\ candidate.evidence.kind
          \in {"CommitQC", "CommitCertificateResponse"}

AsyncCommitImportExecutionProvenance(candidate) ==
  IF AsyncCommitImportExecutionNeedsLineage(candidate)
  THEN \E qc \in commitQCs:
         AsyncCommitImportCandidateLineage(candidate, qc)
  ELSE TRUE

THEOREM DirectCommitQcCandidateHasExactImportLineage ==
  \A item:
    /\ item \in asyncSentItems
    /\ item.kind = "CommitQC"
    /\ item.envelope.qc \in commitQCs
    /\ item.envelope.qc.context = context
    => AsyncCommitImportCandidateLineage(
         DeliveryCandidate(item), item.envelope.qc)
BY Isa
   DEF AsyncCommitImportCandidateLineage,
       AsyncCommitImportDirectEvidence,
       AsyncCommitImportResponseEvidence,
       DeliveryCandidate, AsyncDeliveryCandidateCausalOriginAt,
       DeliveryClass, DeliveryKind, DeliveryHeight, DeliveryView,
       DeliverySubject

THEOREM CommitCertificateResponseCandidateHasExactImportLineage ==
  \A item:
    /\ item \in asyncSentItems
    /\ CommitCertificateResponseAuthorized(item)
    => AsyncCommitImportCandidateLineage(
         CommitCertificateResponseCandidate(item), item.envelope.qc)
BY Isa
   DEF AsyncCommitImportCandidateLineage,
       AsyncCommitImportDirectEvidence,
       AsyncCommitImportResponseEvidence,
       CommitCertificateResponseAuthorized,
       CommitCertificateResponseCandidate,
       AsyncCommitCertificateResponseCandidateCausalOriginAt,
       DiscoveredCommitQcItem, HistoricalCommitQcSigner,
       CommitCertificateRequestAuthorized,
       DeliveryClass, DeliveryKind, DeliveryHeight, DeliveryView,
       DeliverySubject

THEOREM CommitImportCausalSuccessorRetainsExactLineage ==
  \A candidate, qc, successor:
    /\ AsyncCommitImportCandidateLineage(candidate, qc)
    /\ successor \in SequenceSet(CommandSuccessors(candidate))
    /\ successor.kind
         \in {"BeginDecision", "PersistDecision"}
    => AsyncCommitImportCandidateLineage(successor, qc)
BY Isa
   DEF AsyncCommitImportCandidateLineage,
       AsyncCommitImportDirectEvidence,
       AsyncCommitImportResponseEvidence,
       CommandSuccessors, CausalCandidate, AsyncCandidateFrom,
       SequenceSet, DiscoveredCommitQcItem

AsyncCertifiedResponseCandidateCausalOriginAt(item, consumerContext) ==
  AsyncCandidateCausalOrigin(
    "FetchCertifiedBody", item.envelope.recipient,
    item.envelope.height, item.envelope.view, item.envelope.subject,
    item, consumerContext, item, item.envelope.subject,
    item.envelope.subject, item.envelope.subject)

CertifiedResponseCandidate(item) ==
  LET node == item.envelope.recipient
      subject == item.envelope.subject
  IN AsyncCandidateWithIdentityAndOrigin(
       "Completion", "FetchCertifiedBody", node,
       item.envelope.height, item.envelope.view, subject, item,
       context, nodeView[node], generation[node], item,
       subject, subject, subject,
       AsyncCertifiedResponseCandidateCausalOriginAt(item, context))

(***************************************************************************
The production fair ingress rotates sources, but scans each selected source
from its oldest entry to its newest and removes the first entry whose exact
downstream predicate admits it.  Earlier blocked entries stay in place and
the source consumes only one round-robin turn.  Keeping item admission
separate from source selection prevents auxiliary I/O backpressure at a lane
head from hiding later consensus/body progress from the same peer.

One already-claimed certified response receives priority exactly when its
downstream predicate admits it. Ordinary completions stop one slot below the
physical runtime capacity, while only this authenticated response handoff may
use the final slot. The priority therefore cannot be defeated by another
completion source after a runtime service turn. A blocked claim receives no
priority and therefore cannot head-of-line block unrelated traffic.

If no claim is drainable, one pre-existing aggregate-untrusted shared
completion owner receives one-shot priority while this recipient has a live
exact request.  Production's first scan covers an unclaimed stale response
and its second scan covers a generic transport completion; the formal set is
their union.  The stale response drains through its failed claim check and the
generic completion is transport-local.  Fresh aggregate-untrusted generic
completions are policy rejected under that same local fence, so Byzantine
traffic cannot renew this priority class.

Response candidates require scheduler-wide freshness, including causal
ownership, so the exact downstream candidate cannot be admitted into a second
carrier. A fresh authenticated response completes an already-owned effect
fetch and reserves its Completion command directly, matching
`reserve_certified_body_available`; it does not create another I/O-work owner
or an invented local-producer phase. Physical runtime fullness remains
retryable backpressure, exactly as in production.

Authenticated body chunks do not enter the reducer FIFO.  Production routes a
`PayloadChunk` directly to the body/chunk transport, so an ingress chunk is
always locally drainable and records its receipt without a scheduler-capacity
or causal-admission gate.  Keeping that direct path explicit prevents an
artificial Progress-queue blocker from defeating an exact response handoff.
***************************************************************************)
IngressItemCanDrain(node, item) ==
  LET candidate == DeliveryCandidate(item)
  IN item.kind = "Noise"
       \/ ~IngressItemHasAuthenticatedHistory(item)
       \/ IF item.kind \in {"CertifiedRequest",
                             "CommitCertificateRequest"}
          THEN IF AsyncServeLifecycleDrainRequired(node, item)
               THEN ExactServeIngressCanAdvance(node, item)
               ELSE TRUE
          ELSE IF item.kind = "Chunk"
               THEN TRUE
               ELSE IF item.kind = "CertifiedResponse"
               THEN \/ ~CertifiedResponseClaimAuthorized(item)
                    \/ CandidateAdmissionCoalesced(
                         CertifiedResponseCandidate(item))
                    \/ /\ CanEnqueueCertifiedResponse(node)
                          /\ ~CandidateAdmissionCoalesced(
                               CertifiedResponseCandidate(item))
               ELSE IF item.kind = "CommitCertificateResponse"
                    THEN \/ ~CommitCertificateResponseAuthorized(item)
                         \/ CandidateAdmissionCoalesced(
                              CommitCertificateResponseCandidate(item))
                         \/ /\ ~NonCompletionCausalAdmissionDebt(node)
                               /\ CanEnqueueClass(node, "Progress")
                               /\ ~CandidateAdmissionCoalesced(
                                    CommitCertificateResponseCandidate(item))
               ELSE \/ AsyncControlServiceOccurrenceRetired(item)
                    \/ CandidateAdmissionCoalesced(candidate)
                    \/ /\ ~NonCompletionCausalAdmissionDebt(node)
                          /\ CanEnqueueClass(node, candidate.class)

AsyncServeIngressReservationIdentities(node) ==
  UNION {
    {AsyncServeLogicalRequestIdentity(
       node, asyncIngressLanes[node][source][index]):
       index \in
         {position \in 1..Len(asyncIngressLanes[node][source]):
            asyncIngressLanes[node][source][position].kind
              \in AsyncReplyRequestKinds}}:
    source \in AsyncIngressSources}

AsyncServeIngressLiveReservations(node) ==
  {reservation \in asyncServeReservations:
     /\ reservation.node = node
     /\ reservation.identity
          \in AsyncServeIngressReservationIdentities(node)
     /\ ~AsyncServeJobQueued(node, reservation.identity)}

AsyncServeIngressLiveReservationIdentities(node) ==
  {reservation.identity:
     reservation \in AsyncServeIngressLiveReservations(node)}

(***************************************************************************
Every serviceable exact request receives an immutable per-height ingress
ordinal at the atomic transport-to-lane cut.  Its high-watermark is durable
across same-height restart and is reconstructed with the retained lifecycle
record before replay; only a successor-height instance starts again at zero.
Exact duplicates coalesce behind that record while its occurrence remains
queued.  A cached replay receives an ingress ordinal without recreating its
retired Serve lifecycle; a later retry after the occurrence drains receives a
later ingress turn.  The selector
therefore orders the single live off-queue reservation, cached replays, and
already-owned exact requests across all physical source lanes without letting
later work leapfrog an admitted target.
***************************************************************************)
AsyncServeIngressIdentityFrozenByReservation(
    node, reservation, identity) ==
  \E source \in AsyncIngressSources:
    \E index \in 1..reservation.ingressPredecessors[source]:
      /\ asyncIngressLanes[node][source][index].kind
           \in AsyncReplyRequestKinds
      /\ AsyncServeLogicalRequestIdentity(
           node, asyncIngressLanes[node][source][index]) = identity

(***************************************************************************
Every smaller ingress ordinal is a finite predecessor occurrence, including a
cached tombstone replay or retry whose Serve job is already queued.  Draining
one of those owners can rotate the physical ready-source ring, so exact rank
accounting must charge the immutable ordinal even when there is no off-queue
Serve barrier.  A retry after drain receives a larger ordinal and cannot
replenish this set.
***************************************************************************)
AsyncServePreexistingIngressOwnerIdentities(node, identity) ==
  IF AsyncServeIngressAdmissionOwned(node, identity)
  THEN {admission.identity:
          admission \in
            {owned \in asyncServeIngressAdmissions:
               /\ owned.node = node
               /\ owned.identity # identity
               /\ owned.ordinal
                    < AsyncServeIngressAdmissionOrdinal(node, identity)}}
  ELSE {}

AsyncServePreexistingIngressOwnerPredecessorDebtSet(node, identity) ==
  ({"ServeIngressOwner"} \X
     AsyncServePreexistingIngressOwnerIdentities(node, identity))
    \cup
  ({"ServeIngressOwnerPrefix"} \X
     UNION {
       {[identity |-> ownerIdentity,
         source |-> slot.source,
         slot |-> slot.slot]:
          slot \in
            AsyncServeIngressAdmissionPredecessorDebtSlots(
              node, ownerIdentity)}:
       ownerIdentity \in
         AsyncServePreexistingIngressOwnerIdentities(node, identity)})

(***************************************************************************
The singular Rust Serve barrier is the subset of those earlier ingress owners
which still owns an unmaterialized Serve slot.  Its admission snapshot cannot
contain the later target occurrence.  This derived identity is internal
scheduler metadata; it is neither a wire field nor a new request identity.
***************************************************************************)
AsyncServePreexistingIngressBarrierIdentities(node, identity) ==
  IF AsyncServeIngressAdmissionOwned(node, identity)
  THEN {reservation.identity:
          reservation \in
            {owned \in AsyncServeIngressLiveReservations(node):
               /\ owned.identity
                    \in AsyncServePreexistingIngressOwnerIdentities(
                         node, identity)
               /\ ~AsyncServeIngressIdentityFrozenByReservation(
                    node, owned, identity)}}
  ELSE {}

(***************************************************************************
The singular preexisting barrier contributes only its frozen physical I/O
jobs here; the generic earlier-owner set already charges its immutable ingress
prefix and occurrence.  These jobs account for the full-to-resumable
I/O-worker handoff and remain disjoint from the target lifecycle's own tags.
***************************************************************************)
AsyncServePreexistingIngressBarrierPredecessorDebtSet(node, identity) ==
  UNION {
    ({"ServeBarrierIo"} \X
       AsyncServeFrozenPredecessorSet(node, barrierIdentity)):
    barrierIdentity \in
      AsyncServePreexistingIngressBarrierIdentities(node, identity)}

(***************************************************************************
The hidden-ingress acceptance cut freezes one prefix length per physical
source lane for every exact ingress occurrence, including a cached replay.
Removing an occurrence inside a frozen prefix decrements that prefix;
appending later traffic never increases it.  The earliest ingress owner admits
one of those pre-cutoff entries or its exact request.  In particular a later
claimed response, fenced completion, Control producer, or exact request cannot
acquire a selector position ahead of the earliest ingress admission.
***************************************************************************)
AsyncServeIngressIndexMayPrecedeAdmittedTarget(node, source, index) ==
  IF AsyncServeIngressLifecycleOwnerIdentities(node) = {}
  THEN TRUE
  ELSE LET ownerIdentity ==
             AsyncServeEarliestIngressLifecycleOwnerIdentity(node)
           item == asyncIngressLanes[node][source][index]
       IN \/ index <=
                AsyncServeIngressAdmissionPredecessorCounts(
                  node, ownerIdentity)[source]
          \/ /\ item.kind \in AsyncReplyRequestKinds
             /\ AsyncServeLogicalRequestIdentity(node, item)
                  = ownerIdentity

DrainableIngressLaneIndices(node, source) ==
  {index \in 1..Len(IngressLane(node, source)):
     /\ AsyncServeIngressIndexMayPrecedeAdmittedTarget(
          node, source, index)
     /\ IngressItemCanDrain(node, IngressLane(node, source)[index])}

DrainableClaimedResponseLaneIndices(node, source) ==
  {index \in DrainableIngressLaneIndices(node, source):
     LET item == IngressLane(node, source)[index]
     IN item.kind = "CertifiedResponse"
          /\ CertifiedResponseClaimMatches(item)}

DrainableRequestFencedCompletionLaneIndices(node, source) ==
  {index \in DrainableIngressLaneIndices(node, source):
     LET item == IngressLane(node, source)[index]
     IN /\ source = AsyncUntrustedSource
        /\ IngressUsesPhysicalCompletionOwner(item)
        /\ ActiveCertifiedRequestHashesAt(node) # {}}

FirstDrainableIngressLaneIndex(node, source) ==
  LET claimed == DrainableClaimedResponseLaneIndices(node, source)
      fenced ==
        DrainableRequestFencedCompletionLaneIndices(node, source)
      drainable == DrainableIngressLaneIndices(node, source)
  IN IF claimed # {}
     THEN CHOOSE index \in claimed: TRUE
     ELSE IF fenced # {}
          THEN CHOOSE index \in fenced: TRUE
          ELSE CHOOSE index \in drainable:
                 \A other \in drainable: index <= other

IngressSourceCanDrain(node, source) ==
  DrainableIngressLaneIndices(node, source) # {}

SelectedIngressLaneIndex(node, index) ==
  FirstDrainableIngressLaneIndex(node, asyncIngressReady[node][index])

SelectedIngressItemAt(node, index) ==
  LET source == asyncIngressReady[node][index]
  IN IngressLane(node, source)[SelectedIngressLaneIndex(node, index)]

IngressHeadCanDrain(node) ==
  IngressItemCanDrain(node, HeadIngressItem(node))

DrainableIngressIndices(node) ==
  {index \in 1..Len(asyncIngressReady[node]):
     IngressSourceCanDrain(node, asyncIngressReady[node][index])}

DrainableClaimedResponseReadyIndices(node) ==
  {index \in DrainableIngressIndices(node):
     DrainableClaimedResponseLaneIndices(
       node, asyncIngressReady[node][index]) # {}}

DrainableRequestFencedCompletionReadyIndices(node) ==
  {index \in DrainableIngressIndices(node):
     DrainableRequestFencedCompletionLaneIndices(
       node, asyncIngressReady[node][index]) # {}}

FirstDrainableIngressIndex(node) ==
  LET claimed == DrainableClaimedResponseReadyIndices(node)
      fenced == DrainableRequestFencedCompletionReadyIndices(node)
      drainable == DrainableIngressIndices(node)
  IN IF claimed # {}
     THEN CHOOSE index \in claimed: TRUE
     ELSE IF fenced # {}
          THEN CHOOSE index \in fenced: TRUE
          ELSE CHOOSE index \in drainable:
                 \A other \in drainable: index <= other

ReadyAfterSelectedDrain(node, index) ==
  LET ready == asyncIngressReady[node]
      source == ready[index]
      lane == IngressLane(node, source)
      rotatedTail ==
        SubSeq(ready, index + 1, Len(ready))
          \o SubSeq(ready, 1, index - 1)
  IN IF Len(lane) = 1
     THEN rotatedTail
     ELSE Append(rotatedTail, source)

PopSelectedIngress(node, index, laneIndex) ==
  LET source == asyncIngressReady[node][index]
  IN /\ index \in 1..Len(asyncIngressReady[node])
     /\ laneIndex \in 1..Len(IngressLane(node, source))
     /\ asyncIngressLanes' =
          [asyncIngressLanes EXCEPT
             ![node][source] = SequenceWithoutIndex(@, laneIndex)]
     /\ asyncIngressReady' =
          [asyncIngressReady EXCEPT
             ![node] = ReadyAfterSelectedDrain(node, index)]
     /\ asyncServeReservations' =
          AsyncServeReservationsAfterIngressDrain(
            node, source, laneIndex)
     /\ asyncServeIngressAdmissions' =
          AsyncServeIngressAdmissionsAfterIngressDrain(
            node, source, laneIndex)
     /\ UNCHANGED asyncNextServeIngressOrdinal

(***************************************************************************
The serialized Rust ingress authenticates every reducer-directed envelope
before comparing it with scheduler-owned authenticated envelopes.  An exact
retransmission is consumed from transport without taking a second runtime
slot.  A certified-response claim survives the implementation's physical
dequeue/backpressure/restore cycle; this abstraction represents that cycle as
stutter and pops the response only at successful downstream handoff, where the
request authority and claim retire atomically.
***************************************************************************)
DrainFairIngressSelected(node) ==
  LET index == FirstDrainableIngressIndex(node)
      source == asyncIngressReady[node][index]
      laneIndex == SelectedIngressLaneIndex(node, index)
      item == SelectedIngressItemAt(node, index)
      candidate == DeliveryCandidate(item)
  IN /\ asyncIngressReady[node] # <<>>
     /\ DrainableIngressIndices(node) # {}
     /\ PopSelectedIngress(node, index, laneIndex)
     /\ IF /\ item.kind = "CommitCertificateResponse"
              /\ item \in asyncSentItems
              /\ CommitCertificateResponseAuthorized(item)
              /\ DiscoveredCommitQcItem(item).envelope \notin qcNetwork
        THEN ImportAuthenticatedCommitCertificate(
               DiscoveredCommitQcItem(item).envelope)
        ELSE UNCHANGED vars
     /\ IF item.kind = "Noise" \/ ~IngressItemHasAuthenticatedHistory(item)
        THEN /\ UNCHANGED <<asyncCommandQueues,
                            asyncNextCommandClass>>
             /\ UNCHANGED AsyncIoExceptServeReservationsVars
             /\ UNCHANGED <<asyncSentItems, asyncRetainedControl,
                            asyncActiveRequests,
                            asyncCertifiedResponseClaim>>
        ELSE IF item.kind \in {"CertifiedRequest",
                                "CommitCertificateRequest"}
             THEN IF AsyncServeLifecycleDrainRequired(node, item)
                  THEN /\ AcceptOrCoalesceExactServeRequest(node, candidate)
                       /\ UNCHANGED <<asyncOutstandingWork,
                                       asyncIoReadyCompletions,
                                       asyncLocalReadyCompletions,
                                       asyncNextCompletionSource,
                                       asyncIoControlAvailable>>
                       /\ UNCHANGED <<asyncCommandQueues,
                                      asyncNextCommandClass, asyncSentItems,
                                      asyncRetainedControl,
                                      asyncActiveRequests,
                                      asyncCertifiedResponseClaim>>
                  ELSE /\ UNCHANGED <<asyncCommandQueues,
                                      asyncNextCommandClass,
                                      AsyncIoExceptServeReservationsVars>>
                       /\ UNCHANGED <<asyncSentItems,
                                      asyncRetainedControl,
                                      asyncActiveRequests,
                                      asyncCertifiedResponseClaim>>
             ELSE IF item.kind = "Chunk"
                  THEN /\ UNCHANGED <<asyncCommandQueues,
                                       asyncNextCommandClass,
                                       AsyncIoExceptServeReservationsVars>>
                       /\ UNCHANGED <<asyncSentItems,
                                      asyncRetainedControl,
                                      asyncActiveRequests,
                                      asyncCertifiedResponseClaim>>
             ELSE IF item.kind = "CertifiedResponse"
                  THEN IF CertifiedResponseClaimAuthorized(item)
                       THEN LET completion ==
                                  CertifiedResponseCandidate(item)
                            IN /\ IF CandidateAdmissionCoalesced(completion)
                                  THEN UNCHANGED <<
                                                    AsyncIoExceptServeReservationsVars,
                                                    asyncCommandQueues,
                                                    asyncNextCommandClass>>
                                  ELSE /\ EnqueueCandidate(completion)
                                       /\ UNCHANGED
                                            AsyncIoExceptServeReservationsVars
                               /\ asyncActiveRequests' =
                                    asyncActiveRequests \
                                      MatchingCertifiedRequests(item)
                               /\ asyncCertifiedResponseClaim' =
                                    CertifiedResponseClaimForRequests(
                                      asyncActiveRequests')
                               /\ UNCHANGED <<asyncSentItems,
                                              asyncRetainedControl>>
                       ELSE /\ UNCHANGED <<asyncCommandQueues,
                                           asyncNextCommandClass,
                                           AsyncIoExceptServeReservationsVars>>
                            /\ UNCHANGED <<asyncSentItems,
                                           asyncRetainedControl,
                                           asyncActiveRequests,
                                           asyncCertifiedResponseClaim>>
                  ELSE IF item.kind = "CommitCertificateResponse"
                       THEN IF CommitCertificateResponseAuthorized(item)
                            THEN LET discovered ==
                                       DiscoveredCommitQcItem(item)
                                     discoveredCandidate ==
                                       CommitCertificateResponseCandidate(item)
                                 IN /\ IF CandidateAdmissionCoalesced(
                                               discoveredCandidate)
                                        THEN UNCHANGED <<asyncCommandQueues,
                                                          asyncNextCommandClass>>
                                        ELSE EnqueueCandidate(
                                               discoveredCandidate)
                                    /\ UNCHANGED
                                         AsyncIoExceptServeReservationsVars
                                    /\ asyncActiveRequests' =
                                         asyncActiveRequests \
                                           MatchingCommitCertificateRequests(item)
                                    /\ UNCHANGED asyncCertifiedResponseClaim
                                    /\ asyncSentItems' =
                                         asyncSentItems \cup {discovered}
                                    /\ UNCHANGED asyncRetainedControl
                            ELSE /\ UNCHANGED <<asyncCommandQueues,
                                                asyncNextCommandClass,
                                                AsyncIoExceptServeReservationsVars>>
                                 /\ UNCHANGED <<asyncSentItems,
                                                asyncRetainedControl,
                                                asyncActiveRequests,
                                                asyncCertifiedResponseClaim>>
                  ELSE /\ IF AsyncControlServiceOccurrenceRetired(item)
                          THEN UNCHANGED <<asyncCommandQueues,
                                           asyncNextCommandClass>>
                          ELSE IF CandidateAdmissionCoalesced(candidate)
                               THEN UNCHANGED <<asyncCommandQueues,
                                                asyncNextCommandClass>>
                               ELSE EnqueueCandidate(candidate)
                       /\ UNCHANGED AsyncIoExceptServeReservationsVars
                       /\ UNCHANGED <<asyncSentItems,
                                      asyncRetainedControl,
                                      asyncActiveRequests,
                                      asyncCertifiedResponseClaim>>
     /\ asyncHeldChunks' =
          IF /\ item.kind = "Chunk"
             /\ IngressItemHasAuthenticatedHistory(item)
             /\ item.envelope.chunk \in AsyncChunks
          THEN asyncHeldChunks \cup
                 {AsyncChunkReceipt(node, item.envelope.view,
                                    item.envelope.subject,
                                    item.envelope.chunk)}
          ELSE asyncHeldChunks
     /\ asyncTransport' =
          asyncTransport
            \cup PacketsForItems(AsyncServeCachedReplayItems(node, item))
     /\ UNCHANGED <<asyncFifoOwed, asyncTimeoutEmitted,
                    asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines,
                    asyncHistoricalRecoveryTargets
                    >>

(***************************************************************************
Bounded control-slot mutation seams.

The active-pass seam exposes the first immutable slot owner.  The retired-drop
seam exposes both a consumed exact retry and a delayed identity displaced by a
strictly newer view.  Theorems pinning these conditions appear after the
complete slot transition and `AsyncNext`, so they cannot accidentally appeal
to a downstream strong invariant.
***************************************************************************)
AsyncSelectedFairIngressItem(node) ==
  LET index == FirstDrainableIngressIndex(node)
      source == asyncIngressReady[node][index]
      laneIndex == SelectedIngressLaneIndex(node, index)
  IN asyncIngressLanes[node][source][laneIndex]

AsyncActiveControlServiceAdmissionPassCondition(node) ==
  LET item == AsyncSelectedFairIngressItem(node)
      candidate == DeliveryCandidate(item)
  IN /\ asyncIngressReady[node] # <<>>
     /\ DrainableIngressIndices(node) # {}
     /\ item.kind \in AsyncControlKinds
     /\ AsyncControlServiceOccurrenceIsCurrentOwner(item)
     /\ ~CandidateAdmissionCoalesced(candidate)
     /\ ~NonCompletionCausalAdmissionDebt(node)
     /\ CanEnqueueClass(node, candidate.class)

AsyncRetiredControlServiceAdmissionDropCondition(node) ==
  LET item == AsyncSelectedFairIngressItem(node)
  IN /\ asyncIngressReady[node] # <<>>
     /\ DrainableIngressIndices(node) # {}
     /\ item.kind \in AsyncControlKinds
     /\ AsyncControlServiceOccurrenceRetired(item)
     /\ ~CandidateAdmissionCoalesced(DeliveryCandidate(item))

(***************************************************************************
After Apply, the production height loop exits immediately.  Its successor
loop still drains the shared ingress and serves immutable Kura finality/body
artifacts, but it must not execute or retransmit old-height consensus work.
The historical runner below therefore rejects every old-height entry except
the two authenticated recovery request classes, which enter the same bounded
Serve reservation as the live-height implementation.
***************************************************************************)

HistoricalIngressItemCanDrain(node, item) ==
  IF item.kind = "CertifiedRequest"
     THEN \/ ~AsyncServeLifecycleDrainRequired(node, item)
          \/ ExactServeIngressCanAdvance(node, item)
     ELSE IF item.kind = "CommitCertificateRequest"
          THEN \/ ~AsyncServeLifecycleDrainRequired(node, item)
               \/ ExactServeIngressCanAdvance(node, item)
          ELSE TRUE

HistoricalDrainableIngressLaneIndices(node, source) ==
  {index \in 1..Len(IngressLane(node, source)):
     /\ AsyncServeIngressIndexMayPrecedeAdmittedTarget(
          node, source, index)
     /\ HistoricalIngressItemCanDrain(
          node, IngressLane(node, source)[index])}

FirstHistoricalDrainableIngressLaneIndex(node, source) ==
  CHOOSE index \in HistoricalDrainableIngressLaneIndices(node, source):
    \A other \in HistoricalDrainableIngressLaneIndices(node, source):
      index <= other

HistoricalIngressSourceCanDrain(node, source) ==
  HistoricalDrainableIngressLaneIndices(node, source) # {}

HistoricalSelectedIngressLaneIndex(node, index) ==
  FirstHistoricalDrainableIngressLaneIndex(
    node, asyncIngressReady[node][index])

HistoricalSelectedIngressItemAt(node, index) ==
  LET source == asyncIngressReady[node][index]
  IN IngressLane(node, source)[
       HistoricalSelectedIngressLaneIndex(node, index)]

HistoricalDrainableIngressIndices(node) ==
  {index \in 1..Len(asyncIngressReady[node]):
     HistoricalIngressSourceCanDrain(
       node, asyncIngressReady[node][index])}

FirstHistoricalDrainableIngressIndex(node) ==
  CHOOSE index \in HistoricalDrainableIngressIndices(node):
    \A other \in HistoricalDrainableIngressIndices(node): index <= other

DrainHistoricalIngressSelected(node) ==
  LET index == FirstHistoricalDrainableIngressIndex(node)
      laneIndex == HistoricalSelectedIngressLaneIndex(node, index)
      item == HistoricalSelectedIngressItemAt(node, index)
      candidate == DeliveryCandidate(item)
      authorizedRequest ==
        AsyncServeLifecycleDrainRequired(node, item)
  IN /\ HistoricalDrainableIngressIndices(node) # {}
     /\ PopSelectedIngress(node, index, laneIndex)
     /\ IF authorizedRequest
        THEN /\ AcceptOrCoalesceExactServeRequest(node, candidate)
             /\ UNCHANGED <<asyncOutstandingWork,
                             asyncIoReadyCompletions,
                             asyncLocalReadyCompletions,
                             asyncNextCompletionSource,
                             asyncIoControlAvailable>>
        ELSE UNCHANGED AsyncIoExceptServeReservationsVars
     /\ asyncCertifiedResponseClaim' =
          IF item.kind = "CertifiedResponse"
               /\ CertifiedResponseClaimMatches(item)
          THEN asyncCertifiedResponseClaim \
                 {AsyncCertifiedResponseCanonicalWireIdentity(item)}
          ELSE asyncCertifiedResponseClaim
     /\ asyncTransport' =
          asyncTransport
            \cup PacketsForItems(AsyncServeCachedReplayItems(node, item))
     /\ UNCHANGED <<vars, asyncCommandQueues, asyncNextCommandClass,
                    asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncDeferredVars,
                    asyncCausalQueues, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncSentItems, asyncRetainedControl,
                    asyncActiveRequests,
                    asyncHeldChunks,
                    asyncHistoricalRecoveryTargets>>

AdmitCausalHead(node) ==
  LET candidate == HeadCausalCandidate(node)
      duplicate == CandidateInFlight(candidate)
  IN /\ CausalHeadCanAdvance(node)
     /\ asyncCausalQueues' =
          [asyncCausalQueues EXCEPT
             ![node] = SequenceWithoutIndex(
               @, NextCausalCandidateIndex(node))]
     /\ IF duplicate
        THEN /\ UNCHANGED <<asyncCommandQueues,
                            asyncNextCommandClass>>
             /\ UNCHANGED <<asyncIoQueues, asyncOutstandingWork,
                             asyncIoReadyCompletions,
                             asyncLocalReadyCompletions,
                             asyncNextCompletionSource,
                             asyncIoControlAvailable,
                             AsyncServeLifecycleVars,
                             AsyncServeIngressAdmissionVars>>
        ELSE IF candidate.class = "Completion"
             THEN /\ asyncIoQueues' =
                       [asyncIoQueues EXCEPT
                          ![node] = Append(@, AsyncIoConsensusJob(candidate))]
                  /\ asyncOutstandingWork' =
                       [asyncOutstandingWork EXCEPT
                          ![node] = @ \cup {candidate}]
                  /\ UNCHANGED <<asyncCommandQueues,
                                  asyncNextCommandClass,
                                  asyncIoReadyCompletions,
                                  asyncLocalReadyCompletions,
                                  asyncNextCompletionSource,
                                  asyncIoControlAvailable,
                                  AsyncServeLifecycleVars,
                                  AsyncServeIngressAdmissionVars>>
             ELSE /\ EnqueueCandidate(candidate)
                  /\ UNCHANGED AsyncIoVars
     /\ UNCHANGED <<vars, asyncFifoOwed, asyncTimeoutEmitted,
                    asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines, asyncSentItems,
                    asyncRetainedControl, asyncActiveRequests,
                    asyncCertifiedResponseClaim, asyncTransport,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets>>

CompletionCandidateQueue(node, source) ==
  IF source = "Io"
  THEN asyncIoReadyCompletions[node]
  ELSE asyncLocalReadyCompletions[node]

CompletionSourceQueueNonempty(node, source) ==
  Len(CompletionCandidateQueue(node, source)) > 0

CompletionSourceLifecycleOrdinals(node, source) ==
  {AsyncCandidateLifecycleOrdinal(
     CompletionCandidateQueue(node, source)[index]):
     index \in 1..Len(CompletionCandidateQueue(node, source))}

OldestCompletionSourceLifecycleOrdinal(node, source) ==
  CHOOSE ordinal \in CompletionSourceLifecycleOrdinals(node, source):
    \A other \in CompletionSourceLifecycleOrdinals(node, source):
      ordinal <= other

OldestCompletionSourceIndices(node, source) ==
  {index \in 1..Len(CompletionCandidateQueue(node, source)):
     AsyncCandidateLifecycleOrdinal(
       CompletionCandidateQueue(node, source)[index])
       = OldestCompletionSourceLifecycleOrdinal(node, source)}

OldestCompletionSourceIndex(node, source) ==
  CHOOSE index \in OldestCompletionSourceIndices(node, source):
    \A other \in OldestCompletionSourceIndices(node, source):
      index <= other

OldestCompletionSourceCandidate(node, source) ==
  CompletionCandidateQueue(node, source)[
    OldestCompletionSourceIndex(node, source)]

SelectedCompletionSource(node) ==
  LET preferred == asyncNextCompletionSource[node]
      other == IF preferred = "Io" THEN "Local" ELSE "Io"
  IN IF CompletionSourceQueueNonempty(node, preferred)
          /\ CompletionSourceQueueNonempty(node, other)
     THEN IF OldestCompletionSourceLifecycleOrdinal(node, preferred)
               <= OldestCompletionSourceLifecycleOrdinal(node, other)
          THEN preferred
          ELSE other
     ELSE IF CompletionSourceQueueNonempty(node, preferred)
          THEN preferred
          ELSE other

SelectedCompletionQueueNonempty(node) ==
  IF SelectedCompletionSource(node) = "Io"
  THEN Len(asyncIoReadyCompletions[node]) > 0
  ELSE Len(asyncLocalReadyCompletions[node]) > 0

SelectedCompletionCandidate(node) ==
  OldestCompletionSourceCandidate(node, SelectedCompletionSource(node))

ProducerCompletionCanAdmit(node) ==
  /\ SelectedCompletionQueueNonempty(node)
  /\ CanEnqueueClass(node, "Completion")

ProducerCompletionCanAdvance(node) ==
  /\ ProducerCompletionCanAdmit(node)
  /\ ~NonCompletionCausalAdmissionDebt(node)

OtherLocalSource(source) ==
  IF source = "Producer" THEN "Causal" ELSE "Producer"

LocalSourceCanAdmit(node, source) ==
  IF source = "Producer"
  THEN ProducerCompletionCanAdvance(node)
  ELSE CausalHeadCanAdvance(node)

(***************************************************************************
The local admission cursor remains the tie breaker between producer and causal
work with the same lifecycle ordinal.  When both sources can advance, the
older immutable lifecycle wins.  The first producer or no-admission turn that
observes causal work still records sticky debt.  Non-Completion debt reserves
command capacity, while Completion debt permits the exact producer retirement
needed to free an outstanding-work slot.

An authenticated certified-response retry does not fence local admission.
Ordinary producer completions cannot consume its dedicated final runtime slot,
so the existing finite local-turn budget can drain normally before the runner
returns to prioritized ingress.
***************************************************************************)
PreferredLocalSource(node) ==
  IF asyncCausalAdmissionOwed[node] = TRUE
  THEN "Causal"
  ELSE IF asyncNextLocalSource[node] = "Causal"
       THEN "Causal"
       ELSE "Producer"

LocalSourceLifecycleOrdinal(node, source) ==
  IF source = "Producer"
  THEN AsyncCandidateLifecycleOrdinal(
         SelectedCompletionCandidate(node))
  ELSE AsyncCandidateLifecycleOrdinal(HeadCausalCandidate(node))

SelectedLocalSource(node) ==
  LET preferred == PreferredLocalSource(node)
      other == OtherLocalSource(preferred)
  IN IF LocalSourceCanAdmit(node, preferred)
          /\ LocalSourceCanAdmit(node, other)
     THEN IF LocalSourceLifecycleOrdinal(node, preferred)
               <= LocalSourceLifecycleOrdinal(node, other)
          THEN preferred
          ELSE other
     ELSE IF LocalSourceCanAdmit(node, preferred)
          THEN preferred
          ELSE other

LocalAdmissionCanAdvance(node) ==
  /\ asyncRunnerBudget[node] > 0
  /\ (ProducerCompletionCanAdvance(node) \/ CausalHeadCanAdvance(node))

(***************************************************************************
A live Serve ticket may not erase Local work whose immutable lifecycle
ordinal was allocated first.  The comparison is against the selected
admissible Local owner, so it never manufactures an admission for blocked
work and it cannot be satisfied by work allocated after the ticket.
***************************************************************************)
AsyncOlderLocalLifecyclePrecedesServeIngress(node) ==
  /\ AsyncServeIngressLifecycleOwnerIdentities(node) # {}
  /\ LocalAdmissionCanAdvance(node)
  /\ LocalSourceLifecycleOrdinal(node, SelectedLocalSource(node))
       < AsyncServeEarliestIngressSchedulerOrdinal(node)

UpdateLocalAdmissionMetadata(node, source) ==
  /\ asyncNextLocalSource' =
       [asyncNextLocalSource EXCEPT
          ![node] = OtherLocalSource(source)]
  /\ asyncCausalAdmissionOwed' =
       [asyncCausalAdmissionOwed EXCEPT
          ![node] =
            IF source = "Causal"
            THEN FALSE
            ELSE ((@ = TRUE) \/ CausalQueueNonempty(node))]

RecordBlockedCausalDebt(node) ==
  /\ asyncCausalAdmissionOwed' =
       [asyncCausalAdmissionOwed EXCEPT
          ![node] = ((@ = TRUE) \/ CausalQueueNonempty(node))]
  /\ UNCHANGED asyncNextLocalSource

AdmitProducerCompletion(node) ==
  LET source == SelectedCompletionSource(node)
      candidate == SelectedCompletionCandidate(node)
  IN /\ ProducerCompletionCanAdvance(node)
     /\ EnqueueCandidate(candidate)
     /\ asyncIoReadyCompletions' =
          IF source = "Io"
          THEN [asyncIoReadyCompletions EXCEPT
                  ![node] = SequenceWithoutIndex(
                    @, OldestCompletionSourceIndex(node, source))]
          ELSE asyncIoReadyCompletions
     /\ asyncLocalReadyCompletions' =
          IF source = "Local"
          THEN [asyncLocalReadyCompletions EXCEPT
                  ![node] = SequenceWithoutIndex(
                    @, OldestCompletionSourceIndex(node, source))]
          ELSE asyncLocalReadyCompletions
     /\ asyncNextCompletionSource' =
          [asyncNextCompletionSource EXCEPT
             ![node] = IF source = "Io" THEN "Local" ELSE "Io"]
     /\ asyncOutstandingWork' =
          [asyncOutstandingWork EXCEPT ![node] = @ \ {candidate}]
     /\ UNCHANGED <<asyncIoQueues, asyncIoControlAvailable,
                     AsyncServeLifecycleVars,
                     AsyncServeIngressAdmissionVars>>
     /\ UNCHANGED <<vars, asyncFifoOwed, asyncTimeoutEmitted,
                    asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines, asyncSentItems,
                    asyncRetainedControl, asyncActiveRequests,
                    asyncCertifiedResponseClaim, asyncTransport,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets>>

(***************************************************************************
The successful Local-admission body is exposed independently of the ordinary
ticket-free runner action.  This gives the exact-Serve corridor one atomic
way to retire a strictly older Local owner while leaving the ticket and its
logical lifecycle records frozen.  It is an action-shape helper only; fairness
continues to apply to the enclosing PostGstRunNode action.
***************************************************************************)
SelectedLocalAdmissionAdvance(node) ==
  /\ asyncRunnerPhase[node] = "Local"
  /\ LocalAdmissionCanAdvance(node)
  /\ UNCHANGED AsyncDeferredVars
  /\ LET source == SelectedLocalSource(node)
     IN /\ IF source = "Producer"
               THEN /\ AdmitProducerCompletion(node)
                    /\ LeaveCausalQueues
               ELSE AdmitCausalHead(node)
        /\ UpdateLocalAdmissionMetadata(node, source)
  /\ asyncRunnerPhase' = asyncRunnerPhase
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = @ - 1]

ServiceIoWorkerWork(node) ==
  LET job == Head(asyncIoQueues[node])
      responseItems ==
        IF job.class # "Serve"
        THEN {}
        ELSE IF CertifiedServeCanRespond(node, job.candidate.item)
             THEN {CertifiedResponseItem(
                     AsyncUntrustedSource, node, job.candidate.item)}
             ELSE IF CommitCertificateServeCanRespond(job.candidate.item)
                  THEN CommitCertificateResponseItems(job.candidate.item)
                  ELSE {}
      serveIdentity ==
        IF job.class = "Serve"
        THEN AsyncIoServeJobIdentity(node, job)
        ELSE NoAsyncItem
      completedIdentity ==
        IF job.class = "Serve" /\ responseItems # {}
        THEN serveIdentity
        ELSE NoAsyncItem
  IN /\ node \in AsyncActiveServiceNodes
     /\ node \in up
     /\ ResponsiveReplayExecutorAllowed(node)
     /\ AsyncIoQueueDepth(node) > 0
     \* A reserved Serve is admitted only for a durable retention owner.
     \* Missing output is therefore a broken local retention contract and
     \* fail-stops this worker action instead of recreating the request.
     /\ (job.class = "Serve" => responseItems # {})
     /\ asyncIoQueues' =
          [asyncIoQueues EXCEPT ![node] = Tail(@)]
     /\ asyncServeReservations' =
          AsyncServeReservationsAfterIoService(
            node, job, completedIdentity)
     /\ asyncServeAdmissions' =
          IF completedIdentity # NoAsyncItem
          THEN AsyncServeAdmissionsWithout(
                 node, completedIdentity)
          ELSE asyncServeAdmissions
     /\ asyncServeTombstones' =
          IF completedIdentity # NoAsyncItem
          THEN LET reservation ==
                     AsyncServeReservationRecord(
                       node, completedIdentity)
               IN AsyncServeTombstonesWithoutFamily(
                    node, reservation.family)
                    \cup {AsyncServeTombstone(
                            node, completedIdentity,
                            reservation.family, reservation.view,
                            reservation.ordinal, responseItems)}
          ELSE asyncServeTombstones
     /\ UNCHANGED asyncNextServeAdmissionOrdinal
     /\ UNCHANGED AsyncServeIngressAdmissionVars
     /\ asyncIoReadyCompletions' =
          IF job.class = "Consensus"
          THEN [asyncIoReadyCompletions EXCEPT
                  ![node] = Append(@, job.candidate)]
          ELSE asyncIoReadyCompletions
     /\ UNCHANGED <<asyncLocalReadyCompletions,
                     asyncNextCompletionSource>>
     /\ asyncIoControlAvailable' =
          IF job.class = "Control"
          THEN [asyncIoControlAvailable EXCEPT ![node] = TRUE]
          ELSE asyncIoControlAvailable
     /\ PublishEphemeralItems(responseItems)
     /\ UNCHANGED asyncOutstandingWork
     /\ asyncIoServiceDeadlines' =
          [asyncIoServiceDeadlines EXCEPT
             ![node] = asyncNow + AsyncDeliveryBound]
     /\ UNCHANGED asyncNodeServiceDeadlines
     /\ UNCHANGED AsyncDeferredVars
     /\ LeaveCausalQueues
     /\ UNCHANGED AsyncLocalAdmissionVars
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget,
                    asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets>>

ServiceIoWorker(node) ==
  /\ node \in AsyncArchiveIoServiceNodes
  /\ ServiceIoWorkerWork(node)

ServiceHistoricalRecoveryIoWorker(node) ==
  /\ HistoricalRecoveryTarget(node)
  /\ ServiceIoWorkerWork(node)

EnqueueIoLocalControlWork(node) ==
  /\ node \in AsyncActiveServiceNodes
  /\ node \in up
  /\ ~ResponsiveReplayQuarantined(node)
  /\ ~NodeHasApplication(node)
  \* An admitted exact ingress occurrence owns the next post-prefix service
  \* corridor.  Preexisting Control jobs still drain through the I/O worker,
  \* but a later poll cannot recreate Control capacity debt ahead of it.
  /\ AsyncServeIngressLifecycleOwnerIdentities(node) = {}
  /\ asyncIoControlAvailable[node]
  /\ ~CompletionCausalAdmissionDebt(node)
  /\ CanEnqueueIoClass(node, "Control")
  /\ asyncIoQueues' =
       [asyncIoQueues EXCEPT ![node] = Append(@, AsyncIoControlJob)]
  /\ asyncIoControlAvailable' =
       [asyncIoControlAvailable EXCEPT ![node] = FALSE]
  /\ UNCHANGED <<AsyncServeLifecycleVars,
                  AsyncServeIngressAdmissionVars>>
  /\ UNCHANGED AsyncDeferredVars
  /\ LeaveCausalQueues
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                 asyncNextCommandClass, asyncFifoOwed,
                 asyncTimeoutEmitted, asyncRunnerPhase, asyncRunnerBudget,
                 asyncOutstandingWork, asyncIoReadyCompletions,
                 asyncLocalReadyCompletions, asyncNextCompletionSource,
                 asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncNodeServiceDeadlines,
                 asyncIoServiceDeadlines, asyncSentItems,
                 asyncRetainedControl, asyncActiveRequests,
                 asyncCertifiedResponseClaim, asyncTransport,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

EnqueueIoLocalControl(node) ==
  /\ node \in AsyncCurrentResponsiveVoters
  /\ EnqueueIoLocalControlWork(node)

EnqueueHistoricalRecoveryIoLocalControl(node) ==
  /\ HistoricalRecoveryTarget(node)
  /\ EnqueueIoLocalControlWork(node)

RetainedProposalChunks(node) ==
  UNION {
    BroadcastChunkOutbox(node, item.envelope.proposal.view,
                         item.envelope.proposal.subject):
      item \in {retained \in asyncRetainedControl:
                  /\ retained.source = node
                  /\ retained.kind = "Proposal"}}

RetainedControlEmissionItems(node) ==
  SendableItems(node) \cup RetainedProposalChunks(node)

SendAllItems(node) ==
  /\ SendableItems(node) # {}
  /\ asyncSentItems' =
       asyncSentItems \cup RetainedControlEmissionItems(node)
  /\ asyncTransport' =
       asyncTransport \cup PacketsForItems(RetainedControlEmissionItems(node))
  /\ UNCHANGED <<asyncRetainedControl, asyncActiveRequests,
                  asyncCertifiedResponseClaim>>

RetryableItems(node) ==
  RetainedControlEmissionItems(node) \cup ActiveRequestItems(node)

SendNodeRetransmissions(node) ==
  /\ RetryableItems(node) # {}
  /\ asyncSentItems' = asyncSentItems \cup RetryableItems(node)
  /\ asyncTransport' =
       asyncTransport \cup PacketsForItems(RetryableItems(node))
  /\ UNCHANGED <<asyncRetainedControl, asyncActiveRequests,
                  asyncCertifiedResponseClaim>>

NoSendItem ==
  UNCHANGED <<asyncSentItems, asyncRetainedControl,
              asyncActiveRequests, asyncCertifiedResponseClaim,
              asyncTransport>>

AsyncProposedTimeoutCausalCommand(node) ==
  NoItemCandidate("Completion", "BeginTimeout", node, nodeView[node],
                  highestSubject[node])

AsyncProposedTimeoutCausalOrigin(node) ==
  AsyncProposedTimeoutCausalCommand(node).causalOrigin

TimeoutCausalCommand(node) ==
  [AsyncProposedTimeoutCausalCommand(node) EXCEPT
     !.causalOrigin = AsyncEffectiveTimeoutLifecycleOrigin(node)]

(***************************************************************************
The reducer derives historical locked-body retry work from durable lock state
on every RetransmitElapsed event.  The timer is the production handoff: once
it fires, exact current-consumer Completion ownership is installed in the
normal causal scheduler.  During responsive crash recovery the explicit
generation-free ghost projection above retains that already-durable source;
this retransmit installation atomically discharges the projection through
`AsyncHistoricalLockRestartAuthorityTransition`.
***************************************************************************)

HistoricalLockedRetransmitQCs(node) ==
  {qc \in prepareQCs:
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ ~BodyValidatedBy(validatedBodies, node, context, qc.view,
                         generation[node], qc.subject)}

AsyncHistoricalLockedRetransmitCandidateCausalOriginAt(
    node, lockedContext, qc) ==
  AsyncCandidateCausalOrigin(
    "FetchBody", node, qc.context.height,
    qc.view, qc.subject, NoAsyncItem, lockedContext, qc,
    qc.subject, qc.subject, qc.subject)

HistoricalLockedRetransmitCandidate(node) ==
  LET qc == CHOOSE candidateQc \in
                     HistoricalLockedRetransmitQCs(node): TRUE
  IN AsyncCandidateWithIdentityAndOrigin(
       "Completion", "FetchBody", node, qc.context.height,
       qc.view, qc.subject, NoAsyncItem,
       context, nodeView[node], generation[node], qc,
       qc.subject, qc.subject, qc.subject,
       AsyncHistoricalLockedRetransmitCandidateCausalOriginAt(
         node, context, qc))

HistoricalLockedRetransmitSuccessors(node) ==
  IF HistoricalLockedRetransmitQCs(node) = {}
  THEN <<>>
  ELSE FreshCandidateSequence(
         HistoricalLockedRetransmitCandidate(node))

AppendHistoricalLockedRetransmitSuccessors(node) ==
  asyncCausalQueues' =
    [asyncCausalQueues EXCEPT
       ![node] = @ \o HistoricalLockedRetransmitSuccessors(node)]

(***************************************************************************
The same rigid-witness rule is required for the parameterized timeout action.
Runtime callers select nodes from ValidatorIds, making this equivalent to the
direct ENABLED BeginTimeout(node) test on every reachable state.
***************************************************************************)
BeginTimeoutEnabled(node) == BeginTimeoutReady(node)

CommitCertificateDiscoveryStepWork(node) ==
  /\ node \in up
  /\ AsyncServeIngressLifecycleOwnerIdentities(node) = {}
  /\ UNCHANGED <<vars, asyncNow,
                 asyncCommandQueues, asyncNextCommandClass,
                 asyncFifoOwed, asyncTimeoutEmitted,
                 asyncRunnerPhase, asyncRunnerBudget,
                 AsyncLocalAdmissionVars, AsyncIoVars,
                 AsyncDeferredVars,
                 asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                 asyncIngressLanes,
                 asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>
  /\ PublishCommitCertificateRequests(
       CommitCertificateRequestOutbox(node))
  /\ LeaveCausalQueues

DirectCommitCertificateDiscoveryStep(node) ==
  /\ CommitCertificateDiscoveryDue(node)
  /\ CommitCertificateDiscoveryStepWork(node)

DirectHistoricalCommitCertificateDiscoveryStep(node) ==
  /\ HistoricalCommitCertificateDiscoveryDue(node)
  /\ CommitCertificateDiscoveryStepWork(node)

DirectTimeoutStep(node) ==
  /\ TimeoutDue(node)
  /\ asyncTimeoutEmitted' =
       [asyncTimeoutEmitted EXCEPT ![node] = TRUE]
  /\ asyncFifoOwed' =
       [asyncFifoOwed EXCEPT ![node] = NodeQueueNonempty(node)]
  /\ IF BeginTimeoutEnabled(node)
     THEN /\ BeginTimeout(node)
          /\ UNCHANGED asyncOutstandingTags
     ELSE /\ UNCHANGED vars
          /\ asyncOutstandingTags' =
               [asyncOutstandingTags EXCEPT
                  ![node] = @ \cup {"TimeoutElapsed"}]
  /\ IF BeginTimeoutEnabled(node)
     THEN AppendCausalSuccessors(TimeoutCausalCommand(node))
     ELSE LeaveCausalQueues
  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues, asyncDeferredNormalQueues,
                 asyncDeferredHandoffs,
                 asyncNextDeferredClass>>
  /\ asyncDeferredDrainOwed' =
       IF BeginTimeoutEnabled(node)
       THEN [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
       ELSE asyncDeferredDrainOwed
  /\ UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                 asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems,
                 asyncRetainedControl, asyncActiveRequests,
                 asyncCertifiedResponseClaim, asyncTransport,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

DirectRetransmitStep(node) ==
  /\ RetransmitDue(node)
  /\ asyncFifoOwed' =
       [asyncFifoOwed EXCEPT ![node] = NodeQueueNonempty(node)]
  /\ asyncRetransmitDeadlines' =
       [asyncRetransmitDeadlines EXCEPT
          ![node] = asyncNow + AsyncRetransmitPeriod]
  /\ IF NodeIdle(node)
     THEN /\ IF RetryableItems(node) # {}
             THEN SendNodeRetransmissions(node)
             ELSE NoSendItem
          /\ UNCHANGED asyncOutstandingTags
     ELSE /\ NoSendItem
          /\ asyncOutstandingTags' =
               [asyncOutstandingTags EXCEPT
                  ![node] = @ \cup {"RetransmitElapsed"}]
  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues, asyncDeferredNormalQueues,
                 asyncDeferredHandoffs,
                 asyncNextDeferredClass>>
  /\ asyncDeferredDrainOwed' =
       IF NodeIdle(node)
       THEN [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
       ELSE asyncDeferredDrainOwed
  /\ IF NodeIdle(node)
     THEN AppendHistoricalLockedRetransmitSuccessors(node)
     ELSE LeaveCausalQueues
  /\ UNCHANGED <<vars, asyncCommandQueues, asyncNextCommandClass,
                 asyncTimeoutEmitted,
                 asyncNodeDeadlines, asyncIngressLanes,
                 asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets
                 >>

DeferredTimeoutExecutable(node) ==
  /\ "TimeoutElapsed" \in asyncOutstandingTags[node]
  /\ ~AsyncOlderCandidateLifecycleBlocksTimeout(node)
  /\ \/ BeginTimeoutEnabled(node)
     \/ NodeHasDecision(node)
     \/ NodeTimedOut(node, nodeView[node])

DeferredTimeoutStep(node) ==
  /\ DeferredTimeoutExecutable(node)
  /\ IF BeginTimeoutEnabled(node)
     THEN BeginTimeout(node)
     ELSE UNCHANGED vars
  /\ IF BeginTimeoutEnabled(node)
     THEN AppendCausalSuccessors(TimeoutCausalCommand(node))
     ELSE LeaveCausalQueues
  /\ asyncOutstandingTags' =
       [asyncOutstandingTags EXCEPT ![node] = @ \ {"TimeoutElapsed"}]
  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues, asyncDeferredNormalQueues,
                 asyncDeferredHandoffs,
                 asyncNextDeferredClass>>
  /\ asyncDeferredDrainOwed' =
       [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
  /\ UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                 asyncFifoOwed,
                 asyncTimeoutEmitted, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems,
                 asyncRetainedControl, asyncActiveRequests,
                 asyncCertifiedResponseClaim, asyncTransport,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

DeferredRetransmitStep(node) ==
  /\ "RetransmitElapsed" \in asyncOutstandingTags[node]
  \* This is the fixed production run-loop point after local reconciliation,
  \* completion drain, and output retry.  `drive_block_sync` is not gated by
  \* reducer idleness, so neither is this deferred program-counter state.
  /\ UNCHANGED vars
  /\ IF RetryableItems(node) # {}
     THEN SendNodeRetransmissions(node)
     ELSE NoSendItem
  /\ asyncOutstandingTags' =
       [asyncOutstandingTags EXCEPT ![node] = @ \ {"RetransmitElapsed"}]
  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues, asyncDeferredNormalQueues,
                 asyncDeferredHandoffs,
                 asyncNextDeferredClass>>
  /\ asyncDeferredDrainOwed' =
       [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
  /\ AppendHistoricalLockedRetransmitSuccessors(node)
  /\ UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                 asyncFifoOwed,
                 asyncTimeoutEmitted, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

THEOREM DeferredRetransmitConsumesDriveProgramCounter ==
  \A node \in ValidatorIds:
    DeferredRetransmitStep(node)
      => /\ AsyncRetransmitProgramCounter(node) = "DriveDue"
         /\ AsyncRetransmitProgramCounter(node)' = "AwaitDue"
BY Isa
   DEF DeferredRetransmitStep, AsyncRetransmitProgramCounter

DeferredTagExecutable(node) ==
  DeferredTimeoutExecutable(node)
    \/ (/\ "TimeoutElapsed" \notin asyncOutstandingTags[node]
        /\ "RetransmitElapsed" \in asyncOutstandingTags[node])

DeferredTagStep(node) ==
  IF DeferredTimeoutExecutable(node)
  THEN DeferredTimeoutStep(node)
  ELSE DeferredRetransmitStep(node)

AsyncTimeoutPriorityPrecedesCandidate(node, candidate) ==
  /\ \/ AsyncTimeoutClockDue(node)
     \/ "TimeoutElapsed" \in asyncOutstandingTags[node]
  /\ ~AsyncOlderCandidateLifecycleBlocksTimeout(node)
  /\ AsyncEffectiveTimeoutLifecycleOrdinal(node)
       < AsyncCandidateLifecycleOrdinal(candidate)

DeferredWorkOwnsRuntimeTurn(node) ==
  /\ DeferredWorkServiceable(node)
  /\ ~AsyncTimeoutPriorityPrecedesCandidate(
       node, NextDeferredCommand(node))

(***************************************************************************
The historical `FifoRuntimeStep` name denotes the timer-versus-command debt
tracked by `asyncFifoOwed`; command selection itself is lifecycle-first with a
cyclic equal-ordinal class tie break, as defined by `NextNodeCommandIndex`.
***************************************************************************)
FifoRuntimeStep(node) ==
  LET command == NextNodeCommand(node)
      succeeds == CommandDispatchable(command)
  IN /\ NodeQueueNonempty(node)
     /\ \/ succeeds
        \/ NodeIdle(node)
        \/ DeferredCommandCanAdmit(node, command)
     /\ RemoveNextNodeCommand(node)
     /\ IF succeeds
        THEN /\ AsyncCommitImportExecutionProvenance(command)
             /\ ExecuteCommand(command)
             /\ AppendCausalSuccessors(command)
             /\ UNCHANGED <<asyncDeferredCompletionQueues,
                            asyncDeferredProgressQueues,
                            asyncDeferredNormalQueues,
                            asyncDeferredHandoffs,
                            asyncNextDeferredClass>>
             /\ asyncDeferredDrainOwed' =
                  [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
        ELSE IF ~NodeIdle(node)
             THEN /\ DeferCommand(command)
                  /\ LeaveCausalQueues
             ELSE /\ DiscardCommand(command)
                  /\ LeaveCausalQueues
                  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                                 asyncDeferredProgressQueues,
                                 asyncDeferredNormalQueues,
                                 asyncDeferredHandoffs,
                                 asyncNextDeferredClass>>
                  /\ asyncDeferredDrainOwed' =
                       [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
     /\ asyncFifoOwed' = [asyncFifoOwed EXCEPT ![node] = FALSE]
     /\ asyncTimeoutEmitted' =
          IF succeeds /\ command.kind = "PersistInstallTC"
          THEN [asyncTimeoutEmitted EXCEPT ![node] = FALSE]
          ELSE asyncTimeoutEmitted

DeferredDrainStep(node) ==
  /\ DeferredWorkServiceable(node)
  /\ IF ~DeferredQueueNonempty(node)
     THEN /\ UNCHANGED <<vars, asyncCommandQueues,
                         asyncNextCommandClass, asyncFifoOwed,
                         asyncTimeoutEmitted, asyncDeferredCompletionQueues,
                         asyncDeferredProgressQueues,
                         asyncDeferredNormalQueues,
                         asyncDeferredHandoffs,
                         asyncNextDeferredClass, asyncOutstandingTags,
                         asyncNodeDeadlines, asyncRetransmitDeadlines,
                         asyncSentItems, asyncRetainedControl,
                         asyncActiveRequests, asyncCertifiedResponseClaim,
                         asyncTransport, asyncIngressLanes,
                         asyncIngressReady, asyncHeldChunks,
                         asyncHistoricalRecoveryTargets>>
          /\ LeaveCausalQueues
          /\ asyncDeferredDrainOwed' =
               [asyncDeferredDrainOwed EXCEPT ![node] = FALSE]
     ELSE LET command == NextDeferredCommand(node)
              handoffMatches == DeferredHandoffMatches(node, command)
          IN IF DeferredHandoffAllowsExecution(node, command)
             THEN /\ IF handoffMatches
                        THEN /\ RemoveNextDeferredCommand(node)
                             /\ ClearDeferredHandoff(node)
                        ELSE /\ RemoveNextDeferredCommand(node)
                             /\ RetainDeferredHandoffs
                  /\ AsyncCommitImportExecutionProvenance(command)
                  /\ ExecuteCommand(command)
                  /\ AppendCausalSuccessors(command)
                  /\ asyncDeferredDrainOwed' = asyncDeferredDrainOwed
                  /\ asyncTimeoutEmitted' =
                       IF command.kind = "PersistInstallTC"
                       THEN [asyncTimeoutEmitted EXCEPT ![node] = FALSE]
                       ELSE asyncTimeoutEmitted
                  /\ UNCHANGED <<asyncCommandQueues,
                                  asyncNextCommandClass, asyncFifoOwed>>
             ELSE IF DeferredHandoffBlocksExecution(node, command)
                  THEN /\ LeaveCausalQueues
                       /\ AdvanceNextDeferredClass(node)
                       /\ UNCHANGED <<vars, asyncCommandQueues,
                                      asyncNextCommandClass,
                                      asyncFifoOwed, asyncTimeoutEmitted,
                                      asyncDeferredCompletionQueues,
                                      asyncDeferredProgressQueues,
                                      asyncDeferredNormalQueues,
                                      asyncOutstandingTags,
                                      asyncNodeDeadlines,
                                      asyncRetransmitDeadlines,
                                      asyncSentItems, asyncRetainedControl,
                                      asyncActiveRequests,
                                      asyncCertifiedResponseClaim,
                                      asyncTransport, asyncIngressLanes,
                                      asyncIngressReady, asyncHeldChunks,
                                      asyncHistoricalRecoveryTargets>>
                       /\ asyncDeferredDrainOwed' =
                            [asyncDeferredDrainOwed EXCEPT
                               ![node] = FALSE]
                       /\ RetainDeferredHandoffs
                  ELSE IF ~NodeIdle(node)
                       THEN /\ LeaveCausalQueues
                            /\ AdvanceNextDeferredClass(node)
                            /\ UNCHANGED <<vars, asyncCommandQueues,
                                           asyncNextCommandClass,
                                           asyncFifoOwed,
                                           asyncTimeoutEmitted,
                                           asyncDeferredCompletionQueues,
                                           asyncDeferredProgressQueues,
                                           asyncDeferredNormalQueues,
                                           asyncOutstandingTags,
                                           asyncNodeDeadlines,
                                           asyncRetransmitDeadlines,
                                           asyncSentItems,
                                           asyncRetainedControl,
                                           asyncActiveRequests,
                                           asyncCertifiedResponseClaim,
                                           asyncTransport,
                                           asyncIngressLanes,
                                           asyncIngressReady,
                                           asyncHeldChunks,
                                           asyncHistoricalRecoveryTargets>>
                            /\ asyncDeferredDrainOwed' =
                                 [asyncDeferredDrainOwed EXCEPT
                                    ![node] = FALSE]
                            /\ IF DeferredHandoffActive(node)
                               THEN RetainDeferredHandoffs
                               ELSE InstallDeferredHandoff(node, command)
                       ELSE /\ IF handoffMatches
                               THEN /\ RemoveNextDeferredCommand(node)
                                    /\ ClearDeferredHandoff(node)
                               ELSE /\ RemoveNextDeferredCommand(node)
                                    /\ RetainDeferredHandoffs
                            /\ DiscardCommand(command)
                            /\ LeaveCausalQueues
                            /\ asyncDeferredDrainOwed' =
                                 asyncDeferredDrainOwed
                            /\ UNCHANGED <<asyncCommandQueues,
                                           asyncNextCommandClass,
                                           asyncFifoOwed,
                                           asyncTimeoutEmitted>>

IdleRuntimeStep(node) ==
  /\ UNCHANGED <<vars, asyncCommandQueues, asyncNextCommandClass,
                 asyncTimeoutEmitted,
                 AsyncDeferredVars,
                 asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems,
                 asyncRetainedControl, asyncActiveRequests,
                 asyncCertifiedResponseClaim, asyncTransport,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>
  /\ LeaveCausalQueues
  /\ asyncFifoOwed' = [asyncFifoOwed EXCEPT ![node] = FALSE]

RuntimeStep(node) ==
  \/ /\ DeferredWorkOwnsRuntimeTurn(node)
        /\ DeferredDrainStep(node)
  \/ /\ ~DeferredWorkOwnsRuntimeTurn(node)
        /\ DeferredTagExecutable(node)
        /\ DeferredTagStep(node)
  \/ /\ ~DeferredWorkOwnsRuntimeTurn(node)
        /\ ~DeferredTagExecutable(node)
        /\ TimeoutDue(node)
        /\ DirectTimeoutStep(node)
  \/ /\ ~DeferredWorkOwnsRuntimeTurn(node)
        /\ ~DeferredTagExecutable(node)
        /\ ~TimeoutDue(node)
        /\ NodeQueueNonempty(node)
        /\ asyncFifoOwed[node]
        /\ FifoRuntimeStep(node)
  \/ /\ ~DeferredWorkOwnsRuntimeTurn(node)
        /\ ~DeferredTagExecutable(node)
        /\ ~TimeoutDue(node)
        /\ ~(NodeQueueNonempty(node) /\ asyncFifoOwed[node])
        /\ RetransmitDue(node)
        /\ DirectRetransmitStep(node)
  \/ /\ ~DeferredWorkOwnsRuntimeTurn(node)
        /\ ~DeferredTagExecutable(node)
        /\ ~TimeoutDue(node)
        /\ ~(NodeQueueNonempty(node) /\ asyncFifoOwed[node])
        /\ ~RetransmitDue(node)
        /\ NodeQueueNonempty(node)
        /\ FifoRuntimeStep(node)
  \/ /\ ~DeferredWorkOwnsRuntimeTurn(node)
        /\ ~DeferredTagExecutable(node)
        /\ ~TimeoutDue(node)
        /\ ~RetransmitDue(node)
        /\ ~NodeQueueNonempty(node)
        /\ IdleRuntimeStep(node)

(***************************************************************************
When the runner is already in Local and a Serve ticket is live, exactly the
selected admissible Local owner whose immutable ordinal is older than the
ticket may be admitted.  The admission is one ordinary Local macro-step: it
does not replenish the Local budget or change phase, and every Serve ticket,
reservation, tombstone, and ingress-admission record remains frozen.  A
subsequent runner occurrence re-evaluates the next selected owner before the
target-only jump to Ingress.
***************************************************************************)
SerializedLocalPrecedesServeIngressStep(node) ==
  /\ AsyncOlderLocalLifecyclePrecedesServeIngress(node)
  /\ SelectedLocalAdmissionAdvance(node)

LocalAdmissionStep(node) ==
  /\ asyncRunnerPhase[node] = "Local"
  \* A frozen exact ingress owner receives a target-only turn.  No producer,
  \* causal, Completion, or other local admission may acquire a later
  \* position before that owner reaches the ingress selector.
  /\ AsyncServeIngressLifecycleOwnerIdentities(node) = {}
  /\ UNCHANGED AsyncDeferredVars
  /\ IF LocalAdmissionCanAdvance(node)
     THEN LET source == SelectedLocalSource(node)
          IN /\ IF source = "Producer"
                 THEN /\ AdmitProducerCompletion(node)
                      /\ LeaveCausalQueues
                 ELSE AdmitCausalHead(node)
             /\ UpdateLocalAdmissionMetadata(node, source)
             /\ asyncRunnerPhase' = asyncRunnerPhase
             /\ asyncRunnerBudget' =
                  [asyncRunnerBudget EXCEPT ![node] = @ - 1]
     ELSE /\ LeaveCausalQueues
          /\ RecordBlockedCausalDebt(node)
          /\ UNCHANGED <<vars, asyncCommandQueues,
                          asyncNextCommandClass,
                          asyncFifoOwed, asyncTimeoutEmitted,
                          AsyncIoVars,
                          asyncOutstandingTags, asyncNodeDeadlines,
                          asyncRetransmitDeadlines, asyncSentItems,
                          asyncRetainedControl, asyncActiveRequests,
                          asyncCertifiedResponseClaim,
                          asyncTransport, asyncIngressLanes,
                          asyncIngressReady, asyncHeldChunks,
                          asyncHistoricalRecoveryTargets>>
          /\ asyncRunnerPhase' =
               [asyncRunnerPhase EXCEPT ![node] = "Ingress"]
          /\ asyncRunnerBudget' =
               [asyncRunnerBudget EXCEPT
                  ![node] = AsyncIngressCapacity]

IngressDrainStep(node) ==
  /\ asyncRunnerPhase[node] = "Ingress"
  /\ UNCHANGED AsyncDeferredVars
  /\ LeaveCausalQueues
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ IF asyncRunnerBudget[node] > 0
          /\ asyncIngressReady[node] # <<>>
          /\ DrainableIngressIndices(node) # {}
     THEN /\ DrainFairIngressSelected(node)
          /\ asyncRunnerPhase' = asyncRunnerPhase
          /\ asyncRunnerBudget' =
               [asyncRunnerBudget EXCEPT ![node] = @ - 1]
     ELSE /\ UNCHANGED <<vars, asyncCommandQueues,
                         asyncNextCommandClass, asyncFifoOwed,
                         asyncTimeoutEmitted, AsyncIoVars,
                         asyncOutstandingTags,
                         asyncNodeDeadlines, asyncRetransmitDeadlines,
                         asyncSentItems, asyncRetainedControl,
                         asyncActiveRequests, asyncCertifiedResponseClaim,
                         asyncTransport, asyncIngressLanes,
                         asyncIngressReady, asyncHeldChunks,
                         asyncHistoricalRecoveryTargets
                         >>
          /\ asyncRunnerPhase' =
               [asyncRunnerPhase EXCEPT ![node] = "Runtime"]
          /\ asyncRunnerBudget' =
               [asyncRunnerBudget EXCEPT ![node] = 1]

SerializedRuntimeStep(node) ==
  /\ asyncRunnerPhase[node] = "Runtime"
  \* Runtime can enqueue causal successors, execute Completion work, and emit
  \* retransmission/timeout producers.  Once an exact ingress ticket exists,
  \* this unrestricted phase is skipped; the dedicated action below permits
  \* only one macro-step for a lifecycle whose shared ordinal is older.
  /\ AsyncServeIngressLifecycleOwnerIdentities(node) = {}
  /\ UNCHANGED AsyncIoVars
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ RuntimeStep(node)
  /\ asyncRunnerPhase' = [asyncRunnerPhase EXCEPT ![node] = "Local"]
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = AsyncQueueCapacity]

(***************************************************************************
A Serve ticket does not erase a Runtime lifecycle which was admitted first.
Exactly one ordinary Runtime macro-step may run while that strictly older
owner exists.  The macro-step cannot admit Local, ingress, I/O, or Serve work;
it moves the runner to Local, where the existing target-only turn immediately
jumps to Ingress.  Consequently each distinct later Serve replenishment can
interleave one older timeout episode, but replenishment is never itself called
progress and no eventual-absence premise is introduced.
***************************************************************************)
SerializedRuntimePrecedesServeIngressStep(node) ==
  /\ asyncRunnerPhase[node] = "Runtime"
  /\ AsyncServeIngressLifecycleOwnerIdentities(node) # {}
  /\ AsyncOlderRuntimeLifecyclePrecedesServeIngress(node)
  /\ UNCHANGED AsyncIoVars
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ RuntimeStep(node)
  /\ asyncRunnerPhase' = [asyncRunnerPhase EXCEPT ![node] = "Local"]
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = AsyncQueueCapacity]

AsyncServeIngressTargetOnlyTurn(node) ==
  /\ AsyncServeIngressLifecycleOwnerIdentities(node) # {}
  /\ asyncRunnerPhase[node] \in {"Runtime", "Local"}
  /\ ~( /\ asyncRunnerPhase[node] = "Runtime"
         /\ AsyncOlderRuntimeLifecyclePrecedesServeIngress(node))
  /\ ~( /\ asyncRunnerPhase[node] = "Local"
         /\ AsyncOlderLocalLifecyclePrecedesServeIngress(node))
  /\ UNCHANGED <<vars, asyncCommandQueues, asyncNextCommandClass,
                 asyncFifoOwed, asyncTimeoutEmitted,
                 AsyncLocalAdmissionVars, AsyncIoVars, AsyncDeferredVars,
                 asyncCausalQueues, asyncOutstandingTags,
                 asyncNodeDeadlines, asyncRetransmitDeadlines,
                 asyncSentItems, asyncRetainedControl,
                 asyncActiveRequests, asyncCertifiedResponseClaim,
                 asyncTransport, asyncIngressLanes, asyncIngressReady,
                 asyncHeldChunks, asyncHistoricalRecoveryTargets>>
  /\ asyncRunnerPhase' =
       [asyncRunnerPhase EXCEPT ![node] = "Ingress"]
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = AsyncIngressCapacity]

THEOREM AsyncServeIngressTicketExcludesLaterLocalWork ==
  \A node \in ValidatorIds:
    AsyncServeIngressLifecycleOwnerIdentities(node) # {}
      => /\ ~LocalAdmissionStep(node)
         /\ ~SerializedRuntimeStep(node)
         /\ ~EnqueueIoLocalControlWork(node)
         /\ ~CommitCertificateDiscoveryStepWork(node)
BY DEF LocalAdmissionStep, SerializedRuntimeStep,
       EnqueueIoLocalControlWork, CommitCertificateDiscoveryStepWork

THEOREM LocalAdmissionAdvanceSelectsAtomicWork ==
  \A node \in ValidatorIds:
    /\ LocalAdmissionStep(node)
    /\ LocalAdmissionCanAdvance(node)
    => SelectedLocalAdmissionAdvance(node)
BY Isa
   DEF LocalAdmissionStep, SelectedLocalAdmissionAdvance

THEOREM SerializedLocalPrecedesServeIngressExactFrame ==
  \A node \in ValidatorIds:
    SerializedLocalPrecedesServeIngressStep(node)
      => /\ asyncRunnerPhase[node] = "Local"
         /\ asyncRunnerPhase' = asyncRunnerPhase
         /\ asyncRunnerBudget'[node] = asyncRunnerBudget[node] - 1
         /\ LocalSourceLifecycleOrdinal(
              node, SelectedLocalSource(node))
              < AsyncServeEarliestIngressSchedulerOrdinal(node)
         /\ asyncNextServeAdmissionOrdinal' =
              asyncNextServeAdmissionOrdinal
         /\ asyncNextServeIngressOrdinal' =
              asyncNextServeIngressOrdinal
         /\ asyncServeIngressAdmissions' = asyncServeIngressAdmissions
         /\ AsyncServeIngressLifecycleOwnerIdentities(node)' =
              AsyncServeIngressLifecycleOwnerIdentities(node)
         /\ AsyncServeIngressLifecycleOwnerIdentities(node)' # {}
         /\ asyncServeAdmissions' = asyncServeAdmissions
         /\ asyncServeReservations' = asyncServeReservations
         /\ asyncServeTombstones' = asyncServeTombstones
BY Isa
   DEF SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncOlderLocalLifecyclePrecedesServeIngress,
       AdmitProducerCompletion, AdmitCausalHead, EnqueueCandidate,
       AsyncServeEarliestIngressSchedulerOrdinal,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncIoVars, AsyncServeLifecycleVars,
       AsyncServeIngressAdmissionVars

THEOREM AsyncServeIngressTargetOnlyTurnJumpsToIngress ==
  \A node \in ValidatorIds:
    AsyncServeIngressTargetOnlyTurn(node)
      => /\ asyncRunnerPhase[node] \in {"Runtime", "Local"}
         /\ asyncRunnerPhase'[node] = "Ingress"
         /\ asyncRunnerBudget'[node] = AsyncIngressCapacity
BY Isa DEF AsyncServeIngressTargetOnlyTurn

THEOREM AsyncServeIngressTargetOnlyCannotOvertakeOlderRuntimeLifecycle ==
  \A node \in ValidatorIds:
    /\ AsyncServeIngressLifecycleOwnerIdentities(node) # {}
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ AsyncOlderRuntimeLifecyclePrecedesServeIngress(node)
    => ~AsyncServeIngressTargetOnlyTurn(node)
BY DEF AsyncServeIngressTargetOnlyTurn

THEOREM AsyncServeIngressTargetOnlyCannotOvertakeOlderLocalLifecycle ==
  \A node \in ValidatorIds:
    /\ AsyncServeIngressLifecycleOwnerIdentities(node) # {}
    /\ asyncRunnerPhase[node] = "Local"
    /\ AsyncOlderLocalLifecyclePrecedesServeIngress(node)
    => ~AsyncServeIngressTargetOnlyTurn(node)
BY DEF AsyncServeIngressTargetOnlyTurn

THEOREM AsyncOlderRuntimeInterleaveRetainsServeTicketAndYieldsLocal ==
  \A node \in ValidatorIds:
    SerializedRuntimePrecedesServeIngressStep(node)
      => /\ asyncRunnerPhase'[node] = "Local"
         /\ asyncServeIngressAdmissions' = asyncServeIngressAdmissions
         /\ asyncServeReservations' = asyncServeReservations
         /\ asyncIoQueues' = asyncIoQueues
BY Isa
   DEF SerializedRuntimePrecedesServeIngressStep,
       AsyncIoVars, AsyncServeIngressAdmissionVars,
       AsyncServeLifecycleVars

RunNodeWork(node) ==
  /\ node \in AsyncActiveServiceNodes
  /\ node \in up
  /\ ~NodeHasApplication(node)
  /\ IF ResponsiveReplayQuarantined(node)
     THEN /\ ResponsiveReplayDraining(node)
          /\ ~NodeIdle(node)
          /\ asyncIngressReady[node] = <<>>
          /\ \/ LocalAdmissionStep(node)
             \/ IngressDrainStep(node)
             \/ SerializedRuntimeStep(node)
     ELSE IF /\ AsyncServeIngressLifecycleOwnerIdentities(node) # {}
             /\ asyncRunnerPhase[node] \in {"Runtime", "Local"}
          THEN IF /\ asyncRunnerPhase[node] = "Runtime"
                    /\ AsyncOlderRuntimeLifecyclePrecedesServeIngress(node)
               THEN SerializedRuntimePrecedesServeIngressStep(node)
               ELSE IF /\ asyncRunnerPhase[node] = "Local"
                          /\ AsyncOlderLocalLifecyclePrecedesServeIngress(node)
                    THEN SerializedLocalPrecedesServeIngressStep(node)
                    ELSE AsyncServeIngressTargetOnlyTurn(node)
          ELSE \/ LocalAdmissionStep(node)
               \/ IngressDrainStep(node)
               \/ SerializedRuntimeStep(node)
  /\ UNCHANGED asyncNow
  /\ asyncNodeServiceDeadlines' =
       [asyncNodeServiceDeadlines EXCEPT
          ![node] = asyncNow + AsyncDeliveryBound]
  /\ UNCHANGED asyncIoServiceDeadlines

THEOREM AsyncLaterServeTicketInterleavesOlderRuntimeEpisode ==
  \A node \in ValidatorIds:
    /\ AsyncServeIngressLifecycleOwnerIdentities(node) # {}
    /\ AsyncTimeoutLifecycleOwned(node)
    /\ AsyncTimeoutLifecycleOrdinal(node)
         < AsyncServeEarliestIngressSchedulerOrdinal(node)
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ RunNodeWork(node)
    => SerializedRuntimePrecedesServeIngressStep(node)
BY Isa
   DEF RunNodeWork,
       AsyncOlderRuntimeLifecyclePrecedesServeIngress,
       AsyncOlderFrozenTimeoutLifecyclePrecedesServeIngress,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn

THEOREM AsyncLaterServeTicketInterleavesOlderLocalEpisode ==
  \A node \in ValidatorIds:
    /\ AsyncServeIngressLifecycleOwnerIdentities(node) # {}
    /\ LocalAdmissionCanAdvance(node)
    /\ LocalSourceLifecycleOrdinal(node, SelectedLocalSource(node))
         < AsyncServeEarliestIngressSchedulerOrdinal(node)
    /\ asyncRunnerPhase[node] = "Local"
    /\ RunNodeWork(node)
    => SerializedLocalPrecedesServeIngressStep(node)
BY Isa
   DEF RunNodeWork,
       AsyncOlderLocalLifecyclePrecedesServeIngress,
       SerializedLocalPrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn

RunNode(node) ==
  /\ node \in AsyncCurrentResponsiveVoters
  /\ RunNodeWork(node)

RunHistoricalRecoveryNode(node) ==
  /\ HistoricalRecoveryTarget(node)
  /\ RunNodeWork(node)

ResponsiveReplayRunNode ==
  LET node == asyncRecoveryNode
  IN /\ ~gst
     /\ ResponsiveReplayDraining(node)
     /\ RunNode(node)
     /\ AsyncNonCrashOuterFrame

ResponsiveReplayServiceIoWorker ==
  LET node == asyncRecoveryNode
  IN /\ ~gst
     /\ ResponsiveReplayDraining(node)
     /\ ServiceIoWorker(node)
     /\ AsyncNonRunnerOuterFrame

HistoricalIdleStep ==
  /\ UNCHANGED <<vars, asyncCommandQueues, asyncNextCommandClass,
                 asyncFifoOwed,
                 asyncTimeoutEmitted, asyncRunnerPhase,
                 asyncRunnerBudget, AsyncIoVars, AsyncDeferredVars,
                 asyncCausalQueues, asyncOutstandingTags,
                 asyncNodeDeadlines, asyncRetransmitDeadlines,
                 asyncSentItems, asyncRetainedControl,
                 asyncActiveRequests, asyncCertifiedResponseClaim,
                 asyncTransport,
                 asyncIngressLanes, asyncIngressReady,
                 asyncHeldChunks, asyncHistoricalRecoveryTargets>>

RunHistoricalServer(node) ==
  /\ node \in AsyncActiveServiceNodes
  /\ node \in AsyncResponsiveAppliedArchiveServers
  /\ ~ResponsiveReplayQuarantined(node)
  /\ NodeHasApplication(node)
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ IF HistoricalDrainableIngressIndices(node) # {}
     THEN DrainHistoricalIngressSelected(node)
     ELSE HistoricalIdleStep
  /\ UNCHANGED asyncNow
  /\ asyncNodeServiceDeadlines' =
       [asyncNodeServiceDeadlines EXCEPT
          ![node] = asyncNow + AsyncDeliveryBound]
  /\ UNCHANGED asyncIoServiceDeadlines

(***************************************************************************
Responsive pre-GST crash/restart.

This lifecycle admits repeated responsive-validator crashes. Each crash makes
the complete process-local reducer, scheduler, callback sender, and completion
queue inaccessible. Authenticated full-process replacement starts a fresh
generation-zero episode, reconstructs durable control frontiers, and drives
the production signature FIFO one Core owner at a time. Only the
recovering node is quarantined; other validators and network-owned packets
continue independently.  Immutable sent history remains outside the reset.
***************************************************************************)

AsyncRecoveryPhases ==
  {"Eligible", "RestartRequired", "ReplayRequired", "Replaying",
   "Recovered"}

AsyncRestartCandidateCausalOriginAt(
    commandClass, kind, node, restartContext,
    roundView, subject, evidence) ==
  AsyncCandidateCausalOrigin(
    kind, node, restartContext.height, roundView,
    subject, NoAsyncItem, restartContext, evidence,
    subject, subject, subject)

RestartCandidate(commandClass, kind, node, roundView, subject, evidence) ==
  AsyncCandidateWithIdentityAndOrigin(
    commandClass, kind, node, context.height, roundView, subject,
    NoAsyncItem, context, nodeView[node], generation[node], evidence,
    subject, subject, subject,
    AsyncRestartCandidateCausalOriginAt(
      commandClass, kind, node, context, roundView, subject, evidence))

RestartDecisions(node) ==
  {decision \in decisions:
     /\ decision.node = node
     /\ decision.qc.context = context
     /\ decision.qc.phase = "Commit"
     /\ [node |-> node, qc |-> decision.qc] \notin applied}

RestartLockedCommitIntents(node) ==
  {vote \in commitIntents:
     /\ vote.context = context
     /\ vote.signer = node
     /\ vote.phase = "Commit"
     /\ vote.view = lockRank[node]
     /\ vote.subject = lockSubject[node]}

ReplayCommitIntentReady(node, vote) ==
  \/ VoteSign(node, vote) \in signVotes
  \/ \E item \in asyncRetainedControl:
       /\ item.kind = "CommitVote"
       /\ item.source = node
       /\ item.envelope.vote = vote
  \/ VoteAt(node, vote) \in receivedVotes
  \/ \E qc \in commitQCs:
       /\ qc.context = vote.context
       /\ qc.view = vote.view
       /\ qc.subject = vote.subject
  \/ NodeHasDecision(node)

ReplayCommitSourcesReady(node) ==
  \A vote \in RestartLockedCommitIntents(node):
    ReplayCommitIntentReady(node, vote)

RestartTimeoutIntents(node) ==
  {vote \in timeoutIntents:
     /\ vote.context = context
     /\ vote.signer = node
     /\ vote.view = nodeView[node]}

RestartPrepareIntents(node) ==
  {vote \in prepareIntents:
     /\ vote.context = context
     /\ vote.signer = node
     /\ vote.phase = "Prepare"
     /\ vote.view = nodeView[node]
     /\ RestartTimeoutIntents(node) = {}}

RestartProposalIntents(node) ==
  {proposal \in proposalIntents:
     /\ proposal.context = context
     /\ proposal.proposer = node
     /\ proposal.view = nodeView[node]
     /\ RestartTimeoutIntents(node) = {}}

RestartDecision(node) ==
  CHOOSE entry: entry \in RestartDecisions(node)

RestartLockedCommitIntent(node) ==
  CHOOSE entry: entry \in RestartLockedCommitIntents(node)

RestartTimeoutIntent(node) ==
  CHOOSE entry: entry \in RestartTimeoutIntents(node)

RestartPrepareIntent(node) ==
  CHOOSE entry: entry \in RestartPrepareIntents(node)

RestartProposalIntent(node) ==
  CHOOSE entry: entry \in RestartProposalIntents(node)

RestartDecisionReplay(node) ==
  LET decision == RestartDecision(node)
      qc == decision.qc
  IN <<RestartCandidate("Completion", "FetchBody", node,
                        qc.view, qc.subject, qc)>>

RestartLockedPrepareQCs(node) ==
  IF lockPrepareQc[node] = NoPrepareQC
  THEN {}
  ELSE {lockPrepareQc[node]}

RestartLockedPrepareQC(node) ==
  lockPrepareQc[node]

RestartLockedBodyReplay(node) ==
  IF RestartLockedPrepareQCs(node) = {}
  THEN <<>>
  ELSE LET qc == RestartLockedPrepareQC(node)
       IN <<RestartCandidate("Completion", "FetchBody", node,
                             qc.view, qc.subject, qc)>>

RestartLockedCommitReplay(node) ==
  LET vote == RestartLockedCommitIntent(node)
  IN <<RestartCandidate("Completion", "SignVote", node,
                        vote.view, vote.subject, vote)>>

RestartTimeoutReplay(node) ==
  LET vote == RestartTimeoutIntent(node)
  IN <<RestartCandidate("Completion", "SignTimeout", node,
                        vote.view, vote.highSubject, vote)>>

RestartPrepareReplay(node) ==
  LET vote == RestartPrepareIntent(node)
  IN <<RestartCandidate("Completion", "SignVote", node,
                        vote.view, vote.subject, vote)>>

RestartProposalReplay(node) ==
  LET proposal == RestartProposalIntent(node)
  IN <<RestartCandidate("Completion", "SignProposal", node,
                        proposal.view, proposal.subject, proposal)>>

RestartRunnerAssemblyEnabled(node) ==
  /\ node \in Honest \cap up \cap CurrentVoters
  /\ node = Leader(context, nodeView[node])
  /\ ~NodeHasApplication(node)
  /\ RestartDecisions(node) = {}
  /\ ~NodeTimedOut(node, nodeView[node])
  /\ ~BodyHeldBy(durableBodies, node, context, nodeView[node],
                  AsyncProposalSubject(node))

RestartRunnerAssembly(node) ==
  LET subject == AsyncProposalSubject(node)
  IN IF RestartRunnerAssemblyEnabled(node)
     THEN <<RestartCandidate("Normal", "AssembleBody", node,
                             nodeView[node], subject, NoAsyncItem)>>
     ELSE <<>>

(***************************************************************************
Production enqueues every still-active durable signature in one FIFO.  A
Decision short-circuits signing.  Otherwise Timeout excludes Proposal and
Prepare for the current round, while the exact historical locked Commit is
independently appended last.
***************************************************************************)
RestartTimeoutOrProposalReplay(node) ==
  IF RestartTimeoutIntents(node) # {}
  THEN RestartTimeoutReplay(node)
  ELSE IF RestartProposalIntents(node) # {}
       THEN RestartProposalReplay(node)
       ELSE <<>>

RestartPrepareReplayIfActive(node) ==
  IF RestartPrepareIntents(node) # {}
  THEN RestartPrepareReplay(node)
  ELSE <<>>

RestartLockedCommitReplayIfActive(node) ==
  IF RestartLockedCommitIntents(node) # {}
  THEN RestartLockedCommitReplay(node)
  ELSE <<>>

RestartSignatureReplay(node) ==
  IF NodeHasApplication(node) \/ RestartDecisions(node) # {}
  THEN <<>>
  ELSE RestartTimeoutOrProposalReplay(node)
         \o RestartPrepareReplayIfActive(node)
         \o RestartLockedCommitReplayIfActive(node)

RestartReplay(node) ==
  IF NodeHasApplication(node)
  THEN <<>>
  ELSE IF RestartDecisions(node) # {}
  THEN RestartDecisionReplay(node)
  ELSE LET locked == RestartLockedBodyReplay(node)
           signatures == RestartSignatureReplay(node)
       IN IF Len(signatures) > 0
          THEN locked \o <<Head(signatures)>>
          ELSE IF Len(locked) > 0
               THEN locked
               ELSE RestartRunnerAssembly(node)

(***************************************************************************
Restart atomically discards every volatile scheduler carrier for `node`.
Consequently replay must not coalesce against either a matching pre-crash
queue owner or a same-generation transient service marker which that reset is
about to erase.  Only a restart-durable terminal-discard tombstone can
suppress reconstruction here.  Signature completion is deliberately
restart-scoped: its durable intent must reissue a new-generation callback, so
it is never eligible for a terminal replay tombstone.
***************************************************************************)
AsyncCandidateRestartReplayTombstoned(candidate) ==
  /\ candidate.kind \notin AsyncRestartScopedCandidateServiceKinds
  /\ AsyncCandidateTerminalTombstoned(candidate)

FreshRestartCandidateSequence(replay) ==
  CASE Len(replay) = 0 -> <<>>
    [] Len(replay) = 1 ->
         IF AsyncCandidateRestartReplayTombstoned(replay[1])
         THEN <<>>
         ELSE <<replay[1]>>
    [] Len(replay) = 2 ->
         (IF AsyncCandidateRestartReplayTombstoned(replay[1])
          THEN <<>>
          ELSE <<replay[1]>>)
           \o
         (IF AsyncCandidateRestartReplayTombstoned(replay[2])
          THEN <<>>
          ELSE <<replay[2]>>)
    [] Len(replay) = 3 ->
         (IF AsyncCandidateRestartReplayTombstoned(replay[1])
          THEN <<>>
          ELSE <<replay[1]>>)
           \o
         (IF AsyncCandidateRestartReplayTombstoned(replay[2])
          THEN <<>>
          ELSE <<replay[2]>>)
           \o
         (IF AsyncCandidateRestartReplayTombstoned(replay[3])
          THEN <<>>
          ELSE <<replay[3]>>)
    [] OTHER -> <<>>

(***************************************************************************
While the first durable signature is active, the Fetch prefix may resolve a
locally durable body and expose the deterministic Validate successor batch.
These exact descendants remain quarantined with the recovering consumer; no
unrelated ingress or timeout work may enter this corridor.
***************************************************************************)
RestartLockedBodyPipelineCandidate(node, candidate) ==
  \E qc \in RestartLockedPrepareQCs(node):
    /\ candidate.node = node
    /\ candidate.height = qc.context.height
    /\ candidate.view = qc.view
    /\ candidate.subject = qc.subject
    /\ candidate.evidence = qc
    /\ CandidateConsumerCurrent(candidate)
    /\ candidate.kind \in
         {"FetchBody", "ValidateBody", "BeginPrepare",
          "BeginLockCommit", "Apply"}

RestartLockedCertifiedRequest(node, request) ==
  /\ request.kind = "CertifiedRequest"
  /\ request.source = node
  /\ \E qc \in RestartLockedPrepareQCs(node):
       request \in CertifiedRequestOutbox(node, qc)

RestartHighestPrepareQCs(node) ==
  IF highestPrepareQc[node] = NoPrepareQC
  THEN {}
  ELSE {highestPrepareQc[node]}

RestartDecisionQCs(node) ==
  {decision.qc:
     decision \in {entry \in decisions:
       entry.node = node /\ entry.qc.context = context}}

RestartDecisionCertifiedRequest(node, request) ==
  /\ request.kind = "CertifiedRequest"
  /\ request.source = node
  /\ \E qc \in RestartDecisionQCs(node):
       request \in CertifiedRequestOutbox(node, qc)

RestartDurableCertifiedRequest(node, request) ==
  \/ RestartLockedCertifiedRequest(node, request)
  \/ RestartDecisionCertifiedRequest(node, request)

RestartRetainedActiveRequests(node) ==
  {request \in asyncActiveRequests:
     \/ request.source # node
     \/ RestartDurableCertifiedRequest(node, request)}

RestartInstalledTCs(node) ==
  {entry.tc:
     entry \in {installed \in installedTCs:
       installed.node = node /\ installed.tc.context = context}}

RestartLastInstalledTCs(node) ==
  IF lastInstalledTc[node] = NoTimeoutCertificate
  THEN {}
  ELSE {lastInstalledTc[node]}

RestartHighestPrepareControl(node) ==
  IF highestPrepareQc[node] = NoPrepareQC
     THEN {}
     ELSE QcOutbox(node, highestPrepareQc[node])

RestartDecisionControl(node) ==
  LET certificates == RestartDecisionQCs(node)
  IN IF certificates = {}
     THEN {}
     ELSE QcOutbox(node, CHOOSE qc \in certificates: TRUE)

RestartLastTCControl(node) ==
  LET certificates == RestartLastInstalledTCs(node)
  IN IF certificates = {}
     THEN {}
     ELSE TcOutbox(node, CHOOSE tc \in certificates: TRUE)

RestartRetainedControl(node) ==
  LET cleared ==
        {item \in asyncRetainedControl: item.source # node}
      withPrepare ==
        RememberedControl(cleared, RestartHighestPrepareControl(node))
      withDecision ==
        RememberedControl(withPrepare, RestartDecisionControl(node))
  IN RememberedControl(withDecision, RestartLastTCControl(node))

(***************************************************************************
Same-height restart reconstruction.

Ingress occurrences, uncommitted admissions, and reservations are volatile
and are cleared for the restarted node.  Monotone ordinals and terminal
response tombstones are not reset: production reconstructs this bounded
high-watermark state from the durable Core/body/certificate catalog before
reopening ingress.  A displaced lower-view tombstone temporarily carried by
an in-flight reservation is restored as that reservation is abandoned.  Thus
a drained logical request cannot re-enter its old Serve stage after replay.
Only a fresh successor-height Async instance resets the tables in
`AsyncIoInit`.
***************************************************************************)
AsyncServeRestartRecoveredTombstones(node) ==
  asyncServeTombstones
    \cup
  UNION {
    reservation.rollbackTombstones:
      reservation \in
        {owned \in asyncServeReservations: owned.node = node}}

ResetNodeSchedulerForRestart(node, replay) ==
  /\ node \in AsyncActiveServiceNodes
  /\ UNCHANGED asyncServiceActivationState
  /\ asyncNow' = asyncNow
  /\ asyncCommandQueues' =
       [asyncCommandQueues EXCEPT ![node] = <<>>]
  /\ asyncNextCommandClass' =
       [asyncNextCommandClass EXCEPT ![node] = "Completion"]
  /\ asyncFifoOwed' = [asyncFifoOwed EXCEPT ![node] = FALSE]
  /\ asyncTimeoutEmitted' =
       [asyncTimeoutEmitted EXCEPT ![node] = FALSE]
  /\ asyncRunnerPhase' =
       [asyncRunnerPhase EXCEPT ![node] = "Local"]
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = AsyncQueueCapacity]
  /\ asyncCausalAdmissionOwed' =
       [asyncCausalAdmissionOwed EXCEPT ![node] = FALSE]
  /\ asyncNextLocalSource' =
       [asyncNextLocalSource EXCEPT ![node] = "Producer"]
  /\ asyncIoQueues' = [asyncIoQueues EXCEPT ![node] = <<>>]
  /\ UNCHANGED <<asyncNextServeAdmissionOrdinal,
                  asyncNextServeIngressOrdinal>>
  /\ asyncServeIngressAdmissions' =
       AsyncServeIngressAdmissionsWithoutNode(node)
  /\ asyncServeAdmissions' = AsyncServeAdmissionsWithoutNode(node)
  /\ asyncServeReservations' = AsyncServeReservationsWithoutNode(node)
  /\ asyncServeTombstones' =
       AsyncServeRestartRecoveredTombstones(node)
  /\ asyncOutstandingWork' =
       [asyncOutstandingWork EXCEPT ![node] = {}]
  /\ asyncIoReadyCompletions' =
       [asyncIoReadyCompletions EXCEPT ![node] = <<>>]
  /\ asyncLocalReadyCompletions' =
       [asyncLocalReadyCompletions EXCEPT ![node] = <<>>]
  /\ asyncNextCompletionSource' =
       [asyncNextCompletionSource EXCEPT ![node] = "Io"]
  /\ asyncIoControlAvailable' =
       [asyncIoControlAvailable EXCEPT ![node] = TRUE]
  /\ asyncDeferredCompletionQueues' =
       [asyncDeferredCompletionQueues EXCEPT ![node] = <<>>]
  /\ asyncDeferredProgressQueues' =
       [asyncDeferredProgressQueues EXCEPT ![node] = <<>>]
  /\ asyncDeferredNormalQueues' =
       [asyncDeferredNormalQueues EXCEPT ![node] = <<>>]
  /\ asyncDeferredHandoffs' =
       [asyncDeferredHandoffs EXCEPT ![node] = NoAsyncDeferredHandoff]
  /\ asyncNextDeferredClass' =
       [asyncNextDeferredClass EXCEPT ![node] = "Completion"]
  /\ asyncDeferredDrainOwed' =
       [asyncDeferredDrainOwed EXCEPT ![node] = FALSE]
  /\ asyncCausalQueues' =
       [asyncCausalQueues EXCEPT
          ![node] = FreshRestartCandidateSequence(replay)]
  /\ asyncOutstandingTags' =
       [asyncOutstandingTags EXCEPT ![node] = {}]
  /\ asyncNodeDeadlines' =
       [asyncNodeDeadlines EXCEPT
          ![node] = asyncNow + AsyncViewTimeout(nodeView[node])]
  /\ asyncRetransmitDeadlines' =
       [asyncRetransmitDeadlines EXCEPT
          ![node] = asyncNow + AsyncRetransmitPeriod]
  /\ asyncNodeServiceDeadlines' =
       [asyncNodeServiceDeadlines EXCEPT
          ![node] = asyncNow + AsyncDeliveryBound]
  /\ asyncIoServiceDeadlines' =
       [asyncIoServiceDeadlines EXCEPT
          ![node] = asyncNow + AsyncDeliveryBound]
  /\ asyncSentItems' = asyncSentItems
  /\ asyncRetainedControl' = RestartRetainedControl(node)
  \* Same-height recovery reconstructs an outstanding certified-body request
  \* from the durable Decision or retained-lock source.  It therefore keeps
  \* the existing max-certified-requests charge and exact signed request hash
  \* instead of allocating a second request lifecycle after replay.
  /\ asyncActiveRequests' = RestartRetainedActiveRequests(node)
  \* The authenticated response occurrence is process-local and volatile.
  \* Recovery retires it even when its outstanding request survives, reopening
  \* that request-hash family for a deterministic later responder.  The claim
  \* ordinal high-watermark lives in `asyncControlServiceState` and is retained
  \* by the global exact-wire transition below.
  /\ asyncCertifiedResponseClaim' =
       CertifiedResponseClaimForRequestsExceptRecipient(
         asyncActiveRequests', node)
  /\ asyncTransport' = asyncTransport
  /\ asyncIngressLanes' =
       [asyncIngressLanes EXCEPT
          ![node] = [source \in AsyncIngressSources |-> <<>>]]
  /\ asyncIngressReady' = [asyncIngressReady EXCEPT ![node] = <<>>]
  /\ asyncHeldChunks' =
       {receipt \in asyncHeldChunks: receipt.node # node}
  /\ asyncHistoricalRecoveryTargets' =
       asyncHistoricalRecoveryTargets \ {node}

THEOREM SameHeightRestartPreservesServeHighWatermarks ==
  \A node, replay:
    ResetNodeSchedulerForRestart(node, replay)
      => /\ asyncNextServeAdmissionOrdinal'
              = asyncNextServeAdmissionOrdinal
         /\ asyncNextServeIngressOrdinal'
              = asyncNextServeIngressOrdinal
         /\ asyncServeTombstones \subseteq asyncServeTombstones'
BY Isa
   DEF ResetNodeSchedulerForRestart,
       AsyncServeRestartRecoveredTombstones

THEOREM SameHeightRestartReopensDurableCertifiedResponseFamily ==
  \A node, replay, request:
    /\ request \in asyncActiveRequests
    /\ RestartDurableCertifiedRequest(node, request)
    /\ ResetNodeSchedulerForRestart(node, replay)
    => /\ request \in asyncActiveRequests'
       /\ CertifiedResponseClaimsAt(node)' = {}
BY Isa
   DEF ResetNodeSchedulerForRestart,
       RestartRetainedActiveRequests,
       RestartDurableCertifiedRequest,
       CertifiedResponseClaimForRequestsExceptRecipient,
       CertifiedResponseClaimForRequests,
       CertifiedResponseClaimsAt

AsyncSetGST ==
  /\ ~gst
  /\ asyncRecoveryPhase
       \notin {"RestartRequired", "ReplayRequired", "Replaying"}
  /\ Responsive \subseteq up
  /\ Responsive \subseteq AsyncActiveServiceNodes
  /\ SetGST
  /\ UNCHANGED <<AsyncSchedulerVars, AsyncRecoveryVars>>
  /\ AsyncNonRunnerOuterFrame

(***************************************************************************
Faults outside the trusted product loop.  Before GST packets may be lost and
non-responsive validators may crash.  Byzantine noise is bounded in its own
authenticated source lane and cannot occupy an honest source's slots.
***************************************************************************)

AsyncServeIngressOccurrences(node, identity) ==
  UNION {
    {[source |-> source, index |-> index]:
       index \in
         {position \in 1..Len(asyncIngressLanes[node][source]):
            /\ asyncIngressLanes[node][source][position].kind
                 \in AsyncReplyRequestKinds
            /\ AsyncServeLogicalRequestIdentity(
                 node, asyncIngressLanes[node][source][position])
                 = identity}}:
    source \in AsyncIngressSources}

AsyncServeReadyIndicesForSource(node, source) ==
  {index \in 1..Len(asyncIngressReady[node]):
     asyncIngressReady[node][index] = source}

(***************************************************************************
The Rust command channel can close after atomic Serve preparation but before
the fair-ingress owner commits it.  Rollback covers both representations of
that uncommitted transaction: an off-queue PendingCapacity future slot, or a
materialized but unclaimed Reserved placeholder.  A higher-view preparation
may have displaced a terminal family tombstone; the reservation carries that
exact tombstone internally and rollback restores it atomically.  The admission
ordinal is never reused.  This is a pre-GST teardown cut only: after GST a
responsive dual quorum retains its receiver, while a completed/tombstoned
lifecycle is never eligible for rollback (including when its current physical
reply-route set is temporarily empty).
***************************************************************************)
PreGstPendingServeReceiverCloseRollback(node, identity) ==
  LET occurrence ==
        CHOOSE owned \in AsyncServeIngressOccurrences(node, identity): TRUE
      source == occurrence.source
      laneIndex == occurrence.index
      readyIndex ==
        CHOOSE index \in
          AsyncServeReadyIndicesForSource(node, source): TRUE
      drainedReservations ==
        AsyncServeReservationsAfterIngressDrain(
          node, source, laneIndex)
  IN /\ ~gst
     /\ AsyncServeLiveReservationOwned(node, identity)
     /\ ~AsyncServeJobQueued(node, identity)
     /\ Cardinality(
          AsyncServeIngressOccurrences(node, identity)) = 1
     /\ Cardinality(
          AsyncServeReadyIndicesForSource(node, source)) = 1
     /\ asyncIngressLanes' =
          [asyncIngressLanes EXCEPT
             ![node][source] =
               SequenceWithoutIndex(@, laneIndex)]
     /\ asyncIngressReady' =
          [asyncIngressReady EXCEPT
             ![node] = ReadyAfterSelectedDrain(node, readyIndex)]
     /\ UNCHANGED asyncIoQueues
     /\ asyncServeAdmissions' =
          AsyncServeAdmissionsWithout(node, identity)
     /\ asyncServeIngressAdmissions' =
          AsyncServeIngressAdmissionsWithout(node, identity)
     /\ asyncServeReservations' =
          AsyncServeReservationsWithoutFrom(
            drainedReservations, node, identity)
     /\ asyncServeTombstones' =
          asyncServeTombstones
            \cup AsyncServeRollbackTombstones(node, identity)
     /\ UNCHANGED <<asyncNextServeAdmissionOrdinal,
                     asyncNextServeIngressOrdinal>>
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncLocalAdmissionVars,
                    asyncOutstandingWork, asyncIoReadyCompletions,
                    asyncLocalReadyCompletions,
                    asyncNextCompletionSource,
                    asyncIoControlAvailable, AsyncDeferredVars,
                    asyncCausalQueues, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncSentItems, asyncRetainedControl,
                    asyncActiveRequests, asyncCertifiedResponseClaim,
                    asyncTransport, asyncHeldChunks,
                    asyncHistoricalRecoveryTargets>>

PreGstMaterializedServeReceiverCloseRollback(node, identity) ==
  LET jobIndex ==
        CHOOSE index \in
          AsyncIoServeIdentityIndices(node, identity): TRUE
      job == asyncIoQueues[node][jobIndex]
  IN /\ ~gst
     /\ AsyncServeLiveReservationOwned(node, identity)
     /\ Cardinality(
          AsyncIoServeIdentityIndices(node, identity)) = 1
     /\ asyncIoQueues' =
          [asyncIoQueues EXCEPT
             ![node] = SequenceWithoutIndex(@, jobIndex)]
     /\ asyncServeAdmissions' =
          AsyncServeAdmissionsWithout(node, identity)
     /\ UNCHANGED AsyncServeIngressAdmissionVars
     /\ asyncServeReservations' =
          AsyncServeReservationsAfterIoService(
            node, job, identity)
     /\ asyncServeTombstones' =
          asyncServeTombstones
            \cup AsyncServeRollbackTombstones(node, identity)
     /\ UNCHANGED asyncNextServeAdmissionOrdinal
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncLocalAdmissionVars,
                    asyncOutstandingWork, asyncIoReadyCompletions,
                    asyncLocalReadyCompletions,
                    asyncNextCompletionSource,
                    asyncIoControlAvailable, AsyncDeferredVars,
                    asyncCausalQueues, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncSentItems, asyncRetainedControl,
                    asyncActiveRequests, asyncCertifiedResponseClaim,
                    asyncTransport, asyncIngressLanes,
                    asyncIngressReady, asyncHeldChunks,
                    asyncHistoricalRecoveryTargets>>

PreGstServeReceiverCloseRollback(node, identity) ==
  \/ PreGstPendingServeReceiverCloseRollback(node, identity)
  \/ PreGstMaterializedServeReceiverCloseRollback(node, identity)

PreGstLosePacket(packet) ==
  /\ ~gst
  /\ packet \in asyncTransport
  /\ asyncTransport' = asyncTransport \ {packet}
  /\ UNCHANGED AsyncDeferredVars
  /\ LeaveCausalQueues
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                 asyncNextCommandClass, asyncFifoOwed,
                 asyncTimeoutEmitted, asyncRunnerPhase, asyncRunnerBudget,
                 AsyncIoVars, asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncNodeServiceDeadlines,
                 asyncIoServiceDeadlines, asyncSentItems,
                 asyncRetainedControl, asyncActiveRequests,
                 asyncCertifiedResponseClaim, asyncIngressLanes,
                 asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets
                 >>

PreGstCrash(node) ==
  /\ ~gst
  /\ node \notin Responsive
  /\ Crash(node)
  /\ UNCHANGED <<AsyncSchedulerVars, AsyncRecoveryVars>>

PreGstResponsiveCrash(node) ==
  /\ ~gst
  /\ asyncRecoveryPhase = "Eligible"
  /\ node \in AsyncActiveServiceNodes
  /\ node \in Responsive \cap up
  /\ Crash(node)
  /\ asyncRecoveryPhase' = "RestartRequired"
  /\ asyncRecoveryNode' = node
  /\ asyncRecoveryGeneration' = generation[node]
  /\ asyncRecoveryReplayQueue' = <<>>
  /\ UNCHANGED AsyncSchedulerVars

PreGstResponsiveRestart ==
  LET node == asyncRecoveryNode
  IN /\ ~gst
     /\ asyncRecoveryPhase = "RestartRequired"
     /\ node \in Responsive \cap (ValidatorIds \ up)
     /\ generation[node] = asyncRecoveryGeneration
     /\ Restart(node)
     \* A generation-zero tag is safe only across a real process boundary.
     \* Destroy every volatile queue/claim/ingress owner atomically with the
     \* Core restart; the following replay action may reconstruct only
     \* durable, generation-free identities.
     /\ ResetNodeSchedulerForRestart(node, <<>>)
     /\ asyncRecoveryPhase' = "ReplayRequired"
     /\ asyncRecoveryNode' = node
     /\ asyncRecoveryGeneration' = 0
     /\ asyncRecoveryReplayQueue' = asyncRecoveryReplayQueue
     /\ AsyncCoreOuterFrame

RecoveryCoreReplay(node, candidate) ==
  /\ ~AsyncCandidateRestartReplayTombstoned(candidate)
  /\ CASE candidate.kind = "SignProposal" ->
         ResumeProposal(node, candidate.evidence)
    [] candidate.kind = "SignVote" ->
         ResumeVote(node, candidate.evidence)
    [] candidate.kind = "SignTimeout" ->
         ResumeTimeout(node, candidate.evidence)
    [] OTHER -> FALSE

PreGstResponsiveReplay ==
  LET node == asyncRecoveryNode
      signatures == RestartSignatureReplay(node)
      replay == FreshRestartCandidateSequence(RestartReplay(node))
  IN /\ ~gst
     /\ asyncRecoveryPhase = "ReplayRequired"
     /\ node \in Responsive \cap up
     /\ generation[node] = asyncRecoveryGeneration
     /\ NodeIdle(node)
     /\ IF Len(signatures) > 0
        THEN RecoveryCoreReplay(node, Head(signatures))
        ELSE UNCHANGED vars
     /\ ResetNodeSchedulerForRestart(node, replay)
     /\ asyncRecoveryPhase' =
          IF Len(signatures) > 0 THEN "Replaying" ELSE "Recovered"
     /\ asyncRecoveryNode' = node
     /\ asyncRecoveryGeneration' = generation[node]
     /\ asyncRecoveryReplayQueue' =
          IF Len(signatures) > 0 THEN Tail(signatures) ELSE <<>>
     /\ AsyncCoreOuterFrame

DriveResponsiveReplayHead ==
  LET node == asyncRecoveryNode
      candidate == Head(asyncRecoveryReplayQueue)
  IN /\ ~gst
     /\ asyncRecoveryPhase = "Replaying"
     /\ Len(asyncRecoveryReplayQueue) > 0
     /\ node \in Responsive \cap up
     /\ generation[node] = asyncRecoveryGeneration
     /\ NodeIdle(node)
     /\ IF AsyncCandidateRestartReplayTombstoned(candidate)
        THEN UNCHANGED vars
        ELSE RecoveryCoreReplay(node, candidate)
     /\ asyncCausalQueues' =
          [asyncCausalQueues EXCEPT
             ![node] = @ \o FreshCandidateSequence(candidate)]
     /\ asyncRecoveryReplayQueue' = Tail(asyncRecoveryReplayQueue)
     /\ UNCHANGED AsyncRecoveryLifecycleVars
     /\ UNCHANGED <<asyncNow, asyncCommandQueues,
                     asyncNextCommandClass, asyncFifoOwed,
                     asyncTimeoutEmitted, asyncRunnerPhase,
                     asyncRunnerBudget, AsyncLocalAdmissionVars,
                     AsyncIoVars, AsyncDeferredVars,
                     asyncOutstandingTags, asyncNodeDeadlines,
                     asyncRetransmitDeadlines,
                     asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                     asyncSentItems, asyncRetainedControl,
                     asyncActiveRequests, asyncCertifiedResponseClaim,
                     asyncTransport,
                     asyncIngressLanes, asyncIngressReady,
                     asyncHeldChunks, asyncHistoricalRecoveryTargets>>
     /\ AsyncRecoveryOuterFrame

FinishResponsiveReplay ==
  LET node == asyncRecoveryNode
      runner == RestartRunnerAssembly(node)
  IN /\ ~gst
     /\ asyncRecoveryPhase = "Replaying"
     /\ asyncRecoveryReplayQueue = <<>>
     /\ node \in Responsive \cap up
     /\ generation[node] = asyncRecoveryGeneration
     /\ NodeIdle(node)
     /\ ReplayCommitSourcesReady(node)
     /\ UNCHANGED vars
     /\ asyncCausalQueues' =
          IF Len(runner) = 0
          THEN asyncCausalQueues
          ELSE [asyncCausalQueues EXCEPT
                  ![node] = @ \o FreshCandidateSequence(runner[1])]
     /\ asyncRecoveryPhase' = "Recovered"
     /\ asyncRecoveryNode' = node
     /\ asyncRecoveryGeneration' = generation[node]
     /\ asyncRecoveryReplayQueue' = <<>>
     /\ UNCHANGED <<asyncNow, asyncCommandQueues,
                     asyncNextCommandClass, asyncFifoOwed,
                     asyncTimeoutEmitted, asyncRunnerPhase,
                     asyncRunnerBudget, AsyncLocalAdmissionVars,
                     AsyncIoVars, AsyncDeferredVars,
                     asyncOutstandingTags, asyncNodeDeadlines,
                     asyncRetransmitDeadlines,
                     asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                     asyncSentItems, asyncRetainedControl,
                     asyncActiveRequests, asyncCertifiedResponseClaim,
                     asyncTransport,
                     asyncIngressLanes, asyncIngressReady,
                     asyncHeldChunks, asyncHistoricalRecoveryTargets>>
     /\ AsyncRecoveryOuterFrame

RearmResponsiveRecovery ==
  /\ ~gst
  /\ asyncRecoveryPhase = "Recovered"
  /\ Responsive \subseteq up
  /\ asyncRecoveryReplayQueue = <<>>
  /\ asyncRecoveryPhase' = "Eligible"
  /\ asyncRecoveryNode' = 0
  /\ asyncRecoveryGeneration' = 0
  /\ asyncRecoveryReplayQueue' = <<>>
  /\ AsyncHistoricalLockRestartAuthorityTransition
  /\ UNCHANGED <<vars, AsyncSchedulerVars>>

(***************************************************************************
Validation receipts and chunk sessions are deliberately outside the durable
restart frontier in this abstraction.  A durable Prepare/Commit intent is the
post-validation WAL witness consumed by ResumeVote, while chunk assembly is
process-local and is reconstructed through the ordinary body-fetch/validation
pipeline.  A durable Decision is only the completed Decision-WAL frame here;
body recovery, store, validation, application, and successor activation remain
separate modeled stages.  The write/flush/fsync sub-stages before WAL
acknowledgement belong to the implementation/refinement trace, not to a second
consensus replay owner in this module.
***************************************************************************)

InjectByzantineNoise(source, recipient, nonce) ==
  LET envelope ==
        AsyncBodyEnvelope(recipient, context.height, nodeView[recipient],
                          AsyncHeartbeatSubject, NoAsyncChunk, nonce)
      item == AsyncNetworkItem("Noise", source, envelope)
      packet == PacketForItem(item)
  IN /\ source \in (Byzantine(CurrentEpoch) \cap up)
                   \cup {AsyncUntrustedSource}
     /\ recipient \in CurrentVoters
     /\ nonce \in 0..(AsyncIngressCapacity - 1)
     /\ ~ItemScheduled(item)
     /\ packet \notin asyncTransport
     /\ asyncTransport' = asyncTransport \cup {packet}
     /\ UNCHANGED AsyncDeferredVars
     /\ LeaveCausalQueues
     /\ UNCHANGED AsyncLocalAdmissionVars
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncSentItems, asyncRetainedControl,
                    asyncActiveRequests, asyncCertifiedResponseClaim,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets
                    >>

(***************************************************************************
The authenticated resource hop can be the aggregate untrusted lane even when
the completion's semantic origin is a roster validator.  This abstraction has
only `item.source`, so the action represents that relayed completion and spends
the untrusted lane's isolated TransportCompletion owner.  It is not added to
immutable authentication history and therefore remains discardable after fair
admission and service; no validator source owner is consumed.  Nonce zero is
the canonical representative because payload identity does not affect the
resource-hop count gate, and collapsing the other finite nonce aliases avoids
multiplying equivalent fault states.  The production origin/via authentication
premise remains an explicit refinement obligation.
***************************************************************************)
InjectUntrustedTransportCompletion(kind, recipient, nonce) ==
  LET item ==
        AsyncUntrustedTransportCompletionItem(kind, recipient, nonce)
      packet == PacketForItem(item)
  IN /\ kind \in IngressTransportCompletionKinds
     /\ recipient \in CurrentVoters
     /\ nonce \in 0..(AsyncIngressCapacity - 1)
     /\ nonce = 0
     /\ ~ItemScheduled(item)
     /\ packet \notin asyncTransport
     /\ asyncTransport' = asyncTransport \cup {packet}
     /\ UNCHANGED AsyncDeferredVars
     /\ LeaveCausalQueues
     /\ UNCHANGED AsyncLocalAdmissionVars
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncSentItems, asyncRetainedControl,
                    asyncActiveRequests, asyncCertifiedResponseClaim,
                    asyncIngressLanes,
                    asyncIngressReady, asyncHeldChunks,
                    asyncHistoricalRecoveryTargets>>

InjectAuthenticatedJunk(kind, source, recipient, nonce) ==
  LET envelope ==
        AsyncBodyEnvelope(recipient, context.height, nodeView[recipient],
                          AsyncHeartbeatSubject, NoAsyncChunk, nonce)
      item == AsyncNetworkItem(kind, source, envelope)
      packet == PacketForItem(item)
  IN /\ kind \in {"NormalJunk", "ProgressJunk"}
     /\ source \in Byzantine(CurrentEpoch) \cap up
     /\ recipient \in CurrentVoters
     /\ nonce \in 0..(AsyncIngressCapacity - 1)
     /\ ~ItemScheduled(item)
     /\ packet \notin asyncTransport
     /\ asyncSentItems' = asyncSentItems \cup {item}
     /\ asyncTransport' = asyncTransport \cup {packet}
     /\ UNCHANGED <<asyncRetainedControl, asyncActiveRequests,
                     asyncCertifiedResponseClaim>>
     /\ UNCHANGED AsyncDeferredVars
     /\ LeaveCausalQueues
     /\ UNCHANGED AsyncLocalAdmissionVars
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets
                    >>

InjectByzantineCertifiedRequest(source, recipient, qc, nonce) ==
  LET envelope ==
        AsyncCertifiedRequestEnvelope(recipient, source, qc, nonce)
      item == AsyncNetworkItem("CertifiedRequest", source, envelope)
      packet == PacketForItem(item)
  IN /\ source \in Byzantine(CurrentEpoch) \cap up
     /\ recipient
          \in CertifiedArchiveRoutes(source, qc)
               \cap AsyncArchiveIoServiceNodes
     /\ qc \in commitQCs
     /\ nonce \in 0..(AsyncIngressCapacity - 1)
     /\ ~ItemScheduled(item)
     /\ packet \notin asyncTransport
     /\ asyncSentItems' = asyncSentItems \cup {item}
     /\ asyncTransport' = asyncTransport \cup {packet}
     /\ UNCHANGED <<asyncRetainedControl, asyncActiveRequests,
                     asyncCertifiedResponseClaim>>
     /\ UNCHANGED AsyncDeferredVars
     /\ LeaveCausalQueues
     /\ UNCHANGED AsyncLocalAdmissionVars
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets
                    >>

AsyncByzantineProposal(signer, roundView, subject,
                       timeoutCertificate, highestPrepare) ==
  LET proposal == Proposal(context, roundView, subject, signer,
                           timeoutCertificate, highestPrepare)
  IN /\ ByzantineBroadcastProposal(signer, roundView, subject,
                                    timeoutCertificate, highestPrepare)
     /\ PublishEphemeralItems(ByzantineProposalOutbox(signer, proposal))
     /\ UNCHANGED AsyncLocalAdmissionVars
     /\ UNCHANGED <<asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, AsyncDeferredVars,
                    asyncCausalQueues, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets
                    >>

AsyncByzantineVote(signer, roundView, phase, subject) ==
  LET vote == Vote(context, roundView, phase, subject, signer)
  IN /\ ByzantineBroadcastVote(signer, roundView, phase, subject)
     /\ PublishEphemeralItems(ByzantineVoteOutbox(signer, vote))
     /\ UNCHANGED AsyncLocalAdmissionVars
     /\ UNCHANGED <<asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, AsyncDeferredVars,
                    asyncCausalQueues, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets
                    >>

AsyncByzantineTimeout(signer, roundView, highestPrepare) ==
  LET vote == TimeoutVote(context, roundView, signer, highestPrepare)
  IN /\ ByzantineBroadcastTimeout(signer, roundView, highestPrepare)
     /\ PublishEphemeralItems(ByzantineTimeoutOutbox(signer, vote))
     /\ UNCHANGED AsyncLocalAdmissionVars
     /\ UNCHANGED <<asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, AsyncDeferredVars,
                    asyncCausalQueues, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets
                    >>

AsyncFaultStep ==
  \/ \E packet \in asyncTransport: PreGstLosePacket(packet)
  \/ \E reservation \in asyncServeReservations:
       PreGstServeReceiverCloseRollback(
         reservation.node, reservation.identity)
  \/ \E node \in ValidatorIds: PreGstCrash(node)
  \/ \E source \in AsyncIngressSources, recipient \in ValidatorIds,
       nonce \in 0..(AsyncIngressCapacity - 1):
       InjectByzantineNoise(source, recipient, nonce)
  \/ \E kind \in IngressTransportCompletionKinds,
       recipient \in ValidatorIds,
       nonce \in 0..(AsyncIngressCapacity - 1):
       InjectUntrustedTransportCompletion(kind, recipient, nonce)
  \/ \E kind \in {"NormalJunk", "ProgressJunk"},
       source \in ValidatorIds, recipient \in ValidatorIds,
       nonce \in 0..(AsyncIngressCapacity - 1):
       InjectAuthenticatedJunk(kind, source, recipient, nonce)
  \/ \E source \in ValidatorIds, recipient \in ValidatorIds,
       qc \in commitQCs, nonce \in 0..(AsyncIngressCapacity - 1):
       InjectByzantineCertifiedRequest(source, recipient, qc, nonce)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       subject \in Subjects,
       timeoutCertificate \in TimeoutCertificateOptionSet,
       highestPrepare \in PrepareQcOptionSet:
       AsyncByzantineProposal(signer, roundView, subject,
                              timeoutCertificate, highestPrepare)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       phase \in Phases, subject \in Subjects:
       AsyncByzantineVote(signer, roundView, phase, subject)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       highestPrepare \in PrepareQcOptionSet:
       AsyncByzantineTimeout(signer, roundView, highestPrepare)

AsyncNetworkStep ==
  \E recipient \in ValidatorIds, source \in AsyncIngressSources:
    AdmitIngressPacket(recipient, source)

(***************************************************************************
An authorized exact-reply request which has no lifecycle and cannot yet be
answered remains in transport, but it does not own the clock deadline.  The
Serve transport gate deliberately excludes physical capacity and selector
barriers: a serviceable request blocked only by those finite owners therefore
still owns its deadline.  The retained packet keeps its original deadline;
as soon as serviceability or lifecycle ownership opens the gate, it
immediately re-enters the overdue corridor.
***************************************************************************)
AsyncDormantExactReplyRequestPacket(packet) ==
  /\ packet \in asyncTransport
  /\ ~AsyncServeTransportAdmissionGateAllows(
       packet.item.envelope.recipient, packet.item)

(***************************************************************************
The clock waits for bounded post-GST work between timed responsive service
nodes.  A certified response can use an independent relay lane, so its
authenticated envelope occurrence—not the outer `item.source`—places it in
the same deadline corridor.  Commit-certificate responses retain their exact
sent occurrence.  Unauthenticated aggregate-lane fault traffic cannot hold
the clock indefinitely.
***************************************************************************)
AsyncPacketOwnsClockDeadline(packet) ==
  /\ packet \in asyncTransport
  /\ AsyncServeTransportAdmissionGateAllows(
       packet.item.envelope.recipient, packet.item)
  /\ packet.item.envelope.recipient \in AsyncTimedServiceNodes
  /\ \/ packet.item.source \in AsyncTimedServiceNodes
     \/ /\ packet.item.kind
              \in {"CertifiedResponse",
                   "CommitCertificateResponse"}
           /\ IngressItemHasAuthenticatedHistory(packet.item)
  /\ packet.deadline <= asyncNow

OverdueResponsivePackets ==
  {packet \in asyncTransport:
     AsyncPacketOwnsClockDeadline(packet)}

THEOREM AsyncDormantExactReplyRequestPacketIsRetained ==
  \A packet:
    AsyncDormantExactReplyRequestPacket(packet)
      => packet \in asyncTransport
BY DEF AsyncDormantExactReplyRequestPacket

THEOREM AsyncGateOpenDueResponsivePacketReentersClockDeadline ==
  \A packet:
    /\ packet \in asyncTransport
    /\ AsyncServeTransportAdmissionGateAllows(
         packet.item.envelope.recipient, packet.item)
    /\ packet.item.envelope.recipient \in AsyncTimedServiceNodes
    /\ \/ packet.item.source \in AsyncTimedServiceNodes
       \/ /\ packet.item.kind
                \in {"CertifiedResponse",
                     "CommitCertificateResponse"}
             /\ IngressItemHasAuthenticatedHistory(packet.item)
    /\ packet.deadline <= asyncNow
    => /\ AsyncPacketOwnsClockDeadline(packet)
       /\ packet \in OverdueResponsivePackets
BY DEF AsyncPacketOwnsClockDeadline, OverdueResponsivePackets

(***************************************************************************
Endpoint-neutral retained CommitQC corridor.

These predicates retain the exact `AsyncNetworkItem("CommitQC", source,
QcEnvelope(recipient, qc))` lineage while exposing only ordinary async
owners.  The four lemmas below are action-local safety facts: they do not
select a source, runner, or command and therefore introduce no fairness or
historical-recovery dependency.  A temporal caller supplies the matching
fair action and composes these concrete handoffs with its own endpoint.
***************************************************************************)
AsyncExactCommitQcItem(item) ==
  /\ item \in AsyncNetworkItems
  /\ item.kind = "CommitQC"
  /\ item.envelope.qc.phase = "Commit"
  /\ item = AsyncNetworkItem(
       "CommitQC", item.source,
       QcEnvelope(item.envelope.recipient, item.envelope.qc))

AsyncExactCommitQcRetainedOwner(item) ==
  /\ AsyncExactCommitQcItem(item)
  /\ item \in asyncRetainedControl

AsyncExactCommitQcPacketOwner(item, packet) ==
  /\ AsyncExactCommitQcRetainedOwner(item)
  /\ item \in asyncSentItems
  /\ packet \in asyncTransport
  /\ packet.item = item

AsyncExactCommitQcIngressOwner(item) ==
  /\ AsyncExactCommitQcRetainedOwner(item)
  /\ item \in asyncSentItems
  /\ item \in SequenceSet(
       IngressLane(
         item.envelope.recipient, IngressResourceSource(item)))

AsyncExactCommitQcDeliverOwner(item) ==
  LET candidate == DeliveryCandidate(item)
  IN /\ AsyncExactCommitQcRetainedOwner(item)
     /\ item \in asyncSentItems
     /\ candidate \in AsyncCandidateSet
     /\ candidate.node = item.envelope.recipient
     /\ candidate.kind = "DeliverQC"
     /\ candidate.item = item
     /\ CandidateConsumerCurrent(candidate)
     /\ CandidateScheduled(candidate)

AsyncExactCommitQcReceipt(item) ==
  /\ AsyncExactCommitQcItem(item)
  /\ QcAt(item.envelope.recipient, item.envelope.qc) \in receivedQCs

THEOREM AsyncRetainedCommitQcRetransmissionCreatesExactPacket ==
  \A node \in ValidatorIds:
    \A item:
      LET packet == PacketForItem(item)
      IN /\ AsyncExactCommitQcRetainedOwner(item)
         /\ item.source = node
         /\ UNCHANGED vars
         /\ SendNodeRetransmissions(node)
         => AsyncExactCommitQcPacketOwner(item, packet)'
BY IsaT(120)
   DEF AsyncExactCommitQcRetainedOwner,
       AsyncExactCommitQcPacketOwner,
       SendNodeRetransmissions, RetryableItems,
       RetainedControlEmissionItems, SendableItems,
       PacketsForItems, PacketForItem

THEOREM AsyncRetainedCommitQcPacketAdmissionCreatesExactIngressOwner ==
  \A item, packet:
    LET recipient == item.envelope.recipient
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncExactCommitQcPacketOwner(item, packet)
       /\ packet = OldestDueSourcePacket(recipient, item.source)
       /\ ~IngressHasCoalescingOwner(item)
       /\ ~IngressPacketPolicyRejected(item)
       /\ AdmitIngressPacket(recipient, item.source)
       => AsyncExactCommitQcIngressOwner(item)'
BY IsaT(240)
   DEF AsyncExactCommitQcPacketOwner,
       AsyncExactCommitQcIngressOwner,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       IngressLane, IngressResourceSource, SequenceSet,
       DueSourcePackets, OldestDueSourcePacket,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant

THEOREM AsyncRetainedCommitQcIngressCreatesExactDeliverQcOwner ==
  \A item:
    LET node == item.envelope.recipient
        candidate == DeliveryCandidate(item)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncExactCommitQcIngressOwner(item)
       /\ SelectedIngressItemAt(
            node, FirstDrainableIngressIndex(node)) = item
       /\ ~AsyncControlServiceOccurrenceRetired(item)
       /\ ~CandidateAdmissionCoalesced(candidate)
       /\ DrainFairIngressSelected(node)
       => AsyncExactCommitQcDeliverOwner(item)'
BY IsaT(360)
   DEF AsyncExactCommitQcIngressOwner,
       AsyncExactCommitQcDeliverOwner,
       AsyncExactCommitQcRetainedOwner,
       AsyncExactCommitQcItem,
       DrainFairIngressSelected, EnqueueCandidate,
       CandidateScheduled, CandidateScheduledIn,
       CandidateConsumerCurrent, DeliveryCandidate,
       IngressLane, IngressResourceSource, SequenceSet,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant

THEOREM AsyncRetainedCommitQcDeliveryRecordsExactReceipt ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ AsyncExactCommitQcRetainedOwner(item)
    /\ ExecuteCoreDelivery(DeliveryCandidate(item))
    => AsyncExactCommitQcReceipt(item)'
BY IsaT(180)
   DEF AsyncExactCommitQcRetainedOwner,
       AsyncExactCommitQcItem, AsyncExactCommitQcReceipt,
       DeliveryCandidate, ExecuteCoreDelivery,
       DeliverQC, QcDeliveryCreatesReceipt, QcAt

(***************************************************************************
The indexed height product begins from the exact standalone initializer.  Its
first independent join takes this one-shot restriction arm in the same global
transition which publishes the join.  The irreversible `restricted` bit
prevents a later all-active state from recreating the restriction episode.
Standalone behavior may take the same exact AsyncNext arm, but per-responsive
activation fairness below monotonically restores every responsive owner.
***************************************************************************)
AsyncEnterIndexedServiceActivation(node) ==
  /\ node \in ValidatorIds
  /\ ~AsyncServiceActivationRestricted
  /\ AsyncActiveServiceNodes = ValidatorIds
  /\ AsyncServiceActivationClockPristine
  /\ asyncNodeServiceDeadlines' =
       [owner \in ValidatorIds |->
          IF owner = node THEN AsyncDeliveryBound ELSE 0]
  /\ asyncIoServiceDeadlines' =
       [owner \in ValidatorIds |->
          IF owner = node THEN AsyncDeliveryBound ELSE 0]
  /\ asyncServiceActivationState' =
       [restricted |-> TRUE, activeNodes |-> {node}]
  /\ AsyncHistoricalLockRestartAuthorityTransition
  /\ AsyncControlServiceSlotTransition
  /\ UNCHANGED AsyncServiceActivationFrameVars

(***************************************************************************
This is the sole zero-to-armed service transition.  It restores both local
deadline carriers atomically and monotonically extends the internal active
set; no retry, crash, runner, I/O, or clock action can activate a second node.
***************************************************************************)
AsyncActivateServiceNode(node) ==
  /\ node \in ValidatorIds \ AsyncActiveServiceNodes
  /\ AsyncServiceActivationRestricted
  /\ asyncNodeServiceDeadlines[node] = 0
  /\ asyncIoServiceDeadlines[node] = 0
  /\ asyncNodeServiceDeadlines' =
       [asyncNodeServiceDeadlines EXCEPT
          ![node] = asyncNow + AsyncDeliveryBound]
  /\ asyncIoServiceDeadlines' =
       [asyncIoServiceDeadlines EXCEPT
          ![node] = asyncNow + AsyncDeliveryBound]
  /\ asyncServiceActivationState' =
       [asyncServiceActivationState EXCEPT
          !.activeNodes = @ \cup {node}]
  /\ AsyncHistoricalLockRestartAuthorityTransition
  /\ AsyncControlServiceSlotTransition
  /\ UNCHANGED AsyncServiceActivationFrameVars

AsyncServiceActivationTransition ==
  \/ \E node \in ValidatorIds:
       AsyncEnterIndexedServiceActivation(node)
  \/ \E node \in ValidatorIds:
       AsyncActivateServiceNode(node)
  \/ UNCHANGED asyncServiceActivationState

AsyncTickEnabled ==
  \/ ~gst
  \/ /\ gst
     /\ OverdueResponsivePackets = {}
     /\ \A node \in AsyncTimedServiceNodes:
          /\ asyncNodeServiceDeadlines[node] > asyncNow
          /\ \/ AsyncIoQueueDepth(node) = 0
             \/ asyncIoServiceDeadlines[node] > asyncNow

AsyncNonClockVars ==
  <<vars, asyncCommandQueues, asyncNextCommandClass,
    asyncFifoOwed, asyncTimeoutEmitted,
    asyncRunnerPhase, asyncRunnerBudget, AsyncLocalAdmissionVars, AsyncIoVars,
    asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncDeferredHandoffs,
    asyncNextDeferredClass,
    asyncDeferredDrainOwed, asyncCausalQueues,
    asyncOutstandingTags, asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
    asyncSentItems, asyncRetainedControl, asyncActiveRequests,
    asyncCertifiedResponseClaim,
    asyncTransport, asyncIngressLanes, asyncIngressReady,
    asyncHeldChunks, asyncHistoricalRecoveryTargets>>

AsyncTick ==
  /\ AsyncTickEnabled
  /\ asyncNow' = asyncNow + 1
  /\ UNCHANGED AsyncNonClockVars
  /\ AsyncNonRunnerOuterFrame

AsyncRunnerStep ==
  \/ (\E node \in AsyncCurrentResponsiveVoters: RunNode(node))
  \/ (\E node \in asyncHistoricalRecoveryTargets:
        RunHistoricalRecoveryNode(node))
  \/ (\E node \in AsyncResponsiveAppliedArchiveServers:
        RunHistoricalServer(node))

AsyncNonRunnerStep ==
  /\ \/ AsyncSetGST
     \/ AsyncTick
     \/ (\E node \in ValidatorIds: OpenHistoricalRecovery(node))
     \/ (\E node \in AsyncCurrentResponsiveVoters:
           DirectCommitCertificateDiscoveryStep(node))
     \/ (\E node \in asyncHistoricalRecoveryTargets:
           DirectHistoricalCommitCertificateDiscoveryStep(node))
     \/ (\E node \in AsyncArchiveIoServiceNodes:
           ServiceIoWorker(node))
     \/ (\E node \in asyncHistoricalRecoveryTargets:
           ServiceHistoricalRecoveryIoWorker(node))
     \/ (\E node \in AsyncCurrentResponsiveVoters:
           EnqueueIoLocalControl(node))
     \/ (\E node \in asyncHistoricalRecoveryTargets:
           EnqueueHistoricalRecoveryIoLocalControl(node))
     \/ AsyncNetworkStep
     \/ AsyncFaultStep
  /\ UNCHANGED asyncNodeServiceDeadlines

AsyncNonCrashStep ==
  \/ /\ (AsyncRunnerStep \/ AsyncNonRunnerStep)
     /\ UNCHANGED <<up, AsyncRecoveryControlVars>>
     /\ AsyncHistoricalLockRestartAuthorityTransition
  \/ /\ (DriveResponsiveReplayHead \/ FinishResponsiveReplay)
     /\ UNCHANGED up
  \/ /\ RearmResponsiveRecovery
     /\ UNCHANGED up

(***************************************************************************
One global frame owns the bounded control-slot table, the recipient-local
certified-response claim metadata, transient candidate service markers, and
terminal candidate tombstones.  Transient markers are generation-scoped and
cleared by responsive replay; terminal tombstones alone are restart-stable.
The three process-local signature-completion markers are reconstructed when
their durable intents are replayed.

Admission allocates or replaces one slot before the packet can enter the
physical ingress lane.  The per-height ordinal is therefore frozen before
Runtime, Completion, Control, or producer work can run.  Core delivery flips
only the matching record's consumed bit.  Same-height replay preserves the
bounded slot and ordinal table, but marks every slot whose volatile ingress
was cleared as consumed.  That retirement treats an admission which never
reached Core as a permitted pre-GST loss and prevents its retry from
coalescing behind a dead owner.  Only a fresh successor-height instance
receives the empty state from `AsyncTransportInit`.

A certified response uses the already-charged outstanding `requestHash`
family.  Admission records the exact `(requestHash, authenticated responder,
response hash)` occurrence and an immutable recipient-local ordinal; it does
not reserve a second request-capacity unit.  Consumption filters the record
with the active request.  Same-height replay keeps the ordinal high-watermark
but retires the volatile occurrence, so a surviving durable request family can
accept a later responsive archive rather than pinning recovery to a crashed
responder.

Candidate retirement allocates one route-neutral immutable identity and
per-node ordinal at either successful FIFO/deferred service or a terminal
nondispatchable discard.  Successful service writes only the transient,
generation-tagged marker; terminal discard writes the restart-durable
tombstone.  Admission coalesces against either class in the live generation.
Same-height responsive replay clears every transient marker for its node
while preserving the ordinal high-watermark and durable tombstones, so
volatile proposal/vote/QC state can be reconstructed.  Strict certified view
advance and durable Decision reclaim either bounded record class only after
the corresponding old-stage admission path is permanently disabled;
successor-height initialization resets the complete table.
***************************************************************************)
AsyncControlServiceResetNodesThisStep ==
  IF PreGstResponsiveRestart \/ PreGstResponsiveReplay
  THEN {asyncRecoveryNode}
  ELSE {}

AsyncControlServiceAdmissionsThisStep ==
  {item \in {wire \in AsyncNetworkItems:
               wire.kind \in AsyncControlKinds}:
     /\ AsyncControlServiceAdmissionStartsOrReplaces(item)
     /\ \E recipient \in ValidatorIds,
           source \in AsyncIngressSources:
          /\ DueSourcePackets(recipient, source) # {}
          /\ item = OldestDueSourcePacket(recipient, source).item
          /\ AdmitHiddenPacket(recipient, source)}

AsyncControlServicesThisStep ==
  {command.item:
     command \in
       {candidate \in AsyncCandidateSet:
          /\ candidate.item.kind \in AsyncControlKinds
          /\ ExecuteCoreDelivery(candidate)}}

AsyncCertifiedResponseClaimAdmissionsThisStep ==
  {item \in AsyncCertifiedResponseItems:
     /\ CertifiedResponseFreshClaimGateAllows(item)
     /\ \E recipient \in ValidatorIds,
           source \in AsyncIngressSources:
          /\ DueSourcePackets(recipient, source) # {}
          /\ item = OldestDueSourcePacket(recipient, source).item
          /\ AdmitHiddenPacket(recipient, source)}

AsyncCandidateServiceMarkersAfterReset(state, resetNodes) ==
  {record \in state.candidateServiceMarkers:
     record.node \notin resetNodes}

AsyncCandidateServiceTombstonesAfterReset(state, resetNodes) ==
  AsyncCandidateServiceMarkersAfterReset(state, resetNodes)
    \cup state.candidateTerminalTombstones

AsyncControlServiceStateAfterReset(state, resetNodes) ==
  [nextOrdinal |-> state.nextOrdinal,
   slots |->
     {IF record.slot.recipient \in resetNodes
      THEN [record EXCEPT !.consumed = TRUE]
      ELSE record:
        record \in state.slots},
   certifiedResponseNextOrdinal |->
     state.certifiedResponseNextOrdinal,
   certifiedResponseClaims |-> state.certifiedResponseClaims,
   candidateServiceNextOrdinal |->
     state.candidateServiceNextOrdinal,
   candidateServiceMarkers |->
     AsyncCandidateServiceMarkersAfterReset(state, resetNodes),
   candidateTerminalTombstones |->
     state.candidateTerminalTombstones,
   candidateLifecycleNextOrdinal |->
     state.candidateLifecycleNextOrdinal,
   candidateLifecycleAdmissions |->
     state.candidateLifecycleAdmissions,
   timeoutLifecycleOrdinal |->
     state.timeoutLifecycleOrdinal,
   timeoutLifecycleOrigin |->
     state.timeoutLifecycleOrigin]

AsyncControlServiceStateAfterAdmission(state, item) ==
  LET recipient == item.envelope.recipient
      slot ==
        AsyncControlServiceSlot(recipient, item.source, item.kind)
      ordinal == state.nextOrdinal[recipient]
  IN [nextOrdinal |->
        [state.nextOrdinal EXCEPT ![recipient] = @ + 1],
      slots |->
        (state.slots \ {record \in state.slots:
                          record.slot = slot})
          \cup {AsyncControlServiceRecord(item, ordinal, FALSE)},
      certifiedResponseNextOrdinal |->
        state.certifiedResponseNextOrdinal,
      certifiedResponseClaims |-> state.certifiedResponseClaims,
      candidateServiceNextOrdinal |->
        state.candidateServiceNextOrdinal,
      candidateServiceMarkers |->
        state.candidateServiceMarkers,
      candidateTerminalTombstones |->
        state.candidateTerminalTombstones,
      candidateLifecycleNextOrdinal |->
        state.candidateLifecycleNextOrdinal,
      candidateLifecycleAdmissions |->
        state.candidateLifecycleAdmissions,
      timeoutLifecycleOrdinal |->
        state.timeoutLifecycleOrdinal,
      timeoutLifecycleOrigin |->
        state.timeoutLifecycleOrigin]

AsyncControlServiceStateAfterService(state, item) ==
  [nextOrdinal |-> state.nextOrdinal,
   slots |->
     {IF record.identity = AsyncLeaderWireServiceIdentity(item)
      THEN [record EXCEPT !.consumed = TRUE]
      ELSE record:
        record \in state.slots},
   certifiedResponseNextOrdinal |->
     state.certifiedResponseNextOrdinal,
   certifiedResponseClaims |-> state.certifiedResponseClaims,
   candidateServiceNextOrdinal |->
     state.candidateServiceNextOrdinal,
   candidateServiceMarkers |->
     state.candidateServiceMarkers,
   candidateTerminalTombstones |->
     state.candidateTerminalTombstones,
   candidateLifecycleNextOrdinal |->
     state.candidateLifecycleNextOrdinal,
   candidateLifecycleAdmissions |->
     state.candidateLifecycleAdmissions,
   timeoutLifecycleOrdinal |->
     state.timeoutLifecycleOrdinal,
   timeoutLifecycleOrigin |->
     state.timeoutLifecycleOrigin]

AsyncCertifiedResponseClaimStateAfterRetirement(state) ==
  [state EXCEPT
     !.certifiedResponseClaims =
       CertifiedResponseClaimRecordsFor(
         state.certifiedResponseClaims,
         asyncActiveRequests',
         asyncCertifiedResponseClaim')]

AsyncCertifiedResponseClaimStateAfterAdmission(state, item) ==
  LET recipient == item.envelope.recipient
      ordinal == state.certifiedResponseNextOrdinal[recipient]
  IN [state EXCEPT
        !.certifiedResponseNextOrdinal[recipient] = @ + 1,
        !.certifiedResponseClaims =
          @ \cup {AsyncCertifiedResponseClaimRecord(item, ordinal)}]

(***************************************************************************
FIFO or Busy-deferred execution retires the exact candidate only after its
final scheduler carrier is gone.  A reducer command which changes Core state
writes a transient marker.  A dispatchable semantic stutter, and an ignored
ingress/junk/stale occurrence, consume only their finite producer episode and
write no marker.  An ignored exact internal callback may write a durable
tombstone under the narrower eligibility predicate below.  A transfer into
executor work remains scheduled and is not retired.
***************************************************************************)
AsyncCandidateSuccessfullyServicedThisStep(candidate) ==
  /\ CandidateScheduled(candidate)
  /\ ~CandidateScheduledAfter(candidate)
  /\ \/ \E node \in ValidatorIds:
          /\ NodeQueueNonempty(node)
          /\ candidate = NextNodeCommand(node)
          /\ CommandDispatchable(candidate)
          /\ FifoRuntimeStep(node)
     \/ \E node \in ValidatorIds:
          /\ DeferredQueueNonempty(node)
          /\ candidate = NextDeferredCommand(node)
          /\ DeferredHandoffAllowsExecution(node, candidate)
          /\ DeferredDrainStep(node)

AsyncCandidateSemanticallyAppliedThisStep(candidate) ==
  /\ AsyncCandidateSuccessfullyServicedThisStep(candidate)
  /\ vars' # vars

AsyncCandidateServicesThisStep ==
  {candidate \in AsyncCandidateSet:
     /\ candidate.kind \in AsyncCandidateServiceTrackedKinds
     /\ AsyncCandidateSemanticallyAppliedThisStep(candidate)}

AsyncCandidatePhysicallyDiscardedThisStep(candidate) ==
  /\ CandidateScheduled(candidate)
  /\ ~CandidateScheduledAfter(candidate)
  /\ ~CommandDispatchable(candidate)
  /\ \/ \E node \in ValidatorIds:
          /\ NodeQueueNonempty(node)
          /\ candidate = NextNodeCommand(node)
          /\ NodeIdle(node)
          /\ FifoRuntimeStep(node)
     \/ \E node \in ValidatorIds:
          /\ DeferredQueueNonempty(node)
          /\ candidate = NextDeferredCommand(node)
          /\ ~DeferredHandoffAllowsExecution(node, candidate)
          /\ ~DeferredHandoffBlocksExecution(node, candidate)
          /\ DeferredDrainStep(node)

\* Only a reducer-internal callback can need durable no-resurrection memory.
\* Authenticated wire work which becomes stale, policy-rejected, malformed,
\* or otherwise nondispatchable is a finite ignored occurrence: it releases
\* its lifecycle reservation and must not allocate a service-store entry.
AsyncCandidateTerminallyDiscardedThisStep(candidate) ==
  /\ candidate.item = NoAsyncItem
  /\ candidate.kind \in AsyncCandidateServiceTrackedKinds
  /\ AsyncCandidatePhysicallyDiscardedThisStep(candidate)

AsyncCandidateTerminalDiscardsThisStep ==
  {candidate \in AsyncCandidateSet:
     AsyncCandidateTerminallyDiscardedThisStep(candidate)}

THEOREM AsyncCandidateServiceRecordProducersAreTrackedBoundaryKinds ==
  \A candidate \in AsyncCandidateSet:
    candidate \in
      AsyncCandidateServicesThisStep
        \cup AsyncCandidateTerminalDiscardsThisStep
      => candidate.kind \in AsyncCandidateServiceTrackedKinds
BY SMT DEF AsyncCandidateServicesThisStep,
           AsyncCandidateTerminalDiscardsThisStep,
           AsyncCandidateTerminallyDiscardedThisStep

THEOREM AsyncCandidateUntrackedInternalContinuationAllocatesNoServiceRecord ==
  \A candidate \in AsyncCandidateSet:
    candidate.kind \in
      AsyncWorkKinds \ AsyncCandidateServiceTrackedKinds
      => candidate \notin
           AsyncCandidateServicesThisStep
             \cup AsyncCandidateTerminalDiscardsThisStep
BY SMT DEF AsyncCandidateServicesThisStep,
           AsyncCandidateTerminalDiscardsThisStep,
           AsyncCandidateTerminallyDiscardedThisStep

AsyncCandidateIgnoredWithoutApplicationThisStep(candidate) ==
  \/ AsyncCandidatePhysicallyDiscardedThisStep(candidate)
  \/ /\ AsyncCandidateSuccessfullyServicedThisStep(candidate)
     /\ vars' = vars

AsyncCandidateIgnoredWithoutApplicationThisStepSet ==
  {candidate \in AsyncCandidateSet:
     AsyncCandidateIgnoredWithoutApplicationThisStep(candidate)}

AsyncCandidateTerminalRetirementsThisStep ==
  AsyncCandidateTerminalDiscardsThisStep

THEOREM AsyncCandidateCausalAdmissionTransfersSameOwner ==
  \A node \in ValidatorIds:
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ CausalQueueNonempty(node)
    /\ AdmitCausalHead(node)
    => LET candidate == HeadCausalCandidate(node)
       IN /\ CandidateScheduled(candidate)
          /\ CandidateScheduledAfter(candidate)
          /\ candidate \notin AsyncCandidateServicesThisStep
BY IsaT(300)
   DEF AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncLogicalCandidateOwnershipInvariant,
       AdmitCausalHead, CausalHeadCanAdvance,
       CandidateInFlight,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       ConsensusIoCandidates, SequenceSet

THEOREM AsyncCandidateIoCompletionTransfersSameOwner ==
  \A node \in ValidatorIds:
    LET job == Head(asyncIoQueues[node])
    IN /\ AsyncOutstandingCarrierInvariant
       /\ AsyncIoQueueDepth(node) > 0
       /\ job.class = "Consensus"
       /\ ServiceIoWorkerWork(node)
       => /\ CandidateScheduled(job.candidate)
          /\ CandidateScheduledAfter(job.candidate)
          /\ job.candidate \notin AsyncCandidateServicesThisStep
BY IsaT(300)
   DEF AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncOutstandingCarrierInvariant,
       ServiceIoWorkerWork,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn, TrackedWorkCandidates,
       ConsensusIoCandidates, SequenceSet

THEOREM AsyncCandidateProducerCompletionTransfersSameOwner ==
  \A node \in ValidatorIds:
    LET candidate == SelectedCompletionCandidate(node)
    IN /\ AsyncOutstandingCarrierInvariant
       /\ AdmitProducerCompletion(node)
       => /\ CandidateScheduled(candidate)
          /\ CandidateScheduledAfter(candidate)
          /\ candidate \notin AsyncCandidateServicesThisStep
BY IsaT(300)
   DEF AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncOutstandingCarrierInvariant,
       AdmitProducerCompletion,
       EnqueueCandidate,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn, TrackedWorkCandidates,
       ConsensusIoCandidates, SequenceSet

THEOREM AsyncCandidateBusyDeferralTransfersSameOwner ==
  \A node \in ValidatorIds:
    LET candidate == NextNodeCommand(node)
    IN /\ NodeQueueNonempty(node)
       /\ ~CommandDispatchable(candidate)
       /\ ~NodeIdle(node)
       /\ FifoRuntimeStep(node)
       => /\ CandidateScheduled(candidate)
          /\ CandidateScheduledAfter(candidate)
          /\ candidate \notin AsyncCandidateServicesThisStep
BY IsaT(300)
   DEF AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       FifoRuntimeStep, DeferCommand,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn, DeferredCandidates,
       SequenceSet

THEOREM AsyncCandidateDeferredHandoffRetainsSameOwner ==
  \A node \in ValidatorIds:
    LET candidate == NextDeferredCommand(node)
    IN /\ DeferredQueueNonempty(node)
       /\ ~DeferredHandoffAllowsExecution(node, candidate)
       /\ ~NodeIdle(node)
       /\ DeferredDrainStep(node)
       => /\ CandidateScheduled(candidate)
          /\ CandidateScheduledAfter(candidate)
          /\ candidate \notin AsyncCandidateServicesThisStep
BY IsaT(300)
   DEF AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       DeferredDrainStep, InstallDeferredHandoff,
       RetainDeferredHandoffs,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn, DeferredCandidates,
       SequenceSet

THEOREM AsyncCandidateDiscardIsNotSemanticService ==
  \A candidate \in AsyncCandidateSet:
    /\ ~CommandDispatchable(candidate)
    /\ (\/ \E node \in ValidatorIds:
             /\ candidate = NextNodeCommand(node)
             /\ FifoRuntimeStep(node)
        \/ \E node \in ValidatorIds:
             /\ candidate = NextDeferredCommand(node)
             /\ DeferredDrainStep(node))
    => candidate \notin AsyncCandidateServicesThisStep
BY Isa
   DEF AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       DeferredHandoffAllowsExecution

AsyncNodeHasDecisionAfter(node) ==
  \E decision \in decisions':
    /\ decision.node = node
    /\ decision.qc.context = context'
    /\ decision.qc.phase = "Commit"

AsyncCandidateSameOriginScheduledAfter(candidate) ==
  candidate.causalOrigin
    \in AsyncScheduledCandidateOriginsForNodeAfter(candidate.node)

AsyncCandidateSameOriginDurableReplayAfter(candidate) ==
  candidate.causalOrigin
    \in AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter(
         candidate.node)

AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter(candidate) ==
  \/ AsyncCandidateSameOriginScheduledAfter(candidate)
  \/ AsyncCandidateSameOriginDurableReplayAfter(candidate)

AsyncCandidateConsumerEpisodeObsoleteAfter(candidate) ==
  \/ candidate.consumerContext # context'
  \/ candidate.height # height'
  \/ candidate.consumerView < nodeView'[candidate.node]
  \/ candidate.consumerGeneration # generation'[candidate.node]
  \/ AsyncNodeHasDecisionAfter(candidate.node)

AsyncCandidateBodyStageCoveredAfter(candidate) ==
  LET body ==
        BodyRecord(candidate.node, candidate.consumerContext,
                   candidate.view, candidate.subject)
  IN \/ body \in availableBodies'
     \/ BodyHeldBy(durableBodies', candidate.node,
                    candidate.consumerContext,
                    candidate.view, candidate.subject)

AsyncCandidateValidationStageCoveredAfter(candidate) ==
  \/ \E validation \in validatedBodies':
       /\ validation.node = candidate.node
       /\ validation.context = candidate.consumerContext
       /\ validation.view = candidate.view
       /\ validation.subject = candidate.subject
  \/ BodyRecord(candidate.node, candidate.consumerContext,
                candidate.view, candidate.subject)
       \in invalidBodies'

AsyncCandidateProposalStageCoveredAfter(candidate) ==
  \/ \E request \in pendingProposal':
       /\ request.node = candidate.node
       /\ request.proposal.context = candidate.consumerContext
       /\ request.proposal.view = candidate.view
       /\ request.proposal.subject = candidate.subject
  \/ \E proposal \in proposalIntents':
       /\ proposal.proposer = candidate.node
       /\ proposal.context = candidate.consumerContext
       /\ proposal.view = candidate.view
       /\ proposal.subject = candidate.subject

AsyncCandidatePrepareStageCoveredAfter(candidate) ==
  \/ \E request \in pendingPrepare':
       /\ request.node = candidate.node
       /\ request.vote.context = candidate.consumerContext
       /\ request.vote.view = candidate.view
       /\ request.vote.phase = "Prepare"
       /\ request.vote.subject = candidate.subject
  \/ \E vote \in prepareIntents':
       /\ vote.signer = candidate.node
       /\ vote.context = candidate.consumerContext
       /\ vote.view = candidate.view
       /\ vote.phase = "Prepare"
       /\ vote.subject = candidate.subject

AsyncCandidateVoteStageCoveredAfter(candidate) ==
  \E vote \in prepareIntents' \cup commitIntents':
    /\ vote.signer = candidate.node
    /\ vote.context = candidate.consumerContext
    /\ vote.view = candidate.view
    /\ vote.subject = candidate.subject

AsyncCandidatePrepareQcStageCoveredAfter(candidate) ==
  \E qc \in prepareQCs':
    /\ qc.context = candidate.consumerContext
    /\ qc.view = candidate.view
    /\ qc.phase = "Prepare"
    /\ qc.subject = candidate.subject

AsyncCandidateObservePrepareStageCoveredAfter(candidate) ==
  \/ \E request \in pendingObservePrepare':
       /\ request.node = candidate.node
       /\ request.qc.context = candidate.consumerContext
       /\ request.qc.view = candidate.view
       /\ request.qc.phase = "Prepare"
       /\ request.qc.subject = candidate.subject
  \/ highestRank'[candidate.node] > candidate.view
  \/ /\ highestRank'[candidate.node] = candidate.view
        /\ highestSubject'[candidate.node] = candidate.subject

AsyncCandidateLockCommitStageCoveredAfter(candidate) ==
  \/ \E request \in pendingLockCommit':
       /\ request.node = candidate.node
       /\ request.qc.context = candidate.consumerContext
       /\ request.qc.view = candidate.view
       /\ request.qc.phase = "Prepare"
       /\ request.qc.subject = candidate.subject
  \/ \E vote \in commitIntents':
       /\ vote.signer = candidate.node
       /\ vote.context = candidate.consumerContext
       /\ vote.view = candidate.view
       /\ vote.phase = "Commit"
       /\ vote.subject = candidate.subject

AsyncCandidateCommitQcStageCoveredAfter(candidate) ==
  \E qc \in commitQCs':
    /\ qc.context = candidate.consumerContext
    /\ qc.view = candidate.view
    /\ qc.phase = "Commit"
    /\ qc.subject = candidate.subject

AsyncCandidateDecisionStageCoveredAfter(candidate) ==
  \/ \E request \in pendingDecision':
       /\ request.node = candidate.node
       /\ request.qc.context = candidate.consumerContext
       /\ request.qc.view = candidate.view
       /\ request.qc.phase = "Commit"
       /\ request.qc.subject = candidate.subject
  \/ \E decision \in decisions':
       /\ decision.node = candidate.node
       /\ decision.qc.context = candidate.consumerContext
       /\ decision.qc.view = candidate.view
       /\ decision.qc.phase = "Commit"
       /\ decision.qc.subject = candidate.subject

AsyncCandidateTimeoutStageCoveredAfter(candidate) ==
  \/ \E request \in pendingTimeout':
       /\ request.node = candidate.node
       /\ request.vote.context = candidate.consumerContext
       /\ request.vote.view = candidate.view
       /\ request.vote.highSubject = candidate.subject
  \/ \E vote \in timeoutIntents':
       /\ vote.signer = candidate.node
       /\ vote.context = candidate.consumerContext
       /\ vote.view = candidate.view
       /\ vote.highSubject = candidate.subject

AsyncCandidateInstallTcStageCoveredAfter(candidate) ==
  \/ \E request \in pendingInstallTC':
       /\ request.node = candidate.node
       /\ request.tc.context = candidate.consumerContext
       /\ request.tc.view = candidate.view
  \/ \E installed \in installedTCs':
       /\ installed.node = candidate.node
       /\ installed.tc.context = candidate.consumerContext
       /\ installed.tc.view = candidate.view
  \/ candidate.consumerGeneration # generation'[candidate.node]
  \/ nodeView'[candidate.node] > candidate.consumerView

AsyncCandidateApplicationStageCoveredAfter(candidate) ==
  \E application \in applied':
    /\ application.node = candidate.node
    /\ application.qc.context = candidate.consumerContext
    /\ application.qc.view = candidate.view
    /\ application.qc.phase = "Commit"
    /\ application.qc.subject = candidate.subject

AsyncCandidateDeliveryStageCoveredAfterIn(state, candidate) ==
  \/ /\ candidate.item # NoAsyncItem
        /\ candidate.item.kind \in AsyncControlKinds
        /\ AsyncControlServiceIdentityServicedOrAdvancedIn(
             state, candidate.item)
  \/ /\ candidate.item # NoAsyncItem
        /\ candidate.item.kind = "Chunk"
        /\ AsyncChunkReceipt(
             candidate.node, candidate.view,
             candidate.subject, candidate.item.envelope.chunk)
             \in asyncHeldChunks'

AsyncCandidateReducerStageCoveredAfterIn(state, candidate) ==
  CASE candidate.kind = "AssembleBody" ->
         /\ AsyncCandidateBodyStageCoveredAfter(candidate)
         /\ AsyncCandidateValidationStageCoveredAfter(candidate)
    [] candidate.kind \in
         {"BeginProposal", "PersistProposal", "SignProposal"} ->
         AsyncCandidateProposalStageCoveredAfter(candidate)
    [] candidate.kind \in
         {"FetchBody", "RebindRetainedBody", "StoreBody",
          "FetchCertifiedBody"} ->
         AsyncCandidateBodyStageCoveredAfter(candidate)
    [] candidate.kind = "ValidateBody" ->
         AsyncCandidateValidationStageCoveredAfter(candidate)
    [] candidate.kind \in {"BeginPrepare", "PersistPrepare"} ->
         AsyncCandidatePrepareStageCoveredAfter(candidate)
    [] candidate.kind = "SignVote" ->
         AsyncCandidateVoteStageCoveredAfter(candidate)
    [] candidate.kind = "FormPrepareQC" ->
         AsyncCandidatePrepareQcStageCoveredAfter(candidate)
    [] candidate.kind \in
         {"BeginObservePrepare", "PersistObservePrepare"} ->
         AsyncCandidateObservePrepareStageCoveredAfter(candidate)
    [] candidate.kind \in
         {"BeginLockCommit", "PersistLockCommit"} ->
         AsyncCandidateLockCommitStageCoveredAfter(candidate)
    [] candidate.kind = "FormCommitQC" ->
         AsyncCandidateCommitQcStageCoveredAfter(candidate)
    [] candidate.kind \in {"BeginDecision", "PersistDecision"} ->
         AsyncCandidateDecisionStageCoveredAfter(candidate)
    [] candidate.kind \in
         {"BeginTimeout", "PersistTimeout", "SignTimeout"} ->
         AsyncCandidateTimeoutStageCoveredAfter(candidate)
    [] candidate.kind \in {"BeginInstallTC", "PersistInstallTC"} ->
         AsyncCandidateInstallTcStageCoveredAfter(candidate)
    [] candidate.kind = "Apply" ->
         AsyncCandidateApplicationStageCoveredAfter(candidate)
    [] candidate.kind \in AsyncDeliveryKinds ->
         AsyncCandidateDeliveryStageCoveredAfterIn(state, candidate)
    [] OTHER -> FALSE

AsyncCandidateMonotoneSemanticCoverageAfterIn(state, candidate) ==
  \/ AsyncCandidateConsumerEpisodeObsoleteAfter(candidate)
  \/ AsyncCandidateReducerStageCoveredAfterIn(state, candidate)

AsyncCandidateServiceRecordRetainedAfterStep(record) ==
  /\ record.context = context'
  /\ record.height = height'
  /\ record.episodeView >= nodeView'[record.node]
  /\ ~AsyncNodeHasDecisionAfter(record.node)

AsyncCandidateServiceStateAfterReclamation(state) ==
  [state EXCEPT
     !.candidateServiceMarkers =
       {record \in state.candidateServiceMarkers:
          AsyncCandidateServiceRecordRetainedAfterStep(record)},
     !.candidateTerminalTombstones =
       {record \in state.candidateTerminalTombstones:
          AsyncCandidateServiceRecordRetainedAfterStep(record)}]

AsyncCandidateServiceEligibleAfterStep(candidate) ==
  /\ candidate.consumerContext = context'
  /\ candidate.height = height'
  /\ nodeView[candidate.node] >= nodeView'[candidate.node]
  /\ candidate.consumerGeneration = generation'[candidate.node]
  /\ ~AsyncNodeHasDecisionAfter(candidate.node)

AsyncCandidateTerminalRetirementEligibleAfterStep(candidate) ==
  /\ candidate.consumerContext = context'
  /\ candidate.height = height'
  /\ nodeView[candidate.node] >= nodeView'[candidate.node]
  \* An authenticated ingress occurrence which reaches the reducer but is
  \* ignored has not changed semantic state.  It receives no durable marker:
  \* otherwise distinct junk/stale/future envelopes could fill the table.
  \* Exact internal callbacks carry NoAsyncItem and retain the tombstone
  \* needed to prevent an owned Busy-lane stage from being resurrected.
  /\ candidate.item = NoAsyncItem
  /\ candidate.kind \notin AsyncRestartScopedCandidateServiceKinds
  /\ candidate.causalOrigin.phase # "BeginTimeout"
  /\ ~AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter(candidate)
  /\ ~AsyncCandidateMonotoneSemanticCoverageAfterIn(
        asyncControlServiceState, candidate)
  /\ ~AsyncNodeHasDecisionAfter(candidate.node)

AsyncCandidateServiceStateAfterTerminalRetirement(state, candidate) ==
  LET identity == AsyncCandidateServiceIdentity(candidate)
      existing ==
        {record \in
           state.candidateServiceMarkers
             \cup state.candidateTerminalTombstones:
           record.identity = identity}
      node == candidate.node
      ordinal == state.candidateServiceNextOrdinal[node]
  IN IF ~AsyncCandidateTerminalRetirementEligibleAfterStep(candidate)
          \/ existing # {}
     THEN state
     ELSE [state EXCEPT
             !.candidateServiceNextOrdinal[node] = @ + 1,
             !.candidateTerminalTombstones =
               @ \cup
                 {AsyncCandidateServiceTombstone(
                    candidate, nodeView[node], ordinal)}]

AsyncCandidateServiceStateAfterSuccessfulService(state, candidate) ==
  LET identity == AsyncCandidateServiceIdentity(candidate)
      existing ==
        {record \in
           state.candidateServiceMarkers
             \cup state.candidateTerminalTombstones:
           record.identity = identity}
      node == candidate.node
      ordinal == state.candidateServiceNextOrdinal[node]
  IN IF ~AsyncCandidateServiceEligibleAfterStep(candidate)
          \/ existing # {}
     THEN state
     ELSE [state EXCEPT
             !.candidateServiceNextOrdinal[node] = @ + 1,
             !.candidateServiceMarkers =
               @ \cup
                 {AsyncCandidateServiceMarker(
                    candidate, nodeView[node],
                    candidate.consumerGeneration, ordinal)}]

AsyncCandidateLifecycleOriginsRecordedForNodeIn(state, node) ==
  {record.origin:
     record \in state.candidateLifecycleAdmissions,
     record.node = node}

AsyncCandidateLifecycleRecordsForIn(state, node, origin) ==
  {record \in state.candidateLifecycleAdmissions:
     /\ record.node = node
     /\ record.origin = origin}

AsyncCandidateLifecycleRecordsForNodeIn(state, node) ==
  {record \in state.candidateLifecycleAdmissions:
     record.node = node}

AsyncCandidateLifecycleClockRecordBucketIn(state, node) ==
  {record \in AsyncCandidateLifecycleRecordsForNodeIn(state, node):
     record.origin.phase = "BeginTimeout"}

AsyncCandidateLifecycleOrdinaryRecordBucketIn(state, node) ==
  AsyncCandidateLifecycleRecordsForNodeIn(state, node)
    \ AsyncCandidateLifecycleClockRecordBucketIn(state, node)

AsyncCandidateLifecycleActiveOrdinaryBucketIn(state, node) ==
  {record \in
     AsyncCandidateLifecycleOrdinaryRecordBucketIn(state, node):
     ~record.retired}

AsyncCandidateLifecycleDormantOrdinaryBucketIn(state, node) ==
  {record \in
     AsyncCandidateLifecycleOrdinaryRecordBucketIn(state, node):
     record.retired}

AsyncCandidateLifecycleTerminalRecordCoversIn(state, record) ==
  \E terminal \in state.candidateTerminalTombstones:
    /\ terminal.node = record.node
    /\ terminal.identity.payload.causalOrigin = record.origin

AsyncCandidateLifecycleServiceRecordCoversIn(state, record) ==
  \E serviced \in
       state.candidateServiceMarkers
         \cup state.candidateTerminalTombstones:
    /\ serviced.node = record.node
    /\ serviced.identity.payload.causalOrigin = record.origin

AsyncCandidateLifecycleDormantReplayableBucketIn(state, node) ==
  {record \in
     AsyncCandidateLifecycleDormantOrdinaryBucketIn(state, node):
     ~AsyncCandidateLifecycleServiceRecordCoversIn(state, record)}

AsyncCandidateLifecycleDormantServicedBucketIn(state, node) ==
  {record \in
     AsyncCandidateLifecycleDormantOrdinaryBucketIn(state, node):
     AsyncCandidateLifecycleServiceRecordCoversIn(state, record)}

AsyncCandidateLifecycleDormantTerminalBucketIn(state, node) ==
  {record \in
     AsyncCandidateLifecycleDormantOrdinaryBucketIn(state, node):
     AsyncCandidateLifecycleTerminalRecordCoversIn(state, record)}

AsyncUnmaterializedTimeoutLifecycleReservationIn(state, node) ==
  /\ state.timeoutLifecycleOrdinal[node] # 0
  /\ state.timeoutLifecycleOrigin[node]
       # NoAsyncCandidateLifecycleOrigin
  /\ AsyncCandidateLifecycleRecordsForIn(
       state, node, state.timeoutLifecycleOrigin[node]) = {}

AsyncCandidateLifecycleClockOwnerCountIn(state, node) ==
  Cardinality(
    AsyncCandidateLifecycleClockRecordBucketIn(state, node))
    + (IF AsyncUnmaterializedTimeoutLifecycleReservationIn(state, node)
       THEN 1 ELSE 0)

AsyncUnmaterializedTimeoutLifecycleReservationNodesIn(state) ==
  {node \in ValidatorIds:
     AsyncUnmaterializedTimeoutLifecycleReservationIn(state, node)}

AsyncCandidateLifecycleRecordOwnerToken(record) ==
  [kind |-> "CandidateLifecycleRecord",
   node |-> record.node,
   slot |-> record.slot,
   ordinal |-> record.ordinal,
   origin |-> record.origin]

AsyncCandidateLifecycleClockOwnerToken(state, node) ==
  [kind |-> "CandidateLifecycleClockReservation",
   node |-> node,
   slot |-> AsyncCandidateLifecycleClockSlot,
   ordinal |-> state.timeoutLifecycleOrdinal[node],
   origin |-> state.timeoutLifecycleOrigin[node]]

AsyncCandidateLifecycleReviewedOwnerTokensIn(state) ==
  {AsyncCandidateLifecycleRecordOwnerToken(record):
     record \in state.candidateLifecycleAdmissions}
    \cup
  {AsyncCandidateLifecycleClockOwnerToken(state, node):
     node \in
       AsyncUnmaterializedTimeoutLifecycleReservationNodesIn(state)}

AsyncCandidateLifecycleSlotAddresses ==
  [node: ValidatorIds, slot: AsyncCandidateLifecycleSlots]

AsyncCandidateLifecycleSlotProjectionIn(state) ==
  [token \in AsyncCandidateLifecycleReviewedOwnerTokensIn(state) |->
     [node |-> token.node, slot |-> token.slot]]

AsyncCandidateLifecycleSlotInjectionInvariantIn(state) ==
  /\ AsyncCandidateLifecycleSlotProjectionIn(state)
       \in Injection(
            AsyncCandidateLifecycleReviewedOwnerTokensIn(state),
            AsyncCandidateLifecycleSlotAddresses)
  /\ \A node \in ValidatorIds:
       /\ \A record \in
              AsyncCandidateLifecycleRecordsForNodeIn(state, node):
            /\ record.slot \in AsyncCandidateLifecycleSlots
            /\ (record.origin.phase = "BeginTimeout")
                 = (record.slot = AsyncCandidateLifecycleClockSlot)
            /\ (record.origin.phase # "BeginTimeout"
                  => IF /\ record.retired
                        /\ AsyncCandidateLifecycleServiceRecordCoversIn(
                             state, record)
                     THEN record.slot
                            \in AsyncCandidateLifecycleServicedSlots
                     ELSE record.slot
                            \in AsyncCandidateLifecycleActiveSlots)
       /\ \A left, right \in
              AsyncCandidateLifecycleRecordsForNodeIn(state, node):
            left.slot = right.slot => left = right
       /\ (AsyncUnmaterializedTimeoutLifecycleReservationIn(state, node)
             => AsyncCandidateLifecycleClockRecordBucketIn(state, node)
                  = {})

AsyncCandidateLifecycleReviewedCapacityInvariantIn(state) ==
  AsyncCandidateLifecycleSlotInjectionInvariantIn(state)

AsyncCandidateLifecycleRecordedIn(state, node, origin) ==
  AsyncCandidateLifecycleRecordsForIn(state, node, origin) # {}

AsyncCandidateLifecycleRecordForIn(state, node, origin) ==
  CHOOSE record \in
    AsyncCandidateLifecycleRecordsForIn(state, node, origin): TRUE

AsyncCandidateLifecycleViewScopedRootKinds ==
  AsyncDeliveryKinds
    \cup {"AssembleBody", "BeginTimeout", "FetchCertifiedBody"}

AsyncCandidateLifecycleDurableReplayRootKinds ==
  {"FetchBody", "SignProposal", "SignVote", "SignTimeout"}

AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter(node) ==
  {candidate.causalOrigin:
     candidate \in
       SequenceSet(
         FreshRestartCandidateSequence(RestartReplay(node))'),
     candidate.causalOrigin
       \notin AsyncScheduledCandidateOriginsForNodeAfter(node)}
    \cup
  {candidate.causalOrigin:
     candidate \in
       SequenceSet(HistoricalLockedRetransmitSuccessors(node)'),
     candidate.causalOrigin
       \notin AsyncScheduledCandidateOriginsForNodeAfter(node)}

AsyncCandidateLifecycleIgnoredEpisodeCovers(record) ==
  \E candidate \in
       AsyncCandidateIgnoredWithoutApplicationThisStepSet:
    /\ candidate.node = record.node
    /\ candidate.causalOrigin = record.origin
    /\ \/ candidate.item # NoAsyncItem
       \/ record.origin
            \notin
              AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter(
                record.node)

AsyncCandidateLifecyclePermanentlyObsoleteAfter(record) ==
  \/ record.origin.context # context'
  \/ record.origin.height # height'
  \/ AsyncNodeHasDecisionIn(record.node, context', decisions')
  \/ /\ record.origin.phase
           \in AsyncCandidateLifecycleViewScopedRootKinds
     /\ record.origin.view < nodeView'[record.node]
  \/ /\ record.origin.phase
           \in AsyncCandidateLifecycleDurableReplayRootKinds
     /\ record.origin
          \notin
            AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter(
              record.node)

AsyncCandidateLifecycleDormantReservationOwnedAfter(state, record) ==
  \/ AsyncCandidateLifecycleServiceRecordCoversIn(state, record)
  \/ record.origin
       \in AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter(
            record.node)

AsyncCandidateLifecycleRetirementCoveredIn(state, record) ==
  \/ AsyncCandidateLifecyclePermanentlyObsoleteAfter(record)
  \/ ~AsyncCandidateLifecycleDormantReservationOwnedAfter(state, record)

AsyncCandidateLifecycleCarrierUpdatedAdmissions(state) ==
  {IF record.origin
        \in AsyncScheduledCandidateOriginsForNodeAfter(record.node)
   THEN [record EXCEPT !.retired = FALSE]
   ELSE [record EXCEPT !.retired = TRUE]:
     record \in state.candidateLifecycleAdmissions}

AsyncCandidateLifecycleStateAfterCarrierUpdate(state) ==
  [state EXCEPT
     !.candidateLifecycleAdmissions =
       AsyncCandidateLifecycleCarrierUpdatedAdmissions(state)]

AsyncCandidateLifecycleStateAfterCompaction(state) ==
  [state EXCEPT
     !.candidateLifecycleAdmissions =
       {record \in state.candidateLifecycleAdmissions:
          ~(/\ record.retired
            /\ AsyncCandidateLifecycleRetirementCoveredIn(
                 state, record))}]

AsyncNewCandidateLifecycleOriginsForNodeIn(state, node) ==
  AsyncScheduledCandidateOriginsForNodeAfter(node)
    \ AsyncCandidateLifecycleOriginsRecordedForNodeIn(state, node)

AsyncOrderedScheduledCandidatesForNodeAfter(node) ==
  asyncCommandQueues'[node]
    \o asyncDeferredCompletionQueues'[node]
    \o asyncDeferredProgressQueues'[node]
    \o asyncDeferredNormalQueues'[node]
    \o asyncCausalQueues'[node]

AsyncOrderedScheduledOriginsForNodeAfter(node) ==
  {AsyncOrderedScheduledCandidatesForNodeAfter(node)[index].causalOrigin:
     index \in 1..Len(AsyncOrderedScheduledCandidatesForNodeAfter(node))}

AsyncFirstScheduledOriginIndexAfter(node, origin) ==
  CHOOSE index \in 1..Len(AsyncOrderedScheduledCandidatesForNodeAfter(node)):
    /\ AsyncOrderedScheduledCandidatesForNodeAfter(node)[index].causalOrigin
         = origin
    /\ \A other \in
             1..Len(AsyncOrderedScheduledCandidatesForNodeAfter(node)):
         AsyncOrderedScheduledCandidatesForNodeAfter(node)[other].causalOrigin
           = origin
           => index <= other

AsyncCurrentTimeoutCausalOrigin(node) ==
  TimeoutCausalCommand(node).causalOrigin

AsyncTimeoutLifecycleTransfersThisStep(node) ==
  /\ AsyncCurrentTimeoutCausalOrigin(node)
       \notin AsyncScheduledCandidateOriginsForNode(node)
  /\ AsyncCurrentTimeoutCausalOrigin(node)
       \in AsyncScheduledCandidateOriginsForNodeAfter(node)

AsyncTimeoutLifecycleResetThisStep(node) ==
  \/ context' # context
  \/ nodeView'[node] # nodeView[node]
  \/ asyncNodeDeadlines'[node] # asyncNodeDeadlines[node]
  \/ AsyncNodeHasDecisionIn(node, context', decisions')
  \/ /\ "TimeoutElapsed" \in asyncOutstandingTags[node]
     /\ "TimeoutElapsed" \notin asyncOutstandingTags'[node]
     /\ AsyncNodeTimedOutIn(
          node, nodeView'[node], context', timeoutIntents')

AsyncTimeoutLifecycleCanAcquireThisStep(node) ==
  /\ \/ AsyncTimeoutClockDue(node)
     \/ AsyncTimeoutClockDueAfter(node)
     \/ AsyncTimeoutLifecycleTransfersThisStep(node)
  /\ \/ ~AsyncTimeoutLifecycleResetThisStep(node)
     \/ AsyncTimeoutLifecycleTransfersThisStep(node)

AsyncTimeoutLifecycleUsesRecordedOriginOrdinal(state, node) ==
  AsyncCandidateLifecycleRecordedIn(
    state, node, AsyncCurrentTimeoutCausalOrigin(node))

AsyncTimeoutLifecycleOrdinalForStep(state, node) ==
  IF state.timeoutLifecycleOrdinal[node] # 0
  THEN state.timeoutLifecycleOrdinal[node]
  ELSE IF AsyncTimeoutLifecycleUsesRecordedOriginOrdinal(state, node)
       THEN AsyncCandidateLifecycleRecordForIn(
              state, node,
              AsyncCurrentTimeoutCausalOrigin(node)).ordinal
       ELSE state.candidateLifecycleNextOrdinal[node]

AsyncTimeoutLifecycleConsumesFreshOrdinal(state, node) ==
  /\ state.timeoutLifecycleOrdinal[node] = 0
  /\ ~AsyncTimeoutLifecycleUsesRecordedOriginOrdinal(state, node)
  /\ AsyncTimeoutLifecycleCanAcquireThisStep(node)

AsyncTimeoutLifecycleNewOriginsForNodeIn(state, node) ==
  {origin \in AsyncNewCandidateLifecycleOriginsForNodeIn(state, node):
     /\ origin = AsyncCurrentTimeoutCausalOrigin(node)
     /\ AsyncTimeoutLifecycleCanAcquireThisStep(node)}

AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(state, node) ==
  AsyncNewCandidateLifecycleOriginsForNodeIn(state, node)
    \ AsyncTimeoutLifecycleNewOriginsForNodeIn(state, node)

AsyncOrdinaryNewCandidateLifecyclePredecessorsFor(
    state, node, origin) ==
  {other \in
     AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(state, node):
     AsyncFirstScheduledOriginIndexAfter(node, other)
       < AsyncFirstScheduledOriginIndexAfter(node, origin)}

AsyncCandidateLifecycleUsedOrdinarySlotsForNodeIn(state, node) ==
  {record.slot:
     record \in AsyncCandidateLifecycleOrdinaryRecordBucketIn(state, node)}

AsyncCandidateLifecycleUsedActiveSlotsForNodeIn(state, node) ==
  AsyncCandidateLifecycleUsedOrdinarySlotsForNodeIn(state, node)
    \cap AsyncCandidateLifecycleActiveSlots

AsyncCandidateLifecycleUsedServicedSlotsForNodeIn(state, node) ==
  AsyncCandidateLifecycleUsedOrdinarySlotsForNodeIn(state, node)
    \cap AsyncCandidateLifecycleServicedSlots

AsyncCandidateLifecycleOrdinaryOriginsForNodeIn(state, node) ==
  {record.origin:
     record \in AsyncCandidateLifecycleOrdinaryRecordBucketIn(state, node)}

AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn(state, node) ==
  AsyncCandidateLifecycleActiveSlots
    \ AsyncCandidateLifecycleUsedActiveSlotsForNodeIn(state, node)

AsyncCandidateLifecycleFreeServicedSlotsForNodeIn(state, node) ==
  AsyncCandidateLifecycleServicedSlots
    \ AsyncCandidateLifecycleUsedServicedSlotsForNodeIn(state, node)

AsyncCandidateServiceIdentityRecordedIn(state, candidate) ==
  \E record \in
       state.candidateServiceMarkers
         \cup state.candidateTerminalTombstones:
    record.identity = AsyncCandidateServiceIdentity(candidate)

AsyncCandidateTerminalServiceReservationNeededIn(state, candidate) ==
  /\ AsyncCandidateTerminalRetirementEligibleAfterStep(candidate)
  /\ ~AsyncCandidateServiceIdentityRecordedIn(state, candidate)

AsyncCandidateTerminalServiceReservationAvailableIn(state) ==
  IF AsyncCandidateTerminalDiscardsThisStep = {}
  THEN TRUE
  ELSE LET candidate ==
             CHOOSE discarded \in
               AsyncCandidateTerminalDiscardsThisStep: TRUE
       IN \/ ~AsyncCandidateTerminalServiceReservationNeededIn(
                  state, candidate)
          \/ AsyncCandidateLifecycleFreeServicedSlotsForNodeIn(
               state, candidate.node) # {}

AsyncCandidateLifecycleDeparturesThisStep ==
  AsyncCandidateServicesThisStep
    \cup AsyncCandidateIgnoredWithoutApplicationThisStepSet

AsyncCandidateServiceSlotTransferNeededIn(state, candidate) ==
  /\ candidate.causalOrigin.phase # "BeginTimeout"
  /\ candidate.causalOrigin
       \notin AsyncScheduledCandidateOriginsForNodeAfter(candidate.node)
  /\ \E serviced \in
       state.candidateServiceMarkers
         \cup state.candidateTerminalTombstones:
       /\ serviced.node = candidate.node
       /\ serviced.identity.payload.causalOrigin = candidate.causalOrigin
  /\ \E record \in state.candidateLifecycleAdmissions:
       /\ record.node = candidate.node
       /\ record.origin = candidate.causalOrigin
       /\ record.slot \notin AsyncCandidateLifecycleServicedSlots

AsyncCandidateServiceReservationAvailableIn(state) ==
  IF AsyncCandidateLifecycleDeparturesThisStep = {}
  THEN TRUE
  ELSE LET candidate ==
             CHOOSE departed \in
               AsyncCandidateLifecycleDeparturesThisStep: TRUE
       IN \/ ~AsyncCandidateServiceSlotTransferNeededIn(state, candidate)
          \/ AsyncCandidateLifecycleFreeServicedSlotsForNodeIn(
               state, candidate.node) # {}

AsyncCandidateLifecycleFirstFreeServicedSlotForIn(
    state, node, candidate) ==
  CHOOSE slot \in
    AsyncCandidateLifecycleFreeServicedSlotsForNodeIn(state, node):
      \A other \in
        AsyncCandidateLifecycleFreeServicedSlotsForNodeIn(state, node):
        slot <= other

AsyncCandidateLifecycleStateAfterServiceSlotTransfer(state, candidate) ==
  IF ~AsyncCandidateServiceSlotTransferNeededIn(state, candidate)
  THEN state
  ELSE LET slot ==
             AsyncCandidateLifecycleFirstFreeServicedSlotForIn(
               state, candidate.node, candidate)
       IN [state EXCEPT
             !.candidateLifecycleAdmissions =
               {IF /\ record.node = candidate.node
                    /\ record.origin = candidate.causalOrigin
                THEN [record EXCEPT !.slot = slot]
                ELSE record:
                  record \in state.candidateLifecycleAdmissions}]

\* Compatibility name retained for the terminal-retirement proof shard.  The
\* same atomic transfer now also covers a process-generation service marker
\* when its final same-origin physical carrier departs.
AsyncCandidateLifecycleStateAfterTerminalSlotTransfer(state, candidate) ==
  AsyncCandidateLifecycleStateAfterServiceSlotTransfer(state, candidate)

AsyncCandidateLifecycleFreeSlotPredecessorsFor(
    state, node, slot) ==
  {other \in
     AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn(state, node):
     other < slot}

AsyncCandidateLifecycleAdmissionSlotFor(state, node, origin) ==
  IF origin \in AsyncTimeoutLifecycleNewOriginsForNodeIn(state, node)
  THEN AsyncCandidateLifecycleClockSlot
  ELSE CHOOSE slot \in
         AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn(state, node):
         Cardinality(
           AsyncCandidateLifecycleFreeSlotPredecessorsFor(
             state, node, slot))
           = Cardinality(
               AsyncOrdinaryNewCandidateLifecyclePredecessorsFor(
                 state, node, origin))

AsyncCandidateLifecycleOrdinaryReservationsAvailableIn(state) ==
  \A node \in ValidatorIds:
    Cardinality(
      AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(state, node))
      <= Cardinality(
           AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn(state, node))

AsyncCandidateLifecycleClockReservationAvailableIn(state) ==
  \A node \in ValidatorIds:
    /\ (AsyncTimeoutLifecycleNewOriginsForNodeIn(state, node) # {}
          => AsyncCandidateLifecycleClockRecordBucketIn(state, node) = {})
    /\ (AsyncTimeoutLifecycleConsumesFreshOrdinal(state, node)
          => AsyncCandidateLifecycleClockOwnerCountIn(state, node) = 0)

AsyncCandidateLifecycleReservationsAvailableIn(state) ==
  /\ AsyncCandidateLifecycleOrdinaryReservationsAvailableIn(state)
  /\ AsyncCandidateLifecycleClockReservationAvailableIn(state)

AsyncCandidateLifecycleAdmissionOrdinalFor(
    state, node, origin) ==
  IF origin \in AsyncTimeoutLifecycleNewOriginsForNodeIn(state, node)
  THEN AsyncTimeoutLifecycleOrdinalForStep(state, node)
  ELSE state.candidateLifecycleNextOrdinal[node]
         + (IF AsyncTimeoutLifecycleConsumesFreshOrdinal(state, node)
            THEN 1 ELSE 0)
         + Cardinality(
             AsyncOrdinaryNewCandidateLifecyclePredecessorsFor(
               state, node, origin))

AsyncCandidateLifecycleNewAdmissions(state) ==
  UNION
    {{AsyncCandidateLifecycleAdmission(
        node, origin,
        AsyncCandidateLifecycleAdmissionOrdinalFor(
          state, node, origin),
        AsyncCandidateLifecycleAdmissionSlotFor(
          state, node, origin),
        FALSE):
        origin \in AsyncNewCandidateLifecycleOriginsForNodeIn(state, node)}:
       node \in ValidatorIds}

AsyncCandidateLifecycleStateAfterAdmission(state) ==
  [state EXCEPT
     !.candidateLifecycleAdmissions =
       @ \cup AsyncCandidateLifecycleNewAdmissions(state),
     !.candidateLifecycleNextOrdinal =
       [node \in ValidatorIds |->
          state.candidateLifecycleNextOrdinal[node]
            + (IF AsyncTimeoutLifecycleConsumesFreshOrdinal(state, node)
               THEN 1 ELSE 0)
            + (IF AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(
                    state, node) # {}
               THEN Cardinality(
                      AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(
                        state, node))
               ELSE 0)]]

AsyncCandidateLifecycleStateAfterTimeoutOwnership(baseState, state) ==
  [state EXCEPT
     !.timeoutLifecycleOrdinal =
       [node \in ValidatorIds |->
          IF AsyncTimeoutLifecycleResetThisStep(node)
               \/ AsyncTimeoutLifecycleTransfersThisStep(node)
          THEN 0
          ELSE IF baseState.timeoutLifecycleOrdinal[node] # 0
               THEN baseState.timeoutLifecycleOrdinal[node]
               ELSE IF AsyncTimeoutLifecycleCanAcquireThisStep(node)
                    THEN AsyncTimeoutLifecycleOrdinalForStep(
                           baseState, node)
                    ELSE 0],
     !.timeoutLifecycleOrigin =
       [node \in ValidatorIds |->
          IF AsyncTimeoutLifecycleResetThisStep(node)
               \/ AsyncTimeoutLifecycleTransfersThisStep(node)
          THEN NoAsyncCandidateLifecycleOrigin
          ELSE IF baseState.timeoutLifecycleOrdinal[node] # 0
               THEN baseState.timeoutLifecycleOrigin[node]
               ELSE IF AsyncTimeoutLifecycleCanAcquireThisStep(node)
                    THEN AsyncCurrentTimeoutCausalOrigin(node)
                    ELSE NoAsyncCandidateLifecycleOrigin]]

(***************************************************************************
An exact Serve ingress admission is visible in the primed ingress table before
the global control-service transition publishes its matching shared scheduler
ordinal.  Identity comparison, rather than record-set subtraction, avoids
misclassifying the immutable record rewrite which decrements a frozen ingress
predecessor count.  The current runner admits at most one exact ingress owner
per global transition.  Its scheduler ordinal is the pre-admission shared
high-watermark; the high-watermark advances before this same transition can
allocate a fresh timeout or ordinary Candidate root.
***************************************************************************)
AsyncFreshServeIngressAdmissionsForNodeThisStep(node) ==
  {admission \in asyncServeIngressAdmissions':
     /\ admission.node = node
     /\ ~AsyncServeIngressAdmissionOwned(node, admission.identity)}

AsyncFreshServeIngressAdmissionsAreSingularThisStep ==
  \A node \in ValidatorIds:
    Cardinality(
      AsyncFreshServeIngressAdmissionsForNodeThisStep(node)) <= 1

AsyncFreshServeIngressSchedulerReservationMatchesIn(state) ==
  \A node \in ValidatorIds:
    \A admission \in
         AsyncFreshServeIngressAdmissionsForNodeThisStep(node):
      admission.schedulerOrdinal =
        state.candidateLifecycleNextOrdinal[node]

AsyncCandidateLifecycleStateAfterServeIngressAdmission(state) ==
  [state EXCEPT
     !.candidateLifecycleNextOrdinal =
       [node \in ValidatorIds |->
          state.candidateLifecycleNextOrdinal[node]
            + Cardinality(
                AsyncFreshServeIngressAdmissionsForNodeThisStep(node))]]

AsyncCandidateLifecyclePhysicalOwnerToken(
    carrier, node, position, origin) ==
  [kind |-> "CandidateLifecyclePhysicalOwner",
   carrier |-> carrier, node |-> node,
   position |-> position, origin |-> origin]

AsyncCandidateLifecyclePhysicalOwnerTokensForNodeIn(
    node, commandQueues, deferredCompletionQueues,
    deferredProgressQueues, deferredNormalQueues,
    causalQueues, outstandingWork) ==
  {AsyncCandidateLifecyclePhysicalOwnerToken(
     "Command", node, index,
     commandQueues[node][index].causalOrigin):
     index \in 1..Len(commandQueues[node])}
  \cup
  {AsyncCandidateLifecyclePhysicalOwnerToken(
     "DeferredCompletion", node, index,
     deferredCompletionQueues[node][index].causalOrigin):
     index \in 1..Len(deferredCompletionQueues[node])}
  \cup
  {AsyncCandidateLifecyclePhysicalOwnerToken(
     "DeferredProgress", node, index,
     deferredProgressQueues[node][index].causalOrigin):
     index \in 1..Len(deferredProgressQueues[node])}
  \cup
  {AsyncCandidateLifecyclePhysicalOwnerToken(
     "DeferredNormal", node, index,
     deferredNormalQueues[node][index].causalOrigin):
     index \in 1..Len(deferredNormalQueues[node])}
  \cup
  {AsyncCandidateLifecyclePhysicalOwnerToken(
     "Causal", node, index,
     causalQueues[node][index].causalOrigin):
     index \in 1..Len(causalQueues[node])}
  \cup
  {AsyncCandidateLifecyclePhysicalOwnerToken(
     "OutstandingWork", node,
     ExactAsyncCandidateIdentity(candidate), candidate.causalOrigin):
     candidate \in outstandingWork[node]}

AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter(node) ==
  AsyncCandidateLifecyclePhysicalOwnerTokensForNodeIn(
    node, asyncCommandQueues',
    asyncDeferredCompletionQueues', asyncDeferredProgressQueues',
    asyncDeferredNormalQueues', asyncCausalQueues',
    asyncOutstandingWork')

AsyncCandidateLifecycleServiceOwnerToken(state, record) ==
  [kind |-> "CandidateLifecycleServiceOwner",
   carrier |-> "ServicedLifecycle",
   node |-> record.node,
   position |-> record.slot,
   origin |-> record.origin]

AsyncCandidateLifecycleServiceOwnerTokensForNodeIn(state, node) ==
  {AsyncCandidateLifecycleServiceOwnerToken(state, record):
     record \in
       {candidate \in state.candidateLifecycleAdmissions:
          /\ candidate.node = node
          /\ candidate.retired
          /\ candidate.origin.phase # "BeginTimeout"
          /\ AsyncCandidateLifecycleServiceRecordCoversIn(
               state, candidate)}}

AsyncCandidateLifecycleDurableOwnerToken(
    carrier, node, position, origin) ==
  [kind |-> "CandidateLifecycleDurableOwner",
   carrier |-> carrier, node |-> node,
   position |-> position, origin |-> origin]

AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter(node) ==
  {AsyncCandidateLifecycleDurableOwnerToken(
     "RestartReplay", node, index,
     FreshRestartCandidateSequence(
       RestartReplay(node))'[index].causalOrigin):
     /\ index \in
          1..Len(FreshRestartCandidateSequence(RestartReplay(node))')
     /\ FreshRestartCandidateSequence(
          RestartReplay(node))'[index].causalOrigin
          \notin AsyncScheduledCandidateOriginsForNodeAfter(node)}
  \cup
  {AsyncCandidateLifecycleDurableOwnerToken(
     "HistoricalRetransmit", node, index,
     HistoricalLockedRetransmitSuccessors(node)'[index].causalOrigin):
     /\ index \in 1..Len(HistoricalLockedRetransmitSuccessors(node)')
     /\ HistoricalLockedRetransmitSuccessors(node)'[index].causalOrigin
          \notin AsyncScheduledCandidateOriginsForNodeAfter(node)}

AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn(state, node) ==
  AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter(node)
    \cup AsyncCandidateLifecycleServiceOwnerTokensForNodeIn(state, node)
    \cup AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter(node)

AsyncCandidateLifecycleReviewedActiveOwnerTokensForNodeAfter(node) ==
  AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter(node)
    \cup AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter(node)

AsyncCandidateLifecycleActiveOriginsForNodeIn(state, node) ==
  {record.origin:
     record \in AsyncCandidateLifecycleRecordsForNodeIn(state, node),
     record.slot \in AsyncCandidateLifecycleActiveSlots}

AsyncCandidateLifecycleLiveActiveOriginCarrierIn(state, node) ==
  AsyncCandidateLifecycleActiveOriginsForNodeIn(state, node)
    \cup AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(state, node)

AsyncCandidateLifecycleReviewedActiveCoverageIn(state, node) ==
  \A origin \in
       AsyncCandidateLifecycleLiveActiveOriginCarrierIn(state, node):
    \E token \in
         AsyncCandidateLifecycleReviewedActiveOwnerTokensForNodeAfter(node):
      token.origin = origin

AsyncCandidateLifecycleActiveOwnerForOriginIn(state, node, origin) ==
  CHOOSE token \in
    AsyncCandidateLifecycleReviewedActiveOwnerTokensForNodeAfter(node):
      token.origin = origin

AsyncCandidateLifecycleActiveOwnerProjectionIn(state, node) ==
  [origin \in
     AsyncCandidateLifecycleLiveActiveOriginCarrierIn(state, node) |->
     AsyncCandidateLifecycleActiveOwnerForOriginIn(
       state, node, origin)]

AsyncCandidateLifecycleActiveOwnerInjectionIn(state, node) ==
  AsyncCandidateLifecycleActiveOwnerProjectionIn(state, node)
    \in Injection(
         AsyncCandidateLifecycleLiveActiveOriginCarrierIn(state, node),
         AsyncCandidateLifecycleReviewedActiveOwnerTokensForNodeAfter(node))

AsyncCandidateLifecycleLiveOrdinaryOriginCarrierIn(state, node) ==
  AsyncCandidateLifecycleOrdinaryOriginsForNodeIn(state, node)
    \cup AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(state, node)

AsyncCandidateLifecycleReviewedSemanticCoverageIn(state, node) ==
  \A origin \in
       AsyncCandidateLifecycleLiveOrdinaryOriginCarrierIn(state, node):
    \E token \in
         AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn(state, node):
      token.origin = origin

AsyncCandidateLifecycleSemanticOwnerForOriginIn(state, node, origin) ==
  CHOOSE token \in
    AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn(state, node):
      token.origin = origin

AsyncCandidateLifecycleSemanticOwnerProjectionIn(state, node) ==
  [origin \in
     AsyncCandidateLifecycleLiveOrdinaryOriginCarrierIn(state, node) |->
     AsyncCandidateLifecycleSemanticOwnerForOriginIn(
       state, node, origin)]

AsyncCandidateLifecycleSemanticOwnerInjectionIn(state, node) ==
  AsyncCandidateLifecycleSemanticOwnerProjectionIn(state, node)
    \in Injection(
         AsyncCandidateLifecycleLiveOrdinaryOriginCarrierIn(state, node),
         AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn(state, node))

(***************************************************************************
Endpoint-neutral finite lifecycle episodes.

This structural layer is intentionally below timeout, exact-Decision,
adequate-leader, and historical-recovery proofs.  A caller freezes its actual
candidate and Serve identity carriers and proves its own selected-owner fair
step.  Shared arithmetic says only that genuine discovery grows `known` and
strictly consumes the finite complement budget; every caller retains its own
semantic endpoint.
***************************************************************************)

AsyncTargetNeutralLifecycleOwnerCarrier(
    candidateCarrier, serveCarrier) ==
  ({"Candidate"} \X candidateCarrier)
    \cup ({"Serve"} \X serveCarrier)

AsyncTargetNeutralLifecycleKnownOwnerSet(
    candidateCarrier, serveCarrier, known) ==
  /\ IsFiniteSet(candidateCarrier)
  /\ IsFiniteSet(serveCarrier)
  /\ known
       \subseteq AsyncTargetNeutralLifecycleOwnerCarrier(
                    candidateCarrier, serveCarrier)

AsyncTargetNeutralLifecycleEpisodeBudget(
    candidateCarrier, serveCarrier, known) ==
  Cardinality(
    AsyncTargetNeutralLifecycleOwnerCarrier(
      candidateCarrier, serveCarrier) \ known)

AsyncTargetNeutralLifecycleDiscoveredOwnerSet(liveOwners, known) ==
  liveOwners \ known

AsyncTargetNeutralLifecycleEpisodeAtBudget(
    candidateCarrier, serveCarrier, liveOwners, known, budget) ==
  /\ AsyncTargetNeutralLifecycleKnownOwnerSet(
       candidateCarrier, serveCarrier, known)
  /\ liveOwners
       \subseteq AsyncTargetNeutralLifecycleOwnerCarrier(
                    candidateCarrier, serveCarrier)
  /\ budget =
       AsyncTargetNeutralLifecycleEpisodeBudget(
         candidateCarrier, serveCarrier, known)

AsyncTargetNeutralLifecycleKnownAdvanceGoal(
    candidateCarrier, serveCarrier,
    liveOwners, known, budget) ==
  \E discovered,
     known2 \in
       SUBSET AsyncTargetNeutralLifecycleOwnerCarrier(
         candidateCarrier, serveCarrier),
     budget2 \in Nat:
    /\ discovered =
         AsyncTargetNeutralLifecycleDiscoveredOwnerSet(
           liveOwners, known)
    /\ discovered # {}
    /\ known2 = known \cup discovered
    /\ AsyncTargetNeutralLifecycleEpisodeAtBudget(
         candidateCarrier, serveCarrier,
         liveOwners, known2, budget2)
    /\ budget2 < budget

THEOREM AsyncTargetNeutralLifecycleOwnerCarrierIsFinite ==
  \A candidateCarrier, serveCarrier:
    /\ IsFiniteSet(candidateCarrier)
    /\ IsFiniteSet(serveCarrier)
    => IsFiniteSet(
         AsyncTargetNeutralLifecycleOwnerCarrier(
           candidateCarrier, serveCarrier))
BY FS_Product, FS_Union
   DEF AsyncTargetNeutralLifecycleOwnerCarrier

THEOREM AsyncTargetNeutralLifecycleEpisodeBudgetIsFiniteAndCoalesced ==
  \A candidateCarrier, serveCarrier, liveOwners, known, budget:
    AsyncTargetNeutralLifecycleEpisodeAtBudget(
      candidateCarrier, serveCarrier, liveOwners, known, budget)
      => /\ budget \in Nat
         /\ budget
              <= Cardinality(
                   AsyncTargetNeutralLifecycleOwnerCarrier(
                     candidateCarrier, serveCarrier))
         /\ (liveOwners \subseteq known
               <=> AsyncTargetNeutralLifecycleDiscoveredOwnerSet(
                     liveOwners, known) = {})
BY AsyncTargetNeutralLifecycleOwnerCarrierIsFinite,
   FS_Subset, FS_CardinalityType, IsaT(180)
   DEF AsyncTargetNeutralLifecycleEpisodeAtBudget,
       AsyncTargetNeutralLifecycleKnownOwnerSet,
       AsyncTargetNeutralLifecycleEpisodeBudget,
       AsyncTargetNeutralLifecycleDiscoveredOwnerSet

THEOREM AsyncTargetNeutralLifecycleDiscoveryStrictlyConsumesBudget ==
  \A candidateCarrier, serveCarrier, liveOwners, known, budget:
    /\ AsyncTargetNeutralLifecycleEpisodeAtBudget(
         candidateCarrier, serveCarrier,
         liveOwners, known, budget)
    /\ AsyncTargetNeutralLifecycleDiscoveredOwnerSet(
         liveOwners, known) # {}
    => AsyncTargetNeutralLifecycleKnownAdvanceGoal(
         candidateCarrier, serveCarrier,
         liveOwners, known, budget)
BY AsyncTargetNeutralLifecycleOwnerCarrierIsFinite,
   FS_Union, FS_Subset, FS_CardinalityType, IsaT(240)
   DEF AsyncTargetNeutralLifecycleKnownAdvanceGoal,
       AsyncTargetNeutralLifecycleEpisodeAtBudget,
       AsyncTargetNeutralLifecycleKnownOwnerSet,
       AsyncTargetNeutralLifecycleEpisodeBudget,
       AsyncTargetNeutralLifecycleDiscoveredOwnerSet

AsyncTargetNeutralLifecycleBudgetOrdering == OpToRel(<, Nat)

THEOREM AsyncTargetNeutralLifecycleBudgetOrderingIsWellFounded ==
  IsWellFoundedOn(AsyncTargetNeutralLifecycleBudgetOrdering, Nat)
BY NatLessThanWellFounded
   DEF AsyncTargetNeutralLifecycleBudgetOrdering

THEOREM AsyncCandidateLifecycleReviewedTokenOwnsOneOrigin ==
  \A state, node,
     token \in
       AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn(state, node),
     left, right:
    /\ token.origin = left
    /\ token.origin = right
    => left = right
BY Isa
   DEF AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn,
       AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter,
       AsyncCandidateLifecyclePhysicalOwnerTokensForNodeIn,
       AsyncCandidateLifecyclePhysicalOwnerToken,
       AsyncCandidateLifecycleServiceOwnerTokensForNodeIn,
       AsyncCandidateLifecycleServiceOwnerToken,
       AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter,
       AsyncCandidateLifecycleDurableOwnerToken

AsyncCandidateLifecycleFinalStateFromCompacted(state) ==
  LET serveIngressState ==
        AsyncCandidateLifecycleStateAfterServeIngressAdmission(state)
  IN AsyncCandidateLifecycleStateAfterTimeoutOwnership(
       serveIngressState,
       AsyncCandidateLifecycleStateAfterAdmission(serveIngressState))

AsyncCandidateLifecycleNoFreshOrdinaryOriginsIn(state) ==
  \A node \in ValidatorIds:
    AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(state, node) = {}

AsyncCandidateLifecycleFreshClockReservationFitsIn(state) ==
  \A node \in ValidatorIds:
    AsyncTimeoutLifecycleConsumesFreshOrdinal(state, node)
      => AsyncCandidateLifecycleClockOwnerCountIn(state, node) = 0

AsyncCandidateLifecyclePerNodeCapacityRespected(state) ==
  AsyncCandidateLifecycleReviewedCapacityInvariantIn(state)

AsyncControlServiceSlotTransition ==
  LET resetState ==
        AsyncControlServiceStateAfterReset(
          asyncControlServiceState,
          AsyncControlServiceResetNodesThisStep)
      admittedState ==
        IF AsyncControlServiceAdmissionsThisStep = {}
        THEN resetState
        ELSE AsyncControlServiceStateAfterAdmission(
               resetState,
               CHOOSE item \in
                 AsyncControlServiceAdmissionsThisStep: TRUE)
      servicedState ==
        IF AsyncControlServicesThisStep = {}
        THEN admittedState
        ELSE AsyncControlServiceStateAfterService(
               admittedState,
               CHOOSE item \in AsyncControlServicesThisStep: TRUE)
      responseRetirementState ==
        AsyncCertifiedResponseClaimStateAfterRetirement(servicedState)
      responseState ==
        IF AsyncCertifiedResponseClaimAdmissionsThisStep = {}
        THEN responseRetirementState
        ELSE AsyncCertifiedResponseClaimStateAfterAdmission(
               responseRetirementState,
               CHOOSE item \in
                 AsyncCertifiedResponseClaimAdmissionsThisStep: TRUE)
      candidateReclamationState ==
        AsyncCandidateServiceStateAfterReclamation(responseState)
      candidateMarkedState ==
        IF AsyncCandidateServicesThisStep # {}
        THEN AsyncCandidateServiceStateAfterSuccessfulService(
               candidateReclamationState,
               CHOOSE candidate \in AsyncCandidateServicesThisStep: TRUE)
        ELSE IF AsyncCandidateTerminalDiscardsThisStep # {}
             THEN AsyncCandidateServiceStateAfterTerminalRetirement(
                    candidateReclamationState,
                    CHOOSE candidate \in
                      AsyncCandidateTerminalDiscardsThisStep: TRUE)
             ELSE candidateReclamationState
      candidateOwnedState ==
        IF AsyncCandidateLifecycleDeparturesThisStep # {}
        THEN AsyncCandidateLifecycleStateAfterServiceSlotTransfer(
               candidateMarkedState,
               CHOOSE candidate \in
                 AsyncCandidateLifecycleDeparturesThisStep: TRUE)
        ELSE candidateMarkedState
      candidateServiceState ==
        candidateOwnedState
      carrierState ==
        AsyncCandidateLifecycleStateAfterCarrierUpdate(
          candidateServiceState)
      compactedState ==
        AsyncCandidateLifecycleStateAfterCompaction(carrierState)
      serveIngressState ==
        AsyncCandidateLifecycleStateAfterServeIngressAdmission(
          compactedState)
      lifecycleState ==
        AsyncCandidateLifecycleStateAfterAdmission(serveIngressState)
      finalState ==
        AsyncCandidateLifecycleStateAfterTimeoutOwnership(
          serveIngressState, lifecycleState)
  IN /\ AsyncFreshServeIngressAdmissionsAreSingularThisStep
     /\ AsyncFreshServeIngressSchedulerReservationMatchesIn(
          compactedState)
     /\ AsyncCandidateLifecycleReservationsAvailableIn(serveIngressState)
     /\ AsyncCandidateTerminalServiceReservationAvailableIn(
          candidateReclamationState)
     /\ AsyncCandidateServiceReservationAvailableIn(candidateMarkedState)
     /\ \A node \in ValidatorIds:
          AsyncNewCandidateLifecycleOriginsForNodeIn(
            serveIngressState, node)
            \subseteq AsyncOrderedScheduledOriginsForNodeAfter(node)
     /\ AsyncCandidateLifecyclePerNodeCapacityRespected(finalState)
     /\ AsyncCandidateServiceOwnerPartitionInvariantIn(finalState)
     /\ Cardinality(
          finalState.candidateServiceMarkers
            \cup finalState.candidateTerminalTombstones)
          <= AsyncCandidateServiceRecordCapacity
     /\ asyncControlServiceState' = finalState

(***************************************************************************
Reviewed lifecycle capacity is a partition, not an algebraic assertion.
Every retained record occupies exactly one ordinary active, ordinary dormant,
or BeginTimeout clock bucket.  A due clock whose candidate has not yet been
materialized occupies the one synthetic clock reservation instead.  The
ordinary table and clock reservation are disjoint, so a transition which only
continues already-admitted roots (including materializing a reserved timeout)
cannot be disabled by the final capacity check.  A genuinely fresh ordinary
root is the only case which may receive bounded admission backpressure; its
physical source remains owned because the same global transition fail-closes.
***************************************************************************)
THEOREM AsyncCandidateLifecycleReviewedBucketsPartitionRecords ==
  \A state, node:
    /\ node \in ValidatorIds
    /\ IsFiniteSet(state.candidateLifecycleAdmissions)
    => /\ AsyncCandidateLifecycleRecordsForNodeIn(state, node)
             = AsyncCandidateLifecycleActiveOrdinaryBucketIn(state, node)
                 \cup
               AsyncCandidateLifecycleDormantOrdinaryBucketIn(state, node)
                 \cup
               AsyncCandidateLifecycleClockRecordBucketIn(state, node)
       /\ AsyncCandidateLifecycleActiveOrdinaryBucketIn(state, node)
            \cap
          AsyncCandidateLifecycleDormantOrdinaryBucketIn(state, node)
            = {}
       /\ AsyncCandidateLifecycleOrdinaryRecordBucketIn(state, node)
            \cap
          AsyncCandidateLifecycleClockRecordBucketIn(state, node)
            = {}
BY Isa
   DEF AsyncCandidateLifecycleRecordsForNodeIn,
       AsyncCandidateLifecycleClockRecordBucketIn,
       AsyncCandidateLifecycleOrdinaryRecordBucketIn,
       AsyncCandidateLifecycleActiveOrdinaryBucketIn,
       AsyncCandidateLifecycleDormantOrdinaryBucketIn

THEOREM AsyncCandidateLifecycleDormantBucketsSeparateReplayAndService ==
  \A state, node:
    /\ node \in ValidatorIds
    /\ IsFiniteSet(state.candidateLifecycleAdmissions)
    => /\ AsyncCandidateLifecycleDormantOrdinaryBucketIn(state, node)
             = AsyncCandidateLifecycleDormantReplayableBucketIn(
                 state, node)
                 \cup
               AsyncCandidateLifecycleDormantServicedBucketIn(
                 state, node)
       /\ AsyncCandidateLifecycleDormantReplayableBucketIn(state, node)
            \cap
          AsyncCandidateLifecycleDormantServicedBucketIn(state, node)
            = {}
       /\ \A record \in
              AsyncCandidateLifecycleDormantServicedBucketIn(state, node):
            AsyncCandidateLifecycleServiceRecordCoversIn(state, record)
BY Isa
   DEF AsyncCandidateLifecycleDormantOrdinaryBucketIn,
       AsyncCandidateLifecycleDormantReplayableBucketIn,
       AsyncCandidateLifecycleDormantServicedBucketIn,
       AsyncCandidateLifecycleServiceRecordCoversIn

THEOREM AsyncCandidateLifecycleActiveRecordsInjectIntoPhysicalOwners ==
  AsyncCandidateLifecycleSchedulerCoverageInvariant
    => \A record \in AsyncCandidateLifecycleAdmissions:
         ~record.retired
           => record.origin
                \in AsyncScheduledCandidateOriginsForNode(record.node)
BY Isa
   DEF AsyncCandidateLifecycleSchedulerCoverageInvariant,
       AsyncCandidateLifecycleActiveRecords,
       AsyncCandidateLifecycleRecordCoversScheduledOrigin

THEOREM AsyncCandidateLifecycleTransientMarkerRetainsItsReservation ==
  \A state, record:
    /\ record \in state.candidateLifecycleAdmissions
    /\ record.retired
    /\ AsyncCandidateLifecycleServiceRecordCoversIn(state, record)
    /\ ~AsyncCandidateLifecyclePermanentlyObsoleteAfter(record)
    => record \in
         (AsyncCandidateLifecycleStateAfterCompaction(state))
           .candidateLifecycleAdmissions
BY Isa
   DEF AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleRetirementCoveredIn,
       AsyncCandidateLifecycleDormantReservationOwnedAfter

THEOREM AsyncCandidateLifecycleDormantDurableSourceKeepsReservation ==
  \A state, record:
    /\ record \in state.candidateLifecycleAdmissions
    /\ record.retired
    /\ record.origin.phase
         \in AsyncCandidateLifecycleDurableReplayRootKinds
    /\ record.origin.context = context'
    /\ record.origin.height = height'
    /\ ~AsyncNodeHasDecisionIn(record.node, context', decisions')
    /\ record.origin
         \in AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter(
              record.node)
    => record \in
         (AsyncCandidateLifecycleStateAfterCompaction(state))
           .candidateLifecycleAdmissions
BY Isa
   DEF AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleRetirementCoveredIn,
       AsyncCandidateLifecyclePermanentlyObsoleteAfter,
       AsyncCandidateLifecycleDormantReservationOwnedAfter,
       AsyncCandidateLifecycleViewScopedRootKinds,
       AsyncCandidateLifecycleDurableReplayRootKinds

THEOREM AsyncCandidateLifecycleStrictViewCompactsDormantEpisodeRoot ==
  \A state, record:
    /\ record \in state.candidateLifecycleAdmissions
    /\ record.retired
    /\ record.origin.phase
         \in AsyncCandidateLifecycleViewScopedRootKinds
    /\ record.origin.view < nodeView'[record.node]
    => record \notin
         (AsyncCandidateLifecycleStateAfterCompaction(state))
           .candidateLifecycleAdmissions
BY Isa
   DEF AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleRetirementCoveredIn,
       AsyncCandidateLifecyclePermanentlyObsoleteAfter

THEOREM AsyncCandidateLifecycleReviewedBucketsImplyPerNodeCapacity ==
  \A state:
    /\ IsFiniteSet(state.candidateLifecycleAdmissions)
    /\ AsyncCandidateLifecycleReviewedCapacityInvariantIn(state)
    => \A node \in ValidatorIds:
         Cardinality(
           AsyncCandidateLifecycleRecordsForNodeIn(state, node))
           + (IF AsyncUnmaterializedTimeoutLifecycleReservationIn(
                    state, node)
              THEN 1 ELSE 0)
           <= AsyncCandidateLifecyclePerNodeCapacity
BY AsyncCandidateLifecycleReviewedBucketsPartitionRecords,
   FS_Injection, FS_Product, FS_Interval, FS_CardinalityType,
   IsaT(300)
   DEF AsyncCandidateLifecycleReviewedCapacityInvariantIn,
       AsyncCandidateLifecycleSlotInjectionInvariantIn,
       AsyncCandidateLifecycleSlotProjectionIn,
       AsyncCandidateLifecycleReviewedOwnerTokensIn,
       AsyncCandidateLifecycleRecordOwnerToken,
       AsyncCandidateLifecycleClockOwnerToken,
       AsyncCandidateLifecycleSlotAddresses,
       AsyncCandidateLifecycleSlots,
       AsyncCandidateLifecycleOrdinarySlots,
       AsyncCandidateLifecyclePerNodeCapacity

THEOREM AsyncCandidateLifecycleSlotInjectionBoundsGlobalOwners ==
  \A state:
    /\ IsFiniteSet(state.candidateLifecycleAdmissions)
    /\ AsyncCandidateLifecycleReviewedCapacityInvariantIn(state)
    => Cardinality(
         AsyncCandidateLifecycleReviewedOwnerTokensIn(state))
         <= AsyncCandidateLifecycleCapacity
BY FS_Injection, FS_Product, FS_Interval, FS_CardinalityType,
   IsaT(300)
   DEF AsyncCandidateLifecycleReviewedCapacityInvariantIn,
       AsyncCandidateLifecycleSlotInjectionInvariantIn,
       AsyncCandidateLifecycleSlotProjectionIn,
       AsyncCandidateLifecycleSlotAddresses,
       AsyncCandidateLifecycleSlots,
       AsyncCandidateLifecycleOrdinarySlots,
       AsyncCandidateLifecycleCapacity

THEOREM AsyncCandidateLifecyclePhysicalTokensCoverScheduledOriginsAfter ==
  \A node \in ValidatorIds:
    /\ AsyncRuntimeTypeInvariant'
    /\ AsyncIoTypeInvariant'
    /\ AsyncDeferredTypeInvariant'
    => {token.origin:
          token \in
            AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter(node)}
         = AsyncScheduledCandidateOriginsForNodeAfter(node)
BY IsaT(600)
   DEF AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter,
       AsyncCandidateLifecyclePhysicalOwnerTokensForNodeIn,
       AsyncCandidateLifecyclePhysicalOwnerToken,
       AsyncScheduledCandidateOriginsForNodeAfter,
       AsyncScheduledCandidateOriginsForNodeIn,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoWorkContentTypeInvariant,
       AsyncDeferredTypeInvariant, AsyncDeferredContentTypeInvariant,
       AsyncCommandQueueOwnership, AsyncCausalQueueOwnership,
       SequenceSet

THEOREM AsyncCandidateLifecycleDurableTokensCoverReplayOriginsAfter ==
  \A node \in ValidatorIds:
    {token.origin:
       token \in
         AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter(node)}
      = AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter(node)
BY Isa
   DEF AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter,
       AsyncCandidateLifecycleDurableOwnerToken,
       AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter,
       SequenceSet

THEOREM AsyncCandidateLifecycleDurableOwnerCarrierIsBounded ==
  \A node \in ValidatorIds:
    Cardinality(
      AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter(node))
      <= AsyncDormantDurableLifecycleCapacity
BY FS_Interval, FS_Image, FS_Union, FS_CardinalityType, IsaT(600)
   DEF AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter,
       AsyncCandidateLifecycleDurableOwnerToken,
       AsyncDormantDurableLifecycleCapacity,
       FreshRestartCandidateSequence,
       RestartReplay,
       RestartDecisionReplay,
       RestartLockedBodyReplay,
       RestartSignatureReplay,
       RestartTimeoutOrProposalReplay,
       RestartPrepareReplayIfActive,
       RestartLockedCommitReplayIfActive,
       HistoricalLockedRetransmitSuccessors,
       FreshCandidateSequence

THEOREM AsyncCandidateLifecycleServiceOwnerCarrierIsSlotBounded ==
  \A state, node:
    /\ node \in ValidatorIds
    /\ AsyncCandidateServiceOwnerPartitionInvariantIn(state)
    /\ AsyncCandidateLifecycleSlotInjectionInvariantIn(state)
    => Cardinality(
         AsyncCandidateLifecycleServiceOwnerTokensForNodeIn(
           state, node))
         <= AsyncServicedCandidateLifecycleCapacity
BY FS_Injection, FS_Image, FS_Interval, FS_CardinalityType,
   IsaT(300)
   DEF AsyncCandidateLifecycleServiceOwnerTokensForNodeIn,
       AsyncCandidateLifecycleServiceOwnerToken,
       AsyncCandidateServiceOwnerPartitionInvariantIn,
       AsyncCandidateLifecycleSlotInjectionInvariantIn,
       AsyncCandidateLifecycleSlotProjectionIn,
       AsyncCandidateLifecycleReviewedOwnerTokensIn,
       AsyncCandidateLifecycleRecordOwnerToken,
       AsyncCandidateLifecycleClockOwnerToken,
       AsyncCandidateLifecycleServiceRecordCoversIn,
       AsyncCandidateLifecycleServicedSlots,
       AsyncServicedCandidateLifecycleCapacity

THEOREM AsyncCandidateLifecyclePhysicalAndDurableOwnersFitActiveSlots ==
  \A node \in ValidatorIds:
    /\ AsyncRuntimeTypeInvariant'
    /\ AsyncIoTypeInvariant'
    /\ AsyncDeferredTypeInvariant'
    => Cardinality(
         AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter(node)
           \cup
         AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter(node))
         <= AsyncReviewedActiveCandidateLifecycleCapacity
BY AsyncCandidateLifecycleDurableOwnerCarrierIsBounded,
   FS_Interval, FS_Image, FS_Union, FS_CardinalityType, IsaT(900)
   DEF AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter,
       AsyncCandidateLifecyclePhysicalOwnerTokensForNodeIn,
       AsyncCandidateLifecyclePhysicalOwnerToken,
       AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoCapacityTypeInvariant,
       AsyncDeferredTypeInvariant, AsyncDeferredContentTypeInvariant,
       AsyncReviewedActiveCandidateLifecycleCapacity,
       AsyncActiveCandidateLifecycleCapacity,
       AsyncCausalCandidateLifecycleCapacity,
       AsyncDormantDurableLifecycleCapacity

THEOREM AsyncCandidateLifecycleCompactedStateHasSemanticOwnerCoverage ==
  \A state, node:
    LET carrierState ==
          AsyncCandidateLifecycleStateAfterCarrierUpdate(state)
        compactedState ==
          AsyncCandidateLifecycleStateAfterCompaction(carrierState)
    IN /\ node \in ValidatorIds
       /\ AsyncRuntimeTypeInvariant'
       /\ AsyncIoTypeInvariant'
       /\ AsyncDeferredTypeInvariant'
       => AsyncCandidateLifecycleReviewedSemanticCoverageIn(
            compactedState, node)
BY AsyncCandidateLifecyclePhysicalTokensCoverScheduledOriginsAfter,
   AsyncCandidateLifecycleDurableTokensCoverReplayOriginsAfter,
   IsaT(600)
   DEF AsyncCandidateLifecycleReviewedSemanticCoverageIn,
       AsyncCandidateLifecycleLiveOrdinaryOriginCarrierIn,
       AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn,
       AsyncCandidateLifecycleServiceOwnerTokensForNodeIn,
       AsyncCandidateLifecycleServiceOwnerToken,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleCarrierUpdatedAdmissions,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleRetirementCoveredIn,
       AsyncCandidateLifecycleDormantReservationOwnedAfter,
       AsyncCandidateLifecycleServiceRecordCoversIn,
       AsyncCandidateLifecycleOrdinaryOriginsForNodeIn,
       AsyncCandidateLifecycleOrdinaryRecordBucketIn,
       AsyncCandidateLifecycleClockRecordBucketIn,
       AsyncCandidateLifecycleRecordsForNodeIn,
       AsyncNewCandidateLifecycleOriginsForNodeIn,
       AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn,
       AsyncTimeoutLifecycleNewOriginsForNodeIn

THEOREM AsyncCandidateLifecycleCompactedStateHasActiveOwnerCoverage ==
  \A state, node:
    LET carrierState ==
          AsyncCandidateLifecycleStateAfterCarrierUpdate(state)
        compactedState ==
          AsyncCandidateLifecycleStateAfterCompaction(carrierState)
    IN /\ node \in ValidatorIds
       /\ AsyncRuntimeTypeInvariant'
       /\ AsyncIoTypeInvariant'
       /\ AsyncDeferredTypeInvariant'
       /\ AsyncCandidateServiceOwnerPartitionInvariantIn(compactedState)
       => AsyncCandidateLifecycleReviewedActiveCoverageIn(
            compactedState, node)
BY AsyncCandidateLifecyclePhysicalTokensCoverScheduledOriginsAfter,
   AsyncCandidateLifecycleDurableTokensCoverReplayOriginsAfter,
   IsaT(600)
   DEF AsyncCandidateLifecycleReviewedActiveCoverageIn,
       AsyncCandidateLifecycleLiveActiveOriginCarrierIn,
       AsyncCandidateLifecycleActiveOriginsForNodeIn,
       AsyncCandidateLifecycleReviewedActiveOwnerTokensForNodeAfter,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleCarrierUpdatedAdmissions,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleRetirementCoveredIn,
       AsyncCandidateLifecycleDormantReservationOwnedAfter,
       AsyncCandidateLifecycleServiceRecordCoversIn,
       AsyncCandidateLifecycleRecordsForNodeIn,
       AsyncNewCandidateLifecycleOriginsForNodeIn,
       AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn,
       AsyncTimeoutLifecycleNewOriginsForNodeIn

THEOREM AsyncCandidateLifecycleSemanticCoverageGivesOwnerInjection ==
  \A state, node:
    /\ node \in ValidatorIds
    /\ IsFiniteSet(
         AsyncCandidateLifecycleLiveOrdinaryOriginCarrierIn(state, node))
    /\ IsFiniteSet(
         AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn(state, node))
    /\ AsyncCandidateLifecycleReviewedSemanticCoverageIn(state, node)
    => AsyncCandidateLifecycleSemanticOwnerInjectionIn(state, node)
BY FS_Injection, IsaT(300)
   DEF AsyncCandidateLifecycleSemanticOwnerInjectionIn,
       AsyncCandidateLifecycleSemanticOwnerProjectionIn,
       AsyncCandidateLifecycleSemanticOwnerForOriginIn,
       AsyncCandidateLifecycleReviewedSemanticCoverageIn

THEOREM AsyncCandidateLifecycleActiveCoverageGivesOwnerInjection ==
  \A state, node:
    /\ node \in ValidatorIds
    /\ IsFiniteSet(
         AsyncCandidateLifecycleLiveActiveOriginCarrierIn(state, node))
    /\ IsFiniteSet(
         AsyncCandidateLifecycleReviewedActiveOwnerTokensForNodeAfter(node))
    /\ AsyncCandidateLifecycleReviewedActiveCoverageIn(state, node)
    => AsyncCandidateLifecycleActiveOwnerInjectionIn(state, node)
BY FS_Injection, IsaT(300)
   DEF AsyncCandidateLifecycleActiveOwnerInjectionIn,
       AsyncCandidateLifecycleActiveOwnerProjectionIn,
       AsyncCandidateLifecycleActiveOwnerForOriginIn,
       AsyncCandidateLifecycleReviewedActiveCoverageIn

THEOREM AsyncCandidateLifecycleReviewedSemanticOwnersFitOrdinaryCapacity ==
  \A state, node:
    /\ node \in ValidatorIds
    /\ AsyncRuntimeTypeInvariant'
    /\ AsyncIoTypeInvariant'
    /\ AsyncDeferredTypeInvariant'
    /\ AsyncCandidateServiceOwnerPartitionInvariantIn(state)
    /\ AsyncCandidateLifecycleSlotInjectionInvariantIn(state)
    => Cardinality(
         AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn(
           state, node))
         <= AsyncCandidateLifecycleOrdinaryCapacity
BY AsyncCandidateLifecycleServiceOwnerCarrierIsSlotBounded,
   AsyncCandidateLifecyclePhysicalAndDurableOwnersFitActiveSlots,
   FS_Interval, FS_Image, FS_Union, FS_CardinalityType, IsaT(900)
   DEF AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn,
       AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter,
       AsyncCandidateLifecyclePhysicalOwnerTokensForNodeIn,
       AsyncCandidateLifecyclePhysicalOwnerToken,
       AsyncCandidateLifecycleServiceOwnerTokensForNodeIn,
       AsyncCandidateLifecycleServiceOwnerToken,
       AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter,
       AsyncCandidateServiceOwnerPartitionInvariantIn,
       AsyncCandidateLifecycleSlotInjectionInvariantIn,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoCapacityTypeInvariant,
       AsyncDeferredTypeInvariant, AsyncDeferredContentTypeInvariant,
       AsyncCandidateLifecycleOrdinaryCapacity,
       AsyncServicedCandidateLifecycleCapacity,
       AsyncActiveCandidateLifecycleCapacity,
       AsyncCausalCandidateLifecycleCapacity,
       AsyncSemanticIngressLifecycleCapacity,
       AsyncDormantDurableLifecycleCapacity,
       AsyncTerminalCandidateLifecycleCapacity,
       AsyncReviewedActiveCandidateLifecycleCapacity

THEOREM AsyncCandidateLifecycleReviewedOwnerInjectionProvidesReservations ==
  \A state, node:
    /\ node \in ValidatorIds
    /\ AsyncCandidateLifecycleSlotInjectionInvariantIn(state)
    /\ AsyncCandidateLifecycleActiveOwnerInjectionIn(state, node)
    /\ Cardinality(
         AsyncCandidateLifecycleReviewedActiveOwnerTokensForNodeAfter(node))
         <= AsyncReviewedActiveCandidateLifecycleCapacity
    => Cardinality(
         AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(state, node))
         <= Cardinality(
              AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn(
                state, node))
BY FS_Injection, FS_Subset, FS_Union, FS_Interval,
   FS_CardinalityType, IsaT(600)
   DEF AsyncCandidateLifecycleActiveOwnerInjectionIn,
       AsyncCandidateLifecycleLiveActiveOriginCarrierIn,
       AsyncCandidateLifecycleActiveOriginsForNodeIn,
       AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn,
       AsyncCandidateLifecycleUsedActiveSlotsForNodeIn,
       AsyncCandidateLifecycleActiveSlots,
       AsyncCandidateLifecycleSlotInjectionInvariantIn,
       AsyncCandidateLifecycleSlotProjectionIn,
       AsyncCandidateLifecycleReviewedOwnerTokensIn,
       AsyncCandidateLifecycleRecordOwnerToken,
       AsyncCandidateLifecycleClockOwnerToken,
       AsyncCandidateLifecycleSlotAddresses,
       AsyncReviewedActiveCandidateLifecycleCapacity

THEOREM AsyncCandidateLifecycleCompactedStateProvidesFreshReservations ==
  \A state, node:
    LET carrierState ==
          AsyncCandidateLifecycleStateAfterCarrierUpdate(state)
        compactedState ==
          AsyncCandidateLifecycleStateAfterCompaction(carrierState)
    IN /\ node \in ValidatorIds
       /\ IsFiniteSet(state.candidateLifecycleAdmissions)
       /\ IsFiniteSet(state.candidateServiceMarkers)
       /\ IsFiniteSet(state.candidateTerminalTombstones)
       /\ AsyncRuntimeTypeInvariant'
       /\ AsyncIoTypeInvariant'
       /\ AsyncDeferredTypeInvariant'
       /\ AsyncCandidateLifecycleSlotInjectionInvariantIn(compactedState)
       /\ AsyncCandidateServiceOwnerPartitionInvariantIn(compactedState)
       => Cardinality(
            AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(
              compactedState, node))
            <= Cardinality(
                 AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn(
                   compactedState, node))
BY AsyncCandidateLifecycleCompactedStateHasActiveOwnerCoverage,
   AsyncCandidateLifecycleActiveCoverageGivesOwnerInjection,
   AsyncCandidateLifecyclePhysicalAndDurableOwnersFitActiveSlots,
   AsyncCandidateLifecycleReviewedOwnerInjectionProvidesReservations,
   FS_Image, FS_Union, FS_Subset, FS_CardinalityType, IsaT(900)
   DEF AsyncCandidateLifecycleLiveActiveOriginCarrierIn,
       AsyncCandidateLifecycleReviewedActiveOwnerTokensForNodeAfter,
       AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter,
       AsyncCandidateLifecyclePhysicalOwnerTokensForNodeIn,
       AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleStateAfterCompaction

THEOREM AsyncCandidateLifecycleCapacityCannotBlockOwnedContinuation ==
  \A state:
    /\ IsFiniteSet(state.candidateLifecycleAdmissions)
    /\ AsyncCandidateLifecycleReviewedCapacityInvariantIn(state)
    /\ AsyncCandidateLifecycleNoFreshOrdinaryOriginsIn(state)
    /\ AsyncCandidateLifecycleFreshClockReservationFitsIn(state)
    => AsyncCandidateLifecycleReviewedCapacityInvariantIn(
         AsyncCandidateLifecycleFinalStateFromCompacted(state))
BY FS_Injection, FS_Union, FS_Subset, FS_CardinalityType,
   IsaT(600)
   DEF AsyncCandidateLifecycleReviewedCapacityInvariantIn,
       AsyncCandidateLifecycleSlotInjectionInvariantIn,
       AsyncCandidateLifecycleSlotProjectionIn,
       AsyncCandidateLifecycleReviewedOwnerTokensIn,
       AsyncCandidateLifecycleRecordOwnerToken,
       AsyncCandidateLifecycleClockOwnerToken,
       AsyncCandidateLifecycleSlotAddresses,
       AsyncCandidateLifecycleNoFreshOrdinaryOriginsIn,
       AsyncCandidateLifecycleFreshClockReservationFitsIn,
       AsyncCandidateLifecycleFinalStateFromCompacted,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncCandidateLifecycleNewAdmissions,
       AsyncCandidateLifecycleAdmissionOrdinalFor,
       AsyncCandidateLifecycleAdmissionSlotFor,
       AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn,
       AsyncCandidateLifecycleUsedOrdinarySlotsForNodeIn,
       AsyncCandidateLifecycleClockOwnerCountIn,
       AsyncCandidateLifecycleClockRecordBucketIn,
       AsyncCandidateLifecycleOrdinaryRecordBucketIn,
       AsyncCandidateLifecycleRecordsForNodeIn,
       AsyncCandidateLifecycleRecordsForIn,
       AsyncUnmaterializedTimeoutLifecycleReservationIn,
       AsyncUnmaterializedTimeoutLifecycleReservationNodesIn,
       AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn,
       AsyncTimeoutLifecycleNewOriginsForNodeIn,
       AsyncTimeoutLifecycleConsumesFreshOrdinal

THEOREM AsyncCandidateLifecycleCarrierInjectionProvidesFreshReservations ==
  \A state, node, liveOriginCarrier:
    /\ node \in ValidatorIds
    /\ IsFiniteSet(liveOriginCarrier)
    /\ Cardinality(liveOriginCarrier)
         <= AsyncReviewedActiveCandidateLifecycleCapacity
    /\ AsyncCandidateLifecycleSlotInjectionInvariantIn(state)
    /\ AsyncCandidateLifecycleActiveOriginsForNodeIn(state, node)
         \cup
       AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(state, node)
         \subseteq liveOriginCarrier
    /\ AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(state, node)
         \cap
       AsyncCandidateLifecycleActiveOriginsForNodeIn(state, node)
         = {}
    => Cardinality(
         AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(state, node))
         <= Cardinality(
              AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn(
                state, node))
BY FS_Injection, FS_Subset, FS_Union, FS_Interval,
   FS_CardinalityType, IsaT(600)
   DEF AsyncCandidateLifecycleSlotInjectionInvariantIn,
       AsyncCandidateLifecycleSlotProjectionIn,
       AsyncCandidateLifecycleReviewedOwnerTokensIn,
       AsyncCandidateLifecycleRecordOwnerToken,
       AsyncCandidateLifecycleClockOwnerToken,
       AsyncCandidateLifecycleSlotAddresses,
       AsyncCandidateLifecycleActiveOriginsForNodeIn,
       AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn,
       AsyncCandidateLifecycleUsedActiveSlotsForNodeIn,
       AsyncCandidateLifecycleActiveSlots,
       AsyncReviewedActiveCandidateLifecycleCapacity

THEOREM AsyncCandidateLifecycleDistinctNewRootsReceiveDistinctOwnership ==
  \A state, node, left, right:
    /\ node \in ValidatorIds
    /\ AsyncCandidateLifecycleReservationsAvailableIn(state)
    /\ left \in
         AsyncNewCandidateLifecycleOriginsForNodeIn(state, node)
    /\ right \in
         AsyncNewCandidateLifecycleOriginsForNodeIn(state, node)
    /\ left # right
    => /\ AsyncCandidateLifecycleAdmissionOrdinalFor(
              state, node, left)
            # AsyncCandidateLifecycleAdmissionOrdinalFor(
                state, node, right)
       /\ AsyncCandidateLifecycleAdmissionSlotFor(state, node, left)
            # AsyncCandidateLifecycleAdmissionSlotFor(state, node, right)
BY FS_Interval, FS_CardinalityType, IsaT(600)
   DEF AsyncCandidateLifecycleReservationsAvailableIn,
       AsyncCandidateLifecycleOrdinaryReservationsAvailableIn,
       AsyncCandidateLifecycleClockReservationAvailableIn,
       AsyncCandidateLifecycleAdmissionOrdinalFor,
       AsyncCandidateLifecycleAdmissionSlotFor,
       AsyncOrdinaryNewCandidateLifecyclePredecessorsFor,
       AsyncCandidateLifecycleFreeSlotPredecessorsFor,
       AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn,
       AsyncTimeoutLifecycleNewOriginsForNodeIn

THEOREM AsyncCandidateLifecycleHighWatermarkAdvancesByFullFreshSet ==
  \A state, node:
    node \in ValidatorIds
      => (AsyncCandidateLifecycleStateAfterAdmission(state))
           .candidateLifecycleNextOrdinal[node]
           = state.candidateLifecycleNextOrdinal[node]
               + (IF AsyncTimeoutLifecycleConsumesFreshOrdinal(state, node)
                  THEN 1 ELSE 0)
               + Cardinality(
                   AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(
                     state, node))
BY Isa
   DEF AsyncCandidateLifecycleStateAfterAdmission

THEOREM AsyncServeIngressSharedHighWatermarkAdvancesByFreshTickets ==
  \A state, node:
    node \in ValidatorIds
      => (AsyncCandidateLifecycleStateAfterServeIngressAdmission(state))
           .candidateLifecycleNextOrdinal[node]
           = state.candidateLifecycleNextOrdinal[node]
               + Cardinality(
                   AsyncFreshServeIngressAdmissionsForNodeThisStep(node))
BY Isa
   DEF AsyncCandidateLifecycleStateAfterServeIngressAdmission

THEOREM AsyncServeIngressReservationPrecedesSameStepCandidateAllocation ==
  \A state, node,
     admission \in AsyncFreshServeIngressAdmissionsForNodeThisStep(node),
     origin \in
       AsyncNewCandidateLifecycleOriginsForNodeIn(
         AsyncCandidateLifecycleStateAfterServeIngressAdmission(state),
         node):
    /\ node \in ValidatorIds
    /\ AsyncFreshServeIngressAdmissionsAreSingularThisStep
    /\ AsyncFreshServeIngressSchedulerReservationMatchesIn(state)
    /\ \/ origin \in
             AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(
               AsyncCandidateLifecycleStateAfterServeIngressAdmission(state),
               node)
       \/ AsyncTimeoutLifecycleConsumesFreshOrdinal(
            AsyncCandidateLifecycleStateAfterServeIngressAdmission(state),
            node)
    => admission.schedulerOrdinal
         < AsyncCandidateLifecycleAdmissionOrdinalFor(
             AsyncCandidateLifecycleStateAfterServeIngressAdmission(state),
             node, origin)
BY FS_CardinalityType, IsaT(300)
   DEF AsyncFreshServeIngressAdmissionsAreSingularThisStep,
       AsyncFreshServeIngressSchedulerReservationMatchesIn,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleAdmissionOrdinalFor,
       AsyncTimeoutLifecycleOrdinalForStep,
       AsyncTimeoutLifecycleConsumesFreshOrdinal,
       AsyncTimeoutLifecycleNewOriginsForNodeIn

THEOREM AsyncCandidateLifecycleFullOrdinaryTableRejectsBeforeSourcePop ==
  \A state, node:
    /\ node \in ValidatorIds
    /\ AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn(state, node) = {}
    /\ AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(state, node) # {}
    => ~AsyncCandidateLifecycleReservationsAvailableIn(state)
BY FS_EmptySet, FS_CardinalityType, Isa
   DEF AsyncCandidateLifecycleReservationsAvailableIn,
       AsyncCandidateLifecycleOrdinaryReservationsAvailableIn

THEOREM AsyncControlServiceTransitionRequiresAtomicLifecycleReservation ==
  AsyncControlServiceSlotTransition
    => LET resetState ==
             AsyncControlServiceStateAfterReset(
               asyncControlServiceState,
               AsyncControlServiceResetNodesThisStep)
           admittedState ==
             IF AsyncControlServiceAdmissionsThisStep = {}
             THEN resetState
             ELSE AsyncControlServiceStateAfterAdmission(
                    resetState,
                    CHOOSE item \in
                      AsyncControlServiceAdmissionsThisStep: TRUE)
           servicedState ==
             IF AsyncControlServicesThisStep = {}
             THEN admittedState
             ELSE AsyncControlServiceStateAfterService(
                    admittedState,
                    CHOOSE item \in AsyncControlServicesThisStep: TRUE)
           responseRetirementState ==
             AsyncCertifiedResponseClaimStateAfterRetirement(servicedState)
           responseState ==
             IF AsyncCertifiedResponseClaimAdmissionsThisStep = {}
             THEN responseRetirementState
             ELSE AsyncCertifiedResponseClaimStateAfterAdmission(
                    responseRetirementState,
                    CHOOSE item \in
                      AsyncCertifiedResponseClaimAdmissionsThisStep: TRUE)
           candidateReclamationState ==
             AsyncCandidateServiceStateAfterReclamation(responseState)
           candidateMarkedState ==
             IF AsyncCandidateServicesThisStep # {}
             THEN AsyncCandidateServiceStateAfterSuccessfulService(
                    candidateReclamationState,
                    CHOOSE candidate \in
                      AsyncCandidateServicesThisStep: TRUE)
             ELSE IF AsyncCandidateTerminalDiscardsThisStep # {}
                  THEN AsyncCandidateServiceStateAfterTerminalRetirement(
                         candidateReclamationState,
                         CHOOSE candidate \in
                           AsyncCandidateTerminalDiscardsThisStep: TRUE)
                  ELSE candidateReclamationState
           candidateOwnedState ==
             IF AsyncCandidateLifecycleDeparturesThisStep # {}
             THEN AsyncCandidateLifecycleStateAfterServiceSlotTransfer(
                    candidateMarkedState,
                    CHOOSE candidate \in
                      AsyncCandidateLifecycleDeparturesThisStep: TRUE)
             ELSE candidateMarkedState
           candidateServiceState ==
             candidateOwnedState
           carrierState ==
             AsyncCandidateLifecycleStateAfterCarrierUpdate(
               candidateServiceState)
           compactedState ==
             AsyncCandidateLifecycleStateAfterCompaction(carrierState)
           serveIngressState ==
             AsyncCandidateLifecycleStateAfterServeIngressAdmission(
               compactedState)
       IN /\ AsyncFreshServeIngressAdmissionsAreSingularThisStep
          /\ AsyncFreshServeIngressSchedulerReservationMatchesIn(
               compactedState)
          /\ AsyncCandidateLifecycleReservationsAvailableIn(
               serveIngressState)
          /\ AsyncCandidateTerminalServiceReservationAvailableIn(
               candidateReclamationState)
          /\ AsyncCandidateServiceReservationAvailableIn(candidateMarkedState)
BY DEF AsyncControlServiceSlotTransition

THEOREM AsyncServeIngressAdmissionConsumesSharedSchedulerOrdinal ==
  \A node \in ValidatorIds,
     admission \in AsyncFreshServeIngressAdmissionsForNodeThisStep(node):
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    => /\ admission.schedulerOrdinal =
             AsyncNextCandidateLifecycleOrdinal(node)
       /\ admission.schedulerOrdinal <
             AsyncNextCandidateLifecycleOrdinal(node)'
BY IsaT(600)
   DEF AsyncControlServiceSlotTransition,
       AsyncFreshServeIngressSchedulerReservationMatchesIn,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateLifecycleStateAfterServiceSlotTransfer,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncNextCandidateLifecycleOrdinal,
       AsyncControlServiceStateTypeInvariant

THEOREM AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal ==
  \A node \in ValidatorIds,
     admission \in AsyncFreshServeIngressAdmissionsForNodeThisStep(node),
     priorOrdinal \in Nat:
    /\ priorOrdinal < AsyncNextCandidateLifecycleOrdinal(node)
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    => priorOrdinal < admission.schedulerOrdinal
BY AsyncServeIngressAdmissionConsumesSharedSchedulerOrdinal, Isa

THEOREM AsyncFreshServeIngressSchedulerOrdinalInjectsAgainstPriorOwners ==
  \A node \in ValidatorIds,
     admission \in AsyncFreshServeIngressAdmissionsForNodeThisStep(node):
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncServeOrdinalInvariant
    /\ AsyncControlServiceSlotTransition
    => /\ \A prior \in asyncServeIngressAdmissions:
              prior.node = node
                => prior.schedulerOrdinal # admission.schedulerOrdinal
       /\ \A record \in AsyncCandidateLifecycleAdmissions:
              record.node = node
                => record.ordinal # admission.schedulerOrdinal
       /\ (AsyncTimeoutLifecycleOwned(node)
             => AsyncTimeoutLifecycleOrdinal(node)
                  # admission.schedulerOrdinal)
BY AsyncServeIngressAdmissionConsumesSharedSchedulerOrdinal, Isa
   DEF AsyncControlServiceStateTypeInvariant,
       AsyncServeOrdinalInvariant,
       AsyncNextCandidateLifecycleOrdinal,
       AsyncTimeoutLifecycleOwned,
       AsyncTimeoutLifecycleOrdinal,
       AsyncCandidateLifecycleAdmissions

THEOREM AsyncSharedSchedulerHighWatermarkIsMonotone ==
  /\ AsyncControlServiceStateTypeInvariant
  /\ AsyncControlServiceSlotTransition
  => \A node \in ValidatorIds:
       AsyncNextCandidateLifecycleOrdinal(node)
         <= AsyncNextCandidateLifecycleOrdinal(node)'
BY IsaT(600)
   DEF AsyncControlServiceSlotTransition,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateLifecycleStateAfterServiceSlotTransfer,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncNextCandidateLifecycleOrdinal,
       AsyncControlServiceStateTypeInvariant

THEOREM AsyncSameHeightRestartRetainsSharedSchedulerHighWatermark ==
  /\ PreGstResponsiveReplay
  /\ AsyncControlServiceStateTypeInvariant
  /\ AsyncControlServiceSlotTransition
  => \A node \in ValidatorIds:
       AsyncNextCandidateLifecycleOrdinal(node)
         <= AsyncNextCandidateLifecycleOrdinal(node)'
BY AsyncSharedSchedulerHighWatermarkIsMonotone, Isa

THEOREM AsyncIgnoredIngressEpisodeCannotConsumeLifecycleCapacity ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCandidateIgnoredWithoutApplicationThisStep(candidate)
    /\ candidate.item # NoAsyncItem
    /\ AsyncCandidateLifecycleRecorded(
         candidate.node, candidate.causalOrigin)
    /\ AsyncControlServiceSlotTransition
    => /\ AsyncNextCandidateServiceOrdinal(candidate.node)'
             = AsyncNextCandidateServiceOrdinal(candidate.node)
       /\ AsyncCandidateServiceRecordsForIdentity(
            AsyncCandidateServiceIdentity(candidate))' = {}
       /\ ~AsyncCandidateLifecycleRecorded(
             candidate.node, candidate.causalOrigin)'
BY IsaT(600)
   DEF AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidateIgnoredWithoutApplicationThisStepSet,
       AsyncCandidateSemanticallyAppliedThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateServiceRecordsForIdentity,
       AsyncCandidateTransientServiceRecordsForIdentity,
       AsyncCandidateTerminalRecordsForIdentity,
       AsyncCandidateServiceIdentity,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateLifecycleIgnoredEpisodeCovers,
       AsyncCandidateLifecycleRetirementCoveredIn,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleCarrierUpdatedAdmissions,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncCandidateLifecycleRecorded,
       AsyncCandidateLifecycleRecordedIn,
       AsyncCandidateLifecycleRecordsFor,
       AsyncCandidateLifecycleRecordsForIn,
       AsyncNextCandidateServiceOrdinal,
       AsyncControlServiceSlotTransition

AsyncCandidateScheduledLifecycleStageIdentityInvariant ==
  \A left, right \in
       QueuedCandidates \cup DeferredCandidates
         \cup CausalCandidates \cup TrackedWorkCandidates:
    /\ left.node = right.node
    /\ left.causalOrigin = right.causalOrigin
    /\ left.kind = right.kind
    => AsyncCandidateServiceIdentity(left)
         = AsyncCandidateServiceIdentity(right)

AsyncCandidateRecordedLifecycleStageIdentityInvariant ==
  \A serviced \in AsyncCandidateServiceTombstones,
     candidate \in
       QueuedCandidates \cup DeferredCandidates
         \cup CausalCandidates \cup TrackedWorkCandidates:
    /\ serviced.node = candidate.node
    /\ serviced.identity.payload.causalOrigin = candidate.causalOrigin
    /\ serviced.phase = candidate.kind
    => serviced.identity = AsyncCandidateServiceIdentity(candidate)

AsyncCandidateLifecycleStageIdentityInvariant ==
  /\ AsyncCandidateScheduledLifecycleStageIdentityInvariant
  /\ AsyncCandidateRecordedLifecycleStageIdentityInvariant

AsyncCandidateServiceLifecycleInvariant ==
  /\ AsyncControlServiceStateTypeInvariant
  /\ AsyncCandidateLifecycleSchedulerCoverageInvariant
  /\ AsyncCandidateLifecycleStageIdentityInvariant
  /\ \A record \in AsyncCandidateServiceMarkers:
       /\ record.context = context
       /\ record.height = height
       /\ record.episodeView >= nodeView[record.node]
       /\ record.generation <= generation[record.node]
       /\ ~NodeHasDecision(record.node)
  /\ \A record \in AsyncCandidateTerminalTombstones:
       /\ record.context = context
       /\ record.height = height
       /\ record.episodeView >= nodeView[record.node]
       /\ record.phase \notin AsyncRestartScopedCandidateServiceKinds
       /\ ~NodeHasDecision(record.node)
  /\ \A candidate \in
       QueuedCandidates \cup DeferredCandidates
         \cup CausalCandidates \cup TrackedWorkCandidates:
       ~AsyncCandidateServiceCoalesced(candidate)

AsyncCandidateServiceTombstoneLifecycleInvariant ==
  AsyncCandidateServiceLifecycleInvariant

AsyncCandidateTransientServiceActive(candidate) ==
  /\ AsyncCandidateTransientServiceMarked(candidate)
  /\ candidate.consumerContext = context
  /\ candidate.height = height
  /\ candidate.consumerGeneration = generation[candidate.node]
  /\ \A record \in AsyncCandidateTransientServiceRecordsFor(candidate):
       record.episodeView >= nodeView[candidate.node]
  /\ \A record \in AsyncCandidateTransientServiceRecordsFor(candidate):
       record.generation = candidate.consumerGeneration
  /\ ~NodeHasDecision(candidate.node)
  /\ ~CandidateScheduled(candidate)

AsyncCandidateTerminalTombstoneActive(candidate) ==
  /\ AsyncCandidateTerminalTombstoned(candidate)
  /\ candidate.consumerContext = context
  /\ candidate.height = height
  /\ \A record \in AsyncCandidateTerminalRecordsFor(candidate):
       record.episodeView >= nodeView[candidate.node]
  /\ ~NodeHasDecision(candidate.node)
  /\ ~CandidateScheduled(candidate)

AsyncCandidateServiceActiveTombstone(candidate) ==
  \/ AsyncCandidateTransientServiceActive(candidate)
  \/ AsyncCandidateTerminalTombstoneActive(candidate)

AsyncCandidateTransientMarkerExitThisStep(candidate) ==
  \/ candidate.consumerContext # context'
  \/ candidate.height # height'
  \/ nodeView'[candidate.node] > nodeView[candidate.node]
  \/ candidate.consumerGeneration # generation'[candidate.node]
  \/ AsyncNodeHasDecisionAfter(candidate.node)
  \/ /\ (PreGstResponsiveRestart \/ PreGstResponsiveReplay)
     /\ candidate.node = asyncRecoveryNode

AsyncCandidateTerminalTombstoneExitThisStep(candidate) ==
  \/ candidate.consumerContext # context'
  \/ candidate.height # height'
  \/ nodeView'[candidate.node] > nodeView[candidate.node]
  \/ AsyncNodeHasDecisionAfter(candidate.node)

AsyncCandidateServiceExitThisStep(candidate) ==
  AsyncCandidateTransientMarkerExitThisStep(candidate)

THEOREM AsyncCandidateServiceIdentityIgnoresConsumerIncarnation ==
  \A left, right \in AsyncCandidateSet:
    /\ left.node = right.node
    /\ left.consumerContext = right.consumerContext
    /\ left.height = right.height
    /\ left.view = right.view
    /\ left.subject = right.subject
    /\ left.kind = right.kind
    /\ AsyncCandidateServicePayload(left)
         = AsyncCandidateServicePayload(right)
    => AsyncCandidateServiceIdentity(left)
         = AsyncCandidateServiceIdentity(right)
BY Isa
   DEF AsyncCandidateServiceIdentity

THEOREM AsyncCandidateServiceIdentityIgnoresSchedulerClass ==
  \A candidate \in AsyncCandidateSet,
     commandClass \in AsyncCommandClasses:
    AsyncCandidateServiceIdentity(
      [candidate EXCEPT !.class = commandClass])
      = AsyncCandidateServiceIdentity(candidate)
BY Isa
   DEF AsyncCandidateServiceIdentity,
       AsyncCandidateServicePayload

THEOREM AsyncCandidateLifecycleAndServiceIdentityIgnoreSchedulerClass ==
  \A leftClass, rightClass, kind, node, blockHeight, roundView,
     subject, item, consumerView, consumerGeneration, evidence,
     bodyIdentity, manifestIdentity, commitmentIdentity:
    LET left ==
          AsyncCandidateAtConsumer(
            leftClass, kind, node, blockHeight, roundView, subject, item,
            consumerView, consumerGeneration, evidence,
            bodyIdentity, manifestIdentity, commitmentIdentity)
        right ==
          AsyncCandidateAtConsumer(
            rightClass, kind, node, blockHeight, roundView, subject, item,
            consumerView, consumerGeneration, evidence,
            bodyIdentity, manifestIdentity, commitmentIdentity)
    IN /\ left.causalOrigin = right.causalOrigin
       /\ AsyncCandidateServiceIdentity(left)
            = AsyncCandidateServiceIdentity(right)
BY Isa
   DEF AsyncCandidateAtConsumer,
       AsyncCandidateWithIdentity,
       AsyncCandidateWithIdentityAndOrigin,
       AsyncCandidateCausalOrigin,
       AsyncCandidateServiceIdentity,
       AsyncCandidateServicePayload

THEOREM AsyncCandidateServiceRouteNeutralResponseRetryIsStable ==
  \A left, right \in AsyncCandidateSet:
    /\ left.node = right.node
    /\ left.consumerContext = right.consumerContext
    /\ left.height = right.height
    /\ left.view = right.view
    /\ left.subject = right.subject
    /\ left.kind = right.kind
    /\ left.item # NoAsyncItem
    /\ right.item # NoAsyncItem
    /\ left.item.kind = "CertifiedResponse"
    /\ right.item =
         [left.item EXCEPT !.source = right.item.source]
    /\ AsyncRouteNeutralCandidateEvidence(left.evidence)
         = AsyncRouteNeutralCandidateEvidence(right.evidence)
    /\ left.causalOrigin = right.causalOrigin
    /\ left.bodyIdentity = right.bodyIdentity
    /\ left.manifestIdentity = right.manifestIdentity
    /\ left.commitmentIdentity = right.commitmentIdentity
    => AsyncCandidateServiceIdentity(left)
         = AsyncCandidateServiceIdentity(right)
BY Isa
   DEF AsyncCandidateServiceIdentity,
       AsyncCandidateServicePayload,
       AsyncRouteNeutralCandidateItem,
       AsyncRouteNeutralCandidateEvidence

THEOREM AsyncCandidateServiceTombstoneCoalescesFreshCandidate ==
  \A candidate:
    AsyncCandidateServiceCoalesced(candidate)
      => /\ CandidateAdmissionCoalesced(candidate)
         /\ FreshCandidateSequence(candidate) = <<>>
         /\ ~ENABLED EnqueueCandidate(candidate)
BY Isa
   DEF CandidateAdmissionCoalesced,
       FreshCandidateSequence,
       EnqueueCandidate,
       AsyncServeLifecycleVars,
       AsyncServeIngressAdmissionVars,
       AsyncRecoveryLifecycleVars

THEOREM AsyncCandidateInternalBodyAvailableStageRetirementCoalescesFreshCandidate ==
  \A candidate:
    AsyncCandidateInternalBodyAvailableStageRetired(candidate)
      => FreshCandidateSequence(candidate) = <<>>
BY Isa
   DEF AsyncCandidateInternalBodyAvailableStageRetired,
       FreshCandidateSequence

THEOREM AsyncCandidateTransientMarkerCoalescesFreshCandidate ==
  \A candidate:
    AsyncCandidateTransientServiceMarked(candidate)
      => /\ CandidateAdmissionCoalesced(candidate)
         /\ FreshCandidateSequence(candidate) = <<>>
         /\ ~ENABLED EnqueueCandidate(candidate)
BY AsyncCandidateServiceTombstoneCoalescesFreshCandidate, Isa
   DEF AsyncCandidateServiceCoalesced,
       AsyncServeLifecycleVars,
       AsyncServeIngressAdmissionVars,
       AsyncRecoveryLifecycleVars

THEOREM AsyncCandidateTerminalTombstoneCoalescesFreshCandidate ==
  \A candidate:
    AsyncCandidateTerminalTombstoned(candidate)
      => /\ CandidateAdmissionCoalesced(candidate)
         /\ FreshCandidateSequence(candidate) = <<>>
         /\ ~ENABLED EnqueueCandidate(candidate)
BY AsyncCandidateServiceTombstoneCoalescesFreshCandidate, Isa
   DEF AsyncCandidateServiceCoalesced,
       AsyncServeLifecycleVars,
       AsyncServeIngressAdmissionVars,
       AsyncRecoveryLifecycleVars

THEOREM AsyncCandidateServiceTombstoneRejectsTransportReadmission ==
  \A item \in AsyncNetworkItems:
    AsyncCandidateServiceCoalesced(DeliveryCandidate(item))
      => /\ IngressPacketPolicyRejected(item)
         /\ ~CanAdmitIngressItem(item)
BY DEF AsyncCandidateServicePacketRetired,
       IngressPacketPolicyRejected,
       CanAdmitIngressItem

\* Successful service keeps the immutable identity through the complete
\* process-generation/view episode.  A same-origin successor or another
\* origin's monotone reducer coverage is not a substitute for this memory:
\* either substitution admits the A -> B -> A replenishment lasso.
THEOREM AsyncCandidateSuccessfulServiceInstallsTransientMarker ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateServicesThisStep = {candidate}
    /\ AsyncCandidateServiceEligibleAfterStep(candidate)
    /\ AsyncControlServiceSlotTransition
    => /\ AsyncCandidateTransientServiceMarked(candidate)'
       /\ ~CandidateScheduled(candidate)'
       /\ AsyncCandidateTransientServiceActive(candidate)'
BY IsaT(600)
   DEF AsyncCandidateServicesThisStep,
       AsyncCandidateSemanticallyAppliedThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateTransientServiceMarked,
       AsyncCandidateTransientServiceRecordsFor,
       AsyncCandidateTransientServiceRecordsForIdentity,
       AsyncCandidateServiceRecordsFor,
       AsyncCandidateServiceRecordsForIdentity,
       AsyncCandidateServiceMarkers,
       AsyncCandidateTerminalTombstones,
       AsyncCandidateServiceTombstones,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceMarkersAfterReset,
       AsyncCandidateServiceRecordRetainedAfterStep,
       AsyncCandidateServiceEligibleAfterStep,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateServiceMarker,
       AsyncCandidateTransientServiceActive,
       AsyncCandidateServiceOwnerPartitionInvariantIn,
       NodeHasDecision,
       AsyncControlServiceSlotTransition

THEOREM AsyncCandidateSuccessfulServiceInstallsTombstone ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCandidateServicesThisStep = {candidate}
    /\ AsyncCandidateServiceEligibleAfterStep(candidate)
    /\ AsyncControlServiceSlotTransition
    => /\ AsyncCandidateServiceTombstoned(candidate)'
       /\ ~CandidateScheduled(candidate)'
BY AsyncCandidateSuccessfulServiceInstallsTransientMarker, Isa
   DEF AsyncCandidateServiceTombstoned,
       AsyncCandidateServiceCoalesced

THEOREM AsyncCandidateSuccessfulServiceAllocatesExactOrdinal ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateServicesThisStep = {candidate}
    /\ AsyncCandidateServiceEligibleAfterStep(candidate)
    /\ ~AsyncCandidateServiceCoalesced(candidate)
    /\ AsyncControlServiceSlotTransition
    => LET node == candidate.node
           ordinal == AsyncNextCandidateServiceOrdinal(node)
       IN /\ AsyncCandidateServiceMarker(
                candidate, nodeView[node],
                candidate.consumerGeneration, ordinal)
              \in AsyncCandidateServiceMarkers'
          /\ AsyncNextCandidateServiceOrdinal(node)' = ordinal + 1
BY IsaT(300)
   DEF AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateServiceCoalesced,
       AsyncCandidateTransientServiceMarked,
       AsyncCandidateServiceRecordsFor,
       AsyncCandidateServiceRecordsForIdentity,
       AsyncCandidateServiceMarkers,
       AsyncCandidateTerminalTombstones,
       AsyncCandidateServiceTombstones,
       AsyncNextCandidateServiceOrdinal,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceMarkersAfterReset,
       AsyncCandidateServiceRecordRetainedAfterStep,
       AsyncCandidateServiceEligibleAfterStep,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateServiceMarker,
       AsyncControlServiceSlotTransition

THEOREM AsyncCandidateTransientMarkerPersistsWithinGeneration ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateTransientServiceActive(candidate)
    /\ AsyncControlServiceSlotTransition
    /\ ~AsyncCandidateTransientMarkerExitThisStep(candidate)
    => AsyncCandidateTransientServiceActive(candidate)'
BY IsaT(300)
   DEF AsyncCandidateTransientServiceActive,
       AsyncCandidateTransientMarkerExitThisStep,
       AsyncCandidateTransientServiceMarked,
       AsyncCandidateTransientServiceRecordsFor,
       AsyncCandidateTransientServiceRecordsForIdentity,
       AsyncCandidateServiceMarkers,
       AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceMarkersAfterReset,
       AsyncCandidateServiceRecordRetainedAfterStep,
       AsyncCandidateServiceEligibleAfterStep,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncControlServiceSlotTransition,
       CandidateScheduled,
       CandidateScheduledAfter,
       CandidateAdmissionCoalesced,
       FreshCandidateSequence,
       EnqueueCandidate,
       AsyncCandidateServiceOwnerPartitionInvariantIn,
       NodeHasDecision

THEOREM AsyncCandidateServicedMarkerPersistsWithoutExit ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateTransientServiceActive(candidate)
    /\ AsyncControlServiceSlotTransition
    /\ ~AsyncCandidateServiceExitThisStep(candidate)
    => AsyncCandidateServiceTombstoned(candidate)'
BY AsyncCandidateTransientMarkerPersistsWithinGeneration, Isa
   DEF AsyncCandidateServiceExitThisStep,
       AsyncCandidateServiceTombstoned,
       AsyncCandidateServiceCoalesced

THEOREM AsyncCandidateTerminalTombstonePersistsWithoutExit ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateTerminalTombstoneActive(candidate)
    /\ AsyncControlServiceSlotTransition
    /\ ~AsyncCandidateTerminalTombstoneExitThisStep(candidate)
    => AsyncCandidateTerminalTombstoneActive(candidate)'
BY IsaT(300)
   DEF AsyncCandidateTerminalTombstoneActive,
       AsyncCandidateTerminalTombstoneExitThisStep,
       AsyncCandidateTerminalTombstoned,
       AsyncCandidateTerminalRecordsFor,
       AsyncCandidateTerminalRecordsForIdentity,
       AsyncCandidateTerminalTombstones,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceRecordRetainedAfterStep,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncControlServiceSlotTransition,
       CandidateScheduled, CandidateScheduledAfter,
       NodeHasDecision

\* Compatibility theorem name retained for importing proof shards.  Its
\* corrected statement says that responsive replay clears, rather than
\* preserves, a pre-restart transient marker.
THEOREM AsyncCandidateSameHeightRestartPreservesServicedIdentity ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateTransientServiceActive(candidate)
    /\ candidate.node = asyncRecoveryNode
    /\ PreGstResponsiveReplay
    /\ AsyncControlServiceSlotTransition
    => ~AsyncCandidateTransientServiceMarked(candidate)'
BY IsaT(300)
   DEF AsyncCandidateTransientServiceActive,
       AsyncCandidateTransientServiceMarked,
       AsyncCandidateTransientServiceRecordsFor,
       AsyncCandidateTransientServiceRecordsForIdentity,
       AsyncCandidateServiceMarkers,
       AsyncNodeHasDecisionAfter,
       PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart,
       AsyncCandidateServiceMarkersAfterReset,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceRecordRetainedAfterStep,
       FreshRestartCandidateSequence,
       AsyncCandidateRestartReplayTombstoned,
       FreshCandidateSequence,
       CandidateAdmissionCoalesced

\* Same-height restart deletes every transient record for the recovering node,
\* retains terminal tombstones until their stage is independently obsolete,
\* and never rewinds the shared ordinal high-watermark.
THEOREM AsyncCandidateSameHeightRestartPreservesTombstone ==
  /\ AsyncCandidateServiceLifecycleInvariant
  /\ PreGstResponsiveReplay
  /\ AsyncControlServiceSlotTransition
  => /\ AsyncCandidateServiceMarkers'
          = {record \in AsyncCandidateServiceMarkers:
               /\ record.node # asyncRecoveryNode
               /\ AsyncCandidateServiceRecordRetainedAfterStep(record)}
     /\ AsyncCandidateTerminalTombstones'
          = {record \in AsyncCandidateTerminalTombstones:
               AsyncCandidateServiceRecordRetainedAfterStep(record)}
     /\ asyncControlServiceState'.candidateServiceNextOrdinal
          = asyncControlServiceState.candidateServiceNextOrdinal
BY IsaT(300)
   DEF AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateServiceMarkers,
       AsyncCandidateTerminalTombstones,
       AsyncControlServiceSlotTransition,
       AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceRecordRetainedAfterStep,
       AsyncCandidateServiceEligibleAfterStep,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateServiceMarkersAfterReset,
       AsyncNodeHasDecisionAfter,
       PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart

THEOREM AsyncCandidateTransientMarkerDoesNotSuppressRestartReplay ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateTransientServiceMarked(candidate)
    /\ ~AsyncCandidateTerminalTombstoned(candidate)
    => ~AsyncCandidateRestartReplayTombstoned(candidate)
BY DEF AsyncCandidateRestartReplayTombstoned

THEOREM AsyncRestartScopedCandidateIsNeverReplayTombstoned ==
  \A candidate \in AsyncCandidateSet:
    candidate.kind \in AsyncRestartScopedCandidateServiceKinds
      => ~AsyncCandidateRestartReplayTombstoned(candidate)
BY DEF AsyncCandidateRestartReplayTombstoned

THEOREM AsyncCandidateResponsiveRestartPermitsNonterminalReconstruction ==
  \A item \in AsyncNetworkItems:
    LET candidate == DeliveryCandidate(item)
    IN /\ candidate.node = asyncRecoveryNode
       /\ AsyncCandidateTransientServiceActive(candidate)
       /\ ~AsyncCandidateTerminalTombstoned(candidate)
       /\ PreGstResponsiveReplay
       /\ AsyncControlServiceSlotTransition
       => /\ ~AsyncCandidateTransientServiceMarked(candidate)'
          /\ ~AsyncCandidateServicePacketRetired(item)'
BY AsyncCandidateSameHeightRestartPreservesServicedIdentity,
   AsyncCandidateTransientMarkerDoesNotSuppressRestartReplay,
   IsaT(300)
   DEF AsyncCandidateServicePacketRetired,
       AsyncCandidateServiceCoalesced,
       AsyncCandidateTerminalTombstoned,
       AsyncCandidateTerminalRecordsFor,
       AsyncCandidateTerminalRecordsForIdentity,
       AsyncCandidateTerminalTombstones,
       AsyncControlServiceSlotTransition,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateServiceStateAfterSuccessfulService,
       PreGstResponsiveReplay

THEOREM AsyncCandidateStrictViewAdvanceReclaimsOlderTombstones ==
  \A record \in AsyncCandidateServiceTombstones:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ nodeView'[record.node] > record.episodeView
    /\ AsyncControlServiceSlotTransition
    => record \notin AsyncCandidateServiceTombstones'
BY Isa
   DEF AsyncCandidateServiceTombstones,
       AsyncControlServiceSlotTransition,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceTombstonesAfterReset,
       AsyncCandidateServiceRecordRetainedAfterStep,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateServiceTombstone

THEOREM AsyncCandidateDecisionReclaimsNodeTombstones ==
  \A record \in AsyncCandidateServiceTombstones:
    /\ AsyncNodeHasDecisionAfter(record.node)
    /\ AsyncControlServiceSlotTransition
    => record \notin AsyncCandidateServiceTombstones'
BY Isa
   DEF AsyncCandidateServiceTombstones,
       AsyncControlServiceSlotTransition,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceTombstonesAfterReset,
       AsyncCandidateServiceRecordRetainedAfterStep,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateServiceTombstone

THEOREM AsyncCandidateTombstoneSubsetIsBoundedByFrozenOwnerCarrier ==
  \A records, carrier:
    /\ IsFiniteSet(carrier)
    /\ records \subseteq AsyncCandidateServiceTombstones
    /\ {record.identity: record \in records}
         \subseteq carrier
    /\ AsyncControlServiceStateTypeInvariant
    => Cardinality(records)
         <= Cardinality(carrier)
BY FS_Subset, Isa
   DEF AsyncControlServiceStateTypeInvariant,
       AsyncCandidateServiceTombstones

THEOREM AsyncCandidateTombstonesAreBoundedByFrozenOwnerCarrier ==
  \A carrier:
    /\ IsFiniteSet(carrier)
    /\ {record.identity: record \in AsyncCandidateServiceTombstones}
         \subseteq carrier
    /\ AsyncControlServiceStateTypeInvariant
    => Cardinality(AsyncCandidateServiceTombstones)
         <= Cardinality(carrier)
BY AsyncCandidateTombstoneSubsetIsBoundedByFrozenOwnerCarrier, Isa

THEOREM AsyncControlServiceExactRetryCoalesces ==
  \A item:
    /\ AsyncControlServiceCurrentHeightItem(item)
    /\ AsyncControlServiceSlotOwned(item)
    /\ AsyncControlServiceIdentityMatches(
         item, AsyncControlServiceRecordForItem(item))
    /\ AsyncControlItemView(item)
         = AsyncControlServiceRecordForItem(item).view
    => AsyncControlServiceAdmissionCoalesced(item)
BY DEF AsyncControlServiceAdmissionCoalesced,
       AsyncControlServiceStrictlyNewerItem

THEOREM AsyncControlServiceSameOrLowerViewCannotReplace ==
  \A item:
    /\ AsyncControlServiceCurrentHeightItem(item)
    /\ AsyncControlServiceSlotOwned(item)
    /\ AsyncControlServiceRecordForItem(item).context = context
    /\ AsyncControlServiceRecordForItem(item).height = height
    /\ AsyncControlItemView(item)
         <= AsyncControlServiceRecordForItem(item).view
    => /\ AsyncControlServiceAdmissionCoalesced(item)
       /\ ~AsyncControlServiceAdmissionStartsOrReplaces(item)
BY Isa
   DEF AsyncControlServiceAdmissionCoalesced,
       AsyncControlServiceAdmissionStartsOrReplaces,
       AsyncControlServiceStrictlyNewerItem

THEOREM AsyncControlServiceReplacementIsStrictlyNewer ==
  \A item:
    /\ AsyncControlServiceSlotOwned(item)
    /\ AsyncControlServiceAdmissionStartsOrReplaces(item)
    => AsyncControlServiceStrictlyNewerItem(item)
BY DEF AsyncControlServiceAdmissionStartsOrReplaces

THEOREM AsyncControlServiceConsumedBitIsMonotoneWithoutReplacement ==
  \A item:
    /\ AsyncControlServiceConsumed(item)
    /\ AsyncControlServiceSlotTransition
    /\ AsyncControlServiceResetNodesThisStep = {}
    /\ \A admitted \in AsyncControlServiceAdmissionsThisStep:
         AsyncControlServiceSlot(
           admitted.envelope.recipient, admitted.source, admitted.kind)
           # AsyncControlServiceSlot(
               item.envelope.recipient, item.source, item.kind)
    => AsyncControlServiceConsumed(item)'
BY Isa
   DEF AsyncControlServiceConsumed,
       AsyncControlServiceSlotOwned,
       AsyncControlServiceRecordsForItem,
       AsyncControlServiceRecordsForSlot,
       AsyncControlServiceSlots,
       AsyncControlServiceIdentityMatches,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService

THEOREM AsyncControlServiceConsumedOccurrenceIsRetired ==
  \A item:
    AsyncControlServiceConsumed(item)
      => /\ AsyncControlServiceIdentityServicedOrAdvanced(item)
         /\ AsyncControlServiceOccurrenceTombstoned(item)
         /\ AsyncControlServiceOccurrenceRetired(item)
         /\ ~AsyncControlServiceOccurrenceIsCurrentOwner(item)
BY DEF AsyncControlServiceConsumed,
       AsyncControlServiceIdentityServicedOrAdvanced,
       AsyncControlServiceOccurrenceTombstoned,
       AsyncControlServiceOccurrenceRetired,
       AsyncControlServiceOccurrenceIsCurrentOwner

\* A consumed exact identity may lose the physical record only when the same
\* bounded slot advances to a strictly newer identity.  Either post-state is
\* terminal for the old identity: it remains consumed or mismatches the new
\* record.  Same/lower retries cannot replace the slot and therefore cannot
\* recreate the old live owner.
THEOREM AsyncControlServiceConsumedIdentityCannotReactivate ==
  \A item:
    /\ AsyncControlServiceCurrentHeightItem(item)
    /\ AsyncControlServiceConsumed(item)
    /\ AsyncControlServiceSlotTransition
    => /\ AsyncControlServiceIdentityServicedOrAdvanced(item)'
       /\ AsyncControlServiceOccurrenceTombstoned(item)'
       /\ AsyncControlServiceOccurrenceRetired(item)'
       /\ ~AsyncControlServiceOccurrenceIsCurrentOwner(item)'
BY IsaT(300)
   DEF AsyncControlServiceConsumed,
       AsyncControlServiceIdentityServicedOrAdvanced,
       AsyncControlServiceOccurrenceTombstoned,
       AsyncControlServiceOccurrenceRetired,
       AsyncControlServiceOccurrenceIsCurrentOwner,
       AsyncControlServiceSlotOwned,
       AsyncControlServiceRecordsForItem,
       AsyncControlServiceRecordsForSlot,
       AsyncControlServiceSlots,
       AsyncControlServiceIdentityMatches,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncControlServiceAdmissionStartsOrReplaces,
       AsyncControlServiceAdmissionCoalesced,
       AsyncControlServiceStrictlyNewerItem

THEOREM AsyncControlServiceServicedIdentityCannotResurrect ==
  \A item:
    /\ AsyncControlServiceCurrentHeightItem(item)
    /\ AsyncControlServiceIdentityServicedOrAdvanced(item)
    /\ AsyncControlServiceSlotTransition
    => /\ AsyncControlServiceIdentityServicedOrAdvanced(item)'
       /\ AsyncControlServiceOccurrenceTombstoned(item)'
       /\ ~AsyncControlServiceOccurrenceIsCurrentOwner(item)'
BY IsaT(300)
   DEF AsyncControlServiceIdentityServicedOrAdvanced,
       AsyncControlServiceOccurrenceTombstoned,
       AsyncControlServiceOccurrenceIsCurrentOwner,
       AsyncControlServiceSlotOwned,
       AsyncControlServiceRecordsForItem,
       AsyncControlServiceRecordsForSlot,
       AsyncControlServiceSlots,
       AsyncControlServiceIdentityMatches,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncControlServiceAdmissionStartsOrReplaces,
       AsyncControlServiceAdmissionCoalesced,
       AsyncControlServiceStrictlyNewerItem

THEOREM AsyncControlServiceTombstoneCannotReactivate ==
  \A item:
    /\ AsyncControlServiceCurrentHeightItem(item)
    /\ AsyncControlServiceOccurrenceTombstoned(item)
    /\ AsyncControlServiceSlotTransition
    => /\ AsyncControlServiceOccurrenceTombstoned(item)'
       /\ ~AsyncControlServiceOccurrenceIsCurrentOwner(item)'
BY IsaT(300)
   DEF AsyncControlServiceOccurrenceTombstoned,
       AsyncControlServiceOccurrenceIsCurrentOwner,
       AsyncControlServiceSlotOwned,
       AsyncControlServiceRecordsForItem,
       AsyncControlServiceRecordsForSlot,
       AsyncControlServiceSlots,
       AsyncControlServiceIdentityMatches,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncControlServiceAdmissionStartsOrReplaces,
       AsyncControlServiceAdmissionCoalesced,
       AsyncControlServiceStrictlyNewerItem

AsyncControlServiceOccupiedSlotSet ==
  {record.slot: record \in AsyncControlServiceSlots}

THEOREM AsyncControlServiceSameHeightRecoveryRetiresVolatileOwners ==
  /\ PreGstResponsiveReplay
  /\ AsyncControlServiceSlotTransition
  => /\ AsyncControlServiceOccupiedSlotSet'
          = AsyncControlServiceOccupiedSlotSet
     /\ asyncControlServiceState'.nextOrdinal
          = asyncControlServiceState.nextOrdinal
     /\ asyncControlServiceState'.certifiedResponseNextOrdinal
          = asyncControlServiceState.certifiedResponseNextOrdinal
     /\ CertifiedResponseClaimRecordsAt(asyncRecoveryNode)' = {}
     /\ \A record \in AsyncControlServiceSlots':
          record.slot.recipient = asyncRecoveryNode
            => record.consumed
BY Isa
   DEF AsyncControlServiceSlotTransition,
       AsyncControlServiceResetNodesThisStep,
       AsyncControlServiceAdmissionsThisStep,
       AsyncControlServicesThisStep,
       AsyncCertifiedResponseClaimAdmissionsThisStep,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncControlServiceSlots,
       AsyncControlServiceOccupiedSlotSet,
       CertifiedResponseClaimRecordsAt,
       CertifiedResponseClaimRecordsFor,
       AsyncCertifiedResponseClaimRecords,
       PreGstResponsiveReplay, AsyncNonCrashStep

THEOREM CertifiedResponseClaimAdmissionAllocatesExactOrdinal ==
  \A item:
    /\ AsyncCertifiedResponseClaimAdmissionsThisStep = {item}
    /\ AsyncControlServiceSlotTransition
    => LET recipient == item.envelope.recipient
           ordinal == AsyncNextCertifiedResponseClaimOrdinal(recipient)
       IN /\ AsyncCertifiedResponseClaimRecord(item, ordinal)
                \in AsyncCertifiedResponseClaimRecords'
          /\ AsyncNextCertifiedResponseClaimOrdinal(recipient)'
               = ordinal + 1
BY Isa
   DEF AsyncControlServiceSlotTransition,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCertifiedResponseClaimRecords,
       AsyncNextCertifiedResponseClaimOrdinal

THEOREM CertifiedResponseExactRetryKeepsOneClaimOrdinal ==
  \A recipient, source, item:
    /\ CertifiedResponseClaimMatches(item)
    /\ CertifiedResponseClaimMetadataMatches(item)
    /\ DueSourcePackets(recipient, source) # {}
    /\ item = OldestDueSourcePacket(recipient, source).item
    /\ CoalesceHiddenPacket(recipient, source)
    /\ AsyncControlServiceSlotTransition
    => /\ CertifiedResponseClaimMatches(item)'
       /\ CertifiedResponseClaimMetadataMatches(item)'
       /\ CertifiedResponseClaimAdmissionOrdinal(item)'
            = CertifiedResponseClaimAdmissionOrdinal(item)
       /\ AsyncNextCertifiedResponseClaimOrdinal(recipient)'
            = AsyncNextCertifiedResponseClaimOrdinal(recipient)
BY IsaT(300)
   DEF CertifiedResponseClaimMatches,
       CertifiedResponseClaimMetadataMatches,
       CertifiedResponseClaimAdmissionOrdinal,
       CertifiedResponseClaimRecordForItem,
       CertifiedResponseClaimRecordsForIdentity,
       AsyncCertifiedResponseClaimRecords,
       AsyncCertifiedResponseOccurrenceIdentity,
       CoalesceHiddenPacket,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimAdmissionsThisStep,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       CertifiedResponseClaimRecordsFor,
       AsyncNextCertifiedResponseClaimOrdinal

THEOREM CertifiedResponseCompetingResponderCannotDoubleChargeFamily ==
  \A claimed, competitor:
    /\ AsyncCertifiedResponseClaimInvariant
    /\ claimed \in AsyncCertifiedResponseClaimRecords
    /\ competitor.kind = "CertifiedResponse"
    /\ AsyncCertifiedResponseWaiterFamily(competitor) = claimed.family
    /\ AsyncCertifiedResponseOccurrenceIdentity(competitor)
         # claimed.identity
    => /\ ~CertifiedResponseAuthorityReady(claimed.family)
       /\ ~CertifiedResponseFreshClaimGateAllows(competitor)
       /\ Cardinality(
            CertifiedResponseClaimRecordsForFamily(
              claimed.family)) = 1
BY Isa
   DEF AsyncCertifiedResponseClaimInvariant,
       CertifiedResponseAuthorityReady,
       CertifiedResponseAuthorityClaimed,
       CertifiedResponseFreshClaimGateAllows,
       CertifiedResponseRecipientClaimAvailable,
       CertifiedResponseClaimsForFamilyAt,
       CertifiedResponseClaimRecordsForFamilyAt,
       CertifiedResponseClaimRecordsForFamily,
       AsyncCertifiedResponseWaiterFamily

THEOREM CertifiedResponseConsumedFamilyCannotRetainClaim ==
  \A requestHash:
    /\ requestHash \notin ActiveCertifiedRequestHashes'
    /\ AsyncControlServiceSlotTransition
    => CertifiedResponseClaimRecordsForFamily(requestHash)' = {}
BY Isa
   DEF CertifiedResponseClaimRecordsForFamily,
       AsyncCertifiedResponseClaimRecords,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimAdmissionsThisStep,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       CertifiedResponseClaimRecordsFor

THEOREM CertifiedResponseSameHeightRecoveryReopensDurableFamily ==
  \A request:
    /\ PreGstResponsiveReplay
    /\ request \in asyncActiveRequests
    /\ RestartDurableCertifiedRequest(asyncRecoveryNode, request)
    /\ AsyncControlServiceSlotTransition
    => /\ request \in asyncActiveRequests'
       /\ CertifiedResponseClaimsAt(asyncRecoveryNode)' = {}
       /\ CertifiedResponseClaimRecordsAt(asyncRecoveryNode)' = {}
       /\ AsyncNextCertifiedResponseClaimOrdinal(asyncRecoveryNode)'
            = AsyncNextCertifiedResponseClaimOrdinal(asyncRecoveryNode)
BY SameHeightRestartReopensDurableCertifiedResponseFamily,
   AsyncControlServiceSameHeightRecoveryRetiresVolatileOwners, Isa
   DEF PreGstResponsiveReplay

AsyncNext ==
  /\ (AsyncNonCrashStep
        \/ (\E node \in ValidatorIds:
              AsyncEnterIndexedServiceActivation(node))
        \/ (\E node \in ValidatorIds:
              AsyncActivateServiceNode(node))
        \/ (\E node \in ValidatorIds: PreGstCrash(node))
        \/ (\E node \in ValidatorIds: PreGstResponsiveCrash(node))
        \/ PreGstResponsiveRestart
        \/ PreGstResponsiveReplay)
  /\ AsyncHistoricalLockRestartAuthorityTransition
  /\ AsyncControlServiceSlotTransition
  /\ AsyncServiceActivationTransition
  /\ UNCHANGED <<height, context>>
  /\ [Next]_vars

THEOREM AsyncServiceActivationActionsRefineAsyncNext ==
  \A node \in ValidatorIds:
    (AsyncEnterIndexedServiceActivation(node)
      \/ AsyncActivateServiceNode(node))
      => AsyncNext
BY Isa
   DEF AsyncNext, AsyncServiceActivationTransition,
       AsyncEnterIndexedServiceActivation, AsyncActivateServiceNode,
       AsyncServiceActivationFrameVars,
       AsyncSchedulerExceptServiceActivation, vars

(***************************************************************************
Atomic lifecycle-order safety.  The first action which makes a raw timeout
eligible freezes its per-node position in the same global transition.  No
separate arming action or fairness assumption is involved.  The position is
retained until a certified rearm/Decision endpoint or transfer into the exact
BeginTimeout causal origin.  The final capacity conjunct in the same
transition is fail-closed: an action which would create an unrecorded root
cannot pop its ingress/causal owner when no reviewed slot can be compacted.
***************************************************************************)
THEOREM AsyncTimeoutLifecycleDueTransitionMintsBeforeLaterAdmissions ==
  \A node \in ValidatorIds:
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncNext
    /\ AsyncTick
    /\ ~AsyncTimeoutLifecycleOwned(node)
    /\ ~AsyncTimeoutClockDue(node)
    /\ AsyncTimeoutClockDueAfter(node)
    /\ ~AsyncCandidateLifecycleRecorded(
         node, AsyncCurrentTimeoutCausalOrigin(node))
    => /\ AsyncTimeoutLifecycleOwned(node)'
       /\ AsyncTimeoutLifecycleOrdinal(node)'
            = AsyncNextCandidateLifecycleOrdinal(node)
       /\ AsyncTimeoutLifecycleOrigin(node)'
            = AsyncProposedTimeoutCausalOrigin(node)
       /\ AsyncNextCandidateLifecycleOrdinal(node)'
            > AsyncTimeoutLifecycleOrdinal(node)'
BY IsaT(600)
   DEF AsyncNext, AsyncControlServiceSlotTransition,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncTimeoutLifecycleConsumesFreshOrdinal,
       AsyncTimeoutLifecycleOrdinalForStep,
       AsyncTimeoutLifecycleCanAcquireThisStep,
       AsyncTimeoutLifecycleOwned,
       AsyncTimeoutLifecycleOrdinal,
       AsyncTimeoutLifecycleOrigin,
       AsyncNextCandidateLifecycleOrdinal,
       AsyncCandidateLifecycleRecorded,
       AsyncCandidateLifecycleRecordsFor

THEOREM AsyncTimeoutLifecycleOrdinalPersistsUntilEndpoint ==
  \A node \in ValidatorIds:
    /\ AsyncNext
    /\ AsyncTimeoutLifecycleOwned(node)
    /\ ~AsyncTimeoutLifecycleResetThisStep(node)
    /\ ~AsyncTimeoutLifecycleTransfersThisStep(node)
    => /\ AsyncTimeoutLifecycleOrdinal(node)'
            = AsyncTimeoutLifecycleOrdinal(node)
       /\ AsyncTimeoutLifecycleOrigin(node)'
            = AsyncTimeoutLifecycleOrigin(node)
BY IsaT(300)
   DEF AsyncNext, AsyncControlServiceSlotTransition,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncTimeoutLifecycleOrdinal,
       AsyncTimeoutLifecycleOrigin,
       AsyncTimeoutLifecycleOwned

THEOREM AsyncTimeoutLifecycleOrdinalClearsOnlyAtEndpoint ==
  \A node \in ValidatorIds:
    /\ AsyncNext
    /\ AsyncTimeoutLifecycleOwned(node)
    /\ AsyncTimeoutLifecycleOrdinal(node)' = 0
    => /\ AsyncTimeoutLifecycleOrigin(node)'
             = NoAsyncCandidateLifecycleOrigin
       /\ \/ AsyncTimeoutLifecycleResetThisStep(node)
          \/ AsyncTimeoutLifecycleTransfersThisStep(node)
BY IsaT(300)
   DEF AsyncNext, AsyncControlServiceSlotTransition,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncTimeoutLifecycleOrdinal,
       AsyncTimeoutLifecycleOrigin,
       AsyncTimeoutLifecycleOwned

THEOREM AsyncNextNeverSchedulesAnUnownedCandidateLifecycle ==
  AsyncNext
    => \A node \in ValidatorIds:
         AsyncScheduledCandidateOriginsForNodeAfter(node)
           \subseteq
             {record.origin:
                record \in
                  asyncControlServiceState'.candidateLifecycleAdmissions,
                /\ record.node = node
                /\ ~record.retired}
BY IsaT(600)
   DEF AsyncNext, AsyncControlServiceSlotTransition,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncCandidateLifecycleNewAdmissions,
       AsyncNewCandidateLifecycleOriginsForNodeIn,
       AsyncCandidateLifecycleOriginsRecordedForNodeIn,
       AsyncCandidateLifecycleCarrierUpdatedAdmissions

THEOREM AsyncServeQueuedIdentityDepartureInstallsTombstone ==
  \A node \in ValidatorIds,
     identity \in AsyncServeLogicalRequestIdentities:
    /\ AsyncServeLifecycleTypeInvariant
    /\ gst
    /\ AsyncServeJobQueued(node, identity)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~AsyncServeJobQueued(node, identity)'
    => AsyncServeLifecycleTombstone(node, identity)'
BY IsaT(600)
   DEF AsyncServeJobQueued, AsyncIoServeIdentities,
       AsyncIoServeJobIdentity,
       AsyncServeLifecycleTombstone,
       AsyncServeTombstoneRecords,
       AsyncServeLifecycleTypeInvariant,
       AsyncServeLifecyclePartitionInvariant,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RuntimeStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       AsyncServeReservationsAfterIoService,
       AsyncServeAdmissionsWithout,
       AsyncServeTombstonesWithoutFamily,
       AsyncServeReservationRecord, AsyncServeTombstone,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart,
       AsyncAllVars

THEOREM AsyncServeRetiredIdentityCannotRequeueAtGst ==
  \A node \in ValidatorIds,
     identity \in AsyncServeLogicalRequestIdentities:
    /\ AsyncServeLifecycleTypeInvariant
    /\ AsyncServeLogicalIdentityRetiredOrSuperseded(node, identity)
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => /\ AsyncServeLogicalIdentityRetiredOrSuperseded(
            node, identity)'
       /\ ~AsyncServeJobQueued(node, identity)'
BY IsaT(900)
   DEF AsyncServeLogicalIdentityRetiredOrSuperseded,
       AsyncServeLogicalIdentityRequests,
       AsyncServeLifecycleTombstone,
       AsyncServeTombstoneRecords,
       AsyncServeLifecycleFamilyOwned,
       AsyncServeFamilyOwnerIdentity,
       AsyncServeFamilyHighWatermark,
       AsyncServeLifecycleTypeInvariant,
       AsyncServeLifecyclePartitionInvariant,
       AsyncServeFamilyHighWatermarkInvariant,
       AsyncServeJobQueued, AsyncIoServeIdentities,
       AsyncIoServeJobIdentity,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RuntimeStep,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       AcceptOrCoalesceExactServeRequest,
       ReserveExactServeCapacity, AdvanceExactServeCapacity,
       CoalesceExactServeIngressCapacity,
       ResumeExactServeCapacity, CoalesceExactServeCapacity,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       AsyncServeReservationsAfterIoService,
       AsyncServeAdmissionsWithout,
       AsyncServeTombstonesWithoutFamily,
       AsyncServeReservationRecord, AsyncServeTombstone,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart,
       AsyncAllVars

THEOREM AsyncServeTombstonedIdentityCannotRequeueAtGst ==
  \A node \in ValidatorIds,
     identity \in AsyncServeLogicalRequestIdentities:
    /\ AsyncServeLifecycleTypeInvariant
    /\ AsyncServeLifecycleTombstone(node, identity)
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => /\ AsyncServeLogicalIdentityRetiredOrSuperseded(
            node, identity)'
       /\ ~AsyncServeJobQueued(node, identity)'
BY AsyncServeRetiredIdentityCannotRequeueAtGst, Isa
   DEF AsyncServeLogicalIdentityRetiredOrSuperseded

THEOREM AsyncCandidateServicesThisStepIsSingleton ==
  /\ AsyncLogicalCandidateOwnershipInvariant
  /\ AsyncNext
  => Cardinality(AsyncCandidateServicesThisStep) <= 1
BY FS_Singleton, FS_Subset, IsaT(600)
   DEF AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       CandidateScheduled, CandidateScheduledAfter

THEOREM AsyncCandidateTerminalRetirementsThisStepIsSingleton ==
  /\ AsyncLogicalCandidateOwnershipInvariant
  /\ AsyncNext
  => Cardinality(AsyncCandidateTerminalRetirementsThisStep) <= 1
BY FS_Singleton, FS_Subset, IsaT(600)
   DEF AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn

THEOREM AsyncCandidateDiscardInstallsTerminalTombstone ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ AsyncCandidateTerminallyDiscardedThisStep(candidate)
    /\ AsyncCandidateTerminalRetirementEligibleAfterStep(candidate)
    /\ AsyncNext
    => AsyncCandidateTerminalTombstoned(candidate)'
BY AsyncCandidateTerminalRetirementsThisStepIsSingleton,
   IsaT(600)
   DEF AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidateTerminalTombstoned,
       AsyncCandidateTerminalIdentityTombstoned,
       AsyncCandidateTerminalRecordsFor,
       AsyncCandidateTerminalRecordsForIdentity,
       AsyncCandidateTerminalTombstones,
       AsyncCandidateServiceIdentityScheduled,
       AsyncScheduledCandidateServiceIdentities,
       AsyncControlServiceSlotTransition,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateServiceTombstone,
       CandidateScheduled, CandidateScheduledAfter

THEOREM AsyncCandidateTerminalDiscardAllocatesExactOrdinal ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ AsyncCandidateTerminallyDiscardedThisStep(candidate)
    /\ AsyncCandidateTerminalRetirementEligibleAfterStep(candidate)
    /\ ~AsyncCandidateServiceCoalesced(candidate)
    /\ AsyncNext
    => LET node == candidate.node
           ordinal == AsyncNextCandidateServiceOrdinal(node)
       IN /\ AsyncCandidateServiceTombstone(
                candidate, nodeView[node], ordinal)
              \in AsyncCandidateTerminalTombstones'
          /\ AsyncNextCandidateServiceOrdinal(node)' = ordinal + 1
BY AsyncCandidateTerminalRetirementsThisStepIsSingleton,
   IsaT(600)
   DEF AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidateServiceCoalesced,
       AsyncCandidateTerminalTombstoned,
       AsyncCandidateTerminalRecordsFor,
       AsyncCandidateTerminalRecordsForIdentity,
       AsyncCandidateTerminalTombstones,
       AsyncNextCandidateServiceOrdinal,
       AsyncControlServiceSlotTransition,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateServiceTombstone,
       AsyncNext, AsyncAllVars

THEOREM AsyncCandidateDiscardRetiresLogicalLifecycle ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ AsyncCandidateTerminallyDiscardedThisStep(candidate)
    /\ AsyncCandidateTerminalRetirementEligibleAfterStep(candidate)
    /\ AsyncNext
    => AsyncCandidateAdmissionIdentityTerminallyCovered(
         AsyncCandidateAdmissionIdentity(candidate))'
BY AsyncCandidateDiscardInstallsTerminalTombstone, IsaT(600)
   DEF AsyncCandidateAdmissionIdentityTerminallyCovered,
       AsyncCandidateAdmissionIdentityObsolete,
       AsyncCandidateAdmissionIdentity,
       AsyncCandidateTerminalIdentityTombstoned,
       AsyncScheduledCandidateAdmissionIdentities,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidatePhysicallyDiscardedThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateServiceExitThisStep,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn, AsyncAllVars

(***************************************************************************
The BodyAvailable reducer stage is monotone after GST: an available body may
move atomically to durable storage, but the post-GST action relation cannot
crash the owner and erase its process-local Available state.  Once the exact
internal service identity has no scheduler carrier, every ordinary producer
must pass FreshCandidateSequence and therefore cannot recreate that retired
identity.  The theorem is deliberately service-identity based rather than
candidate-value based: class/route details may change, while the immutable
context/view/subject/kind/causal-origin owner may not.
***************************************************************************)
THEOREM AsyncCandidateInternalBodyAvailableStageRetirementIsMonotoneAtGst ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateInternalBodyAvailableStageRetired(candidate)
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => AsyncCandidateInternalBodyAvailableStageRetired(candidate)'
BY IsaT(900)
   DEF AsyncCandidateInternalBodyAvailableStageRetired,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       FetchBody, RebindRetainedBody, StoreBody,
       FetchCertifiedBody, AcceptCertifiedResponseCapability,
       InstallCertifiedBodyEffect,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       AsyncAllVars, vars

THEOREM AsyncCandidateInternalBodyAvailableServiceIdentityCannotReactivateAtGst ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateInternalBodyAvailableStageRetired(candidate)
    /\ AsyncCandidateServiceIdentity(candidate)
         \notin AsyncScheduledCandidateServiceIdentities
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => /\ AsyncCandidateInternalBodyAvailableStageRetired(candidate)'
       /\ AsyncCandidateServiceIdentity(candidate)
            \notin AsyncScheduledCandidateServiceIdentities'
BY AsyncCandidateInternalBodyAvailableStageRetirementIsMonotoneAtGst,
   AsyncCandidateInternalBodyAvailableStageRetirementCoalescesFreshCandidate,
   IsaT(900)
   DEF AsyncCandidateInternalBodyAvailableStageRetired,
       AsyncCandidateServiceIdentity,
       AsyncScheduledCandidateServiceIdentities,
       CandidateAdmissionCoalesced,
       FreshCandidateSequence, EnqueueCandidate,
       AsyncCandidateStageRetired,
       IngressPacketPolicyRejected, CanAdmitIngressItem,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RuntimeStep,
       DrainFairIngressSelected, AdmitCausalHead,
       AdmitProducerCompletion,
       FifoRuntimeStep, DeferredDrainStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       HistoricalLockedRetransmitSuccessors,
       AppendHistoricalLockedRetransmitSuccessors,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       FreshRestartCandidateSequence,
       CandidateScheduledIn, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       AsyncAllVars

THEOREM AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst ==
  \A identity \in AsyncCandidateAdmissionIdentitySet:
    /\ AsyncCandidateAdmissionIdentityObsolete(identity)
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => AsyncCandidateAdmissionIdentityObsolete(identity)'
BY IsaT(300)
   DEF AsyncCandidateAdmissionIdentityObsolete,
       AsyncCandidateAdmissionIdentitySet,
       AsyncCandidateAdmissionIdentity,
       AsyncConsumerEventTag,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       NodeHasDecision, AsyncAllVars

THEOREM AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst ==
  \A identity \in AsyncCandidateAdmissionIdentitySet:
    /\ AsyncCandidateAdmissionIdentityObsolete(identity)
    /\ identity \notin AsyncScheduledCandidateAdmissionIdentities
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => identity \notin AsyncScheduledCandidateAdmissionIdentities'
BY AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst,
   IsaT(600)
   DEF AsyncCandidateAdmissionIdentityObsolete,
       AsyncCandidateAdmissionIdentity,
       AsyncScheduledCandidateAdmissionIdentities,
       CandidateAdmissionCoalesced,
       FreshCandidateSequence, EnqueueCandidate,
       AsyncCandidateStageRetired,
       IngressPacketPolicyRejected, CanAdmitIngressItem,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RuntimeStep,
       DrainFairIngressSelected, AdmitCausalHead,
       AdmitProducerCompletion,
       FifoRuntimeStep, DeferredDrainStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       CandidateScheduledIn, AsyncAllVars

THEOREM AsyncCandidateTerminalIdentityCannotReactivateAtGst ==
  \A identity \in AsyncCandidateAdmissionIdentitySet:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCandidateTerminalIdentityTombstoned(identity.service)
    /\ identity \notin AsyncScheduledCandidateAdmissionIdentities
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => /\ AsyncCandidateAdmissionIdentityTerminallyCovered(identity)'
       /\ identity \notin AsyncScheduledCandidateAdmissionIdentities'
BY AsyncCandidateTerminalTombstoneCoalescesFreshCandidate,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst,
   AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst,
   IsaT(600)
   DEF AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateAdmissionIdentityTerminallyCovered,
       AsyncCandidateAdmissionIdentityObsolete,
       AsyncCandidateAdmissionIdentitySet,
       AsyncCandidateAdmissionIdentity,
       AsyncCandidateTerminalIdentityTombstoned,
       AsyncCandidateTerminalRecordsForIdentity,
       AsyncCandidateTerminalTombstones,
       AsyncScheduledCandidateAdmissionIdentities,
       AsyncScheduledCandidateServiceIdentities,
       AsyncCandidateServiceIdentityScheduled,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceRecordRetainedAfterStep,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceEligibleAfterStep,
       AsyncCandidateServiceMarkersAfterReset,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceResetNodesThisStep,
       CandidateAdmissionCoalesced,
       FreshCandidateSequence, EnqueueCandidate,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RuntimeStep,
       DrainFairIngressSelected, AdmitCausalHead,
       AdmitProducerCompletion,
       FifoRuntimeStep, DeferredDrainStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       AsyncAllVars

THEOREM AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst ==
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
BY AsyncCandidateSuccessfulServiceInstallsTransientMarker,
   AsyncCandidateDiscardRetiresLogicalLifecycle,
   AsyncCandidateTerminalRetirementsThisStepIsSingleton,
   IsaT(900)
   DEF AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidatePhysicallyDiscardedThisStep,
       AsyncCandidateSemanticallyAppliedThisStep,
       AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter,
       AsyncCandidateMonotoneSemanticCoverageAfterIn,
       AsyncCandidateServiceTombstoned,
       AsyncCandidateTerminalTombstoned,
       AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidateServiceEligibleAfterStep,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncProgressOwnershipInvariant,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn, AsyncAllVars

THEOREM AsyncCandidateSameGenerationServicedIdentityCannotReactivateAtGst ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCandidateTransientServiceActive(candidate)
    /\ candidate.consumerGeneration = generation[candidate.node]
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    /\ ~AsyncCandidateTransientMarkerExitThisStep(candidate)
    => /\ AsyncCandidateTransientServiceActive(candidate)'
       /\ ~CandidateScheduled(candidate)'
BY AsyncCandidateTransientMarkerPersistsWithinGeneration,
   AsyncCandidateTransientMarkerCoalescesFreshCandidate,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   IsaT(600)
   DEF AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateTransientServiceActive,
       AsyncCandidateTransientMarkerExitThisStep,
       AsyncCandidateTransientServiceMarked,
       AsyncCandidateTransientServiceRecordsFor,
       AsyncCandidateTransientServiceRecordsForIdentity,
       AsyncCandidateServiceMarkers,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RuntimeStep,
       DrainFairIngressSelected, AdmitCausalHead,
       AdmitProducerCompletion, EnqueueCandidate,
       FifoRuntimeStep, DeferredDrainStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CandidateAdmissionCoalesced,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       IngressPacketPolicyRejected,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       FreshRestartCandidateSequence,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn, NodeHasDecision

THEOREM AsyncCandidateSameGenerationSuccessfulServiceIdentityPersistsUntilStrictExit ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCandidateTransientServiceActive(candidate)
    /\ candidate.consumerGeneration = generation[candidate.node]
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    /\ ~AsyncCandidateTransientMarkerExitThisStep(candidate)
    => /\ AsyncCandidateServiceTombstoned(candidate)'
       /\ ~CandidateScheduled(candidate)'
BY AsyncCandidateSameGenerationServicedIdentityCannotReactivateAtGst, Isa
   DEF AsyncCandidateTransientServiceActive,
       AsyncCandidateServiceTombstoned,
       AsyncCandidateServiceCoalesced

THEOREM AsyncCandidateServicedIdentityCannotReactivate ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateTransientServiceActive(candidate)
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    /\ ~AsyncCandidateServiceExitThisStep(candidate)
    => /\ AsyncCandidateTransientServiceActive(candidate)'
       /\ ~CandidateScheduled(candidate)'
BY AsyncCandidateSameGenerationServicedIdentityCannotReactivateAtGst, Isa
   DEF AsyncCandidateServiceExitThisStep

THEOREM AsyncActiveControlServiceAdmissionPassesSlotGuard ==
  \A node \in ValidatorIds:
    LET item == AsyncSelectedFairIngressItem(node)
        candidate == DeliveryCandidate(item)
    IN /\ AsyncActiveControlServiceAdmissionPassCondition(node)
       /\ AsyncNext
       /\ DrainFairIngressSelected(node)
       => CandidateScheduled(candidate)'
BY IsaT(300)
   DEF AsyncActiveControlServiceAdmissionPassCondition,
       AsyncSelectedFairIngressItem,
       AsyncControlServiceOccurrenceIsCurrentOwner,
       AsyncControlServiceSlotTransition,
       DrainFairIngressSelected, IngressItemCanDrain,
       DeliveryCandidate, CandidateScheduled,
       CandidateAdmissionCoalesced,
       AsyncNext, AsyncAllVars

THEOREM AsyncRetiredControlServiceAdmissionDropsWithoutCandidate ==
  \A node \in ValidatorIds:
    LET item == AsyncSelectedFairIngressItem(node)
        candidate == DeliveryCandidate(item)
    IN /\ AsyncRetiredControlServiceAdmissionDropCondition(node)
       /\ AsyncNext
       /\ DrainFairIngressSelected(node)
       => ~CandidateScheduled(candidate)'
BY IsaT(300)
   DEF AsyncRetiredControlServiceAdmissionDropCondition,
       AsyncSelectedFairIngressItem,
       AsyncControlServiceOccurrenceRetired,
       AsyncControlServiceSlotTransition,
       DrainFairIngressSelected, IngressItemCanDrain,
       DeliveryCandidate, CandidateScheduled,
       CandidateAdmissionCoalesced,
       AsyncNext, AsyncAllVars

PostGstRunNode(node) ==
  /\ gst
  /\ RunNode(node)
  /\ AsyncNonCrashOuterFrame

PostGstOpenHistoricalRecovery(node) ==
  /\ gst
  /\ OpenHistoricalRecovery(node)
  /\ AsyncNonRunnerOuterFrame

PostGstRunHistoricalRecoveryNode(node) ==
  /\ gst
  /\ RunHistoricalRecoveryNode(node)
  /\ AsyncNonCrashOuterFrame

PostGstRunHistoricalServer(node) ==
  /\ gst
  /\ RunHistoricalServer(node)
  /\ AsyncNonCrashOuterFrame

PostGstCommitCertificateDiscovery(node) ==
  /\ gst
  /\ DirectCommitCertificateDiscoveryStep(node)
  /\ AsyncNonRunnerOuterFrame

PostGstHistoricalCommitCertificateDiscovery(node) ==
  /\ gst
  /\ DirectHistoricalCommitCertificateDiscoveryStep(node)
  /\ AsyncNonRunnerOuterFrame

PostGstServiceIoWorker(node) ==
  /\ gst
  /\ ServiceIoWorker(node)
  /\ AsyncNonRunnerOuterFrame

PostGstServiceHistoricalRecoveryIoWorker(node) ==
  /\ gst
  /\ ServiceHistoricalRecoveryIoWorker(node)
  /\ AsyncNonRunnerOuterFrame

PostGstAdmitHiddenPacket(recipient, source) ==
  /\ gst
  /\ AdmitIngressPacket(recipient, source)
  /\ AsyncNonRunnerOuterFrame

HistoricalRecoveryPacketCorridor(recipient, source) ==
  \/ /\ HistoricalRecoveryTarget(recipient)
        /\ source \in AsyncIngressSources
  \/ /\ HistoricalRecoveryTarget(source)
        /\ recipient \in AsyncArchiveIoServiceNodes

PostGstAdmitHistoricalRecoveryPacket(recipient, source) ==
  /\ gst
  /\ HistoricalRecoveryPacketCorridor(recipient, source)
  /\ AdmitIngressPacket(recipient, source)
  /\ AsyncNonRunnerOuterFrame

(***************************************************************************
Exact action inventory shared by the weak-fairness clauses below.  Keeping the
union separate from `WF` makes the semantic audit executable without wrapping
each ENABLED query in the entire Core transition relation.  The structural
checker pins this inventory, every quantifier domain, every action's outer
frame category, and the typed refinement claim against deletion or
substitution.
***************************************************************************)
AsyncFairActionAt(initialContext) ==
  \/ AsyncSetGST
  \/ PreGstResponsiveRestart
  \/ PreGstResponsiveReplay
  \/ ResponsiveReplayRunNode
  \/ ResponsiveReplayServiceIoWorker
  \/ DriveResponsiveReplayHead
  \/ FinishResponsiveReplay
  \/ (\E node \in Responsive:
        AsyncActivateServiceNode(node))
  \/ AsyncTick
  \/ (\E node \in AsyncVotersAt(initialContext):
        PostGstRunNode(node))
  \/ (\E node \in Responsive:
        PostGstOpenHistoricalRecovery(node))
  \/ (\E node \in Responsive:
        PostGstRunHistoricalRecoveryNode(node))
  \/ (\E node \in Responsive:
        PostGstRunHistoricalServer(node))
  \/ (\E node \in AsyncVotersAt(initialContext):
        PostGstCommitCertificateDiscovery(node))
  \/ (\E node \in Responsive:
        PostGstHistoricalCommitCertificateDiscovery(node))
  \/ (\E node \in Responsive:
        PostGstServiceIoWorker(node))
  \/ (\E node \in Responsive:
        PostGstServiceHistoricalRecoveryIoWorker(node))
  \/ (\E recipient \in Responsive,
         source \in AsyncIngressSources:
        PostGstAdmitHiddenPacket(recipient, source))
  \/ (\E recipient \in ValidatorIds, source \in AsyncIngressSources:
        PostGstAdmitHistoricalRecoveryPacket(recipient, source))

AsyncFairnessAt(initialContext) ==
  /\ WF_AsyncAllVars(AsyncSetGST)
  /\ WF_AsyncAllVars(PreGstResponsiveRestart)
  /\ WF_AsyncAllVars(PreGstResponsiveReplay)
  \* The optional locked-body Fetch prefix and current signature execute through
  \* the ordinary serialized node runner and completion I/O worker before the
  \* next durable signature may be installed from the retained tail.  GST stays
  \* disabled until that corridor drains, so its I/O worker needs replay-scoped
  \* fairness independent of post-GST fairness.
  /\ WF_AsyncAllVars(ResponsiveReplayRunNode)
  /\ WF_AsyncAllVars(ResponsiveReplayServiceIoWorker)
  /\ WF_AsyncAllVars(DriveResponsiveReplayHead)
  /\ WF_AsyncAllVars(FinishResponsiveReplay)
  /\ \A node \in Responsive:
       WF_AsyncAllVars(AsyncActivateServiceNode(node))
  /\ WF_AsyncAllVars(AsyncTick)
  /\ \A node \in AsyncVotersAt(initialContext):
       WF_AsyncAllVars(PostGstRunNode(node))
  /\ \A node \in Responsive:
       WF_AsyncAllVars(PostGstOpenHistoricalRecovery(node))
  /\ \A node \in Responsive:
       WF_AsyncAllVars(PostGstRunHistoricalRecoveryNode(node))
  /\ \A node \in Responsive:
       WF_AsyncAllVars(PostGstRunHistoricalServer(node))
  /\ \A node \in AsyncVotersAt(initialContext):
       WF_AsyncAllVars(PostGstCommitCertificateDiscovery(node))
  /\ \A node \in Responsive:
       WF_AsyncAllVars(PostGstHistoricalCommitCertificateDiscovery(node))
  /\ \A node \in Responsive:
       WF_AsyncAllVars(PostGstServiceIoWorker(node))
  /\ \A node \in Responsive:
       WF_AsyncAllVars(PostGstServiceHistoricalRecoveryIoWorker(node))
  /\ \A recipient \in Responsive,
       source \in AsyncIngressSources:
       WF_AsyncAllVars(PostGstAdmitHiddenPacket(recipient, source))
  /\ \A recipient \in ValidatorIds, source \in AsyncIngressSources:
       WF_AsyncAllVars(
         PostGstAdmitHistoricalRecoveryPacket(recipient, source))

AsyncFairness == AsyncFairnessAt(ContextRecord(0, <<>>))

(***************************************************************************
Initialization, invariants, refinement boundary, and release properties.
***************************************************************************)

AsyncRuntimeInit ==
  /\ asyncNow = 0
  /\ asyncCommandQueues = [node \in ValidatorIds |-> <<>>]
  /\ asyncNextCommandClass =
       [node \in ValidatorIds |-> "Completion"]
  /\ asyncFifoOwed = [node \in ValidatorIds |-> FALSE]
  /\ asyncTimeoutEmitted = [node \in ValidatorIds |-> FALSE]
  /\ asyncRunnerPhase = [node \in ValidatorIds |-> "Local"]
  /\ asyncRunnerBudget =
       [node \in ValidatorIds |-> AsyncQueueCapacity]
  /\ asyncCausalAdmissionOwed =
       [node \in ValidatorIds |-> FALSE]
  /\ asyncNextLocalSource =
       [node \in ValidatorIds |-> "Producer"]
  /\ asyncCausalQueues =
       [node \in ValidatorIds |->
          <<NoItemCandidate("Normal", "AssembleBody", node, nodeView[node],
                            AsyncProposalSubject(node))>>]

AsyncIoInit ==
  /\ asyncIoQueues = [node \in ValidatorIds |-> <<>>]
  /\ asyncNextServeAdmissionOrdinal =
       [node \in ValidatorIds |-> 1]
  /\ asyncNextServeIngressOrdinal =
       [node \in ValidatorIds |-> 1]
  /\ asyncServeIngressAdmissions = {}
  /\ asyncServeAdmissions = {}
  /\ asyncServeReservations = {}
  /\ asyncServeTombstones = {}
  /\ asyncOutstandingWork = [node \in ValidatorIds |-> {}]
  /\ asyncIoReadyCompletions = [node \in ValidatorIds |-> <<>>]
  /\ asyncLocalReadyCompletions = [node \in ValidatorIds |-> <<>>]
  /\ asyncNextCompletionSource = [node \in ValidatorIds |-> "Io"]
  /\ asyncIoControlAvailable = [node \in ValidatorIds |-> TRUE]

AsyncDeferredInit ==
  /\ asyncDeferredCompletionQueues =
       [node \in ValidatorIds |-> <<>>]
  /\ asyncDeferredProgressQueues = [node \in ValidatorIds |-> <<>>]
  /\ asyncDeferredNormalQueues = [node \in ValidatorIds |-> <<>>]
  /\ asyncDeferredHandoffs =
       [node \in ValidatorIds |-> NoAsyncDeferredHandoff]
  /\ asyncNextDeferredClass =
       [node \in ValidatorIds |-> "Completion"]
  /\ asyncDeferredDrainOwed = [node \in ValidatorIds |-> FALSE]

AsyncInitialCandidateLifecycleAdmissions ==
  {AsyncCandidateLifecycleAdmission(
     node,
     NoItemCandidate("Normal", "AssembleBody", node, nodeView[node],
                     AsyncProposalSubject(node)).causalOrigin,
     1, 1, FALSE):
     node \in ValidatorIds}

AsyncTransportInit ==
  /\ asyncOutstandingTags = [node \in ValidatorIds |-> {}]
  /\ asyncNodeDeadlines =
       [node \in ValidatorIds |-> AsyncViewTimeout(nodeView[node])]
  /\ asyncRetransmitDeadlines =
       [node \in ValidatorIds |-> AsyncRetransmitPeriod]
  /\ asyncNodeServiceDeadlines =
       [node \in ValidatorIds |-> AsyncDeliveryBound]
  /\ asyncIoServiceDeadlines =
       [node \in ValidatorIds |-> AsyncDeliveryBound]
  /\ asyncSentItems = {}
  /\ asyncRetainedControl = {}
  /\ asyncActiveRequests = {}
  /\ asyncCertifiedResponseClaim = {}
  /\ asyncTransport = {}
  /\ asyncHeldChunks = {}
  /\ asyncHistoricalRecoveryTargets = {}
  /\ asyncControlServiceState =
       [nextOrdinal |-> [node \in ValidatorIds |-> 1],
        slots |-> {},
        certifiedResponseNextOrdinal |->
          [node \in ValidatorIds |-> 1],
        certifiedResponseClaims |-> {},
        candidateServiceNextOrdinal |->
          [node \in ValidatorIds |-> 1],
        candidateServiceMarkers |-> {},
        candidateTerminalTombstones |-> {},
        candidateLifecycleNextOrdinal |->
          [node \in ValidatorIds |-> 2],
        candidateLifecycleAdmissions |->
          AsyncInitialCandidateLifecycleAdmissions,
        timeoutLifecycleOrdinal |->
          [node \in ValidatorIds |-> 0],
        timeoutLifecycleOrigin |->
          [node \in ValidatorIds |-> NoAsyncCandidateLifecycleOrigin]]
  /\ asyncServiceActivationState =
       [restricted |-> FALSE, activeNodes |-> ValidatorIds]

THEOREM AsyncControlServiceRolloverInstanceStartsEmpty ==
  AsyncTransportInit
    => /\ AsyncControlServiceSlots = {}
       /\ \A node \in ValidatorIds:
            AsyncNextControlServiceOrdinal(node) = 1
BY DEF AsyncTransportInit, AsyncControlServiceSlots,
       AsyncNextControlServiceOrdinal

THEOREM AsyncCertifiedResponseClaimRolloverInstanceStartsEmpty ==
  AsyncTransportInit
    => /\ AsyncCertifiedResponseClaimRecords = {}
       /\ \A node \in ValidatorIds:
            AsyncNextCertifiedResponseClaimOrdinal(node) = 1
BY DEF AsyncTransportInit,
       AsyncCertifiedResponseClaimRecords,
       AsyncNextCertifiedResponseClaimOrdinal

THEOREM AsyncCandidateServiceRolloverInstanceStartsEmpty ==
  AsyncTransportInit
    => /\ AsyncCandidateServiceMarkers = {}
       /\ AsyncCandidateTerminalTombstones = {}
       /\ AsyncCandidateServiceTombstones = {}
       /\ \A node \in ValidatorIds:
            AsyncNextCandidateServiceOrdinal(node) = 1
BY DEF AsyncTransportInit,
       AsyncCandidateServiceMarkers,
       AsyncCandidateTerminalTombstones,
       AsyncCandidateServiceTombstones,
       AsyncNextCandidateServiceOrdinal

THEOREM AsyncCandidateLifecycleRolloverStartsWithRootOwners ==
  AsyncRuntimeInit /\ AsyncTransportInit
    => /\ AsyncCandidateLifecycleAdmissions
             = AsyncInitialCandidateLifecycleAdmissions
       /\ Cardinality(AsyncCandidateLifecycleAdmissions) = N
       /\ \A node \in ValidatorIds:
            /\ AsyncNextCandidateLifecycleOrdinal(node) = 2
            /\ AsyncTimeoutLifecycleOrdinal(node) = 0
            /\ AsyncTimeoutLifecycleOrigin(node)
                 = NoAsyncCandidateLifecycleOrigin
            /\ AsyncCandidateLifecycleRecorded(
                 node,
                 NoItemCandidate(
                   "Normal", "AssembleBody", node, nodeView[node],
                   AsyncProposalSubject(node)).causalOrigin)
BY SMT DEF AsyncRuntimeInit, AsyncTransportInit,
           AsyncInitialCandidateLifecycleAdmissions,
           AsyncCandidateLifecycleAdmissions,
           AsyncNextCandidateLifecycleOrdinal,
           AsyncTimeoutLifecycleOrdinal,
           AsyncTimeoutLifecycleOrigin,
           AsyncCandidateLifecycleRecorded,
           AsyncCandidateLifecycleRecordsFor,
           NoItemCandidate

AsyncIngressInit ==
  /\ asyncIngressLanes =
       [recipient \in ValidatorIds |->
          [source \in AsyncIngressSources |-> <<>>]]
  /\ asyncIngressReady = [recipient \in ValidatorIds |-> <<>>]

AsyncRecoveryInit ==
  /\ asyncRecoveryPhase = "Eligible"
  /\ asyncRecoveryNode = 0
  /\ asyncRecoveryGeneration = 0
  /\ asyncRecoveryReplayQueue = <<>>
  /\ asyncHistoricalLockRestartAuthorities = {}

AsyncBaseInitAt(initialContext) ==
  /\ InitAt(initialContext)
  /\ AsyncConfiguration
  /\ AsyncRuntimeInit
  /\ AsyncIoInit
  /\ AsyncDeferredInit
  /\ AsyncTransportInit
  /\ AsyncIngressInit
  /\ AsyncRecoveryInit

AsyncBaseInit == AsyncBaseInitAt(ContextRecord(0, <<>>))

AsyncInitAt(initialContext) ==
  AsyncBaseInitAt(initialContext) /\ ViewDomain = Nat

AsyncInit == AsyncInitAt(ContextRecord(0, <<>>))

AsyncFiniteInitAt(initialContext) ==
  AsyncBaseInitAt(initialContext) /\ ViewDomain = FiniteViews

AsyncFiniteInit == AsyncFiniteInitAt(ContextRecord(0, <<>>))

AsyncSpec ==
  AsyncInit /\ [][AsyncNext]_AsyncAllVars /\ AsyncFairness

AsyncSpecAt(initialContext) ==
  AsyncInitAt(initialContext)
    /\ [][AsyncNext]_AsyncAllVars
    /\ AsyncFairnessAt(initialContext)

AsyncFiniteSpec ==
  AsyncFiniteInit /\ [][AsyncNext]_AsyncAllVars /\ AsyncFairness

AsyncFiniteSpecAt(initialContext) ==
  AsyncFiniteInitAt(initialContext)
    /\ [][AsyncNext]_AsyncAllVars
    /\ AsyncFairnessAt(initialContext)

(***************************************************************************
PersistInstallTC remains a checked transaction: a same-view increment at an
unrepresentable generation rejects before changing the durable snapshot.  A
normal TC or full-process restart resets generation to zero, and each
same-round increment consumes a strictly higher retained Prepare rank.  The
predicate below remains a diagnostic mutation boundary, never a liveness
premise; the type/rank invariant makes exhaustion unreachable in a valid
finite instance.
***************************************************************************)
AsyncInstallGenerationBudget ==
  \A request \in pendingInstallTC:
    (StrictSameRoundTcUpgrade(request.node, request.tc)
      => GenerationCanIncrement(generation[request.node]))

AsyncLiveSpecAt(initialContext) == AsyncSpecAt(initialContext)

AsyncFiniteLiveSpec == AsyncFiniteSpec

(*
Command-bearing queues are indexed by the validator that owns every candidate
in the queue.  Runtime deferral moves a dequeued command into `command.node`'s
deferred queue, so completion-reserve preservation depends on that node being
the same queue owner; structural candidate typing alone is not sufficient.
*)
AsyncCommandQueueOwnership(node, queue) ==
  \A candidate \in SequenceSet(queue): candidate.node = node

AsyncRuntimeScalarTypeInvariant ==
  /\ AsyncConfiguration
  /\ asyncNow \in Nat
  /\ DOMAIN asyncCommandQueues = ValidatorIds
  /\ \A node \in ValidatorIds:
       /\ AsyncQueueTyped(asyncCommandQueues[node])
       /\ AsyncCommandQueueOwnership(node, asyncCommandQueues[node])
  /\ asyncNextCommandClass \in [ValidatorIds -> AsyncCommandClasses]
  /\ asyncFifoOwed \in [ValidatorIds -> BOOLEAN]
  /\ asyncTimeoutEmitted \in [ValidatorIds -> BOOLEAN]
  /\ asyncRunnerPhase \in
       [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
  /\ asyncRunnerBudget \in
       [ValidatorIds -> 0..(AsyncQueueCapacity + AsyncIngressCapacity)]
  /\ asyncCausalAdmissionOwed \in [ValidatorIds -> BOOLEAN]
  /\ asyncNextLocalSource \in [ValidatorIds -> AsyncLocalSources]

AsyncCausalQueueOwnership(node, queue) ==
  \A candidate \in SequenceSet(queue): candidate.node = node

AsyncCausalTypeInvariant ==
  /\ DOMAIN asyncCausalQueues = ValidatorIds
  /\ \A node \in ValidatorIds:
       /\ AsyncQueueTyped(asyncCausalQueues[node])
       /\ AsyncCausalQueueOwnership(node, asyncCausalQueues[node])
       /\ Len(asyncCausalQueues[node])
            <= AsyncCausalCandidateLifecycleCapacity

AsyncRuntimeTypeInvariant ==
  /\ AsyncRuntimeScalarTypeInvariant
  /\ AsyncCausalTypeInvariant

AsyncServeIngressAdmissionTyped(admission) ==
  /\ DOMAIN admission =
       {"node", "identity", "ordinal", "schedulerOrdinal",
        "ingressPredecessors"}
  /\ admission.node \in ValidatorIds
  /\ admission.identity \in AsyncServeLogicalRequestIdentities
  /\ admission.identity.owner = admission.node
  /\ admission.ordinal \in Nat \ {0}
  /\ admission.schedulerOrdinal \in Nat \ {0}
  /\ admission.ingressPredecessors
       \in [AsyncIngressSources -> 0..AsyncIngressCapacity]

AsyncServeAdmissionTyped(admission) ==
  /\ DOMAIN admission =
       {"node", "identity", "family", "view", "ordinal"}
  /\ admission.node \in ValidatorIds
  /\ admission.identity \in AsyncServeLogicalRequestIdentities
  /\ admission.family \in AsyncServeLifecycleFamilies
  /\ admission.identity.owner = admission.node
  /\ admission.family.owner = admission.node
  /\ admission.view \in Views
  /\ AsyncServeLifecycleCoordinates(
       admission.node, admission.identity,
       admission.family, admission.view)
  /\ admission.ordinal \in Nat \ {0}

AsyncServeTombstoneOutputMatchesIdentity(tombstone, output) ==
  IF tombstone.identity.request.kind = "CertifiedRequest"
  THEN /\ output.kind = "CertifiedResponse"
       /\ output.envelope.archiveServer = tombstone.node
       /\ output.envelope.requestHash =
            tombstone.identity.request.requestHash
  ELSE /\ tombstone.identity.request.kind =
             "CommitCertificateRequest"
       /\ output.kind = "CommitCertificateResponse"
       /\ AsyncServeLogicalRequestIdentity(
            tombstone.node, output.envelope.request)
            = tombstone.identity

AsyncServeTombstoneTyped(tombstone) ==
  /\ DOMAIN tombstone =
       {"node", "identity", "family", "view", "ordinal", "outputs"}
  /\ tombstone.node \in ValidatorIds
  /\ tombstone.identity \in AsyncServeLogicalRequestIdentities
  /\ tombstone.family \in AsyncServeLifecycleFamilies
  /\ tombstone.identity.owner = tombstone.node
  /\ tombstone.family.owner = tombstone.node
  /\ tombstone.view \in Views
  /\ AsyncServeLifecycleCoordinates(
       tombstone.node, tombstone.identity,
       tombstone.family, tombstone.view)
  /\ tombstone.ordinal \in Nat \ {0}
  /\ IsFiniteSet(tombstone.outputs)
  /\ tombstone.outputs # {}
  /\ tombstone.outputs \subseteq
       {item \in AsyncNetworkItems:
          item.kind \in {"CertifiedResponse",
                         "CommitCertificateResponse"}}
  /\ \A output \in tombstone.outputs:
       AsyncServeTombstoneOutputMatchesIdentity(
         tombstone, output)

AsyncServeReservationTyped(reservation) ==
  /\ DOMAIN reservation =
       {"node", "identity", "family", "view", "ordinal",
        "predecessors", "ingressPredecessors", "rollbackTombstones"}
  /\ reservation.node \in ValidatorIds
  /\ reservation.identity \in AsyncServeLogicalRequestIdentities
  /\ reservation.family \in AsyncServeLifecycleFamilies
  /\ reservation.identity.owner = reservation.node
  /\ reservation.family.owner = reservation.node
  /\ reservation.view \in Views
  /\ AsyncServeLifecycleCoordinates(
       reservation.node, reservation.identity,
       reservation.family, reservation.view)
  /\ reservation.ordinal \in Nat \ {0}
  /\ IsFiniteSet(reservation.predecessors)
  /\ Cardinality(reservation.predecessors) <= AsyncIoCapacity
  /\ \A job \in reservation.predecessors: AsyncIoJobTyped(job)
  /\ reservation.ingressPredecessors
       \in [AsyncIngressSources -> 0..AsyncIngressCapacity]
  /\ IsFiniteSet(reservation.rollbackTombstones)
  /\ Cardinality(reservation.rollbackTombstones) <= 1
  /\ \A tombstone \in reservation.rollbackTombstones:
       /\ AsyncServeTombstoneTyped(tombstone)
       /\ tombstone.node = reservation.node
       /\ tombstone.family = reservation.family
       /\ tombstone.view < reservation.view
       /\ tombstone.ordinal < reservation.ordinal

AsyncServeAdmissionUniquenessInvariant ==
  /\ \A left, right \in asyncServeAdmissions:
       /\ left.node = right.node
       /\ left.identity = right.identity
       => left = right
  /\ \A left, right \in asyncServeAdmissions:
       /\ left.node = right.node
       /\ left.ordinal = right.ordinal
       => left = right

AsyncServeIngressAdmissionInvariant ==
  /\ \A admission \in asyncServeIngressAdmissions:
       /\ AsyncServeIngressAdmissionTyped(admission)
       /\ admission.identity
            \in AsyncServeIngressReservationIdentities(admission.node)
       /\ \A source \in AsyncIngressSources:
            admission.ingressPredecessors[source]
              <= Len(asyncIngressLanes[admission.node][source])
  /\ \A left, right \in asyncServeIngressAdmissions:
       /\ left.node = right.node
       /\ left.identity = right.identity
       => left = right
  /\ \A left, right \in asyncServeIngressAdmissions:
       /\ left.node = right.node
       /\ left.ordinal = right.ordinal
       => left = right
  /\ \A left, right \in asyncServeIngressAdmissions:
       /\ left.node = right.node
       /\ left.schedulerOrdinal = right.schedulerOrdinal
       => left = right
  /\ \A node \in ValidatorIds,
       source \in AsyncIngressSources,
       index \in 1..Len(asyncIngressLanes[node][source]):
       LET item == asyncIngressLanes[node][source][index]
       IN AsyncServeLifecycleAdmissionRequired(node, item)
            =>
          Cardinality(
            AsyncServeIngressAdmissionRecords(
              node,
              AsyncServeLogicalRequestIdentity(node, item))) = 1

(***************************************************************************
Draining a physical ingress item can consume a member of a live ticket's
frozen predecessor prefix, but it cannot add a replacement member.  This is
the model-side counterpart of the bounded Rust `serve_ingress_waiters`: all
waiters admitted before the timeout cut are charged once to the snapshot;
post-cut arrivals remain behind it and cannot replenish its priority debt.
***************************************************************************)
THEOREM AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain ==
  \A node \in ValidatorIds,
     index \in 1..Len(asyncIngressReady[node]),
     laneIndex \in
       1..Len(IngressLane(node, asyncIngressReady[node][index])),
     identity \in AsyncServeIngressLifecycleOwnerIdentities(node):
    /\ AsyncServeIngressAdmissionInvariant
    /\ PopSelectedIngress(node, index, laneIndex)
    /\ AsyncServeIngressAdmissionOwned(node, identity)'
    => \A source \in AsyncIngressSources:
         AsyncServeIngressAdmissionPredecessorCounts(node, identity)'[source]
           <= AsyncServeIngressAdmissionPredecessorCounts(
                node, identity)[source]
BY IsaT(300)
   DEF PopSelectedIngress,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionInvariant,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionPredecessorCounts,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionIdentities

(***************************************************************************
Serve tickets, Candidate roots, and the unmaterialized timeout reservation
draw from one per-node scheduler ordinal space.  Candidate lifecycle records
already inject among themselves; these cross-carrier clauses prevent a Serve
ticket from aliasing either a recorded Candidate root or the frozen timeout
root which has not yet materialized as a candidate.
***************************************************************************)
AsyncSharedSchedulerOrdinalInjectionInvariant ==
  /\ \A admission \in asyncServeIngressAdmissions,
        record \in AsyncCandidateLifecycleAdmissions:
       /\ admission.node = record.node
       => admission.schedulerOrdinal # record.ordinal
  /\ \A admission \in asyncServeIngressAdmissions:
       AsyncTimeoutLifecycleOwned(admission.node)
         => admission.schedulerOrdinal #
              AsyncTimeoutLifecycleOrdinal(admission.node)

AsyncServeTombstoneOutputBindingInvariant ==
  /\ \A tombstone \in asyncServeTombstones:
       \A output \in tombstone.outputs:
         AsyncServeTombstoneOutputMatchesIdentity(
           tombstone, output)
  /\ \A reservation \in asyncServeReservations:
       \A tombstone \in reservation.rollbackTombstones:
         \A output \in tombstone.outputs:
           AsyncServeTombstoneOutputMatchesIdentity(
             tombstone, output)

AsyncServeLifecyclePartitionInvariant ==
  /\ \A admission \in asyncServeAdmissions:
       AsyncServeReservationRecords(
         admission.node, admission.identity) =
           {AsyncServeReservation(
              admission.node, admission.identity, admission.family,
              admission.view, admission.ordinal,
              AsyncServeFrozenPredecessorSet(
                admission.node, admission.identity),
              AsyncServeFrozenIngressPredecessorCounts(
                admission.node, admission.identity),
              AsyncServeReservationRecord(
                admission.node, admission.identity).rollbackTombstones)}
  /\ \A reservation \in asyncServeReservations:
       AsyncServeAdmissionRecords(
         reservation.node, reservation.identity) =
           {AsyncServeAdmission(
              reservation.node, reservation.identity, reservation.family,
              reservation.view,
              reservation.ordinal)}
  /\ \A tombstone \in asyncServeTombstones:
       /\ AsyncServeAdmissionRecords(
            tombstone.node, tombstone.identity) = {}
       /\ AsyncServeReservationRecords(
            tombstone.node, tombstone.identity) = {}
  /\ \A reservation \in asyncServeReservations:
       \A tombstone \in reservation.rollbackTombstones:
         /\ tombstone \notin asyncServeTombstones
         /\ AsyncServeAdmissionRecords(
              tombstone.node, tombstone.identity) = {}
         /\ AsyncServeReservationRecords(
              tombstone.node, tombstone.identity) = {}

AsyncServeFamilyHighWatermarkInvariant ==
  \A node \in ValidatorIds,
     family \in AsyncServeLifecycleFamilies:
    /\ Cardinality(
         AsyncServeFamilyReservationRecords(node, family)) +
         Cardinality(
           AsyncServeFamilyTombstoneRecords(node, family)) <= 1
    /\ \A left, right \in
         AsyncServeFamilyReservationRecords(node, family)
           \cup AsyncServeFamilyTombstoneRecords(node, family):
         left.view = right.view

AsyncServeHighWatermarkActiveRequestInvariant ==
  \A request \in asyncActiveRequests:
    AsyncServeRequestRespectsHighWatermark(request)

AsyncServeReservationOwnershipInvariant ==
  \A node \in ValidatorIds:
    /\ Cardinality(
         {reservation \in asyncServeReservations:
            reservation.node = node})
           <= AsyncServeLifecycleFamilyBudget
    /\ \A identity \in AsyncIoServeIdentities(node):
         Cardinality(
           AsyncServeReservationRecords(node, identity)) = 1
    /\ \A reservation \in
         {owned \in asyncServeReservations: owned.node = node}:
         LET predecessorIndices ==
               AsyncServeRemainingPredecessorIndices(
                 node, reservation.identity)
             materializationPredecessorIndices ==
               AsyncServeMaterializationPredecessorIndices(
                 node, reservation.identity)
         IN /\ Cardinality(
                  AsyncIoServeIdentityIndices(
                    node, reservation.identity)) <= 1
            /\ predecessorIndices =
                 1..Cardinality(predecessorIndices)
            /\ materializationPredecessorIndices =
                 1..Cardinality(materializationPredecessorIndices)
            /\ reservation.predecessors =
                 {asyncIoQueues[node][index]:
                    index \in predecessorIndices}
            /\ AsyncIoJobsBeforeServeIdentity(
                 node, reservation.identity)
                 \subseteq
                   reservation.predecessors
                     \cup
                       AsyncServeMaterializationPredecessorJobs(
                         node, reservation.identity)
            /\ \A index \in
                 AsyncIoServeIdentityIndices(
                   node, reservation.identity):
                 asyncIoQueues[node][index]
                   \notin reservation.predecessors

AsyncServeSingularOffQueueBarrierInvariant ==
  \A node \in ValidatorIds:
    Cardinality(AsyncServeOffQueueReservations(node)) <= 1

AsyncServeBarrierOwnsEarliestIngressOrdinalInvariant ==
  \A node \in ValidatorIds:
    \A reservation \in AsyncServeOffQueueReservations(node):
      /\ AsyncServeIngressAdmissionOwned(
           node, reservation.identity)
      /\ \A identity \in
           AsyncServeIngressLifecycleOwnerIdentities(node):
           AsyncServeIngressAdmissionOrdinal(
             node, reservation.identity)
             <= AsyncServeIngressAdmissionOrdinal(node, identity)

AsyncServeOrdinalQueueInvariant ==
  \A node \in ValidatorIds:
    \A left, right \in
         AsyncIoServeIndices(asyncIoQueues[node]):
      left < right =>
        AsyncServeAdmissionOrdinal(
          node,
          AsyncIoServeJobIdentity(
            node, asyncIoQueues[node][left]))
          <
        AsyncServeAdmissionOrdinal(
          node,
          AsyncIoServeJobIdentity(
            node, asyncIoQueues[node][right]))

AsyncServeIngressPredecessorInvariant ==
  /\ \A reservation \in asyncServeReservations:
       \A source \in AsyncIngressSources:
         reservation.ingressPredecessors[source]
           <= Len(asyncIngressLanes[reservation.node][source])
  /\ \A node \in ValidatorIds:
       AsyncServeIngressLifecycleOwnerIdentities(node) # {} =>
         LET ownerIdentity ==
               AsyncServeEarliestIngressLifecycleOwnerIdentity(node)
         IN \A source \in AsyncIngressSources:
              \A index \in
                   DrainableIngressLaneIndices(node, source):
                \/ index <=
                     AsyncServeIngressAdmissionPredecessorCounts(
                       node, ownerIdentity)[source]
                \/ /\ asyncIngressLanes[node][source][index].kind
                         \in AsyncReplyRequestKinds
                   /\ AsyncServeLogicalRequestIdentity(
                        node,
                        asyncIngressLanes[node][source][index])
                        = ownerIdentity

AsyncServeServiceabilityInvariant ==
  \A node \in ValidatorIds:
    /\ \A source \in AsyncIngressSources:
         \A index \in 1..Len(asyncIngressLanes[node][source]):
           LET item == asyncIngressLanes[node][source][index]
           IN (/\ item.kind \in AsyncReplyRequestKinds
               /\ AsyncServeLiveReservationOwned(
                    node,
                    AsyncServeLogicalRequestIdentity(node, item)))
                => AsyncServeRequestServiceable(node, item)
    /\ \A index \in AsyncIoServeIndices(asyncIoQueues[node]):
         AsyncServeRequestServiceable(
           node, asyncIoQueues[node][index].candidate.item)

AsyncServeOrdinalInvariant ==
  /\ asyncNextServeIngressOrdinal
       \in [ValidatorIds -> Nat \ {0}]
  /\ asyncNextServeAdmissionOrdinal
       \in [ValidatorIds -> Nat \ {0}]
  /\ \A admission \in asyncServeIngressAdmissions:
       admission.ordinal <
         asyncNextServeIngressOrdinal[admission.node]
  /\ \A admission \in asyncServeIngressAdmissions:
       admission.schedulerOrdinal <
         AsyncNextCandidateLifecycleOrdinal(admission.node)
  /\ \A admission \in asyncServeAdmissions:
       admission.ordinal <
         asyncNextServeAdmissionOrdinal[admission.node]
  /\ \A tombstone \in asyncServeTombstones:
       tombstone.ordinal <
         asyncNextServeAdmissionOrdinal[tombstone.node]
  /\ \A reservation \in asyncServeReservations:
       \A tombstone \in reservation.rollbackTombstones:
         tombstone.ordinal <
           asyncNextServeAdmissionOrdinal[tombstone.node]

AsyncServeLifecycleTypeInvariant ==
  /\ IsFiniteSet(asyncServeIngressAdmissions)
  /\ IsFiniteSet(asyncServeAdmissions)
  /\ IsFiniteSet(asyncServeReservations)
  /\ IsFiniteSet(asyncServeTombstones)
  /\ Cardinality(asyncServeTombstones) <=
       Cardinality(AsyncServeLifecycleFamilies)
  /\ \A admission \in asyncServeAdmissions:
       AsyncServeAdmissionTyped(admission)
  /\ \A reservation \in asyncServeReservations:
       AsyncServeReservationTyped(reservation)
  /\ \A tombstone \in asyncServeTombstones:
       AsyncServeTombstoneTyped(tombstone)
  /\ AsyncServeIngressAdmissionInvariant
  /\ AsyncSharedSchedulerOrdinalInjectionInvariant
  /\ AsyncServeAdmissionUniquenessInvariant
  /\ AsyncServeTombstoneOutputBindingInvariant
  /\ AsyncServeLifecyclePartitionInvariant
  /\ AsyncServeFamilyHighWatermarkInvariant
  /\ AsyncServeHighWatermarkActiveRequestInvariant
  /\ AsyncServeReservationOwnershipInvariant
  /\ AsyncServeSingularOffQueueBarrierInvariant
  /\ AsyncServeBarrierOwnsEarliestIngressOrdinalInvariant
  /\ AsyncServeOrdinalQueueInvariant
  /\ AsyncServeIngressPredecessorInvariant
  /\ AsyncServeServiceabilityInvariant
  /\ AsyncServeOrdinalInvariant

THEOREM AsyncServeIngressSchedulerOrdinalIsTyped ==
  AsyncServeLifecycleTypeInvariant
    => \A admission \in asyncServeIngressAdmissions:
         /\ admission.schedulerOrdinal \in Nat \ {0}
         /\ admission.schedulerOrdinal <
              AsyncNextCandidateLifecycleOrdinal(admission.node)
BY Isa
   DEF AsyncServeLifecycleTypeInvariant,
       AsyncServeIngressAdmissionInvariant,
       AsyncServeIngressAdmissionTyped,
       AsyncServeOrdinalInvariant

THEOREM AsyncServeIngressSharedSchedulerInitIsEmptyAndInjected ==
  /\ AsyncIoInit
  /\ AsyncTransportInit
  => /\ asyncServeIngressAdmissions = {}
     /\ AsyncSharedSchedulerOrdinalInjectionInvariant
     /\ \A node \in ValidatorIds:
          AsyncNextCandidateLifecycleOrdinal(node) = 2
BY Isa
   DEF AsyncIoInit, AsyncTransportInit,
       AsyncSharedSchedulerOrdinalInjectionInvariant,
       AsyncNextCandidateLifecycleOrdinal,
       AsyncTimeoutLifecycleOwned,
       AsyncTimeoutLifecycleOrdinal,
       AsyncTimeoutLifecycleOrigin

AsyncIoTopologyTypeInvariant ==
  /\ DOMAIN asyncIoQueues = ValidatorIds
  /\ DOMAIN asyncOutstandingWork = ValidatorIds
  /\ DOMAIN asyncIoReadyCompletions = ValidatorIds
  /\ DOMAIN asyncLocalReadyCompletions = ValidatorIds
  /\ asyncNextCompletionSource \in [ValidatorIds -> {"Io", "Local"}]
  /\ asyncIoControlAvailable \in [ValidatorIds -> BOOLEAN]

AsyncIoConsensusIndices(queue) ==
  {index \in 1..Len(queue): queue[index].class = "Consensus"}

AsyncIoConsensusQueueOwnership(queue, ioReadyQueue, localReadyQueue) ==
  LET ioReady == SequenceSet(ioReadyQueue)
      localReady == SequenceSet(localReadyQueue)
  IN /\ \A index \in AsyncIoConsensusIndices(queue):
           /\ queue[index].candidate \notin ioReady
           /\ queue[index].candidate \notin localReady
     /\ \A left, right \in AsyncIoConsensusIndices(queue):
          queue[left].candidate = queue[right].candidate => left = right

AsyncIoConsensusCandidateOwnership(node, queues, ioReadyQueues,
                                   localReadyQueues) ==
  AsyncIoConsensusQueueOwnership(
    queues[node], ioReadyQueues[node], localReadyQueues[node])

AsyncIoQueueContentTypeInvariant ==
  \A node \in ValidatorIds:
    /\ AsyncIoSequenceTyped(asyncIoQueues[node])
    /\ AsyncIoServeNonceOwnership(asyncIoQueues[node])
    /\ \A job \in SequenceSet(asyncIoQueues[node]):
         job.class = "Consensus" =>
           job.candidate \in asyncOutstandingWork[node]
    /\ AsyncIoConsensusCandidateOwnership(
         node, asyncIoQueues, asyncIoReadyCompletions,
         asyncLocalReadyCompletions)

AsyncIoWorkContentTypeInvariant ==
  \A node \in ValidatorIds:
       /\ IsFiniteSet(asyncOutstandingWork[node])
       /\ \A candidate \in asyncOutstandingWork[node]:
            /\ AsyncCandidateTyped(candidate)
            /\ candidate.class = "Completion"
            /\ candidate.node = node
       /\ AsyncCompletionSequenceTyped(asyncIoReadyCompletions[node])
       /\ AsyncCompletionSequenceTyped(asyncLocalReadyCompletions[node])
       /\ Len(asyncIoReadyCompletions[node]) =
            Cardinality(SequenceSet(asyncIoReadyCompletions[node]))
       /\ Len(asyncLocalReadyCompletions[node]) =
            Cardinality(SequenceSet(asyncLocalReadyCompletions[node]))
       /\ SequenceSet(asyncIoReadyCompletions[node]) \subseteq
            asyncOutstandingWork[node]
       /\ SequenceSet(asyncLocalReadyCompletions[node]) \subseteq
            asyncOutstandingWork[node]
       /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
            SequenceSet(asyncLocalReadyCompletions[node]) = {}
       /\ SequenceSet(asyncCommandQueues[node]) \cap
            asyncOutstandingWork[node] = {}

ConsensusIoCandidates(node) ==
  {job.candidate:
     job \in {entry \in SequenceSet(asyncIoQueues[node]):
                entry.class = "Consensus"}}

(***************************************************************************
The outstanding-work set is an exact ownership index, not an upper bound on
the independent runtime/deferred completion lanes.  Each entry is carried by
one Consensus I/O job or one producer-ready queue, and those are the only
places from which the producer can retire it.
***************************************************************************)
AsyncOutstandingCarrierInvariant ==
  \A node \in ValidatorIds:
    asyncOutstandingWork[node] =
      ConsensusIoCandidates(node)
        \cup SequenceSet(asyncIoReadyCompletions[node])
        \cup SequenceSet(asyncLocalReadyCompletions[node])

SerializedBusyOwners ==
  AllPendingRequests \cup signProposals \cup signVotes \cup signTimeouts

SerializedBusyOwnershipInvariant ==
  RequestsUniqueByNode(SerializedBusyOwners)

(***************************************************************************
Every serialized Busy owner carries the exact guard required by its Core
completion action.  Ownership alone is insufficient: a matching Completion
candidate can otherwise reach the FIFO head while the corresponding action is
disabled (for example, an already-materialized proposal intent or a stale
InstallTC request).

Readiness moves with serialized ownership.  In particular, preserving only
the guards is not inductive when two owners for one node coexist: installing a
TC can advance that node's view and invalidate a distinct vote-sign request.
The combined kernel therefore keeps readiness and node uniqueness together.
***************************************************************************)
AsyncBusyReadinessInvariant ==
  /\ \A request \in pendingProposal:
       /\ request.proposal.proposer = request.node
       /\ request.proposal.context = context
       /\ request.proposal.view = nodeView[request.node]
       /\ request.proposal \notin proposalIntents
  /\ \A request \in pendingPrepare:
       request.vote \notin prepareIntents
  /\ \A request \in pendingLockCommit:
       /\ request.vote \notin commitIntents
       /\ BodyHeldBy(durableBodies, request.node, request.qc.context,
                     request.qc.view, request.qc.subject)
       /\ RetainedLockedBodyRecord(
            request.node, request.qc.context, request.qc.subject)
            \in RetainedLockedBodyRecordSet
  /\ \A request \in pendingTimeout:
       request.vote \notin timeoutIntents
  /\ \A request \in pendingInstallTC:
       request.tc.view >= nodeView[request.node]
  /\ \A request \in signProposals:
       /\ request.proposal.proposer = request.node
       /\ request.proposal \in proposalIntents
  /\ \A request \in signVotes:
       /\ request.vote.signer = request.node
       /\ (request.vote \in prepareIntents
             \/ request.vote \in commitIntents)
       /\ VoteRoundAdmissible(request.node, request.vote)
  /\ \A request \in signTimeouts:
       /\ request.vote.signer = request.node
       /\ request.vote \in timeoutIntents

AsyncSerializedBusyKernelInvariant ==
  /\ SerializedBusyOwnershipInvariant
  /\ AsyncBusyReadinessInvariant

ActiveBusyCompletionCarrier ==
  QueuedCandidates \cup CausalCandidates \cup TrackedWorkCandidates

BusyCompletionCandidates(node) ==
  {candidate \in ActiveBusyCompletionCarrier:
     /\ candidate.node = node
     /\ candidate.class = "Completion"
     /\ candidate.height = context.height
     /\ CandidateConsumerCurrent(candidate)
     /\ candidate.item = NoAsyncItem
     /\ \/ \E request \in pendingProposal:
              /\ request.node = node
              /\ candidate.kind = "PersistProposal"
              /\ candidate.view = request.proposal.view
              /\ candidate.subject = request.proposal.subject
        \/ \E request \in pendingPrepare:
              /\ request.node = node
              /\ candidate.kind = "PersistPrepare"
              /\ candidate.view = request.vote.view
              /\ candidate.subject = request.vote.subject
        \/ \E request \in pendingObservePrepare:
              /\ request.node = node
              /\ candidate.kind = "PersistObservePrepare"
              /\ candidate.view = request.qc.view
              /\ candidate.subject = request.qc.subject
        \/ \E request \in pendingLockCommit:
              /\ request.node = node
              /\ candidate.kind = "PersistLockCommit"
              /\ candidate.view = request.qc.view
              /\ candidate.subject = request.qc.subject
        \/ \E request \in pendingTimeout:
              /\ request.node = node
              /\ candidate.kind = "PersistTimeout"
              /\ candidate.view = request.vote.view
              /\ candidate.subject = request.vote.highSubject
        \/ \E request \in pendingInstallTC:
              /\ request.node = node
              /\ candidate.kind = "PersistInstallTC"
              /\ candidate.view = request.tc.view
        \/ \E request \in pendingDecision:
              /\ request.node = node
              /\ candidate.kind = "PersistDecision"
              /\ candidate.view = request.qc.view
              /\ candidate.subject = request.qc.subject
        \/ \E request \in signProposals:
              /\ request.node = node
              /\ candidate.kind = "SignProposal"
              /\ candidate.view = request.proposal.view
              /\ candidate.subject = request.proposal.subject
        \/ \E request \in signVotes:
              /\ request.node = node
              /\ candidate.kind = "SignVote"
              /\ candidate.view = request.vote.view
              /\ candidate.subject = request.vote.subject
        \/ \E request \in signTimeouts:
              /\ request.node = node
              /\ candidate.kind = "SignTimeout"
              /\ candidate.view = request.vote.view
              /\ candidate.subject = request.vote.highSubject}

(***************************************************************************
Checked generation overflow is represented explicitly for failing mutation
models instead of pretending the reducer can execute a saturating partial
install.  Under the production relation, generation is view-local and bounded
by the strictly increasing Prepare rank, so this diagnostic condition is
unreachable from the type/rank invariant and is not excluded by a temporal
assumption.
***************************************************************************)
InstallGenerationExhausted(node) ==
  \E request \in pendingInstallTC:
    /\ request.node = node
    /\ StrictSameRoundTcUpgrade(node, request.tc)
    /\ ~GenerationCanIncrement(generation[node])

(***************************************************************************
A busy reducer is never justified by a completion stranded behind the
production Busy-deferred head: its exact persistence/signature completion is
owned by the active causal/I/O/runtime pipeline.  The completion is therefore
reachable without first asking the busy reducer to accept unrelated work.
***************************************************************************)
BusyCompletionWitnessInvariant ==
  \A node \in ValidatorIds:
    ~NodeIdle(node) =>
      \/ BusyCompletionCandidates(node) # {}
      \/ InstallGenerationExhausted(node)

AsyncProgressOwnershipInvariant ==
  /\ AsyncLogicalCandidateOwnershipInvariant
  /\ AsyncOutstandingCarrierInvariant
  /\ SerializedBusyOwnershipInvariant
  /\ BusyCompletionWitnessInvariant

AsyncIoContentTypeInvariant ==
  /\ AsyncIoQueueContentTypeInvariant
  /\ AsyncIoWorkContentTypeInvariant

AsyncIoCapacityTypeInvariant ==
  \A node \in ValidatorIds:
       /\ AsyncQueueDepth(node) <= AsyncQueueCapacity
       /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
       /\ AsyncIoEffectiveQueueDepth(node) <= AsyncIoCapacity
       /\ AsyncOutstandingWorkCount(node) <= AsyncIoWorkCapacity

AsyncIoTypeInvariant ==
  /\ AsyncIoTopologyTypeInvariant
  /\ AsyncServeLifecycleTypeInvariant
  /\ AsyncIoContentTypeInvariant
  /\ AsyncIoCapacityTypeInvariant

AsyncDeferredTopologyTypeInvariant ==
  /\ DOMAIN asyncDeferredCompletionQueues = ValidatorIds
  /\ DOMAIN asyncDeferredProgressQueues = ValidatorIds
  /\ DOMAIN asyncDeferredNormalQueues = ValidatorIds
  /\ asyncDeferredHandoffs
       \in [ValidatorIds -> AsyncDeferredHandoffSet]
  /\ asyncNextDeferredClass \in
       [ValidatorIds -> AsyncCommandClasses]
  /\ asyncDeferredDrainOwed \in [ValidatorIds -> BOOLEAN]

AsyncDeferredContentTypeInvariant ==
  /\ \A node \in ValidatorIds:
       /\ AsyncCompletionSequenceTyped(
            asyncDeferredCompletionQueues[node])
       /\ AsyncQueueTyped(asyncDeferredProgressQueues[node])
       /\ AsyncQueueTyped(asyncDeferredNormalQueues[node])
       /\ AsyncCommandQueueOwnership(
            node, asyncDeferredCompletionQueues[node])
       /\ AsyncCommandQueueOwnership(
            node, asyncDeferredProgressQueues[node])
       /\ AsyncCommandQueueOwnership(
            node, asyncDeferredNormalQueues[node])
       /\ \A candidate \in SequenceSet(
              asyncDeferredProgressQueues[node]):
            candidate.class = "Progress"
       /\ \A candidate \in SequenceSet(asyncDeferredNormalQueues[node]):
            candidate.class = "Normal"
       /\ Len(asyncDeferredCompletionQueues[node]) <=
            AsyncDeferredNormalCapacity
       /\ Len(asyncDeferredProgressQueues[node]) <=
            AsyncDeferredProgressCapacity
       /\ Len(asyncDeferredNormalQueues[node]) <=
            AsyncDeferredNormalCapacity

AsyncDeferredHandoffOwnershipInvariant ==
  \A node \in ValidatorIds:
    IF DeferredHandoffActive(node)
    THEN LET candidate == DeferredHandoffCandidate(node)
         IN /\ AsyncCandidateTyped(candidate)
            /\ candidate.node = node
            /\ asyncDeferredHandoffs[node].identity =
                 ExactAsyncCandidateIdentity(candidate)
            /\ DeferredHandoffQueueHead(node)
    ELSE TRUE

(***************************************************************************
The queue-head relation is a scheduler safety invariant, not a carrier/type
fact.  It is therefore proved separately from `AsyncDeferredTypeInvariant`;
folding it into the type predicate would let unrelated type-preservation
lemmas silently stand in for the required exact-ownership induction.
***************************************************************************)
AsyncDeferredTypeInvariant ==
  /\ AsyncDeferredTopologyTypeInvariant
  /\ AsyncDeferredContentTypeInvariant

AsyncTransportClockTypeInvariant ==
  /\ asyncOutstandingTags \in
       [ValidatorIds -> SUBSET AsyncCompletionTags]
  /\ asyncNodeDeadlines \in [ValidatorIds -> Nat]
  /\ asyncRetransmitDeadlines \in [ValidatorIds -> Nat]
  /\ asyncNodeServiceDeadlines \in [ValidatorIds -> Nat]
  /\ asyncIoServiceDeadlines \in [ValidatorIds -> Nat]

THEOREM AsyncRetransmitProgramCounterIsBounded ==
  AsyncTransportClockTypeInvariant
    => \A node \in ValidatorIds:
         AsyncRetransmitProgramCounter(node)
           \in AsyncRetransmitProgramCounterStates
BY Isa
   DEF AsyncTransportClockTypeInvariant,
       AsyncRetransmitProgramCounter,
       AsyncRetransmitProgramCounterStates

AsyncActiveRequestLogicalIndexConsistencyInvariant ==
  AsyncCertifiedRequestLogicalIndexConsistent(asyncActiveRequests)

AsyncCertifiedResponseClaimInvariant ==
  /\ IsFiniteSet(asyncCertifiedResponseClaim)
  /\ asyncCertifiedResponseClaim
       \subseteq AsyncCertifiedResponseClaimValues
  /\ \A record \in AsyncCertifiedResponseClaimRecords:
       /\ record.family \in ActiveCertifiedRequestHashes
       /\ record.identity.requestHash = record.family
       /\ record.identity.responseHash
            \in asyncCertifiedResponseClaim
       /\ \E item \in AsyncCertifiedResponseItems:
            /\ record =
                 AsyncCertifiedResponseClaimRecord(
                   item, record.ordinal)
            /\ AsyncCertifiedResponseCanonicalWireIdentity(item)
                 \in asyncCertifiedResponseClaim
  /\ \A projection \in asyncCertifiedResponseClaim:
       Cardinality(
         {record \in AsyncCertifiedResponseClaimRecords:
            record.identity.responseHash = projection}) = 1
  /\ \A left, right \in AsyncCertifiedResponseClaimRecords:
       /\ left.recipient = right.recipient
       /\ left.ordinal = right.ordinal
       => left = right
  /\ \A requestHash \in AsyncCertifiedRequestHashes:
       Cardinality(
         CertifiedResponseClaimRecordsForFamily(requestHash)) <= 1
  /\ \A recipient \in ValidatorIds:
       Cardinality(
         CertifiedResponseClaimRecordsAt(recipient)) <= 1
  /\ \A recipient \in ValidatorIds:
       Cardinality(CertifiedResponseClaimsAt(recipient)) <= 1
  /\ \A projection \in asyncCertifiedResponseClaim:
       CertifiedResponseClaimProjectionAuthenticated(projection)
  /\ {projection.envelope.requestHash:
        projection \in asyncCertifiedResponseClaim}
       \subseteq ActiveCertifiedRequestHashes
  /\ \A requestHash \in ActiveCertifiedRequestHashes:
       \/ CertifiedResponseAuthorityReady(requestHash)
       \/ CertifiedResponseAuthorityClaimed(requestHash)
  /\ \A recipient \in ValidatorIds:
       Cardinality(
         {projection.envelope.requestHash:
            projection \in CertifiedResponseClaimsAt(recipient)}) <= 1

THEOREM CertifiedResponseClaimsShareOutstandingRequestCharge ==
  AsyncCertifiedResponseClaimInvariant
    => /\ Cardinality(AsyncCertifiedResponseClaimRecords)
             <= Cardinality(ActiveCertifiedRequestHashes)
       /\ \A record \in AsyncCertifiedResponseClaimRecords:
            record.family \in ActiveCertifiedRequestHashesAt(
              record.recipient)
BY FS_Subset, Isa
   DEF AsyncCertifiedResponseClaimInvariant,
       ActiveCertifiedRequestHashes,
       ActiveCertifiedRequestHashesAt,
       ActiveCertifiedRequestHashesIn,
       AsyncCertifiedResponseClaimRecords,
       AsyncCertifiedResponseClaimRecord,
       AsyncCertifiedResponseOccurrenceIdentity,
       AsyncCertifiedResponseWaiterFamily

CertifiedResponseClaimIngressOwner(projection) ==
  \E recipient \in ValidatorIds:
    \E index \in 1..IngressLaneDepth(recipient, AsyncUntrustedSource):
      LET item == IngressLane(recipient, AsyncUntrustedSource)[index]
      IN /\ item.kind = "CertifiedResponse"
         /\ item.envelope.recipient = recipient
         /\ IngressResourceSource(item) = AsyncUntrustedSource
         /\ AsyncCertifiedResponseCanonicalWireIdentity(item) = projection

(***************************************************************************
The Rust claim is not detached metadata: while it is live, one exact encoded
response remains the physical owner of the normalized aggregate-untrusted
completion slot.  Dequeue/backpressure/restore is abstracted as stutter, and
successful handoff or lifecycle retirement clears the claim atomically.
Stale duplicate occurrences may coexist, so this invariant requires an exact
owner but deliberately does not claim uniqueness of queue occurrences.
***************************************************************************)
AsyncCertifiedResponseClaimIngressOwnershipInvariant ==
  \A projection \in asyncCertifiedResponseClaim:
    CertifiedResponseClaimIngressOwner(projection)

THEOREM CertifiedResponseFamilyLocalClaimsRemainPhysicallySerialized ==
  \A item:
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ item.kind = "CertifiedResponse"
    /\ CertifiedResponseClaimsAt(item.envelope.recipient) # {}
    => ~AsyncTransportCompletionOwnerGateAllows(item)
BY Isa
   DEF AsyncCertifiedResponseClaimIngressOwnershipInvariant,
       CertifiedResponseClaimIngressOwner,
       AsyncTransportCompletionOwnerGateAllows,
       IngressLaneHasTransportCompletionIn,
       IngressUsesPhysicalCompletionOwner,
       IngressAdmissionClass,
       IngressResourceSource,
       IngressLane, IngressLaneDepth,
       SequenceSet

AsyncTransportHistoryTypeInvariant ==
  /\ IsFiniteSet(asyncSentItems)
  /\ \A item \in asyncSentItems: AsyncItemTyped(item)
  /\ IsFiniteSet(asyncRetainedControl)
  /\ \A item \in asyncRetainedControl:
       /\ AsyncItemTyped(item)
       /\ item.kind \in AsyncControlKinds
  /\ \A source \in ValidatorIds, controlClass \in AsyncControlKinds:
       LET retained ==
             RetainedClassItems(asyncRetainedControl, source, controlClass)
       IN \/ retained = {}
          \/ /\ Cardinality(retained) <= Cardinality(CurrentVoters)
             /\ {item.envelope.recipient: item \in retained}
                  = CurrentVoters
             /\ \A left, right \in retained:
                  ControlView(left) = ControlView(right)
  /\ IsFiniteSet(asyncActiveRequests)
  /\ asyncActiveRequests \subseteq asyncSentItems
  /\ \A item \in asyncActiveRequests:
       /\ AsyncItemTyped(item)
       /\ item.kind \in {"CertifiedRequest",
                          "CommitCertificateRequest"}
  /\ AsyncActiveRequestLogicalIndexConsistencyInvariant
  /\ AsyncCertifiedResponseClaimInvariant

AsyncPacketContentTypeInvariant ==
  /\ IsFiniteSet(asyncTransport)
  /\ \A packet \in asyncTransport: AsyncPacketTyped(packet)

AsyncHeldChunksTypeInvariant ==
  /\ asyncHeldChunks \subseteq AsyncChunkReceiptSet

AsyncHistoricalRecoveryTypeInvariant ==
  /\ asyncHistoricalRecoveryTargets \subseteq Responsive \cap up
  /\ (asyncHistoricalRecoveryTargets # {} => gst)
  /\ \A node \in asyncHistoricalRecoveryTargets:
       ~NodeHasApplication(node)

AsyncTransportContentTypeInvariant ==
  /\ AsyncTransportHistoryTypeInvariant
  /\ AsyncPacketContentTypeInvariant
  /\ AsyncHeldChunksTypeInvariant

AsyncTransportTypeInvariant ==
  /\ AsyncTransportClockTypeInvariant
  /\ AsyncTransportContentTypeInvariant

AsyncIngressTopologyTypeInvariant ==
  /\ DOMAIN asyncIngressLanes = ValidatorIds
  /\ DOMAIN asyncIngressReady = ValidatorIds
  /\ \A recipient \in ValidatorIds:
       /\ DOMAIN asyncIngressLanes[recipient] = AsyncIngressSources
       /\ DOMAIN asyncIngressReady[recipient] =
            1..Len(asyncIngressReady[recipient])
       /\ asyncIngressReady[recipient]
            \in Seq(Range(asyncIngressReady[recipient]))
       /\ SequenceSet(asyncIngressReady[recipient]) \subseteq
            AsyncIngressSources
       /\ Len(asyncIngressReady[recipient]) =
            Cardinality(SequenceSet(asyncIngressReady[recipient]))
       /\ SequenceSet(asyncIngressReady[recipient]) =
            {source \in AsyncIngressSources:
               IngressLaneDepth(recipient, source) > 0}

AsyncIngressCapacityTypeInvariant ==
  \A recipient \in ValidatorIds:
       /\ \A source \in AsyncIngressSources:
            IngressLaneDepth(recipient, source) <=
              AsyncIngressCapacity
       /\ IngressDepth(recipient) <= AsyncIngressCapacity
       /\ IngressDepth(recipient)
            + IngressProtectedSlotCountFor(
                asyncIngressLanes, recipient)
            <= AsyncIngressCapacity

AsyncIngressContentTypeInvariant ==
  \A recipient \in ValidatorIds:
       /\ \A source \in AsyncIngressSources:
            /\ IngressLane(recipient, source)
                 \in Seq(Range(IngressLane(recipient, source)))
            /\ DOMAIN IngressLane(recipient, source) =
                 1..IngressLaneDepth(recipient, source)
            /\ \A index \in 1..IngressLaneDepth(recipient, source):
                 /\ AsyncItemTyped(
                      IngressLane(recipient, source)[index])
                 /\ IngressLane(recipient, source)[index].envelope.recipient
                      = recipient
                 /\ IngressResourceSource(
                      IngressLane(recipient, source)[index]) = source

AsyncServeIngressReservationInvariant ==
  \A recipient \in ValidatorIds:
    \A source \in AsyncIngressSources:
      \A index \in 1..IngressLaneDepth(recipient, source):
        LET item == IngressLane(recipient, source)[index]
            identity ==
              AsyncServeLogicalRequestIdentity(recipient, item)
        IN /\ AsyncServeRequestAuthorized(item)
                => AsyncServeTransportAdmissionGateAllows(
                     recipient, item)
           /\ IF AsyncServeLifecycleAdmissionRequired(recipient, item)
              THEN \/ AsyncServeLifecycleOwned(recipient, identity)
                   \/ AsyncServeLifecycleSuperseded(recipient, item)
                   \/ AsyncServeLifecycleConflict(recipient, item)
              ELSE TRUE

AsyncIngressTypeInvariant ==
  /\ AsyncIngressTopologyTypeInvariant
  /\ AsyncIngressCapacityTypeInvariant
  /\ AsyncIngressContentTypeInvariant
  /\ AsyncServeIngressReservationInvariant

AsyncRecoveryTypeInvariant ==
  /\ asyncRecoveryPhase \in AsyncRecoveryPhases
  /\ asyncRecoveryNode \in ValidatorIds
  /\ asyncRecoveryGeneration \in Generations
  /\ AsyncQueueTyped(asyncRecoveryReplayQueue)
  /\ Len(asyncRecoveryReplayQueue) <= 2
  /\ \A candidate \in SequenceSet(asyncRecoveryReplayQueue):
       /\ candidate.class = "Completion"
       /\ candidate.kind \in {"SignProposal", "SignVote", "SignTimeout"}
       /\ candidate.node = asyncRecoveryNode
       /\ candidate.item = NoAsyncItem
       /\ CandidateConsumerCurrent(candidate)
       /\ candidate \in
            SequenceSet(RestartSignatureReplay(asyncRecoveryNode))
  /\ (asyncRecoveryPhase # "Replaying" =>
        asyncRecoveryReplayQueue = <<>>)
  /\ (asyncRecoveryPhase = "Eligible" => Responsive \subseteq up)
  /\ (asyncRecoveryPhase = "RestartRequired" =>
        /\ asyncRecoveryNode \in Responsive \cap (ValidatorIds \ up)
        /\ Responsive \ {asyncRecoveryNode} \subseteq up
        /\ NodeIdle(asyncRecoveryNode))
  /\ (asyncRecoveryPhase = "ReplayRequired" =>
        /\ asyncRecoveryNode \in Responsive \cap up
        /\ Responsive \subseteq up
        /\ generation[asyncRecoveryNode] = asyncRecoveryGeneration
        /\ NodeIdle(asyncRecoveryNode))
  /\ (asyncRecoveryPhase = "Replaying" =>
        /\ asyncRecoveryNode \in Responsive \cap up
        /\ Responsive \subseteq up
        /\ generation[asyncRecoveryNode] = asyncRecoveryGeneration
        /\ ~NodeHasApplication(asyncRecoveryNode)
        /\ RestartDecisions(asyncRecoveryNode) = {}
        /\ asyncIngressReady[asyncRecoveryNode] = <<>>
        /\ \A source \in AsyncIngressSources:
             IngressLane(asyncRecoveryNode, source) = <<>>
        /\ \A request \in asyncActiveRequests:
             \/ request.source # asyncRecoveryNode
             \/ RestartDurableCertifiedRequest(
                  asyncRecoveryNode, request)
        /\ \A candidate \in
             ResponsiveReplayScheduledCandidates(asyncRecoveryNode):
             /\ CandidateConsumerCurrent(candidate)
             /\ \/ candidate \in SequenceSet(
                       RestartSignatureReplay(asyncRecoveryNode))
                \/ RestartLockedBodyPipelineCandidate(
                     asyncRecoveryNode, candidate))
  /\ (asyncRecoveryPhase = "Recovered" =>
        Responsive \subseteq up)

AsyncHistoricalLockRestartAuthorityTypeInvariant ==
  /\ IsFiniteSet(asyncHistoricalLockRestartAuthorities)
  /\ asyncHistoricalLockRestartAuthorities
       \subseteq AsyncHistoricalLockRestartAuthoritySet
  /\ Cardinality(asyncHistoricalLockRestartAuthorities)
       <= Cardinality(ValidatorIds)

AsyncRestartAuthorityInvariant ==
  asyncRecoveryPhase
      \in {"RestartRequired", "ReplayRequired", "Replaying"} =>
    generation[asyncRecoveryNode] = asyncRecoveryGeneration

AsyncSchedulerTypeInvariant ==
  /\ AsyncRuntimeTypeInvariant
  /\ AsyncIoTypeInvariant
  /\ AsyncDeferredTypeInvariant
  /\ AsyncTransportTypeInvariant
  /\ AsyncIngressTypeInvariant
  /\ AsyncHistoricalRecoveryTypeInvariant

AsyncTypeInvariant ==
  /\ TypeInvariant
  /\ AsyncSchedulerTypeInvariant
  /\ AsyncServiceActivationPairInvariant
  /\ ReceivedTimeoutVotePoolInvariant

(***************************************************************************
Exact reachable-state refinement obligation for the fair-action inventory.
The scheduler relation is intentionally executable over arbitrary TLA+ values,
while the Core `Next` carriers are typed.  Consequently this implication is
valid only at the ordinary Core-plus-scheduler type boundary.  It remains an
explicit proof-ledger obligation until the concrete runner projection is
discharged; a structural source check is not a deductive proof.
***************************************************************************)
AsyncFairActionsRefineAsyncNext ==
  /\ TypeInvariant
  /\ AsyncSchedulerTypeInvariant
  => \A initialContext \in ContextRecords:
       AsyncFairActionAt(initialContext) => AsyncNext

AsyncCompletionReserveInvariant ==
  \A node \in ValidatorIds:
    /\ AsyncQueueDepth(node) <= AsyncQueueCapacity
    /\ (AsyncQueueDepth(node) >= AsyncNormalLimit
          => ~CanEnqueueClass(node, "Normal"))
    /\ (AsyncQueueDepth(node) >= AsyncProgressLimit
          => ~CanEnqueueClass(node, "Progress"))
    /\ (AsyncQueueDepth(node) >= AsyncOrdinaryCompletionLimit
          => ~CanEnqueueClass(node, "Completion"))

AsyncIoReservationInvariant ==
  /\ AsyncIoWorkCapacity <= AsyncCompletionReserve
  /\ \A node \in ValidatorIds:
       /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
       /\ AsyncIoEffectiveQueueDepth(node) <= AsyncIoCapacity
       /\ Cardinality(
            {index \in 1..AsyncIoQueueDepth(node):
               asyncIoQueues[node][index].class = "Serve"})
            <= AsyncIoAuxCapacity
       /\ Cardinality(
            {reservation \in asyncServeReservations:
               reservation.node = node})
            <= AsyncServeLifecycleFamilyBudget
       /\ Cardinality(
            {index \in 1..AsyncIoQueueDepth(node):
               asyncIoQueues[node][index].class # "Control"})
            <= AsyncIoAuxCapacity + AsyncIoWorkCapacity
       /\ AsyncOutstandingWorkCount(node) <= AsyncIoWorkCapacity

AsyncBoundedRetransmissionInvariant ==
  \A node \in ValidatorIds:
    /\ Cardinality(SendableItems(node))
         <= AsyncRetainedControlBudget
    /\ Cardinality(RetainedProposalChunks(node))
         <= AsyncRetainedProposalChunkBudget
    /\ Cardinality(ActiveRequestItems(node))
         <= AsyncActiveRequestBudget
    /\ Cardinality(RetryableItems(node))
         <= AsyncRetransmitEmissionBudget

FiniteViewPrefix ==
  \A node \in ValidatorIds: nodeView[node] \in Views

AsyncStepRefinesCore == AsyncNext => [Next]_vars

FiniteViewPrefixRefinement ==
  FiniteViewPrefix

AsyncAllResponsiveDecidedAt(initialContext) ==
  \A node \in AsyncVotersAt(initialContext): NodeHasDecision(node)

AsyncAllResponsiveAppliedAt(initialContext) ==
  \A node \in AsyncVotersAt(initialContext): NodeHasApplication(node)

AsyncAllResponsiveDecided ==
  AsyncAllResponsiveDecidedAt(ContextRecord(0, <<>>))

AsyncAllResponsiveApplied ==
  AsyncAllResponsiveAppliedAt(ContextRecord(0, <<>>))

AsyncHeightDecided == DualQuorum(CurrentEpoch, ConcreteDecisionNodes)
AsyncHeightApplied == DualQuorum(CurrentEpoch, ConcreteAppliedNodes)

PostGstEventuallyAsyncDecision ==
  \A node \in AsyncGenesisResponsiveVoters: gst ~> NodeHasDecision(node)

PostGstEventuallyAsyncDecisionAt(initialContext) ==
  \A node \in AsyncVotersAt(initialContext):
    gst ~> NodeHasDecision(node)

ResponsiveDecisionEventuallyApplied ==
  \A node \in AsyncGenesisResponsiveVoters:
    NodeHasDecision(node) ~> NodeHasApplication(node)

ResponsiveDecisionEventuallyAppliedAt(initialContext) ==
  \A node \in AsyncVotersAt(initialContext):
    NodeHasDecision(node) ~> NodeHasApplication(node)

PostGstEventuallyAsyncApplication ==
  \A node \in AsyncGenesisResponsiveVoters:
    gst ~> NodeHasApplication(node)

PostGstEventuallyAsyncApplicationAt(initialContext) ==
  \A node \in AsyncVotersAt(initialContext):
    gst ~> NodeHasApplication(node)

PostGstEventuallyAsyncHeightCompletion ==
  gst ~> AsyncAllResponsiveApplied

IngressSourceServiceRank(recipient, source) ==
  CHOOSE index \in 1..Len(asyncIngressReady[recipient]):
    asyncIngressReady[recipient][index] = source

BoundedTransportServiceRank(recipient, source) ==
  IF source \in SequenceSet(asyncIngressReady[recipient])
  THEN IngressSourceServiceRank(recipient, source)
  ELSE 0

SchedulerCandidateIndices(node, command) ==
  {index \in 1..Len(asyncCommandQueues[node]):
     asyncCommandQueues[node][index] = command}

SchedulerClassPrefixIndices(node, command) ==
  {index \in 1..Len(asyncCommandQueues[node]):
     AsyncCandidateLifecycleOrdinal(asyncCommandQueues[node][index])
       <= AsyncCandidateLifecycleOrdinal(command)}

CommandClassDistance(fromClass, toClass) ==
  IF fromClass = toClass
  THEN 0
  ELSE IF NextCommandClass(fromClass) = toClass THEN 1 ELSE 2

(***************************************************************************
The scheduler rank counts every physical command whose immutable lifecycle is
no later than the target lifecycle.  Later roots are excluded even when their
class owns the cursor.  Multiplication by three leaves room for the class
cursor tie-break among equal lifecycle ordinals: servicing an equal-lifecycle
sibling lowers the prefix, while selecting another class can change the cursor
distance by at most two.
***************************************************************************)
SchedulerServiceRank(node, command) ==
  3 * Cardinality(SchedulerClassPrefixIndices(node, command))
    + CommandClassDistance(asyncNextCommandClass[node], command.class)

IoServiceRank(node, job) ==
  CHOOSE index \in 1..AsyncIoQueueDepth(node):
    asyncIoQueues[node][index] = job

BoundedIoServiceRank(node, job) ==
  IF job \in SequenceSet(asyncIoQueues[node])
  THEN IoServiceRank(node, job)
  ELSE 0

RuntimeReachRank(node) ==
  CASE asyncRunnerPhase[node] = "Local" ->
         asyncRunnerBudget[node] + AsyncIngressCapacity + 2
    [] asyncRunnerPhase[node] = "Ingress" ->
         asyncRunnerBudget[node] + 1
    [] OTHER -> 0

IoNonControlCount(node) ==
  Cardinality(
    {index \in 1..AsyncIoQueueDepth(node):
       asyncIoQueues[node][index].class # "Control"})

IoServeCount(node) ==
  Cardinality(
    {index \in 1..AsyncIoQueueDepth(node):
       asyncIoQueues[node][index].class = "Serve"})

=============================================================================
