---- MODULE SumeragiV2AsyncProtectedSlotProofs ----
EXTENDS SumeragiV2AsyncProgressOwnershipProofs

(***************************************************************************
Protected deferred progress has one semantic slot per validator's exact
locked Commit delivery, one per validator's exact current locked-reproposal
Prepare delivery, and one per validator's TimeoutVote delivery, plus PrepareQC,
CommitQC, and TC delivery.  Mapping indices to that finite carrier makes the
3 * N + 3 reservation bound a theorem of the duplicate exclusion rule rather
than an independent queue-size guess.
***************************************************************************)

ProtectedProgressSlotUniverse ==
  ({"CommitVote", "PrepareVote", "TimeoutVote"} \X ValidatorIds)
    \cup ({"PrepareQC", "CommitQC", "TimeoutCertificate"} \X {0})

ProtectedProgressSlot(command) ==
  CASE command.kind = "DeliverVote" ->
         <<command.item.kind, command.item.envelope.vote.signer>>
    [] command.kind = "DeliverTimeout" ->
         <<"TimeoutVote", command.item.envelope.vote.signer>>
    [] command.kind = "DeliverQC" -> <<command.item.kind, 0>>
    [] OTHER -> <<"TimeoutCertificate", 0>>

ProtectedProgressSlotMap(node) ==
  [index \in ProtectedDeferredProgressIndices(node) |->
     ProtectedProgressSlot(
       asyncDeferredProgressQueues[node][index])]

THEOREM ProtectedProgressSlotUniverseSize ==
  ModelConfiguration
    => /\ IsFiniteSet(ProtectedProgressSlotUniverse)
       /\ Cardinality(ProtectedProgressSlotUniverse) = 3 * N + 3
PROOF
  <1>1. ASSUME ModelConfiguration
         PROVE /\ IsFiniteSet(ProtectedProgressSlotUniverse)
               /\ Cardinality(ProtectedProgressSlotUniverse) = 3 * N + 3
    <2> DEFINE VoteKinds == {"CommitVote", "PrepareVote", "TimeoutVote"}
    <2> DEFINE VoteSlots == VoteKinds \X ValidatorIds
    <2> DEFINE CertificateKinds ==
           {"PrepareQC", "CommitQC", "TimeoutCertificate"}
    <2> DEFINE CertificateSlots == CertificateKinds \X {0}
    <2>1. /\ N \in Nat \ {0}
           /\ IsFiniteSet(ValidatorIds)
           /\ Cardinality(ValidatorIds) = N
      BY <1>1, FS_Interval, SMT
         DEF ValidatorIds, ModelConfiguration, QuorumConfiguration
    <2>2. /\ IsFiniteSet(VoteKinds)
           /\ Cardinality(VoteKinds) = 3
      BY FS_EmptySet, FS_AddElement, SMT DEF VoteKinds
    <2>3. /\ IsFiniteSet(VoteSlots)
           /\ Cardinality(VoteSlots) = 3 * N
      BY <2>1, <2>2, FS_Product, SMT DEF VoteSlots
    <2>4. /\ IsFiniteSet(CertificateKinds)
           /\ Cardinality(CertificateKinds) = 3
      BY FS_EmptySet, FS_AddElement, SMT DEF CertificateKinds
    <2>5. /\ IsFiniteSet({0})
           /\ Cardinality({0}) = 1
      BY FS_Singleton
    <2>6. /\ IsFiniteSet(CertificateSlots)
           /\ Cardinality(CertificateSlots) = 3
      BY <2>4, <2>5, FS_Product, SMT DEF CertificateSlots
    <2>7. VoteSlots \cap CertificateSlots = {}
      BY Isa DEF VoteSlots, CertificateSlots, CertificateKinds
    <2>8. /\ IsFiniteSet(VoteSlots \cup CertificateSlots)
           /\ Cardinality(VoteSlots \cup CertificateSlots) = 3 * N + 3
      BY <2>3, <2>6, <2>7, FS_Union, FS_EmptySet, SMT
    <2> QED BY <2>8
         DEF ProtectedProgressSlotUniverse, VoteKinds, VoteSlots,
             CertificateSlots, CertificateKinds
  <1> QED BY <1>1

THEOREM ProtectedProgressSlotCharacterization ==
  \A left, right:
    /\ AsyncCandidateTyped(left)
    /\ AsyncCandidateTyped(right)
    /\ ProtectedProgressCommand(left)
    /\ ProtectedProgressCommand(right)
    => /\ ProtectedProgressSlot(left)
              \in ProtectedProgressSlotUniverse
       /\ ProtectedProgressSlot(right)
              \in ProtectedProgressSlotUniverse
       /\ (ProtectedProgressSlot(left)
              = ProtectedProgressSlot(right)
             <=> SameProtectedProgressSlot(left, right))
BY SMTT(30)
   DEF ProtectedProgressSlot, ProtectedProgressSlotUniverse,
       ProtectedProgressCommand, SameProtectedProgressSlot,
       HistoricalLockedCommitItem, AsyncCandidateTyped,
       AsyncItemTyped, VoteEnvelopeSet, VoteEnvelope, VoteRecordSet,
       Vote, TimeoutEnvelopeSet, TimeoutVoteRecordSet, ValidatorIds

THEOREM ProtectedDeferredProgressCountFollowsUniqueness ==
  \A node \in ValidatorIds:
    /\ ModelConfiguration
    /\ AsyncDeferredContentTypeInvariant
    /\ \A left, right \in ProtectedDeferredProgressIndices(node):
         SameProtectedProgressSlot(
           asyncDeferredProgressQueues[node][left],
           asyncDeferredProgressQueues[node][right])
           => left = right
    => Cardinality(ProtectedDeferredProgressIndices(node)) <= 3 * N + 3
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                ModelConfiguration,
                AsyncDeferredContentTypeInvariant,
                \A left, right
                    \in ProtectedDeferredProgressIndices(node):
                  SameProtectedProgressSlot(
                    asyncDeferredProgressQueues[node][left],
                    asyncDeferredProgressQueues[node][right])
                    => left = right
         PROVE Cardinality(ProtectedDeferredProgressIndices(node))
                 <= 3 * N + 3
    <2> DEFINE Indices == ProtectedDeferredProgressIndices(node)
    <2> DEFINE Slots == ProtectedProgressSlotUniverse
    <2> DEFINE SlotMap == ProtectedProgressSlotMap(node)
    <2>1. /\ IsFiniteSet(Slots)
           /\ Cardinality(Slots) = 3 * N + 3
      BY <1>1, ProtectedProgressSlotUniverseSize DEF Slots
    <2>2. \A index \in Indices:
             /\ index \in 1..Len(asyncDeferredProgressQueues[node])
             /\ AsyncCandidateTyped(
                  asyncDeferredProgressQueues[node][index])
             /\ ProtectedProgressCommand(
                  asyncDeferredProgressQueues[node][index])
      BY <1>1, Isa
         DEF Indices, ProtectedDeferredProgressIndices,
             AsyncDeferredContentTypeInvariant, AsyncQueueTyped
    <2>3. SlotMap \in [Indices -> Slots]
      BY <2>2, ProtectedProgressSlotCharacterization, Isa
         DEF SlotMap, ProtectedProgressSlotMap, Slots
    <2>4. \A left, right \in Indices:
             SlotMap[left] = SlotMap[right] => left = right
      BY <1>1, <2>2, ProtectedProgressSlotCharacterization, Isa
         DEF SlotMap, ProtectedProgressSlotMap
    <2>5. SlotMap \in Injection(Indices, Slots)
      BY <2>3, <2>4 DEF Injection, IsInjective
    <2>6. /\ IsFiniteSet(Indices)
           /\ Cardinality(Indices) <= Cardinality(Slots)
      BY <2>1, <2>5, FS_Injection
    <2> QED BY <2>1, <2>6
  <1> QED BY <1>1

(***************************************************************************
The asynchronous type theorem closes by induction over the exact bracketed
next relation, using the strengthened Core/scheduler/timeout-pool/recovery
type-authority-execution invariant.
The temporal release obligations below remain explicit until their concrete
weak-fairness frontier and well-founded service-rank chains are discharged.
***************************************************************************)

THEOREM AsyncTypeInvariantObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []AsyncTypeInvariant
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncStrongTypeProjectsAsyncType, PTL

THEOREM GenerationScopedVoteDeliveryObligation ==
  \A initialContext:
    GenerationScopedVoteDeliveryProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE GenerationScopedVoteDeliveryProperty(
                   AsyncSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
             => [][AsyncNext]_AsyncAllVars
      BY DEF AsyncSpecAt
    <2>2. [AsyncNext]_AsyncAllVars
             => [VoteDeliveryEpochAction]_AsyncAllVars
      BY AsyncStepHasVoteDeliveryEpochSemantics
    <2> QED BY <2>1, <2>2, PTL
         DEF GenerationScopedVoteDeliveryProperty
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesProgressWitness ==
  \A initialContext:
    AsyncInitAt(initialContext) => ProgressWitnessInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE ProgressWitnessInvariant
    <2>1. /\ ModelConfiguration
           /\ FrozenContextAdmissible(initialContext)
           /\ context = initialContext
           /\ (initialContext.height = 0
                 => /\ commitIntents = {}
                    /\ decisions = {})
           /\ (initialContext.height > 0
                 => /\ commitIntents =
                          BootstrapParentCommitIntents(initialContext)
                    /\ decisions =
                          {BootstrapParentDecision(initialContext)}
                    /\ BootstrapParentContext(initialContext)
                          # initialContext)
      BY <1>1, BootstrapParentContextPrecedes, SMT
         DEF AsyncInitAt, AsyncBaseInitAt, InitAt
    <2>2. DurableCommitProgressWitness
      <3>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                    NEW vote \in commitIntents,
                    ActiveLockedCommitIntent(node, vote)
             PROVE CommitIntentProgressWitness(node, vote)
        <4>1. CASE initialContext.height = 0
          BY <2>1, <3>1, <4>1
        <4>2. CASE initialContext.height > 0
          <5>1. vote.context = BootstrapParentContext(initialContext)
            BY <2>1, <3>1, <4>2, Isa
               DEF BootstrapParentCommitIntents, Vote
          <5>2. vote.context = initialContext
            BY <2>1, <3>1 DEF ActiveLockedCommitIntent
          <5> QED BY <2>1, <4>2, <5>1, <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1 DEF DurableCommitProgressWitness
    <2>3. HistoricalLockedCommitRecoveryProgress
      BY <1>1
         DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
             HistoricalLockedCommitRecoveryProgress,
             HistoricalLockedPrepareForCommit,
             InstalledTcSelectsPrepareFor
    <2>4. DurableDecisionProgressWitness
      <3>1. ASSUME NEW decision \in decisions,
                    /\ decision.node \in AsyncCurrentResponsiveVoters
                    /\ decision.qc.context = context
             PROVE DecisionCompletionWitness(decision.node, decision.qc)
        <4>1. CASE initialContext.height = 0
          BY <2>1, <3>1, <4>1
        <4>2. CASE initialContext.height > 0
          <5>1. decision = BootstrapParentDecision(initialContext)
            BY <2>1, <3>1, <4>2
          <5>2. decision.qc.context =
                   BootstrapParentContext(initialContext)
            BY <5>1
               DEF BootstrapParentDecision,
                   BootstrapParentCommitQC, QC
          <5>3. decision.qc.context = initialContext
            BY <2>1, <3>1
          <5> QED BY <2>1, <4>2, <5>2, <5>3
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1 DEF DurableDecisionProgressWitness
    <2>5. ProtectedDeferredProgressInvariant
      BY <1>1, FS_EmptySet, SMT
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncDeferredInit,
             ProtectedDeferredProgressInvariant,
             ProtectedDeferredProgressIndices, SequenceSet,
             AsyncConfiguration, ModelConfiguration,
             QuorumConfiguration
    <2> QED BY <2>2, <2>3, <2>4, <2>5
         DEF ProgressWitnessInvariant
  <1> QED BY <1>1


ProtectedProgressIndicesIn(queue) ==
  {index \in 1..Len(queue): ProtectedProgressCommand(queue[index])}

ProtectedProgressSlotsUniqueIn(queue) ==
  \A left, right \in ProtectedProgressIndicesIn(queue):
    SameProtectedProgressSlot(queue[left], queue[right]) => left = right

ProtectedProgressSlotId(command) ==
  CASE command.kind = "DeliverVote" ->
         IF command.item.kind = "CommitVote"
         THEN command.item.envelope.vote.signer
         ELSE N + command.item.envelope.vote.signer
    [] command.kind = "DeliverTimeout" ->
         2 * N + command.item.envelope.vote.signer
    [] command.kind = "DeliverQC" ->
         IF command.item.kind = "PrepareQC" THEN 3 * N ELSE 3 * N + 1
    [] OTHER -> 3 * N + 2

ProtectedProgressNumericSlotMap(queue) ==
  [index \in ProtectedProgressIndicesIn(queue) |->
     ProtectedProgressSlotId(queue[index])]

THEOREM SameProtectedProgressSlotSymmetric ==
  \A left, right:
    SameProtectedProgressSlot(left, right)
      => SameProtectedProgressSlot(right, left)
BY SMT DEF SameProtectedProgressSlot, ProtectedProgressCommand,
           HistoricalLockedCommitItem

THEOREM SameProtectedProgressSlotTransitive ==
  \A left, middle, right:
    /\ SameProtectedProgressSlot(left, middle)
    /\ SameProtectedProgressSlot(left, right)
    => SameProtectedProgressSlot(middle, right)
BY SMT DEF SameProtectedProgressSlot, ProtectedProgressCommand,
           HistoricalLockedCommitItem

THEOREM TypedProtectedProgressCommandShape ==
  \A command:
    /\ AsyncCandidateTyped(command)
    /\ ProtectedProgressCommand(command)
    => \/ /\ command.kind = "DeliverVote"
           /\ command.item.kind \in {"CommitVote", "PrepareVote"}
           /\ command.item.envelope.vote.signer \in ValidatorIds
       \/ /\ command.kind = "DeliverQC"
           /\ command.item.kind \in {"PrepareQC", "CommitQC"}
       \/ /\ command.kind = "DeliverTimeout"
           /\ command.item.kind = "TimeoutVote"
           /\ command.item.envelope.vote.signer \in ValidatorIds
       \/ /\ command.kind = "DeliverTC"
           /\ command.item.kind = "TimeoutCertificate"
PROOF
  <1>1. ASSUME NEW command,
                AsyncCandidateTyped(command),
                ProtectedProgressCommand(command)
         PROVE \/ /\ command.kind = "DeliverVote"
                    /\ command.item.kind \in {"CommitVote", "PrepareVote"}
                    /\ command.item.envelope.vote.signer \in ValidatorIds
               \/ /\ command.kind = "DeliverQC"
                    /\ command.item.kind \in {"PrepareQC", "CommitQC"}
               \/ /\ command.kind = "DeliverTimeout"
                    /\ command.item.kind = "TimeoutVote"
                    /\ command.item.envelope.vote.signer \in ValidatorIds
               \/ /\ command.kind = "DeliverTC"
                    /\ command.item.kind = "TimeoutCertificate"
    <2>1. AsyncItemTyped(command.item)
      BY <1>1, SMT
         DEF ProtectedProgressCommand, HistoricalLockedCommitItem,
             AsyncCandidateTyped, NoAsyncItem, AsyncBodyEnvelope
    <2>2. CASE command.kind = "DeliverVote"
      <3>1. command.item.kind \in {"CommitVote", "PrepareVote"}
        BY <1>1, <2>2
           DEF ProtectedProgressCommand, HistoricalLockedCommitItem
      <3>2. command.item.envelope \in VoteEnvelopeSet
        BY <2>1, <3>1 DEF AsyncItemTyped
      <3>3. command.item.envelope.vote.signer \in ValidatorIds
        BY <3>2 DEF VoteEnvelopeSet, VoteRecordSet
      <3> QED BY <2>2, <3>1, <3>3
    <2>3. CASE command.kind = "DeliverQC"
      BY <1>1, <2>3 DEF ProtectedProgressCommand
    <2>4. CASE command.kind = "DeliverTimeout"
      <3>1. command.item.kind = "TimeoutVote"
        BY <1>1, <2>4 DEF ProtectedProgressCommand
      <3>2. command.item.envelope \in TimeoutEnvelopeSet
        BY <2>1, <3>1 DEF AsyncItemTyped
      <3>3. command.item.envelope.vote.signer \in ValidatorIds
        BY <3>2 DEF TimeoutEnvelopeSet, TimeoutVoteRecordSet
      <3> QED BY <2>4, <3>1, <3>3
    <2>5. CASE command.kind = "DeliverTC"
      BY <1>1, <2>5 DEF ProtectedProgressCommand
    <2>6. CASE command.kind \notin
                   {"DeliverVote", "DeliverQC", "DeliverTimeout", "DeliverTC"}
      BY <1>1, <2>6, SMT DEF ProtectedProgressCommand
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, SMT
  <1> QED BY <1>1

THEOREM ProtectedProgressSlotIdIsBounded ==
  \A command:
    /\ N \in Nat \ {0}
    /\ AsyncCandidateTyped(command)
    /\ ProtectedProgressCommand(command)
    => ProtectedProgressSlotId(command) \in 0..(3 * N + 2)
BY TypedProtectedProgressCommandShape, SMT
   DEF ProtectedProgressSlotId, ValidatorIds

THEOREM EqualProtectedProgressSlotIdsIdentifySlot ==
  \A left, right:
    /\ N \in Nat \ {0}
    /\ AsyncCandidateTyped(left)
    /\ AsyncCandidateTyped(right)
    /\ ProtectedProgressCommand(left)
    /\ ProtectedProgressCommand(right)
    /\ left.node = right.node
    /\ ProtectedProgressSlotId(left) = ProtectedProgressSlotId(right)
    => SameProtectedProgressSlot(left, right)
BY TypedProtectedProgressCommandShape, SMTT(30)
   DEF ProtectedProgressSlotId, SameProtectedProgressSlot,
       ProtectedProgressCommand, ValidatorIds

THEOREM UniqueOwnedProtectedSlotsGiveInjection ==
  \A node, queue:
    /\ N \in Nat \ {0}
    /\ AsyncQueueTyped(queue)
    /\ AsyncCommandQueueOwnership(node, queue)
    /\ ProtectedProgressSlotsUniqueIn(queue)
    => ProtectedProgressNumericSlotMap(queue)
         \in Injection(ProtectedProgressIndicesIn(queue), 0..(3 * N + 2))
PROOF
  <1>1. ASSUME NEW node, NEW queue,
                N \in Nat \ {0},
                AsyncQueueTyped(queue),
                AsyncCommandQueueOwnership(node, queue),
                ProtectedProgressSlotsUniqueIn(queue)
         PROVE ProtectedProgressNumericSlotMap(queue)
                 \in Injection(
                      ProtectedProgressIndicesIn(queue), 0..(3 * N + 2))
    <2>1. \A index \in ProtectedProgressIndicesIn(queue):
             /\ AsyncCandidateTyped(queue[index])
             /\ queue[index].node = node
             /\ ProtectedProgressSlotId(queue[index]) \in 0..(3 * N + 2)
      BY <1>1, ProtectedProgressSlotIdIsBounded
         DEF ProtectedProgressIndicesIn, AsyncQueueTyped,
             AsyncCommandQueueOwnership, SequenceSet
    <2>2. ProtectedProgressNumericSlotMap(queue)
             \in [ProtectedProgressIndicesIn(queue) -> 0..(3 * N + 2)]
      BY <2>1 DEF ProtectedProgressNumericSlotMap
    <2>3. \A left, right \in ProtectedProgressIndicesIn(queue):
             ProtectedProgressNumericSlotMap(queue)[left]
               = ProtectedProgressNumericSlotMap(queue)[right]
               => left = right
      <3>1. ASSUME NEW left,
                    NEW right,
                    left \in ProtectedProgressIndicesIn(queue),
                    right \in ProtectedProgressIndicesIn(queue),
                    ProtectedProgressNumericSlotMap(queue)[left]
                      = ProtectedProgressNumericSlotMap(queue)[right]
             PROVE left = right
        <4>1. /\ AsyncCandidateTyped(queue[left])
               /\ AsyncCandidateTyped(queue[right])
               /\ queue[left].node = queue[right].node
          BY <2>1, <3>1
        <4>2. /\ ProtectedProgressCommand(queue[left])
               /\ ProtectedProgressCommand(queue[right])
          BY <3>1 DEF ProtectedProgressIndicesIn
        <4>3. ProtectedProgressSlotId(queue[left]) =
                 ProtectedProgressSlotId(queue[right])
          BY <3>1 DEF ProtectedProgressNumericSlotMap
        <4>4. SameProtectedProgressSlot(queue[left], queue[right])
          BY <1>1, <4>1, <4>2, <4>3,
             EqualProtectedProgressSlotIdsIdentifySlot
        <4> QED BY <1>1, <3>1, <4>4
             DEF ProtectedProgressSlotsUniqueIn
      <3> QED BY <3>1
    <2>4. ASSUME NEW left,
                  NEW right,
                  left \in DOMAIN ProtectedProgressNumericSlotMap(queue),
                  right \in DOMAIN ProtectedProgressNumericSlotMap(queue),
                  ProtectedProgressNumericSlotMap(queue)[left] =
                    ProtectedProgressNumericSlotMap(queue)[right]
           PROVE left = right
      BY <2>3, <2>4 DEF ProtectedProgressNumericSlotMap
    <2>5. IsInjective(ProtectedProgressNumericSlotMap(queue))
      BY <2>4 DEF IsInjective
    <2> QED BY <2>2, <2>5 DEF Injection
  <1> QED BY <1>1

THEOREM UniqueTypedOwnedProtectedSlotsAreBounded ==
  \A node, queue:
    /\ N \in Nat \ {0}
    /\ AsyncQueueTyped(queue)
    /\ AsyncCommandQueueOwnership(node, queue)
    /\ ProtectedProgressSlotsUniqueIn(queue)
    => Cardinality(ProtectedProgressIndicesIn(queue)) <= 3 * N + 3
PROOF
  <1>1. ASSUME NEW node, NEW queue,
                N \in Nat \ {0},
                AsyncQueueTyped(queue),
                AsyncCommandQueueOwnership(node, queue),
                ProtectedProgressSlotsUniqueIn(queue)
         PROVE Cardinality(ProtectedProgressIndicesIn(queue)) <= 3 * N + 3
    <2>1. ProtectedProgressNumericSlotMap(queue)
             \in Injection(
                  ProtectedProgressIndicesIn(queue), 0..(3 * N + 2))
      BY <1>1, UniqueOwnedProtectedSlotsGiveInjection
    <2>2. IsFiniteSet(0..(3 * N + 2))
      BY <1>1, FS_Interval, SMT
    <2>3. /\ IsFiniteSet(ProtectedProgressIndicesIn(queue))
           /\ Cardinality(ProtectedProgressIndicesIn(queue))
                <= Cardinality(0..(3 * N + 2))
      BY <2>1, <2>2, FS_Injection
    <2>4. Cardinality(0..(3 * N + 2)) = 3 * N + 3
      BY <1>1, FS_Interval, SMT
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

(***************************************************************************
Protected-slot uniqueness is invariant under the concrete no-displacement
queue edits: a fresh owner appends below capacity, while an exact duplicate,
same-owner collision, or full queue leaves the queue unchanged.  Tail removal
is a pure restriction.
***************************************************************************)

THEOREM AppendFreshProtectedSlotPreservesUniqueness ==
  \A queue, command:
    /\ AsyncQueueTyped(queue)
    /\ ProtectedProgressSlotsUniqueIn(queue)
    /\ \A index \in ProtectedProgressIndicesIn(queue):
         ~SameProtectedProgressSlot(queue[index], command)
    => ProtectedProgressSlotsUniqueIn(Append(queue, command))
PROOF
  <1>1. ASSUME NEW queue, NEW command,
                AsyncQueueTyped(queue),
                ProtectedProgressSlotsUniqueIn(queue),
                \A index \in ProtectedProgressIndicesIn(queue):
                  ~SameProtectedProgressSlot(queue[index], command)
         PROVE ProtectedProgressSlotsUniqueIn(Append(queue, command))
    <2>1. /\ Len(queue) \in Nat
           /\ Len(Append(queue, command)) = Len(queue) + 1
           /\ \A index \in 1..Len(queue):
                Append(queue, command)[index] = queue[index]
           /\ Append(queue, command)[Len(queue) + 1] = command
      BY <1>1, AppendSequenceFacts, LenProperties
         DEF AsyncQueueTyped
    <2>2. ASSUME NEW left,
                  NEW right,
                  left \in ProtectedProgressIndicesIn(
                    Append(queue, command)),
                  right \in ProtectedProgressIndicesIn(
                    Append(queue, command)),
                  SameProtectedProgressSlot(
                    Append(queue, command)[left],
                    Append(queue, command)[right])
           PROVE left = right
      <3>1. /\ left \in 1..(Len(queue) + 1)
             /\ right \in 1..(Len(queue) + 1)
        BY <1>1, <2>1, <2>2 DEF ProtectedProgressIndicesIn
      <3>2. CASE left \in 1..Len(queue)
                       /\ right \in 1..Len(queue)
        BY <1>1, <2>1, <2>2, <3>2
           DEF ProtectedProgressIndicesIn,
               ProtectedProgressSlotsUniqueIn
      <3>3. CASE left \notin 1..Len(queue)
        <4>1. left = Len(queue) + 1
          BY <2>1, <3>1, <3>3, SMT
        <4>2. CASE right \in 1..Len(queue)
          <5>1. /\ right \in ProtectedProgressIndicesIn(queue)
                 /\ SameProtectedProgressSlot(queue[right], command)
            BY <2>1, <2>2, <4>1, <4>2,
               SameProtectedProgressSlotSymmetric
               DEF ProtectedProgressIndicesIn
          <5> QED BY <1>1, <5>1
        <4>3. CASE right \notin 1..Len(queue)
          BY <2>1, <3>1, <3>3, <4>3, SMT
        <4> QED BY <4>2, <4>3
      <3>4. CASE left \in 1..Len(queue)
                       /\ right \notin 1..Len(queue)
        <4>1. right = Len(queue) + 1
          BY <2>1, <3>1, <3>4, SMT
        <4>2. /\ left \in ProtectedProgressIndicesIn(queue)
               /\ SameProtectedProgressSlot(queue[left], command)
          BY <2>1, <2>2, <3>4, <4>1
             DEF ProtectedProgressIndicesIn
        <4> QED BY <1>1, <4>2
      <3> QED BY <3>2, <3>3, <3>4
    <2> QED BY <2>2 DEF ProtectedProgressSlotsUniqueIn
  <1> QED BY <1>1

THEOREM TailPreservesProtectedSlotUniqueness ==
  \A queue:
    /\ AsyncQueueTyped(queue)
    /\ Len(queue) > 0
    /\ ProtectedProgressSlotsUniqueIn(queue)
    => ProtectedProgressSlotsUniqueIn(Tail(queue))
PROOF
  <1>1. ASSUME NEW queue,
                AsyncQueueTyped(queue),
                Len(queue) > 0,
                ProtectedProgressSlotsUniqueIn(queue)
         PROVE ProtectedProgressSlotsUniqueIn(Tail(queue))
    <2>1. /\ Len(Tail(queue)) = Len(queue) - 1
           /\ \A index \in 1..Len(Tail(queue)):
                Tail(queue)[index] = queue[index + 1]
      BY <1>1, TypedQueueHeadTailIndexFacts
    <2>2. ASSUME NEW left,
                  NEW right,
                  left \in ProtectedProgressIndicesIn(Tail(queue)),
                  right \in ProtectedProgressIndicesIn(Tail(queue)),
                  SameProtectedProgressSlot(
                    Tail(queue)[left], Tail(queue)[right])
           PROVE left = right
      <3>1. /\ left + 1 \in ProtectedProgressIndicesIn(queue)
             /\ right + 1 \in ProtectedProgressIndicesIn(queue)
             /\ SameProtectedProgressSlot(
                  queue[left + 1], queue[right + 1])
        BY <1>1, <2>1, <2>2, SMT
           DEF ProtectedProgressIndicesIn, AsyncQueueTyped
      <3>2. left + 1 = right + 1
        BY <1>1, <3>1 DEF ProtectedProgressSlotsUniqueIn
      <3> QED BY <3>2, SMT
    <2> QED BY <2>2 DEF ProtectedProgressSlotsUniqueIn
  <1> QED BY <1>1

THEOREM DeferredProgressAfterPreservesProtectedSlotsUnique ==
  \A node \in ValidatorIds:
    \A command:
      /\ AsyncConfiguration
      /\ AsyncDeferredContentTypeInvariant
      /\ ProtectedProgressSlotsUniqueIn(
           asyncDeferredProgressQueues[node])
      => ProtectedProgressSlotsUniqueIn(
           DeferredProgressAfter(node, command))
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW command,
                AsyncConfiguration,
                AsyncDeferredContentTypeInvariant,
                ProtectedProgressSlotsUniqueIn(
                  asyncDeferredProgressQueues[node])
         PROVE ProtectedProgressSlotsUniqueIn(

                 DeferredProgressAfter(node, command))
    <2> DEFINE Queue == asyncDeferredProgressQueues[node]
    <2>1. /\ AsyncQueueTyped(Queue)
           /\ ProtectedProgressSlotsUniqueIn(Queue)
      BY <1>1 DEF AsyncDeferredContentTypeInvariant, Queue
    <2>2. CASE command \in SequenceSet(Queue)
      BY <2>1, <2>2 DEF DeferredProgressAfter, Queue
    <2>3. CASE /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) # {}
      BY <2>1, <2>3 DEF DeferredProgressAfter, Queue
    <2>4. CASE /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) = {}
                 /\ Len(Queue) < AsyncDeferredProgressCapacity
      <3>1. \A index \in ProtectedProgressIndicesIn(Queue):
               ~SameProtectedProgressSlot(Queue[index], command)
        BY <2>4, SMT
           DEF SameProtectedProgressSlotIndices,
               ProtectedProgressIndicesIn, Queue
      <3>2. ProtectedProgressSlotsUniqueIn(Append(Queue, command))
        BY <2>1, <3>1,
           AppendFreshProtectedSlotPreservesUniqueness
      <3>3. DeferredProgressAfter(node, command) = Append(Queue, command)
        BY <2>4 DEF DeferredProgressAfter, Queue
      <3> QED BY <3>2, <3>3
    <2>5. CASE /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) = {}
                 /\ Len(Queue) >= AsyncDeferredProgressCapacity
      BY <2>1, <2>5 DEF DeferredProgressAfter, Queue
    <2>6. \/ command \in SequenceSet(Queue)
           \/ /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) # {}
           \/ /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) = {}
                 /\ Len(Queue) < AsyncDeferredProgressCapacity
           \/ /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) = {}
                 /\ Len(Queue) >= AsyncDeferredProgressCapacity
      BY <1>1, <2>1, SMT
         DEF AsyncQueueTyped, AsyncConfiguration
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM ProtectedDeferredProgressIndicesAreQueueIndices ==
  \A node \in ValidatorIds:
    ProtectedDeferredProgressIndices(node) =
      ProtectedProgressIndicesIn(asyncDeferredProgressQueues[node])
BY DEF ProtectedDeferredProgressIndices, ProtectedProgressIndicesIn

THEOREM DeferCommandPreservesProtectedDeferredProgressInvariant ==
  \A command:
    /\ N \in Nat \ {0}
    /\ AsyncConfiguration
    /\ AsyncDeferredTypeInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ AsyncCandidateTyped(command)
    /\ DeferCommand(command)
    => ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME NEW command,
                N \in Nat \ {0},
                AsyncConfiguration,
                AsyncDeferredTypeInvariant,
                ProtectedDeferredProgressInvariant,
                AsyncCandidateTyped(command),
                DeferCommand(command)
         PROVE ProtectedDeferredProgressInvariant'
    <2>1. AsyncDeferredContentTypeInvariant'
      BY <1>1, DeferTypedOwnedCommandPreservesDeferredContentType
         DEF AsyncDeferredTypeInvariant
    <2>2. \A node \in ValidatorIds:
             ProtectedProgressSlotsUniqueIn(
               asyncDeferredProgressQueues'[node])
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE ProtectedProgressSlotsUniqueIn(
                     asyncDeferredProgressQueues'[node])
        <4>1. CASE /\ command.class = "Progress"
                     /\ node = command.node
          <5>1. asyncDeferredProgressQueues'[node] =
                   DeferredProgressAfter(node, command)
            BY <1>1, <3>1, <4>1, Isa
               DEF DeferCommand, AsyncDeferredTypeInvariant,
                   AsyncDeferredTopologyTypeInvariant
          <5>2. ProtectedProgressSlotsUniqueIn(
                   asyncDeferredProgressQueues[node])
            BY <1>1, <3>1,
               ProtectedDeferredProgressIndicesAreQueueIndices
               DEF ProtectedDeferredProgressInvariant,
                   ProtectedProgressSlotsUniqueIn
          <5>3. ProtectedProgressSlotsUniqueIn(
                   DeferredProgressAfter(node, command))
            BY <1>1, <3>1, <5>2,
               DeferredProgressAfterPreservesProtectedSlotsUnique
               DEF AsyncDeferredTypeInvariant
          <5> QED BY <5>1, <5>3
        <4>2. CASE ~(command.class = "Progress" /\ node = command.node)
          <5>1. asyncDeferredProgressQueues'[node] =
                   asyncDeferredProgressQueues[node]
            BY <1>1, <3>1, <4>2, FunctionalUpdateAwayFromKey, Isa
               DEF DeferCommand, AsyncDeferredTypeInvariant,
                   AsyncDeferredTopologyTypeInvariant
          <5>2. ProtectedProgressSlotsUniqueIn(
                   asyncDeferredProgressQueues[node])
            BY <1>1, <3>1,
               ProtectedDeferredProgressIndicesAreQueueIndices

               DEF ProtectedDeferredProgressInvariant,
                   ProtectedProgressSlotsUniqueIn
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>3. \A node \in ValidatorIds:
             Cardinality(
               ProtectedProgressIndicesIn(
                 asyncDeferredProgressQueues'[node])) <= 3 * N + 3
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE Cardinality(
                     ProtectedProgressIndicesIn(
                       asyncDeferredProgressQueues'[node])) <= 3 * N + 3
        <4>1. /\ AsyncQueueTyped(
                      asyncDeferredProgressQueues'[node])
               /\ AsyncCommandQueueOwnership(
                    node, asyncDeferredProgressQueues'[node])
          BY <2>1, <3>1 DEF AsyncDeferredContentTypeInvariant
        <4> QED BY <1>1, <2>2, <3>1, <4>1,
             UniqueTypedOwnedProtectedSlotsAreBounded
      <3> QED BY <3>1
    <2>4. /\ context' = context
           /\ nodeView' = nodeView
           /\ lockRank' = lockRank
           /\ lockSubject' = lockSubject
           /\ prepareQCs' = prepareQCs
      BY <1>1, Isa DEF DeferCommand, vars
    <2>5. \A node \in ValidatorIds:
             ProtectedDeferredProgressIndices(node)' =
               ProtectedProgressIndicesIn(
                 asyncDeferredProgressQueues'[node])
      BY <2>4
         DEF ProtectedDeferredProgressIndices,
             ProtectedProgressIndicesIn, ProtectedProgressCommand,
             HistoricalLockedCommitItem, LockedPrepareRound
    <2>6. \A left, right:
             SameProtectedProgressSlot(left, right)' =
               SameProtectedProgressSlot(left, right)
      BY <2>4
         DEF SameProtectedProgressSlot, ProtectedProgressCommand,
             HistoricalLockedCommitItem, LockedPrepareRound
    <2> QED BY <2>2, <2>3, <2>5, <2>6
         DEF ProtectedDeferredProgressInvariant,
             ProtectedProgressSlotsUniqueIn
  <1> QED BY <1>1

THEOREM RemoveSelectedDeferredProgressQueueEffect ==
  \A target \in ValidatorIds:
    \A node \in ValidatorIds:
      /\ AsyncDeferredTypeInvariant
      /\ DeferredQueueNonempty(target)
      /\ RemoveNextDeferredCommand(target)
      => asyncDeferredProgressQueues'[node] =
           IF node = target
                /\ SelectedDeferredClass(target) = "Progress"
           THEN Tail(asyncDeferredProgressQueues[target])
           ELSE asyncDeferredProgressQueues[node]
PROOF
  <1>1. ASSUME NEW target \in ValidatorIds,
                NEW node \in ValidatorIds,
                AsyncDeferredTypeInvariant,
                DeferredQueueNonempty(target),
                RemoveNextDeferredCommand(target)
         PROVE asyncDeferredProgressQueues'[node] =
                 IF node = target
                      /\ SelectedDeferredClass(target) = "Progress"
                 THEN Tail(asyncDeferredProgressQueues[target])
                 ELSE asyncDeferredProgressQueues[node]
    <2>1. DOMAIN asyncDeferredProgressQueues = ValidatorIds
      BY <1>1
         DEF AsyncDeferredTypeInvariant,
             AsyncDeferredTopologyTypeInvariant
    <2>2. CASE SelectedDeferredClass(target) = "Progress"
      <3>1. CASE node = target
        BY <1>1, <2>1, <2>2, <3>1, FunctionalTailUpdateAtKey
           DEF RemoveNextDeferredCommand
      <3>2. CASE node # target
        BY <1>1, <2>1, <2>2, <3>2, FunctionalTailUpdateAwayFromKey
           DEF RemoveNextDeferredCommand
      <3> QED BY <3>1, <3>2
    <2>3. CASE SelectedDeferredClass(target) # "Progress"
      <3>1. CASE SelectedDeferredClass(target) = "Completion"
        BY <1>1, <3>1 DEF RemoveNextDeferredCommand
      <3>2. CASE SelectedDeferredClass(target) # "Completion"
        BY <1>1, <2>3, <3>2 DEF RemoveNextDeferredCommand
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM RemoveNextDeferredCommandPreservesProtectedDeferredProgressInvariant ==
  \A target \in ValidatorIds:
    /\ N \in Nat \ {0}
    /\ AsyncDeferredTypeInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ DeferredQueueNonempty(target)
    /\ RemoveNextDeferredCommand(target)
    /\ UNCHANGED <<context, nodeView, lockRank, lockSubject, prepareQCs>>
    => ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME NEW target \in ValidatorIds,
                N \in Nat \ {0},
                AsyncDeferredTypeInvariant,
                ProtectedDeferredProgressInvariant,
                DeferredQueueNonempty(target),
                RemoveNextDeferredCommand(target),
                UNCHANGED <<context, nodeView, lockRank, lockSubject,
                            prepareQCs>>
         PROVE ProtectedDeferredProgressInvariant'
    <2>1. AsyncDeferredContentTypeInvariant'
      BY <1>1, RemoveNextDeferredCommandPreservesDeferredContentType
         DEF AsyncDeferredTypeInvariant
    <2>2. \A node \in ValidatorIds:
             ProtectedProgressSlotsUniqueIn(
               asyncDeferredProgressQueues'[node])
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE ProtectedProgressSlotsUniqueIn(
                     asyncDeferredProgressQueues'[node])
        <4>1. ProtectedProgressSlotsUniqueIn(
                 asyncDeferredProgressQueues[node])
          BY <1>1, <3>1,
             ProtectedDeferredProgressIndicesAreQueueIndices
             DEF ProtectedDeferredProgressInvariant,
                 ProtectedProgressSlotsUniqueIn
        <4>2. CASE /\ node = target
                     /\ SelectedDeferredClass(target) = "Progress"
          <5>1. asyncDeferredProgressQueues'[node] =
                   Tail(asyncDeferredProgressQueues[node])
            BY <1>1, <3>1, <4>2,
               RemoveSelectedDeferredProgressQueueEffect
               DEF AsyncDeferredTypeInvariant
          <5>2. AsyncQueueTyped(asyncDeferredProgressQueues[node])
            BY <1>1, <3>1
               DEF AsyncDeferredTypeInvariant,
                   AsyncDeferredContentTypeInvariant
          <5>3. ProtectedProgressSlotsUniqueIn(
                   Tail(asyncDeferredProgressQueues[node]))
            BY <4>1, <4>2, <5>2,
               TailPreservesProtectedSlotUniqueness
          <5> QED BY <5>1, <5>3
        <4>3. CASE ~(node = target
                       /\ SelectedDeferredClass(target) = "Progress")
          <5>1. asyncDeferredProgressQueues'[node] =
                   asyncDeferredProgressQueues[node]
            BY <1>1, <3>1, <4>3,
               RemoveSelectedDeferredProgressQueueEffect
               DEF AsyncDeferredTypeInvariant
          <5> QED BY <4>1, <5>1
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>3. \A node \in ValidatorIds:
             Cardinality(
               ProtectedProgressIndicesIn(
                 asyncDeferredProgressQueues'[node])) <= 3 * N + 3
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE Cardinality(
                     ProtectedProgressIndicesIn(
                       asyncDeferredProgressQueues'[node])) <= 3 * N + 3
        <4>1. /\ AsyncQueueTyped(
                      asyncDeferredProgressQueues'[node])
               /\ AsyncCommandQueueOwnership(
                    node, asyncDeferredProgressQueues'[node])
          BY <2>1, <3>1 DEF AsyncDeferredContentTypeInvariant
        <4> QED BY <1>1, <2>2, <3>1, <4>1,
             UniqueTypedOwnedProtectedSlotsAreBounded
      <3> QED BY <3>1
    <2>4. \A node \in ValidatorIds:
             ProtectedDeferredProgressIndices(node)' =
               ProtectedProgressIndicesIn(
                 asyncDeferredProgressQueues'[node])
      BY <1>1
         DEF ProtectedDeferredProgressIndices,
             ProtectedProgressIndicesIn, ProtectedProgressCommand,
             HistoricalLockedCommitItem, LockedPrepareRound
    <2>5. \A left, right:
             SameProtectedProgressSlot(left, right)' =
               SameProtectedProgressSlot(left, right)
      BY <1>1
         DEF SameProtectedProgressSlot, ProtectedProgressCommand,
             HistoricalLockedCommitItem, LockedPrepareRound
    <2> QED BY <2>2, <2>3, <2>4, <2>5
         DEF ProtectedDeferredProgressInvariant,
             ProtectedProgressSlotsUniqueIn
  <1> QED BY <1>1

(***************************************************************************
A Progress-class Commit delivery entered the scheduler while it was a
historical locked vote.  Later lock advancement can only leave that vote
historical or move it strictly behind the lock frontier.  Recording this
small history disjunction prevents an install from activating an arbitrary
dormant Progress command that no concrete delivery classifier could create.
***************************************************************************)
ProgressCommitVoteHistory(command) ==
  IF command.kind = "DeliverVote" /\ command.item.kind = "CommitVote"
  THEN \/ HistoricalLockedCommitItem(command.item)
       \/ command.item.envelope.vote.view <
            lockRank[command.item.envelope.recipient]
  ELSE TRUE

THEOREM StrictCycleImpossible ==
  \A low, high \in Int:
    low < high /\ high <= low => FALSE
BY SMT

THEOREM NaturalWeakOrderTrans ==
  \A low, middle, high \in Nat:
    low <= middle /\ middle <= high => low <= high
BY SMT

THEOREM StrictWeakOrderTrans ==
  \A low, middle, high \in Int:
    low < middle /\ middle <= high => low < high
BY SMT

ProgressSlotShape(left, right) ==
  /\ left.node = right.node
  /\ CASE left.kind = "DeliverVote" ->
            /\ right.kind = "DeliverVote"
            /\ left.item.kind = right.item.kind
            /\ left.item.envelope.vote.signer =
                 right.item.envelope.vote.signer
       [] left.kind = "DeliverQC" ->
            /\ right.kind = "DeliverQC"
            /\ left.item.kind = right.item.kind
       [] left.kind = "DeliverTimeout" ->
            /\ right.kind = "DeliverTimeout"
            /\ left.item.envelope.vote.signer =
                 right.item.envelope.vote.signer
       [] OTHER -> right.kind = "DeliverTC"

THEOREM ProtectedProgressCommandBackwardStableUnderLockAdvance ==
  \A command:
    /\ AsyncCandidateTyped(command)
    /\ TypeInvariant

    /\ ProgressCommitVoteHistory(command)
    /\ ProtectedProgressCommand(command)'
    /\ LockMonotonicityAction
    /\ UNCHANGED <<context, prepareQCs>>
    => ProtectedProgressCommand(command)
PROOF
  <1>1. ASSUME NEW command,
                AsyncCandidateTyped(command),
                TypeInvariant,
                ProgressCommitVoteHistory(command),
                ProtectedProgressCommand(command)',
                LockMonotonicityAction,
                UNCHANGED <<context, prepareQCs>>
         PROVE ProtectedProgressCommand(command)
    <2>2. /\ context' = context
           /\ prepareQCs' = prepareQCs
      BY <1>1, Isa
    <2>3. CASE command.kind = "DeliverVote"
      <3>1. CASE command.item.kind = "PrepareVote"
        BY <1>1, <2>3, <3>1 DEF ProtectedProgressCommand
      <3>2. CASE command.item.kind = "CommitVote"
        <4>1. HistoricalLockedCommitItem(command.item)'
          BY <1>1, <2>3, <3>2
             DEF ProtectedProgressCommand, HistoricalLockedCommitItem
        <4>1a. command.item # NoAsyncItem
          BY <3>2 DEF NoAsyncItem, AsyncBodyEnvelope
        <4>1b. AsyncItemTyped(command.item)
          BY <1>1, <4>1a DEF AsyncCandidateTyped
        <4>1c. command.item.envelope.recipient \in ValidatorIds
          BY <4>1b DEF AsyncItemTyped
        <4>2. \/ HistoricalLockedCommitItem(command.item)
               \/ command.item.envelope.vote.view <
                    lockRank[command.item.envelope.recipient]
          BY <1>1, <2>3, <3>2 DEF ProgressCommitVoteHistory
        <4>3. CASE HistoricalLockedCommitItem(command.item)
          BY <2>3, <4>3 DEF ProtectedProgressCommand
        <4>4. CASE command.item.envelope.vote.view <
                       lockRank[command.item.envelope.recipient]
          <5>0a. command.item.envelope.vote.view \in Views
            BY <3>2, <4>1b DEF AsyncItemTyped, VoteEnvelopeSet,
                                VoteRecordSet
          <5>0b. /\ lockRank \in [ValidatorIds -> Ranks]
                  /\ Ranks \subseteq Int
            BY <1>1, ModelRanksAreIntegers DEF TypeInvariant
          <5>0. /\ command.item.envelope.vote.view \in Int
                 /\ lockRank[command.item.envelope.recipient] \in Int
            BY <4>1c, <5>0a, <5>0b, ViewsAreRanks,
               FunctionValueHasCodomain, Isa
          <5>1. lockRank'[command.item.envelope.recipient] >=
                   lockRank[command.item.envelope.recipient]
            BY <1>1, <2>2, <4>1c DEF LockMonotonicityAction
          <5>2. lockRank'[command.item.envelope.recipient] =
                   command.item.envelope.vote.view
            BY <4>1 DEF HistoricalLockedCommitItem, LockedPrepareRound
          <5>3. FALSE
            BY <4>4, <5>0, <5>1, <5>2, StrictCycleImpossible
          <5> QED BY <5>3
        <4> QED BY <4>2, <4>3, <4>4
      <3>3. CASE command.item.kind \notin {"PrepareVote", "CommitVote"}
        BY <1>1, <2>3, <3>3, SMT DEF ProtectedProgressCommand
      <3> QED BY <3>1, <3>2, <3>3
    <2>4. CASE command.kind = "DeliverQC"
      BY <1>1, <2>4 DEF ProtectedProgressCommand
    <2>5. CASE command.kind = "DeliverTimeout"
      BY <1>1, <2>5 DEF ProtectedProgressCommand
    <2>6. CASE command.kind = "DeliverTC"
      BY <1>1, <2>6 DEF ProtectedProgressCommand
    <2>7. CASE command.kind \notin
                   {"DeliverVote", "DeliverQC", "DeliverTimeout", "DeliverTC"}
      BY <1>1, <2>7, SMT DEF ProtectedProgressCommand
    <2> QED BY <2>3, <2>4, <2>5, <2>6, <2>7, SMT
  <1> QED BY <1>1

ProgressCommitSource(command) ==
  \/ command.class # "Progress"
  \/ ProgressCommitVoteHistory(command)

ProgressCommitSourcesIn(queue) ==
  \A command \in SequenceSet(queue): ProgressCommitSource(command)

QueuedProgressCommitHistoryInvariant ==
  \A node \in ValidatorIds:
    ProgressCommitSourcesIn(asyncCommandQueues[node])

DeferredProgressCommitHistoryInvariant ==
  \A node \in ValidatorIds:
    ProgressCommitSourcesIn(asyncDeferredProgressQueues[node])

CausalProgressCommitHistoryInvariant ==
  \A node \in ValidatorIds:
    ProgressCommitSourcesIn(asyncCausalQueues[node])

THEOREM DeliveryCandidateHasProgressCommitSource ==
  \A item: ProgressCommitSource(DeliveryCandidate(item))
PROOF
  <1>1. ASSUME NEW item
         PROVE ProgressCommitSource(DeliveryCandidate(item))
    <2>1. CASE /\ DeliveryCandidate(item).kind = "DeliverVote"
                 /\ item.kind = "CommitVote"
      <3>1. CASE HistoricalLockedCommitItem(item)
        BY <2>1, <3>1
           DEF ProgressCommitSource, ProgressCommitVoteHistory,
               DeliveryCandidate, AsyncCandidate
      <3>2. CASE ~HistoricalLockedCommitItem(item)
        <4>1. DeliveryClass(item) = "Normal"
          BY <2>1, <3>2, SMT
             DEF DeliveryClass, CurrentLockedReproposalPrepareItem
        <4>2. DeliveryCandidate(item).class # "Progress"
          BY <4>1 DEF DeliveryCandidate, AsyncCandidate
        <4> QED BY <4>2 DEF ProgressCommitSource
      <3> QED BY <3>1, <3>2
    <2>2. CASE ~(DeliveryCandidate(item).kind = "DeliverVote"
                  /\ item.kind = "CommitVote")
      BY <2>2
         DEF ProgressCommitSource, ProgressCommitVoteHistory,
             DeliveryCandidate, AsyncCandidate
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM CommitCertificateResponseCandidateHasProgressCommitSource ==
  \A item:
    ProgressCommitSource(CommitCertificateResponseCandidate(item))
BY DEF ProgressCommitSource, ProgressCommitVoteHistory,
       CommitCertificateResponseCandidate, DeliveryKind,
       DiscoveredCommitQcItem, AsyncNetworkItem,
       AsyncCandidateAtConsumer, AsyncCandidateWithIdentity

THEOREM CausalCandidateHasProgressCommitSource ==
  \A commandClass, kind, command:
    ProgressCommitSource(CausalCandidate(commandClass, kind, command))
BY SMT
   DEF ProgressCommitSource, ProgressCommitVoteHistory,
       CausalCandidate, NoItemCandidate, AsyncCandidate, NoAsyncItem

THEOREM CompletionCandidateHasProgressCommitSource ==
  \A candidate:
    candidate.class = "Completion" => ProgressCommitSource(candidate)
BY DEF ProgressCommitSource

THEOREM AsyncInitEstablishesProgressCommitHistories ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => /\ QueuedProgressCommitHistoryInvariant
         /\ DeferredProgressCommitHistoryInvariant
         /\ CausalProgressCommitHistoryInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE /\ QueuedProgressCommitHistoryInvariant
               /\ DeferredProgressCommitHistoryInvariant
               /\ CausalProgressCommitHistoryInvariant
    <2>1. /\ asyncCommandQueues =
                  [node \in ValidatorIds |-> <<>>]
           /\ asyncDeferredProgressQueues =
                  [node \in ValidatorIds |-> <<>>]
      BY <1>1 DEF AsyncInitAt, AsyncBaseInitAt,
                    AsyncRuntimeInit, AsyncDeferredInit
    <2>2. QueuedProgressCommitHistoryInvariant
      BY <2>1
         DEF QueuedProgressCommitHistoryInvariant,
             ProgressCommitSourcesIn, SequenceSet
    <2>3. DeferredProgressCommitHistoryInvariant
      BY <2>1
         DEF DeferredProgressCommitHistoryInvariant,
             ProgressCommitSourcesIn, SequenceSet
    <2>4. CausalProgressCommitHistoryInvariant
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE ProgressCommitSourcesIn(asyncCausalQueues[node])
        <4> DEFINE Initial ==
               NoItemCandidate("Normal", "AssembleBody", node,
                 nodeView[node], AsyncProposalSubject(node))
        <4>1. asyncCausalQueues[node] = <<Initial>>
          BY <1>1, <3>1, Isa
             DEF AsyncInitAt, AsyncBaseInitAt,
                 AsyncRuntimeInit, Initial
        <4>2. /\ Initial.class = "Normal"
               /\ ProgressCommitSource(Initial)
          BY DEF Initial, NoItemCandidate, AsyncCandidate,
                 ProgressCommitSource
        <4>3. SequenceSet(<<Initial>>) = {Initial}
          BY SingletonSequenceFacts DEF SequenceSet
        <4> QED BY <4>1, <4>2, <4>3
             DEF ProgressCommitSourcesIn
      <3> QED BY <3>1 DEF CausalProgressCommitHistoryInvariant
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM EmptySequenceHasProgressCommitSources ==
  ProgressCommitSourcesIn(<<>>)
BY Isa DEF ProgressCommitSourcesIn, SequenceSet

THEOREM SingletonSequenceHasProgressCommitSources ==
  \A command:
    ProgressCommitSource(command)
      => ProgressCommitSourcesIn(<<command>>)
PROOF
  <1>1. ASSUME NEW command,
                ProgressCommitSource(command)
         PROVE ProgressCommitSourcesIn(<<command>>)
    <2>1. SequenceSet(<<command>>) = {command}
      BY SingletonSequenceFacts, RangeEquality
         DEF SequenceSet
    <2> QED BY <1>1, <2>1 DEF ProgressCommitSourcesIn
  <1> QED BY <1>1

THEOREM PairSequenceHasProgressCommitSources ==
  \A left, right:
    /\ ProgressCommitSource(left)
    /\ ProgressCommitSource(right)
    => ProgressCommitSourcesIn(<<left, right>>)
PROOF
  <1>1. ASSUME NEW left, NEW right,
                ProgressCommitSource(left),
                ProgressCommitSource(right)
         PROVE ProgressCommitSourcesIn(<<left, right>>)
    <2>1. SequenceSet(<<left, right>>) = {left, right}
      BY Isa DEF SequenceSet
    <2> QED BY <1>1, <2>1 DEF ProgressCommitSourcesIn
  <1> QED BY <1>1

THEOREM TripleSequenceHasProgressCommitSources ==
  \A first, second, third:
    /\ ProgressCommitSource(first)
    /\ ProgressCommitSource(second)
    /\ ProgressCommitSource(third)
    => ProgressCommitSourcesIn(<<first, second, third>>)
PROOF
  <1>1. ASSUME NEW first, NEW second, NEW third,
                ProgressCommitSource(first),
                ProgressCommitSource(second),
                ProgressCommitSource(third)
         PROVE ProgressCommitSourcesIn(<<first, second, third>>)
    <2>1. SequenceSet(<<first, second, third>>) =
             {first, second, third}
      BY Isa DEF SequenceSet
    <2> QED BY <1>1, <2>1 DEF ProgressCommitSourcesIn
  <1> QED BY <1>1

THEOREM CommandSuccessorsHaveProgressCommitSources ==
  \A command:
    ProgressCommitSourcesIn(CommandSuccessors(command))
PROOF
  <1>1. ASSUME NEW command
         PROVE ProgressCommitSourcesIn(CommandSuccessors(command))
    <2>1. CASE command.kind \in
                 {"AssembleBody", "BeginProposal", "PersistProposal",
                  "DeliverProposal", "DeliverChunk", "FetchBody",
                  "RebindRetainedBody"}
      <3>1. CASE command.kind = "AssembleBody"
        BY <3>1, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>2. CASE command.kind = "BeginProposal"
        BY <3>2, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>3. CASE command.kind = "PersistProposal"
        BY <3>3, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors

      <3>4. CASE command.kind = "DeliverProposal"
        BY <3>4, CausalCandidateHasProgressCommitSource,
           PairSequenceHasProgressCommitSources
           DEF CommandSuccessors, RetainedBodyRebindCandidate
      <3>5. CASE command.kind = "DeliverChunk"
        BY <3>5, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>6. CASE command.kind = "FetchBody"
        BY <3>6, CausalCandidateHasProgressCommitSource,
           EmptySequenceHasProgressCommitSources,
           SingletonSequenceHasProgressCommitSources, Isa
           DEF CommandSuccessors
      <3>7. CASE command.kind = "RebindRetainedBody"
        BY <3>7, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
                     <3>7, SMT
    <2>2. CASE command.kind \in
                 {"FetchCertifiedBody", "StoreBody", "ValidateBody",
                  "BeginPrepare", "PersistPrepare", "DeliverVote",
                  "DeliverQC"}
      <3>1. CASE command.kind = "FetchCertifiedBody"
        BY <3>1, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>2. CASE command.kind = "StoreBody"
        BY <3>2, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>3. CASE command.kind = "ValidateBody"
        BY <3>3, CausalCandidateHasProgressCommitSource,
           TripleSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>4. CASE command.kind = "BeginPrepare"
        BY <3>4, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>5. CASE command.kind = "PersistPrepare"
        BY <3>5, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>6. CASE command.kind = "DeliverVote"
        BY <3>6, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>7. CASE command.kind = "DeliverQC"
        <4>1. CASE command.item.envelope.qc.phase = "Prepare"
          BY <3>7, <4>1, CausalCandidateHasProgressCommitSource,
             PairSequenceHasProgressCommitSources
             DEF CommandSuccessors
        <4>2. CASE command.item.envelope.qc.phase # "Prepare"
          BY <3>7, <4>2, CausalCandidateHasProgressCommitSource,
             SingletonSequenceHasProgressCommitSources
             DEF CommandSuccessors
        <4> QED BY <4>1, <4>2
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
                     <3>7, SMT
    <2>3. CASE command.kind \in
                 {"BeginObservePrepare", "PersistObservePrepare",
                  "BeginLockCommit", "PersistLockCommit", "FormCommitQC",
                  "BeginDecision", "PersistDecision"}
      <3>1. CASE command.kind = "BeginObservePrepare"
        BY <3>1, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>2. CASE command.kind = "PersistObservePrepare"
        BY <3>2, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>3. CASE command.kind = "BeginLockCommit"
        BY <3>3, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>4. CASE command.kind = "PersistLockCommit"
        BY <3>4, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>5. CASE command.kind = "FormCommitQC"
        BY <3>5, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>6. CASE command.kind = "BeginDecision"
        BY <3>6, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>7. CASE command.kind = "PersistDecision"
        BY <3>7, CompletionCandidateHasProgressCommitSource,
           EmptySequenceHasProgressCommitSources,
           SingletonSequenceHasProgressCommitSources, Isa
           DEF CommandSuccessors, PersistDecisionRecoverySuccessor,
               PersistDecisionRecoveryKind, PersistDecisionBody,
               PersistDecisionValidationHeld, PersistDecisionRequest,
               AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
               AsyncCandidateSuccessorProposalRound,
               AsyncCandidateWithIdentityAndOrigin
      <3> QED BY <2>3, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
                     <3>7, SMT
    <2>4. CASE command.kind \in
                 {"BeginTimeout", "PersistTimeout", "DeliverTimeout",
                  "DeliverTC", "BeginInstallTC", "PersistInstallTC"}
      <3>1. CASE command.kind = "BeginTimeout"
        BY <3>1, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>2. CASE command.kind = "PersistTimeout"
        BY <3>2, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>3. CASE command.kind = "DeliverTimeout"
        BY <3>3, CausalCandidateHasProgressCommitSource,
           EmptySequenceHasProgressCommitSources,
           SingletonSequenceHasProgressCommitSources, Isa
           DEF CommandSuccessors
      <3>4. CASE command.kind = "DeliverTC"
        BY <3>4, CausalCandidateHasProgressCommitSource,
           EmptySequenceHasProgressCommitSources,
           SingletonSequenceHasProgressCommitSources, Isa
           DEF CommandSuccessors
      <3>5. CASE command.kind = "BeginInstallTC"
        BY <3>5, CausalCandidateHasProgressCommitSource,
           SingletonSequenceHasProgressCommitSources
           DEF CommandSuccessors
      <3>6. CASE command.kind = "PersistInstallTC"
        <4>1. ProgressCommitSource(InstallProposalSuccessor(command))
          BY DEF InstallProposalSuccessor, ProgressCommitSource,
                 AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
                 AsyncCandidateWithIdentityAndOrigin,
                 NoItemCandidate, AsyncCandidate
        <4>2. ProgressCommitSource(InstallCommitSignSuccessor(command))
          BY CompletionCandidateHasProgressCommitSource
             DEF InstallCommitSignSuccessor,
                 AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
                 AsyncCandidateWithIdentityAndOrigin,
                 NoItemCandidate,
                 AsyncCandidate
        <4>2a. ProgressCommitSource(InstallLockedFetchSuccessor(command))
          BY CompletionCandidateHasProgressCommitSource
             DEF InstallLockedFetchSuccessor,
                 AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
                 AsyncCandidateWithIdentityAndOrigin,
                 NoItemCandidate,
                 AsyncCandidate
        <4>3. CASE /\ InstallResultingLockedPrepareQCs(command) = {}
                     /\ InstallCommitSignRequests(command) = {}
          BY <3>6, <4>1, <4>3,
             SingletonSequenceHasProgressCommitSources
             DEF CommandSuccessors, InstallCommandSuccessors,
                 InstallLockedFetchSuccessors,
                 InstallCommitSignSuccessors
        <4>4. CASE /\ InstallResultingLockedPrepareQCs(command) = {}
                     /\ InstallCommitSignRequests(command) # {}
          BY <3>6, <4>1, <4>2, <4>4,
             PairSequenceHasProgressCommitSources
             DEF CommandSuccessors, InstallCommandSuccessors,
                 InstallLockedFetchSuccessors,
                 InstallCommitSignSuccessors
        <4>5. CASE /\ InstallResultingLockedPrepareQCs(command) # {}
                     /\ InstallCommitSignRequests(command) = {}
          BY <3>6, <4>1, <4>2a, <4>5,
             PairSequenceHasProgressCommitSources
             DEF CommandSuccessors, InstallCommandSuccessors,
                 InstallLockedFetchSuccessors,
                 InstallCommitSignSuccessors
        <4>6. CASE /\ InstallResultingLockedPrepareQCs(command) # {}
                     /\ InstallCommitSignRequests(command) # {}
          BY <3>6, <4>1, <4>2, <4>2a, <4>6,
             TripleSequenceHasProgressCommitSources
             DEF CommandSuccessors, InstallCommandSuccessors,
                 InstallLockedFetchSuccessors,
                 InstallCommitSignSuccessors
        <4> QED BY <4>3, <4>4, <4>5, <4>6
      <3> QED BY <2>4, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
                     SMT
    <2>5. CASE command.kind \notin
                 {"AssembleBody", "BeginProposal", "PersistProposal",
                  "DeliverProposal", "DeliverChunk", "FetchBody",
                  "RebindRetainedBody", "FetchCertifiedBody", "StoreBody",
                  "ValidateBody", "BeginPrepare", "PersistPrepare",
                  "DeliverVote", "DeliverQC", "BeginObservePrepare",
                  "PersistObservePrepare", "BeginLockCommit",
                  "PersistLockCommit", "FormCommitQC", "BeginDecision",
                  "PersistDecision", "BeginTimeout", "PersistTimeout",
                  "DeliverTimeout", "DeliverTC",
                  "BeginInstallTC", "PersistInstallTC"}
      BY <2>5, EmptySequenceHasProgressCommitSources
         DEF CommandSuccessors
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, Isa
  <1> QED BY <1>1

THEOREM FreshCommandSuccessorsHaveProgressCommitSources ==
  \A command:
    ProgressCommitSourcesIn(FreshCommandSuccessors(command))
BY CommandSuccessorsHaveProgressCommitSources, Isa
   DEF FreshCommandSuccessors, FreshCandidateSequence,
       CommandSuccessors, InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallCommitSignSuccessors,
       ProgressCommitSourcesIn, SequenceSet

(***************************************************************************
The executable reducer never carries a durable Prepare lock ahead of the
node's installed view.  This private strengthening supplies the ordering
needed to retain an exact historical source when a TC advances nodeView but
does not advance the lock.
***************************************************************************)
LockWithinNodeViewInvariant ==
  \A node \in ValidatorIds: lockRank[node] <= nodeView[node]

ProgressFrontierAction ==
  /\ context' = context
  /\ prepareQCs \subseteq prepareQCs'
  /\ LockMonotonicityAction
  /\ \A node \in ValidatorIds:
       nodeView'[node] >= nodeView[node]

THEOREM ProgressCommitVoteHistoryForwardStable ==
  \A command:
    /\ AsyncCandidateTyped(command)
    /\ TypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ ProgressCommitVoteHistory(command)
    /\ ProgressFrontierAction
    => ProgressCommitVoteHistory(command)'
PROOF
  <1>1. ASSUME NEW command,
                AsyncCandidateTyped(command),
                TypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                ProgressCommitVoteHistory(command),
                ProgressFrontierAction
         PROVE ProgressCommitVoteHistory(command)'
    <2>1. CASE command.kind = "DeliverVote"
                 /\ command.item.kind = "CommitVote"
      <3>1. command.item # NoAsyncItem
        BY <2>1 DEF NoAsyncItem, AsyncBodyEnvelope
      <3>2a. command.item = NoAsyncItem
                 \/ AsyncItemTyped(command.item)
        BY <1>1 DEF AsyncCandidateTyped
      <3>2b. AsyncItemTyped(command.item)
        BY <3>1, <3>2a
      <3>2c. command.item.envelope \in VoteEnvelopeSet
        BY <2>1, <3>2b DEF AsyncItemTyped
      <3>2. /\ AsyncItemTyped(command.item)
             /\ command.item.envelope.recipient \in ValidatorIds
             /\ command.item.envelope.vote.view \in Views
        BY <3>2b, <3>2c DEF AsyncItemTyped, VoteEnvelopeSet,
                             VoteRecordSet
      <3>3a. /\ Ranks \subseteq Int
              /\ Views \subseteq Ranks
        BY <1>1, ModelRanksAreIntegers, ViewsAreRanks
           DEF TypeInvariant
      <3>3b. command.item.envelope.vote.view \in Int
        BY <3>2, <3>3a
      <3>3c. lockRank[command.item.envelope.recipient] \in Int
        BY <1>1, <3>2, <3>3a, FunctionValueHasCodomain
           DEF TypeInvariant
      <3>3d. nodeView[command.item.envelope.recipient] \in Int
        BY <1>1, <3>2, <3>3a, FunctionValueHasCodomain
           DEF TypeInvariant
      <3>3e. /\ lockRank'[command.item.envelope.recipient] \in Int
              /\ nodeView'[command.item.envelope.recipient] \in Int
        BY <1>1, <3>2, <3>3a, FunctionValueHasCodomain
           DEF TypeInvariant
      <3>3. /\ command.item.envelope.vote.view \in Int
             /\ lockRank[command.item.envelope.recipient] \in Int
             /\ nodeView[command.item.envelope.recipient] \in Int
             /\ lockRank'[command.item.envelope.recipient] \in Int
             /\ nodeView'[command.item.envelope.recipient] \in Int
        BY <3>3b, <3>3c, <3>3d, <3>3e
      <3>4. \/ HistoricalLockedCommitItem(command.item)
             \/ command.item.envelope.vote.view <
                  lockRank[command.item.envelope.recipient]
        BY <1>1, <2>1 DEF ProgressCommitVoteHistory
      <3>5. CASE command.item.envelope.vote.view <
                     lockRank[command.item.envelope.recipient]
        <4>1. lockRank'[command.item.envelope.recipient] >=
                 lockRank[command.item.envelope.recipient]
          BY <1>1, <3>2 DEF ProgressFrontierAction,
                              LockMonotonicityAction
        <4>2. command.item.envelope.vote.view <
                 lockRank'[command.item.envelope.recipient]
          BY <3>3, <3>5, <4>1, StrictWeakOrderTrans
        <4> QED BY <2>1, <4>2 DEF ProgressCommitVoteHistory
      <3>6. CASE HistoricalLockedCommitItem(command.item)
        <4>1a. /\ nodeView[command.item.envelope.recipient] #
                       command.item.envelope.vote.view
                /\ LockedPrepareRound(
                     command.item.envelope.recipient,
                     command.item.envelope.vote.view,
                     command.item.envelope.vote.subject)
          BY <2>1, <3>6 DEF HistoricalLockedCommitItem
        <4>1b. lockRank[command.item.envelope.recipient] =
                      command.item.envelope.vote.view
          BY <4>1a DEF LockedPrepareRound
        <4>1. /\ lockRank[command.item.envelope.recipient] =
                      command.item.envelope.vote.view
               /\ nodeView[command.item.envelope.recipient] #
                      command.item.envelope.vote.view
               /\ LockedPrepareRound(
                    command.item.envelope.recipient,
                    command.item.envelope.vote.view,

                    command.item.envelope.vote.subject)
          BY <4>1a, <4>1b
        <4>2. command.item.envelope.vote.view <
                 nodeView[command.item.envelope.recipient]
          BY <1>1, <3>2, <3>3, <4>1, SMT
             DEF LockWithinNodeViewInvariant
        <4>3. /\ lockRank'[command.item.envelope.recipient] >=
                      lockRank[command.item.envelope.recipient]
               /\ nodeView'[command.item.envelope.recipient] >=
                      nodeView[command.item.envelope.recipient]
          BY <1>1, <3>2 DEF ProgressFrontierAction,
                              LockMonotonicityAction
        <4>4. nodeView'[command.item.envelope.recipient] #
                 command.item.envelope.vote.view
          BY <3>3, <4>2, <4>3, SMT
        <4>5. CASE lockRank'[command.item.envelope.recipient] >
                       lockRank[command.item.envelope.recipient]
          BY <2>1, <3>3, <4>1, <4>5, SMT
             DEF ProgressCommitVoteHistory
        <4>6. CASE lockRank'[command.item.envelope.recipient] =
                       lockRank[command.item.envelope.recipient]
          <5>1. lockSubject'[command.item.envelope.recipient] =
                   lockSubject[command.item.envelope.recipient]
            BY <1>1, <3>2, <4>6, SMT
               DEF ProgressFrontierAction, LockMonotonicityAction
          <5>2. LockedPrepareRound(
                   command.item.envelope.recipient,
                   command.item.envelope.vote.view,
                   command.item.envelope.vote.subject)'
            BY <1>1, <3>2, <4>1, <4>6, <5>1, Isa
               DEF ProgressFrontierAction, LockedPrepareRound
          <5>3. HistoricalLockedCommitItem(command.item)'
            BY <2>1, <4>4, <5>2 DEF HistoricalLockedCommitItem
          <5> QED BY <2>1, <5>3 DEF ProgressCommitVoteHistory
        <4> QED BY <3>3, <4>3, <4>5, <4>6, SMT
      <3> QED BY <3>4, <3>5, <3>6
    <2>2. CASE ~(command.kind = "DeliverVote"
                  /\ command.item.kind = "CommitVote")
      BY <2>2 DEF ProgressCommitVoteHistory
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ProgressCommitSourceForwardStable ==
  \A command:
    /\ AsyncCandidateTyped(command)
    /\ TypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ ProgressCommitSource(command)
    /\ ProgressFrontierAction
    => ProgressCommitSource(command)'
PROOF
  <1>1. ASSUME NEW command,
                AsyncCandidateTyped(command),
                TypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                ProgressCommitSource(command),
                ProgressFrontierAction
         PROVE ProgressCommitSource(command)'
    <2>1. CASE command.class # "Progress"
      BY <2>1 DEF ProgressCommitSource
    <2>2. CASE command.class = "Progress"
      <3>1. ProgressCommitVoteHistory(command)
        BY <1>1, <2>2 DEF ProgressCommitSource
      <3>2. ProgressCommitVoteHistory(command)'
        BY <1>1, <3>1, ProgressCommitVoteHistoryForwardStable
      <3> QED BY <3>2 DEF ProgressCommitSource
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM AppendProgressCommitSourcePreservesSources ==
  \A queue, command:
    /\ AsyncQueueTyped(queue)
    /\ ProgressCommitSourcesIn(queue)
    /\ ProgressCommitSource(command)
    => ProgressCommitSourcesIn(Append(queue, command))
PROOF
  <1>1. ASSUME NEW queue, NEW command,
                AsyncQueueTyped(queue),
                ProgressCommitSourcesIn(queue),
                ProgressCommitSource(command)
         PROVE ProgressCommitSourcesIn(Append(queue, command))
    <2>1. SequenceSet(Append(queue, command)) =
             SequenceSet(queue) \cup {command}
      BY <1>1, SequenceSetAfterAppend DEF AsyncQueueTyped
    <2> QED BY <1>1, <2>1 DEF ProgressCommitSourcesIn
  <1> QED BY <1>1

THEOREM RemoveProgressCommitSourcePreservesSources ==
  \A queue, index:
    /\ AsyncQueueTyped(queue)
    /\ index \in 1..Len(queue)
    /\ ProgressCommitSourcesIn(queue)
    => ProgressCommitSourcesIn(SequenceWithoutIndex(queue, index))
PROOF
  <1>1. ASSUME NEW queue, NEW index,
                AsyncQueueTyped(queue),
                index \in 1..Len(queue),
                ProgressCommitSourcesIn(queue)
         PROVE ProgressCommitSourcesIn(
                 SequenceWithoutIndex(queue, index))
    <2> DEFINE Result == SequenceWithoutIndex(queue, index)
    <2>1. /\ queue \in Seq(Range(queue))
           /\ Range(Result) \subseteq Range(queue)
           /\ Result \in Seq(Range(queue))
      BY <1>1, SequenceWithoutIndexFacts
         DEF AsyncQueueTyped, Result
    <2>2. /\ SequenceSet(queue) = Range(queue)
           /\ SequenceSet(Result) = Range(Result)
      BY <2>1, RangeEquality DEF SequenceSet
    <2>3. SequenceSet(Result) \subseteq SequenceSet(queue)
      BY <2>1, <2>2
    <2> QED BY <1>1, <2>3 DEF ProgressCommitSourcesIn, Result
  <1> QED BY <1>1

THEOREM TailProgressCommitSourcePreservesSources ==
  \A queue:
    /\ AsyncQueueTyped(queue)
    /\ Len(queue) > 0
    /\ ProgressCommitSourcesIn(queue)
    => ProgressCommitSourcesIn(Tail(queue))
PROOF
  <1>1. ASSUME NEW queue,
                AsyncQueueTyped(queue),
                Len(queue) > 0,
                ProgressCommitSourcesIn(queue)
         PROVE ProgressCommitSourcesIn(Tail(queue))
    <2>1. /\ Tail(queue) \in Seq(Range(queue))
           /\ Range(Tail(queue)) \subseteq Range(queue)
      BY <1>1, HeadTailProperties DEF AsyncQueueTyped
    <2>2. /\ SequenceSet(queue) = Range(queue)
           /\ SequenceSet(Tail(queue)) = Range(Tail(queue))
      BY <1>1, <2>1, RangeEquality DEF AsyncQueueTyped, SequenceSet
    <2>3. SequenceSet(Tail(queue)) \subseteq SequenceSet(queue)
      BY <2>1, <2>2
    <2> QED BY <1>1, <2>3 DEF ProgressCommitSourcesIn
  <1> QED BY <1>1

THEOREM DeferredProgressAfterPreservesProgressCommitSources ==
  \A node \in ValidatorIds:
    \A command:
      /\ AsyncConfiguration
      /\ AsyncDeferredContentTypeInvariant
      /\ ProgressCommitSourcesIn(
           asyncDeferredProgressQueues[node])
      /\ ProgressCommitSource(command)
      => ProgressCommitSourcesIn(
           DeferredProgressAfter(node, command))
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW command,
                AsyncConfiguration,
                AsyncDeferredContentTypeInvariant,
                ProgressCommitSourcesIn(
                  asyncDeferredProgressQueues[node]),
                ProgressCommitSource(command)
         PROVE ProgressCommitSourcesIn(
                 DeferredProgressAfter(node, command))
    <2> DEFINE Queue == asyncDeferredProgressQueues[node]
    <2>1. /\ AsyncQueueTyped(Queue)
           /\ ProgressCommitSourcesIn(Queue)
      BY <1>1 DEF AsyncDeferredContentTypeInvariant, Queue
    <2>2. CASE command \in SequenceSet(Queue)
      BY <2>1, <2>2 DEF DeferredProgressAfter, Queue
    <2>3. CASE /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) # {}
      BY <2>1, <2>3 DEF DeferredProgressAfter, Queue
    <2>4. CASE /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) = {}
                 /\ Len(Queue) < AsyncDeferredProgressCapacity
      <3>1. DeferredProgressAfter(node, command) =
               Append(Queue, command)
        BY <2>4 DEF DeferredProgressAfter, Queue
      <3> QED BY <1>1, <2>1, <3>1,
           AppendProgressCommitSourcePreservesSources
    <2>5. CASE /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) = {}
                 /\ Len(Queue) >= AsyncDeferredProgressCapacity
      <3>1. /\ Len(Queue) \in Nat
             /\ AsyncDeferredProgressCapacity \in Nat
        BY <1>1, <2>1, LenProperties
           DEF AsyncQueueTyped, AsyncConfiguration
      <3>2. ~Len(Queue) < AsyncDeferredProgressCapacity
        BY <2>5, <3>1, SMT
      <3>3. DeferredProgressAfter(node, command) = Queue
        BY <2>5, <3>2 DEF DeferredProgressAfter, Queue
      <3> QED BY <2>1, <3>3
    <2>6. \/ command \in SequenceSet(Queue)
           \/ /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) # {}
           \/ /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) = {}
                 /\ Len(Queue) < AsyncDeferredProgressCapacity
           \/ /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) = {}
                 /\ Len(Queue) >= AsyncDeferredProgressCapacity
      BY <1>1, <2>1, SMT
         DEF AsyncQueueTyped, AsyncConfiguration
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM QueueSequenceSetElementIsTyped ==
  \A queue, command:
    /\ AsyncQueueTyped(queue)
    /\ command \in SequenceSet(queue)
    => AsyncCandidateTyped(command)
BY SMT DEF AsyncQueueTyped, SequenceSet

THEOREM RestrictedQueuePreservesProgressCommitSourcesUnderFrontier ==
  \A queue, restricted:
    /\ AsyncQueueTyped(queue)
    /\ SequenceSet(restricted) \subseteq SequenceSet(queue)
    /\ ProgressCommitSourcesIn(queue)
    /\ TypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ ProgressFrontierAction
    => ProgressCommitSourcesIn(restricted)'
PROOF
  <1>1. ASSUME NEW queue, NEW restricted,
                AsyncQueueTyped(queue),
                SequenceSet(restricted) \subseteq SequenceSet(queue),
                ProgressCommitSourcesIn(queue),
                TypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                ProgressFrontierAction
         PROVE ProgressCommitSourcesIn(restricted)'
    <2>1. ASSUME NEW command \in SequenceSet(restricted)
           PROVE ProgressCommitSource(command)'
      <3>1. command \in SequenceSet(queue)
        BY <1>1, <2>1
      <3>2a. AsyncCandidateTyped(command)
        BY <1>1, <3>1, QueueSequenceSetElementIsTyped
      <3>2b. ProgressCommitSource(command)
        BY <1>1, <3>1 DEF ProgressCommitSourcesIn
      <3>2. /\ AsyncCandidateTyped(command)
             /\ ProgressCommitSource(command)
        BY <3>2a, <3>2b
      <3> QED BY <1>1, <3>2,
           ProgressCommitSourceForwardStable
    <2> QED BY <2>1 DEF ProgressCommitSourcesIn
  <1> QED BY <1>1

THEOREM RigidQueuePreservesProgressCommitSourcesUnderFrontier ==
  \A queue:
    /\ AsyncQueueTyped(queue)
    /\ ProgressCommitSourcesIn(queue)
    /\ TypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ ProgressFrontierAction
    => ProgressCommitSourcesIn(queue)'
BY RestrictedQueuePreservesProgressCommitSourcesUnderFrontier

THEOREM AppendedQueuePreservesProgressCommitSourcesUnderFrontier ==
  \A queue, command:
    /\ AsyncQueueTyped(queue)
    /\ AsyncCandidateTyped(command)
    /\ ProgressCommitSourcesIn(queue)
    /\ ProgressCommitSource(command)
    /\ TypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ ProgressFrontierAction
    => ProgressCommitSourcesIn(Append(queue, command))'
PROOF
  <1>1. ASSUME NEW queue, NEW command,
                AsyncQueueTyped(queue),
                AsyncCandidateTyped(command),
                ProgressCommitSourcesIn(queue),
                ProgressCommitSource(command),
                TypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                ProgressFrontierAction
         PROVE ProgressCommitSourcesIn(Append(queue, command))'
    <2>1. /\ AsyncQueueTyped(Append(queue, command))
           /\ ProgressCommitSourcesIn(Append(queue, command))
      BY <1>1, TypedCandidateAppendPreservesQueueType,
         AppendProgressCommitSourcePreservesSources
    <2> QED BY <1>1, <2>1,
         RigidQueuePreservesProgressCommitSourcesUnderFrontier
  <1> QED BY <1>1

THEOREM RemovedQueuePreservesProgressCommitSourcesUnderFrontier ==
  \A queue, index:
    /\ AsyncQueueTyped(queue)
    /\ index \in 1..Len(queue)
    /\ ProgressCommitSourcesIn(queue)
    /\ TypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ ProgressFrontierAction
    => ProgressCommitSourcesIn(
         SequenceWithoutIndex(queue, index))'
PROOF
  <1>1. ASSUME NEW queue, NEW index,
                AsyncQueueTyped(queue),
                index \in 1..Len(queue),
                ProgressCommitSourcesIn(queue),
                TypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                ProgressFrontierAction
         PROVE ProgressCommitSourcesIn(
                 SequenceWithoutIndex(queue, index))'
    <2>1. SequenceSet(SequenceWithoutIndex(queue, index))
             \subseteq SequenceSet(queue)
      <3>1. /\ Range(SequenceWithoutIndex(queue, index))
                     \subseteq Range(queue)
             /\ SequenceWithoutIndex(queue, index)
                    \in Seq(Range(queue))
        BY <1>1, SequenceWithoutIndexFacts DEF AsyncQueueTyped
      <3> QED BY <1>1, <3>1, RangeEquality
           DEF AsyncQueueTyped, SequenceSet
    <2> QED BY <1>1, <2>1,
         RestrictedQueuePreservesProgressCommitSourcesUnderFrontier
  <1> QED BY <1>1

THEOREM UnchangedCommandQueuesPreserveProgressCommitHistory ==
  /\ AsyncRuntimeScalarTypeInvariant
  /\ QueuedProgressCommitHistoryInvariant
  /\ TypeInvariant
  /\ TypeInvariant'
  /\ LockWithinNodeViewInvariant
  /\ ProgressFrontierAction
  /\ UNCHANGED asyncCommandQueues
  => QueuedProgressCommitHistoryInvariant'
BY RigidQueuePreservesProgressCommitSourcesUnderFrontier, Isa
   DEF AsyncRuntimeScalarTypeInvariant,
       QueuedProgressCommitHistoryInvariant

THEOREM UnchangedDeferredProgressQueuesPreserveCommitHistory ==
  /\ AsyncDeferredContentTypeInvariant
  /\ DeferredProgressCommitHistoryInvariant
  /\ TypeInvariant
  /\ TypeInvariant'
  /\ LockWithinNodeViewInvariant
  /\ ProgressFrontierAction
  /\ UNCHANGED asyncDeferredProgressQueues
  => DeferredProgressCommitHistoryInvariant'
BY RigidQueuePreservesProgressCommitSourcesUnderFrontier, Isa
   DEF AsyncDeferredContentTypeInvariant,
       DeferredProgressCommitHistoryInvariant

THEOREM EnqueueCandidatePreservesQueuedProgressCommitHistory ==
  \A candidate:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ TypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ ProgressFrontierAction
    /\ AsyncCandidateTyped(candidate)
    /\ ProgressCommitSource(candidate)
    /\ EnqueueCandidate(candidate)
    => QueuedProgressCommitHistoryInvariant'
BY AppendedQueuePreservesProgressCommitSourcesUnderFrontier,
   RigidQueuePreservesProgressCommitSourcesUnderFrontier,
   SMT
   DEF AsyncRuntimeScalarTypeInvariant,
       QueuedProgressCommitHistoryInvariant,
       AsyncCandidateTyped, AsyncCandidateSet, EnqueueCandidate

THEOREM RemoveNextNodeCommandPreservesQueuedProgressCommitHistory ==
  \A target \in ValidatorIds:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ TypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ ProgressFrontierAction
    /\ NodeQueueNonempty(target)
    /\ RemoveNextNodeCommand(target)
    => QueuedProgressCommitHistoryInvariant'
BY NextNodeCommandIndexFacts,
   RemovedQueuePreservesProgressCommitSourcesUnderFrontier,
   RigidQueuePreservesProgressCommitSourcesUnderFrontier,
   SMT
   DEF AsyncRuntimeScalarTypeInvariant,
       QueuedProgressCommitHistoryInvariant,
       RemoveNextNodeCommand

THEOREM TypedQueueConcatIsTyped ==
  \A left, right:
    /\ AsyncQueueTyped(left)
    /\ AsyncQueueTyped(right)
    => AsyncQueueTyped(left \o right)
BY ConcatProperties, RangeConcatenation, SeqOfRange,
   LenProperties, RangeEquality, Isa
   DEF AsyncQueueTyped

THEOREM TypedQueueConcatSequenceSet ==
  \A left, right:
    /\ AsyncQueueTyped(left)
    /\ AsyncQueueTyped(right)
    => SequenceSet(left \o right) =
         SequenceSet(left) \cup SequenceSet(right)
BY RangeConcatenation, RangeEquality, Isa
   DEF AsyncQueueTyped, SequenceSet

THEOREM TypedQueueConcatPreservesProgressCommitSources ==
  \A left, right:
    /\ AsyncQueueTyped(left)
    /\ AsyncQueueTyped(right)
    /\ ProgressCommitSourcesIn(left)
    /\ ProgressCommitSourcesIn(right)

    => /\ AsyncQueueTyped(left \o right)
       /\ ProgressCommitSourcesIn(left \o right)
BY TypedQueueConcatIsTyped, TypedQueueConcatSequenceSet, SMT
   DEF ProgressCommitSourcesIn

THEOREM UnchangedCausalQueuesPreserveProgressCommitHistory ==
  /\ AsyncCausalTypeInvariant
  /\ CausalProgressCommitHistoryInvariant
  /\ TypeInvariant
  /\ TypeInvariant'
  /\ LockWithinNodeViewInvariant
  /\ ProgressFrontierAction
  /\ UNCHANGED asyncCausalQueues
  => CausalProgressCommitHistoryInvariant'
BY RigidQueuePreservesProgressCommitSourcesUnderFrontier, SMT
   DEF AsyncCausalTypeInvariant,
       CausalProgressCommitHistoryInvariant

THEOREM ReplaceCausalQueuePreservesProgressCommitHistory ==
  \A target \in ValidatorIds:
    \A replacement:
      /\ AsyncCausalTypeInvariant
      /\ CausalProgressCommitHistoryInvariant
      /\ AsyncQueueTyped(replacement)
      /\ ProgressCommitSourcesIn(replacement)
      /\ TypeInvariant
      /\ TypeInvariant'
      /\ LockWithinNodeViewInvariant
      /\ ProgressFrontierAction
      /\ asyncCausalQueues' =
           [asyncCausalQueues EXCEPT ![target] = replacement]
      => CausalProgressCommitHistoryInvariant'
BY RigidQueuePreservesProgressCommitSourcesUnderFrontier, SMT
   DEF AsyncCausalTypeInvariant,
       CausalProgressCommitHistoryInvariant

THEOREM AppendTypedCausalSequencePreservesProgressCommitHistory ==
  \A target \in ValidatorIds:
    \A successors:
      /\ AsyncCausalTypeInvariant
      /\ CausalProgressCommitHistoryInvariant
      /\ AsyncQueueTyped(successors)
      /\ ProgressCommitSourcesIn(successors)
      /\ TypeInvariant
      /\ TypeInvariant'
      /\ LockWithinNodeViewInvariant
      /\ ProgressFrontierAction
      /\ asyncCausalQueues' =
           [asyncCausalQueues EXCEPT
              ![target] = asyncCausalQueues[target] \o successors]
      => CausalProgressCommitHistoryInvariant'
PROOF
  <1>1. ASSUME NEW target \in ValidatorIds,
                NEW successors,
                AsyncCausalTypeInvariant,
                CausalProgressCommitHistoryInvariant,
                AsyncQueueTyped(successors),
                ProgressCommitSourcesIn(successors),
                TypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                ProgressFrontierAction,
                asyncCausalQueues' =
                  [asyncCausalQueues EXCEPT
                     ![target] =
                       asyncCausalQueues[target] \o successors]
         PROVE CausalProgressCommitHistoryInvariant'
    <2>1. /\ AsyncQueueTyped(asyncCausalQueues[target])
           /\ ProgressCommitSourcesIn(asyncCausalQueues[target])
      BY <1>1
         DEF AsyncCausalTypeInvariant,
             CausalProgressCommitHistoryInvariant
    <2>2. /\ AsyncQueueTyped(
                    asyncCausalQueues[target] \o successors)
           /\ ProgressCommitSourcesIn(
                asyncCausalQueues[target] \o successors)
      BY <1>1, <2>1,
         TypedQueueConcatPreservesProgressCommitSources
    <2> QED BY <1>1, <2>2,
         ReplaceCausalQueuePreservesProgressCommitHistory
  <1> QED BY <1>1

THEOREM AppendCausalSuccessorsPreservesProgressCommitHistory ==
  \A command:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ ProgressFrontierAction
    /\ AsyncCandidateTyped(command)
    /\ ExecuteCommand(command)
    /\ AppendCausalSuccessors(command)
    => CausalProgressCommitHistoryInvariant'
PROOF
  <1>1. ASSUME NEW command,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                CausalProgressCommitHistoryInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                ProgressFrontierAction,
                AsyncCandidateTyped(command),
                ExecuteCommand(command),
                AppendCausalSuccessors(command)
         PROVE CausalProgressCommitHistoryInvariant'
    <2>1. /\ TypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ command.node \in ValidatorIds
           /\ AsyncQueueTyped(asyncCausalQueues[command.node])
           /\ ProgressCommitSourcesIn(asyncCausalQueues[command.node])
      BY <1>1
         DEF StrongInductiveInvariant, Safety, AsyncTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncCausalTypeInvariant,
             AsyncCandidateTyped, CausalProgressCommitHistoryInvariant
    <2>2. /\ AsyncQueueTyped(FreshCommandSuccessors(command))
           /\ ProgressCommitSourcesIn(FreshCommandSuccessors(command))
      BY <1>1, ExecutedFreshCommandSuccessorsTypedAndOwned,
         FreshCommandSuccessorsHaveProgressCommitSources
    <2>3. /\ AsyncQueueTyped(
                    asyncCausalQueues[command.node]
                      \o FreshCommandSuccessors(command))
           /\ ProgressCommitSourcesIn(
                asyncCausalQueues[command.node]
                  \o FreshCommandSuccessors(command))
      BY <2>1, <2>2,
         TypedQueueConcatPreservesProgressCommitSources
    <2>4. asyncCausalQueues' =
             [asyncCausalQueues EXCEPT
                ![command.node] =
                  asyncCausalQueues[command.node]
                    \o FreshCommandSuccessors(command)]
      BY <1>1 DEF AppendCausalSuccessors
    <2> QED BY <1>1, <2>1, <2>3, <2>4,
         ReplaceCausalQueuePreservesProgressCommitHistory
  <1> QED BY <1>1

THEOREM AppendTimeoutCausalSuccessorsPreservesProgressCommitHistory ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProgressFrontierAction
    /\ AppendCausalSuccessors(TimeoutCausalCommand(node))
    => CausalProgressCommitHistoryInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                CausalProgressCommitHistoryInvariant,
                ProgressFrontierAction,
                AppendCausalSuccessors(TimeoutCausalCommand(node))
         PROVE CausalProgressCommitHistoryInvariant'
    <2>1. /\ TypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncQueueTyped(
                FreshCommandSuccessors(TimeoutCausalCommand(node)))
           /\ ProgressCommitSourcesIn(
                FreshCommandSuccessors(TimeoutCausalCommand(node)))
      BY <1>1, FreshTimeoutCausalSuccessorsTypedAndOwned,
         FreshCommandSuccessorsHaveProgressCommitSources
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant
    <2>2. asyncCausalQueues' =
             [asyncCausalQueues EXCEPT
                ![node] = asyncCausalQueues[node]
                  \o FreshCommandSuccessors(TimeoutCausalCommand(node))]
      BY <1>1 DEF AppendCausalSuccessors
    <2> QED BY <1>1, <2>1, <2>2,
         AppendTypedCausalSequencePreservesProgressCommitHistory
  <1> QED BY <1>1

THEOREM CausalHeadHasProgressCommitSource ==
  \A node \in ValidatorIds:
    /\ AsyncCausalTypeInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ CausalQueueNonempty(node)
    => /\ AsyncCandidateTyped(HeadCausalCandidate(node))
       /\ ProgressCommitSource(HeadCausalCandidate(node))
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncCausalTypeInvariant,
                CausalProgressCommitHistoryInvariant,
                CausalQueueNonempty(node)
         PROVE /\ AsyncCandidateTyped(HeadCausalCandidate(node))
               /\ ProgressCommitSource(HeadCausalCandidate(node))
    <2>1. /\ AsyncQueueTyped(asyncCausalQueues[node])
           /\ Len(asyncCausalQueues[node]) > 0
           /\ Head(asyncCausalQueues[node]) =
                asyncCausalQueues[node][1]
           /\ 1 \in 1..Len(asyncCausalQueues[node])
      BY <1>1, NonemptySequenceHeadIsFirst, SMT
         DEF AsyncCausalTypeInvariant, CausalQueueNonempty,
             AsyncQueueTyped
    <2>2. HeadCausalCandidate(node) \in
             SequenceSet(asyncCausalQueues[node])
      BY <2>1 DEF HeadCausalCandidate, SequenceSet
    <2> QED BY <1>1, <2>1, <2>2
         DEF AsyncCausalTypeInvariant,
             CausalProgressCommitHistoryInvariant,
             ProgressCommitSourcesIn, AsyncQueueTyped
  <1> QED BY <1>1

THEOREM AdmitCausalHeadPreservesCausalProgressCommitHistory ==
  \A node \in ValidatorIds:
    /\ AsyncCausalTypeInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ TypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ ProgressFrontierAction
    /\ CausalHeadCanAdvance(node)
    /\ AdmitCausalHead(node)
    => CausalProgressCommitHistoryInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncCausalTypeInvariant,
                CausalProgressCommitHistoryInvariant,
                TypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                ProgressFrontierAction,
                CausalHeadCanAdvance(node),
                AdmitCausalHead(node)
         PROVE CausalProgressCommitHistoryInvariant'
    <2>1. /\ AsyncQueueTyped(asyncCausalQueues[node])
           /\ Len(asyncCausalQueues[node]) > 0
           /\ ProgressCommitSourcesIn(asyncCausalQueues[node])
      BY <1>1
         DEF AsyncCausalTypeInvariant, CausalHeadCanAdvance,
             CausalQueueNonempty,
             CausalProgressCommitHistoryInvariant
    <2>2. /\ AsyncQueueTyped(Tail(asyncCausalQueues[node]))
           /\ ProgressCommitSourcesIn(Tail(asyncCausalQueues[node]))
      BY <2>1, TypedQueueTailFacts,
         TailProgressCommitSourcePreservesSources
    <2>3. asyncCausalQueues' =
             [asyncCausalQueues EXCEPT
                ![node] = Tail(asyncCausalQueues[node])]
      BY <1>1 DEF AdmitCausalHead
    <2> QED BY <1>1, <2>2, <2>3,
         ReplaceCausalQueuePreservesProgressCommitHistory
  <1> QED BY <1>1

THEOREM LocalAdmissionPreservesProgressCommitHistories ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ TypeInvariant'

    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProgressFrontierAction
    /\ LocalAdmissionStep(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                QueuedProgressCommitHistoryInvariant,
                DeferredProgressCommitHistoryInvariant,
                CausalProgressCommitHistoryInvariant,
                ProgressFrontierAction,
                LocalAdmissionStep(node)
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
    <2>1. /\ TypeInvariant
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncDeferredContentTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant,
             AsyncDeferredTypeInvariant
    <2>2. DeferredProgressCommitHistoryInvariant'
      BY <1>1, <2>1,
         UnchangedDeferredProgressQueuesPreserveCommitHistory
         DEF LocalAdmissionStep, AsyncDeferredVars
    <2>3. CASE /\ LocalAdmissionCanAdvance(node)
                 /\ SelectedLocalSource(node) = "Producer"
      <3> DEFINE Candidate == SelectedCompletionCandidate(node)
      <3>1. /\ AdmitProducerCompletion(node)
             /\ LeaveCausalQueues
        BY <1>1, <2>3 DEF LocalAdmissionStep
      <3>2. /\ AsyncCandidateTyped(Candidate)
             /\ Candidate.class = "Completion"
             /\ ProgressCommitSource(Candidate)
        BY <2>1, <3>1, ProducerSelectedCompletionFacts,
           CompletionCandidateHasProgressCommitSource
           DEF Candidate
      <3>3. EnqueueCandidate(Candidate)
        BY <3>1 DEF AdmitProducerCompletion, Candidate
      <3>4. QueuedProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>2, <3>3,
           EnqueueCandidatePreservesQueuedProgressCommitHistory
      <3>5. CausalProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>1,
           UnchangedCausalQueuesPreserveProgressCommitHistory
           DEF LeaveCausalQueues
      <3> QED BY <2>2, <3>4, <3>5
    <2>4. CASE /\ LocalAdmissionCanAdvance(node)
                 /\ SelectedLocalSource(node) = "Causal"
      <3> DEFINE Candidate == HeadCausalCandidate(node)
      <3>1. AdmitCausalHead(node)
        BY <1>1, <2>4 DEF LocalAdmissionStep
      <3>2. /\ CausalQueueNonempty(node)
             /\ AsyncCandidateTyped(Candidate)
             /\ ProgressCommitSource(Candidate)
        BY <1>1, <2>1, <2>4, SelectedCausalCanAdvance,
           CausalHeadHasProgressCommitSource
           DEF CausalHeadCanAdvance, Candidate
      <3>3. CausalProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <2>4, <3>1,
           AdmitCausalHeadPreservesCausalProgressCommitHistory
      <3>4. CASE CandidateInFlight(Candidate)
        <4>1. UNCHANGED asyncCommandQueues
          BY <3>1, <3>4, Isa DEF AdmitCausalHead, Candidate
        <4> QED BY <1>1, <2>1, <4>1,
             UnchangedCommandQueuesPreserveProgressCommitHistory
      <3>5. CASE /\ ~CandidateInFlight(Candidate)
                   /\ Candidate.class = "Completion"
        <4>1. UNCHANGED asyncCommandQueues
          BY <3>1, <3>5, Isa DEF AdmitCausalHead, Candidate
        <4> QED BY <1>1, <2>1, <4>1,
             UnchangedCommandQueuesPreserveProgressCommitHistory
      <3>6. CASE /\ ~CandidateInFlight(Candidate)
                   /\ Candidate.class # "Completion"
        <4>1. EnqueueCandidate(Candidate)
          BY <3>1, <3>6 DEF AdmitCausalHead, Candidate
        <4> QED BY <1>1, <2>1, <3>2, <4>1,
             EnqueueCandidatePreservesQueuedProgressCommitHistory
      <3>7. QueuedProgressCommitHistoryInvariant'
        BY <3>4, <3>5, <3>6, SMT
      <3> QED BY <2>2, <3>3, <3>7
    <2>5. CASE ~LocalAdmissionCanAdvance(node)
      <3>1. /\ UNCHANGED asyncCommandQueues
             /\ LeaveCausalQueues
        BY <1>1, <2>5, Isa DEF LocalAdmissionStep
      <3>2. QueuedProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>1,
           UnchangedCommandQueuesPreserveProgressCommitHistory
      <3>3. CausalProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>1,
           UnchangedCausalQueuesPreserveProgressCommitHistory
           DEF LeaveCausalQueues
      <3> QED BY <2>2, <3>2, <3>3
    <2>6. \/ /\ LocalAdmissionCanAdvance(node)
                 /\ SelectedLocalSource(node) = "Producer"
           \/ /\ LocalAdmissionCanAdvance(node)
                 /\ SelectedLocalSource(node) = "Causal"
           \/ ~LocalAdmissionCanAdvance(node)
      BY SMT
         DEF SelectedLocalSource, PreferredLocalSource,
             OtherLocalSource
    <2> QED BY <2>3, <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM SelectedLocalAdmissionAdvancePreservesProgressCommitHistories ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProgressFrontierAction
    /\ SelectedLocalAdmissionAdvance(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                QueuedProgressCommitHistoryInvariant,
                DeferredProgressCommitHistoryInvariant,
                CausalProgressCommitHistoryInvariant,
                ProgressFrontierAction,
                SelectedLocalAdmissionAdvance(node)
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
    <2>1. /\ TypeInvariant
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncDeferredContentTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant,
             AsyncDeferredTypeInvariant
    <2>2. DeferredProgressCommitHistoryInvariant'
      BY <1>1, <2>1,
         UnchangedDeferredProgressQueuesPreserveCommitHistory
         DEF SelectedLocalAdmissionAdvance, AsyncDeferredVars
    <2>3. CASE SelectedLocalSource(node) = "Producer"
      <3> DEFINE Candidate == SelectedCompletionCandidate(node)
      <3>1. /\ AdmitProducerCompletion(node)
             /\ LeaveCausalQueues
        BY <1>1, <2>3 DEF SelectedLocalAdmissionAdvance
      <3>2. /\ AsyncCandidateTyped(Candidate)
             /\ Candidate.class = "Completion"
             /\ ProgressCommitSource(Candidate)
        BY <2>1, <3>1, ProducerSelectedCompletionFacts,
           CompletionCandidateHasProgressCommitSource
           DEF Candidate
      <3>3. EnqueueCandidate(Candidate)
        BY <3>1 DEF AdmitProducerCompletion, Candidate
      <3>4. QueuedProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>2, <3>3,
           EnqueueCandidatePreservesQueuedProgressCommitHistory
      <3>5. CausalProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>1,
           UnchangedCausalQueuesPreserveProgressCommitHistory
           DEF LeaveCausalQueues
      <3> QED BY <2>2, <3>4, <3>5
    <2>4. CASE SelectedLocalSource(node) = "Causal"
      <3> DEFINE Candidate == HeadCausalCandidate(node)
      <3>1. AdmitCausalHead(node)
        BY <1>1, <2>4 DEF SelectedLocalAdmissionAdvance
      <3>2. /\ CausalQueueNonempty(node)
             /\ AsyncCandidateTyped(Candidate)
             /\ ProgressCommitSource(Candidate)
        BY <1>1, <2>1, <2>4, SelectedCausalCanAdvance,
           CausalHeadHasProgressCommitSource
           DEF SelectedLocalAdmissionAdvance,
               CausalHeadCanAdvance, Candidate
      <3>3. CausalProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <2>4, <3>1,
           AdmitCausalHeadPreservesCausalProgressCommitHistory
      <3>4. CASE CandidateInFlight(Candidate)
        <4>1. UNCHANGED asyncCommandQueues
          BY <3>1, <3>4, Isa DEF AdmitCausalHead, Candidate
        <4> QED BY <1>1, <2>1, <4>1,
             UnchangedCommandQueuesPreserveProgressCommitHistory
      <3>5. CASE /\ ~CandidateInFlight(Candidate)
                   /\ Candidate.class = "Completion"
        <4>1. UNCHANGED asyncCommandQueues
          BY <3>1, <3>5, Isa DEF AdmitCausalHead, Candidate
        <4> QED BY <1>1, <2>1, <4>1,
             UnchangedCommandQueuesPreserveProgressCommitHistory
      <3>6. CASE /\ ~CandidateInFlight(Candidate)
                   /\ Candidate.class # "Completion"
        <4>1. EnqueueCandidate(Candidate)
          BY <3>1, <3>6 DEF AdmitCausalHead, Candidate
        <4> QED BY <1>1, <2>1, <3>2, <4>1,
             EnqueueCandidatePreservesQueuedProgressCommitHistory
      <3>7. QueuedProgressCommitHistoryInvariant'
        BY <3>4, <3>5, <3>6, SMT
      <3> QED BY <2>2, <3>3, <3>7
    <2>5. \/ SelectedLocalSource(node) = "Producer"
           \/ SelectedLocalSource(node) = "Causal"
      BY SMT
         DEF SelectedLocalSource, PreferredLocalSource,
             OtherLocalSource
    <2> QED BY <2>3, <2>4, <2>5
  <1> QED BY <1>1

THEOREM IngressDrainPreservesProgressCommitHistories ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProgressFrontierAction
    /\ IngressDrainStep(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                QueuedProgressCommitHistoryInvariant,
                DeferredProgressCommitHistoryInvariant,
                CausalProgressCommitHistoryInvariant,
                ProgressFrontierAction,
                IngressDrainStep(node)
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
    <2>1. /\ TypeInvariant
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncDeferredContentTypeInvariant
           /\ AsyncIngressTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncDeferredTypeInvariant
    <2>2. /\ DeferredProgressCommitHistoryInvariant'
           /\ CausalProgressCommitHistoryInvariant'
      BY <1>1, <2>1,
         UnchangedDeferredProgressQueuesPreserveCommitHistory,
         UnchangedCausalQueuesPreserveProgressCommitHistory
         DEF IngressDrainStep, AsyncDeferredVars, LeaveCausalQueues
    <2>3. CASE ~(asyncRunnerBudget[node] > 0
                    /\ asyncIngressReady[node] # <<>>
                    /\ DrainableIngressIndices(node) # {})
      <3>1. UNCHANGED asyncCommandQueues
        BY <1>1, <2>3, Isa DEF IngressDrainStep, vars
      <3>2. QueuedProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>1,
           UnchangedCommandQueuesPreserveProgressCommitHistory
      <3> QED BY <2>2, <3>2
    <2>4. CASE /\ asyncRunnerBudget[node] > 0
                 /\ asyncIngressReady[node] # <<>>
                 /\ DrainableIngressIndices(node) # {}
      <3> DEFINE Index == FirstDrainableIngressIndex(node)
      <3> DEFINE Item == SelectedIngressItemAt(node, Index)
      <3> DEFINE Candidate == DeliveryCandidate(Item)
      <3> DEFINE CommitCandidate ==
             CommitCertificateResponseCandidate(Item)
      <3>1. /\ DrainFairIngressSelected(node)
             /\ Index \in DrainableIngressIndices(node)
             /\ Index \in 1..Len(asyncIngressReady[node])
        BY <1>1, <2>4, FirstDrainableIngressIndexIsDrainable
           DEF IngressDrainStep, Index, DrainableIngressIndices
      <3>1a. SelectedIngressLaneIndex(node, Index)
                   \in 1..Len(IngressLane(
                        node, asyncIngressReady[node][Index]))
        BY <3>1, FirstDrainableIngressLaneIndexIsDrainable
           DEF Index, DrainableIngressIndices, IngressSourceCanDrain,
               DrainableIngressLaneIndices, SelectedIngressLaneIndex
      <3>2. /\ AsyncItemTyped(Item)
             /\ Item.envelope.recipient = node
        BY <2>1, <3>1, <3>1a, SelectedIngressItemIsTyped,
           SelectedIngressItemHasLaneOwnership
           DEF Item, SelectedIngressItemAt
      <3>3. /\ AsyncCandidateTyped(Candidate)
             /\ ProgressCommitSource(Candidate)
        BY <1>1, <3>2, TypedIngressDeliveryCandidateFacts,
           DeliveryCandidateHasProgressCommitSource DEF Candidate
      <3>4. \/ UNCHANGED asyncCommandQueues
             \/ EnqueueCandidate(Candidate)
             \/ /\ Item.kind = "CommitCertificateResponse"
                   /\ EnqueueCandidate(CommitCandidate)
        BY <3>1, Isa
           DEF DrainFairIngressSelected, EnqueueCandidate,
               Candidate, CommitCandidate, Item, Index
      <3>5. CASE UNCHANGED asyncCommandQueues
        BY <1>1, <2>1, <2>2, <3>5,
           UnchangedCommandQueuesPreserveProgressCommitHistory
      <3>6. CASE EnqueueCandidate(Candidate)
        BY <1>1, <2>1, <2>2, <3>3, <3>6,
           EnqueueCandidatePreservesQueuedProgressCommitHistory
      <3>7. CASE /\ Item.kind = "CommitCertificateResponse"
                   /\ EnqueueCandidate(CommitCandidate)
        <4>1. /\ AsyncCandidateTyped(CommitCandidate)
               /\ ProgressCommitSource(CommitCandidate)
          BY <1>1, <3>2, <3>7,
             TypedCommitCertificateResponseCandidateFacts,
             CommitCertificateResponseCandidateHasProgressCommitSource
             DEF CommitCandidate
        <4> QED BY <1>1, <2>1, <2>2, <3>7, <4>1,
             EnqueueCandidatePreservesQueuedProgressCommitHistory
      <3>8. QueuedProgressCommitHistoryInvariant'
        BY <3>4, <3>5, <3>6, <3>7
      <3> QED BY <2>2, <3>8
    <2>5. \/ ~(asyncRunnerBudget[node] > 0
                   /\ asyncIngressReady[node] # <<>>
                   /\ DrainableIngressIndices(node) # {})
           \/ /\ asyncRunnerBudget[node] > 0
                 /\ asyncIngressReady[node] # <<>>
                 /\ DrainableIngressIndices(node) # {}
      BY SMT
    <2> QED BY <2>3, <2>4, <2>5
  <1> QED BY <1>1

THEOREM DeferCommandPreservesDeferredProgressCommitHistory ==
  \A command:
    /\ AsyncConfiguration
    /\ AsyncDeferredTypeInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ AsyncCandidateTyped(command)
    /\ ProgressCommitSource(command)
    /\ DeferCommand(command)
    => DeferredProgressCommitHistoryInvariant'
PROOF
  <1>1. ASSUME NEW command,
                AsyncConfiguration,
                AsyncDeferredTypeInvariant,
                DeferredProgressCommitHistoryInvariant,
                AsyncCandidateTyped(command),
                ProgressCommitSource(command),
                DeferCommand(command)
         PROVE DeferredProgressCommitHistoryInvariant'
    <2>1. UNCHANGED <<context, nodeView, lockRank, lockSubject,
                      prepareQCs>>

      BY <1>1, Isa DEF DeferCommand, vars
    <2>2. ASSUME NEW node \in ValidatorIds
           PROVE ProgressCommitSourcesIn(
                   asyncDeferredProgressQueues[node])'
      <3>1. ProgressCommitSourcesIn(
               asyncDeferredProgressQueues[node])
        BY <1>1, <2>2
           DEF DeferredProgressCommitHistoryInvariant
      <3>2. CASE command.class = "Progress" /\ node = command.node
        <4>1. asyncDeferredProgressQueues'[node] =
                 DeferredProgressAfter(node, command)
          BY <1>1, <2>2, <3>2, Isa
             DEF DeferCommand, AsyncDeferredTypeInvariant,
                 AsyncDeferredTopologyTypeInvariant
        <4>2. ProgressCommitSourcesIn(
                 DeferredProgressAfter(node, command))
          BY <1>1, <2>2, <3>1,
             DeferredProgressAfterPreservesProgressCommitSources
             DEF AsyncDeferredTypeInvariant
        <4>3. ProgressCommitSourcesIn(
                 asyncDeferredProgressQueues'[node])
          BY <4>1, <4>2
        <4> QED BY <2>1, <4>3, Isa
             DEF ProgressCommitSourcesIn, ProgressCommitSource,
                 ProgressCommitVoteHistory,
                 HistoricalLockedCommitItem, LockedPrepareRound
      <3>3. CASE ~(command.class = "Progress" /\ node = command.node)
        <4>1. asyncDeferredProgressQueues'[node] =
                 asyncDeferredProgressQueues[node]
          BY <1>1, <2>2, <3>3, FunctionalUpdateAwayFromKey, Isa
             DEF DeferCommand, AsyncDeferredTypeInvariant,
                 AsyncDeferredTopologyTypeInvariant
        <4>2. ProgressCommitSourcesIn(
                 asyncDeferredProgressQueues'[node])
          BY <3>1, <4>1
        <4> QED BY <2>1, <4>2, Isa
             DEF ProgressCommitSourcesIn, ProgressCommitSource,
                 ProgressCommitVoteHistory,
                 HistoricalLockedCommitItem, LockedPrepareRound
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>2 DEF DeferredProgressCommitHistoryInvariant
  <1> QED BY <1>1

THEOREM RemoveNextDeferredCommandPreservesProgressCommitHistory ==
  \A target \in ValidatorIds:
    /\ AsyncDeferredTypeInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ TypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ ProgressFrontierAction
    /\ DeferredQueueNonempty(target)
    /\ RemoveNextDeferredCommand(target)
    => DeferredProgressCommitHistoryInvariant'
PROOF
  <1>1. ASSUME NEW target \in ValidatorIds,
                AsyncDeferredTypeInvariant,
                DeferredProgressCommitHistoryInvariant,
                TypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                ProgressFrontierAction,
                DeferredQueueNonempty(target),
                RemoveNextDeferredCommand(target)
         PROVE DeferredProgressCommitHistoryInvariant'
    <2>1. ASSUME NEW node \in ValidatorIds
           PROVE ProgressCommitSourcesIn(
                   asyncDeferredProgressQueues[node])'
      <3>1. /\ AsyncQueueTyped(
                     asyncDeferredProgressQueues[node])
             /\ ProgressCommitSourcesIn(
                  asyncDeferredProgressQueues[node])
        BY <1>1, <2>1
           DEF AsyncDeferredTypeInvariant,
               AsyncDeferredContentTypeInvariant,
               DeferredProgressCommitHistoryInvariant
      <3>2. asyncDeferredProgressQueues'[node] =
               IF node = target
                    /\ SelectedDeferredClass(target) = "Progress"
               THEN Tail(asyncDeferredProgressQueues[target])
               ELSE asyncDeferredProgressQueues[node]
        BY <1>1, <2>1,
           RemoveSelectedDeferredProgressQueueEffect
           DEF AsyncDeferredTypeInvariant
      <3>3. SequenceSet(
               asyncDeferredProgressQueues'[node])
               \subseteq SequenceSet(
                 asyncDeferredProgressQueues[node])
        <4>1. CASE /\ node = target
                     /\ SelectedDeferredClass(target) = "Progress"
          <5>1. /\ asyncDeferredProgressQueues'[node] =
                          Tail(asyncDeferredProgressQueues[node])
                 /\ Range(Tail(asyncDeferredProgressQueues[node]))
                       \subseteq
                         Range(asyncDeferredProgressQueues[node])
            BY <3>1, <3>2, <4>1, HeadTailProperties
               DEF AsyncQueueTyped
          <5>2. /\ SequenceSet(
                         asyncDeferredProgressQueues'[node]) =
                          Range(asyncDeferredProgressQueues'[node])
                 /\ SequenceSet(
                      asyncDeferredProgressQueues[node]) =
                          Range(asyncDeferredProgressQueues[node])
            BY <3>1, <4>1, <5>1, RangeEquality
               DEF AsyncQueueTyped, SequenceSet
          <5> QED BY <5>1, <5>2
        <4>2. CASE ~(node = target
                       /\ SelectedDeferredClass(target) = "Progress")
          BY <3>2, <4>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <1>1, <3>1, <3>3,
           RestrictedQueuePreservesProgressCommitSourcesUnderFrontier
    <2> QED BY <2>1 DEF DeferredProgressCommitHistoryInvariant
  <1> QED BY <1>1

THEOREM LockStableNextLeavesNodeView ==
  LockStableNext => UNCHANGED nodeView
BY IsaM("blast")
   DEF LockStableNext, SetGST, AssembleLocalBody, BeginLocalProposal,
       PersistProposal, CompleteProposalSignature,
       ByzantineBroadcastProposal, DeliverProposal, FetchBody,
       RebindRetainedBody, StoreBody,
       ValidateBody, ValidateDecidedBody, ValidateLockedBody, RejectBody,
       BeginPrepare,
       PersistPrepare, CompleteVoteSignature, ByzantineBroadcastVote,
       DeliverVote, FormPrepareQC, DeliverQC, BeginObservePrepare,
       PersistObservePrepare, BeginLockCommit, FormCommitQC, BeginDecision,
       PersistDecision, BeginTimeout, PersistTimeout,
       CompleteTimeoutSignature, ByzantineBroadcastTimeout,
       DeliverTimeout, DeliverTC, BeginInstallTC,
       FetchCertifiedBody, ApplyDecision, Crash, Restart, ResumeProposal,
       ResumeVote, ResumeTimeout, DropProposal

THEOREM PersistLockCommitLeavesNodeView ==
  \A request: PersistLockCommit(request) => UNCHANGED nodeView
BY DEF PersistLockCommit

THEOREM ModelViewsAreNaturals ==
  ModelConfiguration => Views \subseteq Nat
BY DEF ModelConfiguration, Views

THEOREM UnchangedNodeViewAdvances ==
  /\ TypeInvariant
  /\ UNCHANGED nodeView
  => \A node \in ValidatorIds:
       nodeView'[node] >= nodeView[node]
PROOF
  <1>1. ASSUME TypeInvariant,
                UNCHANGED nodeView
         PROVE \A node \in ValidatorIds:
                 nodeView'[node] >= nodeView[node]
    <2>1. /\ ModelConfiguration
           /\ nodeView \in [ValidatorIds -> Views]
      BY <1>1 DEF TypeInvariant
    <2>2. Views \subseteq Nat
      BY <2>1, ModelViewsAreNaturals
    <2>3. ASSUME NEW node \in ValidatorIds
           PROVE nodeView'[node] >= nodeView[node]
      <3>1. nodeView[node] \in Views
        BY <2>1, <2>3, FunctionValueHasCodomain
      <3>2. nodeView[node] \in Nat
        BY <2>2, <3>1
      <3>3. nodeView'[node] = nodeView[node]
        BY <1>1
      <3> QED BY <3>2, <3>3, NaturalOrderReflexive
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM PersistInstallTCAdvancesNodeView ==
  \A request:
    TypeInvariant /\ PersistInstallTC(request)
      => \A node \in ValidatorIds:
           nodeView'[node] >= nodeView[node]
PROOF
  <1>1. ASSUME NEW request,
                TypeInvariant,
                PersistInstallTC(request)
         PROVE \A node \in ValidatorIds:
                 nodeView'[node] >= nodeView[node]
    <2>1a. /\ request \in pendingInstallTC
            /\ pendingInstallTC \subseteq InstallTcWalSet
            /\ nodeView \in [ValidatorIds -> Views]
      BY <1>1 DEF PersistInstallTC, TypeInvariant
    <2>1b. request \in InstallTcWalSet
      BY <2>1a
    <2>1c. /\ request.node \in ValidatorIds
            /\ request.tc.view \in Views
      BY <2>1b DEF InstallTcWalSet, TcRecordSet
    <2>1d. nodeView' =
              [nodeView EXCEPT ![request.node] =
                 IF StrictSameRoundTcUpgrade(request.node, request.tc)
                 THEN @ ELSE request.tc.view + 1]
      BY <1>1 DEF PersistInstallTC
    <2>1e. Views \subseteq Nat
      BY <1>1, ModelViewsAreNaturals DEF TypeInvariant
    <2>2. /\ request.node \in ValidatorIds
           /\ request.tc.view \in Views
           /\ nodeView' =
                [nodeView EXCEPT ![request.node] =
                   IF StrictSameRoundTcUpgrade(request.node, request.tc)
                   THEN @ ELSE request.tc.view + 1]
      BY <2>1c, <2>1d
    <2>3. ASSUME NEW node \in ValidatorIds
           PROVE nodeView'[node] >= nodeView[node]
      <3>1. CASE node = request.node
        <4>1. CASE StrictSameRoundTcUpgrade(request.node, request.tc)
          <5>1. nodeView'[node] = nodeView[node]
            BY <2>1a, <2>2, <3>1, <4>1, Isa
          <5> QED BY <5>1
        <4>2. CASE ~StrictSameRoundTcUpgrade(request.node, request.tc)
          <5>1. /\ request.tc.view >= nodeView[request.node]
                 /\ nodeView'[node] = request.tc.view + 1
            BY <1>1, <2>1a, <2>2, <3>1, <4>2, Isa
               DEF PersistInstallTC
          <5>2. /\ request.tc.view \in Nat
                 /\ nodeView[node] \in Nat
            BY <2>1a, <2>1e, <2>2, <2>3, <3>1,
               FunctionValueHasCodomain
          <5>3. nodeView[node] <= request.tc.view + 1
            BY <5>1, <5>2, NaturalBoundBelowSuccessor
          <5> QED BY <5>1, <5>3
        <4> QED BY <4>1, <4>2
      <3>2. CASE node # request.node
        <4>1. nodeView'[node] = nodeView[node]
          BY <2>1a, <2>2, <2>3, <3>2, Isa
        <4>2. nodeView[node] \in Nat
          BY <2>1a, <2>1e, <2>3, FunctionValueHasCodomain
        <4> QED BY <4>1, <4>2, NaturalOrderReflexive
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM AsyncNextAdvancesNodeViews ==
  StrongInductiveInvariant /\ AsyncNext
    => \A node \in ValidatorIds:
         nodeView'[node] >= nodeView[node]
PROOF
  <1>1. ASSUME StrongInductiveInvariant, AsyncNext
         PROVE \A node \in ValidatorIds:
                 nodeView'[node] >= nodeView[node]
    <2>1. TypeInvariant
      BY <1>1 DEF StrongInductiveInvariant, Safety
    <2>2. [Next]_vars
      BY <1>1 DEF AsyncNext
    <2>3. CASE UNCHANGED vars
      <3>1. UNCHANGED nodeView
        BY <2>3, Isa DEF vars
      <3> QED BY <2>1, <3>1, UnchangedNodeViewAdvances
    <2>4. CASE Next
      <3>1. \/ LockStableNext
             \/ (\E request \in pendingLockCommit:
                   PersistLockCommit(request))
             \/ (\E request \in pendingInstallTC:
                   PersistInstallTC(request))

        BY <2>4, NextLockFootprintClassification
      <3>2. CASE LockStableNext
        <4>1. UNCHANGED nodeView
          BY <3>2, LockStableNextLeavesNodeView
        <4> QED BY <2>1, <4>1, UnchangedNodeViewAdvances
      <3>3. CASE \E request \in pendingLockCommit:
                     PersistLockCommit(request)
        <4>1. UNCHANGED nodeView
          BY <3>3, PersistLockCommitLeavesNodeView
        <4> QED BY <2>1, <4>1, UnchangedNodeViewAdvances
      <3>4. CASE \E request \in pendingInstallTC:
                     PersistInstallTC(request)
        <4>1. ASSUME NEW request \in pendingInstallTC,
                      PersistInstallTC(request)
               PROVE \A node \in ValidatorIds:
                       nodeView'[node] >= nodeView[node]
          BY <2>1, <4>1, PersistInstallTCAdvancesNodeView
        <4> QED BY <3>4, <4>1
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM AsyncNextEstablishesProgressFrontierAction ==
  StrongInductiveInvariant /\ AsyncNext => ProgressFrontierAction
PROOF
  <1>1. ASSUME StrongInductiveInvariant, AsyncNext
         PROVE ProgressFrontierAction
    <2>1. /\ context' = context
           /\ [Next]_vars
      BY <1>1 DEF AsyncNext
    <2>2. prepareQCs \subseteq prepareQCs'
      <3>1. CASE Next
        BY <2>1, <3>1, CoreNextKeepsPrepareQcsMonotone
      <3>2. CASE UNCHANGED vars
        BY <3>2, Isa DEF vars
      <3> QED BY <2>1, <3>1, <3>2
    <2>3. LockMonotonicityAction
      BY <1>1, <2>1, StrongInvariantImpliesLockMonotonicityAction
    <2>4. \A node \in ValidatorIds:
             nodeView'[node] >= nodeView[node]
      BY <1>1, AsyncNextAdvancesNodeViews
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF ProgressFrontierAction
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesLockWithinNodeView ==
  \A initialContext:
    AsyncInitAt(initialContext) => LockWithinNodeViewInvariant
BY SMT DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
           LockWithinNodeViewInvariant, NoRank, ValidatorIds

THEOREM LockStableNextPreservesLockWithinNodeView ==
  LockWithinNodeViewInvariant /\ LockStableNext
    => LockWithinNodeViewInvariant'
BY LockStableNextLeavesContextAndLocks, LockStableNextLeavesNodeView, Isa
   DEF LockWithinNodeViewInvariant, UnchangedContextAndLocks

THEOREM PersistLockCommitPreservesLockWithinNodeView ==
  \A request:
    /\ TypeInvariant
    /\ PendingVoteWritesAuthorized
    /\ LockWithinNodeViewInvariant
    /\ PersistLockCommit(request)
    => LockWithinNodeViewInvariant'
PROOF
  <1>1. ASSUME NEW request,
                TypeInvariant,
                PendingVoteWritesAuthorized,
                LockWithinNodeViewInvariant,
                PersistLockCommit(request)
         PROVE LockWithinNodeViewInvariant'
    <2>1a. /\ request \in pendingLockCommit
            /\ pendingLockCommit \subseteq LockCommitWalSet
            /\ lockRank \in [ValidatorIds -> Ranks]
            /\ nodeView \in [ValidatorIds -> Views]
      BY <1>1 DEF PersistLockCommit, TypeInvariant
    <2>1b. request \in LockCommitWalSet
      BY <2>1a
    <2>1c. request.node \in ValidatorIds
      BY <2>1b DEF LockCommitWalSet
    <2>1d. request.qc.view <= nodeView[request.node]
      BY <1>1, <2>1a, PendingLockCommitAuthorizationFacts
    <2>1e. /\ lockRank' =
                 [lockRank EXCEPT
                    ![request.node] = request.qc.view]
            /\ nodeView' = nodeView
      BY <1>1 DEF PersistLockCommit
    <2>1f. Views \subseteq Nat
      BY <1>1, ModelViewsAreNaturals DEF TypeInvariant
    <2>2. /\ request.node \in ValidatorIds
           /\ request.qc.view <= nodeView[request.node]
           /\ lockRank' =
                [lockRank EXCEPT
                   ![request.node] = request.qc.view]
           /\ nodeView' = nodeView
      BY <2>1c, <2>1d, <2>1e
    <2>3. ASSUME NEW node \in ValidatorIds
           PROVE lockRank'[node] <= nodeView'[node]
      <3>1. CASE node = request.node
        <4>1. /\ lockRank'[node] = request.qc.view
               /\ nodeView'[node] = nodeView[node]
          BY <2>1a, <2>2, <2>3, <3>1, Isa
        <4>2. nodeView[node] \in Nat
          BY <2>1a, <2>1f, <2>3, FunctionValueHasCodomain
        <4>3. lockRank'[node] <= nodeView'[node]
          BY <2>2, <3>1, <4>1
        <4> QED BY <4>3
      <3>2. CASE node # request.node
        <4>1. /\ lockRank'[node] = lockRank[node]
               /\ nodeView'[node] = nodeView[node]
          BY <2>1a, <2>2, <2>3, <3>2, Isa
        <4> QED BY <1>1, <2>3, <4>1
             DEF LockWithinNodeViewInvariant
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>3 DEF LockWithinNodeViewInvariant
  <1> QED BY <1>1

THEOREM PersistInstallTCPreservesLockWithinNodeView ==
  \A request:
    /\ ModelConfiguration
    /\ TypeInvariant
    /\ PendingCertificateWritesAuthorized
    /\ LockWithinNodeViewInvariant
    /\ PersistInstallTC(request)
    => LockWithinNodeViewInvariant'
PROOF
  <1>1. ASSUME NEW request,
                ModelConfiguration,
                TypeInvariant,
                PendingCertificateWritesAuthorized,
                LockWithinNodeViewInvariant,
                PersistInstallTC(request)
         PROVE LockWithinNodeViewInvariant'
    <2> DEFINE Node == request.node
    <2> DEFINE Certificate == request.tc
    <2> DEFINE SelectedRank == TcHighRank(Certificate)
    <2>1a. /\ request \in pendingInstallTC
            /\ pendingInstallTC \subseteq InstallTcWalSet
            /\ lockRank \in [ValidatorIds -> Ranks]
            /\ nodeView \in [ValidatorIds -> Views]
      BY <1>1 DEF PersistInstallTC, TypeInvariant
    <2>1b. request \in InstallTcWalSet
      BY <2>1a
    <2>1c. Node \in ValidatorIds
      BY <2>1b DEF InstallTcWalSet, Node
    <2>2a. /\ TCValid(Certificate)
            /\ Certificate.view >= nodeView[Node]
      BY <1>1, <2>1a
         DEF PendingCertificateWritesAuthorized, Node, Certificate
    <2>2. /\ Node \in ValidatorIds
           /\ TCValid(Certificate)
           /\ Certificate.view >= nodeView[Node]
      BY <2>1c, <2>2a
    <2>3. HighestTimeoutVote(Certificate.votes) \in Certificate.votes
      BY <1>1, <2>2, ValidTimeoutCertificateSelectsMember
    <2>4a. /\ HighestTimeoutVote(Certificate.votes)
                       \in TimeoutVoteRecordSet
            /\ HighestTimeoutVote(Certificate.votes).highRank
                       <= Certificate.view
      BY <2>2, <2>3 DEF TCValid
    <2>4b. SelectedRank =
              HighestTimeoutVote(Certificate.votes).highRank
      BY DEF SelectedRank, TcHighRank
    <2>4c. /\ Views \subseteq Ranks
            /\ Ranks \subseteq Int
      BY <1>1, ViewsAreRanks, ModelRanksAreIntegers
    <2>4d. /\ SelectedRank \in Ranks
            /\ Certificate.view \in Views
      BY <2>2, <2>4a, <2>4b
         DEF TCValid, TimeoutVoteRecordSet
    <2>4e. /\ nodeView[Node] \in Views
            /\ lockRank[Node] \in Ranks
      BY <2>1a, <2>2, FunctionValueHasCodomain
    <2>4. /\ SelectedRank <= Certificate.view
           /\ SelectedRank \in Int
           /\ Certificate.view \in Int
           /\ nodeView[Node] \in Int
           /\ lockRank[Node] \in Int
      BY <2>4a, <2>4b, <2>4c, <2>4d, <2>4e
    <2>5. /\ nodeView' =
                [nodeView EXCEPT
                   ![Node] = Certificate.view + 1]
           /\ lockRank' =
                [lockRank EXCEPT
                   ![Node] =
                     IF SelectedRank > lockRank[Node]
                     THEN SelectedRank ELSE @]
      BY <1>1 DEF PersistInstallTC, Node, Certificate, SelectedRank
    <2>6. ASSUME NEW node \in ValidatorIds
           PROVE lockRank'[node] <= nodeView'[node]
      <3>1. CASE node = Node
        <4>1. /\ nodeView'[node] = Certificate.view + 1
               /\ lockRank'[node] =
                    IF SelectedRank > lockRank[node]
                    THEN SelectedRank ELSE lockRank[node]
          BY <2>1a, <2>2, <2>5, <3>1, Isa
        <4>2. lockRank[node] <= nodeView[node]
          BY <1>1, <2>2, <3>1 DEF LockWithinNodeViewInvariant
        <4>3. /\ Certificate.view + 1 \in Int
               /\ Certificate.view < Certificate.view + 1
          BY <2>4, SMT
        <4>4. CASE SelectedRank > lockRank[node]
          <5>1. lockRank'[node] = SelectedRank
            BY <4>1, <4>4
          <5>2. SelectedRank < Certificate.view + 1
            BY <2>4, <4>3, IntegerWeakStrongOrderChain
          <5>3. SelectedRank <= Certificate.view + 1
            BY <2>4, <4>3, <5>2, IntegerStrictImpliesWeak
          <5> QED BY <4>1, <5>1, <5>3
        <4>5. CASE ~(SelectedRank > lockRank[node])
          <5>1. lockRank'[node] = lockRank[node]
            BY <4>1, <4>5
          <5>2. lockRank[node] <= Certificate.view
            BY <2>2, <2>4, <3>1, <4>2,
               IntegerWeakOrderTransitive
          <5>3. lockRank[node] < Certificate.view + 1
            BY <2>4, <3>1, <4>3, <5>2,
               IntegerWeakStrongOrderChain
          <5>4. lockRank[node] <= Certificate.view + 1
            BY <2>4, <3>1, <4>3, <5>3,
               IntegerStrictImpliesWeak
          <5> QED BY <4>1, <5>1, <5>4
        <4> QED BY <4>4, <4>5
      <3>2. CASE node # Node
        <4>1. /\ lockRank'[node] = lockRank[node]
               /\ nodeView'[node] = nodeView[node]
          BY <2>1a, <2>2, <2>5, <2>6, <3>2, Isa
        <4> QED BY <1>1, <2>6, <4>1
             DEF LockWithinNodeViewInvariant
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>6 DEF LockWithinNodeViewInvariant
  <1> QED BY <1>1

THEOREM AsyncNextPreservesLockWithinNodeView ==
  /\ StrongInductiveInvariant
  /\ LockWithinNodeViewInvariant
  /\ AsyncNext
  => LockWithinNodeViewInvariant'
PROOF
  <1>1. ASSUME /\ StrongInductiveInvariant
                /\ LockWithinNodeViewInvariant
                /\ AsyncNext
         PROVE LockWithinNodeViewInvariant'
    <2>1a. Safety
      BY <1>1 DEF StrongInductiveInvariant
    <2>1b. ReducerProvenanceInvariant
      BY <1>1 DEF StrongInductiveInvariant

    <2>1c. TypeInvariant
      BY <2>1a DEF Safety
    <2>1d. /\ PendingVoteWritesAuthorized
            /\ PendingCertificateWritesAuthorized
      BY <2>1b DEF ReducerProvenanceInvariant
    <2>1e. ModelConfiguration
      BY <2>1c DEF TypeInvariant
    <2>1. /\ TypeInvariant
           /\ PendingVoteWritesAuthorized
           /\ PendingCertificateWritesAuthorized
           /\ ModelConfiguration
      BY <2>1c, <2>1d, <2>1e
    <2>2. [Next]_vars
      BY <1>1 DEF AsyncNext
    <2>3. CASE UNCHANGED vars
      BY <1>1, <2>3, Isa
         DEF LockWithinNodeViewInvariant, vars
    <2>4. CASE Next
      <3>1. \/ LockStableNext
             \/ (\E request \in pendingLockCommit:
                   PersistLockCommit(request))
             \/ (\E request \in pendingInstallTC:
                   PersistInstallTC(request))
        BY <2>4, NextLockFootprintClassification
      <3>2. CASE LockStableNext
        BY <1>1, <3>2, LockStableNextPreservesLockWithinNodeView
      <3>3. CASE \E request \in pendingLockCommit:
                     PersistLockCommit(request)
        <4>1. ASSUME NEW request \in pendingLockCommit,
                      PersistLockCommit(request)
               PROVE LockWithinNodeViewInvariant'
          BY <1>1, <2>1, <4>1,
             PersistLockCommitPreservesLockWithinNodeView
        <4> QED BY <3>3, <4>1
      <3>4. CASE \E request \in pendingInstallTC:
                     PersistInstallTC(request)
        <4>1. ASSUME NEW request \in pendingInstallTC,
                      PersistInstallTC(request)
               PROVE LockWithinNodeViewInvariant'
          BY <1>1, <2>1, <4>1,
             PersistInstallTCPreservesLockWithinNodeView
        <4> QED BY <3>4, <4>1
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

ProtectedProgressQueueBoundedUnique(queue) ==
  /\ Cardinality(ProtectedProgressIndicesIn(queue)) <= 3 * N + 3
  /\ ProtectedProgressSlotsUniqueIn(queue)

THEOREM LockAdvancePreservesProtectedProgressQueue ==
  \A node \in ValidatorIds:
    \A queue:
      /\ N \in Nat \ {0}
      /\ TypeInvariant
      /\ AsyncQueueTyped(queue)
      /\ AsyncCommandQueueOwnership(node, queue)
      /\ \A command \in SequenceSet(queue):
           command.class = "Progress"
      /\ ProgressCommitSourcesIn(queue)
      /\ ProtectedProgressSlotsUniqueIn(queue)
      /\ LockMonotonicityAction
      /\ UNCHANGED <<context, prepareQCs>>
      => ProtectedProgressQueueBoundedUnique(queue)'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW queue,
                N \in Nat \ {0},
                TypeInvariant,
                AsyncQueueTyped(queue),
                AsyncCommandQueueOwnership(node, queue),
                \A command \in SequenceSet(queue):
                  command.class = "Progress",
                ProgressCommitSourcesIn(queue),
                ProtectedProgressSlotsUniqueIn(queue),
                LockMonotonicityAction,
                UNCHANGED <<context, prepareQCs>>
         PROVE ProtectedProgressQueueBoundedUnique(queue)'
    <2>1. ProtectedProgressIndicesIn(queue)'
             \subseteq ProtectedProgressIndicesIn(queue)
      <3>1. ASSUME NEW index \in ProtectedProgressIndicesIn(queue)'
             PROVE index \in ProtectedProgressIndicesIn(queue)
        <4>1. /\ index \in 1..Len(queue)
               /\ ProtectedProgressCommand(queue[index])'
               /\ queue[index] \in SequenceSet(queue)
          BY <3>1 DEF ProtectedProgressIndicesIn, SequenceSet
        <4>2. /\ AsyncCandidateTyped(queue[index])
               /\ queue[index].class = "Progress"
               /\ ProgressCommitSource(queue[index])
          BY <1>1, <4>1
             DEF AsyncQueueTyped, ProgressCommitSourcesIn
        <4>3. ProgressCommitVoteHistory(queue[index])
          BY <4>2 DEF ProgressCommitSource
        <4>4. ProtectedProgressCommand(queue[index])
          BY <1>1, <4>1, <4>2, <4>3,
             ProtectedProgressCommandBackwardStableUnderLockAdvance
        <4> QED BY <4>1, <4>4 DEF ProtectedProgressIndicesIn
      <3> QED BY <3>1
    <2>2. /\ IsFiniteSet(ProtectedProgressIndicesIn(queue))
           /\ IsFiniteSet(ProtectedProgressIndicesIn(queue)')
           /\ Cardinality(ProtectedProgressIndicesIn(queue)') <=
                Cardinality(ProtectedProgressIndicesIn(queue))
      <3>1. /\ Len(queue) \in Nat
             /\ IsFiniteSet(1..Len(queue))
        BY <1>1, LenProperties, FS_Interval DEF AsyncQueueTyped
      <3>2. ProtectedProgressIndicesIn(queue) \subseteq
               1..Len(queue)
        BY DEF ProtectedProgressIndicesIn
      <3>3. IsFiniteSet(ProtectedProgressIndicesIn(queue))
        BY <3>1, <3>2, FS_Subset
      <3> QED BY <2>1, <3>3, FS_Subset, FS_CardinalityType
    <2>3. Cardinality(ProtectedProgressIndicesIn(queue)) <= 3 * N + 3
      BY <1>1, UniqueTypedOwnedProtectedSlotsAreBounded
    <2>4. Cardinality(ProtectedProgressIndicesIn(queue)') <= 3 * N + 3
      <3>1. /\ Cardinality(ProtectedProgressIndicesIn(queue)') \in Nat
             /\ Cardinality(ProtectedProgressIndicesIn(queue)) \in Nat
             /\ 3 * N + 3 \in Nat
        BY <1>1, <2>2, FS_CardinalityType, SMT
      <3> QED BY <2>2, <2>3, <3>1, NaturalWeakOrderTrans
    <2>5. ProtectedProgressSlotsUniqueIn(queue)'
      <3>1. ASSUME NEW left,
                    NEW right,
                    left \in ProtectedProgressIndicesIn(queue)',
                    right \in ProtectedProgressIndicesIn(queue)',
                    SameProtectedProgressSlot(
                      queue[left], queue[right])'
             PROVE left = right
        <4>1. /\ left \in ProtectedProgressIndicesIn(queue)
               /\ right \in ProtectedProgressIndicesIn(queue)
          BY <2>1, <3>1
        <4>2. /\ ProtectedProgressCommand(queue[left])
               /\ ProtectedProgressCommand(queue[right])
          BY <4>1 DEF ProtectedProgressIndicesIn
        <4>3. ProgressSlotShape(queue[left], queue[right])'
          BY <3>1 DEF SameProtectedProgressSlot, ProgressSlotShape
        <4>4. ProgressSlotShape(queue[left], queue[right])
          BY <4>3, Isa DEF ProgressSlotShape
        <4>5. SameProtectedProgressSlot(queue[left], queue[right])
          BY <4>2, <4>4 DEF SameProtectedProgressSlot,
                              ProgressSlotShape
        <4> QED BY <1>1, <4>1, <4>5
             DEF ProtectedProgressSlotsUniqueIn
      <3> QED BY <3>1 DEF ProtectedProgressSlotsUniqueIn
    <2> QED BY <2>4, <2>5 DEF ProtectedProgressQueueBoundedUnique
  <1> QED BY <1>1

THEOREM RemoveDeferredAndAdvanceLockPreservesProtectedInvariant ==
  \A target \in ValidatorIds:
    /\ N \in Nat \ {0}
    /\ TypeInvariant
    /\ AsyncDeferredTypeInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ LockMonotonicityAction
    /\ UNCHANGED <<context, prepareQCs>>
    /\ DeferredQueueNonempty(target)
    /\ RemoveNextDeferredCommand(target)
    => ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME NEW target \in ValidatorIds,
                N \in Nat \ {0},
                TypeInvariant,
                AsyncDeferredTypeInvariant,
                ProtectedDeferredProgressInvariant,
                DeferredProgressCommitHistoryInvariant,
                LockMonotonicityAction,
                UNCHANGED <<context, prepareQCs>>,
                DeferredQueueNonempty(target),
                RemoveNextDeferredCommand(target)
         PROVE ProtectedDeferredProgressInvariant'
    <2>1. AsyncDeferredContentTypeInvariant'
      BY <1>1, RemoveNextDeferredCommandPreservesDeferredContentType
         DEF AsyncDeferredTypeInvariant
    <2>2. ASSUME NEW node \in ValidatorIds
           PROVE /\ Cardinality(
                         ProtectedDeferredProgressIndices(node)') <= 3 * N + 3
                 /\ \A left, right \in
                        ProtectedDeferredProgressIndices(node)':
                      SameProtectedProgressSlot(
                        asyncDeferredProgressQueues[node][left],
                        asyncDeferredProgressQueues[node][right])'
                        => left = right
      <3> DEFINE Old == asyncDeferredProgressQueues[node]
      <3> DEFINE New == asyncDeferredProgressQueues'[node]
      <3>1. /\ AsyncQueueTyped(Old)
             /\ AsyncCommandQueueOwnership(node, Old)
             /\ \A command \in SequenceSet(Old):
                  command.class = "Progress"
             /\ ProgressCommitSourcesIn(Old)
        BY <1>1, <2>2
           DEF AsyncDeferredTypeInvariant,
               AsyncDeferredContentTypeInvariant,
               DeferredProgressCommitHistoryInvariant, Old
      <3>2. /\ AsyncQueueTyped(New)
             /\ AsyncCommandQueueOwnership(node, New)
             /\ \A command \in SequenceSet(New):
                  command.class = "Progress"
        BY <2>1, <2>2, Isa
           DEF AsyncDeferredContentTypeInvariant, New
      <3>3. New =
               IF node = target
                    /\ SelectedDeferredClass(target) = "Progress"
               THEN Tail(Old)
               ELSE Old
        BY <1>1, <2>2,
           RemoveSelectedDeferredProgressQueueEffect
           DEF AsyncDeferredTypeInvariant, Old, New
      <3>4. ProgressCommitSourcesIn(New)
        <4>1. CASE /\ node = target
                     /\ SelectedDeferredClass(target) = "Progress"
          <5>1. New = Tail(Old)
            BY <3>3, <4>1
          <5>2. ProgressCommitSourcesIn(Tail(Old))
            BY <3>1, <4>1,
               TailProgressCommitSourcePreservesSources
          <5> QED BY <5>1, <5>2
        <4>2. CASE ~(node = target
                       /\ SelectedDeferredClass(target) = "Progress")
          BY <3>1, <3>3, <4>2
        <4> QED BY <4>1, <4>2
      <3>5. ProtectedProgressSlotsUniqueIn(Old)
        BY <1>1, <2>2,
           ProtectedDeferredProgressIndicesAreQueueIndices
           DEF ProtectedDeferredProgressInvariant,
               ProtectedProgressSlotsUniqueIn, Old
      <3>6. ProtectedProgressSlotsUniqueIn(New)
        <4>1. CASE /\ node = target
                     /\ SelectedDeferredClass(target) = "Progress"
          <5>1. New = Tail(Old)
            BY <3>3, <4>1
          <5>2. ProtectedProgressSlotsUniqueIn(Tail(Old))
            BY <3>1, <3>5, <4>1,
               TailPreservesProtectedSlotUniqueness
          <5> QED BY <5>1, <5>2
        <4>2. CASE ~(node = target
                       /\ SelectedDeferredClass(target) = "Progress")
          BY <3>3, <3>5, <4>2
        <4> QED BY <4>1, <4>2
      <3>7. ProtectedProgressQueueBoundedUnique(Old)'
        BY <1>1, <2>2, <3>2, <3>4, <3>6,
           LockAdvancePreservesProtectedProgressQueue
      <3>8. ProtectedDeferredProgressIndices(node)' =
               ProtectedProgressIndicesIn(Old)'
        BY DEF ProtectedDeferredProgressIndices,
               ProtectedProgressIndicesIn, New

      <3> QED BY <3>7, <3>8
           DEF ProtectedProgressQueueBoundedUnique,
               ProtectedProgressSlotsUniqueIn
    <2> QED BY <2>2 DEF ProtectedDeferredProgressInvariant
  <1> QED BY <1>1

THEOREM LockAdvancePreservesProtectedDeferredProgressInvariant ==
  /\ N \in Nat \ {0}
  /\ TypeInvariant
  /\ AsyncDeferredTypeInvariant
  /\ ProtectedDeferredProgressInvariant
  /\ DeferredProgressCommitHistoryInvariant
  /\ LockMonotonicityAction
  /\ UNCHANGED <<context, prepareQCs,
                  asyncDeferredProgressQueues>>
  => ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME /\ N \in Nat \ {0}
                /\ TypeInvariant
                /\ AsyncDeferredTypeInvariant
                /\ ProtectedDeferredProgressInvariant
                /\ DeferredProgressCommitHistoryInvariant
                /\ LockMonotonicityAction
                /\ UNCHANGED <<context, prepareQCs,
                                asyncDeferredProgressQueues>>
         PROVE ProtectedDeferredProgressInvariant'
    <2>1. /\ context' = context
           /\ prepareQCs' = prepareQCs
           /\ asyncDeferredProgressQueues' =
                asyncDeferredProgressQueues
      BY <1>1, Isa
    <2>2. \A node \in ValidatorIds:
             ProtectedDeferredProgressIndices(node)'
               \subseteq ProtectedDeferredProgressIndices(node)
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE ProtectedDeferredProgressIndices(node)'
                     \subseteq ProtectedDeferredProgressIndices(node)
        <4>1. ASSUME NEW index \in
                       ProtectedDeferredProgressIndices(node)'
               PROVE index \in
                       ProtectedDeferredProgressIndices(node)
          <5>1. /\ index \in
                         1..Len(asyncDeferredProgressQueues[node])
                 /\ ProtectedProgressCommand(
                      asyncDeferredProgressQueues[node][index])'
            BY <2>1, <4>1
               DEF ProtectedDeferredProgressIndices
          <5>2. AsyncCandidateTyped(
                   asyncDeferredProgressQueues[node][index])
            BY <1>1, <3>1, <5>1
               DEF AsyncDeferredTypeInvariant,
                   AsyncDeferredContentTypeInvariant, AsyncQueueTyped
          <5>3. ProgressCommitVoteHistory(
                   asyncDeferredProgressQueues[node][index])
            <6>1. asyncDeferredProgressQueues[node][index]
                     \in SequenceSet(
                          asyncDeferredProgressQueues[node])
              BY <5>1 DEF SequenceSet
            <6>2. ProgressCommitSource(
                   asyncDeferredProgressQueues[node][index])
              BY <1>1, <3>1, <6>1
                 DEF DeferredProgressCommitHistoryInvariant,
                     ProgressCommitSourcesIn
            <6>3. asyncDeferredProgressQueues[node][index].class =
                     "Progress"
              BY <1>1, <3>1, <6>1
                 DEF AsyncDeferredTypeInvariant,
                     AsyncDeferredContentTypeInvariant
            <6> QED BY <6>2, <6>3 DEF ProgressCommitSource
          <5>4. ProtectedProgressCommand(
                   asyncDeferredProgressQueues[node][index])
            BY <1>1, <2>1, <5>1, <5>2, <5>3,
               ProtectedProgressCommandBackwardStableUnderLockAdvance
          <5> QED BY <5>1, <5>4
               DEF ProtectedDeferredProgressIndices
        <4> QED BY <4>1
      <3> QED BY <3>1
    <2>3. \A node \in ValidatorIds:
             /\ IsFiniteSet(ProtectedDeferredProgressIndices(node))
             /\ IsFiniteSet((ProtectedDeferredProgressIndices(node))')
             /\ Cardinality((ProtectedDeferredProgressIndices(node))')
                  <= Cardinality(
                       ProtectedDeferredProgressIndices(node))
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE /\ IsFiniteSet(
                           ProtectedDeferredProgressIndices(node))
                   /\ IsFiniteSet(
                           (ProtectedDeferredProgressIndices(node))')
                   /\ Cardinality(
                         (ProtectedDeferredProgressIndices(node))')
                        <= Cardinality(
                             ProtectedDeferredProgressIndices(node))
        <4>1. Len(asyncDeferredProgressQueues[node]) \in Nat
          BY <1>1, <3>1, LenProperties
             DEF AsyncDeferredTypeInvariant,
                 AsyncDeferredContentTypeInvariant, AsyncQueueTyped
        <4>2. IsFiniteSet(
                 1..Len(asyncDeferredProgressQueues[node]))
          BY <4>1, FS_Interval
        <4>3. ProtectedDeferredProgressIndices(node)
                 \subseteq
                   1..Len(asyncDeferredProgressQueues[node])
          BY DEF ProtectedDeferredProgressIndices
        <4>4. IsFiniteSet(
                 ProtectedDeferredProgressIndices(node))
          BY <4>2, <4>3, FS_Subset
        <4> QED BY <2>2, <3>1, <4>4,
             FS_Subset, FS_CardinalityType
      <3> QED BY <3>1
    <2>4. \A node \in ValidatorIds:
             Cardinality((ProtectedDeferredProgressIndices(node))')
               <= 3 * N + 3
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE Cardinality(
                     (ProtectedDeferredProgressIndices(node))')
                     <= 3 * N + 3
        <4>1. Cardinality(ProtectedDeferredProgressIndices(node))
                 <= 3 * N + 3
          BY <1>1, <3>1 DEF ProtectedDeferredProgressInvariant
        <4>2. Cardinality(
                 (ProtectedDeferredProgressIndices(node))')
                 <= Cardinality(
                      ProtectedDeferredProgressIndices(node))
          BY <2>3, <3>1
        <4>3. /\ Cardinality(
                       (ProtectedDeferredProgressIndices(node))') \in Nat
               /\ Cardinality(
                    ProtectedDeferredProgressIndices(node)) \in Nat
               /\ 3 * N + 3 \in Nat
          BY <1>1, <2>3, <3>1, FS_CardinalityType, SMT
        <4> QED BY <4>1, <4>2, <4>3, NaturalWeakOrderTrans
      <3> QED BY <3>1
    <2>5. \A node \in ValidatorIds:
             \A left, right \in
                  ProtectedDeferredProgressIndices(node)':
               SameProtectedProgressSlot(
                 asyncDeferredProgressQueues[node][left],
                 asyncDeferredProgressQueues[node][right])'
                 => left = right
      <3>1. ASSUME NEW node \in ValidatorIds,
                    NEW left \in
                      ProtectedDeferredProgressIndices(node)',
                    NEW right \in
                      ProtectedDeferredProgressIndices(node)',
                    SameProtectedProgressSlot(
                      asyncDeferredProgressQueues[node][left],
                      asyncDeferredProgressQueues[node][right])'
             PROVE left = right
        <4>1. /\ left \in ProtectedDeferredProgressIndices(node)
               /\ right \in ProtectedDeferredProgressIndices(node)
          BY <2>2, <3>1
        <4>1a. /\ ProtectedProgressCommand(
                       asyncDeferredProgressQueues[node][left])
                /\ ProtectedProgressCommand(
                       asyncDeferredProgressQueues[node][right])
          BY <4>1 DEF ProtectedDeferredProgressIndices
        <4>1b. ProgressSlotShape(
                  asyncDeferredProgressQueues[node][left],
                  asyncDeferredProgressQueues[node][right])'
          BY <3>1 DEF SameProtectedProgressSlot, ProgressSlotShape
        <4>1c. ProgressSlotShape(
                  asyncDeferredProgressQueues[node][left],
                  asyncDeferredProgressQueues[node][right])
          BY <2>1, <4>1b, Isa DEF ProgressSlotShape
        <4>2. SameProtectedProgressSlot(
                 asyncDeferredProgressQueues[node][left],
                 asyncDeferredProgressQueues[node][right])
          BY <4>1a, <4>1c DEF SameProtectedProgressSlot,
                            ProgressSlotShape
        <4> QED BY <1>1, <3>1, <4>1, <4>2
             DEF ProtectedDeferredProgressInvariant
      <3> QED BY <3>1
    <2> QED BY <2>4, <2>5
         DEF ProtectedDeferredProgressInvariant
  <1> QED BY <1>1

ProtectedDeferredProgressCardinality(node) ==
  Cardinality(ProtectedDeferredProgressIndices(node)) <= 3 * N + 3

ProtectedDeferredProgressSlot(node, left, right) ==
  SameProtectedProgressSlot(
    asyncDeferredProgressQueues[node][left],
    asyncDeferredProgressQueues[node][right])

ProtectedDeferredProgressUniqueness(node) ==
  \A left, right \in ProtectedDeferredProgressIndices(node):
    ProtectedDeferredProgressSlot(node, left, right) => left = right

ProtectedDeferredProgressNode(node) ==
  /\ ProtectedDeferredProgressCardinality(node)
  /\ ProtectedDeferredProgressUniqueness(node)

THEOREM PrimedProtectedDeferredProgressNodesImplyInvariant ==
  (\A node \in ValidatorIds: ProtectedDeferredProgressNode(node)')
    => ProtectedDeferredProgressInvariant'
BY Isa
   DEF ProtectedDeferredProgressInvariant,
       ProtectedDeferredProgressNode,
       ProtectedDeferredProgressCardinality,
       ProtectedDeferredProgressUniqueness,
       ProtectedDeferredProgressSlot

=============================================================================
