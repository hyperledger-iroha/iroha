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

PostGstProgressActionEnabled ==
  \/ ENABLED AsyncTick
  \/ \E node \in AsyncCurrentResponsiveVoters:
       ENABLED PostGstRunNode(node)
  \/ \E node \in AsyncCurrentResponsiveVoters:
       ENABLED PostGstRunHistoricalServer(node)
  \/ \E node \in AsyncCurrentResponsiveVoters:
       ENABLED PostGstServiceIoWorker(node)
  \/ \E recipient \in AsyncCurrentResponsiveVoters,
       source \in AsyncCurrentResponsiveVoters:
       ENABLED PostGstAdmitHiddenPacket(recipient, source)

DeadlockFreedomProperty(specification) ==
  specification
    => [](gst /\ ~ResponsiveNodesDecide
           => PostGstProgressActionEnabled)

ProtectedServiceCandidate(candidate) ==
  /\ candidate \in AsyncCandidateSet
  /\ \/ candidate.class = "Completion"
     \/ /\ candidate.class = "Progress"
           /\ candidate.kind # "RejectProgress"

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

CandidateSequenceIndex(candidate, queue) ==
  CHOOSE index \in 1..Len(queue): queue[index] = candidate

CandidateIoIndex(candidate, queue) ==
  CHOOSE index \in 1..Len(queue): queue[index].candidate = candidate

CandidateInTransport(candidate) ==
  \E packet \in asyncTransport:
    DeliveryCandidate(packet.item) = candidate

CandidateInIngress(candidate) ==
  \E source \in AsyncIngressSources:
    candidate.item \in SequenceSet(
      IngressLane(candidate.node, source))

CandidateInIoQueue(candidate) ==
  \E job \in SequenceSet(asyncIoQueues[candidate.node]):
    job.candidate = candidate

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
            THEN <<4, AsyncCompletionLoad(candidate.node)>>
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

StarvationFreedomProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet:
         (gst /\ ResponsiveProtectedCandidateOwned(candidate))
           ~> ~ResponsiveProtectedCandidateOwned(candidate)

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

DecisionPipelineCandidate(node, qc, candidate) ==
  /\ candidate.node = node
  /\ candidate.height = qc.context.height
  /\ candidate.view = qc.view
  /\ candidate.subject = qc.subject
  /\ candidate.kind \in
       {"RequestCertifiedBody", "FetchCertifiedBody", "StoreBody",
        "ValidateBody", "Apply"}
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
    /\ Cardinality(ProtectedDeferredProgressIndices(node)) <= N + 3
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
