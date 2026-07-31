---- MODULE SumeragiV2AsyncIngressRunnerTypeProofs ----
EXTENDS SumeragiV2AsyncSchedulerPrimitiveTypeProofs

(***************************************************************************
The auxiliary first-message cases are named explicitly because they are the
boundary where an empty lane trades one protected owner for the newly admitted
item.  These corollaries prevent later changes from covering only one progress
class while silently regressing the first timeout vote, ordinary proposal, or
roster-origin completion relayed through the aggregate untrusted hop.
***************************************************************************)
THEOREM AdmitFirstTimeoutVotePreservesIngressCapacityType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncTypeInvariant
    /\ source \in ValidatorIds
    /\ IngressLaneDepth(recipient, source) = 0
    /\ DueSourcePackets(recipient, source) # {}
    /\ OldestDueSourcePacket(recipient, source).item.kind = "TimeoutVote"
    /\ AdmitHiddenPacket(recipient, source)
    => AsyncIngressCapacityTypeInvariant'
BY AdmitHiddenPacketPreservesIngressCapacityType

THEOREM AdmitFirstProposalPreservesIngressCapacityType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncTypeInvariant
    /\ source \in ValidatorIds
    /\ IngressLaneDepth(recipient, source) = 0
    /\ DueSourcePackets(recipient, source) # {}
    /\ OldestDueSourcePacket(recipient, source).item.kind = "Proposal"
    /\ AdmitHiddenPacket(recipient, source)
    => AsyncIngressCapacityTypeInvariant'
BY AdmitHiddenPacketPreservesIngressCapacityType

THEOREM AdmitFirstRelayedTransportCompletionPreservesIngressCapacityType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    LET item == OldestDueSourcePacket(recipient, source).item
        resourceSource == IngressResourceSource(item)
    IN /\ AsyncTypeInvariant
       /\ IngressLaneDepth(recipient, resourceSource) = 0
       /\ DueSourcePackets(recipient, source) # {}
       /\ IngressUsesPhysicalCompletionOwner(item)
       /\ AdmitHiddenPacket(recipient, source)
       => AsyncIngressCapacityTypeInvariant'
BY AdmitHiddenPacketPreservesIngressCapacityType

THEOREM AdmitHiddenPacketPreservesSchedulerType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant /\ AdmitHiddenPacket(recipient, source)
      => AsyncSchedulerTypeInvariant'
BY AdmitHiddenPacketPreservesNonIngressType,
   AdmitHiddenPacketPreservesIngressTopologyType,
   AdmitHiddenPacketPreservesIngressCapacityType,
   AdmitHiddenPacketPreservesIngressContentType,
   HistoricalRecoveryFramePreservesType, Isa
   DEF AsyncSchedulerTypeInvariant, AsyncIngressTypeInvariant,
       AdmitHiddenPacket, AsyncHistoricalRecoveryFrameVars, vars

THEOREM CoalesceHiddenPacketPreservesNonIngressType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant /\ CoalesceHiddenPacket(recipient, source)
      => /\ AsyncRuntimeTypeInvariant'
         /\ AsyncIoTypeInvariant'
         /\ AsyncDeferredTypeInvariant'
         /\ AsyncTransportTypeInvariant'
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                CoalesceHiddenPacket(recipient, source)
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
         DEF CoalesceHiddenPacket, AsyncRuntimeScalarTypeVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredVars,
             AsyncDeferredTopologyTypeVars, AsyncTransportClockTypeVars,
             AsyncTransportHistoryTypeVars,
             AsyncCertifiedResponseClaimAuthorityVars,
             AsyncCertifiedResponseClaimCoreAuthorityVars,
             LeaveCausalQueues,
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
      BY <1>1, Isa DEF CoalesceHiddenPacket
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

THEOREM CoalesceHiddenPacketPreservesIngressType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant /\ CoalesceHiddenPacket(recipient, source)
      => AsyncIngressTypeInvariant'
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                CoalesceHiddenPacket(recipient, source)
         PROVE AsyncIngressTypeInvariant'
    <2>1. /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressCapacityTypeInvariant
           /\ AsyncIngressContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIngressTypeInvariant
    <2>2. /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, Isa
         DEF CoalesceHiddenPacket, AsyncIngressTopologyTypeVars
    <2>3. /\ AsyncIngressTopologyTypeInvariant'
           /\ AsyncIngressCapacityTypeInvariant'
           /\ AsyncIngressContentTypeInvariant'
      BY <2>1, <2>2, AsyncIngressTopologyTypeStutter,
         AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter
    <2> QED BY <2>3 DEF AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM CoalesceHiddenPacketPreservesSchedulerType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant /\ CoalesceHiddenPacket(recipient, source)
      => AsyncSchedulerTypeInvariant'
BY CoalesceHiddenPacketPreservesNonIngressType,
   CoalesceHiddenPacketPreservesIngressType,
   HistoricalRecoveryFramePreservesType, Isa
   DEF AsyncSchedulerTypeInvariant, CoalesceHiddenPacket,
       AsyncHistoricalRecoveryFrameVars, vars

THEOREM DropPolicyRejectedHiddenPacketPreservesNonIngressType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant
      /\ DropPolicyRejectedHiddenPacket(recipient, source)
    => /\ AsyncRuntimeTypeInvariant'
       /\ AsyncIoTypeInvariant'
       /\ AsyncDeferredTypeInvariant'
       /\ AsyncTransportTypeInvariant'
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                DropPolicyRejectedHiddenPacket(recipient, source)
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
         DEF DropPolicyRejectedHiddenPacket, AsyncRuntimeScalarTypeVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredVars,
             AsyncDeferredTopologyTypeVars, AsyncTransportClockTypeVars,
             AsyncTransportHistoryTypeVars,
             AsyncCertifiedResponseClaimAuthorityVars,
             AsyncCertifiedResponseClaimCoreAuthorityVars,
             LeaveCausalQueues, AsyncSchedulerVars, vars
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
      BY <1>1, Isa DEF DropPolicyRejectedHiddenPacket
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

THEOREM DropPolicyRejectedHiddenPacketPreservesIngressType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant
      /\ DropPolicyRejectedHiddenPacket(recipient, source)
    => AsyncIngressTypeInvariant'
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                DropPolicyRejectedHiddenPacket(recipient, source)
         PROVE AsyncIngressTypeInvariant'
    <2>1. /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressCapacityTypeInvariant
           /\ AsyncIngressContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIngressTypeInvariant
    <2>2. /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, Isa
         DEF DropPolicyRejectedHiddenPacket,
             AsyncIngressTopologyTypeVars
    <2>3. /\ AsyncIngressTopologyTypeInvariant'
           /\ AsyncIngressCapacityTypeInvariant'
           /\ AsyncIngressContentTypeInvariant'
      BY <2>1, <2>2, AsyncIngressTopologyTypeStutter,
         AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter
    <2> QED BY <2>3 DEF AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM DropPolicyRejectedHiddenPacketPreservesSchedulerType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant
      /\ DropPolicyRejectedHiddenPacket(recipient, source)
    => AsyncSchedulerTypeInvariant'
BY DropPolicyRejectedHiddenPacketPreservesNonIngressType,
   DropPolicyRejectedHiddenPacketPreservesIngressType,
   HistoricalRecoveryFramePreservesType, Isa
   DEF AsyncSchedulerTypeInvariant, DropPolicyRejectedHiddenPacket,
       AsyncHistoricalRecoveryFrameVars, vars

THEOREM AdmitIngressPacketPreservesSchedulerType ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant /\ AdmitIngressPacket(recipient, source)
      => AsyncSchedulerTypeInvariant'
BY AdmitHiddenPacketPreservesSchedulerType,
   CoalesceHiddenPacketPreservesSchedulerType,
   DropPolicyRejectedHiddenPacketPreservesSchedulerType
   DEF AdmitIngressPacket

THEOREM AsyncNetworkStepPreservesSchedulerType ==
  AsyncTypeInvariant /\ AsyncNetworkStep
    => AsyncSchedulerTypeInvariant'
BY AdmitIngressPacketPreservesSchedulerType
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

THEOREM OwnedTypedQueueTailPreservesCommandQueueOwnership ==
  \A node, queue:
    /\ AsyncQueueTyped(queue)
    /\ AsyncCommandQueueOwnership(node, queue)
    /\ Len(queue) > 0
    => AsyncCommandQueueOwnership(node, Tail(queue))
BY TypedQueueTailFacts, SMT
   DEF AsyncCommandQueueOwnership

THEOREM IngressSequenceWithoutIndexFacts ==
  \A sequence, index:
    /\ sequence \in Seq(Range(sequence))
    /\ index \in 1..Len(sequence)
    => LET result == SequenceWithoutIndex(sequence, index)
       IN /\ result \in Seq(Range(sequence))
          /\ Len(result) = Len(sequence) - 1
          /\ DOMAIN result = 1..Len(result)
          /\ \A resultIndex \in 1..Len(result):
               result[resultIndex] =
                 IF resultIndex < index
                 THEN sequence[resultIndex]
                 ELSE sequence[resultIndex + 1]
          /\ Range(result) \subseteq Range(sequence)
PROOF
  <1>1. ASSUME NEW sequence, NEW index,
                sequence \in Seq(Range(sequence)),
                index \in 1..Len(sequence)
         PROVE LET result == SequenceWithoutIndex(sequence, index)
               IN /\ result \in Seq(Range(sequence))
                  /\ Len(result) = Len(sequence) - 1
                  /\ DOMAIN result = 1..Len(result)
                  /\ \A resultIndex \in 1..Len(result):
                       result[resultIndex] =
                         IF resultIndex < index
                         THEN sequence[resultIndex]
                         ELSE sequence[resultIndex + 1]
                  /\ Range(result) \subseteq Range(sequence)
    <2> DEFINE Prefix == SubSeq(sequence, 1, index - 1)
    <2> DEFINE Suffix == SubSeq(sequence, index + 1, Len(sequence))
    <2> DEFINE Result == Prefix \o Suffix
    <2>1. /\ Prefix \in Seq(Range(sequence))
           /\ Len(Prefix) = index - 1
           /\ \A prefixIndex \in 1..Len(Prefix):
                Prefix[prefixIndex] = sequence[prefixIndex]
      BY <1>1, SubSeqProperties, SMT DEF Prefix
    <2>2. /\ Suffix \in Seq(Range(sequence))
           /\ Len(Suffix) = Len(sequence) - index
           /\ \A suffixIndex \in 1..Len(Suffix):
                Suffix[suffixIndex] = sequence[index + suffixIndex]
      BY <1>1, SubSeqProperties, SMT DEF Suffix
    <2>3. /\ Result \in Seq(Range(sequence))
           /\ Len(Result) = Len(sequence) - 1
           /\ DOMAIN Result = 1..Len(Result)
           /\ \A resultIndex \in 1..Len(Result):
                Result[resultIndex] =
                  IF resultIndex < index
                  THEN sequence[resultIndex]
                  ELSE sequence[resultIndex + 1]
      BY <1>1, <2>1, <2>2, ConcatProperties, SMT DEF Result
    <2>4. Range(Result) \subseteq Range(sequence)
      BY <2>3, RangeEquality, SMT
    <2> QED BY <2>3, <2>4
         DEF Result, SequenceWithoutIndex, Prefix, Suffix
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

THEOREM TypedIngressRemovalFacts ==
  \A sequence, selected:
    /\ sequence \in Seq(Range(sequence))
    /\ DOMAIN sequence = 1..Len(sequence)
    /\ \A index \in 1..Len(sequence):
         AsyncItemTyped(sequence[index])
    /\ selected \in 1..Len(sequence)
    => LET result == SequenceWithoutIndex(sequence, selected)
       IN /\ result \in Seq(Range(result))
          /\ DOMAIN result = 1..Len(result)
          /\ \A index \in 1..Len(result):
               AsyncItemTyped(result[index])
          /\ SequenceSet(result) \subseteq SequenceSet(sequence)
          /\ Len(result) + 1 = Len(sequence)
PROOF
  <1>1. ASSUME NEW sequence, NEW selected,
                sequence \in Seq(Range(sequence)),
                DOMAIN sequence = 1..Len(sequence),
                \A index \in 1..Len(sequence):
                  AsyncItemTyped(sequence[index]),
                selected \in 1..Len(sequence)
         PROVE LET result == SequenceWithoutIndex(sequence, selected)
               IN /\ result \in Seq(Range(result))
                  /\ DOMAIN result = 1..Len(result)
                  /\ \A index \in 1..Len(result):
                       AsyncItemTyped(result[index])
                  /\ SequenceSet(result) \subseteq SequenceSet(sequence)
                  /\ Len(result) + 1 = Len(sequence)
    <2> DEFINE Result == SequenceWithoutIndex(sequence, selected)
    <2>1. /\ Result \in Seq(Range(sequence))
           /\ Len(Result) = Len(sequence) - 1
           /\ DOMAIN Result = 1..Len(Result)
           /\ Range(Result) \subseteq Range(sequence)
      BY <1>1, IngressSequenceWithoutIndexFacts DEF Result
    <2>2. /\ Result \in Seq(Range(Result))
           /\ SequenceSet(Result) \subseteq SequenceSet(sequence)
      BY <1>1, <2>1, SeqOfRange, RangeEquality DEF SequenceSet
    <2>3. \A index \in 1..Len(Result):
             AsyncItemTyped(Result[index])
      <3>1. ASSUME NEW index \in 1..Len(Result)
             PROVE AsyncItemTyped(Result[index])
        <4>1. Result[index] \in Range(sequence)
          BY <2>1, <3>1, RangeEquality
        <4>2. PICK original \in 1..Len(sequence):
                 Result[index] = sequence[original]
          BY <1>1, <4>1, RangeEquality
        <4> QED BY <1>1, <4>2
      <3> QED BY <3>1
    <2>4. Len(Result) + 1 = Len(sequence)
      BY <1>1, <2>1, SMT
    <2> QED BY <2>1, <2>2, <2>3, <2>4 DEF Result
  <1> QED BY <1>1

THEOREM IngressSequenceWithoutIndexRetainsOtherValues ==
  \A sequence, selected:
    /\ sequence \in Seq(Range(sequence))
    /\ selected \in 1..Len(sequence)
    => SequenceSet(sequence) \ {sequence[selected]}
         \subseteq
       SequenceSet(SequenceWithoutIndex(sequence, selected))
PROOF
  <1>1. ASSUME NEW sequence, NEW selected,
                sequence \in Seq(Range(sequence)),
                selected \in 1..Len(sequence)
         PROVE SequenceSet(sequence) \ {sequence[selected]}
                 \subseteq
               SequenceSet(SequenceWithoutIndex(sequence, selected))
    <2> DEFINE Result == SequenceWithoutIndex(sequence, selected)
    <2>1. /\ Len(Result) = Len(sequence) - 1
           /\ DOMAIN Result = 1..Len(Result)
           /\ \A resultIndex \in 1..Len(Result):
                Result[resultIndex] =
                  IF resultIndex < selected
                  THEN sequence[resultIndex]
                  ELSE sequence[resultIndex + 1]
      BY <1>1, IngressSequenceWithoutIndexFacts DEF Result
    <2>2. ASSUME NEW value \in
                   SequenceSet(sequence) \ {sequence[selected]}
           PROVE value \in SequenceSet(Result)
      <3>1. PICK original \in 1..Len(sequence):
               value = sequence[original]
        BY <2>2 DEF SequenceSet
      <3>2. original # selected
        BY <2>2, <3>1
      <3>3. CASE original < selected
        <4>1. original \in 1..Len(Result)
          BY <1>1, <2>1, <3>1, <3>3, SMT
        <4>2. Result[original] = sequence[original]
          BY <2>1, <3>3, <4>1
        <4> QED BY <3>1, <4>1, <4>2 DEF SequenceSet
      <3>4. CASE original > selected
        <4>1. original - 1 \in 1..Len(Result)
          BY <1>1, <2>1, <3>1, <3>4, SMT
        <4>2. Result[original - 1] = sequence[original]
          BY <2>1, <3>4, <4>1, SMT
        <4> QED BY <3>1, <4>1, <4>2 DEF SequenceSet
      <3>5. original < selected \/ original > selected
        BY <3>2, SMT
      <3> QED BY <3>3, <3>4, <3>5
    <2> QED BY <2>2 DEF Result
  <1> QED BY <1>1

(***************************************************************************
Proof-only potential for a single ingress source.  The global reservation
count is the sum of the four finite protected-source sets.  Progress,
TimeoutVote, and the shared Chunk/CertifiedResponse TransportCompletion owner
are pairwise distinct.  Splitting out one source lets removal reason locally:
deleting one queued item decreases used depth by one and can add at most one
reservation for that same source.
***************************************************************************)
IngressItemIsNonTimeoutProgress(item) ==
  /\ IngressAdmissionClass(item) = "Progress"
  /\ item.kind # "TimeoutVote"

IngressItemIsTimeoutVote(item) == item.kind = "TimeoutVote"

IngressItemIsTransportCompletion(item) ==
  IngressUsesPhysicalCompletionOwner(item)

IngressSequenceHasNonTimeoutProgress(sequence) ==
  \E queued \in SequenceSet(sequence):
    IngressItemIsNonTimeoutProgress(queued)

IngressSequenceHasTimeoutVote(sequence) ==
  \E queued \in SequenceSet(sequence):
    IngressItemIsTimeoutVote(queued)

IngressSequenceHasTransportCompletion(sequence) ==
  \E queued \in SequenceSet(sequence):
    IngressItemIsTransportCompletion(queued)

IngressSourceProtectionPotential(source, sequence) ==
  IF source \notin ValidatorIds
  THEN (IF Len(sequence) = 0 THEN 1 ELSE 0)
         + (IF ~IngressSequenceHasTransportCompletion(sequence)
            THEN 1 ELSE 0)
         + (IF /\ Len(sequence) = 1
                  /\ IngressSequenceHasTransportCompletion(sequence)
            THEN 1 ELSE 0)
  ELSE (IF Len(sequence) = 0
             \/ ~IngressSequenceHasNonTimeoutProgress(sequence)
        THEN 1 ELSE 0)
         + (IF ~IngressSequenceHasTimeoutVote(sequence)
            THEN 1 ELSE 0)
         + (IF ~IngressSequenceHasTransportCompletion(sequence)
            THEN 1 ELSE 0)
         + (IF \/ Len(sequence) = 0
                  \/ /\ Len(sequence) = 1
                        /\ (IngressSequenceHasNonTimeoutProgress(sequence)
                              \/ IngressSequenceHasTimeoutVote(sequence)
                              \/ IngressSequenceHasTransportCompletion(sequence))
                  \/ /\ Len(sequence) = 2
                        /\ \/ /\ IngressSequenceHasNonTimeoutProgress(sequence)
                                   /\ IngressSequenceHasTimeoutVote(sequence)
                           \/ /\ IngressSequenceHasNonTimeoutProgress(sequence)
                                   /\ IngressSequenceHasTransportCompletion(sequence)
                           \/ /\ IngressSequenceHasTimeoutVote(sequence)
                                   /\ IngressSequenceHasTransportCompletion(sequence)
                  \/ /\ Len(sequence) = 3
                        /\ IngressSequenceHasNonTimeoutProgress(sequence)
                        /\ IngressSequenceHasTimeoutVote(sequence)
                        /\ IngressSequenceHasTransportCompletion(sequence)
            THEN 1 ELSE 0)

IngressProtectedSlotCountWithoutSourceFor(lanes, recipient, source) ==
  Cardinality(
    IngressProtectedSourcesFor(lanes, recipient) \ {source})
    + Cardinality(
        IngressTimeoutVoteProtectedSourcesFor(
          lanes, recipient) \ {source})
    + Cardinality(
        IngressTransportCompletionProtectedSourcesFor(
          lanes, recipient) \ {source})
    + Cardinality(
        IngressContinuationProtectedSourcesFor(lanes, recipient) \ {source})

THEOREM FourNaturalTermsRegroup ==
  \A first, second, third, fourth \in Nat:
    (first + second) + (third + fourth) =
      (first + third) + (second + fourth)
BY SMT

THEOREM CommonNaturalPrefixPreservesOneIncrease ==
  \A common, before, after \in Nat:
    after <= before + 1
      => common + after <= (common + before) + 1
BY SMT

THEOREM NaturalPredecessorPreservesUpperBound ==
  \A before, after, capacity \in Nat:
    /\ after + 1 = before
    /\ before <= capacity
    => after <= capacity
BY SMT

THEOREM CoupledNaturalPotentialPreservesUpperBound ==
  \A beforeDepth, afterDepth, beforeReserve, afterReserve,
      capacity \in Nat:
    /\ afterDepth + 1 = beforeDepth
    /\ afterReserve <= beforeReserve + 1
    /\ beforeDepth + beforeReserve <= capacity
    => afterDepth + afterReserve <= capacity
BY SMT

THEOREM AsyncIngressDepthForIsNatural ==
  \A lanes, recipient:
    /\ AsyncConfiguration
    /\ ModelConfiguration
    => AsyncIngressDepthFor(lanes, recipient) \in Nat
PROOF
  <1>1. ASSUME NEW lanes, NEW recipient,
                AsyncConfiguration,
                ModelConfiguration
         PROVE AsyncIngressDepthFor(lanes, recipient) \in Nat
    <2>1. /\ IsFiniteSet(AsyncIngressSources)
           /\ AsyncIngressCapacity \in Nat
      BY <1>1, AsyncIngressSourcesAreFinite
         DEF AsyncConfiguration
    <2>2. /\ IsFiniteSet(1..AsyncIngressCapacity)
           /\ IsFiniteSet(
                AsyncIngressSources \X (1..AsyncIngressCapacity))
      BY <2>1, FS_Interval, FS_Product
    <2>3. AsyncIngressPairIndicesFor(lanes, recipient)
               \subseteq
             AsyncIngressSources \X (1..AsyncIngressCapacity)
      BY Isa DEF AsyncIngressPairIndicesFor
    <2>4. IsFiniteSet(
             AsyncIngressPairIndicesFor(lanes, recipient))
      BY <2>2, <2>3, FS_Subset
    <2> QED BY <2>4, FS_CardinalityType
         DEF AsyncIngressDepthFor
  <1> QED BY <1>1

THEOREM IngressProtectedSlotCountWithoutSourceIsNatural ==
  \A lanes, recipient, source:
    /\ AsyncConfiguration
    /\ ModelConfiguration
    => IngressProtectedSlotCountWithoutSourceFor(
         lanes, recipient, source) \in Nat
PROOF
  <1>1. ASSUME NEW lanes, NEW recipient, NEW source,
                AsyncConfiguration,
                ModelConfiguration
         PROVE IngressProtectedSlotCountWithoutSourceFor(
                   lanes, recipient, source) \in Nat
    <2>1. IsFiniteSet(AsyncIngressSources)
      BY <1>1, AsyncIngressSourcesAreFinite
    <2>2. IsFiniteSet(
             IngressProtectedSourcesFor(lanes, recipient))
      BY <2>1, IngressProtectedSourcesFinite
    <2>3. IngressContinuationProtectedSourcesFor(lanes, recipient)
               \subseteq AsyncIngressSources
      BY Isa
         DEF IngressContinuationProtectedSourcesFor,
             AsyncIngressSources
    <2>4. IngressTimeoutVoteProtectedSourcesFor(lanes, recipient)
               \subseteq AsyncIngressSources
      BY Isa
         DEF IngressTimeoutVoteProtectedSourcesFor,
             AsyncIngressSources
    <2>4c. IngressTransportCompletionProtectedSourcesFor(lanes, recipient)
                \subseteq AsyncIngressSources
      BY Isa
         DEF IngressTransportCompletionProtectedSourcesFor,
             AsyncIngressSources
    <2>5. /\ IsFiniteSet(
             IngressContinuationProtectedSourcesFor(lanes, recipient))
           /\ IsFiniteSet(
                IngressTimeoutVoteProtectedSourcesFor(lanes, recipient))
           /\ IsFiniteSet(
                IngressTransportCompletionProtectedSourcesFor(
                  lanes, recipient))
      BY <2>1, <2>3, <2>4, <2>4c, FS_Subset
    <2>6. /\ IngressProtectedSourcesFor(lanes, recipient) \ {source}
                  \subseteq
                IngressProtectedSourcesFor(lanes, recipient)
           /\ IngressTimeoutVoteProtectedSourcesFor(
                lanes, recipient) \ {source}
                  \subseteq
                IngressTimeoutVoteProtectedSourcesFor(lanes, recipient)
           /\ IngressContinuationProtectedSourcesFor(
                lanes, recipient) \ {source}
                  \subseteq
                IngressContinuationProtectedSourcesFor(
                  lanes, recipient)
           /\ IngressTransportCompletionProtectedSourcesFor(
                lanes, recipient) \ {source}
                  \subseteq
                IngressTransportCompletionProtectedSourcesFor(
                  lanes, recipient)
      BY Isa
    <2>7. /\ IsFiniteSet(
                  IngressProtectedSourcesFor(
                    lanes, recipient) \ {source})
           /\ IsFiniteSet(
                  IngressTimeoutVoteProtectedSourcesFor(
                    lanes, recipient) \ {source})
           /\ IsFiniteSet(
                  IngressContinuationProtectedSourcesFor(
                    lanes, recipient) \ {source})
           /\ IsFiniteSet(
                  IngressTransportCompletionProtectedSourcesFor(
                    lanes, recipient) \ {source})
      BY <2>2, <2>5, <2>6, FS_Subset
    <2>8. /\ Cardinality(
                  IngressProtectedSourcesFor(
                    lanes, recipient) \ {source}) \in Nat
           /\ Cardinality(
                  IngressTimeoutVoteProtectedSourcesFor(
                    lanes, recipient) \ {source}) \in Nat
           /\ Cardinality(
                  IngressContinuationProtectedSourcesFor(
                    lanes, recipient) \ {source}) \in Nat
           /\ Cardinality(
                  IngressTransportCompletionProtectedSourcesFor(
                    lanes, recipient) \ {source}) \in Nat
      BY <2>7, FS_CardinalityType
    <2> QED BY <2>8, SMT
         DEF IngressProtectedSlotCountWithoutSourceFor
  <1> QED BY <1>1

THEOREM IngressSourceProtectionPotentialIsNatural ==
  \A source, sequence:
    IngressSourceProtectionPotential(source, sequence) \in Nat
BY SMT DEF IngressSourceProtectionPotential

THEOREM IngressClassPresenceAfterExactRemoval ==
  \A before, selected:
    /\ before \in Seq(Range(before))
    /\ selected \in 1..Len(before)
    => LET after == SequenceWithoutIndex(before, selected)
       IN /\ (IngressSequenceHasNonTimeoutProgress(after)
                 => IngressSequenceHasNonTimeoutProgress(before))
          /\ (IngressSequenceHasTimeoutVote(after)
                 => IngressSequenceHasTimeoutVote(before))
          /\ (IngressSequenceHasTransportCompletion(after)
                 => IngressSequenceHasTransportCompletion(before))
          /\ (~IngressItemIsNonTimeoutProgress(before[selected])
                 => (IngressSequenceHasNonTimeoutProgress(before)
                       => IngressSequenceHasNonTimeoutProgress(after)))
          /\ (~IngressItemIsTimeoutVote(before[selected])
                 => (IngressSequenceHasTimeoutVote(before)
                       => IngressSequenceHasTimeoutVote(after)))
          /\ (~IngressItemIsTransportCompletion(before[selected])
                 => (IngressSequenceHasTransportCompletion(before)
                       => IngressSequenceHasTransportCompletion(after)))
          /\ ~(IngressItemIsNonTimeoutProgress(before[selected])
                 /\ IngressItemIsTimeoutVote(before[selected]))
          /\ ~(IngressItemIsNonTimeoutProgress(before[selected])
                 /\ IngressItemIsTransportCompletion(before[selected]))
          /\ ~(IngressItemIsTimeoutVote(before[selected])
                 /\ IngressItemIsTransportCompletion(before[selected]))
PROOF
  <1>1. ASSUME NEW before, NEW selected,
                before \in Seq(Range(before)),
                selected \in 1..Len(before)
         PROVE LET after == SequenceWithoutIndex(before, selected)
               IN /\ (IngressSequenceHasNonTimeoutProgress(after)
                         => IngressSequenceHasNonTimeoutProgress(before))
                  /\ (IngressSequenceHasTimeoutVote(after)
                         => IngressSequenceHasTimeoutVote(before))
                  /\ (IngressSequenceHasTransportCompletion(after)
                         => IngressSequenceHasTransportCompletion(before))
                  /\ (~IngressItemIsNonTimeoutProgress(before[selected])
                         => (IngressSequenceHasNonTimeoutProgress(before)
                               => IngressSequenceHasNonTimeoutProgress(after)))
                  /\ (~IngressItemIsTimeoutVote(before[selected])
                         => (IngressSequenceHasTimeoutVote(before)
                               => IngressSequenceHasTimeoutVote(after)))
                  /\ (~IngressItemIsTransportCompletion(before[selected])
                         => (IngressSequenceHasTransportCompletion(before)
                               => IngressSequenceHasTransportCompletion(after)))
                  /\ ~(IngressItemIsNonTimeoutProgress(before[selected])
                         /\ IngressItemIsTimeoutVote(before[selected]))
                  /\ ~(IngressItemIsNonTimeoutProgress(before[selected])
                         /\ IngressItemIsTransportCompletion(before[selected]))
                  /\ ~(IngressItemIsTimeoutVote(before[selected])
                         /\ IngressItemIsTransportCompletion(before[selected]))
    <2> DEFINE After == SequenceWithoutIndex(before, selected)
    <2>1. SequenceSet(After) \subseteq SequenceSet(before)
      BY <1>1, IngressSequenceWithoutIndexFacts, RangeEquality
         DEF After, SequenceSet
    <2>2. SequenceSet(before) \ {before[selected]}
               \subseteq SequenceSet(After)
      BY <1>1, IngressSequenceWithoutIndexRetainsOtherValues DEF After
    <2>3. IngressSequenceHasNonTimeoutProgress(After)
             => IngressSequenceHasNonTimeoutProgress(before)
      BY <2>1, Isa
         DEF IngressSequenceHasNonTimeoutProgress
    <2>4. IngressSequenceHasTimeoutVote(After)
             => IngressSequenceHasTimeoutVote(before)
      BY <2>1, Isa DEF IngressSequenceHasTimeoutVote
    <2>4c. IngressSequenceHasTransportCompletion(After)
              => IngressSequenceHasTransportCompletion(before)
      BY <2>1, Isa DEF IngressSequenceHasTransportCompletion
    <2>5. ~IngressItemIsNonTimeoutProgress(before[selected])
             => (IngressSequenceHasNonTimeoutProgress(before)
                   => IngressSequenceHasNonTimeoutProgress(After))
      <3>1. ASSUME ~IngressItemIsNonTimeoutProgress(before[selected]),
                    IngressSequenceHasNonTimeoutProgress(before)
             PROVE IngressSequenceHasNonTimeoutProgress(After)
        <4>1. PICK queued \in SequenceSet(before):
                 IngressItemIsNonTimeoutProgress(queued)
          BY <3>1 DEF IngressSequenceHasNonTimeoutProgress
        <4>2. queued # before[selected]
          BY <3>1, <4>1
        <4>3. queued \in SequenceSet(After)
          BY <2>2, <4>1, <4>2
        <4> QED BY <4>1, <4>3
             DEF IngressSequenceHasNonTimeoutProgress
      <3> QED BY <3>1
    <2>6. ~IngressItemIsTimeoutVote(before[selected])
             => (IngressSequenceHasTimeoutVote(before)
                   => IngressSequenceHasTimeoutVote(After))
      <3>1. ASSUME ~IngressItemIsTimeoutVote(before[selected]),
                    IngressSequenceHasTimeoutVote(before)
             PROVE IngressSequenceHasTimeoutVote(After)
        <4>1. PICK queued \in SequenceSet(before):
                 IngressItemIsTimeoutVote(queued)
          BY <3>1 DEF IngressSequenceHasTimeoutVote
        <4>2. queued # before[selected]
          BY <3>1, <4>1
        <4>3. queued \in SequenceSet(After)
          BY <2>2, <4>1, <4>2
        <4> QED BY <4>1, <4>3
             DEF IngressSequenceHasTimeoutVote
      <3> QED BY <3>1
    <2>6c. ~IngressItemIsTransportCompletion(before[selected])
              => (IngressSequenceHasTransportCompletion(before)
                    => IngressSequenceHasTransportCompletion(After))
      <3>1. ASSUME ~IngressItemIsTransportCompletion(before[selected]),
                    IngressSequenceHasTransportCompletion(before)
             PROVE IngressSequenceHasTransportCompletion(After)
        <4>1. PICK queued \in SequenceSet(before):
                 IngressItemIsTransportCompletion(queued)
          BY <3>1 DEF IngressSequenceHasTransportCompletion
        <4>2. queued # before[selected]
          BY <3>1, <4>1
        <4>3. queued \in SequenceSet(After)
          BY <2>2, <4>1, <4>2
        <4> QED BY <4>1, <4>3
             DEF IngressSequenceHasTransportCompletion
      <3> QED BY <3>1
    <2>7. /\ ~(IngressItemIsNonTimeoutProgress(before[selected])
               /\ IngressItemIsTimeoutVote(before[selected]))
           /\ ~(IngressItemIsNonTimeoutProgress(before[selected])
                /\ IngressItemIsTransportCompletion(before[selected]))
           /\ ~(IngressItemIsTimeoutVote(before[selected])
                /\ IngressItemIsTransportCompletion(before[selected]))
      BY DEF IngressItemIsNonTimeoutProgress,
             IngressItemIsTimeoutVote,
             IngressItemIsTransportCompletion,
             IngressUsesPhysicalCompletionOwner,
             IngressAdmissionClass, IngressTransportCompletionKinds
    <2> QED BY <2>3, <2>4, <2>4c, <2>5, <2>6, <2>6c, <2>7 DEF After
  <1> QED BY <1>1

THEOREM OneRemovalIncreasesSourceProtectionByAtMostOne ==
  \A source, before, selected:
    /\ before \in Seq(Range(before))
    /\ selected \in 1..Len(before)
    => LET after == SequenceWithoutIndex(before, selected)
       IN IngressSourceProtectionPotential(source, after)
            <= IngressSourceProtectionPotential(source, before) + 1
PROOF
  <1>1. ASSUME NEW source, NEW before, NEW selected,
                before \in Seq(Range(before)),
                selected \in 1..Len(before)
         PROVE LET after == SequenceWithoutIndex(before, selected)
               IN IngressSourceProtectionPotential(source, after)
                    <= IngressSourceProtectionPotential(source, before) + 1
    <2> DEFINE After == SequenceWithoutIndex(before, selected)
    <2>1. /\ Len(before) \in Nat
           /\ Len(After) \in Nat
           /\ Len(After) + 1 = Len(before)
           /\ Len(before) > 0
      BY <1>1, IngressSequenceWithoutIndexFacts, LenProperties, SMT DEF After
    <2>2. /\ (IngressSequenceHasNonTimeoutProgress(After)
                 => IngressSequenceHasNonTimeoutProgress(before))
           /\ (IngressSequenceHasTimeoutVote(After)
                 => IngressSequenceHasTimeoutVote(before))
           /\ (IngressSequenceHasTransportCompletion(After)
                 => IngressSequenceHasTransportCompletion(before))
           /\ (~IngressItemIsNonTimeoutProgress(before[selected])
                 => (IngressSequenceHasNonTimeoutProgress(before)
                       => IngressSequenceHasNonTimeoutProgress(After)))
           /\ (~IngressItemIsTimeoutVote(before[selected])
                 => (IngressSequenceHasTimeoutVote(before)
                       => IngressSequenceHasTimeoutVote(After)))
           /\ (~IngressItemIsTransportCompletion(before[selected])
                 => (IngressSequenceHasTransportCompletion(before)
                       => IngressSequenceHasTransportCompletion(After)))
           /\ ~(IngressItemIsNonTimeoutProgress(before[selected])
                 /\ IngressItemIsTimeoutVote(before[selected]))
           /\ ~(IngressItemIsNonTimeoutProgress(before[selected])
                 /\ IngressItemIsTransportCompletion(before[selected]))
           /\ ~(IngressItemIsTimeoutVote(before[selected])
                 /\ IngressItemIsTransportCompletion(before[selected]))
      BY <1>1, IngressClassPresenceAfterExactRemoval DEF After
    <2>3. CASE source \notin ValidatorIds
      <3>1. CASE Len(before) = 1
        BY <1>1, <2>1, <2>3, <3>1, SMT
           DEF IngressSourceProtectionPotential
      <3>2. CASE Len(before) > 1
        BY <1>1, <2>1, <2>3, <3>2, SMT
           DEF IngressSourceProtectionPotential
      <3>3. Len(before) = 1 \/ Len(before) > 1
        BY <2>1, SMT
      <3> QED BY <3>1, <3>2, <3>3
    <2>4. CASE source \in ValidatorIds
      <3>1. CASE Len(before) = 1
        BY <2>1, <2>2, <2>4, <3>1, SMT
           DEF IngressSourceProtectionPotential
      <3>2. CASE Len(before) = 2
        BY <2>1, <2>2, <2>4, <3>2, SMT
           DEF IngressSourceProtectionPotential
      <3>3. CASE Len(before) = 3
        BY <2>1, <2>2, <2>4, <3>3, SMT
           DEF IngressSourceProtectionPotential
      <3>4. CASE Len(before) = 4
        BY <2>1, <2>2, <2>4, <3>4, SMT
           DEF IngressSourceProtectionPotential
      <3>5. CASE Len(before) > 4
        BY <2>1, <2>2, <2>4, <3>5, SMT
           DEF IngressSourceProtectionPotential
      <3>6. \/ Len(before) = 1
             \/ Len(before) = 2
             \/ Len(before) = 3
             \/ Len(before) = 4
             \/ Len(before) > 4
        BY <2>1, SMT
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5, <3>6
    <2> QED BY <2>3, <2>4 DEF After
  <1> QED BY <1>1

THEOREM IngressProtectedSlotCountDecomposesAtSource ==
  \A lanes, recipient, source:
    /\ AsyncConfiguration
    /\ ModelConfiguration
    /\ source \in AsyncIngressSources
    => IngressProtectedSlotCountFor(lanes, recipient) =
         IngressProtectedSlotCountWithoutSourceFor(
           lanes, recipient, source)
           + IngressSourceProtectionPotential(
               source, lanes[recipient][source])
PROOF
  <1>1. ASSUME NEW lanes, NEW recipient, NEW source,
                AsyncConfiguration,
                ModelConfiguration,
                source \in AsyncIngressSources
         PROVE IngressProtectedSlotCountFor(lanes, recipient) =
                 IngressProtectedSlotCountWithoutSourceFor(
                   lanes, recipient, source)
                   + IngressSourceProtectionPotential(
                       source, lanes[recipient][source])
    <2>1. /\ IsFiniteSet(AsyncIngressSources)
           /\ IsFiniteSet(ValidatorIds)
      <3>1. IsFiniteSet(AsyncIngressSources)
        BY <1>1, AsyncIngressSourcesAreFinite
      <3>2. ValidatorIds \subseteq AsyncIngressSources
        BY Isa DEF AsyncIngressSources
      <3>3. IsFiniteSet(ValidatorIds)
        BY <3>1, <3>2, FS_Subset
      <3> QED BY <3>1, <3>3
    <2>2. /\ IsFiniteSet(
                  IngressProtectedSourcesFor(lanes, recipient))
           /\ Cardinality(
                  IngressProtectedSourcesFor(lanes, recipient)) \in Nat
      BY <2>1, IngressProtectedSourcesFinite
    <2>3. IngressContinuationProtectedSourcesFor(lanes, recipient)
               \subseteq AsyncIngressSources
      BY Isa DEF IngressContinuationProtectedSourcesFor
    <2>3t. IngressTimeoutVoteProtectedSourcesFor(lanes, recipient)
                \subseteq ValidatorIds
      BY Isa DEF IngressTimeoutVoteProtectedSourcesFor
    <2>3c. IngressTransportCompletionProtectedSourcesFor(lanes, recipient)
                \subseteq AsyncIngressSources
      BY Isa DEF IngressTransportCompletionProtectedSourcesFor
    <2>4. /\ IsFiniteSet(
                  IngressContinuationProtectedSourcesFor(lanes, recipient))
           /\ Cardinality(
                  IngressContinuationProtectedSourcesFor(
                    lanes, recipient)) \in Nat
      BY <2>1, <2>3, FS_Subset, FS_CardinalityType
    <2>4t. /\ IsFiniteSet(
                   IngressTimeoutVoteProtectedSourcesFor(lanes, recipient))
            /\ Cardinality(
                   IngressTimeoutVoteProtectedSourcesFor(
                     lanes, recipient)) \in Nat
      BY <2>1, <2>3t, FS_Subset, FS_CardinalityType
    <2>4c. /\ IsFiniteSet(
                   IngressTransportCompletionProtectedSourcesFor(
                     lanes, recipient))
            /\ Cardinality(
                   IngressTransportCompletionProtectedSourcesFor(
                     lanes, recipient)) \in Nat
      BY <2>1, <2>3c, FS_Subset, FS_CardinalityType
    <2>5. Cardinality(
             IngressProtectedSourcesFor(lanes, recipient)) =
           Cardinality(
             IngressProtectedSourcesFor(lanes, recipient) \ {source})
             + IF source \in
                    IngressProtectedSourcesFor(lanes, recipient)
               THEN 1 ELSE 0
      BY <2>2, FS_RemoveElement, FS_CardinalityType, SMT
    <2>6. Cardinality(
             IngressContinuationProtectedSourcesFor(lanes, recipient)) =
           Cardinality(
             IngressContinuationProtectedSourcesFor(
               lanes, recipient) \ {source})
             + IF source \in
                    IngressContinuationProtectedSourcesFor(
                      lanes, recipient)
               THEN 1 ELSE 0
      BY <2>4, FS_RemoveElement, FS_CardinalityType, SMT
    <2>6t. Cardinality(
              IngressTimeoutVoteProtectedSourcesFor(lanes, recipient)) =
            Cardinality(
              IngressTimeoutVoteProtectedSourcesFor(
                lanes, recipient) \ {source})
              + IF source \in
                     IngressTimeoutVoteProtectedSourcesFor(
                       lanes, recipient)
                THEN 1 ELSE 0
      BY <2>4t, FS_RemoveElement, FS_CardinalityType, SMT
    <2>6c. Cardinality(
              IngressTransportCompletionProtectedSourcesFor(
                lanes, recipient)) =
            Cardinality(
              IngressTransportCompletionProtectedSourcesFor(
                lanes, recipient) \ {source})
              + IF source \in
                     IngressTransportCompletionProtectedSourcesFor(
                       lanes, recipient)
                THEN 1 ELSE 0
      BY <2>4c, FS_RemoveElement, FS_CardinalityType, SMT
    <2>7. IngressSourceProtectionPotential(
             source, lanes[recipient][source]) =
           (IF source \in IngressProtectedSourcesFor(lanes, recipient)
            THEN 1 ELSE 0)
             + (IF source \in
                      IngressTimeoutVoteProtectedSourcesFor(
                        lanes, recipient)
                THEN 1 ELSE 0)
             + (IF source \in
                      IngressTransportCompletionProtectedSourcesFor(
                        lanes, recipient)
                THEN 1 ELSE 0)
             + (IF source \in
                      IngressContinuationProtectedSourcesFor(
                        lanes, recipient)
                THEN 1 ELSE 0)
      BY <1>1, SMT
         DEF IngressSourceProtectionPotential,
             IngressSequenceHasNonTimeoutProgress,
             IngressSequenceHasTimeoutVote,
             IngressSequenceHasTransportCompletion,
             IngressItemIsNonTimeoutProgress,
             IngressItemIsTimeoutVote,
             IngressItemIsTransportCompletion,
             IngressUsesPhysicalCompletionOwner,
             IngressProtectedSourcesFor,
             IngressTimeoutVoteProtectedSourcesFor,
             IngressTransportCompletionProtectedSourcesFor,
             IngressContinuationProtectedSourcesFor,
             IngressLaneHasNonTimeoutProgressIn,
             IngressLaneHasTimeoutVoteIn,
             IngressLaneHasTransportCompletionIn
    <2>8. Cardinality(
             IngressProtectedSourcesFor(lanes, recipient))
             + Cardinality(
                 IngressTimeoutVoteProtectedSourcesFor(
                   lanes, recipient))
             + Cardinality(
                 IngressTransportCompletionProtectedSourcesFor(
                   lanes, recipient))
             + Cardinality(
                 IngressContinuationProtectedSourcesFor(
                   lanes, recipient)) =
           (Cardinality(
              IngressProtectedSourcesFor(lanes, recipient) \ {source})
              + (IF source \in
                       IngressProtectedSourcesFor(lanes, recipient)
                 THEN 1 ELSE 0))
             + (Cardinality(
                  IngressTimeoutVoteProtectedSourcesFor(
                    lanes, recipient) \ {source})
                  + (IF source \in
                           IngressTimeoutVoteProtectedSourcesFor(
                             lanes, recipient)
                     THEN 1 ELSE 0))
             + (Cardinality(
                  IngressTransportCompletionProtectedSourcesFor(
                    lanes, recipient) \ {source})
                  + (IF source \in
                           IngressTransportCompletionProtectedSourcesFor(
                             lanes, recipient)
                     THEN 1 ELSE 0))
             + (Cardinality(
                  IngressContinuationProtectedSourcesFor(
                    lanes, recipient) \ {source})
                  + (IF source \in
                           IngressContinuationProtectedSourcesFor(
                             lanes, recipient)
                     THEN 1 ELSE 0))
      BY <2>5, <2>6, <2>6t, <2>6c
    <2>9. /\ Cardinality(
                  IngressProtectedSourcesFor(
                    lanes, recipient) \ {source}) \in Nat
           /\ Cardinality(
                  IngressTimeoutVoteProtectedSourcesFor(
                    lanes, recipient) \ {source}) \in Nat
           /\ Cardinality(
                  IngressTransportCompletionProtectedSourcesFor(
                    lanes, recipient) \ {source}) \in Nat
           /\ Cardinality(
                  IngressContinuationProtectedSourcesFor(
                    lanes, recipient) \ {source}) \in Nat
           /\ (IF source \in
                    IngressProtectedSourcesFor(lanes, recipient)
               THEN 1 ELSE 0) \in Nat
           /\ (IF source \in
                    IngressTimeoutVoteProtectedSourcesFor(
                      lanes, recipient)
               THEN 1 ELSE 0) \in Nat
           /\ (IF source \in
                    IngressTransportCompletionProtectedSourcesFor(
                      lanes, recipient)
               THEN 1 ELSE 0) \in Nat
           /\ (IF source \in
                    IngressContinuationProtectedSourcesFor(
                      lanes, recipient)
               THEN 1 ELSE 0) \in Nat
      <3>1. IngressProtectedSourcesFor(
               lanes, recipient) \ {source} \subseteq
             IngressProtectedSourcesFor(lanes, recipient)
        BY Isa
      <3>2. IngressContinuationProtectedSourcesFor(
               lanes, recipient) \ {source} \subseteq
             IngressContinuationProtectedSourcesFor(lanes, recipient)
        BY Isa
      <3>2t. IngressTimeoutVoteProtectedSourcesFor(
                lanes, recipient) \ {source} \subseteq
              IngressTimeoutVoteProtectedSourcesFor(lanes, recipient)
        BY Isa
      <3>2c. IngressTransportCompletionProtectedSourcesFor(
                lanes, recipient) \ {source} \subseteq
              IngressTransportCompletionProtectedSourcesFor(
                lanes, recipient)
        BY Isa
      <3>3. /\ IsFiniteSet(
                    IngressProtectedSourcesFor(
                      lanes, recipient) \ {source})
             /\ IsFiniteSet(
                    IngressTimeoutVoteProtectedSourcesFor(
                      lanes, recipient) \ {source})
             /\ IsFiniteSet(
                    IngressTransportCompletionProtectedSourcesFor(
                      lanes, recipient) \ {source})
             /\ IsFiniteSet(
                    IngressContinuationProtectedSourcesFor(
                      lanes, recipient) \ {source})
        BY <2>2, <2>4, <2>4t, <2>4c,
           <3>1, <3>2, <3>2t, <3>2c, FS_Subset
      <3> QED BY <3>3, FS_CardinalityType, SMT
    <2>10. (Cardinality(
               IngressProtectedSourcesFor(lanes, recipient) \ {source})
               + (IF source \in
                        IngressProtectedSourcesFor(lanes, recipient)
                  THEN 1 ELSE 0))
              + (Cardinality(
                   IngressTimeoutVoteProtectedSourcesFor(
                     lanes, recipient) \ {source})
                   + (IF source \in
                            IngressTimeoutVoteProtectedSourcesFor(
                              lanes, recipient)
                      THEN 1 ELSE 0))
              + (Cardinality(
                   IngressTransportCompletionProtectedSourcesFor(
                     lanes, recipient) \ {source})
                   + (IF source \in
                            IngressTransportCompletionProtectedSourcesFor(
                              lanes, recipient)
                      THEN 1 ELSE 0))
              + (Cardinality(
                   IngressContinuationProtectedSourcesFor(
                     lanes, recipient) \ {source})
                   + (IF source \in
                            IngressContinuationProtectedSourcesFor(
                              lanes, recipient)
                      THEN 1 ELSE 0)) =
            (Cardinality(
               IngressProtectedSourcesFor(lanes, recipient) \ {source})
               + Cardinality(
                   IngressTimeoutVoteProtectedSourcesFor(
                     lanes, recipient) \ {source})
               + Cardinality(
                   IngressTransportCompletionProtectedSourcesFor(
                     lanes, recipient) \ {source})
               + Cardinality(
                   IngressContinuationProtectedSourcesFor(
                     lanes, recipient) \ {source}))
              + ((IF source \in
                         IngressProtectedSourcesFor(lanes, recipient)
                    THEN 1 ELSE 0)
                 + (IF source \in
                          IngressTimeoutVoteProtectedSourcesFor(
                            lanes, recipient)
                    THEN 1 ELSE 0)
                 + (IF source \in
                          IngressTransportCompletionProtectedSourcesFor(
                            lanes, recipient)
                    THEN 1 ELSE 0)
                 + (IF source \in
                          IngressContinuationProtectedSourcesFor(
                            lanes, recipient)
                    THEN 1 ELSE 0))
      BY <2>9, SMT
    <2>11. (Cardinality(
               IngressProtectedSourcesFor(lanes, recipient) \ {source})
               + Cardinality(
                   IngressTimeoutVoteProtectedSourcesFor(
                     lanes, recipient) \ {source})
               + Cardinality(
                   IngressTransportCompletionProtectedSourcesFor(
                     lanes, recipient) \ {source})
               + Cardinality(
                   IngressContinuationProtectedSourcesFor(
                     lanes, recipient) \ {source}))
              + ((IF source \in
                         IngressProtectedSourcesFor(lanes, recipient)
                    THEN 1 ELSE 0)
                 + (IF source \in
                          IngressTimeoutVoteProtectedSourcesFor(
                            lanes, recipient)
                    THEN 1 ELSE 0)
                 + (IF source \in
                          IngressTransportCompletionProtectedSourcesFor(
                            lanes, recipient)
                    THEN 1 ELSE 0)
                 + (IF source \in
                          IngressContinuationProtectedSourcesFor(
                            lanes, recipient)
                    THEN 1 ELSE 0)) =
            IngressProtectedSlotCountWithoutSourceFor(
              lanes, recipient, source)
              + IngressSourceProtectionPotential(
                  source, lanes[recipient][source])
      BY <2>7
         DEF IngressProtectedSlotCountWithoutSourceFor
    <2> QED BY <2>8, <2>10, <2>11
         DEF IngressProtectedSlotCountFor
  <1> QED BY <1>1

THEOREM IngressProtectedSlotsWithoutSourceAreLocal ==
  \A beforeLanes, afterLanes, recipient, source:
    (\A otherSource \in AsyncIngressSources \ {source}:
       afterLanes[recipient][otherSource] =
         beforeLanes[recipient][otherSource])
    => IngressProtectedSlotCountWithoutSourceFor(
         afterLanes, recipient, source) =
       IngressProtectedSlotCountWithoutSourceFor(
         beforeLanes, recipient, source)
PROOF
  <1>1. ASSUME NEW beforeLanes, NEW afterLanes,
                NEW recipient, NEW source,
                \A otherSource \in AsyncIngressSources \ {source}:
                  afterLanes[recipient][otherSource] =
                    beforeLanes[recipient][otherSource]
         PROVE IngressProtectedSlotCountWithoutSourceFor(
                   afterLanes, recipient, source) =
                 IngressProtectedSlotCountWithoutSourceFor(
                   beforeLanes, recipient, source)
    <2>1. \A candidate:
             candidate \in
               IngressProtectedSourcesFor(
                 afterLanes, recipient) \ {source}
             <=> candidate \in
               IngressProtectedSourcesFor(
                 beforeLanes, recipient) \ {source}
      <3>1. ASSUME NEW candidate
             PROVE candidate \in
                     IngressProtectedSourcesFor(
                       afterLanes, recipient) \ {source}
                   <=> candidate \in
                     IngressProtectedSourcesFor(
                       beforeLanes, recipient) \ {source}
        <4>1. CASE candidate \in AsyncIngressSources \ {source}
          <5>1. afterLanes[recipient][candidate] =
                   beforeLanes[recipient][candidate]
            BY <1>1, <4>1
          <5> QED BY <4>1, <5>1, Isa
               DEF IngressProtectedSourcesFor,
                   IngressLaneHasNonTimeoutProgressIn,
                   IngressLaneHasTimeoutVoteIn, SequenceSet
        <4>2. CASE candidate \notin AsyncIngressSources \ {source}
          BY <4>2, Isa DEF IngressProtectedSourcesFor
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>2. IngressProtectedSourcesFor(afterLanes, recipient) \ {source} =
             IngressProtectedSourcesFor(beforeLanes, recipient) \ {source}
      BY <2>1, Isa
    <2>3. ValidatorIds \subseteq AsyncIngressSources
      BY Isa DEF AsyncIngressSources
    <2>4. \A candidate:
             candidate \in
               IngressContinuationProtectedSourcesFor(
                 afterLanes, recipient) \ {source}
             <=> candidate \in
               IngressContinuationProtectedSourcesFor(
                 beforeLanes, recipient) \ {source}
      <3>1. ASSUME NEW candidate
             PROVE candidate \in
                     IngressContinuationProtectedSourcesFor(
                       afterLanes, recipient) \ {source}
                   <=> candidate \in
                     IngressContinuationProtectedSourcesFor(
                       beforeLanes, recipient) \ {source}
        <4>1. CASE candidate \in AsyncIngressSources \ {source}
          <5>1. afterLanes[recipient][candidate] =
                   beforeLanes[recipient][candidate]
            BY <1>1, <4>1
          <5> QED BY <4>1, <5>1, Isa
               DEF IngressContinuationProtectedSourcesFor,
                   IngressLaneHasNonTimeoutProgressIn,
                   IngressLaneHasTimeoutVoteIn,
                   IngressLaneHasTransportCompletionIn, SequenceSet
        <4>2. CASE candidate \notin AsyncIngressSources \ {source}
          BY <2>3, <4>2, Isa
             DEF IngressContinuationProtectedSourcesFor
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>5. IngressContinuationProtectedSourcesFor(
             afterLanes, recipient) \ {source} =
           IngressContinuationProtectedSourcesFor(
             beforeLanes, recipient) \ {source}
      BY <2>4, Isa
    <2>6. \A candidate:
             candidate \in
               IngressTimeoutVoteProtectedSourcesFor(
                 afterLanes, recipient) \ {source}
             <=> candidate \in
               IngressTimeoutVoteProtectedSourcesFor(
                 beforeLanes, recipient) \ {source}
      <3>1. ASSUME NEW candidate
             PROVE candidate \in
                     IngressTimeoutVoteProtectedSourcesFor(
                       afterLanes, recipient) \ {source}
                   <=> candidate \in
                     IngressTimeoutVoteProtectedSourcesFor(
                       beforeLanes, recipient) \ {source}
        <4>1. CASE candidate \in AsyncIngressSources \ {source}
          <5>1. afterLanes[recipient][candidate] =
                   beforeLanes[recipient][candidate]
            BY <1>1, <4>1
          <5> QED BY <4>1, <5>1, Isa
               DEF IngressTimeoutVoteProtectedSourcesFor,
                   IngressLaneHasTimeoutVoteIn, SequenceSet
        <4>2. CASE candidate \notin AsyncIngressSources \ {source}
          BY <2>3, <4>2, Isa
             DEF IngressTimeoutVoteProtectedSourcesFor
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>7. IngressTimeoutVoteProtectedSourcesFor(
             afterLanes, recipient) \ {source} =
           IngressTimeoutVoteProtectedSourcesFor(
             beforeLanes, recipient) \ {source}
      BY <2>6, Isa
    <2>8. \A candidate:
             candidate \in
               IngressTransportCompletionProtectedSourcesFor(
                 afterLanes, recipient) \ {source}
             <=> candidate \in
               IngressTransportCompletionProtectedSourcesFor(
                 beforeLanes, recipient) \ {source}
      <3>1. ASSUME NEW candidate
             PROVE candidate \in
                     IngressTransportCompletionProtectedSourcesFor(
                       afterLanes, recipient) \ {source}
                   <=> candidate \in
                     IngressTransportCompletionProtectedSourcesFor(
                       beforeLanes, recipient) \ {source}
        <4>1. CASE candidate \in AsyncIngressSources \ {source}
          <5>1. afterLanes[recipient][candidate] =
                   beforeLanes[recipient][candidate]
            BY <1>1, <4>1
          <5> QED BY <4>1, <5>1, Isa
               DEF IngressTransportCompletionProtectedSourcesFor,
                   IngressLaneHasTransportCompletionIn, SequenceSet
        <4>2. CASE candidate \notin AsyncIngressSources \ {source}
          BY <2>3, <4>2, Isa
             DEF IngressTransportCompletionProtectedSourcesFor
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>9. IngressTransportCompletionProtectedSourcesFor(
             afterLanes, recipient) \ {source} =
           IngressTransportCompletionProtectedSourcesFor(
             beforeLanes, recipient) \ {source}
      BY <2>8, Isa
    <2> QED BY <2>2, <2>5, <2>7, <2>9
         DEF IngressProtectedSlotCountWithoutSourceFor
  <1> QED BY <1>1

THEOREM IngressPairIndicesAfterOneLaneRemoval ==
  \A beforeLanes, afterLanes, recipient, source:
    /\ source \in AsyncIngressSources
    /\ AsyncIngressCapacity \in Nat
    /\ Len(beforeLanes[recipient][source]) \in Nat
    /\ Len(afterLanes[recipient][source]) \in Nat
    /\ Len(afterLanes[recipient][source]) + 1 =
         Len(beforeLanes[recipient][source])
    /\ Len(beforeLanes[recipient][source]) <= AsyncIngressCapacity
    /\ \A otherSource \in AsyncIngressSources \ {source}:
         Len(afterLanes[recipient][otherSource]) =
           Len(beforeLanes[recipient][otherSource])
    => AsyncIngressPairIndicesFor(afterLanes, recipient) =
         AsyncIngressPairIndicesFor(beforeLanes, recipient)
           \ {<<source, Len(beforeLanes[recipient][source])>>}
BY SMT
   DEF AsyncIngressPairIndicesFor

THEOREM IngressDepthDropsByOneAfterLaneRemoval ==
  \A beforeLanes, afterLanes, recipient, source:
    /\ IsFiniteSet(AsyncIngressSources)
    /\ source \in AsyncIngressSources
    /\ AsyncIngressCapacity \in Nat
    /\ Len(beforeLanes[recipient][source]) \in Nat
    /\ Len(afterLanes[recipient][source]) \in Nat
    /\ Len(afterLanes[recipient][source]) + 1 =
         Len(beforeLanes[recipient][source])
    /\ Len(beforeLanes[recipient][source]) <= AsyncIngressCapacity
    /\ \A otherSource \in AsyncIngressSources \ {source}:
         Len(afterLanes[recipient][otherSource]) =
           Len(beforeLanes[recipient][otherSource])
    => AsyncIngressDepthFor(afterLanes, recipient) + 1 =
         AsyncIngressDepthFor(beforeLanes, recipient)
PROOF
  <1>1. ASSUME NEW beforeLanes, NEW afterLanes,
                NEW recipient, NEW source,
                IsFiniteSet(AsyncIngressSources),
                source \in AsyncIngressSources,
                AsyncIngressCapacity \in Nat,
                Len(beforeLanes[recipient][source]) \in Nat,
                Len(afterLanes[recipient][source]) \in Nat,
                Len(afterLanes[recipient][source]) + 1 =
                  Len(beforeLanes[recipient][source]),
                Len(beforeLanes[recipient][source]) <=
                  AsyncIngressCapacity,
                \A otherSource \in AsyncIngressSources \ {source}:
                  Len(afterLanes[recipient][otherSource]) =
                    Len(beforeLanes[recipient][otherSource])
         PROVE AsyncIngressDepthFor(afterLanes, recipient) + 1 =
                 AsyncIngressDepthFor(beforeLanes, recipient)
    <2>1. AsyncIngressPairIndicesFor(afterLanes, recipient) =
             AsyncIngressPairIndicesFor(beforeLanes, recipient)
               \ {<<source, Len(beforeLanes[recipient][source])>>}
      BY <1>1, IngressPairIndicesAfterOneLaneRemoval
    <2>2. <<source, Len(beforeLanes[recipient][source])>>
               \in AsyncIngressPairIndicesFor(beforeLanes, recipient)
      BY <1>1, SMT DEF AsyncIngressPairIndicesFor
    <2>3. IsFiniteSet(
             AsyncIngressPairIndicesFor(beforeLanes, recipient))
      <3>1. /\ IsFiniteSet(1..AsyncIngressCapacity)
             /\ IsFiniteSet(
                  AsyncIngressSources \X (1..AsyncIngressCapacity))
        BY <1>1, FS_Interval, FS_Product
      <3> QED BY <3>1, FS_Subset
           DEF AsyncIngressPairIndicesFor
    <2>4. Cardinality(
             AsyncIngressPairIndicesFor(afterLanes, recipient)) + 1 =
           Cardinality(
             AsyncIngressPairIndicesFor(beforeLanes, recipient))
      BY <2>1, <2>2, <2>3, FS_RemoveElement,
         FS_CardinalityType, SMT
    <2> QED BY <2>4 DEF AsyncIngressDepthFor
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
      <3>1. /\ Rotated \in Seq(Range(sequence))
             /\ Len(Rotated) = Len(Suffix) + Len(Prefix)
        BY <2>2, ConcatProperties DEF Rotated
      <3>2. Len(Rotated) = Len(sequence) - 1
        BY <1>1, <2>1, <2>2, <3>1, SMT
      <3>3. IsInjective(Rotated)
        BY <2>2, <2>3, ConcatInjectiveSeq DEF Rotated
      <3> QED BY <3>1, <3>2, <3>3
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
        <4>1. ASSUME NEW value \in
                        Range(sequence) \ {sequence[selected]}
               PROVE value \in Range(Rotated)
          <5>1. PICK original \in 1..Len(sequence):
                   value = sequence[original]
            BY <1>1, <4>1, RangeEquality
          <5>2. original # selected
            BY <4>1, <5>1
          <5>3. CASE original < selected
            <6>1. /\ original \in 1..Len(Prefix)
                   /\ Prefix[original] = sequence[original]
              BY <1>1, <2>2, <5>1, <5>3, SMT
            <6>2. value \in Range(Prefix)
              BY <2>2, <5>1, <6>1, RangeEquality
            <6> QED BY <3>1, <6>2
          <5>4. CASE original > selected
            <6>1. /\ original - selected \in 1..Len(Suffix)
                   /\ Suffix[original - selected] =
                        sequence[original]
              BY <1>1, <2>2, <5>1, <5>4, SMT
            <6>2. value \in Range(Suffix)
              BY <2>2, <5>1, <6>1, RangeEquality
            <6> QED BY <3>1, <6>2
          <5> QED BY <2>1, <5>1, <5>2, <5>3, <5>4, SMT
        <4> QED BY <4>1
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
    <2>8. Len(Rotated) + 1 = Len(sequence)
      BY <1>1, <2>1, <2>4, SMT
    <2> QED BY <2>4, <2>6, <2>7, <2>8
         DEF Suffix, Prefix, Rotated
  <1> QED BY <1>1

THEOREM PopSelectedIngressPreservesContentType ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      \A laneIndex \in
           1..Len(IngressLane(node, asyncIngressReady[node][index])):
        AsyncIngressTypeInvariant
          /\ PopSelectedIngress(node, index, laneIndex)
          => AsyncIngressContentTypeInvariant'
BY TypedIngressRemovalFacts, SMTT(30)
   DEF AsyncIngressTypeInvariant, AsyncIngressTopologyTypeInvariant,
       AsyncIngressContentTypeInvariant, PopSelectedIngress,
       IngressLaneDepth, IngressLane, SequenceSet

THEOREM SetRestoreRemovedMember ==
  \A set, member:
    member \in set => (set \ {member}) \cup {member} = set
BY Isa

THEOREM FunctionalReplaceUpdateAtKey ==
  \A mapping, key, value:
    key \in DOMAIN mapping
      => [mapping EXCEPT ![key] = value][key] = value
BY Isa

THEOREM FunctionalReplacePreservesDomain ==
  \A mapping, key, value:
    key \in DOMAIN mapping
      => DOMAIN [mapping EXCEPT ![key] = value] = DOMAIN mapping
BY Isa

THEOREM NestedFunctionalReplaceFacts ==
  \A mapping, outerKey, innerKey, value, outerDomain, innerDomain:
    /\ DOMAIN mapping = outerDomain
    /\ outerKey \in outerDomain
    /\ \A candidateOuter \in outerDomain:
         DOMAIN mapping[candidateOuter] = innerDomain
    /\ innerKey \in innerDomain
    => LET inner ==
             [mapping[outerKey] EXCEPT ![innerKey] = value]
           next ==
             [mapping EXCEPT ![outerKey] = inner]
       IN /\ DOMAIN next = outerDomain
          /\ \A candidateOuter \in outerDomain:
               DOMAIN next[candidateOuter] = innerDomain
          /\ \A candidateOuter \in outerDomain,
                candidateInner \in innerDomain:
               IF candidateOuter = outerKey
                    /\ candidateInner = innerKey
               THEN next[candidateOuter][candidateInner] = value
               ELSE next[candidateOuter][candidateInner] =
                      mapping[candidateOuter][candidateInner]
PROOF
  <1>1. ASSUME NEW mapping, NEW outerKey, NEW innerKey,
                NEW value, NEW outerDomain, NEW innerDomain,
                DOMAIN mapping = outerDomain,
                outerKey \in outerDomain,
                \A candidateOuter \in outerDomain:
                  DOMAIN mapping[candidateOuter] = innerDomain,
                innerKey \in innerDomain
         PROVE LET inner ==
                       [mapping[outerKey] EXCEPT ![innerKey] = value]
                   next ==
                       [mapping EXCEPT ![outerKey] = inner]
               IN /\ DOMAIN next = outerDomain
                  /\ \A candidateOuter \in outerDomain:
                       DOMAIN next[candidateOuter] = innerDomain
                  /\ \A candidateOuter \in outerDomain,
                        candidateInner \in innerDomain:
                       IF candidateOuter = outerKey
                            /\ candidateInner = innerKey
                       THEN next[candidateOuter][candidateInner] = value
                       ELSE next[candidateOuter][candidateInner] =
                              mapping[candidateOuter][candidateInner]
    <2> DEFINE Inner ==
           [mapping[outerKey] EXCEPT ![innerKey] = value]
    <2> DEFINE Updated ==
           [mapping EXCEPT ![outerKey] = Inner]
    <2>1. /\ DOMAIN Inner = innerDomain
           /\ Inner[innerKey] = value
      BY <1>1, FunctionalReplacePreservesDomain,
         FunctionalReplaceUpdateAtKey DEF Inner
    <2>2. /\ DOMAIN Updated = outerDomain
           /\ Updated[outerKey] = Inner
      BY <1>1, FunctionalReplacePreservesDomain,
         FunctionalReplaceUpdateAtKey DEF Updated
    <2>3. \A candidateOuter \in outerDomain:
             DOMAIN Updated[candidateOuter] = innerDomain
      <3>1. ASSUME NEW candidateOuter \in outerDomain
             PROVE DOMAIN Updated[candidateOuter] = innerDomain
        <4>1. CASE candidateOuter = outerKey
          BY <2>1, <2>2, <4>1
        <4>2. CASE candidateOuter # outerKey
          <5>1. Updated[candidateOuter] = mapping[candidateOuter]
            BY <1>1, <3>1, <4>2,
               FunctionalUpdateAwayFromKey DEF Updated
          <5> QED BY <1>1, <3>1, <5>1
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>4. \A candidateOuter \in outerDomain,
             candidateInner \in innerDomain:
           IF candidateOuter = outerKey
                /\ candidateInner = innerKey
           THEN Updated[candidateOuter][candidateInner] = value
           ELSE Updated[candidateOuter][candidateInner] =
                  mapping[candidateOuter][candidateInner]
      <3>1. ASSUME NEW candidateOuter \in outerDomain,
                     NEW candidateInner \in innerDomain
             PROVE IF candidateOuter = outerKey
                          /\ candidateInner = innerKey
                   THEN Updated[candidateOuter][candidateInner] = value
                   ELSE Updated[candidateOuter][candidateInner] =
                          mapping[candidateOuter][candidateInner]
        <4>1. CASE candidateOuter = outerKey
                       /\ candidateInner = innerKey
          BY <2>1, <2>2, <4>1
        <4>2. CASE candidateOuter = outerKey
                       /\ candidateInner # innerKey
          <5>1. Inner[candidateInner] =
                   mapping[outerKey][candidateInner]
            BY <1>1, <3>1, <4>2,
               FunctionalUpdateAwayFromKey DEF Inner
          <5> QED BY <2>2, <4>2, <5>1
        <4>3. CASE candidateOuter # outerKey
          <5>1. Updated[candidateOuter] = mapping[candidateOuter]
            BY <1>1, <3>1, <4>3,
               FunctionalUpdateAwayFromKey DEF Updated
          <5> QED BY <4>3, <5>1
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>1
    <2> QED BY <2>2, <2>3, <2>4 DEF Inner, Updated
  <1> QED BY <1>1

THEOREM IngressTopologyReadySetEquality ==
  AsyncIngressTopologyTypeInvariant
    => \A recipient \in ValidatorIds:
         SequenceSet(asyncIngressReady[recipient]) =
           AsyncIngressNonemptySourcesFor(
             asyncIngressLanes, recipient)
BY Isa
   DEF AsyncIngressTopologyTypeInvariant,
       AsyncIngressNonemptySourcesFor,
       IngressLaneDepth, IngressLane

THEOREM ReadyAfterSelectedDrainFacts ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      AsyncIngressTopologyTypeInvariant
        => LET ready == asyncIngressReady[node]
               source == ready[index]
               lane == IngressLane(node, source)
               rotated ==
                 SubSeq(ready, index + 1, Len(ready))
                   \o SubSeq(ready, 1, index - 1)
               next ==
                 IF Len(lane) = 1
                 THEN rotated
                 ELSE Append(rotated, source)
           IN /\ source \in AsyncIngressSources
              /\ next \in Seq(Range(next))
              /\ DOMAIN next = 1..Len(next)
              /\ SequenceSet(next) \subseteq AsyncIngressSources
              /\ Len(next) = Cardinality(SequenceSet(next))
              /\ SequenceSet(next) =
                   IF Len(lane) = 1
                   THEN SequenceSet(ready) \ {source}
                   ELSE SequenceSet(ready)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW index \in 1..Len(asyncIngressReady[node]),
                AsyncIngressTopologyTypeInvariant
         PROVE LET ready == asyncIngressReady[node]
                   source == ready[index]
                   lane == IngressLane(node, source)
                   rotated ==
                     SubSeq(ready, index + 1, Len(ready))
                       \o SubSeq(ready, 1, index - 1)
                   next ==
                     IF Len(lane) = 1
                     THEN rotated
                     ELSE Append(rotated, source)
               IN /\ source \in AsyncIngressSources
                  /\ next \in Seq(Range(next))
                  /\ DOMAIN next = 1..Len(next)
                  /\ SequenceSet(next) \subseteq AsyncIngressSources
                  /\ Len(next) = Cardinality(SequenceSet(next))
                  /\ SequenceSet(next) =
                       IF Len(lane) = 1
                       THEN SequenceSet(ready) \ {source}
                       ELSE SequenceSet(ready)
    <2> DEFINE Ready == asyncIngressReady[node]
    <2> DEFINE Source == Ready[index]
    <2> DEFINE Lane == IngressLane(node, Source)
    <2> DEFINE Rotated ==
           SubSeq(Ready, index + 1, Len(Ready))
             \o SubSeq(Ready, 1, index - 1)
    <2> DEFINE ReadyNext ==
           IF Len(Lane) = 1
           THEN Rotated
           ELSE Append(Rotated, Source)
    <2>1. /\ Ready \in Seq(Range(Ready))
           /\ DOMAIN Ready = 1..Len(Ready)
           /\ SequenceSet(Ready) \subseteq AsyncIngressSources
           /\ Len(Ready) = Cardinality(SequenceSet(Ready))
      BY <1>1 DEF AsyncIngressTopologyTypeInvariant, Ready
    <2>2. /\ Source \in SequenceSet(Ready)
           /\ Source \in AsyncIngressSources
      BY <1>1, <2>1, RangeEquality
         DEF Source, SequenceSet
    <2>3. /\ Rotated \in Seq(Range(Rotated))
           /\ IsInjective(Rotated)
           /\ SequenceSet(Rotated) =
                SequenceSet(Ready) \ {Source}
           /\ Source \notin SequenceSet(Rotated)
           /\ Len(Rotated) =
                Cardinality(SequenceSet(Rotated))
      BY <1>1, <2>1, SelectedSequenceRotationFacts DEF Rotated
    <2>4. CASE Len(Lane) = 1
      <3>1. ReadyNext = Rotated
        BY <2>4 DEF ReadyNext
      <3> QED BY <2>1, <2>2, <2>3, <3>1, LenProperties
    <2>5. CASE Len(Lane) # 1
      <3>1. /\ ReadyNext = Append(Rotated, Source)
             /\ SequenceSet(ReadyNext) =
                  SequenceSet(Rotated) \cup {Source}
        BY <2>3, <2>5, SequenceSetAfterAppend
           DEF ReadyNext
      <3>2. SequenceSet(ReadyNext) = SequenceSet(Ready)
        BY <2>2, <2>3, <3>1, SetRestoreRemovedMember
      <3>3. SequenceSet(ReadyNext) \subseteq AsyncIngressSources
        BY <2>1, <3>2
      <3>4. /\ Rotated \in Seq(AsyncIngressSources)
             /\ Source \notin Range(Rotated)
        BY <2>1, <2>2, <2>3, RangeEquality
           DEF SequenceSet
      <3>5. /\ ReadyNext \in Seq(Range(ReadyNext))
             /\ DOMAIN ReadyNext = 1..Len(ReadyNext)
             /\ IsInjective(ReadyNext)
        BY <2>2, <2>3, <3>1, <3>4,
           AppendInjectiveSeq, AppendSequenceFacts,
           SeqOfRange, LenProperties
      <3>6. Len(ReadyNext) =
               Cardinality(SequenceSet(ReadyNext))
        BY <3>5, InjectiveSequenceLengthMatchesSetCardinality
      <3> QED BY <2>2, <2>5, <3>2, <3>3, <3>5, <3>6
    <2> QED BY <2>4, <2>5
         DEF Ready, Source, Lane, Rotated, ReadyNext
  <1> QED BY <1>1

THEOREM IngressNonemptySourcesAfterOneLaneRemoval ==
  \A beforeLanes, afterLanes, node, source:
    /\ node \in ValidatorIds
    /\ source \in AsyncIngressSources
    /\ Len(beforeLanes[node][source]) \in Nat
    /\ Len(afterLanes[node][source]) \in Nat
    /\ Len(afterLanes[node][source]) + 1 =
         Len(beforeLanes[node][source])
    /\ \A recipient \in ValidatorIds,
          candidate \in AsyncIngressSources:
         IF recipient = node /\ candidate = source
         THEN afterLanes[recipient][candidate] =
                afterLanes[node][source]
         ELSE afterLanes[recipient][candidate] =
                beforeLanes[recipient][candidate]
    => /\ AsyncIngressNonemptySourcesFor(afterLanes, node) =
             (IF Len(beforeLanes[node][source]) = 1
              THEN AsyncIngressNonemptySourcesFor(beforeLanes, node)
                     \ {source}
              ELSE AsyncIngressNonemptySourcesFor(beforeLanes, node))
       /\ \A recipient \in ValidatorIds \ {node}:
            AsyncIngressNonemptySourcesFor(afterLanes, recipient) =
              AsyncIngressNonemptySourcesFor(beforeLanes, recipient)
BY SMT
   DEF AsyncIngressNonemptySourcesFor

THEOREM PopSelectedIngressPreservesTopologyType ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      \A laneIndex \in
           1..Len(IngressLane(node, asyncIngressReady[node][index])):
        AsyncIngressTypeInvariant
          /\ PopSelectedIngress(node, index, laneIndex)
          => AsyncIngressTopologyTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW index \in 1..Len(asyncIngressReady[node]),
                NEW laneIndex \in
                  1..Len(IngressLane(
                    node, asyncIngressReady[node][index])),
                AsyncIngressTypeInvariant,
                PopSelectedIngress(node, index, laneIndex)
         PROVE AsyncIngressTopologyTypeInvariant'
    <2> DEFINE Source == asyncIngressReady[node][index]
    <2> DEFINE BeforeLane == IngressLane(node, Source)
    <2> DEFINE AfterLane ==
           SequenceWithoutIndex(BeforeLane, laneIndex)
    <2> DEFINE NextReady == ReadyAfterSelectedDrain(node, index)
    <2>1. /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressContentTypeInvariant
      BY <1>1 DEF AsyncIngressTypeInvariant
    <2>2. /\ Source \in AsyncIngressSources
           /\ BeforeLane \in Seq(Range(BeforeLane))
           /\ DOMAIN BeforeLane = 1..Len(BeforeLane)
           /\ \A itemIndex \in 1..Len(BeforeLane):
                AsyncItemTyped(BeforeLane[itemIndex])
      BY <1>1, <2>1, ReadyAfterSelectedDrainFacts
         DEF Source, BeforeLane,
             AsyncIngressContentTypeInvariant, IngressLaneDepth
    <2>3. /\ AfterLane \in Seq(Range(AfterLane))
           /\ DOMAIN AfterLane = 1..Len(AfterLane)
           /\ Len(AfterLane) + 1 = Len(BeforeLane)
      BY <1>1, <2>2, TypedIngressRemovalFacts DEF AfterLane
    <2>4. /\ Source = asyncIngressReady[node][index]
           /\ BeforeLane = asyncIngressLanes[node][Source]
           /\ AfterLane =
                SequenceWithoutIndex(
                  asyncIngressLanes[node][Source], laneIndex)
           /\ NextReady = ReadyAfterSelectedDrain(node, index)
      BY DEF Source, BeforeLane, AfterLane, NextReady, IngressLane
    <2>5. /\ asyncIngressLanes' =
                  [asyncIngressLanes EXCEPT
                     ![node][Source] = AfterLane]
           /\ asyncIngressReady' =
                  [asyncIngressReady EXCEPT
                     ![node] = NextReady]
      BY <1>1, <2>4, Isa DEF PopSelectedIngress
    <2>6. LET updated ==
                   [asyncIngressLanes EXCEPT
                      ![node][Source] = AfterLane]
           IN /\ DOMAIN updated = ValidatorIds
              /\ \A recipient \in ValidatorIds:
                   DOMAIN updated[recipient] = AsyncIngressSources
              /\ \A recipient \in ValidatorIds,
                    candidate \in AsyncIngressSources:
                   IF recipient = node /\ candidate = Source
                   THEN updated[recipient][candidate] = AfterLane
                   ELSE updated[recipient][candidate] =
                          asyncIngressLanes[recipient][candidate]
      BY <2>1, <2>2, NestedFunctionalReplaceFacts
         DEF AsyncIngressTopologyTypeInvariant
    <2>7. /\ DOMAIN asyncIngressLanes' = ValidatorIds
           /\ DOMAIN asyncIngressReady' = ValidatorIds
           /\ \A recipient \in ValidatorIds:
                DOMAIN asyncIngressLanes'[recipient] =
                  AsyncIngressSources
      <3>1. DOMAIN asyncIngressReady' = ValidatorIds
        BY <2>1, <2>2, <2>5,
           FunctionalReplacePreservesDomain
           DEF AsyncIngressTopologyTypeInvariant
      <3> QED BY <2>5, <2>6, <3>1
    <2>8. \A recipient \in ValidatorIds,
             candidate \in AsyncIngressSources:
           IF recipient = node /\ candidate = Source
           THEN asyncIngressLanes'[recipient][candidate] = AfterLane
           ELSE asyncIngressLanes'[recipient][candidate] =
                  asyncIngressLanes[recipient][candidate]
      BY <2>5, <2>6
    <2>8a. /\ Len(BeforeLane) \in Nat
            /\ Len(asyncIngressLanes'[node][Source]) \in Nat
            /\ Len(asyncIngressLanes'[node][Source]) + 1 =
                 Len(BeforeLane)
      BY <2>2, <2>3, <2>8, LenProperties
    <2>9. /\ AsyncIngressNonemptySourcesFor(
                  asyncIngressLanes', node) =
                (IF Len(BeforeLane) = 1
                 THEN AsyncIngressNonemptySourcesFor(
                        asyncIngressLanes, node) \ {Source}
                 ELSE AsyncIngressNonemptySourcesFor(
                        asyncIngressLanes, node))
           /\ \A recipient \in ValidatorIds \ {node}:
                AsyncIngressNonemptySourcesFor(
                  asyncIngressLanes', recipient) =
                AsyncIngressNonemptySourcesFor(
                  asyncIngressLanes, recipient)
      <3>1. asyncIngressLanes'[node][Source] = AfterLane
        BY <1>1, <2>2, <2>8
      <3>2. \A recipient \in ValidatorIds,
                 candidate \in AsyncIngressSources:
               IF recipient = node /\ candidate = Source
               THEN asyncIngressLanes'[recipient][candidate] =
                      asyncIngressLanes'[node][Source]
               ELSE asyncIngressLanes'[recipient][candidate] =
                      asyncIngressLanes[recipient][candidate]
        BY <2>8, <3>1
      <3>3. /\ node \in ValidatorIds
              /\ Source \in AsyncIngressSources
              /\ Len(asyncIngressLanes[node][Source]) \in Nat
              /\ Len(asyncIngressLanes'[node][Source]) \in Nat
              /\ Len(asyncIngressLanes'[node][Source]) + 1 =
                   Len(asyncIngressLanes[node][Source])
              /\ \A recipient \in ValidatorIds,
                    candidate \in AsyncIngressSources:
                   IF recipient = node /\ candidate = Source
                   THEN asyncIngressLanes'[recipient][candidate] =
                          asyncIngressLanes'[node][Source]
                   ELSE asyncIngressLanes'[recipient][candidate] =
                          asyncIngressLanes[recipient][candidate]
        BY <1>1, <2>2, <2>8a, <3>2
           DEF BeforeLane, IngressLane
      <3>4. /\ AsyncIngressNonemptySourcesFor(
                       asyncIngressLanes', node) =
                     (IF Len(asyncIngressLanes[node][Source]) = 1
                      THEN AsyncIngressNonemptySourcesFor(
                             asyncIngressLanes, node) \ {Source}
                      ELSE AsyncIngressNonemptySourcesFor(
                             asyncIngressLanes, node))
              /\ \A recipient \in ValidatorIds \ {node}:
                   AsyncIngressNonemptySourcesFor(
                     asyncIngressLanes', recipient) =
                   AsyncIngressNonemptySourcesFor(
                     asyncIngressLanes, recipient)
        BY ONLY <3>3,
           IngressNonemptySourcesAfterOneLaneRemoval
      <3> QED BY <3>4 DEF BeforeLane, IngressLane
    <2>10. /\ Source \in AsyncIngressSources
           /\ NextReady \in Seq(Range(NextReady))
           /\ DOMAIN NextReady = 1..Len(NextReady)
           /\ SequenceSet(NextReady) \subseteq AsyncIngressSources
           /\ Len(NextReady) =
                Cardinality(SequenceSet(NextReady))
           /\ SequenceSet(NextReady) =
                (IF Len(BeforeLane) = 1
                 THEN SequenceSet(asyncIngressReady[node]) \ {Source}
                 ELSE SequenceSet(asyncIngressReady[node]))
      BY <1>1, <2>1, ReadyAfterSelectedDrainFacts
         DEF Source, BeforeLane, NextReady,
             ReadyAfterSelectedDrain
    <2>11. /\ asyncIngressReady'[node] = NextReady
            /\ \A recipient \in ValidatorIds \ {node}:
                 asyncIngressReady'[recipient] =
                   asyncIngressReady[recipient]
      <3>1. asyncIngressReady'[node] = NextReady
        BY <2>1, <2>5, FunctionalReplaceUpdateAtKey
           DEF AsyncIngressTopologyTypeInvariant
      <3>2. \A recipient \in ValidatorIds \ {node}:
               asyncIngressReady'[recipient] =
                 asyncIngressReady[recipient]
        BY <2>1, <2>5, FunctionalUpdateAwayFromKey
           DEF AsyncIngressTopologyTypeInvariant
      <3> QED BY <3>1, <3>2
    <2>12. \A recipient \in ValidatorIds:
              SequenceSet(asyncIngressReady[recipient]) =
                AsyncIngressNonemptySourcesFor(
                  asyncIngressLanes, recipient)
      BY <2>1, IngressTopologyReadySetEquality
    <2>13. ASSUME NEW recipient \in ValidatorIds
           PROVE /\ DOMAIN asyncIngressReady'[recipient] =
                        1..Len(asyncIngressReady'[recipient])
                 /\ asyncIngressReady'[recipient] \in
                        Seq(Range(asyncIngressReady'[recipient]))
                 /\ SequenceSet(asyncIngressReady'[recipient])
                        \subseteq AsyncIngressSources
                 /\ Len(asyncIngressReady'[recipient]) =
                        Cardinality(
                          SequenceSet(asyncIngressReady'[recipient]))
                 /\ SequenceSet(asyncIngressReady'[recipient]) =
                        AsyncIngressNonemptySourcesFor(
                          asyncIngressLanes', recipient)
      <3>1. CASE recipient = node
        <4>1. asyncIngressReady'[recipient] = NextReady
          BY <2>11, <3>1
        <4>2. DOMAIN asyncIngressReady'[recipient] =
                 1..Len(asyncIngressReady'[recipient])
          BY <2>10, <4>1
        <4>3. asyncIngressReady'[recipient] \in
                 Seq(Range(asyncIngressReady'[recipient]))
          BY <2>10, <4>1
        <4>4. SequenceSet(asyncIngressReady'[recipient])
                 \subseteq AsyncIngressSources
          BY <2>10, <4>1
        <4>5. Len(asyncIngressReady'[recipient]) =
                 Cardinality(
                   SequenceSet(asyncIngressReady'[recipient]))
          BY <2>10, <4>1
        <4>6. SequenceSet(asyncIngressReady'[recipient]) =
                 AsyncIngressNonemptySourcesFor(
                   asyncIngressLanes', recipient)
          BY <2>9, <2>10, <2>12, <3>1, <4>1
        <4> QED BY <4>2, <4>3, <4>4, <4>5, <4>6
      <3>2. CASE recipient # node
        <4>1. asyncIngressReady'[recipient] =
                 asyncIngressReady[recipient]
          BY <2>11, <2>13, <3>2
        <4>2. AsyncIngressNonemptySourcesFor(
                 asyncIngressLanes', recipient) =
               AsyncIngressNonemptySourcesFor(
                 asyncIngressLanes, recipient)
          BY <2>9, <2>13, <3>2
        <4>3. /\ DOMAIN asyncIngressReady[recipient] =
                        1..Len(asyncIngressReady[recipient])
               /\ asyncIngressReady[recipient] \in
                        Seq(Range(asyncIngressReady[recipient]))
               /\ SequenceSet(asyncIngressReady[recipient])
                        \subseteq AsyncIngressSources
               /\ Len(asyncIngressReady[recipient]) =
                        Cardinality(
                          SequenceSet(asyncIngressReady[recipient]))
          BY <2>1, <2>13
             DEF AsyncIngressTopologyTypeInvariant
        <4>4. SequenceSet(asyncIngressReady[recipient]) =
                 AsyncIngressNonemptySourcesFor(
                   asyncIngressLanes, recipient)
          BY <2>12, <2>13
        <4> QED BY <4>1, <4>2, <4>3, <4>4
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>7, <2>13
         DEF AsyncIngressTopologyTypeInvariant,
             AsyncIngressNonemptySourcesFor,
             IngressLaneDepth, IngressLane
  <1> QED BY <1>1

THEOREM PopSelectedIngressPreservesCapacityType ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      \A laneIndex \in
           1..Len(IngressLane(node, asyncIngressReady[node][index])):
        /\ AsyncConfiguration
        /\ ModelConfiguration
        /\ AsyncIngressTypeInvariant
        /\ PopSelectedIngress(node, index, laneIndex)
        => AsyncIngressCapacityTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW index \in 1..Len(asyncIngressReady[node]),
                NEW laneIndex \in
                  1..Len(IngressLane(
                    node, asyncIngressReady[node][index])),
                AsyncConfiguration,
                ModelConfiguration,
                AsyncIngressTypeInvariant,
                PopSelectedIngress(node, index, laneIndex)
         PROVE AsyncIngressCapacityTypeInvariant'
    <2> DEFINE Source == asyncIngressReady[node][index]
    <2> DEFINE BeforeLanes == asyncIngressLanes
    <2> DEFINE AfterLanes == asyncIngressLanes'
    <2> DEFINE Before == BeforeLanes[node][Source]
    <2> DEFINE After == AfterLanes[node][Source]
    <2>1. /\ AsyncIngressCapacity \in Nat \ {0}
           /\ IsFiniteSet(AsyncIngressSources)
      BY <1>1, AsyncIngressSourcesAreFinite
         DEF AsyncConfiguration
    <2>2. Source \in AsyncIngressSources
      <3>1. SequenceSet(asyncIngressReady[node]) \subseteq
               AsyncIngressSources
        BY <1>1
           DEF AsyncIngressTypeInvariant,
               AsyncIngressTopologyTypeInvariant
      <3>2. Source \in SequenceSet(asyncIngressReady[node])
        BY <1>1, Isa DEF Source, SequenceSet
      <3> QED BY <3>1, <3>2
    <2>3. /\ Before \in Seq(Range(Before))
           /\ DOMAIN Before = 1..Len(Before)
           /\ (\A queuedIndex \in 1..Len(Before):
                 AsyncItemTyped(Before[queuedIndex]))
           /\ laneIndex \in 1..Len(Before)
           /\ Len(Before) \in Nat
           /\ Len(Before) <= AsyncIngressCapacity
           /\ (\A laneRecipient \in ValidatorIds,
                   laneSource \in AsyncIngressSources:
                 Len(BeforeLanes[laneRecipient][laneSource]) <=
                   AsyncIngressCapacity)
           /\ (\A capacityRecipient \in ValidatorIds:
                 /\ AsyncIngressDepthFor(
                      BeforeLanes, capacityRecipient) <=
                      AsyncIngressCapacity
                 /\ AsyncIngressDepthFor(
                      BeforeLanes, capacityRecipient)
                      + IngressProtectedSlotCountFor(
                          BeforeLanes, capacityRecipient)
                      <= AsyncIngressCapacity)
      BY <1>1, <2>2, LenProperties
         DEF Before, BeforeLanes, Source,
             AsyncIngressTypeInvariant,
             AsyncIngressContentTypeInvariant,
             AsyncIngressCapacityTypeInvariant,
             IngressLane, IngressLaneDepth, IngressDepth,
             AsyncIngressDepthFor, AsyncIngressPairIndicesFor
    <2>4. /\ AfterLanes =
                  [BeforeLanes EXCEPT
                     ![node][Source] =
                       SequenceWithoutIndex(@, laneIndex)]
           /\ After = SequenceWithoutIndex(Before, laneIndex)
      <3>1. asyncIngressLanes' =
               [asyncIngressLanes EXCEPT
                  ![node][Source] =
                    SequenceWithoutIndex(@, laneIndex)]
        BY <1>1 DEF PopSelectedIngress, Source
      <3>2. AfterLanes =
               [BeforeLanes EXCEPT
                  ![node][Source] =
                    SequenceWithoutIndex(@, laneIndex)]
        BY <3>1 DEF BeforeLanes, AfterLanes
      <3>3. After = SequenceWithoutIndex(Before, laneIndex)
        <4>1. AfterLanes[node][Source] =
                 SequenceWithoutIndex(
                   BeforeLanes[node][Source], laneIndex)
          BY <1>1, <2>2, <3>2, Isa
             DEF BeforeLanes,
                 AsyncIngressTypeInvariant,
                 AsyncIngressTopologyTypeInvariant
        <4> QED BY <4>1 DEF Before, After
      <3> QED BY <3>2, <3>3
    <2>5. /\ After \in Seq(Range(After))
           /\ DOMAIN After = 1..Len(After)
           /\ (\A queuedIndex \in 1..Len(After):
                 AsyncItemTyped(After[queuedIndex]))
           /\ Len(After) \in Nat
           /\ Len(After) + 1 = Len(Before)
      BY <2>3, <2>4, TypedIngressRemovalFacts, LenProperties
    <2>6. /\ (\A otherSource \in AsyncIngressSources \ {Source}:
                 AfterLanes[node][otherSource] =
                   BeforeLanes[node][otherSource])
           /\ (\A otherRecipient \in ValidatorIds \ {node}:
                 AfterLanes[otherRecipient] =
                   BeforeLanes[otherRecipient])
      BY <1>1, <2>2, <2>3, <2>4, Isa
         DEF BeforeLanes, AfterLanes,
             AsyncIngressTypeInvariant,
             AsyncIngressTopologyTypeInvariant
    <2>7. \A recipient \in ValidatorIds,
                source \in AsyncIngressSources:
             Len(AfterLanes[recipient][source]) <=
               AsyncIngressCapacity
      <3>1. ASSUME NEW recipient \in ValidatorIds,
                     NEW source \in AsyncIngressSources
             PROVE Len(AfterLanes[recipient][source]) <=
                     AsyncIngressCapacity
        <4>1. CASE recipient = node /\ source = Source
          <5>1. AfterLanes[recipient][source] = After
            BY <4>1 DEF After
          <5>2. Len(After) <= AsyncIngressCapacity
            BY <2>1, <2>3, <2>5,
               NaturalPredecessorPreservesUpperBound
          <5> QED BY <5>1, <5>2
        <4>2. CASE recipient = node /\ source # Source
          BY <2>3, <2>6, <3>1, <4>2
        <4>3. CASE recipient # node
          BY <2>3, <2>6, <3>1, <4>3
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>1
    <2>8. AsyncIngressDepthFor(AfterLanes, node) + 1 =
             AsyncIngressDepthFor(BeforeLanes, node)
      BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
         IngressDepthDropsByOneAfterLaneRemoval
    <2>9. IngressProtectedSlotCountFor(AfterLanes, node) <=
             IngressProtectedSlotCountFor(BeforeLanes, node) + 1
      <3>1. IngressProtectedSlotCountFor(BeforeLanes, node) =
               IngressProtectedSlotCountWithoutSourceFor(
                 BeforeLanes, node, Source)
                 + IngressSourceProtectionPotential(Source, Before)
        BY <1>1, <2>2, IngressProtectedSlotCountDecomposesAtSource
           DEF Before
      <3>2. IngressProtectedSlotCountFor(AfterLanes, node) =
               IngressProtectedSlotCountWithoutSourceFor(
                 AfterLanes, node, Source)
                 + IngressSourceProtectionPotential(Source, After)
        BY <1>1, <2>2, IngressProtectedSlotCountDecomposesAtSource
           DEF After
      <3>3. IngressProtectedSlotCountWithoutSourceFor(
               AfterLanes, node, Source) =
             IngressProtectedSlotCountWithoutSourceFor(
               BeforeLanes, node, Source)
        BY <2>6, IngressProtectedSlotsWithoutSourceAreLocal
      <3>4. IngressSourceProtectionPotential(Source, After) <=
               IngressSourceProtectionPotential(Source, Before) + 1
        BY <2>3, <2>4,
           OneRemovalIncreasesSourceProtectionByAtMostOne
           DEF Before, After
      <3>5. /\ IngressProtectedSlotCountWithoutSourceFor(
                    BeforeLanes, node, Source) \in Nat
             /\ IngressSourceProtectionPotential(Source, Before) \in Nat
             /\ IngressSourceProtectionPotential(Source, After) \in Nat
        BY <1>1,
           IngressProtectedSlotCountWithoutSourceIsNatural,
           IngressSourceProtectionPotentialIsNatural
      <3>6. IngressProtectedSlotCountWithoutSourceFor(
               BeforeLanes, node, Source)
               + IngressSourceProtectionPotential(Source, After) <=
             (IngressProtectedSlotCountWithoutSourceFor(
                BeforeLanes, node, Source)
                + IngressSourceProtectionPotential(Source, Before)) + 1
        BY <3>4, <3>5, CommonNaturalPrefixPreservesOneIncrease
      <3> QED BY <3>1, <3>2, <3>3, <3>6
    <2>10. \A otherRecipient \in ValidatorIds \ {node}:
              /\ AsyncIngressDepthFor(AfterLanes, otherRecipient) =
                   AsyncIngressDepthFor(BeforeLanes, otherRecipient)
              /\ IngressProtectedSlotCountFor(
                   AfterLanes, otherRecipient) =
                   IngressProtectedSlotCountFor(
                     BeforeLanes, otherRecipient)
      <3>1. ASSUME NEW otherRecipient \in ValidatorIds \ {node}
             PROVE /\ AsyncIngressDepthFor(
                          AfterLanes, otherRecipient) =
                        AsyncIngressDepthFor(
                          BeforeLanes, otherRecipient)
                   /\ IngressProtectedSlotCountFor(
                        AfterLanes, otherRecipient) =
                        IngressProtectedSlotCountFor(
                          BeforeLanes, otherRecipient)
        <4>1. AfterLanes[otherRecipient] =
                 BeforeLanes[otherRecipient]
          BY <2>6, <3>1
        <4>2. AsyncIngressDepthFor(
                 AfterLanes, otherRecipient) =
               AsyncIngressDepthFor(
                 BeforeLanes, otherRecipient)
          BY <4>1, Isa
             DEF AsyncIngressDepthFor,
                 AsyncIngressPairIndicesFor
        <4>3. IngressProtectedSlotCountFor(
                 AfterLanes, otherRecipient) =
               IngressProtectedSlotCountFor(
                 BeforeLanes, otherRecipient)
          BY <4>1, Isa
             DEF IngressProtectedSlotCountFor,
                 IngressProtectedSourcesFor,
                 IngressTimeoutVoteProtectedSourcesFor,
                 IngressTransportCompletionProtectedSourcesFor,
                 IngressContinuationProtectedSourcesFor,
                 IngressLaneHasNonTimeoutProgressIn,
                 IngressLaneHasTimeoutVoteIn,
                 IngressLaneHasTransportCompletionIn,
                 SequenceSet
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>11. \A recipient \in ValidatorIds:
              /\ AsyncIngressDepthFor(AfterLanes, recipient) <=
                   AsyncIngressCapacity
              /\ AsyncIngressDepthFor(AfterLanes, recipient)
                   + IngressProtectedSlotCountFor(
                       AfterLanes, recipient)
                   <= AsyncIngressCapacity
      <3>1. ASSUME NEW recipient \in ValidatorIds
             PROVE /\ AsyncIngressDepthFor(AfterLanes, recipient) <=
                          AsyncIngressCapacity
                   /\ AsyncIngressDepthFor(AfterLanes, recipient)
                        + IngressProtectedSlotCountFor(
                            AfterLanes, recipient)
                        <= AsyncIngressCapacity
        <4>1. CASE recipient = node
          <5>1. /\ AsyncIngressDepthFor(BeforeLanes, recipient) \in Nat
                 /\ AsyncIngressDepthFor(AfterLanes, recipient) \in Nat
                 /\ IngressProtectedSlotCountFor(
                      BeforeLanes, recipient) \in Nat
                 /\ IngressProtectedSlotCountFor(
                      AfterLanes, recipient) \in Nat
            BY <1>1, AsyncIngressDepthForIsNatural,
               IngressProtectedSlotCountIsNatural
          <5>2. /\ AsyncIngressDepthFor(BeforeLanes, recipient) <=
                        AsyncIngressCapacity
                 /\ AsyncIngressDepthFor(BeforeLanes, recipient)
                      + IngressProtectedSlotCountFor(
                          BeforeLanes, recipient)
                      <= AsyncIngressCapacity
            BY <2>3, <3>1
          <5>3. AsyncIngressDepthFor(AfterLanes, recipient) <=
                   AsyncIngressCapacity
            BY <2>1, <2>8, <4>1, <5>1, <5>2,
               NaturalPredecessorPreservesUpperBound
          <5>4. AsyncIngressDepthFor(AfterLanes, recipient)
                   + IngressProtectedSlotCountFor(
                       AfterLanes, recipient)
                   <= AsyncIngressCapacity
            BY <2>1, <2>8, <2>9, <4>1, <5>1, <5>2,
               CoupledNaturalPotentialPreservesUpperBound
          <5> QED BY <5>3, <5>4
        <4>2. CASE recipient # node
          <5>1. /\ AsyncIngressDepthFor(AfterLanes, recipient) =
                       AsyncIngressDepthFor(BeforeLanes, recipient)
                 /\ IngressProtectedSlotCountFor(
                      AfterLanes, recipient) =
                      IngressProtectedSlotCountFor(
                        BeforeLanes, recipient)
            BY <2>10, <3>1, <4>2
          <5> QED BY <2>3, <3>1, <5>1
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>7, <2>11
         DEF AsyncIngressCapacityTypeInvariant,
             IngressLaneDepth, IngressLane, IngressDepth,
             AsyncIngressDepthFor, AsyncIngressPairIndicesFor,
             AfterLanes
  <1> QED BY <1>1

THEOREM PopSelectedIngressPreservesIngressType ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      \A laneIndex \in
           1..Len(IngressLane(node, asyncIngressReady[node][index])):
        /\ AsyncConfiguration
        /\ ModelConfiguration
        /\ AsyncIngressTypeInvariant
        /\ PopSelectedIngress(node, index, laneIndex)
        => AsyncIngressTypeInvariant'
BY PopSelectedIngressPreservesContentType,
   PopSelectedIngressPreservesTopologyType,
   PopSelectedIngressPreservesCapacityType
   DEF AsyncIngressTypeInvariant

THEOREM DeliveryCandidateShape ==
  \A item:
    /\ DOMAIN DeliveryCandidate(item) = AsyncCandidateDomain
    /\ DeliveryCandidate(item).class = DeliveryClass(item)
    /\ DeliveryCandidate(item).kind = DeliveryKind(item)
    /\ DeliveryCandidate(item).node = item.envelope.recipient
    /\ DeliveryCandidate(item).height = DeliveryHeight(item)
    /\ DeliveryCandidate(item).view = DeliveryView(item)
    /\ DeliveryCandidate(item).subject = DeliverySubject(item)
    /\ DeliveryCandidate(item).item = item
    /\ DeliveryCandidate(item).consumerContext = context
    /\ DeliveryCandidate(item).consumerView =
         nodeView[item.envelope.recipient]
    /\ DeliveryCandidate(item).consumerGeneration =
         generation[item.envelope.recipient]
    /\ DeliveryCandidate(item).evidence = item
    /\ DeliveryCandidate(item).bodyIdentity = DeliverySubject(item)
    /\ DeliveryCandidate(item).manifestIdentity = DeliverySubject(item)
    /\ DeliveryCandidate(item).commitmentIdentity = DeliverySubject(item)
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
    /\ TypeInvariant
    /\ AsyncItemTyped(item)
    => AsyncCandidateTyped(DeliveryCandidate(item))
PROOF
  <1>1. ASSUME NEW item, TypeInvariant, AsyncItemTyped(item)
         PROVE AsyncCandidateTyped(DeliveryCandidate(item))
    <2>1. /\ item.kind \in AsyncNetworkKinds
           /\ item.envelope.recipient \in ValidatorIds
      BY <1>1 DEF AsyncItemTyped
    <2>2. /\ DOMAIN DeliveryCandidate(item) = AsyncCandidateDomain
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
      <3>1. AsyncCommitCertificateResponseEnvelopeTyped(item.envelope)
        BY <1>1, <2>8, SMT DEF AsyncItemTyped
      <3> QED BY <1>1, <2>2, <2>8, <3>1,
           DeliveryCandidateShape, SMT
           DEF DeliveryHeight, DeliveryView, DeliverySubject,
               AsyncCommitCertificateResponseEnvelopeTyped,
               QcRecordSet, SubjectOrNone, AsyncCandidateTyped
    <2>9. CASE item.kind = "CertifiedResponse"
      <3>1. AsyncCertifiedResponseEnvelopeTyped(item.envelope)
        BY <1>1, <2>9, SMT DEF AsyncItemTyped
      <3> QED BY <1>1, <2>2, <2>9, <3>1,
           DeliveryCandidateShape, SMT
           DEF DeliveryHeight, DeliveryView, DeliverySubject,
               AsyncCertifiedResponseEnvelopeTyped,
               AsyncReplyRequestItemTyped, AsyncBodyEnvelopeTyped,
               SubjectOrNone, AsyncCandidateTyped
    <2>10. CASE item.kind = "CertifiedRequest"
      <3>1. AsyncReplyRequestItemTyped(item, "CertifiedRequest")
        BY <1>1, <2>10, SMT DEF AsyncItemTyped
      <3> QED BY <1>1, <2>2, <2>10, <3>1,
           DeliveryCandidateShape, SMT
           DEF DeliveryHeight, DeliveryView, DeliverySubject,
               AsyncReplyRequestItemTyped, QcRecordSet, SubjectOrNone,
               AsyncCandidateTyped
    <2>11. CASE item.kind \in
      {"Chunk", "CommitCertificateRequest",
       "NormalJunk", "ProgressJunk", "Noise"}
      <3>1. AsyncBodyEnvelopeTyped(item.envelope)
        BY <1>1, <2>11, SMT
           DEF AsyncItemTyped, AsyncReplyRequestItemTyped,
               AsyncCommitCertificateRequestEnvelopeTyped
      <3> QED BY <1>1, <2>2, <2>11, <3>1,
           DeliveryCandidateShape, SMT
           DEF DeliveryHeight, DeliveryView, DeliverySubject,
               AsyncBodyEnvelopeTyped, SubjectOrNone,
               AsyncCandidateTyped
    <2> QED BY <2>1, <2>3, <2>4, <2>5, <2>6, <2>7, <2>8,
                <2>9, <2>10, <2>11,
                SMT DEF AsyncNetworkKinds
  <1> QED BY <1>1

THEOREM AsyncIoServeNoncesAreFiniteAndBounded ==
  \A node \in ValidatorIds:
    /\ AsyncConfiguration
    /\ AsyncIoQueueContentTypeInvariant
    => /\ AsyncIoServeNonces(node) \subseteq 0..AsyncIoAuxCapacity
       /\ IsFiniteSet(AsyncIoServeNonces(node))
       /\ Cardinality(AsyncIoServeNonces(node)) <= AsyncIoQueueDepth(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncConfiguration,
                AsyncIoQueueContentTypeInvariant
         PROVE /\ AsyncIoServeNonces(node) \subseteq 0..AsyncIoAuxCapacity
               /\ IsFiniteSet(AsyncIoServeNonces(node))
               /\ Cardinality(AsyncIoServeNonces(node))
                    <= AsyncIoQueueDepth(node)
    <2> DEFINE Queue == asyncIoQueues[node]
    <2> DEFINE Indices == AsyncIoServeIndices(Queue)
    <2>1. /\ AsyncIoSequenceTyped(Queue)
           /\ Len(Queue) \in Nat
      BY <1>1, LenProperties DEF AsyncIoQueueContentTypeInvariant, Queue
    <2>2. /\ Indices \subseteq 1..Len(Queue)
           /\ IsFiniteSet(Indices)
           /\ Cardinality(Indices) <= Len(Queue)
      <3>1. IsFiniteSet(1..Len(Queue))
        BY <2>1, FS_Interval
      <3>2. Indices \subseteq 1..Len(Queue)
        BY Isa DEF Indices, AsyncIoServeIndices
      <3>3. /\ IsFiniteSet(Indices)
             /\ Cardinality(Indices) <= Cardinality(1..Len(Queue))
        BY <3>1, <3>2, FS_Subset
      <3>4. Cardinality(1..Len(Queue)) = Len(Queue)
        BY <2>1, FS_Interval, SMT
      <3> QED BY <3>2, <3>3, <3>4
    <2>3. /\ IsFiniteSet(
                   {Queue[index].nonce: index \in Indices})
           /\ Cardinality(
                   {Queue[index].nonce: index \in Indices})
                <= Cardinality(Indices)
      BY <2>2, FS_Image
    <2>4. AsyncIoServeNonces(node) =
             {Queue[index].nonce: index \in Indices}
      BY DEF AsyncIoServeNonces, Indices, Queue
    <2>5. AsyncIoServeNonces(node) \subseteq 0..AsyncIoAuxCapacity
      <3>1. ASSUME NEW nonce \in AsyncIoServeNonces(node)
             PROVE nonce \in 0..AsyncIoAuxCapacity
        <4>1. PICK index \in Indices:
                 nonce = Queue[index].nonce
          BY <3>1 DEF AsyncIoServeNonces, Indices, Queue
        <4>2. /\ index \in 1..Len(Queue)
               /\ AsyncIoJobTyped(Queue[index])
          BY <2>1, <2>2, <4>1 DEF AsyncIoSequenceTyped
        <4> QED BY <4>1, <4>2 DEF AsyncIoJobTyped
      <3> QED BY <3>1
    <2> QED BY <2>2, <2>3, <2>4, <2>5
         DEF AsyncIoQueueDepth, Queue
  <1> QED BY <1>1

THEOREM FreshAsyncIoServeNonceFacts ==
  \A node \in ValidatorIds:
    /\ AsyncConfiguration
    /\ AsyncIoQueueContentTypeInvariant
    /\ CanEnqueueIoClass(node, "Serve")
    => /\ FreshAsyncIoServeNonce(node) \in 0..AsyncIoAuxCapacity
       /\ FreshAsyncIoServeNonce(node)
            \notin AsyncIoServeNonces(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncConfiguration,
                AsyncIoQueueContentTypeInvariant,
                CanEnqueueIoClass(node, "Serve")
         PROVE /\ FreshAsyncIoServeNonce(node)
                      \in 0..AsyncIoAuxCapacity
               /\ FreshAsyncIoServeNonce(node)
                      \notin AsyncIoServeNonces(node)
    <2>1. /\ AsyncIoAuxCapacity \in Nat \ {0}
           /\ AsyncIoQueueDepth(node) < AsyncIoAuxCapacity
      BY <1>1 DEF AsyncConfiguration, CanEnqueueIoClass,
                    AsyncIoAdmissionLimit
    <2>2. /\ AsyncIoServeNonces(node) \subseteq
                    0..AsyncIoAuxCapacity
           /\ IsFiniteSet(AsyncIoServeNonces(node))
           /\ Cardinality(AsyncIoServeNonces(node))
                <= AsyncIoQueueDepth(node)
      BY <1>1, AsyncIoServeNoncesAreFiniteAndBounded
    <2>3. /\ IsFiniteSet(0..AsyncIoAuxCapacity)
           /\ Cardinality(0..AsyncIoAuxCapacity) =
                AsyncIoAuxCapacity + 1
      BY <2>1, FS_Interval, SMT
    <2>4. Cardinality(AsyncIoServeNonces(node)) <
             Cardinality(0..AsyncIoAuxCapacity)
      BY <2>1, <2>2, <2>3, SMT
    <2>5. (0..AsyncIoAuxCapacity) \ AsyncIoServeNonces(node) # {}
      BY <2>2, <2>3, <2>4, FS_Subset, SMT
    <2> QED BY <2>5, FS_EmptySet, Zenon DEF FreshAsyncIoServeNonce
  <1> QED BY <1>1

THEOREM TypedRequestMakesTypedServeJob ==
  \A node \in ValidatorIds:
  \A item:
    /\ AsyncConfiguration
    /\ TypeInvariant
    /\ AsyncIoQueueContentTypeInvariant
    /\ CanEnqueueIoClass(node, "Serve")
    /\ AsyncItemTyped(item)
    /\ item.kind \in {"CertifiedRequest", "CommitCertificateRequest"}
    => AsyncIoJobTyped(
         AsyncIoCertifiedServeJob(node, DeliveryCandidate(item)))
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW item,
                AsyncConfiguration,
                TypeInvariant,
                AsyncIoQueueContentTypeInvariant,
                CanEnqueueIoClass(node, "Serve"),
                AsyncItemTyped(item),
                item.kind
                  \in {"CertifiedRequest", "CommitCertificateRequest"}
         PROVE AsyncIoJobTyped(
                 AsyncIoCertifiedServeJob(node, DeliveryCandidate(item)))
    <2>1. /\ item.kind \in AsyncNetworkKinds
           /\ item.envelope.recipient \in ValidatorIds
      BY <1>1 DEF AsyncItemTyped
    <2>2. AsyncCandidateTyped(DeliveryCandidate(item))
      BY <1>1, TypedItemMakesTypedDeliveryCandidate
    <2>3. /\ FreshAsyncIoServeNonce(node) \in 0..AsyncIoAuxCapacity
           /\ FreshAsyncIoServeNonce(node)
                \notin AsyncIoServeNonces(node)
      BY <1>1, FreshAsyncIoServeNonceFacts
    <2> QED BY <1>1, <2>2, <2>3, SMT
         DEF AsyncIoJobTyped, AsyncIoCertifiedServeJob, AsyncIoJob,
             DeliveryCandidate, DeliveryClass, DeliveryKind,
             AsyncIoCommandClasses,
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
              @, AsyncIoCertifiedServeJob(node, DeliveryCandidate(item)))]
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
                            node, DeliveryCandidate(item)))],
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
             AsyncIoCertifiedServeJob(node, DeliveryCandidate(item)))
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
    <2>4s. \A other \in ValidatorIds:
              AsyncIoServeNonceOwnership(asyncIoQueues'[other])
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE AsyncIoServeNonceOwnership(asyncIoQueues'[other])
        <4>1. CASE other = node
          <5>1. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
                 /\ AsyncIoServeNonceOwnership(asyncIoQueues[node])
            BY <2>1 DEF AsyncIoQueueContentTypeInvariant
          <5>2. /\ AsyncIoJobTyped(
                        AsyncIoCertifiedServeJob(
                          node, DeliveryCandidate(item)))
                 /\ AsyncIoCertifiedServeJob(
                      node, DeliveryCandidate(item)).class = "Serve"
                 /\ AsyncIoCertifiedServeJob(
                      node, DeliveryCandidate(item)).nonce
                      \notin AsyncIoServeNonces(node)
            BY <1>1, <2>1, <2>2, FreshAsyncIoServeNonceFacts
               DEF AsyncIoCertifiedServeJob, AsyncIoJob
          <5>3. AsyncIoServeNonces(node) =
                   {asyncIoQueues[node][index].nonce:
                      index \in AsyncIoServeIndices(
                                   asyncIoQueues[node])}
            BY DEF AsyncIoServeNonces
          <5> QED BY <1>1, <4>1, <5>1, <5>2, <5>3,
             AppendFreshServeJobPreservesNonceOwnership
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
                   node, DeliveryCandidate(item)).class # "Consensus"
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
      BY <2>4, <2>4s, <2>5 DEF AsyncIoQueueContentTypeInvariant
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
      BY <2>1, <2>2, SmallestNatural, SMTT(30)
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

THEOREM FirstHistoricalDrainableIngressLaneIndexIsDrainable ==
  \A node, source:
    HistoricalDrainableIngressLaneIndices(node, source) # {}
      => FirstHistoricalDrainableIngressLaneIndex(node, source)
           \in HistoricalDrainableIngressLaneIndices(node, source)
PROOF
  <1>1. ASSUME NEW node, NEW source,
                HistoricalDrainableIngressLaneIndices(node, source) # {}
         PROVE FirstHistoricalDrainableIngressLaneIndex(node, source)
                 \in HistoricalDrainableIngressLaneIndices(node, source)
    <2>1. PICK witness \in
                    HistoricalDrainableIngressLaneIndices(node, source):
             TRUE
      BY <1>1, FS_EmptySet, Zenon
    <2>2. witness \in Nat
      BY <2>1, SMT DEF HistoricalDrainableIngressLaneIndices
    <2>3. \E least \in Nat:
             /\ least \in
                  HistoricalDrainableIngressLaneIndices(node, source)
             /\ \A prior \in 0..(least - 1):
                  prior \notin
                    HistoricalDrainableIngressLaneIndices(node, source)
      BY <2>1, <2>2, SmallestNatural, SMTT(30)
    <2>4. PICK least \in Nat:
             /\ least \in
                  HistoricalDrainableIngressLaneIndices(node, source)
             /\ \A prior \in 0..(least - 1):
                  prior \notin
                    HistoricalDrainableIngressLaneIndices(node, source)
      BY <2>3
    <2>5. \A other \in
                  HistoricalDrainableIngressLaneIndices(node, source):
             least <= other
      BY <2>4, SMT DEF HistoricalDrainableIngressLaneIndices
    <2> QED BY <2>4, <2>5, Zenon
         DEF FirstHistoricalDrainableIngressLaneIndex
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
      BY <2>1, <2>2, SmallestNatural, SMTT(30)
    <2>4. PICK least \in Nat:
             /\ least \in DrainableIngressIndices(node)
             /\ \A prior \in 0..(least - 1):
                  prior \notin DrainableIngressIndices(node)
      BY <2>3
    <2>5. \A other \in DrainableIngressIndices(node): least <= other
      BY <2>4, SMT DEF DrainableIngressIndices
    <2>6. CASE DrainableClaimedResponseReadyIndices(node) # {}
      <3>1. (CHOOSE priority \in
                    DrainableClaimedResponseReadyIndices(node): TRUE)
                  \in DrainableClaimedResponseReadyIndices(node)
        BY <2>6, FS_EmptySet, Zenon
      <3>2. (CHOOSE priority \in
                    DrainableClaimedResponseReadyIndices(node): TRUE)
                  \in DrainableIngressIndices(node)
        BY <3>1 DEF DrainableClaimedResponseReadyIndices
      <3> QED BY <2>6, <3>1, <3>2, Zenon
           DEF FirstDrainableIngressIndex
    <2>7. CASE /\ DrainableClaimedResponseReadyIndices(node) = {}
                /\ DrainableRequestFencedCompletionReadyIndices(node) # {}
      <3>1. (CHOOSE priority \in
                    DrainableRequestFencedCompletionReadyIndices(node):
                    TRUE)
                  \in DrainableRequestFencedCompletionReadyIndices(node)
        BY <2>7, FS_EmptySet, Zenon
      <3>2. (CHOOSE priority \in
                    DrainableRequestFencedCompletionReadyIndices(node):
                    TRUE)
                  \in DrainableIngressIndices(node)
        BY <3>1 DEF DrainableRequestFencedCompletionReadyIndices
      <3> QED BY <2>7, <3>1, <3>2, Zenon
         DEF FirstDrainableIngressIndex
    <2>8. CASE /\ DrainableClaimedResponseReadyIndices(node) = {}
                /\ DrainableRequestFencedCompletionReadyIndices(node) = {}
      BY <2>4, <2>5, <2>8, Zenon
         DEF FirstDrainableIngressIndex
    <2> QED BY <2>6, <2>7, <2>8
  <1> QED BY <1>1

THEOREM FirstDrainableIngressLaneIndexIsDrainable ==
  \A node, source:
    DrainableIngressLaneIndices(node, source) # {}
      => FirstDrainableIngressLaneIndex(node, source)
           \in DrainableIngressLaneIndices(node, source)
PROOF
  <1>1. ASSUME NEW node, NEW source,
                DrainableIngressLaneIndices(node, source) # {}
         PROVE FirstDrainableIngressLaneIndex(node, source)
                 \in DrainableIngressLaneIndices(node, source)
    <2>1. PICK witness \in DrainableIngressLaneIndices(node, source):
             TRUE
      BY <1>1, FS_EmptySet, Zenon
    <2>2. witness \in Nat
      BY <2>1, SMT DEF DrainableIngressLaneIndices
    <2>3. \E least \in Nat:
             /\ least \in DrainableIngressLaneIndices(node, source)
             /\ \A prior \in 0..(least - 1):
                  prior \notin DrainableIngressLaneIndices(node, source)
      BY <2>1, <2>2, SmallestNatural, SMTT(30)
    <2>4. PICK least \in Nat:
             /\ least \in DrainableIngressLaneIndices(node, source)
             /\ \A prior \in 0..(least - 1):
                  prior \notin DrainableIngressLaneIndices(node, source)
      BY <2>3
    <2>5. \A other \in DrainableIngressLaneIndices(node, source):
             least <= other
      BY <2>4, SMT DEF DrainableIngressLaneIndices
    <2>6. CASE DrainableClaimedResponseLaneIndices(node, source) # {}
      <3>1. (CHOOSE priority \in
                    DrainableClaimedResponseLaneIndices(node, source):
                    TRUE)
                  \in DrainableClaimedResponseLaneIndices(node, source)
        BY <2>6, FS_EmptySet, Zenon
      <3>2. (CHOOSE priority \in
                    DrainableClaimedResponseLaneIndices(node, source):
                    TRUE)
                  \in DrainableIngressLaneIndices(node, source)
        BY <3>1 DEF DrainableClaimedResponseLaneIndices
      <3> QED BY <2>6, <3>1, <3>2, Zenon
           DEF FirstDrainableIngressLaneIndex
    <2>7. CASE /\ DrainableClaimedResponseLaneIndices(node, source) = {}
                /\ DrainableRequestFencedCompletionLaneIndices(
                     node, source) # {}
      <3>1. (CHOOSE priority \in
                    DrainableRequestFencedCompletionLaneIndices(
                      node, source):
                    TRUE)
                  \in DrainableRequestFencedCompletionLaneIndices(
                       node, source)
        BY <2>7, FS_EmptySet, Zenon
      <3>2. (CHOOSE priority \in
                    DrainableRequestFencedCompletionLaneIndices(
                      node, source):
                    TRUE)
                  \in DrainableIngressLaneIndices(node, source)
        BY <3>1
           DEF DrainableRequestFencedCompletionLaneIndices
      <3> QED BY <2>7, <3>1, <3>2, Zenon
         DEF FirstDrainableIngressLaneIndex
    <2>8. CASE /\ DrainableClaimedResponseLaneIndices(node, source) = {}
                /\ DrainableRequestFencedCompletionLaneIndices(
                     node, source) = {}
      BY <2>4, <2>5, <2>8, Zenon
         DEF FirstDrainableIngressLaneIndex
    <2> QED BY <2>6, <2>7, <2>8
  <1> QED BY <1>1

THEOREM DrainableIngressBypassesBlockedHeadWithinSource ==
  \A node, source:
    /\ Len(IngressLane(node, source)) >= 2
    /\ ~IngressItemCanDrain(node, IngressLane(node, source)[1])
    /\ \E index \in 2..Len(IngressLane(node, source)):
         IngressItemCanDrain(node, IngressLane(node, source)[index])
    => /\ IngressSourceCanDrain(node, source)
       /\ FirstDrainableIngressLaneIndex(node, source) > 1
       /\ IngressItemCanDrain(
            node,
            IngressLane(node, source)[
              FirstDrainableIngressLaneIndex(node, source)])
PROOF
  <1>1. ASSUME NEW node, NEW source,
                /\ Len(IngressLane(node, source)) >= 2
                /\ ~IngressItemCanDrain(
                     node, IngressLane(node, source)[1])
                /\ \E index \in 2..Len(IngressLane(node, source)):
                     IngressItemCanDrain(
                       node, IngressLane(node, source)[index])
         PROVE /\ IngressSourceCanDrain(node, source)
               /\ FirstDrainableIngressLaneIndex(node, source) > 1
               /\ IngressItemCanDrain(
                    node,
                    IngressLane(node, source)[
                      FirstDrainableIngressLaneIndex(node, source)])
    <2>1. DrainableIngressLaneIndices(node, source) # {}
      BY <1>1, SMT DEF DrainableIngressLaneIndices
    <2>2. FirstDrainableIngressLaneIndex(node, source)
               \in DrainableIngressLaneIndices(node, source)
      BY <2>1, FirstDrainableIngressLaneIndexIsDrainable
    <2> QED BY <1>1, <2>1, <2>2, SMT
         DEF IngressSourceCanDrain, DrainableIngressLaneIndices
  <1> QED BY <1>1

THEOREM SelectedIngressItemIsTyped ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      \A laneIndex \in
           1..Len(IngressLane(node, asyncIngressReady[node][index])):
        AsyncIngressTypeInvariant
          => AsyncItemTyped(
               IngressLane(
                 node, asyncIngressReady[node][index])[laneIndex])
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW index \in 1..Len(asyncIngressReady[node]),
                NEW laneIndex \in
                  1..Len(IngressLane(
                    node, asyncIngressReady[node][index])),
                AsyncIngressTypeInvariant
         PROVE AsyncItemTyped(
                  IngressLane(
                    node, asyncIngressReady[node][index])[laneIndex])
    <2>1. asyncIngressReady[node][index]
               \in SequenceSet(asyncIngressReady[node])
      BY <1>1, RangeEquality
         DEF AsyncIngressTypeInvariant,
             AsyncIngressTopologyTypeInvariant, SequenceSet
    <2>2. /\ IngressLane(
                    node, asyncIngressReady[node][index])
                    \in Seq(Range(IngressLane(
                      node, asyncIngressReady[node][index])))
           /\ \A candidateLaneIndex \in
                  1..IngressLaneDepth(
                    node, asyncIngressReady[node][index]):
                AsyncItemTyped(IngressLane(
                  node, asyncIngressReady[node][index])[
                    candidateLaneIndex])
      BY <1>1, <2>1
         DEF AsyncIngressTypeInvariant,
             AsyncIngressTopologyTypeInvariant,
             AsyncIngressContentTypeInvariant
    <2> QED BY <1>1, <2>2 DEF IngressLaneDepth
  <1> QED BY <1>1

THEOREM SelectedIngressItemHasLaneOwnership ==
  \A node \in ValidatorIds:
    \A index \in 1..Len(asyncIngressReady[node]):
      \A laneIndex \in
           1..Len(IngressLane(node, asyncIngressReady[node][index])):
        AsyncIngressTypeInvariant
         => /\ IngressLane(
                   node, asyncIngressReady[node][index])[laneIndex]
                   .envelope.recipient = node
             /\ IngressResourceSource(
                  IngressLane(
                    node, asyncIngressReady[node][index])[laneIndex]) =
                  asyncIngressReady[node][index]
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW index \in 1..Len(asyncIngressReady[node]),
                NEW laneIndex \in
                  1..Len(IngressLane(
                    node, asyncIngressReady[node][index])),
                AsyncIngressTypeInvariant
         PROVE /\ IngressLane(
                        node, asyncIngressReady[node][index])[laneIndex]
                        .envelope.recipient = node
               /\ IngressResourceSource(
                    IngressLane(
                      node, asyncIngressReady[node][index])[laneIndex]) =
                    asyncIngressReady[node][index]
    <2>1. asyncIngressReady[node][index]
               \in SequenceSet(asyncIngressReady[node])
      BY <1>1, RangeEquality
         DEF AsyncIngressTypeInvariant,
             AsyncIngressTopologyTypeInvariant, SequenceSet
    <2>2. /\ IngressLane(
                    node, asyncIngressReady[node][index])
                    \in Seq(Range(IngressLane(
                      node, asyncIngressReady[node][index])))
           /\ \A candidateLaneIndex \in
                  1..IngressLaneDepth(
                    node, asyncIngressReady[node][index]):
                /\ IngressLane(
                     node, asyncIngressReady[node][index])[
                       candidateLaneIndex]
                     .envelope.recipient = node
                /\ IngressResourceSource(
                     IngressLane(
                       node, asyncIngressReady[node][index])[
                         candidateLaneIndex]) =
                     asyncIngressReady[node][index]
      BY <1>1, <2>1
         DEF AsyncIngressTypeInvariant,
             AsyncIngressTopologyTypeInvariant,
             AsyncIngressContentTypeInvariant
    <2> QED BY <1>1, <2>2 DEF IngressLaneDepth
  <1> QED BY <1>1

THEOREM PopIngressLanePreservesCertifiedResponseClaimIngressOwnership ==
  \A recipient \in ValidatorIds,
     source \in AsyncIngressSources:
    \A index \in 1..IngressLaneDepth(recipient, source):
      LET item == IngressLane(recipient, source)[index]
      IN /\ AsyncIngressTypeInvariant
         /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
         /\ asyncIngressLanes' =
              [asyncIngressLanes EXCEPT
                 ![recipient][source] =
                   SequenceWithoutIndex(@, index)]
         /\ asyncCertifiedResponseClaim'
              \subseteq asyncCertifiedResponseClaim
         /\ (item.kind = "CertifiedResponse"
               /\ CertifiedResponseClaimMatches(item)
               => AsyncCertifiedResponseCanonicalWireIdentity(item)
                    \notin asyncCertifiedResponseClaim')
         => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY IngressSequenceWithoutIndexFacts,
   IngressSequenceWithoutIndexRetainsOtherValues,
   SMTT(90), Isa
   DEF AsyncCertifiedResponseClaimIngressOwnershipInvariant,
       CertifiedResponseClaimIngressOwner,
       CertifiedResponseClaimMatches,
       AsyncIngressTypeInvariant, AsyncIngressTopologyTypeInvariant,
       AsyncIngressContentTypeInvariant,
       IngressLane, IngressLaneDepth, SequenceSet

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
  \A node \in AsyncResponsiveAppliedArchiveServers:
    /\ AsyncTypeInvariant
    /\ RunHistoricalServer(node)
    /\ HistoricalDrainableIngressIndices(node) = {}
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncResponsiveAppliedArchiveServers,
                AsyncTypeInvariant,
                RunHistoricalServer(node),
                HistoricalDrainableIngressIndices(node) = {}
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncResponsiveAppliedArchiveServersAreValidators
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
             AsyncCertifiedResponseClaimAuthorityVars,
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
    <2>10. UNCHANGED AsyncHistoricalRecoveryFrameVars
      BY <1>1, <2>2, Isa
         DEF RunHistoricalServer, HistoricalIdleStep,
             AsyncHistoricalRecoveryFrameVars, vars
    <2>11. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, <2>10, HistoricalRecoveryFramePreservesType
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant
    <2> QED BY <2>5, <2>9, <2>11
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

HistoricalSelectedItem(node) ==
  HistoricalSelectedIngressItemAt(
    node, FirstHistoricalDrainableIngressIndex(node))

HistoricalSelectedRequestAuthorized(node) ==
  \/ /\ HistoricalSelectedItem(node).kind = "CertifiedRequest"
        /\ HistoricalSelectedItem(node) \in asyncSentItems
        /\ CertifiedRequestAuthorized(HistoricalSelectedItem(node))
  \/ /\ HistoricalSelectedItem(node).kind = "CommitCertificateRequest"
        /\ HistoricalSelectedItem(node) \in asyncSentItems
        /\ CommitCertificateRequestAuthorized(HistoricalSelectedItem(node))

THEOREM HistoricalAuthorizedRequestFrame ==
  \A node:
    /\ DrainHistoricalIngressSelected(node)
    /\ HistoricalSelectedRequestAuthorized(node)
    => /\ asyncIoQueues' =
             [asyncIoQueues EXCEPT
                ![node] = Append(
                  @, AsyncIoCertifiedServeJob(
                       node,
                       DeliveryCandidate(HistoricalSelectedItem(node))))]
       /\ UNCHANGED <<asyncCommandQueues,
                       asyncOutstandingWork,
                       asyncIoReadyCompletions,
                       asyncLocalReadyCompletions,
                       asyncNextCompletionSource,
                       asyncIoControlAvailable,
                       asyncDeferredCompletionQueues>>
PROOF
  <1>1. ASSUME NEW node,
                DrainHistoricalIngressSelected(node),
                HistoricalSelectedRequestAuthorized(node)
         PROVE /\ asyncIoQueues' =
                      [asyncIoQueues EXCEPT
                         ![node] = Append(
                           @, AsyncIoCertifiedServeJob(
                                node, DeliveryCandidate(
                                  HistoricalSelectedItem(node))))]
                /\ UNCHANGED <<asyncCommandQueues,
                                asyncOutstandingWork,
                                asyncIoReadyCompletions,
                                asyncLocalReadyCompletions,
                                asyncNextCompletionSource,
                                asyncIoControlAvailable,
                                asyncDeferredCompletionQueues>>
    <2>1. /\ asyncIoQueues' =
                 [asyncIoQueues EXCEPT
                    ![node] = Append(
                      @, AsyncIoCertifiedServeJob(
                           node,
                           DeliveryCandidate(HistoricalSelectedItem(node))))]
           /\ UNCHANGED <<asyncOutstandingWork,
                           asyncIoReadyCompletions,
                           asyncLocalReadyCompletions,
                           asyncNextCompletionSource,
                           asyncIoControlAvailable>>
      BY <1>1, Isa
         DEF DrainHistoricalIngressSelected,
             HistoricalSelectedRequestAuthorized,
             HistoricalSelectedItem, AsyncIoVars
    <2>2. UNCHANGED <<asyncCommandQueues,
                      asyncDeferredCompletionQueues>>
      BY <1>1, Isa
         DEF DrainHistoricalIngressSelected,
             AsyncDeferredVars
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM HistoricalRejectedRequestFrame ==
  \A node:
    /\ DrainHistoricalIngressSelected(node)
    /\ ~HistoricalSelectedRequestAuthorized(node)
    => /\ UNCHANGED AsyncIoContentTypeVars
       /\ UNCHANGED AsyncIoTopologyTypeVars
       /\ UNCHANGED AsyncIoCapacityTypeVars
PROOF
  <1>1. ASSUME NEW node,
                DrainHistoricalIngressSelected(node),
                ~HistoricalSelectedRequestAuthorized(node)
         PROVE /\ UNCHANGED AsyncIoContentTypeVars
                /\ UNCHANGED AsyncIoTopologyTypeVars
                /\ UNCHANGED AsyncIoCapacityTypeVars
    <2>1. UNCHANGED AsyncIoVars
      BY <1>1, Isa
         DEF DrainHistoricalIngressSelected,
             HistoricalSelectedRequestAuthorized,
             HistoricalSelectedItem
    <2>2. UNCHANGED <<asyncCommandQueues,
                      asyncDeferredCompletionQueues>>
      BY <1>1, Isa
         DEF DrainHistoricalIngressSelected,
             AsyncDeferredVars
    <2> QED BY <2>1, <2>2
         DEF AsyncIoContentTypeVars, AsyncIoTopologyTypeVars,
             AsyncIoCapacityTypeVars, AsyncIoVars
  <1> QED BY <1>1

THEOREM HistoricalDrainRunnerPreservesSchedulerType ==
  \A node \in AsyncResponsiveAppliedArchiveServers:
    /\ AsyncTypeInvariant
    /\ RunHistoricalServer(node)
    /\ HistoricalDrainableIngressIndices(node) # {}
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncResponsiveAppliedArchiveServers,
                AsyncTypeInvariant,
                RunHistoricalServer(node),
                HistoricalDrainableIngressIndices(node) # {}
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE DrainIndex == FirstHistoricalDrainableIngressIndex(node)
    <2> DEFINE DrainSource == asyncIngressReady[node][DrainIndex]
    <2> DEFINE DrainLaneIndex ==
           HistoricalSelectedIngressLaneIndex(node, DrainIndex)
    <2> DEFINE DrainItem ==
           HistoricalSelectedIngressItemAt(node, DrainIndex)
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncResponsiveAppliedArchiveServersAreValidators
    <2>2. /\ DrainHistoricalIngressSelected(node)
           /\ PopSelectedIngress(node, DrainIndex, DrainLaneIndex)
      BY <1>1
         DEF RunHistoricalServer, DrainHistoricalIngressSelected,
             DrainIndex, DrainLaneIndex
    <2>3. DrainIndex \in HistoricalDrainableIngressIndices(node)
      BY <1>1, FirstHistoricalDrainableIndexIsDrainable DEF DrainIndex
    <2>4. /\ DrainIndex \in 1..Len(asyncIngressReady[node])
           /\ HistoricalIngressSourceCanDrain(node, DrainSource)
           /\ DrainLaneIndex \in
                HistoricalDrainableIngressLaneIndices(node, DrainSource)
           /\ DrainLaneIndex \in 1..Len(IngressLane(node, DrainSource))
           /\ HistoricalIngressItemCanDrain(node, DrainItem)
      <3>1. /\ DrainIndex \in 1..Len(asyncIngressReady[node])
             /\ HistoricalIngressSourceCanDrain(node, DrainSource)
        BY <2>3
           DEF HistoricalDrainableIngressIndices, DrainSource
      <3>2. DrainLaneIndex \in
               HistoricalDrainableIngressLaneIndices(node, DrainSource)
        BY <3>1, FirstHistoricalDrainableIngressLaneIndexIsDrainable
           DEF HistoricalIngressSourceCanDrain, DrainLaneIndex,
               HistoricalSelectedIngressLaneIndex
      <3> QED BY <3>1, <3>2
           DEF HistoricalDrainableIngressLaneIndices, DrainItem,
               HistoricalSelectedIngressItemAt, DrainSource,
               DrainLaneIndex, HistoricalSelectedIngressLaneIndex
    <2>5. /\ AsyncConfiguration
           /\ ModelConfiguration
           /\ AsyncIngressTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant,
             AsyncIngressTypeInvariant, TypeInvariant
    <2>6. AsyncIngressTypeInvariant'
      BY <2>1, <2>2, <2>4, <2>5,
         PopSelectedIngressPreservesIngressType
    <2>7. AsyncItemTyped(DrainItem)
      BY <2>1, <2>4, <2>5, SelectedIngressItemIsTyped
         DEF DrainItem, HistoricalSelectedIngressItemAt,
             DrainLaneIndex, DrainSource
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
             DEF HistoricalIngressItemCanDrain, DrainItem
        <4>2. /\ asyncIoQueues' =
                    [asyncIoQueues EXCEPT
                       ![node] = Append(
                         @, AsyncIoCertifiedServeJob(
                              node, DeliveryCandidate(DrainItem)))]
               /\ UNCHANGED <<asyncCommandQueues,
                               asyncOutstandingWork,
                               asyncIoReadyCompletions,
                               asyncLocalReadyCompletions,
                               asyncNextCompletionSource,
                               asyncIoControlAvailable,
                               asyncDeferredCompletionQueues>>
          BY <2>2, <3>1, HistoricalAuthorizedRequestFrame
             DEF HistoricalSelectedRequestAuthorized,
                 HistoricalSelectedItem, DrainItem, DrainIndex
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
        <4>2. /\ UNCHANGED AsyncIoContentTypeVars
               /\ UNCHANGED AsyncIoTopologyTypeVars
               /\ UNCHANGED AsyncIoCapacityTypeVars
          BY <2>2, <3>2, HistoricalRejectedRequestFrame
             DEF HistoricalSelectedRequestAuthorized,
                 HistoricalSelectedItem, DrainItem, DrainIndex
        <4> QED BY <4>1, <4>2,
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
             AsyncTransportContentTypeVars,
             AsyncCertifiedResponseClaimAuthorityVars, vars
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
    <2>14. UNCHANGED AsyncHistoricalRecoveryFrameVars
      BY <1>1, <2>2, Isa
         DEF RunHistoricalServer, DrainHistoricalIngressSelected,
             AsyncHistoricalRecoveryFrameVars, vars
    <2>15. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, <2>14, HistoricalRecoveryFramePreservesType
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant
    <2> QED BY <2>6, <2>8, <2>11, <2>13, <2>15
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

=============================================================================
