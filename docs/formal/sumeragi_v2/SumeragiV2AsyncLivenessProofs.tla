---- MODULE SumeragiV2AsyncLivenessProofs ----
EXTENDS SumeragiV2LivenessProofs, SequenceTheorems, FunctionTheorems

(***************************************************************************
Rank and fairness proof for the production-coupled asynchronous layer.

The lemmas start at the concrete service boundaries.  RuntimeReachRank counts
the remaining serialized run-loop invocations before the reducer phase;
ingress source rank follows the exact scan-and-rotate ready queue; IO rank is
the position in the single worker FIFO.  Recurring Control jobs are appended
and re-armed only after service, so they cannot increase the rank of an
already-admitted Serve or Consensus job.
***************************************************************************)

THEOREM LocalAdmissionStrictlyDecreasesRuntimeReach ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant /\ LocalAdmissionStep(node)
      => RuntimeReachRank(node)' < RuntimeReachRank(node)
BY SMT DEF AsyncTypeInvariant, LocalAdmissionStep, RuntimeReachRank,
           ProducerCompletionCanAdmit, CausalHeadCanAdvance

THEOREM IngressDrainStrictlyDecreasesRuntimeReach ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant /\ IngressDrainStep(node)
      => RuntimeReachRank(node)' < RuntimeReachRank(node)
BY SMT DEF AsyncTypeInvariant, IngressDrainStep, RuntimeReachRank

THEOREM ControlWorkerRearmsExactlyAfterService ==
  \A node \in AsyncCurrentResponsiveVoters:
    (ServiceIoWorker(node)
      /\ Head(asyncIoQueues[node]).class = "Control")
      => asyncIoControlAvailable'[node]
BY SMT DEF ServiceIoWorker

THEOREM NonControlWorkerServiceDoesNotSpuriouslyRearm ==
  \A node \in AsyncCurrentResponsiveVoters:
    (ServiceIoWorker(node)
      /\ Head(asyncIoQueues[node]).class # "Control")
      => asyncIoControlAvailable' = asyncIoControlAvailable
BY SMT DEF ServiceIoWorker

THEOREM RecurringControlAppendsBehindAdmittedWork ==
  \A node \in AsyncCurrentResponsiveVoters:
    EnqueueIoLocalControl(node)
      => SubSeq(asyncIoQueues'[node], 1, AsyncIoQueueDepth(node))
           = asyncIoQueues[node]
BY Isa DEF EnqueueIoLocalControl, AsyncIoQueueDepth

THEOREM IoWorkerRemovesOnlyTheFifoHead ==
  \A node \in AsyncCurrentResponsiveVoters:
    ServiceIoWorker(node)
      => asyncIoQueues'[node] = Tail(asyncIoQueues[node])
BY SMT DEF ServiceIoWorker

THEOREM FirstDrainableSourceNeverFollowsAnotherDrainableSource ==
  \A node \in ValidatorIds:
    \A source \in SequenceSet(asyncIngressReady[node]):
      IngressSourceCanDrain(node, source)
        => FirstDrainableIngressIndex(node)
             <= IngressSourceServiceRank(node, source)
BY Isa DEF FirstDrainableIngressIndex, DrainableIngressIndices,
           IngressSourceServiceRank

THEOREM OverdueNodeServiceStopsPostGstClock ==
  \A node \in AsyncCurrentResponsiveVoters:
    gst /\ asyncNodeServiceDeadlines[node] <= asyncNow
      => ~AsyncTickEnabled
BY SMT DEF AsyncTickEnabled

THEOREM OverdueIoServiceStopsPostGstClock ==
  \A node \in AsyncCurrentResponsiveVoters:
    (gst /\ AsyncIoQueueDepth(node) > 0
      /\ asyncIoServiceDeadlines[node] <= asyncNow)
      => ~AsyncTickEnabled
BY SMT DEF AsyncTickEnabled

THEOREM OverdueResponsivePacketStopsPostGstClock ==
  \A packet \in asyncTransport:
    (gst
      /\ packet.item.source \in AsyncCurrentResponsiveVoters
      /\ packet.item.envelope.recipient \in AsyncCurrentResponsiveVoters
      /\ packet.deadline <= asyncNow)
      => ~AsyncTickEnabled
BY SMT DEF AsyncTickEnabled, OverdueResponsivePackets

THEOREM CertifiedRecoveryNeverRequestsSelf ==
  \A node \in ValidatorIds:
    \A qc \in QcRecordSet:
      \A item \in CertifiedRequestOutbox(node, qc):
        item.envelope.recipient # node
BY SMT DEF CertifiedRequestOutbox, AsyncNetworkItem, AsyncBodyEnvelope

THEOREM RetainedProposalRetryContainsEveryChunk ==
  \A node \in ValidatorIds, item \in asyncRetainedControl:
    (item.source = node /\ item.kind = "Proposal")
      => BroadcastChunkOutbox(node, item.envelope.proposal.view,
                              item.envelope.proposal.subject)
           \subseteq RetainedProposalChunks(node)
BY Isa DEF RetainedProposalChunks

(***************************************************************************
Deductive type closure for the scheduler product.  The Core component is
supplied by the parameterized Core Init/Next induction; this layer proves that
the concrete queues, reservations, deadlines, retained requests, transport,
and ingress topology are initialized and preserved as typed values.
***************************************************************************)

THEOREM AsyncInitEstablishesRuntimeScalarType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncRuntimeScalarTypeInvariant
BY SMTT(30)
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncRuntimeInit,
       AsyncRuntimeScalarTypeInvariant, AsyncConfiguration,
       AsyncQueueTyped

AsyncCausalCoreTypingFacts ==
  /\ context.height \in Heights
  /\ nodeView \in [ValidatorIds -> Views]
  /\ highestRank \in [ValidatorIds -> Ranks]
  /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
  /\ AsyncHeartbeatSubject \in SubjectOrNone

THEOREM CoreTypeImpliesCausalTypingFacts ==
  TypeInvariant => AsyncCausalCoreTypingFacts
BY SMTT(30)
   DEF TypeInvariant, AsyncCausalCoreTypingFacts, ModelConfiguration,
       AsyncHeartbeatSubject, SubjectOrNone

InitialCausalCandidate(node) ==
  NoItemCandidate("Normal", "AssembleBody", node,
                  nodeView[node], AsyncProposalSubject(node))

THEOREM InitialCausalCandidateShape ==
  \A node:
    /\ DOMAIN InitialCausalCandidate(node) =
         {"class", "kind", "node", "height", "view", "subject", "item"}
    /\ InitialCausalCandidate(node).class \in AsyncCommandClasses
    /\ InitialCausalCandidate(node).kind \in AsyncWorkKinds
    /\ InitialCausalCandidate(node).node = node
    /\ InitialCausalCandidate(node).height = context.height
    /\ InitialCausalCandidate(node).view = nodeView[node]
    /\ InitialCausalCandidate(node).subject = AsyncProposalSubject(node)
    /\ InitialCausalCandidate(node).item = NoAsyncItem
PROOF
  <1>1. ASSUME NEW node
         PROVE /\ DOMAIN InitialCausalCandidate(node) =
                    {"class", "kind", "node", "height", "view",
                     "subject", "item"}
               /\ InitialCausalCandidate(node).class
                    \in AsyncCommandClasses
               /\ InitialCausalCandidate(node).kind \in AsyncWorkKinds
               /\ InitialCausalCandidate(node).node = node
               /\ InitialCausalCandidate(node).height = context.height
               /\ InitialCausalCandidate(node).view = nodeView[node]
               /\ InitialCausalCandidate(node).subject =
                    AsyncProposalSubject(node)
               /\ InitialCausalCandidate(node).item = NoAsyncItem
    <2>1. DOMAIN InitialCausalCandidate(node) =
             {"class", "kind", "node", "height", "view", "subject", "item"}
      BY SMT DEF InitialCausalCandidate, NoItemCandidate, AsyncCandidate
    <2>2. InitialCausalCandidate(node).class \in AsyncCommandClasses
      BY SMT DEF InitialCausalCandidate, NoItemCandidate, AsyncCandidate,
                 AsyncCommandClasses
    <2>3. InitialCausalCandidate(node).kind \in AsyncWorkKinds
      BY SMT DEF InitialCausalCandidate, NoItemCandidate, AsyncCandidate,
                 AsyncWorkKinds, AsyncReducerKinds
    <2>4. /\ InitialCausalCandidate(node).node = node
           /\ InitialCausalCandidate(node).height = context.height
           /\ InitialCausalCandidate(node).view = nodeView[node]
           /\ InitialCausalCandidate(node).subject =
                AsyncProposalSubject(node)
           /\ InitialCausalCandidate(node).item = NoAsyncItem
      BY SMT DEF InitialCausalCandidate, NoItemCandidate, AsyncCandidate
    <2> QED BY <2>1, <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM InitialCausalCandidateIsTyped ==
  TypeInvariant
    => \A node \in ValidatorIds:
         AsyncCandidateTyped(
           InitialCausalCandidate(node))
PROOF
  <1>1. ASSUME TypeInvariant,
                NEW node \in ValidatorIds
         PROVE AsyncCandidateTyped(
           InitialCausalCandidate(node))
    <2>1. AsyncCausalCoreTypingFacts
      BY <1>1, CoreTypeImpliesCausalTypingFacts
    <2>2. context.height \in Heights
      BY <2>1 DEF AsyncCausalCoreTypingFacts
    <2>3. nodeView[node] \in Views
      BY <1>1, <2>1, SMT DEF AsyncCausalCoreTypingFacts
    <2>4. AsyncProposalSubject(node) \in SubjectOrNone
      <3>1. CASE highestRank[node] = NoRank
        BY <2>1 DEF AsyncCausalCoreTypingFacts, AsyncProposalSubject
      <3>2. CASE highestRank[node] # NoRank
        BY <1>1, <2>1, SMT
           DEF AsyncCausalCoreTypingFacts, AsyncProposalSubject
      <3> QED BY <3>1, <3>2
    <2>5. InitialCausalCandidate(node).node \in ValidatorIds
      BY <1>1, InitialCausalCandidateShape
    <2> QED BY <2>2, <2>3, <2>4, <2>5,
      InitialCausalCandidateShape
      DEF AsyncCandidateTyped
  <1> QED BY <1>1

THEOREM SingletonSequenceFacts ==
  \A value:
    /\ <<value>> \in Seq({value})
    /\ Range(<<value>>) = {value}
PROOF
  <1>1. ASSUME NEW value
         PROVE /\ <<value>> \in Seq({value})
               /\ Range(<<value>>) = {value}
    <2>1. [index \in 1..1 |-> value] \in Seq({value})
      BY IsASeq, SMT
    <2>2. <<value>> = [index \in 1..1 |-> value]
      BY Isa
    <2>3. <<value>> \in Seq({value})
      BY <2>1, <2>2
    <2>4. Range(<<value>>) =
             {<<value>>[index]: index \in 1..Len(<<value>>)}
      BY <2>3, RangeEquality
    <2> QED BY <2>3, <2>4, Isa
  <1> QED BY <1>1

THEOREM TypedCandidateFormsTypedSingleton ==
  \A candidate:
    AsyncCandidateTyped(candidate) => AsyncQueueTyped(<<candidate>>)
BY SingletonSequenceFacts, Isa DEF AsyncQueueTyped

THEOREM AsyncInitEstablishesCausalType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncCausalTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext) /\ TypeInvariant
         PROVE AsyncCausalTypeInvariant
    <2>1. asyncCausalQueues =
             [node \in ValidatorIds |->
                <<InitialCausalCandidate(node)>>]
      BY <1>1
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncRuntimeInit,
             InitialCausalCandidate
    <2>2. \A node \in ValidatorIds:
             AsyncCandidateTyped(InitialCausalCandidate(node))
      BY <1>1, InitialCausalCandidateIsTyped
    <2>3. DOMAIN asyncCausalQueues = ValidatorIds
      BY <2>1, SMT
    <2>4. \A node \in ValidatorIds:
             AsyncQueueTyped(asyncCausalQueues[node])
      BY <2>1, <2>2, TypedCandidateFormsTypedSingleton, SMT
    <2> QED BY <2>3, <2>4 DEF AsyncCausalTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesRuntimeType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncRuntimeTypeInvariant
BY AsyncInitEstablishesRuntimeScalarType,
   AsyncInitEstablishesCausalType
   DEF AsyncRuntimeTypeInvariant

THEOREM EmptySequenceFacts ==
  /\ <<>> \in Seq({})
  /\ Range(<<>>) = {}
BY EmptySeq, RangeEquality, Isa

THEOREM EmptyAsyncQueueIsTyped ==
  AsyncQueueTyped(<<>>)
BY EmptySequenceFacts, Isa DEF AsyncQueueTyped

THEOREM EmptyAsyncIoSequenceIsTyped ==
  AsyncIoSequenceTyped(<<>>)
BY EmptySequenceFacts, Isa DEF AsyncIoSequenceTyped

THEOREM EmptyAsyncCompletionSequenceIsTyped ==
  AsyncCompletionSequenceTyped(<<>>)
BY EmptySequenceFacts, Isa DEF AsyncCompletionSequenceTyped

THEOREM EmptyAsyncSequenceSet ==
  SequenceSet(<<>>) = {}
BY Isa DEF SequenceSet

THEOREM EmptyAsyncSequenceLengthMatchesCardinality ==
  Len(<<>>) = Cardinality(SequenceSet(<<>>))
BY EmptyAsyncSequenceSet, FS_EmptySet, SMT

THEOREM EmptyQueuedCompletionIndexSet ==
  {index \in 1..Len(<<>>): <<>>[index].class = "Completion"} = {}
BY Isa

THEOREM EmptyAsyncIoConsensusCandidateOwnership ==
  \A node:
    asyncIoQueues[node] = <<>>
      => AsyncIoConsensusCandidateOwnership(
           node, asyncIoQueues, asyncIoReadyCompletions,
           asyncLocalReadyCompletions)
BY Isa
   DEF AsyncIoConsensusCandidateOwnership,
       AsyncIoConsensusQueueOwnership, AsyncIoConsensusIndices,
       SequenceSet

THEOREM AsyncInitEstablishesIoTopologyType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncIoTopologyTypeInvariant
BY SMT
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncIoInit,
       AsyncIoTopologyTypeInvariant

THEOREM AsyncInitEstablishesIoContentType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncIoContentTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncIoContentTypeInvariant
    <2>1. ASSUME NEW node \in ValidatorIds
           PROVE /\ AsyncIoSequenceTyped(asyncIoQueues[node])
                 /\ IsFiniteSet(asyncOutstandingWork[node])
                 /\ \A candidate \in asyncOutstandingWork[node]:
                      /\ AsyncCandidateTyped(candidate)
                      /\ candidate.class = "Completion"
                      /\ candidate.node = node
                 /\ AsyncCompletionSequenceTyped(
                      asyncIoReadyCompletions[node])
                 /\ AsyncCompletionSequenceTyped(
                      asyncLocalReadyCompletions[node])
                 /\ Len(asyncIoReadyCompletions[node]) =
                      Cardinality(SequenceSet(
                        asyncIoReadyCompletions[node]))
                 /\ Len(asyncLocalReadyCompletions[node]) =
                      Cardinality(SequenceSet(
                        asyncLocalReadyCompletions[node]))
                 /\ SequenceSet(asyncIoReadyCompletions[node])
                      \subseteq asyncOutstandingWork[node]
                 /\ SequenceSet(asyncLocalReadyCompletions[node])
                      \subseteq asyncOutstandingWork[node]
                 /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                      SequenceSet(asyncLocalReadyCompletions[node]) = {}
                 /\ \A job \in SequenceSet(asyncIoQueues[node]):
                      job.class = "Consensus" =>
                        job.candidate \in asyncOutstandingWork[node]
                 /\ AsyncIoConsensusCandidateOwnership(
                      node, asyncIoQueues, asyncIoReadyCompletions,
                      asyncLocalReadyCompletions)
                 /\ SequenceSet(asyncCommandQueues[node]) \cap
                      asyncOutstandingWork[node] = {}
      <3>1. /\ asyncIoQueues[node] = <<>>
             /\ asyncOutstandingWork[node] = {}
             /\ asyncIoReadyCompletions[node] = <<>>
             /\ asyncLocalReadyCompletions[node] = <<>>
             /\ asyncCommandQueues[node] = <<>>
        BY <1>1, <2>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncRuntimeInit, AsyncIoInit
      <3>2. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions[node])
        BY <3>1, EmptyAsyncIoSequenceIsTyped,
           EmptyAsyncCompletionSequenceIsTyped
      <3>3. /\ Len(asyncIoReadyCompletions[node]) =
                  Cardinality(SequenceSet(
                    asyncIoReadyCompletions[node]))
             /\ Len(asyncLocalReadyCompletions[node]) =
                  Cardinality(SequenceSet(
                    asyncLocalReadyCompletions[node]))
        BY <3>1, EmptyAsyncSequenceLengthMatchesCardinality
      <3>4. /\ IsFiniteSet(asyncOutstandingWork[node])
             /\ (\A candidate \in asyncOutstandingWork[node]:
                   /\ AsyncCandidateTyped(candidate)
                   /\ candidate.class = "Completion"
                   /\ candidate.node = node)
             /\ SequenceSet(asyncIoReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncLocalReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions[node]) = {}
             /\ (\A job \in SequenceSet(asyncIoQueues[node]):
                   job.class = "Consensus" =>
                     job.candidate \in asyncOutstandingWork[node])
             /\ AsyncIoConsensusCandidateOwnership(
                  node, asyncIoQueues, asyncIoReadyCompletions,
                  asyncLocalReadyCompletions)
             /\ SequenceSet(asyncCommandQueues[node]) \cap
                  asyncOutstandingWork[node] = {}
        BY <3>1, EmptyAsyncSequenceSet,
           EmptyAsyncIoConsensusCandidateOwnership, FS_EmptySet, SMT
      <3> QED BY <3>2, <3>3, <3>4
    <2> QED BY <2>1
         DEF AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant,
             AsyncIoWorkContentTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesIoCapacityType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncIoCapacityTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncIoCapacityTypeInvariant
    <2>1. ASSUME NEW node \in ValidatorIds
           PROVE /\ AsyncQueueDepth(node) <= AsyncQueueCapacity
                 /\ AsyncCompletionLoad(node) <= AsyncCompletionReserve
                 /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
                 /\ AsyncCompletionLoad(node) <= AsyncIoWorkCapacity
      <3>1. /\ asyncCommandQueues[node] = <<>>
             /\ asyncIoQueues[node] = <<>>
             /\ asyncOutstandingWork[node] = {}
             /\ asyncDeferredCompletionQueues[node] = <<>>
        BY <1>1, <2>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncRuntimeInit, AsyncIoInit,
               AsyncDeferredInit
      <3>2. /\ AsyncQueueDepth(node) = 0
             /\ AsyncIoQueueDepth(node) = 0
             /\ DeferredCompletionCount(node) = 0
        BY <3>1, Isa
           DEF AsyncQueueDepth, AsyncIoQueueDepth,
               DeferredCompletionCount
      <3>3. QueuedCompletionIndices(node) = {}
        BY <3>1, EmptyQueuedCompletionIndexSet, SMT
           DEF QueuedCompletionIndices
      <3>4. AsyncCompletionLoad(node) = 0
        BY <3>1, <3>2, <3>3, FS_EmptySet, SMT
           DEF AsyncCompletionLoad, AsyncOutstandingWorkCount,
               QueuedCompletionCount
      <3>5. /\ AsyncQueueCapacity \in Nat
             /\ AsyncCompletionReserve \in Nat
             /\ AsyncIoCapacity \in Nat
             /\ AsyncIoWorkCapacity \in Nat
        BY <1>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncConfiguration,
               AsyncIoCapacity
      <3> QED BY <3>2, <3>4, <3>5, SMT
    <2> QED BY <2>1 DEF AsyncIoCapacityTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesIoType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncIoTypeInvariant
BY AsyncInitEstablishesIoTopologyType,
   AsyncInitEstablishesIoContentType,
   AsyncInitEstablishesIoCapacityType
   DEF AsyncIoTypeInvariant

THEOREM AsyncInitEstablishesDeferredTopologyType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncDeferredTopologyTypeInvariant
BY SMT
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncDeferredInit,
       AsyncDeferredTopologyTypeInvariant

THEOREM AsyncInitEstablishesDeferredContentType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncDeferredContentTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncDeferredContentTypeInvariant
    <2>1. ASSUME NEW node \in ValidatorIds
           PROVE /\ AsyncCompletionSequenceTyped(
                        asyncDeferredCompletionQueues[node])
                 /\ AsyncQueueTyped(asyncDeferredProgressQueues[node])
                 /\ AsyncQueueTyped(asyncDeferredNormalQueues[node])
                 /\ \A candidate \in
                        SequenceSet(asyncDeferredProgressQueues[node]):
                      candidate.class = "Progress"
                 /\ \A candidate \in
                        SequenceSet(asyncDeferredNormalQueues[node]):
                      candidate.class = "Normal"
                 /\ Len(asyncDeferredProgressQueues[node]) <=
                      AsyncDeferredProgressCapacity
                 /\ Len(asyncDeferredNormalQueues[node]) <=
                      AsyncDeferredNormalCapacity
      <3>1. /\ asyncDeferredCompletionQueues[node] = <<>>
             /\ asyncDeferredProgressQueues[node] = <<>>
             /\ asyncDeferredNormalQueues[node] = <<>>
        BY <1>1, <2>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncDeferredInit
      <3>2. /\ AsyncCompletionSequenceTyped(
                  asyncDeferredCompletionQueues[node])
             /\ AsyncQueueTyped(asyncDeferredProgressQueues[node])
             /\ AsyncQueueTyped(asyncDeferredNormalQueues[node])
        BY <3>1, EmptyAsyncQueueIsTyped,
           EmptyAsyncCompletionSequenceIsTyped
      <3>3. /\ (\A candidate \in
                       SequenceSet(asyncDeferredProgressQueues[node]):
                     candidate.class = "Progress")
             /\ (\A candidate \in
                       SequenceSet(asyncDeferredNormalQueues[node]):
                     candidate.class = "Normal")
        BY <3>1, EmptyAsyncSequenceSet, SMT
      <3>4. /\ Len(asyncDeferredProgressQueues[node]) <=
                  AsyncDeferredProgressCapacity
             /\ Len(asyncDeferredNormalQueues[node]) <=
                  AsyncDeferredNormalCapacity
        BY <1>1, <3>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncConfiguration
      <3> QED BY <3>2, <3>3, <3>4
    <2> QED BY <2>1 DEF AsyncDeferredContentTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesDeferredType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncDeferredTypeInvariant
BY AsyncInitEstablishesDeferredTopologyType,
   AsyncInitEstablishesDeferredContentType
   DEF AsyncDeferredTypeInvariant

THEOREM SaturatingLinearTimeoutIsPositiveNatural ==
  \A base, maximum, roundView \in Nat:
    /\ base > 0
    /\ maximum > 0
    => (IF base * (roundView + 1) <= maximum
        THEN base * (roundView + 1)
        ELSE maximum) \in Nat \ {0}
BY SMT

THEOREM AsyncViewTimeoutIsPositiveNatural ==
  \A roundView \in Nat:
    AsyncConfiguration => AsyncViewTimeout(roundView) \in Nat \ {0}
BY SaturatingLinearTimeoutIsPositiveNatural, SMT
   DEF AsyncConfiguration, AsyncViewTimeout, AsyncLinearViewTimeout

THEOREM SaturatingLinearTimeoutExceedsRepresentableBound ==
  \A base, maximum, bound \in Nat:
    /\ base > 0
    /\ bound < maximum
    => (IF base * (bound + 1) <= maximum
        THEN base * (bound + 1)
        ELSE maximum) > bound
BY SMT

THEOREM AsyncWorstCaseServiceBudgetIsNatural ==
  /\ ModelConfiguration
  /\ AsyncConfiguration
  => AsyncWorstCaseServiceBudget \in Nat
PROOF
  <1>1. ASSUME ModelConfiguration, AsyncConfiguration
         PROVE AsyncWorstCaseServiceBudget \in Nat
    <2>1. N \in Nat
      BY <1>1, SMT DEF ModelConfiguration, QuorumConfiguration
    <2>2. /\ AsyncQueueCapacity \in Nat
           /\ AsyncProgressReserve \in Nat
           /\ AsyncCompletionReserve \in Nat
           /\ AsyncIngressCapacity \in Nat
           /\ AsyncIoAuxCapacity \in Nat
           /\ AsyncIoWorkCapacity \in Nat
           /\ AsyncDeferredNormalCapacity \in Nat
           /\ AsyncDeferredProgressCapacity \in Nat
           /\ AsyncDeliveryBound \in Nat
           /\ AsyncRetransmitPeriod \in Nat
           /\ AsyncChunkCount \in Nat
      BY <1>1, SMT DEF AsyncConfiguration
    <2>3. /\ AsyncRuntimeCycleBudget \in Nat
           /\ AsyncIoDrainBudget \in Nat
           /\ AsyncDeferredDrainBudget \in Nat
           /\ AsyncRetainedControlBudget \in Nat
           /\ AsyncRetainedProposalChunkBudget \in Nat
           /\ AsyncActiveCertifiedRequestBudget \in Nat
           /\ AsyncActiveCommitRequestBudget \in Nat
      BY <2>1, <2>2, SMT
         DEF AsyncRuntimeCycleBudget, AsyncIoDrainBudget,
             AsyncDeferredDrainBudget, AsyncRetainedControlBudget,
             AsyncRetainedProposalChunkBudget,
             AsyncActiveCertifiedRequestBudget,
             AsyncActiveCommitRequestBudget
    <2>4. /\ AsyncActiveRequestBudget \in Nat
           /\ AsyncRetransmitEmissionBudget \in Nat
      BY <2>3, SMT
         DEF AsyncActiveRequestBudget, AsyncRetransmitEmissionBudget
    <2>5. /\ AsyncOneWayTransportBudget \in Nat
           /\ AsyncProposalPipelineBudget \in Nat
      BY <2>1, <2>2, <2>3, <2>4, SMT
         DEF AsyncOneWayTransportBudget, AsyncProposalPipelineBudget
    <2>6. AsyncCertifiedRecoveryBudget \in Nat
      BY <2>2, <2>3, <2>5, SMT
         DEF AsyncCertifiedRecoveryBudget
    <2> QED BY <2>2, <2>5, <2>6, SMT
         DEF AsyncWorstCaseServiceBudget
  <1> QED BY <1>1

THEOREM AdequateViewTimeoutExists ==
  /\ ModelConfiguration
  /\ AsyncConfiguration
  /\ ViewDomain = Nat
  => \E roundView \in Views:
       /\ roundView <= AsyncMaximumView
       /\ AsyncViewTimeout(roundView) > AsyncWorstCaseServiceBudget
PROOF
  <1>1. ASSUME ModelConfiguration,
                AsyncConfiguration,
                ViewDomain = Nat
         PROVE \E roundView \in Views:
                 /\ roundView <= AsyncMaximumView
                 /\ AsyncViewTimeout(roundView) >
                      AsyncWorstCaseServiceBudget
    <2>1. AsyncWorstCaseServiceBudget \in Nat
      BY <1>1, AsyncWorstCaseServiceBudgetIsNatural
    <2>2. AsyncViewTimeout(AsyncWorstCaseServiceBudget) >
             AsyncWorstCaseServiceBudget
      BY <1>1, <2>1,
         SaturatingLinearTimeoutExceedsRepresentableBound
         DEF AsyncConfiguration, AsyncServiceBoundRepresentable,
             AsyncViewTimeout, AsyncLinearViewTimeout
    <2>3. AsyncWorstCaseServiceBudget \in Views
      BY <1>1, <2>1 DEF Views
    <2>4. AsyncWorstCaseServiceBudget <= AsyncMaximumView
      BY <1>1 DEF AsyncConfiguration, AsyncServiceBoundRepresentable
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesTransportClockType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncTransportClockTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext, AsyncInitAt(initialContext)
         PROVE AsyncTransportClockTypeInvariant
    <2>1. /\ AsyncConfiguration
           /\ nodeView = [node \in ValidatorIds |-> 0]
           /\ asyncOutstandingTags = [node \in ValidatorIds |-> {}]
           /\ asyncNodeDeadlines =
                [node \in ValidatorIds |-> AsyncViewTimeout(nodeView[node])]
           /\ asyncRetransmitDeadlines =
                [node \in ValidatorIds |-> AsyncRetransmitPeriod]
           /\ asyncNodeServiceDeadlines =
                [node \in ValidatorIds |-> AsyncDeliveryBound]
           /\ asyncIoServiceDeadlines =
                [node \in ValidatorIds |-> AsyncDeliveryBound]
      BY <1>1
         DEF AsyncInitAt, AsyncBaseInitAt, InitAt, AsyncTransportInit
    <2>2. /\ AsyncViewTimeout(0) \in Nat
           /\ AsyncRetransmitPeriod \in Nat
           /\ AsyncDeliveryBound \in Nat
      BY <2>1, AsyncViewTimeoutIsPositiveNatural, SMT
         DEF AsyncConfiguration
    <2>3. /\ asyncOutstandingTags \in [ValidatorIds -> SUBSET AsyncCompletionTags]
           /\ asyncNodeDeadlines \in [ValidatorIds -> Nat]
           /\ asyncRetransmitDeadlines \in [ValidatorIds -> Nat]
           /\ asyncNodeServiceDeadlines \in [ValidatorIds -> Nat]
           /\ asyncIoServiceDeadlines \in [ValidatorIds -> Nat]
      BY <2>1, <2>2, Isa
    <2> QED BY <2>3 DEF AsyncTransportClockTypeInvariant
  <1> QED BY <1>1

THEOREM EmptyRetainedClassItems ==
  \A source, controlClass:
    RetainedClassItems({}, source, controlClass) = {}
BY Isa DEF RetainedClassItems

THEOREM AsyncInitEstablishesTransportContentType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncTransportContentTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncTransportContentTypeInvariant
    <2>1. /\ asyncSentItems = {}
           /\ asyncRetainedControl = {}
           /\ asyncActiveRequests = {}
           /\ asyncTransport = {}
           /\ asyncHeldChunks = {}
      BY <1>1 DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit
    <2>2. /\ IsFiniteSet(asyncSentItems)
           /\ IsFiniteSet(asyncRetainedControl)
           /\ IsFiniteSet(asyncActiveRequests)
           /\ IsFiniteSet(asyncTransport)
      BY <2>1, FS_EmptySet, SMT
    <2>3. /\ (\A item \in asyncSentItems: AsyncItemTyped(item))
           /\ (\A item \in asyncRetainedControl:
                 /\ AsyncItemTyped(item)
                 /\ item.kind \in AsyncControlKinds)
           /\ asyncActiveRequests \subseteq asyncSentItems
           /\ (\A item \in asyncActiveRequests:
                 /\ AsyncItemTyped(item)
                 /\ item.kind \in {"CertifiedRequest",
                                     "CommitCertificateRequest"})
           /\ (\A packet \in asyncTransport: AsyncPacketTyped(packet))
           /\ asyncHeldChunks \subseteq AsyncChunkReceiptSet
      BY <2>1, SMT
    <2>4. \A source \in ValidatorIds,
                controlClass \in AsyncControlKinds:
             LET retained ==
                   RetainedClassItems(
                     asyncRetainedControl, source, controlClass)
             IN \/ retained = {}
                \/ /\ Cardinality(retained) <=
                         Cardinality(CurrentVoters)
                   /\ {item.envelope.recipient: item \in retained}
                        = CurrentVoters
                   /\ \A left, right \in retained:
                        ControlView(left) = ControlView(right)
      BY <2>1, EmptyRetainedClassItems
    <2> QED BY <2>2, <2>3, <2>4
      DEF AsyncTransportContentTypeInvariant,
          AsyncTransportHistoryTypeInvariant,
          AsyncPacketContentTypeInvariant,
          AsyncHeldChunksTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesTransportType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncTransportTypeInvariant
BY AsyncInitEstablishesTransportClockType,
   AsyncInitEstablishesTransportContentType
   DEF AsyncTransportTypeInvariant

THEOREM EmptyIngressReadySourceSet ==
  \A sources:
    {source \in sources:
       Len([entry \in sources |-> <<>>][source]) > 0} = {}
BY Isa

THEOREM EmptyIngressZeroLaneSourceSet ==
  \A sources:
    {source \in sources:
       Len([entry \in sources |-> <<>>][source]) = 0} = sources
BY Isa

THEOREM EmptyIngressIndexedPairSet ==
  \A sources, capacity \in Nat:
    {pair \in sources \X (1..capacity): pair[2] <= Len(<<>>)} = {}
PROOF
  <1>1. ASSUME NEW sources, NEW capacity \in Nat
         PROVE {pair \in sources \X (1..capacity):
                  pair[2] <= Len(<<>>)} = {}
    <2>1. {pair \in sources \X (1..capacity):
             pair[2] <= Len(<<>>)} \subseteq {}
      <3>1. ASSUME NEW pair \in
                       {entry \in sources \X (1..capacity):
                          entry[2] <= Len(<<>>)}
             PROVE pair \in {}
        <4>1. /\ pair[2] \in 1..capacity
               /\ pair[2] <= Len(<<>>)
          BY <3>1, Isa
        <4>2. /\ pair[2] >= 1
               /\ Len(<<>>) = 0
          BY <4>1, Isa
        <4> QED BY <4>1, <4>2, SMT
      <3> QED BY <3>1
    <2>2. {} \subseteq
             {pair \in sources \X (1..capacity):
                pair[2] <= Len(<<>>)}
      BY Isa
    <2> QED BY <2>1, <2>2, Isa
  <1> QED BY <1>1

THEOREM AsyncIngressSourcesAreFinite ==
  (AsyncConfiguration /\ ModelConfiguration)
    => IsFiniteSet(AsyncIngressSources)
PROOF
  <1>1. ASSUME AsyncConfiguration /\ ModelConfiguration
         PROVE IsFiniteSet(AsyncIngressSources)
    <2>1. /\ 0 \in Int
           /\ N - 1 \in Int
      BY <1>1, SMT
         DEF AsyncConfiguration, ModelConfiguration, QuorumConfiguration
    <2>2. IsFiniteSet(ValidatorIds)
      BY <2>1, FS_Interval DEF ValidatorIds
    <2>3. IsFiniteSet(ValidatorIds \cup {AsyncUntrustedSource})
      BY <2>2, FS_AddElement
    <2> QED BY <2>3 DEF AsyncIngressSources
  <1> QED BY <1>1

THEOREM AsyncIngressSourceCardinalityIsNatural ==
  (AsyncConfiguration /\ ModelConfiguration)
    => Cardinality(AsyncIngressSources) \in Nat
BY AsyncIngressSourcesAreFinite, FS_CardinalityType

THEOREM AsyncInitEstablishesIngressTopologyType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncIngressTopologyTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncIngressTopologyTypeInvariant
    <2>1. /\ DOMAIN asyncIngressLanes = ValidatorIds
           /\ DOMAIN asyncIngressReady = ValidatorIds
      BY <1>1, SMT
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncIngressInit
    <2>2. ASSUME NEW recipient \in ValidatorIds
           PROVE /\ DOMAIN asyncIngressLanes[recipient] =
                        AsyncIngressSources
                 /\ DOMAIN asyncIngressReady[recipient] =
                      1..Len(asyncIngressReady[recipient])
                 /\ SequenceSet(asyncIngressReady[recipient])
                      \subseteq AsyncIngressSources
                 /\ Len(asyncIngressReady[recipient]) =
                      Cardinality(
                        SequenceSet(asyncIngressReady[recipient]))
                 /\ SequenceSet(asyncIngressReady[recipient]) =
                      {source \in AsyncIngressSources:
                         IngressLaneDepth(recipient, source) > 0}
      <3>1. /\ asyncIngressLanes[recipient] =
                    [source \in AsyncIngressSources |-> <<>>]
             /\ asyncIngressReady[recipient] = <<>>
        BY <1>1, <2>2, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncIngressInit
      <3>2. /\ DOMAIN asyncIngressLanes[recipient] =
                    AsyncIngressSources
             /\ DOMAIN asyncIngressReady[recipient] =
                  1..Len(asyncIngressReady[recipient])
             /\ asyncIngressReady[recipient]
                  \in Seq(Range(asyncIngressReady[recipient]))
        BY <3>1, EmptyAsyncQueueIsTyped, SMT DEF AsyncQueueTyped
      <3>3. /\ SequenceSet(asyncIngressReady[recipient])
                    \subseteq AsyncIngressSources
             /\ Len(asyncIngressReady[recipient]) =
                  Cardinality(
                    SequenceSet(asyncIngressReady[recipient]))
        BY <3>1, EmptyAsyncSequenceSet,
           EmptyAsyncSequenceLengthMatchesCardinality, SMT
      <3>4. SequenceSet(asyncIngressReady[recipient]) =
               {source \in AsyncIngressSources:
                  IngressLaneDepth(recipient, source) > 0}
        BY <3>1, EmptyAsyncSequenceSet,
           EmptyIngressReadySourceSet, SMT
           DEF IngressLaneDepth, IngressLane
      <3> QED BY <3>2, <3>3, <3>4
    <2> QED BY <2>1, <2>2 DEF AsyncIngressTopologyTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesIngressCapacityType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncIngressCapacityTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncIngressCapacityTypeInvariant
    <2>1. ASSUME NEW recipient \in ValidatorIds
           PROVE /\ IngressDepth(recipient) <= AsyncIngressCapacity
                 /\ IngressDepth(recipient)
                      + Cardinality(
                          {source \in AsyncIngressSources:
                             IngressLaneDepth(recipient, source) = 0})
                      <= AsyncIngressCapacity
      <3>1. /\ asyncIngressLanes[recipient] =
                    [source \in AsyncIngressSources |-> <<>>]
             /\ AsyncIngressCapacity \in Nat
             /\ AsyncIngressCapacity >=
                  Cardinality(AsyncIngressSources)
        BY <1>1, <2>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncIngressInit,
               AsyncConfiguration
      <3>2. {pair \in AsyncIngressSources \X
                      (1..AsyncIngressCapacity):
                 pair[2] <= IngressLaneDepth(recipient, pair[1])} = {}
        BY <3>1, EmptyIngressIndexedPairSet, SMT
           DEF IngressLaneDepth, IngressLane
      <3>3. IngressDepth(recipient) = 0
        BY <3>2, FS_EmptySet, SMT DEF IngressDepth
      <3>4. {source \in AsyncIngressSources:
                 IngressLaneDepth(recipient, source) = 0}
               = AsyncIngressSources
        BY <3>1, EmptyIngressZeroLaneSourceSet, SMT
           DEF IngressLaneDepth, IngressLane
      <3>5. AsyncConfiguration /\ ModelConfiguration
        BY <1>1 DEF AsyncInitAt, AsyncBaseInitAt, InitAt
      <3>6. Cardinality(AsyncIngressSources) \in Nat
        BY <3>5, AsyncIngressSourceCardinalityIsNatural
      <3>7. IngressDepth(recipient) <= AsyncIngressCapacity
        BY <3>1, <3>3, SMT
      <3>8. IngressDepth(recipient)
                + Cardinality(
                    {source \in AsyncIngressSources:
                       IngressLaneDepth(recipient, source) = 0})
              = Cardinality(AsyncIngressSources)
        BY <3>3, <3>4, <3>6, SMT
      <3>9. IngressDepth(recipient)
                + Cardinality(
                    {source \in AsyncIngressSources:
                       IngressLaneDepth(recipient, source) = 0})
              <= AsyncIngressCapacity
        BY <3>1, <3>8, SMT
      <3> QED BY <3>7, <3>9
    <2> QED BY <2>1 DEF AsyncIngressCapacityTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesIngressContentType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncIngressContentTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncIngressContentTypeInvariant
    <2>1. ASSUME NEW recipient \in ValidatorIds,
                  NEW source \in AsyncIngressSources
           PROVE /\ DOMAIN IngressLane(recipient, source) =
                        1..IngressLaneDepth(recipient, source)
                 /\ \A index \in
                        1..IngressLaneDepth(recipient, source):
                      AsyncItemTyped(
                        IngressLane(recipient, source)[index])
      <3>1. IngressLane(recipient, source) = <<>>
        BY <1>1, <2>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncIngressInit, IngressLane
      <3>2. DOMAIN IngressLane(recipient, source) =
               1..IngressLaneDepth(recipient, source)
             /\ IngressLane(recipient, source)
                  \in Seq(Range(IngressLane(recipient, source)))
        BY <3>1, EmptyAsyncQueueIsTyped
           DEF AsyncQueueTyped, IngressLaneDepth
      <3>3. \A index \in
                    1..IngressLaneDepth(recipient, source):
                 AsyncItemTyped(IngressLane(recipient, source)[index])
        BY <3>1, Isa DEF IngressLaneDepth
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>1 DEF AsyncIngressContentTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesIngressType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncIngressTypeInvariant
BY AsyncInitEstablishesIngressTopologyType,
   AsyncInitEstablishesIngressCapacityType,
   AsyncInitEstablishesIngressContentType
   DEF AsyncIngressTypeInvariant

THEOREM AsyncInitEstablishesSchedulerType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncSchedulerTypeInvariant
BY AsyncInitEstablishesRuntimeType, AsyncInitEstablishesIoType,
   AsyncInitEstablishesDeferredType, AsyncInitEstablishesTransportType,
   AsyncInitEstablishesIngressType
   DEF AsyncSchedulerTypeInvariant

(***************************************************************************
Scheduler type preservation.  The first boundary records precisely which
Core state the scheduler types can observe: the frozen context determines the
current voting roster, while every other free state variable is contained in
`AsyncSchedulerVars`.  The primitive action proofs below then unfold only the
slice changed by that action.
***************************************************************************)

AsyncRuntimeScalarTypeVars ==
  <<asyncNow, asyncCommandQueues, asyncFifoOwed, asyncTimeoutEmitted,
    asyncRunnerPhase, asyncRunnerBudget>>

AsyncIoTopologyTypeVars ==
  <<asyncIoQueues, asyncOutstandingWork, asyncIoReadyCompletions,
    asyncLocalReadyCompletions, asyncNextCompletionSource,
    asyncIoControlAvailable>>

AsyncIoContentTypeVars ==
  <<asyncCommandQueues, AsyncIoTopologyTypeVars>>

AsyncIoQueueContentTypeVars ==
  <<asyncIoQueues, asyncOutstandingWork,
    asyncIoReadyCompletions, asyncLocalReadyCompletions>>

AsyncIoWorkContentTypeVars ==
  <<asyncCommandQueues, asyncOutstandingWork,
    asyncIoReadyCompletions, asyncLocalReadyCompletions>>

AsyncIoCapacityTypeVars ==
  <<asyncCommandQueues, asyncIoQueues, asyncOutstandingWork,
    asyncDeferredCompletionQueues>>

AsyncDeferredTopologyTypeVars ==
  <<asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncDeferredDrainOwed>>

AsyncTransportClockTypeVars ==
  <<asyncOutstandingTags, asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncNodeServiceDeadlines, asyncIoServiceDeadlines>>

AsyncTransportContentTypeVars ==
  <<context, asyncSentItems, asyncRetainedControl, asyncActiveRequests,
    asyncTransport, asyncHeldChunks>>

AsyncTransportHistoryTypeVars ==
  <<context, asyncSentItems, asyncRetainedControl, asyncActiveRequests>>

AsyncIngressTopologyTypeVars == <<asyncIngressLanes, asyncIngressReady>>

THEOREM AsyncRuntimeScalarTypeStutter ==
  AsyncRuntimeScalarTypeInvariant /\ UNCHANGED AsyncRuntimeScalarTypeVars
    => AsyncRuntimeScalarTypeInvariant'
BY Isa
   DEF AsyncRuntimeScalarTypeInvariant, AsyncRuntimeScalarTypeVars

THEOREM AsyncCausalTypeStutter ==
  AsyncCausalTypeInvariant /\ UNCHANGED asyncCausalQueues
    => AsyncCausalTypeInvariant'
BY Isa DEF AsyncCausalTypeInvariant

THEOREM AsyncIoTopologyTypeStutter ==
  AsyncIoTopologyTypeInvariant /\ UNCHANGED AsyncIoTopologyTypeVars
    => AsyncIoTopologyTypeInvariant'
BY Isa DEF AsyncIoTopologyTypeInvariant, AsyncIoTopologyTypeVars

THEOREM AsyncIoContentTypeStutter ==
  AsyncIoContentTypeInvariant /\ UNCHANGED AsyncIoContentTypeVars
    => AsyncIoContentTypeInvariant'
BY Isa DEF AsyncIoContentTypeInvariant, AsyncIoContentTypeVars,
           AsyncIoTopologyTypeVars,
           AsyncIoQueueContentTypeInvariant,
           AsyncIoWorkContentTypeInvariant,
           AsyncIoConsensusCandidateOwnership,
           AsyncIoConsensusQueueOwnership,
           AsyncIoConsensusIndices

THEOREM AsyncIoQueueContentTypeStutter ==
  AsyncIoQueueContentTypeInvariant
    /\ UNCHANGED AsyncIoQueueContentTypeVars
    => AsyncIoQueueContentTypeInvariant'
BY Isa
   DEF AsyncIoQueueContentTypeInvariant, AsyncIoQueueContentTypeVars,
       AsyncIoConsensusCandidateOwnership,
       AsyncIoConsensusQueueOwnership, AsyncIoConsensusIndices

THEOREM AsyncIoWorkContentTypeStutter ==
  AsyncIoWorkContentTypeInvariant
    /\ UNCHANGED AsyncIoWorkContentTypeVars
    => AsyncIoWorkContentTypeInvariant'
BY Isa
   DEF AsyncIoWorkContentTypeInvariant, AsyncIoWorkContentTypeVars

THEOREM AsyncIoCapacityTypeStutter ==
  AsyncIoCapacityTypeInvariant /\ UNCHANGED AsyncIoCapacityTypeVars
    => AsyncIoCapacityTypeInvariant'
BY Isa
   DEF AsyncIoCapacityTypeInvariant, AsyncIoCapacityTypeVars,
       AsyncQueueDepth, AsyncIoQueueDepth, AsyncCompletionLoad,
       AsyncOutstandingWorkCount, QueuedCompletionCount,
       QueuedCompletionIndices, DeferredCompletionCount

THEOREM AsyncDeferredTopologyTypeStutter ==
  AsyncDeferredTopologyTypeInvariant
    /\ UNCHANGED AsyncDeferredTopologyTypeVars
    => AsyncDeferredTopologyTypeInvariant'
BY Isa
   DEF AsyncDeferredTopologyTypeInvariant, AsyncDeferredTopologyTypeVars

THEOREM AsyncDeferredContentTypeStutter ==
  AsyncDeferredContentTypeInvariant
    /\ UNCHANGED <<asyncDeferredCompletionQueues,
                   asyncDeferredProgressQueues,
                   asyncDeferredNormalQueues>>
    => AsyncDeferredContentTypeInvariant'
BY Isa DEF AsyncDeferredContentTypeInvariant

THEOREM AsyncTransportClockTypeStutter ==
  AsyncTransportClockTypeInvariant
    /\ UNCHANGED AsyncTransportClockTypeVars
    => AsyncTransportClockTypeInvariant'
BY Isa DEF AsyncTransportClockTypeInvariant, AsyncTransportClockTypeVars

THEOREM AsyncTransportContentTypeStutter ==
  AsyncTransportContentTypeInvariant
    /\ UNCHANGED AsyncTransportContentTypeVars
    => AsyncTransportContentTypeInvariant'
BY Isa
   DEF AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       AsyncPacketContentTypeInvariant, AsyncHeldChunksTypeInvariant,
       AsyncTransportContentTypeVars, CurrentVoters, CurrentEpoch

THEOREM AsyncTransportHistoryTypeStutter ==
  AsyncTransportHistoryTypeInvariant
    /\ UNCHANGED AsyncTransportHistoryTypeVars
    => AsyncTransportHistoryTypeInvariant'
BY Isa
   DEF AsyncTransportHistoryTypeInvariant,
       AsyncTransportHistoryTypeVars, CurrentVoters, CurrentEpoch

THEOREM AsyncHeldChunksTypeStutter ==
  AsyncHeldChunksTypeInvariant /\ UNCHANGED asyncHeldChunks
    => AsyncHeldChunksTypeInvariant'
BY Isa DEF AsyncHeldChunksTypeInvariant

THEOREM AsyncIngressTopologyTypeStutter ==
  AsyncIngressTopologyTypeInvariant
    /\ UNCHANGED AsyncIngressTopologyTypeVars
    => AsyncIngressTopologyTypeInvariant'
BY Isa
   DEF AsyncIngressTopologyTypeInvariant, AsyncIngressTopologyTypeVars,
       SequenceSet, IngressLaneDepth, IngressLane

THEOREM AsyncIngressCapacityTypeStutter ==
  AsyncIngressCapacityTypeInvariant /\ UNCHANGED asyncIngressLanes
    => AsyncIngressCapacityTypeInvariant'
BY Isa
   DEF AsyncIngressCapacityTypeInvariant, IngressDepth,
       IngressLaneDepth, IngressLane

THEOREM AsyncIngressContentTypeStutter ==
  AsyncIngressContentTypeInvariant /\ UNCHANGED asyncIngressLanes
    => AsyncIngressContentTypeInvariant'
BY Isa
   DEF AsyncIngressContentTypeInvariant, IngressLaneDepth, IngressLane

THEOREM AsyncSchedulerStateStutterPreservesType ==
  AsyncSchedulerTypeInvariant
    /\ UNCHANGED <<context, AsyncSchedulerVars>>
    => AsyncSchedulerTypeInvariant'
BY AsyncRuntimeScalarTypeStutter, AsyncCausalTypeStutter,
   AsyncIoTopologyTypeStutter, AsyncIoContentTypeStutter,
   AsyncIoCapacityTypeStutter, AsyncDeferredTopologyTypeStutter,
   AsyncDeferredContentTypeStutter, AsyncTransportClockTypeStutter,
   AsyncTransportContentTypeStutter, AsyncIngressTopologyTypeStutter,
   AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter, Isa
   DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
       AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
       AsyncTransportTypeInvariant, AsyncIngressTypeInvariant,
       AsyncRuntimeScalarTypeVars, AsyncIoTopologyTypeVars,
       AsyncIoContentTypeVars, AsyncIoCapacityTypeVars,
       AsyncDeferredTopologyTypeVars, AsyncTransportClockTypeVars,
       AsyncTransportContentTypeVars, AsyncIngressTopologyTypeVars,
       AsyncSchedulerVars

THEOREM AsyncAllVarsStutterPreservesSchedulerType ==
  AsyncSchedulerTypeInvariant /\ UNCHANGED AsyncAllVars
    => AsyncSchedulerTypeInvariant'
BY AsyncSchedulerStateStutterPreservesType, Isa
   DEF AsyncAllVars, AsyncSchedulerVars, vars

THEOREM AsyncSetGstPreservesSchedulerType ==
  AsyncSchedulerTypeInvariant /\ AsyncSetGST
    => AsyncSchedulerTypeInvariant'
BY AsyncSchedulerStateStutterPreservesType, Isa
   DEF AsyncSetGST, SetGST, AsyncSchedulerVars, vars

THEOREM AsyncTickPreservesSchedulerType ==
  AsyncSchedulerTypeInvariant /\ AsyncTick
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME AsyncSchedulerTypeInvariant, AsyncTick
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. AsyncRuntimeScalarTypeInvariant
      BY <1>1 DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant
    <2>2. AsyncTransportClockTypeInvariant
      BY <1>1 DEF AsyncSchedulerTypeInvariant, AsyncTransportTypeInvariant
    <2>3. /\ asyncNow' = asyncNow + 1
           /\ UNCHANGED <<asyncCommandQueues, asyncFifoOwed,
                          asyncTimeoutEmitted, asyncRunnerPhase,
                          asyncRunnerBudget>>
      BY <1>1, Isa DEF AsyncTick, AsyncNonClockVars, vars
    <2>4. AsyncRuntimeScalarTypeInvariant'
      BY <2>1, <2>3, SMT DEF AsyncRuntimeScalarTypeInvariant
    <2>5. UNCHANGED AsyncTransportClockTypeVars
      BY <1>1, Isa
         DEF AsyncTick, AsyncNonClockVars, AsyncTransportClockTypeVars
    <2>6. AsyncTransportClockTypeInvariant'
      BY <2>2, <2>5, AsyncTransportClockTypeStutter
    <2>7. /\ AsyncCausalTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
           /\ AsyncDeferredTopologyTypeInvariant
           /\ AsyncDeferredContentTypeInvariant
           /\ AsyncTransportContentTypeInvariant
           /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressCapacityTypeInvariant
           /\ AsyncIngressContentTypeInvariant
      BY <1>1
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
    <2>8. UNCHANGED AsyncNonClockVars
      BY <1>1 DEF AsyncTick
    <2>9. /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncIoTopologyTypeVars
           /\ UNCHANGED AsyncIoContentTypeVars
           /\ UNCHANGED AsyncIoCapacityTypeVars
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportContentTypeVars
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <2>8, Isa
         DEF AsyncNonClockVars, AsyncIoVars, AsyncIoTopologyTypeVars,
             AsyncIoContentTypeVars, AsyncIoCapacityTypeVars,
             AsyncDeferredTopologyTypeVars,
             AsyncTransportContentTypeVars,
             AsyncIngressTopologyTypeVars, vars
    <2>10. /\ AsyncCausalTypeInvariant'
           /\ AsyncIoTopologyTypeInvariant'
           /\ AsyncIoContentTypeInvariant'
           /\ AsyncIoCapacityTypeInvariant'
           /\ AsyncDeferredTopologyTypeInvariant'
           /\ AsyncDeferredContentTypeInvariant'
           /\ AsyncTransportContentTypeInvariant'
           /\ AsyncIngressTopologyTypeInvariant'
           /\ AsyncIngressCapacityTypeInvariant'
           /\ AsyncIngressContentTypeInvariant'
      BY <2>7, <2>9, AsyncCausalTypeStutter,
         AsyncIoTopologyTypeStutter, AsyncIoContentTypeStutter,
         AsyncIoCapacityTypeStutter, AsyncDeferredTopologyTypeStutter,
         AsyncDeferredContentTypeStutter,
         AsyncTransportContentTypeStutter,
         AsyncIngressTopologyTypeStutter,
         AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter
    <2> QED BY <2>4, <2>6, <2>10
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM PreGstPacketLossPreservesSchedulerType ==
  \A packet \in asyncTransport:
    AsyncSchedulerTypeInvariant /\ PreGstLosePacket(packet)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW packet \in asyncTransport,
                AsyncSchedulerTypeInvariant,
                PreGstLosePacket(packet)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. asyncTransport' \subseteq asyncTransport
      BY <1>1, Isa DEF PreGstLosePacket
    <2>2. AsyncPacketContentTypeInvariant
      BY <1>1
         DEF AsyncSchedulerTypeInvariant, AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant
    <2>3. IsFiniteSet(asyncTransport')
      BY <2>1, <2>2, FS_Subset DEF AsyncPacketContentTypeInvariant
    <2>4. \A remaining \in asyncTransport': AsyncPacketTyped(remaining)
      BY <2>1, <2>2, SMT DEF AsyncPacketContentTypeInvariant
    <2>5. AsyncPacketContentTypeInvariant'
      BY <2>3, <2>4 DEF AsyncPacketContentTypeInvariant
    <2>6. /\ AsyncTransportHistoryTypeInvariant
           /\ AsyncHeldChunksTypeInvariant
      BY <1>1
         DEF AsyncSchedulerTypeInvariant, AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant
    <2>7. /\ UNCHANGED AsyncTransportHistoryTypeVars
           /\ UNCHANGED asyncHeldChunks
      BY <1>1, Isa
         DEF PreGstLosePacket, AsyncTransportHistoryTypeVars,
             AsyncSchedulerVars, vars
    <2>8. /\ AsyncTransportHistoryTypeInvariant'
           /\ AsyncHeldChunksTypeInvariant'
      BY <2>6, <2>7, AsyncTransportHistoryTypeStutter,
         AsyncHeldChunksTypeStutter
    <2>9. AsyncTransportContentTypeInvariant'
      BY <2>5, <2>8 DEF AsyncTransportContentTypeInvariant
    <2>10. /\ AsyncRuntimeScalarTypeInvariant
            /\ AsyncCausalTypeInvariant
            /\ AsyncIoTopologyTypeInvariant
            /\ AsyncIoContentTypeInvariant
            /\ AsyncIoCapacityTypeInvariant
            /\ AsyncDeferredTopologyTypeInvariant
            /\ AsyncDeferredContentTypeInvariant
            /\ AsyncTransportClockTypeInvariant
            /\ AsyncIngressTopologyTypeInvariant
            /\ AsyncIngressCapacityTypeInvariant
            /\ AsyncIngressContentTypeInvariant
      BY <1>1
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
    <2>11. UNCHANGED AsyncDeferredVars
      BY <1>1 DEF PreGstLosePacket
    <2>12. UNCHANGED asyncCausalQueues
      BY <1>1 DEF PreGstLosePacket, LeaveCausalQueues
    <2>13. UNCHANGED
              <<vars, asyncNow, asyncCommandQueues, asyncFifoOwed,
                asyncTimeoutEmitted, asyncRunnerPhase, asyncRunnerBudget,
                AsyncIoVars, asyncOutstandingTags, asyncNodeDeadlines,
                asyncRetransmitDeadlines, asyncNodeServiceDeadlines,
                asyncIoServiceDeadlines, asyncSentItems,
                asyncRetainedControl, asyncActiveRequests,
                asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>
      BY <1>1 DEF PreGstLosePacket
    <2>14. /\ UNCHANGED AsyncRuntimeScalarTypeVars
            /\ UNCHANGED asyncCausalQueues
            /\ UNCHANGED AsyncIoTopologyTypeVars
            /\ UNCHANGED AsyncIoContentTypeVars
            /\ UNCHANGED AsyncIoCapacityTypeVars
            /\ UNCHANGED AsyncDeferredTopologyTypeVars
            /\ UNCHANGED <<asyncDeferredCompletionQueues,
                           asyncDeferredProgressQueues,
                           asyncDeferredNormalQueues>>
            /\ UNCHANGED AsyncTransportClockTypeVars
            /\ UNCHANGED AsyncIngressTopologyTypeVars
            /\ UNCHANGED asyncIngressLanes
      BY <2>11, <2>12, <2>13, Isa
         DEF AsyncRuntimeScalarTypeVars,
             AsyncIoVars, AsyncDeferredVars, AsyncIoTopologyTypeVars,
             AsyncIoContentTypeVars, AsyncIoCapacityTypeVars,
             AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2>15. /\ AsyncRuntimeScalarTypeInvariant'
            /\ AsyncCausalTypeInvariant'
            /\ AsyncIoTopologyTypeInvariant'
            /\ AsyncIoContentTypeInvariant'
            /\ AsyncIoCapacityTypeInvariant'
            /\ AsyncDeferredTopologyTypeInvariant'
            /\ AsyncDeferredContentTypeInvariant'
            /\ AsyncTransportClockTypeInvariant'
            /\ AsyncIngressTopologyTypeInvariant'
            /\ AsyncIngressCapacityTypeInvariant'
            /\ AsyncIngressContentTypeInvariant'
      BY <2>10, <2>14, AsyncRuntimeScalarTypeStutter,
         AsyncCausalTypeStutter, AsyncIoTopologyTypeStutter,
         AsyncIoContentTypeStutter, AsyncIoCapacityTypeStutter,
         AsyncDeferredTopologyTypeStutter,
         AsyncDeferredContentTypeStutter, AsyncTransportClockTypeStutter,
         AsyncIngressTopologyTypeStutter,
         AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter
    <2> QED BY <2>9, <2>15
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM PreGstCrashPreservesSchedulerType ==
  \A node \in ValidatorIds:
    AsyncSchedulerTypeInvariant /\ PreGstCrash(node)
      => AsyncSchedulerTypeInvariant'
BY AsyncSchedulerStateStutterPreservesType, Isa
   DEF PreGstCrash, Crash, AsyncSchedulerVars, vars

THEOREM AsyncControlIoJobIsTyped ==
  AsyncConfiguration => AsyncIoJobTyped(AsyncIoControlJob)
BY SMT
   DEF AsyncConfiguration, AsyncIoJobTyped, AsyncIoControlJob,
       AsyncIoJob, AsyncIoCommandClasses

THEOREM AsyncCurrentResponsiveVotersAreValidators ==
  TypeInvariant => AsyncCurrentResponsiveVoters \subseteq ValidatorIds
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE AsyncCurrentResponsiveVoters \subseteq ValidatorIds
    <2>1. /\ ModelConfiguration
           /\ context \in ContextRecords
      BY <1>1 DEF TypeInvariant
    <2>2. PICK blockHeight \in Heights:
             \E lineage \in LineagesAt(blockHeight):
               context = ContextRecord(blockHeight, lineage)
      BY <2>1, Isa DEF ContextRecords
    <2>3. PICK lineage \in LineagesAt(blockHeight):
             context = ContextRecord(blockHeight, lineage)
      BY <2>2
    <2>4. /\ context.height = blockHeight
          /\ context.epoch = ExpectedEpoch(blockHeight)
      BY <2>3 DEF ContextRecord
    <2>5. MaxEpoch >= ExpectedEpoch(MaxHeight)
      BY <2>1 DEF ModelConfiguration
    <2>6. /\ blockHeight \in Nat
          /\ MaxHeight \in Nat
          /\ EpochLength \in Nat
          /\ EpochLength > 0
          /\ MaxEpoch \in Nat
          /\ blockHeight <= MaxHeight
      BY <2>1, <2>2, SMT
         DEF ModelConfiguration, QuorumConfiguration, Heights
    <2>7. ExpectedEpoch(blockHeight) \in 0..MaxEpoch
      BY <2>5, <2>6, BoundedNaturalQuotient
         DEF ExpectedEpoch
    <2>8. CurrentEpoch \in Epochs
      BY <2>4, <2>7 DEF CurrentEpoch, Epochs
    <2>9. VotingRoster(CurrentEpoch) \subseteq ValidatorIds
      BY <2>1, <2>8
         DEF ModelConfiguration, QuorumConfiguration, VotingRoster
    <2> QED BY <2>9, Isa
         DEF AsyncCurrentResponsiveVoters, CurrentVoters
  <1> QED BY <1>1

THEOREM AppendSequenceFacts ==
  \A sequence, value:
    sequence \in Seq(Range(sequence))
      => /\ Append(sequence, value)
               \in Seq(Range(sequence) \cup {value})
         /\ DOMAIN Append(sequence, value) =
               1..(Len(sequence) + 1)
         /\ Len(Append(sequence, value)) = Len(sequence) + 1
         /\ (\A index \in 1..Len(sequence):
               Append(sequence, value)[index] = sequence[index])
         /\ Append(sequence, value)[Len(sequence) + 1] = value
         /\ Range(Append(sequence, value)) =
              Range(sequence) \cup {value}
PROOF
  <1>1. ASSUME NEW sequence, NEW value,
                sequence \in Seq(Range(sequence))
         PROVE /\ Append(sequence, value)
                      \in Seq(Range(sequence) \cup {value})
                /\ DOMAIN Append(sequence, value) =
                      1..(Len(sequence) + 1)
                /\ Len(Append(sequence, value)) = Len(sequence) + 1
                /\ (\A index \in 1..Len(sequence):
                      Append(sequence, value)[index] = sequence[index])
                /\ Append(sequence, value)[Len(sequence) + 1] = value
                /\ Range(Append(sequence, value)) =
                     Range(sequence) \cup {value}
    <2>1. Range(sequence) \subseteq Range(sequence) \cup {value}
      BY Isa
    <2>2. sequence \in Seq(Range(sequence) \cup {value})
      BY <1>1, <2>1, SeqMonotonic
    <2>3. value \in Range(sequence) \cup {value}
      BY Isa
    <2>4. /\ Append(sequence, value)
                  \in Seq(Range(sequence) \cup {value})
           /\ Len(Append(sequence, value)) = Len(sequence) + 1
           /\ (\A index \in 1..Len(sequence):
                 Append(sequence, value)[index] = sequence[index])
           /\ Append(sequence, value)[Len(sequence) + 1] = value
           /\ Range(Append(sequence, value)) =
                Range(sequence) \cup {value}
      BY <2>2, <2>3, AppendProperties
    <2>5. DOMAIN Append(sequence, value) =
             1..Len(Append(sequence, value))
      BY <2>4, LenProperties
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM FunctionalAppendUpdateAtKey ==
  \A mapping, key, value:
    key \in DOMAIN mapping
      => [mapping EXCEPT ![key] = Append(@, value)][key]
           = Append(mapping[key], value)
BY Isa

THEOREM FunctionalUpdateAwayFromKey ==
  \A mapping, key, value, other:
    other \in DOMAIN mapping /\ other # key
      => [mapping EXCEPT ![key] = value][other] = mapping[other]
BY Isa

THEOREM FunctionalAppendUpdateAwayFromKey ==
  \A mapping, key, value, other:
    other \in DOMAIN mapping /\ other # key
      => [mapping EXCEPT ![key] = Append(@, value)][other]
           = mapping[other]
BY Isa

THEOREM FunctionalTailUpdateAtKey ==
  \A mapping, key:
    key \in DOMAIN mapping
      => [mapping EXCEPT ![key] = Tail(@)][key] = Tail(mapping[key])
BY Isa

THEOREM FunctionalTailUpdateAwayFromKey ==
  \A mapping, key, other:
    other \in DOMAIN mapping /\ other # key
      => [mapping EXCEPT ![key] = Tail(@)][other] = mapping[other]
BY Isa

THEOREM TypedIoAppendPreservesSequenceType ==
  \A queue, job:
    AsyncIoSequenceTyped(queue) /\ AsyncIoJobTyped(job)
      => AsyncIoSequenceTyped(Append(queue, job))
PROOF
  <1>1. ASSUME NEW queue, NEW job,
                AsyncIoSequenceTyped(queue), AsyncIoJobTyped(job)
         PROVE AsyncIoSequenceTyped(Append(queue, job))
    <2>1. DOMAIN queue = 1..Len(queue)
      BY <1>1 DEF AsyncIoSequenceTyped
    <2>2. queue \in Seq(Range(queue))
      BY <1>1 DEF AsyncIoSequenceTyped
    <2>3. /\ Append(queue, job)
                   \in Seq(Range(queue) \cup {job})
           /\ DOMAIN Append(queue, job) = 1..(Len(queue) + 1)
           /\ Len(Append(queue, job)) = Len(queue) + 1
           /\ Range(Append(queue, job)) = Range(queue) \cup {job}
      BY <2>2, AppendSequenceFacts
    <2>4. Len(queue) \in Nat
      BY <2>2, LenProperties
    <2>5. Append(queue, job)
             \in Seq(Range(Append(queue, job)))
      BY <2>3, Isa
    <2>6. \A index \in 1..Len(Append(queue, job)):
             AsyncIoJobTyped(Append(queue, job)[index])
      <3>1. ASSUME NEW index \in 1..Len(Append(queue, job))
             PROVE AsyncIoJobTyped(Append(queue, job)[index])
        <4>1. CASE index \in 1..Len(queue)
          BY <1>1, <2>2, <4>1, AppendSequenceFacts
             DEF AsyncIoSequenceTyped
        <4>2. CASE index \notin 1..Len(queue)
          <5>1. Len(Append(queue, job)) = Len(queue) + 1
            BY <2>3
          <5>2. index = Len(queue) + 1
            BY <2>4, <3>1, <4>2, <5>1, SMT
          <5> QED BY <1>1, <2>2, <5>2, AppendSequenceFacts
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>3, <2>5, <2>6 DEF AsyncIoSequenceTyped
  <1> QED BY <1>1

THEOREM TypedCompletionAppendPreservesSequenceType ==
  \A queue, candidate:
    AsyncCompletionSequenceTyped(queue)
      /\ AsyncCandidateTyped(candidate)
      /\ candidate.class = "Completion"
      => AsyncCompletionSequenceTyped(Append(queue, candidate))
PROOF
  <1>1. ASSUME NEW queue, NEW candidate,
                AsyncCompletionSequenceTyped(queue),
                AsyncCandidateTyped(candidate),
                candidate.class = "Completion"
         PROVE AsyncCompletionSequenceTyped(Append(queue, candidate))
    <2>1. queue \in Seq(Range(queue))
      BY <1>1 DEF AsyncCompletionSequenceTyped
    <2>2. /\ Append(queue, candidate)
                   \in Seq(Range(queue) \cup {candidate})
           /\ DOMAIN Append(queue, candidate) = 1..(Len(queue) + 1)
           /\ Len(Append(queue, candidate)) = Len(queue) + 1
           /\ (\A index \in 1..Len(queue):
                 Append(queue, candidate)[index] = queue[index])
           /\ Append(queue, candidate)[Len(queue) + 1] = candidate
           /\ Range(Append(queue, candidate)) =
                Range(queue) \cup {candidate}
      BY <2>1, AppendSequenceFacts
    <2>3. /\ Append(queue, candidate)
                   \in Seq(Range(Append(queue, candidate)))
           /\ DOMAIN Append(queue, candidate) =
                1..Len(Append(queue, candidate))
      BY <2>2, Isa
    <2>4. Len(queue) \in Nat
      BY <2>1, LenProperties
    <2>5. \A index \in 1..Len(Append(queue, candidate)):
             /\ AsyncCandidateTyped(Append(queue, candidate)[index])
             /\ Append(queue, candidate)[index].class = "Completion"
      <3>1. ASSUME NEW index \in 1..Len(Append(queue, candidate))
             PROVE /\ AsyncCandidateTyped(
                          Append(queue, candidate)[index])
                   /\ Append(queue, candidate)[index].class = "Completion"
        <4>1. CASE index \in 1..Len(queue)
          BY <1>1, <2>2, <4>1
             DEF AsyncCompletionSequenceTyped
        <4>2. CASE index \notin 1..Len(queue)
          <5>1. index = Len(queue) + 1
            BY <2>2, <2>4, <3>1, <4>2, SMT
          <5> QED BY <1>1, <2>2, <5>1
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>3, <2>5 DEF AsyncCompletionSequenceTyped
  <1> QED BY <1>1

THEOREM SequenceSetAfterAppend ==
  \A sequence, value:
    sequence \in Seq(Range(sequence))
      => SequenceSet(Append(sequence, value))
           = SequenceSet(sequence) \cup {value}
PROOF
  <1>1. ASSUME NEW sequence, NEW value,
                sequence \in Seq(Range(sequence))
         PROVE SequenceSet(Append(sequence, value)) =
                 SequenceSet(sequence) \cup {value}
    <2>1. /\ Append(sequence, value)
                   \in Seq(Range(sequence) \cup {value})
           /\ Range(Append(sequence, value)) =
                Range(sequence) \cup {value}
      BY <1>1, AppendSequenceFacts
    <2>2. SequenceSet(sequence) = Range(sequence)
      BY <1>1, RangeEquality DEF SequenceSet
    <2>3. SequenceSet(Append(sequence, value)) =
             Range(Append(sequence, value))
      BY <2>1, RangeEquality DEF SequenceSet
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM UniqueSequenceLengthImpliesInjective ==
  \A sequence:
    /\ sequence \in Seq(Range(sequence))
    /\ Len(sequence) = Cardinality(SequenceSet(sequence))
    => IsInjective(sequence)
PROOF
  <1>1. ASSUME NEW sequence,
                sequence \in Seq(Range(sequence)),
                Len(sequence) = Cardinality(SequenceSet(sequence))
         PROVE IsInjective(sequence)
    <2>1. /\ Len(sequence) \in Nat
           /\ sequence \in [1..Len(sequence) -> Range(sequence)]
      BY <1>1, LenProperties
    <2>2. /\ IsFiniteSet(1..Len(sequence))
           /\ Cardinality(1..Len(sequence)) = Len(sequence)
      BY <2>1, FS_Interval, SMT
    <2>3. SequenceSet(sequence) = Range(sequence)
      BY <1>1, RangeEquality DEF SequenceSet
    <2>4. sequence \in Surjection(1..Len(sequence), Range(sequence))
      BY <2>1, Fun_RangeProperties
    <2>5. Cardinality(1..Len(sequence)) =
             Cardinality(Range(sequence))
      BY <1>1, <2>2, <2>3
    <2>6. sequence \in Injection(1..Len(sequence), Range(sequence))
      BY <2>2, <2>4, <2>5, FS_SurjSameCardinalityImpliesInj
    <2> QED BY <2>6 DEF Injection
  <1> QED BY <1>1

THEOREM InjectiveSequenceLengthMatchesSetCardinality ==
  \A sequence:
    /\ sequence \in Seq(Range(sequence))
    /\ IsInjective(sequence)
    => Len(sequence) = Cardinality(SequenceSet(sequence))
PROOF
  <1>1. ASSUME NEW sequence,
                sequence \in Seq(Range(sequence)),
                IsInjective(sequence)
         PROVE Len(sequence) = Cardinality(SequenceSet(sequence))
    <2>1. /\ Len(sequence) \in Nat
           /\ sequence \in [1..Len(sequence) -> Range(sequence)]
      BY <1>1, LenProperties
    <2>2. /\ IsFiniteSet(1..Len(sequence))
           /\ Cardinality(1..Len(sequence)) = Len(sequence)
      BY <2>1, FS_Interval, SMT
    <2>3. sequence \in Surjection(1..Len(sequence), Range(sequence))
      BY <2>1, Fun_RangeProperties
    <2>4. sequence \in Injection(1..Len(sequence), Range(sequence))
      BY <1>1, <2>1 DEF Injection
    <2>5. Cardinality(Range(sequence)) =
             Cardinality(1..Len(sequence))
      BY <2>2, <2>3, <2>4, FS_Surjection
    <2>6. SequenceSet(sequence) = Range(sequence)
      BY <1>1, RangeEquality DEF SequenceSet
    <2> QED BY <2>2, <2>5, <2>6
  <1> QED BY <1>1

THEOREM UniqueCompletionTailFacts ==
  \A sequence:
    /\ AsyncCompletionSequenceTyped(sequence)
    /\ Len(sequence) = Cardinality(SequenceSet(sequence))
    /\ Len(sequence) > 0
    => /\ AsyncCompletionSequenceTyped(Tail(sequence))
       /\ SequenceSet(Tail(sequence)) =
            SequenceSet(sequence) \ {Head(sequence)}
       /\ Len(Tail(sequence)) =
            Cardinality(SequenceSet(Tail(sequence)))
PROOF
  <1>1. ASSUME NEW sequence,
                AsyncCompletionSequenceTyped(sequence),
                Len(sequence) = Cardinality(SequenceSet(sequence)),
                Len(sequence) > 0
         PROVE /\ AsyncCompletionSequenceTyped(Tail(sequence))
               /\ SequenceSet(Tail(sequence)) =
                    SequenceSet(sequence) \ {Head(sequence)}
               /\ Len(Tail(sequence)) =
                    Cardinality(SequenceSet(Tail(sequence)))
    <2>1. /\ sequence \in Seq(Range(sequence))
           /\ sequence # <<>>
           /\ IsInjective(sequence)
      BY <1>1, EmptySeq, UniqueSequenceLengthImpliesInjective, SMT
         DEF AsyncCompletionSequenceTyped
    <2>2. /\ Tail(sequence) \in Seq(Range(sequence))
           /\ Range(Tail(sequence)) \subseteq Range(sequence)
      BY <2>1, HeadTailProperties
    <2>3. /\ Tail(sequence) \in Seq(Range(Tail(sequence)))
           /\ DOMAIN Tail(sequence) = 1..Len(Tail(sequence))
      BY <2>2, SeqOfRange, LenProperties
    <2>4. \A index \in 1..Len(Tail(sequence)):
             /\ AsyncCandidateTyped(Tail(sequence)[index])
             /\ Tail(sequence)[index].class = "Completion"
      <3>1. ASSUME NEW index \in 1..Len(Tail(sequence))
             PROVE /\ AsyncCandidateTyped(Tail(sequence)[index])
                   /\ Tail(sequence)[index].class = "Completion"
        <4>1. Tail(sequence)[index] \in Range(Tail(sequence))
          BY <2>3, <3>1, RangeEquality
        <4>2. Tail(sequence)[index] \in Range(sequence)
          BY <2>2, <4>1
        <4>3. PICK original \in 1..Len(sequence):
                 Tail(sequence)[index] = sequence[original]
          BY <1>1, <2>1, <4>2, RangeEquality
        <4> QED BY <1>1, <4>3 DEF AsyncCompletionSequenceTyped
      <3> QED BY <3>1
    <2>5. AsyncCompletionSequenceTyped(Tail(sequence))
      BY <2>3, <2>4 DEF AsyncCompletionSequenceTyped
    <2>6. /\ IsInjective(Tail(sequence))
           /\ Range(Tail(sequence)) =
                Range(sequence) \ {Head(sequence)}
      BY <2>1, TailInjectiveSeq
    <2>7. SequenceSet(Tail(sequence)) =
             SequenceSet(sequence) \ {Head(sequence)}
      BY <2>1, <2>3, <2>6, RangeEquality DEF SequenceSet
    <2>8. Len(Tail(sequence)) =
             Cardinality(SequenceSet(Tail(sequence)))
      BY <2>3, <2>6, InjectiveSequenceLengthMatchesSetCardinality
    <2> QED BY <2>5, <2>7, <2>8
  <1> QED BY <1>1

THEOREM ConsensusIndicesAfterNonConsensusAppend ==
  \A sequence, job:
    sequence \in Seq(Range(sequence)) /\ job.class # "Consensus"
      => AsyncIoConsensusIndices(Append(sequence, job))
           = AsyncIoConsensusIndices(sequence)
PROOF
  <1>1. ASSUME NEW sequence, NEW job,
                sequence \in Seq(Range(sequence)),
                job.class # "Consensus"
         PROVE AsyncIoConsensusIndices(Append(sequence, job)) =
                 AsyncIoConsensusIndices(sequence)
    <2>1. /\ Len(sequence) \in Nat
           /\ Len(Append(sequence, job)) = Len(sequence) + 1
           /\ (\A index \in 1..Len(sequence):
                 Append(sequence, job)[index] = sequence[index])
           /\ Append(sequence, job)[Len(sequence) + 1] = job
      BY <1>1, AppendSequenceFacts, LenProperties
    <2> QED BY <1>1, <2>1, SMT
         DEF AsyncIoConsensusIndices
  <1> QED BY <1>1

THEOREM PositiveSequenceIsNonempty ==
  \A sequence:
    sequence \in Seq(Range(sequence)) /\ Len(sequence) > 0
      => sequence # <<>>
BY EmptySeq, SMT

THEOREM NonemptySequenceHeadIsFirst ==
  \A sequence:
    sequence \in Seq(Range(sequence)) /\ sequence # <<>>
      => Head(sequence) = sequence[1]
BY SMT

THEOREM TypedIoSequenceRangeIsTyped ==
  \A sequence:
    AsyncIoSequenceTyped(sequence)
      => \A job \in Range(sequence): AsyncIoJobTyped(job)
PROOF
  <1>1. ASSUME NEW sequence, AsyncIoSequenceTyped(sequence)
         PROVE \A job \in Range(sequence): AsyncIoJobTyped(job)
    <2>1. sequence \in Seq(Range(sequence))
      BY <1>1 DEF AsyncIoSequenceTyped
    <2>2. Range(sequence) =
             {sequence[index]: index \in 1..Len(sequence)}
      BY <2>1, RangeEquality
    <2> QED BY <1>1, <2>2 DEF AsyncIoSequenceTyped
  <1> QED BY <1>1

THEOREM TypedIoTailFacts ==
  \A sequence:
    AsyncIoSequenceTyped(sequence) /\ Len(sequence) > 0
      => /\ sequence # <<>>
         /\ AsyncIoJobTyped(Head(sequence))
         /\ AsyncIoSequenceTyped(Tail(sequence))
         /\ SequenceSet(Tail(sequence)) \subseteq SequenceSet(sequence)
         /\ Len(Tail(sequence)) = Len(sequence) - 1
         /\ (\A index \in 1..Len(Tail(sequence)):
               Tail(sequence)[index] = sequence[index + 1])
PROOF
  <1>1. ASSUME NEW sequence,
                AsyncIoSequenceTyped(sequence),
                Len(sequence) > 0
         PROVE /\ sequence # <<>>
               /\ AsyncIoJobTyped(Head(sequence))
               /\ AsyncIoSequenceTyped(Tail(sequence))
               /\ SequenceSet(Tail(sequence))
                    \subseteq SequenceSet(sequence)
               /\ Len(Tail(sequence)) = Len(sequence) - 1
               /\ (\A index \in 1..Len(Tail(sequence)):
                     Tail(sequence)[index] = sequence[index + 1])
    <2>1. sequence \in Seq(Range(sequence))
      BY <1>1 DEF AsyncIoSequenceTyped
    <2>2. sequence # <<>>
      BY <1>1, <2>1, PositiveSequenceIsNonempty
    <2>3. /\ Head(sequence) \in Range(sequence)
           /\ Tail(sequence) \in Seq(Range(sequence))
           /\ Len(Tail(sequence)) = Len(sequence) - 1
           /\ (\A index \in 1..Len(Tail(sequence)):
                 Tail(sequence)[index] = sequence[index + 1])
           /\ Range(Tail(sequence)) \subseteq Range(sequence)
      BY <2>1, <2>2, HeadTailProperties
    <2>4. AsyncIoJobTyped(Head(sequence))
      BY <1>1, <2>3, TypedIoSequenceRangeIsTyped
    <2>5. /\ Tail(sequence) \in Seq(Range(Tail(sequence)))
           /\ DOMAIN Tail(sequence) = 1..Len(Tail(sequence))
      BY <2>3, SeqOfRange, LenProperties
    <2>6. \A index \in 1..Len(Tail(sequence)):
             AsyncIoJobTyped(Tail(sequence)[index])
      <3>1. ASSUME NEW index \in 1..Len(Tail(sequence))
             PROVE AsyncIoJobTyped(Tail(sequence)[index])
        <4>1. Tail(sequence)[index] \in Range(Tail(sequence))
          BY <2>5, <3>1, RangeEquality
        <4>2. Tail(sequence)[index] \in Range(sequence)
          BY <2>3, <4>1
        <4> QED BY <1>1, <4>2, TypedIoSequenceRangeIsTyped
      <3> QED BY <3>1
    <2>7. AsyncIoSequenceTyped(Tail(sequence))
      BY <2>5, <2>6 DEF AsyncIoSequenceTyped
    <2>8. /\ SequenceSet(sequence) = Range(sequence)
           /\ SequenceSet(Tail(sequence)) = Range(Tail(sequence))
      BY <2>1, <2>5, RangeEquality DEF SequenceSet
    <2> QED BY <2>2, <2>3, <2>4, <2>7, <2>8
  <1> QED BY <1>1

THEOREM TailConsensusIndexMapsForward ==
  \A sequence:
    AsyncIoSequenceTyped(sequence) /\ Len(sequence) > 0
      => \A index \in AsyncIoConsensusIndices(Tail(sequence)):
           /\ index + 1 \in AsyncIoConsensusIndices(sequence)
           /\ Tail(sequence)[index] = sequence[index + 1]
           /\ index + 1 # 1
PROOF
  <1>1. ASSUME NEW sequence,
                AsyncIoSequenceTyped(sequence),
                Len(sequence) > 0
         PROVE \A index \in AsyncIoConsensusIndices(Tail(sequence)):
                 /\ index + 1 \in AsyncIoConsensusIndices(sequence)
                 /\ Tail(sequence)[index] = sequence[index + 1]
                 /\ index + 1 # 1
    <2>1. /\ Len(sequence) \in Nat
           /\ Len(Tail(sequence)) = Len(sequence) - 1
           /\ (\A index \in 1..Len(Tail(sequence)):
                 Tail(sequence)[index] = sequence[index + 1])
      BY <1>1, TypedIoTailFacts, LenProperties
         DEF AsyncIoSequenceTyped
    <2>2. ASSUME NEW index \in
                    AsyncIoConsensusIndices(Tail(sequence))
           PROVE /\ index + 1 \in AsyncIoConsensusIndices(sequence)
                 /\ Tail(sequence)[index] = sequence[index + 1]
                 /\ index + 1 # 1
      BY <2>1, <2>2, SMT DEF AsyncIoConsensusIndices
    <2> QED BY <2>2
  <1> QED BY <1>1

THEOREM ConsensusHeadIsFirstConsensusIndex ==
  \A sequence:
    sequence \in Seq(Range(sequence))
      /\ Len(sequence) > 0
      /\ Head(sequence).class = "Consensus"
      => /\ 1 \in AsyncIoConsensusIndices(sequence)
         /\ sequence[1] = Head(sequence)
PROOF
  <1>1. ASSUME NEW sequence,
                sequence \in Seq(Range(sequence)),
                Len(sequence) > 0,
                Head(sequence).class = "Consensus"
         PROVE /\ 1 \in AsyncIoConsensusIndices(sequence)
               /\ sequence[1] = Head(sequence)
    <2>1. /\ sequence # <<>>
           /\ Len(sequence) \in Nat
      BY <1>1, PositiveSequenceIsNonempty, LenProperties
    <2>2. Head(sequence) = sequence[1]
      BY <1>1, <2>1, NonemptySequenceHeadIsFirst
    <2> QED BY <1>1, <2>1, <2>2, SMT
         DEF AsyncIoConsensusIndices
  <1> QED BY <1>1

AsyncIoReadyAfterService(queue, ioReadyQueue) ==
  IF Head(queue).class = "Consensus"
  THEN Append(ioReadyQueue, Head(queue).candidate)
  ELSE ioReadyQueue

AsyncIoResponseItemsAfterService(queue) ==
  LET job == Head(queue)
  IN IF job.class # "Serve"
     THEN {}
     ELSE IF CertifiedServeCanRespond(job.candidate.item)
          THEN {CertifiedResponseItem(job.candidate.item)}
          ELSE IF CommitCertificateServeCanRespond(job.candidate.item)
               THEN CommitCertificateResponseItems(job.candidate.item)
               ELSE {}

THEOREM CertifiedResponseItemIsTyped ==
  \A request:
    /\ AsyncConfiguration
    /\ AsyncItemTyped(request)
    /\ request.kind = "CertifiedRequest"
    => AsyncItemTyped(CertifiedResponseItem(request))
BY SMTT(30)
   DEF AsyncConfiguration, AsyncItemTyped, CertifiedResponseItem,
       AsyncNetworkItem, AsyncBodyEnvelope, AsyncBodyEnvelopeTyped,
       AsyncNetworkKinds, AsyncIngressSources, ValidatorIds,
       AsyncHeartbeatSubject, NoAsyncChunk

THEOREM CommitCertificateResponseItemIsTyped ==
  \A request, qc:
    /\ AsyncItemTyped(request)
    /\ request.kind = "CommitCertificateRequest"
    /\ qc \in QcRecordSet
    => AsyncItemTyped(CommitCertificateResponseItem(request, qc))
PROOF
  <1>1. ASSUME NEW request, NEW qc,
                AsyncItemTyped(request),
                request.kind = "CommitCertificateRequest",
                qc \in QcRecordSet
         PROVE AsyncItemTyped(CommitCertificateResponseItem(request, qc))
    <2>1. /\ request.source \in ValidatorIds
           /\ request.envelope.recipient \in ValidatorIds
      BY <1>1, SMT DEF AsyncItemTyped
    <2>2. QcEnvelope(request.source, qc) \in QcEnvelopeSet
      BY <1>1, <2>1, Isa DEF QcEnvelope, QcEnvelopeSet
    <2>3. /\ DOMAIN CommitCertificateResponseItem(request, qc) =
                  {"kind", "source", "envelope"}
           /\ CommitCertificateResponseItem(request, qc).kind =
                  "CommitCertificateResponse"
           /\ CommitCertificateResponseItem(request, qc).source =
                  request.envelope.recipient
           /\ CommitCertificateResponseItem(request, qc).envelope =
                  QcEnvelope(request.source, qc)
      BY DEF CommitCertificateResponseItem, AsyncNetworkItem
    <2>4. CommitCertificateResponseItem(request, qc).kind
             \in AsyncNetworkKinds
      BY <2>3, SMT DEF AsyncNetworkKinds
    <2>5. CommitCertificateResponseItem(request, qc).source
             \in ValidatorIds
      BY <2>1, <2>3
    <2>6. CommitCertificateResponseItem(request, qc).envelope.recipient
             \in ValidatorIds
      BY <2>1, <2>3, SMT DEF QcEnvelope
    <2>7. /\ CommitCertificateResponseItem(request, qc).source
                    \in AsyncIngressSources
           /\ (CommitCertificateResponseItem(request, qc).kind # "Noise"
                 => CommitCertificateResponseItem(request, qc).source
                      \in ValidatorIds)
      BY <2>5, Isa DEF AsyncIngressSources
    <2>8. (CASE CommitCertificateResponseItem(request, qc).kind =
                    "Proposal" ->
                  CommitCertificateResponseItem(request, qc).envelope
                    \in ProposalEnvelopeSet
           [] CommitCertificateResponseItem(request, qc).kind
                    \in {"PrepareVote", "CommitVote"} ->
                  CommitCertificateResponseItem(request, qc).envelope
                    \in VoteEnvelopeSet
           [] CommitCertificateResponseItem(request, qc).kind
                    \in {"PrepareQC", "CommitQC"} ->
                  CommitCertificateResponseItem(request, qc).envelope
                    \in QcEnvelopeSet
           [] CommitCertificateResponseItem(request, qc).kind =
                    "TimeoutVote" ->
                  CommitCertificateResponseItem(request, qc).envelope
                    \in TimeoutEnvelopeSet
           [] CommitCertificateResponseItem(request, qc).kind =
                    "TimeoutCertificate" ->
                  CommitCertificateResponseItem(request, qc).envelope
                    \in TcEnvelopeSet
           [] CommitCertificateResponseItem(request, qc).kind =
                    "CommitCertificateResponse" ->
                  CommitCertificateResponseItem(request, qc).envelope
                    \in QcEnvelopeSet
           [] OTHER ->
                  AsyncBodyEnvelopeTyped(
                    CommitCertificateResponseItem(request, qc).envelope))
      BY <2>2, <2>3, SMT
    <2> QED BY <2>3, <2>4, <2>6, <2>7, <2>8
         DEF AsyncItemTyped
  <1> QED BY <1>1

THEOREM StrongInvariantTypesAppliedCertificates ==
  StrongInductiveInvariant
    => \A application \in applied: application.qc \in QcRecordSet
BY SMT
   DEF StrongInductiveInvariant, Safety, TypeInvariant,
       DecisionAgreement, AppliedRequiresDecision

THEOREM ServiceResponseItemsAreFiniteAndTyped ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ StrongInductiveInvariant
    /\ AsyncIoQueueDepth(node) > 0
    => /\ IsFiniteSet(
             AsyncIoResponseItemsAfterService(asyncIoQueues[node]))
       /\ \A item \in
              AsyncIoResponseItemsAfterService(asyncIoQueues[node]):
            AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                StrongInductiveInvariant,
                AsyncIoQueueDepth(node) > 0
         PROVE /\ IsFiniteSet(
                      AsyncIoResponseItemsAfterService(
                        asyncIoQueues[node]))
               /\ \A item \in
                      AsyncIoResponseItemsAfterService(
                        asyncIoQueues[node]):
                    AsyncItemTyped(item)
    <2>1. /\ AsyncConfiguration
           /\ node \in ValidatorIds
           /\ AsyncIoSequenceTyped(asyncIoQueues[node])
           /\ AsyncIoJobTyped(Head(asyncIoQueues[node]))
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators,
         TypedIoTailFacts
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant, AsyncIoQueueDepth
    <2>2. Head(asyncIoQueues[node]).class = "Serve" =>
             /\ AsyncItemTyped(
                  Head(asyncIoQueues[node]).candidate.item)
             /\ Head(asyncIoQueues[node]).candidate.item.kind
                  \in {"CertifiedRequest", "CommitCertificateRequest"}
      BY <2>1, SMT
         DEF AsyncIoJobTyped, AsyncCandidateTyped,
             NoAsyncItem, AsyncNetworkItem
    <2>3. \A application \in applied:
             application.qc \in QcRecordSet
      BY <1>1, StrongInvariantTypesAppliedCertificates
    <2>4. CASE Head(asyncIoQueues[node]).class # "Serve"
      BY <2>4, FS_EmptySet
         DEF AsyncIoResponseItemsAfterService
    <2>5. CASE /\ Head(asyncIoQueues[node]).class = "Serve"
                  /\ CertifiedServeCanRespond(
                       Head(asyncIoQueues[node]).candidate.item)
      <3>1. AsyncItemTyped(CertifiedResponseItem(
                 Head(asyncIoQueues[node]).candidate.item))
        BY <2>1, <2>2, <2>5, CertifiedResponseItemIsTyped
           DEF CertifiedServeCanRespond
      <3>2. AsyncIoResponseItemsAfterService(asyncIoQueues[node]) =
               {CertifiedResponseItem(
                  Head(asyncIoQueues[node]).candidate.item)}
        BY <2>5 DEF AsyncIoResponseItemsAfterService
      <3> QED BY <3>1, <3>2, FS_Singleton
    <2>6. CASE /\ Head(asyncIoQueues[node]).class = "Serve"
                  /\ ~CertifiedServeCanRespond(
                       Head(asyncIoQueues[node]).candidate.item)
                  /\ CommitCertificateServeCanRespond(
                       Head(asyncIoQueues[node]).candidate.item)
      <3>1. LET request == Head(asyncIoQueues[node]).candidate.item
             IN /\ AsyncItemTyped(request)
                /\ request.kind = "CommitCertificateRequest"
        BY <2>2, <2>6 DEF CommitCertificateServeCanRespond
      <3>2. LET request == Head(asyncIoQueues[node]).candidate.item
             IN /\ CommitCertificateServiceApplication(request) \in applied
                /\ CommitCertificateServiceApplication(request).qc
                     \in QcRecordSet
        BY <2>3, <2>6, Zenon
           DEF CommitCertificateServeCanRespond,
               CommitCertificateServiceApplication
      <3>3. LET request == Head(asyncIoQueues[node]).candidate.item
             IN AsyncItemTyped(CommitCertificateResponseItem(
                  request,
                  CommitCertificateServiceApplication(request).qc))
        BY <3>1, <3>2, CommitCertificateResponseItemIsTyped
      <3>4. AsyncIoResponseItemsAfterService(asyncIoQueues[node]) =
               CommitCertificateResponseItems(
                 Head(asyncIoQueues[node]).candidate.item)
        BY <2>6 DEF AsyncIoResponseItemsAfterService
      <3>5. LET request == Head(asyncIoQueues[node]).candidate.item
             IN CommitCertificateResponseItems(request) =
                  {CommitCertificateResponseItem(
                    request,
                    CommitCertificateServiceApplication(request).qc)}
        BY <2>6 DEF CommitCertificateResponseItems
      <3> QED BY <3>3, <3>4, <3>5, FS_Singleton
    <2>7. CASE /\ Head(asyncIoQueues[node]).class = "Serve"
                  /\ ~CertifiedServeCanRespond(
                       Head(asyncIoQueues[node]).candidate.item)
                  /\ ~CommitCertificateServeCanRespond(
                       Head(asyncIoQueues[node]).candidate.item)
      BY <2>7, FS_EmptySet
         DEF AsyncIoResponseItemsAfterService
    <2> QED BY <2>4, <2>5, <2>6, <2>7
  <1> QED BY <1>1

AsyncSentItemsType(items) ==
  /\ IsFiniteSet(items)
  /\ \A item \in items: AsyncItemTyped(item)

AsyncRetainedControlType(retained, voters) ==
  /\ IsFiniteSet(retained)
  /\ \A item \in retained:
       /\ AsyncItemTyped(item)
       /\ item.kind \in AsyncControlKinds
  /\ \A source \in ValidatorIds, controlClass \in AsyncControlKinds:
       LET retainedClass ==
             RetainedClassItems(retained, source, controlClass)
       IN \/ retainedClass = {}
          \/ /\ Cardinality(retainedClass) <= Cardinality(voters)
             /\ {item.envelope.recipient: item \in retainedClass} = voters
             /\ \A left, right \in retainedClass:
                  ControlView(left) = ControlView(right)

AsyncActiveRequestsType(active, sent) ==
  /\ IsFiniteSet(active)
  /\ active \subseteq sent
  /\ \A item \in active:
       /\ AsyncItemTyped(item)
       /\ item.kind \in {"CertifiedRequest",
                          "CommitCertificateRequest"}

THEOREM AsyncTransportHistoryTypeDecomposition ==
  AsyncTransportHistoryTypeInvariant
    <=> /\ AsyncSentItemsType(asyncSentItems)
        /\ AsyncRetainedControlType(
             asyncRetainedControl, CurrentVoters)
        /\ AsyncActiveRequestsType(
             asyncActiveRequests, asyncSentItems)
BY DEF AsyncTransportHistoryTypeInvariant, AsyncSentItemsType,
       AsyncRetainedControlType, AsyncActiveRequestsType

THEOREM PacketForTypedItemIsTyped ==
  \A item:
    /\ AsyncConfiguration
    /\ asyncNow \in Nat
    /\ AsyncItemTyped(item)
    => AsyncPacketTyped(
         AsyncPacket(item, asyncNow, asyncNow + AsyncDeliveryBound))
BY SMT
   DEF AsyncConfiguration, AsyncPacketTyped, AsyncPacket

THEOREM PacketsForItemsAreFiniteAndTyped ==
  \A items:
    /\ AsyncConfiguration
    /\ asyncNow \in Nat
    /\ IsFiniteSet(items)
    /\ \A item \in items: AsyncItemTyped(item)
    => /\ IsFiniteSet(PacketsForItems(items))
       /\ \A packet \in PacketsForItems(items):
            AsyncPacketTyped(packet)
PROOF
  <1>1. ASSUME NEW items,
                AsyncConfiguration,
                asyncNow \in Nat,
                IsFiniteSet(items),
                \A item \in items: AsyncItemTyped(item)
         PROVE /\ IsFiniteSet(PacketsForItems(items))
               /\ \A packet \in PacketsForItems(items):
                    AsyncPacketTyped(packet)
    <2>1. IsFiniteSet(PacketsForItems(items))
      BY <1>1, FS_Image DEF PacketsForItems
    <2>2. \A packet \in PacketsForItems(items):
             AsyncPacketTyped(packet)
      <3>1. ASSUME NEW packet \in PacketsForItems(items)
             PROVE AsyncPacketTyped(packet)
        <4>1. PICK item \in items:
                 packet = AsyncPacket(
                   item, asyncNow, asyncNow + AsyncDeliveryBound)
          BY <3>1 DEF PacketsForItems
        <4>2. AsyncItemTyped(item)
          BY <1>1, <4>1
        <4> QED BY <1>1, <4>1, <4>2,
                     PacketForTypedItemIsTyped
      <3> QED BY <3>1
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM PublishEphemeralItemsPreservesTransportContentType ==
  \A items:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ AsyncTransportContentTypeInvariant
    /\ IsFiniteSet(items)
    /\ \A item \in items: AsyncItemTyped(item)
    /\ PublishEphemeralItems(items)
    /\ UNCHANGED <<context, asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW items,
                AsyncRuntimeScalarTypeInvariant,
                AsyncTransportContentTypeInvariant,
                IsFiniteSet(items),
                \A item \in items: AsyncItemTyped(item),
                PublishEphemeralItems(items),
                UNCHANGED <<context, asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. /\ AsyncSentItemsType(asyncSentItems)
           /\ AsyncRetainedControlType(
                asyncRetainedControl, CurrentVoters)
           /\ AsyncActiveRequestsType(
                asyncActiveRequests, asyncSentItems)
           /\ AsyncPacketContentTypeInvariant
           /\ AsyncHeldChunksTypeInvariant
      BY <1>1, AsyncTransportHistoryTypeDecomposition
         DEF AsyncTransportContentTypeInvariant
    <2>2. /\ asyncSentItems' = asyncSentItems \cup items
           /\ asyncRetainedControl' = asyncRetainedControl
           /\ asyncActiveRequests' = asyncActiveRequests
           /\ asyncTransport' =
                asyncTransport \cup PacketsForItems(items)
           /\ context' = context
           /\ asyncHeldChunks' = asyncHeldChunks
      BY <1>1 DEF PublishEphemeralItems
    <2>3. AsyncSentItemsType(asyncSentItems')
      <3>1. IsFiniteSet(asyncSentItems')
        BY <1>1, <2>1, <2>2, FS_Union
           DEF AsyncSentItemsType
      <3>2. \A item \in asyncSentItems': AsyncItemTyped(item)
        BY <1>1, <2>1, <2>2
           DEF AsyncSentItemsType
      <3> QED BY <3>1, <3>2 DEF AsyncSentItemsType
    <2>4. CurrentVoters' = CurrentVoters
      BY <2>2, Isa DEF CurrentVoters, CurrentEpoch
    <2>5. AsyncRetainedControlType(
             asyncRetainedControl', CurrentVoters')
      BY <2>1, <2>2, <2>4
    <2>6. AsyncActiveRequestsType(
             asyncActiveRequests', asyncSentItems')
      BY <2>1, <2>2, Isa DEF AsyncActiveRequestsType
    <2>7. AsyncTransportHistoryTypeInvariant'
      BY <2>3, <2>5, <2>6
         DEF AsyncTransportHistoryTypeInvariant,
             AsyncSentItemsType, AsyncRetainedControlType,
             AsyncActiveRequestsType
    <2>8. /\ IsFiniteSet(PacketsForItems(items))
           /\ \A packet \in PacketsForItems(items):
                AsyncPacketTyped(packet)
      BY <1>1, PacketsForItemsAreFiniteAndTyped
         DEF AsyncRuntimeScalarTypeInvariant
    <2>9. AsyncPacketContentTypeInvariant'
      <3>1. IsFiniteSet(asyncTransport')
        BY <2>1, <2>2, <2>8, FS_Union
           DEF AsyncPacketContentTypeInvariant
      <3>2. \A packet \in asyncTransport':
               AsyncPacketTyped(packet)
        BY <2>1, <2>2, <2>8
           DEF AsyncPacketContentTypeInvariant
      <3> QED BY <3>1, <3>2 DEF AsyncPacketContentTypeInvariant
    <2>10. AsyncHeldChunksTypeInvariant'
      BY <2>1, <2>2 DEF AsyncHeldChunksTypeInvariant
    <2> QED BY <2>7, <2>9, <2>10
         DEF AsyncTransportContentTypeInvariant
  <1> QED BY <1>1

THEOREM ServiceHeadPreservesConsensusQueueOwnership ==
  \A queue, ioReadyQueue, localReadyQueue:
    /\ AsyncIoSequenceTyped(queue)
    /\ Len(queue) > 0
    /\ ioReadyQueue \in Seq(Range(ioReadyQueue))
    /\ AsyncIoConsensusQueueOwnership(
         queue, ioReadyQueue, localReadyQueue)
    => AsyncIoConsensusQueueOwnership(
         Tail(queue), AsyncIoReadyAfterService(queue, ioReadyQueue),
         localReadyQueue)
PROOF
  <1>1. ASSUME NEW queue, NEW ioReadyQueue, NEW localReadyQueue,
                AsyncIoSequenceTyped(queue),
                Len(queue) > 0,
                ioReadyQueue \in Seq(Range(ioReadyQueue)),
                AsyncIoConsensusQueueOwnership(
                  queue, ioReadyQueue, localReadyQueue)
         PROVE AsyncIoConsensusQueueOwnership(
                 Tail(queue),
                 AsyncIoReadyAfterService(queue, ioReadyQueue),
                 localReadyQueue)
    <2>1. /\ queue \in Seq(Range(queue))
           /\ queue # <<>>
           /\ Head(queue) = queue[1]
      BY <1>1, TypedIoTailFacts, NonemptySequenceHeadIsFirst
         DEF AsyncIoSequenceTyped
    <2>2. /\ (\A index \in AsyncIoConsensusIndices(queue):
                  /\ queue[index].candidate
                       \notin SequenceSet(ioReadyQueue)
                  /\ queue[index].candidate
                       \notin SequenceSet(localReadyQueue))
           /\ (\A left, right \in AsyncIoConsensusIndices(queue):
                 queue[left].candidate = queue[right].candidate
                   => left = right)
      BY <1>1 DEF AsyncIoConsensusQueueOwnership
    <2>3. \A left, right \in
                    AsyncIoConsensusIndices(Tail(queue)):
             Tail(queue)[left].candidate = Tail(queue)[right].candidate
               => left = right
      <3>1. ASSUME NEW left \in
                      AsyncIoConsensusIndices(Tail(queue)),
                    NEW right \in
                      AsyncIoConsensusIndices(Tail(queue)),
                    Tail(queue)[left].candidate =
                      Tail(queue)[right].candidate
             PROVE left = right
        <4>1. /\ left + 1 \in AsyncIoConsensusIndices(queue)
              /\ Tail(queue)[left] = queue[left + 1]
          BY <1>1, <3>1, TailConsensusIndexMapsForward
        <4>2. /\ right + 1 \in AsyncIoConsensusIndices(queue)
              /\ Tail(queue)[right] = queue[right + 1]
          BY <1>1, <3>1, TailConsensusIndexMapsForward
        <4>3. left + 1 = right + 1
          BY <2>2, <3>1, <4>1, <4>2
        <4>4. /\ left \in Nat
              /\ right \in Nat
          BY <3>1, SMT DEF AsyncIoConsensusIndices
        <4> QED BY <4>3, <4>4, Isa
      <3> QED BY <3>1
    <2>4. \A index \in AsyncIoConsensusIndices(Tail(queue)):
             /\ Tail(queue)[index].candidate
                  \notin SequenceSet(
                    AsyncIoReadyAfterService(queue, ioReadyQueue))
             /\ Tail(queue)[index].candidate
                  \notin SequenceSet(localReadyQueue)
      <3>1. ASSUME NEW index \in
                      AsyncIoConsensusIndices(Tail(queue))
             PROVE /\ Tail(queue)[index].candidate
                          \notin SequenceSet(
                            AsyncIoReadyAfterService(
                              queue, ioReadyQueue))
                       /\ Tail(queue)[index].candidate
                          \notin SequenceSet(localReadyQueue)
        <4>1. /\ index + 1 \in AsyncIoConsensusIndices(queue)
              /\ Tail(queue)[index] = queue[index + 1]
              /\ index + 1 # 1
          BY <1>1, <3>1, TailConsensusIndexMapsForward
        <4>2. /\ queue[index + 1].candidate
                       \notin SequenceSet(ioReadyQueue)
              /\ queue[index + 1].candidate
                       \notin SequenceSet(localReadyQueue)
          BY <2>2, <4>1
        <4>3. CASE Head(queue).class = "Consensus"
          <5>1. /\ 1 \in AsyncIoConsensusIndices(queue)
                /\ queue[1] = Head(queue)
            BY <1>1, <2>1, <4>3,
               ConsensusHeadIsFirstConsensusIndex
          <5>2. queue[index + 1].candidate #
                   Head(queue).candidate
            BY <2>2, <4>1, <5>1, SMT
          <5>3. SequenceSet(
                   AsyncIoReadyAfterService(queue, ioReadyQueue)) =
                   SequenceSet(ioReadyQueue) \cup
                     {Head(queue).candidate}
            BY <1>1, <4>3, SequenceSetAfterAppend
               DEF AsyncIoReadyAfterService
          <5> QED BY <4>1, <4>2, <5>2, <5>3
        <4>4. CASE Head(queue).class # "Consensus"
          BY <4>1, <4>2, <4>4
             DEF AsyncIoReadyAfterService
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>1
    <2> QED BY <2>3, <2>4
         DEF AsyncIoConsensusQueueOwnership
  <1> QED BY <1>1

THEOREM ServiceHeadPreservesIoReadyFacts ==
  \A queue, outstanding, ioReadyQueue, localReadyQueue:
    /\ AsyncIoSequenceTyped(queue)
    /\ Len(queue) > 0
    /\ IsFiniteSet(outstanding)
    /\ (Head(queue).class = "Consensus" =>
          Head(queue).candidate \in outstanding)
    /\ AsyncCompletionSequenceTyped(ioReadyQueue)
    /\ Len(ioReadyQueue) = Cardinality(SequenceSet(ioReadyQueue))
    /\ SequenceSet(ioReadyQueue) \subseteq outstanding
    /\ SequenceSet(ioReadyQueue) \cap
         SequenceSet(localReadyQueue) = {}
    /\ AsyncIoConsensusQueueOwnership(
         queue, ioReadyQueue, localReadyQueue)
    => /\ AsyncCompletionSequenceTyped(
             AsyncIoReadyAfterService(queue, ioReadyQueue))
       /\ Len(AsyncIoReadyAfterService(queue, ioReadyQueue)) =
            Cardinality(SequenceSet(
              AsyncIoReadyAfterService(queue, ioReadyQueue)))
       /\ SequenceSet(AsyncIoReadyAfterService(queue, ioReadyQueue))
            \subseteq outstanding
       /\ SequenceSet(AsyncIoReadyAfterService(queue, ioReadyQueue))
            \cap SequenceSet(localReadyQueue) = {}
PROOF
  <1>1. ASSUME NEW queue, NEW outstanding,
                NEW ioReadyQueue, NEW localReadyQueue,
                AsyncIoSequenceTyped(queue),
                Len(queue) > 0,
                IsFiniteSet(outstanding),
                Head(queue).class = "Consensus" =>
                  Head(queue).candidate \in outstanding,
                AsyncCompletionSequenceTyped(ioReadyQueue),
                Len(ioReadyQueue) =
                  Cardinality(SequenceSet(ioReadyQueue)),
                SequenceSet(ioReadyQueue) \subseteq outstanding,
                SequenceSet(ioReadyQueue) \cap
                  SequenceSet(localReadyQueue) = {},
                AsyncIoConsensusQueueOwnership(
                  queue, ioReadyQueue, localReadyQueue)
         PROVE /\ AsyncCompletionSequenceTyped(
                      AsyncIoReadyAfterService(queue, ioReadyQueue))
               /\ Len(AsyncIoReadyAfterService(queue, ioReadyQueue)) =
                    Cardinality(SequenceSet(
                      AsyncIoReadyAfterService(queue, ioReadyQueue)))
               /\ SequenceSet(
                      AsyncIoReadyAfterService(queue, ioReadyQueue))
                    \subseteq outstanding
               /\ SequenceSet(
                      AsyncIoReadyAfterService(queue, ioReadyQueue))
                    \cap SequenceSet(localReadyQueue) = {}
    <2>1. CASE Head(queue).class = "Consensus"
      <3>1. /\ queue \in Seq(Range(queue))
             /\ AsyncIoJobTyped(Head(queue))
        BY <1>1, TypedIoTailFacts DEF AsyncIoSequenceTyped
      <3>2. /\ AsyncCandidateTyped(Head(queue).candidate)
             /\ Head(queue).candidate.class = "Completion"
             /\ Head(queue).candidate \in outstanding
        BY <1>1, <2>1, <3>1 DEF AsyncIoJobTyped
      <3>3. /\ 1 \in AsyncIoConsensusIndices(queue)
             /\ queue[1] = Head(queue)
        BY <1>1, <2>1, <3>1,
           ConsensusHeadIsFirstConsensusIndex
      <3>4. /\ Head(queue).candidate
                    \notin SequenceSet(ioReadyQueue)
             /\ Head(queue).candidate
                    \notin SequenceSet(localReadyQueue)
        BY <1>1, <3>3 DEF AsyncIoConsensusQueueOwnership
      <3>5. IsFiniteSet(SequenceSet(ioReadyQueue))
        BY <1>1, FS_Subset
      <3>6. /\ AsyncIoReadyAfterService(queue, ioReadyQueue) =
                    Append(ioReadyQueue, Head(queue).candidate)
             /\ SequenceSet(
                  AsyncIoReadyAfterService(queue, ioReadyQueue)) =
                    SequenceSet(ioReadyQueue) \cup
                      {Head(queue).candidate}
             /\ Len(AsyncIoReadyAfterService(queue, ioReadyQueue)) =
                    Len(ioReadyQueue) + 1
        BY <1>1, <2>1, <3>1, SequenceSetAfterAppend,
           AppendSequenceFacts
           DEF AsyncIoReadyAfterService,
               AsyncCompletionSequenceTyped
      <3>7. Cardinality(SequenceSet(
                 AsyncIoReadyAfterService(queue, ioReadyQueue))) =
               Cardinality(SequenceSet(ioReadyQueue)) + 1
        BY <3>4, <3>5, <3>6, FS_AddElement
      <3>8. AsyncCompletionSequenceTyped(
               AsyncIoReadyAfterService(queue, ioReadyQueue))
        BY <1>1, <3>2, <3>6,
           TypedCompletionAppendPreservesSequenceType
      <3>9. /\ SequenceSet(
                      AsyncIoReadyAfterService(queue, ioReadyQueue))
                    \subseteq outstanding
             /\ SequenceSet(
                      AsyncIoReadyAfterService(queue, ioReadyQueue))
                    \cap SequenceSet(localReadyQueue) = {}
        BY <1>1, <3>2, <3>4, <3>6, Isa
      <3>10. Len(AsyncIoReadyAfterService(queue, ioReadyQueue)) =
                Cardinality(SequenceSet(
                  AsyncIoReadyAfterService(queue, ioReadyQueue)))
        BY <1>1, <3>6, <3>7, Isa
      <3> QED BY <3>8, <3>9, <3>10
    <2>2. CASE Head(queue).class # "Consensus"
      BY <1>1, <2>2 DEF AsyncIoReadyAfterService
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM EnqueueIoControlPreservesTopologyType ==
  \A node \in AsyncCurrentResponsiveVoters:
    AsyncTypeInvariant /\ EnqueueIoLocalControl(node)
      => AsyncIoTopologyTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                EnqueueIoLocalControl(node)
         PROVE AsyncIoTopologyTypeInvariant'
    <2>1. AsyncIoTopologyTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant
    <2>2. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>3. asyncIoControlAvailable'
               \in [ValidatorIds -> BOOLEAN]
      BY <1>1, <2>1, <2>2, FunctionalUpdatePreservesType
         DEF EnqueueIoLocalControl, AsyncIoTopologyTypeInvariant
    <2> QED BY <1>1, <2>1, <2>3, Isa
         DEF EnqueueIoLocalControl, AsyncIoTopologyTypeInvariant,
             AsyncDeferredVars, LeaveCausalQueues
  <1> QED BY <1>1

THEOREM EnqueueIoControlPreservesContentType ==
  \A node \in AsyncCurrentResponsiveVoters:
    AsyncTypeInvariant /\ EnqueueIoLocalControl(node)
      => AsyncIoContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                EnqueueIoLocalControl(node)
         PROVE AsyncIoContentTypeInvariant'
    <2>1. /\ AsyncConfiguration
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant
    <2>2. AsyncIoJobTyped(AsyncIoControlJob)
      BY <2>1, AsyncControlIoJobIsTyped
    <2>3. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>4. ASSUME NEW other \in ValidatorIds
           PROVE AsyncIoSequenceTyped(asyncIoQueues'[other])
      <3>1. CASE other = node
        <4>1. AsyncIoSequenceTyped(asyncIoQueues[node])
          BY <2>1, <3>1 DEF AsyncIoQueueContentTypeInvariant
        <4>2. asyncIoQueues'[other] =
                 Append(asyncIoQueues[node], AsyncIoControlJob)
          BY <1>1, <2>1, <2>3, <3>1,
             FunctionalAppendUpdateAtKey
             DEF EnqueueIoLocalControl, AsyncIoTopologyTypeInvariant
        <4> QED BY <2>2, <4>1, <4>2,
                     TypedIoAppendPreservesSequenceType
      <3>2. CASE other # node
        <4>1. asyncIoQueues'[other] = asyncIoQueues[other]
          BY <1>1, <2>1, <2>4, <3>2,
             FunctionalAppendUpdateAwayFromKey
             DEF EnqueueIoLocalControl, AsyncIoTopologyTypeInvariant
        <4>2. AsyncIoSequenceTyped(asyncIoQueues[other])
          BY <2>1, <2>4 DEF AsyncIoQueueContentTypeInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1, <3>2
    <2>5. ASSUME NEW other \in ValidatorIds
           PROVE \A job \in SequenceSet(asyncIoQueues'[other]):
                   job.class = "Consensus" =>
                     job.candidate \in asyncOutstandingWork'[other]
      <3>1. CASE other = node
        <4>1. asyncIoQueues[node] \in Seq(Range(asyncIoQueues[node]))
          BY <2>1, <2>3
             DEF AsyncIoQueueContentTypeInvariant, AsyncIoSequenceTyped
        <4>2. asyncIoQueues'[node] =
                 Append(asyncIoQueues[node], AsyncIoControlJob)
          BY <1>1, <2>1, <2>3, FunctionalAppendUpdateAtKey
             DEF EnqueueIoLocalControl, AsyncIoTopologyTypeInvariant
        <4>3. SequenceSet(asyncIoQueues'[node]) =
                 SequenceSet(asyncIoQueues[node]) \cup {AsyncIoControlJob}
          BY <4>1, <4>2, SequenceSetAfterAppend
        <4>4. asyncOutstandingWork'[node] = asyncOutstandingWork[node]
          BY <1>1 DEF EnqueueIoLocalControl
        <4>5. ASSUME NEW job \in SequenceSet(asyncIoQueues'[other]),
                       job.class = "Consensus"
               PROVE job.candidate \in asyncOutstandingWork'[other]
          <5>1. job \in SequenceSet(asyncIoQueues[node])
                  \/ job = AsyncIoControlJob
            BY <3>1, <4>3, <4>5
          <5>2. CASE job \in SequenceSet(asyncIoQueues[node])
            BY <2>1, <3>1, <4>4, <4>5, <5>2
               DEF AsyncIoQueueContentTypeInvariant
          <5>3. CASE job = AsyncIoControlJob
            BY <4>5, <5>3, SMT
               DEF AsyncIoControlJob, AsyncIoJob
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>5
      <3>2. CASE other # node
        <4>1. asyncIoQueues'[other] = asyncIoQueues[other]
          BY <1>1, <2>1, <2>5, <3>2,
             FunctionalAppendUpdateAwayFromKey
             DEF EnqueueIoLocalControl, AsyncIoTopologyTypeInvariant
        <4>2. asyncOutstandingWork'[other] =
                 asyncOutstandingWork[other]
          BY <1>1 DEF EnqueueIoLocalControl
        <4> QED BY <2>1, <2>5, <4>1, <4>2
             DEF AsyncIoQueueContentTypeInvariant
      <3> QED BY <3>1, <3>2
    <2>6. UNCHANGED
             <<asyncIoReadyCompletions, asyncLocalReadyCompletions>>
      BY <1>1 DEF EnqueueIoLocalControl
    <2>7. ASSUME NEW other \in ValidatorIds
           PROVE AsyncIoConsensusCandidateOwnership(
                   other, asyncIoQueues', asyncIoReadyCompletions',
                   asyncLocalReadyCompletions')
      <3>1. CASE other = node
        <4>1. AsyncIoConsensusCandidateOwnership(
                 node, asyncIoQueues, asyncIoReadyCompletions,
                 asyncLocalReadyCompletions)
          BY <2>1, <2>3
             DEF AsyncIoQueueContentTypeInvariant
        <4>2. asyncIoQueues[node] \in Seq(Range(asyncIoQueues[node]))
          BY <2>1, <2>3
             DEF AsyncIoQueueContentTypeInvariant, AsyncIoSequenceTyped
        <4>3. asyncIoQueues'[node] =
                 Append(asyncIoQueues[node], AsyncIoControlJob)
          BY <1>1, <2>1, <2>3, FunctionalAppendUpdateAtKey
             DEF EnqueueIoLocalControl, AsyncIoTopologyTypeInvariant
        <4>4. AsyncIoControlJob.class # "Consensus"
          BY SMT DEF AsyncIoControlJob, AsyncIoJob
        <4>5. AsyncIoConsensusIndices(asyncIoQueues'[node]) =
                 AsyncIoConsensusIndices(asyncIoQueues[node])
          BY <4>2, <4>3, <4>4,
             ConsensusIndicesAfterNonConsensusAppend
        <4>6. \A index \in
                    AsyncIoConsensusIndices(asyncIoQueues[node]):
                 asyncIoQueues'[node][index] =
                   asyncIoQueues[node][index]
          BY <4>2, <4>3, AppendSequenceFacts
             DEF AsyncIoConsensusIndices
        <4>7. AsyncIoConsensusCandidateOwnership(
                 node, asyncIoQueues', asyncIoReadyCompletions',
                 asyncLocalReadyCompletions')
          BY <2>6, <4>1, <4>5, <4>6, SMT
             DEF AsyncIoConsensusCandidateOwnership,
                 AsyncIoConsensusQueueOwnership,
                 AsyncIoConsensusIndices
        <4> QED BY <3>1, <4>7
      <3>2. CASE other # node
        <4>1. asyncIoQueues'[other] = asyncIoQueues[other]
          BY <1>1, <2>1, <2>7, <3>2,
             FunctionalAppendUpdateAwayFromKey
             DEF EnqueueIoLocalControl, AsyncIoTopologyTypeInvariant
        <4>2. AsyncIoConsensusCandidateOwnership(
                 other, asyncIoQueues, asyncIoReadyCompletions,
                 asyncLocalReadyCompletions)
          BY <2>1, <2>7
             DEF AsyncIoQueueContentTypeInvariant
        <4> QED BY <2>6, <4>1, <4>2, SMT
             DEF AsyncIoConsensusCandidateOwnership,
                 AsyncIoConsensusQueueOwnership
      <3> QED BY <3>1, <3>2
    <2>8. AsyncIoQueueContentTypeInvariant'
      BY <2>4, <2>5, <2>7 DEF AsyncIoQueueContentTypeInvariant
    <2>9. UNCHANGED AsyncIoWorkContentTypeVars
      BY <1>1, Isa
         DEF EnqueueIoLocalControl, AsyncIoWorkContentTypeVars,
             AsyncDeferredVars, LeaveCausalQueues
    <2>10. AsyncIoWorkContentTypeInvariant'
      BY <2>1, <2>9, AsyncIoWorkContentTypeStutter
    <2> QED BY <2>8, <2>10 DEF AsyncIoContentTypeInvariant
  <1> QED BY <1>1

THEOREM EnqueueIoControlPreservesCapacityType ==
  \A node \in AsyncCurrentResponsiveVoters:
    AsyncTypeInvariant /\ EnqueueIoLocalControl(node)
      => AsyncIoCapacityTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                EnqueueIoLocalControl(node)
         PROVE AsyncIoCapacityTypeInvariant'
    <2>1. /\ AsyncConfiguration
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant
    <2>2. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>3. ASSUME NEW other \in ValidatorIds
           PROVE /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
                 /\ AsyncCompletionLoad(other)' <=
                      AsyncCompletionReserve
                 /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
                 /\ AsyncCompletionLoad(other)' <= AsyncIoWorkCapacity
      <3>1. CASE other = node
        <4>1. asyncIoQueues[node] \in Seq(Range(asyncIoQueues[node]))
          BY <2>1, <2>2
             DEF AsyncIoQueueContentTypeInvariant, AsyncIoSequenceTyped
        <4>2. asyncIoQueues'[node] =
                 Append(asyncIoQueues[node], AsyncIoControlJob)
          BY <1>1, <2>1, <2>2, FunctionalAppendUpdateAtKey
             DEF EnqueueIoLocalControl, AsyncIoTopologyTypeInvariant
        <4>3. Len(asyncIoQueues'[node]) =
                 Len(asyncIoQueues[node]) + 1
          BY <4>1, <4>2, AppendSequenceFacts
        <4>4. Len(asyncIoQueues[node]) < AsyncIoCapacity
          BY <1>1
             DEF EnqueueIoLocalControl, CanEnqueueIoClass,
                 AsyncIoAdmissionLimit, AsyncIoQueueDepth
        <4>5. Len(asyncIoQueues[node]) \in Nat
          BY <4>1, LenProperties
        <4>6. AsyncIoCapacity \in Nat
          BY <2>1, SMT
             DEF AsyncConfiguration, AsyncIoCapacity
        <4>7. Len(asyncIoQueues'[node]) <= AsyncIoCapacity
          BY <4>3, <4>4, <4>5, <4>6, SMT
        <4>8. AsyncIoQueueDepth(node)' <= AsyncIoCapacity
          BY <4>7 DEF AsyncIoQueueDepth
        <4>9. /\ AsyncQueueDepth(node)' = AsyncQueueDepth(node)
              /\ AsyncCompletionLoad(node)' = AsyncCompletionLoad(node)
          BY <1>1, Isa
             DEF EnqueueIoLocalControl, AsyncDeferredVars,
                 AsyncQueueDepth, AsyncCompletionLoad,
                 AsyncOutstandingWorkCount, QueuedCompletionCount,
                 QueuedCompletionIndices, DeferredCompletionCount
        <4>10. /\ AsyncQueueDepth(node) <= AsyncQueueCapacity
              /\ AsyncCompletionLoad(node) <= AsyncCompletionReserve
              /\ AsyncCompletionLoad(node) <= AsyncIoWorkCapacity
          BY <2>1, <2>2 DEF AsyncIoCapacityTypeInvariant
        <4> QED BY <3>1, <4>8, <4>9, <4>10
      <3>2. CASE other # node
        <4>1. asyncIoQueues'[other] = asyncIoQueues[other]
          BY <1>1, <2>1, <2>3, <3>2,
             FunctionalAppendUpdateAwayFromKey
             DEF EnqueueIoLocalControl, AsyncIoTopologyTypeInvariant
        <4>2. /\ AsyncQueueDepth(other)' = AsyncQueueDepth(other)
              /\ AsyncCompletionLoad(other)' =
                   AsyncCompletionLoad(other)
              /\ AsyncIoQueueDepth(other)' = AsyncIoQueueDepth(other)
          BY <1>1, <4>1, Isa
             DEF EnqueueIoLocalControl, AsyncDeferredVars,
                 AsyncQueueDepth, AsyncIoQueueDepth, AsyncCompletionLoad,
                 AsyncOutstandingWorkCount, QueuedCompletionCount,
                 QueuedCompletionIndices, DeferredCompletionCount
        <4>3. /\ AsyncQueueDepth(other) <= AsyncQueueCapacity
              /\ AsyncCompletionLoad(other) <= AsyncCompletionReserve
              /\ AsyncIoQueueDepth(other) <= AsyncIoCapacity
              /\ AsyncCompletionLoad(other) <= AsyncIoWorkCapacity
          BY <2>1, <2>3 DEF AsyncIoCapacityTypeInvariant
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>3 DEF AsyncIoCapacityTypeInvariant
  <1> QED BY <1>1

THEOREM EnqueueIoControlPreservesNonIoType ==
  \A node \in AsyncCurrentResponsiveVoters:
    AsyncTypeInvariant /\ EnqueueIoLocalControl(node)
      => /\ AsyncRuntimeTypeInvariant'
         /\ AsyncDeferredTypeInvariant'
         /\ AsyncTransportTypeInvariant'
         /\ AsyncIngressTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                EnqueueIoLocalControl(node)
         PROVE /\ AsyncRuntimeTypeInvariant'
               /\ AsyncDeferredTypeInvariant'
               /\ AsyncTransportTypeInvariant'
               /\ AsyncIngressTypeInvariant'
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncDeferredTopologyTypeInvariant
           /\ AsyncDeferredContentTypeInvariant
           /\ AsyncTransportClockTypeInvariant
           /\ AsyncTransportContentTypeInvariant
           /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressCapacityTypeInvariant
           /\ AsyncIngressContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
    <2>2. /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportClockTypeVars
           /\ UNCHANGED AsyncTransportContentTypeVars
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, Isa
         DEF EnqueueIoLocalControl, AsyncRuntimeScalarTypeVars,
             AsyncDeferredVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars, AsyncTransportContentTypeVars,
             AsyncIngressTopologyTypeVars, LeaveCausalQueues, vars
    <2>3. /\ AsyncRuntimeScalarTypeInvariant'
           /\ AsyncCausalTypeInvariant'
           /\ AsyncDeferredTopologyTypeInvariant'
           /\ AsyncDeferredContentTypeInvariant'
           /\ AsyncTransportClockTypeInvariant'
           /\ AsyncTransportContentTypeInvariant'
           /\ AsyncIngressTopologyTypeInvariant'
           /\ AsyncIngressCapacityTypeInvariant'
           /\ AsyncIngressContentTypeInvariant'
      BY <2>1, <2>2, AsyncRuntimeScalarTypeStutter,
         AsyncCausalTypeStutter, AsyncDeferredTopologyTypeStutter,
         AsyncDeferredContentTypeStutter, AsyncTransportClockTypeStutter,
         AsyncTransportContentTypeStutter, AsyncIngressTopologyTypeStutter,
         AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter
    <2> QED BY <2>3
         DEF AsyncRuntimeTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM EnqueueIoControlPreservesSchedulerType ==
  \A node \in AsyncCurrentResponsiveVoters:
    AsyncTypeInvariant /\ EnqueueIoLocalControl(node)
      => AsyncSchedulerTypeInvariant'
BY EnqueueIoControlPreservesTopologyType,
   EnqueueIoControlPreservesContentType,
   EnqueueIoControlPreservesCapacityType,
   EnqueueIoControlPreservesNonIoType, Isa
   DEF AsyncSchedulerTypeInvariant, AsyncIoTypeInvariant

THEOREM ServiceIoWorkerPreservesTopologyType ==
  \A node \in AsyncCurrentResponsiveVoters:
    AsyncTypeInvariant /\ ServiceIoWorker(node)
      => AsyncIoTopologyTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                ServiceIoWorker(node)
         PROVE AsyncIoTopologyTypeInvariant'
    <2>1. AsyncIoTopologyTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant
    <2>2. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>3. asyncIoControlAvailable'
               \in [ValidatorIds -> BOOLEAN]
      BY <1>1, <2>1, <2>2, FunctionalUpdatePreservesType, Isa
         DEF ServiceIoWorker, AsyncIoTopologyTypeInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3, Isa
         DEF ServiceIoWorker, AsyncIoTopologyTypeInvariant,
             AsyncDeferredVars, LeaveCausalQueues
  <1> QED BY <1>1

THEOREM ServiceIoWorkerPreservesContentType ==
  \A node \in AsyncCurrentResponsiveVoters:
    AsyncTypeInvariant /\ ServiceIoWorker(node)
      => AsyncIoContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                ServiceIoWorker(node)
         PROVE AsyncIoContentTypeInvariant'
    <2>1. /\ AsyncConfiguration
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>2. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>3. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
           /\ Len(asyncIoQueues[node]) > 0
           /\ AsyncIoJobTyped(Head(asyncIoQueues[node]))
           /\ AsyncIoConsensusCandidateOwnership(
                node, asyncIoQueues, asyncIoReadyCompletions,
                asyncLocalReadyCompletions)
      BY <1>1, <2>1, <2>2, TypedIoTailFacts
         DEF ServiceIoWorker, AsyncIoQueueDepth,
             AsyncIoQueueContentTypeInvariant
    <2>4. /\ Head(asyncIoQueues[node]) \in
                  SequenceSet(asyncIoQueues[node])
           /\ (Head(asyncIoQueues[node]).class = "Consensus" =>
                 Head(asyncIoQueues[node]).candidate
                   \in asyncOutstandingWork[node])
      <3>1. /\ asyncIoQueues[node]
                      \in Seq(Range(asyncIoQueues[node]))
             /\ asyncIoQueues[node] # <<>>
        BY <2>3, TypedIoTailFacts DEF AsyncIoSequenceTyped
      <3>2. Head(asyncIoQueues[node]) \in
                 Range(asyncIoQueues[node])
        BY <3>1, HeadTailProperties
      <3>3. SequenceSet(asyncIoQueues[node]) =
                 Range(asyncIoQueues[node])
        BY <3>1, RangeEquality DEF SequenceSet
      <3> QED BY <2>1, <2>2, <3>2, <3>3
           DEF AsyncIoQueueContentTypeInvariant
    <2>5. /\ asyncIoQueues'[node] = Tail(asyncIoQueues[node])
           /\ asyncIoReadyCompletions'[node] =
                AsyncIoReadyAfterService(
                  asyncIoQueues[node], asyncIoReadyCompletions[node])
           /\ asyncOutstandingWork' = asyncOutstandingWork
           /\ asyncLocalReadyCompletions' =
                asyncLocalReadyCompletions
           /\ asyncCommandQueues' = asyncCommandQueues
      BY <1>1, <2>1, <2>2, FunctionalTailUpdateAtKey,
         FunctionalAppendUpdateAtKey, Isa
         DEF ServiceIoWorker, AsyncIoReadyAfterService,
             AsyncIoTopologyTypeInvariant, AsyncDeferredVars,
             LeaveCausalQueues, vars
    <2>6. /\ AsyncIoSequenceTyped(asyncIoQueues'[node])
           /\ SequenceSet(asyncIoQueues'[node]) \subseteq
                SequenceSet(asyncIoQueues[node])
      BY <2>3, <2>5, TypedIoTailFacts
    <2>7. \A job \in SequenceSet(asyncIoQueues'[node]):
             job.class = "Consensus" =>
               job.candidate \in asyncOutstandingWork'[node]
      BY <2>1, <2>2, <2>5, <2>6
         DEF AsyncIoQueueContentTypeInvariant
    <2>8. AsyncIoConsensusCandidateOwnership(
             node, asyncIoQueues', asyncIoReadyCompletions',
             asyncLocalReadyCompletions')
      BY <2>1, <2>2, <2>3, <2>5,
         ServiceHeadPreservesConsensusQueueOwnership
         DEF AsyncIoQueueContentTypeInvariant,
             AsyncIoWorkContentTypeInvariant,
             AsyncIoConsensusCandidateOwnership,
             AsyncCompletionSequenceTyped
    <2>9. ASSUME NEW other \in ValidatorIds
           PROVE /\ AsyncIoSequenceTyped(asyncIoQueues'[other])
                 /\ \A job \in SequenceSet(asyncIoQueues'[other]):
                      job.class = "Consensus" =>
                        job.candidate \in asyncOutstandingWork'[other]
                 /\ AsyncIoConsensusCandidateOwnership(
                      other, asyncIoQueues',
                      asyncIoReadyCompletions',
                      asyncLocalReadyCompletions')
      <3>1. CASE other = node
        BY <3>1, <2>6, <2>7, <2>8
      <3>2. CASE other # node
        <4>1. /\ asyncIoQueues'[other] = asyncIoQueues[other]
               /\ asyncIoReadyCompletions'[other] =
                    asyncIoReadyCompletions[other]
               /\ asyncLocalReadyCompletions'[other] =
                    asyncLocalReadyCompletions[other]
               /\ asyncOutstandingWork'[other] =
                    asyncOutstandingWork[other]
          BY <1>1, <2>1, <2>2, <2>9, <3>2,
             FunctionalTailUpdateAwayFromKey,
             FunctionalAppendUpdateAwayFromKey, Isa
             DEF ServiceIoWorker, AsyncIoTopologyTypeInvariant,
                 AsyncDeferredVars, LeaveCausalQueues, vars
        <4> QED BY <2>1, <2>9, <4>1
             DEF AsyncIoQueueContentTypeInvariant,
                 AsyncIoConsensusCandidateOwnership
      <3> QED BY <3>1, <3>2
    <2>10. AsyncIoQueueContentTypeInvariant'
      BY <2>9 DEF AsyncIoQueueContentTypeInvariant
    <2>11. /\ IsFiniteSet(asyncOutstandingWork[node])
            /\ AsyncCompletionSequenceTyped(
                 asyncIoReadyCompletions[node])
            /\ Len(asyncIoReadyCompletions[node]) =
                 Cardinality(SequenceSet(
                   asyncIoReadyCompletions[node]))
            /\ SequenceSet(asyncIoReadyCompletions[node]) \subseteq
                 asyncOutstandingWork[node]
            /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                 SequenceSet(asyncLocalReadyCompletions[node]) = {}
      BY <2>1, <2>2 DEF AsyncIoWorkContentTypeInvariant
    <2>12. /\ AsyncCompletionSequenceTyped(
                  AsyncIoReadyAfterService(
                    asyncIoQueues[node],
                    asyncIoReadyCompletions[node]))
            /\ Len(AsyncIoReadyAfterService(
                   asyncIoQueues[node],
                   asyncIoReadyCompletions[node])) =
                 Cardinality(SequenceSet(
                   AsyncIoReadyAfterService(
                     asyncIoQueues[node],
                     asyncIoReadyCompletions[node])))
            /\ SequenceSet(AsyncIoReadyAfterService(
                   asyncIoQueues[node],
                   asyncIoReadyCompletions[node])) \subseteq
                 asyncOutstandingWork[node]
            /\ SequenceSet(AsyncIoReadyAfterService(
                   asyncIoQueues[node],
                   asyncIoReadyCompletions[node])) \cap
                 SequenceSet(asyncLocalReadyCompletions[node]) = {}
      BY <2>3, <2>4, <2>11,
         ServiceHeadPreservesIoReadyFacts
         DEF AsyncIoConsensusCandidateOwnership
    <2>13. ASSUME NEW other \in ValidatorIds
            PROVE /\ IsFiniteSet(asyncOutstandingWork'[other])
                  /\ \A candidate \in asyncOutstandingWork'[other]:
                       /\ AsyncCandidateTyped(candidate)
                       /\ candidate.class = "Completion"
                       /\ candidate.node = other
                  /\ AsyncCompletionSequenceTyped(
                       asyncIoReadyCompletions'[other])
                  /\ AsyncCompletionSequenceTyped(
                       asyncLocalReadyCompletions'[other])
                  /\ Len(asyncIoReadyCompletions'[other]) =
                       Cardinality(SequenceSet(
                         asyncIoReadyCompletions'[other]))
                  /\ Len(asyncLocalReadyCompletions'[other]) =
                       Cardinality(SequenceSet(
                         asyncLocalReadyCompletions'[other]))
                  /\ SequenceSet(asyncIoReadyCompletions'[other])
                       \subseteq asyncOutstandingWork'[other]
                  /\ SequenceSet(asyncLocalReadyCompletions'[other])
                       \subseteq asyncOutstandingWork'[other]
                  /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                       SequenceSet(
                         asyncLocalReadyCompletions'[other]) = {}
                  /\ SequenceSet(asyncCommandQueues'[other]) \cap
                       asyncOutstandingWork'[other] = {}
      <3>1. CASE other = node
        BY <2>1, <2>2, <2>5, <2>12, <3>1
           DEF AsyncIoWorkContentTypeInvariant
      <3>2. CASE other # node
        <4>1. /\ asyncIoReadyCompletions'[other] =
                    asyncIoReadyCompletions[other]
               /\ asyncLocalReadyCompletions'[other] =
                    asyncLocalReadyCompletions[other]
               /\ asyncOutstandingWork'[other] =
                    asyncOutstandingWork[other]
               /\ asyncCommandQueues'[other] =
                    asyncCommandQueues[other]
          BY <1>1, <2>1, <2>2, <2>5, <2>13, <3>2,
             FunctionalAppendUpdateAwayFromKey, Isa
             DEF ServiceIoWorker, AsyncIoTopologyTypeInvariant,
                 AsyncDeferredVars, LeaveCausalQueues, vars
        <4> QED BY <2>1, <2>13, <4>1
             DEF AsyncIoWorkContentTypeInvariant
      <3> QED BY <3>1, <3>2
    <2>14. AsyncIoWorkContentTypeInvariant'
      BY <2>13 DEF AsyncIoWorkContentTypeInvariant
    <2> QED BY <2>10, <2>14 DEF AsyncIoContentTypeInvariant
  <1> QED BY <1>1

THEOREM ServiceIoWorkerPreservesCapacityType ==
  \A node \in AsyncCurrentResponsiveVoters:
    AsyncTypeInvariant /\ ServiceIoWorker(node)
      => AsyncIoCapacityTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                ServiceIoWorker(node)
         PROVE AsyncIoCapacityTypeInvariant'
    <2>1. /\ AsyncConfiguration
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>2. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>3. /\ asyncCommandQueues' = asyncCommandQueues
           /\ asyncOutstandingWork' = asyncOutstandingWork
           /\ asyncDeferredCompletionQueues' =
                asyncDeferredCompletionQueues
      BY <1>1, Isa
         DEF ServiceIoWorker, AsyncDeferredVars,
             LeaveCausalQueues, vars
    <2>4. \A other \in ValidatorIds:
             /\ AsyncQueueDepth(other)' = AsyncQueueDepth(other)
             /\ AsyncCompletionLoad(other)' =
                  AsyncCompletionLoad(other)
      BY <2>3, Isa
         DEF AsyncQueueDepth, AsyncCompletionLoad,
             AsyncOutstandingWorkCount, QueuedCompletionCount,
             QueuedCompletionIndices, DeferredCompletionCount
    <2>5. ASSUME NEW other \in ValidatorIds
           PROVE AsyncIoQueueDepth(other)' <= AsyncIoCapacity
      <3>1. CASE other = node
        <4>1. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
               /\ Len(asyncIoQueues[node]) > 0
               /\ Len(Tail(asyncIoQueues[node])) =
                    Len(asyncIoQueues[node]) - 1
          BY <1>1, <2>1, <2>2, TypedIoTailFacts
             DEF ServiceIoWorker, AsyncIoQueueDepth,
                 AsyncIoQueueContentTypeInvariant
        <4>2. asyncIoQueues'[node] = Tail(asyncIoQueues[node])
          BY <1>1, <2>1, <2>2, FunctionalTailUpdateAtKey
             DEF ServiceIoWorker, AsyncIoTopologyTypeInvariant
        <4>3. Len(asyncIoQueues[node]) <= AsyncIoCapacity
          BY <2>1, <2>2
             DEF AsyncIoCapacityTypeInvariant, AsyncIoQueueDepth
        <4>4. Len(asyncIoQueues[node]) \in Nat
          BY <4>1, LenProperties DEF AsyncIoSequenceTyped
        <4>5. AsyncIoCapacity \in Nat
          BY <2>1, SMT DEF AsyncConfiguration, AsyncIoCapacity
        <4> QED BY <3>1, <4>1, <4>2, <4>3, <4>4, <4>5, SMT
             DEF AsyncIoQueueDepth
      <3>2. CASE other # node
        <4>1. asyncIoQueues'[other] = asyncIoQueues[other]
          BY <1>1, <2>1, <2>2, <2>5, <3>2,
             FunctionalTailUpdateAwayFromKey
             DEF ServiceIoWorker, AsyncIoTopologyTypeInvariant
        <4>2. AsyncIoQueueDepth(other) <= AsyncIoCapacity
          BY <2>1, <2>5 DEF AsyncIoCapacityTypeInvariant
        <4> QED BY <4>1, <4>2 DEF AsyncIoQueueDepth
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1, <2>4, <2>5
         DEF AsyncIoCapacityTypeInvariant
  <1> QED BY <1>1

THEOREM ServiceIoWorkerPreservesNonIoType ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ StrongInductiveInvariant
    /\ ServiceIoWorker(node)
    => /\ AsyncRuntimeTypeInvariant'
       /\ AsyncDeferredTypeInvariant'
       /\ AsyncTransportTypeInvariant'
       /\ AsyncIngressTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                StrongInductiveInvariant,
                ServiceIoWorker(node)
         PROVE /\ AsyncRuntimeTypeInvariant'
               /\ AsyncDeferredTypeInvariant'
               /\ AsyncTransportTypeInvariant'
               /\ AsyncIngressTypeInvariant'
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncDeferredTopologyTypeInvariant
           /\ AsyncDeferredContentTypeInvariant
           /\ AsyncTransportClockTypeInvariant
           /\ AsyncTransportContentTypeInvariant
           /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressCapacityTypeInvariant
           /\ AsyncIngressContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
    <2>2. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>3. /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, Isa
         DEF ServiceIoWorker, AsyncRuntimeScalarTypeVars,
             AsyncDeferredVars, AsyncDeferredTopologyTypeVars,
             AsyncIngressTopologyTypeVars, LeaveCausalQueues, vars
    <2>4. /\ AsyncRuntimeScalarTypeInvariant'
           /\ AsyncCausalTypeInvariant'
           /\ AsyncDeferredTopologyTypeInvariant'
           /\ AsyncDeferredContentTypeInvariant'
           /\ AsyncIngressTopologyTypeInvariant'
           /\ AsyncIngressCapacityTypeInvariant'
           /\ AsyncIngressContentTypeInvariant'
      BY <2>1, <2>3, AsyncRuntimeScalarTypeStutter,
         AsyncCausalTypeStutter, AsyncDeferredTopologyTypeStutter,
         AsyncDeferredContentTypeStutter,
         AsyncIngressTopologyTypeStutter,
         AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter
    <2>5. /\ AsyncConfiguration
           /\ asyncNow \in Nat
           /\ AsyncDeliveryBound \in Nat
      BY <2>1, SMT
         DEF AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
    <2>6. asyncNow + AsyncDeliveryBound \in Nat
      BY <2>5, SMT
    <2>7. asyncIoServiceDeadlines'
               \in [ValidatorIds -> Nat]
      BY <1>1, <2>1, <2>2, <2>6,
         FunctionalUpdatePreservesType, Isa
         DEF ServiceIoWorker, AsyncTransportClockTypeInvariant
    <2>8. /\ asyncOutstandingTags' = asyncOutstandingTags
           /\ asyncNodeDeadlines' = asyncNodeDeadlines
           /\ asyncRetransmitDeadlines' = asyncRetransmitDeadlines
           /\ asyncNodeServiceDeadlines' = asyncNodeServiceDeadlines
      BY <1>1, Isa DEF ServiceIoWorker, vars
    <2>9. AsyncTransportClockTypeInvariant'
      BY <2>1, <2>7, <2>8
         DEF AsyncTransportClockTypeInvariant
    <2>10. /\ IsFiniteSet(
                    AsyncIoResponseItemsAfterService(asyncIoQueues[node]))
            /\ \A item \in
                   AsyncIoResponseItemsAfterService(asyncIoQueues[node]):
                 AsyncItemTyped(item)
      BY <1>1, ServiceResponseItemsAreFiniteAndTyped
         DEF ServiceIoWorker
    <2>11. /\ PublishEphemeralItems(
                    AsyncIoResponseItemsAfterService(asyncIoQueues[node]))
            /\ UNCHANGED <<context, asyncHeldChunks>>
      BY <1>1
         DEF ServiceIoWorker, AsyncIoResponseItemsAfterService, vars
    <2>12. AsyncTransportContentTypeInvariant'
      BY <2>1, <2>10, <2>11,
         PublishEphemeralItemsPreservesTransportContentType
    <2> QED BY <2>4, <2>9, <2>12
         DEF AsyncRuntimeTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM ServiceIoWorkerPreservesSchedulerType ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ StrongInductiveInvariant
    /\ ServiceIoWorker(node)
    => AsyncSchedulerTypeInvariant'
BY ServiceIoWorkerPreservesTopologyType,
   ServiceIoWorkerPreservesContentType,
   ServiceIoWorkerPreservesCapacityType,
   ServiceIoWorkerPreservesNonIoType, Isa
   DEF AsyncSchedulerTypeInvariant, AsyncIoTypeInvariant

ProducerSelectedReadyQueue(node) ==
  IF SelectedCompletionSource(node) = "Io"
  THEN asyncIoReadyCompletions[node]
  ELSE asyncLocalReadyCompletions[node]

ProducerOtherReadyQueue(node) ==
  IF SelectedCompletionSource(node) = "Io"
  THEN asyncLocalReadyCompletions[node]
  ELSE asyncIoReadyCompletions[node]

THEOREM ProducerSelectedCompletionFacts ==
  \A node \in ValidatorIds:
    /\ AsyncIoTopologyTypeInvariant
    /\ AsyncIoWorkContentTypeInvariant
    /\ ProducerCompletionCanAdmit(node)
    => LET source == SelectedCompletionSource(node)
           queue == ProducerSelectedReadyQueue(node)
           otherQueue == ProducerOtherReadyQueue(node)
           candidate == SelectedCompletionCandidate(node)
       IN /\ source \in {"Io", "Local"}
          /\ AsyncCompletionSequenceTyped(queue)
          /\ Len(queue) = Cardinality(SequenceSet(queue))
          /\ Len(queue) > 0
          /\ candidate = Head(queue)
          /\ candidate \in SequenceSet(queue)
          /\ candidate \in asyncOutstandingWork[node]
          /\ AsyncCandidateTyped(candidate)
          /\ candidate.class = "Completion"
          /\ candidate.node = node
          /\ candidate \notin SequenceSet(otherQueue)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncIoTopologyTypeInvariant,
                AsyncIoWorkContentTypeInvariant,
                ProducerCompletionCanAdmit(node)
         PROVE LET source == SelectedCompletionSource(node)
                   queue == ProducerSelectedReadyQueue(node)
                   otherQueue == ProducerOtherReadyQueue(node)
                   candidate == SelectedCompletionCandidate(node)
               IN /\ source \in {"Io", "Local"}
                  /\ AsyncCompletionSequenceTyped(queue)
                  /\ Len(queue) = Cardinality(SequenceSet(queue))
                  /\ Len(queue) > 0
                  /\ candidate = Head(queue)
                  /\ candidate \in SequenceSet(queue)
                  /\ candidate \in asyncOutstandingWork[node]
                  /\ AsyncCandidateTyped(candidate)
                  /\ candidate.class = "Completion"
                  /\ candidate.node = node
                  /\ candidate \notin SequenceSet(otherQueue)
    <2>1. SelectedCompletionSource(node) \in {"Io", "Local"}
      BY <1>1, SMT
         DEF AsyncIoTopologyTypeInvariant, SelectedCompletionSource
    <2>2. CASE SelectedCompletionSource(node) = "Io"
      <3>1. /\ ProducerSelectedReadyQueue(node) =
                       asyncIoReadyCompletions[node]
             /\ ProducerOtherReadyQueue(node) =
                       asyncLocalReadyCompletions[node]
             /\ SelectedCompletionCandidate(node) =
                       Head(asyncIoReadyCompletions[node])
             /\ Len(asyncIoReadyCompletions[node]) > 0
        BY <1>1, <2>2
           DEF ProducerSelectedReadyQueue, ProducerOtherReadyQueue,
               ProducerCompletionCanAdmit,
               SelectedCompletionQueueNonempty,
               SelectedCompletionCandidate
      <3>2. /\ AsyncCompletionSequenceTyped(
                       asyncIoReadyCompletions[node])
             /\ Len(asyncIoReadyCompletions[node]) =
                  Cardinality(SequenceSet(
                    asyncIoReadyCompletions[node]))
             /\ SequenceSet(asyncIoReadyCompletions[node]) \subseteq
                  asyncOutstandingWork[node]
             /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions[node]) = {}
             /\ \A candidate \in asyncOutstandingWork[node]:
                    /\ AsyncCandidateTyped(candidate)
                    /\ candidate.class = "Completion"
                    /\ candidate.node = node
        BY <1>1 DEF AsyncIoWorkContentTypeInvariant
      <3>3. Head(asyncIoReadyCompletions[node]) \in
                 SequenceSet(asyncIoReadyCompletions[node])
        BY <3>1, <3>2, HeadTailProperties, RangeEquality
           DEF AsyncCompletionSequenceTyped, SequenceSet
      <3> QED BY <2>1, <2>2, <3>1, <3>2, <3>3, SMT
    <2>3. CASE SelectedCompletionSource(node) = "Local"
      <3>1. /\ ProducerSelectedReadyQueue(node) =
                       asyncLocalReadyCompletions[node]
             /\ ProducerOtherReadyQueue(node) =
                       asyncIoReadyCompletions[node]
             /\ SelectedCompletionCandidate(node) =
                       Head(asyncLocalReadyCompletions[node])
             /\ Len(asyncLocalReadyCompletions[node]) > 0
        BY <1>1, <2>3
           DEF ProducerSelectedReadyQueue, ProducerOtherReadyQueue,
               ProducerCompletionCanAdmit,
               SelectedCompletionQueueNonempty,
               SelectedCompletionCandidate
      <3>2. /\ AsyncCompletionSequenceTyped(
                       asyncLocalReadyCompletions[node])
             /\ Len(asyncLocalReadyCompletions[node]) =
                  Cardinality(SequenceSet(
                    asyncLocalReadyCompletions[node]))
             /\ SequenceSet(asyncLocalReadyCompletions[node]) \subseteq
                  asyncOutstandingWork[node]
             /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions[node]) = {}
             /\ \A candidate \in asyncOutstandingWork[node]:
                    /\ AsyncCandidateTyped(candidate)
                    /\ candidate.class = "Completion"
                    /\ candidate.node = node
        BY <1>1 DEF AsyncIoWorkContentTypeInvariant
      <3>3. Head(asyncLocalReadyCompletions[node]) \in
                 SequenceSet(asyncLocalReadyCompletions[node])
        BY <3>1, <3>2, HeadTailProperties, RangeEquality
           DEF AsyncCompletionSequenceTyped, SequenceSet
      <3> QED BY <2>1, <2>3, <3>1, <3>2, <3>3, SMT
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM TypedCandidateAppendPreservesQueueType ==
  \A queue, candidate:
    /\ AsyncQueueTyped(queue)
    /\ AsyncCandidateTyped(candidate)
    => AsyncQueueTyped(Append(queue, candidate))
PROOF
  <1>1. ASSUME NEW queue, NEW candidate,
                AsyncQueueTyped(queue),
                AsyncCandidateTyped(candidate)
         PROVE AsyncQueueTyped(Append(queue, candidate))
    <2>1. queue \in Seq(Range(queue))
      BY <1>1 DEF AsyncQueueTyped
    <2>2. /\ Append(queue, candidate)
                  \in Seq(Range(Append(queue, candidate)))
           /\ DOMAIN Append(queue, candidate) =
                1..Len(Append(queue, candidate))
           /\ Len(Append(queue, candidate)) = Len(queue) + 1
           /\ \A index \in 1..Len(queue):
                  Append(queue, candidate)[index] = queue[index]
           /\ Append(queue, candidate)[Len(queue) + 1] = candidate
      BY <2>1, AppendSequenceFacts, Isa
    <2>3. Len(queue) \in Nat
      BY <2>1, LenProperties
    <2>4. \A index \in 1..Len(Append(queue, candidate)):
             AsyncCandidateTyped(Append(queue, candidate)[index])
      <3>1. ASSUME NEW index \in 1..Len(Append(queue, candidate))
             PROVE AsyncCandidateTyped(
                      Append(queue, candidate)[index])
        <4>1. CASE index \in 1..Len(queue)
          BY <1>1, <2>2, <4>1 DEF AsyncQueueTyped
        <4>2. CASE index \notin 1..Len(queue)
          <5>1. index = Len(queue) + 1
            BY <2>2, <2>3, <3>1, <4>2, SMT
          <5> QED BY <1>1, <2>2, <5>1
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>2, <2>4 DEF AsyncQueueTyped
  <1> QED BY <1>1

AsyncCompletionIndices(queue) ==
  {index \in 1..Len(queue): queue[index].class = "Completion"}

THEOREM CompletionIndicesAfterCompletionAppend ==
  \A queue, candidate:
    /\ AsyncQueueTyped(queue)
    /\ candidate.class = "Completion"
    => AsyncCompletionIndices(Append(queue, candidate)) =
         AsyncCompletionIndices(queue) \cup {Len(queue) + 1}
PROOF
  <1>1. ASSUME NEW queue, NEW candidate,
                AsyncQueueTyped(queue),
                candidate.class = "Completion"
         PROVE AsyncCompletionIndices(Append(queue, candidate)) =
                 AsyncCompletionIndices(queue) \cup {Len(queue) + 1}
    <2>1. queue \in Seq(Range(queue))
      BY <1>1 DEF AsyncQueueTyped
    <2>2. /\ Len(queue) \in Nat
           /\ Len(Append(queue, candidate)) = Len(queue) + 1
           /\ \A index \in 1..Len(queue):
                  Append(queue, candidate)[index] = queue[index]
           /\ Append(queue, candidate)[Len(queue) + 1] = candidate
      BY <2>1, AppendSequenceFacts, LenProperties
    <2> QED BY <1>1, <2>2, SMT DEF AsyncCompletionIndices
  <1> QED BY <1>1

THEOREM CompletionAppendCountIncreasesByOne ==
  \A queue, candidate:
    /\ AsyncQueueTyped(queue)
    /\ candidate.class = "Completion"
    => Cardinality(AsyncCompletionIndices(Append(queue, candidate))) =
         Cardinality(AsyncCompletionIndices(queue)) + 1
PROOF
  <1>1. ASSUME NEW queue, NEW candidate,
                AsyncQueueTyped(queue),
                candidate.class = "Completion"
         PROVE Cardinality(
                   AsyncCompletionIndices(Append(queue, candidate))) =
                 Cardinality(AsyncCompletionIndices(queue)) + 1
    <2>1. queue \in Seq(Range(queue))
      BY <1>1 DEF AsyncQueueTyped
    <2>2. /\ Len(queue) \in Nat
           /\ IsFiniteSet(1..Len(queue))
      BY <2>1, LenProperties, FS_Interval, SMT
    <2>3. /\ AsyncCompletionIndices(queue) \subseteq 1..Len(queue)
           /\ IsFiniteSet(AsyncCompletionIndices(queue))
      BY <2>2, FS_Subset DEF AsyncCompletionIndices
    <2>4. Len(queue) + 1 \notin AsyncCompletionIndices(queue)
      BY <2>2, SMT DEF AsyncCompletionIndices
    <2>5. AsyncCompletionIndices(Append(queue, candidate)) =
             AsyncCompletionIndices(queue) \cup {Len(queue) + 1}
      BY <1>1, CompletionIndicesAfterCompletionAppend
    <2> QED BY <2>3, <2>4, <2>5, FS_AddElement
  <1> QED BY <1>1

THEOREM FinitePacketSetHasOldestSentAt ==
  \A packets:
    /\ IsFiniteSet(packets)
    /\ packets # {}
    /\ \A packet \in packets: packet.sentAt \in Nat
    => \E packet \in packets:
         \A other \in packets: packet.sentAt <= other.sentAt
PROOF
  <1>1. ASSUME NEW packets,
                IsFiniteSet(packets),
                packets # {},
                \A packet \in packets: packet.sentAt \in Nat
         PROVE \E packet \in packets:
                 \A other \in packets: packet.sentAt <= other.sentAt
    <2>1. PICK witness \in packets: TRUE
      BY <1>1, FS_EmptySet, Zenon
    <2>2. witness.sentAt \in Nat
      BY <1>1, <2>1
    <2> DEFINE HasTimestamp(timestamp) ==
           \E packet \in packets: packet.sentAt = timestamp
    <2>3. HasTimestamp(witness.sentAt)
      BY <2>1 DEF HasTimestamp
    <2>4. \E least \in Nat:
             /\ HasTimestamp(least)
             /\ \A prior \in 0..(least - 1): ~HasTimestamp(prior)
      BY <2>2, <2>3, SmallestNatural
    <2>5. PICK least \in Nat:
             /\ HasTimestamp(least)
             /\ \A prior \in 0..(least - 1): ~HasTimestamp(prior)
      BY <2>4
    <2>6. PICK oldest \in packets: oldest.sentAt = least
      BY <2>5 DEF HasTimestamp
    <2>7. \A other \in packets: oldest.sentAt <= other.sentAt
      <3>1. ASSUME NEW other \in packets
             PROVE oldest.sentAt <= other.sentAt
        <4>1. other.sentAt \in Nat
          BY <1>1, <3>1
        <4>2. HasTimestamp(other.sentAt)
          BY <3>1 DEF HasTimestamp
        <4>3. CASE least <= other.sentAt
          BY <2>6, <4>3
        <4>4. CASE least > other.sentAt
          <5>1. other.sentAt \in 0..(least - 1)
            BY <2>5, <4>1, <4>4, SMT
          <5>2. ~HasTimestamp(other.sentAt)
            BY <2>5, <5>1
          <5> QED BY <4>2, <5>2
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>1
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM OldestDueSourcePacketFacts ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncPacketContentTypeInvariant
    /\ DueSourcePackets(recipient, source) # {}
    => LET packet == OldestDueSourcePacket(recipient, source)
       IN /\ packet \in DueSourcePackets(recipient, source)
          /\ AsyncPacketTyped(packet)
          /\ packet.item.envelope.recipient = recipient
          /\ packet.item.source = source
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncPacketContentTypeInvariant,
                DueSourcePackets(recipient, source) # {}
         PROVE LET packet == OldestDueSourcePacket(recipient, source)
               IN /\ packet \in DueSourcePackets(recipient, source)
                  /\ AsyncPacketTyped(packet)
                  /\ packet.item.envelope.recipient = recipient
                  /\ packet.item.source = source
    <2>1. DueSourcePackets(recipient, source) \subseteq asyncTransport
      BY Isa DEF DueSourcePackets
    <2>2. IsFiniteSet(asyncTransport)
      BY <1>1 DEF AsyncPacketContentTypeInvariant
    <2>3. IsFiniteSet(DueSourcePackets(recipient, source))
      BY <2>1, <2>2, FS_Subset
    <2>4. \A packet \in DueSourcePackets(recipient, source):
             /\ AsyncPacketTyped(packet)
             /\ packet.sentAt \in Nat
             /\ packet.item.envelope.recipient = recipient
             /\ packet.item.source = source
      BY <1>1, <2>1, SMT
         DEF AsyncPacketContentTypeInvariant, AsyncPacketTyped,
             DueSourcePackets
    <2>5. \E packet \in DueSourcePackets(recipient, source):
             \A other \in DueSourcePackets(recipient, source):
               packet.sentAt <= other.sentAt
      BY <1>1, <2>3, <2>4, FinitePacketSetHasOldestSentAt
    <2>6. OldestDueSourcePacket(recipient, source)
             \in DueSourcePackets(recipient, source)
      BY <2>5, Zenon DEF OldestDueSourcePacket
    <2> QED BY <2>4, <2>6
  <1> QED BY <1>1

THEOREM TypedIngressAppendPreservesSequence ==
  \A sequence, item:
    /\ sequence \in Seq(Range(sequence))
    /\ DOMAIN sequence = 1..Len(sequence)
    /\ \A index \in 1..Len(sequence): AsyncItemTyped(sequence[index])
    /\ AsyncItemTyped(item)
    => /\ Append(sequence, item)
                \in Seq(Range(Append(sequence, item)))
       /\ DOMAIN Append(sequence, item) = 1..Len(Append(sequence, item))
       /\ \A index \in 1..Len(Append(sequence, item)):
            AsyncItemTyped(Append(sequence, item)[index])
PROOF
  <1>1. ASSUME NEW sequence, NEW item,
                sequence \in Seq(Range(sequence)),
                DOMAIN sequence = 1..Len(sequence),
                \A index \in 1..Len(sequence):
                  AsyncItemTyped(sequence[index]),
                AsyncItemTyped(item)
         PROVE /\ Append(sequence, item)
                       \in Seq(Range(Append(sequence, item)))
               /\ DOMAIN Append(sequence, item) =
                    1..Len(Append(sequence, item))
               /\ \A index \in 1..Len(Append(sequence, item)):
                    AsyncItemTyped(Append(sequence, item)[index])
    <2>1. /\ Append(sequence, item)
                    \in Seq(Range(sequence) \cup {item})
           /\ DOMAIN Append(sequence, item) = 1..(Len(sequence) + 1)
           /\ Len(Append(sequence, item)) = Len(sequence) + 1
           /\ (\A index \in 1..Len(sequence):
                 Append(sequence, item)[index] = sequence[index])
           /\ Append(sequence, item)[Len(sequence) + 1] = item
           /\ Range(Append(sequence, item)) =
                Range(sequence) \cup {item}
      BY <1>1, AppendSequenceFacts
    <2>2. Append(sequence, item)
             \in Seq(Range(Append(sequence, item)))
      BY <2>1, Isa
    <2>3. DOMAIN Append(sequence, item) =
             1..Len(Append(sequence, item))
      BY <2>1, Isa
    <2>4. Len(sequence) \in Nat
      BY <1>1, LenProperties
    <2>5. \A index \in 1..Len(Append(sequence, item)):
             AsyncItemTyped(Append(sequence, item)[index])
      <3>1. ASSUME NEW index \in 1..Len(Append(sequence, item))
             PROVE AsyncItemTyped(Append(sequence, item)[index])
        <4>1. CASE index \in 1..Len(sequence)
          BY <1>1, <2>1, <4>1
        <4>2. CASE index \notin 1..Len(sequence)
          <5>1. index = Len(sequence) + 1
            BY <2>1, <2>4, <3>1, <4>2, SMT
          <5> QED BY <1>1, <2>1, <5>1
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>2, <2>3, <2>5
  <1> QED BY <1>1

THEOREM AdmitHiddenPacketPreservesNonIngressType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant /\ AdmitHiddenPacket(recipient, source)
      => /\ AsyncRuntimeTypeInvariant'
         /\ AsyncIoTypeInvariant'
         /\ AsyncDeferredTypeInvariant'
         /\ AsyncTransportTypeInvariant'
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                AdmitHiddenPacket(recipient, source)
         PROVE /\ AsyncRuntimeTypeInvariant'
               /\ AsyncIoTypeInvariant'
               /\ AsyncDeferredTypeInvariant'
               /\ AsyncTransportTypeInvariant'
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
           /\ AsyncDeferredTopologyTypeInvariant
           /\ AsyncDeferredContentTypeInvariant
           /\ AsyncTransportClockTypeInvariant
           /\ AsyncTransportHistoryTypeInvariant
           /\ AsyncPacketContentTypeInvariant
           /\ AsyncHeldChunksTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncDeferredTypeInvariant, AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant
    <2>2. /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncIoTopologyTypeVars
           /\ UNCHANGED AsyncIoContentTypeVars
           /\ UNCHANGED AsyncIoCapacityTypeVars
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportClockTypeVars
           /\ UNCHANGED AsyncTransportHistoryTypeVars
           /\ UNCHANGED asyncHeldChunks
      BY <1>1, Isa
         DEF AdmitHiddenPacket, AsyncRuntimeScalarTypeVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredVars,
             AsyncDeferredTopologyTypeVars, AsyncTransportClockTypeVars,
             AsyncTransportHistoryTypeVars, LeaveCausalQueues,
             AsyncSchedulerVars, vars
    <2>3. /\ AsyncRuntimeScalarTypeInvariant'
           /\ AsyncCausalTypeInvariant'
           /\ AsyncIoTopologyTypeInvariant'
           /\ AsyncIoContentTypeInvariant'
           /\ AsyncIoCapacityTypeInvariant'
           /\ AsyncDeferredTopologyTypeInvariant'
           /\ AsyncDeferredContentTypeInvariant'
           /\ AsyncTransportClockTypeInvariant'
           /\ AsyncTransportHistoryTypeInvariant'
           /\ AsyncHeldChunksTypeInvariant'
      BY <2>1, <2>2, AsyncRuntimeScalarTypeStutter,
         AsyncCausalTypeStutter, AsyncIoTopologyTypeStutter,
         AsyncIoContentTypeStutter, AsyncIoCapacityTypeStutter,
         AsyncDeferredTopologyTypeStutter,
         AsyncDeferredContentTypeStutter,
         AsyncTransportClockTypeStutter,
         AsyncTransportHistoryTypeStutter, AsyncHeldChunksTypeStutter
    <2>4. asyncTransport' \subseteq asyncTransport
      BY <1>1, Isa DEF AdmitHiddenPacket
    <2>5. /\ IsFiniteSet(asyncTransport')
           /\ \A packet \in asyncTransport': AsyncPacketTyped(packet)
      BY <2>1, <2>4, FS_Subset, SMT
         DEF AsyncPacketContentTypeInvariant
    <2>6. AsyncPacketContentTypeInvariant'
      BY <2>5 DEF AsyncPacketContentTypeInvariant
    <2> QED BY <2>3, <2>6
         DEF AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncDeferredTypeInvariant, AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant
  <1> QED BY <1>1

THEOREM AdmitHiddenPacketPreservesIngressContentType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant /\ AdmitHiddenPacket(recipient, source)
      => AsyncIngressContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                AdmitHiddenPacket(recipient, source)
         PROVE AsyncIngressContentTypeInvariant'
    <2>1. /\ AsyncPacketContentTypeInvariant
           /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressContentTypeInvariant
           /\ DueSourcePackets(recipient, source) # {}
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant, AsyncTransportContentTypeInvariant,
             AsyncIngressTypeInvariant, AdmitHiddenPacket
    <2>2. /\ AsyncPacketTyped(
                    OldestDueSourcePacket(recipient, source))
           /\ AsyncItemTyped(
                OldestDueSourcePacket(recipient, source).item)
      BY <1>1, <2>1, OldestDueSourcePacketFacts
         DEF AsyncPacketTyped
    <2>3. ASSUME NEW otherRecipient \in ValidatorIds
           PROVE \A otherSource \in AsyncIngressSources:
                   /\ asyncIngressLanes'[otherRecipient][otherSource]
                        \in Seq(Range(
                             asyncIngressLanes'[otherRecipient][otherSource]))
                   /\ DOMAIN asyncIngressLanes'[otherRecipient][otherSource] =
                        1..Len(asyncIngressLanes'[otherRecipient][otherSource])
                   /\ \A index \in
                          1..Len(
                            asyncIngressLanes'[otherRecipient][otherSource]):
                        AsyncItemTyped(
                          asyncIngressLanes'[otherRecipient][otherSource][index])
      <3>1. ASSUME NEW otherSource \in AsyncIngressSources
             PROVE /\ asyncIngressLanes'[otherRecipient][otherSource]
                          \in Seq(Range(
                               asyncIngressLanes'[otherRecipient][otherSource]))
                   /\ DOMAIN asyncIngressLanes'[otherRecipient][otherSource] =
                          1..Len(
                            asyncIngressLanes'[otherRecipient][otherSource])
                   /\ \A index \in
                            1..Len(
                              asyncIngressLanes'[otherRecipient][otherSource]):
                          AsyncItemTyped(
                            asyncIngressLanes'[otherRecipient][otherSource][index])
        <4>1. CASE otherRecipient = recipient /\ otherSource = source
          <5>1. asyncIngressLanes'[otherRecipient][otherSource] =
                   Append(IngressLane(otherRecipient, otherSource),
                          OldestDueSourcePacket(recipient, source).item)
            BY <1>1, <2>1, <3>1, <4>1, Isa
               DEF AdmitHiddenPacket, IngressLane
          <5>2. /\ IngressLane(otherRecipient, otherSource)
                         \in Seq(Range(
                              IngressLane(otherRecipient, otherSource)))
                 /\ DOMAIN IngressLane(otherRecipient, otherSource) =
                      1..Len(IngressLane(otherRecipient, otherSource))
                 /\ \A index \in
                        1..Len(IngressLane(otherRecipient, otherSource)):
                      AsyncItemTyped(
                        IngressLane(otherRecipient, otherSource)[index])
            BY <2>1, <3>1
               DEF AsyncIngressContentTypeInvariant, IngressLaneDepth
          <5>3. AsyncItemTyped(
                   OldestDueSourcePacket(recipient, source).item)
            BY <2>2
          <5>4. /\ Append(IngressLane(otherRecipient, otherSource),
                              OldestDueSourcePacket(recipient, source).item)
                           \in Seq(Range(
                                Append(IngressLane(otherRecipient, otherSource),
                                  OldestDueSourcePacket(recipient, source).item)))
                 /\ DOMAIN Append(IngressLane(otherRecipient, otherSource),
                                   OldestDueSourcePacket(recipient, source).item)
                        = 1..Len(
                            Append(IngressLane(otherRecipient, otherSource),
                              OldestDueSourcePacket(recipient, source).item))
                 /\ \A index \in
                        1..Len(Append(
                          IngressLane(otherRecipient, otherSource),
                          OldestDueSourcePacket(recipient, source).item)):
                      AsyncItemTyped(
                        Append(IngressLane(otherRecipient, otherSource),
                          OldestDueSourcePacket(recipient, source).item)[index])
            BY <5>2, <5>3, TypedIngressAppendPreservesSequence
          <5> QED BY <5>1, <5>4 DEF IngressLaneDepth
        <4>2. CASE ~(otherRecipient = recipient /\ otherSource = source)
          <5>1. asyncIngressLanes'[otherRecipient][otherSource] =
                   IngressLane(otherRecipient, otherSource)
            BY <1>1, <2>1, <3>1, <4>2, Isa
               DEF AdmitHiddenPacket, IngressLane
          <5> QED BY <2>1, <3>1, <5>1
               DEF AsyncIngressContentTypeInvariant, IngressLaneDepth
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>3
         DEF AsyncIngressContentTypeInvariant,
             IngressLaneDepth, IngressLane
  <1> QED BY <1>1

AsyncIngressPairIndicesFor(lanes, recipient) ==
  {pair \in AsyncIngressSources \X (1..AsyncIngressCapacity):
     pair[2] <= Len(lanes[recipient][pair[1]])}

AsyncIngressDepthFor(lanes, recipient) ==
  Cardinality(AsyncIngressPairIndicesFor(lanes, recipient))

AsyncIngressZeroSourcesFor(lanes, recipient) ==
  {source \in AsyncIngressSources:
     Len(lanes[recipient][source]) = 0}

AsyncIngressNonemptySourcesFor(lanes, recipient) ==
  {source \in AsyncIngressSources:
     Len(lanes[recipient][source]) > 0}

THEOREM NestedIngressAppendLaneFacts ==
  \A lanes, item:
   \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ DOMAIN lanes = ValidatorIds
    /\ \A otherRecipient \in ValidatorIds:
         /\ DOMAIN lanes[otherRecipient] = AsyncIngressSources
         /\ \A otherSource \in AsyncIngressSources:
              lanes[otherRecipient][otherSource]
                \in Seq(Range(lanes[otherRecipient][otherSource]))
    => LET next ==
             [lanes EXCEPT ![recipient][source] = Append(@, item)]
       IN /\ DOMAIN next = ValidatorIds
          /\ \A otherRecipient \in ValidatorIds:
               /\ DOMAIN next[otherRecipient] = AsyncIngressSources
               /\ \A otherSource \in AsyncIngressSources:
                    /\ IF otherRecipient = recipient
                           /\ otherSource = source
                       THEN next[otherRecipient][otherSource] =
                              Append(lanes[otherRecipient][otherSource], item)
                       ELSE next[otherRecipient][otherSource] =
                              lanes[otherRecipient][otherSource]
                    /\ Len(next[otherRecipient][otherSource]) =
                         IF otherRecipient = recipient
                              /\ otherSource = source
                         THEN Len(lanes[otherRecipient][otherSource]) + 1
                         ELSE Len(lanes[otherRecipient][otherSource])
PROOF
  <1>1. ASSUME NEW lanes,
                NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                NEW item,
                DOMAIN lanes = ValidatorIds,
                \A otherRecipient \in ValidatorIds:
                  /\ DOMAIN lanes[otherRecipient] = AsyncIngressSources
                  /\ \A otherSource \in AsyncIngressSources:
                       lanes[otherRecipient][otherSource]
                         \in Seq(Range(lanes[otherRecipient][otherSource]))
         PROVE LET next ==
                       [lanes EXCEPT
                          ![recipient][source] = Append(@, item)]
               IN /\ DOMAIN next = ValidatorIds
                  /\ \A otherRecipient \in ValidatorIds:
                       /\ DOMAIN next[otherRecipient] = AsyncIngressSources
                       /\ \A otherSource \in AsyncIngressSources:
                            /\ IF otherRecipient = recipient
                                   /\ otherSource = source
                               THEN next[otherRecipient][otherSource] =
                                      Append(
                                        lanes[otherRecipient][otherSource],
                                        item)
                               ELSE next[otherRecipient][otherSource] =
                                      lanes[otherRecipient][otherSource]
                            /\ Len(next[otherRecipient][otherSource]) =
                                 IF otherRecipient = recipient
                                      /\ otherSource = source
                                 THEN Len(
                                        lanes[otherRecipient][otherSource]) + 1
                                 ELSE Len(
                                        lanes[otherRecipient][otherSource])
    <2> DEFINE next ==
           [lanes EXCEPT ![recipient][source] = Append(@, item)]
    <2>1. \A otherRecipient \in ValidatorIds,
                otherSource \in AsyncIngressSources:
             Len(lanes[otherRecipient][otherSource]) \in Nat
      BY <1>1, LenProperties
    <2>2. \A otherRecipient \in ValidatorIds,
                otherSource \in AsyncIngressSources:
             Len(Append(lanes[otherRecipient][otherSource], item)) =
               Len(lanes[otherRecipient][otherSource]) + 1
      BY <1>1, AppendSequenceFacts
    <2>3. DOMAIN next = ValidatorIds
      BY <1>1, Isa DEF next
    <2>4. \A otherRecipient \in ValidatorIds:
             DOMAIN next[otherRecipient] = AsyncIngressSources
      <3>1. ASSUME NEW otherRecipient \in ValidatorIds
             PROVE DOMAIN next[otherRecipient] = AsyncIngressSources
        <4>1. CASE otherRecipient = recipient
          <5>1. next[otherRecipient] =
                   [lanes[otherRecipient] EXCEPT
                      ![source] = Append(@, item)]
            BY <1>1, <3>1, <4>1, Isa DEF next
          <5> QED BY <1>1, <3>1, <5>1, Isa
        <4>2. CASE otherRecipient # recipient
          <5>1. next[otherRecipient] = lanes[otherRecipient]
            BY <1>1, <3>1, <4>2, Isa DEF next
          <5> QED BY <1>1, <3>1, <5>1
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>5. \A otherRecipient \in ValidatorIds,
                otherSource \in AsyncIngressSources:
             IF otherRecipient = recipient /\ otherSource = source
             THEN next[otherRecipient][otherSource] =
                    Append(lanes[otherRecipient][otherSource], item)
             ELSE next[otherRecipient][otherSource] =
                    lanes[otherRecipient][otherSource]
      <3>1. ASSUME NEW otherRecipient \in ValidatorIds,
                     NEW otherSource \in AsyncIngressSources
             PROVE IF otherRecipient = recipient /\ otherSource = source
                   THEN next[otherRecipient][otherSource] =
                          Append(lanes[otherRecipient][otherSource], item)
                   ELSE next[otherRecipient][otherSource] =
                          lanes[otherRecipient][otherSource]
        <4>1. CASE otherRecipient = recipient /\ otherSource = source
          BY <1>1, <3>1, <4>1, Isa DEF next
        <4>2. CASE ~(otherRecipient = recipient /\ otherSource = source)
          <5>1. CASE otherRecipient # recipient
            BY <1>1, <3>1, <5>1, Isa DEF next
          <5>2. CASE otherRecipient = recipient /\ otherSource # source
            BY <1>1, <3>1, <5>2, Isa DEF next
          <5> QED BY <4>2, <5>1, <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>6. \A otherRecipient \in ValidatorIds,
                otherSource \in AsyncIngressSources:
             Len(next[otherRecipient][otherSource]) =
               IF otherRecipient = recipient /\ otherSource = source
               THEN Len(lanes[otherRecipient][otherSource]) + 1
               ELSE Len(lanes[otherRecipient][otherSource])
      <3>1. ASSUME NEW otherRecipient \in ValidatorIds,
                     NEW otherSource \in AsyncIngressSources
             PROVE Len(next[otherRecipient][otherSource]) =
                     IF otherRecipient = recipient /\ otherSource = source
                     THEN Len(lanes[otherRecipient][otherSource]) + 1
                     ELSE Len(lanes[otherRecipient][otherSource])
        <4>1. CASE otherRecipient = recipient /\ otherSource = source
          BY <2>2, <2>5, <3>1, <4>1
        <4>2. CASE ~(otherRecipient = recipient /\ otherSource = source)
          BY <2>5, <3>1, <4>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>3, <2>4, <2>5, <2>6 DEF next
  <1> QED BY <1>1

THEOREM NestedIngressAppendSourceSetFacts ==
  \A lanes, item:
   \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ DOMAIN lanes = ValidatorIds
    /\ \A otherRecipient \in ValidatorIds:
         /\ DOMAIN lanes[otherRecipient] = AsyncIngressSources
         /\ \A otherSource \in AsyncIngressSources:
              lanes[otherRecipient][otherSource]
                \in Seq(Range(lanes[otherRecipient][otherSource]))
    => LET next ==
             [lanes EXCEPT ![recipient][source] = Append(@, item)]
       IN /\ AsyncIngressNonemptySourcesFor(next, recipient) =
                AsyncIngressNonemptySourcesFor(lanes, recipient)
                  \cup {source}
          /\ AsyncIngressZeroSourcesFor(next, recipient) =
               {other \in AsyncIngressSources \ {source}:
                  Len(lanes[recipient][other]) = 0}
          /\ \A otherRecipient \in ValidatorIds \ {recipient}:
               /\ AsyncIngressNonemptySourcesFor(next, otherRecipient) =
                    AsyncIngressNonemptySourcesFor(lanes, otherRecipient)
               /\ AsyncIngressZeroSourcesFor(next, otherRecipient) =
                    AsyncIngressZeroSourcesFor(lanes, otherRecipient)
PROOF
  <1>1. ASSUME NEW lanes,
                NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                NEW item,
                DOMAIN lanes = ValidatorIds,
                \A otherRecipient \in ValidatorIds:
                  /\ DOMAIN lanes[otherRecipient] = AsyncIngressSources
                  /\ \A otherSource \in AsyncIngressSources:
                       lanes[otherRecipient][otherSource]
                         \in Seq(Range(lanes[otherRecipient][otherSource]))
         PROVE LET next ==
                       [lanes EXCEPT
                          ![recipient][source] = Append(@, item)]
               IN /\ AsyncIngressNonemptySourcesFor(next, recipient) =
                        AsyncIngressNonemptySourcesFor(lanes, recipient)
                          \cup {source}
                  /\ AsyncIngressZeroSourcesFor(next, recipient) =
                       {other \in AsyncIngressSources \ {source}:
                          Len(lanes[recipient][other]) = 0}
                  /\ \A otherRecipient \in
                           ValidatorIds \ {recipient}:
                       /\ AsyncIngressNonemptySourcesFor(
                            next, otherRecipient) =
                            AsyncIngressNonemptySourcesFor(
                              lanes, otherRecipient)
                       /\ AsyncIngressZeroSourcesFor(next, otherRecipient) =
                            AsyncIngressZeroSourcesFor(
                              lanes, otherRecipient)
    <2>1. LET next ==
                   [lanes EXCEPT
                      ![recipient][source] = Append(@, item)]
           IN /\ \A otherSource \in AsyncIngressSources:
                    /\ Len(next[recipient][otherSource]) =
                         IF otherSource = source
                         THEN Len(lanes[recipient][otherSource]) + 1
                         ELSE Len(lanes[recipient][otherSource])
                 /\ \A otherRecipient \in ValidatorIds \ {recipient},
                       otherSource \in AsyncIngressSources:
                      Len(next[otherRecipient][otherSource]) =
                        Len(lanes[otherRecipient][otherSource])
      BY <1>1, NestedIngressAppendLaneFacts, SMT
    <2>2. \A otherSource \in AsyncIngressSources:
             Len(lanes[recipient][otherSource]) \in Nat
      BY <1>1, LenProperties
    <2> QED BY <1>1, <2>1, <2>2, SMT
         DEF AsyncIngressNonemptySourcesFor,
             AsyncIngressZeroSourcesFor
  <1> QED BY <1>1

THEOREM NestedIngressAppendPairSetFacts ==
  \A lanes, item:
   \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncIngressCapacity \in Nat
    /\ DOMAIN lanes = ValidatorIds
    /\ \A otherRecipient \in ValidatorIds:
         /\ DOMAIN lanes[otherRecipient] = AsyncIngressSources
         /\ \A otherSource \in AsyncIngressSources:
              lanes[otherRecipient][otherSource]
                \in Seq(Range(lanes[otherRecipient][otherSource]))
    => LET next ==
             [lanes EXCEPT ![recipient][source] = Append(@, item)]
           nextIndex == <<source, Len(lanes[recipient][source]) + 1>>
       IN /\ AsyncIngressPairIndicesFor(next, recipient) =
                IF Len(lanes[recipient][source]) < AsyncIngressCapacity
                THEN AsyncIngressPairIndicesFor(lanes, recipient)
                       \cup {nextIndex}
                ELSE AsyncIngressPairIndicesFor(lanes, recipient)
          /\ \A otherRecipient \in ValidatorIds \ {recipient}:
               AsyncIngressPairIndicesFor(next, otherRecipient) =
                 AsyncIngressPairIndicesFor(lanes, otherRecipient)
PROOF
  <1>1. ASSUME NEW lanes,
                NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                NEW item,
                AsyncIngressCapacity \in Nat,
                DOMAIN lanes = ValidatorIds,
                \A otherRecipient \in ValidatorIds:
                  /\ DOMAIN lanes[otherRecipient] = AsyncIngressSources
                  /\ \A otherSource \in AsyncIngressSources:
                       lanes[otherRecipient][otherSource]
                         \in Seq(Range(lanes[otherRecipient][otherSource]))
         PROVE LET next ==
                       [lanes EXCEPT
                          ![recipient][source] = Append(@, item)]
                     nextIndex ==
                       <<source, Len(lanes[recipient][source]) + 1>>
               IN /\ AsyncIngressPairIndicesFor(next, recipient) =
                        IF Len(lanes[recipient][source]) <
                             AsyncIngressCapacity
                        THEN AsyncIngressPairIndicesFor(lanes, recipient)
                               \cup {nextIndex}
                        ELSE AsyncIngressPairIndicesFor(lanes, recipient)
                  /\ \A otherRecipient \in
                           ValidatorIds \ {recipient}:
                       AsyncIngressPairIndicesFor(next, otherRecipient) =
                         AsyncIngressPairIndicesFor(lanes, otherRecipient)
    <2>1. LET next ==
                   [lanes EXCEPT
                      ![recipient][source] = Append(@, item)]
           IN /\ \A otherSource \in AsyncIngressSources:
                    /\ Len(next[recipient][otherSource]) =
                         IF otherSource = source
                         THEN Len(lanes[recipient][otherSource]) + 1
                         ELSE Len(lanes[recipient][otherSource])
                 /\ \A otherRecipient \in ValidatorIds \ {recipient},
                       otherSource \in AsyncIngressSources:
                      Len(next[otherRecipient][otherSource]) =
                        Len(lanes[otherRecipient][otherSource])
      BY <1>1, NestedIngressAppendLaneFacts, SMT
    <2>2. \A otherSource \in AsyncIngressSources:
             Len(lanes[recipient][otherSource]) \in Nat
      BY <1>1, LenProperties
    <2> QED BY <1>1, <2>1, <2>2, SMT
         DEF AsyncIngressPairIndicesFor
  <1> QED BY <1>1

THEOREM AdmitHiddenPacketPreservesIngressTopologyType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant /\ AdmitHiddenPacket(recipient, source)
      => AsyncIngressTopologyTypeInvariant'
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                AdmitHiddenPacket(recipient, source)
         PROVE AsyncIngressTopologyTypeInvariant'
    <2>1. /\ AsyncConfiguration
           /\ ModelConfiguration
           /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, TypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncIngressTypeInvariant
    <2>2. /\ DOMAIN asyncIngressLanes' = ValidatorIds
           /\ \A otherRecipient \in ValidatorIds:
                DOMAIN asyncIngressLanes'[otherRecipient] =
                  AsyncIngressSources
      BY <1>1, <2>1, NestedIngressAppendLaneFacts
         DEF AdmitHiddenPacket, AsyncIngressTopologyTypeInvariant,
             AsyncIngressContentTypeInvariant, IngressLane
    <2>3. /\ AsyncIngressNonemptySourcesFor(
                    asyncIngressLanes', recipient) =
                  AsyncIngressNonemptySourcesFor(
                    asyncIngressLanes, recipient) \cup {source}
           /\ \A otherRecipient \in ValidatorIds \ {recipient}:
                AsyncIngressNonemptySourcesFor(
                  asyncIngressLanes', otherRecipient) =
                AsyncIngressNonemptySourcesFor(
                  asyncIngressLanes, otherRecipient)
      BY <1>1, <2>1, NestedIngressAppendSourceSetFacts
         DEF AdmitHiddenPacket, AsyncIngressTopologyTypeInvariant,
             AsyncIngressContentTypeInvariant, IngressLane
    <2>4. DOMAIN asyncIngressReady' = ValidatorIds
      BY <1>1, <2>1, Isa
         DEF AdmitHiddenPacket, AsyncIngressTopologyTypeInvariant
    <2>5. IsFiniteSet(AsyncIngressSources)
      BY <2>1, AsyncIngressSourcesAreFinite
    <2>6. ASSUME NEW otherRecipient \in ValidatorIds
           PROVE /\ DOMAIN asyncIngressReady'[otherRecipient] =
                        1..Len(asyncIngressReady'[otherRecipient])
                 /\ asyncIngressReady'[otherRecipient]
                      \in Seq(Range(asyncIngressReady'[otherRecipient]))
                 /\ SequenceSet(asyncIngressReady'[otherRecipient])
                      \subseteq AsyncIngressSources
                 /\ Len(asyncIngressReady'[otherRecipient]) =
                      Cardinality(
                        SequenceSet(asyncIngressReady'[otherRecipient]))
                 /\ SequenceSet(asyncIngressReady'[otherRecipient]) =
                      AsyncIngressNonemptySourcesFor(
                        asyncIngressLanes', otherRecipient)
      <3>1. /\ DOMAIN asyncIngressReady[otherRecipient] =
                       1..Len(asyncIngressReady[otherRecipient])
              /\ asyncIngressReady[otherRecipient]
                   \in Seq(Range(asyncIngressReady[otherRecipient]))
              /\ SequenceSet(asyncIngressReady[otherRecipient])
                   \subseteq AsyncIngressSources
              /\ Len(asyncIngressReady[otherRecipient]) =
                   Cardinality(
                     SequenceSet(asyncIngressReady[otherRecipient]))
              /\ SequenceSet(asyncIngressReady[otherRecipient]) =
                   AsyncIngressNonemptySourcesFor(
                     asyncIngressLanes, otherRecipient)
        BY <2>1, <2>6
           DEF AsyncIngressTopologyTypeInvariant,
               AsyncIngressNonemptySourcesFor,
               IngressLaneDepth, IngressLane
      <3>2. CASE otherRecipient = recipient
        <4>1. CASE Len(IngressLane(recipient, source)) = 0
          <5>1. asyncIngressReady'[otherRecipient] =
                   Append(asyncIngressReady[otherRecipient], source)
            BY <1>1, <2>6, <3>2, <4>1
               DEF AdmitHiddenPacket
          <5>2. /\ DOMAIN Append(asyncIngressReady[otherRecipient], source) =
                         1..(Len(asyncIngressReady[otherRecipient]) + 1)
                 /\ Len(Append(asyncIngressReady[otherRecipient], source)) =
                         Len(asyncIngressReady[otherRecipient]) + 1
                 /\ Append(asyncIngressReady[otherRecipient], source)
                         \in Seq(Range(asyncIngressReady[otherRecipient])
                                  \cup {source})
            BY <3>1, AppendSequenceFacts
          <5>3. /\ Append(asyncIngressReady[otherRecipient], source)
                           \in Seq(Range(
                                Append(asyncIngressReady[otherRecipient],
                                       source)))
                 /\ DOMAIN Append(asyncIngressReady[otherRecipient], source) =
                           1..Len(Append(
                                asyncIngressReady[otherRecipient], source))
                 /\ SequenceSet(
                        Append(asyncIngressReady[otherRecipient], source)) =
                           SequenceSet(asyncIngressReady[otherRecipient])
                             \cup {source}
            BY <3>1, <5>2, SequenceSetAfterAppend,
               SeqOfRange, LenProperties, Isa
          <5>4. source \notin
                   SequenceSet(asyncIngressReady[otherRecipient])
            BY <3>1, <3>2, <4>1, SMT
               DEF AsyncIngressNonemptySourcesFor, IngressLane
          <5>5. IsFiniteSet(
                   SequenceSet(asyncIngressReady[otherRecipient]))
            BY <2>5, <3>1, FS_Subset
          <5>6. Cardinality(
                   SequenceSet(asyncIngressReady[otherRecipient])
                     \cup {source}) =
                   Cardinality(
                     SequenceSet(asyncIngressReady[otherRecipient])) + 1
            BY <5>4, <5>5, FS_AddElement
          <5>7. Len(Append(asyncIngressReady[otherRecipient], source)) =
                   Cardinality(SequenceSet(
                     Append(asyncIngressReady[otherRecipient], source)))
            BY <3>1, <5>2, <5>3, <5>6, SMT
          <5>8. SequenceSet(
                   Append(asyncIngressReady[otherRecipient], source)) =
                 AsyncIngressNonemptySourcesFor(
                   asyncIngressLanes', otherRecipient)
            BY <2>3, <2>6, <3>1, <3>2, <5>3
          <5>9. SequenceSet(
                   Append(asyncIngressReady[otherRecipient], source))
                     \subseteq AsyncIngressSources
            BY <1>1, <3>1, <5>3, Isa
          <5> QED BY <5>1, <5>2, <5>3, <5>7, <5>8, <5>9
        <4>2. CASE Len(IngressLane(recipient, source)) # 0
          <5>1. Len(IngressLane(recipient, source)) \in Nat
            BY <2>1, <1>1, LenProperties
               DEF AsyncIngressContentTypeInvariant, IngressLane
          <5>2. source \in
                   AsyncIngressNonemptySourcesFor(
                     asyncIngressLanes, otherRecipient)
            BY <2>6, <3>2, <4>2, <5>1, SMT
               DEF AsyncIngressNonemptySourcesFor, IngressLane
          <5>3. AsyncIngressNonemptySourcesFor(
                    asyncIngressLanes, otherRecipient) \cup {source} =
                  AsyncIngressNonemptySourcesFor(
                    asyncIngressLanes, otherRecipient)
            BY <5>2, Isa
          <5>4. /\ asyncIngressReady'[otherRecipient] =
                        asyncIngressReady[otherRecipient]
                 /\ AsyncIngressNonemptySourcesFor(
                       asyncIngressLanes', otherRecipient) =
                        AsyncIngressNonemptySourcesFor(
                          asyncIngressLanes, otherRecipient)
            BY <1>1, <2>3, <2>6, <3>2, <4>2, <5>3
               DEF AdmitHiddenPacket
          <5> QED BY <3>1, <5>4
        <4> QED BY <4>1, <4>2
      <3>3. CASE otherRecipient # recipient
        <4>1. /\ asyncIngressReady'[otherRecipient] =
                        asyncIngressReady[otherRecipient]
               /\ AsyncIngressNonemptySourcesFor(
                     asyncIngressLanes', otherRecipient) =
                        AsyncIngressNonemptySourcesFor(
                          asyncIngressLanes, otherRecipient)
          BY <1>1, <2>3, <2>6, <3>3, Isa
             DEF AdmitHiddenPacket,
                 AsyncIngressTopologyTypeInvariant
        <4> QED BY <3>1, <4>1
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>2, <2>4, <2>6
         DEF AsyncIngressTopologyTypeInvariant,
             AsyncIngressNonemptySourcesFor,
             IngressLaneDepth, IngressLane
  <1> QED BY <1>1

THEOREM NaturalReservedCapacityStep ==
  \A used \in Nat, reserved \in Nat, capacity \in Nat:
    used < capacity - reserved
      => used + 1 + reserved <= capacity
BY SMT

THEOREM NaturalSumBoundProjectsLeft ==
  \A left \in Nat, right \in Nat, capacity \in Nat:
    left + right <= capacity => left <= capacity
BY SMT

THEOREM IfElseWhenFalse ==
  \A condition, thenValue, elseValue:
    ~condition => (IF condition THEN thenValue ELSE elseValue) = elseValue
BY SMT

THEOREM NaturalGreaterOrEqualIsNotLess ==
  \A left \in Nat, right \in Nat:
    left >= right => ~(left < right)
BY SMT

THEOREM NextIngressIndexIsFresh ==
  \A lanes, recipient, source:
    lanes[recipient][source]
      \in Seq(Range(lanes[recipient][source]))
      => <<source, Len(lanes[recipient][source]) + 1>>
           \notin AsyncIngressPairIndicesFor(lanes, recipient)
PROOF
  <1>1. ASSUME NEW lanes, NEW recipient, NEW source,
                lanes[recipient][source]
                  \in Seq(Range(lanes[recipient][source]))
         PROVE <<source, Len(lanes[recipient][source]) + 1>>
                 \notin AsyncIngressPairIndicesFor(lanes, recipient)
    <2>1. Len(lanes[recipient][source]) \in Nat
      BY <1>1, LenProperties
    <2> QED BY <2>1, SMT DEF AsyncIngressPairIndicesFor
  <1> QED BY <1>1

THEOREM AdmitHiddenPacketPreservesIngressCapacityType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant /\ AdmitHiddenPacket(recipient, source)
      => AsyncIngressCapacityTypeInvariant'
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                AdmitHiddenPacket(recipient, source)
         PROVE AsyncIngressCapacityTypeInvariant'
    <2>1. /\ AsyncConfiguration
           /\ ModelConfiguration
           /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressCapacityTypeInvariant
           /\ AsyncIngressContentTypeInvariant
           /\ AsyncPacketContentTypeInvariant
           /\ DueSourcePackets(recipient, source) # {}
      BY <1>1
         DEF AsyncTypeInvariant, TypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant,
             AsyncTransportTypeInvariant, AsyncTransportContentTypeInvariant,
             AsyncIngressTypeInvariant, AdmitHiddenPacket
    <2>2. LET packet == OldestDueSourcePacket(recipient, source)
           IN /\ packet.item.envelope.recipient = recipient
              /\ packet.item.source = source
      BY <1>1, <2>1, OldestDueSourcePacketFacts
    <2>3. IngressDepth(recipient) <
             IngressUsableCapacity(recipient, source)
      BY <1>1, <2>2 DEF AdmitHiddenPacket, CanAdmitIngressItem
    <2>4. /\ AsyncIngressZeroSourcesFor(
                    asyncIngressLanes', recipient) =
                  EmptyOtherIngressLanes(recipient, source)
           /\ \A otherRecipient \in ValidatorIds \ {recipient}:
                AsyncIngressZeroSourcesFor(
                  asyncIngressLanes', otherRecipient) =
                AsyncIngressZeroSourcesFor(
                  asyncIngressLanes, otherRecipient)
      BY <1>1, <2>1, NestedIngressAppendSourceSetFacts
         DEF AdmitHiddenPacket, AsyncIngressTopologyTypeInvariant,
             AsyncIngressContentTypeInvariant,
             AsyncIngressZeroSourcesFor, EmptyOtherIngressLanes,
             IngressLaneDepth, IngressLane
    <2>5. LET nextIndex ==
                  <<source, IngressLaneDepth(recipient, source) + 1>>
           IN /\ AsyncIngressPairIndicesFor(
                       asyncIngressLanes', recipient) =
                    IF IngressLaneDepth(recipient, source) <
                         AsyncIngressCapacity
                    THEN AsyncIngressPairIndicesFor(
                           asyncIngressLanes, recipient) \cup {nextIndex}
                    ELSE AsyncIngressPairIndicesFor(
                           asyncIngressLanes, recipient)
              /\ \A otherRecipient \in ValidatorIds \ {recipient}:
                   AsyncIngressPairIndicesFor(
                     asyncIngressLanes', otherRecipient) =
                   AsyncIngressPairIndicesFor(
                     asyncIngressLanes, otherRecipient)
      BY <1>1, <2>1, NestedIngressAppendPairSetFacts
         DEF AdmitHiddenPacket, AsyncConfiguration,
             AsyncIngressTopologyTypeInvariant,
             AsyncIngressContentTypeInvariant,
             IngressLaneDepth, IngressLane
    <2>6. /\ IsFiniteSet(AsyncIngressSources)
           /\ AsyncIngressCapacity \in Nat \ {0}
      BY <2>1, AsyncIngressSourcesAreFinite
         DEF AsyncConfiguration
    <2>7. /\ IsFiniteSet(1..AsyncIngressCapacity)
           /\ IsFiniteSet(
                AsyncIngressSources \X (1..AsyncIngressCapacity))
      BY <2>6, FS_Interval, FS_Product, SMT
    <2>8. IsFiniteSet(
             AsyncIngressPairIndicesFor(asyncIngressLanes, recipient))
      BY <2>7, FS_Subset DEF AsyncIngressPairIndicesFor
    <2>9. /\ IsFiniteSet(EmptyOtherIngressLanes(recipient, source))
           /\ Cardinality(
                EmptyOtherIngressLanes(recipient, source)) \in Nat
           /\ Cardinality(
                AsyncIngressPairIndicesFor(
                  asyncIngressLanes, recipient)) \in Nat
      BY <2>6, <2>8, FS_Subset, FS_CardinalityType
         DEF EmptyOtherIngressLanes
    <2>10. ASSUME NEW otherRecipient \in ValidatorIds
            PROVE /\ AsyncIngressDepthFor(
                         asyncIngressLanes', otherRecipient) <=
                         AsyncIngressCapacity
                  /\ AsyncIngressDepthFor(
                       asyncIngressLanes', otherRecipient)
                       + Cardinality(
                           {otherSource \in AsyncIngressSources:
                              Len(asyncIngressLanes'
                                    [otherRecipient][otherSource]) = 0})
                       <= AsyncIngressCapacity
      <3>1. CASE otherRecipient = recipient
        <4>1. CASE IngressLaneDepth(recipient, source) <
                     AsyncIngressCapacity
          <5>1. LET nextIndex ==
                        <<source,
                          IngressLaneDepth(recipient, source) + 1>>
                 IN /\ nextIndex \notin
                          AsyncIngressPairIndicesFor(
                            asyncIngressLanes, recipient)
                    /\ AsyncIngressPairIndicesFor(
                         asyncIngressLanes', recipient) =
                         AsyncIngressPairIndicesFor(
                           asyncIngressLanes, recipient) \cup {nextIndex}
            <6>1. <<source,
                     IngressLaneDepth(recipient, source) + 1>>
                    \notin AsyncIngressPairIndicesFor(
                              asyncIngressLanes, recipient)
              BY <2>1, NextIngressIndexIsFresh
                 DEF AsyncIngressContentTypeInvariant,
                     IngressLaneDepth, IngressLane
            <6>2. AsyncIngressPairIndicesFor(
                       asyncIngressLanes', recipient) =
                     AsyncIngressPairIndicesFor(
                       asyncIngressLanes, recipient)
                       \cup {<<source,
                                IngressLaneDepth(recipient, source) + 1>>}
              BY <2>5, <4>1
            <6> QED BY <6>1, <6>2
          <5>2. Cardinality(
                   AsyncIngressPairIndicesFor(
                     asyncIngressLanes', recipient)) =
                 Cardinality(
                   AsyncIngressPairIndicesFor(
                     asyncIngressLanes, recipient)) + 1
            BY <2>8, <5>1, FS_AddElement
          <5>3. IngressDepth(recipient) =
                   Cardinality(
                     AsyncIngressPairIndicesFor(
                       asyncIngressLanes, recipient))
                 /\ AsyncIngressDepthFor(
                      asyncIngressLanes', recipient) =
                   Cardinality(
                     AsyncIngressPairIndicesFor(
                       asyncIngressLanes', recipient))
            BY DEF IngressDepth, AsyncIngressDepthFor,
                   AsyncIngressPairIndicesFor,
                   IngressLaneDepth, IngressLane
          <5>4. Cardinality(
                   {otherSource \in AsyncIngressSources:
                      Len(asyncIngressLanes'
                            [recipient][otherSource]) = 0}) =
                 Cardinality(
                   EmptyOtherIngressLanes(recipient, source))
            BY <2>4, <3>1
               DEF AsyncIngressZeroSourcesFor,
                   IngressLaneDepth, IngressLane
          <5>5. IngressDepth(recipient) <
                   AsyncIngressCapacity -
                     Cardinality(
                       EmptyOtherIngressLanes(recipient, source))
            BY <2>3 DEF IngressUsableCapacity
          <5>6. AsyncIngressDepthFor(asyncIngressLanes', recipient)
                   + Cardinality(
                       {otherSource \in AsyncIngressSources:
                          Len(asyncIngressLanes'
                                [recipient][otherSource]) = 0})
                   <= AsyncIngressCapacity
            <6>1. IngressDepth(recipient) \in Nat
              BY <2>9, <5>3
            <6>2. IngressDepth(recipient) + 1
                     + Cardinality(
                         EmptyOtherIngressLanes(recipient, source))
                     <= AsyncIngressCapacity
              BY <2>6, <2>9, <5>5, <6>1,
                 NaturalReservedCapacityStep
            <6>3. AsyncIngressDepthFor(
                       asyncIngressLanes', recipient) =
                     IngressDepth(recipient) + 1
              BY <5>2, <5>3
            <6> QED BY <5>4, <6>2, <6>3
          <5>7. AsyncIngressDepthFor(asyncIngressLanes', recipient) <=
                   AsyncIngressCapacity
            <6>1. Cardinality(
                     {otherSource \in AsyncIngressSources:
                        Len(asyncIngressLanes'
                              [recipient][otherSource]) = 0}) \in Nat
              BY <2>9, <5>4
            <6>2. AsyncIngressDepthFor(
                       asyncIngressLanes', recipient) \in Nat
              BY <2>9, <5>2, <5>3, SMT
            <6> QED BY <2>6, <5>6, <6>1, <6>2,
                 NaturalSumBoundProjectsLeft
          <5> QED BY <3>1, <5>6, <5>7
        <4>2. CASE IngressLaneDepth(recipient, source) >=
                     AsyncIngressCapacity
          <5>1. /\ IngressLaneDepth(recipient, source) \in Nat
                 /\ IngressLaneDepth(recipient, source) > 0
            BY <2>1, <2>6, <4>2, LenProperties, SMT
               DEF AsyncIngressContentTypeInvariant, IngressLaneDepth
          <5>2. AsyncIngressZeroSourcesFor(
                    asyncIngressLanes, recipient) =
                  EmptyOtherIngressLanes(recipient, source)
            BY <5>1, Isa
               DEF AsyncIngressZeroSourcesFor,
                   EmptyOtherIngressLanes, IngressLaneDepth, IngressLane
          <5>3. /\ AsyncIngressPairIndicesFor(
                         asyncIngressLanes', recipient) =
                       AsyncIngressPairIndicesFor(
                         asyncIngressLanes, recipient)
                 /\ AsyncIngressZeroSourcesFor(
                         asyncIngressLanes', recipient) =
                       AsyncIngressZeroSourcesFor(
                         asyncIngressLanes, recipient)
            <6>1. AsyncIngressPairIndicesFor(
                       asyncIngressLanes', recipient) =
                     IF IngressLaneDepth(recipient, source) <
                          AsyncIngressCapacity
                     THEN AsyncIngressPairIndicesFor(
                            asyncIngressLanes, recipient)
                            \cup {<<source,
                                     IngressLaneDepth(recipient, source) + 1>>}
                     ELSE AsyncIngressPairIndicesFor(
                            asyncIngressLanes, recipient)
              BY <2>5
            <6>2. ~(IngressLaneDepth(recipient, source) <
                       AsyncIngressCapacity)
              BY <2>6, <4>2, <5>1,
                 NaturalGreaterOrEqualIsNotLess
            <6>3. (IF IngressLaneDepth(recipient, source) <
                           AsyncIngressCapacity
                     THEN AsyncIngressPairIndicesFor(
                            asyncIngressLanes, recipient)
                            \cup {<<source,
                                     IngressLaneDepth(recipient, source) + 1>>}
                     ELSE AsyncIngressPairIndicesFor(
                            asyncIngressLanes, recipient)) =
                   AsyncIngressPairIndicesFor(
                     asyncIngressLanes, recipient)
              BY <6>2, IfElseWhenFalse
            <6>4. AsyncIngressPairIndicesFor(
                       asyncIngressLanes', recipient) =
                     AsyncIngressPairIndicesFor(
                       asyncIngressLanes, recipient)
              BY <6>1, <6>3
            <6>5. AsyncIngressZeroSourcesFor(
                       asyncIngressLanes', recipient) =
                     AsyncIngressZeroSourcesFor(
                       asyncIngressLanes, recipient)
              BY <2>4, <5>2
            <6> QED BY <6>4, <6>5
          <5>4. /\ AsyncIngressDepthFor(
                         asyncIngressLanes', recipient) =
                       IngressDepth(recipient)
                 /\ Cardinality(
                      {otherSource \in AsyncIngressSources:
                         Len(asyncIngressLanes'
                               [recipient][otherSource]) = 0}) =
                      Cardinality(
                        {otherSource \in AsyncIngressSources:
                           IngressLaneDepth(recipient, otherSource) = 0})
            BY <5>3
               DEF IngressDepth, AsyncIngressDepthFor,
                   AsyncIngressPairIndicesFor,
                   AsyncIngressZeroSourcesFor,
                   IngressLaneDepth, IngressLane
          <5> QED BY <2>1, <3>1, <5>4
               DEF AsyncIngressCapacityTypeInvariant
        <4>3. /\ IngressLaneDepth(recipient, source) \in Nat
               /\ AsyncIngressCapacity \in Nat
          BY <2>1, LenProperties
             DEF AsyncConfiguration,
                 AsyncIngressContentTypeInvariant,
                 IngressLaneDepth, IngressLane
        <4> QED BY <4>1, <4>2, <4>3, SMT
      <3>2. CASE otherRecipient # recipient
        <4>1. /\ AsyncIngressPairIndicesFor(
                       asyncIngressLanes', otherRecipient) =
                     AsyncIngressPairIndicesFor(
                       asyncIngressLanes, otherRecipient)
               /\ AsyncIngressZeroSourcesFor(
                       asyncIngressLanes', otherRecipient) =
                     AsyncIngressZeroSourcesFor(
                       asyncIngressLanes, otherRecipient)
          <5>1. AsyncIngressPairIndicesFor(
                     asyncIngressLanes', otherRecipient) =
                   AsyncIngressPairIndicesFor(
                     asyncIngressLanes, otherRecipient)
            <6>1. otherRecipient \in ValidatorIds \ {recipient}
              BY <2>10, <3>2, Isa
            <6> QED BY <2>5, <6>1
          <5>2. AsyncIngressZeroSourcesFor(
                     asyncIngressLanes', otherRecipient) =
                   AsyncIngressZeroSourcesFor(
                     asyncIngressLanes, otherRecipient)
            BY <2>4, <2>10, <3>2, Isa
          <5> QED BY <5>1, <5>2
        <4>2. /\ AsyncIngressDepthFor(
                       asyncIngressLanes', otherRecipient) =
                       IngressDepth(otherRecipient)
               /\ Cardinality(
                    {otherSource \in AsyncIngressSources:
                       Len(asyncIngressLanes'
                             [otherRecipient][otherSource]) = 0}) =
                    Cardinality(
                      {otherSource \in AsyncIngressSources:
                         IngressLaneDepth(otherRecipient, otherSource) = 0})
          BY <4>1
             DEF IngressDepth, AsyncIngressDepthFor,
                 AsyncIngressPairIndicesFor,
                 AsyncIngressZeroSourcesFor,
                 IngressLaneDepth, IngressLane
        <4> QED BY <2>1, <2>10, <4>2
             DEF AsyncIngressCapacityTypeInvariant
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>10
         DEF AsyncIngressCapacityTypeInvariant,
             IngressDepth, AsyncIngressDepthFor,
             AsyncIngressPairIndicesFor,
             AsyncIngressZeroSourcesFor,
             IngressLaneDepth, IngressLane
  <1> QED BY <1>1

THEOREM AdmitHiddenPacketPreservesSchedulerType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant /\ AdmitHiddenPacket(recipient, source)
      => AsyncSchedulerTypeInvariant'
BY AdmitHiddenPacketPreservesNonIngressType,
   AdmitHiddenPacketPreservesIngressTopologyType,
   AdmitHiddenPacketPreservesIngressCapacityType,
   AdmitHiddenPacketPreservesIngressContentType
   DEF AsyncSchedulerTypeInvariant, AsyncIngressTypeInvariant

THEOREM AsyncNetworkStepPreservesSchedulerType ==
  AsyncTypeInvariant /\ AsyncNetworkStep
    => AsyncSchedulerTypeInvariant'
BY AdmitHiddenPacketPreservesSchedulerType
   DEF AsyncNetworkStep

THEOREM AsyncStepRefinementObligation ==
  AsyncNext => [Next]_vars
BY DEF AsyncNext

(***************************************************************************
GST is a genuine weak-fairness consequence.  Responsive validators cannot
take the pre-GST crash action, so the SetGST guard remains enabled from every
reachable pre-GST state; SetGST is the sole transition that establishes GST
and no transition clears it.
***************************************************************************)

AsyncResponsiveRemainUp == Responsive \subseteq up

THEOREM ModelResponsiveValidators ==
  ModelConfiguration => Responsive \subseteq ValidatorIds
BY SMT DEF ModelConfiguration, QuorumConfiguration

THEOREM InitAtSetsAllValidatorsUp ==
  \A initialContext:
    InitAt(initialContext) => up = ValidatorIds
BY DEF InitAt

THEOREM AsyncInitKeepsResponsiveUp ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncResponsiveRemainUp
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncResponsiveRemainUp
    <2>1. ModelConfiguration
      BY <1>1 DEF AsyncInitAt, AsyncBaseInitAt, InitAt
    <2>2. Responsive \subseteq ValidatorIds
      BY <2>1, ModelResponsiveValidators
    <2>3. up = ValidatorIds
      BY <1>1, InitAtSetsAllValidatorsUp
         DEF AsyncInitAt, AsyncBaseInitAt
    <2> QED BY <2>2, <2>3 DEF AsyncResponsiveRemainUp
  <1> QED BY <1>1

THEOREM AsyncNonCrashStepKeepsResponsiveUp ==
  AsyncResponsiveRemainUp /\ AsyncNonCrashStep
    => AsyncResponsiveRemainUp'
BY SMT DEF AsyncResponsiveRemainUp, AsyncNonCrashStep

THEOREM AsyncPreGstCrashKeepsResponsiveUp ==
  \A node \in ValidatorIds:
    AsyncResponsiveRemainUp /\ PreGstCrash(node)
      => AsyncResponsiveRemainUp'
BY SMT DEF AsyncResponsiveRemainUp, PreGstCrash, Crash

THEOREM AsyncAllVarsStutterKeepsResponsiveUp ==
  AsyncResponsiveRemainUp /\ UNCHANGED AsyncAllVars
    => AsyncResponsiveRemainUp'
BY Isa DEF AsyncResponsiveRemainUp, AsyncAllVars, vars,
           AsyncSchedulerVars

THEOREM AsyncNextKeepsResponsiveUp ==
  AsyncResponsiveRemainUp /\ [AsyncNext]_AsyncAllVars
    => AsyncResponsiveRemainUp'
BY AsyncNonCrashStepKeepsResponsiveUp,
   AsyncPreGstCrashKeepsResponsiveUp,
   AsyncAllVarsStutterKeepsResponsiveUp, Isa
   DEF AsyncNext

THEOREM AsyncSetGstEnabledWhilePending ==
  (~gst /\ AsyncResponsiveRemainUp)
    => ENABLED <<AsyncSetGST>>_AsyncAllVars
BY ExpandENABLED, SMTT(30)
   DEF AsyncSetGST, SetGST, AsyncResponsiveRemainUp,
       AsyncAllVars, AsyncSchedulerVars, vars

THEOREM AsyncSetGstEstablishesGst ==
  <<AsyncSetGST>>_AsyncAllVars => gst'
BY SMT DEF AsyncSetGST, SetGST

THEOREM AsyncGstEventually ==
  \A initialContext:
    AsyncSpecAt(initialContext) => <>gst
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext) => <>gst
    <2>1. AsyncSpecAt(initialContext) => []AsyncResponsiveRemainUp
      <3>1. AsyncInitAt(initialContext) => AsyncResponsiveRemainUp
        BY AsyncInitKeepsResponsiveUp
      <3>2. AsyncResponsiveRemainUp /\ [AsyncNext]_AsyncAllVars
              => AsyncResponsiveRemainUp'
        BY AsyncNextKeepsResponsiveUp
      <3> QED BY <3>1, <3>2, PTL DEF AsyncSpecAt
    <2>2. (~gst /\ AsyncResponsiveRemainUp)
            => ENABLED <<AsyncSetGST>>_AsyncAllVars
      BY AsyncSetGstEnabledWhilePending
    <2>3. <<AsyncSetGST>>_AsyncAllVars => gst'
      BY AsyncSetGstEstablishesGst
    <2>4. AsyncSpecAt(initialContext)
            => WF_AsyncAllVars(AsyncSetGST)
      BY DEF AsyncSpecAt, AsyncFairnessAt
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

(***************************************************************************
The asynchronous theorem is deliberately one-height: every reachable state
keeps the caller-supplied context and height fixed.  Per-node rollover and
historical service are represented by `NodeHasApplication` and
`RunHistoricalServer`, not by the reconfiguration harness's global barrier.
***************************************************************************)

AsyncFrozenContextAt(initialContext) ==
  /\ context = initialContext
  /\ height = initialContext.height

THEOREM AsyncInitEstablishesFrozenContext ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncFrozenContextAt(initialContext)
BY SMT DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
           AsyncFrozenContextAt

THEOREM AsyncNextPreservesFrozenContext ==
  \A initialContext:
    AsyncFrozenContextAt(initialContext)
      /\ [AsyncNext]_AsyncAllVars
      => AsyncFrozenContextAt(initialContext)'
BY Isa DEF AsyncFrozenContextAt, AsyncNext, AsyncAllVars, vars,
           AsyncSchedulerVars

THEOREM AsyncSpecAlwaysKeepsFrozenContext ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []AsyncFrozenContextAt(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []AsyncFrozenContextAt(initialContext)
    <2>1. AsyncInitAt(initialContext)
            => AsyncFrozenContextAt(initialContext)
      BY AsyncInitEstablishesFrozenContext
    <2>2. AsyncFrozenContextAt(initialContext)
            /\ [AsyncNext]_AsyncAllVars
            => AsyncFrozenContextAt(initialContext)'
      BY AsyncNextPreservesFrozenContext
    <2> QED BY <2>1, <2>2, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

=============================================================================
