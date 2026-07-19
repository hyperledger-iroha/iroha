---- MODULE SumeragiV2LivenessProofs ----
EXTENDS SumeragiV2AsyncNetwork, SumeragiV2Proofs

(***************************************************************************
One-height liveness vocabulary and well-founded service measures.

This module contains no second consensus relation and no favourable network
step.  Every temporal property below is stated over the unbounded
`AsyncSpecAt(initialContext)`.  The asynchronous proof module records the exact
release obligations over the concrete FIFO, fair-ingress, IO-worker,
retransmission, and absolute-timeout actions; the proof ledger records their
current mechanization status.
***************************************************************************)

ResponsiveNodesDecide ==
  \A node \in AsyncCurrentResponsiveVoters: NodeHasDecision(node)

ResponsiveNodesApply ==
  \A node \in AsyncCurrentResponsiveVoters: NodeHasApplication(node)

(***************************************************************************
An undecided responsive honest leader has reached a usable rotating-leader
view only when its own current view selects that same validator.  Merely
observing a view number which names a responsive leader is insufficient: the
leader itself must be scheduled in the matching view before proposal service
can discharge the second liveness clause below.
***************************************************************************)
ResponsiveHonestLeaderViewReached ==
  \E leader \in (AsyncCurrentResponsiveVoters \cap Honest):
    /\ ~NodeHasDecision(leader)
    /\ Leader(context, nodeView[leader]) = leader

TimeoutViewProgressProperty(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters,
          roundView \in Views:
         (gst /\ nodeView[node] = roundView /\ ~NodeHasDecision(node))
           ~> (nodeView[node] > roundView \/ NodeHasDecision(node))

RotatingLeaderProgressProperty(specification) ==
  specification
    => /\ (gst /\ ~ResponsiveNodesDecide)
             ~> (ResponsiveHonestLeaderViewReached
                   \/ ResponsiveNodesDecide)
       /\ (gst /\ ResponsiveHonestLeaderViewReached
                 /\ ~ResponsiveNodesDecide)
             ~> ResponsiveNodesDecide

ApplicationCompletionProgressProperty(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters:
         (gst /\ NodeHasDecision(node))
           ~> NodeHasApplication(node)

ApplicationLivenessProperty(specification) ==
  specification
    => /\ \A node \in AsyncCurrentResponsiveVoters:
             (gst /\ NodeHasDecision(node))
               ~> NodeHasApplication(node)
       /\ (gst /\ ResponsiveNodesDecide) ~> ResponsiveNodesApply

(***************************************************************************
Explicit progress obligations.

These predicates expose the proof seams that were previously described only
in prose.  Deadlock freedom names the concrete post-GST actions that can move
an undecided execution.  Starvation freedom is stronger: every admitted
protocol progress/completion candidate must leave scheduler ownership, and the
lexicographic service rank must first decrease unless service completes it.
Neither property treats repeated view changes as height progress.
***************************************************************************)

PostGstSchedulerActionEnabled ==
  \/ ENABLED AsyncTick
  \/ \E node \in AsyncCurrentResponsiveVoters:
       ENABLED PostGstRunNode(node)
  \/ \E node \in AsyncCurrentResponsiveVoters:
       ENABLED PostGstRunHistoricalServer(node)
  \/ \E node \in AsyncCurrentResponsiveVoters:
       ENABLED PostGstCommitCertificateDiscovery(node)
  \/ \E node \in AsyncCurrentResponsiveVoters:
       ENABLED PostGstServiceIoWorker(node)
  \/ \E recipient \in AsyncCurrentResponsiveVoters,
       source \in AsyncCurrentResponsiveVoters:
       ENABLED PostGstAdmitHiddenPacket(recipient, source)

SetGains(before, after) == after \ before # {}

(***************************************************************************
Protocol productiveness is exact evidence growth for the current height.
Timeout certificates and view counters are deliberately absent: rotating
through views without admitting a body, proposal, vote/QC, durable intent,
decision, or application is not a height-progress witness.
***************************************************************************)
HeightProtocolEvidenceGrows ==
  \/ SetGains(availableBodies, availableBodies')
  \/ SetGains(durableBodies, durableBodies')
  \/ SetGains(retainedLockedBodies, retainedLockedBodies')
  \/ SetGains(validatedBodies, validatedBodies')
  \/ SetGains(seenProposals, seenProposals')
  \/ SetGains(receivedVotes, receivedVotes')
  \/ SetGains(receivedQCs, receivedQCs')
  \/ SetGains(proposalIntents, proposalIntents')
  \/ SetGains(prepareIntents, prepareIntents')
  \/ SetGains(commitIntents, commitIntents')
  \/ SetGains(prepareQCs, prepareQCs')
  \/ SetGains(commitQCs, commitQCs')
  \/ SetGains(decisions, decisions')
  \/ SetGains(applied, applied')

DeadlineDistance(deadline, now) ==
  IF now < deadline THEN deadline - now ELSE 0

(***************************************************************************
A clock step is productive only while it strictly consumes a concrete
protocol, retransmission, scheduler, worker, or in-flight delivery deadline.
Once all such debts are zero, another bare tick is not a deadlock witness.
***************************************************************************)
PostGstDeadlineDebtDecreases ==
  \/ \E node \in AsyncCurrentResponsiveVoters:
       \/ DeadlineDistance(asyncNodeDeadlines'[node], asyncNow')
            < DeadlineDistance(asyncNodeDeadlines[node], asyncNow)
       \/ DeadlineDistance(asyncRetransmitDeadlines'[node], asyncNow')
            < DeadlineDistance(asyncRetransmitDeadlines[node], asyncNow)
       \/ DeadlineDistance(asyncNodeServiceDeadlines'[node], asyncNow')
            < DeadlineDistance(asyncNodeServiceDeadlines[node], asyncNow)
       \/ DeadlineDistance(asyncIoServiceDeadlines'[node], asyncNow')
            < DeadlineDistance(asyncIoServiceDeadlines[node], asyncNow)
  \/ \E packet \in asyncTransport \cap asyncTransport':
       DeadlineDistance(packet.deadline, asyncNow')
         < DeadlineDistance(packet.deadline, asyncNow)

NormalProposalPrepareNoItemKinds == {"AssembleBody", "BeginPrepare"}

NormalProposalPrepareNetworkKinds ==
  {"Proposal", "PrepareVote", "CommitVote"}

NormalBeginPrepareParentKinds == {"DeliverProposal", "ValidateBody"}

(***************************************************************************
Frozen constructor snapshots for protected Normal work.

None of these operators reads live protocol state.  Every field which
participates in `ExactAsyncCandidateIdentity` is supplied explicitly, so a
view/generation/context transition cannot silently reclassify a candidate by
reconstructing it against the successor state.  The ordinary constructor
below remains useful at admission, while the predicates quantify the exact
historical consumer snapshot which admission stored.
***************************************************************************)
FrozenNormalDeliveryCandidate(item, consumerContext, consumerView,
                              consumerGeneration) ==
  LET subject == DeliverySubject(item)
  IN AsyncCandidateWithIdentity(
       "Normal", DeliveryKind(item), item.envelope.recipient,
       DeliveryHeight(item), DeliveryView(item), subject, item,
       consumerContext, consumerView, consumerGeneration, item,
       subject, subject, subject)

NormalDeliveryCandidate(item) ==
  FrozenNormalDeliveryCandidate(
    item, context, nodeView[item.envelope.recipient],
    generation[item.envelope.recipient])

FrozenNormalAssemblyCandidate(blockContext, node, roundView,
                              consumerGeneration, subject, evidence) ==
  AsyncCandidateWithIdentity(
    "Normal", "AssembleBody", node, blockContext.height, roundView,
    subject, NoAsyncItem, blockContext, roundView, consumerGeneration,
    evidence, subject, subject, subject)

NextCandidateGeneration(currentGeneration) ==
  IF currentGeneration < MaxGeneration
  THEN currentGeneration + 1
  ELSE currentGeneration

FrozenInstallProposalSuccessor(command, installedContext,
                               priorGeneration, subject) ==
  AsyncCandidateWithIdentity(
    "Normal", "AssembleBody", command.node, installedContext.height,
    command.view + 1, subject, NoAsyncItem, installedContext,
    command.view + 1, NextCandidateGeneration(priorGeneration),
    command.evidence, subject, subject, subject)

FrozenNormalBeginPrepareCandidate(parent, blockHeight) ==
  AsyncCandidateWithIdentity(
    "Normal", "BeginPrepare", parent.node, blockHeight, parent.view,
    parent.subject, NoAsyncItem, parent.consumerContext,
    parent.consumerView, parent.consumerGeneration, parent.evidence,
    parent.bodyIdentity, parent.manifestIdentity,
    parent.commitmentIdentity)

(***************************************************************************
Canonical historical shapes for the two item-free Normal owners.  Initial and
restart assembly carries `NoAsyncItem` evidence.  PersistInstallTC has a
separate explicit family because it inherits the exact durable TC evidence and
stores the post-install consumer view/generation.  BeginPrepare copies every
parent-carried identity field and freezes the construction-time block height
rather than rebuilding either value from live state.
***************************************************************************)
NormalProposalPrepareNoItemCandidate(candidate) ==
  /\ candidate.item = NoAsyncItem
  /\ candidate.kind \in NormalProposalPrepareNoItemKinds
  /\ \/ \E blockContext \in ContextRecords, node \in ValidatorIds,
            roundView \in Views, consumerGeneration \in Generations,
            subject \in SubjectOrNone:
           candidate = FrozenNormalAssemblyCandidate(
                         blockContext, node, roundView,
                         consumerGeneration, subject, NoAsyncItem)
     \/ \E command \in AsyncCandidateSet,
            installedContext \in ContextRecords,
            priorGeneration \in Generations,
            subject \in SubjectOrNone:
          /\ command.kind = "PersistInstallTC"
          /\ command.view + 1 \in Views
          /\ candidate = FrozenInstallProposalSuccessor(
                           command, installedContext,
                           priorGeneration, subject)
     \/ \E parent \in AsyncCandidateSet, blockHeight \in Heights:
          /\ parent.kind \in NormalBeginPrepareParentKinds
          /\ candidate =
               FrozenNormalBeginPrepareCandidate(parent, blockHeight)

NormalProposalPrepareNetworkCandidate(candidate) ==
  \E item \in AsyncNetworkItems,
     consumerContext \in ContextRecords,
     consumerView \in Views,
     consumerGeneration \in Generations:
    /\ item.kind \in NormalProposalPrepareNetworkKinds
    /\ candidate = FrozenNormalDeliveryCandidate(
                     item, consumerContext, consumerView,
                     consumerGeneration)

(***************************************************************************
The proposal/Prepare path has immutable constructor families covering initial
or restart AssembleBody, the explicit PersistInstallTC successor, causal
BeginPrepare after DeliverProposal/ValidateBody, and canonical
Proposal/PrepareVote/CommitVote delivery.  Reachable delivery ownership comes
from authenticated ingress; this state-independent classifier deliberately
also drains a stale stored candidate after view movement.  Persist, signature,
validation, and decision continuations are already Completion or Progress.
Full frozen-constructor equality plus the finite carrier keeps mismatched
class/kind/item/identity Cartesian records outside the temporal promise.
***************************************************************************)
NormalProposalPrepareCandidate(candidate) ==
  /\ candidate \in AsyncCandidateSet
  /\ candidate.class = "Normal"
  /\ \/ NormalProposalPrepareNoItemCandidate(candidate)
     \/ NormalProposalPrepareNetworkCandidate(candidate)

ProtectedServiceCandidate(candidate) ==
  /\ candidate \in AsyncCandidateSet
  /\ \/ candidate.class = "Completion"
     \/ /\ candidate.class = "Progress"
           /\ candidate.kind # "RejectProgress"
     \/ NormalProposalPrepareCandidate(candidate)

(***************************************************************************
`CandidateScheduled` remains a raw structural witness, including queues that
the one-height abstraction retains after Apply.  Production retires that live
height runtime immediately, so protected service ownership ends once the
candidate's validator has applied; the historical runner serves only durable
recovery artifacts and cannot own old-height consensus work.
***************************************************************************)
ProtectedCandidateOwned(candidate) ==
  /\ ProtectedServiceCandidate(candidate)
  /\ CandidateScheduled(candidate)
  /\ ~NodeHasApplication(candidate.node)

(***************************************************************************
The concrete fair-action frontier serves only responsive current voters.
Keep the raw ownership predicate above available to structural invariants, but
scope temporal service promises to the same nodes for which AsyncFairnessAt
supplies RunNode, IO-worker, and ingress weak fairness.
***************************************************************************)
ResponsiveProtectedCandidateOwned(candidate) ==
  /\ candidate.node \in AsyncCurrentResponsiveVoters
  /\ ProtectedCandidateOwned(candidate)

(***************************************************************************
Authenticated recovery requests enter the worker as occurrence-owned Serve
jobs.  Exact retransmissions may create equal candidate values after the
earlier ingress owner has left, so this obligation is keyed by the fresh live
job nonce rather than by candidate equality.
***************************************************************************)
AsyncServeJobSet ==
  {AsyncIoJob("Serve", candidate, nonce):
     candidate \in AsyncCandidateSet,
     nonce \in 0..AsyncIoAuxCapacity}

ResponsiveProtectedServeJobOwned(node, job) ==
  /\ node \in AsyncCurrentResponsiveVoters
  /\ job \in AsyncServeJobSet
  /\ job \in SequenceSet(asyncIoQueues[node])

ServeJobIndex(node, job) ==
  CHOOSE index \in AsyncIoServeIndices(asyncIoQueues[node]):
    asyncIoQueues[node][index] = job

ServeJobRank(node, job) == <<5, ServeJobIndex(node, job)>>

CandidateSequenceIndex(candidate, queue) ==
  CHOOSE index \in 1..Len(queue): queue[index] = candidate

CandidateIoIndex(candidate, queue) ==
  CHOOSE index \in AsyncIoConsensusIndices(queue):
    queue[index].candidate = candidate

CandidateInIngress(candidate) ==
  \E source \in AsyncIngressSources:
    candidate.item \in SequenceSet(
      IngressLane(candidate.node, source))

CandidateInIoQueue(candidate) ==
  \E index \in AsyncIoConsensusIndices(
                 asyncIoQueues[candidate.node]):
    asyncIoQueues[candidate.node][index].candidate = candidate

CandidateInReadyQueue(candidate) ==
  candidate \in SequenceSet(
                   asyncIoReadyCompletions[candidate.node])
    \/ candidate \in SequenceSet(
                       asyncLocalReadyCompletions[candidate.node])

DeferredCandidateIndices(node, candidate) ==
  {index \in 1..Len(DeferredClassQueue(node, candidate.class)):
     DeferredClassQueue(node, candidate.class)[index] = candidate}

DeferredClassPrefixIndices(node, candidate) ==
  {index \in 1..Len(DeferredClassQueue(node, candidate.class)):
     \E matching \in DeferredCandidateIndices(node, candidate):
       index <= matching}

(***************************************************************************
The production deferred reducer queue is cyclic across Completion, Progress,
and Normal, independently of the runtime command cursor.  As for the runtime
queue, multiplying the duplicate-aware class ordinal by three leaves room for
the cursor distance.  Dispatching a different class lowers that distance;
dispatching the candidate's class lowers the ordinal and may reset distance by
at most two.
***************************************************************************)
DeferredCandidatePosition(candidate) ==
  3 * Cardinality(
        DeferredClassPrefixIndices(candidate.node, candidate))
    + CommandClassDistance(
        asyncNextDeferredClass[candidate.node], candidate.class)

(***************************************************************************
The causal source gets one bit of local scheduler distance.  A producer turn
while causal work waits flips this distance from one to zero by recording
debt; removing an earlier causal head lowers the doubled FIFO position enough
to dominate the possible zero-to-one cursor reset.
***************************************************************************)
LocalSourceDistance(node, source) ==
  IF PreferredLocalSource(node) = source THEN 0 ELSE 1

ReadyCompletionQueue(node, source) ==
  IF source = "Io"
  THEN asyncIoReadyCompletions[node]
  ELSE asyncLocalReadyCompletions[node]

ReadyCandidateSource(candidate) ==
  IF candidate \in SequenceSet(
                       asyncIoReadyCompletions[candidate.node])
  THEN "Io"
  ELSE "Local"

(***************************************************************************
The ready-completion rank mirrors both serialized scheduler cursors.  The
queue index is multiplied by four so that consuming an earlier completion
strictly dominates the possible two-bit reset from changing the completion
source and then changing the local Producer/Causal source.
***************************************************************************)
ReadyCandidatePosition(candidate) ==
  LET source == ReadyCandidateSource(candidate)
  IN 4 * CandidateSequenceIndex(
           candidate, ReadyCompletionQueue(candidate.node, source))
       + 2 * (IF SelectedCompletionSource(candidate.node) = source
              THEN 0 ELSE 1)
       + LocalSourceDistance(candidate.node, "Producer")

CausalCandidatePosition(candidate) ==
  2 * CandidateSequenceIndex(
        candidate, asyncCausalQueues[candidate.node])
    + LocalSourceDistance(candidate.node, "Causal")

(***************************************************************************
This rank is intentionally scheduler-owned only. CandidateScheduled contains
deferred, runtime, completion, I/O, outstanding-work, and causal ownership,
which map to stages 2 through 6. A transport packet or an ingress occurrence
does not yet own a persistent candidate value and therefore needs its own
occurrence identity and fairness proof; assigning it a dead stage 7 or 8 here
would discharge no reachable protected-candidate state.
***************************************************************************)
CandidateServiceRank(candidate) ==
  IF candidate \in DeferredCandidates
  THEN <<2, DeferredCandidatePosition(candidate)>>
  ELSE IF candidate \in QueuedCandidates
       THEN <<3, SchedulerServiceRank(candidate.node, candidate)>>
       ELSE IF CandidateInReadyQueue(candidate)
            THEN <<4, ReadyCandidatePosition(candidate)>>
            ELSE IF CandidateInIoQueue(candidate)
                 THEN <<5, CandidateIoIndex(
                              candidate,
                              asyncIoQueues[candidate.node])>>
                 ELSE IF candidate \in
                           asyncOutstandingWork[candidate.node]
                      THEN <<5, AsyncCompletionLoad(candidate.node)>>
                      ELSE IF candidate \in CausalCandidates
                           THEN <<6, CausalCandidatePosition(candidate)>>
                           ELSE <<0, 0>>

ServiceRankLess(left, right) ==
  \/ left[1] < right[1]
  \/ /\ left[1] = right[1]
        /\ left[2] < right[2]

ProtectedServiceRankDecreaseStep ==
  \E candidate \in AsyncCandidateSet,
     stage \in 2..6, position \in Nat:
    /\ ResponsiveProtectedCandidateOwned(candidate)
    /\ CandidateServiceRank(candidate) = <<stage, position>>
    /\ \/ ~ResponsiveProtectedCandidateOwned(candidate)'
       \/ ServiceRankLess(CandidateServiceRank(candidate)',
            <<stage, position>>)

ProtectedServeRankDecreaseStep ==
  \E node \in AsyncCurrentResponsiveVoters,
     job \in AsyncServeJobSet, position \in Nat:
    /\ ResponsiveProtectedServeJobOwned(node, job)
    /\ ServeJobRank(node, job) = <<5, position>>
    /\ \/ ~ResponsiveProtectedServeJobOwned(node, job)'
       \/ ServiceRankLess(ServeJobRank(node, job)', <<5, position>>)

PostGstProductiveStep ==
  /\ gst
  /\ AsyncNext
  /\ \/ HeightProtocolEvidenceGrows
     \/ PostGstDeadlineDebtDecreases
     \/ ProtectedServiceRankDecreaseStep
     \/ ProtectedServeRankDecreaseStep

PostGstProductiveActionEnabled == ENABLED PostGstProductiveStep

DeadlockFreedomProperty(specification) ==
  specification
    => [](gst /\ ~ResponsiveNodesDecide
           => PostGstProductiveActionEnabled)

ProtectedServiceRankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet,
          stage \in 2..6, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<stage, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<stage, position>>))

ProtectedStage4RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<4, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<4, position>>))

ProtectedStage5RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<5, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<5, position>>))

ProtectedServeRankProgressProperty(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters,
          job \in AsyncServeJobSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedServeJobOwned(node, job)
           /\ ServeJobRank(node, job) = <<5, position>>)
           ~> (~ResponsiveProtectedServeJobOwned(node, job)
                \/ ServiceRankLess(
                     ServeJobRank(node, job), <<5, position>>))

(***************************************************************************
The release rank obligation covers both protected reducer candidates and the
separate fresh-nonce Serve FIFO.  Keeping the conjunction named prevents a
proof of the candidate rank alone from being reported as complete scheduler
rank coverage.
***************************************************************************)
ProtectedServiceRanksProgressProperty(specification) ==
  /\ ProtectedServiceRankProgressProperty(specification)
  /\ ProtectedServeRankProgressProperty(specification)

ProtectedServeStarvationProperty(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters,
          job \in AsyncServeJobSet:
         (gst /\ ResponsiveProtectedServeJobOwned(node, job))
           ~> ~ResponsiveProtectedServeJobOwned(node, job)

NormalProposalPrepareRankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet,
          stage \in 2..6, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ NormalProposalPrepareCandidate(candidate)
           /\ CandidateServiceRank(candidate) = <<stage, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<stage, position>>))

StarvationFreedomProperty(specification) ==
  /\ (specification
        => \A candidate \in AsyncCandidateSet:
             (gst /\ ResponsiveProtectedCandidateOwned(candidate))
               ~> ~ResponsiveProtectedCandidateOwned(candidate))
  /\ ProtectedServeStarvationProperty(specification)

(***************************************************************************
Durable source-to-consumer witnesses.  The active locked Commit intent is the
single durable intent matching the node's current exact lock; superseded
historical intents are deliberately excluded.  A persisted decision remains
witnessed until local application by certified-body recovery or a scheduled
store/validate/apply continuation.
***************************************************************************)

ActiveLockedCommitIntent(node, vote) ==
  /\ vote \in commitIntents
  /\ vote.context = context
  /\ vote.signer = node
  /\ vote.phase = "Commit"
  /\ vote.view = lockRank[node]
  /\ vote.subject = lockSubject[node]

RetainedCommitIntent(node, vote) ==
  \E item \in asyncRetainedControl:
    /\ item.kind = "CommitVote"
    /\ item.source = node
    /\ item.envelope.vote = vote

CommitIntentProgressWitness(node, vote) ==
  \/ VoteSign(node, vote) \in signVotes
  \/ RetainedCommitIntent(node, vote)
  \/ VoteAt(node, vote) \in receivedVotes
  \/ \E qc \in commitQCs:
       /\ qc.context = vote.context
       /\ qc.view = vote.view
       /\ qc.subject = vote.subject
  \/ NodeHasDecision(node)

DurableCommitProgressWitness ==
  \A node \in AsyncCurrentResponsiveVoters, vote \in commitIntents:
    ActiveLockedCommitIntent(node, vote)
      => CommitIntentProgressWitness(node, vote)

HistoricalLockedCommitRecoveryWitness(node, qc) ==
  \/ ExactLockedCommitIntents(node, qc.view, qc.subject) # {}
  \/ \E request \in pendingLockCommit:
       /\ request.node = node
       /\ request.qc = qc
  \/ \E candidate \in AsyncCandidateSet:
       /\ candidate.node = node
       /\ candidate.height = qc.context.height
       /\ candidate.view = qc.view
       /\ candidate.subject = qc.subject
       /\ candidate.kind = "BeginLockCommit"
       /\ CandidateScheduled(candidate)

(***************************************************************************
Once the exact TC-promoted historical lock has a current-generation durable
validation witness, the serialized reducer must already own either its exact
Commit intent, the WAL request that will create it, or the validation
successor that begins that request.  The historical guard also forbids
retroactively signing below a higher conflicting-subject local Prepare intent
or known PrepareQC; a higher reproposal of the same subject is harmless.
***************************************************************************)
HistoricalLockedCommitRecoveryProgress ==
  \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
    (/\ HistoricalTcLockedPrepareForCommit(node, qc)
     /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
     /\ BodyValidatedBy(validatedBodies, node, context, qc.view,
                        generation[node], qc.subject))
      => HistoricalLockedCommitRecoveryWitness(node, qc)

(***************************************************************************
Only an exact completion owner for the current reducer consumer epoch counts
as Decision-pipeline progress.  In particular, PersistDecision's immediate
FetchBody successor is a real recovery frontier, while a scheduled occurrence
left behind by a view or generation change is not: the runtime will discard
that stale occurrence instead of executing it.
***************************************************************************)
DecisionPipelineCandidate(node, qc, candidate) ==
  /\ candidate.class = "Completion"
  /\ candidate.node = node
  /\ candidate.height = qc.context.height
  /\ candidate.view = qc.view
  /\ candidate.subject = qc.subject
  /\ candidate.kind \in
       {"FetchBody", "RequestCertifiedBody", "FetchCertifiedBody", "StoreBody",
        "ValidateBody", "Apply"}
  /\ CandidateConsumerCurrent(candidate)
  /\ CandidateScheduled(candidate)

DecisionCompletionWitness(node, qc) ==
  \/ NodeHasApplication(node)
  \/ \E request \in asyncActiveRequests:
       /\ request.kind = "CertifiedRequest"
       /\ request.source = node
       /\ request.envelope.height = qc.context.height
       /\ request.envelope.view = qc.view
       /\ request.envelope.subject = qc.subject
  \/ \E candidate \in AsyncCandidateSet:
       DecisionPipelineCandidate(node, qc, candidate)

DurableDecisionProgressWitness ==
  \A decision \in decisions:
    (decision.node \in AsyncCurrentResponsiveVoters
      /\ decision.qc.context = context)
      => DecisionCompletionWitness(decision.node, decision.qc)

ProtectedDeferredProgressIndices(node) ==
  {index \in 1..Len(asyncDeferredProgressQueues[node]):
     ProtectedProgressCommand(
       asyncDeferredProgressQueues[node][index])}

ProtectedDeferredProgressInvariant ==
  \A node \in ValidatorIds:
    /\ Cardinality(ProtectedDeferredProgressIndices(node)) <= 2 * N + 3
    /\ \A left, right \in ProtectedDeferredProgressIndices(node):
         SameProtectedProgressSlot(
           asyncDeferredProgressQueues[node][left],
           asyncDeferredProgressQueues[node][right])
           => left = right

ProgressWitnessInvariant ==
  /\ DurableCommitProgressWitness
  /\ HistoricalLockedCommitRecoveryProgress
  /\ DurableDecisionProgressWitness
  /\ ProtectedDeferredProgressInvariant

ProgressWitnessProperty(specification) ==
  specification => []ProgressWitnessInvariant

VoteDeliveryEpochAction ==
  /\ \A node \in ValidatorIds, vote \in VoteRecordSet:
       (vote.phase = "Prepare" /\ VoteRoundAdmissible(node, vote))
         => vote.view = nodeView[node]
  /\ \A node \in ValidatorIds, vote \in VoteRecordSet:
       (vote.phase = "Commit" /\ VoteRoundAdmissible(node, vote))
         => LockedPrepareRound(node, vote.view, vote.subject)
  /\ \A node \in ValidatorIds, roundView \in Views,
        subject \in Subjects:
       CommitRoundAdmissible(node, roundView, subject)
         => LockedPrepareRound(node, roundView, subject)
  /\ \A request \in VoteSignSet:
       CompleteVoteSignature(request)
         => /\ VoteAt(request.node, request.vote) \in receivedVotes'
            /\ \A envelope \in BroadcastVotes(request.vote):
                 envelope.recipient # request.node
  /\ \A envelope \in VoteEnvelopeSet:
       DeliverVote(envelope)
         => /\ VoteAt(envelope.recipient, envelope.vote)
                  \notin receivedVotes
            /\ VoteAt(envelope.recipient, envelope.vote)
                  \in receivedVotes'
            /\ voteNetwork' = voteNetwork
  /\ \A request \in InstallTcWalSet:
       PersistInstallTC(request)
         => /\ \A received \in receivedVotes':
                   received.node # request.node
            /\ ActiveLockedCommitSignRequestsAfterInstall(
                 request.node, request.tc) \subseteq signVotes'
            /\ (generation[request.node] < MaxGeneration
                  => generation'[request.node] =
                       generation[request.node] + 1)
  /\ \A request \in LockCommitWalSet:
       PersistLockCommit(request)
         => /\ \A received \in receivedVotes':
                   VoteReceiptSurvivesLockCommit(
                     received, request.node, request.qc.view,
                     request.qc.subject)
            /\ \A received \in receivedVotes:
                 VoteReceiptSurvivesLockCommit(
                   received, request.node, request.qc.view,
                   request.qc.subject)
                   => received \in receivedVotes'

GenerationScopedVoteDeliveryProperty(specification) ==
  specification => [][VoteDeliveryEpochAction]_AsyncAllVars

OneHeightDecisionLiveness(initialContext) ==
  AsyncSpecAt(initialContext)
    => PostGstEventuallyAsyncDecisionAt(initialContext)

OneHeightApplicationLiveness(initialContext) ==
  AsyncSpecAt(initialContext)
    => ResponsiveDecisionEventuallyAppliedAt(initialContext)

OneHeightCompletionLiveness(initialContext) ==
  AsyncSpecAt(initialContext)
    => (gst ~> AsyncAllResponsiveAppliedAt(initialContext))

CanonicalSuccessorContext(initialContext, subject) ==
  ContextRecord(initialContext.height + 1,
                Append(initialContext.lineage, subject))

CanonicalSuccessorAdmissible(initialContext, subject) ==
  /\ FrozenContextAdmissible(initialContext)
  /\ initialContext.height < MaxHeight
  /\ subject \in ValidSubjects
  /\ FrozenContextAdmissible(
       CanonicalSuccessorContext(initialContext, subject))

THEOREM IoAdmissionLimitsAreStrictlyReserved ==
  AsyncConfiguration
    => /\ AsyncIoAdmissionLimit("Serve")
             < AsyncIoAdmissionLimit("Consensus")
       /\ AsyncIoAdmissionLimit("Consensus")
             < AsyncIoAdmissionLimit("Control")
       /\ AsyncIoAdmissionLimit("Control") = AsyncIoCapacity
BY SMT DEF AsyncConfiguration, AsyncIoAdmissionLimit, AsyncIoCapacity

THEOREM RuntimeReachRankIsNatural ==
  AsyncTypeInvariant
    => \A node \in ValidatorIds: RuntimeReachRank(node) \in Nat
PROOF
  <1>1. ASSUME AsyncTypeInvariant
         PROVE \A node \in ValidatorIds: RuntimeReachRank(node) \in Nat
    <2>1. ASSUME NEW node \in ValidatorIds
           PROVE RuntimeReachRank(node) \in Nat
      <3>1. /\ asyncRunnerPhase[node]
                    \in {"Local", "Ingress", "Runtime"}
             /\ asyncRunnerBudget[node]
                    \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
             /\ AsyncQueueCapacity \in Nat \ {0}
             /\ AsyncIngressCapacity \in Nat \ {0}
        BY <1>1, <2>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
               AsyncConfiguration
      <3>2. CASE asyncRunnerPhase[node] = "Local"
        BY <3>1, <3>2, SMT DEF RuntimeReachRank
      <3>3. CASE asyncRunnerPhase[node] = "Ingress"
        BY <3>1, <3>3, SMT DEF RuntimeReachRank
      <3>4. CASE asyncRunnerPhase[node] = "Runtime"
        BY <3>1, <3>4, SMT DEF RuntimeReachRank
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM RetransmissionBudgetCoversEveryClass ==
  ModelConfiguration /\ AsyncConfiguration
    => /\ AsyncRetainedControlBudget \in Nat
       /\ AsyncRetainedProposalChunkBudget \in Nat
       /\ AsyncActiveCertifiedRequestBudget \in Nat
       /\ AsyncActiveCommitRequestBudget \in Nat
       /\ AsyncActiveRequestBudget
             = AsyncActiveCertifiedRequestBudget
                 + AsyncActiveCommitRequestBudget
       /\ AsyncRetransmitEmissionBudget
             = AsyncRetainedControlBudget
                 + AsyncRetainedProposalChunkBudget
                 + AsyncActiveRequestBudget
BY SMT DEF AsyncConfiguration, AsyncRetainedControlBudget,
           AsyncRetainedProposalChunkBudget,
           AsyncActiveCertifiedRequestBudget,
           AsyncActiveCommitRequestBudget, AsyncActiveRequestBudget,
           AsyncRetransmitEmissionBudget, ModelConfiguration,
           QuorumConfiguration

THEOREM CanonicalSuccessorPreservesAdmissibility ==
  ModelConfiguration
    => \A initialContext \in ContextRecords, subject \in ValidSubjects:
         (FrozenContextAdmissible(initialContext)
           /\ initialContext.height < MaxHeight)
           => FrozenContextAdmissible(
                CanonicalSuccessorContext(initialContext, subject))
PROOF
  <1>1. ASSUME ModelConfiguration
         PROVE \A initialContext \in ContextRecords,
                    subject \in ValidSubjects:
                 (FrozenContextAdmissible(initialContext)
                   /\ initialContext.height < MaxHeight)
                   => FrozenContextAdmissible(
                        CanonicalSuccessorContext(initialContext, subject))
    <2>1. ASSUME NEW initialContext \in ContextRecords,
                  NEW subject \in ValidSubjects,
                  FrozenContextAdmissible(initialContext),
                  initialContext.height < MaxHeight
           PROVE FrozenContextAdmissible(
                   CanonicalSuccessorContext(initialContext, subject))
      <3> DEFINE NextHeight == initialContext.height + 1
      <3> DEFINE NextLineage == Append(initialContext.lineage, subject)
      <3> DEFINE NextContext == ContextRecord(NextHeight, NextLineage)
      <3>1. /\ initialContext.height \in Heights
            /\ initialContext.lineage
                 \in LineagesAt(initialContext.height)
        BY <2>1, ContextRecordFieldsTyped
      <3>2. /\ MaxHeight \in Nat
            /\ ValidSubjects \subseteq Subjects
        BY <1>1 DEF ModelConfiguration
      <3>3. /\ initialContext.height \in Nat
            /\ NextHeight \in Heights
        BY <2>1, <3>1, <3>2, SMT DEF Heights, NextHeight
      <3>4. /\ DOMAIN initialContext.lineage =
                    1..initialContext.height
            /\ \A index \in 1..initialContext.height:
                 initialContext.lineage[index] \in ValidSubjects
        <4>1. DOMAIN initialContext.lineage =
                 1..initialContext.height
          BY <3>1 DEF LineagesAt
        <4>2. \A index \in 1..initialContext.height:
                 initialContext.lineage[index] \in ValidSubjects
          BY <2>1, <4>1 DEF FrozenContextAdmissible
        <4> QED BY <4>1, <4>2
      <3>5. initialContext.lineage =
               [index \in 1..initialContext.height |->
                  initialContext.lineage[index]]
        BY <3>1, Isa DEF LineagesAt
      <3>6. [index \in 1..initialContext.height |->
               initialContext.lineage[index]]
               \in Seq(ValidSubjects)
        BY <3>3, <3>4, IsASeq
      <3>7. initialContext.lineage \in Seq(ValidSubjects)
        BY <3>5, <3>6
      <3>8. Len(initialContext.lineage) = initialContext.height
        BY <3>3, <3>4, <3>7, LenProperties, Isa
      <3>9. /\ NextLineage \in Seq(ValidSubjects)
            /\ Len(NextLineage) = NextHeight
        BY <2>1, <3>7, <3>8, AppendProperties
           DEF NextHeight, NextLineage
      <3>10. NextLineage \in Seq(Subjects)
        BY <3>2, <3>9, SeqMonotonic
      <3>11. NextLineage \in LineagesAt(NextHeight)
        BY <3>9, <3>10, LenProperties DEF LineagesAt
      <3>12. NextContext \in ContextRecords
        BY <3>3, <3>11, Isa DEF NextContext, ContextRecords
      <3>13. /\ NextContext.lineage = NextLineage
              /\ NextContext.height = NextHeight
        BY DEF NextContext, ContextRecord
      <3>14. \A index \in DOMAIN NextContext.lineage:
                NextContext.lineage[index] \in ValidSubjects
        <4>1. ASSUME NEW index \in DOMAIN NextContext.lineage
               PROVE NextContext.lineage[index] \in ValidSubjects
          <5>1. index \in 1..Len(NextLineage)
            BY <3>9, <3>13, <4>1, LenProperties
          <5>2. NextLineage[index] \in ValidSubjects
            BY <3>9, <5>1, ElementOfSeq
          <5> QED BY <3>13, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>12, <3>14
           DEF FrozenContextAdmissible, CanonicalSuccessorContext,
               NextContext, NextHeight, NextLineage
    <2> QED BY <2>1
  <1> QED BY <1>1

=============================================================================
