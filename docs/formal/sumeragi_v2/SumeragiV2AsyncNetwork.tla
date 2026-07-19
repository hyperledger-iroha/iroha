---- MODULE SumeragiV2AsyncNetwork ----
EXTENDS SumeragiV2Inductive, Sequences, FiniteSets, Naturals, Functions

(***************************************************************************
Production-coupled asynchronous execution for Sumeragi v2.

There is one reducer scheduler per validator.  The validators are independent
processes: post-GST weak fairness is attached to each responsive validator's
run-loop action, never to a favorable global interleaving and never to an
individual protocol command.  Each runtime owns the same single bounded queue
as `BoundedIngress`; normal, progress, and completion are admission classes
over its total length.  Dispatch follows the production cyclic class cursor
(Completion -> Progress -> Normal), removes the first queued command of the
selected class, and therefore preserves FIFO order within each class.
TimeoutElapsed and RetransmitElapsed are local scheduler inputs.  If the
reducer is busy they are coalesced in the adapter's trusted deferred-completion
set until the busy fence clears.

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
model of `FairV2Ingress`.  Every recipient has one lane for each frozen-roster
validator plus one aggregate untrusted lane.  Admission is bounded by one
total capacity while preserving the empty-lane, Progress, TimeoutVote, shared
TransportCompletion, and post-service continuation potential; non-empty lanes
are serviced by the exact ready-queue rotation used in Rust.  `Chunk` and
`CertifiedResponse` share one TransportCompletion owner per authenticated
resource hop.  A non-roster hop may carry a completion whose semantic origin is
in the roster, so the aggregate untrusted lane owns its own completion slot and
a separate generic continuation without borrowing any validator's owners.  A
source may borrow idle message capacity but cannot consume another source's
reservations.  Each validator source also isolates the fixed valid-timeout-vote
byte reserve from all other wire traffic.
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

`AsyncNetworkItem.source` is the authenticated resource-owning hop in this
direct-origin abstraction.  Production additionally carries a possibly
different semantic origin for validation, response routing, and exact-wire
coalescing.  Establishing that relay-controlled origin churn cannot multiply
source count/byte ownership, while distinct legitimate origins are not
incorrectly coalesced, is an unassigned production-refinement proposition.

An emitted Core envelope remains in immutable authentication history when a
hidden packet is lost before GST.  Retransmission scans only the reducer's
bounded per-class retained controls and active certified-body requests.  Packet
publication is atomic here; production's actor admission, encode, frame, batch,
write, and flush stages and its remaining broadcast cursor must refine that
single action while retaining the exact occurrence until the matching flush
acknowledgement.  This is another unassigned production-refinement proposition,
not a consequence of the abstract packet fairness actions.

Post-GST historical catch-up is also exact scheduler ownership.  A responsive
validator with no local Decision may be opened as one explicit recovery target
only when a current responsive server already holds the applied Commit receipt.
The target then uses its own fair runner, certificate discovery, I/O worker,
and bidirectional bounded packet corridor; the exact Apply command retires that
ownership atomically.  Observer recovery therefore does not broaden any normal
current-voter consensus action.

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
     "PersistTimeout", "SignTimeout", "FormTC", "BeginInstallTC",
   "PersistInstallTC", "RequestCertifiedBody", "FetchCertifiedBody", "Apply"}
AsyncWorkKinds == AsyncCompletionTags \cup AsyncDeliveryKinds \cup AsyncReducerKinds
AsyncCommandClasses == {"Normal", "Progress", "Completion"}
AsyncIoCommandClasses == {"Serve", "Consensus", "Control"}
AsyncControlKinds ==
  {"Proposal", "PrepareVote", "CommitVote", "PrepareQC", "CommitQC",
   "TimeoutVote", "TimeoutCertificate"}
AsyncInstallRetainedControlKinds ==
  {"CommitVote", "CommitQC", "TimeoutCertificate"}

NoAsyncChunk == 0
AsyncChunks == 1..AsyncChunkCount
AsyncHeartbeatSubject == CHOOSE subject \in ValidSubjects: TRUE
AsyncUntrustedSource == N
AsyncIngressSources == ValidatorIds \cup {AsyncUntrustedSource}

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

AsyncNetworkItem(kind, source, envelope) ==
  [kind |-> kind, source |-> source, envelope |-> envelope]

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
  \cup {AsyncNetworkItem(kind, source, envelope):
          kind \in {"CertifiedRequest", "CommitCertificateRequest",
                    "NormalJunk", "ProgressJunk"},
          source \in ValidatorIds, envelope \in AsyncBodyEnvelopeSet}
  \cup {AsyncNetworkItem(kind, source, envelope):
          kind \in {"Chunk", "CertifiedResponse"},
          source \in AsyncIngressSources,
          envelope \in AsyncBodyEnvelopeSet}
  \cup {AsyncNetworkItem("CommitCertificateResponse", source, envelope):
          source \in ValidatorIds, envelope \in QcEnvelopeSet}
  \cup {AsyncNetworkItem("Noise", source, envelope):
          source \in AsyncIngressSources, envelope \in AsyncBodyEnvelopeSet}

\* Exact evidence survives queue/pool epochs independently of the delivery
\* envelope.  Durable restart replay therefore names the authenticated Core
\* record which caused the work, while ordinary ingress keeps the exact wire
\* item as both its payload and evidence.
AsyncEvidenceSet ==
  AsyncNetworkItems \cup {NoAsyncItem}
    \cup ProposalRecordSet \cup VoteRecordSet \cup TimeoutVoteRecordSet
    \cup QcRecordSet \cup TcRecordSet \cup BodyRecordSet

AsyncCandidateWithIdentity(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    consumerContext, consumerView, consumerGeneration, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity) ==
  [class |-> commandClass, kind |-> kind, node |-> node,
   height |-> blockHeight, view |-> roundView, subject |-> subject,
   item |-> item, consumerContext |-> consumerContext,
   consumerView |-> consumerView,
   consumerGeneration |-> consumerGeneration,
   evidence |-> evidence, bodyIdentity |-> bodyIdentity,
   manifestIdentity |-> manifestIdentity,
   commitmentIdentity |-> commitmentIdentity]

AsyncCandidate(commandClass, kind, node, blockHeight, roundView, subject,
               item) ==
  AsyncCandidateWithIdentity(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    context, nodeView[node], generation[node], item,
    subject, subject, subject)

AsyncCandidateFrom(commandClass, kind, command) ==
  AsyncCandidateWithIdentity(
    commandClass, kind, command.node, context.height, command.view,
    command.subject, NoAsyncItem,
    command.consumerContext, command.consumerView,
    command.consumerGeneration, command.evidence,
    command.bodyIdentity, command.manifestIdentity,
    command.commitmentIdentity)

AsyncCandidateAtConsumer(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    consumerView, consumerGeneration, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity) ==
  AsyncCandidateWithIdentity(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    context, consumerView, consumerGeneration, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity)

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
   commitmentIdentity: SubjectOrNone]

AsyncCandidateDomain ==
  {"class", "kind", "node", "height", "view", "subject", "item",
   "consumerContext", "consumerView", "consumerGeneration",
   "evidence", "bodyIdentity", "manifestIdentity", "commitmentIdentity"}

NoAsyncCandidate ==
  AsyncCandidateWithIdentity(
    "Normal", "AssembleBody", 0, 0, 0,
    AsyncHeartbeatSubject, NoAsyncItem, context, 0, 0, NoAsyncItem,
    AsyncHeartbeatSubject, AsyncHeartbeatSubject, AsyncHeartbeatSubject)

AsyncIoCapacity == AsyncIoAuxCapacity + AsyncIoWorkCapacity + 1

AsyncIoJob(commandClass, candidate, nonce) ==
  [class |-> commandClass, candidate |-> candidate, nonce |-> nonce]

AsyncIoConsensusJob(candidate) == AsyncIoJob("Consensus", candidate, 0)
AsyncIoControlJob == AsyncIoJob("Control", NoAsyncCandidate, 0)

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
  /\ DOMAIN tc = {"context", "height", "view", "votes"}
  /\ tc.context \in ContextRecords
  /\ tc.height \in Heights
  /\ tc.view \in Views
  /\ tc.votes \subseteq TimeoutVoteRecordSet

AsyncTcEnvelopeTyped(envelope) ==
  /\ DOMAIN envelope = {"recipient", "tc"}
  /\ envelope.recipient \in ValidatorIds
  /\ AsyncTcRecordTyped(envelope.tc)

AsyncItemTyped(item) ==
  /\ DOMAIN item = {"kind", "source", "envelope"}
  /\ item.kind \in AsyncNetworkKinds
  /\ item.source \in AsyncIngressSources
  /\ (item.kind \notin {"Noise", "Chunk", "CertifiedResponse"}
        => item.source \in ValidatorIds)
  /\ item.envelope.recipient \in ValidatorIds
  /\ CASE item.kind = "Proposal" -> item.envelope \in ProposalEnvelopeSet
       [] item.kind \in {"PrepareVote", "CommitVote"} ->
            item.envelope \in VoteEnvelopeSet
       [] item.kind \in {"PrepareQC", "CommitQC"} ->
            item.envelope \in QcEnvelopeSet
       [] item.kind = "TimeoutVote" -> item.envelope \in TimeoutEnvelopeSet
       [] item.kind = "TimeoutCertificate" ->
            AsyncTcEnvelopeTyped(item.envelope)
       [] item.kind = "CommitCertificateResponse" ->
            item.envelope \in QcEnvelopeSet
       [] OTHER -> AsyncBodyEnvelopeTyped(item.envelope)

AsyncEvidenceTyped(evidence) ==
  \/ evidence = NoAsyncItem
  \/ AsyncItemTyped(evidence)
  \/ evidence \in ProposalRecordSet
  \/ evidence \in VoteRecordSet
  \/ evidence \in TimeoutVoteRecordSet
  \/ evidence \in QcRecordSet
  \/ AsyncTcRecordTyped(evidence)
  \/ evidence \in BodyRecordSet

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
  asyncOutstandingWork,
  asyncIoReadyCompletions,
  asyncLocalReadyCompletions,
  asyncNextCompletionSource,
  asyncIoControlAvailable,
  asyncDeferredCompletionQueues,
  asyncDeferredProgressQueues,
  asyncDeferredNormalQueues,
  asyncNextDeferredClass,
  asyncDeferredDrainOwed,
  asyncCausalQueues,
  asyncOutstandingTags,
  asyncNodeDeadlines,
  asyncRetransmitDeadlines,
  asyncNodeServiceDeadlines,
  asyncIoServiceDeadlines,
  asyncSentItems, asyncRetainedControl, asyncActiveRequests,
  asyncTransport,
  asyncIngressLanes,
  asyncIngressReady,
  asyncHeldChunks,
  asyncHistoricalRecoveryTargets,
  asyncRecoveryPhase,
  asyncRecoveryNode,
  asyncRecoveryGeneration,
  asyncRecoveryReplayQueue

AsyncSchedulerVars ==
  <<asyncNow, asyncCommandQueues, asyncNextCommandClass,
    asyncFifoOwed, asyncTimeoutEmitted,
    asyncRunnerPhase, asyncRunnerBudget,
    asyncCausalAdmissionOwed, asyncNextLocalSource, asyncIoQueues,
    asyncOutstandingWork, asyncIoReadyCompletions,
    asyncLocalReadyCompletions, asyncNextCompletionSource,
    asyncIoControlAvailable, asyncDeferredCompletionQueues,
    asyncDeferredProgressQueues, asyncDeferredNormalQueues,
    asyncNextDeferredClass, asyncDeferredDrainOwed,
    asyncCausalQueues, asyncOutstandingTags,
    asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
    asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
    asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
    asyncHistoricalRecoveryTargets>>

AsyncSchedulerExceptHistoricalRecoveryTargets ==
  <<asyncNow, asyncCommandQueues, asyncNextCommandClass,
    asyncFifoOwed, asyncTimeoutEmitted,
    asyncRunnerPhase, asyncRunnerBudget,
    asyncCausalAdmissionOwed, asyncNextLocalSource, asyncIoQueues,
    asyncOutstandingWork, asyncIoReadyCompletions,
    asyncLocalReadyCompletions, asyncNextCompletionSource,
    asyncIoControlAvailable, asyncDeferredCompletionQueues,
    asyncDeferredProgressQueues, asyncDeferredNormalQueues,
    asyncNextDeferredClass, asyncDeferredDrainOwed,
    asyncCausalQueues, asyncOutstandingTags,
    asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
    asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
    asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>

AsyncRecoveryLifecycleVars ==
  <<asyncRecoveryPhase, asyncRecoveryNode, asyncRecoveryGeneration>>

AsyncRecoveryVars ==
  <<asyncRecoveryPhase, asyncRecoveryNode, asyncRecoveryGeneration,
    asyncRecoveryReplayQueue>>

AsyncAllVars == <<vars, AsyncSchedulerVars, AsyncRecoveryVars>>

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
  /\ UNCHANGED AsyncRecoveryVars
  /\ AsyncCoreOuterFrame

AsyncNonRunnerOuterFrame ==
  /\ UNCHANGED asyncNodeServiceDeadlines
  /\ AsyncNonCrashOuterFrame

AsyncRecoveryOuterFrame ==
  /\ UNCHANGED up
  /\ AsyncCoreOuterFrame

AsyncIoVars ==
  <<asyncIoQueues, asyncOutstandingWork, asyncIoReadyCompletions,
    asyncLocalReadyCompletions, asyncNextCompletionSource,
    asyncIoControlAvailable>>

AsyncDeferredVars ==
  <<asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncNextDeferredClass,
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

HistoricalRecoveryTarget(node) ==
  node \in asyncHistoricalRecoveryTargets

AsyncGenesisResponsiveVoters ==
  AsyncVotersAt(ContextRecord(0, <<>>))

AsyncNormalLimit ==
  AsyncQueueCapacity - AsyncProgressReserve - AsyncCompletionReserve
AsyncProgressLimit == AsyncQueueCapacity - AsyncCompletionReserve

AsyncQueueDepth(node) == Len(asyncCommandQueues[node])

AsyncIoQueueDepth(node) == Len(asyncIoQueues[node])

AsyncIoAdmissionLimit(commandClass) ==
  CASE commandClass = "Serve" -> AsyncIoAuxCapacity
    [] commandClass = "Consensus" ->
         AsyncIoAuxCapacity + AsyncIoWorkCapacity
    [] commandClass = "Control" -> AsyncIoCapacity

CanEnqueueIoClass(node, commandClass) ==
  AsyncIoQueueDepth(node) < AsyncIoAdmissionLimit(commandClass)

AsyncIoServeIndices(queue) ==
  {index \in 1..Len(queue): queue[index].class = "Serve"}

AsyncIoServeNonces(node) ==
  {asyncIoQueues[node][index].nonce:
     index \in AsyncIoServeIndices(asyncIoQueues[node])}

FreshAsyncIoServeNonce(node) ==
  CHOOSE nonce \in 0..AsyncIoAuxCapacity:
    nonce \notin AsyncIoServeNonces(node)

AsyncIoCertifiedServeJob(node, candidate) ==
  AsyncIoJob("Serve", candidate, FreshAsyncIoServeNonce(node))

AsyncIoServeNonceOwnership(queue) ==
  \A left, right \in AsyncIoServeIndices(queue):
    queue[left].nonce = queue[right].nonce => left = right

AsyncIoSequenceTyped(queue) ==
  /\ queue \in Seq(Range(queue))
  /\ DOMAIN queue = 1..Len(queue)
  /\ \A index \in 1..Len(queue): AsyncIoJobTyped(queue[index])

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

CandidateScheduled(candidate) ==
  candidate \in QueuedCandidates \cup DeferredCandidates \cup CausalCandidates
    \cup TrackedWorkCandidates

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
  IN /\ asyncCommandQueues' =
          [asyncCommandQueues EXCEPT ![node] = Append(@, candidate)]
     /\ UNCHANGED asyncNextCommandClass

NodeQueueNonempty(node) == Len(asyncCommandQueues[node]) > 0

(***************************************************************************
The production bounded ingress scans Completion, Progress, and Normal from a
per-validator cyclic cursor, removes the first command of the selected class,
and advances the cursor to the following class.  `admitted_at` and
`eligible_skips` are snapshot diagnostics only: neither participates in
admission or selection, so the refinement deliberately abstracts them away.
***************************************************************************)
NextCommandClass(commandClass) ==
  CASE commandClass = "Completion" -> "Progress"
    [] commandClass = "Progress" -> "Normal"
    [] OTHER -> "Completion"

CommandClassIndices(node, commandClass) ==
  {index \in 1..Len(asyncCommandQueues[node]):
     asyncCommandQueues[node][index].class = commandClass}

FirstCommandClassIndex(node, commandClass) ==
  CHOOSE index \in CommandClassIndices(node, commandClass):
    \A other \in CommandClassIndices(node, commandClass): index <= other

SelectedCommandClass(node) ==
  LET first == asyncNextCommandClass[node]
      second == NextCommandClass(first)
      third == NextCommandClass(second)
  IN IF CommandClassIndices(node, first) # {}
     THEN first
     ELSE IF CommandClassIndices(node, second) # {}
          THEN second
          ELSE third

NextNodeCommandIndex(node) ==
  FirstCommandClassIndex(node, SelectedCommandClass(node))

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

NodeHasApplication(node) ==
  \E application \in applied:
    /\ application.node = node
    /\ application.qc.context = context
    /\ application.qc.phase = "Commit"

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

NoItemCandidate(commandClass, kind, node, roundView, subject) ==
  AsyncCandidate(commandClass, kind, node, context.height, roundView,
                 subject, NoAsyncItem)

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

RetainedBodyRebindCandidate(command) ==
  CausalCandidate("Completion", "RebindRetainedBody", command)

InstallRequests(command) ==
  {installRequest \in pendingInstallTC:
    /\ command.node = installRequest.node
    /\ command.view = installRequest.tc.view}

InstallCommitSignRequests(command) ==
  {signRequest \in VoteSignSet:
    \E installRequest \in InstallRequests(command):
      signRequest \in
        ActiveLockedCommitSignRequestsAfterInstall(
          installRequest.node, installRequest.tc)}

InstallCommitSignSuccessor(command) ==
  LET signRequest ==
        CHOOSE request \in InstallCommitSignRequests(command): TRUE
  IN AsyncCandidateAtConsumer(
       "Completion", "SignVote", signRequest.node,
       signRequest.vote.context.height, signRequest.vote.view,
       signRequest.vote.subject, NoAsyncItem, command.view + 1,
       IF generation[signRequest.node] < MaxGeneration
       THEN generation[signRequest.node] + 1
       ELSE generation[signRequest.node],
       signRequest.vote, signRequest.vote.subject,
       signRequest.vote.subject, signRequest.vote.subject)

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
     ELSE LET request == CHOOSE entry \in requests: TRUE
              selectedRank == TcHighRank(request.tc)
          IN IF selectedRank > highestRank[command.node]
             THEN TcHighSubject(request.tc)
             ELSE AsyncProposalSubject(command.node)

InstallProposalSuccessor(command) ==
  LET subject == InstallProposalSubject(command)
  IN AsyncCandidateAtConsumer(
       "Normal", "AssembleBody", command.node, context.height,
       command.view + 1, subject, NoAsyncItem, command.view + 1,
       IF generation[command.node] < MaxGeneration
       THEN generation[command.node] + 1
       ELSE generation[command.node],
       command.evidence, subject, subject, subject)

(***************************************************************************
The reducer exposes the exact active locked Commit re-sign as the first
causal completion after TC acknowledgement.  The ordinary next-view proposal
path follows it.  When the TC promoted a lock that has no matching local
Commit intent, there is no synthetic signing successor at installation: the
exact historical body must pass StoreBody -> ValidateBody, whose successor
then enters the ordinary WAL-backed BeginLockCommit path.
***************************************************************************)
InstallCommandSuccessors(command) ==
  IF InstallCommitSignRequests(command) = {}
  THEN <<InstallProposalSuccessor(command)>>
  ELSE <<InstallCommitSignSuccessor(command),
         InstallProposalSuccessor(command)>>

DecisionFetchFrontier(command) ==
  /\ command.kind = "FetchBody"
  /\ ExactDecidedLocalBody(command.node, command.view, command.subject)

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
   "PersistTimeout", "DeliverTimeout", "FormTC", "DeliverTC",
   "BeginInstallTC", "PersistInstallTC"}

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
         IF DecisionFetchFrontier(command)
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
           CausalCandidate("Progress", "BeginLockCommit", command),
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
         THEN <<CausalCandidate("Progress", "BeginObservePrepare", command),
                CausalCandidate("Progress", "BeginLockCommit", command)>>
         ELSE <<CausalCandidate("Progress", "BeginDecision", command)>>
    [] command.kind = "BeginObservePrepare" ->
         <<CausalCandidate("Completion", "PersistObservePrepare", command)>>
    [] command.kind = "PersistObservePrepare" ->
         <<CausalCandidate("Progress", "BeginLockCommit", command)>>
    [] command.kind = "BeginLockCommit" ->
         <<CausalCandidate("Completion", "PersistLockCommit", command)>>
    [] command.kind = "PersistLockCommit" ->
         <<CausalCandidate("Completion", "SignVote", command)>>
    [] command.kind = "FormCommitQC" ->
         <<CausalCandidate("Completion", "PersistDecision", command)>>
    [] command.kind = "BeginDecision" ->
         <<CausalCandidate("Completion", "PersistDecision", command)>>
    [] command.kind = "PersistDecision" ->
         <<CausalCandidate("Completion", "FetchBody", command)>>
    [] command.kind = "BeginTimeout" ->
         <<CausalCandidate("Completion", "PersistTimeout", command)>>
    [] command.kind = "PersistTimeout" ->
         <<CausalCandidate("Completion", "SignTimeout", command)>>
    \* Production consumes authenticated timeout traffic after Decision but
    \* returns AlreadyDecided before admitting it.  Core removes the exact
    \* network envelope without changing its receive pool, and the adapter
    \* must therefore emit no FormTC/BeginInstallTC continuation.
    [] command.kind = "DeliverTimeout" ->
         IF NoDecisionForNode(command.node)
         THEN <<CausalCandidate("Progress", "FormTC", command)>>
         ELSE <<>>
    [] command.kind = "FormTC" ->
         <<CausalCandidate("Completion", "PersistInstallTC", command)>>
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
Reducer effects preserve their declared order, but an exact successor which
already has any scheduler owner is coalesced.  This is the causal equivalent
of authenticated-ingress duplicate admission.  In particular, a replayed
Chunk cannot append a second FetchBody behind an already-owned FetchBody and
thereby keep replacing the value at the same service rank forever.
***************************************************************************)
FreshCandidateSequence(candidate) ==
  IF CandidateScheduled(candidate) THEN <<>> ELSE <<candidate>>

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
matching authenticated response is admitted and are retried beside, but never
stored in, the control map.  Chunks, responses, and adversarial junk are
one-shot authenticated emissions.
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

CertifiedRequestOutbox(node, qc) ==
  {AsyncNetworkItem(
     "CertifiedRequest", node,
     AsyncBodyEnvelope(source, context.height, qc.view, qc.subject,
                       NoAsyncChunk, 0)):
       source \in qc.signers \ {node}}

CommitCertificateRequestOutbox(node) ==
  {AsyncNetworkItem(
     "CommitCertificateRequest", node,
     AsyncBodyEnvelope(server, context.height, nodeView[node],
                       AsyncHeartbeatSubject, NoAsyncChunk, 0)):
       server \in CurrentVoters \ {node}}

CertifiedResponseItem(request) ==
  AsyncNetworkItem(
    "CertifiedResponse", request.envelope.recipient,
    AsyncBodyEnvelope(request.source, request.envelope.height,
                      request.envelope.view, request.envelope.subject,
                      NoAsyncChunk, request.envelope.nonce))

CommitCertificateResponseItem(request, qc) ==
  AsyncNetworkItem(
    "CommitCertificateResponse", request.envelope.recipient,
    QcEnvelope(request.source, qc))

(***************************************************************************
Production serves a certified request from the retained canonical body held
by the addressed Commit-QC signer.  The validation cache is consumer-local
pipeline state, not serving authority; requiring it here would suppress a
response that `serve_certified_body` emits from the durable body store.
***************************************************************************)
CertifiedServeCanRespond(request) ==
  /\ request.kind = "CertifiedRequest"
  /\ BodyHeldBy(durableBodies, request.envelope.recipient, context,
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
TimeoutVote, and shared TransportCompletion reservations; the authenticated
reducer still decides whether a Commit vote is the exact locked-round
reconstruction witness.  Body and certificate recovery requests remain
Progress.  `Chunk` and `CertifiedResponse` instead share the one completion
owner whose consumption makes either recovery response structurally
admissible behind generic pressure.
*)
IngressTransportCompletionKinds == {"Chunk", "CertifiedResponse"}

IngressProgressKinds ==
  {"CommitVote", "PrepareQC", "CommitQC", "TimeoutVote",
   "TimeoutCertificate", "CertifiedRequest", "CommitCertificateRequest",
   "CommitCertificateResponse"}

IngressAdmissionClass(item) ==
  IF item.kind \in IngressTransportCompletionKinds
  THEN "TransportCompletion"
  ELSE IF item.kind \in IngressProgressKinds THEN "Progress" ELSE "Auxiliary"

IngressLaneHasNonTimeoutProgressIn(lanes, recipient, source) ==
  \E queued \in SequenceSet(lanes[recipient][source]):
    /\ IngressAdmissionClass(queued) = "Progress"
    /\ queued.kind # "TimeoutVote"

IngressLaneHasTimeoutVoteIn(lanes, recipient, source) ==
  \E queued \in SequenceSet(lanes[recipient][source]):
    queued.kind = "TimeoutVote"

IngressLaneHasTransportCompletionIn(lanes, recipient, source) ==
  \E queued \in SequenceSet(lanes[recipient][source]):
    IngressAdmissionClass(queued) = "TransportCompletion"

AsyncTimeoutVoteByteGateAllows(item) ==
  \/ item.kind # "TimeoutVote"
  \/ item.source \notin ValidatorIds
  \/ /\ AsyncValidTimeoutVoteWireByteBound <= AsyncTimeoutVoteByteReserve
     /\ ~IngressLaneHasTimeoutVoteIn(asyncIngressLanes,
                                      item.envelope.recipient, item.source)

AsyncTransportCompletionOwnerGateAllows(item) ==
  \/ IngressAdmissionClass(item) # "TransportCompletion"
  \/ ~IngressLaneHasTransportCompletionIn(
       asyncIngressLanes, item.envelope.recipient, item.source)

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
     ![item.envelope.recipient][item.source] = Append(@, item)]

IngressProtectedSourcesAfterAdmission(item) ==
  IngressProtectedSourcesFor(
    IngressLanesAfterAdmission(item), item.envelope.recipient)

IngressProtectedSlotCountAfterAdmission(item) ==
  IngressProtectedSlotCountFor(IngressLanesAfterAdmission(item),
                               item.envelope.recipient)

IngressUsableCapacityAfterAdmission(item) ==
  AsyncIngressCapacity
    - IngressProtectedSlotCountAfterAdmission(item)

CanAdmitIngressItem(item) ==
  /\ IngressDepth(item.envelope.recipient)
       < IngressUsableCapacityAfterAdmission(item)
  /\ AsyncTimeoutVoteByteGateAllows(item)
  /\ AsyncTransportCompletionOwnerGateAllows(item)

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

PacketsForItems(items) ==
  {AsyncPacket(item, asyncNow, asyncNow + AsyncDeliveryBound):
     item \in items}

PublishControlItems(items) ==
  /\ items \subseteq {item \in AsyncNetworkItems:
                        item.kind \in AsyncControlKinds}
  /\ asyncRetainedControl' =
       RememberedControl(asyncRetainedControl, items)
  /\ asyncSentItems' = asyncSentItems \cup items
  /\ asyncTransport' = asyncTransport \cup PacketsForItems(items)
  /\ UNCHANGED asyncActiveRequests

PublishEphemeralItems(items) ==
  /\ asyncSentItems' = asyncSentItems \cup items
  /\ asyncTransport' = asyncTransport \cup PacketsForItems(items)
  /\ UNCHANGED <<asyncRetainedControl, asyncActiveRequests>>

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
  /\ UNCHANGED asyncActiveRequests

PublishCertifiedRequests(items) ==
  /\ \A item \in items: item.kind = "CertifiedRequest"
  /\ asyncActiveRequests' = asyncActiveRequests \cup items
  /\ asyncSentItems' = asyncSentItems \cup items
  /\ asyncTransport' = asyncTransport \cup PacketsForItems(items)
  /\ UNCHANGED asyncRetainedControl

PublishCommitCertificateRequests(items) ==
  /\ \A item \in items: item.kind = "CommitCertificateRequest"
  /\ asyncActiveRequests' = asyncActiveRequests \cup items
  /\ asyncSentItems' = asyncSentItems \cup items
  /\ asyncTransport' = asyncTransport \cup PacketsForItems(items)
  /\ UNCHANGED asyncRetainedControl

PersistInstalledControl(node, items, broadcast) ==
  /\ asyncRetainedControl' =
       InstalledControl(asyncRetainedControl, node, items)
  /\ asyncSentItems' =
       IF broadcast THEN asyncSentItems \cup items ELSE asyncSentItems
  /\ asyncTransport' =
       IF broadcast
       THEN asyncTransport \cup PacketsForItems(items)
       ELSE asyncTransport
  /\ UNCHANGED asyncActiveRequests

PersistDecisionControl(items, broadcast) ==
  /\ asyncRetainedControl' =
       RememberedControl(asyncRetainedControl, items)
  /\ asyncSentItems' =
       IF broadcast THEN asyncSentItems \cup items ELSE asyncSentItems
  /\ asyncTransport' =
       IF broadcast
       THEN asyncTransport \cup PacketsForItems(items)
       ELSE asyncTransport
  /\ UNCHANGED asyncActiveRequests

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
  /\ node \in Responsive \cap up
  /\ ~NodeHasDecision(node)
  /\ ~NodeHasApplication(node)
  /\ \E server \in (AsyncCurrentResponsiveVoters \cap up) \ {node}:
       NodeHasApplication(server)

OpenHistoricalRecovery(node) ==
  /\ gst
  /\ HistoricalRecoverySourceReady(node)
  /\ ~HistoricalRecoveryTarget(node)
  /\ asyncHistoricalRecoveryTargets' =
       asyncHistoricalRecoveryTargets \cup {node}
  /\ UNCHANGED <<vars, AsyncSchedulerExceptHistoricalRecoveryTargets,
                 AsyncRecoveryVars>>

TimeoutDue(node) ==
  /\ node \in AsyncCurrentResponsiveVoters
  /\ ~ResponsiveReplayQuarantined(node)
  /\ asyncNow >= asyncNodeDeadlines[node]
  /\ ~NodeHasDecision(node)
  /\ ~NodeTimedOut(node, nodeView[node])
  /\ ~asyncTimeoutEmitted[node]
  /\ TimeoutTagPresent(node)

RetransmitTagPresent(node) ==
  "RetransmitElapsed" \notin asyncOutstandingTags[node]

RetransmitDue(node) ==
  /\ ~ResponsiveReplayQuarantined(node)
  /\ asyncNow >= asyncRetransmitDeadlines[node]
  /\ RetransmitTagPresent(node)
  /\ ~TimeoutDue(node)


(***************************************************************************
Exact Core reducer command execution.
***************************************************************************)

CommandMatches(command, node, roundView, subject) ==
  /\ command.node = node
  /\ command.height = context.height
  /\ command.view = roundView
  /\ command.subject = subject

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
     /\ ~DecisionFetchFrontier(command)
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
  \/ /\ command.kind = "FormTC"
     /\ FormTC(command.node, command.view)
  \/ /\ command.kind = "BeginInstallTC"
     /\ \E tc \in ReceivedTcValues:
          /\ command.node = command.node
          /\ command.view = tc.view
          /\ BeginInstallTC(command.node, tc)
  \/ /\ command.kind = "FetchCertifiedBody"
     /\ command.item.kind = "CertifiedResponse"
     /\ command.item.envelope.recipient = command.node
     /\ command.item.envelope.view = command.view
     /\ command.item.envelope.subject = command.subject
     /\ \E qc \in DecisionQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ command.item.source \in qc.signers
          /\ FetchCertifiedBody(command.node, qc)

AsyncAuxVars ==
  <<asyncOutstandingTags, asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport, asyncIngressLanes, asyncIngressReady,
    asyncHeldChunks, asyncHistoricalRecoveryTargets
    >>

ExecuteRegularCommand(command) ==
  /\ RegularCoreCommand(command)
  /\ UNCHANGED AsyncAuxVars

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
       /\ PersistInstallTC(request)
       /\ PersistInstalledControl(
            request.node, TcOutbox(request.node, request.tc),
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
            QcOutbox(request.node, request.qc), request.rebroadcast)
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady,
                 asyncHeldChunks, asyncHistoricalRecoveryTargets>>

ExecuteRequestCertifiedBody(command) ==
  /\ command.kind = "RequestCertifiedBody"
  /\ ~BodyHeldBy(durableBodies, command.node, context, command.view,
                  command.subject)
  /\ \E decision \in decisions:
       /\ decision.node = command.node
       /\ decision.qc.context = context
       /\ decision.qc.view = command.view
       /\ decision.qc.subject = command.subject
       /\ decision.qc.phase = "Commit"
       /\ UNCHANGED vars
       /\ PublishCertifiedRequests(
            CertifiedRequestOutbox(command.node, decision.qc))
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

(***************************************************************************
The reducer owns one Decision `FetchBody` frontier.  The adapter resolves it
from the reopened durable catalog when possible; otherwise the same frontier
opens the certified request lifecycle.  Later Store/Validate/Apply work is
emitted only by the resulting body state transitions.
***************************************************************************)
ExecuteDecisionFetch(command) ==
  /\ DecisionFetchFrontier(command)
  /\ IF BodyHeldBy(durableBodies, command.node, context, command.view,
                    command.subject)
     THEN /\ UNCHANGED vars
          /\ UNCHANGED <<asyncSentItems, asyncRetainedControl,
                          asyncActiveRequests, asyncTransport>>
     ELSE \E decision \in decisions:
            /\ decision.node = command.node
            /\ decision.qc.context = context
            /\ decision.qc.view = command.view
            /\ decision.qc.subject = command.subject
            /\ decision.qc.phase = "Commit"
            /\ UNCHANGED vars
            /\ PublishCertifiedRequests(
                 CertifiedRequestOutbox(command.node, decision.qc))
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
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
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
          IF item.kind = "PrepareQC"
          THEN RememberedControl(
                 asyncRetainedControl,
                 QcOutbox(command.node, item.envelope.qc))
          ELSE asyncRetainedControl
     /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines, asyncSentItems,
                    asyncActiveRequests, asyncTransport,
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
                    asyncActiveRequests>>
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
                    asyncActiveRequests>>
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

HeadCausalCandidate(node) == Head(asyncCausalQueues[node])

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
                 asyncActiveRequests>>
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
CommandExecutionEnabled(command) ==
  \E selectedCommand \in {command}:
    \/ ENABLED ExecuteRegularCommand(selectedCommand)
    \/ ENABLED ExecuteDecisionFetch(selectedCommand)
    \/ ENABLED ExecuteSignProposal(selectedCommand)
    \/ ENABLED ExecuteSignVote(selectedCommand)
    \/ ENABLED ExecuteFormPrepareQC(selectedCommand)
    \/ ENABLED ExecuteSignTimeout(selectedCommand)
    \/ ENABLED ExecutePersistInstall(selectedCommand)
    \/ ENABLED ExecutePersistDecision(selectedCommand)
    \/ ENABLED ExecuteRequestCertifiedBody(selectedCommand)
    \/ ENABLED ExecuteApply(selectedCommand)
    \/ ENABLED ExecuteCoreDelivery(selectedCommand)
    \/ ENABLED ExecuteChunkDelivery(selectedCommand)
    \/ ENABLED ExecuteRejectAuthenticatedJunk(selectedCommand)

(***************************************************************************
Every scheduler caller obtains the command from an AsyncCandidateTyped queue.
Keep that structural type guard direct: membership in the equivalent finite
Cartesian carrier forces TLC to enumerate millions of irrelevant records.
***************************************************************************)
CommandDispatchable(command) ==
  /\ AsyncCandidateTyped(command)
  /\ CandidateConsumerCurrent(command)
  /\ CommandExecutionEnabled(command)
  /\ (NodeIdle(command.node) \/ command.class = "Completion")

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

DeferCommand(command) ==
  LET node == command.node
  IN /\ UNCHANGED vars
     /\ asyncDeferredCompletionQueues' =
          IF command.class = "Completion"
          THEN [asyncDeferredCompletionQueues EXCEPT
                  ![node] = IF command \in SequenceSet(@)
                           THEN @ ELSE Append(@, command)]
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
     /\ UNCHANGED <<asyncNextDeferredClass, asyncDeferredDrainOwed,
                    asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport, asyncIngressLanes,
                    asyncIngressReady, asyncHeldChunks,
                    asyncHistoricalRecoveryTargets
                    >>

DeferredQueueNonempty(node) ==
  Len(asyncDeferredCompletionQueues[node]) > 0
    \/ Len(asyncDeferredProgressQueues[node]) > 0
    \/ Len(asyncDeferredNormalQueues[node]) > 0

(***************************************************************************
The adapter's deferred reducer inputs use the same three-class cyclic scan as
the runtime command queue, but keep an independent cursor.  A selected class
always advances the cursor, including the Busy case where production pushes
the selected input back to the front of its class queue.
***************************************************************************)
DeferredClassQueue(node, commandClass) ==
  CASE commandClass = "Completion" -> asyncDeferredCompletionQueues[node]
    [] commandClass = "Progress" -> asyncDeferredProgressQueues[node]
    [] OTHER -> asyncDeferredNormalQueues[node]

DeferredClassNonempty(node, commandClass) ==
  Len(DeferredClassQueue(node, commandClass)) > 0

SelectedDeferredClass(node) ==
  LET first == asyncNextDeferredClass[node]
      second == NextCommandClass(first)
      third == NextCommandClass(second)
  IN IF DeferredClassNonempty(node, first)
     THEN first
     ELSE IF DeferredClassNonempty(node, second)
          THEN second
          ELSE third

NextDeferredCommand(node) ==
  Head(DeferredClassQueue(node, SelectedDeferredClass(node)))

AdvanceNextDeferredClass(node) ==
  asyncNextDeferredClass' =
    [asyncNextDeferredClass EXCEPT
       ![node] = NextCommandClass(SelectedDeferredClass(node))]

RemoveNextDeferredCommand(node) ==
  /\ IF SelectedDeferredClass(node) = "Completion"
     THEN /\ asyncDeferredCompletionQueues' =
                [asyncDeferredCompletionQueues EXCEPT ![node] = Tail(@)]
          /\ UNCHANGED <<asyncDeferredProgressQueues,
                         asyncDeferredNormalQueues>>
     ELSE IF SelectedDeferredClass(node) = "Progress"
          THEN /\ asyncDeferredProgressQueues' =
                     [asyncDeferredProgressQueues EXCEPT ![node] = Tail(@)]
               /\ UNCHANGED <<asyncDeferredCompletionQueues,
                              asyncDeferredNormalQueues>>
          ELSE /\ asyncDeferredNormalQueues' =
                     [asyncDeferredNormalQueues EXCEPT ![node] = Tail(@)]
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

DeliveryCandidate(item) ==
  AsyncCandidate(DeliveryClass(item), DeliveryKind(item),
                 item.envelope.recipient, DeliveryHeight(item),
                 DeliveryView(item), DeliverySubject(item), item)

DueSourcePackets(recipient, source) ==
  {packet \in asyncTransport:
     /\ packet.item.envelope.recipient = recipient
     /\ packet.item.source = source
     /\ packet.deadline <= asyncNow}

OldestDueSourcePacket(recipient, source) ==
  CHOOSE packet \in DueSourcePackets(recipient, source):
    \A other \in DueSourcePackets(recipient, source):
      packet.sentAt <= other.sentAt

AdmitHiddenPacket(recipient, source) ==
  LET packet == OldestDueSourcePacket(recipient, source)
      item == packet.item
      lane == IngressLane(recipient, source)
  IN /\ recipient \in up
     /\ ~ResponsiveReplayQuarantined(recipient)
     /\ DueSourcePackets(recipient, source) # {}
     /\ item \notin SequenceSet(lane)
     /\ CanAdmitIngressItem(item)
     /\ asyncTransport' = asyncTransport \ {packet}
     /\ asyncIngressLanes' =
          [asyncIngressLanes EXCEPT
             ![recipient][source] = Append(@, item)]
     /\ asyncIngressReady' =
          IF Len(lane) = 0
          THEN [asyncIngressReady EXCEPT
                  ![recipient] = Append(@, source)]
          ELSE asyncIngressReady
     /\ UNCHANGED AsyncDeferredVars
     /\ LeaveCausalQueues
     /\ UNCHANGED AsyncLocalAdmissionVars
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncSentItems, asyncRetainedControl, asyncActiveRequests,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets
                    >>

(*
FairV2Ingress coalesces an exact wire retransmission only while the same source
still owns an identical queued envelope.  The packet occurrence is consumed,
but the original lane position and enqueue ownership remain unchanged.  Once
the queued occurrence is serviced, a later retransmission is fresh again.
*)
CoalesceHiddenPacket(recipient, source) ==
  LET packet == OldestDueSourcePacket(recipient, source)
      item == packet.item
  IN /\ recipient \in up
     /\ ~ResponsiveReplayQuarantined(recipient)
     /\ DueSourcePackets(recipient, source) # {}
     /\ item \in SequenceSet(IngressLane(recipient, source))
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
                    asyncActiveRequests, asyncHeldChunks,
                    asyncHistoricalRecoveryTargets>>

AdmitFreshHiddenPacket(recipient, source) ==
  AdmitHiddenPacket(recipient, source)

AdmitIngressPacket(recipient, source) ==
  \/ AdmitHiddenPacket(recipient, source)
  \/ CoalesceHiddenPacket(recipient, source)

HeadIngressSource(node) == Head(asyncIngressReady[node])

HeadIngressItem(node) ==
  Head(IngressLane(node, HeadIngressSource(node)))

IngressItemAt(node, index) ==
  Head(IngressLane(node, asyncIngressReady[node][index]))

CertifiedRequestAuthorized(item) ==
  /\ item.kind = "CertifiedRequest"
  /\ \E qc \in commitQCs:
       /\ qc.context = context
       /\ qc.view = item.envelope.view
       /\ qc.subject = item.envelope.subject
       /\ qc.phase = "Commit"
       /\ item.envelope.recipient \in qc.signers

MatchingCertifiedRequests(response) ==
  {request \in asyncActiveRequests:
     /\ request.kind = "CertifiedRequest"
     /\ request.source = response.envelope.recipient
     /\ request.envelope.height = response.envelope.height
     /\ request.envelope.view = response.envelope.view
     /\ request.envelope.subject = response.envelope.subject}

CertifiedResponseAuthorized(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ MatchingCertifiedRequests(item) # {}
  /\ \E decision \in decisions:
       /\ decision.node = item.envelope.recipient
       /\ decision.qc.context = context
       /\ decision.qc.view = item.envelope.view
       /\ decision.qc.subject = item.envelope.subject
       /\ decision.qc.phase = "Commit"
       /\ item.source \in decision.qc.signers

CommitCertificateRequestAuthorized(item) ==
  /\ item.kind = "CommitCertificateRequest"
  /\ item.source
       \in CurrentVoters \cup asyncHistoricalRecoveryTargets
  /\ item.envelope.recipient \in CurrentVoters
  /\ item.envelope.height = context.height

MatchingCommitCertificateRequests(response) ==
  {request \in asyncActiveRequests:
     /\ request.kind = "CommitCertificateRequest"
     /\ request.source = response.envelope.recipient
     /\ request.envelope.height = response.envelope.qc.context.height
     /\ request.envelope.recipient = response.source}

CommitCertificateResponseAuthorized(item) ==
  /\ item.kind = "CommitCertificateResponse"
  /\ item.source \in CurrentVoters
  /\ item.envelope.qc \in commitQCs
  /\ item.envelope.qc.context = context
  /\ item.envelope.qc.phase = "Commit"
  /\ MatchingCommitCertificateRequests(item) # {}

DiscoveredCommitQcItem(response) ==
  AsyncNetworkItem("CommitQC", response.source, response.envelope)

CommitCertificateResponseCandidate(item) ==
  DeliveryCandidate(DiscoveredCommitQcItem(item))

CertifiedResponseCandidate(item) ==
  AsyncCandidate("Completion", "FetchCertifiedBody",
                 item.envelope.recipient, item.envelope.height,
                 item.envelope.view, item.envelope.subject, item)

(***************************************************************************
The production fair ingress rotates sources, but scans each selected source
from its oldest entry to its newest and removes the first entry whose exact
downstream predicate admits it.  Earlier blocked entries stay in place and
the source consumes only one round-robin turn.  Keeping item admission
separate from source selection prevents auxiliary I/O backpressure at a lane
head from hiding later consensus/body progress from the same peer.  Response
candidates require scheduler-wide freshness, including causal ownership, so
the exact downstream candidate cannot be admitted into a second carrier.
***************************************************************************)
IngressItemCanDrain(node, item) ==
  LET candidate == DeliveryCandidate(item)
  IN item.kind = "Noise"
       \/ item \notin asyncSentItems
       \/ IF item.kind \in {"CertifiedRequest",
                             "CommitCertificateRequest"}
          THEN \/ ~(IF item.kind = "CertifiedRequest"
                    THEN CertifiedRequestAuthorized(item)
                    ELSE CommitCertificateRequestAuthorized(item))
               \/ /\ ~CompletionCausalAdmissionDebt(node)
                     /\ CanEnqueueIoClass(node, "Serve")
          ELSE IF item.kind = "CertifiedResponse"
               THEN \/ ~CertifiedResponseAuthorized(item)
                    \/ CandidateScheduled(
                         CertifiedResponseCandidate(item))
                    \/ /\ ~CompletionCausalAdmissionDebt(node)
                          /\ AsyncOutstandingWorkCount(node)
                              < AsyncIoWorkCapacity
                          /\ ~CandidateScheduled(
                               CertifiedResponseCandidate(item))
               ELSE IF item.kind = "CommitCertificateResponse"
                    THEN \/ ~CommitCertificateResponseAuthorized(item)
                         \/ CandidateScheduled(
                              CommitCertificateResponseCandidate(item))
                         \/ /\ ~NonCompletionCausalAdmissionDebt(node)
                               /\ CanEnqueueClass(node, "Progress")
                               /\ ~CandidateScheduled(
                                    CommitCertificateResponseCandidate(item))
               ELSE \/ CandidateScheduled(candidate)
                    \/ /\ ~NonCompletionCausalAdmissionDebt(node)
                          /\ CanEnqueueClass(node, candidate.class)

DrainableIngressLaneIndices(node, source) ==
  {index \in 1..Len(IngressLane(node, source)):
     IngressItemCanDrain(node, IngressLane(node, source)[index])}

FirstDrainableIngressLaneIndex(node, source) ==
  CHOOSE index \in DrainableIngressLaneIndices(node, source):
    \A other \in DrainableIngressLaneIndices(node, source): index <= other

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

FirstDrainableIngressIndex(node) ==
  CHOOSE index \in DrainableIngressIndices(node):
    \A other \in DrainableIngressIndices(node): index <= other

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

(***************************************************************************
The serialized Rust ingress authenticates every reducer-directed envelope
before comparing it with scheduler-owned authenticated envelopes.  An exact
retransmission is consumed from transport without taking a second runtime
slot, including while the first occurrence is deferred or causal; after the
owning occurrence leaves, the same envelope may begin a new ownership interval
and encounter generation-aware semantic admission.
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
              /\ item.envelope \notin qcNetwork
        THEN ImportAuthenticatedCommitCertificate(item.envelope)
        ELSE UNCHANGED vars
     /\ IF item.kind = "Noise" \/ item \notin asyncSentItems
        THEN /\ UNCHANGED <<asyncCommandQueues,
                            asyncNextCommandClass>>
             /\ UNCHANGED AsyncIoVars
             /\ UNCHANGED <<asyncSentItems, asyncRetainedControl,
                            asyncActiveRequests>>
        ELSE IF item.kind \in {"CertifiedRequest",
                                "CommitCertificateRequest"}
             THEN IF (IF item.kind = "CertifiedRequest"
                      THEN CertifiedRequestAuthorized(item)
                      ELSE CommitCertificateRequestAuthorized(item))
                  THEN /\ asyncIoQueues' =
                             [asyncIoQueues EXCEPT
                                ![node] = Append(
                                  @, AsyncIoCertifiedServeJob(node, candidate))]
                       /\ UNCHANGED <<asyncOutstandingWork,
                                       asyncIoReadyCompletions,
                                       asyncLocalReadyCompletions,
                                       asyncNextCompletionSource,
                                       asyncIoControlAvailable>>
                       /\ UNCHANGED <<asyncCommandQueues,
                                      asyncNextCommandClass, asyncSentItems,
                                      asyncRetainedControl,
                                      asyncActiveRequests>>
                  ELSE /\ UNCHANGED <<asyncCommandQueues,
                                      asyncNextCommandClass, AsyncIoVars>>
                       /\ UNCHANGED <<asyncSentItems,
                                      asyncRetainedControl,
                                      asyncActiveRequests>>
             ELSE IF item.kind = "CertifiedResponse"
                  THEN IF CertifiedResponseAuthorized(item)
                       THEN LET completion ==
                                  CertifiedResponseCandidate(item)
                            IN /\ IF CandidateScheduled(completion)
                                  THEN UNCHANGED <<AsyncIoVars,
                                                    asyncCommandQueues,
                                                    asyncNextCommandClass>>
                                  ELSE /\ asyncLocalReadyCompletions' =
                                              [asyncLocalReadyCompletions EXCEPT
                                                 ![node] = Append(@, completion)]
                                       /\ asyncOutstandingWork' =
                                              [asyncOutstandingWork EXCEPT
                                                 ![node] = @ \cup {completion}]
                                       /\ UNCHANGED <<asyncIoQueues,
                                                       asyncIoReadyCompletions,
                                                       asyncNextCompletionSource,
                                                       asyncIoControlAvailable,
                                                       asyncCommandQueues,
                                                       asyncNextCommandClass>>
                               /\ asyncActiveRequests' =
                                    asyncActiveRequests \
                                      MatchingCertifiedRequests(item)
                               /\ UNCHANGED <<asyncSentItems,
                                              asyncRetainedControl>>
                       ELSE /\ UNCHANGED <<asyncCommandQueues,
                                           asyncNextCommandClass,
                                           AsyncIoVars>>
                            /\ UNCHANGED <<asyncSentItems,
                                           asyncRetainedControl,
                                           asyncActiveRequests>>
                  ELSE IF item.kind = "CommitCertificateResponse"
                       THEN IF CommitCertificateResponseAuthorized(item)
                            THEN LET discovered ==
                                       DiscoveredCommitQcItem(item)
                                     discoveredCandidate ==
                                       CommitCertificateResponseCandidate(item)
                                 IN /\ IF CandidateScheduled(
                                               discoveredCandidate)
                                        THEN UNCHANGED <<asyncCommandQueues,
                                                          asyncNextCommandClass>>
                                        ELSE EnqueueCandidate(
                                               discoveredCandidate)
                                    /\ UNCHANGED AsyncIoVars
                                    /\ asyncActiveRequests' =
                                         asyncActiveRequests \
                                           MatchingCommitCertificateRequests(item)
                                    /\ asyncSentItems' =
                                         asyncSentItems \cup {discovered}
                                    /\ UNCHANGED asyncRetainedControl
                            ELSE /\ UNCHANGED <<asyncCommandQueues,
                                                asyncNextCommandClass,
                                                AsyncIoVars>>
                                 /\ UNCHANGED <<asyncSentItems,
                                                asyncRetainedControl,
                                                asyncActiveRequests>>
                  ELSE /\ IF CandidateScheduled(candidate)
                          THEN UNCHANGED <<asyncCommandQueues,
                                           asyncNextCommandClass>>
                          ELSE EnqueueCandidate(candidate)
                       /\ UNCHANGED AsyncIoVars
                       /\ UNCHANGED <<asyncSentItems,
                                      asyncRetainedControl,
                                      asyncActiveRequests>>
     /\ UNCHANGED <<asyncFifoOwed, asyncTimeoutEmitted,
                    asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines, asyncTransport,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets
                    >>

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
     THEN \/ ~CertifiedRequestAuthorized(item)
          \/ CanEnqueueIoClass(node, "Serve")
     ELSE IF item.kind = "CommitCertificateRequest"
          THEN \/ ~CommitCertificateRequestAuthorized(item)
               \/ CanEnqueueIoClass(node, "Serve")
          ELSE TRUE

HistoricalDrainableIngressLaneIndices(node, source) ==
  {index \in 1..Len(IngressLane(node, source)):
     HistoricalIngressItemCanDrain(
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
        \/ /\ item.kind = "CertifiedRequest"
              /\ item \in asyncSentItems
              /\ CertifiedRequestAuthorized(item)
        \/ /\ item.kind = "CommitCertificateRequest"
              /\ item \in asyncSentItems
              /\ CommitCertificateRequestAuthorized(item)
  IN /\ HistoricalDrainableIngressIndices(node) # {}
     /\ PopSelectedIngress(node, index, laneIndex)
     /\ IF authorizedRequest
        THEN /\ asyncIoQueues' =
                   [asyncIoQueues EXCEPT
                      ![node] = Append(
                        @, AsyncIoCertifiedServeJob(node, candidate))]
             /\ UNCHANGED <<asyncOutstandingWork,
                             asyncIoReadyCompletions,
                             asyncLocalReadyCompletions,
                             asyncNextCompletionSource,
                             asyncIoControlAvailable>>
        ELSE UNCHANGED AsyncIoVars
     /\ UNCHANGED <<vars, asyncCommandQueues, asyncNextCommandClass,
                    asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncDeferredVars,
                    asyncCausalQueues, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncSentItems, asyncRetainedControl,
                    asyncActiveRequests, asyncTransport, asyncHeldChunks,
                    asyncHistoricalRecoveryTargets>>

AdmitCausalHead(node) ==
  LET candidate == HeadCausalCandidate(node)
      duplicate == CandidateInFlight(candidate)
  IN /\ CausalHeadCanAdvance(node)
     /\ asyncCausalQueues' =
          [asyncCausalQueues EXCEPT ![node] = Tail(@)]
     /\ IF duplicate
        THEN /\ UNCHANGED <<asyncCommandQueues,
                            asyncNextCommandClass>>
             /\ UNCHANGED <<asyncIoQueues, asyncOutstandingWork,
                             asyncIoReadyCompletions,
                             asyncLocalReadyCompletions,
                             asyncNextCompletionSource,
                             asyncIoControlAvailable>>
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
                                  asyncIoControlAvailable>>
             ELSE /\ EnqueueCandidate(candidate)
                  /\ UNCHANGED AsyncIoVars
     /\ UNCHANGED <<vars, asyncFifoOwed, asyncTimeoutEmitted,
                    asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets>>

SelectedCompletionSource(node) ==
  IF asyncNextCompletionSource[node] = "Io"
  THEN IF Len(asyncIoReadyCompletions[node]) > 0
       THEN "Io" ELSE "Local"
  ELSE IF Len(asyncLocalReadyCompletions[node]) > 0
       THEN "Local" ELSE "Io"

SelectedCompletionQueueNonempty(node) ==
  IF SelectedCompletionSource(node) = "Io"
  THEN Len(asyncIoReadyCompletions[node]) > 0
  ELSE Len(asyncLocalReadyCompletions[node]) > 0

SelectedCompletionCandidate(node) ==
  IF SelectedCompletionSource(node) = "Io"
  THEN Head(asyncIoReadyCompletions[node])
  ELSE Head(asyncLocalReadyCompletions[node])

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
The local admission cursor alternates producer completions with causal work.
The first producer or no-admission turn that observes causal work records
sticky debt.  Non-Completion debt then reserves command capacity, while
Completion debt still permits the exact producer retirement needed to free an
outstanding-work slot.  Once the head is admissible, debt makes it the
deterministic preferred source under the existing fair RunNode action.
***************************************************************************)
PreferredLocalSource(node) ==
  IF asyncCausalAdmissionOwed[node] = TRUE
  THEN "Causal"
  ELSE IF asyncNextLocalSource[node] = "Causal"
       THEN "Causal"
       ELSE "Producer"

SelectedLocalSource(node) ==
  LET preferred == PreferredLocalSource(node)
  IN IF LocalSourceCanAdmit(node, preferred)
     THEN preferred
     ELSE OtherLocalSource(preferred)

LocalAdmissionCanAdvance(node) ==
  /\ asyncRunnerBudget[node] > 0
  /\ (ProducerCompletionCanAdvance(node) \/ CausalHeadCanAdvance(node))

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
          THEN [asyncIoReadyCompletions EXCEPT ![node] = Tail(@)]
          ELSE asyncIoReadyCompletions
     /\ asyncLocalReadyCompletions' =
          IF source = "Local"
          THEN [asyncLocalReadyCompletions EXCEPT ![node] = Tail(@)]
          ELSE asyncLocalReadyCompletions
     /\ asyncNextCompletionSource' =
          [asyncNextCompletionSource EXCEPT
             ![node] = IF source = "Io" THEN "Local" ELSE "Io"]
     /\ asyncOutstandingWork' =
          [asyncOutstandingWork EXCEPT ![node] = @ \ {candidate}]
     /\ UNCHANGED <<asyncIoQueues, asyncIoControlAvailable>>
     /\ UNCHANGED <<vars, asyncFifoOwed, asyncTimeoutEmitted,
                    asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks, asyncHistoricalRecoveryTargets>>

ServiceIoWorkerWork(node) ==
  LET job == Head(asyncIoQueues[node])
      responseItems ==
        IF job.class # "Serve"
        THEN {}
        ELSE IF CertifiedServeCanRespond(job.candidate.item)
             THEN {CertifiedResponseItem(job.candidate.item)}
             ELSE IF CommitCertificateServeCanRespond(job.candidate.item)
                  THEN CommitCertificateResponseItems(job.candidate.item)
                  ELSE {}
  IN /\ node \in up
     /\ ResponsiveReplayExecutorAllowed(node)
     /\ AsyncIoQueueDepth(node) > 0
     /\ asyncIoQueues' =
          [asyncIoQueues EXCEPT ![node] = Tail(@)]
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
  /\ node \in AsyncCurrentResponsiveVoters
  /\ ServiceIoWorkerWork(node)

ServiceHistoricalRecoveryIoWorker(node) ==
  /\ HistoricalRecoveryTarget(node)
  /\ ServiceIoWorkerWork(node)

EnqueueIoLocalControlWork(node) ==
  /\ node \in up
  /\ ~ResponsiveReplayQuarantined(node)
  /\ ~NodeHasApplication(node)
  /\ asyncIoControlAvailable[node]
  /\ ~CompletionCausalAdmissionDebt(node)
  /\ CanEnqueueIoClass(node, "Control")
  /\ asyncIoQueues' =
       [asyncIoQueues EXCEPT ![node] = Append(@, AsyncIoControlJob)]
  /\ asyncIoControlAvailable' =
       [asyncIoControlAvailable EXCEPT ![node] = FALSE]
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
                 asyncIoServiceDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
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
  /\ UNCHANGED <<asyncRetainedControl, asyncActiveRequests>>

RetryableItems(node) ==
  RetainedControlEmissionItems(node) \cup ActiveRequestItems(node)

SendNodeRetransmissions(node) ==
  /\ RetryableItems(node) # {}
  /\ asyncSentItems' = asyncSentItems \cup RetryableItems(node)
  /\ asyncTransport' =
       asyncTransport \cup PacketsForItems(RetryableItems(node))
  /\ UNCHANGED <<asyncRetainedControl, asyncActiveRequests>>

NoSendItem ==
  UNCHANGED <<asyncSentItems, asyncRetainedControl,
              asyncActiveRequests, asyncTransport>>

TimeoutCausalCommand(node) ==
  NoItemCandidate("Completion", "BeginTimeout", node, nodeView[node],
                  highestSubject[node])

(***************************************************************************
The same rigid-witness rule is required for the parameterized timeout action.
Runtime callers select nodes from ValidatorIds, making this equivalent to the
direct ENABLED BeginTimeout(node) test on every reachable state.
***************************************************************************)
BeginTimeoutEnabled(node) ==
  \E selectedNode \in ValidatorIds:
    /\ selectedNode = node
    /\ ENABLED BeginTimeout(selectedNode)

CommitCertificateDiscoveryStepWork(node) ==
  /\ node \in up
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
                 asyncNextDeferredClass>>
  /\ asyncDeferredDrainOwed' =
       IF BeginTimeoutEnabled(node)
       THEN [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
       ELSE asyncDeferredDrainOwed
  /\ UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                 asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
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
                 asyncNextDeferredClass>>
  /\ asyncDeferredDrainOwed' =
       IF NodeIdle(node)
       THEN [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
       ELSE asyncDeferredDrainOwed
  /\ LeaveCausalQueues
  /\ UNCHANGED <<vars, asyncCommandQueues, asyncNextCommandClass,
                 asyncTimeoutEmitted,
                 asyncNodeDeadlines, asyncIngressLanes,
                 asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets
                 >>

DeferredTimeoutExecutable(node) ==
  /\ "TimeoutElapsed" \in asyncOutstandingTags[node]
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
                 asyncNextDeferredClass>>
  /\ asyncDeferredDrainOwed' =
       [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
  /\ UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                 asyncFifoOwed,
                 asyncTimeoutEmitted, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

DeferredRetransmitStep(node) ==
  /\ "RetransmitElapsed" \in asyncOutstandingTags[node]
  /\ NodeIdle(node)
  /\ UNCHANGED vars
  /\ IF RetryableItems(node) # {}
     THEN SendNodeRetransmissions(node)
     ELSE NoSendItem
  /\ asyncOutstandingTags' =
       [asyncOutstandingTags EXCEPT ![node] = @ \ {"RetransmitElapsed"}]
  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues, asyncDeferredNormalQueues,
                 asyncNextDeferredClass>>
  /\ asyncDeferredDrainOwed' =
       [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
  /\ LeaveCausalQueues
  /\ UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                 asyncFifoOwed,
                 asyncTimeoutEmitted, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

DeferredTagExecutable(node) ==
  DeferredTimeoutExecutable(node)
    \/ (/\ "TimeoutElapsed" \notin asyncOutstandingTags[node]
        /\ "RetransmitElapsed" \in asyncOutstandingTags[node]
        /\ NodeIdle(node))

DeferredTagStep(node) ==
  IF DeferredTimeoutExecutable(node)
  THEN DeferredTimeoutStep(node)
  ELSE DeferredRetransmitStep(node)

(***************************************************************************
The historical `FifoRuntimeStep` name denotes the timer-versus-command debt
tracked by `asyncFifoOwed`; command selection itself is the cyclic class-aware
dispatch defined by `NextNodeCommandIndex`, not a global FIFO-head pop.
***************************************************************************)
FifoRuntimeStep(node) ==
  LET command == NextNodeCommand(node)
      succeeds == CommandDispatchable(command)
  IN /\ NodeQueueNonempty(node)
     /\ RemoveNextNodeCommand(node)
     /\ IF succeeds
        THEN /\ ExecuteCommand(command)
             /\ AppendCausalSuccessors(command)
             /\ UNCHANGED <<asyncDeferredCompletionQueues,
                            asyncDeferredProgressQueues,
                            asyncDeferredNormalQueues,
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
                                 asyncNextDeferredClass>>
                  /\ asyncDeferredDrainOwed' =
                       [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
     /\ asyncFifoOwed' = [asyncFifoOwed EXCEPT ![node] = FALSE]
     /\ asyncTimeoutEmitted' =
          IF succeeds /\ command.kind = "PersistInstallTC"
          THEN [asyncTimeoutEmitted EXCEPT ![node] = FALSE]
          ELSE asyncTimeoutEmitted

DeferredDrainStep(node) ==
  /\ asyncDeferredDrainOwed[node]
  /\ IF ~DeferredQueueNonempty(node)
     THEN /\ UNCHANGED <<vars, asyncCommandQueues,
                         asyncNextCommandClass, asyncFifoOwed,
                         asyncTimeoutEmitted, asyncDeferredCompletionQueues,
                         asyncDeferredProgressQueues,
                         asyncDeferredNormalQueues,
                         asyncNextDeferredClass, asyncOutstandingTags,
                         asyncNodeDeadlines, asyncRetransmitDeadlines,
                         asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport, asyncIngressLanes,
                         asyncIngressReady, asyncHeldChunks,
                         asyncHistoricalRecoveryTargets>>
          /\ LeaveCausalQueues
          /\ asyncDeferredDrainOwed' =
               [asyncDeferredDrainOwed EXCEPT ![node] = FALSE]
     ELSE LET command == NextDeferredCommand(node)
          IN IF CommandDispatchable(command)
             THEN /\ RemoveNextDeferredCommand(node)
                  /\ ExecuteCommand(command)
                  /\ AppendCausalSuccessors(command)
                  /\ asyncDeferredDrainOwed' = asyncDeferredDrainOwed
                  /\ asyncTimeoutEmitted' =
                       IF command.kind = "PersistInstallTC"
                       THEN [asyncTimeoutEmitted EXCEPT ![node] = FALSE]
                       ELSE asyncTimeoutEmitted
                  /\ UNCHANGED <<asyncCommandQueues,
                                  asyncNextCommandClass, asyncFifoOwed>>
             ELSE IF ~NodeIdle(node)
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
                                      asyncRetransmitDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests,
                                      asyncTransport, asyncIngressLanes,
                                      asyncIngressReady, asyncHeldChunks,
                                      asyncHistoricalRecoveryTargets>>
                       /\ asyncDeferredDrainOwed' =
                            [asyncDeferredDrainOwed EXCEPT ![node] = FALSE]
                  ELSE /\ RemoveNextDeferredCommand(node)
                       /\ DiscardCommand(command)
                       /\ LeaveCausalQueues
                       /\ asyncDeferredDrainOwed' = asyncDeferredDrainOwed
                       /\ UNCHANGED <<asyncCommandQueues,
                                      asyncNextCommandClass, asyncFifoOwed,
                                      asyncTimeoutEmitted>>

IdleRuntimeStep(node) ==
  /\ UNCHANGED <<vars, asyncCommandQueues, asyncNextCommandClass,
                 asyncTimeoutEmitted,
                 AsyncDeferredVars,
                 asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>
  /\ LeaveCausalQueues
  /\ asyncFifoOwed' = [asyncFifoOwed EXCEPT ![node] = FALSE]

RuntimeStep(node) ==
  \/ /\ asyncDeferredDrainOwed[node]
        /\ DeferredDrainStep(node)
  \/ /\ ~asyncDeferredDrainOwed[node]
        /\ DeferredTagExecutable(node)
        /\ DeferredTagStep(node)
  \/ /\ ~asyncDeferredDrainOwed[node]
        /\ ~DeferredTagExecutable(node)
        /\ TimeoutDue(node)
        /\ DirectTimeoutStep(node)
  \/ /\ ~asyncDeferredDrainOwed[node]
        /\ ~DeferredTagExecutable(node)
        /\ ~TimeoutDue(node)
        /\ NodeQueueNonempty(node)
        /\ asyncFifoOwed[node]
        /\ FifoRuntimeStep(node)
  \/ /\ ~asyncDeferredDrainOwed[node]
        /\ ~DeferredTagExecutable(node)
        /\ ~TimeoutDue(node)
        /\ ~(NodeQueueNonempty(node) /\ asyncFifoOwed[node])
        /\ RetransmitDue(node)
        /\ DirectRetransmitStep(node)
  \/ /\ ~asyncDeferredDrainOwed[node]
        /\ ~DeferredTagExecutable(node)
        /\ ~TimeoutDue(node)
        /\ ~(NodeQueueNonempty(node) /\ asyncFifoOwed[node])
        /\ ~RetransmitDue(node)
        /\ NodeQueueNonempty(node)
        /\ FifoRuntimeStep(node)
  \/ /\ ~asyncDeferredDrainOwed[node]
        /\ ~DeferredTagExecutable(node)
        /\ ~TimeoutDue(node)
        /\ ~RetransmitDue(node)
        /\ ~NodeQueueNonempty(node)
        /\ IdleRuntimeStep(node)

LocalAdmissionStep(node) ==
  /\ asyncRunnerPhase[node] = "Local"
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
                         asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport, asyncIngressLanes,
                         asyncIngressReady, asyncHeldChunks,
                         asyncHistoricalRecoveryTargets
                         >>
          /\ asyncRunnerPhase' =
               [asyncRunnerPhase EXCEPT ![node] = "Runtime"]
          /\ asyncRunnerBudget' =
               [asyncRunnerBudget EXCEPT ![node] = 1]

SerializedRuntimeStep(node) ==
  /\ asyncRunnerPhase[node] = "Runtime"
  /\ UNCHANGED AsyncIoVars
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ RuntimeStep(node)
  /\ asyncRunnerPhase' = [asyncRunnerPhase EXCEPT ![node] = "Local"]
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = AsyncQueueCapacity]

RunNodeWork(node) ==
  /\ node \in up
  /\ ~NodeHasApplication(node)
  /\ IF ResponsiveReplayQuarantined(node)
     THEN /\ ResponsiveReplayDraining(node)
          /\ asyncIngressReady[node] = <<>>
          /\ \/ LocalAdmissionStep(node)
             \/ IngressDrainStep(node)
             \/ SerializedRuntimeStep(node)
     ELSE \/ LocalAdmissionStep(node)
          \/ IngressDrainStep(node)
          \/ SerializedRuntimeStep(node)
  /\ UNCHANGED asyncNow
  /\ asyncNodeServiceDeadlines' =
       [asyncNodeServiceDeadlines EXCEPT
          ![node] = asyncNow + AsyncDeliveryBound]
  /\ UNCHANGED asyncIoServiceDeadlines

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
                 asyncActiveRequests, asyncTransport,
                 asyncIngressLanes, asyncIngressReady,
                 asyncHeldChunks, asyncHistoricalRecoveryTargets>>

RunHistoricalServer(node) ==
  /\ node \in AsyncCurrentResponsiveVoters \cap up
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

This lifecycle admits repeated responsive-validator crashes while a validator
has another finite generation available.  Each crash makes process-local
reducer and scheduler memory inaccessible.  Authenticated
restart increments the generation, reconstructs durable control frontiers,
and drives the production signature FIFO one Core owner at a time.  Only the
recovering node is quarantined; other validators and network-owned packets
continue independently.  Immutable sent history remains outside the reset.
***************************************************************************)

AsyncRecoveryPhases ==
  {"Eligible", "RestartRequired", "ReplayRequired", "Replaying",
   "Recovered"}

RestartCandidate(commandClass, kind, node, roundView, subject, evidence) ==
  AsyncCandidateAtConsumer(
    commandClass, kind, node, context.height, roundView, subject,
    NoAsyncItem, nodeView[node], generation[node], evidence,
    subject, subject, subject)

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
  ELSE LET signatures == RestartSignatureReplay(node)
       IN IF Len(signatures) > 0
          THEN <<Head(signatures)>>
          ELSE RestartRunnerAssembly(node)

RestartHighestPrepareQCs(node) ==
  {qc \in prepareQCs:
     /\ highestRank[node] # NoRank
     /\ qc.context = context
     /\ qc.phase = "Prepare"
     /\ qc.view = highestRank[node]
     /\ qc.subject = highestSubject[node]}

RestartDecisionQCs(node) ==
  {decision.qc:
     decision \in {entry \in decisions:
       entry.node = node /\ entry.qc.context = context}}

RestartInstalledTCs(node) ==
  {entry.tc:
     entry \in {installed \in installedTCs:
       installed.node = node /\ installed.tc.context = context}}

RestartLastInstalledTCs(node) ==
  {tc \in RestartInstalledTCs(node):
     \A other \in RestartInstalledTCs(node): other.view <= tc.view}

RestartHighestPrepareControl(node) ==
  LET certificates == RestartHighestPrepareQCs(node)
  IN IF certificates = {}
     THEN {}
     ELSE QcOutbox(node, CHOOSE qc \in certificates: TRUE)

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

ResetNodeSchedulerForRestart(node, replay) ==
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
  /\ asyncNextDeferredClass' =
       [asyncNextDeferredClass EXCEPT ![node] = "Completion"]
  /\ asyncDeferredDrainOwed' =
       [asyncDeferredDrainOwed EXCEPT ![node] = FALSE]
  /\ asyncCausalQueues' = [asyncCausalQueues EXCEPT ![node] = replay]
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
  /\ asyncActiveRequests' =
       {item \in asyncActiveRequests: item.source # node}
  /\ asyncTransport' = asyncTransport
  /\ asyncIngressLanes' =
       [asyncIngressLanes EXCEPT
          ![node] = [source \in AsyncIngressSources |-> <<>>]]
  /\ asyncIngressReady' = [asyncIngressReady EXCEPT ![node] = <<>>]
  /\ asyncHeldChunks' =
       {receipt \in asyncHeldChunks: receipt.node # node}
  /\ asyncHistoricalRecoveryTargets' =
       asyncHistoricalRecoveryTargets \ {node}

AsyncSetGST ==
  /\ ~gst
  /\ asyncRecoveryPhase
       \notin {"RestartRequired", "ReplayRequired", "Replaying"}
  /\ Responsive \subseteq up
  /\ SetGST
  /\ UNCHANGED <<AsyncSchedulerVars, AsyncRecoveryVars>>
  /\ AsyncNonRunnerOuterFrame

(***************************************************************************
Faults outside the trusted product loop.  Before GST packets may be lost and
non-responsive validators may crash.  Byzantine noise is bounded in its own
authenticated source lane and cannot occupy an honest source's slots.
***************************************************************************)

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
                 asyncIoServiceDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncIngressLanes,
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
  /\ node \in Responsive \cap up
  /\ generation[node] < MaxGeneration
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
     /\ generation[node] < MaxGeneration
     /\ Restart(node)
     /\ UNCHANGED AsyncSchedulerVars
     /\ asyncRecoveryPhase' = "ReplayRequired"
     /\ asyncRecoveryNode' = node
     /\ asyncRecoveryGeneration' = generation[node] + 1
     /\ asyncRecoveryReplayQueue' = asyncRecoveryReplayQueue
     /\ AsyncCoreOuterFrame

RecoveryCoreReplay(node, candidate) ==
  CASE candidate.kind = "SignProposal" ->
         ResumeProposal(node, candidate.evidence)
    [] candidate.kind = "SignVote" ->
         ResumeVote(node, candidate.evidence)
    [] candidate.kind = "SignTimeout" ->
         ResumeTimeout(node, candidate.evidence)
    [] OTHER -> FALSE

PreGstResponsiveReplay ==
  LET node == asyncRecoveryNode
      signatures == RestartSignatureReplay(node)
      replay == RestartReplay(node)
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
     /\ RecoveryCoreReplay(node, candidate)
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
                     asyncActiveRequests, asyncTransport,
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
                     asyncActiveRequests, asyncTransport,
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
      packet == AsyncPacket(item, asyncNow, asyncNow + AsyncDeliveryBound)
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
                    asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncIngressLanes, asyncIngressReady,
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
  LET envelope ==
        AsyncBodyEnvelope(recipient, context.height, nodeView[recipient],
                          AsyncHeartbeatSubject, NoAsyncChunk, nonce)
      item == AsyncNetworkItem(
                kind, AsyncUntrustedSource, envelope)
      packet == AsyncPacket(item, asyncNow, asyncNow + AsyncDeliveryBound)
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
                    asyncActiveRequests, asyncIngressLanes,
                    asyncIngressReady, asyncHeldChunks,
                    asyncHistoricalRecoveryTargets>>

InjectAuthenticatedJunk(kind, source, recipient, nonce) ==
  LET envelope ==
        AsyncBodyEnvelope(recipient, context.height, nodeView[recipient],
                          AsyncHeartbeatSubject, NoAsyncChunk, nonce)
      item == AsyncNetworkItem(kind, source, envelope)
      packet == AsyncPacket(item, asyncNow, asyncNow + AsyncDeliveryBound)
  IN /\ kind \in {"NormalJunk", "ProgressJunk"}
     /\ source \in Byzantine(CurrentEpoch) \cap up
     /\ recipient \in CurrentVoters
     /\ nonce \in 0..(AsyncIngressCapacity - 1)
     /\ ~ItemScheduled(item)
     /\ packet \notin asyncTransport
     /\ asyncSentItems' = asyncSentItems \cup {item}
     /\ asyncTransport' = asyncTransport \cup {packet}
     /\ UNCHANGED <<asyncRetainedControl, asyncActiveRequests>>
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
        AsyncBodyEnvelope(recipient, context.height, qc.view, qc.subject,
                          NoAsyncChunk, nonce)
      item == AsyncNetworkItem("CertifiedRequest", source, envelope)
      packet == AsyncPacket(item, asyncNow, asyncNow + AsyncDeliveryBound)
  IN /\ source \in Byzantine(CurrentEpoch) \cap up
     /\ recipient \in qc.signers \cap AsyncCurrentResponsiveVoters
     /\ qc \in commitQCs
     /\ nonce \in 0..(AsyncIngressCapacity - 1)
     /\ ~ItemScheduled(item)
     /\ packet \notin asyncTransport
     /\ asyncSentItems' = asyncSentItems \cup {item}
     /\ asyncTransport' = asyncTransport \cup {packet}
     /\ UNCHANGED <<asyncRetainedControl, asyncActiveRequests>>
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
                       justifyRank, justifySubject) ==
  LET proposal == Proposal(context, roundView, subject, signer,
                           justifyRank, justifySubject)
  IN /\ ByzantineBroadcastProposal(signer, roundView, subject,
                                    justifyRank, justifySubject)
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

AsyncByzantineTimeout(signer, roundView, highRank, highSubject) ==
  LET vote == TimeoutVote(context, roundView, signer, highRank, highSubject)
  IN /\ ByzantineBroadcastTimeout(signer, roundView, highRank, highSubject)
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
       subject \in Subjects, justifyRank \in Ranks,
       justifySubject \in SubjectOrNone:
       AsyncByzantineProposal(signer, roundView, subject,
                              justifyRank, justifySubject)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       phase \in Phases, subject \in Subjects:
       AsyncByzantineVote(signer, roundView, phase, subject)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       highRank \in Ranks, highSubject \in SubjectOrNone:
       AsyncByzantineTimeout(signer, roundView, highRank, highSubject)

AsyncNetworkStep ==
  \E recipient \in ValidatorIds, source \in AsyncIngressSources:
    AdmitIngressPacket(recipient, source)

OverdueResponsivePackets ==
  {packet \in asyncTransport:
     /\ \/ /\ packet.item.source \in AsyncCurrentResponsiveVoters
              /\ packet.item.envelope.recipient
                   \in AsyncCurrentResponsiveVoters
        \/ /\ HistoricalRecoveryTarget(packet.item.source)
              /\ packet.item.envelope.recipient
                   \in AsyncCurrentResponsiveVoters
        \/ /\ packet.item.source \in AsyncCurrentResponsiveVoters
              /\ HistoricalRecoveryTarget(
                   packet.item.envelope.recipient)
     /\ packet.deadline <= asyncNow}

AsyncTickEnabled ==
  \/ ~gst
  \/ /\ gst
     /\ OverdueResponsivePackets = {}
     /\ \A node \in AsyncCurrentResponsiveVoters
                       \cup asyncHistoricalRecoveryTargets:
          /\ asyncNodeServiceDeadlines[node] > asyncNow
          /\ \/ AsyncIoQueueDepth(node) = 0
             \/ asyncIoServiceDeadlines[node] > asyncNow

AsyncNonClockVars ==
  <<vars, asyncCommandQueues, asyncNextCommandClass,
    asyncFifoOwed, asyncTimeoutEmitted,
    asyncRunnerPhase, asyncRunnerBudget, AsyncLocalAdmissionVars, AsyncIoVars,
    asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncNextDeferredClass,
    asyncDeferredDrainOwed, asyncCausalQueues,
    asyncOutstandingTags, asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncNodeServiceDeadlines, asyncIoServiceDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests,
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
  \/ (\E node \in AsyncCurrentResponsiveVoters:
        RunHistoricalServer(node))

AsyncNonRunnerStep ==
  /\ \/ AsyncSetGST
     \/ AsyncTick
     \/ (\E node \in ValidatorIds: OpenHistoricalRecovery(node))
     \/ (\E node \in AsyncCurrentResponsiveVoters:
           DirectCommitCertificateDiscoveryStep(node))
     \/ (\E node \in asyncHistoricalRecoveryTargets:
           DirectHistoricalCommitCertificateDiscoveryStep(node))
     \/ (\E node \in AsyncCurrentResponsiveVoters: ServiceIoWorker(node))
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
     /\ UNCHANGED <<up, AsyncRecoveryVars>>
  \/ /\ (DriveResponsiveReplayHead \/ FinishResponsiveReplay)
     /\ UNCHANGED up
  \/ /\ RearmResponsiveRecovery
     /\ UNCHANGED up

AsyncNext ==
  /\ (AsyncNonCrashStep
        \/ (\E node \in ValidatorIds: PreGstCrash(node))
        \/ (\E node \in ValidatorIds: PreGstResponsiveCrash(node))
        \/ PreGstResponsiveRestart
        \/ PreGstResponsiveReplay)
  /\ UNCHANGED <<height, context>>
  /\ [Next]_vars

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
        /\ source \in AsyncCurrentResponsiveVoters
  \/ /\ HistoricalRecoveryTarget(source)
        /\ recipient \in AsyncCurrentResponsiveVoters

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
  \/ AsyncTick
  \/ (\E node \in AsyncVotersAt(initialContext):
        PostGstRunNode(node))
  \/ (\E node \in Responsive:
        PostGstOpenHistoricalRecovery(node))
  \/ (\E node \in Responsive:
        PostGstRunHistoricalRecoveryNode(node))
  \/ (\E node \in AsyncVotersAt(initialContext):
        PostGstRunHistoricalServer(node))
  \/ (\E node \in AsyncVotersAt(initialContext):
        PostGstCommitCertificateDiscovery(node))
  \/ (\E node \in Responsive:
        PostGstHistoricalCommitCertificateDiscovery(node))
  \/ (\E node \in AsyncVotersAt(initialContext):
        PostGstServiceIoWorker(node))
  \/ (\E node \in Responsive:
        PostGstServiceHistoricalRecoveryIoWorker(node))
  \/ (\E recipient \in AsyncVotersAt(initialContext),
         source \in AsyncVotersAt(initialContext):
        PostGstAdmitHiddenPacket(recipient, source))
  \/ (\E recipient \in ValidatorIds, source \in ValidatorIds:
        PostGstAdmitHistoricalRecoveryPacket(recipient, source))

AsyncFairnessAt(initialContext) ==
  /\ WF_AsyncAllVars(AsyncSetGST)
  /\ WF_AsyncAllVars(PreGstResponsiveRestart)
  /\ WF_AsyncAllVars(PreGstResponsiveReplay)
  \* Signature replay executes through the ordinary serialized node runner
  \* and completion I/O worker before the next durable intent may be installed
  \* in Core.  GST remains disabled until that replay corridor drains, so its
  \* I/O worker needs replay-scoped fairness independent of post-GST fairness.
  /\ WF_AsyncAllVars(ResponsiveReplayRunNode)
  /\ WF_AsyncAllVars(ResponsiveReplayServiceIoWorker)
  /\ WF_AsyncAllVars(DriveResponsiveReplayHead)
  /\ WF_AsyncAllVars(FinishResponsiveReplay)
  /\ WF_AsyncAllVars(AsyncTick)
  /\ \A node \in AsyncVotersAt(initialContext):
       WF_AsyncAllVars(PostGstRunNode(node))
  /\ \A node \in Responsive:
       WF_AsyncAllVars(PostGstOpenHistoricalRecovery(node))
  /\ \A node \in Responsive:
       WF_AsyncAllVars(PostGstRunHistoricalRecoveryNode(node))
  /\ \A node \in AsyncVotersAt(initialContext):
       WF_AsyncAllVars(PostGstRunHistoricalServer(node))
  /\ \A node \in AsyncVotersAt(initialContext):
       WF_AsyncAllVars(PostGstCommitCertificateDiscovery(node))
  /\ \A node \in Responsive:
       WF_AsyncAllVars(PostGstHistoricalCommitCertificateDiscovery(node))
  /\ \A node \in AsyncVotersAt(initialContext):
       WF_AsyncAllVars(PostGstServiceIoWorker(node))
  /\ \A node \in Responsive:
       WF_AsyncAllVars(PostGstServiceHistoricalRecoveryIoWorker(node))
  /\ \A recipient \in AsyncVotersAt(initialContext),
       source \in AsyncVotersAt(initialContext):
       WF_AsyncAllVars(PostGstAdmitHiddenPacket(recipient, source))
  /\ \A recipient \in ValidatorIds, source \in ValidatorIds:
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
  /\ asyncNextDeferredClass =
       [node \in ValidatorIds |-> "Completion"]
  /\ asyncDeferredDrainOwed = [node \in ValidatorIds |-> FALSE]

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
  /\ asyncTransport = {}
  /\ asyncHeldChunks = {}
  /\ asyncHistoricalRecoveryTargets = {}

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

AsyncRuntimeTypeInvariant ==
  /\ AsyncRuntimeScalarTypeInvariant
  /\ AsyncCausalTypeInvariant

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

ActiveBusyCompletionCarrier ==
  QueuedCandidates \cup CausalCandidates \cup TrackedWorkCandidates

BusyCompletionCandidates(node) ==
  {candidate \in ActiveBusyCompletionCarrier:
     /\ candidate.node = node
     /\ candidate.class = "Completion"
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
A busy reducer is never justified by a completion stranded behind the
production Busy-deferred head: its exact persistence/signature completion is
owned by the active causal/I/O/runtime pipeline.  The completion is therefore
reachable without first asking the busy reducer to accept unrelated work.
***************************************************************************)
BusyCompletionWitnessInvariant ==
  \A node \in ValidatorIds:
    ~NodeIdle(node) =>
      BusyCompletionCandidates(node) # {}

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
       /\ AsyncOutstandingWorkCount(node) <= AsyncIoWorkCapacity

AsyncIoTypeInvariant ==
  /\ AsyncIoTopologyTypeInvariant
  /\ AsyncIoContentTypeInvariant
  /\ AsyncIoCapacityTypeInvariant

AsyncDeferredTopologyTypeInvariant ==
  /\ DOMAIN asyncDeferredCompletionQueues = ValidatorIds
  /\ DOMAIN asyncDeferredProgressQueues = ValidatorIds
  /\ DOMAIN asyncDeferredNormalQueues = ValidatorIds
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
       /\ Len(asyncDeferredProgressQueues[node]) <=
            AsyncDeferredProgressCapacity
       /\ Len(asyncDeferredNormalQueues[node]) <=
            AsyncDeferredNormalCapacity

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
                 /\ IngressLane(recipient, source)[index].source = source

AsyncIngressTypeInvariant ==
  /\ AsyncIngressTopologyTypeInvariant
  /\ AsyncIngressCapacityTypeInvariant
  /\ AsyncIngressContentTypeInvariant

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
        /\ asyncRecoveryGeneration < MaxGeneration
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
        /\ asyncIngressReady[asyncRecoveryNode] = <<>>
        /\ \A source \in AsyncIngressSources:
             IngressLane(asyncRecoveryNode, source) = <<>>
        /\ \A request \in asyncActiveRequests:
             request.source # asyncRecoveryNode
        /\ \A candidate \in
             ResponsiveReplayScheduledCandidates(asyncRecoveryNode):
             /\ candidate.class = "Completion"
             /\ candidate.kind
                  \in {"SignProposal", "SignVote", "SignTimeout"}
             /\ CandidateConsumerCurrent(candidate))
  /\ (asyncRecoveryPhase = "Recovered" =>
        Responsive \subseteq up)

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

AsyncIoReservationInvariant ==
  /\ AsyncIoWorkCapacity <= AsyncCompletionReserve
  /\ \A node \in ValidatorIds:
       /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
       /\ Cardinality(
            {index \in 1..AsyncIoQueueDepth(node):
               asyncIoQueues[node][index].class = "Serve"})
            <= AsyncIoAuxCapacity
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
     /\ asyncCommandQueues[node][index].class = command.class
     /\ \E matching \in SchedulerCandidateIndices(node, command):
          index <= matching}

CommandClassDistance(fromClass, toClass) ==
  IF fromClass = toClass
  THEN 0
  ELSE IF NextCommandClass(fromClass) = toClass THEN 1 ELSE 2

(***************************************************************************
The scheduler rank is the duplicate-aware ordinal through the last matching
command value, not an arbitrary matching index.  Multiplication by three
leaves room for the cyclic cursor distance: a non-target class dispatch lowers
that distance, while a target-class dispatch lowers the ordinal and may reset
the distance by at most two.
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
