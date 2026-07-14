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
    <2>5. \A node \in ValidatorIds:
             AsyncCausalQueueOwnership(node, asyncCausalQueues[node])
      BY <2>1, InitialCausalCandidateShape, SingletonSequenceFacts, SMT
         DEF AsyncCausalQueueOwnership, SequenceSet
    <2> QED BY <2>3, <2>4, <2>5 DEF AsyncCausalTypeInvariant
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
BY Isa DEF AsyncCausalTypeInvariant, AsyncCausalQueueOwnership

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
           /\ (\A index \in 1..Len(queue):
                  Append(queue, candidate)[index] = queue[index])
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
          <5>2. Append(queue, candidate)[index] = candidate
            BY <2>2, <5>1, Isa
          <5> QED BY <1>1, <5>2
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

THEOREM CompletionIndicesAfterNonCompletionAppend ==
  \A queue, candidate:
    /\ AsyncQueueTyped(queue)
    /\ candidate.class # "Completion"
    => AsyncCompletionIndices(Append(queue, candidate)) =
         AsyncCompletionIndices(queue)
PROOF
  <1>1. ASSUME NEW queue, NEW candidate,
                AsyncQueueTyped(queue),
                candidate.class # "Completion"
         PROVE AsyncCompletionIndices(Append(queue, candidate)) =
                 AsyncCompletionIndices(queue)
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
                       historicalSource \in AsyncIngressSources:
                      Len(next[otherRecipient][historicalSource]) =
                        Len(lanes[otherRecipient][historicalSource])
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
                       historicalSource \in AsyncIngressSources:
                      Len(next[otherRecipient][historicalSource]) =
                        Len(lanes[otherRecipient][historicalSource])
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

(***************************************************************************
Runner scheduler type preservation.  The leaf actions deliberately omit the
service-clock variables owned by the outer runner.  The branch lemmas below
therefore keep the exact `RunNode`/`RunHistoricalServer` service frame in
their hypotheses instead of silently treating an unconstrained primed value
as a stutter.
***************************************************************************)

RunnerServiceFrame(node) ==
  /\ UNCHANGED asyncNow
  /\ asyncNodeServiceDeadlines' =
       [asyncNodeServiceDeadlines EXCEPT
          ![node] = asyncNow + AsyncDeliveryBound]
  /\ UNCHANGED asyncIoServiceDeadlines

THEOREM TypedQueueTailFacts ==
  \A queue:
    AsyncQueueTyped(queue) /\ Len(queue) > 0
      => /\ AsyncQueueTyped(Tail(queue))
         /\ SequenceSet(Tail(queue)) \subseteq SequenceSet(queue)
         /\ Len(Tail(queue)) + 1 = Len(queue)
PROOF
  <1>1. ASSUME NEW queue,
                AsyncQueueTyped(queue),
                Len(queue) > 0
         PROVE /\ AsyncQueueTyped(Tail(queue))
               /\ SequenceSet(Tail(queue)) \subseteq SequenceSet(queue)
               /\ Len(Tail(queue)) + 1 = Len(queue)
    <2>1. /\ queue \in Seq(Range(queue))
           /\ queue # <<>>
      BY <1>1, EmptySeq, SMT DEF AsyncQueueTyped
    <2>2. /\ Tail(queue) \in Seq(Range(queue))
           /\ Range(Tail(queue)) \subseteq Range(queue)
           /\ Len(Tail(queue)) + 1 = Len(queue)
      BY <2>1, HeadTailProperties
    <2>3. /\ Tail(queue) \in Seq(Range(Tail(queue)))
           /\ DOMAIN Tail(queue) = 1..Len(Tail(queue))
      BY <2>2, SeqOfRange, LenProperties
    <2>4. \A index \in 1..Len(Tail(queue)):
             AsyncCandidateTyped(Tail(queue)[index])
      <3>1. ASSUME NEW index \in 1..Len(Tail(queue))
             PROVE AsyncCandidateTyped(Tail(queue)[index])
        <4>1. Tail(queue)[index] \in Range(Tail(queue))
          BY <2>3, <3>1, RangeEquality
        <4>2. Tail(queue)[index] \in Range(queue)
          BY <2>2, <4>1
        <4>3. PICK original \in 1..Len(queue):
                 Tail(queue)[index] = queue[original]
          BY <1>1, <4>2, RangeEquality DEF AsyncQueueTyped
        <4> QED BY <1>1, <4>3 DEF AsyncQueueTyped
      <3> QED BY <3>1
    <2>5. AsyncQueueTyped(Tail(queue))
      BY <2>3, <2>4 DEF AsyncQueueTyped
    <2>6. SequenceSet(Tail(queue)) \subseteq SequenceSet(queue)
      BY <1>1, <2>2, <2>3, RangeEquality
         DEF AsyncQueueTyped, SequenceSet
    <2> QED BY <2>2, <2>5, <2>6
  <1> QED BY <1>1

THEOREM TypedIngressTailFacts ==
  \A sequence:
    /\ sequence \in Seq(Range(sequence))
    /\ DOMAIN sequence = 1..Len(sequence)
    /\ \A index \in 1..Len(sequence):
         AsyncItemTyped(sequence[index])
    /\ Len(sequence) > 0
    => /\ Tail(sequence) \in Seq(Range(Tail(sequence)))
       /\ DOMAIN Tail(sequence) = 1..Len(Tail(sequence))
       /\ \A index \in 1..Len(Tail(sequence)):
            AsyncItemTyped(Tail(sequence)[index])
       /\ SequenceSet(Tail(sequence)) \subseteq SequenceSet(sequence)
       /\ Len(Tail(sequence)) + 1 = Len(sequence)
PROOF
  <1>1. ASSUME NEW sequence,
                sequence \in Seq(Range(sequence)),
                DOMAIN sequence = 1..Len(sequence),
                \A index \in 1..Len(sequence):
                  AsyncItemTyped(sequence[index]),
                Len(sequence) > 0
         PROVE /\ Tail(sequence) \in Seq(Range(Tail(sequence)))
               /\ DOMAIN Tail(sequence) = 1..Len(Tail(sequence))
               /\ \A index \in 1..Len(Tail(sequence)):
                    AsyncItemTyped(Tail(sequence)[index])
               /\ SequenceSet(Tail(sequence))
                    \subseteq SequenceSet(sequence)
               /\ Len(Tail(sequence)) + 1 = Len(sequence)
    <2>1. sequence # <<>>
      BY <1>1, EmptySeq, SMT
    <2>2. /\ Tail(sequence) \in Seq(Range(sequence))
           /\ Range(Tail(sequence)) \subseteq Range(sequence)
           /\ Len(Tail(sequence)) + 1 = Len(sequence)
      BY <1>1, <2>1, HeadTailProperties
    <2>3. /\ Tail(sequence) \in Seq(Range(Tail(sequence)))
           /\ DOMAIN Tail(sequence) = 1..Len(Tail(sequence))
      BY <2>2, SeqOfRange, LenProperties
    <2>4. \A index \in 1..Len(Tail(sequence)):
             AsyncItemTyped(Tail(sequence)[index])
      <3>1. ASSUME NEW index \in 1..Len(Tail(sequence))
             PROVE AsyncItemTyped(Tail(sequence)[index])
        <4>1. Tail(sequence)[index] \in Range(Tail(sequence))
          BY <2>3, <3>1, RangeEquality
        <4>2. PICK original \in 1..Len(sequence):
                 Tail(sequence)[index] = sequence[original]
          BY <1>1, <2>2, <4>1, RangeEquality
        <4> QED BY <1>1, <4>2
      <3> QED BY <3>1
    <2>5. SequenceSet(Tail(sequence)) \subseteq SequenceSet(sequence)
      BY <1>1, <2>2, <2>3, RangeEquality DEF SequenceSet
    <2> QED BY <2>2, <2>3, <2>4, <2>5
  <1> QED BY <1>1

THEOREM SelectedSequenceRotationFacts ==
  \A sequence:
    \A selected \in 1..Len(sequence):
      /\ sequence \in Seq(Range(sequence))
      /\ Len(sequence) = Cardinality(SequenceSet(sequence))
      => LET suffix == SubSeq(sequence, selected + 1, Len(sequence))
             prefix == SubSeq(sequence, 1, selected - 1)
             rotated == suffix \o prefix
         IN /\ rotated \in Seq(Range(rotated))
            /\ IsInjective(rotated)
            /\ SequenceSet(rotated) =
                 SequenceSet(sequence) \ {sequence[selected]}
            /\ sequence[selected] \notin SequenceSet(rotated)
            /\ Len(rotated) + 1 = Len(sequence)
            /\ Len(rotated) = Cardinality(SequenceSet(rotated))
PROOF
  <1>1. ASSUME NEW sequence,
                NEW selected \in 1..Len(sequence),
                sequence \in Seq(Range(sequence)),
                Len(sequence) = Cardinality(SequenceSet(sequence))
         PROVE LET suffix ==
                       SubSeq(sequence, selected + 1, Len(sequence))
                   prefix == SubSeq(sequence, 1, selected - 1)
                   rotated == suffix \o prefix
               IN /\ rotated \in Seq(Range(rotated))
                  /\ IsInjective(rotated)
                  /\ SequenceSet(rotated) =
                       SequenceSet(sequence) \ {sequence[selected]}
                  /\ sequence[selected] \notin SequenceSet(rotated)
                  /\ Len(rotated) + 1 = Len(sequence)
                  /\ Len(rotated) =
                       Cardinality(SequenceSet(rotated))
    <2> DEFINE Suffix ==
           SubSeq(sequence, selected + 1, Len(sequence))
    <2> DEFINE Prefix == SubSeq(sequence, 1, selected - 1)
    <2> DEFINE Rotated == Suffix \o Prefix
    <2>1. /\ Len(sequence) \in Nat
           /\ IsInjective(sequence)
      BY <1>1, LenProperties, UniqueSequenceLengthImpliesInjective
    <2>2. /\ Suffix \in Seq(Range(sequence))
           /\ Prefix \in Seq(Range(sequence))
           /\ Len(Suffix) = Len(sequence) - selected
           /\ Len(Prefix) = selected - 1
           /\ \A offset \in 1..Len(Suffix):
                Suffix[offset] = sequence[selected + offset]
           /\ \A prefixOffset \in 1..Len(Prefix):
                Prefix[prefixOffset] = sequence[prefixOffset]
      BY <1>1, <2>1, SubSeqProperties, SMT
         DEF Suffix, Prefix
    <2>3. /\ IsInjective(Suffix)
           /\ IsInjective(Prefix)
           /\ Range(Suffix) \cap Range(Prefix) = {}
      BY <1>1, <2>1, <2>2, RangeEquality, SMT
         DEF IsInjective
    <2>4. /\ Rotated \in Seq(Range(sequence))
           /\ Len(Rotated) = Len(sequence) - 1
           /\ IsInjective(Rotated)
      BY <1>1, <2>1, <2>2, <2>3,
         ConcatProperties, ConcatInjectiveSeq, SMT DEF Rotated
    <2>5. Range(Rotated) =
             Range(sequence) \ {sequence[selected]}
      <3>1. Range(Rotated) = Range(Suffix) \cup Range(Prefix)
        BY <2>2, RangeConcatenation DEF Rotated
      <3>2. Range(Rotated) \subseteq
               Range(sequence) \ {sequence[selected]}
        BY <1>1, <2>1, <2>2, <3>1, RangeEquality, SMT
           DEF IsInjective
      <3>3. Range(sequence) \ {sequence[selected]}
               \subseteq Range(Rotated)
        BY <1>1, <2>1, <2>2, <3>1, RangeEquality, SMT
           DEF IsInjective
      <3> QED BY <3>2, <3>3
    <2>6. /\ Rotated \in Seq(Range(Rotated))
           /\ SequenceSet(Rotated) =
                SequenceSet(sequence) \ {sequence[selected]}
           /\ sequence[selected] \notin SequenceSet(Rotated)
      BY <1>1, <2>4, <2>5, SeqOfRange, RangeEquality
         DEF SequenceSet
    <2>7. Len(Rotated) =
             Cardinality(SequenceSet(Rotated))
      BY <2>4, <2>6, InjectiveSequenceLengthMatchesSetCardinality
    <2> QED BY <2>4, <2>6, <2>7 DEF Suffix, Prefix, Rotated
  <1> QED BY <1>1

THEOREM PopSelectedIngressPreservesContentType ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      AsyncIngressTypeInvariant /\ PopSelectedIngress(node, index)
        => AsyncIngressContentTypeInvariant'
BY TypedIngressTailFacts, SMTT(30)
   DEF AsyncIngressTypeInvariant, AsyncIngressTopologyTypeInvariant,
       AsyncIngressContentTypeInvariant, PopSelectedIngress,
       IngressLaneDepth, IngressLane, SequenceSet

THEOREM PopSelectedIngressPreservesTopologyType ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      AsyncIngressTypeInvariant /\ PopSelectedIngress(node, index)
        => AsyncIngressTopologyTypeInvariant'
BY UniqueSequenceLengthImpliesInjective,
   InjectiveSequenceLengthMatchesSetCardinality,
   SelectedSequenceRotationFacts,
   AppendInjectiveSeq, ConcatInjectiveSeq, SubSeqProperties,
   RangeConcatenation, AppendSequenceFacts, SMTT(60)
   DEF AsyncIngressTypeInvariant, AsyncIngressTopologyTypeInvariant,
       AsyncIngressContentTypeInvariant, PopSelectedIngress,
       ReadyAfterSelectedDrain, IngressLaneDepth, IngressLane,
       SequenceSet, IsInjective

THEOREM PopSelectedIngressPreservesCapacityType ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      AsyncIngressTypeInvariant /\ PopSelectedIngress(node, index)
        => AsyncIngressCapacityTypeInvariant'
BY TypedIngressTailFacts, FS_RemoveElement, FS_AddElement,
   FS_CardinalityType, SMTT(60)
   DEF AsyncIngressTypeInvariant, AsyncIngressTopologyTypeInvariant,
       AsyncIngressCapacityTypeInvariant,
       AsyncIngressContentTypeInvariant, PopSelectedIngress,
       IngressDepth, IngressLaneDepth, IngressLane, SequenceSet

THEOREM PopSelectedIngressPreservesIngressType ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      AsyncIngressTypeInvariant /\ PopSelectedIngress(node, index)
        => AsyncIngressTypeInvariant'
BY PopSelectedIngressPreservesContentType,
   PopSelectedIngressPreservesTopologyType,
   PopSelectedIngressPreservesCapacityType
   DEF AsyncIngressTypeInvariant

THEOREM DeliveryCandidateShape ==
  \A item:
    /\ DOMAIN DeliveryCandidate(item) =
         {"class", "kind", "node", "height", "view", "subject", "item"}
    /\ DeliveryCandidate(item).class = DeliveryClass(item)
    /\ DeliveryCandidate(item).kind = DeliveryKind(item)
    /\ DeliveryCandidate(item).node = item.envelope.recipient
    /\ DeliveryCandidate(item).height = DeliveryHeight(item)
    /\ DeliveryCandidate(item).view = DeliveryView(item)
    /\ DeliveryCandidate(item).subject = DeliverySubject(item)
    /\ DeliveryCandidate(item).item = item
BY SMT DEF DeliveryCandidate, AsyncCandidate

THEOREM TypedNetworkKindMakesTypedDeliveryDiscriminants ==
  \A item:
    item.kind \in AsyncNetworkKinds
      => /\ DeliveryClass(item) \in AsyncCommandClasses
         /\ DeliveryKind(item) \in AsyncWorkKinds
BY SMT
   DEF AsyncNetworkKinds, DeliveryClass, DeliveryKind,
       AsyncCommandClasses, AsyncWorkKinds, AsyncReducerKinds

THEOREM TypedItemMakesTypedDeliveryCandidate ==
  \A item:
    AsyncItemTyped(item) => AsyncCandidateTyped(DeliveryCandidate(item))
PROOF
  <1>1. ASSUME NEW item, AsyncItemTyped(item)
         PROVE AsyncCandidateTyped(DeliveryCandidate(item))
    <2>1. /\ item.kind \in AsyncNetworkKinds
           /\ item.envelope.recipient \in ValidatorIds
      BY <1>1 DEF AsyncItemTyped
    <2>2. /\ DOMAIN DeliveryCandidate(item) =
                    {"class", "kind", "node", "height", "view",
                     "subject", "item"}
           /\ DeliveryCandidate(item).class \in AsyncCommandClasses
           /\ DeliveryCandidate(item).kind \in AsyncWorkKinds
           /\ DeliveryCandidate(item).node \in ValidatorIds
           /\ DeliveryCandidate(item).item = item
      BY <2>1, DeliveryCandidateShape,
         TypedNetworkKindMakesTypedDeliveryDiscriminants
    <2>3. CASE item.kind = "Proposal"
      <3>1. item.envelope \in ProposalEnvelopeSet
        BY <1>1, <2>3, SMT DEF AsyncItemTyped
      <3> QED BY <1>1, <2>2, <2>3, <3>1,
           DeliveryCandidateShape, SMT
           DEF DeliveryHeight, DeliveryView, DeliverySubject,
               ProposalEnvelopeSet, SubjectOrNone, AsyncCandidateTyped
    <2>4. CASE item.kind \in {"PrepareVote", "CommitVote"}
      <3>1. item.envelope \in VoteEnvelopeSet
        BY <1>1, <2>4, SMT DEF AsyncItemTyped
      <3> QED BY <1>1, <2>2, <2>4, <3>1,
           DeliveryCandidateShape, SMT
           DEF DeliveryHeight, DeliveryView, DeliverySubject,
               VoteEnvelopeSet, SubjectOrNone, AsyncCandidateTyped
    <2>5. CASE item.kind \in {"PrepareQC", "CommitQC"}
      <3>1. item.envelope \in QcEnvelopeSet
        BY <1>1, <2>5, SMT DEF AsyncItemTyped
      <3> QED BY <1>1, <2>2, <2>5, <3>1,
           DeliveryCandidateShape, SMT
           DEF DeliveryHeight, DeliveryView, DeliverySubject,
               QcEnvelopeSet, SubjectOrNone, AsyncCandidateTyped
    <2>6. CASE item.kind = "TimeoutVote"
      <3>1. item.envelope \in TimeoutEnvelopeSet
        BY <1>1, <2>6, SMT DEF AsyncItemTyped
      <3> QED BY <1>1, <2>2, <2>6, <3>1,
           DeliveryCandidateShape, SMT
           DEF DeliveryHeight, DeliveryView, DeliverySubject,
               TimeoutEnvelopeSet, SubjectOrNone, AsyncCandidateTyped
    <2>7. CASE item.kind = "TimeoutCertificate"
      <3>1. item.envelope \in TcEnvelopeSet
        BY <1>1, <2>7, SMT DEF AsyncItemTyped
      <3> QED BY <1>1, <2>2, <2>7, <3>1,
           DeliveryCandidateShape, SMT
           DEF DeliveryHeight, DeliveryView, DeliverySubject,
               TcEnvelopeSet, SubjectOrNone, AsyncCandidateTyped
    <2>8. CASE item.kind = "CommitCertificateResponse"
      <3>1. item.envelope \in QcEnvelopeSet
        BY <1>1, <2>8, SMT DEF AsyncItemTyped
      <3> QED BY <1>1, <2>2, <2>8, <3>1,
           DeliveryCandidateShape, SMT
           DEF DeliveryHeight, DeliveryView, DeliverySubject,
               QcEnvelopeSet, SubjectOrNone, AsyncCandidateTyped
    <2>9. CASE item.kind \in
      {"Chunk", "CertifiedRequest", "CertifiedResponse",
       "CommitCertificateRequest", "NormalJunk", "ProgressJunk", "Noise"}
      <3>1. AsyncBodyEnvelopeTyped(item.envelope)
        BY <1>1, <2>9, SMT DEF AsyncItemTyped
      <3> QED BY <1>1, <2>2, <2>9, <3>1,
           DeliveryCandidateShape, SMT
           DEF DeliveryHeight, DeliveryView, DeliverySubject,
               AsyncBodyEnvelopeTyped, SubjectOrNone,
               AsyncCandidateTyped
    <2> QED BY <2>1, <2>3, <2>4, <2>5, <2>6, <2>7, <2>8, <2>9,
                SMT DEF AsyncNetworkKinds
  <1> QED BY <1>1

THEOREM TypedRequestMakesTypedServeJob ==
  \A item:
    /\ AsyncConfiguration
    /\ AsyncItemTyped(item)
    /\ item.kind \in {"CertifiedRequest", "CommitCertificateRequest"}
    => AsyncIoJobTyped(
         AsyncIoCertifiedServeJob(DeliveryCandidate(item)))
PROOF
  <1>1. ASSUME NEW item,
                AsyncConfiguration,
                AsyncItemTyped(item),
                item.kind
                  \in {"CertifiedRequest", "CommitCertificateRequest"}
         PROVE AsyncIoJobTyped(
                 AsyncIoCertifiedServeJob(DeliveryCandidate(item)))
    <2>1. /\ item.kind \in AsyncNetworkKinds
           /\ item.envelope.recipient \in ValidatorIds
           /\ AsyncBodyEnvelopeTyped(item.envelope)
      BY <1>1, SMT DEF AsyncItemTyped
    <2>2. AsyncCandidateTyped(DeliveryCandidate(item))
      BY <1>1, <2>1, DeliveryCandidateShape,
         TypedNetworkKindMakesTypedDeliveryDiscriminants, SMT
         DEF AsyncCandidateTyped, DeliveryHeight, DeliveryView,
             DeliverySubject, AsyncBodyEnvelopeTyped, SubjectOrNone
    <2> QED BY <1>1, <2>2, SMT
         DEF AsyncIoJobTyped, AsyncIoCertifiedServeJob, AsyncIoJob,
             DeliveryCandidate, DeliveryKind, AsyncIoCommandClasses,
             AsyncConfiguration
  <1> QED BY <1>1

THEOREM TypedCompletionCandidateMakesConsensusJob ==
  \A candidate:
    /\ AsyncConfiguration
    /\ AsyncCandidateTyped(candidate)
    /\ candidate.class = "Completion"
    => AsyncIoJobTyped(AsyncIoConsensusJob(candidate))
BY SMT
   DEF AsyncIoJobTyped, AsyncIoConsensusJob, AsyncIoJob,
       AsyncIoCommandClasses, AsyncConfiguration

THEOREM AppendTypedServeJobPreservesIoType ==
  \A node \in ValidatorIds:
  \A item:
    /\ AsyncTypeInvariant
    /\ AsyncItemTyped(item)
    /\ item.kind \in {"CertifiedRequest", "CommitCertificateRequest"}
    /\ CanEnqueueIoClass(node, "Serve")
    /\ asyncIoQueues' =
         [asyncIoQueues EXCEPT
            ![node] = Append(
              @, AsyncIoCertifiedServeJob(DeliveryCandidate(item)))]
    /\ UNCHANGED <<asyncCommandQueues, asyncOutstandingWork,
                    asyncIoReadyCompletions,
                    asyncLocalReadyCompletions,
                    asyncNextCompletionSource,
                    asyncIoControlAvailable,
                    asyncDeferredCompletionQueues>>
    => AsyncIoTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW item,
                AsyncTypeInvariant,
                AsyncItemTyped(item),
                item.kind
                  \in {"CertifiedRequest", "CommitCertificateRequest"},
                CanEnqueueIoClass(node, "Serve"),
                asyncIoQueues' =
                  [asyncIoQueues EXCEPT
                     ![node] = Append(
                       @, AsyncIoCertifiedServeJob(
                            DeliveryCandidate(item)))],
                UNCHANGED <<asyncCommandQueues, asyncOutstandingWork,
                            asyncIoReadyCompletions,
                            asyncLocalReadyCompletions,
                            asyncNextCompletionSource,
                            asyncIoControlAvailable,
                            asyncDeferredCompletionQueues>>
         PROVE AsyncIoTypeInvariant'
    <2>1. /\ AsyncConfiguration
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>2. AsyncIoJobTyped(
             AsyncIoCertifiedServeJob(DeliveryCandidate(item)))
      BY <1>1, <2>1, TypedRequestMakesTypedServeJob
    <2>3. AsyncIoTopologyTypeInvariant'
      BY <1>1, <2>1, Isa DEF AsyncIoTopologyTypeInvariant
    <2>4. \A other \in ValidatorIds:
             AsyncIoSequenceTyped(asyncIoQueues'[other])
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE AsyncIoSequenceTyped(asyncIoQueues'[other])
        <4>1. CASE other = node
          <5>1. AsyncIoSequenceTyped(asyncIoQueues[node])
            BY <2>1 DEF AsyncIoQueueContentTypeInvariant
          <5> QED BY <1>1, <2>2, <4>1, <5>1,
                       TypedIoAppendPreservesSequenceType
        <4>2. CASE other # node
          BY <1>1, <2>1, <4>2, FunctionalUpdateAwayFromKey
             DEF AsyncIoQueueContentTypeInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>5. \A other \in ValidatorIds:
             /\ \A job \in SequenceSet(asyncIoQueues'[other]):
                  job.class = "Consensus" =>
                    job.candidate \in asyncOutstandingWork'[other]
             /\ AsyncIoConsensusCandidateOwnership(
                  other, asyncIoQueues', asyncIoReadyCompletions',
                  asyncLocalReadyCompletions')
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE /\ \A job \in SequenceSet(asyncIoQueues'[other]):
                          job.class = "Consensus" =>
                            job.candidate
                              \in asyncOutstandingWork'[other]
                   /\ AsyncIoConsensusCandidateOwnership(
                        other, asyncIoQueues',
                        asyncIoReadyCompletions',
                        asyncLocalReadyCompletions')
        <4>1. CASE other = node
          <5>1. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
                 /\ \A job \in SequenceSet(asyncIoQueues[node]):
                      job.class = "Consensus" =>
                        job.candidate \in asyncOutstandingWork[node]
                 /\ AsyncIoConsensusCandidateOwnership(
                      node, asyncIoQueues, asyncIoReadyCompletions,
                      asyncLocalReadyCompletions)
            BY <2>1 DEF AsyncIoQueueContentTypeInvariant
          <5>2. AsyncIoCertifiedServeJob(
                   DeliveryCandidate(item)).class # "Consensus"
            BY DEF AsyncIoCertifiedServeJob, AsyncIoJob
          <5>3. AsyncIoConsensusIndices(asyncIoQueues'[node]) =
                   AsyncIoConsensusIndices(asyncIoQueues[node])
            BY <1>1, <5>1, <5>2,
               ConsensusIndicesAfterNonConsensusAppend
               DEF AsyncIoSequenceTyped
          <5> QED BY <1>1, <4>1, <5>1, <5>3, Isa
               DEF AsyncIoConsensusCandidateOwnership,
                   AsyncIoConsensusQueueOwnership,
                   AsyncIoConsensusIndices, SequenceSet
        <4>2. CASE other # node
          BY <1>1, <2>1, <4>2, FunctionalUpdateAwayFromKey
             DEF AsyncIoQueueContentTypeInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>6. AsyncIoQueueContentTypeInvariant'
      BY <2>4, <2>5 DEF AsyncIoQueueContentTypeInvariant
    <2>7. AsyncIoWorkContentTypeInvariant'
      BY <1>1, <2>1, AsyncIoWorkContentTypeStutter
         DEF AsyncIoWorkContentTypeVars
    <2>8. AsyncIoCapacityTypeInvariant'
      <3>1. AsyncIoQueueDepth(node) < AsyncIoAuxCapacity
        BY <1>1 DEF CanEnqueueIoClass, AsyncIoAdmissionLimit
      <3>2. /\ AsyncIoQueueDepth(node) \in Nat
             /\ AsyncIoQueueDepth(node) + 1 <= AsyncIoCapacity
        BY <2>1, <3>1, SMT
           DEF AsyncIoQueueContentTypeInvariant,
               AsyncIoSequenceTyped, AsyncIoQueueDepth,
               AsyncConfiguration, AsyncIoCapacity
      <3>3. \A other \in ValidatorIds:
               AsyncIoQueueDepth(other) <= AsyncIoCapacity
        BY <2>1 DEF AsyncIoCapacityTypeInvariant
      <3> QED BY <1>1, <2>1, <3>2, <3>3, Isa
           DEF AsyncIoCapacityTypeInvariant, AsyncIoQueueDepth,
               AsyncQueueDepth, AsyncCompletionLoad,
               AsyncOutstandingWorkCount, QueuedCompletionCount,
               DeferredCompletionCount
    <2> QED BY <2>3, <2>6, <2>7, <2>8
         DEF AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
  <1> QED BY <1>1

THEOREM FirstHistoricalDrainableIndexIsDrainable ==
  \A node:
    HistoricalDrainableIngressIndices(node) # {}
      => FirstHistoricalDrainableIngressIndex(node)
           \in HistoricalDrainableIngressIndices(node)
PROOF
  <1>1. ASSUME NEW node,
                HistoricalDrainableIngressIndices(node) # {}
         PROVE FirstHistoricalDrainableIngressIndex(node)
                 \in HistoricalDrainableIngressIndices(node)
    <2>1. PICK witness \in HistoricalDrainableIngressIndices(node): TRUE
      BY <1>1, FS_EmptySet, Zenon
    <2>2. witness \in Nat
      BY <2>1, SMT DEF HistoricalDrainableIngressIndices
    <2>3. \E least \in Nat:
             /\ least \in HistoricalDrainableIngressIndices(node)
             /\ \A prior \in 0..(least - 1):
                  prior \notin HistoricalDrainableIngressIndices(node)
      BY <2>1, <2>2, SmallestNatural
    <2>4. PICK least \in Nat:
             /\ least \in HistoricalDrainableIngressIndices(node)
             /\ \A prior \in 0..(least - 1):
                  prior \notin HistoricalDrainableIngressIndices(node)
      BY <2>3
    <2>5. \A other \in HistoricalDrainableIngressIndices(node):
             least <= other
      BY <2>4, SMT DEF HistoricalDrainableIngressIndices
    <2> QED BY <2>4, <2>5, Zenon
         DEF FirstHistoricalDrainableIngressIndex
  <1> QED BY <1>1

THEOREM FirstDrainableIngressIndexIsDrainable ==
  \A node:
    DrainableIngressIndices(node) # {}
      => FirstDrainableIngressIndex(node)
           \in DrainableIngressIndices(node)
PROOF
  <1>1. ASSUME NEW node,
                DrainableIngressIndices(node) # {}
         PROVE FirstDrainableIngressIndex(node)
                 \in DrainableIngressIndices(node)
    <2>1. PICK witness \in DrainableIngressIndices(node): TRUE
      BY <1>1, FS_EmptySet, Zenon
    <2>2. witness \in Nat
      BY <2>1, SMT DEF DrainableIngressIndices
    <2>3. \E least \in Nat:
             /\ least \in DrainableIngressIndices(node)
             /\ \A prior \in 0..(least - 1):
                  prior \notin DrainableIngressIndices(node)
      BY <2>1, <2>2, SmallestNatural
    <2>4. PICK least \in Nat:
             /\ least \in DrainableIngressIndices(node)
             /\ \A prior \in 0..(least - 1):
                  prior \notin DrainableIngressIndices(node)
      BY <2>3
    <2>5. \A other \in DrainableIngressIndices(node): least <= other
      BY <2>4, SMT DEF DrainableIngressIndices
    <2> QED BY <2>4, <2>5, Zenon DEF FirstDrainableIngressIndex
  <1> QED BY <1>1

THEOREM SelectedIngressItemIsTyped ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      AsyncIngressTypeInvariant
        => AsyncItemTyped(IngressItemAt(node, index))
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW index \in 1..Len(asyncIngressReady[node]),
                AsyncIngressTypeInvariant
         PROVE AsyncItemTyped(IngressItemAt(node, index))
    <2>1. asyncIngressReady[node][index]
               \in SequenceSet(asyncIngressReady[node])
      BY <1>1, RangeEquality
         DEF AsyncIngressTypeInvariant,
             AsyncIngressTopologyTypeInvariant, SequenceSet
    <2>2. IngressLaneDepth(
               node, asyncIngressReady[node][index]) > 0
      BY <1>1, <2>1
         DEF AsyncIngressTypeInvariant,
             AsyncIngressTopologyTypeInvariant
    <2>3. /\ IngressLane(
                    node, asyncIngressReady[node][index])
                    \in Seq(Range(IngressLane(
                      node, asyncIngressReady[node][index])))
           /\ \A laneIndex \in
                  1..IngressLaneDepth(
                    node, asyncIngressReady[node][index]):
                AsyncItemTyped(IngressLane(
                  node, asyncIngressReady[node][index])[laneIndex])
      BY <1>1, <2>1
         DEF AsyncIngressTypeInvariant,
             AsyncIngressTopologyTypeInvariant,
             AsyncIngressContentTypeInvariant
    <2>4. Head(IngressLane(
               node, asyncIngressReady[node][index])) =
             IngressLane(
               node, asyncIngressReady[node][index])[1]
      BY <2>2, <2>3, NonemptySequenceHeadIsFirst
         DEF IngressLaneDepth
    <2> QED BY <2>2, <2>3, <2>4
         DEF IngressItemAt, IngressLaneDepth
  <1> QED BY <1>1

THEOREM SelectedIngressItemHasLaneOwnership ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      AsyncIngressTypeInvariant
        => /\ IngressItemAt(node, index).envelope.recipient = node
           /\ IngressItemAt(node, index).source =
                asyncIngressReady[node][index]
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW index \in 1..Len(asyncIngressReady[node]),
                AsyncIngressTypeInvariant
         PROVE /\ IngressItemAt(node, index).envelope.recipient = node
               /\ IngressItemAt(node, index).source =
                    asyncIngressReady[node][index]
    <2>1. asyncIngressReady[node][index]
               \in SequenceSet(asyncIngressReady[node])
      BY <1>1, RangeEquality
         DEF AsyncIngressTypeInvariant,
             AsyncIngressTopologyTypeInvariant, SequenceSet
    <2>2. IngressLaneDepth(
               node, asyncIngressReady[node][index]) > 0
      BY <1>1, <2>1
         DEF AsyncIngressTypeInvariant,
             AsyncIngressTopologyTypeInvariant
    <2>3. /\ IngressLane(
                    node, asyncIngressReady[node][index])
                    \in Seq(Range(IngressLane(
                      node, asyncIngressReady[node][index])))
           /\ \A laneIndex \in
                  1..IngressLaneDepth(
                    node, asyncIngressReady[node][index]):
                /\ IngressLane(
                     node, asyncIngressReady[node][index])[laneIndex]
                     .envelope.recipient = node
                /\ IngressLane(
                     node, asyncIngressReady[node][index])[laneIndex]
                     .source = asyncIngressReady[node][index]
      BY <1>1, <2>1
         DEF AsyncIngressTypeInvariant,
             AsyncIngressTopologyTypeInvariant,
             AsyncIngressContentTypeInvariant
    <2>4. Head(IngressLane(
               node, asyncIngressReady[node][index])) =
             IngressLane(
               node, asyncIngressReady[node][index])[1]
      BY <2>2, <2>3, NonemptySequenceHeadIsFirst
         DEF IngressLaneDepth
    <2> QED BY <2>2, <2>3, <2>4
         DEF IngressItemAt, IngressLaneDepth
  <1> QED BY <1>1

THEOREM RunnerServiceFramePreservesClockType ==
  \A node \in ValidatorIds:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ AsyncTransportClockTypeInvariant
    /\ RunnerServiceFrame(node)
    /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines>>
    => AsyncTransportClockTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncRuntimeScalarTypeInvariant,
                AsyncTransportClockTypeInvariant,
                RunnerServiceFrame(node),
                UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                            asyncRetransmitDeadlines>>
         PROVE AsyncTransportClockTypeInvariant'
    <2>1. asyncNow + AsyncDeliveryBound \in Nat
      BY <1>1, SMT
         DEF AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
    <2>2. asyncNodeServiceDeadlines'
               \in [ValidatorIds -> Nat]
      BY <1>1, <2>1, FunctionalUpdatePreservesType
         DEF RunnerServiceFrame, AsyncTransportClockTypeInvariant
    <2> QED BY <1>1, <2>2
         DEF RunnerServiceFrame, AsyncTransportClockTypeInvariant
  <1> QED BY <1>1

THEOREM HistoricalIdleRunnerPreservesSchedulerType ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ RunHistoricalServer(node)
    /\ HistoricalDrainableIngressIndices(node) = {}
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                RunHistoricalServer(node),
                HistoricalDrainableIngressIndices(node) = {}
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>2. HistoricalIdleStep
      BY <1>1 DEF RunHistoricalServer
    <2>3. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
           /\ AsyncDeferredTopologyTypeInvariant
           /\ AsyncDeferredContentTypeInvariant
           /\ AsyncTransportClockTypeInvariant
           /\ AsyncTransportContentTypeInvariant
           /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressCapacityTypeInvariant
           /\ AsyncIngressContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncDeferredTypeInvariant, AsyncTransportTypeInvariant,
             AsyncIngressTypeInvariant
    <2>4. /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
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
      BY <1>1, <2>2, Isa
         DEF RunHistoricalServer, HistoricalIdleStep,
             AsyncRuntimeScalarTypeVars, AsyncIoVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredVars,
             AsyncDeferredTopologyTypeVars,
             AsyncTransportContentTypeVars,
             AsyncIngressTopologyTypeVars, vars
    <2>5. /\ AsyncRuntimeScalarTypeInvariant'
           /\ AsyncCausalTypeInvariant'
           /\ AsyncIoTopologyTypeInvariant'
           /\ AsyncIoContentTypeInvariant'
           /\ AsyncIoCapacityTypeInvariant'
           /\ AsyncDeferredTopologyTypeInvariant'
           /\ AsyncDeferredContentTypeInvariant'
           /\ AsyncTransportContentTypeInvariant'
           /\ AsyncIngressTopologyTypeInvariant'
           /\ AsyncIngressCapacityTypeInvariant'
           /\ AsyncIngressContentTypeInvariant'
      BY <2>3, <2>4, AsyncRuntimeScalarTypeStutter,
         AsyncCausalTypeStutter, AsyncIoTopologyTypeStutter,
         AsyncIoContentTypeStutter, AsyncIoCapacityTypeStutter,
         AsyncDeferredTopologyTypeStutter,
         AsyncDeferredContentTypeStutter,
         AsyncTransportContentTypeStutter,
         AsyncIngressTopologyTypeStutter,
         AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter
    <2>6. /\ asyncOutstandingTags' = asyncOutstandingTags
           /\ asyncNodeDeadlines' = asyncNodeDeadlines
           /\ asyncRetransmitDeadlines' = asyncRetransmitDeadlines
           /\ asyncIoServiceDeadlines' = asyncIoServiceDeadlines
           /\ asyncNodeServiceDeadlines' =
                [asyncNodeServiceDeadlines EXCEPT
                   ![node] = asyncNow + AsyncDeliveryBound]
      BY <1>1, <2>2, Isa DEF RunHistoricalServer, HistoricalIdleStep, vars
    <2>7. asyncNow + AsyncDeliveryBound \in Nat
      BY <2>3, SMT
         DEF AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
    <2>8. asyncNodeServiceDeadlines'
               \in [ValidatorIds -> Nat]
      BY <2>1, <2>3, <2>6, <2>7,
         FunctionalUpdatePreservesType
         DEF AsyncTransportClockTypeInvariant
    <2>9. AsyncTransportClockTypeInvariant'
      BY <2>3, <2>6, <2>8
         DEF AsyncTransportClockTypeInvariant
    <2> QED BY <2>5, <2>9
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM HistoricalDrainRunnerPreservesSchedulerType ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ RunHistoricalServer(node)
    /\ HistoricalDrainableIngressIndices(node) # {}
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                RunHistoricalServer(node),
                HistoricalDrainableIngressIndices(node) # {}
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE DrainIndex == FirstHistoricalDrainableIngressIndex(node)
    <2> DEFINE DrainSource == asyncIngressReady[node][DrainIndex]
    <2> DEFINE DrainItem == IngressItemAt(node, DrainIndex)
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>2. /\ DrainHistoricalIngressSelected(node)
           /\ PopSelectedIngress(node, DrainIndex)
      BY <1>1
         DEF RunHistoricalServer, DrainHistoricalIngressSelected,
             DrainIndex
    <2>3. DrainIndex \in HistoricalDrainableIngressIndices(node)
      BY <1>1, FirstHistoricalDrainableIndexIsDrainable DEF DrainIndex
    <2>4. /\ DrainIndex \in 1..Len(asyncIngressReady[node])
           /\ HistoricalIngressSourceCanDrain(node, DrainSource)
      BY <2>3
         DEF HistoricalDrainableIngressIndices, DrainSource
    <2>5. AsyncIngressTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIngressTypeInvariant
    <2>6. AsyncIngressTypeInvariant'
      BY <2>1, <2>2, <2>4, <2>5,
         PopSelectedIngressPreservesIngressType
    <2>7. AsyncItemTyped(DrainItem)
      BY <2>1, <2>4, <2>5, SelectedIngressItemIsTyped DEF DrainItem
    <2>8. AsyncIoTypeInvariant'
      <3>1. CASE
        \/ /\ DrainItem.kind = "CertifiedRequest"
              /\ DrainItem \in asyncSentItems
              /\ CertifiedRequestAuthorized(DrainItem)
        \/ /\ DrainItem.kind = "CommitCertificateRequest"
              /\ DrainItem \in asyncSentItems
              /\ CommitCertificateRequestAuthorized(DrainItem)
        <4>1. /\ DrainItem.kind
                        \in {"CertifiedRequest",
                              "CommitCertificateRequest"}
               /\ CanEnqueueIoClass(node, "Serve")
          BY <2>4, <3>1, SMT
             DEF HistoricalIngressSourceCanDrain, DrainSource,
                 DrainItem, IngressItemAt
        <4>2. /\ asyncIoQueues' =
                    [asyncIoQueues EXCEPT
                       ![node] = Append(
                         @, AsyncIoCertifiedServeJob(
                              DeliveryCandidate(DrainItem)))]
               /\ UNCHANGED <<asyncCommandQueues,
                               asyncOutstandingWork,
                               asyncIoReadyCompletions,
                               asyncLocalReadyCompletions,
                               asyncNextCompletionSource,
                               asyncIoControlAvailable,
                               asyncDeferredCompletionQueues>>
          BY <1>1, <2>2, <3>1, Isa
             DEF RunHistoricalServer,
                 DrainHistoricalIngressSelected, DrainItem
        <4> QED BY <1>1, <2>1, <2>7, <4>1, <4>2,
             AppendTypedServeJobPreservesIoType
      <3>2. CASE ~(
        \/ /\ DrainItem.kind = "CertifiedRequest"
              /\ DrainItem \in asyncSentItems
              /\ CertifiedRequestAuthorized(DrainItem)
        \/ /\ DrainItem.kind = "CommitCertificateRequest"
              /\ DrainItem \in asyncSentItems
              /\ CommitCertificateRequestAuthorized(DrainItem))
        <4>1. AsyncIoTypeInvariant
          BY <1>1
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncIoTypeInvariant
        <4>2. UNCHANGED AsyncIoContentTypeVars
          BY <1>1, <2>2, <3>2, Isa
             DEF RunHistoricalServer,
                 DrainHistoricalIngressSelected,
                 AsyncIoContentTypeVars, AsyncIoVars, DrainItem
        <4>3. /\ UNCHANGED AsyncIoTopologyTypeVars
               /\ UNCHANGED AsyncIoCapacityTypeVars
          BY <1>1, <2>2, <3>2, Isa
             DEF RunHistoricalServer,
                 DrainHistoricalIngressSelected,
                 AsyncIoTopologyTypeVars, AsyncIoCapacityTypeVars,
                 AsyncIoVars, DrainItem
        <4> QED BY <4>1, <4>2, <4>3,
             AsyncIoTopologyTypeStutter, AsyncIoContentTypeStutter,
             AsyncIoCapacityTypeStutter
             DEF AsyncIoTypeInvariant
      <3> QED BY <3>1, <3>2
    <2>9. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncDeferredTopologyTypeInvariant
           /\ AsyncDeferredContentTypeInvariant
           /\ AsyncTransportClockTypeInvariant
           /\ AsyncTransportContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant
    <2>10. /\ UNCHANGED AsyncRuntimeScalarTypeVars
            /\ UNCHANGED asyncCausalQueues
            /\ UNCHANGED AsyncDeferredTopologyTypeVars
            /\ UNCHANGED <<asyncDeferredCompletionQueues,
                           asyncDeferredProgressQueues,
                           asyncDeferredNormalQueues>>
            /\ UNCHANGED AsyncTransportContentTypeVars
      BY <1>1, <2>2, Isa
         DEF RunHistoricalServer, DrainHistoricalIngressSelected,
             AsyncRuntimeScalarTypeVars, AsyncDeferredVars,
             AsyncDeferredTopologyTypeVars,
             AsyncTransportContentTypeVars, vars
    <2>11. /\ AsyncRuntimeScalarTypeInvariant'
            /\ AsyncCausalTypeInvariant'
            /\ AsyncDeferredTopologyTypeInvariant'
            /\ AsyncDeferredContentTypeInvariant'
            /\ AsyncTransportContentTypeInvariant'
      BY <2>9, <2>10, AsyncRuntimeScalarTypeStutter,
         AsyncCausalTypeStutter, AsyncDeferredTopologyTypeStutter,
         AsyncDeferredContentTypeStutter,
         AsyncTransportContentTypeStutter
    <2>12. /\ RunnerServiceFrame(node)
            /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                           asyncRetransmitDeadlines>>
      BY <1>1, <2>2, Isa
         DEF RunHistoricalServer, DrainHistoricalIngressSelected,
             RunnerServiceFrame, vars
    <2>13. AsyncTransportClockTypeInvariant'
      BY <2>1, <2>9, <2>12,
         RunnerServiceFramePreservesClockType
    <2> QED BY <2>6, <2>8, <2>11, <2>13
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM RunHistoricalServerPreservesSchedulerType ==
  \A node \in AsyncCurrentResponsiveVoters:
    AsyncTypeInvariant /\ RunHistoricalServer(node)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                RunHistoricalServer(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE HistoricalDrainableIngressIndices(node) = {}
      BY <1>1, <2>1, HistoricalIdleRunnerPreservesSchedulerType
    <2>2. CASE HistoricalDrainableIngressIndices(node) # {}
      BY <1>1, <2>2, HistoricalDrainRunnerPreservesSchedulerType
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM RunnerScalarClockAndSchedulerStutterPreservesType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunnerServiceFrame(node)
    /\ asyncRunnerPhase'
         \in [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
    /\ asyncRunnerBudget'
         \in [ValidatorIds ->
               0..(AsyncQueueCapacity + AsyncIngressCapacity)]
    /\ UNCHANGED <<asyncCommandQueues, asyncFifoOwed,
                    asyncTimeoutEmitted,
                    asyncCausalQueues, AsyncIoVars, AsyncDeferredVars,
                    asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines, asyncSentItems,
                    asyncRetainedControl, asyncActiveRequests,
                    asyncTransport, asyncIngressLanes,
                    asyncIngressReady, asyncHeldChunks>>
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                RunnerServiceFrame(node),
                asyncRunnerPhase'
                  \in [ValidatorIds -> {"Local", "Ingress", "Runtime"}],
                asyncRunnerBudget'
                  \in [ValidatorIds ->
                        0..(AsyncQueueCapacity + AsyncIngressCapacity)],
                UNCHANGED <<asyncCommandQueues, asyncFifoOwed,
                            asyncTimeoutEmitted, asyncCausalQueues,
                            AsyncIoVars, AsyncDeferredVars,
                            asyncOutstandingTags, asyncNodeDeadlines,
                            asyncRetransmitDeadlines, asyncSentItems,
                            asyncRetainedControl, asyncActiveRequests,
                            asyncTransport, asyncIngressLanes,
                            asyncIngressReady, asyncHeldChunks>>
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
           /\ AsyncDeferredTopologyTypeInvariant
           /\ AsyncDeferredContentTypeInvariant
           /\ AsyncTransportClockTypeInvariant
           /\ AsyncTransportContentTypeInvariant
           /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressCapacityTypeInvariant
           /\ AsyncIngressContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncDeferredTypeInvariant, AsyncTransportTypeInvariant,
             AsyncIngressTypeInvariant
    <2>2. AsyncRuntimeScalarTypeInvariant'
      BY <1>1, <2>1, Isa
         DEF RunnerServiceFrame, AsyncRuntimeScalarTypeInvariant,
             AsyncIoVars, AsyncDeferredVars
    <2>3. AsyncTransportClockTypeInvariant'
      BY <1>1, <2>1, RunnerServiceFramePreservesClockType,
         Isa DEF AsyncIoVars, AsyncDeferredVars
    <2>4. /\ UNCHANGED asyncCausalQueues
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
      BY <1>1, Isa
         DEF AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportContentTypeVars,
             AsyncIngressTopologyTypeVars
    <2>5. /\ AsyncCausalTypeInvariant'
           /\ AsyncIoTopologyTypeInvariant'
           /\ AsyncIoContentTypeInvariant'
           /\ AsyncIoCapacityTypeInvariant'
           /\ AsyncDeferredTopologyTypeInvariant'
           /\ AsyncDeferredContentTypeInvariant'
           /\ AsyncTransportContentTypeInvariant'
           /\ AsyncIngressTopologyTypeInvariant'
           /\ AsyncIngressCapacityTypeInvariant'
           /\ AsyncIngressContentTypeInvariant'
      BY <2>1, <2>4, AsyncCausalTypeStutter,
         AsyncIoTopologyTypeStutter, AsyncIoContentTypeStutter,
         AsyncIoCapacityTypeStutter, AsyncDeferredTopologyTypeStutter,
         AsyncDeferredContentTypeStutter,
         AsyncTransportContentTypeStutter,
         AsyncIngressTopologyTypeStutter,
         AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter
    <2> QED BY <2>2, <2>3, <2>5
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM LocalAdmissionPhaseAdvancePreservesSchedulerType ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ RunNode(node)
    /\ LocalAdmissionStep(node)
    /\ ~(asyncRunnerBudget[node] > 0
           /\ ProducerCompletionCanAdmit(node))
    /\ ~(asyncRunnerBudget[node] > 0
           /\ CausalHeadCanAdvance(node))
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                RunNode(node),
                LocalAdmissionStep(node),
                ~(asyncRunnerBudget[node] > 0
                    /\ ProducerCompletionCanAdmit(node)),
                ~(asyncRunnerBudget[node] > 0
                    /\ CausalHeadCanAdvance(node))
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>2. /\ RunnerServiceFrame(node)
           /\ asyncRunnerPhase' =
                [asyncRunnerPhase EXCEPT ![node] = "Ingress"]
           /\ asyncRunnerBudget' =
                [asyncRunnerBudget EXCEPT
                   ![node] = AsyncIngressCapacity]
           /\ UNCHANGED <<asyncCommandQueues, asyncFifoOwed,
                          asyncTimeoutEmitted, asyncCausalQueues,
                          AsyncIoVars, AsyncDeferredVars,
                          asyncOutstandingTags, asyncNodeDeadlines,
                          asyncRetransmitDeadlines, asyncSentItems,
                          asyncRetainedControl, asyncActiveRequests,
                          asyncTransport, asyncIngressLanes,
                          asyncIngressReady, asyncHeldChunks>>
      BY <1>1, Isa
         DEF RunNode, RunnerServiceFrame, LocalAdmissionStep,
             LeaveCausalQueues, vars
    <2>3. /\ asyncRunnerPhase
                    \in [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
           /\ asyncRunnerBudget
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
           /\ AsyncIngressCapacity
                    \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
      BY <1>1, SMT
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncConfiguration
    <2>4. /\ asyncRunnerPhase'
                    \in [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
           /\ asyncRunnerBudget'
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
      BY <2>1, <2>2, <2>3, FunctionalUpdatePreservesType
    <2> QED BY <1>1, <2>1, <2>2, <2>4,
                    RunnerScalarClockAndSchedulerStutterPreservesType
  <1> QED BY <1>1

THEOREM AdmitProducerCompletionPreservesIoTopologyType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ ProducerCompletionCanAdmit(node)
    /\ AdmitProducerCompletion(node)
    => AsyncIoTopologyTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                ProducerCompletionCanAdmit(node),
                AdmitProducerCompletion(node)
         PROVE AsyncIoTopologyTypeInvariant'
    <2>1. AsyncIoTopologyTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant
    <2>2. SelectedCompletionSource(node) \in {"Io", "Local"}
      BY <2>1, SMT
         DEF AsyncIoTopologyTypeInvariant, SelectedCompletionSource
    <2>3. /\ asyncIoQueues' = asyncIoQueues
           /\ asyncOutstandingWork' =
                [asyncOutstandingWork EXCEPT
                   ![node] = @ \ {SelectedCompletionCandidate(node)}]
           /\ asyncIoReadyCompletions' =
                (IF SelectedCompletionSource(node) = "Io"
                 THEN [asyncIoReadyCompletions EXCEPT
                         ![node] = Tail(@)]
                 ELSE asyncIoReadyCompletions)
           /\ asyncLocalReadyCompletions' =
                (IF SelectedCompletionSource(node) = "Local"
                 THEN [asyncLocalReadyCompletions EXCEPT
                         ![node] = Tail(@)]
                 ELSE asyncLocalReadyCompletions)
           /\ asyncNextCompletionSource' =
                [asyncNextCompletionSource EXCEPT
                   ![node] =
                     IF SelectedCompletionSource(node) = "Io"
                     THEN "Local" ELSE "Io"]
           /\ asyncIoControlAvailable' = asyncIoControlAvailable
      BY <1>1, Isa
         DEF AdmitProducerCompletion, EnqueueCandidate
    <2>4. /\ DOMAIN asyncOutstandingWork' = ValidatorIds
           /\ DOMAIN asyncIoReadyCompletions' = ValidatorIds
           /\ DOMAIN asyncLocalReadyCompletions' = ValidatorIds
           /\ asyncNextCompletionSource'
                \in [ValidatorIds -> {"Io", "Local"}]
      BY <1>1, <2>1, <2>2, <2>3,
         FunctionalUpdatePreservesType, Isa
         DEF AsyncIoTopologyTypeInvariant
    <2> QED BY <2>1, <2>3, <2>4
         DEF AsyncIoTopologyTypeInvariant
  <1> QED BY <1>1

THEOREM AdmitProducerCompletionPreservesIoQueueContentType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ ProducerCompletionCanAdmit(node)
    /\ AdmitProducerCompletion(node)
    => AsyncIoQueueContentTypeInvariant'
BY ProducerSelectedCompletionFacts, UniqueCompletionTailFacts,
   FunctionalUpdateAwayFromKey, SMTT(30)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
       AsyncIoContentTypeInvariant, AsyncIoTopologyTypeInvariant,
       AsyncIoQueueContentTypeInvariant,
       AsyncIoWorkContentTypeInvariant,
       AsyncIoConsensusCandidateOwnership,
       AsyncIoConsensusQueueOwnership, AsyncIoConsensusIndices,
       AdmitProducerCompletion, EnqueueCandidate,
       ProducerSelectedReadyQueue, ProducerOtherReadyQueue,
       SelectedCompletionSource, SequenceSet

THEOREM ProducerCompletionMovePreservesWorkFacts ==
  \A node, commandQueue, selectedQueue, otherQueue,
     outstanding, candidate:
    /\ node \in ValidatorIds
    /\ AsyncQueueTyped(commandQueue)
    /\ IsFiniteSet(outstanding)
    /\ \A work \in outstanding:
         /\ AsyncCandidateTyped(work)
         /\ work.class = "Completion"
         /\ work.node = node
    /\ AsyncCompletionSequenceTyped(selectedQueue)
    /\ AsyncCompletionSequenceTyped(otherQueue)
    /\ Len(selectedQueue) =
         Cardinality(SequenceSet(selectedQueue))
    /\ Len(otherQueue) =
         Cardinality(SequenceSet(otherQueue))
    /\ SequenceSet(selectedQueue) \subseteq outstanding
    /\ SequenceSet(otherQueue) \subseteq outstanding
    /\ SequenceSet(selectedQueue) \cap
         SequenceSet(otherQueue) = {}
    /\ SequenceSet(commandQueue) \cap outstanding = {}
    /\ Len(selectedQueue) > 0
    /\ candidate = Head(selectedQueue)
    /\ candidate \in SequenceSet(selectedQueue)
    => LET remaining == outstanding \ {candidate}
           selectedTail == Tail(selectedQueue)
           commandWithCandidate == Append(commandQueue, candidate)
       IN /\ IsFiniteSet(remaining)
          /\ \A work \in remaining:
               /\ AsyncCandidateTyped(work)
               /\ work.class = "Completion"
               /\ work.node = node
          /\ AsyncCompletionSequenceTyped(selectedTail)
          /\ AsyncCompletionSequenceTyped(otherQueue)
          /\ Len(selectedTail) =
               Cardinality(SequenceSet(selectedTail))
          /\ Len(otherQueue) =
               Cardinality(SequenceSet(otherQueue))
          /\ SequenceSet(selectedTail) \subseteq remaining
          /\ SequenceSet(otherQueue) \subseteq remaining
          /\ SequenceSet(selectedTail) \cap
               SequenceSet(otherQueue) = {}
          /\ AsyncQueueTyped(commandWithCandidate)
          /\ SequenceSet(commandWithCandidate) \cap remaining = {}
PROOF
  <1>1. ASSUME NEW node, NEW commandQueue, NEW selectedQueue,
                NEW otherQueue, NEW outstanding, NEW candidate,
                node \in ValidatorIds,
                AsyncQueueTyped(commandQueue),
                IsFiniteSet(outstanding),
                \A work \in outstanding:
                  /\ AsyncCandidateTyped(work)
                  /\ work.class = "Completion"
                  /\ work.node = node,
                AsyncCompletionSequenceTyped(selectedQueue),
                AsyncCompletionSequenceTyped(otherQueue),
                Len(selectedQueue) =
                  Cardinality(SequenceSet(selectedQueue)),
                Len(otherQueue) =
                  Cardinality(SequenceSet(otherQueue)),
                SequenceSet(selectedQueue) \subseteq outstanding,
                SequenceSet(otherQueue) \subseteq outstanding,
                SequenceSet(selectedQueue) \cap
                  SequenceSet(otherQueue) = {},
                SequenceSet(commandQueue) \cap outstanding = {},
                Len(selectedQueue) > 0,
                candidate = Head(selectedQueue),
                candidate \in SequenceSet(selectedQueue)
         PROVE LET remaining == outstanding \ {candidate}
                   selectedTail == Tail(selectedQueue)
                   commandWithCandidate ==
                     Append(commandQueue, candidate)
               IN /\ IsFiniteSet(remaining)
                  /\ \A work \in remaining:
                       /\ AsyncCandidateTyped(work)
                       /\ work.class = "Completion"
                       /\ work.node = node
                  /\ AsyncCompletionSequenceTyped(selectedTail)
                  /\ AsyncCompletionSequenceTyped(otherQueue)
                  /\ Len(selectedTail) =
                       Cardinality(SequenceSet(selectedTail))
                  /\ Len(otherQueue) =
                       Cardinality(SequenceSet(otherQueue))
                  /\ SequenceSet(selectedTail) \subseteq remaining
                  /\ SequenceSet(otherQueue) \subseteq remaining
                  /\ SequenceSet(selectedTail) \cap
                       SequenceSet(otherQueue) = {}
                  /\ AsyncQueueTyped(commandWithCandidate)
                  /\ SequenceSet(commandWithCandidate) \cap
                       remaining = {}
    <2>1. /\ AsyncCandidateTyped(candidate)
           /\ candidate.class = "Completion"
           /\ candidate.node = node
      BY <1>1, SMT
    <2>2. /\ AsyncCompletionSequenceTyped(Tail(selectedQueue))
           /\ SequenceSet(Tail(selectedQueue)) =
                SequenceSet(selectedQueue) \ {candidate}
           /\ Len(Tail(selectedQueue)) =
                Cardinality(SequenceSet(Tail(selectedQueue)))
      BY <1>1, UniqueCompletionTailFacts
    <2>3. AsyncQueueTyped(Append(commandQueue, candidate))
      BY <1>1, <2>1, TypedCandidateAppendPreservesQueueType
    <2>4. SequenceSet(Append(commandQueue, candidate)) =
             SequenceSet(commandQueue) \cup {candidate}
      BY <1>1, SequenceSetAfterAppend DEF AsyncQueueTyped
    <2>5. /\ IsFiniteSet(outstanding \ {candidate})
           /\ \A work \in outstanding \ {candidate}:
                /\ AsyncCandidateTyped(work)
                /\ work.class = "Completion"
                /\ work.node = node
      BY <1>1, FS_RemoveElement, SMT
    <2>6. candidate \notin SequenceSet(otherQueue)
      BY <1>1, SMT
    <2>7. /\ SequenceSet(Tail(selectedQueue)) \subseteq
                  outstanding \ {candidate}
           /\ SequenceSet(otherQueue) \subseteq
                  outstanding \ {candidate}
           /\ SequenceSet(Tail(selectedQueue)) \cap
                  SequenceSet(otherQueue) = {}
      BY <1>1, <2>2, <2>6, SMT
    <2>8. SequenceSet(Append(commandQueue, candidate)) \cap
             (outstanding \ {candidate}) = {}
      BY <1>1, <2>4, SMT
    <2> QED BY <1>1, <2>2, <2>3, <2>5, <2>7, <2>8
  <1> QED BY <1>1

THEOREM AdmitProducerCompletionPreservesIoWorkContentType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ ProducerCompletionCanAdmit(node)
    /\ AdmitProducerCompletion(node)
    => AsyncIoWorkContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                ProducerCompletionCanAdmit(node),
                AdmitProducerCompletion(node)
         PROVE AsyncIoWorkContentTypeInvariant'
    <2> DEFINE Candidate == SelectedCompletionCandidate(node)
    <2> DEFINE Selected == ProducerSelectedReadyQueue(node)
    <2> DEFINE Other == ProducerOtherReadyQueue(node)
    <2>1. /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>2. /\ SelectedCompletionSource(node) \in {"Io", "Local"}
           /\ AsyncCompletionSequenceTyped(Selected)
           /\ Len(Selected) = Cardinality(SequenceSet(Selected))
           /\ Len(Selected) > 0
           /\ Candidate = Head(Selected)
           /\ Candidate \in SequenceSet(Selected)
           /\ Candidate \in asyncOutstandingWork[node]
           /\ AsyncCandidateTyped(Candidate)
           /\ Candidate.class = "Completion"
           /\ Candidate.node = node
           /\ Candidate \notin SequenceSet(Other)
      BY <1>1, <2>1, ProducerSelectedCompletionFacts
         DEF Candidate, Selected, Other
    <2>3. /\ DOMAIN asyncCommandQueues = ValidatorIds
           /\ AsyncQueueTyped(asyncCommandQueues[node])
           /\ IsFiniteSet(asyncOutstandingWork[node])
           /\ (\A work \in asyncOutstandingWork[node]:
                 /\ AsyncCandidateTyped(work)
                 /\ work.class = "Completion"
                 /\ work.node = node)
           /\ SequenceSet(asyncCommandQueues[node]) \cap
                asyncOutstandingWork[node] = {}
      BY <1>1, <2>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncIoWorkContentTypeInvariant
    <2>4. /\ AsyncCompletionSequenceTyped(
                    asyncIoReadyCompletions[node])
           /\ AsyncCompletionSequenceTyped(
                    asyncLocalReadyCompletions[node])
           /\ Len(asyncIoReadyCompletions[node]) =
                Cardinality(SequenceSet(
                  asyncIoReadyCompletions[node]))
           /\ Len(asyncLocalReadyCompletions[node]) =
                Cardinality(SequenceSet(
                  asyncLocalReadyCompletions[node]))
           /\ SequenceSet(asyncIoReadyCompletions[node]) \subseteq
                asyncOutstandingWork[node]
           /\ SequenceSet(asyncLocalReadyCompletions[node]) \subseteq
                asyncOutstandingWork[node]
           /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                SequenceSet(asyncLocalReadyCompletions[node]) = {}
      BY <2>1 DEF AsyncIoWorkContentTypeInvariant
    <2>5. /\ AsyncCompletionSequenceTyped(Other)
           /\ Len(Other) = Cardinality(SequenceSet(Other))
           /\ SequenceSet(Selected) \subseteq
                asyncOutstandingWork[node]
           /\ SequenceSet(Other) \subseteq
                asyncOutstandingWork[node]
           /\ SequenceSet(Selected) \cap SequenceSet(Other) = {}
      <3>1. CASE SelectedCompletionSource(node) = "Io"
        BY <2>4, <3>1
           DEF Selected, Other, ProducerSelectedReadyQueue,
               ProducerOtherReadyQueue
      <3>2. CASE SelectedCompletionSource(node) = "Local"
        BY <2>4, <3>2, SMT
           DEF Selected, Other, ProducerSelectedReadyQueue,
               ProducerOtherReadyQueue
      <3> QED BY <2>2, <3>1, <3>2
    <2>6. LET Remaining ==
                     asyncOutstandingWork[node] \ {Candidate}
               SelectedTail == Tail(Selected)
               CommandWithCandidate ==
                     Append(asyncCommandQueues[node], Candidate)
           IN /\ IsFiniteSet(Remaining)
              /\ \A work \in Remaining:
                   /\ AsyncCandidateTyped(work)
                   /\ work.class = "Completion"
                   /\ work.node = node
              /\ AsyncCompletionSequenceTyped(SelectedTail)
              /\ AsyncCompletionSequenceTyped(Other)
              /\ Len(SelectedTail) =
                   Cardinality(SequenceSet(SelectedTail))
              /\ Len(Other) = Cardinality(SequenceSet(Other))
              /\ SequenceSet(SelectedTail) \subseteq Remaining
              /\ SequenceSet(Other) \subseteq Remaining
              /\ SequenceSet(SelectedTail) \cap
                   SequenceSet(Other) = {}
              /\ AsyncQueueTyped(CommandWithCandidate)
              /\ SequenceSet(CommandWithCandidate) \cap Remaining = {}
      BY <1>1, <2>2, <2>3, <2>5,
         ProducerCompletionMovePreservesWorkFacts
         DEF Candidate, Selected, Other
    <2>7. \A otherNode \in ValidatorIds:
             /\ IsFiniteSet(asyncOutstandingWork'[otherNode])
             /\ \A candidate \in asyncOutstandingWork'[otherNode]:
                  /\ AsyncCandidateTyped(candidate)
                  /\ candidate.class = "Completion"
                  /\ candidate.node = otherNode
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions'[otherNode])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions'[otherNode])
             /\ Len(asyncIoReadyCompletions'[otherNode]) =
                  Cardinality(SequenceSet(
                    asyncIoReadyCompletions'[otherNode]))
             /\ Len(asyncLocalReadyCompletions'[otherNode]) =
                  Cardinality(SequenceSet(
                    asyncLocalReadyCompletions'[otherNode]))
             /\ SequenceSet(asyncIoReadyCompletions'[otherNode])
                  \subseteq asyncOutstandingWork'[otherNode]
             /\ SequenceSet(asyncLocalReadyCompletions'[otherNode])
                  \subseteq asyncOutstandingWork'[otherNode]
             /\ SequenceSet(asyncIoReadyCompletions'[otherNode]) \cap
                  SequenceSet(asyncLocalReadyCompletions'[otherNode]) = {}
             /\ SequenceSet(asyncCommandQueues'[otherNode]) \cap
                  asyncOutstandingWork'[otherNode] = {}
      <3>1. ASSUME NEW otherNode \in ValidatorIds
             PROVE /\ IsFiniteSet(asyncOutstandingWork'[otherNode])
                   /\ \A candidate \in
                          asyncOutstandingWork'[otherNode]:
                        /\ AsyncCandidateTyped(candidate)
                        /\ candidate.class = "Completion"
                        /\ candidate.node = otherNode
                   /\ AsyncCompletionSequenceTyped(
                        asyncIoReadyCompletions'[otherNode])
                   /\ AsyncCompletionSequenceTyped(
                        asyncLocalReadyCompletions'[otherNode])
                   /\ Len(asyncIoReadyCompletions'[otherNode]) =
                        Cardinality(SequenceSet(
                          asyncIoReadyCompletions'[otherNode]))
                   /\ Len(asyncLocalReadyCompletions'[otherNode]) =
                        Cardinality(SequenceSet(
                          asyncLocalReadyCompletions'[otherNode]))
                   /\ SequenceSet(
                        asyncIoReadyCompletions'[otherNode])
                        \subseteq asyncOutstandingWork'[otherNode]
                   /\ SequenceSet(
                        asyncLocalReadyCompletions'[otherNode])
                        \subseteq asyncOutstandingWork'[otherNode]
                   /\ SequenceSet(
                        asyncIoReadyCompletions'[otherNode]) \cap
                        SequenceSet(
                          asyncLocalReadyCompletions'[otherNode]) = {}
                   /\ SequenceSet(asyncCommandQueues'[otherNode]) \cap
                        asyncOutstandingWork'[otherNode] = {}
        <4>1. CASE otherNode = node
          <5>1. CASE SelectedCompletionSource(node) = "Io"
            <6>1. /\ asyncOutstandingWork'[node] =
                              asyncOutstandingWork[node] \ {Candidate}
                   /\ asyncCommandQueues'[node] =
                              Append(asyncCommandQueues[node], Candidate)
                   /\ asyncIoReadyCompletions'[node] = Tail(Selected)
                   /\ asyncLocalReadyCompletions'[node] = Other
              BY <1>1, <2>1, <2>2, <2>3, <5>1,
                 FunctionalAppendUpdateAtKey,
                 FunctionalTailUpdateAtKey, Isa
                 DEF AsyncIoTopologyTypeInvariant,
                     AdmitProducerCompletion, EnqueueCandidate,
                     Candidate, Selected, Other,
                     ProducerSelectedReadyQueue,
                     ProducerOtherReadyQueue
            <6> QED BY <2>6, <4>1, <6>1
                 DEF Candidate, Selected, Other
          <5>2. CASE SelectedCompletionSource(node) = "Local"
            <6>1. /\ asyncOutstandingWork'[node] =
                              asyncOutstandingWork[node] \ {Candidate}
                   /\ asyncCommandQueues'[node] =
                              Append(asyncCommandQueues[node], Candidate)
                   /\ asyncIoReadyCompletions'[node] = Other
                   /\ asyncLocalReadyCompletions'[node] = Tail(Selected)
              BY <1>1, <2>1, <2>2, <2>3, <5>2,
                 FunctionalAppendUpdateAtKey,
                 FunctionalTailUpdateAtKey, Isa
                 DEF AsyncIoTopologyTypeInvariant,
                     AdmitProducerCompletion, EnqueueCandidate,
                     Candidate, Selected, Other,
                     ProducerSelectedReadyQueue,
                     ProducerOtherReadyQueue
            <6> QED BY <2>6, <4>1, <6>1
                 DEF Candidate, Selected, Other
          <5> QED BY <2>2, <5>1, <5>2
        <4>2. CASE otherNode # node
          <5>1. CASE SelectedCompletionSource(node) = "Io"
            <6>1. /\ asyncOutstandingWork'[otherNode] =
                              asyncOutstandingWork[otherNode]
                   /\ asyncCommandQueues'[otherNode] =
                              asyncCommandQueues[otherNode]
                   /\ asyncIoReadyCompletions'[otherNode] =
                              asyncIoReadyCompletions[otherNode]
                   /\ asyncLocalReadyCompletions'[otherNode] =
                              asyncLocalReadyCompletions[otherNode]
              BY <1>1, <2>1, <2>2, <2>3, <3>1, <4>2, <5>1,
                 FunctionalUpdateAwayFromKey,
                 FunctionalAppendUpdateAwayFromKey,
                 FunctionalTailUpdateAwayFromKey, Isa
                 DEF AsyncIoTopologyTypeInvariant,
                     AdmitProducerCompletion, EnqueueCandidate,
                     Candidate
            <6> QED BY <2>1, <3>1, <6>1
                 DEF AsyncIoWorkContentTypeInvariant
          <5>2. CASE SelectedCompletionSource(node) = "Local"
            <6>1. /\ asyncOutstandingWork'[otherNode] =
                              asyncOutstandingWork[otherNode]
                   /\ asyncCommandQueues'[otherNode] =
                              asyncCommandQueues[otherNode]
                   /\ asyncIoReadyCompletions'[otherNode] =
                              asyncIoReadyCompletions[otherNode]
                   /\ asyncLocalReadyCompletions'[otherNode] =
                              asyncLocalReadyCompletions[otherNode]
              BY <1>1, <2>1, <2>2, <2>3, <3>1, <4>2, <5>2,
                 FunctionalUpdateAwayFromKey,
                 FunctionalAppendUpdateAwayFromKey,
                 FunctionalTailUpdateAwayFromKey, Isa
                 DEF AsyncIoTopologyTypeInvariant,
                     AdmitProducerCompletion, EnqueueCandidate,
                     Candidate
            <6> QED BY <2>1, <3>1, <6>1
                 DEF AsyncIoWorkContentTypeInvariant
          <5> QED BY <2>1, <2>2, <3>1, <5>1, <5>2
               DEF AsyncIoWorkContentTypeInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>7 DEF AsyncIoWorkContentTypeInvariant
  <1> QED BY <1>1

THEOREM AdmitProducerCompletionWithDeferredFramePreservesIoCapacityType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ ProducerCompletionCanAdmit(node)
    /\ AdmitProducerCompletion(node)
    /\ UNCHANGED asyncDeferredCompletionQueues
    => AsyncIoCapacityTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                ProducerCompletionCanAdmit(node),
                AdmitProducerCompletion(node),
                UNCHANGED asyncDeferredCompletionQueues
         PROVE AsyncIoCapacityTypeInvariant'
    <2> DEFINE Candidate == SelectedCompletionCandidate(node)
    <2>1. /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>2. /\ Candidate \in asyncOutstandingWork[node]
           /\ Candidate.class = "Completion"
           /\ Candidate.node = node
           /\ DOMAIN asyncCommandQueues = ValidatorIds
           /\ AsyncQueueTyped(asyncCommandQueues[node])
           /\ IsFiniteSet(asyncOutstandingWork[node])
      BY <1>1, <2>1, ProducerSelectedCompletionFacts
         DEF Candidate, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoWorkContentTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped
    <2>3. /\ IsFiniteSet(
                    asyncOutstandingWork[node] \ {Candidate})
           /\ Cardinality(asyncOutstandingWork[node]) \in Nat
           /\ Cardinality(asyncOutstandingWork[node]) # 0
           /\ Cardinality(
                asyncOutstandingWork[node] \ {Candidate}) =
                Cardinality(asyncOutstandingWork[node]) - 1
      BY <2>2, FS_RemoveElement, FS_CardinalityType,
         FS_EmptySet, SMT
    <2>4. Cardinality(
              asyncOutstandingWork[node] \ {Candidate}) + 1 =
            Cardinality(asyncOutstandingWork[node])
      BY <2>3, SMT
    <2>5. Cardinality(AsyncCompletionIndices(
              Append(asyncCommandQueues[node], Candidate))) =
            Cardinality(AsyncCompletionIndices(
              asyncCommandQueues[node])) + 1
      BY <2>2, CompletionAppendCountIncreasesByOne
    <2>6. /\ asyncCommandQueues'[node] =
                    Append(asyncCommandQueues[node], Candidate)
           /\ asyncOutstandingWork'[node] =
                    asyncOutstandingWork[node] \ {Candidate}
           /\ asyncIoQueues'[node] = asyncIoQueues[node]
           /\ asyncDeferredCompletionQueues'[node] =
                    asyncDeferredCompletionQueues[node]
      BY <1>1, <2>1, <2>2, FunctionalAppendUpdateAtKey, Isa
         DEF AsyncIoTopologyTypeInvariant,
             AdmitProducerCompletion, EnqueueCandidate, Candidate
    <2>7. Len(Append(asyncCommandQueues[node], Candidate)) =
             Len(asyncCommandQueues[node]) + 1
      BY <2>2, AppendSequenceFacts DEF AsyncQueueTyped
    <2>8. AsyncQueueDepth(node)' = AsyncQueueDepth(node) + 1
      BY <2>6, <2>7, Isa DEF AsyncQueueDepth
    <2>9. AsyncIoQueueDepth(node)' = AsyncIoQueueDepth(node)
      BY <2>6 DEF AsyncIoQueueDepth
    <2>10. AsyncOutstandingWorkCount(node)' + 1 =
             AsyncOutstandingWorkCount(node)
      BY <2>4, <2>6 DEF AsyncOutstandingWorkCount
    <2>11. QueuedCompletionCount(node)' =
              QueuedCompletionCount(node) + 1
      BY <2>5, <2>6
         DEF QueuedCompletionCount, QueuedCompletionIndices,
             AsyncCompletionIndices
    <2>12. DeferredCompletionCount(node)' =
              DeferredCompletionCount(node)
      BY <2>6 DEF DeferredCompletionCount
    <2>13. AsyncOutstandingWorkCount(node) \in Nat
      BY <2>2, FS_CardinalityType DEF AsyncOutstandingWorkCount
    <2>14. /\ Len(asyncCommandQueues[node]) \in Nat
            /\ IsFiniteSet(1..Len(asyncCommandQueues[node]))
      BY <2>2, LenProperties, FS_Interval, SMT DEF AsyncQueueTyped
    <2>15. /\ QueuedCompletionIndices(node)
                  \subseteq 1..Len(asyncCommandQueues[node])
            /\ IsFiniteSet(QueuedCompletionIndices(node))
      BY <2>14, FS_Subset DEF QueuedCompletionIndices
    <2>16. QueuedCompletionCount(node) \in Nat
      BY <2>15, FS_CardinalityType DEF QueuedCompletionCount
    <2>17. asyncDeferredCompletionQueues[node]
                \in Seq(Range(asyncDeferredCompletionQueues[node]))
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncDeferredTypeInvariant,
             AsyncDeferredContentTypeInvariant,
             AsyncCompletionSequenceTyped
    <2>18. DeferredCompletionCount(node) \in Nat
      BY <2>17, LenProperties DEF DeferredCompletionCount
    <2>19. Cardinality(
                asyncOutstandingWork[node] \ {Candidate}) \in Nat
      BY <2>3, FS_CardinalityType
    <2>20. AsyncOutstandingWorkCount(node)' \in Nat
      BY <2>6, <2>19 DEF AsyncOutstandingWorkCount
    <2>21. QueuedCompletionCount(node)' \in Nat
      BY <2>11, <2>16, SMT
    <2>22. DeferredCompletionCount(node)' \in Nat
      BY <2>12, <2>18, SMT
    <2>23. AsyncCompletionLoad(node)' = AsyncCompletionLoad(node)
      BY <2>10, <2>11, <2>12, <2>13, <2>16, <2>18,
         <2>20, <2>21, <2>22, SMT
         DEF AsyncCompletionLoad
    <2>24. AsyncQueueDepth(node) < AsyncQueueCapacity
      BY <1>1 DEF ProducerCompletionCanAdmit, CanEnqueueClass
    <2>25. /\ AsyncQueueDepth(node) \in Nat
            /\ AsyncQueueCapacity \in Nat
      BY <1>1, <2>2, LenProperties, SMT
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncConfiguration, AsyncQueueDepth, AsyncQueueTyped
    <2>26. /\ AsyncCompletionLoad(node) <= AsyncCompletionReserve
            /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
            /\ AsyncCompletionLoad(node) <= AsyncIoWorkCapacity
      BY <2>1 DEF AsyncIoCapacityTypeInvariant
    <2>27. AsyncQueueDepth(node)' <= AsyncQueueCapacity
      BY <2>8, <2>24, <2>25, SMT
    <2>28. \A otherNode \in ValidatorIds:
             /\ AsyncQueueDepth(otherNode)' <= AsyncQueueCapacity
             /\ AsyncCompletionLoad(otherNode)' <=
                  AsyncCompletionReserve
             /\ AsyncIoQueueDepth(otherNode)' <= AsyncIoCapacity
             /\ AsyncCompletionLoad(otherNode)' <= AsyncIoWorkCapacity
      <3>1. ASSUME NEW otherNode \in ValidatorIds
             PROVE /\ AsyncQueueDepth(otherNode)' <=
                          AsyncQueueCapacity
                   /\ AsyncCompletionLoad(otherNode)' <=
                          AsyncCompletionReserve
                   /\ AsyncIoQueueDepth(otherNode)' <= AsyncIoCapacity
                   /\ AsyncCompletionLoad(otherNode)' <=
                          AsyncIoWorkCapacity
        <4>1. CASE otherNode = node
          BY <2>9, <2>23, <2>26, <2>27,
             <3>1, <4>1, SMT
        <4>2. CASE otherNode # node
          <5>1. /\ asyncCommandQueues'[otherNode] =
                            asyncCommandQueues[otherNode]
                 /\ asyncOutstandingWork'[otherNode] =
                            asyncOutstandingWork[otherNode]
                 /\ asyncIoQueues'[otherNode] = asyncIoQueues[otherNode]
                 /\ asyncDeferredCompletionQueues'[otherNode] =
                            asyncDeferredCompletionQueues[otherNode]
            BY <1>1, <2>1, <2>2, <3>1, <4>2,
               FunctionalUpdateAwayFromKey, Isa
               DEF AsyncIoTopologyTypeInvariant,
                   AdmitProducerCompletion, EnqueueCandidate
          <5>2. /\ AsyncQueueDepth(otherNode)' =
                            AsyncQueueDepth(otherNode)
                 /\ AsyncCompletionLoad(otherNode)' =
                            AsyncCompletionLoad(otherNode)
                 /\ AsyncIoQueueDepth(otherNode)' =
                            AsyncIoQueueDepth(otherNode)
            BY <5>1, Isa
               DEF AsyncQueueDepth, AsyncIoQueueDepth,
                   AsyncCompletionLoad, AsyncOutstandingWorkCount,
                   QueuedCompletionCount, QueuedCompletionIndices,
                   DeferredCompletionCount
          <5> QED BY <2>1, <3>1, <5>2
               DEF AsyncIoCapacityTypeInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>28 DEF AsyncIoCapacityTypeInvariant
  <1> QED BY <1>1

THEOREM AdmitProducerCompletionWithDeferredFramePreservesIoType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ ProducerCompletionCanAdmit(node)
    /\ AdmitProducerCompletion(node)
    /\ UNCHANGED asyncDeferredCompletionQueues
    => AsyncIoTypeInvariant'
BY AdmitProducerCompletionPreservesIoTopologyType,
   AdmitProducerCompletionPreservesIoQueueContentType,
   AdmitProducerCompletionPreservesIoWorkContentType,
   AdmitProducerCompletionWithDeferredFramePreservesIoCapacityType
   DEF AsyncIoTypeInvariant, AsyncIoContentTypeInvariant

THEOREM ProducerAdmissionRunnerPreservesSchedulerType ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ RunNode(node)
    /\ LocalAdmissionStep(node)
    /\ asyncRunnerBudget[node] > 0
    /\ ProducerCompletionCanAdmit(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                RunNode(node),
                LocalAdmissionStep(node),
                asyncRunnerBudget[node] > 0,
                ProducerCompletionCanAdmit(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>2. /\ AdmitProducerCompletion(node)
           /\ UNCHANGED asyncDeferredCompletionQueues
      BY <1>1 DEF LocalAdmissionStep, AsyncDeferredVars
    <2>3. AsyncIoTypeInvariant'
      BY <1>1, <2>1, <2>2,
         AdmitProducerCompletionWithDeferredFramePreservesIoType
    <2>4. AsyncRuntimeScalarTypeInvariant'
      BY <1>1, <2>1, <2>2, ProducerSelectedCompletionFacts,
         TypedCandidateAppendPreservesQueueType,
         FunctionalUpdatePreservesType, FunctionalUpdateAwayFromKey,
         SMTT(30)
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoTopologyTypeInvariant,
             AsyncIoWorkContentTypeInvariant, RunNode,
             LocalAdmissionStep, AdmitProducerCompletion,
             EnqueueCandidate, ProducerSelectedReadyQueue,
             ProducerOtherReadyQueue, SelectedCompletionSource,
             AsyncConfiguration, vars
    <2>5. /\ AsyncCausalTypeInvariant
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
    <2>6. /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportContentTypeVars
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, <2>2, Isa
         DEF RunNode, LocalAdmissionStep, AdmitProducerCompletion,
             LeaveCausalQueues, AsyncDeferredVars,
             AsyncDeferredTopologyTypeVars,
             AsyncTransportContentTypeVars,
             AsyncIngressTopologyTypeVars, vars
    <2>7. /\ AsyncCausalTypeInvariant'
           /\ AsyncDeferredTopologyTypeInvariant'
           /\ AsyncDeferredContentTypeInvariant'
           /\ AsyncTransportContentTypeInvariant'
           /\ AsyncIngressTopologyTypeInvariant'
           /\ AsyncIngressCapacityTypeInvariant'
           /\ AsyncIngressContentTypeInvariant'
      BY <2>5, <2>6, AsyncCausalTypeStutter,
         AsyncDeferredTopologyTypeStutter,
         AsyncDeferredContentTypeStutter,
         AsyncTransportContentTypeStutter,
         AsyncIngressTopologyTypeStutter,
         AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter
    <2>8. /\ RunnerServiceFrame(node)
           /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                          asyncRetransmitDeadlines>>
      BY <1>1, <2>2, Isa
         DEF RunNode, RunnerServiceFrame, LocalAdmissionStep,
             AdmitProducerCompletion, vars
    <2>9. AsyncTransportClockTypeInvariant'
      BY <1>1, <2>1, <2>5, <2>8,
         RunnerServiceFramePreservesClockType
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant
    <2> QED BY <2>3, <2>4, <2>7, <2>9
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM CausalTailUpdatePreservesCausalType ==
  \A node \in ValidatorIds:
    /\ AsyncCausalTypeInvariant
    /\ CausalQueueNonempty(node)
    /\ asyncCausalQueues' =
         [asyncCausalQueues EXCEPT ![node] = Tail(@)]
    => AsyncCausalTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncCausalTypeInvariant,
                CausalQueueNonempty(node),
                asyncCausalQueues' =
                  [asyncCausalQueues EXCEPT ![node] = Tail(@)]
         PROVE AsyncCausalTypeInvariant'
    <2>1. /\ DOMAIN asyncCausalQueues = ValidatorIds
           /\ AsyncQueueTyped(asyncCausalQueues[node])
           /\ AsyncCausalQueueOwnership(node, asyncCausalQueues[node])
           /\ Len(asyncCausalQueues[node]) > 0
      BY <1>1 DEF AsyncCausalTypeInvariant, CausalQueueNonempty
    <2>2. /\ AsyncQueueTyped(Tail(asyncCausalQueues[node]))
           /\ SequenceSet(Tail(asyncCausalQueues[node]))
                \subseteq SequenceSet(asyncCausalQueues[node])
      BY <2>1, TypedQueueTailFacts
    <2>3. AsyncCausalQueueOwnership(
             node, Tail(asyncCausalQueues[node]))
      BY <2>1, <2>2 DEF AsyncCausalQueueOwnership
    <2>4. DOMAIN asyncCausalQueues' = ValidatorIds
      BY <1>1, <2>1, Isa
    <2>5. \A other \in ValidatorIds:
             /\ AsyncQueueTyped(asyncCausalQueues'[other])
             /\ AsyncCausalQueueOwnership(
                  other, asyncCausalQueues'[other])
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE /\ AsyncQueueTyped(asyncCausalQueues'[other])
                   /\ AsyncCausalQueueOwnership(
                        other, asyncCausalQueues'[other])
        <4>1. CASE other = node
          <5>1. asyncCausalQueues'[other] =
                   Tail(asyncCausalQueues[node])
            BY <1>1, <2>1, <4>1, FunctionalTailUpdateAtKey
          <5> QED BY <2>2, <2>3, <4>1, <5>1
        <4>2. CASE other # node
          <5>1. asyncCausalQueues'[other] = asyncCausalQueues[other]
            BY <1>1, <2>1, <3>1, <4>2,
               FunctionalTailUpdateAwayFromKey
          <5>2. /\ AsyncQueueTyped(asyncCausalQueues[other])
                 /\ AsyncCausalQueueOwnership(
                      other, asyncCausalQueues[other])
            BY <1>1, <3>1 DEF AsyncCausalTypeInvariant
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>4, <2>5 DEF AsyncCausalTypeInvariant
  <1> QED BY <1>1

THEOREM CausalHeadCandidateIsTyped ==
  \A node \in ValidatorIds:
    AsyncCausalTypeInvariant /\ CausalQueueNonempty(node)
      => AsyncCandidateTyped(HeadCausalCandidate(node))
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncCausalTypeInvariant,
                CausalQueueNonempty(node)
         PROVE AsyncCandidateTyped(HeadCausalCandidate(node))
    <2>1. /\ AsyncQueueTyped(asyncCausalQueues[node])
           /\ Len(asyncCausalQueues[node]) > 0
      BY <1>1 DEF AsyncCausalTypeInvariant, CausalQueueNonempty
    <2>2. /\ 1 \in 1..Len(asyncCausalQueues[node])
           /\ Head(asyncCausalQueues[node]) = asyncCausalQueues[node][1]
      BY <2>1, NonemptySequenceHeadIsFirst, SMT
         DEF AsyncQueueTyped
    <2> QED BY <2>1, <2>2
         DEF AsyncQueueTyped, HeadCausalCandidate
  <1> QED BY <1>1

THEOREM CausalHeadCandidateIsOwned ==
  \A node \in ValidatorIds:
    AsyncCausalTypeInvariant /\ CausalQueueNonempty(node)
      => HeadCausalCandidate(node).node = node
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncCausalTypeInvariant,
                CausalQueueNonempty(node)
         PROVE HeadCausalCandidate(node).node = node
    <2>1. /\ AsyncQueueTyped(asyncCausalQueues[node])
           /\ AsyncCausalQueueOwnership(node, asyncCausalQueues[node])
           /\ Len(asyncCausalQueues[node]) > 0
      BY <1>1 DEF AsyncCausalTypeInvariant, CausalQueueNonempty
    <2>2. /\ 1 \in 1..Len(asyncCausalQueues[node])
           /\ Head(asyncCausalQueues[node]) = asyncCausalQueues[node][1]
      BY <2>1, NonemptySequenceHeadIsFirst, SMT
         DEF AsyncQueueTyped
    <2>3. HeadCausalCandidate(node)
             \in SequenceSet(asyncCausalQueues[node])
      BY <2>2 DEF HeadCausalCandidate, SequenceSet
    <2> QED BY <2>1, <2>3 DEF AsyncCausalQueueOwnership
  <1> QED BY <1>1

THEOREM CausalUntrackedCandidateFacts ==
  \A node \in ValidatorIds:
    \A candidate:
      ~CandidateInFlight(candidate)
        => /\ candidate \notin asyncOutstandingWork[node]
           /\ candidate \notin QueuedCandidates
           /\ candidate \notin DeferredCandidates
BY SMTT(30)
   DEF CandidateInFlight, TrackedWorkCandidates

THEOREM ConsensusIndicesAfterConsensusAppend ==
  \A queue, job:
    /\ AsyncIoSequenceTyped(queue)
    /\ job.class = "Consensus"
    => AsyncIoConsensusIndices(Append(queue, job)) =
         AsyncIoConsensusIndices(queue) \cup {Len(queue) + 1}
PROOF
  <1>1. ASSUME NEW queue, NEW job,
                AsyncIoSequenceTyped(queue),
                job.class = "Consensus"
         PROVE AsyncIoConsensusIndices(Append(queue, job)) =
                 AsyncIoConsensusIndices(queue) \cup {Len(queue) + 1}
    <2>1. queue \in Seq(Range(queue))
      BY <1>1 DEF AsyncIoSequenceTyped
    <2>2. /\ Len(queue) \in Nat
           /\ Len(Append(queue, job)) = Len(queue) + 1
           /\ \A index \in 1..Len(queue):
                Append(queue, job)[index] = queue[index]
           /\ Append(queue, job)[Len(queue) + 1] = job
      BY <2>1, AppendSequenceFacts, LenProperties
    <2> QED BY <1>1, <2>2, SMT DEF AsyncIoConsensusIndices
  <1> QED BY <1>1

THEOREM AppendFreshConsensusJobPreservesQueueFacts ==
  \A queue, outstanding, ioReadyQueue, localReadyQueue, candidate:
    /\ AsyncConfiguration
    /\ AsyncIoSequenceTyped(queue)
    /\ (\A job \in SequenceSet(queue):
          job.class = "Consensus" => job.candidate \in outstanding)
    /\ AsyncIoConsensusQueueOwnership(
         queue, ioReadyQueue, localReadyQueue)
    /\ AsyncCandidateTyped(candidate)
    /\ candidate.class = "Completion"
    /\ candidate \notin outstanding
    /\ candidate \notin SequenceSet(ioReadyQueue)
    /\ candidate \notin SequenceSet(localReadyQueue)
    => /\ AsyncIoSequenceTyped(
             Append(queue, AsyncIoConsensusJob(candidate)))
       /\ (\A job \in
                  SequenceSet(
                    Append(queue, AsyncIoConsensusJob(candidate))):
             job.class = "Consensus" =>
               job.candidate \in outstanding \cup {candidate})
       /\ AsyncIoConsensusQueueOwnership(
            Append(queue, AsyncIoConsensusJob(candidate)),
            ioReadyQueue, localReadyQueue)
PROOF
  <1>1. ASSUME NEW queue, NEW outstanding,
                NEW ioReadyQueue, NEW localReadyQueue, NEW candidate,
                AsyncConfiguration,
                AsyncIoSequenceTyped(queue),
                (\A job \in SequenceSet(queue):
                   job.class = "Consensus" =>
                     job.candidate \in outstanding),
                AsyncIoConsensusQueueOwnership(
                  queue, ioReadyQueue, localReadyQueue),
                AsyncCandidateTyped(candidate),
                candidate.class = "Completion",
                candidate \notin outstanding,
                candidate \notin SequenceSet(ioReadyQueue),
                candidate \notin SequenceSet(localReadyQueue)
         PROVE /\ AsyncIoSequenceTyped(
                      Append(queue, AsyncIoConsensusJob(candidate)))
                /\ (\A job \in
                           SequenceSet(
                             Append(queue, AsyncIoConsensusJob(candidate))):
                      job.class = "Consensus" =>
                        job.candidate \in outstanding \cup {candidate})
                /\ AsyncIoConsensusQueueOwnership(
                     Append(queue, AsyncIoConsensusJob(candidate)),
                     ioReadyQueue, localReadyQueue)
    <2> DEFINE NewJob == AsyncIoConsensusJob(candidate)
    <2>1. /\ AsyncIoJobTyped(NewJob)
           /\ NewJob.class = "Consensus"
           /\ NewJob.candidate = candidate
      BY <1>1, TypedCompletionCandidateMakesConsensusJob, SMT
         DEF NewJob, AsyncIoConsensusJob, AsyncIoJob
    <2>2. /\ queue \in Seq(Range(queue))
           /\ SequenceSet(Append(queue, NewJob)) =
                SequenceSet(queue) \cup {NewJob}
           /\ AsyncIoConsensusIndices(Append(queue, NewJob)) =
                AsyncIoConsensusIndices(queue) \cup {Len(queue) + 1}
           /\ (\A index \in 1..Len(queue):
                 Append(queue, NewJob)[index] = queue[index])
           /\ Append(queue, NewJob)[Len(queue) + 1] = NewJob
      BY <1>1, <2>1, SequenceSetAfterAppend,
         ConsensusIndicesAfterConsensusAppend, AppendSequenceFacts
         DEF AsyncIoSequenceTyped
    <2>3. AsyncIoSequenceTyped(Append(queue, NewJob))
      BY <1>1, <2>1, TypedIoAppendPreservesSequenceType
    <2>4. \A job \in SequenceSet(Append(queue, NewJob)):
             job.class = "Consensus" =>
               job.candidate \in outstanding \cup {candidate}
      <3>1. ASSUME NEW job \in SequenceSet(Append(queue, NewJob)),
                    job.class = "Consensus"
             PROVE job.candidate \in outstanding \cup {candidate}
        <4>1. job \in SequenceSet(queue) \cup {NewJob}
          BY <2>2, <3>1
        <4>2. CASE job = NewJob
          BY <2>1, <4>2
        <4>3. CASE job # NewJob
          <5>1. job \in SequenceSet(queue)
            BY <4>1, <4>3
          <5>2. job.candidate \in outstanding
            BY <1>1, <3>1, <5>1
          <5> QED BY <5>2
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>5. /\ (\A index \in AsyncIoConsensusIndices(queue):
                    /\ queue[index].candidate
                         \notin SequenceSet(ioReadyQueue)
                    /\ queue[index].candidate
                         \notin SequenceSet(localReadyQueue))
           /\ (\A left, right \in AsyncIoConsensusIndices(queue):
                 queue[left].candidate = queue[right].candidate =>
                   left = right)
      BY <1>1 DEF AsyncIoConsensusQueueOwnership
    <2>6. \A index \in AsyncIoConsensusIndices(Append(queue, NewJob)):
             \/ /\ index \in AsyncIoConsensusIndices(queue)
                   /\ queue[index] \in SequenceSet(queue)
                   /\ queue[index].class = "Consensus"
                   /\ Append(queue, NewJob)[index] = queue[index]
             \/ /\ index = Len(queue) + 1
                   /\ Append(queue, NewJob)[index] = NewJob
      <3>1. ASSUME NEW index \in
                      AsyncIoConsensusIndices(Append(queue, NewJob))
             PROVE \/ /\ index \in AsyncIoConsensusIndices(queue)
                          /\ queue[index] \in SequenceSet(queue)
                          /\ queue[index].class = "Consensus"
                          /\ Append(queue, NewJob)[index] = queue[index]
                    \/ /\ index = Len(queue) + 1
                          /\ Append(queue, NewJob)[index] = NewJob
        BY <2>2, <3>1, SMT
           DEF AsyncIoConsensusIndices, SequenceSet
      <3> QED BY <3>1
    <2>7. \A index \in AsyncIoConsensusIndices(Append(queue, NewJob)):
             /\ Append(queue, NewJob)[index].candidate
                  \notin SequenceSet(ioReadyQueue)
             /\ Append(queue, NewJob)[index].candidate
                  \notin SequenceSet(localReadyQueue)
      BY <1>1, <2>1, <2>5, <2>6, SMTT(20)
    <2>8. \A left, right \in
                    AsyncIoConsensusIndices(Append(queue, NewJob)):
             Append(queue, NewJob)[left].candidate =
               Append(queue, NewJob)[right].candidate => left = right
      BY <1>1, <2>1, <2>5, <2>6, SMTT(30)
    <2>9. AsyncIoConsensusQueueOwnership(
             Append(queue, NewJob), ioReadyQueue, localReadyQueue)
      BY <2>7, <2>8 DEF AsyncIoConsensusQueueOwnership
    <2> QED BY <2>3, <2>4, <2>9 DEF NewJob
  <1> QED BY <1>1

THEOREM AddFreshCompletionPreservesNodeWorkFacts ==
  \A node, commandQueue, outstanding, ioReadyQueue, localReadyQueue,
     candidate:
    /\ IsFiniteSet(outstanding)
    /\ (\A work \in outstanding:
          /\ AsyncCandidateTyped(work)
          /\ work.class = "Completion"
          /\ work.node = node)
    /\ AsyncCompletionSequenceTyped(ioReadyQueue)
    /\ AsyncCompletionSequenceTyped(localReadyQueue)
    /\ Len(ioReadyQueue) = Cardinality(SequenceSet(ioReadyQueue))
    /\ Len(localReadyQueue) = Cardinality(SequenceSet(localReadyQueue))
    /\ SequenceSet(ioReadyQueue) \subseteq outstanding
    /\ SequenceSet(localReadyQueue) \subseteq outstanding
    /\ SequenceSet(ioReadyQueue) \cap SequenceSet(localReadyQueue) = {}
    /\ SequenceSet(commandQueue) \cap outstanding = {}
    /\ AsyncCandidateTyped(candidate)
    /\ candidate.class = "Completion"
    /\ candidate.node = node
    /\ candidate \notin outstanding
    /\ candidate \notin SequenceSet(commandQueue)
    => /\ IsFiniteSet(outstanding \cup {candidate})
       /\ (\A work \in outstanding \cup {candidate}:
             /\ AsyncCandidateTyped(work)
             /\ work.class = "Completion"
             /\ work.node = node)
       /\ AsyncCompletionSequenceTyped(ioReadyQueue)
       /\ AsyncCompletionSequenceTyped(localReadyQueue)
       /\ Len(ioReadyQueue) = Cardinality(SequenceSet(ioReadyQueue))
       /\ Len(localReadyQueue) = Cardinality(SequenceSet(localReadyQueue))
       /\ SequenceSet(ioReadyQueue) \subseteq outstanding \cup {candidate}
       /\ SequenceSet(localReadyQueue) \subseteq outstanding \cup {candidate}
       /\ SequenceSet(ioReadyQueue) \cap SequenceSet(localReadyQueue) = {}
       /\ SequenceSet(commandQueue) \cap (outstanding \cup {candidate}) = {}
BY FS_AddElement, SMTT(30)

THEOREM AppendFreshLocalCompletionPreservesNodeWorkFacts ==
  \A node, commandQueue, outstanding, ioReadyQueue, localReadyQueue,
     candidate:
    /\ IsFiniteSet(outstanding)
    /\ (\A work \in outstanding:
          /\ AsyncCandidateTyped(work)
          /\ work.class = "Completion"
          /\ work.node = node)
    /\ AsyncCompletionSequenceTyped(ioReadyQueue)
    /\ AsyncCompletionSequenceTyped(localReadyQueue)
    /\ Len(ioReadyQueue) = Cardinality(SequenceSet(ioReadyQueue))
    /\ Len(localReadyQueue) = Cardinality(SequenceSet(localReadyQueue))
    /\ SequenceSet(ioReadyQueue) \subseteq outstanding
    /\ SequenceSet(localReadyQueue) \subseteq outstanding
    /\ SequenceSet(ioReadyQueue) \cap SequenceSet(localReadyQueue) = {}
    /\ SequenceSet(commandQueue) \cap outstanding = {}
    /\ AsyncCandidateTyped(candidate)
    /\ candidate.class = "Completion"
    /\ candidate.node = node
    /\ candidate \notin outstanding
    /\ candidate \notin SequenceSet(commandQueue)
    => /\ IsFiniteSet(outstanding \cup {candidate})
       /\ (\A work \in outstanding \cup {candidate}:
             /\ AsyncCandidateTyped(work)
             /\ work.class = "Completion"
             /\ work.node = node)
       /\ AsyncCompletionSequenceTyped(ioReadyQueue)
       /\ AsyncCompletionSequenceTyped(Append(localReadyQueue, candidate))
       /\ Len(ioReadyQueue) = Cardinality(SequenceSet(ioReadyQueue))
       /\ Len(Append(localReadyQueue, candidate)) =
            Cardinality(SequenceSet(Append(localReadyQueue, candidate)))
       /\ SequenceSet(ioReadyQueue) \subseteq outstanding \cup {candidate}
       /\ SequenceSet(Append(localReadyQueue, candidate))
            \subseteq outstanding \cup {candidate}
       /\ SequenceSet(ioReadyQueue) \cap
            SequenceSet(Append(localReadyQueue, candidate)) = {}
       /\ SequenceSet(commandQueue) \cap
            (outstanding \cup {candidate}) = {}
PROOF
  <1>1. ASSUME NEW node, NEW commandQueue, NEW outstanding,
                NEW ioReadyQueue, NEW localReadyQueue, NEW candidate,
                IsFiniteSet(outstanding),
                \A work \in outstanding:
                  /\ AsyncCandidateTyped(work)
                  /\ work.class = "Completion"
                  /\ work.node = node,
                AsyncCompletionSequenceTyped(ioReadyQueue),
                AsyncCompletionSequenceTyped(localReadyQueue),
                Len(ioReadyQueue) =
                  Cardinality(SequenceSet(ioReadyQueue)),
                Len(localReadyQueue) =
                  Cardinality(SequenceSet(localReadyQueue)),
                SequenceSet(ioReadyQueue) \subseteq outstanding,
                SequenceSet(localReadyQueue) \subseteq outstanding,
                SequenceSet(ioReadyQueue) \cap
                  SequenceSet(localReadyQueue) = {},
                SequenceSet(commandQueue) \cap outstanding = {},
                AsyncCandidateTyped(candidate),
                candidate.class = "Completion",
                candidate.node = node,
                candidate \notin outstanding,
                candidate \notin SequenceSet(commandQueue)
         PROVE /\ IsFiniteSet(outstanding \cup {candidate})
               /\ (\A work \in outstanding \cup {candidate}:
                     /\ AsyncCandidateTyped(work)
                     /\ work.class = "Completion"
                     /\ work.node = node)
               /\ AsyncCompletionSequenceTyped(ioReadyQueue)
               /\ AsyncCompletionSequenceTyped(
                    Append(localReadyQueue, candidate))
               /\ Len(ioReadyQueue) =
                    Cardinality(SequenceSet(ioReadyQueue))
               /\ Len(Append(localReadyQueue, candidate)) =
                    Cardinality(
                      SequenceSet(Append(localReadyQueue, candidate)))
               /\ SequenceSet(ioReadyQueue)
                    \subseteq outstanding \cup {candidate}
               /\ SequenceSet(Append(localReadyQueue, candidate))
                    \subseteq outstanding \cup {candidate}
               /\ SequenceSet(ioReadyQueue) \cap
                    SequenceSet(Append(localReadyQueue, candidate)) = {}
               /\ SequenceSet(commandQueue) \cap
                    (outstanding \cup {candidate}) = {}
    <2>1. /\ IsFiniteSet(outstanding \cup {candidate})
           /\ (\A work \in outstanding \cup {candidate}:
                 /\ AsyncCandidateTyped(work)
                 /\ work.class = "Completion"
                 /\ work.node = node)
           /\ SequenceSet(ioReadyQueue)
                \subseteq outstanding \cup {candidate}
           /\ SequenceSet(localReadyQueue)
                \subseteq outstanding \cup {candidate}
           /\ SequenceSet(commandQueue) \cap
                (outstanding \cup {candidate}) = {}
      BY <1>1, AddFreshCompletionPreservesNodeWorkFacts
    <2>2. /\ localReadyQueue \in Seq(Range(localReadyQueue))
           /\ candidate \notin SequenceSet(localReadyQueue)
           /\ candidate \notin SequenceSet(ioReadyQueue)
      BY <1>1, SMT DEF AsyncCompletionSequenceTyped
    <2>3. /\ SequenceSet(Append(localReadyQueue, candidate)) =
                    SequenceSet(localReadyQueue) \cup {candidate}
           /\ Len(Append(localReadyQueue, candidate)) =
                    Len(localReadyQueue) + 1
           /\ AsyncCompletionSequenceTyped(
                    Append(localReadyQueue, candidate))
      BY <1>1, <2>2, SequenceSetAfterAppend, AppendSequenceFacts,
         TypedCompletionAppendPreservesSequenceType
    <2>4. IsFiniteSet(SequenceSet(localReadyQueue))
      BY <1>1, FS_Subset
    <2>5. Cardinality(
               SequenceSet(Append(localReadyQueue, candidate))) =
             Cardinality(SequenceSet(localReadyQueue)) + 1
      BY <2>2, <2>3, <2>4, FS_AddElement
    <2>6. /\ Len(Append(localReadyQueue, candidate)) =
                    Cardinality(
                      SequenceSet(Append(localReadyQueue, candidate)))
           /\ SequenceSet(Append(localReadyQueue, candidate))
                \subseteq outstanding \cup {candidate}
           /\ SequenceSet(ioReadyQueue) \cap
                SequenceSet(Append(localReadyQueue, candidate)) = {}
      BY <1>1, <2>2, <2>3, <2>5, SMT
    <2> QED BY <1>1, <2>1, <2>3, <2>6
  <1> QED BY <1>1

THEOREM AppendFreshLocalReadyPreservesConsensusOwnership ==
  \A queue, outstanding, ioReadyQueue, localReadyQueue, candidate:
    /\ AsyncIoSequenceTyped(queue)
    /\ (\A job \in SequenceSet(queue):
          job.class = "Consensus" => job.candidate \in outstanding)
    /\ AsyncIoConsensusQueueOwnership(
         queue, ioReadyQueue, localReadyQueue)
    /\ localReadyQueue \in Seq(Range(localReadyQueue))
    /\ candidate \notin outstanding
    => AsyncIoConsensusQueueOwnership(
         queue, ioReadyQueue, Append(localReadyQueue, candidate))
PROOF
  <1>1. ASSUME NEW queue, NEW outstanding, NEW ioReadyQueue,
                NEW localReadyQueue, NEW candidate,
                AsyncIoSequenceTyped(queue),
                \A job \in SequenceSet(queue):
                  job.class = "Consensus" => job.candidate \in outstanding,
                AsyncIoConsensusQueueOwnership(
                  queue, ioReadyQueue, localReadyQueue),
                localReadyQueue \in Seq(Range(localReadyQueue)),
                candidate \notin outstanding
         PROVE AsyncIoConsensusQueueOwnership(
                 queue, ioReadyQueue,
                 Append(localReadyQueue, candidate))
    <2>1. SequenceSet(Append(localReadyQueue, candidate)) =
             SequenceSet(localReadyQueue) \cup {candidate}
      BY <1>1, SequenceSetAfterAppend
    <2>2. /\ (\A index \in AsyncIoConsensusIndices(queue):
                    /\ queue[index].candidate
                         \notin SequenceSet(ioReadyQueue)
                    /\ queue[index].candidate
                         \notin SequenceSet(localReadyQueue))
           /\ (\A left, right \in AsyncIoConsensusIndices(queue):
                 queue[left].candidate = queue[right].candidate =>
                   left = right)
      BY <1>1 DEF AsyncIoConsensusQueueOwnership
    <2>3. \A index \in AsyncIoConsensusIndices(queue):
             queue[index] \in SequenceSet(queue)
               /\ queue[index].class = "Consensus"
      BY <1>1, SMT
         DEF AsyncIoConsensusIndices, AsyncIoSequenceTyped, SequenceSet
    <2>4. \A index \in AsyncIoConsensusIndices(queue):
             /\ queue[index].candidate \notin SequenceSet(ioReadyQueue)
             /\ queue[index].candidate
                  \notin SequenceSet(Append(localReadyQueue, candidate))
      BY <1>1, <2>1, <2>2, <2>3, SMT
    <2> QED BY <2>2, <2>4 DEF AsyncIoConsensusQueueOwnership
  <1> QED BY <1>1

THEOREM AdmitFreshLocalCompletionPreservesIoType ==
  \A node \in ValidatorIds:
    \A candidate:
      /\ AsyncTypeInvariant
      /\ AsyncCandidateTyped(candidate)
      /\ candidate.class = "Completion"
      /\ candidate.node = node
      /\ ~CandidateInFlight(candidate)
      /\ AsyncCompletionLoad(node) < AsyncIoWorkCapacity
      /\ asyncLocalReadyCompletions' =
           [asyncLocalReadyCompletions EXCEPT
              ![node] = Append(@, candidate)]
      /\ asyncOutstandingWork' =
           [asyncOutstandingWork EXCEPT ![node] = @ \cup {candidate}]
      /\ UNCHANGED <<asyncCommandQueues, asyncIoQueues,
                      asyncIoReadyCompletions, asyncNextCompletionSource,
                      asyncIoControlAvailable,
                      asyncDeferredCompletionQueues>>
      => AsyncIoTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW candidate,
                AsyncTypeInvariant,
                AsyncCandidateTyped(candidate),
                candidate.class = "Completion",
                candidate.node = node,
                ~CandidateInFlight(candidate),
                AsyncCompletionLoad(node) < AsyncIoWorkCapacity,
                asyncLocalReadyCompletions' =
                  [asyncLocalReadyCompletions EXCEPT
                     ![node] = Append(@, candidate)],
                asyncOutstandingWork' =
                  [asyncOutstandingWork EXCEPT
                     ![node] = @ \cup {candidate}],
                UNCHANGED <<asyncCommandQueues, asyncIoQueues,
                            asyncIoReadyCompletions,
                            asyncNextCompletionSource,
                            asyncIoControlAvailable,
                            asyncDeferredCompletionQueues>>
         PROVE AsyncIoTypeInvariant'
    <2>1. /\ AsyncConfiguration
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>2. /\ candidate \notin asyncOutstandingWork[node]
           /\ candidate \notin SequenceSet(asyncCommandQueues[node])
      BY <1>1, CausalUntrackedCandidateFacts, SMT
         DEF QueuedCandidates
    <2>3. AsyncIoTopologyTypeInvariant'
      BY <1>1, <2>1, Isa DEF AsyncIoTopologyTypeInvariant
    <2>4. AsyncIoQueueContentTypeInvariant'
      <3>1. /\ asyncIoQueues' = asyncIoQueues
             /\ asyncIoReadyCompletions' = asyncIoReadyCompletions
        BY <1>1
      <3>2. /\ asyncOutstandingWork'[node] =
                    asyncOutstandingWork[node] \cup {candidate}
             /\ asyncLocalReadyCompletions'[node] =
                    Append(asyncLocalReadyCompletions[node], candidate)
        BY <1>1, <2>1, Isa DEF AsyncIoTopologyTypeInvariant
      <3>3. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
             /\ (\A job \in SequenceSet(asyncIoQueues[node]):
                   job.class = "Consensus" =>
                     job.candidate \in asyncOutstandingWork[node])
             /\ AsyncIoConsensusQueueOwnership(
                  asyncIoQueues[node], asyncIoReadyCompletions[node],
                  asyncLocalReadyCompletions[node])
             /\ asyncLocalReadyCompletions[node]
                  \in Seq(Range(asyncLocalReadyCompletions[node]))
        BY <2>1
           DEF AsyncIoQueueContentTypeInvariant,
               AsyncIoWorkContentTypeInvariant,
               AsyncIoConsensusCandidateOwnership,
               AsyncCompletionSequenceTyped
      <3>4. AsyncIoConsensusQueueOwnership(
               asyncIoQueues[node], asyncIoReadyCompletions[node],
               Append(asyncLocalReadyCompletions[node], candidate))
        BY <1>1, <2>2, <3>3,
           AppendFreshLocalReadyPreservesConsensusOwnership
      <3>5. /\ AsyncIoSequenceTyped(asyncIoQueues'[node])
             /\ (\A job \in SequenceSet(asyncIoQueues'[node]):
                   job.class = "Consensus" =>
                     job.candidate \in asyncOutstandingWork'[node])
             /\ AsyncIoConsensusCandidateOwnership(
                  node, asyncIoQueues', asyncIoReadyCompletions',
                  asyncLocalReadyCompletions')
        BY <3>1, <3>2, <3>3, <3>4, Isa
           DEF AsyncIoConsensusCandidateOwnership
      <3>6. \A other \in ValidatorIds:
               /\ AsyncIoSequenceTyped(asyncIoQueues'[other])
               /\ (\A job \in SequenceSet(asyncIoQueues'[other]):
                     job.class = "Consensus" =>
                       job.candidate \in asyncOutstandingWork'[other])
               /\ AsyncIoConsensusCandidateOwnership(
                    other, asyncIoQueues', asyncIoReadyCompletions',
                    asyncLocalReadyCompletions')
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ AsyncIoSequenceTyped(asyncIoQueues'[other])
                     /\ (\A job \in SequenceSet(asyncIoQueues'[other]):
                           job.class = "Consensus" =>
                             job.candidate \in asyncOutstandingWork'[other])
                     /\ AsyncIoConsensusCandidateOwnership(
                          other, asyncIoQueues',
                          asyncIoReadyCompletions',
                          asyncLocalReadyCompletions')
          <5>1. CASE other = node
            BY <3>5, <5>1
          <5>2. CASE other # node
            <6>1. /\ asyncOutstandingWork'[other] =
                           asyncOutstandingWork[other]
                   /\ asyncLocalReadyCompletions'[other] =
                           asyncLocalReadyCompletions[other]
              BY <1>1, <2>1, <4>1, <5>2,
                 FunctionalUpdateAwayFromKey
                 DEF AsyncIoTopologyTypeInvariant
            <6> QED BY <2>1, <3>1, <4>1, <6>1, Isa
                 DEF AsyncIoQueueContentTypeInvariant,
                     AsyncIoConsensusCandidateOwnership
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>6 DEF AsyncIoQueueContentTypeInvariant
    <2>5. AsyncIoWorkContentTypeInvariant'
      <3>1. /\ asyncOutstandingWork'[node] =
                    asyncOutstandingWork[node] \cup {candidate}
             /\ asyncLocalReadyCompletions'[node] =
                    Append(asyncLocalReadyCompletions[node], candidate)
             /\ asyncIoReadyCompletions' = asyncIoReadyCompletions
             /\ asyncCommandQueues' = asyncCommandQueues
        BY <1>1, <2>1, Isa DEF AsyncIoTopologyTypeInvariant
      <3>2. /\ IsFiniteSet(asyncOutstandingWork[node])
             /\ (\A work \in asyncOutstandingWork[node]:
                   /\ AsyncCandidateTyped(work)
                   /\ work.class = "Completion"
                   /\ work.node = node)
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions[node])
             /\ Len(asyncIoReadyCompletions[node]) =
                  Cardinality(
                    SequenceSet(asyncIoReadyCompletions[node]))
             /\ Len(asyncLocalReadyCompletions[node]) =
                  Cardinality(
                    SequenceSet(asyncLocalReadyCompletions[node]))
             /\ SequenceSet(asyncIoReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncLocalReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions[node]) = {}
             /\ SequenceSet(asyncCommandQueues[node]) \cap
                  asyncOutstandingWork[node] = {}
        BY <2>1 DEF AsyncIoWorkContentTypeInvariant
      <3>3. /\ IsFiniteSet(asyncOutstandingWork'[node])
             /\ (\A work \in asyncOutstandingWork'[node]:
                   /\ AsyncCandidateTyped(work)
                   /\ work.class = "Completion"
                   /\ work.node = node)
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions'[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions'[node])
             /\ Len(asyncIoReadyCompletions'[node]) =
                  Cardinality(
                    SequenceSet(asyncIoReadyCompletions'[node]))
             /\ Len(asyncLocalReadyCompletions'[node]) =
                  Cardinality(
                    SequenceSet(asyncLocalReadyCompletions'[node]))
             /\ SequenceSet(asyncIoReadyCompletions'[node])
                  \subseteq asyncOutstandingWork'[node]
             /\ SequenceSet(asyncLocalReadyCompletions'[node])
                  \subseteq asyncOutstandingWork'[node]
             /\ SequenceSet(asyncIoReadyCompletions'[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions'[node]) = {}
             /\ SequenceSet(asyncCommandQueues'[node]) \cap
                  asyncOutstandingWork'[node] = {}
        BY <1>1, <2>2, <3>1, <3>2,
           AppendFreshLocalCompletionPreservesNodeWorkFacts
      <3>4. \A other \in ValidatorIds:
               /\ IsFiniteSet(asyncOutstandingWork'[other])
               /\ (\A work \in asyncOutstandingWork'[other]:
                     /\ AsyncCandidateTyped(work)
                     /\ work.class = "Completion"
                     /\ work.node = other)
               /\ AsyncCompletionSequenceTyped(
                    asyncIoReadyCompletions'[other])
               /\ AsyncCompletionSequenceTyped(
                    asyncLocalReadyCompletions'[other])
               /\ Len(asyncIoReadyCompletions'[other]) =
                    Cardinality(
                      SequenceSet(asyncIoReadyCompletions'[other]))
               /\ Len(asyncLocalReadyCompletions'[other]) =
                    Cardinality(
                      SequenceSet(asyncLocalReadyCompletions'[other]))
               /\ SequenceSet(asyncIoReadyCompletions'[other])
                    \subseteq asyncOutstandingWork'[other]
               /\ SequenceSet(asyncLocalReadyCompletions'[other])
                    \subseteq asyncOutstandingWork'[other]
               /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                    SequenceSet(asyncLocalReadyCompletions'[other]) = {}
               /\ SequenceSet(asyncCommandQueues'[other]) \cap
                    asyncOutstandingWork'[other] = {}
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ IsFiniteSet(asyncOutstandingWork'[other])
                     /\ (\A work \in asyncOutstandingWork'[other]:
                           /\ AsyncCandidateTyped(work)
                           /\ work.class = "Completion"
                           /\ work.node = other)
                     /\ AsyncCompletionSequenceTyped(
                          asyncIoReadyCompletions'[other])
                     /\ AsyncCompletionSequenceTyped(
                          asyncLocalReadyCompletions'[other])
                     /\ Len(asyncIoReadyCompletions'[other]) =
                          Cardinality(
                            SequenceSet(asyncIoReadyCompletions'[other]))
                     /\ Len(asyncLocalReadyCompletions'[other]) =
                          Cardinality(
                            SequenceSet(asyncLocalReadyCompletions'[other]))
                     /\ SequenceSet(asyncIoReadyCompletions'[other])
                          \subseteq asyncOutstandingWork'[other]
                     /\ SequenceSet(asyncLocalReadyCompletions'[other])
                          \subseteq asyncOutstandingWork'[other]
                     /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                          SequenceSet(asyncLocalReadyCompletions'[other]) = {}
                     /\ SequenceSet(asyncCommandQueues'[other]) \cap
                          asyncOutstandingWork'[other] = {}
          <5>1. CASE other = node
            BY <3>3, <5>1
          <5>2. CASE other # node
            <6>1. /\ asyncOutstandingWork'[other] =
                           asyncOutstandingWork[other]
                   /\ asyncLocalReadyCompletions'[other] =
                           asyncLocalReadyCompletions[other]
              BY <1>1, <2>1, <4>1, <5>2,
                 FunctionalUpdateAwayFromKey
                 DEF AsyncIoTopologyTypeInvariant
            <6>2. /\ IsFiniteSet(asyncOutstandingWork[other])
                   /\ (\A work \in asyncOutstandingWork[other]:
                         /\ AsyncCandidateTyped(work)
                         /\ work.class = "Completion"
                         /\ work.node = other)
                   /\ AsyncCompletionSequenceTyped(
                        asyncIoReadyCompletions[other])
                   /\ AsyncCompletionSequenceTyped(
                        asyncLocalReadyCompletions[other])
                   /\ Len(asyncIoReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncIoReadyCompletions[other]))
                   /\ Len(asyncLocalReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncLocalReadyCompletions[other]))
                   /\ SequenceSet(asyncIoReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncLocalReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncIoReadyCompletions[other]) \cap
                        SequenceSet(asyncLocalReadyCompletions[other]) = {}
                   /\ SequenceSet(asyncCommandQueues[other]) \cap
                        asyncOutstandingWork[other] = {}
              BY <2>1, <4>1 DEF AsyncIoWorkContentTypeInvariant
            <6>3. /\ asyncIoReadyCompletions'[other] =
                           asyncIoReadyCompletions[other]
                   /\ asyncCommandQueues'[other] =
                           asyncCommandQueues[other]
              BY <3>1
            <6> QED BY <6>1, <6>2, <6>3
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>4 DEF AsyncIoWorkContentTypeInvariant
    <2>6. AsyncIoCapacityTypeInvariant'
      <3>1. /\ IsFiniteSet(asyncOutstandingWork[node])
             /\ candidate \notin asyncOutstandingWork[node]
             /\ AsyncOutstandingWorkCount(node) \in Nat
        BY <2>1, <2>2, FS_CardinalityType
           DEF AsyncIoWorkContentTypeInvariant,
               AsyncOutstandingWorkCount
      <3>2. Cardinality(
                   asyncOutstandingWork[node] \cup {candidate}) =
                 Cardinality(asyncOutstandingWork[node]) + 1
        BY <3>1, FS_AddElement
      <3>3. /\ asyncCommandQueues' = asyncCommandQueues
             /\ asyncIoQueues' = asyncIoQueues
             /\ asyncOutstandingWork'[node] =
                    asyncOutstandingWork[node] \cup {candidate}
             /\ asyncDeferredCompletionQueues' =
                    asyncDeferredCompletionQueues
        BY <1>1, <2>1, Isa
           DEF AsyncIoTopologyTypeInvariant
      <3>4. /\ AsyncQueueDepth(node)' = AsyncQueueDepth(node)
             /\ AsyncIoQueueDepth(node)' = AsyncIoQueueDepth(node)
             /\ AsyncOutstandingWorkCount(node)' =
                    AsyncOutstandingWorkCount(node) + 1
             /\ QueuedCompletionCount(node)' =
                    QueuedCompletionCount(node)
             /\ DeferredCompletionCount(node)' =
                    DeferredCompletionCount(node)
        BY <3>2, <3>3, Isa
           DEF AsyncQueueDepth, AsyncIoQueueDepth,
               AsyncOutstandingWorkCount, QueuedCompletionCount,
               QueuedCompletionIndices, DeferredCompletionCount
      <3>5. AsyncCompletionLoad(node)' =
               AsyncCompletionLoad(node) + 1
        BY <3>4, SMT DEF AsyncCompletionLoad
      <3>6. /\ AsyncQueueDepth(node) <= AsyncQueueCapacity
             /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
             /\ AsyncCompletionLoad(node) <= AsyncCompletionReserve
             /\ AsyncCompletionLoad(node) <= AsyncIoWorkCapacity
        BY <2>1 DEF AsyncIoCapacityTypeInvariant
      <3>7. /\ AsyncIoWorkCapacity \in Nat
             /\ AsyncCompletionReserve \in Nat
             /\ AsyncIoWorkCapacity <= AsyncCompletionReserve
        BY <2>1 DEF AsyncConfiguration
      <3>8. AsyncCompletionLoad(node) \in Nat
        <4>1. /\ AsyncQueueTyped(asyncCommandQueues[node])
               /\ asyncDeferredCompletionQueues[node]
                    \in Seq(Range(asyncDeferredCompletionQueues[node]))
          BY <1>1
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncRuntimeTypeInvariant,
                 AsyncRuntimeScalarTypeInvariant,
                 AsyncDeferredTypeInvariant,
                 AsyncDeferredContentTypeInvariant,
                 AsyncCompletionSequenceTyped
        <4>2. /\ Len(asyncCommandQueues[node]) \in Nat
               /\ IsFiniteSet(1..Len(asyncCommandQueues[node]))
          BY <4>1, LenProperties, FS_Interval, SMT
             DEF AsyncQueueTyped
        <4>3. /\ QueuedCompletionIndices(node)
                         \subseteq 1..Len(asyncCommandQueues[node])
               /\ IsFiniteSet(QueuedCompletionIndices(node))
          BY <4>2, FS_Subset DEF QueuedCompletionIndices
        <4> QED BY <3>1, <4>1, <4>3,
             FS_CardinalityType, LenProperties, SMT
             DEF AsyncOutstandingWorkCount, QueuedCompletionCount,
                 DeferredCompletionCount, AsyncCompletionLoad
      <3>9. /\ AsyncCompletionLoad(node)' <= AsyncIoWorkCapacity
             /\ AsyncCompletionLoad(node)' <= AsyncCompletionReserve
        <4>1. AsyncCompletionLoad(node) + 1 <= AsyncIoWorkCapacity
          BY <1>1, <3>7, <3>8, NaturalIncrementWithinBound
        <4> QED BY <3>5, <3>7, <4>1, SMT
      <3>10. /\ AsyncQueueDepth(node)' <= AsyncQueueCapacity
              /\ AsyncIoQueueDepth(node)' <= AsyncIoCapacity
              /\ AsyncCompletionLoad(node)' <= AsyncCompletionReserve
              /\ AsyncCompletionLoad(node)' <= AsyncIoWorkCapacity
        BY <3>4, <3>6, <3>9
      <3>11. \A other \in ValidatorIds:
               /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
               /\ AsyncCompletionLoad(other)' <= AsyncCompletionReserve
               /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
               /\ AsyncCompletionLoad(other)' <= AsyncIoWorkCapacity
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
                     /\ AsyncCompletionLoad(other)' <=
                          AsyncCompletionReserve
                     /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
                     /\ AsyncCompletionLoad(other)' <=
                          AsyncIoWorkCapacity
          <5>1. CASE other = node
            BY <3>10, <5>1
          <5>2. CASE other # node
            <6>1. /\ asyncOutstandingWork'[other] =
                           asyncOutstandingWork[other]
                   /\ asyncLocalReadyCompletions'[other] =
                           asyncLocalReadyCompletions[other]
              BY <1>1, <2>1, <4>1, <5>2,
                 FunctionalUpdateAwayFromKey
                 DEF AsyncIoTopologyTypeInvariant
            <6>2. /\ AsyncQueueDepth(other)' =
                           AsyncQueueDepth(other)
                   /\ AsyncIoQueueDepth(other)' =
                           AsyncIoQueueDepth(other)
                   /\ AsyncCompletionLoad(other)' =
                           AsyncCompletionLoad(other)
              BY <1>1, <6>1, Isa
                 DEF AsyncQueueDepth, AsyncIoQueueDepth,
                     AsyncCompletionLoad, AsyncOutstandingWorkCount,
                     QueuedCompletionCount, QueuedCompletionIndices,
                     DeferredCompletionCount
            <6> QED BY <2>1, <4>1, <6>2
                 DEF AsyncIoCapacityTypeInvariant
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>11 DEF AsyncIoCapacityTypeInvariant
    <2> QED BY <2>3, <2>4, <2>5, <2>6
         DEF AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
  <1> QED BY <1>1

THEOREM CausalCompletionAdmissionIoFrame ==
  \A node \in ValidatorIds:
    /\ AdmitCausalHead(node)
    /\ ~CandidateInFlight(HeadCausalCandidate(node))
    /\ HeadCausalCandidate(node).class = "Completion"
    => LET candidate == HeadCausalCandidate(node)
       IN /\ asyncIoQueues' =
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
BY Isa
   DEF AdmitCausalHead, CandidateInFlight, HeadCausalCandidate

THEOREM CausalCompletionAdmissionPreservesIoType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AdmitCausalHead(node)
    /\ ~CandidateInFlight(HeadCausalCandidate(node))
    /\ HeadCausalCandidate(node).class = "Completion"
    /\ UNCHANGED asyncDeferredCompletionQueues
    => AsyncIoTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                AdmitCausalHead(node),
                ~CandidateInFlight(HeadCausalCandidate(node)),
                HeadCausalCandidate(node).class = "Completion",
                UNCHANGED asyncDeferredCompletionQueues
         PROVE AsyncIoTypeInvariant'
    <2>1. AsyncIoTopologyTypeInvariant'
      BY <1>1, FunctionalUpdatePreservesType, SMTT(30)
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant,
             AdmitCausalHead, HeadCausalCandidate, CandidateInFlight
    <2>2. AsyncIoQueueContentTypeInvariant'
      <3> DEFINE Candidate == HeadCausalCandidate(node)
      <3> DEFINE NewJob == AsyncIoConsensusJob(Candidate)
      <3>1. /\ AsyncCausalTypeInvariant
             /\ AsyncIoQueueContentTypeInvariant
             /\ AsyncIoWorkContentTypeInvariant
        BY <1>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
               AsyncIoContentTypeInvariant
      <3>2. /\ AsyncCandidateTyped(Candidate)
             /\ Candidate \notin asyncOutstandingWork[node]
             /\ Candidate \notin QueuedCandidates
             /\ Candidate \notin DeferredCandidates
        BY <1>1, <3>1, CausalHeadCandidateIsTyped,
           CausalUntrackedCandidateFacts
           DEF AdmitCausalHead, CausalHeadCanAdvance, Candidate
      <3>3. /\ AsyncIoJobTyped(NewJob)
             /\ NewJob.class = "Consensus"
             /\ NewJob.candidate = Candidate
        BY <1>1, <3>1, <3>2,
           TypedCompletionCandidateMakesConsensusJob, SMT
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
               NewJob, AsyncIoConsensusJob, AsyncIoJob, Candidate
      <3>4. /\ asyncIoQueues' =
                    [asyncIoQueues EXCEPT ![node] = Append(@, NewJob)]
             /\ asyncOutstandingWork' =
                    [asyncOutstandingWork EXCEPT
                       ![node] = @ \cup {Candidate}]
             /\ UNCHANGED <<asyncCommandQueues,
                            asyncIoReadyCompletions,
                            asyncLocalReadyCompletions,
                            asyncNextCompletionSource,
                            asyncIoControlAvailable>>
        BY <1>1, CausalCompletionAdmissionIoFrame
           DEF Candidate, NewJob
      <3>5. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
             /\ \A job \in SequenceSet(asyncIoQueues[node]):
                  job.class = "Consensus" =>
                    job.candidate \in asyncOutstandingWork[node]
             /\ AsyncIoConsensusCandidateOwnership(
                  node, asyncIoQueues, asyncIoReadyCompletions,
                  asyncLocalReadyCompletions)
             /\ SequenceSet(asyncIoReadyCompletions[node])
                    \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncLocalReadyCompletions[node])
                    \subseteq asyncOutstandingWork[node]
        BY <3>1
           DEF AsyncIoQueueContentTypeInvariant,
               AsyncIoWorkContentTypeInvariant
      <3>6. /\ AsyncIoSequenceTyped(
                    Append(asyncIoQueues[node], NewJob))
             /\ SequenceSet(Append(asyncIoQueues[node], NewJob)) =
                    SequenceSet(asyncIoQueues[node]) \cup {NewJob}
             /\ AsyncIoConsensusIndices(
                    Append(asyncIoQueues[node], NewJob)) =
                    AsyncIoConsensusIndices(asyncIoQueues[node])
                      \cup {Len(asyncIoQueues[node]) + 1}
        BY <3>3, <3>5, TypedIoAppendPreservesSequenceType,
           SequenceSetAfterAppend, ConsensusIndicesAfterConsensusAppend
           DEF AsyncIoSequenceTyped
      <3>7. /\ Candidate \notin
                    SequenceSet(asyncIoReadyCompletions[node])
             /\ Candidate \notin
                    SequenceSet(asyncLocalReadyCompletions[node])
             /\ \A index \in AsyncIoConsensusIndices(
                              asyncIoQueues[node]):
                  asyncIoQueues[node][index].candidate # Candidate
        <4>1. /\ Candidate \notin
                        SequenceSet(asyncIoReadyCompletions[node])
               /\ Candidate \notin
                        SequenceSet(asyncLocalReadyCompletions[node])
          <5>1. SequenceSet(asyncIoReadyCompletions[node])
                    \subseteq asyncOutstandingWork[node]
            BY <3>1 DEF AsyncIoWorkContentTypeInvariant
          <5>2. SequenceSet(asyncLocalReadyCompletions[node])
                    \subseteq asyncOutstandingWork[node]
            BY <3>1 DEF AsyncIoWorkContentTypeInvariant
          <5>3. Candidate \notin asyncOutstandingWork[node]
            BY <3>2
          <5> QED BY <5>1, <5>2, <5>3, Isa
        <4>2. \A index \in AsyncIoConsensusIndices(
                                 asyncIoQueues[node]):
                    asyncIoQueues[node][index].candidate # Candidate
          <5>1. ASSUME NEW index \in AsyncIoConsensusIndices(
                                      asyncIoQueues[node])
                 PROVE asyncIoQueues[node][index].candidate # Candidate
            <6>1. asyncIoQueues[node][index].class = "Consensus"
              BY <5>1 DEF AsyncIoConsensusIndices
            <6>2. asyncIoQueues[node][index]
                        \in SequenceSet(asyncIoQueues[node])
              BY <5>1 DEF AsyncIoConsensusIndices, SequenceSet
            <6>3. asyncIoQueues[node][index].candidate
                        \in asyncOutstandingWork[node]
              BY <3>5, <6>1, <6>2
            <6> QED BY <3>2, <6>3
          <5> QED BY <5>1
        <4> QED BY <4>1, <4>2
      <3>8. /\ AsyncIoSequenceTyped(asyncIoQueues'[node])
             /\ (\A job \in SequenceSet(asyncIoQueues'[node]):
                   job.class = "Consensus" =>
                     job.candidate \in asyncOutstandingWork'[node])
             /\ AsyncIoConsensusCandidateOwnership(
                  node, asyncIoQueues', asyncIoReadyCompletions',
                  asyncLocalReadyCompletions')
        <4>1. /\ asyncIoQueues'[node] =
                      Append(asyncIoQueues[node], NewJob)
               /\ asyncOutstandingWork'[node] =
                      asyncOutstandingWork[node] \cup {Candidate}
               /\ asyncIoReadyCompletions'[node] =
                      asyncIoReadyCompletions[node]
               /\ asyncLocalReadyCompletions'[node] =
                      asyncLocalReadyCompletions[node]
          BY <1>1, <3>4, FunctionalAppendUpdateAtKey, Isa
             DEF AsyncIoTopologyTypeInvariant, AsyncTypeInvariant,
                 AsyncSchedulerTypeInvariant, AsyncIoTypeInvariant
        <4>2. AsyncConfiguration
          BY <1>1
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant
        <4>3. /\ (\A job \in SequenceSet(asyncIoQueues[node]):
                        job.class = "Consensus" =>
                          job.candidate \in asyncOutstandingWork[node])
               /\ AsyncIoConsensusQueueOwnership(
                    asyncIoQueues[node],
                    asyncIoReadyCompletions[node],
                    asyncLocalReadyCompletions[node])
          BY <3>1 DEF AsyncIoQueueContentTypeInvariant,
                       AsyncIoConsensusCandidateOwnership
        <4>4. /\ Candidate \notin
                        SequenceSet(asyncIoReadyCompletions[node])
               /\ Candidate \notin
                        SequenceSet(asyncLocalReadyCompletions[node])
          BY <3>7
        <4>5. /\ AsyncIoSequenceTyped(
                       Append(asyncIoQueues[node], NewJob))
               /\ (\A job \in
                          SequenceSet(Append(asyncIoQueues[node], NewJob)):
                     job.class = "Consensus" =>
                       job.candidate \in
                         asyncOutstandingWork[node] \cup {Candidate})
               /\ AsyncIoConsensusQueueOwnership(
                    Append(asyncIoQueues[node], NewJob),
                    asyncIoReadyCompletions[node],
                    asyncLocalReadyCompletions[node])
          BY <1>1, <3>2, <3>3, <3>5, <4>2, <4>3, <4>4,
             AppendFreshConsensusJobPreservesQueueFacts
             DEF Candidate, NewJob
        <4> QED BY <4>1, <4>5, Isa
             DEF AsyncIoConsensusCandidateOwnership
      <3>9. \A other \in ValidatorIds:
               /\ AsyncIoSequenceTyped(asyncIoQueues'[other])
               /\ (\A job \in SequenceSet(asyncIoQueues'[other]):
                     job.class = "Consensus" =>
                       job.candidate \in asyncOutstandingWork'[other])
               /\ AsyncIoConsensusCandidateOwnership(
                    other, asyncIoQueues', asyncIoReadyCompletions',
                    asyncLocalReadyCompletions')
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ AsyncIoSequenceTyped(asyncIoQueues'[other])
                     /\ (\A job \in SequenceSet(asyncIoQueues'[other]):
                           job.class = "Consensus" =>
                             job.candidate \in
                               asyncOutstandingWork'[other])
                     /\ AsyncIoConsensusCandidateOwnership(
                          other, asyncIoQueues',
                          asyncIoReadyCompletions',
                          asyncLocalReadyCompletions')
          <5>1. CASE other = node
            BY <3>8, <5>1, Isa
          <5>2. CASE other # node
            <6>1. /\ asyncIoQueues'[other] =
                          asyncIoQueues[other]
                   /\ asyncOutstandingWork'[other] =
                          asyncOutstandingWork[other]
                   /\ asyncIoReadyCompletions' =
                          asyncIoReadyCompletions
                   /\ asyncLocalReadyCompletions' =
                          asyncLocalReadyCompletions
              BY <1>1, <3>4, <4>1, <5>2,
                 FunctionalAppendUpdateAwayFromKey,
                 FunctionalUpdateAwayFromKey, Isa
                 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                     AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant
            <6>2. /\ AsyncIoSequenceTyped(asyncIoQueues[other])
                   /\ (\A job \in SequenceSet(asyncIoQueues[other]):
                         job.class = "Consensus" =>
                           job.candidate \in asyncOutstandingWork[other])
                   /\ AsyncIoConsensusCandidateOwnership(
                        other, asyncIoQueues, asyncIoReadyCompletions,
                        asyncLocalReadyCompletions)
              BY <3>1, <4>1 DEF AsyncIoQueueContentTypeInvariant
            <6> QED BY <6>1, <6>2, Isa
                 DEF AsyncIoConsensusCandidateOwnership
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>9 DEF AsyncIoQueueContentTypeInvariant
    <2>3. AsyncIoWorkContentTypeInvariant'
      <3> DEFINE Candidate == HeadCausalCandidate(node)
      <3>1. AsyncIoWorkContentTypeInvariant
        BY <1>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
      <3>2. /\ AsyncCandidateTyped(Candidate)
             /\ Candidate.class = "Completion"
             /\ Candidate.node = node
             /\ Candidate \notin asyncOutstandingWork[node]
             /\ Candidate \notin QueuedCandidates
        BY <1>1, CausalHeadCandidateIsTyped,
           CausalHeadCandidateIsOwned, CausalUntrackedCandidateFacts
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AdmitCausalHead,
               CausalHeadCanAdvance, Candidate
      <3>3. Candidate \notin
               SequenceSet(asyncCommandQueues[node])
        BY <1>1, <3>2, SMT DEF QueuedCandidates
      <3>4. /\ asyncOutstandingWork' =
                    [asyncOutstandingWork EXCEPT
                       ![node] = @ \cup {Candidate}]
             /\ UNCHANGED <<asyncCommandQueues,
                            asyncIoReadyCompletions,
                            asyncLocalReadyCompletions>>
        BY <1>1, CausalCompletionAdmissionIoFrame DEF Candidate
      <3>5. /\ IsFiniteSet(asyncOutstandingWork[node])
             /\ (\A work \in asyncOutstandingWork[node]:
                   /\ AsyncCandidateTyped(work)
                   /\ work.class = "Completion"
                   /\ work.node = node)
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions[node])
             /\ Len(asyncIoReadyCompletions[node]) =
                  Cardinality(SequenceSet(asyncIoReadyCompletions[node]))
             /\ Len(asyncLocalReadyCompletions[node]) =
                  Cardinality(SequenceSet(asyncLocalReadyCompletions[node]))
             /\ SequenceSet(asyncIoReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncLocalReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions[node]) = {}
             /\ SequenceSet(asyncCommandQueues[node]) \cap
                  asyncOutstandingWork[node] = {}
        BY <3>1 DEF AsyncIoWorkContentTypeInvariant
      <3>6. /\ IsFiniteSet(
                       asyncOutstandingWork[node] \cup {Candidate})
             /\ (\A work \in
                        asyncOutstandingWork[node] \cup {Candidate}:
                   /\ AsyncCandidateTyped(work)
                   /\ work.class = "Completion"
                   /\ work.node = node)
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions[node])
             /\ Len(asyncIoReadyCompletions[node]) =
                  Cardinality(SequenceSet(asyncIoReadyCompletions[node]))
             /\ Len(asyncLocalReadyCompletions[node]) =
                  Cardinality(SequenceSet(asyncLocalReadyCompletions[node]))
             /\ SequenceSet(asyncIoReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node] \cup {Candidate}
             /\ SequenceSet(asyncLocalReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node] \cup {Candidate}
             /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions[node]) = {}
             /\ SequenceSet(asyncCommandQueues[node]) \cap
                  (asyncOutstandingWork[node] \cup {Candidate}) = {}
        BY <3>2, <3>3, <3>5,
           AddFreshCompletionPreservesNodeWorkFacts
      <3>7. /\ IsFiniteSet(asyncOutstandingWork'[node])
             /\ (\A work \in asyncOutstandingWork'[node]:
                   /\ AsyncCandidateTyped(work)
                   /\ work.class = "Completion"
                   /\ work.node = node)
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions'[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions'[node])
             /\ Len(asyncIoReadyCompletions'[node]) =
                  Cardinality(SequenceSet(asyncIoReadyCompletions'[node]))
             /\ Len(asyncLocalReadyCompletions'[node]) =
                  Cardinality(SequenceSet(asyncLocalReadyCompletions'[node]))
             /\ SequenceSet(asyncIoReadyCompletions'[node])
                  \subseteq asyncOutstandingWork'[node]
             /\ SequenceSet(asyncLocalReadyCompletions'[node])
                  \subseteq asyncOutstandingWork'[node]
             /\ SequenceSet(asyncIoReadyCompletions'[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions'[node]) = {}
             /\ SequenceSet(asyncCommandQueues'[node]) \cap
                  asyncOutstandingWork'[node] = {}
        BY <1>1, <3>4, <3>6, Isa
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant
      <3>8. \A other \in ValidatorIds:
               /\ IsFiniteSet(asyncOutstandingWork'[other])
               /\ (\A work \in asyncOutstandingWork'[other]:
                     /\ AsyncCandidateTyped(work)
                     /\ work.class = "Completion"
                     /\ work.node = other)
               /\ AsyncCompletionSequenceTyped(
                    asyncIoReadyCompletions'[other])
               /\ AsyncCompletionSequenceTyped(
                    asyncLocalReadyCompletions'[other])
               /\ Len(asyncIoReadyCompletions'[other]) =
                    Cardinality(
                      SequenceSet(asyncIoReadyCompletions'[other]))
               /\ Len(asyncLocalReadyCompletions'[other]) =
                    Cardinality(
                      SequenceSet(asyncLocalReadyCompletions'[other]))
               /\ SequenceSet(asyncIoReadyCompletions'[other])
                    \subseteq asyncOutstandingWork'[other]
               /\ SequenceSet(asyncLocalReadyCompletions'[other])
                    \subseteq asyncOutstandingWork'[other]
               /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                    SequenceSet(asyncLocalReadyCompletions'[other]) = {}
               /\ SequenceSet(asyncCommandQueues'[other]) \cap
                    asyncOutstandingWork'[other] = {}
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ IsFiniteSet(asyncOutstandingWork'[other])
                     /\ (\A work \in asyncOutstandingWork'[other]:
                           /\ AsyncCandidateTyped(work)
                           /\ work.class = "Completion"
                           /\ work.node = other)
                     /\ AsyncCompletionSequenceTyped(
                          asyncIoReadyCompletions'[other])
                     /\ AsyncCompletionSequenceTyped(
                          asyncLocalReadyCompletions'[other])
                     /\ Len(asyncIoReadyCompletions'[other]) =
                          Cardinality(
                            SequenceSet(asyncIoReadyCompletions'[other]))
                     /\ Len(asyncLocalReadyCompletions'[other]) =
                          Cardinality(
                            SequenceSet(asyncLocalReadyCompletions'[other]))
                     /\ SequenceSet(asyncIoReadyCompletions'[other])
                          \subseteq asyncOutstandingWork'[other]
                     /\ SequenceSet(asyncLocalReadyCompletions'[other])
                          \subseteq asyncOutstandingWork'[other]
                     /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                          SequenceSet(asyncLocalReadyCompletions'[other]) = {}
                     /\ SequenceSet(asyncCommandQueues'[other]) \cap
                          asyncOutstandingWork'[other] = {}
          <5>1. CASE other = node
            BY <3>7, <5>1
          <5>2. CASE other # node
            <6>1. /\ asyncOutstandingWork'[other] =
                          asyncOutstandingWork[other]
                   /\ asyncCommandQueues' = asyncCommandQueues
                   /\ asyncIoReadyCompletions' =
                          asyncIoReadyCompletions
                   /\ asyncLocalReadyCompletions' =
                          asyncLocalReadyCompletions
              BY <1>1, <3>4, <4>1, <5>2,
                 FunctionalUpdateAwayFromKey, Isa
                 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                     AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant
            <6>2. /\ IsFiniteSet(asyncOutstandingWork[other])
                   /\ (\A work \in asyncOutstandingWork[other]:
                         /\ AsyncCandidateTyped(work)
                         /\ work.class = "Completion"
                         /\ work.node = other)
                   /\ AsyncCompletionSequenceTyped(
                        asyncIoReadyCompletions[other])
                   /\ AsyncCompletionSequenceTyped(
                        asyncLocalReadyCompletions[other])
                   /\ Len(asyncIoReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncIoReadyCompletions[other]))
                   /\ Len(asyncLocalReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncLocalReadyCompletions[other]))
                   /\ SequenceSet(asyncIoReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncLocalReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncIoReadyCompletions[other]) \cap
                        SequenceSet(asyncLocalReadyCompletions[other]) = {}
                   /\ SequenceSet(asyncCommandQueues[other]) \cap
                        asyncOutstandingWork[other] = {}
              BY <3>1, <4>1 DEF AsyncIoWorkContentTypeInvariant
            <6> QED BY <6>1, <6>2, Isa
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>8 DEF AsyncIoWorkContentTypeInvariant
    <2>4. AsyncIoCapacityTypeInvariant'
      <3> DEFINE Candidate == HeadCausalCandidate(node)
      <3> DEFINE NewJob == AsyncIoConsensusJob(Candidate)
      <3>1. /\ AsyncConfiguration
             /\ AsyncIoTopologyTypeInvariant
             /\ AsyncIoQueueContentTypeInvariant
             /\ AsyncIoWorkContentTypeInvariant
             /\ AsyncIoCapacityTypeInvariant
        BY <1>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
               AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
      <3>2. /\ Candidate \notin asyncOutstandingWork[node]
             /\ IsFiniteSet(asyncOutstandingWork[node])
             /\ AsyncIoSequenceTyped(asyncIoQueues[node])
             /\ AsyncQueueTyped(asyncCommandQueues[node])
             /\ DOMAIN asyncIoQueues = ValidatorIds
             /\ DOMAIN asyncOutstandingWork = ValidatorIds
             /\ DOMAIN asyncCommandQueues = ValidatorIds
        BY <1>1, <3>1, CausalUntrackedCandidateFacts
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
               AsyncIoQueueContentTypeInvariant,
               AsyncIoWorkContentTypeInvariant,
               AsyncIoTopologyTypeInvariant, AdmitCausalHead,
               CausalHeadCanAdvance, Candidate
      <3>3. /\ IsFiniteSet(
                       asyncOutstandingWork[node] \cup {Candidate})
             /\ Cardinality(
                  asyncOutstandingWork[node] \cup {Candidate}) =
                    Cardinality(asyncOutstandingWork[node]) + 1
        BY <3>2, FS_AddElement
      <3>4. /\ asyncCommandQueues' = asyncCommandQueues
             /\ asyncIoQueues' =
                    [asyncIoQueues EXCEPT
                       ![node] = Append(@, NewJob)]
             /\ asyncOutstandingWork' =
                    [asyncOutstandingWork EXCEPT
                       ![node] = @ \cup {Candidate}]
             /\ asyncDeferredCompletionQueues' =
                    asyncDeferredCompletionQueues
        BY <1>1, CausalCompletionAdmissionIoFrame, Isa
           DEF Candidate, NewJob
      <3>5. /\ asyncIoQueues'[node] =
                    Append(asyncIoQueues[node], NewJob)
             /\ asyncOutstandingWork'[node] =
                    asyncOutstandingWork[node] \cup {Candidate}
             /\ asyncDeferredCompletionQueues'[node] =
                    asyncDeferredCompletionQueues[node]
        BY <1>1, <3>2, <3>4, FunctionalAppendUpdateAtKey, Isa
      <3>6. Len(Append(asyncIoQueues[node], NewJob)) =
               Len(asyncIoQueues[node]) + 1
        BY <3>2, AppendSequenceFacts DEF AsyncIoSequenceTyped
      <3>7. /\ AsyncQueueDepth(node)' = AsyncQueueDepth(node)
             /\ AsyncIoQueueDepth(node)' = AsyncIoQueueDepth(node) + 1
             /\ AsyncOutstandingWorkCount(node)' =
                    AsyncOutstandingWorkCount(node) + 1
             /\ QueuedCompletionCount(node)' =
                    QueuedCompletionCount(node)
             /\ DeferredCompletionCount(node)' =
                    DeferredCompletionCount(node)
        BY <3>3, <3>4, <3>5, <3>6, Isa
           DEF AsyncQueueDepth, AsyncIoQueueDepth,
               AsyncOutstandingWorkCount, QueuedCompletionCount,
               QueuedCompletionIndices, DeferredCompletionCount
      <3>8. /\ AsyncOutstandingWorkCount(node) \in Nat
             /\ AsyncOutstandingWorkCount(node)' \in Nat
             /\ QueuedCompletionCount(node) \in Nat
             /\ QueuedCompletionCount(node)' \in Nat
             /\ DeferredCompletionCount(node) \in Nat
             /\ DeferredCompletionCount(node)' \in Nat
             /\ AsyncQueueDepth(node) \in Nat
             /\ AsyncIoQueueDepth(node) \in Nat
             /\ AsyncCompletionLoad(node) \in Nat
             /\ AsyncCompletionLoad(node)' \in Nat
        <4>1. /\ Cardinality(asyncOutstandingWork[node]) \in Nat
               /\ Cardinality(
                    asyncOutstandingWork[node] \cup {Candidate}) \in Nat
          BY <3>2, <3>3, FS_CardinalityType
        <4>2. /\ Len(asyncCommandQueues[node]) \in Nat
               /\ IsFiniteSet(1..Len(asyncCommandQueues[node]))
               /\ Len(asyncIoQueues[node]) \in Nat
          BY <3>2, LenProperties, FS_Interval, SMT
             DEF AsyncQueueTyped, AsyncIoSequenceTyped
        <4>3. /\ QueuedCompletionIndices(node)
                         \subseteq 1..Len(asyncCommandQueues[node])
               /\ IsFiniteSet(QueuedCompletionIndices(node))
          BY <4>2, FS_Subset DEF QueuedCompletionIndices
        <4>4. asyncDeferredCompletionQueues[node]
                    \in Seq(Range(asyncDeferredCompletionQueues[node]))
          BY <1>1
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncDeferredTypeInvariant,
                 AsyncDeferredContentTypeInvariant,
                 AsyncCompletionSequenceTyped
        <4> QED BY <3>4, <3>5, <3>7, <4>1, <4>2, <4>3, <4>4,
             FS_CardinalityType, LenProperties, SMT
             DEF AsyncOutstandingWorkCount, QueuedCompletionCount,
                 DeferredCompletionCount, AsyncQueueDepth,
                 AsyncIoQueueDepth, AsyncCompletionLoad
      <3>9. AsyncCompletionLoad(node)' =
               AsyncCompletionLoad(node) + 1
        BY <3>7, <3>8, SMT DEF AsyncCompletionLoad
      <3>10. /\ AsyncCompletionLoad(node) < AsyncIoWorkCapacity
              /\ AsyncIoQueueDepth(node) <
                   AsyncIoAuxCapacity + AsyncIoWorkCapacity
        BY <1>1 DEF AdmitCausalHead, CausalHeadCanAdvance,
                       CanEnqueueIoClass, AsyncIoAdmissionLimit
      <3>11. /\ AsyncQueueDepth(node) <= AsyncQueueCapacity
              /\ AsyncCompletionLoad(node) <= AsyncCompletionReserve
              /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
              /\ AsyncCompletionLoad(node) <= AsyncIoWorkCapacity
        BY <3>1 DEF AsyncIoCapacityTypeInvariant
      <3>12. /\ AsyncQueueDepth(node)' <= AsyncQueueCapacity
              /\ AsyncCompletionLoad(node)' <= AsyncCompletionReserve
              /\ AsyncIoQueueDepth(node)' <= AsyncIoCapacity
              /\ AsyncCompletionLoad(node)' <= AsyncIoWorkCapacity
        <4>1. AsyncQueueDepth(node)' <= AsyncQueueCapacity
          BY <3>7, <3>11
        <4>2. AsyncCompletionLoad(node)' <= AsyncIoWorkCapacity
          BY <3>1, <3>8, <3>9, <3>10, SMT
             DEF AsyncConfiguration
        <4>3. AsyncIoWorkCapacity <= AsyncCompletionReserve
          BY <3>1 DEF AsyncConfiguration
        <4>4. /\ AsyncCompletionLoad(node)' \in Nat
               /\ AsyncIoWorkCapacity \in Nat
               /\ AsyncCompletionReserve \in Nat
          BY <3>1, <3>8 DEF AsyncConfiguration
        <4>5. AsyncCompletionLoad(node)' <= AsyncCompletionReserve
          BY <4>2, <4>3, <4>4, SMT
        <4>6. AsyncIoQueueDepth(node)' <= AsyncIoCapacity
          BY <3>1, <3>7, <3>8, <3>10, SMT
             DEF AsyncConfiguration, AsyncIoCapacity
        <4> QED BY <4>1, <4>2, <4>5, <4>6
      <3>13. \A other \in ValidatorIds:
                /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
                /\ AsyncCompletionLoad(other)' <= AsyncCompletionReserve
                /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
                /\ AsyncCompletionLoad(other)' <= AsyncIoWorkCapacity
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
                     /\ AsyncCompletionLoad(other)' <=
                          AsyncCompletionReserve
                     /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
                     /\ AsyncCompletionLoad(other)' <=
                          AsyncIoWorkCapacity
          <5>1. CASE other = node
            BY <3>12, <5>1
          <5>2. CASE other # node
            <6>1. /\ asyncCommandQueues'[other] =
                          asyncCommandQueues[other]
                   /\ asyncIoQueues'[other] = asyncIoQueues[other]
                   /\ asyncOutstandingWork'[other] =
                          asyncOutstandingWork[other]
                   /\ asyncDeferredCompletionQueues'[other] =
                          asyncDeferredCompletionQueues[other]
              BY <3>2, <3>4, <4>1, <5>2,
                 FunctionalAppendUpdateAwayFromKey,
                 FunctionalUpdateAwayFromKey, Isa
            <6>2. /\ AsyncQueueDepth(other)' = AsyncQueueDepth(other)
                   /\ AsyncCompletionLoad(other)' =
                          AsyncCompletionLoad(other)
                   /\ AsyncIoQueueDepth(other)' = AsyncIoQueueDepth(other)
              BY <6>1, Isa
                 DEF AsyncQueueDepth, AsyncIoQueueDepth,
                     AsyncCompletionLoad, AsyncOutstandingWorkCount,
                     QueuedCompletionCount, QueuedCompletionIndices,
                     DeferredCompletionCount
            <6> QED BY <3>1, <4>1, <6>2
                 DEF AsyncIoCapacityTypeInvariant
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>13 DEF AsyncIoCapacityTypeInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncAdmissionLimitsBelowQueueCapacity ==
  AsyncConfiguration
    => /\ AsyncNormalLimit < AsyncQueueCapacity
       /\ AsyncProgressLimit < AsyncQueueCapacity
BY SMT
   DEF AsyncConfiguration, AsyncNormalLimit, AsyncProgressLimit

THEOREM NaturalIncrementWithinBound ==
  \A value, bound \in Nat:
    value < bound => value + 1 <= bound
BY SMT

THEOREM StrictLessTransitive ==
  \A lower, middle, upper:
    lower < middle /\ middle < upper => lower < upper
BY SMT

THEOREM EnqueueNonCompletionCandidatePreservesIoType ==
  \A node \in ValidatorIds:
    \A candidate:
      /\ AsyncTypeInvariant
      /\ AsyncCandidateTyped(candidate)
      /\ candidate.node = node
      /\ candidate.class # "Completion"
      /\ CanEnqueueClass(node, candidate.class)
      /\ asyncCommandQueues' =
           [asyncCommandQueues EXCEPT ![node] = Append(@, candidate)]
      /\ UNCHANGED <<AsyncIoVars, asyncDeferredCompletionQueues>>
      => AsyncIoTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW candidate,
                AsyncTypeInvariant,
                AsyncCandidateTyped(candidate),
                candidate.node = node,
                candidate.class # "Completion",
                CanEnqueueClass(node, candidate.class),
                asyncCommandQueues' =
                  [asyncCommandQueues EXCEPT
                     ![node] = Append(@, candidate)],
                UNCHANGED <<AsyncIoVars,
                            asyncDeferredCompletionQueues>>
         PROVE AsyncIoTypeInvariant'
    <2>1. /\ AsyncConfiguration
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>2. AsyncIoTopologyTypeInvariant'
      BY <1>1, <2>1, AsyncIoTopologyTypeStutter
         DEF AsyncIoVars, AsyncIoTopologyTypeVars
    <2>3. AsyncIoQueueContentTypeInvariant'
      BY <1>1, <2>1, AsyncIoQueueContentTypeStutter
         DEF AsyncIoVars, AsyncIoQueueContentTypeVars
    <2>4. AsyncIoWorkContentTypeInvariant'
      <3>1. /\ DOMAIN asyncCommandQueues = ValidatorIds
             /\ AsyncQueueTyped(asyncCommandQueues[node])
             /\ asyncCommandQueues[node] \in
                  Seq(Range(asyncCommandQueues[node]))
        BY <1>1, <2>1
           DEF AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped
      <3>2. /\ asyncOutstandingWork' = asyncOutstandingWork
             /\ asyncIoReadyCompletions' = asyncIoReadyCompletions
             /\ asyncLocalReadyCompletions' =
                  asyncLocalReadyCompletions
        BY <1>1 DEF AsyncIoVars
      <3>3. candidate \notin asyncOutstandingWork[node]
        BY <1>1, <2>1, SMT
           DEF AsyncIoWorkContentTypeInvariant
      <3>4. /\ asyncCommandQueues'[node] =
                    Append(asyncCommandQueues[node], candidate)
             /\ SequenceSet(asyncCommandQueues'[node]) =
                  SequenceSet(asyncCommandQueues[node]) \cup {candidate}
        BY <1>1, <3>1, FunctionalAppendUpdateAtKey,
           SequenceSetAfterAppend
      <3>5. SequenceSet(asyncCommandQueues'[node]) \cap
               asyncOutstandingWork'[node] = {}
        BY <2>1, <3>2, <3>3, <3>4, SMT
           DEF AsyncIoWorkContentTypeInvariant
      <3>6. \A other \in ValidatorIds:
               /\ IsFiniteSet(asyncOutstandingWork'[other])
               /\ (\A work \in asyncOutstandingWork'[other]:
                     /\ AsyncCandidateTyped(work)
                     /\ work.class = "Completion"
                     /\ work.node = other)
               /\ AsyncCompletionSequenceTyped(
                    asyncIoReadyCompletions'[other])
               /\ AsyncCompletionSequenceTyped(
                    asyncLocalReadyCompletions'[other])
               /\ Len(asyncIoReadyCompletions'[other]) =
                    Cardinality(
                      SequenceSet(asyncIoReadyCompletions'[other]))
               /\ Len(asyncLocalReadyCompletions'[other]) =
                    Cardinality(
                      SequenceSet(asyncLocalReadyCompletions'[other]))
               /\ SequenceSet(asyncIoReadyCompletions'[other])
                    \subseteq asyncOutstandingWork'[other]
               /\ SequenceSet(asyncLocalReadyCompletions'[other])
                    \subseteq asyncOutstandingWork'[other]
               /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                    SequenceSet(asyncLocalReadyCompletions'[other]) = {}
               /\ SequenceSet(asyncCommandQueues'[other]) \cap
                    asyncOutstandingWork'[other] = {}
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ IsFiniteSet(asyncOutstandingWork'[other])
                     /\ (\A work \in asyncOutstandingWork'[other]:
                           /\ AsyncCandidateTyped(work)
                           /\ work.class = "Completion"
                           /\ work.node = other)
                     /\ AsyncCompletionSequenceTyped(
                          asyncIoReadyCompletions'[other])
                     /\ AsyncCompletionSequenceTyped(
                          asyncLocalReadyCompletions'[other])
                     /\ Len(asyncIoReadyCompletions'[other]) =
                          Cardinality(
                            SequenceSet(asyncIoReadyCompletions'[other]))
                     /\ Len(asyncLocalReadyCompletions'[other]) =
                          Cardinality(
                            SequenceSet(asyncLocalReadyCompletions'[other]))
                     /\ SequenceSet(asyncIoReadyCompletions'[other])
                          \subseteq asyncOutstandingWork'[other]
                     /\ SequenceSet(asyncLocalReadyCompletions'[other])
                          \subseteq asyncOutstandingWork'[other]
                     /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                          SequenceSet(asyncLocalReadyCompletions'[other]) = {}
                     /\ SequenceSet(asyncCommandQueues'[other]) \cap
                          asyncOutstandingWork'[other] = {}
          <5>1. CASE other = node
            <6>1. /\ asyncOutstandingWork'[other] =
                           asyncOutstandingWork[other]
                   /\ asyncIoReadyCompletions'[other] =
                           asyncIoReadyCompletions[other]
                   /\ asyncLocalReadyCompletions'[other] =
                           asyncLocalReadyCompletions[other]
              BY <3>2, Isa
            <6>2. SequenceSet(asyncCommandQueues'[other]) \cap
                       asyncOutstandingWork'[other] = {}
              BY <3>5, <5>1
            <6>3. /\ IsFiniteSet(asyncOutstandingWork[other])
                   /\ (\A work \in asyncOutstandingWork[other]:
                         /\ AsyncCandidateTyped(work)
                         /\ work.class = "Completion"
                         /\ work.node = other)
                   /\ AsyncCompletionSequenceTyped(
                        asyncIoReadyCompletions[other])
                   /\ AsyncCompletionSequenceTyped(
                        asyncLocalReadyCompletions[other])
                   /\ Len(asyncIoReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncIoReadyCompletions[other]))
                   /\ Len(asyncLocalReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncLocalReadyCompletions[other]))
                   /\ SequenceSet(asyncIoReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncLocalReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncIoReadyCompletions[other]) \cap
                        SequenceSet(asyncLocalReadyCompletions[other]) = {}
              BY <2>1, <4>1 DEF AsyncIoWorkContentTypeInvariant
            <6> QED BY <6>1, <6>2, <6>3, Isa
          <5>2. CASE other # node
            <6>1. asyncCommandQueues'[other] =
                     asyncCommandQueues[other]
              BY <1>1, <3>1, <4>1, <5>2,
                 FunctionalAppendUpdateAwayFromKey
            <6>2. /\ asyncOutstandingWork'[other] =
                           asyncOutstandingWork[other]
                   /\ asyncIoReadyCompletions'[other] =
                           asyncIoReadyCompletions[other]
                   /\ asyncLocalReadyCompletions'[other] =
                           asyncLocalReadyCompletions[other]
              BY <3>2, Isa
            <6>3. /\ IsFiniteSet(asyncOutstandingWork[other])
                   /\ (\A work \in asyncOutstandingWork[other]:
                         /\ AsyncCandidateTyped(work)
                         /\ work.class = "Completion"
                         /\ work.node = other)
                   /\ AsyncCompletionSequenceTyped(
                        asyncIoReadyCompletions[other])
                   /\ AsyncCompletionSequenceTyped(
                        asyncLocalReadyCompletions[other])
                   /\ Len(asyncIoReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncIoReadyCompletions[other]))
                   /\ Len(asyncLocalReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncLocalReadyCompletions[other]))
                   /\ SequenceSet(asyncIoReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncLocalReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncIoReadyCompletions[other]) \cap
                        SequenceSet(asyncLocalReadyCompletions[other]) = {}
                   /\ SequenceSet(asyncCommandQueues[other]) \cap
                        asyncOutstandingWork[other] = {}
              BY <2>1, <4>1 DEF AsyncIoWorkContentTypeInvariant
            <6> QED BY <6>1, <6>2, <6>3, Isa
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>6 DEF AsyncIoWorkContentTypeInvariant
    <2>5. AsyncIoCapacityTypeInvariant'
      <3>1. /\ DOMAIN asyncCommandQueues = ValidatorIds
             /\ AsyncQueueTyped(asyncCommandQueues[node])
             /\ asyncCommandQueues[node] \in
                  Seq(Range(asyncCommandQueues[node]))
             /\ AsyncQueueDepth(node) \in Nat
        BY <1>1, <2>1, LenProperties
           DEF AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped,
               AsyncQueueDepth
      <3>2. /\ asyncOutstandingWork' = asyncOutstandingWork
             /\ asyncIoQueues' = asyncIoQueues
             /\ asyncDeferredCompletionQueues' =
                  asyncDeferredCompletionQueues
        BY <1>1 DEF AsyncIoVars
      <3>3. /\ asyncCommandQueues'[node] =
                    Append(asyncCommandQueues[node], candidate)
             /\ AsyncQueueDepth(node)' = AsyncQueueDepth(node) + 1
        BY <1>1, <3>1, FunctionalAppendUpdateAtKey,
           AppendSequenceFacts, Isa
           DEF AsyncQueueDepth
      <3>4. AsyncCompletionIndices(
                    Append(asyncCommandQueues[node], candidate)) =
                  AsyncCompletionIndices(asyncCommandQueues[node])
        BY <1>1, <3>1, CompletionIndicesAfterNonCompletionAppend
      <3>5. /\ QueuedCompletionCount(node)' =
                    QueuedCompletionCount(node)
             /\ AsyncCompletionLoad(node)' =
                    AsyncCompletionLoad(node)
             /\ AsyncIoQueueDepth(node)' = AsyncIoQueueDepth(node)
        BY <1>1, <3>2, <3>3, <3>4, Isa
           DEF QueuedCompletionCount, QueuedCompletionIndices,
               AsyncCompletionIndices, AsyncCompletionLoad,
               AsyncOutstandingWorkCount, DeferredCompletionCount,
               AsyncIoQueueDepth
      <3>6. AsyncQueueDepth(node) < AsyncQueueCapacity
        <4>1. candidate.class \in {"Normal", "Progress"}
          BY <1>1, SMT DEF AsyncCandidateTyped, AsyncCommandClasses
        <4>2. /\ AsyncNormalLimit < AsyncQueueCapacity
               /\ AsyncProgressLimit < AsyncQueueCapacity
          BY <2>1, AsyncAdmissionLimitsBelowQueueCapacity
        <4>3. CASE candidate.class = "Normal"
          <5>1. AsyncQueueDepth(node) < AsyncNormalLimit
            BY <1>1, <4>3 DEF CanEnqueueClass
          <5> QED BY <4>2, <5>1, StrictLessTransitive
        <4>4. CASE candidate.class = "Progress"
          <5>1. AsyncQueueDepth(node) < AsyncProgressLimit
            BY <1>1, <4>4 DEF CanEnqueueClass
          <5> QED BY <4>2, <5>1, StrictLessTransitive
        <4> QED BY <4>1, <4>3, <4>4
      <3>7. /\ AsyncQueueDepth(node)' <= AsyncQueueCapacity
             /\ AsyncCompletionLoad(node)' <= AsyncCompletionReserve
             /\ AsyncIoQueueDepth(node)' <= AsyncIoCapacity
             /\ AsyncCompletionLoad(node)' <= AsyncIoWorkCapacity
        <4>1. AsyncQueueDepth(node)' <= AsyncQueueCapacity
          <5>1. /\ AsyncQueueDepth(node) \in Nat
                 /\ AsyncQueueCapacity \in Nat
            BY <2>1, <3>1 DEF AsyncConfiguration
          <5>2. AsyncQueueDepth(node) + 1 <= AsyncQueueCapacity
            BY <3>6, <5>1, NaturalIncrementWithinBound
          <5> QED BY <3>3, <5>2
        <4>2. /\ AsyncCompletionLoad(node) <=
                         AsyncCompletionReserve
               /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
               /\ AsyncCompletionLoad(node) <= AsyncIoWorkCapacity
          BY <2>1 DEF AsyncIoCapacityTypeInvariant
        <4> QED BY <3>5, <4>1, <4>2
      <3>8. \A other \in ValidatorIds:
               /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
               /\ AsyncCompletionLoad(other)' <= AsyncCompletionReserve
               /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
               /\ AsyncCompletionLoad(other)' <= AsyncIoWorkCapacity
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
                     /\ AsyncCompletionLoad(other)' <=
                          AsyncCompletionReserve
                     /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
                     /\ AsyncCompletionLoad(other)' <=
                          AsyncIoWorkCapacity
          <5>1. CASE other = node
            BY <3>7, <5>1
          <5>2. CASE other # node
            <6>1. asyncCommandQueues'[other] =
                     asyncCommandQueues[other]
              BY <1>1, <3>1, <4>1, <5>2,
                 FunctionalAppendUpdateAwayFromKey
            <6>2. /\ AsyncQueueDepth(other)' =
                           AsyncQueueDepth(other)
                   /\ AsyncCompletionLoad(other)' =
                           AsyncCompletionLoad(other)
                   /\ AsyncIoQueueDepth(other)' =
                           AsyncIoQueueDepth(other)
              BY <3>2, <6>1, Isa
                 DEF AsyncQueueDepth, AsyncCompletionLoad,
                     AsyncOutstandingWorkCount, QueuedCompletionCount,
                     QueuedCompletionIndices, DeferredCompletionCount,
                     AsyncIoQueueDepth
            <6> QED BY <2>1, <4>1, <6>2
                 DEF AsyncIoCapacityTypeInvariant
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>8 DEF AsyncIoCapacityTypeInvariant
    <2> QED BY <2>2, <2>3, <2>4, <2>5
         DEF AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
  <1> QED BY <1>1

THEOREM CausalAdmissionRunnerPreservesSchedulerType ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ RunNode(node)
    /\ LocalAdmissionStep(node)
    /\ ~(asyncRunnerBudget[node] > 0
           /\ ProducerCompletionCanAdmit(node))
    /\ asyncRunnerBudget[node] > 0
    /\ CausalHeadCanAdvance(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                RunNode(node),
                LocalAdmissionStep(node),
                ~(asyncRunnerBudget[node] > 0
                    /\ ProducerCompletionCanAdmit(node)),
                asyncRunnerBudget[node] > 0,
                CausalHeadCanAdvance(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE Candidate == HeadCausalCandidate(node)
    <2>1. CASE CandidateInFlight(Candidate)
      <3>1. node \in ValidatorIds
        BY <1>1, AsyncCurrentResponsiveVotersAreValidators
           DEF AsyncTypeInvariant
      <3>2. /\ AsyncRuntimeScalarTypeInvariant
             /\ AsyncCausalTypeInvariant
             /\ AsyncIoTopologyTypeInvariant
             /\ AsyncIoContentTypeInvariant
             /\ AsyncIoCapacityTypeInvariant
             /\ AsyncDeferredTopologyTypeInvariant
             /\ AsyncDeferredContentTypeInvariant
             /\ AsyncTransportClockTypeInvariant
             /\ AsyncTransportContentTypeInvariant
             /\ AsyncIngressTopologyTypeInvariant
             /\ AsyncIngressCapacityTypeInvariant
             /\ AsyncIngressContentTypeInvariant
        BY <1>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
               AsyncDeferredTypeInvariant, AsyncTransportTypeInvariant,
               AsyncIngressTypeInvariant
      <3>3. CausalQueueNonempty(node)
        BY <1>1 DEF CausalHeadCanAdvance
      <3>4. AdmitCausalHead(node)
        BY <1>1 DEF LocalAdmissionStep
      <3>5. /\ asyncCausalQueues' =
                    [asyncCausalQueues EXCEPT ![node] = Tail(@)]
             /\ UNCHANGED vars
             /\ UNCHANGED <<asyncCommandQueues, asyncFifoOwed,
                            asyncTimeoutEmitted, AsyncIoVars,
                            asyncOutstandingTags, asyncNodeDeadlines,
                            asyncRetransmitDeadlines, asyncSentItems,
                            asyncRetainedControl, asyncActiveRequests,
                            asyncTransport, asyncIngressLanes,
                            asyncIngressReady, asyncHeldChunks>>
        <4>1. CandidateInFlight(HeadCausalCandidate(node))
          BY <2>1 DEF Candidate
        <4> QED BY <3>4, <4>1, Isa
             DEF AdmitCausalHead, CandidateInFlight,
                 HeadCausalCandidate, AsyncIoVars,
                 LeaveCausalQueues, vars
      <3>6. /\ asyncRunnerPhase' = asyncRunnerPhase
             /\ asyncRunnerBudget' =
                    [asyncRunnerBudget EXCEPT ![node] = @ - 1]
             /\ UNCHANGED AsyncDeferredVars
        BY <1>1 DEF LocalAdmissionStep
      <3>7. RunnerServiceFrame(node)
        BY <1>1 DEF RunNode, RunnerServiceFrame
      <3>8. /\ asyncRunnerBudget
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
             /\ asyncRunnerBudget[node] \in Nat
             /\ asyncRunnerBudget[node] <=
                    AsyncQueueCapacity + AsyncIngressCapacity
             /\ AsyncQueueCapacity \in Nat
             /\ AsyncIngressCapacity \in Nat
        BY <1>1, <3>1, <3>2, SMT
           DEF AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
      <3>9. asyncRunnerBudget[node] - 1
                 \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
        BY <1>1, <3>8, SMT
      <3>10. asyncRunnerBudget'
                 \in [ValidatorIds ->
                       0..(AsyncQueueCapacity + AsyncIngressCapacity)]
        BY <3>1, <3>6, <3>8, <3>9,
           FunctionalUpdatePreservesType
      <3>11. AsyncRuntimeScalarTypeInvariant'
        BY <3>2, <3>5, <3>6, <3>7, <3>10, Isa
           DEF RunnerServiceFrame, AsyncRuntimeScalarTypeInvariant,
               AsyncIoVars, AsyncDeferredVars
      <3>12. AsyncCausalTypeInvariant'
        BY <3>1, <3>2, <3>3, <3>5,
           CausalTailUpdatePreservesCausalType
      <3>13. /\ UNCHANGED AsyncIoTopologyTypeVars
              /\ UNCHANGED AsyncIoContentTypeVars
              /\ UNCHANGED AsyncIoCapacityTypeVars
              /\ UNCHANGED AsyncDeferredTopologyTypeVars
              /\ UNCHANGED <<asyncDeferredCompletionQueues,
                             asyncDeferredProgressQueues,
                             asyncDeferredNormalQueues>>
              /\ UNCHANGED AsyncTransportContentTypeVars
              /\ UNCHANGED AsyncIngressTopologyTypeVars
              /\ UNCHANGED asyncIngressLanes
        BY <3>5, <3>6, Isa
           DEF AsyncIoVars, AsyncDeferredVars,
               AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
               AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
               AsyncTransportContentTypeVars,
               AsyncIngressTopologyTypeVars, vars
      <3>14. /\ AsyncIoTopologyTypeInvariant'
              /\ AsyncIoContentTypeInvariant'
              /\ AsyncIoCapacityTypeInvariant'
              /\ AsyncDeferredTopologyTypeInvariant'
              /\ AsyncDeferredContentTypeInvariant'
              /\ AsyncTransportContentTypeInvariant'
              /\ AsyncIngressTopologyTypeInvariant'
              /\ AsyncIngressCapacityTypeInvariant'
              /\ AsyncIngressContentTypeInvariant'
        BY <3>2, <3>13, AsyncIoTopologyTypeStutter,
           AsyncIoContentTypeStutter, AsyncIoCapacityTypeStutter,
           AsyncDeferredTopologyTypeStutter,
           AsyncDeferredContentTypeStutter,
           AsyncTransportContentTypeStutter,
           AsyncIngressTopologyTypeStutter,
           AsyncIngressCapacityTypeStutter,
           AsyncIngressContentTypeStutter
      <3>15. AsyncTransportClockTypeInvariant'
        BY <3>1, <3>2, <3>5, <3>7,
           RunnerServiceFramePreservesClockType
      <3> QED BY <3>11, <3>12, <3>14, <3>15
           DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
               AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
               AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
    <2>2. CASE ~CandidateInFlight(Candidate)
                /\ Candidate.class = "Completion"
      <3>1. node \in ValidatorIds
        BY <1>1, AsyncCurrentResponsiveVotersAreValidators
           DEF AsyncTypeInvariant
      <3>2. /\ AsyncRuntimeScalarTypeInvariant
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
      <3>3. CausalQueueNonempty(node)
        BY <1>1 DEF CausalHeadCanAdvance
      <3>4. AdmitCausalHead(node)
        BY <1>1 DEF LocalAdmissionStep
      <3>5. /\ asyncCausalQueues' =
                    [asyncCausalQueues EXCEPT ![node] = Tail(@)]
             /\ UNCHANGED vars
             /\ UNCHANGED <<asyncCommandQueues, asyncFifoOwed,
                            asyncTimeoutEmitted,
                            asyncOutstandingTags, asyncNodeDeadlines,
                            asyncRetransmitDeadlines, asyncSentItems,
                            asyncRetainedControl, asyncActiveRequests,
                            asyncTransport, asyncIngressLanes,
                            asyncIngressReady, asyncHeldChunks>>
        BY <2>2, <3>4, Isa
           DEF AdmitCausalHead, Candidate, vars
      <3>6. /\ asyncRunnerPhase' = asyncRunnerPhase
             /\ asyncRunnerBudget' =
                    [asyncRunnerBudget EXCEPT ![node] = @ - 1]
             /\ UNCHANGED AsyncDeferredVars
        BY <1>1 DEF LocalAdmissionStep
      <3>7. RunnerServiceFrame(node)
        BY <1>1 DEF RunNode, RunnerServiceFrame
      <3>8. /\ asyncRunnerBudget
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
             /\ asyncRunnerBudget[node] \in Nat
             /\ asyncRunnerBudget[node] <=
                    AsyncQueueCapacity + AsyncIngressCapacity
             /\ AsyncQueueCapacity \in Nat
             /\ AsyncIngressCapacity \in Nat
        BY <1>1, <3>1, <3>2, SMT
           DEF AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
      <3>9. asyncRunnerBudget[node] - 1
                 \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
        BY <1>1, <3>8, SMT
      <3>10. asyncRunnerBudget'
                 \in [ValidatorIds ->
                       0..(AsyncQueueCapacity + AsyncIngressCapacity)]
        BY <3>1, <3>6, <3>8, <3>9,
           FunctionalUpdatePreservesType
      <3>11. AsyncRuntimeScalarTypeInvariant'
        BY <3>2, <3>5, <3>6, <3>7, <3>10, Isa
           DEF RunnerServiceFrame, AsyncRuntimeScalarTypeInvariant
      <3>12. AsyncCausalTypeInvariant'
        BY <3>1, <3>2, <3>3, <3>5,
           CausalTailUpdatePreservesCausalType
      <3>13. AsyncIoTypeInvariant'
        BY <1>1, <2>2, <3>1, <3>4, <3>6,
           CausalCompletionAdmissionPreservesIoType
           DEF Candidate, AsyncDeferredVars
      <3>14. /\ UNCHANGED AsyncDeferredTopologyTypeVars
              /\ UNCHANGED <<asyncDeferredCompletionQueues,
                             asyncDeferredProgressQueues,
                             asyncDeferredNormalQueues>>
              /\ UNCHANGED AsyncTransportContentTypeVars
              /\ UNCHANGED AsyncIngressTopologyTypeVars
              /\ UNCHANGED asyncIngressLanes
        BY <3>5, <3>6, Isa
           DEF AsyncDeferredVars, AsyncDeferredTopologyTypeVars,
               AsyncTransportContentTypeVars,
               AsyncIngressTopologyTypeVars, vars
      <3>15. /\ AsyncDeferredTopologyTypeInvariant'
              /\ AsyncDeferredContentTypeInvariant'
              /\ AsyncTransportContentTypeInvariant'
              /\ AsyncIngressTopologyTypeInvariant'
              /\ AsyncIngressCapacityTypeInvariant'
              /\ AsyncIngressContentTypeInvariant'
        BY <3>2, <3>14, AsyncDeferredTopologyTypeStutter,
           AsyncDeferredContentTypeStutter,
           AsyncTransportContentTypeStutter,
           AsyncIngressTopologyTypeStutter,
           AsyncIngressCapacityTypeStutter,
           AsyncIngressContentTypeStutter
      <3>16. AsyncTransportClockTypeInvariant'
        BY <3>1, <3>2, <3>5, <3>7,
           RunnerServiceFramePreservesClockType
      <3> QED BY <3>11, <3>12, <3>13, <3>15, <3>16
           DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
               AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
               AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
    <2>3. CASE ~CandidateInFlight(Candidate)
                /\ Candidate.class # "Completion"
      <3>1. node \in ValidatorIds
        BY <1>1, AsyncCurrentResponsiveVotersAreValidators
           DEF AsyncTypeInvariant
      <3>2. /\ AsyncRuntimeScalarTypeInvariant
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
      <3>3. CausalQueueNonempty(node)
        BY <1>1 DEF CausalHeadCanAdvance
      <3>4. AdmitCausalHead(node)
        BY <1>1 DEF LocalAdmissionStep
      <3>5. /\ AsyncCandidateTyped(Candidate)
             /\ Candidate.node = node
             /\ CanEnqueueClass(node, Candidate.class)
        BY <1>1, <2>3, <3>1, <3>2, <3>3,
           CausalHeadCandidateIsTyped, CausalHeadCandidateIsOwned
           DEF CausalHeadCanAdvance, Candidate
      <3>6. /\ asyncCausalQueues' =
                    [asyncCausalQueues EXCEPT ![node] = Tail(@)]
             /\ asyncCommandQueues' =
                    [asyncCommandQueues EXCEPT
                       ![node] = Append(@, Candidate)]
             /\ UNCHANGED <<vars, asyncFifoOwed,
                            asyncTimeoutEmitted, AsyncIoVars,
                            asyncOutstandingTags, asyncNodeDeadlines,
                            asyncRetransmitDeadlines, asyncSentItems,
                            asyncRetainedControl, asyncActiveRequests,
                            asyncTransport, asyncIngressLanes,
                            asyncIngressReady, asyncHeldChunks>>
        BY <2>3, <3>4, <3>5, Isa
           DEF AdmitCausalHead, EnqueueCandidate, Candidate, vars
      <3>7. /\ asyncRunnerPhase' = asyncRunnerPhase
             /\ asyncRunnerBudget' =
                    [asyncRunnerBudget EXCEPT ![node] = @ - 1]
             /\ UNCHANGED AsyncDeferredVars
        BY <1>1 DEF LocalAdmissionStep
      <3>8. RunnerServiceFrame(node)
        BY <1>1 DEF RunNode, RunnerServiceFrame
      <3>9. /\ asyncRunnerBudget
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
             /\ asyncRunnerBudget[node] \in Nat
             /\ asyncRunnerBudget[node] <=
                    AsyncQueueCapacity + AsyncIngressCapacity
             /\ AsyncQueueCapacity \in Nat
             /\ AsyncIngressCapacity \in Nat
        BY <1>1, <3>1, <3>2, SMT
           DEF AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
      <3>10. asyncRunnerBudget[node] - 1
                  \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
        BY <1>1, <3>9, SMT
      <3>11. asyncRunnerBudget'
                  \in [ValidatorIds ->
                        0..(AsyncQueueCapacity + AsyncIngressCapacity)]
        BY <3>1, <3>7, <3>9, <3>10,
           FunctionalUpdatePreservesType
      <3>12. /\ DOMAIN asyncCommandQueues' = ValidatorIds
              /\ \A other \in ValidatorIds:
                   AsyncQueueTyped(asyncCommandQueues'[other])
        <4>1. DOMAIN asyncCommandQueues' = ValidatorIds
          BY <3>1, <3>2, <3>6, Isa
             DEF AsyncRuntimeScalarTypeInvariant
        <4>2. \A other \in ValidatorIds:
                   AsyncQueueTyped(asyncCommandQueues'[other])
          <5>1. ASSUME NEW other \in ValidatorIds
                 PROVE AsyncQueueTyped(asyncCommandQueues'[other])
            <6>1. CASE other = node
              BY <3>2, <3>5, <3>6, <6>1,
                 TypedCandidateAppendPreservesQueueType
                 DEF AsyncRuntimeScalarTypeInvariant
            <6>2. CASE other # node
              <7>1. asyncCommandQueues'[other] =
                       asyncCommandQueues[other]
                BY <3>1, <3>2, <3>6, <5>1, <6>2,
                   FunctionalAppendUpdateAwayFromKey
                   DEF AsyncRuntimeScalarTypeInvariant
              <7> QED BY <3>2, <5>1, <7>1
                   DEF AsyncRuntimeScalarTypeInvariant
            <6> QED BY <6>1, <6>2
          <5> QED BY <5>1
        <4> QED BY <4>1, <4>2
      <3>13. AsyncRuntimeScalarTypeInvariant'
        BY <3>2, <3>6, <3>7, <3>8, <3>11, <3>12, Isa
           DEF RunnerServiceFrame, AsyncRuntimeScalarTypeInvariant,
               AsyncIoVars, AsyncDeferredVars
      <3>14. AsyncCausalTypeInvariant'
        BY <3>1, <3>2, <3>3, <3>6,
           CausalTailUpdatePreservesCausalType
      <3>15. AsyncIoTypeInvariant'
        BY <1>1, <2>3, <3>1, <3>5, <3>6, <3>7,
           EnqueueNonCompletionCandidatePreservesIoType
           DEF AsyncDeferredVars
      <3>16. /\ UNCHANGED AsyncDeferredTopologyTypeVars
              /\ UNCHANGED <<asyncDeferredCompletionQueues,
                             asyncDeferredProgressQueues,
                             asyncDeferredNormalQueues>>
              /\ UNCHANGED AsyncTransportContentTypeVars
              /\ UNCHANGED AsyncIngressTopologyTypeVars
              /\ UNCHANGED asyncIngressLanes
        BY <3>6, <3>7, Isa
           DEF AsyncDeferredVars, AsyncDeferredTopologyTypeVars,
               AsyncTransportContentTypeVars,
               AsyncIngressTopologyTypeVars, vars
      <3>17. /\ AsyncDeferredTopologyTypeInvariant'
              /\ AsyncDeferredContentTypeInvariant'
              /\ AsyncTransportContentTypeInvariant'
              /\ AsyncIngressTopologyTypeInvariant'
              /\ AsyncIngressCapacityTypeInvariant'
              /\ AsyncIngressContentTypeInvariant'
        BY <3>2, <3>16, AsyncDeferredTopologyTypeStutter,
           AsyncDeferredContentTypeStutter,
           AsyncTransportContentTypeStutter,
           AsyncIngressTopologyTypeStutter,
           AsyncIngressCapacityTypeStutter,
           AsyncIngressContentTypeStutter
      <3>18. AsyncTransportClockTypeInvariant'
        BY <3>1, <3>2, <3>6, <3>8,
           RunnerServiceFramePreservesClockType
      <3> QED BY <3>13, <3>14, <3>15, <3>17, <3>18
           DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
               AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
               AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM LocalAdmissionRunnerPreservesSchedulerType ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ RunNode(node)
    /\ LocalAdmissionStep(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                RunNode(node),
                LocalAdmissionStep(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE asyncRunnerBudget[node] > 0
                   /\ ProducerCompletionCanAdmit(node)
      BY <1>1, <2>1, ProducerAdmissionRunnerPreservesSchedulerType
    <2>2. CASE ~(asyncRunnerBudget[node] > 0
                    /\ ProducerCompletionCanAdmit(node))
                /\ asyncRunnerBudget[node] > 0
                /\ CausalHeadCanAdvance(node)
      BY <1>1, <2>2, CausalAdmissionRunnerPreservesSchedulerType
    <2>3. CASE ~(asyncRunnerBudget[node] > 0
                    /\ ProducerCompletionCanAdmit(node))
                /\ ~(asyncRunnerBudget[node] > 0
                       /\ CausalHeadCanAdvance(node))
      BY <1>1, <2>3,
         LocalAdmissionPhaseAdvancePreservesSchedulerType
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM TypedIngressDeliveryCandidateFacts ==
  \A node \in ValidatorIds:
    \A item:
      (AsyncItemTyped(item) /\ item.envelope.recipient = node)
      => /\ AsyncCandidateTyped(DeliveryCandidate(item))
         /\ DeliveryCandidate(item).node = node
         /\ DeliveryCandidate(item).class # "Completion"
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW item,
                AsyncItemTyped(item),
                item.envelope.recipient = node
         PROVE /\ AsyncCandidateTyped(DeliveryCandidate(item))
               /\ DeliveryCandidate(item).node = node
               /\ DeliveryCandidate(item).class # "Completion"
    <2>1. AsyncCandidateTyped(DeliveryCandidate(item))
      BY <1>1, TypedItemMakesTypedDeliveryCandidate
    <2>2. /\ DeliveryCandidate(item).node = node
           /\ DeliveryCandidate(item).class \in {"Normal", "Progress"}
      BY <1>1, DeliveryCandidateShape, SMT DEF DeliveryClass
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM TypedCertifiedResponseCandidateFacts ==
  \A node \in ValidatorIds:
    \A item:
      (AsyncItemTyped(item)
        /\ item.kind = "CertifiedResponse"
        /\ item.envelope.recipient = node)
      => /\ AsyncCandidateTyped(CertifiedResponseCandidate(item))
         /\ CertifiedResponseCandidate(item).node = node
         /\ CertifiedResponseCandidate(item).class = "Completion"
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW item,
                AsyncItemTyped(item),
                item.kind = "CertifiedResponse",
                item.envelope.recipient = node
         PROVE /\ AsyncCandidateTyped(CertifiedResponseCandidate(item))
               /\ CertifiedResponseCandidate(item).node = node
               /\ CertifiedResponseCandidate(item).class = "Completion"
    <2>1. /\ AsyncBodyEnvelopeTyped(item.envelope)
           /\ item \in AsyncNetworkItems
      BY <1>1, SMT DEF AsyncItemTyped, AsyncNetworkItems
    <2>2. /\ "Completion" \in AsyncCommandClasses
           /\ "FetchCertifiedBody" \in AsyncWorkKinds
      BY DEF AsyncCommandClasses, AsyncWorkKinds, AsyncReducerKinds
    <2> QED BY <1>1, <2>1, <2>2, SMT
         DEF CertifiedResponseCandidate, AsyncCandidateTyped,
             AsyncCandidate, AsyncBodyEnvelopeTyped, SubjectOrNone
  <1> QED BY <1>1

THEOREM TypedCommitCertificateResponseCandidateFacts ==
  \A node \in ValidatorIds:
    \A item:
      (AsyncItemTyped(item)
        /\ item.kind = "CommitCertificateResponse"
        /\ item.envelope.recipient = node)
      => /\ AsyncItemTyped(DiscoveredCommitQcItem(item))
         /\ AsyncCandidateTyped(
              CommitCertificateResponseCandidate(item))
         /\ CommitCertificateResponseCandidate(item).node = node
         /\ CommitCertificateResponseCandidate(item).class # "Completion"
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW item,
                AsyncItemTyped(item),
                item.kind = "CommitCertificateResponse",
                item.envelope.recipient = node
         PROVE /\ AsyncItemTyped(DiscoveredCommitQcItem(item))
               /\ AsyncCandidateTyped(
                    CommitCertificateResponseCandidate(item))
               /\ CommitCertificateResponseCandidate(item).node = node
               /\ CommitCertificateResponseCandidate(item).class #
                    "Completion"
    <2>1. /\ item.source \in ValidatorIds
           /\ item.envelope \in QcEnvelopeSet
      BY <1>1, SMT DEF AsyncItemTyped
    <2>2. AsyncItemTyped(DiscoveredCommitQcItem(item))
      BY <1>1, <2>1, SMT
         DEF DiscoveredCommitQcItem, AsyncNetworkItem,
             AsyncItemTyped, AsyncNetworkKinds, AsyncIngressSources
    <2>3. /\ DiscoveredCommitQcItem(item).envelope.recipient = node
           /\ CommitCertificateResponseCandidate(item) =
                DeliveryCandidate(DiscoveredCommitQcItem(item))
      BY <1>1 DEF DiscoveredCommitQcItem,
                    CommitCertificateResponseCandidate,
                    AsyncNetworkItem
    <2> QED BY <1>1, <2>2, <2>3,
         TypedIngressDeliveryCandidateFacts
  <1> QED BY <1>1

THEOREM RemoveRequestsAndAddSentPreservesTransportContentType ==
  \A removed, additions:
    /\ AsyncTransportContentTypeInvariant
    /\ IsFiniteSet(additions)
    /\ \A item \in additions: AsyncItemTyped(item)
    /\ asyncSentItems' = asyncSentItems \cup additions
    /\ asyncActiveRequests' = asyncActiveRequests \ removed
    /\ asyncRetainedControl' = asyncRetainedControl
    /\ UNCHANGED <<context, asyncTransport, asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW removed, NEW additions,
                AsyncTransportContentTypeInvariant,
                IsFiniteSet(additions),
                \A item \in additions: AsyncItemTyped(item),
                asyncSentItems' = asyncSentItems \cup additions,
                asyncActiveRequests' = asyncActiveRequests \ removed,
                asyncRetainedControl' = asyncRetainedControl,
                UNCHANGED <<context, asyncTransport, asyncHeldChunks>>
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
    <2>2. AsyncSentItemsType(asyncSentItems')
      <3>1. IsFiniteSet(asyncSentItems')
        BY <1>1, <2>1, FS_Union DEF AsyncSentItemsType
      <3>2. \A item \in asyncSentItems': AsyncItemTyped(item)
        BY <1>1, <2>1 DEF AsyncSentItemsType
      <3> QED BY <3>1, <3>2 DEF AsyncSentItemsType
    <2>3. CurrentVoters' = CurrentVoters
      BY <1>1, Isa DEF CurrentVoters, CurrentEpoch
    <2>4. AsyncRetainedControlType(
             asyncRetainedControl', CurrentVoters')
      BY <1>1, <2>1, <2>3
    <2>5. AsyncActiveRequestsType(
             asyncActiveRequests', asyncSentItems')
      <3>1. asyncActiveRequests' \subseteq asyncActiveRequests
        BY <1>1
      <3>2. IsFiniteSet(asyncActiveRequests')
        BY <2>1, <3>1, FS_Subset DEF AsyncActiveRequestsType
      <3>3. /\ asyncActiveRequests' \subseteq asyncSentItems'
             /\ \A item \in asyncActiveRequests':
                  /\ AsyncItemTyped(item)
                  /\ item.kind \in {"CertifiedRequest",
                                      "CommitCertificateRequest"}
        BY <1>1, <2>1, <3>1 DEF AsyncActiveRequestsType
      <3> QED BY <3>2, <3>3 DEF AsyncActiveRequestsType
    <2>6. AsyncTransportHistoryTypeInvariant'
      BY <2>2, <2>4, <2>5,
         AsyncTransportHistoryTypeDecomposition
    <2>7. /\ AsyncPacketContentTypeInvariant'
           /\ AsyncHeldChunksTypeInvariant'
      BY <1>1, <2>1
         DEF AsyncPacketContentTypeInvariant,
             AsyncHeldChunksTypeInvariant
    <2> QED BY <2>6, <2>7
         DEF AsyncTransportContentTypeInvariant
  <1> QED BY <1>1

THEOREM IngressSelectedPreservesRuntimeScalarType ==
  \A node \in ValidatorIds:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ asyncRunnerBudget[node] > 0
    /\ asyncRunnerPhase' = asyncRunnerPhase
    /\ asyncRunnerBudget' =
         [asyncRunnerBudget EXCEPT ![node] = @ - 1]
    /\ UNCHANGED <<asyncNow, asyncFifoOwed, asyncTimeoutEmitted>>
    /\ \/ asyncCommandQueues' = asyncCommandQueues
       \/ \E candidate:
            /\ AsyncCandidateTyped(candidate)
            /\ candidate.node = node
            /\ asyncCommandQueues' =
                 [asyncCommandQueues EXCEPT
                    ![node] = Append(@, candidate)]
    => AsyncRuntimeScalarTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncRuntimeScalarTypeInvariant,
                asyncRunnerBudget[node] > 0,
                asyncRunnerPhase' = asyncRunnerPhase,
                asyncRunnerBudget' =
                  [asyncRunnerBudget EXCEPT ![node] = @ - 1],
                UNCHANGED <<asyncNow, asyncFifoOwed,
                            asyncTimeoutEmitted>>,
                \/ asyncCommandQueues' = asyncCommandQueues
                \/ \E candidate:
                     /\ AsyncCandidateTyped(candidate)
                     /\ candidate.node = node
                     /\ asyncCommandQueues' =
                          [asyncCommandQueues EXCEPT
                             ![node] = Append(@, candidate)]
         PROVE AsyncRuntimeScalarTypeInvariant'
    <2>1. /\ DOMAIN asyncCommandQueues = ValidatorIds
           /\ \A other \in ValidatorIds:
                AsyncQueueTyped(asyncCommandQueues[other])
           /\ asyncRunnerBudget \in
                [ValidatorIds ->
                  0..(AsyncQueueCapacity + AsyncIngressCapacity)]
      BY <1>1 DEF AsyncRuntimeScalarTypeInvariant
    <2>2. asyncRunnerBudget[node] - 1
               \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
      BY <1>1, <2>1, SMT
    <2>3. asyncRunnerBudget' \in
             [ValidatorIds ->
               0..(AsyncQueueCapacity + AsyncIngressCapacity)]
      BY <1>1, <2>1, <2>2, FunctionalUpdatePreservesType
    <2>4. /\ DOMAIN asyncCommandQueues' = ValidatorIds
           /\ \A other \in ValidatorIds:
                AsyncQueueTyped(asyncCommandQueues'[other])
      <3>1. CASE asyncCommandQueues' = asyncCommandQueues
        BY <2>1, <3>1
      <3>2. CASE \E candidate:
                       /\ AsyncCandidateTyped(candidate)
                       /\ candidate.node = node
                       /\ asyncCommandQueues' =
                            [asyncCommandQueues EXCEPT
                               ![node] = Append(@, candidate)]
        <4>1. PICK candidate:
                       /\ AsyncCandidateTyped(candidate)
                       /\ candidate.node = node
                       /\ asyncCommandQueues' =
                            [asyncCommandQueues EXCEPT
                               ![node] = Append(@, candidate)]
          BY <3>2
        <4>2. AsyncQueueTyped(
                 Append(asyncCommandQueues[node], candidate))
          BY <2>1, <4>1, TypedCandidateAppendPreservesQueueType
        <4>3. \A other \in ValidatorIds:
                 AsyncQueueTyped(asyncCommandQueues'[other])
          <5>1. ASSUME NEW other \in ValidatorIds
                 PROVE AsyncQueueTyped(asyncCommandQueues'[other])
            <6>1. CASE other = node
              BY <2>1, <4>1, <4>2, <6>1,
                 FunctionalAppendUpdateAtKey
            <6>2. CASE other # node
              BY <2>1, <4>1, <5>1, <6>2,
                 FunctionalUpdateAwayFromKey
            <6> QED BY <6>1, <6>2
          <5> QED BY <5>1
        <4> QED BY <2>1, <4>1, <4>3, Isa
      <3> QED BY <1>1, <3>1, <3>2
    <2> QED BY <1>1, <2>3, <2>4
         DEF AsyncRuntimeScalarTypeInvariant
  <1> QED BY <1>1

THEOREM IngressPhaseAdvancePreservesSchedulerType ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ RunNode(node)
    /\ IngressDrainStep(node)
    /\ ~(asyncRunnerBudget[node] > 0
           /\ asyncIngressReady[node] # <<>>
           /\ DrainableIngressIndices(node) # {})
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                RunNode(node),
                IngressDrainStep(node),
                ~(asyncRunnerBudget[node] > 0
                    /\ asyncIngressReady[node] # <<>>
                    /\ DrainableIngressIndices(node) # {})
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncTypeInvariant
    <2>2. /\ RunnerServiceFrame(node)
           /\ asyncRunnerPhase' =
                [asyncRunnerPhase EXCEPT ![node] = "Runtime"]
           /\ asyncRunnerBudget' =
                [asyncRunnerBudget EXCEPT ![node] = 1]
           /\ UNCHANGED <<asyncCommandQueues, asyncFifoOwed,
                          asyncTimeoutEmitted, asyncCausalQueues,
                          AsyncIoVars, AsyncDeferredVars,
                          asyncOutstandingTags, asyncNodeDeadlines,
                          asyncRetransmitDeadlines, asyncSentItems,
                          asyncRetainedControl, asyncActiveRequests,
                          asyncTransport, asyncIngressLanes,
                          asyncIngressReady, asyncHeldChunks>>
      BY <1>1, Isa
         DEF RunNode, RunnerServiceFrame, IngressDrainStep,
             LeaveCausalQueues, vars
    <2>3. /\ asyncRunnerPhase
                    \in [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
           /\ asyncRunnerBudget
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
           /\ 1 \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
      BY <1>1, SMT
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncConfiguration
    <2>4. /\ asyncRunnerPhase'
                    \in [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
           /\ asyncRunnerBudget'
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
      BY <2>1, <2>2, <2>3, FunctionalUpdatePreservesType
    <2> QED BY <1>1, <2>1, <2>2, <2>4,
                    RunnerScalarClockAndSchedulerStutterPreservesType
  <1> QED BY <1>1

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

(***************************************************************************
Timeout-certificate progress boundary.

The production timeout-vote map retains one receipt per
recipient/context/view/signer.  `TimeoutReceiptSignerUniqueAt` records that
concrete ingress property: together with finite receipt storage it is exactly
the missing bridge from a responsive receipt quorum to the disjoint signer
set required by `TCValid`.  The temporal proof may use these milestones only
after deriving them from `AsyncSpecAt`; none is an additional fairness or
deployment assumption.
***************************************************************************)

TimeoutViewGoal(node, roundView) ==
  nodeView[node] > roundView \/ NodeHasDecision(node)

DurableTimeoutVoteAt(node, roundView) ==
  \E vote \in timeoutIntents:
    /\ vote.signer = node
    /\ vote.context = context
    /\ vote.view = roundView

SentTimeoutVoteAt(signer, recipient, roundView) ==
  \E item \in asyncSentItems:
    /\ item.kind = "TimeoutVote"
    /\ item.source = signer
    /\ item.envelope.recipient = recipient
    /\ item.envelope.vote.view = roundView
    /\ item.envelope.vote.signer = signer

ReceivedTimeoutVoteAt(recipient, signer, roundView) ==
  \E received \in receivedTimeoutVotes:
    /\ received.node = recipient
    /\ received.vote.context = context
    /\ received.vote.view = roundView
    /\ received.vote.signer = signer

TimeoutReceiptSignerUniqueAt(recipient, roundView) ==
  \A left, right \in TimeoutVotesAt(recipient, roundView):
    left.signer = right.signer => left = right

ResponsiveTimeoutReceiptQuorumAt(recipient, roundView) ==
  \A signer \in AsyncCurrentResponsiveVoters:
    ReceivedTimeoutVoteAt(recipient, signer, roundView)

TimeoutSignerMap(votes) == [vote \in votes |-> vote.signer]

THEOREM PersistTimeoutMakesVoteDurable ==
  \A request \in pendingTimeout:
    (StrongInductiveInvariant /\ PersistTimeout(request))
      => DurableTimeoutVoteAt(request.node, request.vote.view)'
BY SMTT(30)
   DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
       PendingVoteWritesAuthorized, PersistTimeout,
       DurableTimeoutVoteAt

THEOREM ExecuteSignTimeoutPublishesEveryRecipient ==
  \A command:
    ExecuteSignTimeout(command)
      => \A recipient \in CurrentVoters:
           SentTimeoutVoteAt(command.node, recipient, command.view)'
BY SMTT(30)
   DEF ExecuteSignTimeout, CommandMatches, CompleteTimeoutSignature,
       PublishControlItems, TimeoutOutbox, SentTimeoutVoteAt,
       AsyncNetworkItem, TimeoutEnvelope

THEOREM ExecuteCoreTimeoutDeliveryRecordsReceipt ==
  \A command:
    (ExecuteCoreDelivery(command) /\ command.kind = "DeliverTimeout")
      => ReceivedTimeoutVoteAt(
           command.node, command.item.envelope.vote.signer,
           command.item.envelope.vote.view)'
BY SMTT(30)
   DEF ExecuteCoreDelivery, DeliverTimeout, ReceivedTimeoutVoteAt,
       TimeoutVoteAt

THEOREM ExecutePersistInstallAdvancesCertifiedView ==
  \A command:
    ExecutePersistInstall(command)
      => TimeoutViewGoal(command.node, command.view)'
BY SMTT(30)
   DEF ExecutePersistInstall, PersistInstallTC, TimeoutViewGoal

THEOREM TimeoutSignerMapRange ==
  \A votes:
    Range(TimeoutSignerMap(votes)) = TimeoutSignerSet(votes)
PROOF
  <1>1. ASSUME NEW votes
         PROVE Range(TimeoutSignerMap(votes)) = TimeoutSignerSet(votes)
    <2>1. Range(TimeoutSignerMap(votes)) =
             {TimeoutSignerMap(votes)[vote]: vote \in votes}
      BY Isa DEF TimeoutSignerMap, Range
    <2>2. {TimeoutSignerMap(votes)[vote]: vote \in votes} =
             {vote.signer: vote \in votes}
      BY Isa DEF TimeoutSignerMap
    <2> QED BY <2>1, <2>2 DEF TimeoutSignerSet
  <1> QED BY <1>1

THEOREM TimeoutSignerMapSurjects ==
  \A votes:
    TimeoutSignerMap(votes)
      \in Surjection(votes, TimeoutSignerSet(votes))
BY TimeoutSignerMapRange, Fun_RangeProperties
   DEF TimeoutSignerMap

THEOREM UniqueFiniteTimeoutVotesAreDisjoint ==
  \A votes:
    (IsFiniteSet(votes)
      /\ (\A left, right \in votes:
            left.signer = right.signer => left = right))
      => TimeoutVotesDisjoint(votes)
PROOF
  <1>1. ASSUME NEW votes,
                IsFiniteSet(votes)
                  /\ (\A left, right \in votes:
                        left.signer = right.signer => left = right)
         PROVE TimeoutVotesDisjoint(votes)
    <2>1. TimeoutSignerMap(votes)
             \in Surjection(votes, TimeoutSignerSet(votes))
      BY TimeoutSignerMapSurjects
    <2>2. TimeoutSignerMap(votes)
             \in Injection(votes, TimeoutSignerSet(votes))
      BY <1>1, Isa DEF TimeoutSignerMap, Injection
    <2>3. Cardinality(TimeoutSignerSet(votes)) = Cardinality(votes)
      BY <1>1, <2>1, <2>2, FS_Surjection
    <2> QED BY <2>3 DEF TimeoutVotesDisjoint
  <1> QED BY <1>1

THEOREM UniqueFiniteTimeoutReceiptsAreDisjoint ==
  \A recipient \in ValidatorIds, roundView \in Views:
    (IsFiniteSet(TimeoutVotesAt(recipient, roundView))
      /\ TimeoutReceiptSignerUniqueAt(recipient, roundView))
      => TimeoutVotesDisjoint(TimeoutVotesAt(recipient, roundView))
BY UniqueFiniteTimeoutVotesAreDisjoint
   DEF TimeoutReceiptSignerUniqueAt

THEOREM TimeoutPoolMakesVoteSetsFinite ==
  \A recipient \in ValidatorIds, roundView \in Views:
    ReceivedTimeoutVotePoolInvariant
      => IsFiniteSet(TimeoutVotesAt(recipient, roundView))
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW roundView \in Views,
                ReceivedTimeoutVotePoolInvariant
         PROVE IsFiniteSet(TimeoutVotesAt(recipient, roundView))
    <2> DEFINE Matching ==
          {entry \in receivedTimeoutVotes:
             /\ entry.node = recipient
             /\ entry.vote.context = context
             /\ entry.vote.view = roundView}
    <2>1. IsFiniteSet(receivedTimeoutVotes)
      BY <1>1 DEF ReceivedTimeoutVotePoolInvariant
    <2>2. Matching \subseteq receivedTimeoutVotes
      BY DEF Matching
    <2>3. IsFiniteSet(Matching)
      BY <2>1, <2>2, Isa
    <2>4. LET voteSet == {entry.vote: entry \in Matching}
           IN IsFiniteSet(voteSet)
      BY <2>3, FS_Image
    <2>5. TimeoutVotesAt(recipient, roundView) =
             {entry.vote: entry \in Matching}
      BY Isa DEF TimeoutVotesAt, Matching
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM TimeoutPoolMakesSignerSlotsUnique ==
  \A recipient \in ValidatorIds, roundView \in Views:
    ReceivedTimeoutVotePoolInvariant
      => TimeoutReceiptSignerUniqueAt(recipient, roundView)
BY SMTT(30)
   DEF ReceivedTimeoutVotePoolInvariant,
       ReceivedTimeoutVoteSlotsUnique, SameTimeoutVoteSlot,
       TimeoutReceiptSignerUniqueAt, TimeoutVotesAt

THEOREM TimeoutPoolMakesVotesDisjoint ==
  \A recipient \in ValidatorIds, roundView \in Views:
    ReceivedTimeoutVotePoolInvariant
      => TimeoutVotesDisjoint(TimeoutVotesAt(recipient, roundView))
BY TimeoutPoolMakesVoteSetsFinite,
   TimeoutPoolMakesSignerSlotsUnique,
   UniqueFiniteTimeoutReceiptsAreDisjoint

THEOREM ConflictingTimeoutDeliveryDoesNotGrowPool ==
  \A envelope:
    (TimeoutVoteSlotOccupied(envelope.recipient, envelope.vote)
      /\ DeliverTimeout(envelope))
      => receivedTimeoutVotes' = receivedTimeoutVotes
BY SMT DEF DeliverTimeout

THEOREM DeliverTimeoutPreservesSlotUniqueness ==
  \A envelope:
    (ReceivedTimeoutVoteSlotsUnique /\ DeliverTimeout(envelope))
      => ReceivedTimeoutVoteSlotsUnique'
BY SMTT(30)
   DEF DeliverTimeout, TimeoutVoteSlotOccupied,
       ReceivedTimeoutVoteSlotsUnique, SameTimeoutVoteSlot,
       TimeoutVoteAt

THEOREM TypedDeliverTimeoutPreservesPoolInvariant ==
  \A envelope \in TimeoutEnvelopeSet:
    (ReceivedTimeoutVotePoolInvariant /\ DeliverTimeout(envelope))
      => ReceivedTimeoutVotePoolInvariant'
PROOF
  <1>1. ASSUME NEW envelope \in TimeoutEnvelopeSet,
                ReceivedTimeoutVotePoolInvariant,
                DeliverTimeout(envelope)
         PROVE ReceivedTimeoutVotePoolInvariant'
    <2> DEFINE Received ==
          TimeoutVoteAt(envelope.recipient, envelope.vote)
    <2>1. ReceivedTimeoutVoteSlotsUnique'
      <3>1. ReceivedTimeoutVoteSlotsUnique
        BY <1>1 DEF ReceivedTimeoutVotePoolInvariant
      <3>2. DeliverTimeout(envelope)
        BY <1>1
      <3>3. (ReceivedTimeoutVoteSlotsUnique
               /\ DeliverTimeout(envelope))
              => ReceivedTimeoutVoteSlotsUnique'
        BY DeliverTimeoutPreservesSlotUniqueness
      <3> QED BY <3>1, <3>2, <3>3
    <2>2. receivedTimeoutVotes' = receivedTimeoutVotes
             \/ receivedTimeoutVotes' =
                  receivedTimeoutVotes \cup {Received}
      BY <1>1 DEF DeliverTimeout, Received
    <2>3. IsFiniteSet(receivedTimeoutVotes')
      <3>1. IsFiniteSet(receivedTimeoutVotes)
        BY <1>1 DEF ReceivedTimeoutVotePoolInvariant
      <3>2. IsFiniteSet(receivedTimeoutVotes \cup {Received})
        BY <3>1, FS_AddElement
      <3> QED BY <2>2, <3>1, <3>2
    <2>4. /\ Received.node \in ValidatorIds
          /\ Received.vote \in TimeoutVoteRecordSet
          /\ Received.vote.context = context'
          /\ Received.vote.height = height'
          /\ Received.vote.signer \in CurrentVoters'
          /\ AuthenticatedHighRef(
               Received.vote.highRank, Received.vote.highSubject)'
          /\ Received.vote.highRank <= Received.vote.view
      <3>1. /\ Received.node = envelope.recipient
            /\ Received.vote = envelope.vote
        BY DEF Received, TimeoutVoteAt
      <3>2. /\ envelope.recipient \in ValidatorIds
            /\ envelope.vote \in TimeoutVoteRecordSet
        BY <1>1 DEF TimeoutEnvelopeSet
      <3>3. /\ context' = context
            /\ height' = height
            /\ prepareQCs' = prepareQCs
        BY <1>1 DEF DeliverTimeout
      <3>4. /\ envelope.vote.context = context
            /\ envelope.vote.height = height
            /\ envelope.vote.signer \in CurrentVoters
            /\ AuthenticatedHighRef(
                 envelope.vote.highRank,
                 envelope.vote.highSubject)
            /\ envelope.vote.highRank <= envelope.vote.view
        BY <1>1 DEF DeliverTimeout
      <3>5. /\ CurrentVoters' = CurrentVoters
            /\ (AuthenticatedHighRef(
                  envelope.vote.highRank,
                  envelope.vote.highSubject)'
                  <=> AuthenticatedHighRef(
                        envelope.vote.highRank,
                        envelope.vote.highSubject))
        BY <3>3, Isa
           DEF CurrentVoters, CurrentEpoch,
               AuthenticatedHighRef, HighRefValid
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
    <2>5. \A received \in receivedTimeoutVotes:
             /\ received.node \in ValidatorIds
             /\ received.vote \in TimeoutVoteRecordSet
             /\ received.vote.context = context'
             /\ received.vote.height = height'
             /\ received.vote.signer \in CurrentVoters'
             /\ AuthenticatedHighRef(
                  received.vote.highRank,
                  received.vote.highSubject)'
             /\ received.vote.highRank <= received.vote.view
      <3>1. /\ context' = context
            /\ height' = height
            /\ prepareQCs' = prepareQCs
        BY <1>1 DEF DeliverTimeout
      <3>2. CurrentVoters' = CurrentVoters
        BY <3>1 DEF CurrentVoters, CurrentEpoch
      <3>3. \A highRank, highSubject:
               AuthenticatedHighRef(highRank, highSubject)'
                 <=> AuthenticatedHighRef(highRank, highSubject)
        BY <3>1, Isa DEF AuthenticatedHighRef, HighRefValid
      <3> QED BY <1>1, <3>1, <3>2, <3>3
         DEF ReceivedTimeoutVotePoolInvariant
    <2>6. \A received \in receivedTimeoutVotes':
             /\ received.node \in ValidatorIds
             /\ received.vote \in TimeoutVoteRecordSet
             /\ received.vote.context = context'
             /\ received.vote.height = height'
             /\ received.vote.signer \in CurrentVoters'
             /\ AuthenticatedHighRef(
                  received.vote.highRank,
                  received.vote.highSubject)'
             /\ received.vote.highRank <= received.vote.view
      BY <2>2, <2>4, <2>5, Isa
    <2> QED BY <2>1, <2>3, <2>6
       DEF ReceivedTimeoutVotePoolInvariant
  <1> QED BY <1>1

THEOREM CoreNextKeepsPrepareQcsMonotone ==
  Next => prepareQCs \subseteq prepareQCs'
PROOF
  <1>1. ASSUME Next
         PROVE prepareQCs \subseteq prepareQCs'
    <2>1. CASE SetGST
      BY <2>1, Isa DEF SetGST
    <2>2. CASE \E node \in ValidatorIds, subject \in Subjects:
                  AssembleLocalBody(node, subject)
      BY <2>2, Isa DEF AssembleLocalBody
    <2>3. CASE \E node \in ValidatorIds, subject \in Subjects:
                  BeginLocalProposal(node, subject)
      BY <2>3, Isa DEF BeginLocalProposal
    <2>4. CASE \E request \in pendingProposal: PersistProposal(request)
      BY <2>4, Isa DEF PersistProposal
    <2>5. CASE \E request \in signProposals:
                  CompleteProposalSignature(request)
      BY <2>5, Isa DEF CompleteProposalSignature
    <2>6. CASE \E signer \in ValidatorIds, roundView \in Views,
                  subject \in Subjects, justifyRank \in Ranks,
                  justifySubject \in SubjectOrNone:
                  ByzantineBroadcastProposal(
                    signer, roundView, subject, justifyRank, justifySubject)
      BY <2>6, Isa DEF ByzantineBroadcastProposal
    <2>7. CASE \E envelope \in proposalNetwork:
                  DeliverProposal(envelope)
      BY <2>7, Isa DEF DeliverProposal
    <2>8. CASE \E node \in ValidatorIds,
                  proposal \in SeenProposalValues:
                  FetchBody(node, proposal)
      BY <2>8, Isa DEF FetchBody
    <2>9. CASE \E node \in ValidatorIds, subject \in Subjects:
                  StoreBody(node, subject)
      BY <2>9, Isa DEF StoreBody
    <2>10. CASE \E node \in ValidatorIds,
                   proposal \in SeenProposalValues:
                   ValidateBody(node, proposal) \/ RejectBody(node, proposal)
      BY <2>10, Isa DEF ValidateBody, RejectBody
    <2>11. CASE \E node \in ValidatorIds,
                   proposal \in SeenProposalValues:
                   BeginPrepare(node, proposal)
      BY <2>11, Isa DEF BeginPrepare
    <2>12. CASE \E request \in pendingPrepare: PersistPrepare(request)
      BY <2>12, Isa DEF PersistPrepare
    <2>13. CASE \E request \in signVotes: CompleteVoteSignature(request)
      BY <2>13, Isa DEF CompleteVoteSignature
    <2>14. CASE \E signer \in ValidatorIds, roundView \in Views,
                   phase \in Phases, subject \in Subjects:
                   ByzantineBroadcastVote(
                     signer, roundView, phase, subject)
      BY <2>14, Isa DEF ByzantineBroadcastVote
    <2>15. CASE \E envelope \in voteNetwork: DeliverVote(envelope)
      BY <2>15, Isa DEF DeliverVote
    <2>16. CASE \E node \in ValidatorIds, roundView \in Views,
                   subject \in Subjects:
                   FormPrepareQC(node, roundView, subject)
      BY <2>16, Isa DEF FormPrepareQC
    <2>17. CASE \E envelope \in qcNetwork: DeliverQC(envelope)
      BY <2>17, Isa DEF DeliverQC
    <2>18. CASE \E node \in ValidatorIds, qc \in ReceivedQcValues:
                   BeginObservePrepare(node, qc)
      BY <2>18, Isa DEF BeginObservePrepare
    <2>19. CASE \E request \in pendingObservePrepare:
                   PersistObservePrepare(request)
      BY <2>19, Isa DEF PersistObservePrepare
    <2>20. CASE \E node \in ValidatorIds, qc \in ReceivedQcValues:
                   BeginLockCommit(node, qc)
      BY <2>20, Isa DEF BeginLockCommit
    <2>21. CASE \E request \in pendingLockCommit:
                   PersistLockCommit(request)
      BY <2>21, Isa DEF PersistLockCommit
    <2>22. CASE \E node \in ValidatorIds, roundView \in Views,
                   subject \in Subjects:
                   FormCommitQC(node, roundView, subject)
      BY <2>22, Isa DEF FormCommitQC
    <2>23. CASE \E node \in ValidatorIds, qc \in ReceivedQcValues:
                   BeginDecision(node, qc)
      BY <2>23, Isa DEF BeginDecision
    <2>24. CASE \E request \in pendingDecision: PersistDecision(request)
      BY <2>24, Isa DEF PersistDecision
    <2>25. CASE \E node \in ValidatorIds: BeginTimeout(node)
      BY <2>25, Isa DEF BeginTimeout
    <2>26. CASE \E request \in pendingTimeout: PersistTimeout(request)
      BY <2>26, Isa DEF PersistTimeout
    <2>27. CASE \E request \in signTimeouts:
                   CompleteTimeoutSignature(request)
      BY <2>27, Isa DEF CompleteTimeoutSignature
    <2>28. CASE \E signer \in ValidatorIds, roundView \in Views,
                   highRank \in Ranks, highSubject \in SubjectOrNone:
                   ByzantineBroadcastTimeout(
                     signer, roundView, highRank, highSubject)
      BY <2>28, Isa DEF ByzantineBroadcastTimeout
    <2>29. CASE \E envelope \in timeoutNetwork: DeliverTimeout(envelope)
      BY <2>29, Isa DEF DeliverTimeout
    <2>30. CASE \E node \in ValidatorIds, roundView \in Views:
                   FormTC(node, roundView)
      BY <2>30, Isa DEF FormTC
    <2>31. CASE \E envelope \in tcNetwork: DeliverTC(envelope)
      BY <2>31, Isa DEF DeliverTC
    <2>32. CASE \E node \in ValidatorIds, tc \in ReceivedTcValues:
                   BeginInstallTC(node, tc)
      BY <2>32, Isa DEF BeginInstallTC
    <2>33. CASE \E request \in pendingInstallTC: PersistInstallTC(request)
      BY <2>33, Isa DEF PersistInstallTC
    <2>34. CASE \E node \in ValidatorIds, qc \in DecisionQcValues:
                   FetchCertifiedBody(node, qc)
      BY <2>34, Isa DEF FetchCertifiedBody
    <2>35. CASE \E node \in ValidatorIds, qc \in DecisionQcValues:
                   ApplyDecision(node, qc)
      BY <2>35, Isa DEF ApplyDecision
    <2>36. CASE \E node \in ValidatorIds: Crash(node) \/ Restart(node)
      BY <2>36, Isa DEF Crash, Restart
    <2>37. CASE \E node \in ValidatorIds,
                   proposal \in proposalIntents:
                   ResumeProposal(node, proposal)
      BY <2>37, Isa DEF ResumeProposal
    <2>38. CASE \E node \in ValidatorIds,
                   vote \in prepareIntents \cup commitIntents:
                   ResumeVote(node, vote)
      BY <2>38, Isa DEF ResumeVote
    <2>39. CASE \E node \in ValidatorIds, vote \in timeoutIntents:
                   ResumeTimeout(node, vote)
      BY <2>39, Isa DEF ResumeTimeout
    <2>40. CASE \E envelope \in proposalNetwork: DropProposal(envelope)
      BY <2>40, Isa DEF DropProposal
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
                <2>8, <2>9, <2>10, <2>11, <2>12, <2>13, <2>14,
                <2>15, <2>16, <2>17, <2>18, <2>19, <2>20, <2>21,
                <2>22, <2>23, <2>24, <2>25, <2>26, <2>27, <2>28,
                <2>29, <2>30, <2>31, <2>32, <2>33, <2>34, <2>35,
                <2>36, <2>37, <2>38, <2>39, <2>40
         DEF Next
  <1> QED BY <1>1

THEOREM AuthenticatedHighRefSurvivesPrepareQcGrowth ==
  \A highRank, highSubject:
    (context' = context
      /\ prepareQCs \subseteq prepareQCs'
      /\ AuthenticatedHighRef(highRank, highSubject))
      => AuthenticatedHighRef(highRank, highSubject)'
BY SMT DEF AuthenticatedHighRef, HighRefValid

THEOREM TimeoutPoolFramePreservesInvariant ==
  (ReceivedTimeoutVotePoolInvariant
    /\ receivedTimeoutVotes' = receivedTimeoutVotes
    /\ context' = context
    /\ height' = height
    /\ prepareQCs \subseteq prepareQCs')
    => ReceivedTimeoutVotePoolInvariant'
PROOF
  <1>1. ASSUME ReceivedTimeoutVotePoolInvariant,
              receivedTimeoutVotes' = receivedTimeoutVotes,
              context' = context,
              height' = height,
              prepareQCs \subseteq prepareQCs'
         PROVE ReceivedTimeoutVotePoolInvariant'
    <2>1. CurrentVoters' = CurrentVoters
      BY <1>1, Isa DEF CurrentVoters, CurrentEpoch
    <2>2. \A highRank, highSubject:
             AuthenticatedHighRef(highRank, highSubject)
               => AuthenticatedHighRef(highRank, highSubject)'
      BY <1>1, AuthenticatedHighRefSurvivesPrepareQcGrowth
    <2> QED BY <1>1, <2>1, <2>2, Isa
         DEF ReceivedTimeoutVotePoolInvariant,
             ReceivedTimeoutVoteSlotsUnique, SameTimeoutVoteSlot
  <1> QED BY <1>1

THEOREM ChangedCoreTimeoutDeliveryIsTyped ==
  \A command:
    (AsyncSchedulerTypeInvariant
      /\ ExecuteCoreDelivery(command)
      /\ receivedTimeoutVotes' # receivedTimeoutVotes)
      => \E envelope \in TimeoutEnvelopeSet: DeliverTimeout(envelope)
PROOF
  <1>1. ASSUME NEW command,
              AsyncSchedulerTypeInvariant,
              ExecuteCoreDelivery(command),
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \E envelope \in TimeoutEnvelopeSet:
                 DeliverTimeout(envelope)
    <2>1. AsyncItemTyped(command.item)
      BY <1>1
         DEF AsyncSchedulerTypeInvariant, AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant, ExecuteCoreDelivery
    <2>2. /\ command.item.kind = "TimeoutVote"
           /\ DeliverTimeout(command.item.envelope)
      BY <1>1, SMT
         DEF ExecuteCoreDelivery, DeliverProposal, DeliverVote, DeliverQC,
             DeliverTC
    <2>3. command.item.envelope \in TimeoutEnvelopeSet
      BY <2>1, <2>2, SMT DEF AsyncItemTyped
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM RegularCoreCommandKeepsTimeoutPool ==
  \A command:
    RegularCoreCommand(command)
      => receivedTimeoutVotes' = receivedTimeoutVotes
PROOF
  <1>1. ASSUME NEW command, RegularCoreCommand(command)
         PROVE receivedTimeoutVotes' = receivedTimeoutVotes
    <2>1. CASE command.kind = "AssembleBody"
                /\ AssembleLocalBody(command.node, command.subject)
      BY <2>1, Isa DEF AssembleLocalBody
    <2>2. CASE command.kind = "BeginProposal"
                /\ BeginLocalProposal(command.node, command.subject)
      BY <2>2, Isa DEF BeginLocalProposal
    <2>3. CASE command.kind = "PersistProposal"
                /\ \E request \in pendingProposal:
                     /\ CommandMatches(
                          command, request.node, request.proposal.view,
                          request.proposal.subject)
                     /\ PersistProposal(request)
      BY <2>3, Isa DEF PersistProposal
    <2>4. CASE \/ /\ command.kind = "FetchBody"
                         /\ HeldChunksFor(command.node, command.view,
                                           command.subject) = AsyncChunks
                         /\ ~BodyHeldBy(
                               durableBodies, command.node, context,
                               command.subject)
                         /\ \E proposal \in SeenProposalValues:
                              /\ CommandMatches(
                                   command, command.node, proposal.view,
                                   proposal.subject)
                              /\ FetchBody(command.node, proposal)
                    \/ /\ command.kind = "RebindRetainedBody"
                         /\ BodyHeldBy(
                               durableBodies, command.node, context,
                               command.subject)
                         /\ \E proposal \in SeenProposalValues:
                              /\ CommandMatches(
                                   command, command.node, proposal.view,
                                   proposal.subject)
                              /\ FetchBody(command.node, proposal)
      BY <2>4, Isa DEF FetchBody
    <2>5. CASE command.kind = "StoreBody"
                /\ StoreBody(command.node, command.subject)
      BY <2>5, Isa DEF StoreBody
    <2>6. CASE command.kind = "ValidateBody"
                /\ \E proposal \in SeenProposalValues:
                     /\ CommandMatches(
                          command, command.node, proposal.view,
                          proposal.subject)
                     /\ ValidateBody(command.node, proposal)
      BY <2>6, Isa DEF ValidateBody
    <2>7. CASE command.kind = "BeginPrepare"
                /\ \E proposal \in SeenProposalValues:
                     /\ CommandMatches(
                          command, command.node, proposal.view,
                          proposal.subject)
                     /\ BeginPrepare(command.node, proposal)
      BY <2>7, Isa DEF BeginPrepare
    <2>8. CASE command.kind = "PersistPrepare"
                /\ \E request \in pendingPrepare:
                     /\ CommandMatches(
                          command, request.node, request.vote.view,
                          request.vote.subject)
                     /\ PersistPrepare(request)
      BY <2>8, Isa DEF PersistPrepare
    <2>9. CASE command.kind = "BeginObservePrepare"
                /\ \E qc \in ReceivedQcValues:
                     /\ CommandMatches(
                          command, command.node, qc.view, qc.subject)
                     /\ BeginObservePrepare(command.node, qc)
      BY <2>9, Isa DEF BeginObservePrepare
    <2>10. CASE command.kind = "PersistObservePrepare"
                 /\ \E request \in pendingObservePrepare:
                      /\ CommandMatches(
                           command, request.node, request.qc.view,
                           request.qc.subject)
                      /\ PersistObservePrepare(request)
      BY <2>10, Isa DEF PersistObservePrepare
    <2>11. CASE command.kind = "BeginLockCommit"
                 /\ \E qc \in ReceivedQcValues:
                      /\ CommandMatches(
                           command, command.node, qc.view, qc.subject)
                      /\ BeginLockCommit(command.node, qc)
      BY <2>11, Isa DEF BeginLockCommit
    <2>12. CASE command.kind = "PersistLockCommit"
                 /\ \E request \in pendingLockCommit:
                      /\ CommandMatches(
                           command, request.node, request.qc.view,
                           request.qc.subject)
                      /\ PersistLockCommit(request)
      BY <2>12, Isa DEF PersistLockCommit
    <2>13. CASE command.kind = "FormCommitQC"
                 /\ FormCommitQC(
                      command.node, command.view, command.subject)
      BY <2>13, Isa DEF FormCommitQC
    <2>14. CASE command.kind = "BeginDecision"
                 /\ \E qc \in ReceivedQcValues:
                      /\ CommandMatches(
                           command, command.node, qc.view, qc.subject)
                      /\ BeginDecision(command.node, qc)
      BY <2>14, Isa DEF BeginDecision
    <2>15. CASE command.kind = "PersistTimeout"
                 /\ \E request \in pendingTimeout:
                      /\ CommandMatches(
                           command, request.node, request.vote.view,
                           request.vote.highSubject)
                      /\ PersistTimeout(request)
      BY <2>15, Isa DEF PersistTimeout
    <2>16. CASE command.kind = "FormTC"
                 /\ FormTC(command.node, command.view)
      BY <2>16, Isa DEF FormTC
    <2>17. CASE command.kind = "BeginInstallTC"
                 /\ \E tc \in ReceivedTcValues:
                      /\ command.node = command.node
                      /\ command.view = tc.view
                      /\ BeginInstallTC(command.node, tc)
      BY <2>17, Isa DEF BeginInstallTC
    <2>18. CASE command.kind = "FetchCertifiedBody"
                 /\ command.item.kind = "CertifiedResponse"
                 /\ command.item.envelope.recipient = command.node
                 /\ command.item.envelope.view = command.view
                 /\ command.item.envelope.subject = command.subject
                 /\ \E qc \in DecisionQcValues:
                      /\ CommandMatches(
                           command, command.node, qc.view, qc.subject)
                      /\ command.item.source \in qc.signers
                      /\ FetchCertifiedBody(command.node, qc)
      BY <2>18, Isa DEF FetchCertifiedBody
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11, <2>12, <2>13,
                <2>14, <2>15, <2>16, <2>17, <2>18
         DEF RegularCoreCommand
  <1> QED BY <1>1

THEOREM ChangedExecuteCommandIsCoreTimeoutDelivery ==
  \A command:
    (ExecuteCommand(command)
      /\ receivedTimeoutVotes' # receivedTimeoutVotes)
      => ExecuteCoreDelivery(command)
PROOF
  <1>1. ASSUME NEW command,
              ExecuteCommand(command),
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE ExecuteCoreDelivery(command)
    <2>1. CASE ExecuteRegularCommand(command)
      BY <1>1, <2>1, RegularCoreCommandKeepsTimeoutPool, Isa
         DEF ExecuteRegularCommand
    <2>2. CASE ExecuteSignProposal(command)
      BY <1>1, <2>2, Isa
         DEF ExecuteSignProposal, CompleteProposalSignature
    <2>3. CASE ExecuteSignVote(command)
      BY <1>1, <2>3, Isa DEF ExecuteSignVote, CompleteVoteSignature
    <2>4. CASE ExecuteFormPrepareQC(command)
      BY <1>1, <2>4, Isa DEF ExecuteFormPrepareQC, FormPrepareQC
    <2>5. CASE ExecuteSignTimeout(command)
      BY <1>1, <2>5, Isa
         DEF ExecuteSignTimeout, CompleteTimeoutSignature
    <2>6. CASE ExecutePersistInstall(command)
      BY <1>1, <2>6, Isa DEF ExecutePersistInstall, PersistInstallTC
    <2>7. CASE ExecutePersistDecision(command)
      BY <1>1, <2>7, Isa DEF ExecutePersistDecision, PersistDecision
    <2>8. CASE ExecuteRequestCertifiedBody(command)
      BY <1>1, <2>8, Isa DEF ExecuteRequestCertifiedBody, vars
    <2>9. CASE ExecuteApply(command)
      BY <1>1, <2>9, Isa DEF ExecuteApply, ApplyDecision
    <2>10. CASE ExecuteCoreDelivery(command)
      BY <2>10
    <2>11. CASE ExecuteChunkDelivery(command)
      BY <1>1, <2>11, Isa DEF ExecuteChunkDelivery, vars
    <2>12. CASE ExecuteRejectAuthenticatedJunk(command)
      BY <1>1, <2>12, Isa DEF ExecuteRejectAuthenticatedJunk, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11, <2>12
         DEF ExecuteCommand
  <1> QED BY <1>1

THEOREM ChangedFifoRuntimeExecutesCommand ==
  \A node:
    (FifoRuntimeStep(node)
      /\ receivedTimeoutVotes' # receivedTimeoutVotes)
      => ExecuteCommand(NextNodeCommand(node))
BY SMTT(30)
   DEF FifoRuntimeStep, DeferCommand, DiscardCommand, vars

THEOREM ChangedDeferredDrainExecutesCommand ==
  \A node:
    (DeferredDrainStep(node)
      /\ receivedTimeoutVotes' # receivedTimeoutVotes)
      => ExecuteCommand(NextDeferredCommand(node))
BY SMTT(30)
   DEF DeferredDrainStep, DiscardCommand, vars

THEOREM ChangedRuntimeStepExecutesCommand ==
  \A node:
    (RuntimeStep(node)
      /\ receivedTimeoutVotes' # receivedTimeoutVotes)
      => \E command: ExecuteCommand(command)
PROOF
  <1>1. ASSUME NEW node,
              RuntimeStep(node),
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \E command: ExecuteCommand(command)
    <2>1. CASE DirectCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>1, Isa
         DEF DirectCommitCertificateDiscoveryStep, vars
    <2>2. CASE DeferredDrainStep(node)
      <3>1. ExecuteCommand(NextDeferredCommand(node))
        BY <1>1, <2>2, ChangedDeferredDrainExecutesCommand
      <3> QED BY <3>1
    <2>3. CASE DeferredTagStep(node)
      BY <1>1, <2>3, Isa
         DEF DeferredTagStep, DeferredTimeoutStep,
             DeferredRetransmitStep, BeginTimeout, vars
    <2>4. CASE DirectTimeoutStep(node)
      BY <1>1, <2>4, Isa DEF DirectTimeoutStep, BeginTimeout, vars
    <2>5. CASE FifoRuntimeStep(node)
      <3>1. ExecuteCommand(NextNodeCommand(node))
        BY <1>1, <2>5, ChangedFifoRuntimeExecutesCommand
      <3> QED BY <3>1
    <2>6. CASE DirectRetransmitStep(node)
      BY <1>1, <2>6, Isa DEF DirectRetransmitStep, vars
    <2>7. CASE IdleRuntimeStep(node)
      BY <1>1, <2>7, Isa DEF IdleRuntimeStep, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7
         DEF RuntimeStep
  <1> QED BY <1>1

THEOREM ChangedRunNodeExecutesCommand ==
  \A node:
    (RunNode(node)
      /\ receivedTimeoutVotes' # receivedTimeoutVotes)
      => \E command: ExecuteCommand(command)
PROOF
  <1>1. ASSUME NEW node,
              RunNode(node),
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \E command: ExecuteCommand(command)
    <2>1. CASE LocalAdmissionStep(node)
      BY <1>1, <2>1, Isa
         DEF LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
             vars
    <2>2. CASE IngressDrainStep(node)
      BY <1>1, <2>2, Isa
         DEF IngressDrainStep, DrainFairIngressSelected, vars
    <2>3. CASE SerializedRuntimeStep(node)
      <3>1. RuntimeStep(node)
        BY <2>3 DEF SerializedRuntimeStep
      <3>2. \E command: ExecuteCommand(command)
        BY <1>1, <3>1, ChangedRuntimeStepExecutesCommand
      <3> QED BY <3>2
    <2> QED BY <1>1, <2>1, <2>2, <2>3 DEF RunNode
  <1> QED BY <1>1

THEOREM AsyncFaultStepKeepsTimeoutPool ==
  AsyncFaultStep => receivedTimeoutVotes' = receivedTimeoutVotes
PROOF
  <1>1. ASSUME AsyncFaultStep
         PROVE receivedTimeoutVotes' = receivedTimeoutVotes
    <2>1. CASE \E packet \in asyncTransport: PreGstLosePacket(packet)
      BY <2>1, Isa DEF PreGstLosePacket, vars
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <2>2, Isa DEF PreGstCrash, Crash
    <2>3. CASE \E source \in AsyncIngressSources,
                  recipient \in ValidatorIds,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectByzantineNoise(source, recipient, nonce)
      BY <2>3, Isa DEF InjectByzantineNoise, vars
    <2>4. CASE \E kind \in {"NormalJunk", "ProgressJunk"},
                  source \in ValidatorIds, recipient \in ValidatorIds,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectAuthenticatedJunk(kind, source, recipient, nonce)
      BY <2>4, Isa DEF InjectAuthenticatedJunk, vars
    <2>5. CASE \E source \in ValidatorIds, recipient \in ValidatorIds,
                  qc \in commitQCs,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectByzantineCertifiedRequest(
                    source, recipient, qc, nonce)
      BY <2>5, Isa DEF InjectByzantineCertifiedRequest, vars
    <2>6. CASE \E signer \in ValidatorIds, roundView \in Views,
                  subject \in Subjects, justifyRank \in Ranks,
                  justifySubject \in SubjectOrNone:
                  AsyncByzantineProposal(
                    signer, roundView, subject, justifyRank, justifySubject)
      BY <2>6, Isa
         DEF AsyncByzantineProposal, ByzantineBroadcastProposal
    <2>7. CASE \E signer \in ValidatorIds, roundView \in Views,
                  phase \in Phases, subject \in Subjects:
                  AsyncByzantineVote(signer, roundView, phase, subject)
      BY <2>7, Isa DEF AsyncByzantineVote, ByzantineBroadcastVote
    <2>8. CASE \E signer \in ValidatorIds, roundView \in Views,
                  highRank \in Ranks, highSubject \in SubjectOrNone:
                  AsyncByzantineTimeout(
                    signer, roundView, highRank, highSubject)
      BY <2>8, Isa
         DEF AsyncByzantineTimeout, ByzantineBroadcastTimeout
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
                <2>8
         DEF AsyncFaultStep
  <1> QED BY <1>1

THEOREM AsyncNonRunnerStepKeepsTimeoutPool ==
  AsyncNonRunnerStep => receivedTimeoutVotes' = receivedTimeoutVotes
PROOF
  <1>1. ASSUME AsyncNonRunnerStep
         PROVE receivedTimeoutVotes' = receivedTimeoutVotes
    <2>1. CASE AsyncSetGST
      BY <2>1, Isa DEF AsyncSetGST, SetGST
    <2>2. CASE AsyncTick
      BY <2>2, Isa DEF AsyncTick, AsyncNonClockVars, vars
    <2>3. CASE \E node \in AsyncCurrentResponsiveVoters:
                  ServiceIoWorker(node)
      BY <2>3, Isa DEF ServiceIoWorker, vars
    <2>4. CASE \E node \in AsyncCurrentResponsiveVoters:
                  EnqueueIoLocalControl(node)
      BY <2>4, Isa DEF EnqueueIoLocalControl, vars
    <2>5. CASE AsyncNetworkStep
      BY <2>5, Isa DEF AsyncNetworkStep, AdmitHiddenPacket, vars
    <2>6. CASE AsyncFaultStep
      BY <2>6, AsyncFaultStepKeepsTimeoutPool
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
         DEF AsyncNonRunnerStep
  <1> QED BY <1>1

THEOREM ChangedAsyncRunnerExecutesCommand ==
  (AsyncRunnerStep
    /\ receivedTimeoutVotes' # receivedTimeoutVotes)
    => \E command: ExecuteCommand(command)
PROOF
  <1>1. ASSUME AsyncRunnerStep,
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \E command: ExecuteCommand(command)
    <2>1. CASE \E node \in AsyncCurrentResponsiveVoters: RunNode(node)
      <3>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                    RunNode(node)
             PROVE \E command: ExecuteCommand(command)
        BY <1>1, <3>1, ChangedRunNodeExecutesCommand
      <3> QED BY <2>1, <3>1
    <2>2. CASE \E node \in AsyncCurrentResponsiveVoters:
                  RunHistoricalServer(node)
      BY <1>1, <2>2, Isa
         DEF RunHistoricalServer, DrainHistoricalIngressSelected,
             HistoricalIdleStep, vars
    <2> QED BY <1>1, <2>1, <2>2 DEF AsyncRunnerStep
  <1> QED BY <1>1

THEOREM ChangedAsyncNextExecutesCommand ==
  (AsyncNext
    /\ receivedTimeoutVotes' # receivedTimeoutVotes)
    => \E command: ExecuteCommand(command)
PROOF
  <1>1. ASSUME AsyncNext,
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \E command: ExecuteCommand(command)
    <2>1. CASE AsyncNonCrashStep
      <3>1. CASE AsyncRunnerStep
        BY <1>1, <3>1, ChangedAsyncRunnerExecutesCommand
      <3>2. CASE AsyncNonRunnerStep
        BY <1>1, <3>2, AsyncNonRunnerStepKeepsTimeoutPool
      <3> QED BY <2>1, <3>1, <3>2 DEF AsyncNonCrashStep
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <1>1, <2>2, Isa DEF PreGstCrash, Crash
    <2> QED BY <1>1, <2>1, <2>2 DEF AsyncNext
  <1> QED BY <1>1

THEOREM ChangedTypedAsyncNextIsTimeoutDelivery ==
  (AsyncTypeInvariant
    /\ AsyncNext
    /\ receivedTimeoutVotes' # receivedTimeoutVotes)
    => \E envelope \in TimeoutEnvelopeSet: DeliverTimeout(envelope)
PROOF
  <1>1. ASSUME AsyncTypeInvariant,
              AsyncNext,
              receivedTimeoutVotes' # receivedTimeoutVotes
         PROVE \E envelope \in TimeoutEnvelopeSet: DeliverTimeout(envelope)
    <2>1. \E command: ExecuteCommand(command)
      BY <1>1, ChangedAsyncNextExecutesCommand
    <2>2. ASSUME NEW command, ExecuteCommand(command)
           PROVE \E envelope \in TimeoutEnvelopeSet:
                   DeliverTimeout(envelope)
      <3>1. ExecuteCoreDelivery(command)
        BY <1>1, <2>2, ChangedExecuteCommandIsCoreTimeoutDelivery
      <3>2. AsyncSchedulerTypeInvariant
        BY <1>1 DEF AsyncTypeInvariant
      <3> QED BY <1>1, <3>1, <3>2,
                    ChangedCoreTimeoutDeliveryIsTyped
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncNextPreservesTimeoutPoolInvariant ==
  AsyncTypeInvariant /\ AsyncNext
    => ReceivedTimeoutVotePoolInvariant'
PROOF
  <1>1. ASSUME AsyncTypeInvariant, AsyncNext
         PROVE ReceivedTimeoutVotePoolInvariant'
    <2>1. ReceivedTimeoutVotePoolInvariant
      BY <1>1 DEF AsyncTypeInvariant
    <2>2. /\ context' = context
           /\ height' = height
      BY <1>1 DEF AsyncNext
    <2>3. [Next]_vars
      BY <1>1 DEF AsyncNext
    <2>4. prepareQCs \subseteq prepareQCs'
      <3>1. CASE Next
        BY <3>1, CoreNextKeepsPrepareQcsMonotone
      <3>2. CASE UNCHANGED vars
        BY <3>2, Isa DEF vars
      <3> QED BY <2>3, <3>1, <3>2
    <2>5. CASE receivedTimeoutVotes' = receivedTimeoutVotes
      BY <2>1, <2>2, <2>4, <2>5,
         TimeoutPoolFramePreservesInvariant
    <2>6. CASE receivedTimeoutVotes' # receivedTimeoutVotes
      <3>1. \E envelope \in TimeoutEnvelopeSet:
               DeliverTimeout(envelope)
        BY <1>1, <2>6, ChangedTypedAsyncNextIsTimeoutDelivery
      <3>2. ASSUME NEW envelope \in TimeoutEnvelopeSet,
                    DeliverTimeout(envelope)
             PROVE ReceivedTimeoutVotePoolInvariant'
        BY <2>1, <3>2, TypedDeliverTimeoutPreservesPoolInvariant
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM DualQuorumMonotoneWithinRoster ==
  \A epoch \in Epochs:
    \A left, right \in SUBSET VotingRoster(epoch):
      (QuorumConfiguration
        /\ left \subseteq right
        /\ DualQuorum(epoch, left))
        => DualQuorum(epoch, right)
PROOF
  <1>1. ASSUME NEW epoch \in Epochs,
              NEW left \in SUBSET VotingRoster(epoch),
              NEW right \in SUBSET VotingRoster(epoch),
              QuorumConfiguration,
              left \subseteq right,
              DualQuorum(epoch, left)
         PROVE DualQuorum(epoch, right)
    <2>1. /\ IsFiniteSet(VotingRoster(epoch))
           /\ left \subseteq VotingRoster(epoch)
           /\ right \subseteq VotingRoster(epoch)
      BY <1>1 DEF QuorumConfiguration
    <2>2. /\ IsFiniteSet(left)
           /\ IsFiniteSet(right)
      BY <2>1, FS_Subset
    <2>3. /\ Cardinality(left) \in Nat
           /\ Cardinality(right) \in Nat
           /\ Cardinality(VotingRoster(epoch)) \in Nat
           /\ Cardinality(left) <= Cardinality(right)
      BY <1>1, <2>1, <2>2, FS_CardinalityType, FS_Subset
    <2>4. CountQuorum(epoch, right)
      BY <1>1, <2>1, <2>3, SMT
         DEF DualQuorum, CountQuorum
    <2>5. /\ VotingRoster(epoch) \in SUBSET ValidatorIds
           /\ left \in SUBSET ValidatorIds
           /\ right \in SUBSET ValidatorIds
      BY <1>1, <2>1, Isa DEF QuorumConfiguration, VotingRoster
    <2>6. PowerUnits(epoch, left)
             \subseteq PowerUnits(epoch, right)
      BY <1>1, <2>5, PowerUnitsMonotone
    <2>7. /\ IsFiniteSet(PowerUnits(epoch, left))
           /\ IsFiniteSet(PowerUnits(epoch, right))
           /\ IsFiniteSet(PowerUnits(epoch, VotingRoster(epoch)))
      BY <1>1, PowerUnitsFinite
    <2>8. /\ Cardinality(PowerUnits(epoch, left)) \in Nat
           /\ Cardinality(PowerUnits(epoch, right)) \in Nat
           /\ Cardinality(PowerUnits(epoch, VotingRoster(epoch))) \in Nat
           /\ Cardinality(PowerUnits(epoch, left))
                <= Cardinality(PowerUnits(epoch, right))
      BY <2>6, <2>7, FS_CardinalityType, FS_Subset
    <2>9. PowerQuorum(epoch, right)
      BY <1>1, <2>1, <2>8, SMT
         DEF DualQuorum, PowerQuorum, PowerOf
    <2> QED BY <2>4, <2>9 DEF DualQuorum
  <1> QED BY <1>1

THEOREM ResponsiveReceiptsCoverResponsiveSigners ==
  \A recipient \in ValidatorIds, roundView \in Views:
    ResponsiveTimeoutReceiptQuorumAt(recipient, roundView)
      => AsyncCurrentResponsiveVoters
           \subseteq TimeoutSignerSet(
             TimeoutVotesAt(recipient, roundView))
BY SMTT(30)
   DEF ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       AsyncCurrentResponsiveVoters, TimeoutSignerSet, TimeoutVotesAt

THEOREM TimeoutPoolSignersStayInCurrentRoster ==
  \A recipient \in ValidatorIds, roundView \in Views:
    ReceivedTimeoutVotePoolInvariant
      => TimeoutSignerSet(TimeoutVotesAt(recipient, roundView))
           \subseteq CurrentVoters
BY SMTT(30)
   DEF ReceivedTimeoutVotePoolInvariant,
       TimeoutSignerSet, TimeoutVotesAt

THEOREM TypeInvariantMakesCurrentEpochTyped ==
  TypeInvariant => CurrentEpoch \in Epochs
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE CurrentEpoch \in Epochs
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
    <2>4. /\ context.epoch = ExpectedEpoch(blockHeight)
           /\ MaxEpoch >= ExpectedEpoch(MaxHeight)
      BY <2>1, <2>3
         DEF ContextRecord, ModelConfiguration
    <2>5. /\ blockHeight \in Nat
           /\ blockHeight <= MaxHeight
           /\ MaxHeight \in Nat
           /\ EpochLength \in Nat \ {0}
           /\ MaxEpoch \in Nat
      BY <2>1, <2>2, SMT
         DEF Heights, ModelConfiguration, QuorumConfiguration
    <2>6. ExpectedEpoch(blockHeight) \in 0..MaxEpoch
      BY <2>4, <2>5, BoundedNaturalQuotient DEF ExpectedEpoch
    <2> QED BY <2>4, <2>6 DEF CurrentEpoch, Epochs
  <1> QED BY <1>1

THEOREM ResponsiveReceiptsMakeDualQuorum ==
  \A recipient \in ValidatorIds, roundView \in Views:
    (TypeInvariant
      /\ ReceivedTimeoutVotePoolInvariant
      /\ ResponsiveTimeoutReceiptQuorumAt(recipient, roundView))
      => DualQuorum(
           CurrentEpoch,
           TimeoutSignerSet(TimeoutVotesAt(recipient, roundView)))
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
              NEW roundView \in Views,
              TypeInvariant,
              ReceivedTimeoutVotePoolInvariant,
              ResponsiveTimeoutReceiptQuorumAt(recipient, roundView)
         PROVE DualQuorum(
                 CurrentEpoch,
                 TimeoutSignerSet(TimeoutVotesAt(recipient, roundView)))
    <2> DEFINE Signers ==
          TimeoutSignerSet(TimeoutVotesAt(recipient, roundView))
    <2>1. /\ ModelConfiguration
           /\ QuorumConfiguration
      BY <1>1 DEF TypeInvariant, ModelConfiguration
    <2>2. CurrentEpoch \in Epochs
      BY <1>1, TypeInvariantMakesCurrentEpochTyped
    <2>3. /\ CurrentVoters = VotingRoster(CurrentEpoch)
           /\ DualQuorum(CurrentEpoch, AsyncCurrentResponsiveVoters)
      BY <2>1, <2>2, Isa
         DEF ModelConfiguration, CurrentVoters,
             AsyncCurrentResponsiveVoters
    <2>4. AsyncCurrentResponsiveVoters \subseteq Signers
      BY <1>1, ResponsiveReceiptsCoverResponsiveSigners DEF Signers
    <2>5. Signers \subseteq CurrentVoters
      BY <1>1, TimeoutPoolSignersStayInCurrentRoster DEF Signers
    <2>6. /\ AsyncCurrentResponsiveVoters
                  \in SUBSET VotingRoster(CurrentEpoch)
           /\ Signers \in SUBSET VotingRoster(CurrentEpoch)
      BY <2>3, <2>5, Isa
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>6,
                  DualQuorumMonotoneWithinRoster
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesTimeoutPoolInvariant ==
  \A initialContext:
    AsyncInitAt(initialContext) => ReceivedTimeoutVotePoolInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE ReceivedTimeoutVotePoolInvariant
    <2>1. receivedTimeoutVotes = {}
      BY <1>1 DEF AsyncInitAt, AsyncBaseInitAt, InitAt
    <2>2. IsFiniteSet(receivedTimeoutVotes)
      BY <2>1, FS_EmptySet
    <2>3. ReceivedTimeoutVoteSlotsUnique
      BY <2>1 DEF ReceivedTimeoutVoteSlotsUnique
    <2>4. \A received \in receivedTimeoutVotes:
             /\ received.node \in ValidatorIds
             /\ received.vote \in TimeoutVoteRecordSet
             /\ received.vote.context = context
             /\ received.vote.height = height
             /\ received.vote.signer \in CurrentVoters
             /\ AuthenticatedHighRef(
                  received.vote.highRank,
                  received.vote.highSubject)
             /\ received.vote.highRank <= received.vote.view
      BY <2>1
    <2> QED BY <2>2, <2>3, <2>4
       DEF ReceivedTimeoutVotePoolInvariant
  <1> QED BY <1>1

(***************************************************************************
Non-runner scheduler closure.  Fault injections may extend only the
transport-history slice; this projection lemma reuses the primitive stutter
proofs for every other scheduler component.
***************************************************************************)

THEOREM AsyncTransportContentChangePreservesSchedulerType ==
  /\ AsyncSchedulerTypeInvariant
  /\ AsyncTransportContentTypeInvariant'
  /\ UNCHANGED AsyncRuntimeScalarTypeVars
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
  => AsyncSchedulerTypeInvariant'
BY AsyncRuntimeScalarTypeStutter, AsyncCausalTypeStutter,
   AsyncIoTopologyTypeStutter, AsyncIoContentTypeStutter,
   AsyncIoCapacityTypeStutter, AsyncDeferredTopologyTypeStutter,
   AsyncDeferredContentTypeStutter, AsyncTransportClockTypeStutter,
   AsyncIngressTopologyTypeStutter, AsyncIngressCapacityTypeStutter,
   AsyncIngressContentTypeStutter
   DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
       AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
       AsyncTransportTypeInvariant, AsyncIngressTypeInvariant

THEOREM AddTypedPacketPreservesPacketContentType ==
  \A packet:
    /\ AsyncPacketContentTypeInvariant
    /\ AsyncPacketTyped(packet)
    /\ asyncTransport' = asyncTransport \cup {packet}
    => AsyncPacketContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW packet,
                AsyncPacketContentTypeInvariant,
                AsyncPacketTyped(packet),
                asyncTransport' = asyncTransport \cup {packet}
         PROVE AsyncPacketContentTypeInvariant'
    <2>1. IsFiniteSet(asyncTransport')
      BY <1>1, FS_AddElement DEF AsyncPacketContentTypeInvariant
    <2>2. \A queued \in asyncTransport': AsyncPacketTyped(queued)
      BY <1>1 DEF AsyncPacketContentTypeInvariant
    <2> QED BY <2>1, <2>2 DEF AsyncPacketContentTypeInvariant
  <1> QED BY <1>1

THEOREM AddUntrackedTypedPacketPreservesTransportContentType ==
  \A packet:
    /\ AsyncTransportContentTypeInvariant
    /\ AsyncPacketTyped(packet)
    /\ asyncTransport' = asyncTransport \cup {packet}
    /\ UNCHANGED AsyncTransportHistoryTypeVars
    /\ UNCHANGED asyncHeldChunks
    => AsyncTransportContentTypeInvariant'
BY AddTypedPacketPreservesPacketContentType,
   AsyncTransportHistoryTypeStutter, AsyncHeldChunksTypeStutter
   DEF AsyncTransportContentTypeInvariant

THEOREM AsyncHeartbeatSubjectIsValid ==
  ModelConfiguration => AsyncHeartbeatSubject \in ValidSubjects
BY SMT DEF ModelConfiguration, AsyncHeartbeatSubject

THEOREM AsyncNoiseItemIsTyped ==
  \A source \in AsyncIngressSources, recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    LET envelope ==
          AsyncBodyEnvelope(recipient, context.height,
                            nodeView[recipient],
                            AsyncHeartbeatSubject, NoAsyncChunk, nonce)
        item == AsyncNetworkItem("Noise", source, envelope)
    IN /\ TypeInvariant
       /\ AsyncConfiguration
       => AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW source \in AsyncIngressSources,
                NEW recipient \in ValidatorIds,
                NEW nonce \in 0..(AsyncIngressCapacity - 1)
         PROVE LET envelope ==
                 AsyncBodyEnvelope(recipient, context.height,
                                   nodeView[recipient],
                                   AsyncHeartbeatSubject,
                                   NoAsyncChunk, nonce)
               item == AsyncNetworkItem("Noise", source, envelope)
               IN /\ TypeInvariant
                  /\ AsyncConfiguration
                  => AsyncItemTyped(item)
    <2>1. ASSUME TypeInvariant, AsyncConfiguration
           PROVE LET envelope ==
                   AsyncBodyEnvelope(recipient, context.height,
                                     nodeView[recipient],
                                     AsyncHeartbeatSubject,
                                     NoAsyncChunk, nonce)
                 item == AsyncNetworkItem("Noise", source, envelope)
                 IN AsyncItemTyped(item)
      <3>1. /\ ModelConfiguration
             /\ context.height \in Heights
             /\ nodeView[recipient] \in Views
             /\ AsyncHeartbeatSubject \in ValidSubjects
        BY <1>1, <2>1, AsyncHeartbeatSubjectIsValid
           DEF TypeInvariant
      <3> QED BY <1>1, <2>1, <3>1, SMT
           DEF AsyncItemTyped, AsyncNetworkItem,
               AsyncBodyEnvelopeTyped, AsyncBodyEnvelope,
               AsyncNetworkKinds, AsyncIngressSources,
               AsyncConfiguration, NoAsyncChunk
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM InjectedByzantineNoisePacketIsTyped ==
  \A source \in AsyncIngressSources, recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    LET envelope ==
          AsyncBodyEnvelope(recipient, context.height,
                            nodeView[recipient],
                            AsyncHeartbeatSubject, NoAsyncChunk, nonce)
        item == AsyncNetworkItem("Noise", source, envelope)
        packet ==
          AsyncPacket(item, asyncNow, asyncNow + AsyncDeliveryBound)
    IN /\ AsyncTypeInvariant
       /\ InjectByzantineNoise(source, recipient, nonce)
       => AsyncPacketTyped(packet)
BY AsyncNoiseItemIsTyped, PacketForTypedItemIsTyped
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant

THEOREM InjectByzantineNoisePreservesSchedulerType ==
  \A source \in AsyncIngressSources, recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    AsyncTypeInvariant
      /\ InjectByzantineNoise(source, recipient, nonce)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW source \in AsyncIngressSources,
                NEW recipient \in ValidatorIds,
                NEW nonce \in 0..(AsyncIngressCapacity - 1),
                AsyncTypeInvariant,
                InjectByzantineNoise(source, recipient, nonce)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE Envelope ==
          AsyncBodyEnvelope(recipient, context.height,
                            nodeView[recipient],
                            AsyncHeartbeatSubject, NoAsyncChunk, nonce)
    <2> DEFINE Item == AsyncNetworkItem("Noise", source, Envelope)
    <2> DEFINE Packet ==
          AsyncPacket(Item, asyncNow, asyncNow + AsyncDeliveryBound)
    <2>1. AsyncPacketTyped(Packet)
      BY <1>1, InjectedByzantineNoisePacketIsTyped
         DEF Envelope, Item, Packet
    <2>2. /\ AsyncTransportContentTypeInvariant
           /\ asyncTransport' = asyncTransport \cup {Packet}
           /\ UNCHANGED AsyncTransportHistoryTypeVars
           /\ UNCHANGED asyncHeldChunks
      BY <1>1, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant, InjectByzantineNoise,
             AsyncTransportHistoryTypeVars, Envelope, Item, Packet,
             LeaveCausalQueues, AsyncSchedulerVars, vars
    <2>3. AsyncTransportContentTypeInvariant'
      BY <2>1, <2>2,
         AddUntrackedTypedPacketPreservesTransportContentType
    <2>4. /\ UNCHANGED AsyncRuntimeScalarTypeVars
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
      BY <1>1, Isa
         DEF InjectByzantineNoise, LeaveCausalQueues,
             AsyncRuntimeScalarTypeVars, AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2> QED BY <1>1, <2>3, <2>4,
                 AsyncTransportContentChangePreservesSchedulerType
         DEF AsyncTypeInvariant
  <1> QED BY <1>1

THEOREM PublishTypedSingletonPreservesTransportContentType ==
  \A item:
    /\ AsyncTypeInvariant
    /\ AsyncItemTyped(item)
    /\ PublishEphemeralItems({item})
    /\ UNCHANGED <<context, asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW item,
                AsyncTypeInvariant,
                AsyncItemTyped(item),
                PublishEphemeralItems({item}),
                UNCHANGED <<context, asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncTransportContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncTransportTypeInvariant
    <2>2. /\ IsFiniteSet({item})
           /\ \A queued \in {item}: AsyncItemTyped(queued)
      BY <1>1, FS_Singleton
    <2> QED BY <1>1, <2>1, <2>2,
                 PublishEphemeralItemsPreservesTransportContentType
  <1> QED BY <1>1

THEOREM PublishTypedSingletonPreservesSchedulerType ==
  \A item:
    /\ AsyncTypeInvariant
    /\ AsyncItemTyped(item)
    /\ PublishEphemeralItems({item})
    /\ UNCHANGED <<context, asyncHeldChunks>>
    /\ UNCHANGED AsyncRuntimeScalarTypeVars
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
    => AsyncSchedulerTypeInvariant'
BY PublishTypedSingletonPreservesTransportContentType,
   AsyncTransportContentChangePreservesSchedulerType
   DEF AsyncTypeInvariant

THEOREM PublishTypedItemsPreservesSchedulerType ==
  \A items:
    /\ AsyncTypeInvariant
    /\ IsFiniteSet(items)
    /\ \A item \in items: AsyncItemTyped(item)
    /\ PublishEphemeralItems(items)
    /\ UNCHANGED <<context, asyncHeldChunks>>
    /\ UNCHANGED AsyncRuntimeScalarTypeVars
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
    => AsyncSchedulerTypeInvariant'
BY PublishEphemeralItemsPreservesTransportContentType,
   AsyncTransportContentChangePreservesSchedulerType
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncTransportTypeInvariant

THEOREM CurrentVotersAreFinite ==
  TypeInvariant => IsFiniteSet(CurrentVoters)
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE IsFiniteSet(CurrentVoters)
    <2>1. /\ QuorumConfiguration
           /\ CurrentEpoch \in Epochs
      BY <1>1, TypeInvariantMakesCurrentEpochTyped
         DEF TypeInvariant, ModelConfiguration
    <2>2. IsFiniteSet(VotingRoster(CurrentEpoch))
      BY <2>1 DEF QuorumConfiguration
    <2> QED BY <2>2 DEF CurrentVoters
  <1> QED BY <1>1

THEOREM CurrentVotersAreFiniteValidators ==
  TypeInvariant
    => /\ IsFiniteSet(CurrentVoters)
       /\ CurrentVoters \subseteq ValidatorIds
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE /\ IsFiniteSet(CurrentVoters)
               /\ CurrentVoters \subseteq ValidatorIds
    <2>1. IsFiniteSet(CurrentVoters)
      BY <1>1, CurrentVotersAreFinite
    <2>2. /\ QuorumConfiguration
           /\ CurrentEpoch \in Epochs
      BY <1>1, TypeInvariantMakesCurrentEpochTyped
         DEF TypeInvariant, ModelConfiguration
    <2>3. VotingRoster(CurrentEpoch) \subseteq ValidatorIds
      BY <2>2 DEF QuorumConfiguration, VotingRoster
    <2> QED BY <2>1, <2>3 DEF CurrentVoters
  <1> QED BY <1>1

THEOREM ProposalEnvelopeMakesTypedAsyncItem ==
  \A source \in ValidatorIds, envelope \in ProposalEnvelopeSet:
    AsyncItemTyped(AsyncNetworkItem("Proposal", source, envelope))
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW envelope \in ProposalEnvelopeSet
         PROVE AsyncItemTyped(
                 AsyncNetworkItem("Proposal", source, envelope))
    <2>1. /\ DOMAIN AsyncNetworkItem("Proposal", source, envelope) =
                 {"kind", "source", "envelope"}
           /\ AsyncNetworkItem("Proposal", source, envelope).kind =
                "Proposal"
           /\ AsyncNetworkItem("Proposal", source, envelope).source =
                source
           /\ AsyncNetworkItem("Proposal", source, envelope).envelope =
                envelope
      BY DEF AsyncNetworkItem
    <2>2. /\ "Proposal" \in AsyncNetworkKinds
           /\ source \in AsyncIngressSources
           /\ envelope.recipient \in ValidatorIds
      BY <1>1, SMT
         DEF AsyncNetworkKinds, AsyncIngressSources, ProposalEnvelopeSet
    <2> QED BY <1>1, <2>1, <2>2, SMT DEF AsyncItemTyped
  <1> QED BY <1>1

THEOREM VoteEnvelopeMakesTypedAsyncItem ==
  \A source \in ValidatorIds, envelope \in VoteEnvelopeSet:
    AsyncItemTyped(
      AsyncNetworkItem(
        IF envelope.vote.phase = "Prepare"
        THEN "PrepareVote" ELSE "CommitVote",
        source, envelope))
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW envelope \in VoteEnvelopeSet
         PROVE AsyncItemTyped(
                 AsyncNetworkItem(
                   IF envelope.vote.phase = "Prepare"
                   THEN "PrepareVote" ELSE "CommitVote",
                   source, envelope))
    <2>1. /\ envelope.recipient \in ValidatorIds
           /\ envelope.vote.phase \in Phases
           /\ source \in AsyncIngressSources
      BY <1>1, SMT
         DEF VoteEnvelopeSet, VoteRecordSet, AsyncIngressSources
    <2>2. CASE envelope.vote.phase = "Prepare"
      BY <1>1, <2>1, <2>2, SMT
         DEF AsyncItemTyped, AsyncNetworkItem, AsyncNetworkKinds
    <2>3. CASE envelope.vote.phase # "Prepare"
      BY <1>1, <2>1, <2>3, SMT
         DEF AsyncItemTyped, AsyncNetworkItem,
             AsyncNetworkKinds, Phases
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM TimeoutEnvelopeMakesTypedAsyncItem ==
  \A source \in ValidatorIds, envelope \in TimeoutEnvelopeSet:
    AsyncItemTyped(AsyncNetworkItem("TimeoutVote", source, envelope))
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW envelope \in TimeoutEnvelopeSet
         PROVE AsyncItemTyped(
                 AsyncNetworkItem("TimeoutVote", source, envelope))
    <2>1. /\ DOMAIN AsyncNetworkItem("TimeoutVote", source, envelope) =
                 {"kind", "source", "envelope"}
           /\ AsyncNetworkItem("TimeoutVote", source, envelope).kind =
                "TimeoutVote"
           /\ AsyncNetworkItem("TimeoutVote", source, envelope).source =
                source
           /\ AsyncNetworkItem("TimeoutVote", source, envelope).envelope =
                envelope
      BY DEF AsyncNetworkItem
    <2>2. /\ "TimeoutVote" \in AsyncNetworkKinds
           /\ source \in AsyncIngressSources
           /\ envelope.recipient \in ValidatorIds
      BY <1>1, SMT
         DEF AsyncNetworkKinds, AsyncIngressSources, TimeoutEnvelopeSet
    <2> QED BY <1>1, <2>1, <2>2, SMT DEF AsyncItemTyped
  <1> QED BY <1>1

THEOREM ByzantineProposalItemIsTyped ==
  \A signer \in ValidatorIds, recipient \in ValidatorIds,
     roundView \in Views, subject \in Subjects,
     justifyRank \in Ranks, justifySubject \in SubjectOrNone:
    TypeInvariant
      => AsyncItemTyped(
           AsyncNetworkItem(
             "Proposal", signer,
             ProposalEnvelope(
               recipient,
               Proposal(context, roundView, subject, signer,
                        justifyRank, justifySubject))))
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW roundView \in Views,
                NEW subject \in Subjects,
                NEW justifyRank \in Ranks,
                NEW justifySubject \in SubjectOrNone,
                TypeInvariant
         PROVE AsyncItemTyped(
                 AsyncNetworkItem(
                   "Proposal", signer,
                   ProposalEnvelope(
                     recipient,
                     Proposal(context, roundView, subject, signer,
                              justifyRank, justifySubject))))
    <2>1. /\ context \in ContextRecords
           /\ context.height \in Heights
      BY <1>1 DEF TypeInvariant
    <2>2. Proposal(context, roundView, subject, signer,
                   justifyRank, justifySubject) \in ProposalRecordSet
      BY <1>1, <2>1, SMT DEF Proposal, ProposalRecordSet
    <2>3. ProposalEnvelope(
             recipient,
             Proposal(context, roundView, subject, signer,
                      justifyRank, justifySubject)) \in ProposalEnvelopeSet
      BY <1>1, <2>2, SMT DEF ProposalEnvelope, ProposalEnvelopeSet
    <2> QED BY <1>1, <2>3,
                 ProposalEnvelopeMakesTypedAsyncItem
  <1> QED BY <1>1

THEOREM ByzantineProposalOutboxIsFiniteAndTyped ==
  \A signer \in ValidatorIds, roundView \in Views,
     subject \in Subjects, justifyRank \in Ranks,
     justifySubject \in SubjectOrNone:
    LET proposal == Proposal(context, roundView, subject, signer,
                             justifyRank, justifySubject)
        items == ByzantineProposalOutbox(signer, proposal)
    IN TypeInvariant
         => /\ IsFiniteSet(items)
            /\ \A item \in items: AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW roundView \in Views,
                NEW subject \in Subjects,
                NEW justifyRank \in Ranks,
                NEW justifySubject \in SubjectOrNone
         PROVE LET proposal ==
                 Proposal(context, roundView, subject, signer,
                          justifyRank, justifySubject)
               items == ByzantineProposalOutbox(signer, proposal)
               IN TypeInvariant
                    => /\ IsFiniteSet(items)
                       /\ \A item \in items: AsyncItemTyped(item)
    <2>1. ASSUME TypeInvariant
           PROVE LET proposal ==
                   Proposal(context, roundView, subject, signer,
                            justifyRank, justifySubject)
                 items == ByzantineProposalOutbox(signer, proposal)
                 IN /\ IsFiniteSet(items)
                    /\ \A item \in items: AsyncItemTyped(item)
      <3>1. /\ IsFiniteSet(CurrentVoters)
             /\ CurrentVoters \subseteq ValidatorIds
             /\ context \in ContextRecords
             /\ context.height \in Heights
        BY <2>1, CurrentVotersAreFiniteValidators
           DEF TypeInvariant
      <3>2. IsFiniteSet(
               ByzantineProposalOutbox(
                 signer, Proposal(context, roundView, subject, signer,
                                  justifyRank, justifySubject)))
        BY <3>1, FS_Image DEF ByzantineProposalOutbox
      <3>3. \A item \in
                    ByzantineProposalOutbox(
                      signer,
                      Proposal(context, roundView, subject, signer,
                               justifyRank, justifySubject)):
               AsyncItemTyped(item)
        <4>1. ASSUME NEW item \in
                     ByzantineProposalOutbox(
                       signer,
                       Proposal(context, roundView, subject, signer,
                                justifyRank, justifySubject))
               PROVE AsyncItemTyped(item)
          <5>1. PICK recipient \in CurrentVoters:
                   item = AsyncNetworkItem(
                     "Proposal", signer,
                     ProposalEnvelope(
                       recipient,
                       Proposal(context, roundView, subject, signer,
                                justifyRank, justifySubject)))
            BY <4>1 DEF ByzantineProposalOutbox
          <5>2. recipient \in ValidatorIds
            BY <3>1, <5>1
          <5>3. AsyncItemTyped(
                   AsyncNetworkItem(
                     "Proposal", signer,
                     ProposalEnvelope(
                       recipient,
                       Proposal(context, roundView, subject, signer,
                                justifyRank, justifySubject))))
            BY <1>1, <2>1, <5>2, ByzantineProposalItemIsTyped
          <5> QED BY <5>1, <5>3
        <4> QED BY <4>1
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM AsyncByzantineProposalPreservesSchedulerType ==
  \A signer \in ValidatorIds, roundView \in Views,
     subject \in Subjects, justifyRank \in Ranks,
     justifySubject \in SubjectOrNone:
    AsyncTypeInvariant
      /\ AsyncByzantineProposal(
           signer, roundView, subject, justifyRank, justifySubject)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW roundView \in Views,
                NEW subject \in Subjects,
                NEW justifyRank \in Ranks,
                NEW justifySubject \in SubjectOrNone,
                AsyncTypeInvariant,
                AsyncByzantineProposal(
                  signer, roundView, subject, justifyRank, justifySubject)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE ProposalValue ==
          Proposal(context, roundView, subject, signer,
                   justifyRank, justifySubject)
    <2> DEFINE Items ==
          ByzantineProposalOutbox(signer, ProposalValue)
    <2>1. /\ IsFiniteSet(Items)
           /\ \A item \in Items: AsyncItemTyped(item)
      BY <1>1, ByzantineProposalOutboxIsFiniteAndTyped
         DEF AsyncTypeInvariant, ProposalValue, Items
    <2>2. /\ PublishEphemeralItems(Items)
           /\ UNCHANGED <<context, asyncHeldChunks>>
           /\ UNCHANGED AsyncRuntimeScalarTypeVars
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
      BY <1>1, Isa
         DEF AsyncByzantineProposal, ByzantineBroadcastProposal,
             ProposalValue, Items, AsyncRuntimeScalarTypeVars,
             AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2> QED BY <1>1, <2>1, <2>2,
                 PublishTypedItemsPreservesSchedulerType
  <1> QED BY <1>1

THEOREM ByzantineVoteItemIsTyped ==
  \A signer \in ValidatorIds, recipient \in ValidatorIds,
     roundView \in Views, phase \in Phases, subject \in Subjects:
    TypeInvariant
      => AsyncItemTyped(
           AsyncNetworkItem(
             IF phase = "Prepare" THEN "PrepareVote" ELSE "CommitVote",
             signer,
             VoteEnvelope(
               recipient,
               Vote(context, roundView, phase, subject, signer))))
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW roundView \in Views,
                NEW phase \in Phases,
                NEW subject \in Subjects,
                TypeInvariant
         PROVE AsyncItemTyped(
                 AsyncNetworkItem(
                   IF phase = "Prepare"
                   THEN "PrepareVote" ELSE "CommitVote",
                   signer,
                   VoteEnvelope(
                     recipient,
                     Vote(context, roundView, phase, subject, signer))))
    <2>1. /\ context \in ContextRecords
           /\ context.height \in Heights
      BY <1>1 DEF TypeInvariant
    <2>2. Vote(context, roundView, phase, subject, signer)
             \in VoteRecordSet
      BY <1>1, <2>1, SMT DEF Vote, VoteRecordSet
    <2>3. VoteEnvelope(
             recipient,
             Vote(context, roundView, phase, subject, signer))
             \in VoteEnvelopeSet
      BY <1>1, <2>2, SMT DEF VoteEnvelope, VoteEnvelopeSet
    <2>4. VoteEnvelope(
             recipient,
             Vote(context, roundView, phase, subject, signer)).vote.phase
             = phase
      BY DEF VoteEnvelope, Vote
    <2> QED BY <1>1, <2>3, <2>4,
                 VoteEnvelopeMakesTypedAsyncItem
  <1> QED BY <1>1

THEOREM ByzantineVoteOutboxIsFiniteAndTyped ==
  \A signer \in ValidatorIds, roundView \in Views,
     phase \in Phases, subject \in Subjects:
    LET vote == Vote(context, roundView, phase, subject, signer)
        items == ByzantineVoteOutbox(signer, vote)
    IN TypeInvariant
         => /\ IsFiniteSet(items)
            /\ \A item \in items: AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW roundView \in Views,
                NEW phase \in Phases,
                NEW subject \in Subjects
         PROVE LET vote ==
                 Vote(context, roundView, phase, subject, signer)
               items == ByzantineVoteOutbox(signer, vote)
               IN TypeInvariant
                    => /\ IsFiniteSet(items)
                       /\ \A item \in items: AsyncItemTyped(item)
    <2>1. ASSUME TypeInvariant
           PROVE LET vote ==
                   Vote(context, roundView, phase, subject, signer)
                 items == ByzantineVoteOutbox(signer, vote)
                 IN /\ IsFiniteSet(items)
                    /\ \A item \in items: AsyncItemTyped(item)
      <3>1. /\ IsFiniteSet(CurrentVoters)
             /\ CurrentVoters \subseteq ValidatorIds
             /\ context \in ContextRecords
             /\ context.height \in Heights
        BY <2>1, CurrentVotersAreFiniteValidators
           DEF TypeInvariant
      <3>2. IsFiniteSet(
               ByzantineVoteOutbox(
                 signer, Vote(context, roundView, phase, subject, signer)))
        BY <3>1, FS_Image DEF ByzantineVoteOutbox
      <3>3. \A item \in
                    ByzantineVoteOutbox(
                      signer,
                      Vote(context, roundView, phase, subject, signer)):
               AsyncItemTyped(item)
        <4>1. ASSUME NEW item \in
                     ByzantineVoteOutbox(
                       signer,
                       Vote(context, roundView, phase, subject, signer))
               PROVE AsyncItemTyped(item)
          <5>1. PICK recipient \in CurrentVoters:
                   item = AsyncNetworkItem(
                     IF phase = "Prepare" THEN "PrepareVote"
                     ELSE "CommitVote",
                     signer,
                     VoteEnvelope(
                       recipient,
                       Vote(context, roundView, phase, subject, signer)))
            BY <4>1 DEF ByzantineVoteOutbox
          <5>2. recipient \in ValidatorIds
            BY <3>1, <5>1
          <5>3. AsyncItemTyped(
                   AsyncNetworkItem(
                     IF phase = "Prepare"
                     THEN "PrepareVote" ELSE "CommitVote",
                     signer,
                     VoteEnvelope(
                       recipient,
                       Vote(context, roundView, phase, subject, signer))))
            BY <1>1, <2>1, <5>2, ByzantineVoteItemIsTyped
          <5> QED BY <5>1, <5>3
        <4> QED BY <4>1
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM AsyncByzantineVotePreservesSchedulerType ==
  \A signer \in ValidatorIds, roundView \in Views,
     phase \in Phases, subject \in Subjects:
    AsyncTypeInvariant
      /\ AsyncByzantineVote(signer, roundView, phase, subject)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW roundView \in Views,
                NEW phase \in Phases,
                NEW subject \in Subjects,
                AsyncTypeInvariant,
                AsyncByzantineVote(signer, roundView, phase, subject)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE VoteValue ==
          Vote(context, roundView, phase, subject, signer)
    <2> DEFINE Items == ByzantineVoteOutbox(signer, VoteValue)
    <2>1. /\ IsFiniteSet(Items)
           /\ \A item \in Items: AsyncItemTyped(item)
      BY <1>1, ByzantineVoteOutboxIsFiniteAndTyped
         DEF AsyncTypeInvariant, VoteValue, Items
    <2>2. /\ PublishEphemeralItems(Items)
           /\ UNCHANGED <<context, asyncHeldChunks>>
           /\ UNCHANGED AsyncRuntimeScalarTypeVars
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
      BY <1>1, Isa
         DEF AsyncByzantineVote, ByzantineBroadcastVote,
             VoteValue, Items, AsyncRuntimeScalarTypeVars,
             AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2> QED BY <1>1, <2>1, <2>2,
                 PublishTypedItemsPreservesSchedulerType
  <1> QED BY <1>1

THEOREM ByzantineTimeoutItemIsTyped ==
  \A signer \in ValidatorIds, recipient \in ValidatorIds,
     roundView \in Views, highRank \in Ranks,
     highSubject \in SubjectOrNone:
    TypeInvariant
      => AsyncItemTyped(
           AsyncNetworkItem(
             "TimeoutVote", signer,
             TimeoutEnvelope(
               recipient,
               TimeoutVote(context, roundView, signer,
                           highRank, highSubject))))
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW roundView \in Views,
                NEW highRank \in Ranks,
                NEW highSubject \in SubjectOrNone,
                TypeInvariant
         PROVE AsyncItemTyped(
                 AsyncNetworkItem(
                   "TimeoutVote", signer,
                   TimeoutEnvelope(
                     recipient,
                     TimeoutVote(context, roundView, signer,
                                 highRank, highSubject))))
    <2>1. /\ context \in ContextRecords
           /\ context.height \in Heights
      BY <1>1 DEF TypeInvariant
    <2>2. TimeoutVote(context, roundView, signer,
                      highRank, highSubject) \in TimeoutVoteRecordSet
      BY <1>1, <2>1, SMT
         DEF TimeoutVote, TimeoutVoteRecordSet
    <2>3. TimeoutEnvelope(
             recipient,
             TimeoutVote(context, roundView, signer,
                         highRank, highSubject)) \in TimeoutEnvelopeSet
      BY <1>1, <2>2, SMT DEF TimeoutEnvelope, TimeoutEnvelopeSet
    <2> QED BY <1>1, <2>3,
                 TimeoutEnvelopeMakesTypedAsyncItem
  <1> QED BY <1>1

THEOREM ByzantineTimeoutOutboxIsFiniteAndTyped ==
  \A signer \in ValidatorIds, roundView \in Views,
     highRank \in Ranks, highSubject \in SubjectOrNone:
    LET vote ==
          TimeoutVote(context, roundView, signer, highRank, highSubject)
        items == ByzantineTimeoutOutbox(signer, vote)
    IN TypeInvariant
         => /\ IsFiniteSet(items)
            /\ \A item \in items: AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW roundView \in Views,
                NEW highRank \in Ranks,
                NEW highSubject \in SubjectOrNone
         PROVE LET vote ==
                 TimeoutVote(context, roundView, signer,
                             highRank, highSubject)
               items == ByzantineTimeoutOutbox(signer, vote)
               IN TypeInvariant
                    => /\ IsFiniteSet(items)
                       /\ \A item \in items: AsyncItemTyped(item)
    <2>1. ASSUME TypeInvariant
           PROVE LET vote ==
                   TimeoutVote(context, roundView, signer,
                               highRank, highSubject)
                 items == ByzantineTimeoutOutbox(signer, vote)
                 IN /\ IsFiniteSet(items)
                    /\ \A item \in items: AsyncItemTyped(item)
      <3>1. /\ IsFiniteSet(CurrentVoters)
             /\ CurrentVoters \subseteq ValidatorIds
             /\ context \in ContextRecords
             /\ context.height \in Heights
        BY <2>1, CurrentVotersAreFiniteValidators
           DEF TypeInvariant
      <3>2. IsFiniteSet(
               ByzantineTimeoutOutbox(
                 signer,
                 TimeoutVote(context, roundView, signer,
                             highRank, highSubject)))
        BY <3>1, FS_Image DEF ByzantineTimeoutOutbox
      <3>3. \A item \in
                    ByzantineTimeoutOutbox(
                      signer,
                      TimeoutVote(context, roundView, signer,
                                  highRank, highSubject)):
               AsyncItemTyped(item)
        <4>1. ASSUME NEW item \in
                     ByzantineTimeoutOutbox(
                       signer,
                       TimeoutVote(context, roundView, signer,
                                   highRank, highSubject))
               PROVE AsyncItemTyped(item)
          <5>1. PICK recipient \in CurrentVoters:
                   item = AsyncNetworkItem(
                     "TimeoutVote", signer,
                     TimeoutEnvelope(
                       recipient,
                       TimeoutVote(context, roundView, signer,
                                   highRank, highSubject)))
            BY <4>1 DEF ByzantineTimeoutOutbox
          <5>2. recipient \in ValidatorIds
            BY <3>1, <5>1
          <5>3. AsyncItemTyped(
                   AsyncNetworkItem(
                     "TimeoutVote", signer,
                     TimeoutEnvelope(
                       recipient,
                       TimeoutVote(context, roundView, signer,
                                   highRank, highSubject))))
            BY <1>1, <2>1, <5>2, ByzantineTimeoutItemIsTyped
          <5> QED BY <5>1, <5>3
        <4> QED BY <4>1
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM AsyncByzantineTimeoutPreservesSchedulerType ==
  \A signer \in ValidatorIds, roundView \in Views,
     highRank \in Ranks, highSubject \in SubjectOrNone:
    AsyncTypeInvariant
      /\ AsyncByzantineTimeout(
           signer, roundView, highRank, highSubject)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW signer \in ValidatorIds,
                NEW roundView \in Views,
                NEW highRank \in Ranks,
                NEW highSubject \in SubjectOrNone,
                AsyncTypeInvariant,
                AsyncByzantineTimeout(
                  signer, roundView, highRank, highSubject)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE VoteValue ==
          TimeoutVote(context, roundView, signer, highRank, highSubject)
    <2> DEFINE Items == ByzantineTimeoutOutbox(signer, VoteValue)
    <2>1. /\ IsFiniteSet(Items)
           /\ \A item \in Items: AsyncItemTyped(item)
      BY <1>1, ByzantineTimeoutOutboxIsFiniteAndTyped
         DEF AsyncTypeInvariant, VoteValue, Items
    <2>2. /\ PublishEphemeralItems(Items)
           /\ UNCHANGED <<context, asyncHeldChunks>>
           /\ UNCHANGED AsyncRuntimeScalarTypeVars
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
      BY <1>1, Isa
         DEF AsyncByzantineTimeout, ByzantineBroadcastTimeout,
             VoteValue, Items, AsyncRuntimeScalarTypeVars,
             AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2> QED BY <1>1, <2>1, <2>2,
                 PublishTypedItemsPreservesSchedulerType
  <1> QED BY <1>1

THEOREM AuthenticatedJunkItemIsTyped ==
  \A kind \in {"NormalJunk", "ProgressJunk"},
     source \in ValidatorIds, recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    LET envelope ==
          AsyncBodyEnvelope(recipient, context.height,
                            nodeView[recipient],
                            AsyncHeartbeatSubject, NoAsyncChunk, nonce)
        item == AsyncNetworkItem(kind, source, envelope)
    IN /\ TypeInvariant
       /\ AsyncConfiguration
       => AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW kind \in {"NormalJunk", "ProgressJunk"},
                NEW source \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW nonce \in 0..(AsyncIngressCapacity - 1)
         PROVE LET envelope ==
                 AsyncBodyEnvelope(recipient, context.height,
                                   nodeView[recipient],
                                   AsyncHeartbeatSubject,
                                   NoAsyncChunk, nonce)
               item == AsyncNetworkItem(kind, source, envelope)
               IN /\ TypeInvariant
                  /\ AsyncConfiguration
                  => AsyncItemTyped(item)
    <2>1. ASSUME TypeInvariant, AsyncConfiguration
           PROVE LET envelope ==
                   AsyncBodyEnvelope(recipient, context.height,
                                     nodeView[recipient],
                                     AsyncHeartbeatSubject,
                                     NoAsyncChunk, nonce)
                 item == AsyncNetworkItem(kind, source, envelope)
                 IN AsyncItemTyped(item)
      <3>1. /\ ModelConfiguration
             /\ context.height \in Heights
             /\ nodeView[recipient] \in Views
             /\ AsyncHeartbeatSubject \in ValidSubjects
        BY <1>1, <2>1, AsyncHeartbeatSubjectIsValid
           DEF TypeInvariant
      <3> QED BY <1>1, <2>1, <3>1, SMT
           DEF AsyncItemTyped, AsyncNetworkItem,
               AsyncBodyEnvelopeTyped, AsyncBodyEnvelope,
               AsyncNetworkKinds, AsyncIngressSources,
               AsyncConfiguration, NoAsyncChunk
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM InjectAuthenticatedJunkPreservesSchedulerType ==
  \A kind \in {"NormalJunk", "ProgressJunk"},
     source \in ValidatorIds, recipient \in ValidatorIds,
     nonce \in 0..(AsyncIngressCapacity - 1):
    AsyncTypeInvariant
      /\ InjectAuthenticatedJunk(kind, source, recipient, nonce)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW kind \in {"NormalJunk", "ProgressJunk"},
                NEW source \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW nonce \in 0..(AsyncIngressCapacity - 1),
                AsyncTypeInvariant,
                InjectAuthenticatedJunk(kind, source, recipient, nonce)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE Envelope ==
          AsyncBodyEnvelope(recipient, context.height,
                            nodeView[recipient],
                            AsyncHeartbeatSubject, NoAsyncChunk, nonce)
    <2> DEFINE Item == AsyncNetworkItem(kind, source, Envelope)
    <2>1. AsyncItemTyped(Item)
      BY <1>1, AuthenticatedJunkItemIsTyped
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             Envelope, Item
    <2>2. PublishEphemeralItems({Item})
      BY <1>1, Isa
         DEF InjectAuthenticatedJunk, PublishEphemeralItems,
             PacketsForItems, Envelope, Item
    <2>3. /\ UNCHANGED <<context, asyncHeldChunks>>
           /\ UNCHANGED AsyncRuntimeScalarTypeVars
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
      BY <1>1, Isa
         DEF InjectAuthenticatedJunk, LeaveCausalQueues,
             AsyncRuntimeScalarTypeVars, AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
                 PublishTypedSingletonPreservesSchedulerType
  <1> QED BY <1>1

THEOREM CertifiedRequestEnvelopeMakesTypedAsyncItem ==
  \A source \in ValidatorIds:
    \A envelope:
      AsyncBodyEnvelopeTyped(envelope)
        => AsyncItemTyped(
             AsyncNetworkItem("CertifiedRequest", source, envelope))
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW envelope,
                AsyncBodyEnvelopeTyped(envelope)
         PROVE AsyncItemTyped(
                 AsyncNetworkItem("CertifiedRequest", source, envelope))
    <2>1. /\ DOMAIN
                  AsyncNetworkItem("CertifiedRequest", source, envelope) =
                  {"kind", "source", "envelope"}
           /\ AsyncNetworkItem(
                "CertifiedRequest", source, envelope).kind =
                "CertifiedRequest"
           /\ AsyncNetworkItem(
                "CertifiedRequest", source, envelope).source = source
           /\ AsyncNetworkItem(
                "CertifiedRequest", source, envelope).envelope = envelope
      BY DEF AsyncNetworkItem
    <2>2. /\ "CertifiedRequest" \in AsyncNetworkKinds
           /\ source \in AsyncIngressSources
           /\ envelope.recipient \in ValidatorIds
      BY <1>1, SMT
         DEF AsyncNetworkKinds, AsyncIngressSources,
             AsyncBodyEnvelopeTyped
    <2> QED BY <1>1, <2>1, <2>2, SMT DEF AsyncItemTyped
  <1> QED BY <1>1

THEOREM CertifiedRequestFieldsMakeTypedItem ==
  \A source \in ValidatorIds, recipient \in ValidatorIds,
     blockHeight \in Heights, roundView \in Views,
     subject \in ValidSubjects,
     nonce \in 0..(AsyncIngressCapacity - 1):
    AsyncConfiguration
      => AsyncItemTyped(
           AsyncNetworkItem(
             "CertifiedRequest", source,
             AsyncBodyEnvelope(recipient, blockHeight, roundView, subject,
                               NoAsyncChunk, nonce)))
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW blockHeight \in Heights,
                NEW roundView \in Views,
                NEW subject \in ValidSubjects,
                NEW nonce \in 0..(AsyncIngressCapacity - 1),
                AsyncConfiguration
         PROVE AsyncItemTyped(
                 AsyncNetworkItem(
                   "CertifiedRequest", source,
                   AsyncBodyEnvelope(
                     recipient, blockHeight, roundView, subject,
                     NoAsyncChunk, nonce)))
    <2>1. AsyncBodyEnvelopeTyped(
             AsyncBodyEnvelope(
               recipient, blockHeight, roundView, subject,
               NoAsyncChunk, nonce))
      BY <1>1, SMT
         DEF AsyncBodyEnvelopeTyped, AsyncBodyEnvelope,
             AsyncConfiguration, NoAsyncChunk
    <2> QED BY <1>1, <2>1,
                 CertifiedRequestEnvelopeMakesTypedAsyncItem
  <1> QED BY <1>1

THEOREM ByzantineCertifiedRequestItemIsTyped ==
  \A source \in ValidatorIds, recipient \in ValidatorIds,
     qc \in commitQCs, nonce \in 0..(AsyncIngressCapacity - 1):
    LET envelope ==
          AsyncBodyEnvelope(recipient, context.height, qc.view, qc.subject,
                            NoAsyncChunk, nonce)
        item == AsyncNetworkItem("CertifiedRequest", source, envelope)
    IN /\ StrongInductiveInvariant
       /\ AsyncConfiguration
       => AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW qc \in commitQCs,
                NEW nonce \in 0..(AsyncIngressCapacity - 1)
         PROVE LET envelope ==
                 AsyncBodyEnvelope(recipient, context.height,
                                   qc.view, qc.subject,
                                   NoAsyncChunk, nonce)
               item ==
                 AsyncNetworkItem("CertifiedRequest", source, envelope)
               IN /\ StrongInductiveInvariant
                  /\ AsyncConfiguration
                  => AsyncItemTyped(item)
    <2>1. ASSUME StrongInductiveInvariant, AsyncConfiguration
           PROVE LET envelope ==
                   AsyncBodyEnvelope(recipient, context.height,
                                     qc.view, qc.subject,
                                     NoAsyncChunk, nonce)
                 item ==
                   AsyncNetworkItem("CertifiedRequest", source, envelope)
                 IN AsyncItemTyped(item)
      <3>1. /\ context.height \in Heights
             /\ qc.view \in Views
             /\ qc.subject \in ValidSubjects
        BY <1>1, <2>1
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               ReducerProvenanceInvariant, CertificatesBackedByIntents,
               HistoricalQcValid
      <3> QED BY <1>1, <2>1, <3>1,
                   CertifiedRequestFieldsMakeTypedItem
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM InjectByzantineCertifiedRequestPreservesSchedulerType ==
  \A source \in ValidatorIds, recipient \in ValidatorIds,
     qc \in commitQCs, nonce \in 0..(AsyncIngressCapacity - 1):
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ InjectByzantineCertifiedRequest(source, recipient, qc, nonce)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW recipient \in ValidatorIds,
                NEW qc \in commitQCs,
                NEW nonce \in 0..(AsyncIngressCapacity - 1),
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                InjectByzantineCertifiedRequest(
                  source, recipient, qc, nonce)
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE Envelope ==
          AsyncBodyEnvelope(recipient, context.height, qc.view, qc.subject,
                            NoAsyncChunk, nonce)
    <2> DEFINE Item ==
          AsyncNetworkItem("CertifiedRequest", source, Envelope)
    <2>1. AsyncItemTyped(Item)
      BY <1>1, ByzantineCertifiedRequestItemIsTyped
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             Envelope, Item
    <2>2. PublishEphemeralItems({Item})
      BY <1>1, Isa
         DEF InjectByzantineCertifiedRequest, PublishEphemeralItems,
             PacketsForItems, Envelope, Item
    <2>3. /\ UNCHANGED <<context, asyncHeldChunks>>
           /\ UNCHANGED AsyncRuntimeScalarTypeVars
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
      BY <1>1, Isa
         DEF InjectByzantineCertifiedRequest, LeaveCausalQueues,
             AsyncRuntimeScalarTypeVars, AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars,
             AsyncIngressTopologyTypeVars, AsyncSchedulerVars, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
                 PublishTypedSingletonPreservesSchedulerType
  <1> QED BY <1>1

THEOREM AsyncFaultStepPreservesSchedulerType ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncFaultStep
  => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncFaultStep
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE \E packet \in asyncTransport:
                  PreGstLosePacket(packet)
      BY <1>1, <2>1, PreGstPacketLossPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <1>1, <2>2, PreGstCrashPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>3. CASE \E source \in AsyncIngressSources,
                  recipient \in ValidatorIds,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectByzantineNoise(source, recipient, nonce)
      BY <1>1, <2>3, InjectByzantineNoisePreservesSchedulerType
    <2>4. CASE \E kind \in {"NormalJunk", "ProgressJunk"},
                  source \in ValidatorIds, recipient \in ValidatorIds,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectAuthenticatedJunk(
                    kind, source, recipient, nonce)
      BY <1>1, <2>4,
         InjectAuthenticatedJunkPreservesSchedulerType
    <2>5. CASE \E source \in ValidatorIds,
                  recipient \in ValidatorIds, qc \in commitQCs,
                  nonce \in 0..(AsyncIngressCapacity - 1):
                  InjectByzantineCertifiedRequest(
                    source, recipient, qc, nonce)
      BY <1>1, <2>5,
         InjectByzantineCertifiedRequestPreservesSchedulerType
    <2>6. CASE \E signer \in ValidatorIds, roundView \in Views,
                  subject \in Subjects, justifyRank \in Ranks,
                  justifySubject \in SubjectOrNone:
                  AsyncByzantineProposal(
                    signer, roundView, subject,
                    justifyRank, justifySubject)
      BY <1>1, <2>6,
         AsyncByzantineProposalPreservesSchedulerType
    <2>7. CASE \E signer \in ValidatorIds, roundView \in Views,
                  phase \in Phases, subject \in Subjects:
                  AsyncByzantineVote(
                    signer, roundView, phase, subject)
      BY <1>1, <2>7, AsyncByzantineVotePreservesSchedulerType
    <2>8. CASE \E signer \in ValidatorIds, roundView \in Views,
                  highRank \in Ranks, highSubject \in SubjectOrNone:
                  AsyncByzantineTimeout(
                    signer, roundView, highRank, highSubject)
      BY <1>1, <2>8,
         AsyncByzantineTimeoutPreservesSchedulerType
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4,
                <2>5, <2>6, <2>7, <2>8
         DEF AsyncFaultStep
  <1> QED BY <1>1

THEOREM AsyncNonRunnerStepPreservesSchedulerType ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncNonRunnerStep
  => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncNonRunnerStep
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE AsyncSetGST
      BY <1>1, <2>1, AsyncSetGstPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>2. CASE AsyncTick
      BY <1>1, <2>2, AsyncTickPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2>3. CASE \E node \in AsyncCurrentResponsiveVoters:
                  ServiceIoWorker(node)
      BY <1>1, <2>3, ServiceIoWorkerPreservesSchedulerType
    <2>4. CASE \E node \in AsyncCurrentResponsiveVoters:
                  EnqueueIoLocalControl(node)
      BY <1>1, <2>4, EnqueueIoControlPreservesSchedulerType
    <2>5. CASE AsyncNetworkStep
      BY <1>1, <2>5, AsyncNetworkStepPreservesSchedulerType
    <2>6. CASE AsyncFaultStep
      BY <1>1, <2>6, AsyncFaultStepPreservesSchedulerType
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
         DEF AsyncNonRunnerStep
  <1> QED BY <1>1

(***************************************************************************
Strengthened asynchronous induction.  The Core safety proof is reusable
through the refinement boundary, while scheduler state and the concrete
timeout-receipt pool require their own asynchronous preservation arguments.
Keeping all three conjuncts in one invariant makes the final temporal proof
an ordinary Init/Next induction rather than an implicit reachability claim.
***************************************************************************)

AsyncStrongTypeInvariant ==
  /\ StrongInductiveInvariant
  /\ AsyncSchedulerTypeInvariant
  /\ ReceivedTimeoutVotePoolInvariant

THEOREM AsyncStrongTypeProjectsAsyncType ==
  AsyncStrongTypeInvariant => AsyncTypeInvariant
BY DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
       StrongInductiveInvariant, Safety

THEOREM AsyncInitEstablishesStrongTypeInvariant ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncStrongTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncStrongTypeInvariant
    <2>1. StrongInductiveInvariant
      BY <1>1, InitAtEstablishesStrongInductiveInvariant
         DEF AsyncInitAt, AsyncBaseInitAt
    <2>2. TypeInvariant
      BY <2>1 DEF StrongInductiveInvariant, Safety
    <2>3. AsyncSchedulerTypeInvariant
      BY <1>1, <2>2, AsyncInitEstablishesSchedulerType
    <2>4. ReceivedTimeoutVotePoolInvariant
      BY <1>1, AsyncInitEstablishesTimeoutPoolInvariant
    <2> QED BY <2>1, <2>3, <2>4 DEF AsyncStrongTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncNextPreservesStrongInductiveInvariant ==
  StrongInductiveInvariant /\ AsyncNext
    => StrongInductiveInvariant'
BY AsyncStepRefinementObligation,
   CoreStrongInductiveActionPreservation

THEOREM AsyncAllVarsStutterPreservesTimeoutPoolInvariant ==
  ReceivedTimeoutVotePoolInvariant /\ UNCHANGED AsyncAllVars
    => ReceivedTimeoutVotePoolInvariant'
PROOF
  <1>1. ASSUME ReceivedTimeoutVotePoolInvariant,
              UNCHANGED AsyncAllVars
         PROVE ReceivedTimeoutVotePoolInvariant'
    <2>1. /\ receivedTimeoutVotes' = receivedTimeoutVotes
           /\ context' = context
           /\ height' = height
           /\ prepareQCs' = prepareQCs
      BY <1>1, Isa DEF AsyncAllVars, vars, AsyncSchedulerVars
    <2>2. prepareQCs \subseteq prepareQCs'
      BY <2>1
    <2> QED BY <1>1, <2>1, <2>2,
                TimeoutPoolFramePreservesInvariant
  <1> QED BY <1>1

THEOREM AsyncAllVarsStutterPreservesStrongTypeInvariant ==
  AsyncStrongTypeInvariant /\ UNCHANGED AsyncAllVars
    => AsyncStrongTypeInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              UNCHANGED AsyncAllVars
         PROVE AsyncStrongTypeInvariant'
    <2>1. /\ StrongInductiveInvariant
           /\ AsyncSchedulerTypeInvariant
           /\ ReceivedTimeoutVotePoolInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2. UNCHANGED vars
      BY <1>1, Isa DEF AsyncAllVars
    <2>3. StrongInductiveInvariant'
      BY <2>1, <2>2, CoreStrongInductiveActionPreservation
    <2>4. AsyncSchedulerTypeInvariant'
      BY <1>1, <2>1, AsyncAllVarsStutterPreservesSchedulerType
    <2>5. ReceivedTimeoutVotePoolInvariant'
      BY <1>1, <2>1,
         AsyncAllVarsStutterPreservesTimeoutPoolInvariant
    <2> QED BY <2>3, <2>4, <2>5 DEF AsyncStrongTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncNextPreservesSchedulerType ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncNext
  => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncNext
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE AsyncNonCrashStep
      <3>1. CASE AsyncRunnerStep
        BY <1>1, <2>1, <3>1,
           AsyncRunnerStepPreservesSchedulerType
      <3>2. CASE AsyncNonRunnerStep
        BY <1>1, <2>1, <3>2,
           AsyncNonRunnerStepPreservesSchedulerType
      <3> QED BY <2>1, <3>1, <3>2 DEF AsyncNonCrashStep
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <1>1, <2>2, PreGstCrashPreservesSchedulerType
         DEF AsyncTypeInvariant
    <2> QED BY <1>1, <2>1, <2>2 DEF AsyncNext
  <1> QED BY <1>1

THEOREM AsyncNextPreservesStrongTypeInvariant ==
  AsyncStrongTypeInvariant /\ AsyncNext
    => AsyncStrongTypeInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncNext
         PROVE AsyncStrongTypeInvariant'
    <2>1. StrongInductiveInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>3. StrongInductiveInvariant'
      BY <1>1, <2>1, AsyncNextPreservesStrongInductiveInvariant
    <2>4. AsyncSchedulerTypeInvariant'
      BY <1>1, <2>1, <2>2,
         AsyncNextPreservesSchedulerType
    <2>5. ReceivedTimeoutVotePoolInvariant'
      BY <1>1, <2>2, AsyncNextPreservesTimeoutPoolInvariant
    <2> QED BY <2>3, <2>4, <2>5 DEF AsyncStrongTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncBracketNextPreservesStrongTypeInvariant ==
  AsyncStrongTypeInvariant /\ [AsyncNext]_AsyncAllVars
    => AsyncStrongTypeInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              [AsyncNext]_AsyncAllVars
         PROVE AsyncStrongTypeInvariant'
    <2>1. CASE AsyncNext
      BY <1>1, <2>1, AsyncNextPreservesStrongTypeInvariant
    <2>2. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>2,
         AsyncAllVarsStutterPreservesStrongTypeInvariant
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Retained exact-body round rebinding.  The immutable body fact may survive a
view change, but the production adapter does not reuse old manifest, storage,
or validation completions.  Proposal delivery therefore emits a completion-
class `RebindRetainedBody` candidate.  It crosses the current-round FetchBody
boundary, then the ordinary StoreBody -> ValidateBody chain writes validation
evidence keyed by the proposal view and the generation current at execution.
***************************************************************************)

RetainedBodyRebindReady(command) ==
  /\ command.kind = "RebindRetainedBody"
  /\ command.class = "Completion"
  /\ BodyHeldBy(durableBodies, command.node, context, command.subject)
  /\ BodyRecord(command.node, context, command.subject)
       \notin availableBodies
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ ProposalAt(command.node, proposal) \in seenProposals

RetainedBodyRebindAction(command, proposal) ==
  /\ command.kind = "RebindRetainedBody"
  /\ BodyHeldBy(durableBodies, command.node, context, command.subject)
  /\ CommandMatches(command, command.node, proposal.view,
                    proposal.subject)
  /\ FetchBody(command.node, proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM RetainedBodyRebindCandidateIsTypedAndOwned ==
  \A command:
    (AsyncTypeInvariant /\ AsyncCandidateTyped(command))
      => /\ AsyncCandidateTyped(
               RetainedBodyRebindCandidate(command))
         /\ RetainedBodyRebindCandidate(command)
              \in AsyncCandidateSet
         /\ RetainedBodyRebindCandidate(command).node = command.node
         /\ RetainedBodyRebindCandidate(command).class = "Completion"
         /\ RetainedBodyRebindCandidate(command).kind =
              "RebindRetainedBody"
PROOF
  <1>1. ASSUME NEW command,
                AsyncTypeInvariant,
                AsyncCandidateTyped(command)
         PROVE /\ AsyncCandidateTyped(
                      RetainedBodyRebindCandidate(command))
                /\ RetainedBodyRebindCandidate(command)
                     \in AsyncCandidateSet
                /\ RetainedBodyRebindCandidate(command).node =
                     command.node
                /\ RetainedBodyRebindCandidate(command).class =
                     "Completion"
                /\ RetainedBodyRebindCandidate(command).kind =
                     "RebindRetainedBody"
    <2>1. context.height \in Heights
      BY <1>1 DEF AsyncTypeInvariant, TypeInvariant
    <2>2. /\ command.node \in ValidatorIds
           /\ command.view \in Views
           /\ command.subject \in SubjectOrNone
      BY <1>1 DEF AsyncCandidateTyped
    <2>3. /\ "Completion" \in AsyncCommandClasses
           /\ "RebindRetainedBody" \in AsyncWorkKinds
           /\ NoAsyncItem \in AsyncNetworkItems \cup {NoAsyncItem}
      BY DEF AsyncCommandClasses, AsyncWorkKinds, AsyncReducerKinds
    <2>4. RetainedBodyRebindCandidate(command) =
             AsyncCandidate(
               "Completion", "RebindRetainedBody", command.node,
               context.height, command.view, command.subject, NoAsyncItem)
      BY DEF RetainedBodyRebindCandidate, CausalCandidate,
             NoItemCandidate
    <2>5. AsyncCandidateTyped(
             RetainedBodyRebindCandidate(command))
      BY <2>1, <2>2, <2>3, <2>4, Isa
         DEF AsyncCandidateTyped, AsyncCandidate
    <2>6. RetainedBodyRebindCandidate(command)
             \in AsyncCandidateSet
      BY <2>1, <2>2, <2>3, <2>4, SMTT(60)
         DEF AsyncCandidateSet, AsyncCandidate
    <2> QED BY <2>4, <2>5, <2>6
       DEF AsyncCandidate
  <1> QED BY <1>1

THEOREM DeliverProposalSchedulesRetainedBodyRebind ==
  \A command:
    command.kind = "DeliverProposal"
      => CommandSuccessors(command) =
           <<RetainedBodyRebindCandidate(command),
             CausalCandidate("Normal", "BeginPrepare", command)>>
BY DEF CommandSuccessors

THEOREM RebindSchedulesCurrentRoundStore ==
  \A command:
    command.kind = "RebindRetainedBody"
      => CommandSuccessors(command) =
           <<CausalCandidate("Completion", "StoreBody", command)>>
BY DEF CommandSuccessors

THEOREM StoreSchedulesCurrentRoundValidation ==
  \A command:
    command.kind = "StoreBody"
      => CommandSuccessors(command) =
           <<CausalCandidate("Completion", "ValidateBody", command)>>
BY DEF CommandSuccessors

THEOREM ValidationSchedulesFreshPrepareAttempt ==
  \A command:
    command.kind = "ValidateBody"
      => CommandSuccessors(command) =
           <<CausalCandidate("Normal", "BeginPrepare", command),
             CausalCandidate("Completion", "Apply", command)>>
BY DEF CommandSuccessors

THEOREM ReadyRetainedBodyRebindEnablesExecution ==
  \A command:
    RetainedBodyRebindReady(command)
      => ENABLED ExecuteCommand(command)
PROOF
  <1>1. ASSUME NEW command,
                RetainedBodyRebindReady(command)
         PROVE ENABLED ExecuteCommand(command)
    <2>1. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ ProposalAt(command.node, proposal) \in seenProposals
      BY <1>1 DEF RetainedBodyRebindReady
    <2>2. ENABLED RetainedBodyRebindAction(command, proposal)
      BY <1>1, <2>1, ExpandENABLED, Isa
         DEF RetainedBodyRebindReady, RetainedBodyRebindAction,
             CommandMatches, FetchBody, AsyncAuxVars
    <2>3. RetainedBodyRebindAction(command, proposal) \in BOOLEAN
      BY Isa DEF RetainedBodyRebindAction
    <2>4. ExecuteCommand(command) \in BOOLEAN
      BY Isa DEF ExecuteCommand
    <2>5. RetainedBodyRebindAction(command, proposal)
             => ExecuteCommand(command)
      BY Isa
         DEF RetainedBodyRebindAction, ExecuteCommand,
             ExecuteRegularCommand, RegularCoreCommand
    <2>6. (ENABLED RetainedBodyRebindAction(command, proposal))
             => ENABLED ExecuteCommand(command)
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>6
  <1> QED BY <1>1

THEOREM ReadyRetainedBodyRebindIsDispatchable ==
  \A command:
    (RetainedBodyRebindReady(command)
      /\ command \in AsyncCandidateSet)
      => CommandDispatchable(command)
PROOF
  <1>1. ASSUME NEW command,
                RetainedBodyRebindReady(command),
                command \in AsyncCandidateSet
         PROVE \E selectedCommand \in AsyncCandidateSet:
                   /\ selectedCommand = command
                   /\ ENABLED ExecuteCommand(selectedCommand)
                   /\ (NodeIdle(selectedCommand.node)
                         \/ selectedCommand.class = "Completion")
    <2>1. ENABLED ExecuteCommand(command)
      BY <1>1, ReadyRetainedBodyRebindEnablesExecution
    <2>2. command.class = "Completion"
      BY <1>1 DEF RetainedBodyRebindReady
    <2>3. WITNESS command \in AsyncCandidateSet
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1 DEF CommandDispatchable

THEOREM RebindCommandSelectsFetchBody ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "RebindRetainedBody")
      => \E proposal \in SeenProposalValues:
           /\ CommandMatches(command, command.node, proposal.view,
                             proposal.subject)
           /\ FetchBody(command.node, proposal)
BY IsaT(60) DEF RegularCoreCommand

THEOREM ExecuteRebindStagesCurrentRoundBody ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "RebindRetainedBody")
      => /\ BodyRecord(command.node, context', command.subject)
                \in availableBodies'
         /\ BodyHeldBy(durableBodies', command.node, context',
                       command.subject)
PROOF
  <1>1. ASSUME NEW command,
                RegularCoreCommand(command),
                command.kind = "RebindRetainedBody"
         PROVE /\ BodyRecord(command.node, context', command.subject)
                       \in availableBodies'
                /\ BodyHeldBy(durableBodies', command.node, context',
                              command.subject)
    <2>1. \E proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ FetchBody(command.node, proposal)
      BY <1>1, RebindCommandSelectsFetchBody
    <2>2. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ FetchBody(command.node, proposal)
      BY <2>1
    <2>3. /\ command.subject = proposal.subject
           /\ context' = context
           /\ durableBodies' = durableBodies
           /\ BodyRecord(command.node, context, proposal.subject)
                \in availableBodies'
      BY <2>2, Isa DEF CommandMatches, FetchBody
    <2> QED BY <1>1, <2>3 DEF RegularCoreCommand, BodyHeldBy
  <1> QED BY <1>1

THEOREM ValidationCommandSelectsValidateBody ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "ValidateBody")
      => \E proposal \in SeenProposalValues:
           /\ CommandMatches(command, command.node, proposal.view,
                             proposal.subject)
           /\ ValidateBody(command.node, proposal)
BY Isa DEF RegularCoreCommand

THEOREM ExecuteValidationBindsCurrentViewAndGeneration ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "ValidateBody")
      => BodyValidatedBy(
           validatedBodies', command.node, context', command.view,
           generation'[command.node], command.subject)
PROOF
  <1>1. ASSUME NEW command,
                RegularCoreCommand(command),
                command.kind = "ValidateBody"
         PROVE BodyValidatedBy(
                 validatedBodies', command.node, context', command.view,
                 generation'[command.node], command.subject)
    <2>1. \E proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ ValidateBody(command.node, proposal)
      BY <1>1, ValidationCommandSelectsValidateBody
    <2>2. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ ValidateBody(command.node, proposal)
      BY <2>1
    <2>3. /\ command.view = proposal.view
           /\ command.subject = proposal.subject
           /\ context' = context
           /\ generation' = generation
           /\ ValidationRecord(
                command.node, context, proposal.view,
                generation[command.node], proposal.subject)
                \in validatedBodies'
      BY <2>2, Isa DEF CommandMatches, ValidateBody
    <2> QED BY <2>3 DEF BodyValidatedBy
  <1> QED BY <1>1

(***************************************************************************
Locked-round CommitVote recovery after a TC install.  The install clears only
the installing node's volatile vote receipts.  Retained CommitVote control is
still retryable, and delivery may rebuild an older round only when it matches
the exact durable Prepare lock.  No unrelated historical vote is admissible.
***************************************************************************)

THEOREM HistoricalVoteAdmissionIsExactLockedCommit ==
  \A node, vote:
    (VoteRoundAdmissible(node, vote)
      /\ vote.view # nodeView[node])
      => /\ vote.phase = "Commit"
         /\ LockedPrepareRound(node, vote.view, vote.subject)
BY DEF VoteRoundAdmissible

THEOREM HistoricalCommitFormationIsExactLockedRound ==
  \A node, roundView, subject:
    (CommitRoundAdmissible(node, roundView, subject)
      /\ roundView # nodeView[node])
      => LockedPrepareRound(node, roundView, subject)
BY DEF CommitRoundAdmissible

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

THEOREM DeliveredVoteRebuildsItsRoundPool ==
  \A envelope:
    DeliverVote(envelope)
      => VoteAt(envelope.recipient, envelope.vote) \in receivedVotes'
BY SMT DEF DeliverVote

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
  \A node \in AsyncCurrentResponsiveVoters:
    (AsyncTypeInvariant /\ gst /\ AsyncIoQueueDepth(node) > 0)
      => ENABLED PostGstServiceIoWorker(node)
BY ExpandENABLED, Isa
   DEF PostGstServiceIoWorker, ServiceIoWorker,
       PublishEphemeralItems, LeaveCausalQueues,
       AsyncIoQueueDepth, AsyncAllVars, AsyncSchedulerVars,
       AsyncDeferredVars, vars

(***************************************************************************
The proof ledger records these exact release obligations as specified but
unproved.  Leaving them proofless is intentional: the structural gate admits
only explicitly ledgered debt, while the release gate continues to reject it.
***************************************************************************)

THEOREM AsyncTypeInvariantObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []AsyncTypeInvariant

THEOREM TimeoutViewProgressObligation ==
  \A initialContext:
    TimeoutViewProgressProperty(AsyncSpecAt(initialContext))

THEOREM RotatingLeaderProgressObligation ==
  \A initialContext:
    RotatingLeaderProgressProperty(AsyncSpecAt(initialContext))

THEOREM ApplicationLivenessObligation ==
  \A initialContext:
    ApplicationLivenessProperty(AsyncSpecAt(initialContext))

=============================================================================
