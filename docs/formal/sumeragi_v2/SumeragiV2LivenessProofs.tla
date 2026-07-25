---- MODULE SumeragiV2LivenessProofs ----
EXTENDS SumeragiV2AsyncNetwork, SumeragiV2Proofs

(***************************************************************************
One-height liveness vocabulary and well-founded service measures.

This module contains no second consensus relation and no favourable network
step.  Safety and recovery properties remain statements over the unbounded
`AsyncSpecAt(initialContext)`.  Progress claims which can traverse a checked
generation increment use `AsyncLiveSpecAt(initialContext)`, whose explicit
finite-resource budget rules out a pending install at `MaxGeneration`.
The asynchronous proof module records the exact release obligations over the
concrete FIFO, fair-ingress, IO-worker, retransmission, and absolute-timeout
actions; the proof ledger records their current mechanization status.
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
in prose.  Scheduler deadlock freedom asks only that a concrete post-GST action
can move an undecided execution.  The separate height-productivity frontier
requires evidence growth or a well-founded debt/rank descent and therefore
does not count a bare clock, phase handoff, or view change.  Starvation freedom
is stronger still: every admitted protocol progress/completion candidate must
leave scheduler ownership, and the lexicographic service rank must first
decrease unless service completes it.
***************************************************************************)

PostGstSchedulerActionEnabled ==
  \/ ENABLED AsyncTick
  \/ \E node \in AsyncCurrentResponsiveVoters:
       ENABLED PostGstRunNode(node)
  \/ \E node \in AsyncResponsiveAppliedArchiveServers:
       ENABLED PostGstRunHistoricalServer(node)
  \/ \E node \in AsyncCurrentResponsiveVoters:
       ENABLED PostGstCommitCertificateDiscovery(node)
  \/ \E node \in AsyncArchiveIoServiceNodes:
       ENABLED PostGstServiceIoWorker(node)
  \/ \E recipient \in AsyncArchiveIoServiceNodes,
       source \in AsyncIngressSources:
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
  \/ \E node \in AsyncTimedServiceNodes:
       \/ DeadlineDistance(asyncNodeServiceDeadlines'[node], asyncNow')
            < DeadlineDistance(asyncNodeServiceDeadlines[node], asyncNow)
       \/ DeadlineDistance(asyncIoServiceDeadlines'[node], asyncNow')
            < DeadlineDistance(asyncIoServiceDeadlines[node], asyncNow)
  \/ \E packet \in asyncTransport \cap asyncTransport':
       DeadlineDistance(packet.deadline, asyncNow')
         < DeadlineDistance(packet.deadline, asyncNow)

(***************************************************************************
Zero-distance scheduler blockers need a separate local handoff measure.
A due node turn may create the first due I/O owner because that worker's
absolute service deadline was already zero while its queue was empty.  Giving
the node blocker weight two and the I/O blocker weight one makes that exact
node-to-worker transfer descend.  This is a local scheduler certificate, not a
global height rank: a later Tick may make a future deadline due again.

Packets are deliberately excluded from the weighted sum.  Admission consumes
one concrete packet occurrence, so its progress witness is the explicit owner
exit below; `PostGstDeadlineDebtDecreases` covers only packets which remain in
transport while their positive distance is consumed.
***************************************************************************)
PostGstServiceNodes ==
  AsyncTimedServiceNodes

PostGstNodeServiceBlockers ==
  {node \in PostGstServiceNodes:
     asyncNodeServiceDeadlines[node] <= asyncNow}

PostGstIoServiceBlockers ==
  {node \in PostGstServiceNodes:
     /\ AsyncIoQueueDepth(node) > 0
     /\ asyncIoServiceDeadlines[node] <= asyncNow}

PostGstNodeIoBlockerDebt ==
  2 * Cardinality(PostGstNodeServiceBlockers)
    + Cardinality(PostGstIoServiceBlockers)

PostGstNodeIoBlockerDebtDecreases ==
  PostGstNodeIoBlockerDebt' < PostGstNodeIoBlockerDebt

PostGstOverduePacketOwnershipExits ==
  \E packet \in OverdueResponsivePackets:
    packet \notin asyncTransport'

(***************************************************************************
Local and Ingress runner turns are genuine bounded scheduler descent even when
they only advance the serialized phase.  RuntimeReachRank resets after the
single Runtime turn, so this arm is used only for its already-proved strict
Local/Ingress descent within one runner cycle; a raw phase change is never
accepted independently of that decrease.
***************************************************************************)
PostGstRuntimeReachDecreases ==
  \E node \in PostGstServiceNodes:
    RuntimeReachRank(node)' < RuntimeReachRank(node)

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
  /\ node \in AsyncArchiveIoServiceNodes
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

(***************************************************************************
The productive-step rank witnesses range over the finite live carriers.
The ownership guards already require a candidate to be scheduled and a Serve
job to occur in a concrete I/O queue, so ranging over the Cartesian candidate
and job universes adds no behavior.  The equivalence theorems below pin that
this first search-domain reduction is semantic only.

The `Live*` predicates are the executable reachable-state normal form for the
remaining canonical protection guards.  They inspect only an occurrence which
is already stored in a live queue/set.  In particular they never enumerate
`AsyncCandidateSet` or `AsyncServeJobSet`.  The asynchronous proof module pins
that every such occurrence is typed and that the live and canonical owners,
including their primed exit tests, are equivalent under the reachable type
invariant.  The canonical predicates remain the state-independent statement
of which historical constructor families receive the temporal promise.
***************************************************************************)
ActiveScheduledCandidates ==
  QueuedCandidates \cup DeferredCandidates \cup CausalCandidates
    \cup TrackedWorkCandidates

ActiveIoJobs ==
  UNION {SequenceSet(asyncIoQueues[node]): node \in ValidatorIds}

(***************************************************************************
TLC-safe algebraic normalization of the canonical Normal families.

For AssembleBody, equality with the frozen constructor pins all identity
fields.  Ordinary assembly has `NoAsyncItem` evidence; a persisted-TC
successor instead has a positive successor view and a generation in the image
of `NextCandidateGeneration`.  A typed BeginPrepare record always has a
canonical Cartesian parent witness because that family copies every other
identity field.  Network delivery reconstructs its unique frozen candidate
from the occurrence-carried item and consumer snapshot.
***************************************************************************)
LiveNormalProposalPrepareCandidate(candidate) ==
  /\ candidate.class = "Normal"
  /\ \/ /\ candidate = FrozenNormalAssemblyCandidate(
                         candidate.consumerContext, candidate.node,
                         candidate.view, candidate.consumerGeneration,
                         candidate.subject, candidate.evidence)
          /\ \/ candidate.evidence = NoAsyncItem
             \/ \E priorView \in Views,
                   priorGeneration \in Generations:
                  /\ candidate.view = priorView + 1
                  /\ candidate.consumerGeneration =
                       NextCandidateGeneration(priorGeneration)
     \/ /\ candidate.kind = "BeginPrepare"
           /\ candidate.item = NoAsyncItem
     \/ /\ candidate.item.kind \in NormalProposalPrepareNetworkKinds
           /\ candidate = FrozenNormalDeliveryCandidate(
                            candidate.item,
                            candidate.consumerContext,
                            candidate.consumerView,
                            candidate.consumerGeneration)

LiveProtectedServiceCandidate(candidate) ==
  \/ candidate.class = "Completion"
  \/ /\ candidate.class = "Progress"
        /\ candidate.kind # "RejectProgress"
  \/ LiveNormalProposalPrepareCandidate(candidate)

LiveResponsiveProtectedCandidateOwned(candidate) ==
  /\ candidate \in ActiveScheduledCandidates
  /\ candidate.node \in AsyncCurrentResponsiveVoters
  /\ ~NodeHasApplication(candidate.node)
  /\ LiveProtectedServiceCandidate(candidate)

LiveResponsiveProtectedServeJobOwned(node, job) ==
  /\ node \in AsyncArchiveIoServiceNodes
  /\ job \in SequenceSet(asyncIoQueues[node])
  /\ job.class = "Serve"

LiveProtectedServiceRankDecreaseStep ==
  \E candidate \in ActiveScheduledCandidates:
    LET rank == CandidateServiceRank(candidate)
    IN /\ LiveResponsiveProtectedCandidateOwned(candidate)
       /\ rank[1] \in 2..6
       /\ rank[2] \in Nat
       /\ \/ ~LiveResponsiveProtectedCandidateOwned(candidate)'
          \/ ServiceRankLess(CandidateServiceRank(candidate)', rank)

LiveProtectedServeRankDecreaseStep ==
  \E node \in AsyncArchiveIoServiceNodes,
     job \in ActiveIoJobs:
    LET rank == ServeJobRank(node, job)
    IN /\ LiveResponsiveProtectedServeJobOwned(node, job)
       /\ rank[1] = 5
       /\ rank[2] \in Nat
       /\ \/ ~LiveResponsiveProtectedServeJobOwned(node, job)'
          \/ ServiceRankLess(ServeJobRank(node, job)', rank)

ProtectedServiceRankDecreaseStep ==
  \E candidate \in ActiveScheduledCandidates:
    LET rank == CandidateServiceRank(candidate)
    IN /\ ResponsiveProtectedCandidateOwned(candidate)
       /\ rank[1] \in 2..6
       /\ rank[2] \in Nat
       /\ \/ ~ResponsiveProtectedCandidateOwned(candidate)'
          \/ ServiceRankLess(CandidateServiceRank(candidate)', rank)

ProtectedServeRankDecreaseStep ==
  \E node \in AsyncArchiveIoServiceNodes,
     job \in ActiveIoJobs:
    LET rank == ServeJobRank(node, job)
    IN /\ ResponsiveProtectedServeJobOwned(node, job)
       /\ rank[1] = 5
       /\ rank[2] \in Nat
       /\ \/ ~ResponsiveProtectedServeJobOwned(node, job)'
          \/ ServiceRankLess(ServeJobRank(node, job)', rank)

UniverseProtectedServiceRankDecreaseStep ==
  \E candidate \in AsyncCandidateSet,
     stage \in 2..6, position \in Nat:
    /\ ResponsiveProtectedCandidateOwned(candidate)
    /\ CandidateServiceRank(candidate) = <<stage, position>>
    /\ \/ ~ResponsiveProtectedCandidateOwned(candidate)'
       \/ ServiceRankLess(CandidateServiceRank(candidate)',
            <<stage, position>>)

UniverseProtectedServeRankDecreaseStep ==
  \E node \in AsyncArchiveIoServiceNodes,
     job \in AsyncServeJobSet, position \in Nat:
    /\ ResponsiveProtectedServeJobOwned(node, job)
    /\ ServeJobRank(node, job) = <<5, position>>
    /\ \/ ~ResponsiveProtectedServeJobOwned(node, job)'
       \/ ServiceRankLess(ServeJobRank(node, job)', <<5, position>>)

THEOREM ActiveScheduledRankStepIsUniverseEquivalent ==
  ProtectedServiceRankDecreaseStep
    <=> UniverseProtectedServiceRankDecreaseStep
BY Isa
   DEF ProtectedServiceRankDecreaseStep,
       UniverseProtectedServiceRankDecreaseStep,
       ActiveScheduledCandidates, ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateScheduled

THEOREM ActiveServeRankStepIsUniverseEquivalent ==
  ProtectedServeRankDecreaseStep
    <=> UniverseProtectedServeRankDecreaseStep
BY Isa
   DEF ProtectedServeRankDecreaseStep,
       UniverseProtectedServeRankDecreaseStep,
       ActiveIoJobs, ResponsiveProtectedServeJobOwned

(***************************************************************************
Executable fixed-action surface for immediate height productivity.

Only actions which occur in the post-GST weak-fairness inventory appear here.
This prevents an existential `ENABLED AsyncNext` query from selecting a fault,
an unfair auxiliary action, or an unrelated Core branch.  Each named action
already carries its exact runner/non-runner outer frame.  The live rank guards
range only over stored scheduler occurrences; the async proof module proves
them equivalent to the canonical protected-owner guards under the reachable
type invariant.
***************************************************************************)
PostGstProductiveSchedulerStep ==
  \/ AsyncTick
  \/ \E node \in AsyncCurrentResponsiveVoters:
       \/ PostGstRunNode(node)
       \/ PostGstCommitCertificateDiscovery(node)
  \/ \E node \in AsyncResponsiveAppliedArchiveServers:
       PostGstRunHistoricalServer(node)
  \/ \E node \in AsyncArchiveIoServiceNodes:
       PostGstServiceIoWorker(node)
  \/ \E node \in Responsive:
       \/ PostGstOpenHistoricalRecovery(node)
       \/ PostGstRunHistoricalRecoveryNode(node)
       \/ PostGstHistoricalCommitCertificateDiscovery(node)
       \/ PostGstServiceHistoricalRecoveryIoWorker(node)
  \/ \E recipient \in AsyncArchiveIoServiceNodes,
       source \in AsyncIngressSources:
       PostGstAdmitHiddenPacket(recipient, source)
  \/ \E recipient \in ValidatorIds,
       source \in AsyncIngressSources:
       PostGstAdmitHistoricalRecoveryPacket(recipient, source)

PostGstProductiveEffect ==
  \/ HeightProtocolEvidenceGrows
  \/ PostGstDeadlineDebtDecreases
  \/ PostGstNodeIoBlockerDebtDecreases
  \/ PostGstOverduePacketOwnershipExits
  \/ PostGstRuntimeReachDecreases
  \/ LiveProtectedServiceRankDecreaseStep
  \/ LiveProtectedServeRankDecreaseStep

PostGstProductiveStepWith(localWorkDecreaseStep) ==
  /\ gst
  /\ AsyncNext
  /\ \/ HeightProtocolEvidenceGrows
     \/ PostGstDeadlineDebtDecreases
     \/ ProtectedServiceRankDecreaseStep
     \/ ProtectedServeRankDecreaseStep
     \/ localWorkDecreaseStep

PostGstProductiveStep ==
  /\ gst
  /\ PostGstProductiveSchedulerStep
  /\ PostGstProductiveEffect

PostGstProductiveActionEnabled == ENABLED PostGstProductiveStep

DeadlockFreedomWithLocalWorkProperty(specification,
                                     productiveActionEnabled) ==
  specification
    => [](gst /\ ~ResponsiveNodesDecide
           => productiveActionEnabled)

DeadlockFreedomProperty(specification) ==
  DeadlockFreedomWithLocalWorkProperty(
    specification, PostGstSchedulerActionEnabled)

HeightProductivityFrontierProperty(specification) ==
  specification
    => (gst /\ ~ResponsiveNodesDecide)
         ~> (PostGstProductiveActionEnabled
               \/ ResponsiveNodesDecide)

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

ProtectedStage3RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<3, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<3, position>>))

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

ProtectedStage6RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<6, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<6, position>>))

ProtectedServeRankProgressProperty(specification) ==
  specification
    => \A node \in Responsive,
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
    => \A node \in Responsive,
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

(***************************************************************************
If validation of an exact TC-promoted lock completes after the node has
durably timed out its current finality view, the timeout intent owns the retry
frontier.  It does not authorize Commit in the closed view.  A later certified
view transition preserves the immutable lock origin and restarts recovery.
***************************************************************************)
ExactLockedCommitTimeoutRecoveryWitness(node, qc) ==
  /\ qc.context = context
  /\ qc.height = height
  /\ qc.view = lockRank[node]
  /\ qc.subject = lockSubject[node]
  /\ qc.view < nodeView[node]
  /\ \E timeoutVote \in timeoutIntents:
       /\ timeoutVote.signer = node
       /\ timeoutVote.context = qc.context
       /\ timeoutVote.height = qc.height
       /\ timeoutVote.view = nodeView[node]

(***************************************************************************
PrepareQCs with different signer-set encodings can certify the same semantic
lock.  LockAndCommit persistence is keyed by the canonical
node/context/view/subject request identity, so one pending WAL owner services
every such encoding; exact QcRecord equality would invent duplicate local
work and lose the owner when a later equivalent certificate is formed.
***************************************************************************)
HistoricalLockedCommitWalMatches(node, qc, request) ==
  /\ request.node = node
  /\ request.qc.context = qc.context
  /\ request.qc.phase = "Prepare"
  /\ request.qc.view = qc.view
  /\ request.qc.subject = qc.subject

(***************************************************************************
The historical BeginLock corridor has two exact evidence representations.
Restart/replay and direct locked-body validation retain a concrete Prepare
QcRecord and compare its stable recovery reference.  Certified-body recovery
instead carries the complete authenticated CertifiedResponse through
FetchCertifiedBody, StoreBody, and ValidateBody into BeginLockCommit.

The response hash binds the exact signed request and PrepareQC.  Physical
request routing is only a liveness mechanism: an authenticated archive which
learns the same signed request through reconnect or relay need not belong to
the original route set.  Archive signature ownership and the cited frozen-QC
signer remain separate roles.  The outer response source is only a transport
relay and is deliberately absent from the recovery predicate.
***************************************************************************)
HistoricalCertifiedResponseRecoveryEvidence(node, qc, evidence) ==
  /\ evidence \in AsyncNetworkItems
  /\ evidence.kind = "CertifiedResponse"
  /\ evidence.envelope.recipient = node
  /\ evidence.envelope.height = qc.context.height
  /\ evidence.envelope.view = qc.view
  /\ evidence.envelope.subject = qc.subject
  /\ evidence.envelope.requestHash =
       AsyncCertifiedRequestHashOf(node, qc, 0)
  /\ evidence.envelope.signatureOwner =
       evidence.envelope.archiveServer
  /\ evidence.envelope.citedResponder \in qc.signers
  /\ CertifiedResponseAuthenticatedOccurrence(evidence)

HistoricalBeginLockRecoveryEvidence(node, qc, evidence) ==
  \/ /\ evidence \in QcRecordSet
     /\ SamePrepareRecoveryRef(evidence, qc)
  \/ HistoricalCertifiedResponseRecoveryEvidence(node, qc, evidence)

HistoricalBeginLockRecoveryCandidate(node, qc, candidate) ==
  /\ candidate \in AsyncCandidateSet
  /\ candidate.node = node
  /\ candidate.height = qc.context.height
  /\ candidate.view = qc.view
  /\ candidate.subject = qc.subject
  /\ candidate.kind = "BeginLockCommit"
  /\ HistoricalBeginLockRecoveryEvidence(node, qc, candidate.evidence)
  /\ CandidateScheduled(candidate)

THEOREM HistoricalCertifiedResponseRecoveryEvidenceBindsExactIdentities ==
  \A node, qc, evidence:
    HistoricalCertifiedResponseRecoveryEvidence(node, qc, evidence)
      => /\ evidence \in AsyncNetworkItems
         /\ evidence.kind = "CertifiedResponse"
         /\ evidence.envelope.recipient = node
         /\ evidence.envelope.height = qc.context.height
         /\ evidence.envelope.view = qc.view
         /\ evidence.envelope.subject = qc.subject
         /\ evidence.envelope.requestHash =
              AsyncCertifiedRequestHashOf(node, qc, 0)
         /\ evidence.envelope.signatureOwner =
              evidence.envelope.archiveServer
         /\ evidence.envelope.citedResponder \in qc.signers
         /\ CertifiedResponseAuthenticatedOccurrence(evidence)
BY DEF HistoricalCertifiedResponseRecoveryEvidence

THEOREM SameReferenceQcProvidesHistoricalBeginLockRecoveryEvidence ==
  \A node, qc, evidence:
    /\ evidence \in QcRecordSet
    /\ SamePrepareRecoveryRef(evidence, qc)
    => HistoricalBeginLockRecoveryEvidence(node, qc, evidence)
BY DEF HistoricalBeginLockRecoveryEvidence

THEOREM AuthenticatedResponseProvidesHistoricalBeginLockRecoveryEvidence ==
  \A node, qc, evidence:
    HistoricalCertifiedResponseRecoveryEvidence(node, qc, evidence)
      => HistoricalBeginLockRecoveryEvidence(node, qc, evidence)
BY DEF HistoricalBeginLockRecoveryEvidence

HistoricalLockedCommitRecoveryWitness(node, qc) ==
  \/ ExactLockedCommitIntents(node, qc.view, qc.subject) # {}
  \/ \E request \in pendingLockCommit:
       HistoricalLockedCommitWalMatches(node, qc, request)
  \/ \E candidate \in AsyncCandidateSet:
       HistoricalBeginLockRecoveryCandidate(node, qc, candidate)
  \/ ExactLockedCommitTimeoutRecoveryWitness(node, qc)

(***************************************************************************
Once the TC-promoted historical lock has a current-generation durable
validation witness, the serialized reducer must already own either its exact
Commit intent, a WAL request for the same stable Prepare reference, or a
BeginLock successor carrying either a same-reference QcRecord or the exact
authenticated CertifiedResponse which survived the body pipeline.  If the
current finality view closed while validation was in flight, its exact durable
local timeout owns the retry until the next certified view transition.
Response matching uses the exact signed-request hash, and the whole outer
response remains the candidate evidence without making its relay source
authoritative.  The historical guard also forbids retroactively signing below
any higher local Prepare origin or known PrepareQC.
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
as Decision-pipeline progress.  PersistDecision classifies the exact local
body state and emits Fetch, Store, Validate, or Apply accordingly, while a
scheduled occurrence left behind by a view or generation change does not
count: the runtime will discard that stale occurrence instead of executing it.
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
            /\ generation'[request.node] =
                 generation[request.node] + 1
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

THEOREM AsyncLiveSpecProjectsAsyncSpec ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext) => AsyncSpecAt(initialContext)
BY DEF AsyncLiveSpecAt

OneHeightDecisionLiveness(initialContext) ==
  AsyncLiveSpecAt(initialContext)
    => PostGstEventuallyAsyncDecisionAt(initialContext)

OneHeightApplicationLiveness(initialContext) ==
  AsyncSpecAt(initialContext)
    => ResponsiveDecisionEventuallyAppliedAt(initialContext)

OneHeightCompletionLiveness(initialContext) ==
  AsyncLiveSpecAt(initialContext)
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
