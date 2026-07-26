---- MODULE SumeragiV2AsyncStage6Proofs ----
EXTENDS SumeragiV2AsyncStage3Proofs

(***************************************************************************
Stage 6: causal FIFO kernel.

Admission of the exact causal head moves it to an earlier service stage;
admission of a predecessor strictly lowers the doubled queue/cursor rank.
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
        <4>10. candidate \in SequenceSet(
                 asyncCausalQueues'[candidate.node])
          BY <2>1, <2>2, <2>5, Isa
        <4>11. AsyncQueueTyped(
                 asyncCausalQueues'[candidate.node])
          BY <2>1, <2>3
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncRuntimeTypeInvariant, AsyncCausalTypeInvariant
        <4>12. CandidateSequenceIndex(
                 candidate, asyncCausalQueues'[candidate.node])
                 \in 1..Len(asyncCausalQueues'[candidate.node])
          BY <4>10, <4>11, CandidateSequenceIndexIsPosition
             DEF AsyncQueueTyped
        <4>13. CandidateSequenceIndex(
                 candidate, asyncCausalQueues[candidate.node]) \in Nat
          BY <2>1, Isa
        <4>14. CandidateSequenceIndex(
                 candidate, asyncCausalQueues'[candidate.node]) \in Nat
          BY <4>12, Isa
        <4>15. CandidateSequenceIndex(
                 candidate, asyncCausalQueues'[candidate.node]) + 1
                 = CandidateSequenceIndex(
                     candidate, asyncCausalQueues[candidate.node])
          BY <2>2, <2>5
        <4>16. CausalCandidatePosition(candidate)' < position
          BY <2>1, <4>13, <4>14, <4>15,
             CausalPredecessorPositionDrops
             DEF CausalCandidatePosition, LocalSourceDistance
        <4>17. ServiceRankLess(
                 CandidateServiceRank(candidate), <<6, position>>)'
          BY <4>9, <4>16 DEF ServiceRankLess
        <4> QED BY <4>17 DEF ProtectedRankProgressExit
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1


(***************************************************************************
Stage 6 capacity closure.

The causal FIFO kernel above covers the exact admission step.  The remaining
work is to show that a protected causal owner cannot remain behind a blocked
head forever.  Non-Completion heads reuse the command-prefix debt and runner
ordering already established for Stage 4.  Completion heads use a separate
rank because they consume the executor I/O reservation rather than a runtime
queue prefix.
***************************************************************************)

Stage6OwedCausalReady(candidate, position) ==
  /\ ProtectedStage6Pending(candidate, position)
  /\ CausalAdmissionDebtActive(candidate.node)
  /\ CausalHeadCanAdvance(candidate.node)

Stage6NonCompletionCapacityBlockedAtRank(candidate, position, rank) ==
  /\ ProtectedStage6Pending(candidate, position)
  /\ NonCompletionCausalAdmissionDebt(candidate.node)
  /\ ~CausalHeadCanAdvance(candidate.node)
  /\ Stage4CapacityRank(candidate.node) = rank

Stage6NonCompletionCapacityGoal(candidate, position) ==
  \/ ProtectedRankProgressExit(candidate, <<6, position>>)
  \/ Stage6OwedCausalReady(candidate, position)

Stage6NonCompletionCapacityProgress(candidate, position, rank) ==
  \/ Stage6NonCompletionCapacityGoal(candidate, position)
  \/ \E lower \in SetLessThan(
       rank, Stage4CapacityOrdering, Stage4CapacityCarrier):
       Stage6NonCompletionCapacityBlockedAtRank(
         candidate, position, lower)

Stage6NonCompletionCapacityStrictResult(candidate, position, rank) ==
  Stage6NonCompletionCapacityProgress(candidate, position, rank)'

Stage6NonCompletionCapacityStepResult(candidate, position, rank) ==
  \/ Stage6NonCompletionCapacityStrictResult(candidate, position, rank)
  \/ Stage6NonCompletionCapacityBlockedAtRank(
       candidate, position, rank)'

THEOREM Stage6NonCompletionCapacityBlockedCoreFacts ==
  \A candidate, position, rank:
    Stage6NonCompletionCapacityBlockedAtRank(candidate, position, rank)
      => /\ candidate.node \in ValidatorIds
         /\ candidate.node \in AsyncCurrentResponsiveVoters
         /\ AsyncStrongTypeInvariant
         /\ AsyncTypeInvariant
         /\ AsyncProgressOwnershipInvariant
         /\ NonCompletionCausalAdmissionDebt(candidate.node)
         /\ ~CausalHeadCanAdvance(candidate.node)
PROOF
  <1>1. ASSUME NEW candidate, NEW position, NEW rank,
                Stage6NonCompletionCapacityBlockedAtRank(
                  candidate, position, rank)
         PROVE /\ candidate.node \in ValidatorIds
               /\ candidate.node \in AsyncCurrentResponsiveVoters
               /\ AsyncStrongTypeInvariant
               /\ AsyncTypeInvariant
               /\ AsyncProgressOwnershipInvariant
               /\ NonCompletionCausalAdmissionDebt(candidate.node)
               /\ ~CausalHeadCanAdvance(candidate.node)
    <2>1. ProtectedStage6Pending(candidate, position)
      BY <1>1 DEF Stage6NonCompletionCapacityBlockedAtRank
    <2>2. /\ candidate.node \in ValidatorIds
           /\ candidate.node \in AsyncCurrentResponsiveVoters
      BY <2>1, ProtectedStage6CarrierFacts
    <2>3. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
      BY <2>1 DEF ProtectedStage6Pending
    <2>4. AsyncTypeInvariant
      BY <2>3, AsyncStrongTypeProjectsAsyncType
    <2> QED BY <1>1, <2>2, <2>3, <2>4
         DEF Stage6NonCompletionCapacityBlockedAtRank
  <1> QED BY <1>1

THEOREM Stage6NonCompletionCapacityLocalAdmissionStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage6NonCompletionCapacityBlockedAtRank(
         candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ PostGstRunNode(candidate.node)
    /\ LocalAdmissionStep(candidate.node)
    => Stage6NonCompletionCapacityStrictResult(
         candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   Stage4CapacityRankInCarrier, Isa
   DEF Stage6NonCompletionCapacityStrictResult,
       Stage6NonCompletionCapacityProgress,
       Stage6NonCompletionCapacityGoal,
       Stage6NonCompletionCapacityBlockedAtRank,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier, ProtectedStage6Pending,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCandidatePosition, LocalSourceDistance,
       PreferredLocalSource, CausalCommandCapacityDebt,
       CausalHeadCommandLimit, NonCompletionCausalAdmissionDebt,
       CausalAdmissionDebtActive, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       ProducerCompletionCanAdvance, ProducerCompletionCanAdmit,
       RecordBlockedCausalDebt, CandidateScheduled,
       CandidateInFlight, CausalHeadCanAdvance, CanEnqueueClass,
       AsyncQueueDepth, NodeQueueNonempty, CausalCandidates,
       SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage6NonCompletionCapacityIngressStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage6NonCompletionCapacityBlockedAtRank(
         candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ PostGstRunNode(candidate.node)
    /\ IngressDrainStep(candidate.node)
    => Stage6NonCompletionCapacityStrictResult(
         candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   IngressDrainStrictlyDecreasesRuntimeReach,
   Stage4CapacityRankInCarrier, Isa
   DEF Stage6NonCompletionCapacityStrictResult,
       Stage6NonCompletionCapacityProgress,
       Stage6NonCompletionCapacityGoal,
       Stage6NonCompletionCapacityBlockedAtRank,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier, ProtectedStage6Pending,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCandidatePosition, LocalSourceDistance,
       PreferredLocalSource, CausalCommandCapacityDebt,
       CausalHeadCommandLimit, NonCompletionCausalAdmissionDebt,
       CausalAdmissionDebtActive, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       IngressDrainStep, DrainFairIngressSelected,
       IngressItemCanDrain, PopSelectedIngress,
       CandidateScheduled, CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueClass, AsyncQueueDepth, NodeQueueNonempty,
       CausalCandidates, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage6NonCompletionCapacityFifoStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage6NonCompletionCapacityBlockedAtRank(
         candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ SerializedRuntimeStep(candidate.node)
    /\ FifoRuntimeStep(candidate.node)
    => Stage6NonCompletionCapacityStrictResult(
         candidate, position, rank)
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                NEW rank \in Stage4CapacityCarrier,
                Stage6NonCompletionCapacityBlockedAtRank(
                  candidate, position, rank),
                [AsyncNext]_AsyncAllVars,
                SerializedRuntimeStep(candidate.node),
                FifoRuntimeStep(candidate.node)
         PROVE Stage6NonCompletionCapacityStrictResult(
                 candidate, position, rank)
    <2>1. /\ candidate.node \in ValidatorIds
           /\ AsyncTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ NonCompletionCausalAdmissionDebt(candidate.node)
      BY <1>1, Stage6NonCompletionCapacityBlockedCoreFacts
    <2>2. \/ CausalHeadCanAdvance(candidate.node)'
             \/ CausalCommandCapacityDebt(candidate.node)'
                  < CausalCommandCapacityDebt(candidate.node)
      BY <1>1, <2>1, NonCompletionDebtFifoCapacityProgress
    <2>3. /\ AsyncStrongTypeInvariant'
           /\ AsyncProgressOwnershipInvariant'
      BY <1>1, AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesProgressOwnership
    <2>4. CASE CausalHeadCanAdvance(candidate.node)'
      <3>1. CASE ProtectedRankProgressExit(
                    candidate, <<6, position>>)'
        BY <3>1 DEF Stage6NonCompletionCapacityStrictResult,
                         Stage6NonCompletionCapacityProgress,
                         Stage6NonCompletionCapacityGoal
      <3>2. CASE ~ProtectedRankProgressExit(
                    candidate, <<6, position>>)'
        <4>1. /\ NonCompletionCausalAdmissionDebt(candidate.node)'
               /\ ProtectedStage6Pending(candidate, position)'
          BY <1>1, <2>1, <2>3, <3>2,
             SerializedFifoRetainsNonCompletionCausalDebt,
             SerializedFifoRetainsExistingCausalHead, Isa
             DEF ProtectedStage6Pending,
                 ProtectedOwnedAtServiceRank,
                 ProtectedRankProgressExit,
                 ProtectedServiceOwnershipExit,
                 ResponsiveProtectedCandidateOwned,
                 ProtectedCandidateOwned, CandidateServiceRank,
                 CausalCandidatePosition, CandidateScheduled,
                 CausalCandidates, SequenceSet,
                 AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant,
                 AsyncOutstandingCarrierInvariant, AsyncAllVars
        <4>2. Stage6OwedCausalReady(candidate, position)'
          BY <2>4, <4>1
             DEF Stage6OwedCausalReady,
                 NonCompletionCausalAdmissionDebt
        <4> QED BY <4>2
             DEF Stage6NonCompletionCapacityStrictResult,
                 Stage6NonCompletionCapacityProgress,
                 Stage6NonCompletionCapacityGoal
      <3> QED BY <3>1, <3>2
    <2>5. CASE ~CausalHeadCanAdvance(candidate.node)'
      <3>1. CausalCommandCapacityDebt(candidate.node)'
               < CausalCommandCapacityDebt(candidate.node)
        BY <2>2, <2>5
      <3>2. CASE ProtectedRankProgressExit(
                    candidate, <<6, position>>)'
        BY <3>2 DEF Stage6NonCompletionCapacityStrictResult,
                         Stage6NonCompletionCapacityProgress,
                         Stage6NonCompletionCapacityGoal
      <3>3. CASE ~ProtectedRankProgressExit(
                    candidate, <<6, position>>)'
        <4>1. AsyncTypeInvariant'
          BY <2>3, AsyncStrongTypeProjectsAsyncType
        <4>2. Stage4CapacityRank(candidate.node)'
                 \in Stage4CapacityCarrier
          BY <2>1, <4>1, Stage4CapacityRankInCarrier
        <4>3. PICK lower \in Stage4CapacityCarrier:
                 lower = Stage4CapacityRank(candidate.node)'
          BY <4>2
        <4>4. lower \in SetLessThan(
                 rank, Stage4CapacityOrdering,
                 Stage4CapacityCarrier)
          BY <1>1, <3>1, <4>3, Isa
             DEF Stage6NonCompletionCapacityBlockedAtRank,
                 Stage4CapacityRank, Stage4CapacityOrdering,
                 Stage4CapacityCarrier, LexPairOrdering, OpToRel
        <4>5. /\ NonCompletionCausalAdmissionDebt(candidate.node)'
               /\ ProtectedStage6Pending(candidate, position)'
          BY <1>1, <2>1, <2>3, <2>5, <3>3,
             SerializedFifoRetainsNonCompletionCausalDebt,
             SerializedFifoRetainsExistingCausalHead, Isa
             DEF ProtectedStage6Pending,
                 ProtectedOwnedAtServiceRank,
                 ProtectedRankProgressExit,
                 ProtectedServiceOwnershipExit,
                 ResponsiveProtectedCandidateOwned,
                 ProtectedCandidateOwned, CandidateServiceRank,
                 CausalCandidatePosition, CandidateScheduled,
                 CausalCandidates, SequenceSet,
                 AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant,
                 AsyncOutstandingCarrierInvariant, AsyncAllVars
        <4>6. Stage6NonCompletionCapacityBlockedAtRank(
                 candidate, position, lower)'
          BY <2>5, <4>3, <4>5
             DEF Stage6NonCompletionCapacityBlockedAtRank
        <4> QED BY <4>4, <4>6
             DEF Stage6NonCompletionCapacityStrictResult,
                 Stage6NonCompletionCapacityProgress
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM Stage6NonCompletionCapacityNonFifoRuntimeStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage6NonCompletionCapacityBlockedAtRank(
         candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ SerializedRuntimeStep(candidate.node)
    /\ ~FifoRuntimeStep(candidate.node)
    => Stage6NonCompletionCapacityStrictResult(
         candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   Stage4CapacityRankInCarrier, Isa
   DEF Stage6NonCompletionCapacityStrictResult,
       Stage6NonCompletionCapacityProgress,
       Stage6NonCompletionCapacityGoal,
       Stage6NonCompletionCapacityBlockedAtRank,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier, ProtectedStage6Pending,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCandidatePosition, LocalSourceDistance,
       PreferredLocalSource, CausalCommandCapacityDebt,
       CausalHeadCommandLimit, NonCompletionCausalAdmissionDebt,
       CausalAdmissionDebtActive, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       SerializedRuntimeStep, RuntimeStep, DeferredDrainStep,
       DeferredTagStep, DirectTimeoutStep, DirectRetransmitStep,
       IdleRuntimeStep, RemoveNextDeferredCommand, DiscardCommand,
       AdvanceNextDeferredClass, DeferredQueueNonempty,
       DeferredHandoffActive, DeferredHandoffMatches,
       DeferredHandoffAllowsExecution, DeferredHandoffBlocksExecution,
       InstallDeferredHandoff, RetainDeferredHandoffs,
       ClearDeferredHandoff, AppendCausalSuccessors,
       FreshCommandSuccessors, LocalAdmissionCanAdvance,
       ProducerCompletionCanAdvance, CandidateScheduled,
       CandidateInFlight, CausalHeadCanAdvance, CanEnqueueClass,
       AsyncQueueDepth, NodeQueueNonempty, CausalCandidates,
       SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage6NonCompletionCapacitySameNodeRunStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage6NonCompletionCapacityBlockedAtRank(
         candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ PostGstRunNode(candidate.node)
    => Stage6NonCompletionCapacityStrictResult(
         candidate, position, rank)
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                NEW rank \in Stage4CapacityCarrier,
                Stage6NonCompletionCapacityBlockedAtRank(
                  candidate, position, rank),
                [AsyncNext]_AsyncAllVars,
                PostGstRunNode(candidate.node)
         PROVE Stage6NonCompletionCapacityStrictResult(
                 candidate, position, rank)
    <2>1. RunNode(candidate.node)
      BY <1>1 DEF PostGstRunNode
    <2>2. CASE LocalAdmissionStep(candidate.node)
      BY <1>1, <2>2,
         Stage6NonCompletionCapacityLocalAdmissionStrictlyProgresses
    <2>3. CASE IngressDrainStep(candidate.node)
      BY <1>1, <2>3,
         Stage6NonCompletionCapacityIngressStrictlyProgresses
    <2>4. CASE SerializedRuntimeStep(candidate.node)
      <3>1. CASE FifoRuntimeStep(candidate.node)
        BY <1>1, <2>4, <3>1,
           Stage6NonCompletionCapacityFifoStrictlyProgresses
      <3>2. CASE ~FifoRuntimeStep(candidate.node)
        BY <1>1, <2>4, <3>2,
           Stage6NonCompletionCapacityNonFifoRuntimeStrictlyProgresses
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1, <2>2, <2>3, <2>4 DEF RunNode
  <1> QED BY <1>1

THEOREM Stage6NonCompletionCapacityOtherStepPreservesOrProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage6NonCompletionCapacityBlockedAtRank(
         candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~PostGstRunNode(candidate.node)
    => Stage6NonCompletionCapacityStepResult(
         candidate, position, rank)
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                NEW rank \in Stage4CapacityCarrier,
                Stage6NonCompletionCapacityBlockedAtRank(
                  candidate, position, rank),
                [AsyncNext]_AsyncAllVars,
                ~PostGstRunNode(candidate.node)
         PROVE Stage6NonCompletionCapacityStepResult(
                 candidate, position, rank)
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>1, Isa
         DEF Stage6NonCompletionCapacityStepResult,
             Stage6NonCompletionCapacityBlockedAtRank,
             Stage4CapacityRank, CausalCommandCapacityDebt,
             CausalHeadCommandLimit, ReadyRunAuxRank,
             ReadyRunDeferredRank, ReadyRunTimeoutRank,
             ReadyRunInnerRank, ProtectedStage6Pending,
             ProtectedOwnedAtServiceRank, AsyncAllVars,
             AsyncSchedulerVars, vars
    <2>2. CASE AsyncNext
      <3>1. CASE \/ (\E node \in AsyncCurrentResponsiveVoters:
                           /\ node # candidate.node
                           /\ RunNode(node))
                   \/ (\E node \in AsyncCurrentResponsiveVoters:
                           RunHistoricalServer(node))
                   \/ (\E node \in asyncHistoricalRecoveryTargets:
                           /\ node # candidate.node
                           /\ RunHistoricalRecoveryNode(node))
        BY <1>1, <2>2, <3>1,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership,
           Stage4CapacityRankInCarrier, Isa
           DEF Stage6NonCompletionCapacityStepResult,
               Stage6NonCompletionCapacityStrictResult,
               Stage6NonCompletionCapacityProgress,
               Stage6NonCompletionCapacityGoal,
               Stage6NonCompletionCapacityBlockedAtRank,
               Stage4CapacityRank, Stage4CapacityOrdering,
               Stage4CapacityCarrier, ProtectedStage6Pending,
               ProtectedOwnedAtServiceRank,
               ProtectedRankProgressExit,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, CausalCandidatePosition,
               LocalSourceDistance, PreferredLocalSource,
               CausalCommandCapacityDebt, CausalHeadCommandLimit,
               NonCompletionCausalAdmissionDebt,
               CausalAdmissionDebtActive, ReadyRunAuxRank,
               ReadyRunDeferredRank, ReadyRunTimeoutRank,
               ReadyRunInnerRank, ReadyRunAuxOrdering,
               ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
               ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
               ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
               ReadyRunInnerCarrier, ReadyFifoDebt,
               ReadyDeferredCount, ReadyTimeoutDebt,
               ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
               RunNode, RunHistoricalRecoveryNode, RunNodeWork,
               RunHistoricalServer, CandidateScheduled,
               CandidateInFlight, CausalHeadCanAdvance,
               CanEnqueueClass, AsyncQueueDepth, NodeQueueNonempty,
               CausalCandidates, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>2. CASE AsyncTick
        BY <1>1, <2>2, <3>2,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership,
           Stage4CapacityRankInCarrier, Isa
           DEF Stage6NonCompletionCapacityStepResult,
               Stage6NonCompletionCapacityStrictResult,
               Stage6NonCompletionCapacityProgress,
               Stage6NonCompletionCapacityGoal,
               Stage6NonCompletionCapacityBlockedAtRank,
               Stage4CapacityRank, Stage4CapacityOrdering,
               Stage4CapacityCarrier, ProtectedStage6Pending,
               ProtectedOwnedAtServiceRank,
               ProtectedRankProgressExit,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, CausalCandidatePosition,
               LocalSourceDistance, CausalCommandCapacityDebt,
               CausalHeadCommandLimit,
               NonCompletionCausalAdmissionDebt,
               CausalAdmissionDebtActive, ReadyRunAuxRank,
               ReadyRunDeferredRank, ReadyRunTimeoutRank,
               ReadyRunInnerRank, ReadyRunAuxOrdering,
               ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
               ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
               ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
               ReadyRunInnerCarrier, ReadyFifoDebt,
               ReadyDeferredCount, ReadyTimeoutDebt,
               ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
               AsyncTick, AsyncNonClockVars, CandidateScheduled,
               CandidateInFlight, CausalHeadCanAdvance,
               CanEnqueueClass, AsyncQueueDepth, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>3. CASE \E node \in ValidatorIds:
                    OpenHistoricalRecovery(node)
        BY <1>1, <2>2, <3>3,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership, Isa
           DEF Stage6NonCompletionCapacityStepResult,
               Stage6NonCompletionCapacityStrictResult,
               Stage6NonCompletionCapacityProgress,
               Stage6NonCompletionCapacityGoal,
               Stage6NonCompletionCapacityBlockedAtRank,
               Stage4CapacityRank, ProtectedStage6Pending,
               ProtectedOwnedAtServiceRank,
               ProtectedRankProgressExit,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               CausalCommandCapacityDebt, CausalHeadCommandLimit,
               ReadyRunAuxRank, OpenHistoricalRecovery, AsyncAllVars
      <3>4. CASE \/ \E node \in AsyncCurrentResponsiveVoters:
                          DirectCommitCertificateDiscoveryStep(node)
                   \/ \E historicalNode
                          \in asyncHistoricalRecoveryTargets:
                          DirectHistoricalCommitCertificateDiscoveryStep(
                            historicalNode)
        BY <1>1, <2>2, <3>4,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership,
           Stage4CapacityRankInCarrier, Isa
           DEF Stage6NonCompletionCapacityStepResult,
               Stage6NonCompletionCapacityStrictResult,
               Stage6NonCompletionCapacityProgress,
               Stage6NonCompletionCapacityGoal,
               Stage6NonCompletionCapacityBlockedAtRank,
               Stage4CapacityRank, Stage4CapacityOrdering,
               Stage4CapacityCarrier, ProtectedStage6Pending,
               ProtectedOwnedAtServiceRank,
               ProtectedRankProgressExit,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               CausalCommandCapacityDebt, CausalHeadCommandLimit,
               ReadyRunAuxRank, ReadyRunDeferredRank,
               ReadyRunTimeoutRank, ReadyRunInnerRank,
               DirectCommitCertificateDiscoveryStep,
               DirectHistoricalCommitCertificateDiscoveryStep,
               CommitCertificateDiscoveryStepWork,
               CandidateScheduled, CandidateInFlight,
               CausalHeadCanAdvance, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>5. CASE \/ \E ioNode \in AsyncCurrentResponsiveVoters:
                          ServiceIoWorker(ioNode)
                   \/ \E historicalIoNode
                          \in asyncHistoricalRecoveryTargets:
                          ServiceHistoricalRecoveryIoWorker(
                            historicalIoNode)
                   \/ \E controlNode
                          \in AsyncCurrentResponsiveVoters:
                          EnqueueIoLocalControl(controlNode)
                   \/ \E historicalControlNode
                          \in asyncHistoricalRecoveryTargets:
                          EnqueueHistoricalRecoveryIoLocalControl(
                            historicalControlNode)
        BY <1>1, <2>2, <3>5,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership,
           Stage4CapacityRankInCarrier, HeadTailProperties,
           SequenceSetAfterAppend, Isa
           DEF Stage6NonCompletionCapacityStepResult,
               Stage6NonCompletionCapacityStrictResult,
               Stage6NonCompletionCapacityProgress,
               Stage6NonCompletionCapacityGoal,
               Stage6NonCompletionCapacityBlockedAtRank,
               Stage4CapacityRank, Stage4CapacityOrdering,
               Stage4CapacityCarrier, ProtectedStage6Pending,
               ProtectedOwnedAtServiceRank,
               ProtectedRankProgressExit,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               CausalCommandCapacityDebt, CausalHeadCommandLimit,
               ReadyRunAuxRank, ReadyRunDeferredRank,
               ReadyRunTimeoutRank, ReadyRunInnerRank,
               ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
               ServiceIoWorkerWork, EnqueueIoLocalControl,
               EnqueueHistoricalRecoveryIoLocalControl,
               EnqueueIoLocalControlWork, CandidateScheduled,
               CandidateInFlight, CausalHeadCanAdvance, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>6. CASE AsyncNetworkStep \/ AsyncFaultStep
        BY <1>1, <2>2, <3>6,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership,
           Stage4CapacityRankInCarrier, Isa
           DEF Stage6NonCompletionCapacityStepResult,
               Stage6NonCompletionCapacityStrictResult,
               Stage6NonCompletionCapacityProgress,
               Stage6NonCompletionCapacityGoal,
               Stage6NonCompletionCapacityBlockedAtRank,
               Stage4CapacityRank, Stage4CapacityOrdering,
               Stage4CapacityCarrier, ProtectedStage6Pending,
               ProtectedOwnedAtServiceRank,
               ProtectedRankProgressExit,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               CausalCommandCapacityDebt, CausalHeadCommandLimit,
               NonCompletionCausalAdmissionDebt,
               CausalAdmissionDebtActive, ReadyRunAuxRank,
               ReadyRunDeferredRank, ReadyRunTimeoutRank,
               ReadyRunInnerRank, AsyncNetworkStep,
               AdmitIngressPacket, IngressItemCanDrain,
               DrainFairIngressSelected, AsyncFaultStep, PreGstCrash,
               CandidateScheduled, CandidateInFlight,
               CausalHeadCanAdvance, CanEnqueueClass,
               AsyncQueueDepth, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>7. CASE AsyncSetGST
        BY <1>1, <3>7
           DEF Stage6NonCompletionCapacityStepResult,
               Stage6NonCompletionCapacityBlockedAtRank,
               ProtectedStage6Pending,
               ProtectedOwnedAtServiceRank, AsyncSetGST
      <3>8. CASE \E node \in ValidatorIds: PreGstCrash(node)
        BY <1>1, <3>8
           DEF Stage6NonCompletionCapacityStepResult,
               Stage6NonCompletionCapacityBlockedAtRank,
               ProtectedStage6Pending,
               ProtectedOwnedAtServiceRank, PreGstCrash
      <3>9. CASE RunNode(candidate.node)
        <4>1. PostGstRunNode(candidate.node)
          BY <1>1, <2>2, <3>9, Isa
             DEF Stage6NonCompletionCapacityBlockedAtRank,
                 ProtectedStage6Pending,
                 ProtectedOwnedAtServiceRank, PostGstRunNode,
                 AsyncNext, AsyncNonCrashStep, AsyncRunnerStep
        <4> QED BY <1>1, <4>1
      <3>10. CASE RunHistoricalRecoveryNode(candidate.node)
        <4>1. RunNode(candidate.node)
          BY <1>1, <3>10
             DEF Stage6NonCompletionCapacityBlockedAtRank,
                 ProtectedStage6Pending,
                 ProtectedOwnedAtServiceRank,
                 ResponsiveProtectedCandidateOwned, RunNode,
                 RunHistoricalRecoveryNode
        <4>2. PostGstRunNode(candidate.node)
          BY <1>1, <2>2, <4>1, Isa
             DEF Stage6NonCompletionCapacityBlockedAtRank,
                 ProtectedStage6Pending,
                 ProtectedOwnedAtServiceRank, PostGstRunNode,
                 AsyncNext, AsyncNonCrashStep, AsyncRunnerStep
        <4> QED BY <1>1, <4>2
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
           <3>7, <3>8, <3>9, <3>10
           DEF AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
               AsyncNonRunnerStep
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM FairStage6NonCompletionCapacityOneStep ==
  \A initialContext, candidate, position:
    \A rank \in Stage4CapacityCarrier:
      AsyncSpecAt(initialContext)
        => (Stage6NonCompletionCapacityBlockedAtRank(
              candidate, position, rank)
              ~> Stage6NonCompletionCapacityProgress(
                   candidate, position, rank))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW rank \in Stage4CapacityCarrier
         PROVE AsyncSpecAt(initialContext)
                 => (Stage6NonCompletionCapacityBlockedAtRank(
                       candidate, position, rank)
                       ~> Stage6NonCompletionCapacityProgress(
                            candidate, position, rank))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>2. /\ Stage6NonCompletionCapacityBlockedAtRank(
                  candidate, position, rank)
             /\ ~Stage6NonCompletionCapacityProgress(
                  candidate, position, rank)
            => ENABLED
                 <<PostGstRunNode(candidate.node)>>_AsyncAllVars
      BY ProtectedOwnedCandidateEnablesFairRunNode
         DEF Stage6NonCompletionCapacityBlockedAtRank,
             ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
             Stage6NonCompletionCapacityProgress,
             Stage6NonCompletionCapacityGoal
    <2>3. /\ Stage6NonCompletionCapacityBlockedAtRank(
                  candidate, position, rank)
             /\ ~Stage6NonCompletionCapacityProgress(
                  candidate, position, rank)
             /\ <<PostGstRunNode(candidate.node)>>_AsyncAllVars
            => Stage6NonCompletionCapacityProgress(
                 candidate, position, rank)'
      BY Stage6NonCompletionCapacitySameNodeRunStrictlyProgresses,
         Isa
         DEF Stage6NonCompletionCapacityStrictResult,
             PostGstRunNode
    <2>4. Stage6NonCompletionCapacityBlockedAtRank(
              candidate, position, rank)
              /\ [AsyncNext]_AsyncAllVars
            => Stage6NonCompletionCapacityBlockedAtRank(
                 candidate, position, rank)'
                 \/ Stage6NonCompletionCapacityProgress(
                      candidate, position, rank)'
      BY Stage6NonCompletionCapacitySameNodeRunStrictlyProgresses,
         Stage6NonCompletionCapacityOtherStepPreservesOrProgresses,
         Isa
         DEF Stage6NonCompletionCapacityStepResult,
             Stage6NonCompletionCapacityStrictResult
    <2>5. CASE candidate.node \in AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(PostGstRunNode(candidate.node))
        BY <2>5 DEF AsyncSpecAt, AsyncFairnessAt
      <3> QED BY <2>2, <2>3, <2>4, <3>1, PTL
           DEF AsyncSpecAt
    <2>6. CASE candidate.node \notin AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => []~Stage6NonCompletionCapacityBlockedAtRank(
                      candidate, position, rank)
        BY <2>1, <2>6, PTL
           DEF Stage6NonCompletionCapacityBlockedAtRank,
               ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
               ResponsiveProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM FairStage6NonCompletionCapacityOpens ==
  \A initialContext, candidate, position:
    AsyncSpecAt(initialContext)
      => \A rank \in Stage4CapacityCarrier:
           Stage6NonCompletionCapacityBlockedAtRank(
             candidate, position, rank)
             ~> Stage6NonCompletionCapacityGoal(candidate, position)
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position
         PROVE AsyncSpecAt(initialContext)
                 => \A rank \in Stage4CapacityCarrier:
                      Stage6NonCompletionCapacityBlockedAtRank(
                        candidate, position, rank)
                        ~> Stage6NonCompletionCapacityGoal(
                             candidate, position)
    <2>1. AsyncSpecAt(initialContext)
             => \A rank \in Stage4CapacityCarrier:
                  Stage6NonCompletionCapacityBlockedAtRank(
                    candidate, position, rank)
                    ~> (Stage6NonCompletionCapacityGoal(
                          candidate, position)
                         \/ \E lower \in SetLessThan(
                              rank, Stage4CapacityOrdering,
                              Stage4CapacityCarrier):
                              Stage6NonCompletionCapacityBlockedAtRank(
                                candidate, position, lower))
      BY FairStage6NonCompletionCapacityOneStep
         DEF Stage6NonCompletionCapacityProgress
    <2> QED BY <2>1, Stage4CapacityOrderingIsWellFounded,
         WellFoundedLeadsTo
  <1> QED BY <1>1

(***************************************************************************
Completion capacity uses the physical I/O FIFO before the producer-ready
lane.  While Completion causal debt is active, authenticated Serve, Control,
and fresh CertifiedResponse admissions cannot append at the protected node.
Consequently a nonempty physical FIFO has a natural-number drain rank.  Once
it is empty, `AsyncOutstandingCarrierInvariant` places every remaining work
owner in a ready queue and the already proved exact Stage-4 leaf retires one
such owner.  This is the source-isolated finite-drain argument; it never
assumes that a momentarily free I/O slot remains enabled.
***************************************************************************)

Stage6CompletionCapacityGoal(candidate, position) ==
  \/ ProtectedRankProgressExit(candidate, <<6, position>>)
  \/ Stage6OwedCausalReady(candidate, position)

Stage6CompletionIoDrainGoal(candidate, position) ==
  \/ Stage6CompletionCapacityGoal(candidate, position)
  \/ AsyncIoQueueDepth(candidate.node) = 0

Stage6CompletionIoBlockedAtDepth(candidate, position, depth) ==
  /\ ProtectedStage6Pending(candidate, position)
  /\ CompletionCausalAdmissionDebt(candidate.node)
  /\ ~CausalHeadCanAdvance(candidate.node)
  /\ AsyncIoQueueDepth(candidate.node) > 0
  /\ AsyncIoQueueDepth(candidate.node) = depth

Stage6CompletionIoProgress(candidate, position, depth) ==
  \/ Stage6CompletionIoDrainGoal(candidate, position)
  \/ \E lower \in SetLessThan(depth, OpToRel(<, Nat), Nat):
       Stage6CompletionIoBlockedAtDepth(candidate, position, lower)

THEOREM Stage6CompletionIoBlockedCoreFacts ==
  \A candidate, position, depth:
    Stage6CompletionIoBlockedAtDepth(candidate, position, depth)
      => /\ candidate.node \in ValidatorIds
         /\ candidate.node \in AsyncCurrentResponsiveVoters
         /\ AsyncStrongTypeInvariant
         /\ AsyncTypeInvariant
         /\ AsyncProgressOwnershipInvariant
         /\ CompletionCausalAdmissionDebt(candidate.node)
         /\ ~CausalHeadCanAdvance(candidate.node)
         /\ AsyncIoQueueDepth(candidate.node) \in Nat \ {0}
         /\ depth \in Nat \ {0}
PROOF
  <1>1. ASSUME NEW candidate, NEW position, NEW depth,
                Stage6CompletionIoBlockedAtDepth(
                  candidate, position, depth)
         PROVE /\ candidate.node \in ValidatorIds
               /\ candidate.node \in AsyncCurrentResponsiveVoters
               /\ AsyncStrongTypeInvariant
               /\ AsyncTypeInvariant
               /\ AsyncProgressOwnershipInvariant
               /\ CompletionCausalAdmissionDebt(candidate.node)
               /\ ~CausalHeadCanAdvance(candidate.node)
               /\ AsyncIoQueueDepth(candidate.node) \in Nat \ {0}
               /\ depth \in Nat \ {0}
    <2>1. /\ ProtectedStage6Pending(candidate, position)
           /\ AsyncIoQueueDepth(candidate.node) > 0
           /\ AsyncIoQueueDepth(candidate.node) = depth
      BY <1>1 DEF Stage6CompletionIoBlockedAtDepth
    <2>2. /\ candidate.node \in ValidatorIds
           /\ candidate.node \in AsyncCurrentResponsiveVoters
      BY <2>1, ProtectedStage6CarrierFacts
    <2>3. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
      BY <2>1 DEF ProtectedStage6Pending
    <2>4. AsyncTypeInvariant
      BY <2>3, AsyncStrongTypeProjectsAsyncType
    <2>5. AsyncIoSequenceTyped(asyncIoQueues[candidate.node])
      BY <2>2, <2>4
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant
    <2>6. AsyncIoQueueDepth(candidate.node) \in Nat \ {0}
      BY <2>1, <2>5, LenProperties, SMT
         DEF AsyncIoSequenceTyped, AsyncIoQueueDepth
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>6
         DEF Stage6CompletionIoBlockedAtDepth
  <1> QED BY <1>1

THEOREM ServiceIoWorkerDropsQueueDepth ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ ServiceIoWorkerWork(node)
    => AsyncIoQueueDepth(node)' + 1 = AsyncIoQueueDepth(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                ServiceIoWorkerWork(node)
         PROVE AsyncIoQueueDepth(node)' + 1
                 = AsyncIoQueueDepth(node)
    <2>1. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
           /\ AsyncIoQueueDepth(node) > 0
           /\ asyncIoQueues'[node] = Tail(asyncIoQueues[node])
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant,
             ServiceIoWorkerWork
    <2>2. /\ asyncIoQueues[node]
                  \in Seq(Range(asyncIoQueues[node]))
           /\ asyncIoQueues[node] # <<>>
           /\ Len(Tail(asyncIoQueues[node])) + 1
                  = Len(asyncIoQueues[node])
      BY <2>1, PositiveSequenceIsNonempty,
         HeadTailProperties, SMT
         DEF AsyncIoSequenceTyped, AsyncIoQueueDepth
    <2> QED BY <2>1, <2>2 DEF AsyncIoQueueDepth
  <1> QED BY <1>1

THEOREM Stage6CompletionIoWorkerStrictlyProgresses ==
  \A candidate, position:
    \A depth \in Nat:
    /\ Stage6CompletionIoBlockedAtDepth(
         candidate, position, depth)
    /\ [AsyncNext]_AsyncAllVars
    /\ PostGstServiceIoWorker(candidate.node)
    => Stage6CompletionIoProgress(candidate, position, depth)'
PROOF
  <1>1. ASSUME NEW candidate, NEW position, NEW depth \in Nat,
                Stage6CompletionIoBlockedAtDepth(
                  candidate, position, depth),
                [AsyncNext]_AsyncAllVars,
                PostGstServiceIoWorker(candidate.node)
         PROVE Stage6CompletionIoProgress(
                 candidate, position, depth)'
    <2>1. /\ candidate.node \in ValidatorIds
           /\ AsyncTypeInvariant
           /\ AsyncIoQueueDepth(candidate.node) \in Nat \ {0}
           /\ AsyncIoQueueDepth(candidate.node) = depth
      BY <1>1, Stage6CompletionIoBlockedCoreFacts
    <2>2. ServiceIoWorkerWork(candidate.node)
      BY <1>1 DEF PostGstServiceIoWorker, ServiceIoWorker
    <2>3. AsyncIoQueueDepth(candidate.node)' + 1 = depth
      BY <2>1, <2>2, ServiceIoWorkerDropsQueueDepth
    <2>4. CASE Stage6CompletionIoDrainGoal(candidate, position)'
      BY <2>4 DEF Stage6CompletionIoProgress
    <2>5. CASE ~Stage6CompletionIoDrainGoal(candidate, position)'
      <3>1. /\ ProtectedStage6Pending(candidate, position)'
             /\ CompletionCausalAdmissionDebt(candidate.node)'
             /\ ~CausalHeadCanAdvance(candidate.node)'
             /\ AsyncIoQueueDepth(candidate.node)' > 0
        BY <1>1, <2>5,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership, Isa
           DEF Stage6CompletionIoDrainGoal,
               Stage6CompletionCapacityGoal,
               ProtectedStage6Pending,
               ProtectedOwnedAtServiceRank,
               ProtectedRankProgressExit,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, CausalCandidatePosition,
               CompletionCausalAdmissionDebt,
               CausalAdmissionDebtActive, PostGstServiceIoWorker,
               ServiceIoWorker, ServiceIoWorkerWork,
               LeaveCausalQueues, AsyncLocalAdmissionVars,
               CandidateScheduled, CandidateInFlight,
               CausalHeadCanAdvance, CausalCandidates,
               SequenceSet, AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>2. AsyncIoQueueDepth(candidate.node)' \in Nat \ {0}
        BY <2>3, <3>1, SMT
      <3>3. AsyncIoQueueDepth(candidate.node)'
               \in SetLessThan(depth, OpToRel(<, Nat), Nat)
        BY <2>3, <3>2, SMT DEF SetLessThan, OpToRel
      <3>4. \E lower \in SetLessThan(
                    depth, OpToRel(<, Nat), Nat):
                 /\ lower = AsyncIoQueueDepth(candidate.node)'
                 /\ Stage6CompletionIoBlockedAtDepth(
                      candidate, position, lower)'
        BY <3>1, <3>3, Isa
           DEF Stage6CompletionIoBlockedAtDepth
      <3> QED BY <3>4
           DEF Stage6CompletionIoProgress
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM Stage6CompletionIoOtherStepPreservesOrProgresses ==
  \A candidate, position:
    \A depth \in Nat:
    /\ Stage6CompletionIoBlockedAtDepth(
         candidate, position, depth)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~PostGstServiceIoWorker(candidate.node)
    => \/ Stage6CompletionIoBlockedAtDepth(
             candidate, position, depth)'
       \/ Stage6CompletionIoProgress(candidate, position, depth)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HeadTailProperties, SequenceSetAfterAppend, Isa
   DEF Stage6CompletionIoBlockedAtDepth,
       Stage6CompletionIoProgress, Stage6CompletionIoDrainGoal,
       Stage6CompletionCapacityGoal, ProtectedStage6Pending,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCandidatePosition, LocalSourceDistance,
       CompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       CandidateScheduled, CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueIoClass, AsyncIoAdmissionLimit,
       AsyncIoQueueDepth, AsyncOutstandingWorkCount,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer, OpenHistoricalRecovery,
       LocalAdmissionStep, AdmitProducerCompletion,
       AdmitCausalHead, RecordBlockedCausalDebt,
       UpdateLocalAdmissionMetadata, IngressDrainStep,
       DrainFairIngressSelected, IngressItemCanDrain,
       PopSelectedIngress, SerializedRuntimeStep, RuntimeStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, AsyncNetworkStep,
       AdmitIngressPacket, AsyncFaultStep, PreGstCrash,
       AsyncTick, AsyncNonClockVars, AsyncSetGST,
       EnqueueCandidate, AppendCausalSuccessors,
       CandidateInReadyQueue, CausalCandidates,
       TrackedWorkCandidates, ConsensusIoCandidates,
       SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM FairStage6CompletionIoOneStep ==
  \A initialContext, candidate, position:
    \A depth \in Nat:
      AsyncSpecAt(initialContext)
        => (Stage6CompletionIoBlockedAtDepth(
              candidate, position, depth)
              ~> Stage6CompletionIoProgress(
                   candidate, position, depth))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW depth \in Nat
         PROVE AsyncSpecAt(initialContext)
                 => (Stage6CompletionIoBlockedAtDepth(
                       candidate, position, depth)
                       ~> Stage6CompletionIoProgress(
                            candidate, position, depth))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>2. /\ Stage6CompletionIoBlockedAtDepth(
                  candidate, position, depth)
             /\ ~Stage6CompletionIoProgress(
                  candidate, position, depth)
            => ENABLED
                 <<PostGstServiceIoWorker(
                      candidate.node)>>_AsyncAllVars
      BY QueuedIoEnablesPostGstService,
         QueuedIoServiceIsNonstuttering, ENABLEDaxioms, Isa
         DEF Stage6CompletionIoBlockedAtDepth,
             ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
             Stage6CompletionIoProgress,
             Stage6CompletionIoDrainGoal,
             Stage6CompletionCapacityGoal,
             PostGstServiceIoWorker
    <2>3. /\ Stage6CompletionIoBlockedAtDepth(
                  candidate, position, depth)
             /\ ~Stage6CompletionIoProgress(
                  candidate, position, depth)
             /\ <<PostGstServiceIoWorker(
                    candidate.node)>>_AsyncAllVars
            => Stage6CompletionIoProgress(
                 candidate, position, depth)'
      BY Stage6CompletionIoWorkerStrictlyProgresses, Isa
         DEF PostGstServiceIoWorker
    <2>4. Stage6CompletionIoBlockedAtDepth(
              candidate, position, depth)
              /\ [AsyncNext]_AsyncAllVars
            => Stage6CompletionIoBlockedAtDepth(
                 candidate, position, depth)'
                 \/ Stage6CompletionIoProgress(
                      candidate, position, depth)'
      BY Stage6CompletionIoWorkerStrictlyProgresses,
         Stage6CompletionIoOtherStepPreservesOrProgresses, Isa
    <2>5. CASE candidate.node \in AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(
                    PostGstServiceIoWorker(candidate.node))
        BY <2>5 DEF AsyncSpecAt, AsyncFairnessAt
      <3> QED BY <2>2, <2>3, <2>4, <3>1, PTL
           DEF AsyncSpecAt
    <2>6. CASE candidate.node \notin AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => []~Stage6CompletionIoBlockedAtDepth(
                      candidate, position, depth)
        BY <2>1, <2>6, PTL
           DEF Stage6CompletionIoBlockedAtDepth,
               ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
               ResponsiveProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM FairStage6CompletionIoDrains ==
  \A initialContext, candidate, position:
    AsyncSpecAt(initialContext)
      => \A depth \in Nat:
           Stage6CompletionIoBlockedAtDepth(
             candidate, position, depth)
             ~> Stage6CompletionIoDrainGoal(candidate, position)
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position
         PROVE AsyncSpecAt(initialContext)
                 => \A depth \in Nat:
                      Stage6CompletionIoBlockedAtDepth(
                        candidate, position, depth)
                        ~> Stage6CompletionIoDrainGoal(
                             candidate, position)
    <2>1. AsyncSpecAt(initialContext)
             => \A depth \in Nat:
                  Stage6CompletionIoBlockedAtDepth(
                    candidate, position, depth)
                    ~> (Stage6CompletionIoDrainGoal(
                          candidate, position)
                         \/ \E lower \in SetLessThan(
                              depth, OpToRel(<, Nat), Nat):
                              Stage6CompletionIoBlockedAtDepth(
                                candidate, position, lower))
      BY FairStage6CompletionIoOneStep
         DEF Stage6CompletionIoProgress
    <2> QED BY <2>1, NatLessThanWellFounded,
         WellFoundedLeadsTo
  <1> QED BY <1>1

THEOREM SelectedReadyCompletionFactsWithoutRuntimeCapacity ==
  \A node \in ValidatorIds:
    /\ AsyncIoTopologyTypeInvariant
    /\ AsyncIoWorkContentTypeInvariant
    /\ SelectedCompletionQueueNonempty(node)
    => LET source == SelectedCompletionSource(node)
           queue == ProducerSelectedReadyQueue(node)
           candidate == SelectedCompletionCandidate(node)
       IN /\ source \in {"Io", "Local"}
          /\ AsyncCompletionSequenceTyped(queue)
          /\ Len(queue) > 0
          /\ candidate = Head(queue)
          /\ candidate \in SequenceSet(queue)
          /\ candidate \in asyncOutstandingWork[node]
          /\ AsyncCandidateTyped(candidate)
          /\ candidate.class = "Completion"
          /\ candidate.node = node
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncIoTopologyTypeInvariant,
                AsyncIoWorkContentTypeInvariant,
                SelectedCompletionQueueNonempty(node)
         PROVE LET source == SelectedCompletionSource(node)
                   queue == ProducerSelectedReadyQueue(node)
                   candidate == SelectedCompletionCandidate(node)
               IN /\ source \in {"Io", "Local"}
                  /\ AsyncCompletionSequenceTyped(queue)
                  /\ Len(queue) > 0
                  /\ candidate = Head(queue)
                  /\ candidate \in SequenceSet(queue)
                  /\ candidate \in asyncOutstandingWork[node]
                  /\ AsyncCandidateTyped(candidate)
                  /\ candidate.class = "Completion"
                  /\ candidate.node = node
    <2> DEFINE Source == SelectedCompletionSource(node)
    <2> DEFINE Queue == ProducerSelectedReadyQueue(node)
    <2> DEFINE Candidate == SelectedCompletionCandidate(node)
    <2>1. Source \in {"Io", "Local"}
      BY <1>1, SMT DEF Source, SelectedCompletionSource,
                        AsyncIoTopologyTypeInvariant
    <2>2. CASE Source = "Io"
      <3>1. /\ Queue = asyncIoReadyCompletions[node]
             /\ Candidate = Head(asyncIoReadyCompletions[node])
             /\ Len(asyncIoReadyCompletions[node]) > 0
        BY <1>1, <2>2
           DEF Source, Queue, Candidate,
               ProducerSelectedReadyQueue,
               SelectedCompletionQueueNonempty,
               SelectedCompletionCandidate
      <3>2. /\ AsyncCompletionSequenceTyped(Queue)
             /\ SequenceSet(Queue)
                  \subseteq asyncOutstandingWork[node]
             /\ \A work \in asyncOutstandingWork[node]:
                    /\ AsyncCandidateTyped(work)
                    /\ work.class = "Completion"
                    /\ work.node = node
        BY <1>1, <3>1 DEF AsyncIoWorkContentTypeInvariant
      <3>3. Candidate \in SequenceSet(Queue)
        BY <3>1, <3>2, NonemptySequenceHeadIsFirst
           DEF AsyncCompletionSequenceTyped, SequenceSet
      <3> QED BY <2>1, <3>1, <3>2, <3>3
    <2>3. CASE Source = "Local"
      <3>1. /\ Queue = asyncLocalReadyCompletions[node]
             /\ Candidate = Head(asyncLocalReadyCompletions[node])
             /\ Len(asyncLocalReadyCompletions[node]) > 0
        BY <1>1, <2>3
           DEF Source, Queue, Candidate,
               ProducerSelectedReadyQueue,
               SelectedCompletionQueueNonempty,
               SelectedCompletionCandidate
      <3>2. /\ AsyncCompletionSequenceTyped(Queue)
             /\ SequenceSet(Queue)
                  \subseteq asyncOutstandingWork[node]
             /\ \A work \in asyncOutstandingWork[node]:
                    /\ AsyncCandidateTyped(work)
                    /\ work.class = "Completion"
                    /\ work.node = node
        BY <1>1, <3>1 DEF AsyncIoWorkContentTypeInvariant
      <3>3. Candidate \in SequenceSet(Queue)
        BY <3>1, <3>2, NonemptySequenceHeadIsFirst
           DEF AsyncCompletionSequenceTyped, SequenceSet
      <3> QED BY <2>1, <3>1, <3>2, <3>3
    <2> QED BY <2>1, <2>2, <2>3, SMT DEF Source, Queue, Candidate
  <1> QED BY <1>1

Stage6CompletionReadyBlocked(candidate, position) ==
  /\ ProtectedStage6Pending(candidate, position)
  /\ CompletionCausalAdmissionDebt(candidate.node)
  /\ ~CausalHeadCanAdvance(candidate.node)
  /\ AsyncIoQueueDepth(candidate.node) = 0

Stage6CompletionReadyWitnessBlocked(
    candidate, position, readyCandidate, readyPosition) ==
  /\ Stage6CompletionReadyBlocked(candidate, position)
  /\ readyCandidate \in AsyncCandidateSet
  /\ readyPosition \in Nat
  /\ ProtectedOwnedAtServiceRank(
       readyCandidate, <<4, readyPosition>>)

THEOREM Stage6CompletionZeroIoReadyFacts ==
  \A candidate, position:
    Stage6CompletionReadyBlocked(candidate, position)
      => /\ candidate.node \in ValidatorIds
         /\ candidate.node \in AsyncCurrentResponsiveVoters
         /\ AsyncOutstandingWorkCount(candidate.node)
               = AsyncIoWorkCapacity
         /\ SelectedCompletionQueueNonempty(candidate.node)
         /\ SelectedCompletionCandidate(candidate.node)
               \in AsyncCandidateSet
         /\ SelectedCompletionCandidate(candidate.node)
               \in asyncOutstandingWork[candidate.node]
         /\ SelectedCompletionCandidate(candidate.node).class
               = "Completion"
         /\ SelectedCompletionCandidate(candidate.node).node
               = candidate.node
         /\ CandidateInReadyQueue(
              SelectedCompletionCandidate(candidate.node))
         /\ ReadyCandidatePosition(
              SelectedCompletionCandidate(candidate.node)) \in Nat
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                Stage6CompletionReadyBlocked(candidate, position)
         PROVE /\ candidate.node \in ValidatorIds
               /\ candidate.node \in AsyncCurrentResponsiveVoters
               /\ AsyncOutstandingWorkCount(candidate.node)
                     = AsyncIoWorkCapacity
               /\ SelectedCompletionQueueNonempty(candidate.node)
               /\ SelectedCompletionCandidate(candidate.node)
                     \in AsyncCandidateSet
               /\ SelectedCompletionCandidate(candidate.node)
                     \in asyncOutstandingWork[candidate.node]
               /\ SelectedCompletionCandidate(candidate.node).class
                     = "Completion"
               /\ SelectedCompletionCandidate(candidate.node).node
                     = candidate.node
               /\ CandidateInReadyQueue(
                    SelectedCompletionCandidate(candidate.node))
               /\ ReadyCandidatePosition(
                    SelectedCompletionCandidate(candidate.node)) \in Nat
    <2> DEFINE Node == candidate.node
    <2> DEFINE Ready == SelectedCompletionCandidate(Node)
    <2>1. /\ ProtectedStage6Pending(candidate, position)
           /\ CompletionCausalAdmissionDebt(Node)
           /\ ~CausalHeadCanAdvance(Node)
           /\ AsyncIoQueueDepth(Node) = 0
      BY <1>1 DEF Stage6CompletionReadyBlocked, Node
    <2>2. /\ Node \in ValidatorIds
           /\ Node \in AsyncCurrentResponsiveVoters
           /\ AsyncStrongTypeInvariant
           /\ AsyncTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ CausalQueueNonempty(Node)
           /\ HeadCausalCandidate(Node).class = "Completion"
      BY <2>1, ProtectedStage6CarrierFacts,
         AsyncStrongTypeProjectsAsyncType
         DEF ProtectedStage6Pending,
             CompletionCausalAdmissionDebt,
             CausalAdmissionDebtActive
    <2>3. ~CandidateInFlight(HeadCausalCandidate(Node))
      BY <2>2, OwnedCausalHeadIsNotInFlight
    <2>4. CanEnqueueIoClass(Node, "Consensus")
      BY <2>1, <2>2, SMT
         DEF CanEnqueueIoClass, AsyncIoAdmissionLimit,
             AsyncIoQueueDepth, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
    <2>5. ~(AsyncOutstandingWorkCount(Node)
                  < AsyncIoWorkCapacity)
      BY <2>1, <2>2, <2>3, <2>4
         DEF CausalHeadCanAdvance
    <2>6. /\ AsyncOutstandingWorkCount(Node)
                  <= AsyncIoWorkCapacity
           /\ AsyncIoWorkCapacity \in Nat \ {0}
      BY <2>2
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoCapacityTypeInvariant,
             AsyncConfiguration
    <2>7. /\ AsyncOutstandingWorkCount(Node)
                  = AsyncIoWorkCapacity
           /\ AsyncOutstandingWorkCount(Node) > 0
      BY <2>5, <2>6, SMT
    <2>8. ConsensusIoCandidates(Node) = {}
      BY <2>1, Isa
         DEF ConsensusIoCandidates, AsyncIoQueueDepth, SequenceSet
    <2>9. asyncOutstandingWork[Node] =
             SequenceSet(asyncIoReadyCompletions[Node])
               \cup SequenceSet(asyncLocalReadyCompletions[Node])
      BY <2>2, <2>8 DEF AsyncProgressOwnershipInvariant,
                              AsyncOutstandingCarrierInvariant
    <2>10. asyncOutstandingWork[Node] # {}
      BY <2>2, <2>7, FS_CardinalityType, FS_EmptySet, SMT
         DEF AsyncOutstandingWorkCount, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant,
             AsyncIoWorkContentTypeInvariant
    <2>11. SelectedCompletionQueueNonempty(Node)
      BY <2>9, <2>10, Isa
         DEF SelectedCompletionQueueNonempty,
             SelectedCompletionSource, SequenceSet
    <2>12. /\ AsyncIoTopologyTypeInvariant
            /\ AsyncIoWorkContentTypeInvariant
      BY <2>2 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                    AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>13. /\ Ready \in asyncOutstandingWork[Node]
            /\ AsyncCandidateTyped(Ready)
            /\ Ready.class = "Completion"
            /\ Ready.node = Node
            /\ Ready \in
                 SequenceSet(asyncIoReadyCompletions[Node])
                   \cup SequenceSet(asyncLocalReadyCompletions[Node])
      BY <2>11, <2>12,
         SelectedReadyCompletionFactsWithoutRuntimeCapacity, Isa
         DEF Ready, ProducerSelectedReadyQueue,
             SelectedCompletionSource
    <2>14. Ready \in AsyncCandidateSet
      BY <2>13, Isa DEF AsyncCandidateTyped, AsyncCandidateSet,
                           AsyncCandidateDomain
    <2>15. /\ CandidateScheduled(Ready)
            /\ CandidateInReadyQueue(Ready)
      BY <2>13, Isa
         DEF CandidateScheduled, TrackedWorkCandidates,
             CandidateInReadyQueue
    <2>16. ReadyCandidatePosition(Ready) \in Nat
      BY <2>2, <2>15, ReadyCandidatePositionIsNatural
    <2> QED BY <2>2, <2>7, <2>11, <2>13, <2>14, <2>15, <2>16
         DEF Node, Ready
  <1> QED BY <1>1

THEOREM Stage6CompletionReadyWitnessExists ==
  \A candidate, position:
    Stage6CompletionReadyBlocked(candidate, position)
      => \E readyCandidate \in AsyncCandidateSet,
            readyPosition \in Nat:
           Stage6CompletionReadyWitnessBlocked(
             candidate, position, readyCandidate, readyPosition)
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                Stage6CompletionReadyBlocked(candidate, position)
         PROVE \E readyCandidate \in AsyncCandidateSet,
                   readyPosition \in Nat:
                  Stage6CompletionReadyWitnessBlocked(
                    candidate, position,
                    readyCandidate, readyPosition)
    <2> DEFINE Ready == SelectedCompletionCandidate(candidate.node)
    <2> DEFINE Position == ReadyCandidatePosition(Ready)
    <2>1. /\ candidate.node \in ValidatorIds
           /\ candidate.node \in AsyncCurrentResponsiveVoters
           /\ Ready \in AsyncCandidateSet
           /\ Ready \in asyncOutstandingWork[candidate.node]
           /\ CandidateInReadyQueue(Ready)
           /\ Position \in Nat
      BY <1>1, Stage6CompletionZeroIoReadyFacts
         DEF Ready, Position
    <2>2. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ gst
           /\ ~NodeHasApplication(candidate.node)
      BY <1>1
         DEF Stage6CompletionReadyBlocked,
             ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned
    <2>3. /\ Ready.node = candidate.node
           /\ Ready.class = "Completion"
      BY <1>1, <2>1, Stage6CompletionZeroIoReadyFacts,
         SelectedReadyCompletionFactsWithoutRuntimeCapacity, Isa
         DEF Ready, Stage6CompletionReadyBlocked,
             ProtectedStage6Pending, AsyncStrongTypeInvariant,
             StrongInductiveInvariant, Safety, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant
    <2>4. /\ Ready \notin QueuedCandidates
           /\ Ready \notin DeferredCandidates
           /\ Ready \in TrackedWorkCandidates
      BY <2>1, <2>2, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant,
             TrackedWorkCandidates
    <2>5. CandidateServiceRank(Ready) = <<4, Position>>
      BY <2>1, <2>4, Isa
         DEF CandidateServiceRank, ReadyCandidatePosition
    <2>6. ResponsiveProtectedCandidateOwned(Ready)
      BY <2>1, <2>2, <2>3, <2>4, Isa
         DEF ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned, ProtectedServiceCandidate,
             CandidateScheduled
    <2>7. ProtectedOwnedAtServiceRank(Ready, <<4, Position>>)
      BY <2>2, <2>5, <2>6 DEF ProtectedOwnedAtServiceRank
    <2>8. Stage6CompletionReadyWitnessBlocked(
             candidate, position, Ready, Position)
      BY <1>1, <2>1, <2>7
         DEF Stage6CompletionReadyWitnessBlocked
    <2> QED BY <2>1, <2>8
  <1> QED BY <1>1

THEOREM Stage6CompletionReadyRankProgressOpensCapacity ==
  \A candidate, position, readyCandidate, readyPosition:
    /\ Stage6CompletionReadyWitnessBlocked(
         candidate, position, readyCandidate, readyPosition)
    /\ [AsyncNext]_AsyncAllVars
    /\ ProtectedRankProgressExit(
         readyCandidate, <<4, readyPosition>>)'
    => Stage6CompletionCapacityGoal(candidate, position)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   FS_RemoveElement, FS_CardinalityType,
   HeadTailProperties, SequenceSetAfterAppend, Isa
   DEF Stage6CompletionReadyWitnessBlocked,
       Stage6CompletionReadyBlocked,
       Stage6CompletionCapacityGoal, ProtectedStage6Pending,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       ProtectedServiceCandidate, CandidateServiceRank,
       ServiceRankLess, ReadyCandidatePosition,
       ReadyCandidateSource, ReadyCompletionQueue,
       LocalSourceDistance, PreferredLocalSource,
       CompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       CandidateScheduled, CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueIoClass, AsyncIoAdmissionLimit,
       AsyncIoQueueDepth, AsyncOutstandingWorkCount,
       SelectedCompletionSource, SelectedCompletionCandidate,
       SelectedCompletionQueueNonempty, ProducerCompletionCanAdmit,
       ProducerCompletionCanAdvance,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer, OpenHistoricalRecovery,
       LocalAdmissionStep, AdmitProducerCompletion,
       AdmitCausalHead, RecordBlockedCausalDebt,
       UpdateLocalAdmissionMetadata, IngressDrainStep,
       DrainFairIngressSelected, IngressItemCanDrain,
       PopSelectedIngress, SerializedRuntimeStep, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, AsyncNetworkStep,
       AdmitIngressPacket, AsyncFaultStep, PreGstCrash,
       AsyncTick, AsyncNonClockVars, AsyncSetGST,
       EnqueueCandidate, AppendCausalSuccessors,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       SequenceWithoutIndex, DeferCommand, DiscardCommand,
       CandidateInReadyQueue, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       ConsensusIoCandidates, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage6CompletionReadyWitnessUnlessCapacity ==
  \A candidate, position, readyCandidate, readyPosition:
    /\ Stage6CompletionReadyWitnessBlocked(
         candidate, position, readyCandidate, readyPosition)
    /\ [AsyncNext]_AsyncAllVars
    => \/ Stage6CompletionReadyWitnessBlocked(
             candidate, position, readyCandidate, readyPosition)'
       \/ Stage6CompletionCapacityGoal(candidate, position)'
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                NEW readyCandidate, NEW readyPosition,
                Stage6CompletionReadyWitnessBlocked(
                  candidate, position,
                  readyCandidate, readyPosition),
                [AsyncNext]_AsyncAllVars
         PROVE \/ Stage6CompletionReadyWitnessBlocked(
                      candidate, position,
                      readyCandidate, readyPosition)'
               \/ Stage6CompletionCapacityGoal(
                    candidate, position)'
    <2>1. ProtectedStage4Pending(readyCandidate, readyPosition)
      BY <1>1
         DEF Stage6CompletionReadyWitnessBlocked,
             Stage6CompletionReadyBlocked,
             ProtectedStage6Pending, ProtectedStage4Pending
    <2>2. \/ ProtectedStage4Pending(
                 readyCandidate, readyPosition)'
           \/ ProtectedRankProgressExit(
                readyCandidate, <<4, readyPosition>>)'
      BY <1>1, <2>1, ProtectedStage4UnlessProgress
    <2>3. CASE ProtectedRankProgressExit(
                  readyCandidate, <<4, readyPosition>>)'
      BY <1>1, <2>3,
         Stage6CompletionReadyRankProgressOpensCapacity
    <2>4. CASE ProtectedStage4Pending(
                  readyCandidate, readyPosition)'
      <3>1. CASE Stage6CompletionCapacityGoal(
                    candidate, position)'
        BY <3>1
      <3>2. CASE ~Stage6CompletionCapacityGoal(
                    candidate, position)'
        <4>1. Stage6CompletionReadyBlocked(candidate, position)'
          BY <1>1, <3>2,
             AsyncBracketNextPreservesStrongTypeInvariant,
             AsyncBracketNextPreservesProgressOwnership,
             HeadTailProperties, SequenceSetAfterAppend, Isa
             DEF Stage6CompletionReadyWitnessBlocked,
                 Stage6CompletionReadyBlocked,
                 Stage6CompletionCapacityGoal,
                 ProtectedStage6Pending,
                 ProtectedOwnedAtServiceRank,
                 ProtectedRankProgressExit,
                 ProtectedServiceOwnershipExit,
                 ResponsiveProtectedCandidateOwned,
                 ProtectedCandidateOwned, CandidateServiceRank,
                 ServiceRankLess, CompletionCausalAdmissionDebt,
                 CausalAdmissionDebtActive, CandidateScheduled,
                 CandidateInFlight, CausalHeadCanAdvance,
                 CanEnqueueIoClass, AsyncIoAdmissionLimit,
                 AsyncIoQueueDepth, AsyncOutstandingWorkCount,
                 AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
                 AsyncNonRunnerStep, RunNode,
                 RunHistoricalRecoveryNode, RunNodeWork,
                 RunHistoricalServer, OpenHistoricalRecovery,
                 LocalAdmissionStep, AdmitProducerCompletion,
                 AdmitCausalHead, RecordBlockedCausalDebt,
                 UpdateLocalAdmissionMetadata, IngressDrainStep,
                 DrainFairIngressSelected, IngressItemCanDrain,
                 PopSelectedIngress, SerializedRuntimeStep,
                 RuntimeStep, DirectCommitCertificateDiscoveryStep,
                 DirectHistoricalCommitCertificateDiscoveryStep,
                 CommitCertificateDiscoveryStepWork,
                 ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
                 ServiceIoWorkerWork, EnqueueIoLocalControl,
                 EnqueueHistoricalRecoveryIoLocalControl,
                 EnqueueIoLocalControlWork, AsyncNetworkStep,
                 AdmitIngressPacket, AsyncFaultStep, PreGstCrash,
                 AsyncTick, AsyncNonClockVars, AsyncSetGST,
                 EnqueueCandidate, AppendCausalSuccessors,
                 CandidateInReadyQueue, CausalCandidates,
                 TrackedWorkCandidates, ConsensusIoCandidates,
                 SequenceSet, AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant,
                 AsyncOutstandingCarrierInvariant, AsyncAllVars
        <4>2. Stage6CompletionReadyWitnessBlocked(
                 candidate, position,
                 readyCandidate, readyPosition)'
          BY <1>1, <3>2, <4>1, <2>4
             DEF Stage6CompletionReadyWitnessBlocked,
                 ProtectedStage4Pending,
                 ProtectedOwnedAtServiceRank
        <4> QED BY <4>2
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM FairStage6CompletionReadyWitnessOpens ==
  \A initialContext, candidate, position:
    \A readyCandidate \in AsyncCandidateSet, readyPosition \in Nat:
      AsyncSpecAt(initialContext)
        => (Stage6CompletionReadyWitnessBlocked(
              candidate, position, readyCandidate, readyPosition)
              ~> Stage6CompletionCapacityGoal(candidate, position))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW readyCandidate \in AsyncCandidateSet,
                NEW readyPosition \in Nat
         PROVE AsyncSpecAt(initialContext)
                 => (Stage6CompletionReadyWitnessBlocked(
                       candidate, position,
                       readyCandidate, readyPosition)
                       ~> Stage6CompletionCapacityGoal(
                            candidate, position))
    <2>1. ProtectedStage4RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedStage4RankProgressFromFairScheduler
    <2>2. AsyncSpecAt(initialContext)
             => (ProtectedOwnedAtServiceRank(
                   readyCandidate, <<4, readyPosition>>)
                   ~> ProtectedRankProgressExit(
                        readyCandidate, <<4, readyPosition>>))
      BY <2>1
         DEF ProtectedStage4RankProgressProperty,
             ProtectedOwnedAtServiceRank
    <2>3. Stage6CompletionReadyWitnessBlocked(
              candidate, position, readyCandidate, readyPosition)
              /\ [AsyncNext]_AsyncAllVars
            => Stage6CompletionReadyWitnessBlocked(
                 candidate, position,
                 readyCandidate, readyPosition)'
                 \/ Stage6CompletionCapacityGoal(
                      candidate, position)'
      BY Stage6CompletionReadyWitnessUnlessCapacity
    <2>4. Stage6CompletionReadyWitnessBlocked(
              candidate, position, readyCandidate, readyPosition)
            => /\ ProtectedOwnedAtServiceRank(
                     readyCandidate, <<4, readyPosition>>)
               /\ ~ProtectedRankProgressExit(
                    readyCandidate, <<4, readyPosition>>)
      BY Isa
         DEF Stage6CompletionReadyWitnessBlocked,
             ProtectedRankProgressExit,
             ProtectedServiceOwnershipExit, ServiceRankLess
    <2> QED BY <2>2, <2>3, <2>4, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM FairStage6CompletionReadyCapacityOpens ==
  \A initialContext, candidate, position:
    AsyncSpecAt(initialContext)
      => (Stage6CompletionReadyBlocked(candidate, position)
            ~> Stage6CompletionCapacityGoal(candidate, position))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position
         PROVE AsyncSpecAt(initialContext)
                 => (Stage6CompletionReadyBlocked(
                       candidate, position)
                       ~> Stage6CompletionCapacityGoal(
                            candidate, position))
    <2>1. Stage6CompletionReadyBlocked(candidate, position)
             => \E readyCandidate \in AsyncCandidateSet,
                   readyPosition \in Nat:
                  Stage6CompletionReadyWitnessBlocked(
                    candidate, position,
                    readyCandidate, readyPosition)
      BY Stage6CompletionReadyWitnessExists
    <2>2. AsyncSpecAt(initialContext)
             => \A readyCandidate \in AsyncCandidateSet,
                   readyPosition \in Nat:
                  Stage6CompletionReadyWitnessBlocked(
                    candidate, position,
                    readyCandidate, readyPosition)
                    ~> Stage6CompletionCapacityGoal(
                         candidate, position)
      BY FairStage6CompletionReadyWitnessOpens
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM FairStage6CompletionCapacityOpens ==
  \A initialContext, candidate, position:
    AsyncSpecAt(initialContext)
      => ((ProtectedStage6Pending(candidate, position)
             /\ CompletionCausalAdmissionDebt(candidate.node)
             /\ ~CausalHeadCanAdvance(candidate.node))
            ~> Stage6CompletionCapacityGoal(candidate, position))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position
         PROVE AsyncSpecAt(initialContext)
                 => ((ProtectedStage6Pending(candidate, position)
                        /\ CompletionCausalAdmissionDebt(candidate.node)
                        /\ ~CausalHeadCanAdvance(candidate.node))
                       ~> Stage6CompletionCapacityGoal(
                            candidate, position))
    <2>1. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2>2. AsyncSpecAt(initialContext)
             => ((ProtectedStage6Pending(candidate, position)
                    /\ CompletionCausalAdmissionDebt(candidate.node)
                    /\ ~CausalHeadCanAdvance(candidate.node))
                   ~> (Stage6CompletionCapacityGoal(
                         candidate, position)
                        \/ Stage6CompletionReadyBlocked(
                             candidate, position)))
      <3>1. AsyncSpecAt(initialContext)
               => ((ProtectedStage6Pending(candidate, position)
                      /\ CompletionCausalAdmissionDebt(candidate.node)
                      /\ ~CausalHeadCanAdvance(candidate.node)
                      /\ AsyncIoQueueDepth(candidate.node) > 0)
                     ~> Stage6CompletionIoDrainGoal(
                          candidate, position))
        <4>1. AsyncSpecAt(initialContext)
                 => ((ProtectedStage6Pending(candidate, position)
                        /\ CompletionCausalAdmissionDebt(candidate.node)
                        /\ ~CausalHeadCanAdvance(candidate.node)
                        /\ AsyncIoQueueDepth(candidate.node) > 0)
                       ~> Stage6CompletionIoBlockedAtDepth(
                            candidate, position,
                            AsyncIoQueueDepth(candidate.node)))
          BY <2>1, PTL DEF Stage6CompletionIoBlockedAtDepth
        <4>2. AsyncSpecAt(initialContext)
                 => \A depth \in Nat:
                      Stage6CompletionIoBlockedAtDepth(
                        candidate, position, depth)
                        ~> Stage6CompletionIoDrainGoal(
                             candidate, position)
          BY FairStage6CompletionIoDrains
        <4> QED BY <4>1, <4>2, PTL
      <3>2. Stage6CompletionIoDrainGoal(candidate, position)
               => Stage6CompletionCapacityGoal(candidate, position)
                    \/ Stage6CompletionReadyBlocked(candidate, position)
        BY Isa
           DEF Stage6CompletionIoDrainGoal,
               Stage6CompletionReadyBlocked,
               Stage6CompletionCapacityGoal
      <3>3. (ProtectedStage6Pending(candidate, position)
               /\ CompletionCausalAdmissionDebt(candidate.node)
               /\ ~CausalHeadCanAdvance(candidate.node)
               /\ AsyncIoQueueDepth(candidate.node) = 0)
              => Stage6CompletionReadyBlocked(candidate, position)
        BY DEF Stage6CompletionReadyBlocked
      <3> QED BY <3>1, <3>2, <3>3, PTL
    <2>3. AsyncSpecAt(initialContext)
             => (Stage6CompletionReadyBlocked(candidate, position)
                   ~> Stage6CompletionCapacityGoal(
                        candidate, position))
      BY FairStage6CompletionReadyCapacityOpens
    <2> QED BY <2>2, <2>3, PTL
  <1> QED BY <1>1

(***************************************************************************
Once a capacity proof exposes an admissible head, sticky causal debt makes
that head the deterministic Local source.  The auxiliary runner rank is still
needed because the concrete runner may currently be in Ingress or Runtime;
weak fairness over `RunNode` alone does not collapse those finite prefixes.
***************************************************************************)

Stage6OwedReadyBlockedAtAux(candidate, position, rank) ==
  /\ Stage6OwedCausalReady(candidate, position)
  /\ ~ProtectedRankProgressExit(candidate, <<6, position>>)
  /\ ReadyRunAuxRank(candidate.node) = rank

Stage6OwedReadyAuxProgress(candidate, position, rank) ==
  \/ ProtectedRankProgressExit(candidate, <<6, position>>)
  \/ \E lower \in SetLessThan(
       rank, ReadyRunAuxOrdering, ReadyRunAuxCarrier):
       Stage6OwedReadyBlockedAtAux(candidate, position, lower)

THEOREM Stage6OwedReadySameNodeRunStrictlyProgresses ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ Stage6OwedReadyBlockedAtAux(candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ PostGstRunNode(candidate.node)
    => Stage6OwedReadyAuxProgress(candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   IngressDrainStrictlyDecreasesRuntimeReach,
   Stage6CausalAdmissionStrictlyProgresses,
   ReadyRunAuxRankInCarrier, HeadTailProperties,
   SequenceSetAfterAppend, Isa
   DEF Stage6OwedReadyAuxProgress,
       Stage6OwedReadyBlockedAtAux, Stage6OwedCausalReady,
       ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
       ProtectedRankProgressExit, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCandidatePosition, LocalSourceDistance,
       PreferredLocalSource, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering,
       ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
       ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
       ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
       ReadyRunInnerCarrier, ReadyFifoDebt,
       ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       CausalAdmissionDebtActive, NonCompletionCausalAdmissionDebt,
       CompletionCausalAdmissionDebt, CandidateScheduled,
       CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueClass, CanEnqueueIoClass,
       AsyncQueueDepth, AsyncIoQueueDepth,
       AsyncOutstandingWorkCount,
       PostGstRunNode, RunNode, LocalAdmissionStep,
       LocalAdmissionCanAdvance, SelectedLocalSource,
       LocalSourceCanAdmit, AdmitProducerCompletion,
       AdmitCausalHead, UpdateLocalAdmissionMetadata,
       RecordBlockedCausalDebt, IngressDrainStep,
       DrainFairIngressSelected, IngressItemCanDrain,
       SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       CandidateInReadyQueue, CausalCandidates,
       TrackedWorkCandidates, ConsensusIoCandidates,
       SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage6OwedReadyOtherStepPreservesOrProgresses ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ Stage6OwedReadyBlockedAtAux(candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~PostGstRunNode(candidate.node)
    => \/ Stage6OwedReadyBlockedAtAux(
             candidate, position, rank)'
       \/ Stage6OwedReadyAuxProgress(candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   ReadyRunAuxRankInCarrier, HeadTailProperties,
   SequenceSetAfterAppend, Isa
   DEF Stage6OwedReadyAuxProgress,
       Stage6OwedReadyBlockedAtAux, Stage6OwedCausalReady,
       ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
       ProtectedRankProgressExit, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCandidatePosition, LocalSourceDistance,
       PreferredLocalSource, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering,
       ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
       ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
       ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
       ReadyRunInnerCarrier, ReadyFifoDebt,
       ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       CausalAdmissionDebtActive, NonCompletionCausalAdmissionDebt,
       CompletionCausalAdmissionDebt, CandidateScheduled,
       CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueClass, CanEnqueueIoClass,
       AsyncQueueDepth, AsyncIoQueueDepth,
       AsyncOutstandingWorkCount,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer, OpenHistoricalRecovery,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       SelectedLocalSource, LocalSourceCanAdmit,
       AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
       IngressDrainStep, DrainFairIngressSelected,
       IngressItemCanDrain, PopSelectedIngress,
       SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, AsyncNetworkStep,
       AdmitIngressPacket, AsyncFaultStep, PreGstCrash,
       AsyncTick, AsyncNonClockVars, AsyncSetGST,
       EnqueueCandidate, AppendCausalSuccessors,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       SequenceWithoutIndex, DeferCommand, DiscardCommand,
       CandidateInReadyQueue, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       ConsensusIoCandidates, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM FairStage6OwedReadyAuxOneStep ==
  \A initialContext, candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      AsyncSpecAt(initialContext)
        => (Stage6OwedReadyBlockedAtAux(
              candidate, position, rank)
              ~> Stage6OwedReadyAuxProgress(
                   candidate, position, rank))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW rank \in ReadyRunAuxCarrier
         PROVE AsyncSpecAt(initialContext)
                 => (Stage6OwedReadyBlockedAtAux(
                       candidate, position, rank)
                       ~> Stage6OwedReadyAuxProgress(
                            candidate, position, rank))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>2. /\ Stage6OwedReadyBlockedAtAux(
                  candidate, position, rank)
             /\ ~Stage6OwedReadyAuxProgress(
                  candidate, position, rank)
            => ENABLED
                 <<PostGstRunNode(candidate.node)>>_AsyncAllVars
      BY ProtectedOwnedCandidateEnablesFairRunNode
         DEF Stage6OwedReadyBlockedAtAux,
             Stage6OwedCausalReady, ProtectedStage6Pending,
             ProtectedOwnedAtServiceRank,
             Stage6OwedReadyAuxProgress
    <2>3. /\ Stage6OwedReadyBlockedAtAux(
                  candidate, position, rank)
             /\ ~Stage6OwedReadyAuxProgress(
                  candidate, position, rank)
             /\ <<PostGstRunNode(candidate.node)>>_AsyncAllVars
            => Stage6OwedReadyAuxProgress(
                 candidate, position, rank)'
      BY Stage6OwedReadySameNodeRunStrictlyProgresses, Isa
         DEF PostGstRunNode
    <2>4. Stage6OwedReadyBlockedAtAux(
              candidate, position, rank)
              /\ [AsyncNext]_AsyncAllVars
            => Stage6OwedReadyBlockedAtAux(
                 candidate, position, rank)'
                 \/ Stage6OwedReadyAuxProgress(
                      candidate, position, rank)'
      BY Stage6OwedReadySameNodeRunStrictlyProgresses,
         Stage6OwedReadyOtherStepPreservesOrProgresses, Isa
    <2>5. CASE candidate.node \in AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(PostGstRunNode(candidate.node))
        BY <2>5 DEF AsyncSpecAt, AsyncFairnessAt
      <3> QED BY <2>2, <2>3, <2>4, <3>1, PTL
           DEF AsyncSpecAt
    <2>6. CASE candidate.node \notin AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => []~Stage6OwedReadyBlockedAtAux(
                      candidate, position, rank)
        BY <2>1, <2>6, PTL
           DEF Stage6OwedReadyBlockedAtAux,
               Stage6OwedCausalReady, ProtectedStage6Pending,
               ProtectedOwnedAtServiceRank,
               ResponsiveProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM FairStage6OwedCausalAdmission ==
  \A initialContext, candidate, position:
    AsyncSpecAt(initialContext)
      => (Stage6OwedCausalReady(candidate, position)
            ~> ProtectedRankProgressExit(candidate, <<6, position>>))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position
         PROVE AsyncSpecAt(initialContext)
                 => (Stage6OwedCausalReady(candidate, position)
                       ~> ProtectedRankProgressExit(
                            candidate, <<6, position>>))
    <2>1. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2>2. AsyncSpecAt(initialContext)
             => (Stage6OwedCausalReady(candidate, position)
                   ~> Stage6OwedReadyBlockedAtAux(
                        candidate, position,
                        ReadyRunAuxRank(candidate.node)))
      BY <2>1, ReadyRunAuxRankInCarrier, PTL
         DEF Stage6OwedReadyBlockedAtAux
    <2>3. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  Stage6OwedReadyBlockedAtAux(
                    candidate, position, rank)
                    ~> (ProtectedRankProgressExit(
                          candidate, <<6, position>>)
                         \/ \E lower \in SetLessThan(
                              rank, ReadyRunAuxOrdering,
                              ReadyRunAuxCarrier):
                              Stage6OwedReadyBlockedAtAux(
                                candidate, position, lower))
      BY FairStage6OwedReadyAuxOneStep
         DEF Stage6OwedReadyAuxProgress
    <2>4. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  Stage6OwedReadyBlockedAtAux(
                    candidate, position, rank)
                    ~> ProtectedRankProgressExit(
                         candidate, <<6, position>>)
      BY <2>3, ReadyRunAuxOrderingIsWellFounded,
         WellFoundedLeadsTo
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

Stage6NonCompletionCapacityBlocked(candidate, position) ==
  /\ ProtectedStage6Pending(candidate, position)
  /\ NonCompletionCausalAdmissionDebt(candidate.node)
  /\ ~CausalHeadCanAdvance(candidate.node)

Stage6CompletionCapacityBlocked(candidate, position) ==
  /\ ProtectedStage6Pending(candidate, position)
  /\ CompletionCausalAdmissionDebt(candidate.node)
  /\ ~CausalHeadCanAdvance(candidate.node)

Stage6PreAdmissionGoal(candidate, position) ==
  \/ ProtectedRankProgressExit(candidate, <<6, position>>)
  \/ Stage6OwedCausalReady(candidate, position)
  \/ Stage6NonCompletionCapacityBlocked(candidate, position)
  \/ Stage6CompletionCapacityBlocked(candidate, position)

Stage6PreAdmissionBlockedAtAux(candidate, position, rank) ==
  /\ ProtectedStage6Pending(candidate, position)
  /\ ~Stage6PreAdmissionGoal(candidate, position)
  /\ ReadyRunAuxRank(candidate.node) = rank

Stage6PreAdmissionAuxProgress(candidate, position, rank) ==
  \/ Stage6PreAdmissionGoal(candidate, position)
  \/ \E lower \in SetLessThan(
       rank, ReadyRunAuxOrdering, ReadyRunAuxCarrier):
       Stage6PreAdmissionBlockedAtAux(candidate, position, lower)

THEOREM Stage6PreAdmissionSameNodeRunStrictlyProgresses ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ Stage6PreAdmissionBlockedAtAux(candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ PostGstRunNode(candidate.node)
    => Stage6PreAdmissionAuxProgress(candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   IngressDrainStrictlyDecreasesRuntimeReach,
   Stage6CausalAdmissionStrictlyProgresses,
   ReadyRunAuxRankInCarrier, HeadTailProperties,
   SequenceSetAfterAppend, Isa
   DEF Stage6PreAdmissionAuxProgress,
       Stage6PreAdmissionBlockedAtAux, Stage6PreAdmissionGoal,
       Stage6OwedCausalReady,
       Stage6NonCompletionCapacityBlocked,
       Stage6CompletionCapacityBlocked,
       ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
       ProtectedRankProgressExit, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCandidatePosition, LocalSourceDistance,
       PreferredLocalSource, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering,
       ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
       ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
       ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
       ReadyRunInnerCarrier, ReadyFifoDebt,
       ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       CausalAdmissionDebtActive, NonCompletionCausalAdmissionDebt,
       CompletionCausalAdmissionDebt, CandidateScheduled,
       CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueClass, CanEnqueueIoClass,
       AsyncQueueDepth, AsyncIoQueueDepth,
       AsyncOutstandingWorkCount,
       PostGstRunNode, RunNode, LocalAdmissionStep,
       LocalAdmissionCanAdvance, SelectedLocalSource,
       LocalSourceCanAdmit, AdmitProducerCompletion,
       AdmitCausalHead, UpdateLocalAdmissionMetadata,
       RecordBlockedCausalDebt, IngressDrainStep,
       DrainFairIngressSelected, IngressItemCanDrain,
       SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       CandidateInReadyQueue, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       ConsensusIoCandidates, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage6PreAdmissionOtherStepPreservesOrProgresses ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
    /\ Stage6PreAdmissionBlockedAtAux(candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~PostGstRunNode(candidate.node)
    => \/ Stage6PreAdmissionBlockedAtAux(
             candidate, position, rank)'
       \/ Stage6PreAdmissionAuxProgress(candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   ReadyRunAuxRankInCarrier, HeadTailProperties,
   SequenceSetAfterAppend, Isa
   DEF Stage6PreAdmissionAuxProgress,
       Stage6PreAdmissionBlockedAtAux, Stage6PreAdmissionGoal,
       Stage6OwedCausalReady,
       Stage6NonCompletionCapacityBlocked,
       Stage6CompletionCapacityBlocked,
       ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
       ProtectedRankProgressExit, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCandidatePosition, LocalSourceDistance,
       PreferredLocalSource, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering,
       ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
       ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
       ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
       ReadyRunInnerCarrier, ReadyFifoDebt,
       ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       CausalAdmissionDebtActive, NonCompletionCausalAdmissionDebt,
       CompletionCausalAdmissionDebt, CandidateScheduled,
       CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueClass, CanEnqueueIoClass,
       AsyncQueueDepth, AsyncIoQueueDepth,
       AsyncOutstandingWorkCount,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer, OpenHistoricalRecovery,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       SelectedLocalSource, LocalSourceCanAdmit,
       AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
       IngressDrainStep, DrainFairIngressSelected,
       IngressItemCanDrain, PopSelectedIngress,
       SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, AsyncNetworkStep,
       AdmitIngressPacket, AsyncFaultStep, PreGstCrash,
       AsyncTick, AsyncNonClockVars, AsyncSetGST,
       EnqueueCandidate, AppendCausalSuccessors,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       SequenceWithoutIndex, DeferCommand, DiscardCommand,
       CandidateInReadyQueue, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       ConsensusIoCandidates, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM FairStage6PreAdmissionAuxOneStep ==
  \A initialContext, candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      AsyncSpecAt(initialContext)
        => (Stage6PreAdmissionBlockedAtAux(
              candidate, position, rank)
              ~> Stage6PreAdmissionAuxProgress(
                   candidate, position, rank))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW rank \in ReadyRunAuxCarrier
         PROVE AsyncSpecAt(initialContext)
                 => (Stage6PreAdmissionBlockedAtAux(
                       candidate, position, rank)
                       ~> Stage6PreAdmissionAuxProgress(
                            candidate, position, rank))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>2. /\ Stage6PreAdmissionBlockedAtAux(
                  candidate, position, rank)
             /\ ~Stage6PreAdmissionAuxProgress(
                  candidate, position, rank)
            => ENABLED
                 <<PostGstRunNode(candidate.node)>>_AsyncAllVars
      BY ProtectedOwnedCandidateEnablesFairRunNode
         DEF Stage6PreAdmissionBlockedAtAux,
             ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
             Stage6PreAdmissionAuxProgress,
             Stage6PreAdmissionGoal
    <2>3. /\ Stage6PreAdmissionBlockedAtAux(
                  candidate, position, rank)
             /\ ~Stage6PreAdmissionAuxProgress(
                  candidate, position, rank)
             /\ <<PostGstRunNode(candidate.node)>>_AsyncAllVars
            => Stage6PreAdmissionAuxProgress(
                 candidate, position, rank)'
      BY Stage6PreAdmissionSameNodeRunStrictlyProgresses, Isa
         DEF PostGstRunNode
    <2>4. Stage6PreAdmissionBlockedAtAux(
              candidate, position, rank)
              /\ [AsyncNext]_AsyncAllVars
            => Stage6PreAdmissionBlockedAtAux(
                 candidate, position, rank)'
                 \/ Stage6PreAdmissionAuxProgress(
                      candidate, position, rank)'
      BY Stage6PreAdmissionSameNodeRunStrictlyProgresses,
         Stage6PreAdmissionOtherStepPreservesOrProgresses, Isa
    <2>5. CASE candidate.node \in AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(PostGstRunNode(candidate.node))
        BY <2>5 DEF AsyncSpecAt, AsyncFairnessAt
      <3> QED BY <2>2, <2>3, <2>4, <3>1, PTL
           DEF AsyncSpecAt
    <2>6. CASE candidate.node \notin AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => []~Stage6PreAdmissionBlockedAtAux(
                      candidate, position, rank)
        BY <2>1, <2>6, PTL
           DEF Stage6PreAdmissionBlockedAtAux,
               ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
               ResponsiveProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM FairStage6PreAdmissionProgress ==
  \A initialContext, candidate, position:
    AsyncSpecAt(initialContext)
      => (ProtectedStage6Pending(candidate, position)
            ~> Stage6PreAdmissionGoal(candidate, position))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position
         PROVE AsyncSpecAt(initialContext)
                 => (ProtectedStage6Pending(candidate, position)
                       ~> Stage6PreAdmissionGoal(
                            candidate, position))
    <2>1. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2>2. AsyncSpecAt(initialContext)
             => (ProtectedStage6Pending(candidate, position)
                   ~> (Stage6PreAdmissionGoal(candidate, position)
                        \/ Stage6PreAdmissionBlockedAtAux(
                             candidate, position,
                             ReadyRunAuxRank(candidate.node))))
      BY <2>1, ReadyRunAuxRankInCarrier, PTL
         DEF Stage6PreAdmissionBlockedAtAux
    <2>3. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  Stage6PreAdmissionBlockedAtAux(
                    candidate, position, rank)
                    ~> (Stage6PreAdmissionGoal(candidate, position)
                         \/ \E lower \in SetLessThan(
                              rank, ReadyRunAuxOrdering,
                              ReadyRunAuxCarrier):
                              Stage6PreAdmissionBlockedAtAux(
                                candidate, position, lower))
      BY FairStage6PreAdmissionAuxOneStep
         DEF Stage6PreAdmissionAuxProgress
    <2>4. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  Stage6PreAdmissionBlockedAtAux(
                    candidate, position, rank)
                    ~> Stage6PreAdmissionGoal(candidate, position)
      BY <2>3, ReadyRunAuxOrderingIsWellFounded,
         WellFoundedLeadsTo
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

THEOREM FairStage6NonCompletionBlockedOpens ==
  \A initialContext, candidate, position:
    AsyncSpecAt(initialContext)
      => (Stage6NonCompletionCapacityBlocked(candidate, position)
            ~> Stage6NonCompletionCapacityGoal(candidate, position))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position
         PROVE AsyncSpecAt(initialContext)
                 => (Stage6NonCompletionCapacityBlocked(
                       candidate, position)
                       ~> Stage6NonCompletionCapacityGoal(
                            candidate, position))
    <2>1. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2>2. AsyncSpecAt(initialContext)
             => (Stage6NonCompletionCapacityBlocked(
                   candidate, position)
                   ~> Stage6NonCompletionCapacityBlockedAtRank(
                        candidate, position,
                        Stage4CapacityRank(candidate.node)))
      BY <2>1, Stage4CapacityRankInCarrier, PTL
         DEF Stage6NonCompletionCapacityBlocked,
             Stage6NonCompletionCapacityBlockedAtRank
    <2>3. AsyncSpecAt(initialContext)
             => \A rank \in Stage4CapacityCarrier:
                  Stage6NonCompletionCapacityBlockedAtRank(
                    candidate, position, rank)
                    ~> Stage6NonCompletionCapacityGoal(
                         candidate, position)
      BY FairStage6NonCompletionCapacityOpens
    <2> QED BY <2>2, <2>3, PTL
  <1> QED BY <1>1

THEOREM ProtectedStage6RankProgressFromFairCausalAdmissionObligation ==
  \A initialContext:
    ProtectedStage6RankProgressProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ProtectedStage6RankProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME NEW candidate \in AsyncCandidateSet,
                  NEW position \in Nat
           PROVE AsyncSpecAt(initialContext)
                   => ((gst
                         /\ ResponsiveProtectedCandidateOwned(candidate)
                         /\ CandidateServiceRank(candidate)
                              = <<6, position>>)
                        ~> ProtectedRankProgressExit(
                             candidate, <<6, position>>))
      <3>1. AsyncSpecAt(initialContext)
               => [](AsyncStrongTypeInvariant
                      /\ AsyncProgressOwnershipInvariant)
        BY AsyncSpecAlwaysStrongTypeInvariant,
           AsyncSpecAlwaysProgressOwnershipInvariant, PTL
      <3>2. AsyncSpecAt(initialContext)
               => ((gst
                     /\ ResponsiveProtectedCandidateOwned(candidate)
                     /\ CandidateServiceRank(candidate)
                          = <<6, position>>)
                    ~> ProtectedStage6Pending(candidate, position))
        BY <3>1, PTL
           DEF ProtectedStage6Pending,
               ProtectedOwnedAtServiceRank
      <3>3. AsyncSpecAt(initialContext)
               => (ProtectedStage6Pending(candidate, position)
                     ~> Stage6PreAdmissionGoal(candidate, position))
        BY FairStage6PreAdmissionProgress
      <3>4. AsyncSpecAt(initialContext)
               => (Stage6OwedCausalReady(candidate, position)
                     ~> ProtectedRankProgressExit(
                          candidate, <<6, position>>))
        BY FairStage6OwedCausalAdmission
      <3>5. AsyncSpecAt(initialContext)
               => (Stage6NonCompletionCapacityBlocked(
                     candidate, position)
                     ~> ProtectedRankProgressExit(
                          candidate, <<6, position>>))
        <4>1. AsyncSpecAt(initialContext)
                 => (Stage6NonCompletionCapacityBlocked(
                       candidate, position)
                       ~> Stage6NonCompletionCapacityGoal(
                            candidate, position))
          BY FairStage6NonCompletionBlockedOpens
        <4> QED BY <4>1, <3>4, PTL
             DEF Stage6NonCompletionCapacityGoal
      <3>6. AsyncSpecAt(initialContext)
               => (Stage6CompletionCapacityBlocked(
                     candidate, position)
                     ~> ProtectedRankProgressExit(
                          candidate, <<6, position>>))
        <4>1. AsyncSpecAt(initialContext)
                 => (Stage6CompletionCapacityBlocked(
                       candidate, position)
                       ~> Stage6CompletionCapacityGoal(
                            candidate, position))
          BY FairStage6CompletionCapacityOpens
             DEF Stage6CompletionCapacityBlocked
        <4> QED BY <4>1, <3>4, PTL
             DEF Stage6CompletionCapacityGoal
      <3>7. AsyncSpecAt(initialContext)
               => (Stage6PreAdmissionGoal(candidate, position)
                     ~> ProtectedRankProgressExit(
                          candidate, <<6, position>>))
        BY <3>4, <3>5, <3>6, PTL
           DEF Stage6PreAdmissionGoal
      <3> QED BY <3>2, <3>3, <3>7, PTL
    <2> QED BY <2>1
         DEF ProtectedStage6RankProgressProperty,
             ProtectedRankProgressExit
  <1> QED BY <1>1

THEOREM FairProtectedStage6RankProgress ==
  \A initialContext:
    ProtectedStage6RankProgressProperty(AsyncSpecAt(initialContext))
BY ProtectedStage6RankProgressFromFairCausalAdmissionObligation

=============================================================================
