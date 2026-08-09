---- MODULE SumeragiV2AsyncFairServiceProofs ----
EXTENDS SumeragiV2AsyncRecoveryVoteEpochContinuationProofs

THEOREM VoteDeliveryConsumesCurrentPoolEpoch ==
  \A envelope:
    DeliverVote(envelope)
      => /\ VoteAt(envelope.recipient, envelope.vote)
               \notin receivedVotes
         /\ VoteAt(envelope.recipient, envelope.vote)
               \in receivedVotes'
BY SMT DEF DeliverVote

THEOREM VoteDeliveryRetainsAuthenticatedHistory ==
  \A envelope:
    DeliverVote(envelope) => voteNetwork' = voteNetwork
BY DEF DeliverVote

THEOREM ReceivedVoteSuppressesSameEpochRedelivery ==
  \A envelope:
    VoteAt(envelope.recipient, envelope.vote) \in receivedVotes
      => ~DeliverVote(envelope)
BY SMT DEF DeliverVote

THEOREM CompletedVoteSignatureReconstructsLocalPool ==
  \A request:
    CompleteVoteSignature(request)
      => VoteAt(request.node, request.vote) \in receivedVotes'
BY SMT DEF CompleteVoteSignature

THEOREM CompletedVoteSignatureExcludesLocalNetworkDelivery ==
  \A request:
    CompleteVoteSignature(request)
      => \A envelope \in BroadcastVotes(request.vote):
           envelope.recipient # request.node
BY SMT DEF CompleteVoteSignature, BroadcastVotes, VoteEnvelope

THEOREM InstallClearsLocalVolatileVotePool ==
  \A request:
    PersistInstallTC(request)
      => \A received \in receivedVotes':
           received.node # request.node
BY SMT DEF PersistInstallTC

THEOREM InstallPreservesOtherNodesVotePools ==
  \A request, received:
    (PersistInstallTC(request)
      /\ received \in receivedVotes
      /\ received.node # request.node)
      => received \in receivedVotes'
BY SMT DEF PersistInstallTC

THEOREM InstallOpensFreshVoteDeliveryPool ==
  \A request:
    PersistInstallTC(request)
      => \A envelope \in VoteEnvelopeSet:
           envelope.recipient = request.node
             => VoteAt(envelope.recipient, envelope.vote)
                  \notin receivedVotes'
BY SMT DEF PersistInstallTC, VoteAt

THEOREM InstallQueuesExactActiveLockedCommitForResigning ==
  \A request:
    PersistInstallTC(request)
      => ActiveLockedCommitSignRequestsAfterInstall(
           request.node, request.tc) \subseteq signVotes'
BY SMT DEF PersistInstallTC

THEOREM HonestVoteOutboxExcludesSigner ==
  \A request:
    \A item \in VoteOutbox(request):
      item.envelope.recipient # request.node
BY SMT DEF VoteOutbox, AsyncNetworkItem, VoteEnvelope

THEOREM InstallSchedulesAuthorizedCommitResignBeforeProposal ==
  \A command:
    /\ command.kind = "PersistInstallTC"
    /\ InstallCommitSignRequests(command) # {}
    => /\ IF InstallResultingLockedPrepareQCs(command) = {}
          THEN CommandSuccessors(command) =
                 <<InstallCommitSignSuccessor(command),
                   InstallProposalSuccessor(command)>>
          ELSE CommandSuccessors(command) =
                 <<InstallLockedFetchSuccessor(command),
                   InstallCommitSignSuccessor(command),
                   InstallProposalSuccessor(command)>>
       /\ InstallCommitSignSuccessor(command).kind = "SignVote"
       /\ InstallCommitSignSuccessor(command).class = "Completion"
BY Isa DEF CommandSuccessors, InstallCommandSuccessors,
           InstallLockedFetchSuccessors, InstallCommitSignSuccessors,
           InstallCommitSignSuccessor,
           AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
           AsyncCandidateWithIdentityAndOrigin,
           NoItemCandidate, AsyncCandidate

THEOREM InstallAdvancesDeliveryTag ==
  \A request:
    PersistInstallTC(request)
      => \/ nodeView'[request.node] > nodeView[request.node]
         \/ /\ nodeView'[request.node] = nodeView[request.node]
            /\ generation'[request.node] > generation[request.node]
BY Isa DEF PersistInstallTC

THEOREM PersistLockCommitPrunesSupersededVotePools ==
  \A request:
    PersistLockCommit(request)
      => \A received \in receivedVotes':
           VoteReceiptSurvivesLockCommit(
             received, request.node, request.qc.view,
             request.qc.subject)
BY SMT DEF PersistLockCommit, VoteReceiptSurvivesLockCommit

THEOREM PersistLockCommitPreservesSurvivingVotePools ==
  \A request, received:
    (/\ PersistLockCommit(request)
     /\ received \in receivedVotes
     /\ VoteReceiptSurvivesLockCommit(
          received, request.node, request.qc.view,
          request.qc.subject))
      => received \in receivedVotes'
BY SMT DEF PersistLockCommit, VoteReceiptSurvivesLockCommit

THEOREM PersistLockCommitPreservesOtherNodesVotePools ==
  \A request, received:
    (PersistLockCommit(request)
      /\ received \in receivedVotes
      /\ received.node # request.node)
      => received \in receivedVotes'
BY SMT DEF PersistLockCommit, VoteReceiptSurvivesLockCommit

THEOREM PersistLockCommitPreservesCurrentViewVotePools ==
  \A request, received:
    (PersistLockCommit(request)
      /\ received \in receivedVotes
      /\ received.node = request.node
      /\ received.vote.view = nodeView[request.node])
      => received \in receivedVotes'
BY SMT DEF PersistLockCommit, VoteReceiptSurvivesLockCommit

THEOREM PersistLockCommitPreservesExactLockedCommitPool ==
  \A request, received:
    (PersistLockCommit(request)
      /\ received \in receivedVotes
      /\ received.node = request.node
      /\ received.vote.phase = "Commit"
      /\ received.vote.view = request.qc.view
      /\ received.vote.subject = request.qc.subject)
      => received \in receivedVotes'
BY SMT DEF PersistLockCommit, VoteReceiptSurvivesLockCommit

THEOREM DeliveredVoteRebuildsItsRoundPool ==
  \A envelope:
    DeliverVote(envelope)
      => VoteAt(envelope.recipient, envelope.vote) \in receivedVotes'
BY SMT DEF DeliverVote

THEOREM VoteDeliveryEpochActionIsSemantic ==
  VoteDeliveryEpochAction
BY PrepareVoteAdmissionIsCurrentView,
   CommitVoteAdmissionIsExactLockedCommit,
   CommitFormationIsExactLockedRound,
   VoteDeliveryConsumesCurrentPoolEpoch,
   VoteDeliveryRetainsAuthenticatedHistory,
   CompletedVoteSignatureReconstructsLocalPool,
   CompletedVoteSignatureExcludesLocalNetworkDelivery,
   InstallClearsLocalVolatileVotePool,
   InstallQueuesExactActiveLockedCommitForResigning,
   InstallAdvancesDeliveryTag,
   PersistLockCommitPrunesSupersededVotePools,
   PersistLockCommitPreservesSurvivingVotePools
   DEF VoteDeliveryEpochAction

THEOREM AsyncStepHasVoteDeliveryEpochSemantics ==
  [AsyncNext]_AsyncAllVars
    => [VoteDeliveryEpochAction]_AsyncAllVars
BY VoteDeliveryEpochActionIsSemantic

THEOREM RetainedCommitVoteIsRetryable ==
  \A node, item:
    (item \in asyncRetainedControl
      /\ item.source = node
      /\ item.kind = "CommitVote")
      => item \in RetryableItems(node)
BY DEF RetryableItems, RetainedControlEmissionItems, SendableItems

(***************************************************************************
Concrete weak-fairness frontiers used by timeout progress.  These lemmas do
not assume an abstract scheduler: they expose ENABLED facts for the exact
worker actions named by `AsyncFairnessAt`.
***************************************************************************)

THEOREM QueuedIoEnablesPostGstService ==
  \A node \in AsyncArchiveIoServiceNodes:
    (AsyncTypeInvariant /\ gst /\ AsyncIoQueueDepth(node) > 0)
      => ENABLED PostGstServiceIoWorker(node)
BY ExpandENABLED, Isa
   DEF PostGstServiceIoWorker, ServiceIoWorker,
       PublishEphemeralItems, LeaveCausalQueues,
       AsyncIoQueueDepth, AsyncAllVars, AsyncSchedulerVars,
       AsyncLocalAdmissionVars, AsyncDeferredVars, vars

THEOREM QueuedIoServiceIsNonstuttering ==
  \A node \in AsyncArchiveIoServiceNodes:
    /\ AsyncTypeInvariant
    /\ AsyncIoQueueDepth(node) > 0
    /\ PostGstServiceIoWorker(node)
    => <<PostGstServiceIoWorker(node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW node \in AsyncArchiveIoServiceNodes,
                AsyncTypeInvariant,
                AsyncIoQueueDepth(node) > 0,
                PostGstServiceIoWorker(node)
         PROVE <<PostGstServiceIoWorker(node)>>_AsyncAllVars
    <2>1. /\ node \in ValidatorIds
           /\ AsyncIoSequenceTyped(asyncIoQueues[node])
      BY <1>1, AsyncArchiveIoServiceNodesAreValidators
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant
    <2>2. /\ asyncIoQueues[node] \in Seq(Range(asyncIoQueues[node]))
           /\ Len(asyncIoQueues[node]) > 0
      BY <1>1, <2>1
         DEF AsyncIoSequenceTyped, AsyncIoQueueDepth
    <2>3. /\ asyncIoQueues[node] # <<>>
           /\ Len(Tail(asyncIoQueues[node]))
                = Len(asyncIoQueues[node]) - 1
      BY <2>2, PositiveSequenceIsNonempty, HeadTailProperties
    <2>4. Tail(asyncIoQueues[node]) # asyncIoQueues[node]
      BY <2>2, <2>3, LenProperties, Isa
    <2>5. asyncIoQueues'[node] = Tail(asyncIoQueues[node])
      BY <1>1, <2>1
         DEF PostGstServiceIoWorker, ServiceIoWorker
    <2>6. asyncIoQueues' # asyncIoQueues
      BY <2>4, <2>5, Isa
    <2> QED BY <1>1, <2>6, Isa
         DEF AsyncAllVars, AsyncSchedulerVars
  <1> QED BY <1>1

(***************************************************************************
The IO worker is the smallest concrete protected-service frontier.  A
consensus completion at the FIFO head continuously enables the exact worker
named by `AsyncFairnessAt`; servicing that head removes a nonempty sequence
element and appends the candidate to the ready-completion queue.  The exit
predicate also records invariant or ownership loss, so its unless step is a
tautology rather than an assumed scheduler frame.  `AsyncTypeInvariant` later
rules out that auxiliary exit on every behavior of `AsyncSpecAt`.
***************************************************************************)

IoConsensusHead(node, candidate) ==
  /\ AsyncIoQueueDepth(node) > 0
  /\ Head(asyncIoQueues[node]) = AsyncIoConsensusJob(candidate)

IoCandidateIoReady(node, candidate) ==
  candidate \in SequenceSet(asyncIoReadyCompletions[node])

THEOREM IoConsensusHeadServiceIsNonstuttering ==
  \A node \in AsyncCurrentResponsiveVoters,
     candidate \in AsyncCandidateSet:
    (AsyncTypeInvariant /\ IoConsensusHead(node, candidate)
      /\ PostGstServiceIoWorker(node))
      => <<PostGstServiceIoWorker(node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                NEW candidate \in AsyncCandidateSet,
                AsyncTypeInvariant,
                IoConsensusHead(node, candidate),
                PostGstServiceIoWorker(node)
         PROVE <<PostGstServiceIoWorker(node)>>_AsyncAllVars
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>2. AsyncIoSequenceTyped(asyncIoQueues[node])
      BY <1>1, <2>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant
    <2>3. asyncIoQueues[node] \in Seq(Range(asyncIoQueues[node]))
      BY <2>2 DEF AsyncIoSequenceTyped
    <2>4. Len(asyncIoQueues[node]) > 0
      BY <1>1 DEF IoConsensusHead, AsyncIoQueueDepth
    <2>5. asyncIoQueues[node] # <<>>
      BY <2>3, <2>4, PositiveSequenceIsNonempty
    <2>6. Len(Tail(asyncIoQueues[node])) =
             Len(asyncIoQueues[node]) - 1
      BY <2>3, <2>5, HeadTailProperties
    <2>7. Len(asyncIoQueues[node]) \in Nat
      BY <2>3, LenProperties
    <2>8. Tail(asyncIoQueues[node]) # asyncIoQueues[node]
      BY <2>4, <2>6, <2>7, SMT
    <2>9. DOMAIN asyncIoQueues = ValidatorIds
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant
    <2>10. asyncIoQueues'[node] = Tail(asyncIoQueues[node])
      BY <1>1, <2>1, <2>9, Isa
         DEF PostGstServiceIoWorker, ServiceIoWorker
    <2>11. asyncIoQueues' # asyncIoQueues
      BY <2>8, <2>10, Isa
    <2> QED BY <1>1, <2>11, Isa
       DEF AsyncAllVars, AsyncSchedulerVars
  <1> QED BY <1>1

THEOREM IoConsensusHeadEnablesFairService ==
  \A node \in AsyncCurrentResponsiveVoters,
     candidate \in AsyncCandidateSet:
    (AsyncTypeInvariant /\ gst /\ IoConsensusHead(node, candidate))
      => ENABLED <<PostGstServiceIoWorker(node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                NEW candidate \in AsyncCandidateSet,
                AsyncTypeInvariant,
                gst,
                IoConsensusHead(node, candidate)
         PROVE ENABLED <<PostGstServiceIoWorker(node)>>_AsyncAllVars
    <2>1. ENABLED PostGstServiceIoWorker(node)
      BY <1>1, QueuedIoEnablesPostGstService
         DEF IoConsensusHead, AsyncArchiveIoServiceNodes
    <2>2. PostGstServiceIoWorker(node) \in BOOLEAN
      BY Isa DEF PostGstServiceIoWorker
    <2>3. <<PostGstServiceIoWorker(node)>>_AsyncAllVars \in BOOLEAN
      BY Isa
    <2>4. PostGstServiceIoWorker(node)
             => <<PostGstServiceIoWorker(node)>>_AsyncAllVars
      BY <1>1, IoConsensusHeadServiceIsNonstuttering
    <2>5. (ENABLED PostGstServiceIoWorker(node))
             => ENABLED <<PostGstServiceIoWorker(node)>>_AsyncAllVars
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

THEOREM ServiceIoConsensusHeadMakesCandidateReady ==
  \A node \in AsyncCurrentResponsiveVoters,
     candidate \in AsyncCandidateSet:
    (AsyncTypeInvariant
      /\ IoConsensusHead(node, candidate)
      /\ PostGstServiceIoWorker(node))
      => IoCandidateIoReady(node, candidate)'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                NEW candidate \in AsyncCandidateSet,
                AsyncTypeInvariant,
                IoConsensusHead(node, candidate),
                PostGstServiceIoWorker(node)
         PROVE IoCandidateIoReady(node, candidate)'
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>2. DOMAIN asyncIoReadyCompletions = ValidatorIds
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant
    <2>3. AsyncCompletionSequenceTyped(
             asyncIoReadyCompletions[node])
      BY <1>1, <2>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoWorkContentTypeInvariant
    <2>4. asyncIoReadyCompletions[node]
             \in Seq(Range(asyncIoReadyCompletions[node]))
      BY <2>3 DEF AsyncCompletionSequenceTyped
    <2>5. /\ Head(asyncIoQueues[node]).class = "Consensus"
           /\ Head(asyncIoQueues[node]).candidate = candidate
      BY <1>1 DEF IoConsensusHead, AsyncIoConsensusJob, AsyncIoJob
    <2>6. asyncIoReadyCompletions'[node] =
             Append(asyncIoReadyCompletions[node], candidate)
      BY <1>1, <2>1, <2>2, <2>5, Isa
         DEF PostGstServiceIoWorker, ServiceIoWorker
    <2>7. candidate \in
             SequenceSet(Append(asyncIoReadyCompletions[node], candidate))
      BY <2>4, SequenceSetAfterAppend, Isa
    <2> QED BY <2>6, <2>7 DEF IoCandidateIoReady
  <1> QED BY <1>1

IoConsensusHeadServicePending(node, candidate) ==
  /\ AsyncTypeInvariant
  /\ gst
  /\ node \in AsyncCurrentResponsiveVoters
  /\ IoConsensusHead(node, candidate)

IoConsensusHeadServiceExit(node, candidate) ==
  ~IoConsensusHeadServicePending(node, candidate)
    \/ IoCandidateIoReady(node, candidate)

THEOREM FairIoConsensusHeadService ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => \A node \in AsyncVotersAt(initialContext),
            candidate \in AsyncCandidateSet:
           IoConsensusHeadServicePending(node, candidate)
             ~> IoConsensusHeadServiceExit(node, candidate)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => \A node \in AsyncVotersAt(initialContext),
                       candidate \in AsyncCandidateSet:
                      IoConsensusHeadServicePending(node, candidate)
                        ~> IoConsensusHeadServiceExit(node, candidate)
    <2>1. ASSUME NEW node \in AsyncVotersAt(initialContext),
                  NEW candidate \in AsyncCandidateSet
           PROVE AsyncSpecAt(initialContext)
                   => (IoConsensusHeadServicePending(node, candidate)
                         ~> IoConsensusHeadServiceExit(node, candidate))
      <3>1. (IoConsensusHeadServicePending(node, candidate)
                /\ ~IoConsensusHeadServiceExit(node, candidate))
               => ENABLED
                    <<PostGstServiceIoWorker(node)>>_AsyncAllVars
        BY IoConsensusHeadEnablesFairService
           DEF IoConsensusHeadServicePending
      <3>2. (IoConsensusHeadServicePending(node, candidate)
                /\ ~IoConsensusHeadServiceExit(node, candidate)
                /\ <<PostGstServiceIoWorker(node)>>_AsyncAllVars)
               => IoConsensusHeadServiceExit(node, candidate)'
        BY ServiceIoConsensusHeadMakesCandidateReady, Isa
           DEF IoConsensusHeadServicePending,
               IoConsensusHeadServiceExit
      <3>3. IoConsensusHeadServicePending(node, candidate)
                /\ [AsyncNext]_AsyncAllVars
               => IoConsensusHeadServicePending(node, candidate)'
                    \/ IoConsensusHeadServiceExit(node, candidate)'
        BY PTL DEF IoConsensusHeadServiceExit
      <3>4. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(PostGstServiceIoWorker(node))
        BY <2>1 DEF AsyncSpecAt, AsyncFairnessAt, AsyncVotersAt
      <3> QED BY <3>1, <3>2, <3>3, <3>4, PTL
           DEF AsyncSpecAt
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM LocalProducerAdmissionIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Local"
    /\ ~AsyncIngressSchedulerBarrierActive(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Producer"
    => ENABLED LocalAdmissionStep(node)
BY SelectedProducerCanAdvance, ExpandENABLED, Isa
   DEF LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, OtherLocalSource,
       EnqueueCandidate, LeaveCausalQueues, AsyncIoVars,
       AsyncDeferredVars, vars

THEOREM LocalDuplicateCausalAdmissionIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Local"
    /\ ~AsyncIngressSchedulerBarrierActive(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Causal"
    /\ CandidateInFlight(HeadCausalCandidate(node))
    => ENABLED LocalAdmissionStep(node)
BY SelectedCausalCanAdvance, ExpandENABLED, Isa
   DEF LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, OtherLocalSource,
       EnqueueCandidate, LeaveCausalQueues, AsyncIoVars,
       AsyncDeferredVars, vars

LocalCompletionCausalStep(node) ==
  LET candidate == HeadCausalCandidate(node)
  IN /\ asyncCausalQueues' =
           [asyncCausalQueues EXCEPT ![node] = Tail(@)]
     /\ asyncIoQueues' =
          [asyncIoQueues EXCEPT
             ![node] = Append(@, AsyncIoConsensusJob(candidate))]
     /\ asyncOutstandingWork' =
          [asyncOutstandingWork EXCEPT ![node] = @ \cup {candidate}]
     /\ UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                     asyncIoReadyCompletions,
                     asyncLocalReadyCompletions,
                     asyncNextCompletionSource,
                     asyncIoControlAvailable>>
     /\ UNCHANGED <<vars, asyncFifoOwed, asyncTimeoutEmitted,
                     asyncOutstandingTags, asyncNodeDeadlines,
                     asyncRetransmitDeadlines, asyncSentItems,
                     asyncRetainedControl, asyncActiveRequests,
                     asyncCertifiedResponseClaim,
                     asyncTransport, asyncIngressLanes,
                     asyncIngressReady, asyncHeldChunks,
                     asyncHistoricalRecoveryTargets>>
  /\ UNCHANGED AsyncDeferredVars
  /\ UpdateLocalAdmissionMetadata(node, "Causal")
  /\ asyncRunnerPhase' = asyncRunnerPhase
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = @ - 1]

THEOREM LocalCompletionCausalAdmissionIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Local"
    /\ ~AsyncIngressSchedulerBarrierActive(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Causal"
    /\ ~CandidateInFlight(HeadCausalCandidate(node))
    /\ HeadCausalCandidate(node).class = "Completion"
    => ENABLED LocalAdmissionStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Local"
                /\ ~AsyncIngressSchedulerBarrierActive(node)
                /\ LocalAdmissionCanAdvance(node)
                /\ SelectedLocalSource(node) = "Causal"
                /\ ~CandidateInFlight(HeadCausalCandidate(node))
                /\ HeadCausalCandidate(node).class = "Completion"
         PROVE ENABLED LocalAdmissionStep(node)
    <2>1. ENABLED LocalCompletionCausalStep(node)
      BY <1>1, SelectedCausalCanAdvance, ExpandENABLED, Isa
         DEF LocalCompletionCausalStep, UpdateLocalAdmissionMetadata,
             OtherLocalSource, AsyncDeferredVars, vars
    <2>2. LocalCompletionCausalStep(node) \in BOOLEAN
      BY Isa DEF LocalCompletionCausalStep,
                 UpdateLocalAdmissionMetadata, OtherLocalSource
    <2>3. LocalAdmissionStep(node) \in BOOLEAN
      BY Isa DEF LocalAdmissionStep
    <2>4. LocalCompletionCausalStep(node)
             => LocalAdmissionStep(node)
      BY <1>1, SelectedCausalCanAdvance, Isa
         DEF LocalCompletionCausalStep, LocalAdmissionStep,
             AdmitCausalHead, AdmitProducerCompletion,
             UpdateLocalAdmissionMetadata, OtherLocalSource,
             EnqueueCandidate, LeaveCausalQueues,
             AsyncLocalAdmissionVars, AsyncIoVars, AsyncDeferredVars, vars
    <2>5. ENABLED LocalCompletionCausalStep(node)
             => ENABLED LocalAdmissionStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

LocalCommandCausalStep(node) ==
  LET candidate == HeadCausalCandidate(node)
  IN /\ asyncCausalQueues' =
           [asyncCausalQueues EXCEPT ![node] = Tail(@)]
     /\ asyncCommandQueues' =
          [asyncCommandQueues EXCEPT
             ![candidate.node] = Append(@, candidate)]
     /\ UNCHANGED asyncNextCommandClass
     /\ UNCHANGED AsyncIoVars
     /\ UNCHANGED <<vars, asyncFifoOwed, asyncTimeoutEmitted,
                     asyncOutstandingTags, asyncNodeDeadlines,
                     asyncRetransmitDeadlines, asyncSentItems,
                     asyncRetainedControl, asyncActiveRequests,
                     asyncCertifiedResponseClaim,
                     asyncTransport, asyncIngressLanes,
                     asyncIngressReady, asyncHeldChunks,
                     asyncHistoricalRecoveryTargets>>
     /\ UNCHANGED AsyncDeferredVars
     /\ UpdateLocalAdmissionMetadata(node, "Causal")
     /\ asyncRunnerPhase' = asyncRunnerPhase
     /\ asyncRunnerBudget' =
          [asyncRunnerBudget EXCEPT ![node] = @ - 1]

THEOREM LocalCommandCausalAdmissionIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Local"
    /\ ~AsyncIngressSchedulerBarrierActive(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Causal"
    /\ ~CandidateInFlight(HeadCausalCandidate(node))
    /\ HeadCausalCandidate(node).class # "Completion"
    => ENABLED LocalAdmissionStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Local"
                /\ ~AsyncIngressSchedulerBarrierActive(node)
                /\ LocalAdmissionCanAdvance(node)
                /\ SelectedLocalSource(node) = "Causal"
                /\ ~CandidateInFlight(HeadCausalCandidate(node))
                /\ HeadCausalCandidate(node).class # "Completion"
         PROVE ENABLED LocalAdmissionStep(node)
    <2>1. ENABLED LocalCommandCausalStep(node)
      BY <1>1, SelectedCausalCanAdvance, ExpandENABLED, Isa
         DEF LocalCommandCausalStep, AsyncIoVars,
             UpdateLocalAdmissionMetadata, OtherLocalSource,
             AsyncDeferredVars, vars
    <2>2. LocalCommandCausalStep(node) \in BOOLEAN
      BY Isa DEF LocalCommandCausalStep,
                 UpdateLocalAdmissionMetadata, OtherLocalSource
    <2>3. LocalAdmissionStep(node) \in BOOLEAN
      BY Isa DEF LocalAdmissionStep
    <2>4. LocalCommandCausalStep(node)
             => LocalAdmissionStep(node)
      BY <1>1, SelectedCausalCanAdvance, Isa
         DEF LocalCommandCausalStep, LocalAdmissionStep,
             AdmitCausalHead, AdmitProducerCompletion,
             UpdateLocalAdmissionMetadata, OtherLocalSource,
             EnqueueCandidate, LeaveCausalQueues,
             AsyncLocalAdmissionVars, AsyncIoVars, AsyncDeferredVars, vars
    <2>5. ENABLED LocalCommandCausalStep(node)
             => ENABLED LocalAdmissionStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

THEOREM LocalCausalAdmissionIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Local"
    /\ ~AsyncIngressSchedulerBarrierActive(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Causal"
    => ENABLED LocalAdmissionStep(node)
BY LocalDuplicateCausalAdmissionIsEnabled,
   LocalCompletionCausalAdmissionIsEnabled,
   LocalCommandCausalAdmissionIsEnabled, Isa

LocalPhaseAdvanceStep(node) ==
  /\ ~AsyncIngressSchedulerBarrierActive(node)
  /\ UNCHANGED AsyncDeferredVars
  /\ LeaveCausalQueues
  /\ RecordBlockedCausalDebt(node)
  /\ UNCHANGED <<vars, asyncCommandQueues,
                  asyncNextCommandClass, asyncFifoOwed,
                  asyncTimeoutEmitted, AsyncIoVars,
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
       [asyncRunnerBudget EXCEPT ![node] = AsyncIngressCapacity]

THEOREM LocalPhaseAdvanceIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Local"
    /\ ~AsyncIngressSchedulerBarrierActive(node)
    /\ ~LocalAdmissionCanAdvance(node)
    => ENABLED LocalAdmissionStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Local"
                /\ ~AsyncIngressSchedulerBarrierActive(node)
                /\ ~LocalAdmissionCanAdvance(node)
         PROVE ENABLED LocalAdmissionStep(node)
    <2>1. ENABLED LocalPhaseAdvanceStep(node)
      BY ExpandENABLED, Isa
         DEF LocalPhaseAdvanceStep, LeaveCausalQueues,
             RecordBlockedCausalDebt, AsyncIoVars,
             AsyncDeferredVars, vars
    <2>2. LocalPhaseAdvanceStep(node) \in BOOLEAN
      BY Isa DEF LocalPhaseAdvanceStep
    <2>3. LocalAdmissionStep(node) \in BOOLEAN
      BY Isa DEF LocalAdmissionStep
    <2>4. LocalPhaseAdvanceStep(node) => LocalAdmissionStep(node)
      BY <1>1, Isa
         DEF LocalPhaseAdvanceStep, LocalAdmissionStep,
             AdmitProducerCompletion, AdmitCausalHead,
             UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
             EnqueueCandidate,
             LeaveCausalQueues,
             AsyncLocalAdmissionVars, AsyncIoVars, AsyncDeferredVars, vars
    <2>5. ENABLED LocalPhaseAdvanceStep(node)
             => ENABLED LocalAdmissionStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

THEOREM LocalAdmissionStepIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Local"
    /\ ~AsyncIngressSchedulerBarrierActive(node)
      => ENABLED LocalAdmissionStep(node)
BY LocalProducerAdmissionIsEnabled,
   LocalCausalAdmissionIsEnabled,
   LocalPhaseAdvanceIsEnabled, Isa
   DEF SelectedLocalSource, PreferredLocalSource,
       OtherLocalSource

IngressSelectedDrainStep(node) ==
  /\ DrainFairIngressSelected(node)
  /\ UNCHANGED AsyncDeferredVars
  /\ LeaveCausalQueues
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ asyncRunnerPhase' = asyncRunnerPhase
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = @ - 1]

SelectedIngressItem(node) ==
  SelectedIngressItemAt(node, FirstDrainableIngressIndex(node))

SelectedIngressDrops(node) ==
  LET item == SelectedIngressItem(node)
  IN item.kind = "Noise" \/ ~IngressItemHasAuthenticatedHistory(item)

SelectedIngressIsRequest(node) ==
  SelectedIngressItem(node).kind
    \in {"CertifiedRequest", "CommitCertificateRequest"}

SelectedIngressRequestAuthorized(node) ==
  LET item == SelectedIngressItem(node)
  IN IF item.kind = "CertifiedRequest"
     THEN CertifiedRequestAuthorized(item)
     ELSE CommitCertificateRequestAuthorized(item)

THEOREM IngressSelectedDrainIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Ingress"
    /\ asyncRunnerBudget[node] > 0
    /\ asyncIngressReady[node] # <<>>
    /\ DrainableIngressIndices(node) # {}
    => ENABLED IngressDrainStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Ingress"
                /\ asyncRunnerBudget[node] > 0
                /\ asyncIngressReady[node] # <<>>
                /\ DrainableIngressIndices(node) # {}
         PROVE ENABLED IngressDrainStep(node)
    <2>1. ENABLED IngressSelectedDrainStep(node)
      <3>1. FirstDrainableIngressIndex(node)
               \in DrainableIngressIndices(node)
        BY <1>1, FirstDrainableIngressIndexIsDrainable
      <3>10. FirstDrainableIngressIndex(node)
                \in 1..Len(asyncIngressReady[node])
        BY <3>1 DEF DrainableIngressIndices
      <3>11. /\ SelectedIngressLaneIndex(
                       node, FirstDrainableIngressIndex(node))
                     \in 1..Len(IngressLane(
                          node,
                          asyncIngressReady[node][
                            FirstDrainableIngressIndex(node)]))
                /\ IngressItemCanDrain(
                     node, SelectedIngressItem(node))
        BY <3>1, FirstDrainableIngressLaneIndexIsDrainable
           DEF DrainableIngressIndices, IngressSourceCanDrain,
               DrainableIngressLaneIndices, SelectedIngressLaneIndex,
               SelectedIngressItem, SelectedIngressItemAt
      <3>2. CASE SelectedIngressDrops(node)
        BY <1>1, <3>1, <3>10, <3>11, <3>2, ExpandENABLED, Isa
           DEF SelectedIngressDrops, SelectedIngressItem,
               IngressItemCanDrain,
               IngressSelectedDrainStep, DrainFairIngressSelected,
               PopSelectedIngress, EnqueueCandidate,
               LeaveCausalQueues, AsyncIoVars, AsyncDeferredVars, vars
      <3>3. CASE /\ ~SelectedIngressDrops(node)
                    /\ SelectedIngressIsRequest(node)
                    /\ SelectedIngressRequestAuthorized(node)
        BY <1>1, <3>1, <3>10, <3>11, <3>3, ExpandENABLED, Isa
           DEF SelectedIngressDrops, SelectedIngressIsRequest,
               SelectedIngressRequestAuthorized, SelectedIngressItem,
               IngressItemCanDrain,
               IngressSelectedDrainStep, DrainFairIngressSelected,
               PopSelectedIngress, EnqueueCandidate,
               LeaveCausalQueues, AsyncIoVars, AsyncDeferredVars, vars
      <3>4. CASE /\ ~SelectedIngressDrops(node)
                    /\ SelectedIngressIsRequest(node)
                    /\ ~SelectedIngressRequestAuthorized(node)
        BY <1>1, <3>1, <3>10, <3>11, <3>4, ExpandENABLED, Isa
           DEF SelectedIngressDrops, SelectedIngressIsRequest,
               SelectedIngressRequestAuthorized, SelectedIngressItem,
               IngressItemCanDrain,
               IngressSelectedDrainStep, DrainFairIngressSelected,
               PopSelectedIngress, EnqueueCandidate,
               LeaveCausalQueues, AsyncIoVars, AsyncDeferredVars, vars
      <3>5. CASE /\ ~SelectedIngressDrops(node)
                    /\ ~SelectedIngressIsRequest(node)
                    /\ SelectedIngressItem(node).kind =
                         "CertifiedResponse"
                    /\ CertifiedResponseClaimAuthorized(
                         SelectedIngressItem(node))
        BY <1>1, <3>1, <3>10, <3>11, <3>5, ExpandENABLED, Isa
           DEF SelectedIngressDrops, SelectedIngressIsRequest,
               SelectedIngressItem, IngressItemCanDrain,
               IngressSelectedDrainStep,
               DrainFairIngressSelected, PopSelectedIngress,
               EnqueueCandidate, LeaveCausalQueues,
               AsyncIoVars, AsyncDeferredVars, vars
      <3>6. CASE /\ ~SelectedIngressDrops(node)
                    /\ ~SelectedIngressIsRequest(node)
                    /\ SelectedIngressItem(node).kind =
                         "CertifiedResponse"
                    /\ ~CertifiedResponseClaimAuthorized(
                          SelectedIngressItem(node))
        BY <1>1, <3>1, <3>10, <3>11, <3>6, ExpandENABLED, Isa
           DEF SelectedIngressDrops, SelectedIngressIsRequest,
               SelectedIngressItem, IngressItemCanDrain,
               IngressSelectedDrainStep,
               DrainFairIngressSelected, PopSelectedIngress,
               EnqueueCandidate, LeaveCausalQueues,
               AsyncIoVars, AsyncDeferredVars, vars
      <3>7. CASE /\ ~SelectedIngressDrops(node)
                    /\ ~SelectedIngressIsRequest(node)
                    /\ SelectedIngressItem(node).kind #
                         "CertifiedResponse"
                    /\ SelectedIngressItem(node).kind =
                         "CommitCertificateResponse"
                    /\ CommitCertificateResponseAuthorized(
                         SelectedIngressItem(node))
        BY <1>1, <3>1, <3>10, <3>11, <3>7, ExpandENABLED, Isa
           DEF SelectedIngressDrops, SelectedIngressIsRequest,
               SelectedIngressItem, IngressItemCanDrain,
               IngressSelectedDrainStep,
               DrainFairIngressSelected, PopSelectedIngress,
               EnqueueCandidate, LeaveCausalQueues,
               AsyncIoVars, AsyncDeferredVars, vars
      <3>8. CASE /\ ~SelectedIngressDrops(node)
                    /\ ~SelectedIngressIsRequest(node)
                    /\ SelectedIngressItem(node).kind #
                         "CertifiedResponse"
                    /\ SelectedIngressItem(node).kind =
                         "CommitCertificateResponse"
                    /\ ~CommitCertificateResponseAuthorized(
                          SelectedIngressItem(node))
        BY <1>1, <3>1, <3>10, <3>11, <3>8, ExpandENABLED, Isa
           DEF SelectedIngressDrops, SelectedIngressIsRequest,
               SelectedIngressItem, IngressItemCanDrain,
               IngressSelectedDrainStep,
               DrainFairIngressSelected, PopSelectedIngress,
               EnqueueCandidate, LeaveCausalQueues,
               AsyncIoVars, AsyncDeferredVars, vars
      <3>9. CASE /\ ~SelectedIngressDrops(node)
                    /\ ~SelectedIngressIsRequest(node)
                    /\ SelectedIngressItem(node).kind #
                         "CertifiedResponse"
                    /\ SelectedIngressItem(node).kind #
                         "CommitCertificateResponse"
        BY <1>1, <3>1, <3>10, <3>11, <3>9, ExpandENABLED, Isa
           DEF SelectedIngressDrops, SelectedIngressIsRequest,
               SelectedIngressItem, IngressItemCanDrain,
               IngressSelectedDrainStep,
               DrainFairIngressSelected, PopSelectedIngress,
               EnqueueCandidate, LeaveCausalQueues,
               AsyncIoVars, AsyncDeferredVars, vars
      <3> QED BY <3>2, <3>3, <3>4, <3>5,
                    <3>6, <3>7, <3>8, <3>9
    <2>2. IngressSelectedDrainStep(node) \in BOOLEAN
      BY Isa DEF IngressSelectedDrainStep, DrainFairIngressSelected
    <2>3. IngressDrainStep(node) \in BOOLEAN
      BY Isa DEF IngressDrainStep
    <2>4. IngressSelectedDrainStep(node) => IngressDrainStep(node)
      BY <1>1, Isa
         DEF IngressSelectedDrainStep, IngressDrainStep,
             DrainFairIngressSelected, PopSelectedIngress,
             EnqueueCandidate, LeaveCausalQueues,
             AsyncIoVars, AsyncDeferredVars, vars
    <2>5. ENABLED IngressSelectedDrainStep(node)
             => ENABLED IngressDrainStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

IngressPhaseAdvanceStep(node) ==
  /\ UNCHANGED AsyncDeferredVars
  /\ LeaveCausalQueues
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ UNCHANGED <<vars, asyncCommandQueues,
                  asyncNextCommandClass, asyncFifoOwed,
                  asyncTimeoutEmitted, AsyncIoVars,
                  asyncOutstandingTags, asyncNodeDeadlines,
                  asyncRetransmitDeadlines, asyncSentItems,
                  asyncRetainedControl, asyncActiveRequests,
                  asyncCertifiedResponseClaim,
                  asyncTransport, asyncIngressLanes,
                  asyncIngressReady, asyncHeldChunks,
                  asyncHistoricalRecoveryTargets>>
  /\ asyncRunnerPhase' =
       [asyncRunnerPhase EXCEPT ![node] = "Runtime"]
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = 1]

THEOREM IngressPhaseAdvanceIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Ingress"
    /\ ~(asyncRunnerBudget[node] > 0
          /\ asyncIngressReady[node] # <<>>
          /\ DrainableIngressIndices(node) # {})
    => ENABLED IngressDrainStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Ingress"
                /\ ~(asyncRunnerBudget[node] > 0
                      /\ asyncIngressReady[node] # <<>>
                      /\ DrainableIngressIndices(node) # {})
         PROVE ENABLED IngressDrainStep(node)
    <2>1. ENABLED IngressPhaseAdvanceStep(node)
      BY ExpandENABLED, Isa
         DEF IngressPhaseAdvanceStep, LeaveCausalQueues,
             AsyncIoVars, AsyncDeferredVars, vars
    <2>2. IngressPhaseAdvanceStep(node) \in BOOLEAN
      BY Isa DEF IngressPhaseAdvanceStep
    <2>3. IngressDrainStep(node) \in BOOLEAN
      BY Isa DEF IngressDrainStep
    <2>4. IngressPhaseAdvanceStep(node) => IngressDrainStep(node)
      BY <1>1, Isa
         DEF IngressPhaseAdvanceStep, IngressDrainStep,
             DrainFairIngressSelected, PopSelectedIngress,
             EnqueueCandidate, LeaveCausalQueues,
             AsyncIoVars, AsyncDeferredVars, vars
    <2>5. ENABLED IngressPhaseAdvanceStep(node)
             => ENABLED IngressDrainStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

THEOREM IngressDrainStepIsEnabled ==
  \A node \in ValidatorIds:
    asyncRunnerPhase[node] = "Ingress"
      => ENABLED IngressDrainStep(node)
BY IngressSelectedDrainIsEnabled,
   IngressPhaseAdvanceIsEnabled, Isa

(***************************************************************************
The recurring discovery prefix has an independent concrete successor.  The
serialized runtime relation below is a priority union, but every selected arm
also has a concrete successor.  These small lemmas keep both outer-loop action
boundaries and the runtime priority tests visible in enabledness proofs.
***************************************************************************)

THEOREM CommitCertificateDiscoveryPrefixIsEnabled ==
  \A node \in ValidatorIds:
    CommitCertificateDiscoveryDue(node)
      => ENABLED DirectCommitCertificateDiscoveryStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                CommitCertificateDiscoveryDue(node)
         PROVE ENABLED DirectCommitCertificateDiscoveryStep(node)
    <2>1. ENABLED DirectCommitCertificateDiscoveryStep(node)
      BY <1>1, ExpandENABLED, Isa
         DEF DirectCommitCertificateDiscoveryStep,
             PublishCommitCertificateRequests, LeaveCausalQueues,
             AsyncIoVars, AsyncDeferredVars, vars
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM GstCoreStepIsMonotone ==
  gst /\ [Next]_vars => gst'
BY IsaM("blast")
   DEF Next, SetGST, AssembleLocalBody, BeginLocalProposal, PersistProposal,
       CompleteProposalSignature, ByzantineBroadcastProposal,
       DeliverProposal, FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody,
       ValidateDecidedBody, ValidateLockedBody, RejectBody,
       BeginPrepare, PersistPrepare,
       CompleteVoteSignature, ByzantineBroadcastVote, DeliverVote,
       FormPrepareQC, DeliverQC, BeginObservePrepare,
       PersistObservePrepare, BeginLockCommit, PersistLockCommit,
       FormCommitQC, BeginDecision, PersistDecision, BeginTimeout,
       PersistTimeout, CompleteTimeoutSignature, ByzantineBroadcastTimeout,
       DeliverTimeout, FormTC, DeliverTC, BeginInstallTC, PersistInstallTC,
       FetchCertifiedBody, ApplyDecision, Crash, Restart, ResumeProposal,
       ResumeVote, ResumeTimeout, DropProposal, vars

THEOREM GstAsyncStepIsMonotone ==
  gst /\ [AsyncNext]_AsyncAllVars => gst'
PROOF
  <1>1. ASSUME gst, [AsyncNext]_AsyncAllVars
         PROVE gst'
    <2>1. CASE AsyncNext
      <3>1. [Next]_vars
        BY <2>1, AsyncStepRefinementObligation
      <3> QED BY <1>1, <3>1, GstCoreStepIsMonotone
    <2>2. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>2, Isa DEF AsyncAllVars, vars
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

CommitCertificateDiscoveryPending(node) ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ CommitCertificateDiscoveryDue(node)

CommitCertificateDiscoveryOutcome(node) ==
  \/ NodeHasDecision(node)
  \/ ActiveCommitCertificateRequests(node) # {}

THEOREM DirectCommitCertificateDiscoveryPublishes ==
  \A node \in ValidatorIds:
    DirectCommitCertificateDiscoveryStep(node)
      => ActiveCommitCertificateRequests(node)' # {}
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                DirectCommitCertificateDiscoveryStep(node)
         PROVE ActiveCommitCertificateRequests(node)' # {}
    <2>1. /\ CommitCertificateRequestOutbox(node) # {}
           /\ asyncActiveRequests' =
                asyncActiveRequests
                  \cup CommitCertificateRequestOutbox(node)
      BY <1>1
         DEF DirectCommitCertificateDiscoveryStep,
             CommitCertificateDiscoveryDue,
             PublishCommitCertificateRequests
    <2>2. \A item \in CommitCertificateRequestOutbox(node):
             /\ item.source = node
             /\ item.kind = "CommitCertificateRequest"
      BY Isa
         DEF CommitCertificateRequestOutbox, AsyncNetworkItem
    <2> QED BY <2>1, <2>2, Isa
         DEF ActiveCommitCertificateRequests
  <1> QED BY <1>1

THEOREM CommitCertificateRequestOutboxNonemptyIffRemoteVoter ==
  \A node:
    (CommitCertificateRequestOutbox(node) # {})
      <=> (CurrentVoters \ {node}) # {}
BY Isa
   DEF CommitCertificateRequestOutbox, AsyncNetworkItem

THEOREM AsyncRunnerStepLeavesDiscoveryClock ==
  AsyncRunnerStep => asyncNow' = asyncNow
BY Isa
   DEF AsyncRunnerStep, RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       RunHistoricalServer

THEOREM AsyncFaultStepLeavesDiscoveryClock ==
  AsyncFaultStep => asyncNow' = asyncNow
PROOF
  <1>1. ASSUME AsyncFaultStep
         PROVE asyncNow' = asyncNow
    <2>1. CASE \E packet \in asyncTransport: PreGstLosePacket(packet)
      BY <2>1, Isa DEF PreGstLosePacket
    <2>1a. CASE \E reservation \in asyncServeReservations:
                    PreGstServeReceiverCloseRollback(
                      reservation.node, reservation.identity)
      BY <2>1a, Isa
         DEF PreGstServeReceiverCloseRollback,
             PreGstPendingServeReceiverCloseRollback,
             PreGstMaterializedServeReceiverCloseRollback
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <2>2, Isa DEF PreGstCrash, AsyncSchedulerVars
    <2>3. CASE \E source \in AsyncIngressSources,
                  recipient \in ValidatorIds,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectByzantineNoise(source, recipient, nonce)
      BY <2>3, Isa DEF InjectByzantineNoise
    <2>3c. CASE \E kind \in IngressTransportCompletionKinds,
                   recipient \in ValidatorIds,
                   nonce \in 0..(AsyncIngressCapacity - 1):
                   InjectUntrustedTransportCompletion(
                     kind, recipient, nonce)
      BY <2>3c, Isa DEF InjectUntrustedTransportCompletion
    <2>4. CASE \E kind \in {"NormalJunk", "ProgressJunk"},
                  source \in ValidatorIds, recipient \in ValidatorIds,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectAuthenticatedJunk(
                    kind, source, recipient, nonce)
      BY <2>4, Isa DEF InjectAuthenticatedJunk
    <2>5. CASE \E source \in ValidatorIds,
                  recipient \in ValidatorIds, qc \in commitQCs,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectByzantineCertifiedRequest(
                    source, recipient, qc, nonce)
      BY <2>5, Isa DEF InjectByzantineCertifiedRequest
    <2>6. CASE \E signer \in ValidatorIds, roundView \in Views,
                  subject \in Subjects,
                  timeoutCertificate \in TimeoutCertificateOptionSet,
                  highestPrepare \in PrepareQcOptionSet:
                  AsyncByzantineProposal(
                    signer, roundView, subject,
                    timeoutCertificate, highestPrepare)
      BY <2>6, Isa DEF AsyncByzantineProposal
    <2>7. CASE \E signer \in ValidatorIds, roundView \in Views,
                  phase \in Phases, subject \in Subjects:
                  AsyncByzantineVote(
                    signer, roundView, phase, subject)
      BY <2>7, Isa DEF AsyncByzantineVote
    <2>8. CASE \E signer \in ValidatorIds, roundView \in Views,
                  highestPrepare \in PrepareQcOptionSet:
                  AsyncByzantineTimeout(
                    signer, roundView, highestPrepare)
      BY <2>8, Isa DEF AsyncByzantineTimeout
    <2> QED BY <1>1, <2>1, <2>1a, <2>2, <2>3, <2>3c, <2>4,
                <2>5, <2>6, <2>7, <2>8
         DEF AsyncFaultStep
  <1> QED BY <1>1

THEOREM AsyncNonRunnerStepPreservesDiscoveryClockThreshold ==
  /\ AsyncTypeInvariant
  /\ asyncNow >= AsyncRoundTimeout
  /\ AsyncNonRunnerStep
  => asyncNow' >= AsyncRoundTimeout
PROOF
  <1>1. ASSUME AsyncTypeInvariant,
              asyncNow >= AsyncRoundTimeout,
              AsyncNonRunnerStep
         PROVE asyncNow' >= AsyncRoundTimeout
    <2>1. /\ asyncNow \in Nat
           /\ AsyncRoundTimeout \in Nat
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
    <2>2. CASE AsyncSetGST
      BY <1>1, <2>2, Isa
         DEF AsyncSetGST, AsyncSchedulerVars
    <2>3. CASE AsyncTick
      BY <1>1, <2>1, <2>3, Isa DEF AsyncTick
    <2>4. CASE \E node \in ValidatorIds:
                  OpenHistoricalRecovery(node)
      BY <1>1, <2>4, Isa DEF OpenHistoricalRecovery
    <2>5. CASE \E node \in AsyncCurrentResponsiveVoters:
                  DirectCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>5, Isa
         DEF DirectCommitCertificateDiscoveryStep
    <2>6. CASE \E node \in asyncHistoricalRecoveryTargets:
                  DirectHistoricalCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>6, Isa
         DEF DirectHistoricalCommitCertificateDiscoveryStep,
             CommitCertificateDiscoveryStepWork
    <2>7. CASE \E node \in AsyncArchiveIoServiceNodes:
                  ServiceIoWorker(node)
      BY <1>1, <2>7, Isa DEF ServiceIoWorker, ServiceIoWorkerWork
    <2>8. CASE \E node \in asyncHistoricalRecoveryTargets:
                  ServiceHistoricalRecoveryIoWorker(node)
      BY <1>1, <2>8, Isa
         DEF ServiceHistoricalRecoveryIoWorker, ServiceIoWorkerWork
    <2>9. CASE \E node \in AsyncCurrentResponsiveVoters:
                  EnqueueIoLocalControl(node)
      BY <1>1, <2>9, Isa
         DEF EnqueueIoLocalControl, EnqueueIoLocalControlWork
    <2>10. CASE \E node \in asyncHistoricalRecoveryTargets:
                   EnqueueHistoricalRecoveryIoLocalControl(node)
      BY <1>1, <2>10, Isa
         DEF EnqueueHistoricalRecoveryIoLocalControl,
             EnqueueIoLocalControlWork
    <2>11. CASE AsyncNetworkStep
      BY <1>1, <2>11, Isa
         DEF AsyncNetworkStep, AdmitIngressPacket,
             AdmitHiddenPacket, CoalesceHiddenPacket
    <2>12. CASE AsyncFaultStep
      BY <1>1, <2>12, AsyncFaultStepLeavesDiscoveryClock
    <2> QED BY <1>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
                <2>8, <2>9, <2>10, <2>11, <2>12
         DEF AsyncNonRunnerStep
  <1> QED BY <1>1

THEOREM AsyncBracketNextPreservesDiscoveryClockThreshold ==
  /\ AsyncTypeInvariant
  /\ asyncNow >= AsyncRoundTimeout
  /\ [AsyncNext]_AsyncAllVars
  => asyncNow' >= AsyncRoundTimeout
PROOF
  <1>1. ASSUME AsyncTypeInvariant,
              asyncNow >= AsyncRoundTimeout,
              [AsyncNext]_AsyncAllVars
         PROVE asyncNow' >= AsyncRoundTimeout
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars
    <2>2. CASE AsyncNext
      <3>1. CASE AsyncNonCrashStep
        <4>1. CASE AsyncRunnerStep
          BY <1>1, <4>1, AsyncRunnerStepLeavesDiscoveryClock
        <4>2. CASE AsyncNonRunnerStep
          BY <1>1, <4>2,
             AsyncNonRunnerStepPreservesDiscoveryClockThreshold
        <4>3. CASE DriveResponsiveReplayHead \/ FinishResponsiveReplay
          BY <1>1, <4>3, Isa
             DEF DriveResponsiveReplayHead, FinishResponsiveReplay
        <4>4. CASE RearmResponsiveRecovery
          BY <1>1, <4>4, Isa DEF RearmResponsiveRecovery
        <4> QED BY <3>1, <4>1, <4>2, <4>3, <4>4
             DEF AsyncNonCrashStep
      <3>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
        BY <1>1, <3>2, Isa
           DEF PreGstCrash, AsyncSchedulerVars
      <3>3. CASE \E node \in ValidatorIds:
                    PreGstResponsiveCrash(node)
        BY <1>1, <3>3, Isa
           DEF PreGstResponsiveCrash, AsyncSchedulerVars
      <3>4. CASE PreGstResponsiveRestart
        BY <1>1, <3>4, Isa
           DEF PreGstResponsiveRestart, AsyncSchedulerVars
      <3>5. CASE PreGstResponsiveReplay
        BY <1>1, <3>5, Isa
           DEF PreGstResponsiveReplay, ResetNodeSchedulerForRestart
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4, <3>5 DEF AsyncNext
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM CommitCertificateDiscoveryPendingUnlessOutcome ==
  \A node:
    CommitCertificateDiscoveryPending(node)
      /\ [AsyncNext]_AsyncAllVars
    => CommitCertificateDiscoveryPending(node)'
         \/ CommitCertificateDiscoveryOutcome(node)'
PROOF
  <1>1. ASSUME NEW node,
                CommitCertificateDiscoveryPending(node),
                [AsyncNext]_AsyncAllVars
         PROVE CommitCertificateDiscoveryPending(node)'
                 \/ CommitCertificateDiscoveryOutcome(node)'
    <2>1. AsyncStrongTypeInvariant'
      BY <1>1, AsyncBracketNextPreservesStrongTypeInvariant
         DEF CommitCertificateDiscoveryPending
    <2>2. gst'
      BY <1>1, GstAsyncStepIsMonotone
         DEF CommitCertificateDiscoveryPending
    <2>3. asyncNow' >= AsyncRoundTimeout
      BY <1>1, AsyncStrongTypeProjectsAsyncType,
         AsyncBracketNextPreservesDiscoveryClockThreshold
         DEF CommitCertificateDiscoveryPending,
             CommitCertificateDiscoveryDue
    <2>4. /\ context' = context
           /\ CurrentVoters' = CurrentVoters
           /\ AsyncCurrentResponsiveVoters' =
                AsyncCurrentResponsiveVoters
      BY <1>1, Isa
         DEF AsyncNext, AsyncAllVars, vars,
             AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch
    <2>5. CommitCertificateRequestOutbox(node)' # {}
      <3>1. (CurrentVoters \ {node}) # {}
        BY <1>1, CommitCertificateRequestOutboxNonemptyIffRemoteVoter
           DEF CommitCertificateDiscoveryPending,
               CommitCertificateDiscoveryDue
      <3>2. (CurrentVoters' \ {node}) # {}
        BY <2>4, <3>1
      <3> QED BY <3>2,
           CommitCertificateRequestOutboxNonemptyIffRemoteVoter
    <2>6. CASE CommitCertificateDiscoveryOutcome(node)'
      BY <2>6
    <2>7. CASE ~CommitCertificateDiscoveryOutcome(node)'
      <3>1. /\ ~NodeHasDecision(node)'
             /\ ActiveCommitCertificateRequests(node)' = {}
        BY <2>7 DEF CommitCertificateDiscoveryOutcome
      <3>2. node \in AsyncCurrentResponsiveVoters'
        BY <1>1, <2>4
           DEF CommitCertificateDiscoveryPending,
               CommitCertificateDiscoveryDue
      <3>3. CommitCertificateDiscoveryDue(node)'
        BY <2>3, <2>5, <3>1, <3>2
           DEF CommitCertificateDiscoveryDue
      <3>4. CommitCertificateDiscoveryPending(node)'
        BY <2>1, <2>2, <3>3
           DEF CommitCertificateDiscoveryPending
      <3> QED BY <3>4
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM CommitCertificateDiscoveryPendingEnablesFairPrefix ==
  \A node:
    CommitCertificateDiscoveryPending(node)
      => ENABLED
           <<PostGstCommitCertificateDiscovery(node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW node,
                CommitCertificateDiscoveryPending(node)
         PROVE ENABLED
                 <<PostGstCommitCertificateDiscovery(node)>>_AsyncAllVars
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF CommitCertificateDiscoveryPending,
             CommitCertificateDiscoveryDue,
             AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety
    <2>2. ENABLED DirectCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>1, CommitCertificateDiscoveryPrefixIsEnabled
         DEF CommitCertificateDiscoveryPending
    <2>3. DirectCommitCertificateDiscoveryStep(node) \in BOOLEAN
      BY Isa DEF DirectCommitCertificateDiscoveryStep
    <2>4. <<PostGstCommitCertificateDiscovery(node)>>_AsyncAllVars
             \in BOOLEAN
      BY Isa DEF PostGstCommitCertificateDiscovery
    <2>5. DirectCommitCertificateDiscoveryStep(node)
             => <<PostGstCommitCertificateDiscovery(node)>>_AsyncAllVars
      BY <1>1, <2>1, DirectCommitCertificateDiscoveryPublishes, Isa
         DEF CommitCertificateDiscoveryPending,
             PostGstCommitCertificateDiscovery,
             ActiveCommitCertificateRequests, AsyncAllVars
    <2>6. ENABLED DirectCommitCertificateDiscoveryStep(node)
             => ENABLED
                  <<PostGstCommitCertificateDiscovery(node)>>_AsyncAllVars
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>6
  <1> QED BY <1>1

THEOREM FairCommitCertificateDiscoveryPublishesOrDecides ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => \A node \in AsyncVotersAt(initialContext):
           CommitCertificateDiscoveryPending(node)
             ~> CommitCertificateDiscoveryOutcome(node)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => \A node \in AsyncVotersAt(initialContext):
                      CommitCertificateDiscoveryPending(node)
                        ~> CommitCertificateDiscoveryOutcome(node)
    <2>1. ASSUME NEW node \in AsyncVotersAt(initialContext)
           PROVE AsyncSpecAt(initialContext)
                   => (CommitCertificateDiscoveryPending(node)
                         ~> CommitCertificateDiscoveryOutcome(node))
      <3>1. (CommitCertificateDiscoveryPending(node)
                /\ ~CommitCertificateDiscoveryOutcome(node))
               => ENABLED
                    <<PostGstCommitCertificateDiscovery(node)>>_AsyncAllVars
        BY CommitCertificateDiscoveryPendingEnablesFairPrefix
      <3>2. (CommitCertificateDiscoveryPending(node)
                /\ ~CommitCertificateDiscoveryOutcome(node)
                /\ <<PostGstCommitCertificateDiscovery(node)>>_AsyncAllVars)
               => CommitCertificateDiscoveryOutcome(node)'
        BY DirectCommitCertificateDiscoveryPublishes, Isa
           DEF CommitCertificateDiscoveryPending,
               CommitCertificateDiscoveryOutcome,
               PostGstCommitCertificateDiscovery,
               ActiveCommitCertificateRequests, AsyncAllVars
      <3>3. CommitCertificateDiscoveryPending(node)
                /\ [AsyncNext]_AsyncAllVars
               => CommitCertificateDiscoveryPending(node)'
                    \/ CommitCertificateDiscoveryOutcome(node)'
        BY CommitCertificateDiscoveryPendingUnlessOutcome
      <3>4. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(
                    PostGstCommitCertificateDiscovery(node))
        BY <2>1 DEF AsyncSpecAt, AsyncFairnessAt
      <3> QED BY <3>1, <3>2, <3>3, <3>4, PTL
           DEF AsyncSpecAt
    <2> QED BY <2>1
  <1> QED BY <1>1

(***************************************************************************
The Runtime leaf witnesses below construct the scheduler transition without
choosing its Serve-ingress gate.  The production relation applies that gate
after the leaf is known enabled: an empty ticket set selects the ordinary
serialized action, while a strictly older Runtime lifecycle selects the one
permitted predecessor interleave.  Keeping this wrapper proof-only prevents
either production arm from silently assuming the other's state predicate.
***************************************************************************)
UngatedSerializedRuntimeStep(node) ==
  /\ asyncRunnerPhase[node] = "Runtime"
  /\ UNCHANGED AsyncIoVars
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ RuntimeStep(node)
  /\ asyncRunnerPhase' =
       [asyncRunnerPhase EXCEPT ![node] = "Local"]
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = AsyncQueueCapacity]

IdleSerializedRuntimeStep(node) ==
  /\ IdleRuntimeStep(node)
  /\ UNCHANGED AsyncIoVars
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ asyncRunnerPhase' =
       [asyncRunnerPhase EXCEPT ![node] = "Local"]
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = AsyncQueueCapacity]

THEOREM IdleSerializedRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ ~DeferredWorkServiceable(node)
    /\ ~DeferredTagExecutable(node)
    /\ ~TimeoutDue(node)
    /\ ~RetransmitDue(node)
    /\ ~NodeQueueNonempty(node)
    => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ ~DeferredWorkServiceable(node)
                /\ ~DeferredTagExecutable(node)
                /\ ~TimeoutDue(node)
                /\ ~RetransmitDue(node)
                /\ ~NodeQueueNonempty(node)
         PROVE ENABLED UngatedSerializedRuntimeStep(node)
    <2>1. ENABLED IdleSerializedRuntimeStep(node)
      BY ExpandENABLED, Isa
         DEF IdleSerializedRuntimeStep, IdleRuntimeStep,
             LeaveCausalQueues, AsyncIoVars, AsyncDeferredVars, vars
    <2>2. IdleSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF IdleSerializedRuntimeStep, IdleRuntimeStep
    <2>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>4. IdleSerializedRuntimeStep(node)
             => UngatedSerializedRuntimeStep(node)
      BY <1>1, Isa
         DEF IdleSerializedRuntimeStep, UngatedSerializedRuntimeStep,
             RuntimeStep
    <2>5. ENABLED IdleSerializedRuntimeStep(node)
             => ENABLED UngatedSerializedRuntimeStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

DirectRetransmitSerializedStep(node) ==
  /\ DirectRetransmitStep(node)
  /\ UNCHANGED AsyncIoVars
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ asyncRunnerPhase' =
       [asyncRunnerPhase EXCEPT ![node] = "Local"]
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = AsyncQueueCapacity]

THEOREM DirectRetransmitRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ ~DeferredWorkServiceable(node)
    /\ ~DeferredTagExecutable(node)
    /\ ~TimeoutDue(node)
    /\ ~(NodeQueueNonempty(node) /\ asyncFifoOwed[node])
    /\ RetransmitDue(node)
    => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ ~DeferredWorkServiceable(node)
                /\ ~DeferredTagExecutable(node)
                /\ ~TimeoutDue(node)
                /\ ~(NodeQueueNonempty(node) /\ asyncFifoOwed[node])
                /\ RetransmitDue(node)
         PROVE ENABLED UngatedSerializedRuntimeStep(node)
    <2>1. ENABLED DirectRetransmitSerializedStep(node)
      BY <1>1, ExpandENABLED, Isa
         DEF DirectRetransmitSerializedStep, DirectRetransmitStep,
             SendNodeRetransmissions, NoSendItem, LeaveCausalQueues,
             AsyncIoVars, AsyncDeferredVars, vars
    <2>2. DirectRetransmitSerializedStep(node) \in BOOLEAN
      BY Isa DEF DirectRetransmitSerializedStep, DirectRetransmitStep
    <2>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>4. DirectRetransmitSerializedStep(node)
             => UngatedSerializedRuntimeStep(node)
      BY <1>1, Isa
         DEF DirectRetransmitSerializedStep,
             UngatedSerializedRuntimeStep, RuntimeStep
    <2>5. ENABLED DirectRetransmitSerializedStep(node)
             => ENABLED UngatedSerializedRuntimeStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

DeferredRetransmitSerializedStep(node) ==
  /\ DeferredRetransmitStep(node)
  /\ UNCHANGED AsyncIoVars
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ asyncRunnerPhase' =
       [asyncRunnerPhase EXCEPT ![node] = "Local"]
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = AsyncQueueCapacity]

THEOREM DeferredRetransmitRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ ~DeferredWorkServiceable(node)
    /\ DeferredTagExecutable(node)
    /\ ~DeferredTimeoutExecutable(node)
    => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ ~DeferredWorkServiceable(node)
                /\ DeferredTagExecutable(node)
                /\ ~DeferredTimeoutExecutable(node)
         PROVE ENABLED UngatedSerializedRuntimeStep(node)
    <2>1. ENABLED DeferredRetransmitSerializedStep(node)
      BY <1>1, ExpandENABLED, Isa
         DEF DeferredRetransmitSerializedStep,
             DeferredRetransmitStep, DeferredTagExecutable,
             SendNodeRetransmissions, NoSendItem, LeaveCausalQueues,
             AsyncIoVars, AsyncDeferredVars, vars
    <2>2. DeferredRetransmitSerializedStep(node) \in BOOLEAN
      BY Isa DEF DeferredRetransmitSerializedStep,
                 DeferredRetransmitStep
    <2>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>4. DeferredRetransmitSerializedStep(node)
             => UngatedSerializedRuntimeStep(node)
      BY <1>1, Isa
         DEF DeferredRetransmitSerializedStep,
             UngatedSerializedRuntimeStep, RuntimeStep,
             DeferredTagStep
    <2>5. ENABLED DeferredRetransmitSerializedStep(node)
             => ENABLED UngatedSerializedRuntimeStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

SerializedRunnerReset(node) ==
  /\ UNCHANGED AsyncIoVars
  /\ UNCHANGED AsyncLocalAdmissionVars
  /\ asyncRunnerPhase' =
       [asyncRunnerPhase EXCEPT ![node] = "Local"]
  /\ asyncRunnerBudget' =
       [asyncRunnerBudget EXCEPT ![node] = AsyncQueueCapacity]

DeferredBusyServiceFence(node) ==
  /\ asyncDeferredDrainOwed[node]
  /\ DeferredQueueNonempty(node)
  /\ ~NodeIdle(node)

THEOREM DeferredBusyServiceFenceClosesDeferredDrain ==
  \A node:
    DeferredBusyServiceFence(node) => ~DeferredWorkServiceable(node)
BY DEF DeferredBusyServiceFence, DeferredWorkServiceable

DeferredHandoffSkipSerializedStep(node) ==
  /\ LeaveCausalQueues
  /\ AdvanceNextDeferredClass(node)
  /\ UNCHANGED <<vars, asyncCommandQueues,
                  asyncNextCommandClass, asyncFifoOwed,
                  asyncTimeoutEmitted, asyncDeferredCompletionQueues,
                  asyncDeferredProgressQueues,
                  asyncDeferredNormalQueues, asyncOutstandingTags,
                  asyncNodeDeadlines, asyncRetransmitDeadlines,
                  asyncSentItems, asyncRetainedControl,
                  asyncActiveRequests, asyncTransport,
                  asyncIngressLanes, asyncIngressReady,
                  asyncHeldChunks, asyncHistoricalRecoveryTargets>>
  /\ asyncDeferredDrainOwed' =
       [asyncDeferredDrainOwed EXCEPT ![node] = FALSE]
  /\ RetainDeferredHandoffs
  /\ SerializedRunnerReset(node)

THEOREM DeferredHandoffSkipRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ asyncDeferredDrainOwed[node]
    /\ DeferredQueueNonempty(node)
    /\ DeferredHandoffActive(node)
    /\ ~DeferredHandoffMatches(node, NextDeferredCommand(node))
    /\ (~CommandDispatchable(NextDeferredCommand(node))
          \/ NextDeferredCommand(node).class # "Completion")
    => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ asyncDeferredDrainOwed[node]
                /\ DeferredQueueNonempty(node)
                /\ DeferredHandoffActive(node)
                /\ ~DeferredHandoffMatches(
                     node, NextDeferredCommand(node))
                /\ (~CommandDispatchable(NextDeferredCommand(node))
                      \/ NextDeferredCommand(node).class # "Completion")
         PROVE ENABLED UngatedSerializedRuntimeStep(node)
    <2>1. ENABLED DeferredHandoffSkipSerializedStep(node)
      BY ExpandENABLED, Isa
         DEF DeferredHandoffSkipSerializedStep, SerializedRunnerReset,
             LeaveCausalQueues, AsyncIoVars, vars
    <2>2. DeferredHandoffSkipSerializedStep(node) \in BOOLEAN
      BY Isa DEF DeferredHandoffSkipSerializedStep,
                 SerializedRunnerReset
    <2>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>4. DeferredHandoffSkipSerializedStep(node)
             => UngatedSerializedRuntimeStep(node)
      BY <1>1, Isa
         DEF DeferredHandoffSkipSerializedStep,
             SerializedRunnerReset, UngatedSerializedRuntimeStep,
             RuntimeStep, DeferredDrainStep,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution
    <2>5. ENABLED DeferredHandoffSkipSerializedStep(node)
             => ENABLED UngatedSerializedRuntimeStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

DeferredDrainDiscardSerializedStep(node) ==
  /\ IF DeferredHandoffMatches(node, NextDeferredCommand(node))
     THEN /\ RemoveNextDeferredCommand(node)
          /\ ClearDeferredHandoff(node)
     ELSE /\ RemoveNextDeferredCommand(node)
          /\ RetainDeferredHandoffs
  /\ DiscardCommand(NextDeferredCommand(node))
  /\ LeaveCausalQueues
  /\ asyncDeferredDrainOwed' = asyncDeferredDrainOwed
  /\ UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                  asyncFifoOwed, asyncTimeoutEmitted>>
  /\ SerializedRunnerReset(node)

THEOREM DeferredDrainDiscardRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ DeferredWorkServiceable(node)
    /\ ~CommandDispatchable(NextDeferredCommand(node))
    => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ DeferredWorkServiceable(node)
                /\ ~CommandDispatchable(NextDeferredCommand(node))
         PROVE ENABLED UngatedSerializedRuntimeStep(node)
    <2>1. ENABLED DeferredDrainDiscardSerializedStep(node)
      BY <1>1, ExpandENABLED, Isa
         DEF DeferredDrainDiscardSerializedStep,
             SerializedRunnerReset, RemoveNextDeferredCommand,
             DiscardCommand, LeaveCausalQueues,
             AsyncIoVars, AsyncDeferredVars, vars
    <2>2. DeferredDrainDiscardSerializedStep(node) \in BOOLEAN
      BY Isa DEF DeferredDrainDiscardSerializedStep,
                 SerializedRunnerReset,
                 RemoveNextDeferredCommand, DiscardCommand
    <2>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>4. DeferredDrainDiscardSerializedStep(node)
             => UngatedSerializedRuntimeStep(node)
      BY <1>1, Isa
         DEF DeferredDrainDiscardSerializedStep,
             SerializedRunnerReset, UngatedSerializedRuntimeStep,
             RuntimeStep, DeferredDrainStep,
             DeferredWorkServiceable
    <2>5. ENABLED DeferredDrainDiscardSerializedStep(node)
             => ENABLED UngatedSerializedRuntimeStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

FifoRuntimeSelected(node) ==
  /\ ~DeferredWorkServiceable(node)
  /\ ~DeferredTagExecutable(node)
  /\ ~TimeoutDue(node)
  /\ NodeQueueNonempty(node)
  /\ (asyncFifoOwed[node] \/ ~RetransmitDue(node))

FifoDeferSerializedStep(node) ==
  LET command == NextNodeCommand(node)
  IN /\ RemoveNextNodeCommand(node)
     /\ DeferCommand(command)
     /\ LeaveCausalQueues
     /\ asyncFifoOwed' =
          [asyncFifoOwed EXCEPT ![node] = FALSE]
     /\ asyncTimeoutEmitted' = asyncTimeoutEmitted
     /\ SerializedRunnerReset(node)

THEOREM FifoDeferRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ FifoRuntimeSelected(node)
    /\ ~CommandDispatchable(NextNodeCommand(node))
    /\ ~NodeIdle(node)
    => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ FifoRuntimeSelected(node)
                /\ ~CommandDispatchable(NextNodeCommand(node))
                /\ ~NodeIdle(node)
         PROVE ENABLED UngatedSerializedRuntimeStep(node)
    <2>1. ENABLED FifoDeferSerializedStep(node)
      BY ExpandENABLED, Isa
         DEF FifoDeferSerializedStep, SerializedRunnerReset,
             RemoveNextNodeCommand, DeferCommand, LeaveCausalQueues,
             AsyncIoVars, AsyncDeferredVars, vars
    <2>2. FifoDeferSerializedStep(node) \in BOOLEAN
      BY Isa DEF FifoDeferSerializedStep, SerializedRunnerReset,
                 RemoveNextNodeCommand, DeferCommand
    <2>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>4. FifoDeferSerializedStep(node)
             => UngatedSerializedRuntimeStep(node)
      BY <1>1, Isa
         DEF FifoDeferSerializedStep, SerializedRunnerReset,
             UngatedSerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
             FifoRuntimeSelected
    <2>5. ENABLED FifoDeferSerializedStep(node)
             => ENABLED UngatedSerializedRuntimeStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

FifoDiscardSerializedStep(node) ==
  LET command == NextNodeCommand(node)
  IN /\ RemoveNextNodeCommand(node)
     /\ DiscardCommand(command)
     /\ LeaveCausalQueues
     /\ UNCHANGED <<asyncDeferredCompletionQueues,
                     asyncDeferredProgressQueues,
                     asyncDeferredNormalQueues,
                     asyncDeferredHandoffs,
                     asyncNextDeferredClass>>
     /\ asyncDeferredDrainOwed' =
          [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
     /\ asyncFifoOwed' =
          [asyncFifoOwed EXCEPT ![node] = FALSE]
     /\ asyncTimeoutEmitted' = asyncTimeoutEmitted
     /\ SerializedRunnerReset(node)

THEOREM FifoDiscardRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ FifoRuntimeSelected(node)
    /\ ~CommandDispatchable(NextNodeCommand(node))
    /\ NodeIdle(node)
    => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ FifoRuntimeSelected(node)
                /\ ~CommandDispatchable(NextNodeCommand(node))
                /\ NodeIdle(node)
         PROVE ENABLED UngatedSerializedRuntimeStep(node)
    <2>1. ENABLED FifoDiscardSerializedStep(node)
      BY <1>1, ExpandENABLED, Isa
         DEF FifoDiscardSerializedStep, SerializedRunnerReset,
             RemoveNextNodeCommand, DiscardCommand, LeaveCausalQueues,
             AsyncIoVars, AsyncDeferredVars, vars
    <2>2. FifoDiscardSerializedStep(node) \in BOOLEAN
      BY Isa DEF FifoDiscardSerializedStep, SerializedRunnerReset,
                 RemoveNextNodeCommand, DiscardCommand
    <2>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>4. FifoDiscardSerializedStep(node)
             => UngatedSerializedRuntimeStep(node)
      BY <1>1, Isa
         DEF FifoDiscardSerializedStep, SerializedRunnerReset,
             UngatedSerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
             FifoRuntimeSelected
    <2>5. ENABLED FifoDiscardSerializedStep(node)
             => ENABLED UngatedSerializedRuntimeStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

DirectTimeoutDeferredSerializedStep(node) ==
  /\ asyncTimeoutEmitted' =
       [asyncTimeoutEmitted EXCEPT ![node] = TRUE]
  /\ asyncFifoOwed' =
       [asyncFifoOwed EXCEPT ![node] = NodeQueueNonempty(node)]
  /\ UNCHANGED vars
  /\ asyncOutstandingTags' =
       [asyncOutstandingTags EXCEPT
          ![node] = @ \cup {"TimeoutElapsed"}]
  /\ LeaveCausalQueues
  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                  asyncDeferredProgressQueues,
                  asyncDeferredNormalQueues,
                  asyncDeferredHandoffs,
                  asyncNextDeferredClass>>
  /\ asyncDeferredDrainOwed' = asyncDeferredDrainOwed
  /\ UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                  asyncNodeDeadlines, asyncRetransmitDeadlines,
                  asyncSentItems, asyncRetainedControl,
                  asyncActiveRequests, asyncCertifiedResponseClaim,
                  asyncTransport,
                  asyncIngressLanes, asyncIngressReady,
                  asyncHeldChunks, asyncHistoricalRecoveryTargets>>
  /\ SerializedRunnerReset(node)

THEOREM DirectTimeoutDeferredRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ ~DeferredWorkServiceable(node)
    /\ ~DeferredTagExecutable(node)
    /\ TimeoutDue(node)
    /\ ~BeginTimeoutEnabled(node)
    => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ ~DeferredWorkServiceable(node)
                /\ ~DeferredTagExecutable(node)
                /\ TimeoutDue(node)
                /\ ~BeginTimeoutEnabled(node)
         PROVE ENABLED UngatedSerializedRuntimeStep(node)
    <2>1. ENABLED DirectTimeoutDeferredSerializedStep(node)
      BY ExpandENABLED, Isa
         DEF DirectTimeoutDeferredSerializedStep,
             SerializedRunnerReset, LeaveCausalQueues,
             AsyncIoVars, vars
    <2>2. DirectTimeoutDeferredSerializedStep(node) \in BOOLEAN
      BY Isa DEF DirectTimeoutDeferredSerializedStep,
                 SerializedRunnerReset
    <2>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>4. DirectTimeoutDeferredSerializedStep(node)
             => UngatedSerializedRuntimeStep(node)
      BY <1>1, Isa
         DEF DirectTimeoutDeferredSerializedStep,
             SerializedRunnerReset, UngatedSerializedRuntimeStep,
             RuntimeStep, DirectTimeoutStep
    <2>5. ENABLED DirectTimeoutDeferredSerializedStep(node)
             => ENABLED UngatedSerializedRuntimeStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

DeferredTimeoutNoBeginSerializedStep(node) ==
  /\ UNCHANGED vars
  /\ LeaveCausalQueues
  /\ asyncOutstandingTags' =
       [asyncOutstandingTags EXCEPT
          ![node] = @ \ {"TimeoutElapsed"}]
  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                  asyncDeferredProgressQueues,
                  asyncDeferredNormalQueues,
                  asyncDeferredHandoffs,
                  asyncNextDeferredClass>>
  /\ asyncDeferredDrainOwed' =
       [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
  /\ UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                  asyncFifoOwed, asyncTimeoutEmitted,
                  asyncNodeDeadlines, asyncRetransmitDeadlines,
                  asyncSentItems, asyncRetainedControl,
                  asyncActiveRequests, asyncCertifiedResponseClaim,
                  asyncTransport,
                  asyncIngressLanes, asyncIngressReady,
                  asyncHeldChunks, asyncHistoricalRecoveryTargets>>
  /\ SerializedRunnerReset(node)

THEOREM DeferredTimeoutNoBeginRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ ~DeferredWorkServiceable(node)
    /\ DeferredTimeoutExecutable(node)
    /\ ~BeginTimeoutEnabled(node)
    => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ ~DeferredWorkServiceable(node)
                /\ DeferredTimeoutExecutable(node)
                /\ ~BeginTimeoutEnabled(node)
         PROVE ENABLED UngatedSerializedRuntimeStep(node)
    <2>1. ENABLED DeferredTimeoutNoBeginSerializedStep(node)
      BY ExpandENABLED, Isa
         DEF DeferredTimeoutNoBeginSerializedStep,
             SerializedRunnerReset, LeaveCausalQueues,
             AsyncIoVars, vars
    <2>2. DeferredTimeoutNoBeginSerializedStep(node) \in BOOLEAN
      BY Isa DEF DeferredTimeoutNoBeginSerializedStep,
                 SerializedRunnerReset
    <2>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>4. DeferredTimeoutNoBeginSerializedStep(node)
             => UngatedSerializedRuntimeStep(node)
      BY <1>1, Isa
         DEF DeferredTimeoutNoBeginSerializedStep,
             SerializedRunnerReset, UngatedSerializedRuntimeStep,
             RuntimeStep, DeferredTagExecutable, DeferredTagStep,
             DeferredTimeoutStep
    <2>5. ENABLED DeferredTimeoutNoBeginSerializedStep(node)
             => ENABLED UngatedSerializedRuntimeStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

DirectTimeoutBeginSerializedStep(node) ==
  /\ asyncTimeoutEmitted' =
       [asyncTimeoutEmitted EXCEPT ![node] = TRUE]
  /\ asyncFifoOwed' =
       [asyncFifoOwed EXCEPT ![node] = NodeQueueNonempty(node)]
  /\ BeginTimeout(node)
  /\ UNCHANGED asyncOutstandingTags
  /\ AppendCausalSuccessors(TimeoutCausalCommand(node))
  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                  asyncDeferredProgressQueues,
                  asyncDeferredNormalQueues,
                  asyncDeferredHandoffs,
                  asyncNextDeferredClass>>
  /\ asyncDeferredDrainOwed' =
       [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
  /\ UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                  asyncNodeDeadlines, asyncRetransmitDeadlines,
                  asyncSentItems, asyncRetainedControl,
                  asyncActiveRequests, asyncCertifiedResponseClaim,
                  asyncTransport,
                  asyncIngressLanes, asyncIngressReady,
                  asyncHeldChunks, asyncHistoricalRecoveryTargets>>
  /\ SerializedRunnerReset(node)

THEOREM DirectTimeoutBeginRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ ~DeferredWorkServiceable(node)
    /\ ~DeferredTagExecutable(node)
    /\ TimeoutDue(node)
    /\ BeginTimeoutEnabled(node)
    => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ ~DeferredWorkServiceable(node)
                /\ ~DeferredTagExecutable(node)
                /\ TimeoutDue(node)
                /\ BeginTimeoutEnabled(node)
         PROVE ENABLED UngatedSerializedRuntimeStep(node)
    <2>1. ENABLED DirectTimeoutBeginSerializedStep(node)
      BY <1>1, ExpandENABLED, Isa
         DEF BeginTimeoutEnabled, DirectTimeoutBeginSerializedStep,
             SerializedRunnerReset, BeginTimeout,
             AppendCausalSuccessors, AsyncIoVars, vars
    <2>2. DirectTimeoutBeginSerializedStep(node) \in BOOLEAN
      BY Isa DEF DirectTimeoutBeginSerializedStep,
                 SerializedRunnerReset, BeginTimeout
    <2>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>4. DirectTimeoutBeginSerializedStep(node)
             => UngatedSerializedRuntimeStep(node)
      BY <1>1, Isa
         DEF DirectTimeoutBeginSerializedStep,
             SerializedRunnerReset, UngatedSerializedRuntimeStep,
             RuntimeStep, DirectTimeoutStep
    <2>5. ENABLED DirectTimeoutBeginSerializedStep(node)
             => ENABLED UngatedSerializedRuntimeStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

DeferredTimeoutBeginSerializedStep(node) ==
  /\ BeginTimeout(node)
  /\ AppendCausalSuccessors(TimeoutCausalCommand(node))
  /\ asyncOutstandingTags' =
       [asyncOutstandingTags EXCEPT
          ![node] = @ \ {"TimeoutElapsed"}]
  /\ UNCHANGED <<asyncDeferredCompletionQueues,
                  asyncDeferredProgressQueues,
                  asyncDeferredNormalQueues,
                  asyncDeferredHandoffs,
                  asyncNextDeferredClass>>
  /\ asyncDeferredDrainOwed' =
       [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
  /\ UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                  asyncFifoOwed, asyncTimeoutEmitted,
                  asyncNodeDeadlines, asyncRetransmitDeadlines,
                  asyncSentItems, asyncRetainedControl,
                  asyncActiveRequests, asyncCertifiedResponseClaim,
                  asyncTransport,
                  asyncIngressLanes, asyncIngressReady,
                  asyncHeldChunks, asyncHistoricalRecoveryTargets>>
  /\ SerializedRunnerReset(node)

THEOREM DeferredTimeoutBeginRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ ~DeferredWorkServiceable(node)
    /\ DeferredTimeoutExecutable(node)
    /\ BeginTimeoutEnabled(node)
    => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ ~DeferredWorkServiceable(node)
                /\ DeferredTimeoutExecutable(node)
                /\ BeginTimeoutEnabled(node)
         PROVE ENABLED UngatedSerializedRuntimeStep(node)
    <2>1. ENABLED DeferredTimeoutBeginSerializedStep(node)
      BY <1>1, ExpandENABLED, Isa
         DEF BeginTimeoutEnabled, DeferredTimeoutBeginSerializedStep,
             SerializedRunnerReset, BeginTimeout,
             AppendCausalSuccessors, AsyncIoVars, vars
    <2>2. DeferredTimeoutBeginSerializedStep(node) \in BOOLEAN
      BY Isa DEF DeferredTimeoutBeginSerializedStep,
                 SerializedRunnerReset, BeginTimeout
    <2>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>4. DeferredTimeoutBeginSerializedStep(node)
             => UngatedSerializedRuntimeStep(node)
      BY <1>1, Isa
         DEF DeferredTimeoutBeginSerializedStep,
             SerializedRunnerReset, UngatedSerializedRuntimeStep,
             RuntimeStep, DeferredTagExecutable, DeferredTagStep,
             DeferredTimeoutStep
    <2>5. ENABLED DeferredTimeoutBeginSerializedStep(node)
             => ENABLED UngatedSerializedRuntimeStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

ExecutionRuntimeSelected(node) ==
  \/ DeferredWorkServiceable(node)
  \/ FifoRuntimeSelected(node)

SelectedRuntimeCommand(node) ==
  IF DeferredWorkServiceable(node)
  THEN NextDeferredCommand(node)
  ELSE NextNodeCommand(node)

SelectedExecutionSchedulerFrame(node, command) ==
  /\ IF DeferredWorkServiceable(node)
     THEN /\ RemoveNextDeferredCommand(node)
          /\ asyncDeferredDrainOwed' = asyncDeferredDrainOwed
          /\ UNCHANGED <<asyncCommandQueues,
                          asyncNextCommandClass, asyncFifoOwed>>
     ELSE /\ RemoveNextNodeCommand(node)
          /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues,
                          asyncDeferredHandoffs,
                          asyncNextDeferredClass>>
          /\ asyncDeferredDrainOwed' =
               [asyncDeferredDrainOwed EXCEPT ![node] = TRUE]
          /\ asyncFifoOwed' =
               [asyncFifoOwed EXCEPT ![node] = FALSE]
  /\ AppendCausalSuccessors(command)
  /\ asyncTimeoutEmitted' =
       IF command.kind = "PersistInstallTC"
       THEN [asyncTimeoutEmitted EXCEPT ![node] = FALSE]
       ELSE asyncTimeoutEmitted
  /\ SerializedRunnerReset(node)

RejectSelectedExecutionSerializedStep(node) ==
  LET command == SelectedRuntimeCommand(node)
  IN /\ ExecuteRejectAuthenticatedJunk(command)
     /\ SelectedExecutionSchedulerFrame(node, command)

THEOREM RejectSelectedExecutionRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    LET command == SelectedRuntimeCommand(node)
    IN /\ asyncRunnerPhase[node] = "Runtime"
       /\ ExecutionRuntimeSelected(node)
       /\ CommandDispatchable(command)
       /\ ENABLED ExecuteRejectAuthenticatedJunk(command)
       => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds
         PROVE LET command == SelectedRuntimeCommand(node)
               IN /\ asyncRunnerPhase[node] = "Runtime"
                  /\ ExecutionRuntimeSelected(node)
                  /\ CommandDispatchable(command)
                  /\ ENABLED ExecuteRejectAuthenticatedJunk(command)
                  => ENABLED UngatedSerializedRuntimeStep(node)
    <2> DEFINE Command == SelectedRuntimeCommand(node)
    <2>1. ASSUME /\ asyncRunnerPhase[node] = "Runtime"
                  /\ ExecutionRuntimeSelected(node)
                  /\ CommandDispatchable(Command)
                  /\ ENABLED ExecuteRejectAuthenticatedJunk(Command)
           PROVE ENABLED UngatedSerializedRuntimeStep(node)
      <3>1. ENABLED RejectSelectedExecutionSerializedStep(node)
        BY <2>1, ExpandENABLED, Isa
           DEF RejectSelectedExecutionSerializedStep,
               SelectedExecutionSchedulerFrame,
               SelectedRuntimeCommand, SerializedRunnerReset,
               ExecuteRejectAuthenticatedJunk,
               RemoveNextDeferredCommand,
               ClearDeferredHandoff, RetainDeferredHandoffs,
               RemoveNextNodeCommand,
               AppendCausalSuccessors, AsyncIoVars, vars, Command
      <3>2. RejectSelectedExecutionSerializedStep(node) \in BOOLEAN
        BY Isa DEF RejectSelectedExecutionSerializedStep,
                   SelectedExecutionSchedulerFrame,
                   SerializedRunnerReset,
                   ExecuteRejectAuthenticatedJunk
      <3>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
        BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
      <3>4. RejectSelectedExecutionSerializedStep(node)
               => UngatedSerializedRuntimeStep(node)
        BY <2>1, Isa
           DEF RejectSelectedExecutionSerializedStep,
               SelectedExecutionSchedulerFrame,
               SelectedRuntimeCommand, SerializedRunnerReset,
               UngatedSerializedRuntimeStep, RuntimeStep,
               DeferredDrainStep, FifoRuntimeStep,
               FifoRuntimeSelected, ExecutionRuntimeSelected,
               DeferredHandoffAllowsExecution,
               DeferredHandoffBlocksExecution,
               ExecuteCommand, Command
      <3>5. ENABLED RejectSelectedExecutionSerializedStep(node)
               => ENABLED UngatedSerializedRuntimeStep(node)
        BY <3>2, <3>3, <3>4, ENABLEDaxioms
      <3> QED BY <3>1, <3>5
    <2> QED BY <2>1
  <1> QED BY <1>1

SelectedExecutionSerializedStep(node) ==
  LET command == SelectedRuntimeCommand(node)
  IN /\ ExecuteCommand(command)
     /\ SelectedExecutionSchedulerFrame(node, command)

THEOREM SelectedExecutionRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    LET command == SelectedRuntimeCommand(node)
    IN /\ asyncRunnerPhase[node] = "Runtime"
       /\ ExecutionRuntimeSelected(node)
       /\ CommandDispatchable(command)
       => ENABLED UngatedSerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds
         PROVE LET command == SelectedRuntimeCommand(node)
               IN /\ asyncRunnerPhase[node] = "Runtime"
                  /\ ExecutionRuntimeSelected(node)
                  /\ CommandDispatchable(command)
                  => ENABLED UngatedSerializedRuntimeStep(node)
    <2> DEFINE Command == SelectedRuntimeCommand(node)
    <2>1. ASSUME /\ asyncRunnerPhase[node] = "Runtime"
                  /\ ExecutionRuntimeSelected(node)
                  /\ CommandDispatchable(Command)
           PROVE ENABLED UngatedSerializedRuntimeStep(node)
      <3>1. ENABLED SelectedExecutionSerializedStep(node)
        BY <2>1, ExpandENABLED, Isa
           DEF SelectedExecutionSerializedStep,
               SelectedExecutionSchedulerFrame,
               SelectedRuntimeCommand, SerializedRunnerReset,
               CommandDispatchable, CommandExecutionReady,
               ExecuteCommand, ExecuteRegularCommand,
               ExecuteSignProposal, ExecuteSignVote,
               ExecuteFormPrepareQC, ExecuteSignTimeout,
               ExecutePersistInstall, ExecutePersistDecision,
               ExecuteRequestCertifiedBody, ExecuteApply,
               ExecuteCoreDelivery, ExecuteChunkDelivery,
               ExecuteRejectAuthenticatedJunk, RegularCoreCommand,
               AssembleLocalBody, BeginLocalProposal, PersistProposal,
               FetchBody, RebindRetainedBody, StoreBody, ValidateBody,
               RejectBody,
               ValidateDecidedBody, ValidateLockedBody,
               BeginPrepare, PersistPrepare,
               BeginObservePrepare, PersistObservePrepare,
               BeginLockCommit, PersistLockCommit, FormCommitQC,
               BeginDecision, PersistTimeout, FormTC, BeginInstallTC,
               FetchCertifiedBody, CompleteProposalSignature,
               CompleteVoteSignature, FormPrepareQC,
               CompleteTimeoutSignature, PersistInstallTC,
               PersistDecision, ApplyDecision, DeliverProposal,
               DeliverVote, DeliverQC, DeliverTimeout, DeliverTC,
               PublishControlAndEphemeralItems, PublishControlItems,
               PersistInstalledControl, PersistInstalledControlAfterInstall,
               PersistDecisionControl,
               PublishCertifiedRequests,
               RemoveNextDeferredCommand,
               ClearDeferredHandoff, RetainDeferredHandoffs,
               RemoveNextNodeCommand,
               AppendCausalSuccessors, AsyncAuxVars,
               AsyncIoVars, vars, Command
      <3>2. SelectedExecutionSerializedStep(node) \in BOOLEAN
        BY Isa DEF SelectedExecutionSerializedStep,
                   SelectedExecutionSchedulerFrame,
                   SerializedRunnerReset, ExecuteCommand
      <3>3. UngatedSerializedRuntimeStep(node) \in BOOLEAN
        BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
      <3>4. SelectedExecutionSerializedStep(node)
               => UngatedSerializedRuntimeStep(node)
        BY <2>1, Isa
           DEF SelectedExecutionSerializedStep,
               SelectedExecutionSchedulerFrame,
               SelectedRuntimeCommand, SerializedRunnerReset,
               UngatedSerializedRuntimeStep, RuntimeStep,
               DeferredDrainStep, FifoRuntimeStep,
               FifoRuntimeSelected, ExecutionRuntimeSelected,
               DeferredHandoffAllowsExecution,
               DeferredHandoffBlocksExecution,
               CommandDispatchable, Command
      <3>5. ENABLED SelectedExecutionSerializedStep(node)
               => ENABLED UngatedSerializedRuntimeStep(node)
        BY <3>2, <3>3, <3>4, ENABLEDaxioms
      <3> QED BY <3>1, <3>5
    <2> QED BY <2>1
  <1> QED BY <1>1

DeferredBusyCompletionSelected(node) ==
  /\ DeferredBusyServiceFence(node)
  /\ FifoRuntimeSelected(node)
  /\ NextNodeCommand(node).class = "Completion"
  /\ CommandDispatchable(NextNodeCommand(node))

THEOREM DeferredBusyCompletionRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ DeferredBusyCompletionSelected(node)
    => ENABLED UngatedSerializedRuntimeStep(node)
BY SelectedExecutionRuntimeIsEnabled, Isa
   DEF DeferredBusyCompletionSelected, DeferredBusyServiceFence,
       DeferredWorkServiceable, ExecutionRuntimeSelected,
       SelectedRuntimeCommand, FifoRuntimeSelected

THEOREM UngatedSerializedRuntimeStepIsEnabled ==
  \A node \in ValidatorIds:
    asyncRunnerPhase[node] = "Runtime"
      => ENABLED UngatedSerializedRuntimeStep(node)
BY DeferredDrainDiscardRuntimeIsEnabled,
   DeferredRetransmitRuntimeIsEnabled,
   DirectTimeoutDeferredRuntimeIsEnabled,
   DeferredTimeoutNoBeginRuntimeIsEnabled,
   DirectTimeoutBeginRuntimeIsEnabled,
   DeferredTimeoutBeginRuntimeIsEnabled,
   SelectedExecutionRuntimeIsEnabled,
   DeferredBusyCompletionRuntimeIsEnabled,
   FifoDeferRuntimeIsEnabled,
   FifoDiscardRuntimeIsEnabled,
   DirectRetransmitRuntimeIsEnabled,
   IdleSerializedRuntimeIsEnabled, Isa
   DEF ExecutionRuntimeSelected, FifoRuntimeSelected,
       DeferredTagExecutable, DeferredWorkServiceable

(***************************************************************************
Apply the production Serve-ingress gate only after the selected Runtime leaf
has supplied a successor.  These are the three exact Runtime scheduler arms;
none of them treats ticket replenishment as progress or adds fairness.
***************************************************************************)
THEOREM NoServeIngressTicketSerializedRuntimeIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ ~AsyncIngressSchedulerBarrierActive(node)
    => ENABLED SerializedRuntimeStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ ~AsyncIngressSchedulerBarrierActive(node)
         PROVE ENABLED SerializedRuntimeStep(node)
    <2>1. ENABLED UngatedSerializedRuntimeStep(node)
      BY <1>1, UngatedSerializedRuntimeStepIsEnabled
    <2>2. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>3. SerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF SerializedRuntimeStep, RuntimeStep
    <2>4. UngatedSerializedRuntimeStep(node)
             => SerializedRuntimeStep(node)
      BY <1>1, Isa
         DEF UngatedSerializedRuntimeStep, SerializedRuntimeStep
    <2>5. ENABLED UngatedSerializedRuntimeStep(node)
             => ENABLED SerializedRuntimeStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

THEOREM OlderRuntimePrecedesServeIngressStepIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ AsyncIngressSchedulerBarrierActive(node)
    /\ AsyncOlderRuntimeLifecyclePrecedesServeIngress(node)
    => ENABLED SerializedRuntimePrecedesServeIngressStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ asyncRunnerPhase[node] = "Runtime"
                /\ AsyncIngressSchedulerBarrierActive(node)
                /\ AsyncOlderRuntimeLifecyclePrecedesServeIngress(node)
         PROVE ENABLED SerializedRuntimePrecedesServeIngressStep(node)
    <2>1. ENABLED UngatedSerializedRuntimeStep(node)
      BY <1>1, UngatedSerializedRuntimeStepIsEnabled
    <2>2. UngatedSerializedRuntimeStep(node) \in BOOLEAN
      BY Isa DEF UngatedSerializedRuntimeStep, RuntimeStep
    <2>3. SerializedRuntimePrecedesServeIngressStep(node) \in BOOLEAN
      BY Isa DEF SerializedRuntimePrecedesServeIngressStep, RuntimeStep
    <2>4. UngatedSerializedRuntimeStep(node)
             => SerializedRuntimePrecedesServeIngressStep(node)
      BY <1>1, Isa
         DEF UngatedSerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep
    <2>5. ENABLED UngatedSerializedRuntimeStep(node)
             => ENABLED SerializedRuntimePrecedesServeIngressStep(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

THEOREM ServeIngressTargetOnlyTurnIsEnabled ==
  \A node \in ValidatorIds:
    /\ AsyncIngressSchedulerBarrierActive(node)
    /\ asyncRunnerPhase[node] \in {"Runtime", "Local"}
    /\ ~( /\ asyncRunnerPhase[node] = "Runtime"
           /\ AsyncOlderRuntimeLifecyclePrecedesServeIngress(node))
    /\ ~( /\ asyncRunnerPhase[node] = "Local"
           /\ AsyncOlderLocalLifecyclePrecedesServeIngress(node))
    => ENABLED AsyncServeIngressTargetOnlyTurn(node)
BY ExpandENABLED, Isa
   DEF AsyncServeIngressTargetOnlyTurn,
       AsyncLocalAdmissionVars, AsyncIoVars, AsyncDeferredVars, vars

THEOREM OlderLocalPrecedesServeIngressStepIsEnabled ==
  \A node \in ValidatorIds:
    /\ asyncRunnerPhase[node] = "Local"
    /\ AsyncOlderLocalLifecyclePrecedesServeIngress(node)
    => ENABLED SerializedLocalPrecedesServeIngressStep(node)
BY SelectedProducerCanAdvance, SelectedCausalCanAdvance,
   HeadTailProperties, ExpandENABLED, IsaT(300)
   DEF SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncOlderLocalLifecyclePrecedesServeIngress,
       AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, OtherLocalSource,
       EnqueueCandidate, LeaveCausalQueues,
       AsyncIoVars, AsyncDeferredVars, vars

LocalServeSchedulerStep(node) ==
  \/ LocalAdmissionStep(node)
  \/ SerializedLocalPrecedesServeIngressStep(node)
  \/ AsyncServeIngressTargetOnlyTurn(node)

RuntimeServeSchedulerStep(node) ==
  \/ SerializedRuntimeStep(node)
  \/ SerializedRuntimePrecedesServeIngressStep(node)
  \/ AsyncServeIngressTargetOnlyTurn(node)

THEOREM LocalServeSchedulerStepIsEnabled ==
  \A node \in ValidatorIds:
    asyncRunnerPhase[node] = "Local"
      => ENABLED LocalServeSchedulerStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                asyncRunnerPhase[node] = "Local"
         PROVE ENABLED LocalServeSchedulerStep(node)
    <2>1. LocalServeSchedulerStep(node) \in BOOLEAN
      BY Isa DEF LocalServeSchedulerStep, LocalAdmissionStep,
                 SerializedLocalPrecedesServeIngressStep,
                 AsyncServeIngressTargetOnlyTurn
    <2>2. CASE ~AsyncIngressSchedulerBarrierActive(node)
      <3>1. ENABLED LocalAdmissionStep(node)
        BY <1>1, <2>2, LocalAdmissionStepIsEnabled
      <3>2. LocalAdmissionStep(node) \in BOOLEAN
        BY Isa DEF LocalAdmissionStep
      <3>3. LocalAdmissionStep(node) => LocalServeSchedulerStep(node)
        BY DEF LocalServeSchedulerStep
      <3> QED BY <2>1, <3>1, <3>2, <3>3, ENABLEDaxioms
    <2>3. CASE AsyncIngressSchedulerBarrierActive(node)
      <3>1. CASE AsyncOlderLocalLifecyclePrecedesServeIngress(node)
        <4>1. ENABLED SerializedLocalPrecedesServeIngressStep(node)
          BY <1>1, <3>1,
             OlderLocalPrecedesServeIngressStepIsEnabled
        <4>2. SerializedLocalPrecedesServeIngressStep(node) \in BOOLEAN
          BY Isa DEF SerializedLocalPrecedesServeIngressStep
        <4>3. SerializedLocalPrecedesServeIngressStep(node)
                 => LocalServeSchedulerStep(node)
          BY DEF LocalServeSchedulerStep
        <4> QED BY <2>1, <4>1, <4>2, <4>3, ENABLEDaxioms
      <3>2. CASE ~AsyncOlderLocalLifecyclePrecedesServeIngress(node)
        <4>1. ENABLED AsyncServeIngressTargetOnlyTurn(node)
          BY <1>1, <2>3, <3>2,
             ServeIngressTargetOnlyTurnIsEnabled
        <4>2. AsyncServeIngressTargetOnlyTurn(node) \in BOOLEAN
          BY Isa DEF AsyncServeIngressTargetOnlyTurn
        <4>3. AsyncServeIngressTargetOnlyTurn(node)
                 => LocalServeSchedulerStep(node)
          BY DEF LocalServeSchedulerStep
        <4> QED BY <2>1, <4>1, <4>2, <4>3, ENABLEDaxioms
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM RuntimeServeSchedulerStepIsEnabled ==
  \A node \in ValidatorIds:
    asyncRunnerPhase[node] = "Runtime"
      => ENABLED RuntimeServeSchedulerStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                asyncRunnerPhase[node] = "Runtime"
         PROVE ENABLED RuntimeServeSchedulerStep(node)
    <2>1. RuntimeServeSchedulerStep(node) \in BOOLEAN
      BY Isa
         DEF RuntimeServeSchedulerStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             AsyncServeIngressTargetOnlyTurn, RuntimeStep
    <2>2. CASE ~AsyncIngressSchedulerBarrierActive(node)
      <3>1. ENABLED SerializedRuntimeStep(node)
        BY <1>1, <2>2,
           NoServeIngressTicketSerializedRuntimeIsEnabled
      <3>2. SerializedRuntimeStep(node) \in BOOLEAN
        BY Isa DEF SerializedRuntimeStep, RuntimeStep
      <3>3. SerializedRuntimeStep(node)
               => RuntimeServeSchedulerStep(node)
        BY DEF RuntimeServeSchedulerStep
      <3> QED BY <2>1, <3>1, <3>2, <3>3, ENABLEDaxioms
    <2>3. CASE /\ AsyncIngressSchedulerBarrierActive(node)
                  /\ AsyncOlderRuntimeLifecyclePrecedesServeIngress(node)
      <3>1. ENABLED SerializedRuntimePrecedesServeIngressStep(node)
        BY <1>1, <2>3,
           OlderRuntimePrecedesServeIngressStepIsEnabled
      <3>2. SerializedRuntimePrecedesServeIngressStep(node) \in BOOLEAN
        BY Isa
           DEF SerializedRuntimePrecedesServeIngressStep, RuntimeStep
      <3>3. SerializedRuntimePrecedesServeIngressStep(node)
               => RuntimeServeSchedulerStep(node)
        BY DEF RuntimeServeSchedulerStep
      <3> QED BY <2>1, <3>1, <3>2, <3>3, ENABLEDaxioms
    <2>4. CASE /\ AsyncIngressSchedulerBarrierActive(node)
                  /\ ~AsyncOlderRuntimeLifecyclePrecedesServeIngress(node)
      <3>1. ENABLED AsyncServeIngressTargetOnlyTurn(node)
        BY <1>1, <2>4, ServeIngressTargetOnlyTurnIsEnabled
      <3>2. AsyncServeIngressTargetOnlyTurn(node) \in BOOLEAN
        BY Isa DEF AsyncServeIngressTargetOnlyTurn
      <3>3. AsyncServeIngressTargetOnlyTurn(node)
               => RuntimeServeSchedulerStep(node)
        BY DEF RuntimeServeSchedulerStep
      <3> QED BY <2>1, <3>1, <3>2, <3>3, ENABLEDaxioms
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

NodeServiceFrame(node) ==
  /\ UNCHANGED asyncNow
  /\ asyncNodeServiceDeadlines' =
       [asyncNodeServiceDeadlines EXCEPT
          ![node] = asyncNow + AsyncDeliveryBound]
  /\ UNCHANGED asyncIoServiceDeadlines

RecoveryRunNodeGuard(node) ==
  \/ ~ResponsiveReplayQuarantined(node)
  \/ /\ ResponsiveReplayDraining(node)
     /\ ~NodeIdle(node)
     /\ asyncIngressReady[node] = <<>>
     /\ \/ ~AsyncIngressSchedulerBarrierActive(node)
        \/ asyncRunnerPhase[node] = "Ingress"

LocalRunNodeStep(node) ==
  LocalServeSchedulerStep(node) /\ NodeServiceFrame(node)

THEOREM EnabledLocalServeSchedulerLiftsToRunNode ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ node \in up
    /\ ~NodeHasApplication(node)
    /\ RecoveryRunNodeGuard(node)
    /\ ~AsyncCandidateProducerContinuationResolutionRequired(node)
    /\ ENABLED LocalServeSchedulerStep(node)
    => ENABLED RunNode(node)
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                /\ node \in up
                /\ ~NodeHasApplication(node)
                /\ RecoveryRunNodeGuard(node)
                /\ ~AsyncCandidateProducerContinuationResolutionRequired(node)
                /\ ENABLED LocalServeSchedulerStep(node)
         PROVE ENABLED RunNode(node)
    <2>1. ENABLED LocalRunNodeStep(node)
      BY <1>1, ExpandENABLED, Isa
         DEF LocalRunNodeStep, NodeServiceFrame,
             LocalServeSchedulerStep,
             LocalAdmissionStep, AdmitProducerCompletion,
             AdmitCausalHead, UpdateLocalAdmissionMetadata,
             SelectedLocalSource, PreferredLocalSource,
             LocalSourceCanAdmit, OtherLocalSource,
             EnqueueCandidate, LeaveCausalQueues,
             SerializedLocalPrecedesServeIngressStep,
             SelectedLocalAdmissionAdvance,
             AsyncServeIngressTargetOnlyTurn,
             AsyncIoVars, AsyncDeferredVars,
             AsyncLocalAdmissionVars, vars
    <2>2. LocalRunNodeStep(node) \in BOOLEAN
      BY Isa DEF LocalRunNodeStep, NodeServiceFrame,
                 LocalServeSchedulerStep,
                 LocalAdmissionStep,
                 SerializedLocalPrecedesServeIngressStep,
                 AsyncServeIngressTargetOnlyTurn
    <2>3. RunNode(node) \in BOOLEAN
      BY Isa DEF RunNode
    <2>4. LocalRunNodeStep(node) => RunNode(node)
      BY <1>1, Isa
         DEF LocalRunNodeStep, NodeServiceFrame,
             LocalServeSchedulerStep,
             RecoveryRunNodeGuard, RunNode, RunNodeWork
    <2>5. ENABLED LocalRunNodeStep(node) => ENABLED RunNode(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

IngressRunNodeStep(node) ==
  IngressDrainStep(node) /\ NodeServiceFrame(node)

THEOREM EnabledIngressDrainLiftsToRunNode ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ node \in up
    /\ ~NodeHasApplication(node)
    /\ RecoveryRunNodeGuard(node)
    /\ ~AsyncCandidateProducerContinuationResolutionRequired(node)
    /\ ENABLED IngressDrainStep(node)
    => ENABLED RunNode(node)
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                /\ node \in up
                /\ ~NodeHasApplication(node)
                /\ RecoveryRunNodeGuard(node)
                /\ ~AsyncCandidateProducerContinuationResolutionRequired(node)
                /\ ENABLED IngressDrainStep(node)
         PROVE ENABLED RunNode(node)
    <2>1. ENABLED IngressRunNodeStep(node)
      BY <1>1, ExpandENABLED, Isa
         DEF IngressRunNodeStep, NodeServiceFrame,
             IngressDrainStep, DrainFairIngressSelected,
             PopSelectedIngress, EnqueueCandidate, LeaveCausalQueues,
             AsyncIoVars, AsyncDeferredVars, vars
    <2>2. IngressRunNodeStep(node) \in BOOLEAN
      BY Isa DEF IngressRunNodeStep, NodeServiceFrame,
                 IngressDrainStep
    <2>3. RunNode(node) \in BOOLEAN
      BY Isa DEF RunNode
    <2>4. IngressRunNodeStep(node) => RunNode(node)
      BY <1>1, Isa
         DEF IngressRunNodeStep, NodeServiceFrame,
             RecoveryRunNodeGuard, RunNode, RunNodeWork
    <2>5. ENABLED IngressRunNodeStep(node) => ENABLED RunNode(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

RuntimeServeSchedulerRunNodeStep(node) ==
  RuntimeServeSchedulerStep(node) /\ NodeServiceFrame(node)

THEOREM EnabledRuntimeServeSchedulerLiftsToRunNode ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ node \in up
    /\ ~NodeHasApplication(node)
    /\ RecoveryRunNodeGuard(node)
    /\ ~AsyncCandidateProducerContinuationResolutionRequired(node)
    /\ ENABLED RuntimeServeSchedulerStep(node)
    => ENABLED RunNode(node)
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                /\ node \in up
                /\ ~NodeHasApplication(node)
                /\ RecoveryRunNodeGuard(node)
                /\ ~AsyncCandidateProducerContinuationResolutionRequired(node)
                /\ ENABLED RuntimeServeSchedulerStep(node)
         PROVE ENABLED RunNode(node)
    <2>1. ENABLED RuntimeServeSchedulerRunNodeStep(node)
      BY <1>1, ExpandENABLED, Isa
         DEF RuntimeServeSchedulerRunNodeStep,
             RuntimeServeSchedulerStep, NodeServiceFrame,
             SerializedRuntimeStep, RuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             AsyncServeIngressTargetOnlyTurn,
             DeferredDrainStep, DeferredTagStep,
             DeferredTimeoutStep, DeferredRetransmitStep,
             DirectTimeoutStep, FifoRuntimeStep,
             DirectRetransmitStep, IdleRuntimeStep,
             BeginTimeoutEnabled, CommandDispatchable,
             CommandExecutionReady, ExecuteCommand,
             ExecuteRegularCommand, ExecuteSignProposal,
             ExecuteSignVote, ExecuteFormPrepareQC,
             ExecuteSignTimeout, ExecutePersistInstall,
             ExecutePersistDecision, ExecuteRequestCertifiedBody,
             ExecuteApply, ExecuteCoreDelivery,
             ExecuteChunkDelivery, ExecuteRejectAuthenticatedJunk,
             RegularCoreCommand, AssembleLocalBody,
             BeginLocalProposal, PersistProposal, FetchBody,
             RebindRetainedBody,
             StoreBody, ValidateBody, RejectBody,
             ValidateDecidedBody, ValidateLockedBody,
             BeginPrepare, PersistPrepare,
             BeginObservePrepare, PersistObservePrepare,
             BeginLockCommit, PersistLockCommit, FormCommitQC,
             BeginDecision, BeginTimeout, PersistTimeout, FormTC,
             BeginInstallTC, FetchCertifiedBody,
             CompleteProposalSignature, CompleteVoteSignature,
             FormPrepareQC, CompleteTimeoutSignature,
             PersistInstallTC, PersistDecision, ApplyDecision,
             DeliverProposal, DeliverVote, DeliverQC,
             DeliverTimeout, DeliverTC,
             PublishCommitCertificateRequests,
             PublishControlAndEphemeralItems, PublishControlItems,
             PersistInstalledControl, PersistInstalledControlAfterInstall,
             PersistDecisionControl,
             PublishCertifiedRequests, SendNodeRetransmissions,
             NoSendItem, RemoveNextNodeCommand,
             RemoveNextDeferredCommand, DeferCommand, DiscardCommand,
             AppendCausalSuccessors, LeaveCausalQueues,
             AsyncAuxVars, AsyncIoVars, AsyncDeferredVars,
             AsyncLocalAdmissionVars, vars
    <2>2. RuntimeServeSchedulerRunNodeStep(node) \in BOOLEAN
      BY Isa DEF RuntimeServeSchedulerRunNodeStep,
                 RuntimeServeSchedulerStep, NodeServiceFrame,
                 SerializedRuntimeStep,
                 SerializedRuntimePrecedesServeIngressStep,
                 AsyncServeIngressTargetOnlyTurn, RuntimeStep
    <2>3. RunNode(node) \in BOOLEAN
      BY Isa DEF RunNode
    <2>4. RuntimeServeSchedulerRunNodeStep(node) => RunNode(node)
      BY <1>1, Isa
         DEF RuntimeServeSchedulerRunNodeStep,
             RuntimeServeSchedulerStep, NodeServiceFrame,
             RecoveryRunNodeGuard, RunNode, RunNodeWork
    <2>5. ENABLED RuntimeServeSchedulerRunNodeStep(node)
             => ENABLED RunNode(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

ResponsiveRunNodeBlockedOnExternalContinuation(node) ==
  /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionReady(node)
  /\ (AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord(node))
       .sourceClass \in {"ConditionalTransport", "VolatileBody"}

THEOREM ResponsiveUnappliedRunNodeEnabledOrAwaitsExternalContinuation ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ node \in up
    /\ ~NodeHasApplication(node)
    /\ RecoveryRunNodeGuard(node)
    => \/ ENABLED RunNode(node)
       \/ ResponsiveRunNodeBlockedOnExternalContinuation(node)
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                /\ AsyncStrongTypeInvariant
                /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
                /\ node \in up
                /\ ~NodeHasApplication(node)
                /\ RecoveryRunNodeGuard(node)
         PROVE \/ ENABLED RunNode(node)
                \/ ResponsiveRunNodeBlockedOnExternalContinuation(node)
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncStrongTypeProjectsAsyncType,
         AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>2. asyncRunnerPhase[node]
             \in {"Local", "Ingress", "Runtime"}
      BY <1>1, <2>1, AsyncStrongTypeProjectsAsyncType
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant
    <2>3. CASE
              AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
      <3>1. CASE
                AsyncCandidateProducerContinuationRunnerResolutionReady(node)
        BY <1>1, <2>1, <2>3, <3>1,
           AutoUSE, ExpandENABLED, IsaT(300)
           DEF RunNode, RunNodeWork,
               ResolveRunNodeCandidateProducerContinuation,
               AsyncSchedulerExceptCausalControlAndNodeService,
               AsyncAllVars, AsyncSchedulerVars,
               AsyncIoVars, AsyncDeferredVars,
               AsyncLocalAdmissionVars, vars
      <3>2. CASE
                /\ ~AsyncCandidateProducerContinuationRunnerResolutionReady(
                     node)
                /\ (AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord(
                       node)).sourceClass = "Local"
        BY <1>1, <2>1, <2>2, <2>3, <3>2,
           AsyncCandidateProducerContinuationSelectedLocalReplayHasReservedCapacity,
           AsyncStrongTypeProjectsControlServiceStateType,
           AsyncCandidateProducerContinuationRunnerSelectionIsTwoStageLogicalMinimum,
           AutoUSE, ExpandENABLED, IsaT(900)
           DEF RunNode, RunNodeWork,
               ReplayRunNodeCandidateProducerContinuation,
               AsyncCandidateProducerContinuationExactLocalReplayStep,
               AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
               AsyncCandidateProducerContinuationExactRuntimeReplayStep,
               AsyncCandidateProducerContinuationSelectedReplayRecord,
               AsyncCandidateProducerContinuationExactReplayIdentity,
               AsyncCandidateProducerContinuationSelectedLocalCandidate,
               AsyncCandidateProducerContinuationRuntimeReplayCarrier,
               AsyncStrongTypeInvariant,
               AsyncCandidateProducerContinuationRunnerResolutionRequired,
               AsyncCandidateProducerContinuationRunnerResolutionReady,
               AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
               AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
               AsyncCandidateProducerContinuationRunnerMayPrecedeIngress,
               AsyncCandidateProducerContinuationResolutionRecordsForNode,
               AsyncCandidateProducerContinuations,
               AsyncCandidateProducerContinuationRecordSet,
               AsyncCandidateProducerContinuationRecord,
               EnqueueCandidate,
               AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
               AsyncSchedulerExceptCausalControlRunnerAndNodeService,
               AsyncAllVars, AsyncSchedulerVars,
               AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
               AsyncConfiguration, vars
      <3>3. CASE
                /\ ~AsyncCandidateProducerContinuationRunnerResolutionReady(
                     node)
                /\ (AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord(
                       node)).sourceClass
                     \in {"ConditionalTransport", "VolatileBody"}
        BY <2>3, <3>3
           DEF ResponsiveRunNodeBlockedOnExternalContinuation
      <3> QED BY <1>1, <2>1, <2>3, <3>1, <3>2, <3>3, Isa
           DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
               AsyncSchedulerTypeInvariant,
               AsyncControlServiceStateTypeInvariant,
               AsyncCandidateProducerContinuationRunnerResolutionRequired,
               AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
               AsyncCandidateProducerContinuationRunnerMayPrecedeIngress,
               AsyncCandidateProducerContinuationResolutionRecordsForNode,
               AsyncCandidateProducerContinuationRecordSet,
               AsyncCandidateProducerContinuationRecord,
               AsyncCandidateProducerContinuationSourceClasses
    <2>4. CASE
              /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
                   node)
              /\ asyncRunnerPhase[node] = "Local"
      <3>1. ENABLED LocalServeSchedulerStep(node)
        BY <2>1, <2>4, LocalServeSchedulerStepIsEnabled
      <3> QED BY <1>1, <2>4, <3>1,
           EnabledLocalServeSchedulerLiftsToRunNode
    <2>5. CASE
              /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
                   node)
              /\ asyncRunnerPhase[node] = "Ingress"
      <3>1. ENABLED IngressDrainStep(node)
        BY <2>1, <2>5, IngressDrainStepIsEnabled
      <3> QED BY <1>1, <2>5, <3>1,
           EnabledIngressDrainLiftsToRunNode
    <2>6. CASE
              /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
                   node)
              /\ asyncRunnerPhase[node] = "Runtime"
      <3>1. ENABLED RuntimeServeSchedulerStep(node)
        BY <2>1, <2>6, RuntimeServeSchedulerStepIsEnabled
      <3> QED BY <1>1, <2>6, <3>1,
           EnabledRuntimeServeSchedulerLiftsToRunNode
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM ResponsiveExternalContinuationSelectionIsReady ==
  \A node \in ValidatorIds:
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
    /\ (AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord(node))
         .sourceClass \in {"ConditionalTransport", "VolatileBody"}
      => AsyncCandidateProducerContinuationRunnerResolutionReady(node)
BY Isa
   DEF AsyncCandidateProducerContinuationExternalCoverageInvariant,
       AsyncCandidateProducerContinuationRunnerResolutionReady,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
       AsyncCandidateProducerContinuationRunnerMayPrecedeIngress,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationDurableTerminal

THEOREM ResponsiveUnappliedRunNodeIsEnabled ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ node \in up
    /\ ~NodeHasApplication(node)
    /\ RecoveryRunNodeGuard(node)
    => ENABLED RunNode(node)
BY ResponsiveUnappliedRunNodeEnabledOrAwaitsExternalContinuation,
   ResponsiveExternalContinuationSelectionIsReady, Isa
   DEF ResponsiveRunNodeBlockedOnExternalContinuation

THEOREM AppliedNodeHasDecision ==
  \A node \in ValidatorIds:
    StrongInductiveInvariant /\ NodeHasApplication(node)
      => NodeHasDecision(node)
BY SMT
   DEF StrongInductiveInvariant, Safety, AppliedRequiresDecision,
       NodeHasApplication, NodeHasDecision

THEOREM GstExcludesResponsiveReplayQuarantine ==
  /\ AsyncGstRecoveryPhaseInvariant
  /\ gst
  => \A node: ~ResponsiveReplayQuarantined(node)
BY Isa
   DEF AsyncGstRecoveryPhaseInvariant,
       ResponsiveReplayQuarantined

THEOREM GstResponsiveNodesAreUp ==
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncGstRecoveryPhaseInvariant
  /\ gst
  => Responsive \subseteq up
BY Isa
   DEF AsyncRecoveryTypeInvariant, AsyncRecoveryPhases,
       AsyncGstRecoveryPhaseInvariant

THEOREM EnabledRunNodeLiftsPostGst ==
  \A node \in AsyncCurrentResponsiveVoters:
    gst /\ ENABLED RunNode(node)
      => ENABLED PostGstRunNode(node)
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                gst,
                ENABLED RunNode(node)
         PROVE ENABLED PostGstRunNode(node)
    <2>1. RunNode(node) \in BOOLEAN
      BY Isa DEF RunNode
    <2>2. PostGstRunNode(node) \in BOOLEAN
      BY Isa DEF PostGstRunNode, RunNode
    <2>3. RunNode(node) => PostGstRunNode(node)
      BY <1>1 DEF PostGstRunNode
    <2>4. ENABLED RunNode(node) => ENABLED PostGstRunNode(node)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED BY <1>1, <2>4
  <1> QED BY <1>1

THEOREM UndecidedResponsiveStateEnablesSchedulerAction ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
  /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
  /\ gst
  /\ ~ResponsiveNodesDecide
  => PostGstSchedulerActionEnabled
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncCandidateProducerContinuationExternalCoverageInvariant,
              AsyncCandidateProducerContinuationLocalReplayCapacityInvariant,
              gst,
              ~ResponsiveNodesDecide
         PROVE PostGstSchedulerActionEnabled
    <2>1. PICK node \in AsyncCurrentResponsiveVoters:
             ~NodeHasDecision(node)
      BY <1>1, Isa DEF ResponsiveNodesDecide
    <2>2. node \in ValidatorIds
      BY <1>1, <2>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
             Safety
    <2>3. ~NodeHasApplication(node)
      BY <1>1, <2>1, <2>2, AppliedNodeHasDecision
         DEF AsyncStrongTypeInvariant
    <2>4. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>5. Responsive \subseteq up
      BY <1>1, GstResponsiveNodesAreUp
         DEF AsyncStrongTypeInvariant
    <2>6. node \in up
      BY <2>1, <2>5 DEF AsyncCurrentResponsiveVoters
    <2>7. ~ResponsiveReplayQuarantined(node)
      BY <1>1, GstExcludesResponsiveReplayQuarantine
         DEF AsyncStrongTypeInvariant
    <2>8. RecoveryRunNodeGuard(node)
      BY <2>7 DEF RecoveryRunNodeGuard
    <2>9. ENABLED RunNode(node)
      BY <1>1, <2>1, <2>3, <2>4, <2>6, <2>8,
         ResponsiveUnappliedRunNodeIsEnabled
    <2>10. ENABLED PostGstRunNode(node)
      BY <1>1, <2>1, <2>9, EnabledRunNodeLiftsPostGst
    <2> QED BY <2>1, <2>10 DEF PostGstSchedulerActionEnabled
  <1> QED BY <1>1

THEOREM AsyncSpecAlwaysStrongTypeInvariant ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
    <2>1. AsyncInitAt(initialContext) => AsyncStrongTypeInvariant
      BY AsyncInitEstablishesStrongTypeInvariant
    <2>2. AsyncStrongTypeInvariant /\ [AsyncNext]_AsyncAllVars
             => AsyncStrongTypeInvariant'
      BY AsyncBracketNextPreservesStrongTypeInvariant
    <2> QED BY <2>1, <2>2, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM HistoricalLockRestartAuthoritySourceRetentionFromAsyncSpec ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []HistoricalLockRestartAuthoritySourceRetentionInvariant
BY AsyncSpecAlwaysStrongTypeInvariant, PTL DEF AsyncStrongTypeInvariant

THEOREM AsyncSpecAlwaysHistoricalLockedBodyRecoveryStage ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []HistoricalLockedBodyRecoveryStageInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []HistoricalLockedBodyRecoveryStageInvariant
    <2>1. AsyncInitAt(initialContext)
             => HistoricalLockedBodyRecoveryStageInvariant
      BY AsyncInitEstablishesHistoricalLockedBodyRecoveryStage
    <2>2. AsyncSpecAt(initialContext)
             => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>3. AsyncStrongTypeInvariant
             /\ HistoricalLockedBodyRecoveryStageInvariant
             /\ [AsyncNext]_AsyncAllVars
           => HistoricalLockedBodyRecoveryStageInvariant'
      BY AsyncBracketPreservesHistoricalLockedBodyRecoveryStage
    <2> QED BY <2>1, <2>2, <2>3, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM AsyncSpecAlwaysSerializedBusyKernelInvariant ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []AsyncSerializedBusyKernelInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []AsyncSerializedBusyKernelInvariant
    <2>1. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>2. AsyncStrongTypeInvariant
             => AsyncSerializedBusyKernelInvariant
      BY DEF AsyncStrongTypeInvariant
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM AsyncResponsiveRestartEnabledWhileRequired ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncRecoveryRequiredPending
  => ENABLED <<PreGstResponsiveRestart>>_AsyncAllVars
BY ExpandENABLED, RestartReplayIsTypedOwnedAndUnique, SMTT(30)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRestartAuthorityInvariant, AsyncRecoveryRequiredPending,
       PreGstResponsiveRestart,
       Restart, AsyncAllVars, AsyncSchedulerVars, vars

THEOREM AsyncResponsiveRestartEstablishesReplayPending ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncRecoveryRequiredPending
  /\ <<PreGstResponsiveRestart>>_AsyncAllVars
  => AsyncRecoveryReplayPending'
BY SMTT(30), Isa
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRecoveryRequiredPending, AsyncRecoveryReplayPending,
       PreGstResponsiveRestart,
       Restart, AsyncAllVars

THEOREM AsyncResponsiveReplayEnabledWhileRequired ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncRecoveryReplayPending
  => ENABLED <<PreGstResponsiveReplay>>_AsyncAllVars
BY ExpandENABLED, RestartReplayIsTypedOwnedAndUnique, SMTT(30), Isa
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRestartAuthorityInvariant, AsyncRecoveryReplayPending,
       PreGstResponsiveReplay, RecoveryCoreReplay,
       ResetNodeSchedulerForRestart, RestartReplay,
       RestartDecisions, RestartLockedCommitIntents,
       RestartTimeoutIntents, RestartPrepareIntents,
       RestartProposalIntents, ResumeProposal, ResumeVote, ResumeTimeout,
       VoteResumeAuthorized, NodeIdle,
       AsyncAllVars, AsyncSchedulerVars, vars

THEOREM AsyncResponsiveReplayEstablishesDrainingOrRecovered ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncRecoveryReplayPending
  /\ <<PreGstResponsiveReplay>>_AsyncAllVars
  => AsyncRecoveryReplayingPending' \/ AsyncRecoveryRecoveredReady'
BY SMTT(30), Isa
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRecoveryReplayPending, AsyncRecoveryReplayingPending,
       AsyncRecoveryRecoveredReady,
       PreGstResponsiveReplay, RecoveryCoreReplay,
       ResetNodeSchedulerForRestart,
       ResumeProposal, ResumeVote, ResumeTimeout, AsyncAllVars

THEOREM AsyncEligibleReadyStep ==
  /\ AsyncRecoveryEligibleReady
  /\ [AsyncNext]_AsyncAllVars
  => \/ AsyncRecoveryEligibleReady'
     \/ gst'
     \/ AsyncRecoveryRequiredPending'
BY SMTT(30), Isa
   DEF AsyncRecoveryEligibleReady, AsyncRecoveryRequiredPending,
       AsyncNext, AsyncNonCrashStep, AsyncSetGST, SetGST,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       Crash, Restart,
       AsyncAllVars, AsyncRecoveryVars, vars

THEOREM AsyncRecoveryRequiredStep ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncRecoveryRequiredPending
  /\ [AsyncNext]_AsyncAllVars
  => \/ AsyncRecoveryRequiredPending'
     \/ AsyncRecoveryReplayPending'
BY SMTT(30), Isa
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRecoveryRequiredPending, AsyncRecoveryReplayPending,
       AsyncNext, AsyncNonCrashStep, AsyncSetGST,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       Crash, Restart, AsyncAllVars,
       AsyncRecoveryVars, vars

THEOREM AsyncRecoveryReplayStep ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncRecoveryReplayPending
  /\ [AsyncNext]_AsyncAllVars
  => \/ AsyncRecoveryReplayPending'
     \/ AsyncRecoveryReplayingPending'
     \/ AsyncRecoveryRecoveredReady'
BY SMTT(30), Isa
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRecoveryReplayPending, AsyncRecoveryReplayingPending,
       AsyncRecoveryRecoveredReady,
       AsyncNext, AsyncNonCrashStep, AsyncSetGST,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       RecoveryCoreReplay, Crash, Restart,
       ResumeProposal, ResumeVote, ResumeTimeout,
       ResetNodeSchedulerForRestart, AsyncAllVars,
       AsyncRecoveryVars, vars

THEOREM AsyncRecoveryReplayingStep ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncRecoveryReplayingPending
  /\ [AsyncNext]_AsyncAllVars
  => AsyncRecoveryReplayingPending' \/ AsyncRecoveryRecoveredReady'
BY SMTT(30), Isa
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRecoveryReplayingPending, AsyncRecoveryRecoveredReady,
       AsyncNext, AsyncNonCrashStep, AsyncSetGST,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       RearmResponsiveRecovery, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       AsyncAllVars, AsyncRecoveryVars, vars

THEOREM AsyncRecoveredReadyStep ==
  /\ AsyncRecoveryRecoveredReady
  /\ [AsyncNext]_AsyncAllVars
  => AsyncRecoveryRecoveredReady' \/ AsyncRecoveryEligibleReady' \/ gst'
BY SMTT(30), Isa
   DEF AsyncRecoveryEligibleReady, AsyncRecoveryRecoveredReady,
       AsyncNext, AsyncNonCrashStep, AsyncSetGST, SetGST,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       RearmResponsiveRecovery,
       Crash, Restart,
       AsyncAllVars, AsyncRecoveryVars, vars

THEOREM AsyncEligibleReadyLeadsToGstOrRecovery ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => (AsyncRecoveryEligibleReady
            ~> (gst \/ AsyncRecoveryRequiredPending))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => (AsyncRecoveryEligibleReady
                       ~> (gst \/ AsyncRecoveryRequiredPending))
    <2>1. AsyncRecoveryEligibleReady /\ [AsyncNext]_AsyncAllVars
            => AsyncRecoveryEligibleReady'
                 \/ (gst \/ AsyncRecoveryRequiredPending)'
      BY AsyncEligibleReadyStep
    <2>2. AsyncRecoveryEligibleReady
            => ENABLED <<AsyncSetGST>>_AsyncAllVars
      BY AsyncSetGstEnabledWhileReady
    <2>3. <<AsyncSetGST>>_AsyncAllVars
            => (gst \/ AsyncRecoveryRequiredPending)'
      BY AsyncSetGstEstablishesGst
    <2>4. AsyncSpecAt(initialContext)
            => WF_AsyncAllVars(AsyncSetGST)
      BY DEF AsyncSpecAt, AsyncFairnessAt
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM AsyncRecoveryRequiredLeadsToReplayRequired ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => (AsyncRecoveryRequiredPending
            ~> AsyncRecoveryReplayPending)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => (AsyncRecoveryRequiredPending
                       ~> AsyncRecoveryReplayPending)
    <2>1. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>2. /\ AsyncStrongTypeInvariant
           /\ AsyncRecoveryRequiredPending
          /\ [AsyncNext]_AsyncAllVars
          => AsyncRecoveryRequiredPending'
               \/ AsyncRecoveryReplayPending'
      BY AsyncRecoveryRequiredStep
    <2>3. /\ AsyncStrongTypeInvariant
           /\ AsyncRecoveryRequiredPending
          => ENABLED <<PreGstResponsiveRestart>>_AsyncAllVars
      BY AsyncResponsiveRestartEnabledWhileRequired
    <2>4. /\ AsyncStrongTypeInvariant
           /\ AsyncRecoveryRequiredPending
           /\ <<PreGstResponsiveRestart>>_AsyncAllVars
          => AsyncRecoveryReplayPending'
      BY AsyncResponsiveRestartEstablishesReplayPending
    <2>5. AsyncSpecAt(initialContext)
            => WF_AsyncAllVars(PreGstResponsiveRestart)
      BY DEF AsyncSpecAt, AsyncFairnessAt
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, PTL
         DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM AsyncRecoveryReplayLeadsToDrainingOrRecovered ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => (AsyncRecoveryReplayPending
            ~> (AsyncRecoveryReplayingPending
                  \/ AsyncRecoveryRecoveredReady))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => (AsyncRecoveryReplayPending
                       ~> (AsyncRecoveryReplayingPending
                             \/ AsyncRecoveryRecoveredReady))
    <2>1. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>2. /\ AsyncStrongTypeInvariant
          /\ AsyncRecoveryReplayPending
          /\ [AsyncNext]_AsyncAllVars
          => AsyncRecoveryReplayPending'
               \/ AsyncRecoveryReplayingPending'
               \/ AsyncRecoveryRecoveredReady'
      BY AsyncRecoveryReplayStep
    <2>3. /\ AsyncStrongTypeInvariant
           /\ AsyncRecoveryReplayPending
          => ENABLED <<PreGstResponsiveReplay>>_AsyncAllVars
      BY AsyncResponsiveReplayEnabledWhileRequired
    <2>4. /\ AsyncStrongTypeInvariant
           /\ AsyncRecoveryReplayPending
           /\ <<PreGstResponsiveReplay>>_AsyncAllVars
          => AsyncRecoveryReplayingPending'
               \/ AsyncRecoveryRecoveredReady'
      BY AsyncResponsiveReplayEstablishesDrainingOrRecovered
    <2>5. AsyncSpecAt(initialContext)
            => WF_AsyncAllVars(PreGstResponsiveReplay)
      BY DEF AsyncSpecAt, AsyncFairnessAt
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, PTL
         DEF AsyncSpecAt
  <1> QED BY <1>1

(***************************************************************************
The replay-drain obligation is intentionally separate from restart
selection.  It is the exact starvation-freedom boundary: the quarantined
node's ordinary serialized runner and completion I/O worker must consume the
optional locked-body Fetch prefix and current signature before
DriveResponsiveReplayHead may install the next signature from the FIFO tail,
and an empty tail must eventually enable FinishResponsiveReplay.
***************************************************************************)
THEOREM AsyncInitEstablishesReplayTailCommitReadyInvariant ==
  \A initialContext:
    AsyncInitAt(initialContext) => ReplayTailCommitReadyInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncRecoveryInit,
       ReplayTailCommitReadyInvariant

THEOREM EmptyReplayTailProvidesCommitSourcesReady ==
  /\ ReplayTailCommitReadyInvariant
  /\ asyncRecoveryPhase = "Replaying"
  /\ asyncRecoveryReplayQueue = <<>>
  => ReplayCommitSourcesReady(asyncRecoveryNode)
BY Isa
   DEF ReplayTailCommitReadyInvariant, ReplayCommitSourcesReady,
       SequenceSet

THEOREM ReplayCommitCarrierFramePreservesReplayTailInvariant ==
  /\ ReplayTailCommitReadyInvariant
  /\ asyncRecoveryPhase = "Replaying"
  /\ asyncRecoveryPhase' = "Replaying"
  /\ asyncRecoveryReplayQueue' = asyncRecoveryReplayQueue
  /\ ReplayCommitCarrierFrame
  => ReplayTailCommitReadyInvariant'
BY Isa
   DEF ReplayTailCommitReadyInvariant, ReplayCommitCarrierFrame

THEOREM PreGstResponsiveReplayEstablishesReplayTailCommitReadyInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ PreGstResponsiveReplay
  => ReplayTailCommitReadyInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              PreGstResponsiveReplay
         PROVE ReplayTailCommitReadyInvariant'
    <2> DEFINE Node == asyncRecoveryNode
    <2> DEFINE Signatures == RestartSignatureReplay(Node)
    <2>1. CASE asyncRecoveryPhase' # "Replaying"
      BY <2>1 DEF ReplayTailCommitReadyInvariant
    <2>2. CASE asyncRecoveryPhase' = "Replaying"
      <3>1. /\ Len(Signatures) > 0
             /\ asyncRecoveryNode' = Node
             /\ asyncRecoveryReplayQueue' = Tail(Signatures)
             /\ Node \in Responsive
             /\ ~NodeHasApplication(Node)
        BY <1>1, <2>2, Isa
           DEF Node, Signatures, PreGstResponsiveReplay,
               AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
               StrongInductiveInvariant, Safety
      <3>2. RestartLockedCommitIntents(Node)' =
               RestartLockedCommitIntents(Node)
        BY <1>1, Isa
           DEF PreGstResponsiveReplay, RecoveryCoreReplay,
               ResumeProposal, ResumeVote, ResumeTimeout,
               RestartLockedCommitIntents, vars
      <3>3. ASSUME NEW vote \in RestartLockedCommitIntents(Node)'
             PROVE \/ ReplayCommitIntentReady(Node, vote)'
                   \/ ReplayLockedCommitCandidate(Node, vote)'
                        \in SequenceSet(asyncRecoveryReplayQueue')
        <4>1. CASE ReplayCommitIntentReady(Node, vote)'
          BY <4>1
        <4>2. CASE ~ReplayCommitIntentReady(Node, vote)'
          <5>1. /\ vote \in RestartLockedCommitIntents(Node)
                 /\ ~NodeHasDecision(Node)
            BY <1>1, <3>2, <3>3, <4>2, Isa
               DEF PreGstResponsiveReplay, RecoveryCoreReplay,
                   ResumeProposal, ResumeVote, ResumeTimeout,
                   ReplayCommitIntentReady, NodeHasDecision, vars
          <5>2. ReplayLockedCommitCandidate(Node, vote)
                   \in SequenceSet(Signatures)
            BY <1>1, <3>1, <5>1,
               UndecidedActiveLockedCommitIsInSignatureReplay
               DEF AsyncStrongTypeInvariant, Node, Signatures
          <5>3. ReplayLockedCommitCandidate(Node, vote) #
                   Head(Signatures)
            BY <1>1, <3>1, <4>2, Isa
               DEF Node, Signatures, PreGstResponsiveReplay,
                   RecoveryCoreReplay, ResumeProposal, ResumeVote,
                   ResumeTimeout, ReplayCommitIntentReady,
                   ReplayLockedCommitCandidate, RestartCandidate,
                   AsyncCandidateAtConsumer, AsyncCandidateWithIdentity,
                   VoteSign, vars
          <5>4. Signatures \in Seq(AsyncCandidateSet)
            BY <1>1, RestartSignatureReplayProperties
               DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
                   Safety, Signatures, AsyncQueueTyped
          <5>5. ReplayLockedCommitCandidate(Node, vote)
                   \in SequenceSet(Tail(Signatures))
            BY <3>1, <5>2, <5>3, <5>4,
               ReplayTailRetainsNonHeadValue
          <5>6. ReplayLockedCommitCandidate(Node, vote)' =
                   ReplayLockedCommitCandidate(Node, vote)
            BY <1>1, Isa
               DEF ReplayLockedCommitCandidate, RestartCandidate,
                   AsyncCandidateAtConsumer, AsyncCandidateWithIdentity,
                   PreGstResponsiveReplay, RecoveryCoreReplay,
                   ResumeProposal, ResumeVote, ResumeTimeout, vars
          <5> QED BY <3>1, <5>5, <5>6
        <4> QED BY <4>1, <4>2
      <3>4. \A vote \in (RestartLockedCommitIntents(asyncRecoveryNode))':
               \/ (ReplayCommitIntentReady(asyncRecoveryNode, vote))'
               \/ (ReplayLockedCommitCandidate(asyncRecoveryNode, vote))'
                    \in SequenceSet(asyncRecoveryReplayQueue')
        BY <3>1, <3>2, <3>3
      <3> QED BY <2>2, <3>4 DEF ReplayTailCommitReadyInvariant
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM DriveResponsiveReplayHeadPreservesReplayTailCommitReadyInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ ReplayTailCommitReadyInvariant
  /\ DriveResponsiveReplayHead
  => ReplayTailCommitReadyInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              ReplayTailCommitReadyInvariant,
              DriveResponsiveReplayHead
         PROVE ReplayTailCommitReadyInvariant'
    <2> DEFINE Node == asyncRecoveryNode
    <2> DEFINE Queue == asyncRecoveryReplayQueue
    <2>1. /\ asyncRecoveryPhase = "Replaying"
           /\ asyncRecoveryPhase' = "Replaying"
           /\ asyncRecoveryNode' = Node
           /\ Len(Queue) > 0
           /\ asyncRecoveryReplayQueue' = Tail(Queue)
      BY <1>1 DEF Node, Queue, DriveResponsiveReplayHead,
                    AsyncRecoveryLifecycleVars
    <2>2. /\ RestartLockedCommitIntents(Node)' =
                 RestartLockedCommitIntents(Node)
           /\ \A vote \in RestartLockedCommitIntents(Node):
                ReplayLockedCommitCandidate(Node, vote)' =
                  ReplayLockedCommitCandidate(Node, vote)
      BY <1>1, Isa
         DEF DriveResponsiveReplayHead, RecoveryCoreReplay,
             ResumeProposal, ResumeVote, ResumeTimeout,
             RestartLockedCommitIntents,
             ReplayLockedCommitCandidate, RestartCandidate,
             AsyncCandidateAtConsumer, AsyncCandidateWithIdentity,
             AsyncRecoveryLifecycleVars, vars
    <2>3. Queue \in Seq(AsyncCandidateSet)
      BY <1>1
         DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
             AsyncQueueTyped, Queue
    <2>4. ASSUME NEW vote \in RestartLockedCommitIntents(Node)'
           PROVE \/ ReplayCommitIntentReady(Node, vote)'
                 \/ ReplayLockedCommitCandidate(Node, vote)'
                      \in SequenceSet(asyncRecoveryReplayQueue')
      <3>1. vote \in RestartLockedCommitIntents(Node)
        BY <2>2, <2>4
      <3>2. CASE ReplayCommitIntentReady(Node, vote)
        <4>1. ReplayCommitIntentReady(Node, vote)'
          BY <1>1, <3>1, <3>2, Isa
             DEF DriveResponsiveReplayHead, RecoveryCoreReplay,
                 ResumeProposal, ResumeVote, ResumeTimeout,
                 ReplayCommitIntentReady, NodeHasDecision, vars
        <4> QED BY <4>1
      <3>3. CASE ~ReplayCommitIntentReady(Node, vote)
        <4>1. ReplayLockedCommitCandidate(Node, vote)
                 \in SequenceSet(Queue)
          BY <1>1, <2>1, <3>1, <3>3
             DEF ReplayTailCommitReadyInvariant, Node, Queue
        <4>2. CASE ReplayLockedCommitCandidate(Node, vote) = Head(Queue)
          <5>1. ReplayCommitIntentReady(Node, vote)'
            BY <1>1, <2>1, <3>1, <4>2, Isa
               DEF Node, Queue, DriveResponsiveReplayHead,
                   RecoveryCoreReplay, ResumeProposal, ResumeVote,
                   ResumeTimeout, ReplayCommitIntentReady,
                   ReplayLockedCommitCandidate, RestartCandidate,
                   AsyncCandidateAtConsumer, AsyncCandidateWithIdentity,
                   VoteSign, vars
          <5> QED BY <5>1
        <4>3. CASE ReplayLockedCommitCandidate(Node, vote) # Head(Queue)
          <5>1. ReplayLockedCommitCandidate(Node, vote)
                   \in SequenceSet(Tail(Queue))
            BY <2>1, <2>3, <4>1, <4>3,
               ReplayTailRetainsNonHeadValue
          <5> QED BY <2>1, <2>2, <5>1
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>2, <3>3
    <2>5. \A vote \in (RestartLockedCommitIntents(asyncRecoveryNode))':
             \/ (ReplayCommitIntentReady(asyncRecoveryNode, vote))'
             \/ (ReplayLockedCommitCandidate(asyncRecoveryNode, vote))'
                  \in SequenceSet(asyncRecoveryReplayQueue')
      BY <2>1, <2>2, <2>4
    <2> QED BY <2>1, <2>5 DEF ReplayTailCommitReadyInvariant
  <1> QED BY <1>1

THEOREM ReplayingRunNodeWorkPreservesCommitCarrierFrame ==
  \A runner \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ RunNodeWork(runner)
    /\ UNCHANGED AsyncRecoveryControlVars
    => ReplayCommitCarrierFrame
PROOF
  <1>1. ASSUME NEW runner \in ValidatorIds,
                AsyncStrongTypeInvariant,
                asyncRecoveryPhase = "Replaying",
                RunNodeWork(runner),
                UNCHANGED AsyncRecoveryControlVars
         PROVE ReplayCommitCarrierFrame
    <2>1. CASE runner = asyncRecoveryNode
      BY <1>1, <2>1, SMTT(120), Isa
         DEF ReplayCommitCarrierFrame, ReplayCommitIntentReady,
             ReplayLockedCommitCandidate, RestartCandidate,
             RestartLockedCommitIntents, NodeHasDecision,
             AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
             AsyncSchedulerTypeInvariant,
             ResponsiveReplayScheduledCandidates,
             CandidateScheduled, QueuedCandidates, DeferredCandidates,
             CausalCandidates, TrackedWorkCandidates,
             RunNodeWork, LocalAdmissionStep, IngressDrainStep,
             ResolveRunNodeCandidateProducerContinuation,
             ReplayRunNodeCandidateProducerContinuation,
             AsyncCandidateProducerContinuationExactLocalReplayStep,
             AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
             AsyncCandidateProducerContinuationExactRuntimeReplayStep,
             AsyncCandidateProducerContinuationExactReplayIdentity,
             AsyncCandidateProducerContinuationSelectedLocalCandidate,
             AsyncCandidateProducerContinuationSelectedRuntimeCandidate,
             AsyncCandidateProducerContinuationSelectedReplayRecord,
             AsyncCandidateProducerContinuationSelectedResolutionRecord,
             AsyncCandidateProducerContinuationResolutionRequired,
             AsyncCandidateProducerContinuationResolutionRecordsForNode,
             ResponsiveReplayQuarantined,
             AsyncSchedulerExceptCausalControlAndNodeService,
             AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
             AsyncSchedulerExceptCausalControlRunnerAndNodeService,
             AsyncIoTimeoutLifecycleRetirementTransition,
             EnqueueCandidate,
             SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
             DeferredDrainStep, DeferredTagStep, DirectTimeoutStep,
             DirectRetransmitStep, IdleRuntimeStep,
             ExecuteCommand, ExecuteRegularCommand,
             ExecuteDecisionFetch, ExecuteSignProposal,
             ExecuteSignVote, ExecuteFormPrepareQC,
             ExecuteSignTimeout, ExecutePersistInstall,
             ExecutePersistDecision, ExecuteRequestCertifiedBody,
             ExecuteApply, ExecuteCoreDelivery, ExecuteChunkDelivery,
             ExecuteRejectAuthenticatedJunk,
             CompleteVoteSignature, PersistInstallTC,
             PublishControlItems, RememberedControl,
             AsyncCandidateAtConsumer, AsyncCandidateWithIdentity,
             AsyncRecoveryVars, vars
    <2>2. CASE runner # asyncRecoveryNode
      BY <1>1, <2>2, SMTT(120), Isa
         DEF ReplayCommitCarrierFrame, ReplayCommitIntentReady,
             ReplayLockedCommitCandidate, RestartCandidate,
             RestartLockedCommitIntents, NodeHasDecision,
             AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
             RunNodeWork, LocalAdmissionStep, IngressDrainStep,
             ResolveRunNodeCandidateProducerContinuation,
             ReplayRunNodeCandidateProducerContinuation,
             AsyncCandidateProducerContinuationExactLocalReplayStep,
             AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
             AsyncCandidateProducerContinuationExactRuntimeReplayStep,
             AsyncCandidateProducerContinuationExactReplayIdentity,
             AsyncCandidateProducerContinuationSelectedLocalCandidate,
             AsyncCandidateProducerContinuationSelectedRuntimeCandidate,
             AsyncCandidateProducerContinuationSelectedReplayRecord,
             AsyncCandidateProducerContinuationSelectedResolutionRecord,
             AsyncCandidateProducerContinuationResolutionRequired,
             AsyncCandidateProducerContinuationResolutionRecordsForNode,
             ResponsiveReplayQuarantined,
             AsyncSchedulerExceptCausalControlAndNodeService,
             AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
             AsyncSchedulerExceptCausalControlRunnerAndNodeService,
             AsyncIoTimeoutLifecycleRetirementTransition,
             EnqueueCandidate,
             SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
             DeferredDrainStep, DeferredTagStep, DirectTimeoutStep,
             DirectRetransmitStep, IdleRuntimeStep,
             ExecuteCommand, ExecuteRegularCommand,
             ExecuteDecisionFetch, ExecuteSignProposal,
             ExecuteSignVote, ExecuteFormPrepareQC,
             ExecuteSignTimeout, ExecutePersistInstall,
             ExecutePersistDecision, ExecuteRequestCertifiedBody,
             ExecuteApply, ExecuteCoreDelivery, ExecuteChunkDelivery,
             ExecuteRejectAuthenticatedJunk,
             CompleteVoteSignature, PersistLockCommit,
             PersistInstallTC, PublishControlItems, RememberedControl,
             AsyncCandidateAtConsumer, AsyncCandidateWithIdentity,
             AsyncRecoveryVars, vars
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplayingNonRunnerStepPreservesCommitCarrierFrame ==
  /\ AsyncStrongTypeInvariant
  /\ asyncRecoveryPhase = "Replaying"
  /\ AsyncNonRunnerStep
  /\ UNCHANGED AsyncRecoveryControlVars
  => ReplayCommitCarrierFrame
BY SMTT(120), Isa
   DEF ReplayCommitCarrierFrame, ReplayCommitIntentReady,
       ReplayLockedCommitCandidate, RestartCandidate,
       RestartLockedCommitIntents, NodeHasDecision,
       AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncNonRunnerStep, AsyncSetGST, AsyncTick,
       OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, AsyncNetworkStep, AsyncFaultStep,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       PreGstLosePacket, PreGstCrash, Crash,
       InjectByzantineNoise, InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk,
       InjectByzantineCertifiedRequest, AsyncByzantineProposal,
       AsyncByzantineVote, AsyncByzantineTimeout,
       ByzantineBroadcastProposal, ByzantineBroadcastVote,
       ByzantineBroadcastTimeout, PublishEphemeralItems,
       AsyncCandidateAtConsumer, AsyncCandidateWithIdentity,
       AsyncRecoveryVars, vars

THEOREM ReplayingOrdinaryAsyncStepPreservesCommitCarrierFrame ==
  /\ AsyncStrongTypeInvariant
  /\ asyncRecoveryPhase = "Replaying"
  /\ (AsyncRunnerStep \/ AsyncNonRunnerStep)
  /\ UNCHANGED AsyncRecoveryControlVars
  => ReplayCommitCarrierFrame
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              asyncRecoveryPhase = "Replaying",
              AsyncRunnerStep \/ AsyncNonRunnerStep,
              UNCHANGED AsyncRecoveryControlVars
         PROVE ReplayCommitCarrierFrame
    <2>1. CASE AsyncRunnerStep
      <3>1. CASE \E runner \in AsyncCurrentResponsiveVoters:
                    RunNode(runner)
        BY <1>1, <3>1, AsyncCurrentResponsiveVotersAreValidators,
           ReplayingRunNodeWorkPreservesCommitCarrierFrame
           DEF RunNode
      <3>2. CASE \E runner \in asyncHistoricalRecoveryTargets:
                    RunHistoricalRecoveryNode(runner)
        BY <1>1, <3>2, HistoricalRecoveryTargetsAreValidators,
           ReplayingRunNodeWorkPreservesCommitCarrierFrame
           DEF RunHistoricalRecoveryNode
      <3>3. CASE \E runner \in AsyncResponsiveAppliedArchiveServers:
                    RunHistoricalServer(runner)
        BY <1>1, <3>3, Isa
           DEF ReplayCommitCarrierFrame, ReplayCommitIntentReady,
               ReplayLockedCommitCandidate, RestartCandidate,
               RestartLockedCommitIntents, NodeHasDecision,
               RunHistoricalServer, AsyncRecoveryVars, vars
      <3> QED BY <3>1, <3>2, <3>3 DEF AsyncRunnerStep
    <2>2. CASE AsyncNonRunnerStep
      BY <1>1, <2>2,
         ReplayingNonRunnerStepPreservesCommitCarrierFrame
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplayingPreGstCrashPreservesCommitCarrierFrame ==
  \A crashed \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ asyncRecoveryPhase = "Replaying"
    /\ PreGstCrash(crashed)
    => ReplayCommitCarrierFrame
BY SMTT(45), Isa
   DEF ReplayCommitCarrierFrame, ReplayCommitIntentReady,
       ReplayLockedCommitCandidate, RestartCandidate,
       RestartLockedCommitIntents, NodeHasDecision,
       AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       PreGstCrash, Crash, AsyncSchedulerVars, AsyncRecoveryVars,
       AsyncCandidateAtConsumer, AsyncCandidateWithIdentity, vars

THEOREM AsyncNextPreservesReplayTailCommitReadyInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ ReplayTailCommitReadyInvariant
  /\ AsyncNext
  => ReplayTailCommitReadyInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              ReplayTailCommitReadyInvariant,
              AsyncNext
         PROVE ReplayTailCommitReadyInvariant'
    <2>1. CASE asyncRecoveryPhase' # "Replaying"
      BY <2>1 DEF ReplayTailCommitReadyInvariant
    <2>2. CASE asyncRecoveryPhase' = "Replaying"
      <3>1. asyncRecoveryPhase \in {"ReplayRequired", "Replaying"}
        BY <1>1, <2>2, SMTT(45), Isa
           DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
               AsyncRecoveryPhases, AsyncNext, AsyncNonCrashStep,
               AsyncSetGST, PreGstCrash, PreGstResponsiveCrash,
               PreGstResponsiveRestart, PreGstResponsiveReplay,
               DriveResponsiveReplayHead, FinishResponsiveReplay,
               RearmResponsiveRecovery, AsyncRecoveryVars
      <3>2. CASE asyncRecoveryPhase = "ReplayRequired"
        <4>1. PreGstResponsiveReplay
          BY <1>1, <2>2, <3>2, SMTT(45), Isa
             DEF AsyncNext, AsyncNonCrashStep, AsyncSetGST,
                 PreGstCrash, PreGstResponsiveCrash,
                 PreGstResponsiveRestart, PreGstResponsiveReplay,
                 DriveResponsiveReplayHead, FinishResponsiveReplay,
                 RearmResponsiveRecovery, AsyncRecoveryVars
        <4> QED BY <1>1, <4>1,
                     PreGstResponsiveReplayEstablishesReplayTailCommitReadyInvariant
      <3>3. CASE asyncRecoveryPhase = "Replaying"
        <4>1. CASE DriveResponsiveReplayHead
          BY <1>1, <4>1,
             DriveResponsiveReplayHeadPreservesReplayTailCommitReadyInvariant
        <4>2. CASE ~DriveResponsiveReplayHead
          <5>1. \/ /\ (AsyncRunnerStep \/ AsyncNonRunnerStep)
                       /\ UNCHANGED AsyncRecoveryControlVars
                       /\ asyncRecoveryReplayQueue' =
                            asyncRecoveryReplayQueue
                 \/ \E crashed \in ValidatorIds:
                       /\ PreGstCrash(crashed)
                       /\ asyncRecoveryReplayQueue' =
                            asyncRecoveryReplayQueue
            BY <1>1, <2>2, <3>3, <4>2, SMTT(60), Isa
               DEF AsyncNext, AsyncNonCrashStep, AsyncSetGST,
                   PreGstCrash, PreGstResponsiveCrash,
                   PreGstResponsiveRestart, PreGstResponsiveReplay,
                   DriveResponsiveReplayHead, FinishResponsiveReplay,
                   RearmResponsiveRecovery, AsyncRecoveryVars
          <5>2. CASE /\ (AsyncRunnerStep \/ AsyncNonRunnerStep)
                        /\ UNCHANGED AsyncRecoveryControlVars
                        /\ asyncRecoveryReplayQueue' =
                             asyncRecoveryReplayQueue
            <6>1. ReplayCommitCarrierFrame
              BY <1>1, <3>3, <5>2,
                 ReplayingOrdinaryAsyncStepPreservesCommitCarrierFrame
            <6> QED BY <1>1, <2>2, <3>3, <5>2, <6>1,
                 ReplayCommitCarrierFramePreservesReplayTailInvariant
          <5>3. CASE \E crashed \in ValidatorIds:
                        /\ PreGstCrash(crashed)
                        /\ asyncRecoveryReplayQueue' =
                             asyncRecoveryReplayQueue
            <6>1. PICK crashed \in ValidatorIds:
                     /\ PreGstCrash(crashed)
                     /\ asyncRecoveryReplayQueue' =
                          asyncRecoveryReplayQueue
              BY <5>3
            <6>2. ReplayCommitCarrierFrame
              BY <1>1, <3>3, <6>1,
                 ReplayingPreGstCrashPreservesCommitCarrierFrame
            <6> QED BY <1>1, <2>2, <3>3, <6>1, <6>2,
                 ReplayCommitCarrierFramePreservesReplayTailInvariant
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1, <3>2, <3>3
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncAllVarsStutterPreservesReplayTailCommitReadyInvariant ==
  /\ ReplayTailCommitReadyInvariant
  /\ UNCHANGED AsyncAllVars
  => ReplayTailCommitReadyInvariant'
BY Isa
   DEF ReplayTailCommitReadyInvariant, ReplayCommitIntentReady,
       ReplayLockedCommitCandidate, RestartCandidate,
       RestartLockedCommitIntents, NodeHasDecision,
       AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars, vars

THEOREM AsyncBracketNextPreservesReplayTailCommitReadyInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ ReplayTailCommitReadyInvariant
  /\ [AsyncNext]_AsyncAllVars
  => ReplayTailCommitReadyInvariant'
BY AsyncNextPreservesReplayTailCommitReadyInvariant,
   AsyncAllVarsStutterPreservesReplayTailCommitReadyInvariant,
   Isa

THEOREM ReplayTailCommitReadyInvariantObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []ReplayTailCommitReadyInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []ReplayTailCommitReadyInvariant
    <2>1. AsyncInitAt(initialContext)
            => ReplayTailCommitReadyInvariant
      BY AsyncInitEstablishesReplayTailCommitReadyInvariant
    <2>2. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>3. /\ AsyncStrongTypeInvariant
           /\ ReplayTailCommitReadyInvariant
           /\ [AsyncNext]_AsyncAllVars
          => ReplayTailCommitReadyInvariant'
      BY AsyncBracketNextPreservesReplayTailCommitReadyInvariant
    <2> QED BY <2>1, <2>2, <2>3, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM ResponsiveReplayRunNodeEnabledWhileReplaying ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
  /\ AsyncRecoveryReplayingPending
  /\ ~NodeIdle(asyncRecoveryNode)
  => ENABLED <<ResponsiveReplayRunNode>>_AsyncAllVars
BY ResponsiveUnappliedRunNodeEnabledOrAwaitsExternalContinuation,
   ExpandENABLED, Isa
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRecoveryExecutionInvariant,
       AsyncRecoveryReplayingPending, ResponsiveReplayRunNode,
       RecoveryRunNodeGuard,
       ResponsiveReplayQuarantined, ResponsiveReplayDraining,
       ResponsiveRunNodeBlockedOnExternalContinuation,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       RunNode, RunNodeWork, AsyncAllVars,
       AsyncRecoveryVars

THEOREM ResponsiveReplayIoWorkerEnabledWhileQueued ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncRecoveryReplayingPending
  /\ AsyncIoQueueDepth(asyncRecoveryNode) > 0
  => ENABLED <<ResponsiveReplayServiceIoWorker>>_AsyncAllVars
BY ExpandENABLED, Isa
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       AsyncRecoveryReplayingPending,
       ResponsiveReplayServiceIoWorker, ResponsiveReplayDraining,
       ServiceIoWorker, ResponsiveReplayExecutorAllowed,
       PublishEphemeralItems, LeaveCausalQueues,
       AsyncIoQueueDepth, AsyncAllVars, AsyncSchedulerVars,
       AsyncLocalAdmissionVars, AsyncDeferredVars, vars

THEOREM DriveResponsiveReplayEnabledAtIdleHead ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncRecoveryReplayingPending
  /\ NodeIdle(asyncRecoveryNode)
  /\ Len(asyncRecoveryReplayQueue) > 0
  => ENABLED <<DriveResponsiveReplayHead>>_AsyncAllVars
BY RestartSignatureReplayProperties, ExpandENABLED, SMTT(45), Isa
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety,
       AsyncRecoveryTypeInvariant, AsyncRecoveryReplayingPending,
       DriveResponsiveReplayHead, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout, VoteResumeAuthorized,
       RestartSignatureReplay, RestartTimeoutOrProposalReplay,
       RestartPrepareReplayIfActive, RestartLockedCommitReplayIfActive,
       RestartTimeoutIntents, RestartProposalIntents,
       RestartPrepareIntents, RestartLockedCommitIntents,
       FreshCandidateSequence, CandidateScheduled,
       AsyncAllVars, AsyncRecoveryVars, AsyncSchedulerVars, vars,
       SequenceSet

THEOREM FinishResponsiveReplayEnabledAtIdleTail ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncRecoveryReplayingPending
  /\ NodeIdle(asyncRecoveryNode)
  /\ asyncRecoveryReplayQueue = <<>>
  /\ ReplayCommitSourcesReady(asyncRecoveryNode)
  => ENABLED <<FinishResponsiveReplay>>_AsyncAllVars
BY RestartRunnerAssemblyProperties, ExpandENABLED, SMTT(30), Isa
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety,
       AsyncRecoveryTypeInvariant, AsyncRecoveryReplayingPending,
       FinishResponsiveReplay, ReplayCommitSourcesReady,
       ReplayCommitIntentReady, RestartRunnerAssembly,
       RestartRunnerAssemblyEnabled, FreshCandidateSequence,
       CandidateScheduled, AsyncAllVars, AsyncRecoveryVars,
       AsyncSchedulerVars, vars

THEOREM FinishResponsiveReplayEnabledAtCarriedIdleTail ==
  /\ AsyncStrongTypeInvariant
  /\ ReplayTailCommitReadyInvariant
  /\ AsyncRecoveryReplayingPending
  /\ NodeIdle(asyncRecoveryNode)
  /\ asyncRecoveryReplayQueue = <<>>
  => ENABLED <<FinishResponsiveReplay>>_AsyncAllVars
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              ReplayTailCommitReadyInvariant,
              AsyncRecoveryReplayingPending,
              NodeIdle(asyncRecoveryNode),
              asyncRecoveryReplayQueue = <<>>
         PROVE ENABLED <<FinishResponsiveReplay>>_AsyncAllVars
    <2>1. ReplayCommitSourcesReady(asyncRecoveryNode)
      BY <1>1, EmptyReplayTailProvidesCommitSourcesReady
         DEF AsyncRecoveryReplayingPending
    <2> QED BY <1>1, <2>1,
         FinishResponsiveReplayEnabledAtIdleTail
  <1> QED BY <1>1

THEOREM AsyncRecoverySignatureDrainObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => (AsyncRecoveryReplayingPending ~> AsyncRecoveryRecoveredReady)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => (AsyncRecoveryReplayingPending
                       ~> AsyncRecoveryRecoveredReady)
    <2>1. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>1b. AsyncSpecAt(initialContext)
               => []AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
      BY AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity
    <2>1a. AsyncSpecAt(initialContext)
              => []ReplayTailCommitReadyInvariant
      BY ReplayTailCommitReadyInvariantObligation
    <2>2. /\ AsyncStrongTypeInvariant
           /\ AsyncRecoveryReplayingPending
           /\ [AsyncNext]_AsyncAllVars
          => AsyncRecoveryReplayingPending'
               \/ AsyncRecoveryRecoveredReady'
      BY AsyncRecoveryReplayingStep
    <2>3. /\ AsyncStrongTypeInvariant
           /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
           /\ AsyncRecoveryReplayingPending
           /\ ~NodeIdle(asyncRecoveryNode)
          => ENABLED <<ResponsiveReplayRunNode>>_AsyncAllVars
      BY ResponsiveReplayRunNodeEnabledWhileReplaying
    <2>4. /\ AsyncStrongTypeInvariant
           /\ AsyncRecoveryReplayingPending
           /\ AsyncIoQueueDepth(asyncRecoveryNode) > 0
          => ENABLED
               <<ResponsiveReplayServiceIoWorker>>_AsyncAllVars
      BY ResponsiveReplayIoWorkerEnabledWhileQueued
    <2>5. /\ AsyncStrongTypeInvariant
           /\ AsyncRecoveryReplayingPending
           /\ NodeIdle(asyncRecoveryNode)
           /\ Len(asyncRecoveryReplayQueue) > 0
          => ENABLED <<DriveResponsiveReplayHead>>_AsyncAllVars
      BY DriveResponsiveReplayEnabledAtIdleHead
    <2>6. /\ AsyncStrongTypeInvariant
           /\ ReplayTailCommitReadyInvariant
           /\ AsyncRecoveryReplayingPending
           /\ NodeIdle(asyncRecoveryNode)
           /\ asyncRecoveryReplayQueue = <<>>
          => ENABLED <<FinishResponsiveReplay>>_AsyncAllVars
      BY FinishResponsiveReplayEnabledAtCarriedIdleTail
    <2>7. AsyncSpecAt(initialContext)
            => WF_AsyncAllVars(ResponsiveReplayRunNode)
      BY DEF AsyncSpecAt, AsyncFairnessAt
    <2>8. AsyncSpecAt(initialContext)
            => WF_AsyncAllVars(ResponsiveReplayServiceIoWorker)
      BY DEF AsyncSpecAt, AsyncFairnessAt
    <2>9. AsyncSpecAt(initialContext)
            => WF_AsyncAllVars(DriveResponsiveReplayHead)
      BY DEF AsyncSpecAt, AsyncFairnessAt
    <2>10. AsyncSpecAt(initialContext)
             => WF_AsyncAllVars(FinishResponsiveReplay)
      BY DEF AsyncSpecAt, AsyncFairnessAt
    <2> QED BY <2>1, <2>1a, <2>1b, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10,
                RestartSignatureReplayProperties, SMTT(60), Isa, PTL
         DEF AsyncSpecAt, AsyncRecoveryReplayingPending,
             AsyncRecoveryRecoveredReady,
             ResponsiveReplayRunNode,
             ResponsiveReplayServiceIoWorker,
             DriveResponsiveReplayHead, FinishResponsiveReplay,
             RunNode, ServiceIoWorker, RecoveryCoreReplay,
             ResumeProposal, ResumeVote, ResumeTimeout,
             AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
             AsyncAllVars, AsyncRecoveryVars, vars
  <1> QED BY <1>1

THEOREM AsyncRecoveryReplayLeadsToRecoveredReady ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => (AsyncRecoveryReplayPending ~> AsyncRecoveryRecoveredReady)
BY AsyncRecoveryReplayLeadsToDrainingOrRecovered,
   AsyncRecoverySignatureDrainObligation, PTL

(***************************************************************************
GST is an explicit liveness condition.  Crash/restart before GST may repeat
without consuming a protocol-owned resource, so no finite process-generation
budget is used to derive GST.  The recovery replay lemmas above are local
corridor facts only; all release liveness theorems begin from `gst` or an
explicit `<>gst` premise.
***************************************************************************)
=============================================================================
