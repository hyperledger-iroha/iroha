---- MODULE SumeragiV2AsyncNetwork ----
EXTENDS SumeragiV2Inductive, Sequences, FiniteSets, Naturals, Functions

(***************************************************************************
Production-coupled asynchronous execution for Sumeragi v2.

There is one reducer scheduler per validator.  The validators are independent
processes: post-GST weak fairness is attached to each responsive validator's
run-loop action, never to a favorable global interleaving and never to an
individual protocol command.  Each runtime owns the same single tagged FIFO as
`BoundedIngress`; normal, progress, and completion are admission classes over
the total FIFO length, and service always removes its head.  TimeoutElapsed and
RetransmitElapsed are local scheduler inputs.  If the reducer is busy they are
coalesced in the adapter's trusted deferred-completion set until the busy fence
clears.

Queued stale I/O is deliberately retained in this model as a conservative
service burden.  Production cancels queued certified-stale Sign/Store/Validate
work by exact identity and coalesces exact retries until the completion is
acknowledged; non-stale admission remains lossless.  Thus a full model FIFO
leaves its causal producer head pending until weakly fair head service creates
capacity, over-approximating rather than hiding stale work.  Likewise all
validators in one instance use the same view-indexed pacemaker rule; mixed
fixed/adaptive binaries require a version or configuration-fingerprint gate
before the temporal theorem applies to a deployment.

The hidden transport carries actual Core network envelopes into a distinct
model of `FairV2Ingress`.  Every recipient has one lane for each frozen-roster
validator plus one aggregate untrusted lane.  Admission is bounded by one
total capacity while reserving one slot for every empty other lane; non-empty
lanes are serviced by the exact ready-queue rotation used in Rust.  A source
may borrow idle capacity but cannot consume another source's protected empty
slot.  An emitted Core envelope remains in immutable authentication history
when a hidden packet is lost before GST.  Retransmission scans only the
reducer's bounded per-class retained controls and active certified-body
requests.

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
   "FetchBody", "StoreBody", "ValidateBody", "BeginPrepare",
   "PersistPrepare", "SignVote", "FormPrepareQC", "BeginObservePrepare",
   "PersistObservePrepare", "BeginLockCommit", "PersistLockCommit",
   "FormCommitQC", "BeginDecision", "PersistDecision", "BeginTimeout",
     "PersistTimeout", "SignTimeout", "FormTC", "BeginInstallTC",
   "PersistInstallTC", "BeginCommitCertificateDiscovery",
   "RequestCertifiedBody", "FetchCertifiedBody", "Apply"}
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

AsyncChunkReceipt(node, roundView, subject, chunk) ==
  [node |-> node, view |-> roundView, subject |-> subject, chunk |-> chunk]

AsyncChunkReceiptSet ==
  [node: ValidatorIds, view: Views, subject: ValidSubjects,
   chunk: AsyncChunks]

AsyncRuntimeCycleBudget ==
  AsyncQueueCapacity + AsyncIngressCapacity + 3

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
  /\ AsyncIngressCapacity >= Cardinality(AsyncIngressSources)
  /\ AsyncIoAuxCapacity \in Nat \ {0}
  /\ AsyncIoWorkCapacity \in Nat \ {0}
  /\ AsyncIoWorkCapacity <= AsyncCompletionReserve
  /\ AsyncDeferredNormalCapacity \in Nat \ {0}
  /\ AsyncDeferredProgressCapacity \in Nat \ {0}
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
   subject: ValidSubjects, chunk: 0..AsyncChunkCount,
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
          kind \in {"Chunk", "CertifiedRequest",
                    "CommitCertificateRequest",
                    "NormalJunk", "ProgressJunk"},
          source \in ValidatorIds, envelope \in AsyncBodyEnvelopeSet}
  \cup {AsyncNetworkItem("CertifiedResponse", source, envelope):
          source \in ValidatorIds, envelope \in AsyncBodyEnvelopeSet}
  \cup {AsyncNetworkItem("CommitCertificateResponse", source, envelope):
          source \in ValidatorIds, envelope \in QcEnvelopeSet}
  \cup {AsyncNetworkItem("Noise", source, envelope):
          source \in AsyncIngressSources, envelope \in AsyncBodyEnvelopeSet}

AsyncCandidate(commandClass, kind, node, blockHeight, roundView, subject,
               item) ==
  [class |-> commandClass, kind |-> kind, node |-> node,
   height |-> blockHeight, view |-> roundView, subject |-> subject,
   item |-> item]

AsyncCandidateSet ==
  [class: AsyncCommandClasses, kind: AsyncWorkKinds, node: ValidatorIds,
   height: Heights, view: Views, subject: SubjectOrNone,
   item: AsyncNetworkItems \cup {NoAsyncItem}]

NoAsyncCandidate ==
  AsyncCandidate("Normal", "AssembleBody", 0, 0, 0,
                 AsyncHeartbeatSubject, NoAsyncItem)

AsyncIoCapacity == AsyncIoAuxCapacity + AsyncIoWorkCapacity + 1

AsyncIoJob(commandClass, candidate, nonce) ==
  [class |-> commandClass, candidate |-> candidate, nonce |-> nonce]

AsyncIoConsensusJob(candidate) == AsyncIoJob("Consensus", candidate, 0)
AsyncIoCertifiedServeJob(candidate) == AsyncIoJob("Serve", candidate, 0)
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
  /\ envelope.subject \in ValidSubjects
  /\ envelope.chunk \in 0..AsyncChunkCount
  /\ envelope.nonce \in 0..(AsyncIngressCapacity - 1)

AsyncItemTyped(item) ==
  /\ DOMAIN item = {"kind", "source", "envelope"}
  /\ item.kind \in AsyncNetworkKinds
  /\ item.source \in AsyncIngressSources
  /\ (item.kind # "Noise" => item.source \in ValidatorIds)
  /\ item.envelope.recipient \in ValidatorIds
  /\ CASE item.kind = "Proposal" -> item.envelope \in ProposalEnvelopeSet
       [] item.kind \in {"PrepareVote", "CommitVote"} ->
            item.envelope \in VoteEnvelopeSet
       [] item.kind \in {"PrepareQC", "CommitQC"} ->
            item.envelope \in QcEnvelopeSet
       [] item.kind = "TimeoutVote" -> item.envelope \in TimeoutEnvelopeSet
       [] item.kind = "TimeoutCertificate" -> item.envelope \in TcEnvelopeSet
       [] item.kind = "CommitCertificateResponse" ->
            item.envelope \in QcEnvelopeSet
       [] OTHER -> AsyncBodyEnvelopeTyped(item.envelope)

AsyncCandidateTyped(candidate) ==
  /\ DOMAIN candidate =
       {"class", "kind", "node", "height", "view", "subject", "item"}
  /\ candidate.class \in AsyncCommandClasses
  /\ candidate.kind \in AsyncWorkKinds
  /\ candidate.node \in ValidatorIds
  /\ candidate.height \in Heights
  /\ candidate.view \in Views
  /\ candidate.subject \in SubjectOrNone
  /\ (candidate.item = NoAsyncItem \/ AsyncItemTyped(candidate.item))

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
  asyncFifoOwed,
  asyncTimeoutEmitted,
  asyncRunnerPhase,
  asyncRunnerBudget,
  asyncIoQueues,
  asyncOutstandingWork,
  asyncIoReadyCompletions,
  asyncLocalReadyCompletions,
  asyncNextCompletionSource,
  asyncIoControlAvailable,
  asyncDeferredCompletionQueues,
  asyncDeferredProgressQueues,
  asyncDeferredNormalQueues,
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
  asyncHeldChunks

AsyncSchedulerVars ==
  <<asyncNow, asyncCommandQueues, asyncFifoOwed, asyncTimeoutEmitted,
    asyncRunnerPhase, asyncRunnerBudget, asyncIoQueues,
    asyncOutstandingWork, asyncIoReadyCompletions,
    asyncLocalReadyCompletions, asyncNextCompletionSource,
    asyncIoControlAvailable, asyncDeferredCompletionQueues,
    asyncDeferredProgressQueues, asyncDeferredNormalQueues,
    asyncDeferredDrainOwed, asyncCausalQueues, asyncOutstandingTags,
    asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
    asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
    asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>

AsyncAllVars == <<vars, AsyncSchedulerVars>>

AsyncIoVars ==
  <<asyncIoQueues, asyncOutstandingWork, asyncIoReadyCompletions,
    asyncLocalReadyCompletions, asyncNextCompletionSource,
    asyncIoControlAvailable>>

AsyncDeferredVars ==
  <<asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncDeferredDrainOwed>>

HeldChunksFor(node, roundView, subject) ==
  {receipt.chunk:
     receipt \in {entry \in asyncHeldChunks:
       /\ entry.node = node
       /\ entry.view = roundView
       /\ entry.subject = subject}}

AsyncVotersAt(initialContext) ==
  Responsive \cap VotingRoster(initialContext.epoch)

AsyncCurrentResponsiveVoters == Responsive \cap CurrentVoters

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

CandidateScheduled(candidate) ==
  candidate \in QueuedCandidates \cup DeferredCandidates \cup CausalCandidates
    \cup TrackedWorkCandidates

EnqueueCandidate(candidate) ==
  LET node == candidate.node
  IN asyncCommandQueues' =
       [asyncCommandQueues EXCEPT ![node] = Append(@, candidate)]

NodeQueueNonempty(node) == Len(asyncCommandQueues[node]) > 0

NextNodeCommand(node) == Head(asyncCommandQueues[node])

RemoveNextNodeCommand(node) ==
  asyncCommandQueues' =
    [asyncCommandQueues EXCEPT ![node] = Tail(@)]

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
by the exact Core guard.
***************************************************************************)

CausalCandidate(commandClass, kind, command) ==
  NoItemCandidate(commandClass, kind, command.node, command.view,
                  command.subject)

CommandSuccessors(command) ==
  CASE command.kind = "AssembleBody" ->
         <<CausalCandidate("Completion", "BeginProposal", command)>>
    [] command.kind = "BeginProposal" ->
         <<CausalCandidate("Completion", "PersistProposal", command)>>
    [] command.kind = "PersistProposal" ->
         <<CausalCandidate("Completion", "SignProposal", command)>>
    [] command.kind = "DeliverProposal" ->
         <<CausalCandidate("Normal", "BeginPrepare", command)>>
    [] command.kind = "DeliverChunk" ->
         <<CausalCandidate("Completion", "FetchBody", command)>>
    [] command.kind = "FetchBody" ->
         <<CausalCandidate("Completion", "StoreBody", command)>>
    [] command.kind = "FetchCertifiedBody" ->
         <<CausalCandidate("Completion", "StoreBody", command)>>
    [] command.kind = "StoreBody" ->
         <<CausalCandidate("Completion", "ValidateBody", command)>>
    [] command.kind = "ValidateBody" ->
         <<CausalCandidate("Normal", "BeginPrepare", command),
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
         <<CausalCandidate("Completion", "ValidateBody", command),
           CausalCandidate("Progress", "RequestCertifiedBody", command),
           CausalCandidate("Completion", "Apply", command)>>
    [] command.kind = "BeginTimeout" ->
         <<CausalCandidate("Completion", "PersistTimeout", command)>>
    [] command.kind = "PersistTimeout" ->
         <<CausalCandidate("Completion", "SignTimeout", command)>>
    [] command.kind = "DeliverTimeout" ->
         <<CausalCandidate("Progress", "FormTC", command)>>
    [] command.kind = "FormTC" ->
         <<CausalCandidate("Completion", "PersistInstallTC", command)>>
    [] command.kind = "DeliverTC" ->
         <<CausalCandidate("Progress", "BeginInstallTC", command)>>
    [] command.kind = "BeginInstallTC" ->
         <<CausalCandidate("Completion", "PersistInstallTC", command)>>
    [] command.kind = "PersistInstallTC" ->
         <<NoItemCandidate("Normal", "AssembleBody", command.node,
                           command.view + 1, AsyncProposalSubject(command.node))>>
    [] OTHER -> <<>>

AppendCausalSuccessors(command) ==
  asyncCausalQueues' =
    [asyncCausalQueues EXCEPT
       ![command.node] = @ \o CommandSuccessors(command)]

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
     recipient \in CurrentVoters}

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

CertifiedServeCanRespond(request) ==
  /\ request.kind = "CertifiedRequest"
  /\ BodyHeldBy(durableBodies, request.envelope.recipient, context,
                request.envelope.subject)
  /\ \E validation \in validatedBodies:
       /\ validation.node = request.envelope.recipient
       /\ validation.context = context
       /\ validation.subject = request.envelope.subject

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

ItemInQueuedDelivery(item) ==
  \E candidate \in QueuedCandidates: candidate.item = item

IngressLane(recipient, source) == asyncIngressLanes[recipient][source]

IngressLaneDepth(recipient, source) == Len(IngressLane(recipient, source))

IngressDepth(recipient) ==
  Cardinality(
    {pair \in AsyncIngressSources \X (1..AsyncIngressCapacity):
       pair[2] <= IngressLaneDepth(recipient, pair[1])})

EmptyOtherIngressLanes(recipient, source) ==
  {other \in AsyncIngressSources \ {source}:
     IngressLaneDepth(recipient, other) = 0}

IngressUsableCapacity(recipient, source) ==
  AsyncIngressCapacity - Cardinality(EmptyOtherIngressLanes(recipient, source))

CanAdmitIngressItem(item) ==
  IngressDepth(item.envelope.recipient)
    < IngressUsableCapacity(item.envelope.recipient, item.source)

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

ItemInLocalCompletion(item) ==
  \E node \in ValidatorIds:
    \E candidate \in SequenceSet(asyncLocalReadyCompletions[node]):
      candidate.item = item

ItemScheduled(item) ==
  ItemInQueuedDelivery(item) \/ ItemInIngress(item) \/ ItemHasPacket(item)
    \/ ItemInIoServe(item) \/ ItemInLocalCompletion(item)

SendableItems(source) ==
  {item \in asyncRetainedControl: item.source = source}

ActiveRequestItems(source) ==
  {item \in asyncActiveRequests: item.source = source}

ControlClass(item) == item.kind

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

CommitCertificateDiscoveryDue(node) ==
  /\ node \in AsyncCurrentResponsiveVoters
  /\ asyncNow >= AsyncRoundTimeout
  /\ ~NodeHasDecision(node)
  /\ ActiveCommitCertificateRequests(node) = {}
  /\ CommitCertificateRequestOutbox(node) # {}

TimeoutDue(node) ==
  /\ node \in AsyncCurrentResponsiveVoters
  /\ asyncNow >= asyncNodeDeadlines[node]
  /\ ~NodeHasDecision(node)
  /\ ~NodeTimedOut(node, nodeView[node])
  /\ ~asyncTimeoutEmitted[node]
  /\ TimeoutTagPresent(node)

RetransmitTagPresent(node) ==
  "RetransmitElapsed" \notin asyncOutstandingTags[node]

RetransmitDue(node) ==
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
     /\ AssembleLocalBody(command.node, command.subject)
  \/ /\ command.kind = "BeginProposal"
     /\ BeginLocalProposal(command.node, command.subject)
  \/ /\ command.kind = "PersistProposal"
     /\ \E request \in pendingProposal:
          /\ CommandMatches(command, request.node, request.proposal.view,
                            request.proposal.subject)
          /\ PersistProposal(request)
  \/ /\ command.kind = "FetchBody"
     /\ HeldChunksFor(command.node, command.view, command.subject) =
          AsyncChunks
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ FetchBody(command.node, proposal)
  \/ /\ command.kind = "StoreBody"
     /\ StoreBody(command.node, command.subject)
  \/ /\ command.kind = "ValidateBody"
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ ValidateBody(command.node, proposal)
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
     /\ \E qc \in ReceivedQcValues:
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
    asyncHeldChunks
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
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>

ExecuteSignVote(command) ==
  /\ command.kind = "SignVote"
  /\ \E request \in signVotes:
       /\ CommandMatches(command, request.node, request.vote.view,
                         request.vote.subject)
       /\ CompleteVoteSignature(request)
       /\ PublishControlItems(VoteOutbox(request))
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>

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
                    asyncHeldChunks
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
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>

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
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>

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
                 asyncHeldChunks>>

ExecuteRequestCertifiedBody(command) ==
  /\ command.kind = "RequestCertifiedBody"
  /\ ~BodyHeldBy(durableBodies, command.node, context, command.subject)
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
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>

ExecuteApply(command) ==
  /\ command.kind = "Apply"
  /\ \E qc \in DecisionQcValues:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ ApplyDecision(command.node, qc)
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
                    asyncHeldChunks
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
                    asyncIngressLanes, asyncIngressReady
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
                    asyncHeldChunks
                    >>

ExecuteCommand(command) ==
  \/ ExecuteRegularCommand(command)
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
              /\ AsyncCompletionLoad(node) < AsyncIoWorkCapacity
        \/ /\ candidate.class # "Completion"
              /\ CanEnqueueClass(node, candidate.class)

DiscardCommand(command) ==
  /\ UNCHANGED vars
  /\ UNCHANGED <<asyncSentItems, asyncRetainedControl,
                 asyncActiveRequests>>
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncTransport,
                 asyncIngressLanes, asyncIngressReady,
                 asyncHeldChunks
                 >>

CommandDispatchable(command) ==
  /\ ENABLED ExecuteCommand(command)
  /\ NodeIdle(command.node) \/ command.class = "Completion"

DeferredProgressRank(command) ==
  IF command.kind = "DeliverQC" /\ command.item.kind = "CommitQC"
  THEN 3
  ELSE IF command.kind = "DeliverTC"
       THEN 2
       ELSE IF command.kind = "DeliverQC"
            THEN 1
            ELSE 0

SequenceWithoutIndex(sequence, index) ==
  SubSeq(sequence, 1, index - 1)
    \o SubSeq(sequence, index + 1, Len(sequence))

ReplaceableProgressIndices(node, command) ==
  {index \in 1..Len(asyncDeferredProgressQueues[node]):
     DeferredProgressRank(asyncDeferredProgressQueues[node][index])
       <= DeferredProgressRank(command)}

FirstReplaceableProgressIndex(node, command) ==
  CHOOSE index \in ReplaceableProgressIndices(node, command):
    \A other \in ReplaceableProgressIndices(node, command): index <= other

DeferredProgressAfter(node, command) ==
  LET queue == asyncDeferredProgressQueues[node]
  IN IF command \in SequenceSet(queue)
     THEN queue
     ELSE IF Len(queue) < AsyncDeferredProgressCapacity
          THEN Append(queue, command)
          ELSE IF ReplaceableProgressIndices(node, command) # {}
               THEN Append(
                      SequenceWithoutIndex(
                        queue, FirstReplaceableProgressIndex(node, command)),
                      command)
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
     /\ UNCHANGED <<asyncDeferredDrainOwed, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport, asyncIngressLanes,
                    asyncIngressReady, asyncHeldChunks
                    >>

DeferredQueueNonempty(node) ==
  Len(asyncDeferredCompletionQueues[node]) > 0
    \/ Len(asyncDeferredProgressQueues[node]) > 0
    \/ Len(asyncDeferredNormalQueues[node]) > 0

NextDeferredCommand(node) ==
  IF Len(asyncDeferredCompletionQueues[node]) > 0
  THEN Head(asyncDeferredCompletionQueues[node])
  ELSE IF Len(asyncDeferredProgressQueues[node]) > 0
       THEN Head(asyncDeferredProgressQueues[node])
       ELSE Head(asyncDeferredNormalQueues[node])

RemoveNextDeferredCommand(node) ==
  IF Len(asyncDeferredCompletionQueues[node]) > 0
  THEN /\ asyncDeferredCompletionQueues' =
             [asyncDeferredCompletionQueues EXCEPT ![node] = Tail(@)]
       /\ UNCHANGED <<asyncDeferredProgressQueues,
                      asyncDeferredNormalQueues>>
  ELSE IF Len(asyncDeferredProgressQueues[node]) > 0
       THEN /\ asyncDeferredProgressQueues' =
                  [asyncDeferredProgressQueues EXCEPT ![node] = Tail(@)]
            /\ UNCHANGED <<asyncDeferredCompletionQueues,
                           asyncDeferredNormalQueues>>
       ELSE /\ asyncDeferredNormalQueues' =
                  [asyncDeferredNormalQueues EXCEPT ![node] = Tail(@)]
            /\ UNCHANGED <<asyncDeferredCompletionQueues,
                           asyncDeferredProgressQueues>>

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

DeliveryClass(item) ==
  IF item.kind \in {"PrepareQC", "CommitQC", "TimeoutCertificate",
                    "Chunk", "CertifiedResponse",
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
  IN /\ DueSourcePackets(recipient, source) # {}
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
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncHeldChunks
                    >>

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

CertifiedResponseAuthorized(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ \E decision \in decisions:
       /\ decision.node = item.envelope.recipient
       /\ decision.qc.context = context
       /\ decision.qc.view = item.envelope.view
       /\ decision.qc.subject = item.envelope.subject
       /\ decision.qc.phase = "Commit"
       /\ item.source \in decision.qc.signers

CommitCertificateRequestAuthorized(item) ==
  /\ item.kind = "CommitCertificateRequest"
  /\ item.source \in CurrentVoters
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

MatchingCertifiedRequests(response) ==
  {request \in asyncActiveRequests:
     /\ request.kind = "CertifiedRequest"
     /\ request.source = response.envelope.recipient
     /\ request.envelope.height = response.envelope.height
     /\ request.envelope.view = response.envelope.view
     /\ request.envelope.subject = response.envelope.subject}

IngressSourceCanDrain(node, source) ==
  LET item == Head(IngressLane(node, source))
      candidate == DeliveryCandidate(item)
  IN item.kind = "Noise"
       \/ item \notin asyncSentItems
       \/ IF item.kind \in {"CertifiedRequest",
                             "CommitCertificateRequest"}
          THEN \/ ~(IF item.kind = "CertifiedRequest"
                    THEN CertifiedRequestAuthorized(item)
                    ELSE CommitCertificateRequestAuthorized(item))
               \/ CanEnqueueIoClass(node, "Serve")
          ELSE IF item.kind = "CertifiedResponse"
               THEN \/ ~CertifiedResponseAuthorized(item)
                    \/ /\ AsyncCompletionLoad(node) < AsyncIoWorkCapacity
                          /\ ~CandidateInFlight(
                               CertifiedResponseCandidate(item))
               ELSE IF item.kind = "CommitCertificateResponse"
                    THEN \/ ~CommitCertificateResponseAuthorized(item)
                         \/ /\ CanEnqueueClass(node, "Progress")
                               /\ ~CandidateInFlight(
                                    CommitCertificateResponseCandidate(item))
               ELSE CanEnqueueClass(node, candidate.class)

IngressHeadCanDrain(node) ==
  IngressSourceCanDrain(node, HeadIngressSource(node))

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

PopSelectedIngress(node, index) ==
  LET source == asyncIngressReady[node][index]
  IN /\ index \in 1..Len(asyncIngressReady[node])
     /\ asyncIngressLanes' =
          [asyncIngressLanes EXCEPT ![node][source] = Tail(@)]
     /\ asyncIngressReady' =
          [asyncIngressReady EXCEPT
             ![node] = ReadyAfterSelectedDrain(node, index)]

DrainFairIngressSelected(node) ==
  LET index == FirstDrainableIngressIndex(node)
      source == asyncIngressReady[node][index]
      item == IngressItemAt(node, index)
      candidate == DeliveryCandidate(item)
  IN /\ asyncIngressReady[node] # <<>>
     /\ DrainableIngressIndices(node) # {}
     /\ PopSelectedIngress(node, index)
     /\ IF item.kind = "Noise" \/ item \notin asyncSentItems
        THEN /\ UNCHANGED asyncCommandQueues
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
                                  @, AsyncIoCertifiedServeJob(candidate))]
                       /\ UNCHANGED <<asyncOutstandingWork,
                                       asyncIoReadyCompletions,
                                       asyncLocalReadyCompletions,
                                       asyncNextCompletionSource,
                                       asyncIoControlAvailable>>
                       /\ UNCHANGED <<asyncCommandQueues, asyncSentItems,
                                      asyncRetainedControl,
                                      asyncActiveRequests>>
                  ELSE /\ UNCHANGED <<asyncCommandQueues, AsyncIoVars>>
                       /\ UNCHANGED <<asyncSentItems,
                                      asyncRetainedControl,
                                      asyncActiveRequests>>
             ELSE IF item.kind = "CertifiedResponse"
                  THEN IF CertifiedResponseAuthorized(item)
                       THEN LET completion ==
                                  CertifiedResponseCandidate(item)
                            IN /\ asyncLocalReadyCompletions' =
                                     [asyncLocalReadyCompletions EXCEPT
                                        ![node] = Append(@, completion)]
                               /\ asyncOutstandingWork' =
                                     [asyncOutstandingWork EXCEPT
                                        ![node] = @ \cup {completion}]
                               /\ UNCHANGED <<asyncIoQueues,
                                               asyncIoReadyCompletions,
                                               asyncNextCompletionSource,
                                               asyncIoControlAvailable,
                                               asyncCommandQueues>>
                               /\ asyncActiveRequests' =
                                    asyncActiveRequests \
                                      MatchingCertifiedRequests(item)
                               /\ UNCHANGED <<asyncSentItems,
                                              asyncRetainedControl>>
                       ELSE /\ UNCHANGED <<asyncCommandQueues, AsyncIoVars>>
                            /\ UNCHANGED <<asyncSentItems,
                                           asyncRetainedControl,
                                           asyncActiveRequests>>
                  ELSE IF item.kind = "CommitCertificateResponse"
                       THEN IF CommitCertificateResponseAuthorized(item)
                            THEN LET discovered ==
                                       DiscoveredCommitQcItem(item)
                                     discoveredCandidate ==
                                       CommitCertificateResponseCandidate(item)
                                 IN /\ EnqueueCandidate(discoveredCandidate)
                                    /\ UNCHANGED AsyncIoVars
                                    /\ asyncActiveRequests' =
                                         asyncActiveRequests \
                                           MatchingCommitCertificateRequests(item)
                                    /\ asyncSentItems' =
                                         asyncSentItems \cup {discovered}
                                    /\ UNCHANGED asyncRetainedControl
                            ELSE /\ UNCHANGED <<asyncCommandQueues,
                                                AsyncIoVars>>
                                 /\ UNCHANGED <<asyncSentItems,
                                                asyncRetainedControl,
                                                asyncActiveRequests>>
                  ELSE /\ EnqueueCandidate(candidate)
                       /\ UNCHANGED AsyncIoVars
                       /\ UNCHANGED <<asyncSentItems,
                                      asyncRetainedControl,
                                      asyncActiveRequests>>
     /\ UNCHANGED <<vars, asyncFifoOwed, asyncTimeoutEmitted,
                    asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines, asyncTransport,
                    asyncHeldChunks
                    >>

(***************************************************************************
After Apply, the production height loop exits immediately.  Its successor
loop still drains the shared ingress and serves immutable Kura finality/body
artifacts, but it must not execute or retransmit old-height consensus work.
The historical runner below therefore rejects every old-height head except
the two authenticated recovery request classes, which enter the same bounded
Serve reservation as the live-height implementation.
***************************************************************************)

HistoricalIngressSourceCanDrain(node, source) ==
  LET item == Head(IngressLane(node, source))
  IN IF item.kind = "CertifiedRequest"
     THEN \/ ~CertifiedRequestAuthorized(item)
          \/ CanEnqueueIoClass(node, "Serve")
     ELSE IF item.kind = "CommitCertificateRequest"
          THEN \/ ~CommitCertificateRequestAuthorized(item)
               \/ CanEnqueueIoClass(node, "Serve")
          ELSE TRUE

HistoricalDrainableIngressIndices(node) ==
  {index \in 1..Len(asyncIngressReady[node]):
     HistoricalIngressSourceCanDrain(
       node, asyncIngressReady[node][index])}

FirstHistoricalDrainableIngressIndex(node) ==
  CHOOSE index \in HistoricalDrainableIngressIndices(node):
    \A other \in HistoricalDrainableIngressIndices(node): index <= other

DrainHistoricalIngressSelected(node) ==
  LET index == FirstHistoricalDrainableIngressIndex(node)
      item == IngressItemAt(node, index)
      candidate == DeliveryCandidate(item)
      authorizedRequest ==
        \/ /\ item.kind = "CertifiedRequest"
              /\ item \in asyncSentItems
              /\ CertifiedRequestAuthorized(item)
        \/ /\ item.kind = "CommitCertificateRequest"
              /\ item \in asyncSentItems
              /\ CommitCertificateRequestAuthorized(item)
  IN /\ HistoricalDrainableIngressIndices(node) # {}
     /\ PopSelectedIngress(node, index)
     /\ IF authorizedRequest
        THEN /\ asyncIoQueues' =
                   [asyncIoQueues EXCEPT
                      ![node] = Append(
                        @, AsyncIoCertifiedServeJob(candidate))]
             /\ UNCHANGED <<asyncOutstandingWork,
                             asyncIoReadyCompletions,
                             asyncLocalReadyCompletions,
                             asyncNextCompletionSource,
                             asyncIoControlAvailable>>
        ELSE UNCHANGED AsyncIoVars
     /\ UNCHANGED <<vars, asyncCommandQueues, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncDeferredVars,
                    asyncCausalQueues, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncSentItems, asyncRetainedControl,
                    asyncActiveRequests, asyncTransport, asyncHeldChunks>>

AdmitCausalHead(node) ==
  LET candidate == HeadCausalCandidate(node)
      duplicate == CandidateInFlight(candidate)
  IN /\ CausalHeadCanAdvance(node)
     /\ asyncCausalQueues' =
          [asyncCausalQueues EXCEPT ![node] = Tail(@)]
     /\ IF duplicate
        THEN /\ UNCHANGED asyncCommandQueues
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
                    asyncHeldChunks>>

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

AdmitProducerCompletion(node) ==
  LET source == SelectedCompletionSource(node)
      candidate == SelectedCompletionCandidate(node)
  IN /\ ProducerCompletionCanAdmit(node)
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
                    asyncHeldChunks>>

ServiceIoWorker(node) ==
  LET job == Head(asyncIoQueues[node])
      responseItems ==
        IF job.class # "Serve"
        THEN {}
        ELSE IF CertifiedServeCanRespond(job.candidate.item)
             THEN {CertifiedResponseItem(job.candidate.item)}
             ELSE IF CommitCertificateServeCanRespond(job.candidate.item)
                  THEN CommitCertificateResponseItems(job.candidate.item)
                  ELSE {}
  IN /\ node \in AsyncCurrentResponsiveVoters
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
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget,
                    asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks>>

EnqueueIoLocalControl(node) ==
  /\ node \in AsyncCurrentResponsiveVoters
  /\ ~NodeHasApplication(node)
  /\ asyncIoControlAvailable[node]
  /\ CanEnqueueIoClass(node, "Control")
  /\ asyncIoQueues' =
       [asyncIoQueues EXCEPT ![node] = Append(@, AsyncIoControlJob)]
  /\ asyncIoControlAvailable' =
       [asyncIoControlAvailable EXCEPT ![node] = FALSE]
  /\ UNCHANGED AsyncDeferredVars
  /\ LeaveCausalQueues
  /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues, asyncFifoOwed,
                 asyncTimeoutEmitted, asyncRunnerPhase, asyncRunnerBudget,
                 asyncOutstandingWork, asyncIoReadyCompletions,
                 asyncLocalReadyCompletions, asyncNextCompletionSource,
                 asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncNodeServiceDeadlines,
                 asyncIoServiceDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>

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

DirectCommitCertificateDiscoveryStep(node) ==
  /\ CommitCertificateDiscoveryDue(node)
  /\ UNCHANGED <<vars, asyncCommandQueues, asyncFifoOwed,
                 asyncTimeoutEmitted, AsyncDeferredVars,
                 asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncIngressLanes,
                 asyncIngressReady, asyncHeldChunks>>
  /\ PublishCommitCertificateRequests(
       CommitCertificateRequestOutbox(node))
  /\ LeaveCausalQueues

DirectTimeoutStep(node) ==
  /\ TimeoutDue(node)
  /\ asyncTimeoutEmitted' =
       [asyncTimeoutEmitted EXCEPT ![node] = TRUE]
  /\ asyncFifoOwed' =
       [asyncFifoOwed EXCEPT ![node] = NodeQueueNonempty(node)]
  /\ IF ENABLED BeginTimeout(node)
     THEN /\ BeginTimeout(node)
          /\ UNCHANGED asyncOutstandingTags
     ELSE /\ UNCHANGED vars
          /\ asyncOutstandingTags' =
               [asyncOutstandingTags EXCEPT
                  ![node] = @ \cup {"TimeoutElapsed"}]
  /\ IF ENABLED BeginTimeout(node)
     THEN AppendCausalSuccessors(TimeoutCausalCommand(node))
     ELSE LeaveCausalQueues
  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues, asyncDeferredNormalQueues>>
  /\ asyncDeferredDrainOwed' =
       IF ENABLED BeginTimeout(node)
       THEN [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
       ELSE asyncDeferredDrainOwed
  /\ UNCHANGED <<asyncCommandQueues, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>

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
                 asyncDeferredProgressQueues, asyncDeferredNormalQueues>>
  /\ asyncDeferredDrainOwed' =
       IF NodeIdle(node)
       THEN [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
       ELSE asyncDeferredDrainOwed
  /\ LeaveCausalQueues
  /\ UNCHANGED <<vars, asyncCommandQueues, asyncTimeoutEmitted,
                 asyncNodeDeadlines, asyncIngressLanes,
                 asyncIngressReady, asyncHeldChunks
                 >>

DeferredTimeoutExecutable(node) ==
  /\ "TimeoutElapsed" \in asyncOutstandingTags[node]
  /\ \/ ENABLED BeginTimeout(node)
     \/ NodeHasDecision(node)
     \/ NodeTimedOut(node, nodeView[node])

DeferredTimeoutStep(node) ==
  /\ DeferredTimeoutExecutable(node)
  /\ IF ENABLED BeginTimeout(node)
     THEN BeginTimeout(node)
     ELSE UNCHANGED vars
  /\ IF ENABLED BeginTimeout(node)
     THEN AppendCausalSuccessors(TimeoutCausalCommand(node))
     ELSE LeaveCausalQueues
  /\ asyncOutstandingTags' =
       [asyncOutstandingTags EXCEPT ![node] = @ \ {"TimeoutElapsed"}]
  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues, asyncDeferredNormalQueues>>
  /\ asyncDeferredDrainOwed' =
       [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
  /\ UNCHANGED <<asyncCommandQueues, asyncFifoOwed,
                 asyncTimeoutEmitted, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>

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
                 asyncDeferredProgressQueues, asyncDeferredNormalQueues>>
  /\ asyncDeferredDrainOwed' =
       [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
  /\ LeaveCausalQueues
  /\ UNCHANGED <<asyncCommandQueues, asyncFifoOwed,
                 asyncTimeoutEmitted, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>

DeferredTagExecutable(node) ==
  DeferredTimeoutExecutable(node)
    \/ (/\ "TimeoutElapsed" \notin asyncOutstandingTags[node]
        /\ "RetransmitElapsed" \in asyncOutstandingTags[node]
        /\ NodeIdle(node))

DeferredTagStep(node) ==
  IF DeferredTimeoutExecutable(node)
  THEN DeferredTimeoutStep(node)
  ELSE DeferredRetransmitStep(node)

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
                            asyncDeferredNormalQueues>>
             /\ asyncDeferredDrainOwed' =
                  [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
        ELSE IF ~NodeIdle(node)
             THEN /\ DeferCommand(command)
                  /\ LeaveCausalQueues
             ELSE /\ DiscardCommand(command)
                  /\ LeaveCausalQueues
                  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                                 asyncDeferredProgressQueues,
                                 asyncDeferredNormalQueues>>
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
     THEN /\ UNCHANGED <<vars, asyncCommandQueues, asyncFifoOwed,
                         asyncTimeoutEmitted, asyncDeferredCompletionQueues,
                         asyncDeferredProgressQueues,
                         asyncDeferredNormalQueues, asyncOutstandingTags,
                         asyncNodeDeadlines, asyncRetransmitDeadlines,
                         asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport, asyncIngressLanes,
                         asyncIngressReady, asyncHeldChunks>>
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
                  /\ UNCHANGED <<asyncCommandQueues, asyncFifoOwed>>
             ELSE IF ~NodeIdle(node)
                  THEN /\ LeaveCausalQueues
                       /\ UNCHANGED <<vars, asyncCommandQueues,
                                      asyncFifoOwed, asyncTimeoutEmitted,
                                      asyncDeferredCompletionQueues,
                                      asyncDeferredProgressQueues,
                                      asyncDeferredNormalQueues,
                                      asyncOutstandingTags,
                                      asyncNodeDeadlines,
                                      asyncRetransmitDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests,
                                      asyncTransport, asyncIngressLanes,
                                      asyncIngressReady, asyncHeldChunks>>
                       /\ asyncDeferredDrainOwed' =
                            [asyncDeferredDrainOwed EXCEPT ![node] = FALSE]
                  ELSE /\ RemoveNextDeferredCommand(node)
                       /\ DiscardCommand(command)
                       /\ LeaveCausalQueues
                       /\ asyncDeferredDrainOwed' = asyncDeferredDrainOwed
                       /\ UNCHANGED <<asyncCommandQueues, asyncFifoOwed,
                                      asyncTimeoutEmitted>>

IdleRuntimeStep(node) ==
  /\ UNCHANGED <<vars, asyncCommandQueues, asyncTimeoutEmitted,
                 AsyncDeferredVars,
                 asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>
  /\ LeaveCausalQueues
  /\ asyncFifoOwed' = [asyncFifoOwed EXCEPT ![node] = FALSE]

RuntimeStep(node) ==
  \/ /\ CommitCertificateDiscoveryDue(node)
     /\ DirectCommitCertificateDiscoveryStep(node)
  \/ /\ ~CommitCertificateDiscoveryDue(node)
        /\ asyncDeferredDrainOwed[node]
     /\ DeferredDrainStep(node)
  \/ /\ ~CommitCertificateDiscoveryDue(node)
        /\ ~asyncDeferredDrainOwed[node]
        /\ DeferredTagExecutable(node)
     /\ DeferredTagStep(node)
  \/ /\ ~CommitCertificateDiscoveryDue(node)
        /\ ~asyncDeferredDrainOwed[node]
        /\ ~DeferredTagExecutable(node)
        /\ TimeoutDue(node)
        /\ DirectTimeoutStep(node)
  \/ /\ ~CommitCertificateDiscoveryDue(node)
        /\ ~asyncDeferredDrainOwed[node]
        /\ ~DeferredTagExecutable(node)
        /\ ~TimeoutDue(node)
        /\ NodeQueueNonempty(node)
        /\ asyncFifoOwed[node]
        /\ FifoRuntimeStep(node)
  \/ /\ ~CommitCertificateDiscoveryDue(node)
        /\ ~asyncDeferredDrainOwed[node]
        /\ ~DeferredTagExecutable(node)
        /\ ~TimeoutDue(node)
        /\ ~(NodeQueueNonempty(node) /\ asyncFifoOwed[node])
        /\ RetransmitDue(node)
        /\ DirectRetransmitStep(node)
  \/ /\ ~CommitCertificateDiscoveryDue(node)
        /\ ~asyncDeferredDrainOwed[node]
        /\ ~DeferredTagExecutable(node)
        /\ ~TimeoutDue(node)
        /\ ~(NodeQueueNonempty(node) /\ asyncFifoOwed[node])
        /\ ~RetransmitDue(node)
        /\ NodeQueueNonempty(node)
        /\ FifoRuntimeStep(node)
  \/ /\ ~CommitCertificateDiscoveryDue(node)
        /\ ~asyncDeferredDrainOwed[node]
        /\ ~DeferredTagExecutable(node)
        /\ ~TimeoutDue(node)
        /\ ~RetransmitDue(node)
        /\ ~NodeQueueNonempty(node)
        /\ IdleRuntimeStep(node)

LocalAdmissionStep(node) ==
  /\ asyncRunnerPhase[node] = "Local"
  /\ UNCHANGED AsyncDeferredVars
  /\ IF asyncRunnerBudget[node] > 0 /\ ProducerCompletionCanAdmit(node)
     THEN /\ AdmitProducerCompletion(node)
          /\ LeaveCausalQueues
          /\ asyncRunnerPhase' = asyncRunnerPhase
          /\ asyncRunnerBudget' =
               [asyncRunnerBudget EXCEPT ![node] = @ - 1]
     ELSE IF asyncRunnerBudget[node] > 0 /\ CausalHeadCanAdvance(node)
          THEN /\ AdmitCausalHead(node)
               /\ asyncRunnerPhase' = asyncRunnerPhase
               /\ asyncRunnerBudget' =
                    [asyncRunnerBudget EXCEPT ![node] = @ - 1]
          ELSE /\ LeaveCausalQueues
               /\ UNCHANGED <<vars, asyncCommandQueues,
                               asyncFifoOwed, asyncTimeoutEmitted,
                               AsyncIoVars, asyncOutstandingTags,
                               asyncNodeDeadlines,
                               asyncRetransmitDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests,
                               asyncTransport, asyncIngressLanes,
                               asyncIngressReady, asyncHeldChunks>>
               /\ asyncRunnerPhase' =
                    [asyncRunnerPhase EXCEPT ![node] = "Ingress"]
               /\ asyncRunnerBudget' =
                    [asyncRunnerBudget EXCEPT
                       ![node] = AsyncIngressCapacity]

IngressDrainStep(node) ==
  /\ asyncRunnerPhase[node] = "Ingress"
  /\ UNCHANGED AsyncDeferredVars
  /\ LeaveCausalQueues
  /\ IF asyncRunnerBudget[node] > 0
          /\ asyncIngressReady[node] # <<>>
          /\ DrainableIngressIndices(node) # {}
     THEN /\ DrainFairIngressSelected(node)
          /\ asyncRunnerPhase' = asyncRunnerPhase
          /\ asyncRunnerBudget' =
               [asyncRunnerBudget EXCEPT ![node] = @ - 1]
     ELSE /\ UNCHANGED <<vars, asyncCommandQueues, asyncFifoOwed,
                         asyncTimeoutEmitted, AsyncIoVars,
                         asyncOutstandingTags,
                         asyncNodeDeadlines, asyncRetransmitDeadlines,
                         asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncTransport, asyncIngressLanes,
                         asyncIngressReady, asyncHeldChunks
                         >>
          /\ asyncRunnerPhase' =
               [asyncRunnerPhase EXCEPT ![node] = "Runtime"]
          /\ asyncRunnerBudget' =
               [asyncRunnerBudget EXCEPT ![node] = 1]

SerializedRuntimeStep(node) ==
  /\ asyncRunnerPhase[node] = "Runtime"
  /\ UNCHANGED AsyncIoVars
  /\ RuntimeStep(node)
  /\ asyncRunnerPhase' = [asyncRunnerPhase EXCEPT ![node] = "Local"]
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = AsyncQueueCapacity]

RunNode(node) ==
  /\ node \in AsyncCurrentResponsiveVoters
  /\ ~NodeHasApplication(node)
  /\ \/ LocalAdmissionStep(node)
     \/ IngressDrainStep(node)
     \/ SerializedRuntimeStep(node)
  /\ UNCHANGED asyncNow
  /\ asyncNodeServiceDeadlines' =
       [asyncNodeServiceDeadlines EXCEPT
          ![node] = asyncNow + AsyncDeliveryBound]
  /\ UNCHANGED asyncIoServiceDeadlines

HistoricalIdleStep ==
  /\ UNCHANGED <<vars, asyncCommandQueues, asyncFifoOwed,
                 asyncTimeoutEmitted, asyncRunnerPhase,
                 asyncRunnerBudget, AsyncIoVars, AsyncDeferredVars,
                 asyncCausalQueues, asyncOutstandingTags,
                 asyncNodeDeadlines, asyncRetransmitDeadlines,
                 asyncSentItems, asyncRetainedControl,
                 asyncActiveRequests, asyncTransport,
                 asyncIngressLanes, asyncIngressReady,
                 asyncHeldChunks>>

RunHistoricalServer(node) ==
  /\ node \in AsyncCurrentResponsiveVoters
  /\ NodeHasApplication(node)
  /\ IF HistoricalDrainableIngressIndices(node) # {}
     THEN DrainHistoricalIngressSelected(node)
     ELSE HistoricalIdleStep
  /\ UNCHANGED asyncNow
  /\ asyncNodeServiceDeadlines' =
       [asyncNodeServiceDeadlines EXCEPT
          ![node] = asyncNow + AsyncDeliveryBound]
  /\ UNCHANGED asyncIoServiceDeadlines

AsyncSetGST ==
  /\ ~gst
  /\ SetGST
  /\ UNCHANGED AsyncSchedulerVars

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
  /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues, asyncFifoOwed,
                 asyncTimeoutEmitted, asyncRunnerPhase, asyncRunnerBudget,
                 AsyncIoVars, asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncNodeServiceDeadlines,
                 asyncIoServiceDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncIngressLanes,
                 asyncIngressReady, asyncHeldChunks
                 >>

PreGstCrash(node) ==
  /\ ~gst
  /\ node \notin Responsive
  /\ Crash(node)
  /\ UNCHANGED <<AsyncSchedulerVars>>

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
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncSentItems, asyncRetainedControl, asyncActiveRequests, asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks
                    >>

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
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks
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
     /\ UNCHANGED <<vars, asyncNow, asyncCommandQueues, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks
                    >>

AsyncByzantineProposal(signer, roundView, subject,
                       justifyRank, justifySubject) ==
  LET proposal == Proposal(context, roundView, subject, signer,
                           justifyRank, justifySubject)
  IN /\ ByzantineBroadcastProposal(signer, roundView, subject,
                                    justifyRank, justifySubject)
     /\ PublishEphemeralItems(ByzantineProposalOutbox(signer, proposal))
     /\ UNCHANGED <<asyncNow, asyncCommandQueues, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, AsyncDeferredVars,
                    asyncCausalQueues, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks
                    >>

AsyncByzantineVote(signer, roundView, phase, subject) ==
  LET vote == Vote(context, roundView, phase, subject, signer)
  IN /\ ByzantineBroadcastVote(signer, roundView, phase, subject)
     /\ PublishEphemeralItems(ByzantineVoteOutbox(signer, vote))
     /\ UNCHANGED <<asyncNow, asyncCommandQueues, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, AsyncDeferredVars,
                    asyncCausalQueues, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks
                    >>

AsyncByzantineTimeout(signer, roundView, highRank, highSubject) ==
  LET vote == TimeoutVote(context, roundView, signer, highRank, highSubject)
  IN /\ ByzantineBroadcastTimeout(signer, roundView, highRank, highSubject)
     /\ PublishEphemeralItems(ByzantineTimeoutOutbox(signer, vote))
     /\ UNCHANGED <<asyncNow, asyncCommandQueues, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncRunnerPhase,
                    asyncRunnerBudget, AsyncIoVars, AsyncDeferredVars,
                    asyncCausalQueues, asyncOutstandingTags,
                    asyncNodeDeadlines, asyncRetransmitDeadlines,
                    asyncNodeServiceDeadlines, asyncIoServiceDeadlines,
                    asyncIngressLanes, asyncIngressReady,
                    asyncHeldChunks
                    >>

AsyncFaultStep ==
  \/ \E packet \in asyncTransport: PreGstLosePacket(packet)
  \/ \E node \in ValidatorIds: PreGstCrash(node)
  \/ \E source \in AsyncIngressSources, recipient \in ValidatorIds,
       nonce \in 0..(AsyncIngressCapacity - 1):
       InjectByzantineNoise(source, recipient, nonce)
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
    AdmitHiddenPacket(recipient, source)

OverdueResponsivePackets ==
  {packet \in asyncTransport:
     /\ packet.item.source \in AsyncCurrentResponsiveVoters
     /\ packet.item.envelope.recipient \in AsyncCurrentResponsiveVoters
     /\ packet.deadline <= asyncNow}

AsyncTickEnabled ==
  \/ ~gst
  \/ /\ gst
     /\ OverdueResponsivePackets = {}
     /\ \A node \in AsyncCurrentResponsiveVoters:
          /\ asyncNodeServiceDeadlines[node] > asyncNow
          /\ \/ AsyncIoQueueDepth(node) = 0
             \/ asyncIoServiceDeadlines[node] > asyncNow

AsyncNonClockVars ==
  <<vars, asyncCommandQueues, asyncFifoOwed, asyncTimeoutEmitted,
    asyncRunnerPhase, asyncRunnerBudget, AsyncIoVars,
    asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncDeferredDrainOwed, asyncCausalQueues,
    asyncOutstandingTags, asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncNodeServiceDeadlines, asyncIoServiceDeadlines, asyncSentItems, asyncRetainedControl, asyncActiveRequests,
    asyncTransport, asyncIngressLanes, asyncIngressReady,
    asyncHeldChunks>>

AsyncTick ==
  /\ AsyncTickEnabled
  /\ asyncNow' = asyncNow + 1
  /\ UNCHANGED AsyncNonClockVars

AsyncRunnerStep ==
  \/ (\E node \in AsyncCurrentResponsiveVoters: RunNode(node))
  \/ (\E node \in AsyncCurrentResponsiveVoters:
        RunHistoricalServer(node))

AsyncNonRunnerStep ==
  /\ \/ AsyncSetGST
     \/ AsyncTick
     \/ (\E node \in AsyncCurrentResponsiveVoters: ServiceIoWorker(node))
     \/ (\E node \in AsyncCurrentResponsiveVoters:
           EnqueueIoLocalControl(node))
     \/ AsyncNetworkStep
     \/ AsyncFaultStep
  /\ UNCHANGED asyncNodeServiceDeadlines

AsyncNonCrashStep ==
  /\ (AsyncRunnerStep \/ AsyncNonRunnerStep)
  /\ UNCHANGED up

AsyncNext ==
  /\ (AsyncNonCrashStep
        \/ (\E node \in ValidatorIds: PreGstCrash(node)))
  /\ UNCHANGED <<height, context>>
  /\ [Next]_vars

PostGstRunNode(node) == gst /\ RunNode(node)

PostGstRunHistoricalServer(node) == gst /\ RunHistoricalServer(node)

PostGstServiceIoWorker(node) == gst /\ ServiceIoWorker(node)

PostGstAdmitHiddenPacket(recipient, source) ==
  gst /\ AdmitHiddenPacket(recipient, source)

AsyncFairnessAt(initialContext) ==
  /\ WF_AsyncAllVars(AsyncSetGST)
  /\ WF_AsyncAllVars(AsyncTick)
  /\ \A node \in AsyncVotersAt(initialContext):
       WF_AsyncAllVars(PostGstRunNode(node))
  /\ \A node \in AsyncVotersAt(initialContext):
       WF_AsyncAllVars(PostGstRunHistoricalServer(node))
  /\ \A node \in AsyncVotersAt(initialContext):
       WF_AsyncAllVars(PostGstServiceIoWorker(node))
  /\ \A recipient \in AsyncVotersAt(initialContext),
       source \in AsyncVotersAt(initialContext):
       WF_AsyncAllVars(PostGstAdmitHiddenPacket(recipient, source))

AsyncFairness == AsyncFairnessAt(ContextRecord(0, <<>>))

(***************************************************************************
Initialization, invariants, refinement boundary, and release properties.
***************************************************************************)

AsyncRuntimeInit ==
  /\ asyncNow = 0
  /\ asyncCommandQueues = [node \in ValidatorIds |-> <<>>]
  /\ asyncFifoOwed = [node \in ValidatorIds |-> FALSE]
  /\ asyncTimeoutEmitted = [node \in ValidatorIds |-> FALSE]
  /\ asyncRunnerPhase = [node \in ValidatorIds |-> "Local"]
  /\ asyncRunnerBudget =
       [node \in ValidatorIds |-> AsyncQueueCapacity]
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

AsyncIngressInit ==
  /\ asyncIngressLanes =
       [recipient \in ValidatorIds |->
          [source \in AsyncIngressSources |-> <<>>]]
  /\ asyncIngressReady = [recipient \in ValidatorIds |-> <<>>]

AsyncBaseInitAt(initialContext) ==
  /\ InitAt(initialContext)
  /\ AsyncConfiguration
  /\ AsyncRuntimeInit
  /\ AsyncIoInit
  /\ AsyncDeferredInit
  /\ AsyncTransportInit
  /\ AsyncIngressInit

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

AsyncRuntimeScalarTypeInvariant ==
  /\ AsyncConfiguration
  /\ asyncNow \in Nat
  /\ DOMAIN asyncCommandQueues = ValidatorIds
  /\ \A node \in ValidatorIds:
       AsyncQueueTyped(asyncCommandQueues[node])
  /\ asyncFifoOwed \in [ValidatorIds -> BOOLEAN]
  /\ asyncTimeoutEmitted \in [ValidatorIds -> BOOLEAN]
  /\ asyncRunnerPhase \in
       [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
  /\ asyncRunnerBudget \in
       [ValidatorIds -> 0..(AsyncQueueCapacity + AsyncIngressCapacity)]

AsyncCausalTypeInvariant ==
  /\ DOMAIN asyncCausalQueues = ValidatorIds
  /\ \A node \in ValidatorIds: AsyncQueueTyped(asyncCausalQueues[node])

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

AsyncIoContentTypeInvariant ==
  /\ AsyncIoQueueContentTypeInvariant
  /\ AsyncIoWorkContentTypeInvariant

AsyncIoCapacityTypeInvariant ==
  \A node \in ValidatorIds:
       /\ AsyncQueueDepth(node) <= AsyncQueueCapacity
       /\ AsyncCompletionLoad(node) <= AsyncCompletionReserve
       /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
       /\ AsyncCompletionLoad(node) <= AsyncIoWorkCapacity

AsyncIoTypeInvariant ==
  /\ AsyncIoTopologyTypeInvariant
  /\ AsyncIoContentTypeInvariant
  /\ AsyncIoCapacityTypeInvariant

AsyncDeferredTopologyTypeInvariant ==
  /\ DOMAIN asyncDeferredCompletionQueues = ValidatorIds
  /\ DOMAIN asyncDeferredProgressQueues = ValidatorIds
  /\ DOMAIN asyncDeferredNormalQueues = ValidatorIds
  /\ asyncDeferredDrainOwed \in [ValidatorIds -> BOOLEAN]

AsyncDeferredContentTypeInvariant ==
  /\ \A node \in ValidatorIds:
       /\ AsyncCompletionSequenceTyped(
            asyncDeferredCompletionQueues[node])
       /\ AsyncQueueTyped(asyncDeferredProgressQueues[node])
       /\ AsyncQueueTyped(asyncDeferredNormalQueues[node])
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
       /\ IngressDepth(recipient) <= AsyncIngressCapacity
       /\ IngressDepth(recipient)
            + Cardinality(
                {source \in AsyncIngressSources:
                   IngressLaneDepth(recipient, source) = 0})
            <= AsyncIngressCapacity

AsyncIngressContentTypeInvariant ==
  \A recipient \in ValidatorIds:
       /\ \A source \in AsyncIngressSources:
            /\ IngressLane(recipient, source)
                 \in Seq(Range(IngressLane(recipient, source)))
            /\ DOMAIN IngressLane(recipient, source) =
                 1..IngressLaneDepth(recipient, source)
            /\ \A index \in 1..IngressLaneDepth(recipient, source):
                 AsyncItemTyped(IngressLane(recipient, source)[index])

AsyncIngressTypeInvariant ==
  /\ AsyncIngressTopologyTypeInvariant
  /\ AsyncIngressCapacityTypeInvariant
  /\ AsyncIngressContentTypeInvariant

AsyncSchedulerTypeInvariant ==
  /\ AsyncRuntimeTypeInvariant
  /\ AsyncIoTypeInvariant
  /\ AsyncDeferredTypeInvariant
  /\ AsyncTransportTypeInvariant
  /\ AsyncIngressTypeInvariant

AsyncTypeInvariant ==
  /\ TypeInvariant
  /\ AsyncSchedulerTypeInvariant

AsyncCompletionReserveInvariant ==
  \A node \in ValidatorIds:
    /\ AsyncCompletionLoad(node) <= AsyncCompletionReserve
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
       /\ AsyncCompletionLoad(node) <= AsyncIoWorkCapacity

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

SchedulerServiceRank(node, command) ==
  CHOOSE index \in 1..Len(asyncCommandQueues[node]):
    asyncCommandQueues[node][index] = command

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
