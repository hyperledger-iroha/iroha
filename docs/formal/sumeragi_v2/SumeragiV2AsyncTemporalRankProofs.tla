---- MODULE SumeragiV2AsyncTemporalRankProofs ----
EXTENDS SumeragiV2AsyncRecoveryProgressWitnessProofs

(***************************************************************************
GST cannot coexist with the responsive replay quarantine.  `AsyncSetGST`
rejects every replay-required/replaying phase, and every action which enters
or advances those phases is pre-GST only.  This reachable-state fact is needed
by productive deadlock freedom: an applied voter uses the historical server
to rearm its service deadline after GST, so its enabledness must not rest on
an informal assumption that the quarantine has disappeared.
***************************************************************************)

PostGstReplayQuarantineExcluded ==
  gst => /\ asyncRecoveryPhase \in {"Eligible", "Recovered"}
         /\ \A node \in ValidatorIds:
              ~ResponsiveReplayQuarantined(node)

THEOREM AsyncInitExcludesPostGstReplayQuarantine ==
  \A initialContext:
    AsyncInitAt(initialContext) => PostGstReplayQuarantineExcluded
BY DEF AsyncInitAt, AsyncBaseInitAt,
       PostGstReplayQuarantineExcluded

THEOREM AsyncNextPreservesPostGstReplayQuarantineExclusion ==
  /\ PostGstReplayQuarantineExcluded
  /\ [AsyncNext]_AsyncAllVars
  => PostGstReplayQuarantineExcluded'
BY IsaT(120)
   DEF PostGstReplayQuarantineExcluded,
       ResponsiveReplayQuarantined, AsyncRecoveryPhases,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncSetGST, SetGST,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       RunHistoricalServer, OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       AsyncNetworkStep, AdmitIngressPacket, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery,
       AsyncTick, AsyncAllVars

THEOREM AsyncSpecAlwaysExcludesPostGstReplayQuarantine ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []PostGstReplayQuarantineExcluded
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []PostGstReplayQuarantineExcluded
    <2>1. AsyncInitAt(initialContext)
             => PostGstReplayQuarantineExcluded
      BY AsyncInitExcludesPostGstReplayQuarantine
    <2>2. PostGstReplayQuarantineExcluded
             /\ [AsyncNext]_AsyncAllVars
           => PostGstReplayQuarantineExcluded'
      BY AsyncNextPreservesPostGstReplayQuarantineExclusion
    <2> QED BY <2>1, <2>2, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

(***************************************************************************
Well-founded composition of protected service ranks.

The concrete rank property promises either ownership exit or a strict
lexicographic decrease.  Scheduler ownership uses exactly stages 2 through 6,
so typing keeps every still-owned candidate inside the `(2..6) \X Nat`
carrier.  The checked LATTICE rule then turns those local decreases into
starvation freedom.  Keeping this derivation here makes the exported
starvation theorem depend on the exact rank obligation instead of restating
fairness as a second assumption or silently widening the carrier.
***************************************************************************)

THEOREM CandidateSequenceIndexIsPosition ==
  \A candidate, queue:
    /\ queue \in Seq(Range(queue))
    /\ candidate \in SequenceSet(queue)
    => CandidateSequenceIndex(candidate, queue) \in 1..Len(queue)
BY Isa DEF CandidateSequenceIndex, SequenceSet

THEOREM CandidateSequenceIndexCharacterization ==
  \A candidate, queue:
    /\ queue \in Seq(Range(queue))
    /\ SequenceHasUniqueValues(queue)
    /\ candidate \in SequenceSet(queue)
    => /\ CandidateSequenceIndex(candidate, queue) \in 1..Len(queue)
       /\ queue[CandidateSequenceIndex(candidate, queue)] = candidate
       /\ \A index \in 1..Len(queue):
            queue[index] = candidate
              => index = CandidateSequenceIndex(candidate, queue)
PROOF
  <1>1. ASSUME NEW candidate, NEW queue,
                queue \in Seq(Range(queue)),
                SequenceHasUniqueValues(queue),
                candidate \in SequenceSet(queue)
         PROVE /\ CandidateSequenceIndex(candidate, queue)
                       \in 1..Len(queue)
               /\ queue[CandidateSequenceIndex(candidate, queue)]
                    = candidate
               /\ \A index \in 1..Len(queue):
                    queue[index] = candidate
                      => index = CandidateSequenceIndex(candidate, queue)
    <2>1. PICK matching \in 1..Len(queue):
             queue[matching] = candidate
      BY <1>1 DEF SequenceSet
    <2>2. \E index \in 1..Len(queue): queue[index] = candidate
      BY <2>1
    <2>3. PICK chosen \in
                  {index \in 1..Len(queue): queue[index] = candidate}:
             chosen = CandidateSequenceIndex(candidate, queue)
      BY <2>2 DEF CandidateSequenceIndex
    <2>4. /\ CandidateSequenceIndex(candidate, queue)
                    \in 1..Len(queue)
           /\ queue[CandidateSequenceIndex(candidate, queue)] = candidate
      BY <2>3
    <2>5. IsInjective(queue)
      BY <1>1, UniqueSequenceLengthImpliesInjective
         DEF SequenceHasUniqueValues
    <2>6. DOMAIN queue = 1..Len(queue)
      BY <1>1, SeqOfRange, LenProperties
    <2>7. \A index \in 1..Len(queue):
             queue[index] = candidate
               => index = CandidateSequenceIndex(candidate, queue)
      <3>1. ASSUME NEW index \in 1..Len(queue),
                    queue[index] = candidate
             PROVE index = CandidateSequenceIndex(candidate, queue)
        <4>1. /\ index \in DOMAIN queue
               /\ CandidateSequenceIndex(candidate, queue)
                    \in DOMAIN queue
          BY <2>4, <2>6, <3>1
        <4>2. queue[index]
                 = queue[CandidateSequenceIndex(candidate, queue)]
          BY <2>4, <3>1
        <4> QED BY <2>5, <4>1, <4>2 DEF IsInjective
      <3> QED BY <3>1
    <2> QED BY <2>4, <2>7
  <1> QED BY <1>1

THEOREM PositiveSequencePredecessorFacts ==
  \A length \in Nat:
    \A index \in 2..length:
      /\ index - 1 \in 1..(length - 1)
      /\ (index - 1) + 1 = index
BY SMTT(30)

THEOREM CandidateSequenceIndexAfterNonTargetHead ==
  \A candidate, queue:
    /\ queue \in Seq(Range(queue))
    /\ SequenceHasUniqueValues(queue)
    /\ Len(queue) > 0
    /\ candidate \in SequenceSet(queue)
    /\ Head(queue) # candidate
    => CandidateSequenceIndex(candidate, Tail(queue)) + 1
         = CandidateSequenceIndex(candidate, queue)
PROOF
  <1>1. ASSUME NEW candidate, NEW queue,
                queue \in Seq(Range(queue)),
                SequenceHasUniqueValues(queue),
                Len(queue) > 0,
                candidate \in SequenceSet(queue),
                Head(queue) # candidate
         PROVE CandidateSequenceIndex(candidate, Tail(queue)) + 1
                 = CandidateSequenceIndex(candidate, queue)
    <2> DEFINE Old == CandidateSequenceIndex(candidate, queue)
    <2> DEFINE New == Old - 1
    <2>1. /\ Old \in 1..Len(queue)
           /\ queue[Old] = candidate
      BY <1>1, CandidateSequenceIndexCharacterization DEF Old
    <2>2. /\ queue # <<>>
           /\ Head(queue) = queue[1]
           /\ Len(Tail(queue)) = Len(queue) - 1
           /\ \A index \in 1..Len(Tail(queue)):
                Tail(queue)[index] = queue[index + 1]
      BY <1>1, PositiveSequenceIsNonempty,
         NonemptySequenceHeadIsFirst, HeadTailProperties
    <2>3. Old # 1
      BY <1>1, <2>1, <2>2, Isa
    <2>4. Old \in 2..Len(queue)
      BY <2>1, <2>3, SMT
    <2>5. Len(queue) \in Nat
      BY <1>1, LenProperties
    <2>6. /\ Old - 1 \in 1..(Len(queue) - 1)
           /\ (Old - 1) + 1 = Old
      BY <2>4, <2>5, PositiveSequencePredecessorFacts
    <2>7. /\ New \in 1..Len(Tail(queue))
           /\ New + 1 = Old
      BY <2>2, <2>6 DEF New
    <2>8. Tail(queue)[New] = candidate
      BY <2>1, <2>2, <2>7
    <2>9. /\ Tail(queue) \in Seq(Range(Tail(queue)))
           /\ SequenceHasUniqueValues(Tail(queue))
           /\ candidate \in SequenceSet(Tail(queue))
      <3>1. /\ Tail(queue) \in Seq(Range(queue))
             /\ SequenceHasUniqueValues(Tail(queue))
        BY <1>1, <2>2, UniqueSequenceTailSetFacts,
           HeadTailProperties
      <3>2. Tail(queue) \in Seq(Range(Tail(queue)))
        BY <3>1, RangeEquality
      <3>3. candidate \in SequenceSet(Tail(queue))
        BY <2>7, <2>8 DEF SequenceSet
      <3> QED BY <3>1, <3>2, <3>3
    <2>10. CandidateSequenceIndex(candidate, Tail(queue)) = New
      BY <2>7, <2>8, <2>9,
         CandidateSequenceIndexCharacterization
    <2> QED BY <2>7, <2>10 DEF Old, New
  <1> QED BY <1>1

THEOREM CandidateIoIndexIsPosition ==
  \A candidate, queue:
    /\ AsyncIoSequenceTyped(queue)
    /\ \E index \in AsyncIoConsensusIndices(queue):
         queue[index].candidate = candidate
    => CandidateIoIndex(candidate, queue) \in 1..Len(queue)
BY Isa
   DEF CandidateIoIndex, AsyncIoConsensusIndices,
       AsyncIoSequenceTyped

THEOREM CandidateIoIndexCharacterization ==
  \A candidate, queue, ioReadyQueue, localReadyQueue:
    /\ AsyncIoSequenceTyped(queue)
    /\ AsyncIoConsensusQueueOwnership(
         queue, ioReadyQueue, localReadyQueue)
    /\ \E index \in AsyncIoConsensusIndices(queue):
         queue[index].candidate = candidate
    => /\ CandidateIoIndex(candidate, queue)
              \in AsyncIoConsensusIndices(queue)
       /\ queue[CandidateIoIndex(candidate, queue)].candidate = candidate
       /\ \A index \in AsyncIoConsensusIndices(queue):
            queue[index].candidate = candidate
              => index = CandidateIoIndex(candidate, queue)
PROOF
  <1>1. ASSUME NEW candidate, NEW queue,
                NEW ioReadyQueue, NEW localReadyQueue,
                AsyncIoSequenceTyped(queue),
                AsyncIoConsensusQueueOwnership(
                  queue, ioReadyQueue, localReadyQueue),
                \E index \in AsyncIoConsensusIndices(queue):
                  queue[index].candidate = candidate
         PROVE /\ CandidateIoIndex(candidate, queue)
                       \in AsyncIoConsensusIndices(queue)
               /\ queue[CandidateIoIndex(candidate, queue)].candidate
                    = candidate
               /\ \A index \in AsyncIoConsensusIndices(queue):
                    queue[index].candidate = candidate
                      => index = CandidateIoIndex(candidate, queue)
    <2>1. /\ CandidateIoIndex(candidate, queue)
                    \in AsyncIoConsensusIndices(queue)
           /\ queue[CandidateIoIndex(candidate, queue)].candidate
                = candidate
      BY <1>1, Isa DEF CandidateIoIndex
    <2>2. \A index \in AsyncIoConsensusIndices(queue):
             queue[index].candidate = candidate
               => index = CandidateIoIndex(candidate, queue)
      BY <1>1, <2>1 DEF AsyncIoConsensusQueueOwnership
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM CandidateIoIndexAfterNonTargetHead ==
  \A candidate, queue, ioReadyQueue, localReadyQueue:
    /\ AsyncIoSequenceTyped(queue)
    /\ Len(queue) > 0
    /\ AsyncCompletionSequenceTyped(ioReadyQueue)
    /\ AsyncCompletionSequenceTyped(localReadyQueue)
    /\ AsyncIoConsensusQueueOwnership(
         queue, ioReadyQueue, localReadyQueue)
    /\ \E index \in AsyncIoConsensusIndices(queue):
         queue[index].candidate = candidate
    /\ ~(Head(queue).class = "Consensus"
           /\ Head(queue).candidate = candidate)
    => CandidateIoIndex(candidate, Tail(queue)) + 1
         = CandidateIoIndex(candidate, queue)
PROOF
  <1>1. ASSUME NEW candidate, NEW queue,
                NEW ioReadyQueue, NEW localReadyQueue,
                AsyncIoSequenceTyped(queue),
                Len(queue) > 0,
                AsyncCompletionSequenceTyped(ioReadyQueue),
                AsyncCompletionSequenceTyped(localReadyQueue),
                AsyncIoConsensusQueueOwnership(
                  queue, ioReadyQueue, localReadyQueue),
                \E index \in AsyncIoConsensusIndices(queue):
                  queue[index].candidate = candidate,
                ~(Head(queue).class = "Consensus"
                   /\ Head(queue).candidate = candidate)
         PROVE CandidateIoIndex(candidate, Tail(queue)) + 1
                 = CandidateIoIndex(candidate, queue)
    <2> DEFINE Old == CandidateIoIndex(candidate, queue)
    <2> DEFINE New == Old - 1
    <2>1. /\ Old \in AsyncIoConsensusIndices(queue)
           /\ queue[Old].candidate = candidate
      BY <1>1, CandidateIoIndexCharacterization DEF Old
    <2>2. /\ queue \in Seq(Range(queue))
           /\ queue # <<>>
           /\ Head(queue) = queue[1]
           /\ Len(Tail(queue)) = Len(queue) - 1
           /\ \A index \in 1..Len(Tail(queue)):
                Tail(queue)[index] = queue[index + 1]
      BY <1>1, PositiveSequenceIsNonempty,
         NonemptySequenceHeadIsFirst, HeadTailProperties
         DEF AsyncIoSequenceTyped
    <2>3. /\ Old \in 2..Len(queue)
           /\ New \in 1..Len(Tail(queue))
           /\ Tail(queue)[New] = queue[Old]
      BY <1>1, <2>1, <2>2, Isa
         DEF AsyncIoConsensusIndices, New
    <2>4. New \in AsyncIoConsensusIndices(Tail(queue))
      BY <2>1, <2>3, Isa DEF AsyncIoConsensusIndices
    <2>5. AsyncIoConsensusQueueOwnership(
             Tail(queue),
             AsyncIoReadyAfterService(queue, ioReadyQueue),
             localReadyQueue)
      BY <1>1, ServiceHeadPreservesConsensusQueueOwnership
         DEF AsyncCompletionSequenceTyped
    <2>6. AsyncIoSequenceTyped(Tail(queue))
      BY <1>1, TypedIoTailFacts
    <2>7. CandidateIoIndex(candidate, Tail(queue)) = New
      BY <2>3, <2>4, <2>5, <2>6,
         CandidateIoIndexCharacterization
    <2> QED BY <2>7, Isa DEF Old, New
  <1> QED BY <1>1

(***************************************************************************
Serve recovery work is occurrence-owned.  Candidate equality is deliberately
irrelevant here: the fresh live nonce makes one exact Serve job occur at one
FIFO position, even when another request produces an equal candidate value.
***************************************************************************)

ServeOccurrenceIndex(job, queue) ==
  CHOOSE index \in AsyncIoServeIndices(queue): queue[index] = job

THEOREM ServeOccurrenceIndexCharacterization ==
  \A job, queue:
    /\ AsyncIoSequenceTyped(queue)
    /\ AsyncIoServeNonceOwnership(queue)
    /\ job \in AsyncServeJobSet
    /\ job \in SequenceSet(queue)
    => /\ ServeOccurrenceIndex(job, queue)
              \in AsyncIoServeIndices(queue)
       /\ queue[ServeOccurrenceIndex(job, queue)] = job
       /\ \A index \in AsyncIoServeIndices(queue):
            queue[index] = job
              => index = ServeOccurrenceIndex(job, queue)
PROOF
  <1>1. ASSUME NEW job, NEW queue,
                AsyncIoSequenceTyped(queue),
                AsyncIoServeNonceOwnership(queue),
                job \in AsyncServeJobSet,
                job \in SequenceSet(queue)
         PROVE /\ ServeOccurrenceIndex(job, queue)
                       \in AsyncIoServeIndices(queue)
               /\ queue[ServeOccurrenceIndex(job, queue)] = job
               /\ \A index \in AsyncIoServeIndices(queue):
                    queue[index] = job
                      => index = ServeOccurrenceIndex(job, queue)
    <2>1. job.class = "Serve"
      BY <1>1, Isa DEF AsyncServeJobSet, AsyncIoJob
    <2>2. PICK matching \in 1..Len(queue): queue[matching] = job
      BY <1>1 DEF SequenceSet
    <2>3. matching \in AsyncIoServeIndices(queue)
      BY <2>1, <2>2 DEF AsyncIoServeIndices
    <2>4. /\ ServeOccurrenceIndex(job, queue)
                    \in AsyncIoServeIndices(queue)
           /\ queue[ServeOccurrenceIndex(job, queue)] = job
      BY <2>2, <2>3, Isa DEF ServeOccurrenceIndex
    <2>5. \A index \in AsyncIoServeIndices(queue):
             queue[index] = job
               => index = ServeOccurrenceIndex(job, queue)
      BY <1>1, <2>4 DEF AsyncIoServeNonceOwnership
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM ServeOccurrenceIndexAfterNonTargetHead ==
  \A job, queue:
    /\ AsyncIoSequenceTyped(queue)
    /\ Len(queue) > 0
    /\ AsyncIoServeNonceOwnership(queue)
    /\ job \in AsyncServeJobSet
    /\ job \in SequenceSet(queue)
    /\ Head(queue) # job
    => ServeOccurrenceIndex(job, Tail(queue)) + 1
         = ServeOccurrenceIndex(job, queue)
PROOF
  <1>1. ASSUME NEW job, NEW queue,
                AsyncIoSequenceTyped(queue),
                Len(queue) > 0,
                AsyncIoServeNonceOwnership(queue),
                job \in AsyncServeJobSet,
                job \in SequenceSet(queue),
                Head(queue) # job
         PROVE ServeOccurrenceIndex(job, Tail(queue)) + 1
                 = ServeOccurrenceIndex(job, queue)
    <2> DEFINE Old == ServeOccurrenceIndex(job, queue)
    <2> DEFINE New == Old - 1
    <2>1. /\ Old \in AsyncIoServeIndices(queue)
           /\ queue[Old] = job
      BY <1>1, ServeOccurrenceIndexCharacterization DEF Old
    <2>2. /\ queue \in Seq(Range(queue))
           /\ queue # <<>>
           /\ Head(queue) = queue[1]
           /\ Len(Tail(queue)) = Len(queue) - 1
           /\ \A index \in 1..Len(Tail(queue)):
                Tail(queue)[index] = queue[index + 1]
      BY <1>1, PositiveSequenceIsNonempty,
         NonemptySequenceHeadIsFirst, HeadTailProperties
         DEF AsyncIoSequenceTyped
    <2>3. /\ Old \in 2..Len(queue)
           /\ New \in 1..Len(Tail(queue))
           /\ Tail(queue)[New] = job
      BY <1>1, <2>1, <2>2, Isa
         DEF AsyncIoServeIndices, New
    <2>4. /\ AsyncIoSequenceTyped(Tail(queue))
           /\ AsyncIoServeNonceOwnership(Tail(queue))
           /\ job \in SequenceSet(Tail(queue))
      BY <1>1, <2>3, TypedIoTailFacts,
         TailPreservesServeNonceOwnership, Isa DEF SequenceSet
    <2>5. ServeOccurrenceIndex(job, Tail(queue)) = New
      BY <2>3, <2>4, ServeOccurrenceIndexCharacterization
    <2> QED BY <2>5, Isa DEF Old, New
  <1> QED BY <1>1

THEOREM TailRemovesUniqueServeOccurrence ==
  \A job, queue:
    /\ AsyncIoSequenceTyped(queue)
    /\ Len(queue) > 0
    /\ AsyncIoServeNonceOwnership(queue)
    /\ job \in AsyncServeJobSet
    /\ job \in SequenceSet(queue)
    /\ Head(queue) = job
    => job \notin SequenceSet(Tail(queue))
BY ServeOccurrenceIndexCharacterization, HeadTailProperties,
   NonemptySequenceHeadIsFirst, Isa
   DEF AsyncIoSequenceTyped, AsyncIoServeIndices, SequenceSet

THEOREM ServeJobIndexMatchesOccurrenceIndex ==
  \A node, job:
    ServeJobIndex(node, job) =
      ServeOccurrenceIndex(job, asyncIoQueues[node])
BY DEF ServeJobIndex, ServeOccurrenceIndex

THEOREM ReadyCandidatePositionIsNatural ==
  \A candidate:
    /\ AsyncTypeInvariant
    /\ CandidateScheduled(candidate)
    /\ CandidateInReadyQueue(candidate)
    => ReadyCandidatePosition(candidate) \in Nat
BY CandidateSequenceIndexIsPosition, Isa
   DEF ReadyCandidatePosition, ReadyCandidateSource,
       ReadyCompletionQueue, CandidateInReadyQueue,
       SelectedCompletionSource, SequenceSet,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncIoWorkContentTypeInvariant,
       AsyncCompletionSequenceTyped

THEOREM ScheduledCandidateProjectsToOwner ==
  \A candidate:
    /\ AsyncTypeInvariant
    /\ CandidateScheduled(candidate)
    => /\ candidate.node \in ValidatorIds
       /\ (candidate \in QueuedCandidates
             => candidate \in
                  SequenceSet(asyncCommandQueues[candidate.node]))
       /\ (candidate \in DeferredCandidates
             => candidate \in
                  SequenceSet(
                    asyncDeferredCompletionQueues[candidate.node])
                    \cup SequenceSet(
                          asyncDeferredProgressQueues[candidate.node])
                    \cup SequenceSet(
                          asyncDeferredNormalQueues[candidate.node]))
       /\ (candidate \in CausalCandidates
             => candidate \in
                  SequenceSet(asyncCausalQueues[candidate.node]))
       /\ (candidate \in TrackedWorkCandidates
             => candidate \in asyncOutstandingWork[candidate.node])
BY Isa
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant, AsyncCausalQueueOwnership,
       AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncIoWorkContentTypeInvariant, AsyncDeferredTypeInvariant,
       AsyncDeferredContentTypeInvariant, AsyncCommandQueueOwnership,
       CandidateScheduled, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, SequenceSet

THEOREM DeferredCandidatePositionIsNatural ==
  \A candidate:
    /\ AsyncTypeInvariant
    /\ CandidateScheduled(candidate)
    /\ candidate \in DeferredCandidates
    => DeferredCandidatePosition(candidate) \in Nat
PROOF
  <1>1. ASSUME NEW candidate,
                AsyncTypeInvariant,
                CandidateScheduled(candidate),
                candidate \in DeferredCandidates
         PROVE DeferredCandidatePosition(candidate) \in Nat
    <2>1. /\ candidate.node \in ValidatorIds
           /\ candidate \in
                SequenceSet(
                  asyncDeferredCompletionQueues[candidate.node])
                  \cup SequenceSet(
                        asyncDeferredProgressQueues[candidate.node])
                  \cup SequenceSet(
                        asyncDeferredNormalQueues[candidate.node])
      BY <1>1, ScheduledCandidateProjectsToOwner
    <2>2. /\ AsyncCandidateTyped(candidate)
           /\ asyncNextDeferredClass[candidate.node]
                \in AsyncCommandClasses
           /\ candidate \in SequenceSet(
                DeferredClassQueue(candidate.node, candidate.class))
      BY <1>1, <2>1, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncDeferredTypeInvariant,
             AsyncDeferredTopologyTypeInvariant,
             AsyncDeferredContentTypeInvariant,
             AsyncCompletionSequenceTyped, AsyncQueueTyped,
             DeferredClassQueue, SequenceSet
    <2>3. PICK matching \in
                    1..Len(DeferredClassQueue(
                              candidate.node, candidate.class)):
             DeferredClassQueue(candidate.node, candidate.class)[matching]
               = candidate
      BY <2>2 DEF SequenceSet
    <2>4. matching \in
             DeferredCandidateIndices(candidate.node, candidate)
      BY <2>3 DEF DeferredCandidateIndices
    <2>5. matching \in
             DeferredClassPrefixIndices(candidate.node, candidate)
      BY <2>4 DEF DeferredClassPrefixIndices
    <2>6. DeferredClassPrefixIndices(candidate.node, candidate)
             \subseteq
               1..Len(DeferredClassQueue(
                         candidate.node, candidate.class))
      BY DEF DeferredClassPrefixIndices
    <2>7. IsFiniteSet(
             1..Len(DeferredClassQueue(
                       candidate.node, candidate.class)))
      BY FS_Interval
    <2>8. IsFiniteSet(
             DeferredClassPrefixIndices(candidate.node, candidate))
      BY <2>6, <2>7, FS_Subset
    <2>9. Cardinality(
             DeferredClassPrefixIndices(candidate.node, candidate))
             \in Nat
      BY <2>8, FS_CardinalityType
    <2>10. CommandClassDistance(
              asyncNextDeferredClass[candidate.node], candidate.class)
              \in 0..2
      BY <2>2, SMTT(30)
         DEF AsyncCandidateTyped, AsyncCommandClasses,
             CommandClassDistance, NextCommandClass
    <2> QED BY <2>9, <2>10, SMT
         DEF DeferredCandidatePosition
  <1> QED BY <1>1

THEOREM ScheduledCandidateServiceRankInCarrier ==
  \A candidate:
    /\ AsyncTypeInvariant
    /\ CandidateScheduled(candidate)
    => CandidateServiceRank(candidate) \in OwnedServiceRankCarrier
PROOF
  <1>1. ASSUME NEW candidate,
                AsyncTypeInvariant,
                CandidateScheduled(candidate)
         PROVE CandidateServiceRank(candidate) \in OwnedServiceRankCarrier
    <2>1. /\ candidate.node \in ValidatorIds
           /\ (candidate \in QueuedCandidates
                 => candidate \in SequenceSet(
                      asyncCommandQueues[candidate.node]))
           /\ (candidate \in CausalCandidates
                 => candidate \in SequenceSet(
                      asyncCausalQueues[candidate.node]))
           /\ (candidate \in TrackedWorkCandidates
                 => candidate \in
                      asyncOutstandingWork[candidate.node])
      BY <1>1, ScheduledCandidateProjectsToOwner
    <2>2. AsyncCompletionLoad(candidate.node) \in Nat
      BY <1>1, <2>1, FS_Interval, SMT
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoWorkContentTypeInvariant, AsyncCompletionLoad,
             AsyncOutstandingWorkCount, QueuedCompletionCount,
             DeferredCompletionCount, QueuedCompletionIndices
    <2>3. CandidateInIoQueue(candidate)
             => CandidateIoIndex(
                  candidate, asyncIoQueues[candidate.node]) \in Nat
      BY <1>1, <2>1, CandidateIoIndexIsPosition, SMT
         DEF CandidateInIoQueue, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant
    <2>4. candidate \in CausalCandidates
             => CausalCandidatePosition(candidate) \in Nat
      BY <1>1, <2>1, CandidateSequenceIndexIsPosition, SMT
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncCausalTypeInvariant,
             AsyncQueueTyped, CausalCandidatePosition,
             LocalSourceDistance
    <2>5. CASE candidate \in DeferredCandidates
      BY <1>1, <2>5, DeferredCandidatePositionIsNatural, SMT
         DEF CandidateServiceRank, OwnedServiceRankCarrier
    <2>6. CASE candidate \notin DeferredCandidates
               /\ candidate \in QueuedCandidates
      BY <1>1, <2>1, <2>6, SchedulerClassPrefixRankBound, SMT
         DEF CandidateServiceRank, OwnedServiceRankCarrier
    <2>7. CASE candidate \notin DeferredCandidates
               /\ candidate \notin QueuedCandidates
               /\ CandidateInReadyQueue(candidate)
      BY <1>1, <2>7, ReadyCandidatePositionIsNatural, Isa
         DEF CandidateServiceRank, OwnedServiceRankCarrier
    <2>8. CASE candidate \notin DeferredCandidates
               /\ candidate \notin QueuedCandidates
               /\ ~CandidateInReadyQueue(candidate)
               /\ CandidateInIoQueue(candidate)
      BY <2>3, <2>8, SMT
         DEF CandidateServiceRank, OwnedServiceRankCarrier
    <2>9. CASE candidate \notin DeferredCandidates
               /\ candidate \notin QueuedCandidates
               /\ ~CandidateInReadyQueue(candidate)
               /\ ~CandidateInIoQueue(candidate)
               /\ candidate \in
                    asyncOutstandingWork[candidate.node]
      BY <2>2, <2>9, SMT
         DEF CandidateServiceRank, OwnedServiceRankCarrier
    <2>10. CASE candidate \notin DeferredCandidates
                /\ candidate \notin QueuedCandidates
                /\ ~CandidateInReadyQueue(candidate)
                /\ ~CandidateInIoQueue(candidate)
                /\ candidate \notin
                     asyncOutstandingWork[candidate.node]
                /\ candidate \in CausalCandidates
      BY <2>4, <2>10, SMT
         DEF CandidateServiceRank, OwnedServiceRankCarrier
    <2>11. CASE candidate \notin DeferredCandidates
                /\ candidate \notin QueuedCandidates
                /\ ~CandidateInReadyQueue(candidate)
                /\ ~CandidateInIoQueue(candidate)
                /\ candidate \notin
                     asyncOutstandingWork[candidate.node]
                /\ candidate \notin CausalCandidates
      BY <1>1, <2>1, <2>11
         DEF CandidateScheduled, TrackedWorkCandidates
    <2> QED BY <2>5, <2>6, <2>7, <2>8, <2>9, <2>10, <2>11
  <1> QED BY <1>1

ProtectedOwnedAtServiceRank(candidate, rank) ==
  /\ gst
  /\ ResponsiveProtectedCandidateOwned(candidate)
  /\ CandidateServiceRank(candidate) = rank

ProtectedServiceOwnershipExit(candidate) ==
  ~ResponsiveProtectedCandidateOwned(candidate)

THEOREM ProtectedRankExitHasWellFoundedSuccessor ==
  \A candidate:
    \A rank \in OwnedServiceRankCarrier:
      /\ AsyncTypeInvariant
      /\ gst
      /\ ~ProtectedServiceOwnershipExit(candidate)
      /\ ServiceRankLess(CandidateServiceRank(candidate), rank)
      => \E lower \in SetLessThan(
                       rank, OwnedServiceRankOrdering,
                       OwnedServiceRankCarrier):
           ProtectedOwnedAtServiceRank(candidate, lower)
PROOF
  <1>1. ASSUME NEW candidate,
                NEW rank \in OwnedServiceRankCarrier,
                AsyncTypeInvariant,
                gst,
                ~ProtectedServiceOwnershipExit(candidate),
                ServiceRankLess(CandidateServiceRank(candidate), rank)
         PROVE \E lower \in SetLessThan(
                         rank, OwnedServiceRankOrdering,
                         OwnedServiceRankCarrier):
                 ProtectedOwnedAtServiceRank(candidate, lower)
    <2>1. CandidateScheduled(candidate)
      BY <1>1
         DEF ProtectedServiceOwnershipExit,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned
    <2>2. CandidateServiceRank(candidate) \in OwnedServiceRankCarrier
      BY <1>1, <2>1, ScheduledCandidateServiceRankInCarrier
    <2>3. <<CandidateServiceRank(candidate), rank>>
             \in OwnedServiceRankOrdering
      BY <1>1, <2>2, OwnedServiceRankOrderingMatchesLess
    <2>4. CandidateServiceRank(candidate)
             \in SetLessThan(
                  rank, OwnedServiceRankOrdering,
                  OwnedServiceRankCarrier)
      BY <2>2, <2>3 DEF SetLessThan
    <2> QED BY <1>1, <2>4
         DEF ProtectedOwnedAtServiceRank,
             ProtectedServiceOwnershipExit
  <1> QED BY <1>1

THEOREM ProtectedRankProgressSuppliesWellFoundedStep ==
  \A initialContext, candidate:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedServiceRankProgressProperty(
         AsyncSpecAt(initialContext))
    => \A rank \in OwnedServiceRankCarrier:
         ProtectedOwnedAtServiceRank(candidate, rank)
           ~> (ProtectedServiceOwnershipExit(candidate)
                \/ \E lower \in SetLessThan(
                     rank, OwnedServiceRankOrdering,
                     OwnedServiceRankCarrier):
                     ProtectedOwnedAtServiceRank(candidate, lower))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW candidate,
                AsyncSpecAt(initialContext),
                ProtectedServiceRankProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE \A rank \in OwnedServiceRankCarrier:
                 ProtectedOwnedAtServiceRank(candidate, rank)
                   ~> (ProtectedServiceOwnershipExit(candidate)
                        \/ \E lower \in SetLessThan(
                             rank, OwnedServiceRankOrdering,
                             OwnedServiceRankCarrier):
                             ProtectedOwnedAtServiceRank(
                               candidate, lower))
    <2>1. ASSUME NEW rank \in OwnedServiceRankCarrier
           PROVE ProtectedOwnedAtServiceRank(candidate, rank)
                   ~> (ProtectedServiceOwnershipExit(candidate)
                        \/ \E lower \in SetLessThan(
                             rank, OwnedServiceRankOrdering,
                             OwnedServiceRankCarrier):
                             ProtectedOwnedAtServiceRank(
                               candidate, lower))
      <3>1. PICK stage \in 2..6, position \in Nat:
               rank = <<stage, position>>
        BY <2>1 DEF OwnedServiceRankCarrier
      <3>2. ProtectedOwnedAtServiceRank(candidate, rank)
               ~> (ProtectedServiceOwnershipExit(candidate)
                    \/ ServiceRankLess(
                         CandidateServiceRank(candidate), rank))
        BY <1>1, <3>1
           DEF ProtectedServiceRankProgressProperty,
               ProtectedOwnedAtServiceRank,
               ProtectedServiceOwnershipExit
      <3>3. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
        BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
           AsyncStrongTypeProjectsAsyncType, PTL
      <3>4. AsyncSpecAt(initialContext) => [](gst => []gst)
        BY <1>1, AsyncSpecKeepsGstOnceSet
      <3>5. /\ AsyncTypeInvariant
               /\ gst
               /\ ~ProtectedServiceOwnershipExit(candidate)
               /\ ServiceRankLess(
                    CandidateServiceRank(candidate), rank)
              => \E lower \in SetLessThan(
                   rank, OwnedServiceRankOrdering,
                   OwnedServiceRankCarrier):
                   ProtectedOwnedAtServiceRank(candidate, lower)
        BY <2>1, ProtectedRankExitHasWellFoundedSuccessor
      <3> QED BY <1>1, <3>2, <3>3, <3>4, <3>5, PTL
    <2> QED BY <2>1
  <1> QED BY <1>1

(***************************************************************************
One-height temporal composition.

The one-height asynchronous instance freezes its context, GST is monotone,
and the responsive voter projection therefore remains exactly the voter set
of the caller-supplied context.  These facts compose the independently proved
rotating-leader and application properties without adding a fairness or
completion assumption.
***************************************************************************)

THEOREM FrozenContextFixesResponsiveVoters ==
  \A initialContext:
    AsyncFrozenContextAt(initialContext)
      => AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
BY SMT DEF AsyncFrozenContextAt, AsyncCurrentResponsiveVoters,
           AsyncVotersAt, CurrentVoters, CurrentEpoch

THEOREM ResponsiveProtectedOwnerUsesFairNode ==
  \A initialContext, candidate:
    /\ AsyncFrozenContextAt(initialContext)
    /\ ResponsiveProtectedCandidateOwned(candidate)
    => candidate.node \in AsyncVotersAt(initialContext)
BY FrozenContextFixesResponsiveVoters
   DEF ResponsiveProtectedCandidateOwned

THEOREM AsyncSpecAlwaysUsesFixedResponsiveVoters ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => [](AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => [](AsyncCurrentResponsiveVoters
                         = AsyncVotersAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
            => []AsyncFrozenContextAt(initialContext)
      BY AsyncSpecAlwaysKeepsFrozenContext
    <2>2. AsyncFrozenContextAt(initialContext)
            => AsyncCurrentResponsiveVoters
                 = AsyncVotersAt(initialContext)
      BY FrozenContextFixesResponsiveVoters
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

(***************************************************************************
Application-completion temporal closure.

The substantive recovery/validation/application pipeline remains the explicit
`ApplicationCompletionProgressObligation` below.  This section proves only the
finite temporal composition which that per-validator obligation entails.  An
application receipt is durable under every Core step, the one-height async
refinement preserves that durability, and induction over the finite validator
identifier prefix therefore closes the aggregate responsive-application
clause.  No global apply barrier or additional fairness premise is introduced.
***************************************************************************)

ResponsiveApplicationPrefixAt(initialContext, limit) ==
  \A node \in AsyncVotersAt(initialContext) \cap (0..limit):
    NodeHasApplication(node)

THEOREM CoreBracketStepPreservesNodeApplication ==
  \A node:
    NodeHasApplication(node)
      /\ [Next]_vars
      => NodeHasApplication(node)'
PROOF
  <1>1. ASSUME NEW node,
                NodeHasApplication(node),
                [Next]_vars
         PROVE NodeHasApplication(node)'
    <2>1. CASE UNCHANGED vars
      BY <1>1, <2>1, Isa DEF NodeHasApplication, vars
    <2>2. CASE Next
      <3>1. UNCHANGED context
        BY <2>2, CoreNextLeavesContext
      <3>2. \/ UNCHANGED <<decisions, applied>>
             \/ (\E request \in pendingDecision:
                   PersistDecision(request))
             \/ (\E owner \in ValidatorIds,
                        qc \in DecisionQcValues:
                   ApplyDecision(owner, qc))
        BY <2>2, NextDurableReceiptActionClassification
      <3>3. CASE UNCHANGED <<decisions, applied>>
        BY <1>1, <3>1, <3>3, Isa DEF NodeHasApplication
      <3>4. CASE \E request \in pendingDecision:
                    PersistDecision(request)
        <4>1. PICK request \in pendingDecision:
                 PersistDecision(request)
          BY <3>4
        <4> QED BY <1>1, <3>1, <4>1, Isa
             DEF PersistDecision, NodeHasApplication
      <3>5. CASE \E owner \in ValidatorIds,
                        qc \in DecisionQcValues:
                    ApplyDecision(owner, qc)
        <4>1. PICK owner \in ValidatorIds,
                     qc \in DecisionQcValues:
                 ApplyDecision(owner, qc)
          BY <3>5
        <4> QED BY <1>1, <3>1, <4>1, Isa
             DEF ApplyDecision, NodeHasApplication
      <3> QED BY <3>2, <3>3, <3>4, <3>5
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncBracketStepPreservesNodeApplication ==
  \A node:
    NodeHasApplication(node)
      /\ [AsyncNext]_AsyncAllVars
      => NodeHasApplication(node)'
PROOF
  <1>1. ASSUME NEW node,
                NodeHasApplication(node),
                [AsyncNext]_AsyncAllVars
         PROVE NodeHasApplication(node)'
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>1, Isa
         DEF NodeHasApplication, AsyncAllVars, AsyncSchedulerVars, vars
    <2>2. CASE AsyncNext
      <3>1. [Next]_vars
        BY <2>2, AsyncStepRefinementObligation
      <3> QED BY <1>1, <3>1,
           CoreBracketStepPreservesNodeApplication
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM ResponsiveApplicationPrefixAtIsStable ==
  \A initialContext:
    \A limit \in Nat:
      ResponsiveApplicationPrefixAt(initialContext, limit)
        /\ [AsyncNext]_AsyncAllVars
        => ResponsiveApplicationPrefixAt(initialContext, limit)'
BY Isa, AsyncBracketStepPreservesNodeApplication
   DEF ResponsiveApplicationPrefixAt

THEOREM FrozenContextFullApplicationPrefixImpliesResponsiveApply ==
  \A initialContext:
    /\ ModelConfiguration
    /\ AsyncFrozenContextAt(initialContext)
    /\ ResponsiveApplicationPrefixAt(initialContext, N - 1)
    => ResponsiveNodesApply
BY FrozenContextFixesResponsiveVoters, Isa
   DEF ResponsiveApplicationPrefixAt, ResponsiveNodesApply,
       AsyncVotersAt, ValidatorIds, ModelConfiguration,
       QuorumConfiguration

THEOREM ApplicationCompletionProgressAppliesFixedResponsiveNode ==
  \A initialContext:
    \A node \in AsyncVotersAt(initialContext):
      /\ AsyncSpecAt(initialContext)
      /\ ApplicationCompletionProgressProperty(
           AsyncSpecAt(initialContext))
      => (gst /\ ResponsiveNodesDecide)
           ~> NodeHasApplication(node)
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node \in AsyncVotersAt(initialContext),
                AsyncSpecAt(initialContext),
                ApplicationCompletionProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE (gst /\ ResponsiveNodesDecide)
                 ~> NodeHasApplication(node)
    <2>1. [](AsyncCurrentResponsiveVoters
               = AsyncVotersAt(initialContext))
      BY <1>1, AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>2. \A currentNode \in AsyncCurrentResponsiveVoters:
             (gst /\ NodeHasDecision(currentNode))
               ~> NodeHasApplication(currentNode)
      BY <1>1, PTL DEF ApplicationCompletionProgressProperty
    <2>3. (gst /\ NodeHasDecision(node))
             ~> NodeHasApplication(node)
      BY <1>1, <2>1, <2>2, PTL
    <2>4. []((gst /\ ResponsiveNodesDecide)
                => (gst /\ NodeHasDecision(node)))
      BY <1>1, <2>1, PTL DEF ResponsiveNodesDecide
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM ApplicationCompletionReachesEveryResponsivePrefix ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ApplicationCompletionProgressProperty(
         AsyncSpecAt(initialContext))
    => \A limit \in Nat:
         (gst /\ ResponsiveNodesDecide)
           ~> ResponsiveApplicationPrefixAt(initialContext, limit)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                ApplicationCompletionProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE \A limit \in Nat:
                 (gst /\ ResponsiveNodesDecide)
                   ~> ResponsiveApplicationPrefixAt(
                        initialContext, limit)
    <2> DEFINE P(limit) ==
           (gst /\ ResponsiveNodesDecide)
             ~> ResponsiveApplicationPrefixAt(initialContext, limit)
    <2>1. P(0)
      <3>1. CASE 0 \in AsyncVotersAt(initialContext)
        <4>1. (gst /\ ResponsiveNodesDecide)
                 ~> NodeHasApplication(0)
          BY <1>1, <3>1,
             ApplicationCompletionProgressAppliesFixedResponsiveNode
        <4> QED BY <4>1, PTL
             DEF P, ResponsiveApplicationPrefixAt
      <3>2. CASE 0 \notin AsyncVotersAt(initialContext)
        BY <3>2, PTL DEF P, ResponsiveApplicationPrefixAt
      <3> QED BY <3>1, <3>2
    <2>2. ASSUME NEW limit \in Nat,
                  P(limit)
           PROVE P(limit + 1)
      <3>1. CASE limit + 1 \in AsyncVotersAt(initialContext)
        <4>1. (gst /\ ResponsiveNodesDecide)
                 ~> NodeHasApplication(limit + 1)
          BY <1>1, <3>1,
             ApplicationCompletionProgressAppliesFixedResponsiveNode
        <4>2. ResponsiveApplicationPrefixAt(initialContext, limit)
                 /\ [AsyncNext]_AsyncAllVars
                 => ResponsiveApplicationPrefixAt(
                      initialContext, limit)'
          BY <2>2, ResponsiveApplicationPrefixAtIsStable
        <4>3. NodeHasApplication(limit + 1)
                 /\ [AsyncNext]_AsyncAllVars
                 => NodeHasApplication(limit + 1)'
          BY AsyncBracketStepPreservesNodeApplication
        <4>4. ResponsiveApplicationPrefixAt(
                 initialContext, limit + 1)
                 <=> /\ ResponsiveApplicationPrefixAt(
                           initialContext, limit)
                     /\ NodeHasApplication(limit + 1)
          BY <2>2, <3>1, Isa DEF ResponsiveApplicationPrefixAt
        <4> QED BY <2>2, <4>1, <4>2, <4>3, <4>4, PTL DEF P
      <3>2. CASE limit + 1 \notin AsyncVotersAt(initialContext)
        <4>1. ResponsiveApplicationPrefixAt(initialContext, limit)
                 => ResponsiveApplicationPrefixAt(
                      initialContext, limit + 1)
          BY <2>2, <3>2, Isa DEF ResponsiveApplicationPrefixAt
        <4> QED BY <2>2, <4>1, PTL DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A limit \in Nat: P(limit)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

THEOREM ModelConfigurationMakesLastValidatorNatural ==
  ModelConfiguration => N - 1 \in Nat
BY SMT DEF ModelConfiguration, QuorumConfiguration

THEOREM ApplicationCompletionProgressImpliesAggregateApplication ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ApplicationCompletionProgressProperty(
         AsyncSpecAt(initialContext))
    => (gst /\ ResponsiveNodesDecide) ~> ResponsiveNodesApply
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                ApplicationCompletionProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE (gst /\ ResponsiveNodesDecide) ~> ResponsiveNodesApply
    <2>1. ModelConfiguration
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant, PTL
         DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
             Safety, TypeInvariant
    <2>2. N - 1 \in Nat
      BY <2>1, ModelConfigurationMakesLastValidatorNatural
    <2>3. (gst /\ ResponsiveNodesDecide)
             ~> ResponsiveApplicationPrefixAt(initialContext, N - 1)
      BY <1>1, <2>2,
         ApplicationCompletionReachesEveryResponsivePrefix
    <2>4. []AsyncFrozenContextAt(initialContext)
      BY <1>1, AsyncSpecAlwaysKeepsFrozenContext
    <2>5. [](ResponsiveApplicationPrefixAt(initialContext, N - 1)
               => ResponsiveNodesApply)
      BY <2>1, <2>4,
         FrozenContextFullApplicationPrefixImpliesResponsiveApply, PTL
    <2> QED BY <2>3, <2>5, PTL
  <1> QED BY <1>1

(***************************************************************************
Stage 5: exact Consensus-I/O FIFO service.

The rank ignores Serve/Control candidate identities but counts their physical
FIFO positions.  `CandidateIoIndex` therefore selects the unique Consensus
occurrence, not an arbitrary I/O job carrying an equal record.  Servicing an
earlier head shifts that occurrence left by one; servicing the occurrence
itself moves it to stage 4.  No enqueue action inserts before an existing
job, so the target rank cannot increase between fair worker occurrences.
***************************************************************************)

ProtectedRankProgressExit(candidate, rank) ==
  \/ ProtectedServiceOwnershipExit(candidate)
  \/ ServiceRankLess(CandidateServiceRank(candidate), rank)

ProtectedStage5Pending(candidate, position) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ ProtectedOwnedAtServiceRank(candidate, <<5, position>>)

THEOREM ProtectedStage5CarrierFacts ==
  \A candidate, position:
    ProtectedStage5Pending(candidate, position)
      => /\ candidate.node \in AsyncCurrentResponsiveVoters
         /\ candidate \in asyncOutstandingWork[candidate.node]
         /\ CandidateInIoQueue(candidate)
         /\ CandidateIoIndex(
              candidate, asyncIoQueues[candidate.node]) = position
         /\ AsyncIoQueueDepth(candidate.node) > 0
         /\ AsyncIoSequenceTyped(asyncIoQueues[candidate.node])
         /\ AsyncIoConsensusQueueOwnership(
              asyncIoQueues[candidate.node],
              asyncIoReadyCompletions[candidate.node],
              asyncLocalReadyCompletions[candidate.node])
BY CandidateIoIndexCharacterization, Isa
   DEF ProtectedStage5Pending, ProtectedOwnedAtServiceRank,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CandidateInIoQueue,
       AsyncProgressOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant,
       AsyncIoContentTypeInvariant, AsyncIoQueueContentTypeInvariant,
       AsyncIoWorkContentTypeInvariant,
       AsyncIoConsensusCandidateOwnership,
       ConsensusIoCandidates, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, CandidateScheduled,
       CandidateInReadyQueue, SequenceSet, AsyncIoQueueDepth

THEOREM QueuedIoServiceIsNonstuttering ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ AsyncIoQueueDepth(node) > 0
    /\ PostGstServiceIoWorker(node)
    => <<PostGstServiceIoWorker(node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                AsyncIoQueueDepth(node) > 0,
                PostGstServiceIoWorker(node)
         PROVE <<PostGstServiceIoWorker(node)>>_AsyncAllVars
    <2>1. /\ node \in ValidatorIds
           /\ AsyncIoSequenceTyped(asyncIoQueues[node])
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
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

THEOREM ProtectedStage5EnablesFairWorker ==
  \A candidate, position:
    ProtectedStage5Pending(candidate, position)
      => ENABLED
           <<PostGstServiceIoWorker(candidate.node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                ProtectedStage5Pending(candidate, position)
         PROVE ENABLED
                 <<PostGstServiceIoWorker(candidate.node)>>_AsyncAllVars
    <2>1. /\ AsyncTypeInvariant
           /\ gst
           /\ candidate.node \in AsyncCurrentResponsiveVoters
           /\ AsyncIoQueueDepth(candidate.node) > 0
      BY <1>1, ProtectedStage5CarrierFacts,
         AsyncStrongTypeProjectsAsyncType
         DEF ProtectedStage5Pending, ProtectedOwnedAtServiceRank
    <2>2. ENABLED PostGstServiceIoWorker(candidate.node)
      BY <2>1, QueuedIoEnablesPostGstService
    <2>3. PostGstServiceIoWorker(candidate.node)
             => <<PostGstServiceIoWorker(candidate.node)>>_AsyncAllVars
      BY <2>1, QueuedIoServiceIsNonstuttering
    <2>4. ENABLED
             <<PostGstServiceIoWorker(candidate.node)>>_AsyncAllVars
      BY <2>2, <2>3, ENABLEDaxioms
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM ProtectedStage5WorkerStrictlyProgresses ==
  \A candidate, position:
    /\ ProtectedStage5Pending(candidate, position)
    /\ PostGstServiceIoWorker(candidate.node)
    => ProtectedRankProgressExit(candidate, <<5, position>>)'
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                ProtectedStage5Pending(candidate, position),
                PostGstServiceIoWorker(candidate.node)
         PROVE ProtectedRankProgressExit(
                 candidate, <<5, position>>)'
    <2> DEFINE Queue == asyncIoQueues[candidate.node]
    <2>1. /\ AsyncTypeInvariant
           /\ candidate.node \in AsyncCurrentResponsiveVoters
           /\ candidate \in asyncOutstandingWork[candidate.node]
           /\ CandidateInIoQueue(candidate)
           /\ CandidateIoIndex(candidate, Queue) = position
           /\ AsyncIoQueueDepth(candidate.node) > 0
           /\ AsyncIoSequenceTyped(Queue)
           /\ AsyncIoConsensusQueueOwnership(
                Queue, asyncIoReadyCompletions[candidate.node],
                asyncLocalReadyCompletions[candidate.node])
      BY <1>1, ProtectedStage5CarrierFacts,
         AsyncStrongTypeProjectsAsyncType DEF Queue
    <2>2. /\ asyncIoQueues'[candidate.node] = Tail(Queue)
           /\ asyncOutstandingWork'[candidate.node]
                = asyncOutstandingWork[candidate.node]
           /\ asyncCommandQueues' = asyncCommandQueues
           /\ asyncCausalQueues' = asyncCausalQueues
           /\ asyncDeferredCompletionQueues'
                = asyncDeferredCompletionQueues
           /\ asyncDeferredProgressQueues'
                = asyncDeferredProgressQueues
           /\ asyncDeferredNormalQueues' = asyncDeferredNormalQueues
      BY <1>1, Isa
         DEF PostGstServiceIoWorker, ServiceIoWorker,
             LeaveCausalQueues, AsyncDeferredVars, vars, Queue
    <2>3. CASE /\ Head(Queue).class = "Consensus"
                   /\ Head(Queue).candidate = candidate
      <3>1. Head(Queue).class = "Consensus"
        BY <2>3
      <3>2. candidate \in
               SequenceSet(asyncIoReadyCompletions'[candidate.node])
        BY <1>1, <2>3, <3>1, SequenceSetAfterAppend, Isa
           DEF PostGstServiceIoWorker, ServiceIoWorker, Queue
      <3>3. CandidateServiceRank(candidate)'[1] = 4
        BY <1>1, <2>1, <2>2, <3>2, Isa
           DEF CandidateServiceRank, CandidateInReadyQueue,
               QueuedCandidates, DeferredCandidates, CausalCandidates,
               TrackedWorkCandidates, SequenceSet
      <3> QED BY <3>3, Isa
           DEF ProtectedRankProgressExit,
               ServiceRankLess
    <2>4. CASE ~(Head(Queue).class = "Consensus"
                   /\ Head(Queue).candidate = candidate)
      <3>1. CandidateIoIndex(candidate, Tail(Queue)) + 1 = position
        BY <1>1, <2>1, <2>4,
           CandidateIoIndexAfterNonTargetHead
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
               AsyncIoWorkContentTypeInvariant, Queue
      <3>2. /\ CandidateInIoQueue(candidate)'
             /\ CandidateIoIndex(
                  candidate, asyncIoQueues'[candidate.node]) < position
        BY <2>1, <2>2, <3>1, Isa
           DEF CandidateInIoQueue, AsyncIoConsensusIndices, Queue
      <3>3. /\ ~ProtectedServiceOwnershipExit(candidate)'
                 => CandidateServiceRank(candidate)' =
                      <<5, CandidateIoIndex(
                              candidate,
                              asyncIoQueues'[candidate.node])>>
        BY <1>1, <2>1, <2>2, <3>2, Isa
           DEF ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               CandidateInReadyQueue, QueuedCandidates,
               DeferredCandidates, CausalCandidates,
               TrackedWorkCandidates, CandidateScheduled, SequenceSet
      <3> QED BY <3>2, <3>3, Isa
           DEF ProtectedRankProgressExit, ServiceRankLess
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM ProtectedStage5UnlessProgress ==
  \A candidate, position:
    ProtectedStage5Pending(candidate, position)
      /\ [AsyncNext]_AsyncAllVars
      => ProtectedStage5Pending(candidate, position)'
           \/ ProtectedRankProgressExit(candidate, <<5, position>>)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   CandidateIoIndexAfterNonTargetHead,
   AppendProperties, HeadTailProperties, Isa
   DEF ProtectedStage5Pending, ProtectedOwnedAtServiceRank,
       ProtectedRankProgressExit, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess, CandidateInIoQueue,
       CandidateInReadyQueue, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, CandidateScheduled,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer, OpenHistoricalRecovery,
       LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
       IngressDrainStep, DrainFairIngressSelected,
       SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncNetworkStep, AdmitIngressPacket, AsyncFaultStep,
       PreGstCrash, EnqueueCandidate, AppendCausalSuccessors,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       SequenceWithoutIndex, DeferCommand, DiscardCommand,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, ConsensusIoCandidates,
       SequenceHasUniqueValues, SequenceSet, AsyncIoConsensusIndices,
       AsyncAllVars

THEOREM FairProtectedStage5RankDescent ==
  \A initialContext, candidate:
    \A position \in Nat:
      AsyncSpecAt(initialContext)
        => (ProtectedOwnedAtServiceRank(candidate, <<5, position>>)
              ~> ProtectedRankProgressExit(candidate, <<5, position>>))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position \in Nat
         PROVE AsyncSpecAt(initialContext)
                 => (ProtectedOwnedAtServiceRank(
                       candidate, <<5, position>>)
                       ~> ProtectedRankProgressExit(
                            candidate, <<5, position>>))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
    <2>2. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>3. ProtectedStage5Pending(candidate, position)
               /\ ~ProtectedRankProgressExit(
                    candidate, <<5, position>>)
              => ENABLED
                   <<PostGstServiceIoWorker(candidate.node)>>_AsyncAllVars
      BY ProtectedStage5EnablesFairWorker
    <2>4. /\ ProtectedStage5Pending(candidate, position)
             /\ ~ProtectedRankProgressExit(candidate, <<5, position>>)
             /\ <<PostGstServiceIoWorker(
                    candidate.node)>>_AsyncAllVars
            => ProtectedRankProgressExit(
                 candidate, <<5, position>>)'
      BY ProtectedStage5WorkerStrictlyProgresses, Isa
         DEF ProtectedStage5Pending,
             ProtectedRankProgressExit
    <2>5. ProtectedStage5Pending(candidate, position)
               /\ [AsyncNext]_AsyncAllVars
              => ProtectedStage5Pending(candidate, position)'
                   \/ ProtectedRankProgressExit(
                        candidate, <<5, position>>)'
      BY ProtectedStage5UnlessProgress
    <2>6. CASE candidate.node \in AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(
                    PostGstServiceIoWorker(candidate.node))
        BY <2>6 DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (ProtectedStage5Pending(candidate, position)
                     ~> ProtectedRankProgressExit(
                          candidate, <<5, position>>))
        BY <2>3, <2>4, <2>5, <3>1, PTL DEF AsyncSpecAt
      <3> QED BY <3>2
    <2>7. CASE candidate.node \notin AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => []~ProtectedOwnedAtServiceRank(
                      candidate, <<5, position>>)
        BY <2>2, <2>7, PTL
           DEF ProtectedOwnedAtServiceRank,
               ResponsiveProtectedCandidateOwned
      <3>2. AsyncSpecAt(initialContext)
               => (ProtectedStage5Pending(candidate, position)
                     ~> ProtectedRankProgressExit(
                          candidate, <<5, position>>))
        BY <3>1, PTL DEF ProtectedStage5Pending
      <3> QED BY <3>2
    <2>8. AsyncSpecAt(initialContext)
             => (ProtectedStage5Pending(candidate, position)
                   ~> ProtectedRankProgressExit(
                        candidate, <<5, position>>))
      BY <2>6, <2>7
    <2>9. AsyncSpecAt(initialContext)
             => (ProtectedOwnedAtServiceRank(candidate, <<5, position>>)
                   ~> ProtectedStage5Pending(candidate, position))
      BY <2>1, PTL DEF ProtectedStage5Pending
    <2> QED BY <2>8, <2>9, PTL
  <1> QED BY <1>1

(***************************************************************************
Stage 5 Serve occurrences use the same physical worker FIFO but a different
identity rule from Consensus work.  A live nonce selects exactly one queue
position.  Appends therefore preserve that position, while servicing an
earlier head lowers it and servicing the occurrence itself ends ownership.
The separate proof is required because candidate equality may legitimately
hold between a Serve request and unrelated scheduler work.
***************************************************************************)

ProtectedServeOwnedAtServiceRank(node, job, rank) ==
  /\ gst
  /\ ResponsiveProtectedServeJobOwned(node, job)
  /\ ServeJobRank(node, job) = rank

ProtectedServeOwnershipExit(node, job) ==
  ~ResponsiveProtectedServeJobOwned(node, job)

ProtectedServeRankProgressExit(node, job, rank) ==
  \/ ProtectedServeOwnershipExit(node, job)
  \/ ServiceRankLess(ServeJobRank(node, job), rank)

ProtectedServeStage5Pending(node, job, position) ==
  /\ AsyncStrongTypeInvariant
  /\ ProtectedServeOwnedAtServiceRank(node, job, <<5, position>>)

THEOREM ProtectedServeStage5CarrierFacts ==
  \A node, job, position:
    ProtectedServeStage5Pending(node, job, position)
      => /\ node \in AsyncCurrentResponsiveVoters
         /\ job \in AsyncServeJobSet
         /\ job \in SequenceSet(asyncIoQueues[node])
         /\ ServeOccurrenceIndex(job, asyncIoQueues[node]) = position
         /\ AsyncIoQueueDepth(node) > 0
         /\ AsyncIoSequenceTyped(asyncIoQueues[node])
         /\ AsyncIoServeNonceOwnership(asyncIoQueues[node])
BY ServeOccurrenceIndexCharacterization,
   ServeJobIndexMatchesOccurrenceIndex, Isa
   DEF ProtectedServeStage5Pending,
       ProtectedServeOwnedAtServiceRank,
       ResponsiveProtectedServeJobOwned, ServeJobRank,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncIoQueueContentTypeInvariant, AsyncIoQueueDepth,
       SequenceSet

THEOREM ProtectedServeStage5EnablesFairWorker ==
  \A node, job, position:
    ProtectedServeStage5Pending(node, job, position)
      => ENABLED <<PostGstServiceIoWorker(node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW node, NEW job, NEW position,
                ProtectedServeStage5Pending(node, job, position)
         PROVE ENABLED
                 <<PostGstServiceIoWorker(node)>>_AsyncAllVars
    <2>1. /\ AsyncTypeInvariant
           /\ gst
           /\ node \in AsyncCurrentResponsiveVoters
           /\ AsyncIoQueueDepth(node) > 0
      BY <1>1, ProtectedServeStage5CarrierFacts,
         AsyncStrongTypeProjectsAsyncType
         DEF ProtectedServeStage5Pending,
             ProtectedServeOwnedAtServiceRank
    <2>2. ENABLED PostGstServiceIoWorker(node)
      BY <2>1, QueuedIoEnablesPostGstService
    <2>3. PostGstServiceIoWorker(node)
             => <<PostGstServiceIoWorker(node)>>_AsyncAllVars
      BY <2>1, QueuedIoServiceIsNonstuttering
    <2> QED BY <2>2, <2>3, ENABLEDaxioms
  <1> QED BY <1>1

THEOREM ProtectedServeStage5WorkerStrictlyProgresses ==
  \A node, job, position:
    /\ ProtectedServeStage5Pending(node, job, position)
    /\ PostGstServiceIoWorker(node)
    => ProtectedServeRankProgressExit(
         node, job, <<5, position>>)'
PROOF
  <1>1. ASSUME NEW node, NEW job, NEW position,
                ProtectedServeStage5Pending(node, job, position),
                PostGstServiceIoWorker(node)
         PROVE ProtectedServeRankProgressExit(
                 node, job, <<5, position>>)'
    <2> DEFINE Queue == asyncIoQueues[node]
    <2>1. /\ job \in AsyncServeJobSet
           /\ job \in SequenceSet(Queue)
           /\ ServeOccurrenceIndex(job, Queue) = position
           /\ AsyncIoQueueDepth(node) > 0
           /\ AsyncIoSequenceTyped(Queue)
           /\ AsyncIoServeNonceOwnership(Queue)
      BY <1>1, ProtectedServeStage5CarrierFacts DEF Queue
    <2>2. asyncIoQueues'[node] = Tail(Queue)
      BY <1>1 DEF PostGstServiceIoWorker, ServiceIoWorker, Queue
    <2>3. CASE Head(Queue) = job
      <3>1. job \notin SequenceSet(Tail(Queue))
        BY <2>1, <2>3, TailRemovesUniqueServeOccurrence
      <3>2. ProtectedServeOwnershipExit(node, job)'
        BY <2>2, <3>1, Isa
           DEF ProtectedServeOwnershipExit,
               ResponsiveProtectedServeJobOwned
      <3> QED BY <3>2 DEF ProtectedServeRankProgressExit
    <2>4. CASE Head(Queue) # job
      <3>1. ServeOccurrenceIndex(job, Tail(Queue)) + 1 = position
        BY <2>1, <2>4, ServeOccurrenceIndexAfterNonTargetHead
      <3>2. /\ ~ProtectedServeOwnershipExit(node, job)'
                    => ServeJobRank(node, job)' =
                         <<5, ServeOccurrenceIndex(
                                job, Tail(Queue))>>
        BY <1>1, <2>2, ServeJobIndexMatchesOccurrenceIndex, Isa
           DEF ProtectedServeOwnershipExit,
               ResponsiveProtectedServeJobOwned, ServeJobRank
      <3>3. ProtectedServeRankProgressExit(
               node, job, <<5, position>>)'
        BY <3>1, <3>2, Isa
           DEF ProtectedServeRankProgressExit, ServiceRankLess
      <3> QED BY <3>3
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM ProtectedServeStage5UnlessProgress ==
  \A node, job, position:
    ProtectedServeStage5Pending(node, job, position)
      /\ [AsyncNext]_AsyncAllVars
      => ProtectedServeStage5Pending(node, job, position)'
           \/ ProtectedServeRankProgressExit(
                node, job, <<5, position>>)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   ServeOccurrenceIndexAfterNonTargetHead,
   TailRemovesUniqueServeOccurrence,
   ServeJobIndexMatchesOccurrenceIndex,
   AppendProperties, HeadTailProperties, Isa
   DEF ProtectedServeStage5Pending,
       ProtectedServeOwnedAtServiceRank,
       ProtectedServeRankProgressExit,
       ProtectedServeOwnershipExit,
       ResponsiveProtectedServeJobOwned,
       ServeJobRank, ServiceRankLess,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer, OpenHistoricalRecovery,
       LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
       IngressDrainStep, DrainFairIngressSelected,
       DrainHistoricalIngressSelected,
       SerializedRuntimeStep, RuntimeStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncNetworkStep, AdmitIngressPacket, AsyncFaultStep,
       PreGstCrash, AsyncIoCertifiedServeJob,
       AsyncIoServeNonceOwnership, SequenceSet,
       AsyncAllVars

THEOREM FairProtectedServeStage5RankDescent ==
  \A initialContext, node, job:
    \A position \in Nat:
      AsyncSpecAt(initialContext)
        => (ProtectedServeOwnedAtServiceRank(node, job, <<5, position>>)
              ~> ProtectedServeRankProgressExit(node, job, <<5, position>>))
PROOF
  <1>1. ASSUME NEW initialContext, NEW node, NEW job,
                NEW position \in Nat
         PROVE AsyncSpecAt(initialContext)
                 => (ProtectedServeOwnedAtServiceRank(
                       node, job, <<5, position>>)
                       ~> ProtectedServeRankProgressExit(
                            node, job, <<5, position>>))
    <2>1. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>2. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>3. ProtectedServeStage5Pending(node, job, position)
               /\ ~ProtectedServeRankProgressExit(
                    node, job, <<5, position>>)
              => ENABLED
                   <<PostGstServiceIoWorker(node)>>_AsyncAllVars
      BY ProtectedServeStage5EnablesFairWorker
    <2>4. /\ ProtectedServeStage5Pending(node, job, position)
             /\ ~ProtectedServeRankProgressExit(
                  node, job, <<5, position>>)
             /\ <<PostGstServiceIoWorker(node)>>_AsyncAllVars
            => ProtectedServeRankProgressExit(
                 node, job, <<5, position>>)'
      BY ProtectedServeStage5WorkerStrictlyProgresses, Isa
         DEF ProtectedServeStage5Pending,
             ProtectedServeRankProgressExit
    <2>5. ProtectedServeStage5Pending(node, job, position)
               /\ [AsyncNext]_AsyncAllVars
              => ProtectedServeStage5Pending(node, job, position)'
                   \/ ProtectedServeRankProgressExit(
                        node, job, <<5, position>>)'
      BY ProtectedServeStage5UnlessProgress
    <2>6. CASE node \in AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(PostGstServiceIoWorker(node))
        BY <2>6 DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (ProtectedServeStage5Pending(node, job, position)
                     ~> ProtectedServeRankProgressExit(
                          node, job, <<5, position>>))
        BY <2>3, <2>4, <2>5, <3>1, PTL DEF AsyncSpecAt
      <3> QED BY <3>2
    <2>7. CASE node \notin AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => []~ProtectedServeOwnedAtServiceRank(
                      node, job, <<5, position>>)
        BY <2>2, <2>7, PTL
           DEF ProtectedServeOwnedAtServiceRank,
               ResponsiveProtectedServeJobOwned
      <3>2. AsyncSpecAt(initialContext)
               => (ProtectedServeStage5Pending(node, job, position)
                     ~> ProtectedServeRankProgressExit(
                          node, job, <<5, position>>))
        BY <3>1, PTL DEF ProtectedServeStage5Pending,
                            ProtectedServeOwnedAtServiceRank
      <3> QED BY <3>2
    <2>8. AsyncSpecAt(initialContext)
             => (ProtectedServeStage5Pending(node, job, position)
                   ~> ProtectedServeRankProgressExit(
                        node, job, <<5, position>>))
      BY <2>6, <2>7
    <2>9. AsyncSpecAt(initialContext)
             => (ProtectedServeOwnedAtServiceRank(
                   node, job, <<5, position>>)
                   ~> ProtectedServeStage5Pending(
                        node, job, position))
      BY <2>1, PTL DEF ProtectedServeStage5Pending
    <2> QED BY <2>8, <2>9, PTL
  <1> QED BY <1>1

THEOREM ProtectedServeRankProgressFromFairFifo ==
  \A initialContext:
    ProtectedServeRankProgressProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ProtectedServeRankProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node \in AsyncCurrentResponsiveVoters,
                     job \in AsyncServeJobSet, position \in Nat:
              (gst
                /\ ResponsiveProtectedServeJobOwned(node, job)
                /\ ServeJobRank(node, job) = <<5, position>>)
                ~> (~ResponsiveProtectedServeJobOwned(node, job)
                     \/ ServiceRankLess(
                          ServeJobRank(node, job), <<5, position>>))
      <3>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                    NEW job \in AsyncServeJobSet,
                    NEW position \in Nat
             PROVE (gst
                      /\ ResponsiveProtectedServeJobOwned(node, job)
                      /\ ServeJobRank(node, job) = <<5, position>>)
                      ~> (~ResponsiveProtectedServeJobOwned(node, job)
                           \/ ServiceRankLess(
                                ServeJobRank(node, job), <<5, position>>))
        <4>1. AsyncSpecAt(initialContext)
                 => (ProtectedServeOwnedAtServiceRank(
                       node, job, <<5, position>>)
                       ~> ProtectedServeRankProgressExit(
                            node, job, <<5, position>>))
          BY FairProtectedServeStage5RankDescent
        <4>2. ProtectedServeOwnedAtServiceRank(
                 node, job, <<5, position>>)
                 ~> ProtectedServeRankProgressExit(
                      node, job, <<5, position>>)
          BY <2>1, <4>1, PTL
        <4> QED BY <4>2, PTL
             DEF ProtectedServeOwnedAtServiceRank,
                 ProtectedServeRankProgressExit,
                 ProtectedServeOwnershipExit
      <3> QED BY <3>1
    <2> QED BY <2>1 DEF ProtectedServeRankProgressProperty
  <1> QED BY <1>1

(***************************************************************************
Serve ownership has only the FIFO position component of the stage-5 rank.
The fresh live nonce makes that position a natural-number occurrence index.
Consequently the Serve rank-progress property supplies a genuine step in the
well-founded natural ordering; it is not necessary to assume the stronger
Serve-starvation conclusion separately when composing scheduler starvation.
***************************************************************************)

THEOREM ResponsiveProtectedServeJobPositionIsNatural ==
  \A node, job:
    /\ AsyncStrongTypeInvariant
    /\ ResponsiveProtectedServeJobOwned(node, job)
    => ServeJobIndex(node, job) \in Nat
BY ServeOccurrenceIndexCharacterization,
   ServeJobIndexMatchesOccurrenceIndex, Isa
   DEF ResponsiveProtectedServeJobOwned,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncIoQueueContentTypeInvariant, AsyncIoServeIndices

THEOREM ResponsiveProtectedServeJobHasRankPosition ==
  \A node, job:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ ResponsiveProtectedServeJobOwned(node, job)
    => \E position \in Nat:
         ProtectedServeOwnedAtServiceRank(
           node, job, <<5, position>>)
PROOF
  <1>1. ASSUME NEW node, NEW job,
                AsyncStrongTypeInvariant,
                gst,
                ResponsiveProtectedServeJobOwned(node, job)
         PROVE \E position \in Nat:
                 ProtectedServeOwnedAtServiceRank(
                   node, job, <<5, position>>)
    <2>1. ServeJobIndex(node, job) \in Nat
      BY <1>1, ResponsiveProtectedServeJobPositionIsNatural
    <2>2. WITNESS ServeJobIndex(node, job) \in Nat
    <2> QED BY <1>1 DEF ProtectedServeOwnedAtServiceRank, ServeJobRank
  <1> QED BY <1>1

THEOREM ProtectedServeRankExitHasWellFoundedSuccessor ==
  \A node, job:
    \A position \in Nat:
      /\ AsyncStrongTypeInvariant
      /\ gst
      /\ ~ProtectedServeOwnershipExit(node, job)
      /\ ServiceRankLess(
           ServeJobRank(node, job), <<5, position>>)
      => \E lower \in SetLessThan(
                       position, OpToRel(<, Nat), Nat):
           ProtectedServeOwnedAtServiceRank(
             node, job, <<5, lower>>)
PROOF
  <1>1. ASSUME NEW node, NEW job, NEW position \in Nat,
                AsyncStrongTypeInvariant,
                gst,
                ~ProtectedServeOwnershipExit(node, job),
                ServiceRankLess(
                  ServeJobRank(node, job), <<5, position>>)
         PROVE \E lower \in SetLessThan(
                          position, OpToRel(<, Nat), Nat):
                 ProtectedServeOwnedAtServiceRank(
                   node, job, <<5, lower>>)
    <2>1. ServeJobIndex(node, job) \in Nat
      BY <1>1, ResponsiveProtectedServeJobPositionIsNatural
         DEF ProtectedServeOwnershipExit
    <2>2. ServeJobIndex(node, job) < position
      BY <1>1, SMT DEF ServeJobRank, ServiceRankLess
    <2>3. ServeJobIndex(node, job)
             \in SetLessThan(position, OpToRel(<, Nat), Nat)
      BY <2>1, <2>2, SMT DEF SetLessThan, OpToRel
    <2>4. WITNESS ServeJobIndex(node, job)
    <2> QED BY <1>1, <2>3
         DEF ProtectedServeOwnedAtServiceRank,
             ProtectedServeOwnershipExit, ServeJobRank
  <1> QED BY <1>1

THEOREM ProtectedServeRankProgressSuppliesWellFoundedPositionStep ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedServeRankProgressProperty(AsyncSpecAt(initialContext))
    => \A node \in AsyncCurrentResponsiveVoters,
          job \in AsyncServeJobSet:
         \A position \in Nat:
           ProtectedServeOwnedAtServiceRank(
             node, job, <<5, position>>)
             ~> (ProtectedServeOwnershipExit(node, job)
                  \/ \E lower \in SetLessThan(
                       position, OpToRel(<, Nat), Nat):
                       ProtectedServeOwnedAtServiceRank(
                         node, job, <<5, lower>>))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                ProtectedServeRankProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE \A node \in AsyncCurrentResponsiveVoters,
                   job \in AsyncServeJobSet:
                 \A position \in Nat:
                   ProtectedServeOwnedAtServiceRank(
                     node, job, <<5, position>>)
                     ~> (ProtectedServeOwnershipExit(node, job)
                          \/ \E lower \in SetLessThan(
                               position, OpToRel(<, Nat), Nat):
                               ProtectedServeOwnedAtServiceRank(
                                 node, job, <<5, lower>>))
    <2>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                    NEW job \in AsyncServeJobSet
           PROVE \A position \in Nat:
                   ProtectedServeOwnedAtServiceRank(
                     node, job, <<5, position>>)
                     ~> (ProtectedServeOwnershipExit(node, job)
                          \/ \E lower \in SetLessThan(
                               position, OpToRel(<, Nat), Nat):
                               ProtectedServeOwnedAtServiceRank(
                                 node, job, <<5, lower>>))
      <3>1. ASSUME NEW position \in Nat
             PROVE ProtectedServeOwnedAtServiceRank(
                       node, job, <<5, position>>)
                       ~> (ProtectedServeOwnershipExit(node, job)
                            \/ \E lower \in SetLessThan(
                                 position, OpToRel(<, Nat), Nat):
                                 ProtectedServeOwnedAtServiceRank(
                                   node, job, <<5, lower>>))
        <4>1. ProtectedServeOwnedAtServiceRank(
                 node, job, <<5, position>>)
                 ~> (ProtectedServeOwnershipExit(node, job)
                      \/ ServiceRankLess(
                           ServeJobRank(node, job), <<5, position>>))
          BY <1>1, <2>1
             DEF ProtectedServeRankProgressProperty,
                 ProtectedServeOwnedAtServiceRank,
                 ProtectedServeOwnershipExit
        <4>2. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
          BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
        <4>3. AsyncSpecAt(initialContext) => [](gst => []gst)
          BY <1>1, AsyncSpecKeepsGstOnceSet
        <4>4. /\ AsyncStrongTypeInvariant
                 /\ gst
                 /\ ~ProtectedServeOwnershipExit(node, job)
                 /\ ServiceRankLess(
                      ServeJobRank(node, job), <<5, position>>)
                => \E lower \in SetLessThan(
                     position, OpToRel(<, Nat), Nat):
                     ProtectedServeOwnedAtServiceRank(
                       node, job, <<5, lower>>)
          BY <3>1, ProtectedServeRankExitHasWellFoundedSuccessor
        <4> QED BY <1>1, <4>1, <4>2, <4>3, <4>4, PTL
      <3> QED BY <3>1
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM BoxedResponsiveProtectedServeJobHasRankPosition ==
  \A node, job:
    [](/\ AsyncStrongTypeInvariant
       /\ gst
       /\ ResponsiveProtectedServeJobOwned(node, job)
       => \E position \in Nat:
            ProtectedServeOwnedAtServiceRank(
              node, job, <<5, position>>))
PROOF
  <1>1. []ResponsiveProtectedServeJobHasRankPosition
    BY ResponsiveProtectedServeJobHasRankPosition, PTL
  <1>2. \A node, job:
          [](/\ AsyncStrongTypeInvariant
             /\ gst
             /\ ResponsiveProtectedServeJobOwned(node, job)
             => \E position \in Nat:
                  ProtectedServeOwnedAtServiceRank(
                    node, job, <<5, position>>))
    BY <1>1, Isa
  <1> QED BY <1>2

THEOREM ProtectedServeNatQuantifierEquivalence ==
  \A node, job:
    (\A position \in Nat:
       ProtectedServeOwnedAtServiceRank(
         node, job, <<5, position>>)
         => <>ProtectedServeOwnershipExit(node, job))
      <=> ((\E position \in Nat:
               ProtectedServeOwnedAtServiceRank(
                 node, job, <<5, position>>))
             => <>ProtectedServeOwnershipExit(node, job))
BY SMT

THEOREM AlwaysProtectedServeNatQuantifierEquivalence ==
  []ProtectedServeNatQuantifierEquivalence
BY ProtectedServeNatQuantifierEquivalence, PTL

THEOREM BoxedProtectedServeNatQuantifierEquivalence ==
  \A node, job:
    [](\A position \in Nat:
         ProtectedServeOwnedAtServiceRank(
           node, job, <<5, position>>)
           => <>ProtectedServeOwnershipExit(node, job))
      <=> []((\E position \in Nat:
                ProtectedServeOwnedAtServiceRank(
                  node, job, <<5, position>>))
               => <>ProtectedServeOwnershipExit(node, job))
PROOF
  <1>1. \A node, job:
          []((\A position \in Nat:
                ProtectedServeOwnedAtServiceRank(
                  node, job, <<5, position>>)
                  => <>ProtectedServeOwnershipExit(node, job))
             <=> ((\E position \in Nat:
                      ProtectedServeOwnedAtServiceRank(
                        node, job, <<5, position>>))
                    => <>ProtectedServeOwnershipExit(node, job)))
    BY AlwaysProtectedServeNatQuantifierEquivalence, Isa
  <1>2. ASSUME NEW node, NEW job
         PROVE [](\A position \in Nat:
                    ProtectedServeOwnedAtServiceRank(
                      node, job, <<5, position>>)
                      => <>ProtectedServeOwnershipExit(node, job))
                 <=> []((\E position \in Nat:
                            ProtectedServeOwnedAtServiceRank(
                              node, job, <<5, position>>))
                           => <>ProtectedServeOwnershipExit(node, job))
    <2>1. []((\A position \in Nat:
                ProtectedServeOwnedAtServiceRank(
                  node, job, <<5, position>>)
                  => <>ProtectedServeOwnershipExit(node, job))
               <=> ((\E position \in Nat:
                        ProtectedServeOwnedAtServiceRank(
                          node, job, <<5, position>>))
                      => <>ProtectedServeOwnershipExit(node, job)))
      BY <1>1, SMT
    <2> QED BY <2>1, PTL
  <1> QED BY <1>2

THEOREM ProtectedServeLowerQuantifierEquivalence ==
  \A node, job, position:
    (\A lower \in SetLessThan(
       position, OpToRel(<, Nat), Nat):
       ProtectedServeOwnedAtServiceRank(
         node, job, <<5, lower>>)
         => <>ProtectedServeOwnershipExit(node, job))
      <=> ((\E lower \in SetLessThan(
               position, OpToRel(<, Nat), Nat):
               ProtectedServeOwnedAtServiceRank(
                 node, job, <<5, lower>>))
             => <>ProtectedServeOwnershipExit(node, job))
BY SMT

THEOREM AlwaysProtectedServeLowerQuantifierEquivalence ==
  []ProtectedServeLowerQuantifierEquivalence
BY ProtectedServeLowerQuantifierEquivalence, PTL

THEOREM BoxedProtectedServeLowerQuantifierEquivalence ==
  \A node, job, position:
    [](\A lower \in SetLessThan(
         position, OpToRel(<, Nat), Nat):
         ProtectedServeOwnedAtServiceRank(
           node, job, <<5, lower>>)
           => <>ProtectedServeOwnershipExit(node, job))
      <=> []((\E lower \in SetLessThan(
                  position, OpToRel(<, Nat), Nat):
                  ProtectedServeOwnedAtServiceRank(
                    node, job, <<5, lower>>))
                => <>ProtectedServeOwnershipExit(node, job))
PROOF
  <1>1. \A node, job, position:
          []((\A lower \in SetLessThan(
               position, OpToRel(<, Nat), Nat):
                ProtectedServeOwnedAtServiceRank(
                  node, job, <<5, lower>>)
                  => <>ProtectedServeOwnershipExit(node, job))
             <=> ((\E lower \in SetLessThan(
                      position, OpToRel(<, Nat), Nat):
                      ProtectedServeOwnedAtServiceRank(
                        node, job, <<5, lower>>))
                    => <>ProtectedServeOwnershipExit(node, job)))
    BY AlwaysProtectedServeLowerQuantifierEquivalence, Isa
  <1>2. ASSUME NEW node, NEW job, NEW position
         PROVE [](\A lower \in SetLessThan(
                    position, OpToRel(<, Nat), Nat):
                    ProtectedServeOwnedAtServiceRank(
                      node, job, <<5, lower>>)
                      => <>ProtectedServeOwnershipExit(node, job))
                 <=> []((\E lower \in SetLessThan(
                              position, OpToRel(<, Nat), Nat):
                            ProtectedServeOwnedAtServiceRank(
                              node, job, <<5, lower>>))
                           => <>ProtectedServeOwnershipExit(node, job))
    <2>1. []((\A lower \in SetLessThan(
                 position, OpToRel(<, Nat), Nat):
                ProtectedServeOwnedAtServiceRank(
                  node, job, <<5, lower>>)
                  => <>ProtectedServeOwnershipExit(node, job))
               <=> ((\E lower \in SetLessThan(
                        position, OpToRel(<, Nat), Nat):
                        ProtectedServeOwnedAtServiceRank(
                          node, job, <<5, lower>>))
                      => <>ProtectedServeOwnershipExit(node, job)))
      BY <1>1, SMT
    <2> QED BY <2>1, PTL
  <1> QED BY <1>2

THEOREM ProtectedServeRankComposition ==
  \A node, job, position:
    /\ ProtectedServeOwnedAtServiceRank(
         node, job, <<5, position>>)
         ~> (ProtectedServeOwnershipExit(node, job)
              \/ \E lower \in SetLessThan(
                   position, OpToRel(<, Nat), Nat):
                   ProtectedServeOwnedAtServiceRank(
                     node, job, <<5, lower>>))
    /\ \A lower \in SetLessThan(
         position, OpToRel(<, Nat), Nat):
         ProtectedServeOwnedAtServiceRank(
           node, job, <<5, lower>>)
           ~> ProtectedServeOwnershipExit(node, job)
    => ProtectedServeOwnedAtServiceRank(
         node, job, <<5, position>>)
         ~> ProtectedServeOwnershipExit(node, job)
PROOF
  <1>1. ASSUME NEW node, NEW job, NEW position
         PROVE
           /\ ProtectedServeOwnedAtServiceRank(
                node, job, <<5, position>>)
                ~> (ProtectedServeOwnershipExit(node, job)
                     \/ \E lower \in SetLessThan(
                          position, OpToRel(<, Nat), Nat):
                          ProtectedServeOwnedAtServiceRank(
                            node, job, <<5, lower>>))
           /\ \A lower \in SetLessThan(
                position, OpToRel(<, Nat), Nat):
                ProtectedServeOwnedAtServiceRank(
                  node, job, <<5, lower>>)
                  ~> ProtectedServeOwnershipExit(node, job)
           => ProtectedServeOwnedAtServiceRank(
                node, job, <<5, position>>)
                ~> ProtectedServeOwnershipExit(node, job)
    <2>1. (\A lower \in SetLessThan(
                 position, OpToRel(<, Nat), Nat):
                 [](ProtectedServeOwnedAtServiceRank(
                      node, job, <<5, lower>>)
                    => <>ProtectedServeOwnershipExit(node, job)))
            <=> [](\A lower \in SetLessThan(
                         position, OpToRel(<, Nat), Nat):
                         ProtectedServeOwnedAtServiceRank(
                           node, job, <<5, lower>>)
                           => <>ProtectedServeOwnershipExit(node, job))
      OBVIOUS
    <2>2. [](\A lower \in SetLessThan(
                  position, OpToRel(<, Nat), Nat):
                  ProtectedServeOwnedAtServiceRank(
                    node, job, <<5, lower>>)
                    => <>ProtectedServeOwnershipExit(node, job))
            <=> []((\E lower \in SetLessThan(
                         position, OpToRel(<, Nat), Nat):
                         ProtectedServeOwnedAtServiceRank(
                           node, job, <<5, lower>>))
                       => <>ProtectedServeOwnershipExit(node, job))
      BY BoxedProtectedServeLowerQuantifierEquivalence, SMT
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM ProtectedServeWellFoundedRankConvergence ==
  ASSUME NEW node, NEW job,
         \A position \in Nat:
           ProtectedServeOwnedAtServiceRank(
             node, job, <<5, position>>)
             ~> (ProtectedServeOwnershipExit(node, job)
                  \/ \E lower \in SetLessThan(
                       position, OpToRel(<, Nat), Nat):
                       ProtectedServeOwnedAtServiceRank(
                         node, job, <<5, lower>>))
  PROVE \A position \in Nat:
          ProtectedServeOwnedAtServiceRank(
            node, job, <<5, position>>)
            ~> ProtectedServeOwnershipExit(node, job)
PROOF
  <1> DEFINE H(position) ==
               ProtectedServeOwnedAtServiceRank(
                 node, job, <<5, position>>)
                 ~> ProtectedServeOwnershipExit(node, job)
             LT(position) ==
               ProtectedServeOwnedAtServiceRank(
                 node, job, <<5, position>>)
                 ~> (ProtectedServeOwnershipExit(node, job)
                      \/ \E lower \in SetLessThan(
                           position, OpToRel(<, Nat), Nat):
                           ProtectedServeOwnedAtServiceRank(
                             node, job, <<5, lower>>))
  <1>1. \A position \in Nat:
           /\ LT(position)
           /\ \A lower \in SetLessThan(
                position, OpToRel(<, Nat), Nat): H(lower)
           => H(position)
    BY ONLY ProtectedServeRankComposition, SMT DEF H, LT
  <1>2. IsWellFoundedOn(OpToRel(<, Nat), Nat)
    BY NatLessThanWellFounded
  <1>3. \A position \in Nat: LT(position)
    BY DEF LT
  <1> HIDE DEF H, LT
  <1>4. \A position \in Nat:
           (\A lower \in SetLessThan(
              position, OpToRel(<, Nat), Nat): H(lower))
             => H(position)
    BY ONLY <1>1, <1>3, SMT
  <1>5. \A position \in Nat: H(position)
    BY ONLY <1>2, <1>4, WFInduction, IsaM("blast")
  <1> QED BY <1>5 DEF H

THEOREM ProtectedServeRankExistentialLift ==
  ASSUME NEW node, NEW job,
         \A position \in Nat:
           ProtectedServeOwnedAtServiceRank(
             node, job, <<5, position>>)
             ~> ProtectedServeOwnershipExit(node, job)
  PROVE (\E position \in Nat:
           ProtectedServeOwnedAtServiceRank(
             node, job, <<5, position>>))
          ~> ProtectedServeOwnershipExit(node, job)
PROOF
  <1>1. (\A position \in Nat:
            [](ProtectedServeOwnedAtServiceRank(
                 node, job, <<5, position>>)
               => <>ProtectedServeOwnershipExit(node, job)))
          <=> [](\A position \in Nat:
                   ProtectedServeOwnedAtServiceRank(
                     node, job, <<5, position>>)
                     => <>ProtectedServeOwnershipExit(node, job))
    OBVIOUS
  <1>2. [](\A position \in Nat:
             ProtectedServeOwnedAtServiceRank(
               node, job, <<5, position>>)
               => <>ProtectedServeOwnershipExit(node, job))
          <=> []((\E position \in Nat:
                     ProtectedServeOwnedAtServiceRank(
                       node, job, <<5, position>>))
                   => <>ProtectedServeOwnershipExit(node, job))
    BY ONLY BoxedProtectedServeNatQuantifierEquivalence, SMT
  <1> QED BY <1>1, <1>2, PTL

THEOREM ResponsiveProtectedServeJobHasRankPositionAt ==
  ASSUME NEW node, NEW job
  PROVE /\ AsyncStrongTypeInvariant
        /\ gst
        /\ ResponsiveProtectedServeJobOwned(node, job)
        => \E position \in Nat:
             ProtectedServeOwnedAtServiceRank(
               node, job, <<5, position>>)
BY ONLY ResponsiveProtectedServeJobHasRankPosition, SMT

THEOREM ProtectedServeRankProgressImpliesStarvation ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedServeRankProgressProperty(AsyncSpecAt(initialContext))
    => ProtectedServeStarvationProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                ProtectedServeRankProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE ProtectedServeStarvationProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                    NEW job \in AsyncServeJobSet
           PROVE (gst /\ ResponsiveProtectedServeJobOwned(node, job))
                   ~> ~ResponsiveProtectedServeJobOwned(node, job)
      <3>1. \A position \in Nat:
               ProtectedServeOwnedAtServiceRank(
                 node, job, <<5, position>>)
                 ~> (ProtectedServeOwnershipExit(node, job)
                      \/ \E lower \in SetLessThan(
                           position, OpToRel(<, Nat), Nat):
                           ProtectedServeOwnedAtServiceRank(
                             node, job, <<5, lower>>))
        BY <1>1, <2>1,
           ProtectedServeRankProgressSuppliesWellFoundedPositionStep
      <3>2. \A position \in Nat:
               ProtectedServeOwnedAtServiceRank(
                 node, job, <<5, position>>)
                 ~> ProtectedServeOwnershipExit(node, job)
        BY ONLY <3>1, ProtectedServeWellFoundedRankConvergence, SMT
      <3>3. (\E position \in Nat:
                 ProtectedServeOwnedAtServiceRank(
                   node, job, <<5, position>>))
                 ~> ProtectedServeOwnershipExit(node, job)
        BY ONLY <3>2, ProtectedServeRankExistentialLift, SMT
      <3>4. []AsyncStrongTypeInvariant
        BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
      <3>5. [](/\ AsyncStrongTypeInvariant
                /\ gst
                /\ ResponsiveProtectedServeJobOwned(node, job)
                => \E position \in Nat:
                     ProtectedServeOwnedAtServiceRank(
                       node, job, <<5, position>>))
        BY ResponsiveProtectedServeJobHasRankPositionAt, PTL
      <3>6. [](gst /\ ResponsiveProtectedServeJobOwned(node, job)
                 => \E position \in Nat:
                      ProtectedServeOwnedAtServiceRank(
                        node, job, <<5, position>>))
        BY <3>4, <3>5, PTL
      <3> QED BY <3>3, <3>6, PTL
           DEF ProtectedServeOwnershipExit
    <2> QED BY <1>1, <2>1 DEF ProtectedServeStarvationProperty
  <1> QED BY <1>1

THEOREM ProtectedServiceRankProgressImpliesStarvation ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedServiceRanksProgressProperty(AsyncSpecAt(initialContext))
    => StarvationFreedomProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                ProtectedServiceRanksProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE StarvationFreedomProperty(AsyncSpecAt(initialContext))
    <2>1. ASSUME NEW candidate \in AsyncCandidateSet
           PROVE (gst /\ ResponsiveProtectedCandidateOwned(candidate))
                   ~> ~ResponsiveProtectedCandidateOwned(candidate)
      <3>1. \A rank \in OwnedServiceRankCarrier:
               ProtectedOwnedAtServiceRank(candidate, rank)
                 ~> (ProtectedServiceOwnershipExit(candidate)
                      \/ \E lower \in SetLessThan(
                           rank, OwnedServiceRankOrdering,
                           OwnedServiceRankCarrier):
                           ProtectedOwnedAtServiceRank(candidate, lower))
        BY <1>1, ProtectedRankProgressSuppliesWellFoundedStep
           DEF ProtectedServiceRanksProgressProperty
      <3>2. \A rank \in OwnedServiceRankCarrier:
               ProtectedOwnedAtServiceRank(candidate, rank)
                 ~> ProtectedServiceOwnershipExit(candidate)
        BY <3>1, OwnedServiceRankOrderingWellFounded,
           WellFoundedLeadsTo
      <3>3. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
        BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
           AsyncStrongTypeProjectsAsyncType, PTL
      <3>4. AsyncTypeInvariant
               /\ gst
               /\ ResponsiveProtectedCandidateOwned(candidate)
              => \E rank \in OwnedServiceRankCarrier:
                   ProtectedOwnedAtServiceRank(candidate, rank)
        BY ScheduledCandidateServiceRankInCarrier
           DEF ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, ProtectedOwnedAtServiceRank
      <3> QED BY <1>1, <3>2, <3>3, <3>4, PTL
           DEF ProtectedServiceOwnershipExit
    <2>2. ProtectedServeStarvationProperty(AsyncSpecAt(initialContext))
      BY <1>1, ProtectedServeRankProgressImpliesStarvation
         DEF ProtectedServiceRanksProgressProperty
    <2> QED BY <1>1, <2>1, <2>2 DEF StarvationFreedomProperty
  <1> QED BY <1>1

(***************************************************************************
Stage 4: producer-ready completion service.

A ready completion can be blocked only by a full runtime queue.  The nested
auxiliary rank follows the exact runtime priority order: establishing sticky
FIFO debt outranks all later blockers; satisfying the one-view timeout debt
outranks tags/deferred work; consuming a tag dominates the at-most-one drain
bit it may set; and ordinary Local/Ingress runner turns decrease the final
`RuntimeReachRank` component.  A FIFO execution opens a Completion slot and
leaves the runner in Local, where the next fair RunNode occurrence either
admits a producer completion or services causal debt; both strictly lower the
stage-4 position.
***************************************************************************)

ReadyTimeoutDebt(node) ==
  IF NodeHasDecision(node)
       \/ NodeTimedOut(node, nodeView[node])
       \/ asyncTimeoutEmitted[node]
       \/ "TimeoutElapsed" \in asyncOutstandingTags[node]
  THEN 0
  ELSE IF asyncNow < asyncNodeDeadlines[node]
       THEN asyncNodeDeadlines[node] - asyncNow + 1
       ELSE 1

ReadyDeferredCount(node) ==
  Len(asyncDeferredCompletionQueues[node])
    + Len(asyncDeferredProgressQueues[node])
    + Len(asyncDeferredNormalQueues[node])

ReadyTagCount(node) ==
  Cardinality(
    asyncOutstandingTags[node]
      \cap {"TimeoutElapsed", "RetransmitElapsed"})

ReadyTagDrainDebt(node) ==
  2 * ReadyTagCount(node)
    + (IF asyncDeferredDrainOwed[node] THEN 1 ELSE 0)

ReadyFifoDebt(node) ==
  IF NodeQueueNonempty(node) /\ ~asyncFifoOwed[node] THEN 1 ELSE 0

ReadyRunInnerRank(node) ==
  <<ReadyTagDrainDebt(node), RuntimeReachRank(node)>>

ReadyRunTimeoutRank(node) ==
  <<ReadyTimeoutDebt(node), ReadyRunInnerRank(node)>>

ReadyRunDeferredRank(node) ==
  <<ReadyDeferredCount(node), ReadyRunTimeoutRank(node)>>

ReadyRunAuxRank(node) ==
  <<ReadyFifoDebt(node), ReadyRunDeferredRank(node)>>

ReadyRunInnerCarrier == Nat \X Nat
ReadyRunTimeoutCarrier == Nat \X ReadyRunInnerCarrier
ReadyRunDeferredCarrier == Nat \X ReadyRunTimeoutCarrier
ReadyRunAuxCarrier == (0..1) \X ReadyRunDeferredCarrier

ReadyRunInnerOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)

ReadyRunTimeoutOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), ReadyRunInnerOrdering,
    Nat, ReadyRunInnerCarrier)

ReadyRunDeferredOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), ReadyRunTimeoutOrdering,
    Nat, ReadyRunTimeoutCarrier)

ReadyRunAuxOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), ReadyRunDeferredOrdering,
    0..1, ReadyRunDeferredCarrier)

THEOREM ReadyRunAuxOrderingIsWellFounded ==
  IsWellFoundedOn(ReadyRunAuxOrdering, ReadyRunAuxCarrier)
PROOF
  <1>1. IsWellFoundedOn(OpToRel(<, Nat), 0..1)
    BY NatLessThanWellFounded, IsWellFoundedOnSubset, Isa
  <1>2. IsWellFoundedOn(
           ReadyRunInnerOrdering, ReadyRunInnerCarrier)
    BY NatLessThanWellFounded, WFLexPairOrdering
       DEF ReadyRunInnerOrdering, ReadyRunInnerCarrier
  <1>3. IsWellFoundedOn(
           ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier)
    BY NatLessThanWellFounded, <1>2, WFLexPairOrdering
       DEF ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier
  <1>4. IsWellFoundedOn(
           ReadyRunDeferredOrdering, ReadyRunDeferredCarrier)
    BY NatLessThanWellFounded, <1>3, WFLexPairOrdering
       DEF ReadyRunDeferredOrdering, ReadyRunDeferredCarrier
  <1> QED BY <1>1, <1>4, WFLexPairOrdering
       DEF ReadyRunAuxOrdering, ReadyRunAuxCarrier

THEOREM ReadyRunAuxRankInCarrier ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant => ReadyRunAuxRank(node) \in ReadyRunAuxCarrier
BY RuntimeReachRankWithinRunnerCycle, FS_Intersection, FS_CardinalityType,
   Isa
   DEF ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxCarrier, ReadyRunDeferredCarrier,
       ReadyRunTimeoutCarrier, ReadyRunInnerCarrier, ReadyFifoDebt,
       ReadyTimeoutDebt, ReadyTagDrainDebt, ReadyTagCount,
       ReadyDeferredCount,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncDeferredTypeInvariant, AsyncDeferredTopologyTypeInvariant,
       AsyncDeferredContentTypeInvariant, NodeQueueNonempty

(***************************************************************************
Non-Completion causal debt reserves the exact class prefix that the causal
head needs.  The debt is one above the number of serialized removals still
required: when the head is blocked, a FIFO runtime step lowers this quantity
by one, and the producer/unique-ingress gates prevent an outer source from
refilling the released command slot before the causal head is admitted.
***************************************************************************)

CausalHeadCommandLimit(node) ==
  LET candidate == HeadCausalCandidate(node)
  IN CASE candidate.class = "Normal" -> AsyncNormalLimit
       [] candidate.class = "Progress" -> AsyncProgressLimit
       [] OTHER -> AsyncQueueCapacity

CausalCommandCapacityDebt(node) ==
  LET candidate == HeadCausalCandidate(node)
  IN IF NonCompletionCausalAdmissionDebt(node)
          /\ ~CandidateInFlight(candidate)
          /\ ~CanEnqueueClass(node, candidate.class)
     THEN AsyncQueueDepth(node) - CausalHeadCommandLimit(node) + 1
     ELSE 0

THEOREM CausalCommandCapacityDebtIsNatural ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant => CausalCommandCapacityDebt(node) \in Nat
BY Isa
   DEF CausalCommandCapacityDebt, CausalHeadCommandLimit,
       NonCompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       AsyncQueueDepth, AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration

ProtectedStage4Pending(candidate, position) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ ProtectedOwnedAtServiceRank(candidate, <<4, position>>)

ReadyStage4Actionable(candidate) ==
  /\ asyncRunnerPhase[candidate.node] = "Local"
  /\ LocalAdmissionCanAdvance(candidate.node)

ReadyStage4CausalCapacityBlocked(candidate, position) ==
  /\ ProtectedStage4Pending(candidate, position)
  /\ NonCompletionCausalAdmissionDebt(candidate.node)
  /\ ~CausalHeadCanAdvance(candidate.node)

ReadyBlockedAtAux(candidate, position, rank) ==
  /\ ProtectedStage4Pending(candidate, position)
  /\ ~ReadyStage4Actionable(candidate)
  /\ ~ReadyStage4CausalCapacityBlocked(candidate, position)
  /\ ReadyRunAuxRank(candidate.node) = rank

THEOREM ProtectedStage4CarrierFacts ==
  \A candidate, position:
    ProtectedStage4Pending(candidate, position)
      => /\ candidate.node \in AsyncCurrentResponsiveVoters
         /\ candidate \in asyncOutstandingWork[candidate.node]
         /\ CandidateInReadyQueue(candidate)
         /\ ReadyCandidatePosition(candidate) = position
         /\ SelectedCompletionQueueNonempty(candidate.node)
BY Isa
   DEF ProtectedStage4Pending, ProtectedOwnedAtServiceRank,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CandidateInReadyQueue,
       ReadyCandidatePosition, ReadyCandidateSource,
       ReadyCompletionQueue, SelectedCompletionQueueNonempty,
       SelectedCompletionSource, AsyncProgressOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, CandidateScheduled, SequenceSet

THEOREM RunNodeIsNonstuttering ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ RunNode(node)
    => <<RunNode(node)>>_AsyncAllVars
BY Isa
   DEF RunNode, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep, AsyncAllVars, AsyncSchedulerVars,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant

THEOREM ProtectedOwnedCandidateEnablesFairRunNode ==
  \A candidate:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ ResponsiveProtectedCandidateOwned(candidate)
    => ENABLED <<PostGstRunNode(candidate.node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW candidate,
                AsyncStrongTypeInvariant,
                gst,
                ResponsiveProtectedCandidateOwned(candidate)
         PROVE ENABLED <<PostGstRunNode(candidate.node)>>_AsyncAllVars
    <2>1. /\ AsyncTypeInvariant
           /\ candidate.node \in AsyncCurrentResponsiveVoters
           /\ ~NodeHasApplication(candidate.node)
      BY <1>1, AsyncStrongTypeProjectsAsyncType
         DEF ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned
    <2>2. ENABLED RunNode(candidate.node)
      BY <2>1, ResponsiveUnappliedRunNodeIsEnabled
    <2>3. ENABLED PostGstRunNode(candidate.node)
      BY <1>1, <2>1, <2>2, EnabledRunNodeLiftsPostGst
    <2>4. PostGstRunNode(candidate.node)
             => <<PostGstRunNode(candidate.node)>>_AsyncAllVars
      BY <1>1, <2>1, RunNodeIsNonstuttering
         DEF PostGstRunNode
    <2> QED BY <2>3, <2>4, ENABLEDaxioms
  <1> QED BY <1>1

THEOREM Stage4LocalAdvanceStrictlyProgresses ==
  \A candidate, position:
    /\ ProtectedStage4Pending(candidate, position)
    /\ ReadyStage4Actionable(candidate)
    /\ PostGstRunNode(candidate.node)
    => ProtectedRankProgressExit(candidate, <<4, position>>)'
BY ProducerAdmissionRecordsCausalDebt,
   OwedAdmissibleCausalCannotBeOvertaken,
   CandidateSequenceIndexIsPosition, HeadTailProperties,
   SequenceSetAfterAppend, Isa
   DEF ProtectedStage4Pending, ProtectedOwnedAtServiceRank,
       ReadyStage4Actionable, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyCandidatePosition, ReadyCandidateSource,
       ReadyCompletionQueue, LocalSourceDistance,
       CandidateSequenceIndex, CandidateInReadyQueue,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, CandidateScheduled, PostGstRunNode,
       RunNode, LocalAdmissionStep, AdmitProducerCompletion,
       AdmitCausalHead, UpdateLocalAdmissionMetadata,
       SelectedLocalSource, PreferredLocalSource,
       SelectedCompletionSource, SelectedCompletionCandidate,
       SelectedCompletionQueueNonempty, OtherLocalSource,
       EnqueueCandidate, SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant, AsyncIoVars,
       AsyncDeferredVars, vars

Stage4AuxProgress(candidate, position, rank) ==
  \/ ProtectedRankProgressExit(candidate, <<4, position>>)
  \/ ReadyStage4Actionable(candidate)
  \/ ReadyStage4CausalCapacityBlocked(candidate, position)
  \/ \E lower \in SetLessThan(
       rank, ReadyRunAuxOrdering, ReadyRunAuxCarrier):
       ReadyBlockedAtAux(candidate, position, lower)

Stage4AuxStrictResult(candidate, position, rank) ==
  Stage4AuxProgress(candidate, position, rank)'

Stage4AuxStepResult(candidate, position, rank) ==
  \/ Stage4AuxStrictResult(candidate, position, rank)
  \/ ReadyBlockedAtAux(candidate, position, rank)'

THEOREM Stage4LocalAdmissionDecreasesAux ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ LocalAdmissionStep(candidate.node)
    => Stage4AuxStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   ReadyRunAuxRankInCarrier, Isa
   DEF Stage4AuxStrictResult, Stage4AuxProgress, ReadyBlockedAtAux,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       CandidateInReadyQueue, CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4IngressDrainDecreasesAux ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ IngressDrainStep(candidate.node)
    => Stage4AuxStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   IngressDrainStrictlyDecreasesRuntimeReach,
   ReadyRunAuxRankInCarrier, Isa
   DEF Stage4AuxStrictResult, Stage4AuxProgress, ReadyBlockedAtAux,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       IngressDrainStep, DrainFairIngressSelected,
       CandidateInReadyQueue, CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4DiscoveryPrefixPreservesAux ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ \/ \E node \in AsyncCurrentResponsiveVoters:
              DirectCommitCertificateDiscoveryStep(node)
       \/ \E node \in asyncHistoricalRecoveryTargets:
              DirectHistoricalCommitCertificateDiscoveryStep(node)
    => Stage4AuxStepResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4AuxStepResult, Stage4AuxStrictResult, Stage4AuxProgress,
       ReadyBlockedAtAux,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       CandidateInReadyQueue, CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4DeferredDrainDecreasesDebt ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ SerializedRuntimeStep(candidate.node)
    /\ DeferredDrainStep(candidate.node)
    => Stage4AuxStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   ReadyRunAuxRankInCarrier, Isa
   DEF Stage4AuxStrictResult, Stage4AuxProgress, ReadyBlockedAtAux,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       SerializedRuntimeStep, DeferredDrainStep,
       RemoveNextDeferredCommand, DiscardCommand,
       AdvanceNextDeferredClass, DeferredQueueNonempty,
       CandidateInReadyQueue, CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4DeferredTagDecreasesDebt ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ SerializedRuntimeStep(candidate.node)
    /\ DeferredTagStep(candidate.node)
    => Stage4AuxStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   ReadyRunAuxRankInCarrier, Isa
   DEF Stage4AuxStrictResult, Stage4AuxProgress, ReadyBlockedAtAux,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       SerializedRuntimeStep, DeferredTagStep,
       DeferredTimeoutStep, DeferredRetransmitStep,
       CandidateInReadyQueue, CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4DirectTimeoutDecreasesDebt ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ SerializedRuntimeStep(candidate.node)
    /\ DirectTimeoutStep(candidate.node)
    => Stage4AuxStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   ReadyRunAuxRankInCarrier, Isa
   DEF Stage4AuxStrictResult, Stage4AuxProgress, ReadyBlockedAtAux,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       SerializedRuntimeStep, DirectTimeoutStep, TimeoutDue,
       CandidateInReadyQueue, CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4FifoRuntimeOpensCompletionSlot ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ SerializedRuntimeStep(candidate.node)
    /\ FifoRuntimeStep(candidate.node)
    => Stage4AuxStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   ReadyRunAuxRankInCarrier, Isa
   DEF Stage4AuxStrictResult, Stage4AuxProgress, ReadyBlockedAtAux,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       SerializedRuntimeStep, FifoRuntimeStep,
       RemoveNextNodeCommand, DeferCommand, DiscardCommand,
       LocalAdmissionCanAdvance, ProducerCompletionCanAdmit,
       CanEnqueueClass, AsyncQueueDepth, NodeQueueNonempty,
       CandidateInReadyQueue, CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4RetransmitDecreasesFifoDebt ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ SerializedRuntimeStep(candidate.node)
    /\ DirectRetransmitStep(candidate.node)
    /\ ~(NodeQueueNonempty(candidate.node)
           /\ asyncFifoOwed[candidate.node])
    => Stage4AuxStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   ReadyRunAuxRankInCarrier, Isa
   DEF Stage4AuxStrictResult, Stage4AuxProgress, ReadyBlockedAtAux,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       SerializedRuntimeStep, DirectRetransmitStep,
       LocalAdmissionCanAdvance, ProducerCompletionCanAdmit,
       CanEnqueueClass, AsyncQueueDepth, NodeQueueNonempty,
       CandidateInReadyQueue, CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4IdleRuntimeMakesReadyActionable ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ SerializedRuntimeStep(candidate.node)
    /\ IdleRuntimeStep(candidate.node)
    /\ ~NodeQueueNonempty(candidate.node)
    => Stage4AuxStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4AuxStrictResult, Stage4AuxProgress,
       ReadyBlockedAtAux, ProtectedStage4Pending,
       ProtectedOwnedAtServiceRank, ReadyStage4Actionable,
       ProtectedRankProgressExit, ProtectedServiceOwnershipExit,
       CandidateServiceRank, CandidateInReadyQueue,
       LocalAdmissionCanAdvance, ProducerCompletionCanAdmit,
       CanEnqueueClass, AsyncQueueDepth, IdleRuntimeStep,
       SerializedRuntimeStep, AsyncProgressOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4SerializedRuntimeDecreasesAux ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ SerializedRuntimeStep(candidate.node)
    => Stage4AuxStrictResult(candidate, position, rank)
PROOF
  <1>1. ASSUME NEW candidate,
                NEW position,
                NEW rank \in ReadyRunAuxCarrier,
                ReadyBlockedAtAux(candidate, position, rank),
                SerializedRuntimeStep(candidate.node)
         PROVE Stage4AuxStrictResult(candidate, position, rank)
    <2>1. RuntimeStep(candidate.node)
      BY <1>1 DEF SerializedRuntimeStep
    <2>2. CASE DeferredDrainStep(candidate.node)
      BY <1>1, <2>2, Stage4DeferredDrainDecreasesDebt
    <2>3. CASE DeferredTagStep(candidate.node)
      BY <1>1, <2>3, Stage4DeferredTagDecreasesDebt
    <2>4. CASE DirectTimeoutStep(candidate.node)
      BY <1>1, <2>4, Stage4DirectTimeoutDecreasesDebt
    <2>5. CASE FifoRuntimeStep(candidate.node)
      BY <1>1, <2>5, Stage4FifoRuntimeOpensCompletionSlot
    <2>6. CASE /\ DirectRetransmitStep(candidate.node)
                 /\ ~(NodeQueueNonempty(candidate.node)
                       /\ asyncFifoOwed[candidate.node])
      BY <1>1, <2>6, Stage4RetransmitDecreasesFifoDebt
    <2>7. CASE /\ IdleRuntimeStep(candidate.node)
                 /\ ~NodeQueueNonempty(candidate.node)
      BY <1>1, <2>7, Stage4IdleRuntimeMakesReadyActionable
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7
         DEF RuntimeStep
  <1> QED BY <1>1

THEOREM Stage4SameNodeRunDecreasesAux ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ PostGstRunNode(candidate.node)
    => Stage4AuxStrictResult(candidate, position, rank)
PROOF
  <1>1. ASSUME NEW candidate,
                NEW position,
                NEW rank \in ReadyRunAuxCarrier,
                ReadyBlockedAtAux(candidate, position, rank),
                PostGstRunNode(candidate.node)
         PROVE Stage4AuxStrictResult(candidate, position, rank)
    <2>1. RunNode(candidate.node)
      BY <1>1 DEF PostGstRunNode
    <2>2. CASE LocalAdmissionStep(candidate.node)
      BY <1>1, <2>2, Stage4LocalAdmissionDecreasesAux
    <2>3. CASE IngressDrainStep(candidate.node)
      BY <1>1, <2>3, Stage4IngressDrainDecreasesAux
    <2>4. CASE SerializedRuntimeStep(candidate.node)
      BY <1>1, <2>4, Stage4SerializedRuntimeDecreasesAux
    <2> QED BY <2>1, <2>2, <2>3, <2>4 DEF RunNode
  <1> QED BY <1>1

THEOREM Stage4OtherRunnerPreservesOrDecreasesAux ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ \/ \E node \in AsyncCurrentResponsiveVoters:
              /\ node # candidate.node
              /\ RunNode(node)
       \/ \E node \in AsyncCurrentResponsiveVoters:
              RunHistoricalServer(node)
       \/ \E node \in asyncHistoricalRecoveryTargets:
              /\ node # candidate.node
              /\ RunHistoricalRecoveryNode(node)
    => Stage4AuxStepResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4AuxStepResult, Stage4AuxStrictResult,
       ReadyBlockedAtAux, ProtectedStage4Pending,
       ReadyStage4Actionable, ProtectedOwnedAtServiceRank,
       ProtectedRankProgressExit, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer,
       CandidateInReadyQueue, CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4ClockStepPreservesOrDecreasesAux ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ AsyncTick
    => Stage4AuxStepResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4AuxStepResult, Stage4AuxStrictResult,
       ReadyBlockedAtAux, ProtectedStage4Pending,
       ReadyStage4Actionable, ProtectedOwnedAtServiceRank,
       ProtectedRankProgressExit, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       AsyncTick, AsyncNonClockVars, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4IoStepPreservesOrDecreasesAux ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ \/ \E node \in AsyncCurrentResponsiveVoters: ServiceIoWorker(node)
       \/ \E node \in asyncHistoricalRecoveryTargets:
              ServiceHistoricalRecoveryIoWorker(node)
       \/ \E node \in AsyncCurrentResponsiveVoters:
              EnqueueIoLocalControl(node)
       \/ \E node \in asyncHistoricalRecoveryTargets:
              EnqueueHistoricalRecoveryIoLocalControl(node)
    => Stage4AuxStepResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HeadTailProperties, SequenceSetAfterAppend, Isa
   DEF Stage4AuxStepResult, Stage4AuxStrictResult,
       ReadyBlockedAtAux, ProtectedStage4Pending,
       ReadyStage4Actionable, ProtectedOwnedAtServiceRank,
       ProtectedRankProgressExit, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyCandidatePosition, ReadyCandidateSource,
       ReadyCompletionQueue, CandidateSequenceIndex,
       CandidateInReadyQueue, CandidateScheduled, SequenceSet,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4NetworkOrFaultStepPreservesAux ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ (AsyncNetworkStep \/ AsyncFaultStep)
    => Stage4AuxStepResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4AuxStepResult, Stage4AuxStrictResult,
       ReadyBlockedAtAux, ProtectedStage4Pending,
       ReadyStage4Actionable, ProtectedOwnedAtServiceRank,
       ProtectedRankProgressExit, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstCrash, CandidateInReadyQueue,
       CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4OpenHistoricalRecoveryPreservesAux ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ \E node \in ValidatorIds: OpenHistoricalRecovery(node)
    => Stage4AuxStepResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4AuxStepResult, Stage4AuxStrictResult,
       Stage4AuxProgress, ReadyBlockedAtAux,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ReadyStage4CausalCapacityBlocked,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ReadyRunAuxRank,
       OpenHistoricalRecovery, AsyncAllVars

THEOREM Stage4StutterPreservesAux ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ UNCHANGED AsyncAllVars
    => ReadyBlockedAtAux(candidate, position, rank)'
BY Isa DEF ReadyBlockedAtAux, ProtectedStage4Pending,
           ReadyStage4Actionable, ProtectedOwnedAtServiceRank,
           ReadyRunAuxRank, AsyncAllVars, AsyncSchedulerVars, vars

THEOREM Stage4BlockedAuxStep ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    => Stage4AuxStepResult(candidate, position, rank)
PROOF
  <1>1. ASSUME NEW candidate,
                NEW position,
                NEW rank \in ReadyRunAuxCarrier,
                ReadyBlockedAtAux(candidate, position, rank),
                [AsyncNext]_AsyncAllVars
         PROVE Stage4AuxStepResult(candidate, position, rank)
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>1, Stage4StutterPreservesAux
         DEF Stage4AuxStepResult
    <2>2. CASE AsyncNext
      <3>1. CASE \E node \in AsyncCurrentResponsiveVoters:
                    RunNode(node)
        <4>1. CASE RunNode(candidate.node)
          BY <1>1, <2>2, <3>1, <4>1,
             Stage4SameNodeRunDecreasesAux
             DEF Stage4AuxStepResult, PostGstRunNode,
                 ReadyBlockedAtAux, ProtectedStage4Pending,
                 ProtectedOwnedAtServiceRank
        <4>2. CASE ~RunNode(candidate.node)
          BY <1>1, <3>1, <4>2,
             Stage4OtherRunnerPreservesOrDecreasesAux
        <4> QED BY <4>1, <4>2
      <3>2. CASE \E node \in AsyncCurrentResponsiveVoters:
                    RunHistoricalServer(node)
        BY <1>1, <3>2, Stage4OtherRunnerPreservesOrDecreasesAux
      <3>3. CASE \E node \in asyncHistoricalRecoveryTargets:
                    RunHistoricalRecoveryNode(node)
        <4>1. CASE RunNode(candidate.node)
          BY <1>1, <2>2, <3>3, <4>1,
             Stage4SameNodeRunDecreasesAux
             DEF Stage4AuxStepResult, PostGstRunNode,
                 ReadyBlockedAtAux, ProtectedStage4Pending,
                 ProtectedOwnedAtServiceRank
        <4>2. CASE ~RunNode(candidate.node)
          BY <1>1, <3>3, <4>2,
             Stage4OtherRunnerPreservesOrDecreasesAux
             DEF RunNode, RunHistoricalRecoveryNode
        <4> QED BY <4>1, <4>2
      <3>4. CASE AsyncTick
        BY <1>1, <3>4, Stage4ClockStepPreservesOrDecreasesAux
      <3>5. CASE \E node \in ValidatorIds:
                    OpenHistoricalRecovery(node)
        BY <1>1, <3>5, Stage4OpenHistoricalRecoveryPreservesAux
      <3>6. CASE \/ \E node \in AsyncCurrentResponsiveVoters:
                          DirectCommitCertificateDiscoveryStep(node)
                   \/ \E historicalNode \in asyncHistoricalRecoveryTargets:
                          DirectHistoricalCommitCertificateDiscoveryStep(
                            historicalNode)
        BY <1>1, <3>6, Stage4DiscoveryPrefixPreservesAux
      <3>7. CASE \/ \E ioNode \in AsyncCurrentResponsiveVoters:
                          ServiceIoWorker(ioNode)
                   \/ \E historicalIoNode \in asyncHistoricalRecoveryTargets:
                          ServiceHistoricalRecoveryIoWorker(historicalIoNode)
                   \/ \E controlNode \in AsyncCurrentResponsiveVoters:
                          EnqueueIoLocalControl(controlNode)
                   \/ \E historicalControlNode
                          \in asyncHistoricalRecoveryTargets:
                          EnqueueHistoricalRecoveryIoLocalControl(
                            historicalControlNode)
        BY <1>1, <3>7, Stage4IoStepPreservesOrDecreasesAux
      <3>8. CASE AsyncNetworkStep \/ AsyncFaultStep
        BY <1>1, <3>8, Stage4NetworkOrFaultStepPreservesAux
      <3>9. CASE AsyncSetGST
        BY <1>1, <3>9
           DEF ReadyBlockedAtAux, ProtectedStage4Pending,
               ProtectedOwnedAtServiceRank, AsyncSetGST
      <3>10. CASE \E node \in ValidatorIds: PreGstCrash(node)
        BY <1>1, <3>10
           DEF ReadyBlockedAtAux, ProtectedStage4Pending,
               ProtectedOwnedAtServiceRank, PreGstCrash
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
           <3>7, <3>8, <3>9, <3>10
           DEF AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
               AsyncNonRunnerStep
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

Stage4AuxGoal(candidate, position) ==
  \/ ProtectedRankProgressExit(candidate, <<4, position>>)
  \/ ReadyStage4Actionable(candidate)
  \/ ReadyStage4CausalCapacityBlocked(candidate, position)

THEOREM FairStage4AuxOneStep ==
  \A initialContext, candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      AsyncSpecAt(initialContext)
        => (ReadyBlockedAtAux(candidate, position, rank)
              ~> Stage4AuxProgress(candidate, position, rank))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW candidate,
                NEW position,
                NEW rank \in ReadyRunAuxCarrier
         PROVE AsyncSpecAt(initialContext)
                 => (ReadyBlockedAtAux(candidate, position, rank)
                       ~> Stage4AuxProgress(candidate, position, rank))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
    <2>2. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>3. /\ ReadyBlockedAtAux(candidate, position, rank)
             /\ ~Stage4AuxProgress(candidate, position, rank)
            => ENABLED
                 <<PostGstRunNode(candidate.node)>>_AsyncAllVars
      BY ProtectedOwnedCandidateEnablesFairRunNode
         DEF ReadyBlockedAtAux, ProtectedStage4Pending,
             ProtectedOwnedAtServiceRank, Stage4AuxProgress
    <2>4. /\ ReadyBlockedAtAux(candidate, position, rank)
             /\ ~Stage4AuxProgress(candidate, position, rank)
             /\ <<PostGstRunNode(candidate.node)>>_AsyncAllVars
            => Stage4AuxProgress(candidate, position, rank)'
      BY Stage4SameNodeRunDecreasesAux
         DEF Stage4AuxStrictResult
    <2>5. ReadyBlockedAtAux(candidate, position, rank)
              /\ [AsyncNext]_AsyncAllVars
            => ReadyBlockedAtAux(candidate, position, rank)'
                 \/ Stage4AuxProgress(candidate, position, rank)'
      BY Stage4BlockedAuxStep
         DEF Stage4AuxStepResult, Stage4AuxStrictResult
    <2>6. CASE candidate.node \in AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(
                    PostGstRunNode(candidate.node))
        BY <2>6 DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (ReadyBlockedAtAux(candidate, position, rank)
                     ~> Stage4AuxProgress(candidate, position, rank))
        BY <2>3, <2>4, <2>5, <3>1, PTL DEF AsyncSpecAt
      <3> QED BY <3>2
    <2>7. CASE candidate.node \notin AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => []~ReadyBlockedAtAux(candidate, position, rank)
        BY <2>2, <2>7, PTL
           DEF ReadyBlockedAtAux, ProtectedStage4Pending,
               ProtectedOwnedAtServiceRank,
               ResponsiveProtectedCandidateOwned
      <3>2. AsyncSpecAt(initialContext)
               => (ReadyBlockedAtAux(candidate, position, rank)
                     ~> Stage4AuxProgress(candidate, position, rank))
        BY <3>1, PTL
      <3> QED BY <3>2
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM FairStage4AuxRankDescent ==
  \A initialContext, candidate, position:
    AsyncSpecAt(initialContext)
      => \A rank \in ReadyRunAuxCarrier:
           ReadyBlockedAtAux(candidate, position, rank)
             ~> Stage4AuxGoal(candidate, position)
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position
         PROVE AsyncSpecAt(initialContext)
                 => \A rank \in ReadyRunAuxCarrier:
                      ReadyBlockedAtAux(candidate, position, rank)
                        ~> Stage4AuxGoal(candidate, position)
    <2>1. ASSUME NEW rank \in ReadyRunAuxCarrier
           PROVE AsyncSpecAt(initialContext)
                   => (ReadyBlockedAtAux(candidate, position, rank)
                         ~> (Stage4AuxGoal(candidate, position)
                              \/ \E lower \in SetLessThan(
                                   rank, ReadyRunAuxOrdering,
                                   ReadyRunAuxCarrier):
                                   ReadyBlockedAtAux(
                                     candidate, position, lower)))
      BY FairStage4AuxOneStep
         DEF Stage4AuxProgress, Stage4AuxGoal
    <2>2. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  ReadyBlockedAtAux(candidate, position, rank)
                    ~> (Stage4AuxGoal(candidate, position)
                         \/ \E lower \in SetLessThan(
                              rank, ReadyRunAuxOrdering,
                              ReadyRunAuxCarrier):
                              ReadyBlockedAtAux(
                                candidate, position, lower))
      BY <2>1
    <2>3. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  ReadyBlockedAtAux(candidate, position, rank)
                    ~> Stage4AuxGoal(candidate, position)
      BY <2>2, ReadyRunAuxOrderingIsWellFounded,
         WellFoundedLeadsTo
    <2> QED BY <2>3
  <1> QED BY <1>1

=============================================================================
