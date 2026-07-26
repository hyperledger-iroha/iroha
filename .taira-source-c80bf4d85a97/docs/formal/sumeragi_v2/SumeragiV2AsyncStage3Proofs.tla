---- MODULE SumeragiV2AsyncStage3Proofs ----
EXTENDS SumeragiV2AsyncStage4RefinementProofs

(***************************************************************************
Stage 3: cyclic runtime scheduler.

The finite queue/cursor kernel and its fair temporal shell are kept in the
production proof so the aggregate rank obligation consumes the exact leaf
rather than a scratch-only premise.
***************************************************************************)

Stage3KernelPending(candidate, position) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ ProtectedOwnedAtServiceRank(candidate, <<3, position>>)

THEOREM Stage3KernelCarrierFacts ==
  \A candidate, position:
    Stage3KernelPending(candidate, position)
      => /\ candidate.node \in AsyncCurrentResponsiveVoters
         /\ candidate.node \in ValidatorIds
         /\ candidate \in QueuedCandidates
         /\ candidate \in SequenceSet(
                              asyncCommandQueues[candidate.node])
         /\ AsyncQueueTyped(asyncCommandQueues[candidate.node])
         /\ SequenceHasUniqueValues(
              asyncCommandQueues[candidate.node])
         /\ NodeQueueNonempty(candidate.node)
         /\ SchedulerServiceRank(candidate.node, candidate) = position
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                Stage3KernelPending(candidate, position)
         PROVE /\ candidate.node \in AsyncCurrentResponsiveVoters
               /\ candidate.node \in ValidatorIds
               /\ candidate \in QueuedCandidates
               /\ candidate \in SequenceSet(
                                    asyncCommandQueues[candidate.node])
               /\ AsyncQueueTyped(asyncCommandQueues[candidate.node])
               /\ SequenceHasUniqueValues(
                    asyncCommandQueues[candidate.node])
               /\ NodeQueueNonempty(candidate.node)
               /\ SchedulerServiceRank(candidate.node, candidate) = position
    <2>1. /\ AsyncTypeInvariant
           /\ candidate.node \in AsyncCurrentResponsiveVoters
           /\ CandidateScheduled(candidate)
           /\ CandidateServiceRank(candidate) = <<3, position>>
      BY <1>1, AsyncStrongTypeProjectsAsyncType
         DEF Stage3KernelPending, ProtectedOwnedAtServiceRank,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned
    <2>2. candidate.node \in ValidatorIds
      BY <2>1, ScheduledCandidateProjectsToOwner
    <2>3. candidate \notin DeferredCandidates
      BY <2>1, SMT DEF CandidateServiceRank
    <2>4. candidate \in QueuedCandidates
      BY <2>1, <2>3, SMT
         DEF CandidateServiceRank, CandidateScheduled
    <2>5. candidate \in
             SequenceSet(asyncCommandQueues[candidate.node])
      BY <2>1, <2>4, ScheduledCandidateProjectsToOwner
    <2>6. /\ AsyncQueueTyped(asyncCommandQueues[candidate.node])
           /\ SequenceHasUniqueValues(
                asyncCommandQueues[candidate.node])
      BY <1>1, <2>2
         DEF Stage3KernelPending, AsyncStrongTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant,
             AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>7. PICK matching \in
                    1..Len(asyncCommandQueues[candidate.node]):
             asyncCommandQueues[candidate.node][matching] = candidate
      BY <2>5 DEF SequenceSet
    <2>8. NodeQueueNonempty(candidate.node)
      BY <2>7, SMT DEF NodeQueueNonempty
    <2>9. SchedulerServiceRank(candidate.node, candidate) = position
      BY <2>1, <2>3, <2>4, SMT DEF CandidateServiceRank
    <2> QED BY <2>1, <2>2, <2>4, <2>5, <2>6, <2>8, <2>9
  <1> QED BY <1>1

THEOREM UniqueRemovalDeletesSelectedValue ==
  \A sequence, selected:
    /\ sequence \in Seq(Range(sequence))
    /\ SequenceHasUniqueValues(sequence)
    /\ selected \in 1..Len(sequence)
    => sequence[selected]
         \notin SequenceSet(SequenceWithoutIndex(sequence, selected))
PROOF
  <1>1. ASSUME NEW sequence, NEW selected,
                sequence \in Seq(Range(sequence)),
                SequenceHasUniqueValues(sequence),
                selected \in 1..Len(sequence)
         PROVE sequence[selected]
                 \notin SequenceSet(
                          SequenceWithoutIndex(sequence, selected))
    <2>1. IsInjective(sequence)
      BY <1>1, UniqueSequenceLengthImpliesInjective
         DEF SequenceHasUniqueValues
    <2>2. /\ Len(SequenceWithoutIndex(sequence, selected))
                    = Len(sequence) - 1
           /\ \A resultIndex \in
                    1..Len(SequenceWithoutIndex(sequence, selected)):
                SequenceWithoutIndex(sequence, selected)[resultIndex] =
                  IF resultIndex < selected
                  THEN sequence[resultIndex]
                  ELSE sequence[resultIndex + 1]
      BY <1>1, SequenceWithoutIndexFacts
    <2>3. ASSUME sequence[selected]
                   \in SequenceSet(
                        SequenceWithoutIndex(sequence, selected))
           PROVE FALSE
      <3>1. PICK resultIndex \in
                    1..Len(SequenceWithoutIndex(sequence, selected)):
                  SequenceWithoutIndex(sequence, selected)[resultIndex]
                    = sequence[selected]
        BY <2>3 DEF SequenceSet
      <3>2. CASE resultIndex < selected
        <4>1. sequence[resultIndex] = sequence[selected]
          BY <2>2, <3>1, <3>2
        <4>2. /\ resultIndex \in 1..Len(sequence)
               /\ resultIndex # selected
          BY <1>1, <2>2, <3>1, <3>2, SMT
        <4> QED BY <1>1, <2>1, <4>1, <4>2
             DEF IsInjective
      <3>3. CASE ~(resultIndex < selected)
        <4>1. sequence[resultIndex + 1] = sequence[selected]
          BY <2>2, <3>1, <3>3
        <4>2. /\ resultIndex + 1 \in 1..Len(sequence)
               /\ resultIndex + 1 # selected
          BY <1>1, <2>2, <3>1, <3>3, SMT
        <4> QED BY <1>1, <2>1, <4>1, <4>2
             DEF IsInjective
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM FifoRuntimeQueueCursorFacts ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ SerializedRuntimeStep(node)
    /\ FifoRuntimeStep(node)
    => LET selected == NextNodeCommandIndex(node)
       IN /\ selected \in 1..Len(asyncCommandQueues[node])
          /\ asyncCommandQueues'[node] =
               SequenceWithoutIndex(asyncCommandQueues[node], selected)
          /\ asyncNextCommandClass'[node] =
               NextCommandClass(SelectedCommandClass(node))
BY NextNodeCommandIndexFacts, Isa
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, SerializedRuntimeStep,
       FifoRuntimeStep, RemoveNextNodeCommand

THEOREM SelectedTargetOccurrenceLeavesRuntimeQueue ==
  \A candidate, position:
    /\ Stage3KernelPending(candidate, position)
    /\ SerializedRuntimeStep(candidate.node)
    /\ FifoRuntimeStep(candidate.node)
    /\ NextNodeCommand(candidate.node) = candidate
    => candidate \notin
         SequenceSet(asyncCommandQueues'[candidate.node])
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                Stage3KernelPending(candidate, position),
                SerializedRuntimeStep(candidate.node),
                FifoRuntimeStep(candidate.node),
                NextNodeCommand(candidate.node) = candidate
         PROVE candidate \notin
                 SequenceSet(asyncCommandQueues'[candidate.node])
    <2>1. /\ candidate.node \in ValidatorIds
           /\ AsyncQueueTyped(asyncCommandQueues[candidate.node])
           /\ SequenceHasUniqueValues(
                asyncCommandQueues[candidate.node])
           /\ NodeQueueNonempty(candidate.node)
      BY <1>1, Stage3KernelCarrierFacts
    <2>2. LET selected == NextNodeCommandIndex(candidate.node)
           IN /\ selected \in
                    1..Len(asyncCommandQueues[candidate.node])
              /\ asyncCommandQueues[candidate.node][selected] = candidate
              /\ asyncCommandQueues'[candidate.node] =
                   SequenceWithoutIndex(
                     asyncCommandQueues[candidate.node], selected)
      BY <1>1, <2>1, FifoRuntimeQueueCursorFacts,
         NextNodeCommandIndexFacts
         DEF NextNodeCommand
    <2> QED BY <2>1, <2>2, UniqueRemovalDeletesSelectedValue
         DEF AsyncQueueTyped
  <1> QED BY <1>1

ClassPrefixThrough(sequence, target) ==
  {index \in 1..Len(sequence):
     /\ sequence[index].class = sequence[target].class
     /\ index <= target}

THEOREM UniqueSchedulerPrefixUsesTargetIndex ==
  \A node, candidate, target:
    /\ node \in ValidatorIds
    /\ AsyncQueueTyped(asyncCommandQueues[node])
    /\ SequenceHasUniqueValues(asyncCommandQueues[node])
    /\ target \in 1..Len(asyncCommandQueues[node])
    /\ asyncCommandQueues[node][target] = candidate
    => SchedulerClassPrefixIndices(node, candidate)
         = ClassPrefixThrough(asyncCommandQueues[node], target)
PROOF
  <1>1. ASSUME NEW node, NEW candidate, NEW target,
                node \in ValidatorIds,
                AsyncQueueTyped(asyncCommandQueues[node]),
                SequenceHasUniqueValues(asyncCommandQueues[node]),
                target \in 1..Len(asyncCommandQueues[node]),
                asyncCommandQueues[node][target] = candidate
         PROVE SchedulerClassPrefixIndices(node, candidate)
                 = ClassPrefixThrough(
                     asyncCommandQueues[node], target)
    <2>1. IsInjective(asyncCommandQueues[node])
      BY <1>1, UniqueSequenceLengthImpliesInjective
         DEF AsyncQueueTyped, SequenceHasUniqueValues
    <2>2. DOMAIN asyncCommandQueues[node] =
             1..Len(asyncCommandQueues[node])
      BY <1>1 DEF AsyncQueueTyped
    <2>3. SchedulerCandidateIndices(node, candidate) = {target}
      <3>1. target \in SchedulerCandidateIndices(node, candidate)
        BY <1>1 DEF SchedulerCandidateIndices
      <3>2. \A matching \in SchedulerCandidateIndices(node, candidate):
               matching = target
        <4>1. ASSUME NEW matching \in
                      SchedulerCandidateIndices(node, candidate)
               PROVE matching = target
          BY <1>1, <2>1, <2>2, <4>1
             DEF SchedulerCandidateIndices, IsInjective
        <4> QED BY <4>1
      <3> QED BY <3>1, <3>2, Isa
    <2>4. \A index:
             index \in SchedulerClassPrefixIndices(node, candidate)
               <=> index \in
                     ClassPrefixThrough(asyncCommandQueues[node], target)
      BY <1>1, <2>3, SMT
         DEF SchedulerClassPrefixIndices, ClassPrefixThrough,
             SchedulerCandidateIndices
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM Stage3CandidateSequenceIndexCharacterization ==
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

THEOREM Stage3CandidateSequenceIndexAfterNonTargetHead ==
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
      BY <1>1, Stage3CandidateSequenceIndexCharacterization DEF Old
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
         Stage3CandidateSequenceIndexCharacterization
    <2> QED BY <2>7, <2>10 DEF Old, New
  <1> QED BY <1>1

RemovalTargetIndex(removed, target) ==
  IF target < removed THEN target ELSE target - 1

RemovalIndexShift(removed, index) ==
  IF index < removed THEN index ELSE index - 1

THEOREM RemovalIndexShiftIsInjectiveAwayFromRemoved ==
  \A removed, left, right \in Nat:
    /\ left # removed
    /\ right # removed
    /\ RemovalIndexShift(removed, left) =
         RemovalIndexShift(removed, right)
    => left = right
BY SMT DEF RemovalIndexShift

THEOREM RemovalIndexShiftHasExplicitInverse ==
  \A removed, index \in Nat:
    RemovalIndexShift(
      removed,
      IF index < removed THEN index ELSE index + 1) = index
BY SMT DEF RemovalIndexShift

RemovalPrefixShift(sequence, removed, target) ==
  [oldIndex \in ClassPrefixThrough(sequence, target) \ {removed} |->
     RemovalIndexShift(removed, oldIndex)]

THEOREM RemovalPrefixShiftIsBijection ==
  \A sequence, removed, target:
    /\ sequence \in Seq(Range(sequence))
    /\ removed \in 1..Len(sequence)
    /\ target \in 1..Len(sequence)
    /\ removed # target
    => RemovalPrefixShift(sequence, removed, target)
         \in Bijection(
              ClassPrefixThrough(sequence, target) \ {removed},
              ClassPrefixThrough(
                SequenceWithoutIndex(sequence, removed),
                RemovalTargetIndex(removed, target)))
PROOF
  <1>1. ASSUME NEW sequence, NEW removed, NEW target,
                sequence \in Seq(Range(sequence)),
                removed \in 1..Len(sequence),
                target \in 1..Len(sequence),
                removed # target
         PROVE RemovalPrefixShift(sequence, removed, target)
                 \in Bijection(
                      ClassPrefixThrough(sequence, target) \ {removed},
                      ClassPrefixThrough(
                        SequenceWithoutIndex(sequence, removed),
                        RemovalTargetIndex(removed, target)))
    <2> DEFINE Old == ClassPrefixThrough(sequence, target) \ {removed}
    <2> DEFINE Result == SequenceWithoutIndex(sequence, removed)
    <2> DEFINE NewTarget == RemovalTargetIndex(removed, target)
    <2> DEFINE New == ClassPrefixThrough(Result, NewTarget)
    <2>1. /\ Result \in Seq(Range(sequence))
           /\ Len(Result) = Len(sequence) - 1
           /\ \A resultIndex \in 1..Len(Result):
                Result[resultIndex] =
                  IF resultIndex < removed
                  THEN sequence[resultIndex]
                  ELSE sequence[resultIndex + 1]
      BY <1>1, SequenceWithoutIndexFacts DEF Result
    <2>2. /\ NewTarget \in 1..Len(Result)
           /\ Result[NewTarget] = sequence[target]
      BY <1>1, <2>1, SMT
         DEF NewTarget, RemovalTargetIndex
    <2>3. /\ Old \subseteq 1..Len(sequence)
           /\ New \subseteq 1..Len(Result)
      BY <1>1 DEF Old, New, ClassPrefixThrough
    <2>4. \A oldIndex \in Old:
             IF oldIndex < removed
             THEN oldIndex \in New
             ELSE oldIndex - 1 \in New
      BY <1>1, <2>1, <2>2, <2>3, SMT
         DEF Old, New, ClassPrefixThrough,
             NewTarget, RemovalTargetIndex
    <2>5. \A newIndex \in New:
             IF newIndex < removed
             THEN newIndex \in Old
             ELSE newIndex + 1 \in Old
      BY <1>1, <2>1, <2>2, <2>3, SMT
         DEF Old, New, ClassPrefixThrough,
             NewTarget, RemovalTargetIndex
    <2>6. RemovalPrefixShift(sequence, removed, target) \in [Old -> New]
      BY <2>4, Isa DEF RemovalPrefixShift, Old
    <2>7. ASSUME NEW left \in Old, NEW right \in Old,
                  RemovalPrefixShift(sequence, removed, target)[left] =
                    RemovalPrefixShift(sequence, removed, target)[right]
           PROVE left = right
      <3>1. RemovalPrefixShift(sequence, removed, target)[left] =
               RemovalIndexShift(removed, left)
        BY <2>7, Isa DEF RemovalPrefixShift, Old
      <3>2. RemovalPrefixShift(sequence, removed, target)[right] =
               RemovalIndexShift(removed, right)
        BY <2>7, Isa DEF RemovalPrefixShift, Old
      <3>3. /\ left \in Nat
             /\ right \in Nat
             /\ left # removed
             /\ right # removed
        BY <2>3, <2>7, Isa DEF Old
      <3> QED BY <2>7, <3>1, <3>2, <3>3,
           RemovalIndexShiftIsInjectiveAwayFromRemoved
    <2>8. IsInjective(
             RemovalPrefixShift(sequence, removed, target))
      BY <2>6, <2>7 DEF IsInjective
    <2>9. RemovalPrefixShift(sequence, removed, target)
             \in Injection(Old, New)
      BY <2>6, <2>8 DEF Injection
    <2>10. ASSUME NEW newIndex \in New
            PROVE \E oldIndex \in Old:
                    RemovalPrefixShift(
                      sequence, removed, target)[oldIndex] = newIndex
      <3>1. CASE newIndex < removed
        <4>1. newIndex \in Old
          BY <2>5, <2>10, <3>1
        <4>2. RemovalPrefixShift(
                 sequence, removed, target)[newIndex] = newIndex
          BY <3>1, <4>1, Isa
             DEF RemovalPrefixShift, RemovalIndexShift, Old
        <4> QED BY <4>1, <4>2
      <3>2. CASE ~(newIndex < removed)
        <4>1. newIndex + 1 \in Old
          BY <2>5, <2>10, <3>2
        <4>2. /\ newIndex \in Nat
               /\ ~(newIndex + 1 < removed)
               /\ (newIndex + 1) - 1 = newIndex
          BY <2>3, <2>10, <3>2, SMT
        <4>3. RemovalPrefixShift(
                 sequence, removed, target)[newIndex + 1] = newIndex
          BY <4>1, <4>2, Isa
             DEF RemovalPrefixShift, RemovalIndexShift, Old
        <4> QED BY <4>1, <4>3
      <3> QED BY <3>1, <3>2
    <2>11. RemovalPrefixShift(sequence, removed, target)
              \in Surjection(Old, New)
      BY <2>6, <2>10 DEF Surjection
    <2> QED BY <2>9, <2>11 DEF Bijection, Old, New
  <1> QED BY <1>1

THEOREM PrefixCardinalityAfterNonTargetRemoval ==
  \A sequence, removed, target:
    /\ sequence \in Seq(Range(sequence))
    /\ removed \in 1..Len(sequence)
    /\ target \in 1..Len(sequence)
    /\ removed # target
    => Cardinality(
         ClassPrefixThrough(
           SequenceWithoutIndex(sequence, removed),
           RemovalTargetIndex(removed, target)))
       = IF removed \in ClassPrefixThrough(sequence, target)
         THEN Cardinality(ClassPrefixThrough(sequence, target)) - 1
         ELSE Cardinality(ClassPrefixThrough(sequence, target))
PROOF
  <1>1. ASSUME NEW sequence, NEW removed, NEW target,
                sequence \in Seq(Range(sequence)),
                removed \in 1..Len(sequence),
                target \in 1..Len(sequence),
                removed # target
         PROVE Cardinality(
                 ClassPrefixThrough(
                   SequenceWithoutIndex(sequence, removed),
                   RemovalTargetIndex(removed, target)))
               = IF removed \in ClassPrefixThrough(sequence, target)
                 THEN Cardinality(
                        ClassPrefixThrough(sequence, target)) - 1
                 ELSE Cardinality(ClassPrefixThrough(sequence, target))
    <2> DEFINE All == ClassPrefixThrough(sequence, target)
    <2> DEFINE Remaining == All \ {removed}
    <2> DEFINE New ==
           ClassPrefixThrough(
             SequenceWithoutIndex(sequence, removed),
             RemovalTargetIndex(removed, target))
    <2>1. IsFiniteSet(All)
      BY <1>1, FS_Interval, FS_Subset DEF All, ClassPrefixThrough
    <2>2. /\ IsFiniteSet(Remaining)
           /\ Cardinality(Remaining) =
                IF removed \in All
                THEN Cardinality(All) - 1
                ELSE Cardinality(All)
      BY <2>1, FS_RemoveElement DEF Remaining
    <2>3. ExistsBijection(Remaining, New)
      BY <1>1, RemovalPrefixShiftIsBijection
         DEF ExistsBijection, Remaining, New, All
    <2>4. Cardinality(New) = Cardinality(Remaining)
      BY <2>2, <2>3, FS_Bijection
    <2> QED BY <2>2, <2>4 DEF All, Remaining, New
  <1> QED BY <1>1

THEOREM NonTargetRemovalPreservesShiftedTarget ==
  \A sequence, removed, target:
    /\ sequence \in Seq(Range(sequence))
    /\ removed \in 1..Len(sequence)
    /\ target \in 1..Len(sequence)
    /\ removed # target
    => LET result == SequenceWithoutIndex(sequence, removed)
           shifted == RemovalTargetIndex(removed, target)
       IN /\ shifted \in 1..Len(result)
          /\ result[shifted] = sequence[target]
PROOF
  <1>1. ASSUME NEW sequence, NEW removed, NEW target,
                sequence \in Seq(Range(sequence)),
                removed \in 1..Len(sequence),
                target \in 1..Len(sequence),
                removed # target
         PROVE LET result == SequenceWithoutIndex(sequence, removed)
                   shifted == RemovalTargetIndex(removed, target)
               IN /\ shifted \in 1..Len(result)
                  /\ result[shifted] = sequence[target]
    <2> DEFINE Result == SequenceWithoutIndex(sequence, removed)
    <2> DEFINE Shifted == RemovalTargetIndex(removed, target)
    <2>1. /\ Len(Result) = Len(sequence) - 1
           /\ \A resultIndex \in 1..Len(Result):
                Result[resultIndex] =
                  IF resultIndex < removed
                  THEN sequence[resultIndex]
                  ELSE sequence[resultIndex + 1]
      BY <1>1, SequenceWithoutIndexFacts DEF Result
    <2> QED BY <1>1, <2>1, SMT
         DEF Result, Shifted, RemovalTargetIndex
  <1> QED BY <1>1

THEOREM SelectedSameClassPrecedesTarget ==
  \A node \in ValidatorIds:
    \A target \in 1..Len(asyncCommandQueues[node]):
      /\ AsyncRuntimeScalarTypeInvariant
      /\ NodeQueueNonempty(node)
      /\ asyncCommandQueues[node][target].class
           = SelectedCommandClass(node)
      => NextNodeCommandIndex(node) <= target
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW target \in 1..Len(asyncCommandQueues[node]),
                AsyncRuntimeScalarTypeInvariant,
                NodeQueueNonempty(node),
                asyncCommandQueues[node][target].class
                  = SelectedCommandClass(node)
         PROVE NextNodeCommandIndex(node) <= target
    <2>1. target \in
             CommandClassIndices(node, SelectedCommandClass(node))
      BY <1>1 DEF CommandClassIndices
    <2> QED BY <1>1, <2>1, NextNodeCommandIndexFacts
  <1> QED BY <1>1

THEOREM SelectedDifferentClassStrictlyAdvancesCursor ==
  \A node \in ValidatorIds, targetClass \in AsyncCommandClasses:
    /\ asyncNextCommandClass[node] \in AsyncCommandClasses
    /\ CommandClassIndices(node, targetClass) # {}
    /\ SelectedCommandClass(node) # targetClass
    => CommandClassDistance(
         NextCommandClass(SelectedCommandClass(node)), targetClass)
         < CommandClassDistance(
             asyncNextCommandClass[node], targetClass)
BY SMTT(30)
   DEF SelectedCommandClass, CommandClassDistance,
       NextCommandClass, AsyncCommandClasses

(***************************************************************************
Strict Stage-3 FIFO closure.

The queue arithmetic above is the local kernel.  The temporal closure needs
one additional, independent rank: `ReadyRunAuxRank` measures the finite Local,
Ingress, deferred-drain, tag, timeout, retransmit, and Runtime prefix before a
runtime FIFO removal.  A FIFO removal itself either removes the target, removes
an earlier same-class prefix occurrence, or advances the cursor across a
different class.  No other action may add an occurrence ahead of the target.

The two action-induction leaves below are deliberately explicit.  They are the
remaining strict TLAPS frontier, not fairness assumptions: the first checks the
exact same-node `PostGstRunNode` arm, and the second checks every other
`[AsyncNext]_AsyncAllVars` arm for preservation or strict auxiliary descent.
***************************************************************************)

Stage3RankProgressExit(candidate, position) ==
  \/ ~ResponsiveProtectedCandidateOwned(candidate)
  \/ ServiceRankLess(
       CandidateServiceRank(candidate), <<3, position>>)

THEOREM Stage3FifoRuntimeStrictlyProgressesObligation ==
  \A candidate, position:
    /\ Stage3KernelPending(candidate, position)
    /\ SerializedRuntimeStep(candidate.node)
    /\ FifoRuntimeStep(candidate.node)
    => Stage3RankProgressExit(candidate, position)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   SelectedTargetOccurrenceLeavesRuntimeQueue,
   UniqueSchedulerPrefixUsesTargetIndex,
   Stage3CandidateSequenceIndexCharacterization,
   PrefixCardinalityAfterNonTargetRemoval,
   NonTargetRemovalPreservesShiftedTarget,
   SelectedSameClassPrecedesTarget,
   SelectedDifferentClassStrictlyAdvancesCursor,
   FifoRuntimeQueueCursorFacts, NextNodeCommandIndexFacts,
   FS_RemoveElement, Isa
   DEF Stage3RankProgressExit, Stage3KernelPending,
       ProtectedOwnedAtServiceRank, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess, SchedulerServiceRank,
       SchedulerClassPrefixIndices, SchedulerCandidateIndices,
       ClassPrefixThrough, RemovalTargetIndex, FifoRuntimeStep,
       RemoveNextNodeCommand, NextNodeCommand, SequenceSet,
       CandidateScheduled, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant, AsyncAllVars

Stage3AuxBlocked(candidate, position, rank) ==
  /\ Stage3KernelPending(candidate, position)
  /\ ~Stage3RankProgressExit(candidate, position)
  /\ ReadyRunAuxRank(candidate.node) = rank

Stage3AuxProgress(candidate, position, rank) ==
  \/ Stage3RankProgressExit(candidate, position)
  \/ \E lower \in SetLessThan(
       rank, ReadyRunAuxOrdering, ReadyRunAuxCarrier):
       Stage3AuxBlocked(candidate, position, lower)

THEOREM Stage3AuxRankInCarrierObligation ==
  \A candidate, position:
    Stage3KernelPending(candidate, position)
      => ReadyRunAuxRank(candidate.node) \in ReadyRunAuxCarrier
BY Stage3KernelCarrierFacts, ReadyRunAuxRankInCarrier,
   AsyncStrongTypeProjectsAsyncType
   DEF Stage3KernelPending

THEOREM Stage3SameNodeRunAuxDescentObligation ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      /\ Stage3AuxBlocked(candidate, position, rank)
      /\ PostGstRunNode(candidate.node)
      => Stage3AuxProgress(candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                NEW rank \in ReadyRunAuxCarrier,
                Stage3AuxBlocked(candidate, position, rank),
                PostGstRunNode(candidate.node)
         PROVE Stage3AuxProgress(candidate, position, rank)'
    <2>1. RunNode(candidate.node)
      BY <1>1 DEF PostGstRunNode
    <2>2. CASE LocalAdmissionStep(candidate.node)
      BY <1>1, <2>2,
         AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesProgressOwnership,
         LocalAdmissionStrictlyDecreasesRuntimeReach,
         ReadyRunAuxRankInCarrier, Isa
         DEF Stage3AuxProgress, Stage3AuxBlocked,
             Stage3KernelPending, Stage3RankProgressExit,
             ProtectedOwnedAtServiceRank,
             ProtectedServiceOwnershipExit,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned, CandidateServiceRank,
             ServiceRankLess, ReadyRunAuxRank,
             ReadyRunDeferredRank, ReadyRunTimeoutRank,
             ReadyRunInnerRank, ReadyRunAuxOrdering,
             ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
             ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
             ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
             ReadyRunInnerCarrier, ReadyFifoDebt,
             ReadyDeferredCount, ReadyTimeoutDebt,
             ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
             LocalAdmissionStep, LocalAdmissionCanAdvance,
             CandidateScheduled, SequenceSet,
             AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant,
             AsyncOutstandingCarrierInvariant, AsyncAllVars
    <2>3. CASE IngressDrainStep(candidate.node)
      BY <1>1, <2>3,
         AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesProgressOwnership,
         IngressDrainStrictlyDecreasesRuntimeReach,
         ReadyRunAuxRankInCarrier, Isa
         DEF Stage3AuxProgress, Stage3AuxBlocked,
             Stage3KernelPending, Stage3RankProgressExit,
             ProtectedOwnedAtServiceRank,
             ProtectedServiceOwnershipExit,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned, CandidateServiceRank,
             ServiceRankLess, ReadyRunAuxRank,
             ReadyRunDeferredRank, ReadyRunTimeoutRank,
             ReadyRunInnerRank, ReadyRunAuxOrdering,
             ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
             ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
             ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
             ReadyRunInnerCarrier, ReadyFifoDebt,
             ReadyDeferredCount, ReadyTimeoutDebt,
             ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
             IngressDrainStep, DrainFairIngressSelected,
             CandidateScheduled, SequenceSet,
             AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant,
             AsyncOutstandingCarrierInvariant, AsyncAllVars
    <2>4. CASE SerializedRuntimeStep(candidate.node)
      <3>1. RuntimeStep(candidate.node)
        BY <2>4 DEF SerializedRuntimeStep
      <3>2. CASE DeferredDrainStep(candidate.node)
        BY <1>1, <2>4, <3>2,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership,
           ReadyRunAuxRankInCarrier, Isa
           DEF Stage3AuxProgress, Stage3AuxBlocked,
               Stage3KernelPending, Stage3RankProgressExit,
               ProtectedOwnedAtServiceRank,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, ReadyRunAuxRank,
               ReadyRunDeferredRank, ReadyRunTimeoutRank,
               ReadyRunInnerRank, ReadyRunAuxOrdering,
               ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
               ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
               ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
               ReadyRunInnerCarrier, ReadyFifoDebt,
               ReadyDeferredCount, ReadyTimeoutDebt,
               ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
               SerializedRuntimeStep, DeferredDrainStep,
               RemoveNextDeferredCommand, DiscardCommand,
               AdvanceNextDeferredClass, DeferredQueueNonempty,
               CandidateScheduled, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>3. CASE DeferredTagStep(candidate.node)
        BY <1>1, <2>4, <3>3,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership,
           ReadyRunAuxRankInCarrier, Isa
           DEF Stage3AuxProgress, Stage3AuxBlocked,
               Stage3KernelPending, Stage3RankProgressExit,
               ProtectedOwnedAtServiceRank,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, ReadyRunAuxRank,
               ReadyRunDeferredRank, ReadyRunTimeoutRank,
               ReadyRunInnerRank, ReadyRunAuxOrdering,
               ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
               ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
               ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
               ReadyRunInnerCarrier, ReadyFifoDebt,
               ReadyDeferredCount, ReadyTimeoutDebt,
               ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
               SerializedRuntimeStep, DeferredTagStep,
               DeferredTimeoutStep, DeferredRetransmitStep,
               CandidateScheduled, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>4. CASE DirectTimeoutStep(candidate.node)
        BY <1>1, <2>4, <3>4,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership,
           ReadyRunAuxRankInCarrier, Isa
           DEF Stage3AuxProgress, Stage3AuxBlocked,
               Stage3KernelPending, Stage3RankProgressExit,
               ProtectedOwnedAtServiceRank,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, ReadyRunAuxRank,
               ReadyRunDeferredRank, ReadyRunTimeoutRank,
               ReadyRunInnerRank, ReadyRunAuxOrdering,
               ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
               ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
               ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
               ReadyRunInnerCarrier, ReadyFifoDebt,
               ReadyDeferredCount, ReadyTimeoutDebt,
               ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
               SerializedRuntimeStep, DirectTimeoutStep, TimeoutDue,
               CandidateScheduled, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>5. CASE FifoRuntimeStep(candidate.node)
        BY <1>1, <2>4, <3>5,
           Stage3FifoRuntimeStrictlyProgressesObligation
           DEF Stage3AuxProgress, Stage3AuxBlocked
      <3>6. CASE /\ DirectRetransmitStep(candidate.node)
                   /\ ~(NodeQueueNonempty(candidate.node)
                         /\ asyncFifoOwed[candidate.node])
        BY <1>1, <2>4, <3>6,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership,
           ReadyRunAuxRankInCarrier, Isa
           DEF Stage3AuxProgress, Stage3AuxBlocked,
               Stage3KernelPending, Stage3RankProgressExit,
               ProtectedOwnedAtServiceRank,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, ReadyRunAuxRank,
               ReadyRunDeferredRank, ReadyRunTimeoutRank,
               ReadyRunInnerRank, ReadyRunAuxOrdering,
               ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
               ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
               ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
               ReadyRunInnerCarrier, ReadyFifoDebt,
               ReadyDeferredCount, ReadyTimeoutDebt,
               ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
               SerializedRuntimeStep, DirectRetransmitStep,
               LocalAdmissionCanAdvance, ProducerCompletionCanAdmit,
               CanEnqueueClass, AsyncQueueDepth, NodeQueueNonempty,
               CandidateScheduled, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>7. CASE /\ IdleRuntimeStep(candidate.node)
                   /\ ~NodeQueueNonempty(candidate.node)
        BY <1>1, <2>4, <3>7,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership, Isa
           DEF Stage3AuxProgress, Stage3AuxBlocked,
               Stage3KernelPending, Stage3RankProgressExit,
               ProtectedOwnedAtServiceRank,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, CandidateScheduled, SequenceSet,
               NodeQueueNonempty, IdleRuntimeStep,
               SerializedRuntimeStep,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5, <3>6, <3>7
           DEF RuntimeStep
    <2> QED BY <2>1, <2>2, <2>3, <2>4 DEF RunNode
  <1> QED BY <1>1

THEOREM Stage3OtherStepUnlessAuxDescentObligation ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      /\ Stage3AuxBlocked(candidate, position, rank)
      /\ [AsyncNext]_AsyncAllVars
      /\ ~PostGstRunNode(candidate.node)
      => \/ Stage3AuxBlocked(candidate, position, rank)'
         \/ Stage3AuxProgress(candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                NEW rank \in ReadyRunAuxCarrier,
                Stage3AuxBlocked(candidate, position, rank),
                [AsyncNext]_AsyncAllVars,
                ~PostGstRunNode(candidate.node)
         PROVE \/ Stage3AuxBlocked(candidate, position, rank)'
               \/ Stage3AuxProgress(candidate, position, rank)'
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>1, Isa
         DEF Stage3AuxBlocked, Stage3KernelPending,
             Stage3RankProgressExit, Stage3AuxProgress,
             ProtectedOwnedAtServiceRank, ReadyRunAuxRank,
             AsyncAllVars, AsyncSchedulerVars, vars
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
           AsyncBracketNextPreservesProgressOwnership, Isa
           DEF Stage3AuxBlocked, Stage3AuxProgress,
               Stage3KernelPending, Stage3RankProgressExit,
               ProtectedOwnedAtServiceRank,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, ReadyRunAuxRank,
               ReadyRunDeferredRank, ReadyRunTimeoutRank,
               ReadyRunInnerRank, RunNode,
               RunHistoricalRecoveryNode, RunNodeWork,
               RunHistoricalServer, CandidateScheduled, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>2. CASE AsyncTick
        BY <1>1, <2>2, <3>2,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership, Isa
           DEF Stage3AuxBlocked, Stage3AuxProgress,
               Stage3KernelPending, Stage3RankProgressExit,
               ProtectedOwnedAtServiceRank,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, ReadyRunAuxRank,
               ReadyRunDeferredRank, ReadyRunTimeoutRank,
               ReadyRunInnerRank, ReadyRunAuxOrdering,
               ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
               ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
               ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
               ReadyRunInnerCarrier, ReadyFifoDebt,
               ReadyDeferredCount, ReadyTimeoutDebt,
               ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
               AsyncTick, AsyncNonClockVars,
               CandidateScheduled, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>3. CASE \E node \in ValidatorIds:
                    OpenHistoricalRecovery(node)
        BY <1>1, <2>2, <3>3,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership, Isa
           DEF Stage3AuxBlocked, Stage3AuxProgress,
               Stage3KernelPending, Stage3RankProgressExit,
               ProtectedOwnedAtServiceRank,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, ReadyRunAuxRank,
               OpenHistoricalRecovery, CandidateScheduled,
               SequenceSet, AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>4. CASE \/ \E node \in AsyncCurrentResponsiveVoters:
                          DirectCommitCertificateDiscoveryStep(node)
                   \/ \E historicalNode
                          \in asyncHistoricalRecoveryTargets:
                          DirectHistoricalCommitCertificateDiscoveryStep(
                            historicalNode)
        BY <1>1, <2>2, <3>4,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership, Isa
           DEF Stage3AuxBlocked, Stage3AuxProgress,
               Stage3KernelPending, Stage3RankProgressExit,
               ProtectedOwnedAtServiceRank,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, ReadyRunAuxRank,
               ReadyRunDeferredRank, ReadyRunTimeoutRank,
               ReadyRunInnerRank,
               DirectCommitCertificateDiscoveryStep,
               DirectHistoricalCommitCertificateDiscoveryStep,
               CommitCertificateDiscoveryStepWork,
               CandidateScheduled, SequenceSet,
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
           HeadTailProperties, SequenceSetAfterAppend, Isa
           DEF Stage3AuxBlocked, Stage3AuxProgress,
               Stage3KernelPending, Stage3RankProgressExit,
               ProtectedOwnedAtServiceRank,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, ReadyRunAuxRank,
               ReadyRunDeferredRank, ReadyRunTimeoutRank,
               ReadyRunInnerRank, ServiceIoWorker,
               ServiceHistoricalRecoveryIoWorker,
               ServiceIoWorkerWork, EnqueueIoLocalControl,
               EnqueueHistoricalRecoveryIoLocalControl,
               EnqueueIoLocalControlWork, CandidateScheduled,
               SequenceSet, AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>6. CASE AsyncNetworkStep \/ AsyncFaultStep
        BY <1>1, <2>2, <3>6,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership, Isa
           DEF Stage3AuxBlocked, Stage3AuxProgress,
               Stage3KernelPending, Stage3RankProgressExit,
               ProtectedOwnedAtServiceRank,
               ProtectedServiceOwnershipExit,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, ReadyRunAuxRank,
               ReadyRunDeferredRank, ReadyRunTimeoutRank,
               ReadyRunInnerRank, AsyncNetworkStep,
               AdmitIngressPacket, AsyncFaultStep, PreGstCrash,
               CandidateScheduled, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>7. CASE AsyncSetGST
        BY <1>1, <3>7
           DEF Stage3AuxBlocked, Stage3KernelPending,
               ProtectedOwnedAtServiceRank, AsyncSetGST
      <3>8. CASE \E node \in ValidatorIds: PreGstCrash(node)
        BY <1>1, <3>8
           DEF Stage3AuxBlocked, Stage3KernelPending,
               ProtectedOwnedAtServiceRank, PreGstCrash
      <3>9. CASE RunNode(candidate.node)
        <4>1. PostGstRunNode(candidate.node)
          BY <1>1, <2>2, <3>9, Isa
             DEF Stage3AuxBlocked, Stage3KernelPending,
                 ProtectedOwnedAtServiceRank, PostGstRunNode,
                 AsyncNext, AsyncNonCrashStep, AsyncRunnerStep
        <4> QED BY <1>1, <4>1
      <3>10. CASE RunHistoricalRecoveryNode(candidate.node)
        <4>1. RunNode(candidate.node)
          BY <1>1, <3>10
             DEF Stage3AuxBlocked, Stage3KernelPending,
                 ProtectedOwnedAtServiceRank,
                 ResponsiveProtectedCandidateOwned, RunNode,
                 RunHistoricalRecoveryNode
        <4>2. PostGstRunNode(candidate.node)
          BY <1>1, <2>2, <4>1, Isa
             DEF Stage3AuxBlocked, Stage3KernelPending,
                 ProtectedOwnedAtServiceRank, PostGstRunNode,
                 AsyncNext, AsyncNonCrashStep, AsyncRunnerStep
        <4> QED BY <1>1, <4>2
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
           <3>7, <3>8, <3>9, <3>10
           DEF AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
               AsyncNonRunnerStep
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM FairStage3AuxOneStepObligation ==
  \A initialContext, candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      AsyncSpecAt(initialContext)
        => (Stage3AuxBlocked(candidate, position, rank)
              ~> Stage3AuxProgress(candidate, position, rank))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW rank \in ReadyRunAuxCarrier
         PROVE AsyncSpecAt(initialContext)
                 => (Stage3AuxBlocked(candidate, position, rank)
                       ~> Stage3AuxProgress(candidate, position, rank))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>2. /\ Stage3AuxBlocked(candidate, position, rank)
             /\ ~Stage3AuxProgress(candidate, position, rank)
            => ENABLED
                 <<PostGstRunNode(candidate.node)>>_AsyncAllVars
      BY ProtectedOwnedCandidateEnablesFairRunNode
         DEF Stage3AuxBlocked, Stage3KernelPending,
             ProtectedOwnedAtServiceRank, Stage3AuxProgress
    <2>3. /\ Stage3AuxBlocked(candidate, position, rank)
             /\ ~Stage3AuxProgress(candidate, position, rank)
             /\ <<PostGstRunNode(candidate.node)>>_AsyncAllVars
            => Stage3AuxProgress(candidate, position, rank)'
      BY Stage3SameNodeRunAuxDescentObligation, Isa
         DEF PostGstRunNode
    <2>4. Stage3AuxBlocked(candidate, position, rank)
              /\ [AsyncNext]_AsyncAllVars
            => Stage3AuxBlocked(candidate, position, rank)'
                 \/ Stage3AuxProgress(candidate, position, rank)'
      BY Stage3SameNodeRunAuxDescentObligation,
         Stage3OtherStepUnlessAuxDescentObligation, Isa
    <2>5. CASE candidate.node \in AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(PostGstRunNode(candidate.node))
        BY <2>5 DEF AsyncSpecAt, AsyncFairnessAt
      <3> QED BY <2>2, <2>3, <2>4, <3>1, PTL
           DEF AsyncSpecAt
    <2>6. CASE candidate.node \notin AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => []~Stage3AuxBlocked(candidate, position, rank)
        BY <2>1, <2>6, PTL
           DEF Stage3AuxBlocked, Stage3KernelPending,
               ProtectedOwnedAtServiceRank,
               ResponsiveProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM FairStage3AuxRankDescentObligation ==
  \A initialContext, candidate, position:
    AsyncSpecAt(initialContext)
      => \A rank \in ReadyRunAuxCarrier:
           Stage3AuxBlocked(candidate, position, rank)
             ~> Stage3RankProgressExit(candidate, position)
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position
         PROVE AsyncSpecAt(initialContext)
                 => \A rank \in ReadyRunAuxCarrier:
                      Stage3AuxBlocked(candidate, position, rank)
                        ~> Stage3RankProgressExit(candidate, position)
    <2>1. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  Stage3AuxBlocked(candidate, position, rank)
                    ~> (Stage3RankProgressExit(candidate, position)
                         \/ \E lower \in SetLessThan(
                              rank, ReadyRunAuxOrdering,
                              ReadyRunAuxCarrier):
                              Stage3AuxBlocked(
                                candidate, position, lower))
      BY FairStage3AuxOneStepObligation
         DEF Stage3AuxProgress
    <2> QED BY <2>1, ReadyRunAuxOrderingIsWellFounded,
         WellFoundedLeadsTo
  <1> QED BY <1>1

THEOREM ProtectedStage3RankProgressFromFairSchedulerObligation ==
  \A initialContext:
    ProtectedStage3RankProgressProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ProtectedStage3RankProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME NEW candidate \in AsyncCandidateSet,
                  NEW position \in Nat
           PROVE AsyncSpecAt(initialContext)
                   => ((gst
                         /\ ResponsiveProtectedCandidateOwned(candidate)
                         /\ CandidateServiceRank(candidate)
                              = <<3, position>>)
                        ~> Stage3RankProgressExit(candidate, position))
      <3>1. AsyncSpecAt(initialContext)
               => [](AsyncStrongTypeInvariant
                      /\ AsyncProgressOwnershipInvariant)
        BY AsyncSpecAlwaysStrongTypeInvariant,
           AsyncSpecAlwaysProgressOwnershipInvariant, PTL
      <3>2. AsyncSpecAt(initialContext)
               => ((gst
                     /\ ResponsiveProtectedCandidateOwned(candidate)
                     /\ CandidateServiceRank(candidate) = <<3, position>>)
                    ~> Stage3AuxBlocked(
                         candidate, position,
                         ReadyRunAuxRank(candidate.node)))
        BY <3>1, Stage3AuxRankInCarrierObligation, PTL
           DEF Stage3KernelPending, Stage3AuxBlocked,
               ProtectedOwnedAtServiceRank, Stage3RankProgressExit
      <3>3. AsyncSpecAt(initialContext)
               => \A rank \in ReadyRunAuxCarrier:
                    Stage3AuxBlocked(candidate, position, rank)
                      ~> Stage3RankProgressExit(candidate, position)
        BY FairStage3AuxRankDescentObligation
      <3> QED BY <3>2, <3>3, PTL
    <2> QED BY <2>1 DEF ProtectedStage3RankProgressProperty
  <1> QED BY <1>1

THEOREM FairProtectedStage3RankProgress ==
  \A initialContext:
    ProtectedStage3RankProgressProperty(AsyncSpecAt(initialContext))
BY ProtectedStage3RankProgressFromFairSchedulerObligation

=============================================================================
