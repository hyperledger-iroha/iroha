---- MODULE SumeragiV2Stage6CausalRankScratch ----
EXTENDS SumeragiV2AsyncLivenessProofs

(***************************************************************************
Scratch proof kernel for stage-6 causal FIFO ownership.  This module remains
outside the release proof until each carrier, capacity, and temporal leaf has
passed pinned strict TLAPS with a fresh cache.
***************************************************************************)

ProtectedStage6Pending(candidate, position) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ ProtectedOwnedAtServiceRank(candidate, <<6, position>>)

THEOREM ProtectedStage6CarrierFacts ==
  \A candidate, position:
    ProtectedStage6Pending(candidate, position)
      => /\ candidate.node \in AsyncCurrentResponsiveVoters
         /\ candidate.node \in ValidatorIds
         /\ candidate \in CausalCandidates
         /\ candidate \in
              SequenceSet(asyncCausalQueues[candidate.node])
         /\ AsyncQueueTyped(asyncCausalQueues[candidate.node])
         /\ SequenceHasUniqueValues(
              asyncCausalQueues[candidate.node])
         /\ CausalQueueNonempty(candidate.node)
         /\ CausalCandidatePosition(candidate) = position
         /\ CandidateSequenceIndex(
              candidate, asyncCausalQueues[candidate.node])
              \in 1..Len(asyncCausalQueues[candidate.node])
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                ProtectedStage6Pending(candidate, position)
         PROVE /\ candidate.node \in AsyncCurrentResponsiveVoters
               /\ candidate.node \in ValidatorIds
               /\ candidate \in CausalCandidates
               /\ candidate \in
                    SequenceSet(asyncCausalQueues[candidate.node])
               /\ AsyncQueueTyped(asyncCausalQueues[candidate.node])
               /\ SequenceHasUniqueValues(
                    asyncCausalQueues[candidate.node])
               /\ CausalQueueNonempty(candidate.node)
               /\ CausalCandidatePosition(candidate) = position
               /\ CandidateSequenceIndex(
                    candidate, asyncCausalQueues[candidate.node])
                    \in 1..Len(asyncCausalQueues[candidate.node])
    <2>1. /\ AsyncTypeInvariant
           /\ candidate.node \in AsyncCurrentResponsiveVoters
           /\ CandidateScheduled(candidate)
           /\ CandidateServiceRank(candidate) = <<6, position>>
      BY <1>1, AsyncStrongTypeProjectsAsyncType
         DEF ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned
    <2>2. candidate.node \in ValidatorIds
      BY <2>1, ScheduledCandidateProjectsToOwner
    <2>3. candidate \in CausalCandidates
      BY <2>1, SMT
         DEF CandidateServiceRank, CandidateScheduled
    <2>4. candidate \in
             SequenceSet(asyncCausalQueues[candidate.node])
      BY <2>1, <2>3, ScheduledCandidateProjectsToOwner
    <2>5. /\ AsyncQueueTyped(asyncCausalQueues[candidate.node])
           /\ SequenceHasUniqueValues(
                asyncCausalQueues[candidate.node])
      BY <1>1, <2>2
         DEF ProtectedStage6Pending, AsyncStrongTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncCausalTypeInvariant,
             AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>6. CandidateSequenceIndex(
             candidate, asyncCausalQueues[candidate.node])
             \in 1..Len(asyncCausalQueues[candidate.node])
      BY <2>4, <2>5, CandidateSequenceIndexIsPosition
         DEF AsyncQueueTyped
    <2>7. Len(asyncCausalQueues[candidate.node]) \in Nat
      BY <2>5, LenProperties DEF AsyncQueueTyped
    <2>8. CausalQueueNonempty(candidate.node)
      BY <2>6, <2>7, SMTT(30) DEF CausalQueueNonempty
    <2>9. CausalCandidatePosition(candidate) = position
      BY <2>1, <2>3, SMT DEF CandidateServiceRank
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>8,
         <2>9
  <1> QED BY <1>1

(***************************************************************************
Admitting the causal head strictly lowers the protected stage-6 rank.  If
the protected candidate is the head, admission moves it to the runtime FIFO
or Completion I/O, both earlier service stages.  If another head is removed,
the protected candidate's doubled queue index drops by two, which dominates
the possible one-bit reset of the local-source cursor.
***************************************************************************)

THEOREM ConsensusIoCandidateProjection ==
  \A candidate, queue:
    (\E index \in AsyncIoConsensusIndices(queue):
       queue[index].candidate = candidate)
      => candidate \in
           {job.candidate:
              job \in {entry \in SequenceSet(queue):
                         entry.class = "Consensus"}}
PROOF
  <1>1. ASSUME NEW candidate, NEW queue,
                \E index \in AsyncIoConsensusIndices(queue):
                  queue[index].candidate = candidate
         PROVE candidate \in
                 {job.candidate:
                    job \in {entry \in SequenceSet(queue):
                               entry.class = "Consensus"}}
    <2>1. PICK index \in AsyncIoConsensusIndices(queue):
             queue[index].candidate = candidate
      BY <1>1
    <2>2. /\ index \in 1..Len(queue)
           /\ queue[index].class = "Consensus"
           /\ queue[index] \in SequenceSet(queue)
      BY <2>1 DEF AsyncIoConsensusIndices, SequenceSet
    <2>3. queue[index] \in
             {entry \in SequenceSet(queue):
                entry.class = "Consensus"}
      BY <2>2
    <2>4. queue[index].candidate \in
             {job.candidate:
                job \in {entry \in SequenceSet(queue):
                           entry.class = "Consensus"}}
      BY <2>3
    <2> QED BY <2>1, <2>4
  <1> QED BY <1>1

THEOREM CausalPredecessorPositionDrops ==
  \A oldIndex, newIndex \in Nat:
    newIndex + 1 = oldIndex
      => \A oldPreferred, newPreferred \in BOOLEAN:
           2 * newIndex + (IF newPreferred THEN 0 ELSE 1)
             < 2 * oldIndex + (IF oldPreferred THEN 0 ELSE 1)
PROOF
  <1>1. ASSUME NEW oldIndex \in Nat,
                NEW newIndex \in Nat,
                newIndex + 1 = oldIndex
         PROVE \A oldPreferred, newPreferred \in BOOLEAN:
                 2 * newIndex +
                   (IF newPreferred THEN 0 ELSE 1)
                   < 2 * oldIndex +
                       (IF oldPreferred THEN 0 ELSE 1)
    <2>1. ASSUME NEW oldPreferred \in BOOLEAN,
                  NEW newPreferred \in BOOLEAN
           PROVE 2 * newIndex +
                   (IF newPreferred THEN 0 ELSE 1)
                   < 2 * oldIndex +
                       (IF oldPreferred THEN 0 ELSE 1)
      <3>1. CASE oldPreferred /\ newPreferred
        BY <1>1, <2>1, <3>1, SMTT(30)
      <3>2. CASE oldPreferred /\ ~newPreferred
        BY <1>1, <2>1, <3>2, SMTT(30)
      <3>3. CASE ~oldPreferred /\ newPreferred
        BY <1>1, <2>1, <3>3, SMTT(30)
      <3>4. CASE ~oldPreferred /\ ~newPreferred
        BY <1>1, <2>1, <3>4, SMTT(30)
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM Stage6AdmissionQueueFacts ==
  \A candidate, position:
    /\ ProtectedStage6Pending(candidate, position)
    /\ AdmitCausalHead(candidate.node)
    => /\ asyncCausalQueues'[candidate.node] =
              Tail(asyncCausalQueues[candidate.node])
       /\ SequenceSet(Tail(asyncCausalQueues[candidate.node])) =
              SequenceSet(asyncCausalQueues[candidate.node])
                \ {HeadCausalCandidate(candidate.node)}
       /\ CausalCandidates' =
              CausalCandidates \ {HeadCausalCandidate(candidate.node)}
       /\ (HeadCausalCandidate(candidate.node) # candidate
             => CandidateSequenceIndex(
                  candidate, asyncCausalQueues'[candidate.node]) + 1
                  = CandidateSequenceIndex(
                      candidate, asyncCausalQueues[candidate.node]))
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                ProtectedStage6Pending(candidate, position),
                AdmitCausalHead(candidate.node)
         PROVE /\ asyncCausalQueues'[candidate.node] =
                       Tail(asyncCausalQueues[candidate.node])
               /\ SequenceSet(Tail(
                    asyncCausalQueues[candidate.node])) =
                    SequenceSet(asyncCausalQueues[candidate.node])
                      \ {HeadCausalCandidate(candidate.node)}
               /\ CausalCandidates' =
                    CausalCandidates
                      \ {HeadCausalCandidate(candidate.node)}
               /\ (HeadCausalCandidate(candidate.node) # candidate
                     => CandidateSequenceIndex(
                          candidate,
                          asyncCausalQueues'[candidate.node]) + 1
                          = CandidateSequenceIndex(
                              candidate,
                              asyncCausalQueues[candidate.node]))
    <2>1. /\ candidate.node \in ValidatorIds
           /\ candidate \in
                SequenceSet(asyncCausalQueues[candidate.node])
           /\ AsyncQueueTyped(asyncCausalQueues[candidate.node])
           /\ SequenceHasUniqueValues(
                asyncCausalQueues[candidate.node])
           /\ CausalQueueNonempty(candidate.node)
      BY <1>1, ProtectedStage6CarrierFacts
    <2>2. /\ asyncCausalQueues[candidate.node]
                    \in Seq(Range(asyncCausalQueues[candidate.node]))
           /\ Len(asyncCausalQueues[candidate.node]) > 0
      BY <2>1 DEF AsyncQueueTyped, CausalQueueNonempty
    <2>3. /\ SequenceSet(Tail(
                    asyncCausalQueues[candidate.node])) =
                  SequenceSet(asyncCausalQueues[candidate.node])
                    \ {HeadCausalCandidate(candidate.node)}
           /\ SequenceHasUniqueValues(
                Tail(asyncCausalQueues[candidate.node]))
      BY <2>1, <2>2, UniqueSequenceTailSetFacts
         DEF HeadCausalCandidate
    <2>4. asyncCausalQueues' =
             [asyncCausalQueues EXCEPT
                ![candidate.node] = Tail(@)]
      BY <1>1 DEF AdmitCausalHead
    <2>5. asyncCausalQueues'[candidate.node] =
             Tail(asyncCausalQueues[candidate.node])
      <3>1. candidate.node \in DOMAIN asyncCausalQueues
        BY <1>1, <2>1
           DEF ProtectedStage6Pending, AsyncStrongTypeInvariant,
               AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
               AsyncCausalTypeInvariant
      <3>2. [asyncCausalQueues EXCEPT
                 ![candidate.node] = Tail(@)][candidate.node] =
               Tail(asyncCausalQueues[candidate.node])
        BY <3>1, FunctionalTailUpdateAtKey
      <3> QED BY <2>4, <3>2
    <2>6. /\ DOMAIN asyncCausalQueues = ValidatorIds
           /\ HeadCausalCandidate(candidate.node).node = candidate.node
           /\ \A owner \in ValidatorIds:
                \A owned \in SequenceSet(asyncCausalQueues[owner]):
                  owned.node = owner
      <3>1. AsyncCausalTypeInvariant
        BY <1>1
           DEF ProtectedStage6Pending, AsyncStrongTypeInvariant,
               AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant
      <3>2. HeadCausalCandidate(candidate.node).node = candidate.node
        BY <2>1, <3>1, CausalHeadCandidateIsOwned
      <3> QED BY <3>1, <3>2 DEF AsyncCausalTypeInvariant,
           AsyncCausalQueueOwnership
    <2>7. CausalCandidates' =
             CausalCandidates \ {HeadCausalCandidate(candidate.node)}
      BY <2>1, <2>3, <2>4, <2>6,
         UnionOfSequenceSetsAfterTailAtKey
         DEF CausalCandidates
    <2>8. HeadCausalCandidate(candidate.node) # candidate
             => CandidateSequenceIndex(
                  candidate, asyncCausalQueues'[candidate.node]) + 1
                  = CandidateSequenceIndex(
                      candidate, asyncCausalQueues[candidate.node])
      BY <2>1, <2>2, <2>5,
         CandidateSequenceIndexAfterNonTargetHead
         DEF HeadCausalCandidate
    <2> QED BY <2>3, <2>5, <2>7, <2>8
  <1> QED BY <1>1

THEOREM Stage6CausalAdmissionStrictlyProgresses ==
  \A candidate, position:
    /\ ProtectedStage6Pending(candidate, position)
    /\ AsyncStrongTypeInvariant'
    /\ AsyncProgressOwnershipInvariant'
    /\ AdmitCausalHead(candidate.node)
    => ProtectedRankProgressExit(candidate, <<6, position>>)'
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                ProtectedStage6Pending(candidate, position),
                AsyncStrongTypeInvariant',
                AsyncProgressOwnershipInvariant',
                AdmitCausalHead(candidate.node)
         PROVE ProtectedRankProgressExit(
                 candidate, <<6, position>>)'
    <2>1. /\ candidate.node \in ValidatorIds
           /\ candidate \in CausalCandidates
           /\ candidate \in
                SequenceSet(asyncCausalQueues[candidate.node])
           /\ CausalCandidatePosition(candidate) = position
           /\ CandidateSequenceIndex(
                candidate, asyncCausalQueues[candidate.node])
                \in 1..Len(asyncCausalQueues[candidate.node])
      BY <1>1, ProtectedStage6CarrierFacts
    <2>2. /\ asyncCausalQueues'[candidate.node] =
                  Tail(asyncCausalQueues[candidate.node])
           /\ SequenceSet(Tail(
                  asyncCausalQueues[candidate.node])) =
                  SequenceSet(asyncCausalQueues[candidate.node])
                    \ {HeadCausalCandidate(candidate.node)}
           /\ CausalCandidates' =
                  CausalCandidates
                    \ {HeadCausalCandidate(candidate.node)}
           /\ (HeadCausalCandidate(candidate.node) # candidate
                 => CandidateSequenceIndex(
                      candidate,
                      asyncCausalQueues'[candidate.node]) + 1
                      = CandidateSequenceIndex(
                          candidate,
                          asyncCausalQueues[candidate.node]))
      BY <1>1, Stage6AdmissionQueueFacts
    <2>3. AsyncTypeInvariant'
      BY <1>1
         DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
             StrongInductiveInvariant, Safety
    <2>4. CASE HeadCausalCandidate(candidate.node) = candidate
      <3>1. candidate \notin CausalCandidates'
        BY <2>2, <2>4
      <3>2. CASE ProtectedServiceOwnershipExit(candidate)'
        BY <3>2 DEF ProtectedRankProgressExit
      <3>3. CASE ~ProtectedServiceOwnershipExit(candidate)'
        <4>1. CandidateScheduled(candidate)'
          BY <3>3
             DEF ProtectedServiceOwnershipExit,
                 ResponsiveProtectedCandidateOwned,
                 ProtectedCandidateOwned
        <4>2. candidate \in DeferredCandidates'
                 \/ candidate \in QueuedCandidates'
                 \/ candidate \in TrackedWorkCandidates'
          BY <3>1, <4>1, Isa DEF CandidateScheduled
        <4>3. candidate \in TrackedWorkCandidates'
                 => candidate \in
                      asyncOutstandingWork'[candidate.node]
          BY <2>1, <2>3, Isa
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
                 AsyncIoWorkContentTypeInvariant,
                 TrackedWorkCandidates
        <4>4. CASE candidate \in DeferredCandidates'
          BY <4>4, SMT
             DEF ProtectedRankProgressExit, CandidateServiceRank,
                 ServiceRankLess
        <4>5. CASE /\ candidate \notin DeferredCandidates'
                    /\ candidate \in QueuedCandidates'
          BY <4>5, SMT
             DEF ProtectedRankProgressExit, CandidateServiceRank,
                 ServiceRankLess
        <4>6. CASE /\ candidate \notin DeferredCandidates'
                    /\ candidate \notin QueuedCandidates'
                    /\ candidate \in TrackedWorkCandidates'
                    /\ CandidateInReadyQueue(candidate)'
          BY <4>6, SMT
             DEF ProtectedRankProgressExit, CandidateServiceRank,
                 ServiceRankLess
        <4>7. CASE /\ candidate \notin DeferredCandidates'
                    /\ candidate \notin QueuedCandidates'
                    /\ candidate \in TrackedWorkCandidates'
                    /\ ~CandidateInReadyQueue(candidate)'
                    /\ CandidateInIoQueue(candidate)'
          BY <4>7, SMT
             DEF ProtectedRankProgressExit, CandidateServiceRank,
                 ServiceRankLess
        <4>8. CASE /\ candidate \notin DeferredCandidates'
                    /\ candidate \notin QueuedCandidates'
                    /\ candidate \in TrackedWorkCandidates'
                    /\ ~CandidateInReadyQueue(candidate)'
                    /\ ~CandidateInIoQueue(candidate)'
          BY <4>3, <4>8, SMT
             DEF ProtectedRankProgressExit, CandidateServiceRank,
                 ServiceRankLess
        <4> QED BY <4>2, <4>4, <4>5, <4>6, <4>7, <4>8
      <3> QED BY <3>2, <3>3
    <2>5. CASE HeadCausalCandidate(candidate.node) # candidate
      <3>1. candidate \in CausalCandidates'
        BY <2>1, <2>2, <2>5
      <3>2. CASE ProtectedServiceOwnershipExit(candidate)'
        BY <3>2 DEF ProtectedRankProgressExit
      <3>3. CASE ~ProtectedServiceOwnershipExit(candidate)'
        <4>1. /\ candidate \notin QueuedCandidates'
               /\ candidate \notin DeferredCandidates'
               /\ candidate \notin TrackedWorkCandidates'
          BY <1>1, <3>1, Isa
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>2. AsyncOutstandingCarrierInvariant'
          BY <1>1 DEF AsyncProgressOwnershipInvariant
        <4>3. candidate \notin
                 asyncOutstandingWork'[candidate.node]
          BY <2>1, <4>1, Isa DEF TrackedWorkCandidates
        <4>4. candidate \notin SequenceSet(
                 asyncIoReadyCompletions'[candidate.node])
          BY <2>1, <4>2, <4>3, Isa
             DEF AsyncOutstandingCarrierInvariant
        <4>5. candidate \notin SequenceSet(
                 asyncLocalReadyCompletions'[candidate.node])
          BY <2>1, <4>2, <4>3, Isa
             DEF AsyncOutstandingCarrierInvariant
        <4>6. ~CandidateInReadyQueue(candidate)'
          BY <4>4, <4>5 DEF CandidateInReadyQueue
        <4>7. candidate \notin
                 ConsensusIoCandidates(candidate.node)'
          BY <2>1, <4>2, <4>3, Isa
             DEF AsyncOutstandingCarrierInvariant
        <4>8. ~CandidateInIoQueue(candidate)'
          <5>1. ASSUME CandidateInIoQueue(candidate)'
                 PROVE FALSE
            <6>1. candidate \in
                     ConsensusIoCandidates(candidate.node)'
              BY <5>1, ConsensusIoCandidateProjection
                 DEF CandidateInIoQueue, ConsensusIoCandidates
            <6> QED BY <4>7, <6>1
          <5> QED BY <5>1
        <4>9. CandidateServiceRank(candidate)' =
                 <<6, CausalCandidatePosition(candidate)'>>
          BY <3>1, <4>1, <4>3, <4>6, <4>8
             DEF CandidateServiceRank
        <4>10. CandidateSequenceIndex(
                 candidate, asyncCausalQueues'[candidate.node]) + 1
                 = CandidateSequenceIndex(
                     candidate, asyncCausalQueues[candidate.node])
          BY <2>2, <2>5
        <4>11. /\ CandidateSequenceIndex(
                     candidate, asyncCausalQueues[candidate.node]) \in Nat
                /\ CandidateSequenceIndex(
                     candidate,
                     asyncCausalQueues'[candidate.node]) \in Nat
          BY <2>1, <4>10, SMTT(30)
        <4>12. CausalCandidatePosition(candidate)' < position
          BY <2>1, <4>10, <4>11,
             CausalPredecessorPositionDrops
             DEF CausalCandidatePosition, LocalSourceDistance
        <4>13. ServiceRankLess(
                 CandidateServiceRank(candidate), <<6, position>>)'
          BY <4>9, <4>12 DEF ServiceRankLess
        <4> QED BY <4>13 DEF ProtectedRankProgressExit
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

=============================================================================
