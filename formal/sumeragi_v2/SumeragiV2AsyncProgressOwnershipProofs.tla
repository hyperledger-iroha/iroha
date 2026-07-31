---- MODULE SumeragiV2AsyncProgressOwnershipProofs ----
EXTENDS SumeragiV2AsyncFairServiceProofs

(***************************************************************************
Exact ownership induction for the asynchronous executor.

The progress rank may use an ownership carrier only after that carrier has
been proved invariant on every concrete scheduler transition.  Keep the
relevant variable tuple deliberately smaller than `AsyncAllVars`: network
history, clocks, and protocol certificates may change without changing an
executor owner, while the pending/signing sets are included because they are
the source of each Busy completion witness.
***************************************************************************)

AsyncProgressOwnershipCoreVars ==
  <<pendingProposal, pendingPrepare, pendingObservePrepare,
    pendingLockCommit, pendingTimeout, pendingInstallTC, pendingDecision,
    signProposals, signVotes, signTimeouts>>

AsyncBusyConsumerVars ==
  <<context, nodeView, generation>>

AsyncProgressOwnershipSchedulerVars ==
  <<asyncCommandQueues, asyncIoQueues, asyncOutstandingWork,
    asyncIoReadyCompletions, asyncLocalReadyCompletions,
    asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncCausalQueues>>

AsyncProgressOwnershipVars ==
  <<AsyncProgressOwnershipCoreVars, AsyncBusyConsumerVars,
    AsyncProgressOwnershipSchedulerVars>>

THEOREM AsyncProgressOwnershipStutter ==
  AsyncProgressOwnershipInvariant /\ UNCHANGED AsyncProgressOwnershipVars
    => AsyncProgressOwnershipInvariant'
BY Isa
   DEF AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       SerializedBusyOwnershipInvariant, BusyCompletionWitnessInvariant,
       AsyncProgressOwnershipVars, AsyncProgressOwnershipCoreVars,
       AsyncProgressOwnershipSchedulerVars, QueuedCandidates,
       DeferredCandidates, CausalCandidates, TrackedWorkCandidates,
       ConsensusIoCandidates, SerializedBusyOwners,
       BusyCompletionCandidates, ActiveBusyCompletionCarrier,
       CandidateConsumerCurrent, AsyncBusyConsumerVars,
       SequenceHasUniqueValues, SequenceSet, NodeIdle, PendingNodes,
       SigningNodes, AllPendingRequests, RequestNodeSet,
       RequestsUniqueByNode

THEOREM AsyncInitEstablishesProgressOwnership ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncProgressOwnershipInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncProgressOwnershipInvariant
    <2>1. \A node \in ValidatorIds:
             /\ asyncCommandQueues[node] = <<>>
             /\ asyncCausalQueues[node] =
                  <<NoItemCandidate(
                      "Normal", "AssembleBody", node, nodeView[node],
                      AsyncProposalSubject(node))>>
             /\ asyncDeferredCompletionQueues[node] = <<>>
             /\ asyncDeferredProgressQueues[node] = <<>>
             /\ asyncDeferredNormalQueues[node] = <<>>
             /\ asyncOutstandingWork[node] = {}
      BY <1>1, Isa
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncRuntimeInit,
             AsyncIoInit, AsyncDeferredInit
    <2>2. \A node \in ValidatorIds:
             /\ SequenceHasUniqueValues(asyncCommandQueues[node])
             /\ SequenceHasUniqueValues(asyncCausalQueues[node])
             /\ SequenceHasUniqueValues(
                  asyncDeferredCompletionQueues[node])
             /\ SequenceHasUniqueValues(
                  asyncDeferredProgressQueues[node])
             /\ SequenceHasUniqueValues(
                  asyncDeferredNormalQueues[node])
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE /\ SequenceHasUniqueValues(asyncCommandQueues[node])
                   /\ SequenceHasUniqueValues(asyncCausalQueues[node])
                   /\ SequenceHasUniqueValues(
                        asyncDeferredCompletionQueues[node])
                   /\ SequenceHasUniqueValues(
                        asyncDeferredProgressQueues[node])
                   /\ SequenceHasUniqueValues(
                        asyncDeferredNormalQueues[node])
        <4>1. SequenceSet(
                 <<NoItemCandidate(
                     "Normal", "AssembleBody", node, nodeView[node],
                     AsyncProposalSubject(node))>>) =
                 {NoItemCandidate(
                    "Normal", "AssembleBody", node, nodeView[node],
                    AsyncProposalSubject(node))}
          BY SingletonSequenceFacts, RangeEquality
             DEF SequenceSet
        <4>2. Len(
                 <<NoItemCandidate(
                     "Normal", "AssembleBody", node, nodeView[node],
                     AsyncProposalSubject(node))>>) = 1
          BY Isa
        <4>3. Cardinality(
                 {NoItemCandidate(
                    "Normal", "AssembleBody", node, nodeView[node],
                    AsyncProposalSubject(node))}) = 1
          BY FS_Singleton
        <4>4. SequenceHasUniqueValues(
                 <<NoItemCandidate(
                     "Normal", "AssembleBody", node, nodeView[node],
                     AsyncProposalSubject(node))>>)
          BY <4>1, <4>2, <4>3 DEF SequenceHasUniqueValues
        <4> QED BY <2>1, <3>1, <4>4, EmptyReplayProperties
      <3> QED BY <3>1
    <2>3. /\ QueuedCandidates = {}
           /\ DeferredCandidates = {}
           /\ TrackedWorkCandidates = {}
      BY <2>1, Isa
         DEF QueuedCandidates, DeferredCandidates,
             TrackedWorkCandidates, SequenceSet
    <2>4. AsyncLogicalCandidateOwnershipInvariant
      BY <2>2, <2>3, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>5. AsyncOutstandingCarrierInvariant
      BY <1>1, Isa
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncIoInit,
             AsyncOutstandingCarrierInvariant,
             ConsensusIoCandidates, SequenceSet
    <2>6. SerializedBusyOwnershipInvariant
      BY <1>1, Isa
         DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
             SerializedBusyOwnershipInvariant, SerializedBusyOwners,
             AllPendingRequests, RequestNodeSet, RequestsUniqueByNode
    <2>7. BusyCompletionWitnessInvariant
      BY <1>1, Isa
         DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
             AsyncRuntimeInit, AsyncIoInit,
             BusyCompletionWitnessInvariant, BusyCompletionCandidates,
             ActiveBusyCompletionCarrier, QueuedCandidates,
             CausalCandidates, TrackedWorkCandidates, SequenceSet,
             NodeIdle, PendingNodes, SigningNodes, AllPendingRequests,
             RequestNodeSet, NoItemCandidate, AsyncCandidate
    <2> QED BY <2>4, <2>5, <2>6, <2>7
         DEF AsyncProgressOwnershipInvariant
  <1> QED BY <1>1

THEOREM AsyncAllVarsStutterPreservesProgressOwnership ==
  AsyncProgressOwnershipInvariant /\ UNCHANGED AsyncAllVars
    => AsyncProgressOwnershipInvariant'
BY AsyncProgressOwnershipStutter, Isa
   DEF AsyncAllVars, AsyncSchedulerVars,
       AsyncProgressOwnershipVars, AsyncProgressOwnershipCoreVars,
       AsyncProgressOwnershipSchedulerVars, vars

THEOREM AsyncSetGstPreservesProgressOwnership ==
  AsyncProgressOwnershipInvariant /\ AsyncSetGST
    => AsyncProgressOwnershipInvariant'
BY AsyncProgressOwnershipStutter, Isa
   DEF AsyncSetGST, SetGST, AsyncSchedulerVars,
       AsyncProgressOwnershipVars, AsyncProgressOwnershipCoreVars,
       AsyncProgressOwnershipSchedulerVars, vars

THEOREM AsyncTickPreservesProgressOwnership ==
  AsyncProgressOwnershipInvariant /\ AsyncTick
    => AsyncProgressOwnershipInvariant'
BY AsyncProgressOwnershipStutter, Isa
   DEF AsyncTick, AsyncNonClockVars, AsyncIoVars, AsyncDeferredVars,
       AsyncLocalAdmissionVars, AsyncProgressOwnershipVars,
       AsyncProgressOwnershipCoreVars,
       AsyncProgressOwnershipSchedulerVars, vars

THEOREM AdmitIngressPacketPreservesProgressOwnership ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncProgressOwnershipInvariant /\ AdmitIngressPacket(recipient, source)
      => AsyncProgressOwnershipInvariant'
BY AsyncProgressOwnershipStutter, Isa
   DEF AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       AsyncIoVars, AsyncDeferredVars, LeaveCausalQueues,
       AsyncLocalAdmissionVars, AsyncProgressOwnershipVars,
       AsyncProgressOwnershipCoreVars,
       AsyncProgressOwnershipSchedulerVars, vars

THEOREM TransportOnlyFaultPreservesProgressOwnership ==
  \A packet, source, recipient, nonce, kind, qc, signer, roundView,
     subject, timeoutCertificate, highestPrepare, phase:
    /\ AsyncProgressOwnershipInvariant
    /\ \/ PreGstLosePacket(packet)
       \/ InjectByzantineNoise(source, recipient, nonce)
       \/ InjectUntrustedTransportCompletion(kind, recipient, nonce)
       \/ InjectAuthenticatedJunk(kind, source, recipient, nonce)
       \/ InjectByzantineCertifiedRequest(source, recipient, qc, nonce)
       \/ AsyncByzantineProposal(
            signer, roundView, subject,
            timeoutCertificate, highestPrepare)
       \/ AsyncByzantineVote(signer, roundView, phase, subject)
       \/ AsyncByzantineTimeout(signer, roundView, highestPrepare)
    => AsyncProgressOwnershipInvariant'
BY AsyncProgressOwnershipStutter, Isa
   DEF PreGstLosePacket, InjectByzantineNoise,
       InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk, InjectByzantineCertifiedRequest,
       AsyncByzantineProposal, AsyncByzantineVote,
       AsyncByzantineTimeout, ByzantineBroadcastProposal,
       ByzantineBroadcastVote, ByzantineBroadcastTimeout,
       PublishEphemeralItems, PacketsForItems, NoSendItem,
       AsyncIoVars, AsyncDeferredVars, LeaveCausalQueues,
       AsyncLocalAdmissionVars, AsyncProgressOwnershipVars,
       AsyncProgressOwnershipCoreVars,
       AsyncProgressOwnershipSchedulerVars, AsyncAuxVars, vars

THEOREM CrashWithSchedulerFramePreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncProgressOwnershipInvariant
    /\ Crash(node)
    /\ UNCHANGED AsyncSchedulerVars
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncProgressOwnershipInvariant,
                Crash(node),
                UNCHANGED AsyncSchedulerVars
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. AsyncLogicalCandidateOwnershipInvariant'
      BY <1>1, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant,
             AsyncSchedulerVars, QueuedCandidates, DeferredCandidates,
             CausalCandidates, TrackedWorkCandidates,
             SequenceHasUniqueValues, SequenceSet
    <2>2. AsyncOutstandingCarrierInvariant'
      BY <1>1, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncOutstandingCarrierInvariant, AsyncSchedulerVars,
             ConsensusIoCandidates, SequenceSet
    <2>3. SerializedBusyOwners' \subseteq SerializedBusyOwners
      BY <1>1, Isa
         DEF Crash, SerializedBusyOwners, AllPendingRequests
    <2>4. SerializedBusyOwnershipInvariant'
      <3>1. RequestsUniqueByNode(SerializedBusyOwners)
        BY <1>1
           DEF AsyncProgressOwnershipInvariant,
               SerializedBusyOwnershipInvariant
      <3> QED BY <2>3, <3>1,
           RemovingRequestsPreservesNodeUniqueness
           DEF SerializedBusyOwnershipInvariant
    <2>5. ActiveBusyCompletionCarrier' =
             ActiveBusyCompletionCarrier
      BY <1>1, Isa
         DEF AsyncSchedulerVars, ActiveBusyCompletionCarrier,
             QueuedCandidates, CausalCandidates,
             TrackedWorkCandidates, SequenceSet
    <2>6. NodeIdle(node)'
      BY <1>1, Isa
         DEF Crash, NodeIdle, PendingNodes, SigningNodes,
             AllPendingRequests, RequestNodeSet
    <2>7. \A other \in ValidatorIds \ {node}:
             NodeIdle(other)' <=> NodeIdle(other)
      BY <1>1, Isa
         DEF Crash, NodeIdle, PendingNodes, SigningNodes,
             AllPendingRequests, RequestNodeSet
    <2>8. \A other \in ValidatorIds \ {node}:
             BusyCompletionCandidates(other)
               \subseteq BusyCompletionCandidates(other)'
      BY <1>1, <2>5, Isa
         DEF Crash, BusyCompletionCandidates,
             CandidateConsumerCurrent, ActiveBusyCompletionCarrier
    <2>9. BusyCompletionWitnessInvariant'
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE ~NodeIdle(other)'
                     => \/ BusyCompletionCandidates(other)' # {}
                        \/ InstallGenerationExhausted(other)'
        <4>1. CASE other = node
          <5>1. NodeIdle(other)'
            BY <2>6, <4>1
          <5> QED BY <5>1
        <4>2. CASE other # node
          <5>1. other \in ValidatorIds \ {node}
            BY <3>1, <4>2
          <5>2. ASSUME ~NodeIdle(other)'
                 PROVE \/ BusyCompletionCandidates(other)' # {}
                       \/ InstallGenerationExhausted(other)'
            <6>1. ~NodeIdle(other)
              BY <2>7, <5>1, <5>2
            <6>2. \/ BusyCompletionCandidates(other) # {}
                   \/ InstallGenerationExhausted(other)
              BY <1>1, <6>1
                 DEF AsyncProgressOwnershipInvariant,
                     BusyCompletionWitnessInvariant
            <6>3. CASE BusyCompletionCandidates(other) # {}
              <7>1. PICK candidate \in
                           BusyCompletionCandidates(other): TRUE
                BY <6>3
              <7>2. candidate \in BusyCompletionCandidates(other)'
                BY <2>8, <5>1, <7>1
              <7> QED BY <7>2
            <6>4. CASE InstallGenerationExhausted(other)
              <7>1. InstallGenerationExhausted(other)'
                BY <1>1, <5>1, <6>4, Isa
                   DEF Crash, InstallGenerationExhausted
              <7> QED BY <7>1
            <6> QED BY <6>2, <6>3, <6>4
          <5> QED BY <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1 DEF BusyCompletionWitnessInvariant
    <2> QED BY <2>1, <2>2, <2>4, <2>9
         DEF AsyncProgressOwnershipInvariant
  <1> QED BY <1>1

THEOREM PreGstCrashPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncProgressOwnershipInvariant
    /\ PreGstCrash(node)
    => AsyncProgressOwnershipInvariant'
BY CrashWithSchedulerFramePreservesProgressOwnership
   DEF PreGstCrash

THEOREM PreGstResponsiveCrashPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncProgressOwnershipInvariant
    /\ PreGstResponsiveCrash(node)
    => AsyncProgressOwnershipInvariant'
BY CrashWithSchedulerFramePreservesProgressOwnership
   DEF PreGstResponsiveCrash

THEOREM PreGstResponsiveRestartPreservesProgressOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ PreGstResponsiveRestart
  => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              PreGstResponsiveRestart
         PROVE AsyncProgressOwnershipInvariant'
    <2> DEFINE Node == asyncRecoveryNode
    <2>1. /\ Node \in ValidatorIds
           /\ NodeIdle(Node)
           /\ context' = context
           /\ nodeView' = nodeView
           /\ generation' = [generation EXCEPT ![Node] = @ + 1]
           /\ UNCHANGED AsyncProgressOwnershipCoreVars
           /\ UNCHANGED AsyncProgressOwnershipSchedulerVars
      BY <1>1, Isa
         DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
             PreGstResponsiveRestart, Restart, AsyncSchedulerVars,
             AsyncProgressOwnershipCoreVars,
             AsyncProgressOwnershipSchedulerVars, Node, vars
    <2>2. /\ AsyncLogicalCandidateOwnershipInvariant'
           /\ AsyncOutstandingCarrierInvariant'
           /\ SerializedBusyOwnershipInvariant'
      BY <1>1, <2>1, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant,
             AsyncOutstandingCarrierInvariant,
             SerializedBusyOwnershipInvariant,
             QueuedCandidates, DeferredCandidates, CausalCandidates,
             TrackedWorkCandidates, ConsensusIoCandidates,
             SerializedBusyOwners, SequenceHasUniqueValues, SequenceSet
    <2>3. NodeIdle(Node)'
      BY <2>1, Isa
         DEF AsyncProgressOwnershipCoreVars, NodeIdle, PendingNodes,
             SigningNodes, AllPendingRequests
    <2>4. \A other \in ValidatorIds \ {Node}:
             /\ (NodeIdle(other)' <=> NodeIdle(other))
             /\ BusyCompletionCandidates(other)
                  \subseteq BusyCompletionCandidates(other)'
      BY <1>1, <2>1, Isa
         DEF BusyCompletionCandidates, CandidateConsumerCurrent,
             ActiveBusyCompletionCarrier, QueuedCandidates,
             CausalCandidates, TrackedWorkCandidates,
             AsyncProgressOwnershipCoreVars,
             AsyncProgressOwnershipSchedulerVars,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant,
             AsyncCausalTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoTopologyTypeInvariant, NodeIdle, Node
    <2>5. BusyCompletionWitnessInvariant'
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE ~NodeIdle(other)'
                     => \/ BusyCompletionCandidates(other)' # {}
                        \/ InstallGenerationExhausted(other)'
        <4>1. CASE other = Node
          BY <2>3, <4>1
        <4>2. CASE other # Node
          <5>1. other \in ValidatorIds \ {Node}
            BY <3>1, <4>2
          <5>2. ASSUME ~NodeIdle(other)'
                 PROVE \/ BusyCompletionCandidates(other)' # {}
                       \/ InstallGenerationExhausted(other)'
            <6>1. ~NodeIdle(other)
              BY <2>4, <5>1, <5>2
            <6>2. \/ BusyCompletionCandidates(other) # {}
                   \/ InstallGenerationExhausted(other)
              BY <1>1, <6>1
                 DEF AsyncProgressOwnershipInvariant,
                     BusyCompletionWitnessInvariant
            <6>3. CASE BusyCompletionCandidates(other) # {}
              <7>1. PICK candidate \in
                           BusyCompletionCandidates(other): TRUE
                BY <6>3
              <7> QED BY <2>4, <5>1, <7>1
            <6>4. CASE InstallGenerationExhausted(other)
              <7>1. /\ generation'[other] = generation[other]
                     /\ pendingInstallTC' = pendingInstallTC
                BY <1>1, <2>1, <5>1,
                   FunctionalUpdateAwayFromKey, Isa
                   DEF AsyncProgressOwnershipCoreVars, Node
              <7>2. InstallGenerationExhausted(other)'
                BY <6>4, <7>1 DEF InstallGenerationExhausted
              <7> QED BY <7>2
            <6> QED BY <6>2, <6>3, <6>4
          <5> QED BY <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1 DEF BusyCompletionWitnessInvariant
    <2> QED BY <2>2, <2>5 DEF AsyncProgressOwnershipInvariant
  <1> QED BY <1>1

THEOREM FreshCausalAppendPreservesLogicalOwnership ==
  \A node \in ValidatorIds:
  \A fresh:
    /\ AsyncCausalTypeInvariant
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ AsyncQueueTyped(fresh)
    /\ AsyncCausalQueueOwnership(node, fresh)
    /\ SequenceHasUniqueValues(fresh)
    /\ SequenceSet(fresh) \cap
         (QueuedCandidates \cup DeferredCandidates \cup
            CausalCandidates \cup TrackedWorkCandidates) = {}
    /\ asyncCausalQueues' =
         [asyncCausalQueues EXCEPT ![node] = @ \o fresh]
    /\ UNCHANGED
         <<asyncCommandQueues, asyncOutstandingWork,
           asyncDeferredCompletionQueues,
           asyncDeferredProgressQueues,
           asyncDeferredNormalQueues>>
    => /\ AsyncLogicalCandidateOwnershipInvariant'
       /\ CausalCandidates' =
            CausalCandidates \cup SequenceSet(fresh)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW fresh,
                AsyncCausalTypeInvariant,
                AsyncLogicalCandidateOwnershipInvariant,
                AsyncQueueTyped(fresh),
                AsyncCausalQueueOwnership(node, fresh),
                SequenceHasUniqueValues(fresh),
                SequenceSet(fresh) \cap
                  (QueuedCandidates \cup DeferredCandidates \cup
                     CausalCandidates \cup TrackedWorkCandidates) = {},
                asyncCausalQueues' =
                  [asyncCausalQueues EXCEPT ![node] = @ \o fresh],
                UNCHANGED
                  <<asyncCommandQueues, asyncOutstandingWork,
                    asyncDeferredCompletionQueues,
                    asyncDeferredProgressQueues,
                    asyncDeferredNormalQueues>>
         PROVE /\ AsyncLogicalCandidateOwnershipInvariant'
               /\ CausalCandidates' =
                    CausalCandidates \cup SequenceSet(fresh)
    <2>1. /\ AsyncQueueTyped(asyncCausalQueues[node])
           /\ AsyncCausalQueueOwnership(
                node, asyncCausalQueues[node])
           /\ SequenceHasUniqueValues(asyncCausalQueues[node])
      BY <1>1, Isa
         DEF AsyncCausalTypeInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>2. SequenceSet(asyncCausalQueues[node])
             \subseteq CausalCandidates
      BY <1>1, Isa DEF CausalCandidates
    <2>3. SequenceSet(fresh) \cap CausalCandidates = {}
      BY <1>1, Isa
    <2>4. SequenceSet(asyncCausalQueues[node]) \cap
             SequenceSet(fresh) = {}
      BY <2>2, <2>3, Isa
    <2>5. /\ AsyncQueueTyped(asyncCausalQueues[node] \o fresh)
           /\ AsyncCausalQueueOwnership(
                node, asyncCausalQueues[node] \o fresh)
           /\ SequenceHasUniqueValues(
                asyncCausalQueues[node] \o fresh)
      BY <1>1, <2>1, <2>4,
         ConcatTypedOwnedDisjointReplay
    <2>6. Range(asyncCausalQueues[node] \o fresh) =
             Range(asyncCausalQueues[node]) \cup Range(fresh)
      BY <1>1, <2>1, RangeConcatenation
         DEF AsyncQueueTyped
    <2>7. /\ SequenceSet(asyncCausalQueues[node]) =
                  Range(asyncCausalQueues[node])
           /\ SequenceSet(fresh) = Range(fresh)
           /\ SequenceSet(asyncCausalQueues[node] \o fresh) =
                  Range(asyncCausalQueues[node] \o fresh)
      BY <1>1, <2>1, RangeEquality
         DEF AsyncQueueTyped, SequenceSet
    <2>8. SequenceSet(asyncCausalQueues[node] \o fresh) =
             SequenceSet(asyncCausalQueues[node]) \cup SequenceSet(fresh)
      BY <2>6, <2>7
    <2>9. asyncCausalQueues'[node] =
             asyncCausalQueues[node] \o fresh
      BY <1>1, FunctionalConcatUpdateAtKey
         DEF AsyncCausalTypeInvariant
    <2>10. SequenceSet(asyncCausalQueues'[node]) =
             SequenceSet(asyncCausalQueues[node]) \cup SequenceSet(fresh)
      BY <2>8, <2>9
    <2>11. \A other \in ValidatorIds \ {node}:
             asyncCausalQueues'[other] = asyncCausalQueues[other]
      BY <1>1, FunctionalUpdateAwayFromKey
         DEF AsyncCausalTypeInvariant
    <2>12. CausalCandidates' \subseteq
             CausalCandidates \cup SequenceSet(fresh)
      <3>1. ASSUME NEW candidate \in CausalCandidates'
             PROVE candidate \in
                     CausalCandidates \cup SequenceSet(fresh)
        <4>1. PICK other \in ValidatorIds:
                   candidate \in SequenceSet(asyncCausalQueues'[other])
          BY <3>1 DEF CausalCandidates
        <4>2. CASE other = node
          BY <2>2, <2>10, <4>1, <4>2, Isa
        <4>3. CASE other # node
          <5>1. other \in ValidatorIds \ {node}
            BY <4>1, <4>3
          <5> QED BY <2>11, <4>1, <5>1, Isa
               DEF CausalCandidates
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>13. CausalCandidates \cup SequenceSet(fresh)
               \subseteq CausalCandidates'
      <3>1. ASSUME NEW candidate \in
                    CausalCandidates \cup SequenceSet(fresh)
             PROVE candidate \in CausalCandidates'
        <4>1. CASE candidate \in SequenceSet(fresh)
          BY <2>10, <4>1, Isa DEF CausalCandidates
        <4>2. CASE candidate \in CausalCandidates
          <5>1. PICK other \in ValidatorIds:
                   candidate \in SequenceSet(asyncCausalQueues[other])
            BY <4>2 DEF CausalCandidates
          <5>2. CASE other = node
            BY <2>10, <5>1, <5>2, Isa DEF CausalCandidates
          <5>3. CASE other # node
            <6>1. other \in ValidatorIds \ {node}
              BY <5>1, <5>3
            <6> QED BY <2>11, <5>1, <6>1, Isa
                 DEF CausalCandidates
          <5> QED BY <5>2, <5>3
        <4> QED BY <3>1, <4>1, <4>2
      <3> QED BY <3>1
    <2>14. CausalCandidates' =
             CausalCandidates \cup SequenceSet(fresh)
      BY <2>12, <2>13, Isa
    <2>15. \A other \in ValidatorIds:
             SequenceHasUniqueValues(asyncCausalQueues'[other])
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE SequenceHasUniqueValues(asyncCausalQueues'[other])
        <4>1. CASE other = node
          BY <2>5, <2>9, <4>1
        <4>2. CASE other # node
          <5>1. other \in ValidatorIds \ {node}
            BY <3>1, <4>2
          <5> QED BY <1>1, <2>11, <5>1
               DEF AsyncLogicalCandidateOwnershipInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>16. /\ QueuedCandidates' = QueuedCandidates
            /\ DeferredCandidates' = DeferredCandidates
            /\ TrackedWorkCandidates' = TrackedWorkCandidates
      BY <1>1, Isa
         DEF QueuedCandidates, DeferredCandidates,
             TrackedWorkCandidates, SequenceSet
    <2>17. QueuedCandidates' \cap DeferredCandidates' = {}
      BY <1>1, <2>16, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>18. QueuedCandidates' \cap CausalCandidates' = {}
      BY <1>1, <2>3, <2>14, <2>16, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>19. QueuedCandidates' \cap TrackedWorkCandidates' = {}
      BY <1>1, <2>16, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>20. DeferredCandidates' \cap CausalCandidates' = {}
      BY <1>1, <2>3, <2>14, <2>16, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>21. DeferredCandidates' \cap TrackedWorkCandidates' = {}
      BY <1>1, <2>16, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>22. CausalCandidates' \cap TrackedWorkCandidates' = {}
      BY <1>1, <2>3, <2>14, <2>16, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>23. AsyncLogicalCandidateOwnershipInvariant'
      BY <1>1, <2>15, <2>17, <2>18, <2>19, <2>20, <2>21,
         <2>22
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2> QED BY <2>14, <2>23
  <1> QED BY <1>1

THEOREM RestartResetPreservesLogicalOwnership ==
  \A node \in ValidatorIds:
  \A replay:
    /\ AsyncSchedulerTypeInvariant
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ AsyncQueueTyped(replay)
    /\ AsyncCausalQueueOwnership(node, replay)
    /\ SequenceHasUniqueValues(replay)
    /\ ResetNodeSchedulerForRestart(node, replay)
    => AsyncLogicalCandidateOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW replay,
                AsyncSchedulerTypeInvariant,
                AsyncLogicalCandidateOwnershipInvariant,
                AsyncQueueTyped(replay),
                AsyncCausalQueueOwnership(node, replay),
                SequenceHasUniqueValues(replay),
                ResetNodeSchedulerForRestart(node, replay)
         PROVE AsyncLogicalCandidateOwnershipInvariant'
    <2>1. /\ DOMAIN asyncCommandQueues = ValidatorIds
           /\ DOMAIN asyncCausalQueues = ValidatorIds
           /\ DOMAIN asyncDeferredCompletionQueues = ValidatorIds
           /\ DOMAIN asyncDeferredProgressQueues = ValidatorIds
           /\ DOMAIN asyncDeferredNormalQueues = ValidatorIds
           /\ DOMAIN asyncOutstandingWork = ValidatorIds
      BY <1>1
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncCausalTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant,
             AsyncDeferredTypeInvariant,
             AsyncDeferredTopologyTypeInvariant
    <2>2. /\ asyncCommandQueues'[node] = <<>>
           /\ asyncCausalQueues'[node] = replay
           /\ asyncDeferredCompletionQueues'[node] = <<>>
           /\ asyncDeferredProgressQueues'[node] = <<>>
           /\ asyncDeferredNormalQueues'[node] = <<>>
           /\ asyncOutstandingWork'[node] = {}
      BY <1>1, <2>1, FunctionalReplaceUpdateAtKey
         DEF ResetNodeSchedulerForRestart
    <2>3. \A other \in ValidatorIds \ {node}:
             /\ asyncCommandQueues'[other] =
                  asyncCommandQueues[other]
             /\ asyncCausalQueues'[other] = asyncCausalQueues[other]
             /\ asyncDeferredCompletionQueues'[other] =
                  asyncDeferredCompletionQueues[other]
             /\ asyncDeferredProgressQueues'[other] =
                  asyncDeferredProgressQueues[other]
             /\ asyncDeferredNormalQueues'[other] =
                  asyncDeferredNormalQueues[other]
             /\ asyncOutstandingWork'[other] =
                  asyncOutstandingWork[other]
      BY <1>1, <2>1, FunctionalUpdateAwayFromKey
         DEF ResetNodeSchedulerForRestart
    <2>4. \A other \in ValidatorIds:
             /\ SequenceHasUniqueValues(asyncCommandQueues'[other])
             /\ SequenceHasUniqueValues(asyncCausalQueues'[other])
             /\ SequenceHasUniqueValues(
                  asyncDeferredCompletionQueues'[other])
             /\ SequenceHasUniqueValues(
                  asyncDeferredProgressQueues'[other])
             /\ SequenceHasUniqueValues(
                  asyncDeferredNormalQueues'[other])
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE
               /\ SequenceHasUniqueValues(asyncCommandQueues'[other])
               /\ SequenceHasUniqueValues(asyncCausalQueues'[other])
               /\ SequenceHasUniqueValues(
                    asyncDeferredCompletionQueues'[other])
               /\ SequenceHasUniqueValues(
                    asyncDeferredProgressQueues'[other])
               /\ SequenceHasUniqueValues(
                    asyncDeferredNormalQueues'[other])
        <4>1. CASE other = node
          BY <1>1, <2>2, <4>1, EmptyReplayProperties
        <4>2. CASE other # node
          <5>1. other \in ValidatorIds \ {node}
            BY <3>1, <4>2
          <5> QED BY <1>1, <2>3, <5>1
               DEF AsyncLogicalCandidateOwnershipInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>5. QueuedCandidates' \subseteq QueuedCandidates
      <3>1. ASSUME NEW candidate \in QueuedCandidates'
             PROVE candidate \in QueuedCandidates
        <4>1. PICK other \in ValidatorIds:
                   candidate \in SequenceSet(asyncCommandQueues'[other])
          BY <3>1 DEF QueuedCandidates
        <4>2. other # node
          BY <2>2, <4>1, Isa DEF SequenceSet
        <4>3. other \in ValidatorIds \ {node}
          BY <4>1, <4>2
        <4> QED BY <2>3, <4>1, <4>3, Isa
             DEF QueuedCandidates
      <3> QED BY <3>1
    <2>6. DeferredCandidates' \subseteq DeferredCandidates
      <3>1. ASSUME NEW candidate \in DeferredCandidates'
             PROVE candidate \in DeferredCandidates
        <4>1. PICK other \in ValidatorIds:
                   candidate \in
                     SequenceSet(asyncDeferredCompletionQueues'[other])
                       \cup SequenceSet(
                              asyncDeferredProgressQueues'[other])
                       \cup SequenceSet(
                              asyncDeferredNormalQueues'[other])
          BY <3>1 DEF DeferredCandidates
        <4>2. other # node
          BY <2>2, <4>1, Isa DEF SequenceSet
        <4>3. other \in ValidatorIds \ {node}
          BY <4>1, <4>2
        <4> QED BY <2>3, <4>1, <4>3, Isa
             DEF DeferredCandidates
      <3> QED BY <3>1
    <2>7. TrackedWorkCandidates' \subseteq TrackedWorkCandidates
      <3>1. ASSUME NEW candidate \in TrackedWorkCandidates'
             PROVE candidate \in TrackedWorkCandidates
        <4>1. PICK other \in ValidatorIds:
                   candidate \in asyncOutstandingWork'[other]
          BY <3>1 DEF TrackedWorkCandidates
        <4>2. other # node
          BY <2>2, <4>1
        <4>3. other \in ValidatorIds \ {node}
          BY <4>1, <4>2
        <4> QED BY <2>3, <4>1, <4>3, Isa
             DEF TrackedWorkCandidates
      <3> QED BY <3>1
    <2>8. CausalCandidates' \subseteq
             CausalCandidates \cup SequenceSet(replay)
      <3>1. ASSUME NEW candidate \in CausalCandidates'
             PROVE candidate \in
                     CausalCandidates \cup SequenceSet(replay)
        <4>1. PICK other \in ValidatorIds:
                   candidate \in SequenceSet(asyncCausalQueues'[other])
          BY <3>1 DEF CausalCandidates
        <4>2. CASE other = node
          BY <2>2, <4>1, <4>2
        <4>3. CASE other # node
          <5>1. other \in ValidatorIds \ {node}
            BY <4>1, <4>3
          <5> QED BY <2>3, <4>1, <5>1, Isa
               DEF CausalCandidates
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>9. \A candidate \in QueuedCandidates': candidate.node # node
      <3>1. ASSUME NEW candidate \in QueuedCandidates'
             PROVE candidate.node # node
        <4>1. PICK other \in ValidatorIds:
                   candidate \in SequenceSet(asyncCommandQueues'[other])
          BY <3>1 DEF QueuedCandidates
        <4>2. other # node
          BY <2>2, <4>1, Isa DEF SequenceSet
        <4>3. other \in ValidatorIds \ {node}
          BY <4>1, <4>2
        <4>4. candidate.node = other
          BY <1>1, <2>3, <4>1, <4>3
             DEF AsyncSchedulerTypeInvariant,
                 AsyncRuntimeTypeInvariant,
                 AsyncRuntimeScalarTypeInvariant,
                 AsyncCommandQueueOwnership
        <4> QED BY <4>2, <4>4
      <3> QED BY <3>1
    <2>10. \A candidate \in DeferredCandidates':
              candidate.node # node
      <3>1. ASSUME NEW candidate \in DeferredCandidates'
             PROVE candidate.node # node
        <4>1. PICK other \in ValidatorIds:
                   candidate \in
                     SequenceSet(asyncDeferredCompletionQueues'[other])
                       \cup SequenceSet(
                              asyncDeferredProgressQueues'[other])
                       \cup SequenceSet(
                              asyncDeferredNormalQueues'[other])
          BY <3>1 DEF DeferredCandidates
        <4>2. other # node
          BY <2>2, <4>1, Isa DEF SequenceSet
        <4>3. other \in ValidatorIds \ {node}
          BY <4>1, <4>2
        <4>4. candidate.node = other
          BY <1>1, <2>3, <4>1, <4>3, Isa
             DEF AsyncSchedulerTypeInvariant,
                 AsyncDeferredTypeInvariant,
                 AsyncDeferredContentTypeInvariant,
                 AsyncCommandQueueOwnership
        <4> QED BY <4>2, <4>4
      <3> QED BY <3>1
    <2>11. \A candidate \in TrackedWorkCandidates':
              candidate.node # node
      <3>1. ASSUME NEW candidate \in TrackedWorkCandidates'
             PROVE candidate.node # node
        <4>1. PICK other \in ValidatorIds:
                   candidate \in asyncOutstandingWork'[other]
          BY <3>1 DEF TrackedWorkCandidates
        <4>2. other # node
          BY <2>2, <4>1
        <4>3. other \in ValidatorIds \ {node}
          BY <4>1, <4>2
        <4>4. candidate.node = other
          BY <1>1, <2>3, <4>1, <4>3
             DEF AsyncSchedulerTypeInvariant,
                 AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
                 AsyncIoWorkContentTypeInvariant
        <4> QED BY <4>2, <4>4
      <3> QED BY <3>1
    <2>12. \A candidate \in SequenceSet(replay):
              candidate.node = node
      BY <1>1 DEF AsyncCausalQueueOwnership
    <2>13. SequenceSet(replay) \cap QueuedCandidates' = {}
      BY <2>9, <2>12, Isa
    <2>14. SequenceSet(replay) \cap DeferredCandidates' = {}
      BY <2>10, <2>12, Isa
    <2>15. SequenceSet(replay) \cap TrackedWorkCandidates' = {}
      BY <2>11, <2>12, Isa
    <2>16. QueuedCandidates' \cap DeferredCandidates' = {}
      BY <1>1, <2>5, <2>6, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>17. QueuedCandidates' \cap CausalCandidates' = {}
      BY <1>1, <2>5, <2>8, <2>13, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>18. QueuedCandidates' \cap TrackedWorkCandidates' = {}
      BY <1>1, <2>5, <2>7, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>19. DeferredCandidates' \cap CausalCandidates' = {}
      BY <1>1, <2>6, <2>8, <2>14, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>20. DeferredCandidates' \cap TrackedWorkCandidates' = {}
      BY <1>1, <2>6, <2>7, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>21. CausalCandidates' \cap TrackedWorkCandidates' = {}
      BY <1>1, <2>7, <2>8, <2>15, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2> QED BY <2>4, <2>16, <2>17, <2>18, <2>19, <2>20,
                <2>21
         DEF AsyncLogicalCandidateOwnershipInvariant
  <1> QED BY <1>1

THEOREM ResetNodeSchedulerPreservesOutstandingCarrier ==
  \A node \in ValidatorIds:
  \A replay:
    /\ AsyncSchedulerTypeInvariant
    /\ AsyncOutstandingCarrierInvariant
    /\ ResetNodeSchedulerForRestart(node, replay)
    => AsyncOutstandingCarrierInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW replay,
                AsyncSchedulerTypeInvariant,
                AsyncOutstandingCarrierInvariant,
                ResetNodeSchedulerForRestart(node, replay)
         PROVE AsyncOutstandingCarrierInvariant'
    <2>1. /\ DOMAIN asyncIoQueues = ValidatorIds
           /\ DOMAIN asyncOutstandingWork = ValidatorIds
           /\ DOMAIN asyncIoReadyCompletions = ValidatorIds
           /\ DOMAIN asyncLocalReadyCompletions = ValidatorIds
      BY <1>1
         DEF AsyncSchedulerTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoTopologyTypeInvariant
    <2>2. /\ asyncIoQueues'[node] = <<>>
           /\ asyncOutstandingWork'[node] = {}
           /\ asyncIoReadyCompletions'[node] = <<>>
           /\ asyncLocalReadyCompletions'[node] = <<>>
      BY <1>1, <2>1, FunctionalReplaceUpdateAtKey
         DEF ResetNodeSchedulerForRestart
    <2>3. \A other \in ValidatorIds \ {node}:
             /\ asyncIoQueues'[other] = asyncIoQueues[other]
             /\ asyncOutstandingWork'[other] =
                  asyncOutstandingWork[other]
             /\ asyncIoReadyCompletions'[other] =
                  asyncIoReadyCompletions[other]
             /\ asyncLocalReadyCompletions'[other] =
                  asyncLocalReadyCompletions[other]
      BY <1>1, <2>1, FunctionalUpdateAwayFromKey
         DEF ResetNodeSchedulerForRestart
    <2>4. \A other \in ValidatorIds:
             asyncOutstandingWork'[other] =
               ConsensusIoCandidates(other)'
                 \cup SequenceSet(asyncIoReadyCompletions'[other])
                 \cup SequenceSet(asyncLocalReadyCompletions'[other])
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE asyncOutstandingWork'[other] =
                     ConsensusIoCandidates(other)'
                       \cup SequenceSet(
                            asyncIoReadyCompletions'[other])
                       \cup SequenceSet(
                            asyncLocalReadyCompletions'[other])
        <4>1. CASE other = node
          <5>1. ConsensusIoCandidates(other)' = {}
            BY <2>2, <4>1, Isa
               DEF ConsensusIoCandidates, SequenceSet
          <5> QED BY <2>2, <4>1, <5>1, Isa DEF SequenceSet
        <4>2. CASE other # node
          <5>1. other \in ValidatorIds \ {node}
            BY <3>1, <4>2
          <5>2. ConsensusIoCandidates(other)' =
                   ConsensusIoCandidates(other)
            BY <2>3, <5>1 DEF ConsensusIoCandidates
          <5>3. asyncOutstandingWork[other] =
                  ConsensusIoCandidates(other)
                    \cup SequenceSet(asyncIoReadyCompletions[other])
                    \cup SequenceSet(
                         asyncLocalReadyCompletions[other])
            BY <1>1 DEF AsyncOutstandingCarrierInvariant
          <5> QED BY <2>3, <5>1, <5>2, <5>3
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>4 DEF AsyncOutstandingCarrierInvariant
  <1> QED BY <1>1

THEOREM ProgressCoreStutterFacts ==
  UNCHANGED AsyncProgressOwnershipCoreVars
    => /\ SerializedBusyOwners' = SerializedBusyOwners
       /\ PendingNodes' = PendingNodes
       /\ SigningNodes' = SigningNodes
       /\ \A node: NodeIdle(node)' <=> NodeIdle(node)
PROOF
  <1>1. ASSUME UNCHANGED AsyncProgressOwnershipCoreVars
         PROVE /\ SerializedBusyOwners' = SerializedBusyOwners
               /\ PendingNodes' = PendingNodes
               /\ SigningNodes' = SigningNodes
               /\ \A node: NodeIdle(node)' <=> NodeIdle(node)
    <2>1. /\ pendingProposal' = pendingProposal
           /\ pendingPrepare' = pendingPrepare
           /\ pendingObservePrepare' = pendingObservePrepare
           /\ pendingLockCommit' = pendingLockCommit
           /\ pendingTimeout' = pendingTimeout
           /\ pendingInstallTC' = pendingInstallTC
           /\ pendingDecision' = pendingDecision
           /\ signProposals' = signProposals
           /\ signVotes' = signVotes
           /\ signTimeouts' = signTimeouts
      BY <1>1, Isa DEF AsyncProgressOwnershipCoreVars
    <2>2. /\ SerializedBusyOwners' = SerializedBusyOwners
           /\ PendingNodes' = PendingNodes
           /\ SigningNodes' = SigningNodes
      BY <2>1, Isa
         DEF SerializedBusyOwners, PendingNodes, SigningNodes,
             AllPendingRequests
    <2>3. \A node: NodeIdle(node)' <=> NodeIdle(node)
      BY <2>2 DEF NodeIdle
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ProgressCoreStutterAndCarrierGrowthRetainsBusyCandidates ==
  /\ UNCHANGED AsyncProgressOwnershipCoreVars
  /\ UNCHANGED AsyncBusyConsumerVars
  /\ ActiveBusyCompletionCarrier \subseteq
       ActiveBusyCompletionCarrier'
  => \A node:
       BusyCompletionCandidates(node) \subseteq
         BusyCompletionCandidates(node)'
PROOF
  <1>1. ASSUME UNCHANGED AsyncProgressOwnershipCoreVars,
              UNCHANGED AsyncBusyConsumerVars,
              ActiveBusyCompletionCarrier \subseteq
                ActiveBusyCompletionCarrier'
         PROVE \A node:
                 BusyCompletionCandidates(node) \subseteq
                   BusyCompletionCandidates(node)'
    <2>1. /\ pendingProposal' = pendingProposal
           /\ pendingPrepare' = pendingPrepare
           /\ pendingObservePrepare' = pendingObservePrepare
           /\ pendingLockCommit' = pendingLockCommit
           /\ pendingTimeout' = pendingTimeout
           /\ pendingInstallTC' = pendingInstallTC
           /\ pendingDecision' = pendingDecision
           /\ signProposals' = signProposals
           /\ signVotes' = signVotes
           /\ signTimeouts' = signTimeouts
      BY <1>1, Isa DEF AsyncProgressOwnershipCoreVars
    <2>2. /\ context' = context
           /\ nodeView' = nodeView
           /\ generation' = generation
      BY <1>1, Isa DEF AsyncBusyConsumerVars
    <2>3. ASSUME NEW node,
                  NEW candidate \in BusyCompletionCandidates(node)
           PROVE candidate \in BusyCompletionCandidates(node)'
      <3>1. candidate \in ActiveBusyCompletionCarrier'
        BY <1>1, <2>3 DEF BusyCompletionCandidates
      <3> QED BY <2>1, <2>2, <2>3, <3>1, Isa
           DEF BusyCompletionCandidates, CandidateConsumerCurrent
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM ProgressCoreStutterKeepsBusyWitnessWhenCarried ==
  \A owner, witness:
    /\ UNCHANGED AsyncProgressOwnershipCoreVars
    /\ UNCHANGED AsyncBusyConsumerVars
    /\ witness \in BusyCompletionCandidates(owner)
    /\ witness \in ActiveBusyCompletionCarrier'
    => witness \in BusyCompletionCandidates(owner)'
PROOF
  <1>1. ASSUME NEW owner, NEW witness,
                UNCHANGED AsyncProgressOwnershipCoreVars,
                UNCHANGED AsyncBusyConsumerVars,
                witness \in BusyCompletionCandidates(owner),
                witness \in ActiveBusyCompletionCarrier'
         PROVE witness \in BusyCompletionCandidates(owner)'
    <2>1. /\ pendingProposal' = pendingProposal
           /\ pendingPrepare' = pendingPrepare
           /\ pendingObservePrepare' = pendingObservePrepare
           /\ pendingLockCommit' = pendingLockCommit
           /\ pendingTimeout' = pendingTimeout
           /\ pendingInstallTC' = pendingInstallTC
           /\ pendingDecision' = pendingDecision
           /\ signProposals' = signProposals
           /\ signVotes' = signVotes
           /\ signTimeouts' = signTimeouts
      BY <1>1, Isa DEF AsyncProgressOwnershipCoreVars
    <2>2. /\ context' = context
           /\ nodeView' = nodeView
           /\ generation' = generation
      BY <1>1, Isa DEF AsyncBusyConsumerVars
    <2> QED BY <1>1, <2>1, <2>2, Isa
         DEF BusyCompletionCandidates, CandidateConsumerCurrent
  <1> QED BY <1>1

THEOREM AddSerializedBusyOwnerPreservesOwnership ==
  \A node, request:
    /\ request.node = node
    /\ SerializedBusyOwnershipInvariant
    /\ NodeIdle(node)
    /\ SerializedBusyOwners' = SerializedBusyOwners \cup {request}
    /\ PendingNodes' = PendingNodes
    /\ SigningNodes' = SigningNodes \cup {node}
    => /\ SerializedBusyOwnershipInvariant'
       /\ ~NodeIdle(node)'
       /\ \A other \in ValidatorIds \ {node}:
            NodeIdle(other)' <=> NodeIdle(other)
PROOF
  <1>1. ASSUME NEW node, NEW request,
                request.node = node,
                SerializedBusyOwnershipInvariant,
                NodeIdle(node),
                SerializedBusyOwners' =
                  SerializedBusyOwners \cup {request},
                PendingNodes' = PendingNodes,
                SigningNodes' = SigningNodes \cup {node}
         PROVE /\ SerializedBusyOwnershipInvariant'
               /\ ~NodeIdle(node)'
               /\ \A other \in ValidatorIds \ {node}:
                    NodeIdle(other)' <=> NodeIdle(other)
    <2>1. RequestsUniqueByNode(SerializedBusyOwners)
      BY <1>1 DEF SerializedBusyOwnershipInvariant
    <2>2. request.node \notin RequestNodeSet(SerializedBusyOwners)
      BY <1>1, SerializedBusyOwnerNodeSet
         DEF NodeIdle
    <2>3. RequestsUniqueByNode(SerializedBusyOwners \cup {request})
      BY <2>1, <2>2, NewRequestPreservesNodeUniqueness
    <2>4. SerializedBusyOwnershipInvariant'
      BY <1>1, <2>3 DEF SerializedBusyOwnershipInvariant
    <2>5. ~NodeIdle(node)'
      BY <1>1 DEF NodeIdle
    <2>6. \A other \in ValidatorIds \ {node}:
             NodeIdle(other)' <=> NodeIdle(other)
      BY <1>1, Isa DEF NodeIdle
    <2> QED BY <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM RecoveryCoreReplayPreservesBusyOwnership ==
  \A node, candidate:
    /\ SerializedBusyOwnershipInvariant
    /\ NodeIdle(node)
    /\ RecoveryCoreReplay(node, candidate)
    => /\ SerializedBusyOwnershipInvariant'
       /\ ~NodeIdle(node)'
       /\ \A other \in ValidatorIds \ {node}:
            NodeIdle(other)' <=> NodeIdle(other)
PROOF
  <1>1. ASSUME NEW node, NEW candidate,
                SerializedBusyOwnershipInvariant,
                NodeIdle(node),
                RecoveryCoreReplay(node, candidate)
         PROVE /\ SerializedBusyOwnershipInvariant'
               /\ ~NodeIdle(node)'
               /\ \A other \in ValidatorIds \ {node}:
                    NodeIdle(other)' <=> NodeIdle(other)
    <2>1. CASE candidate.kind = "SignProposal"
      <3> DEFINE Request == ProposalSign(node, candidate.evidence)
      <3>1. /\ Request.node = node
             /\ SerializedBusyOwners' =
                  SerializedBusyOwners \cup {Request}
             /\ PendingNodes' = PendingNodes
             /\ SigningNodes' = SigningNodes \cup {node}
        BY <1>1, <2>1, Isa
           DEF RecoveryCoreReplay, ResumeProposal, Request,
               ProposalSign, SerializedBusyOwners, PendingNodes,
               SigningNodes, AllPendingRequests
      <3> QED BY <1>1, <3>1,
           AddSerializedBusyOwnerPreservesOwnership
    <2>2. CASE candidate.kind = "SignVote"
      <3> DEFINE Request == VoteSign(node, candidate.evidence)
      <3>1. /\ Request.node = node
             /\ SerializedBusyOwners' =
                  SerializedBusyOwners \cup {Request}
             /\ PendingNodes' = PendingNodes
             /\ SigningNodes' = SigningNodes \cup {node}
        BY <1>1, <2>2, Isa
           DEF RecoveryCoreReplay, ResumeVote, Request,
               VoteSign, SerializedBusyOwners, PendingNodes,
               SigningNodes, AllPendingRequests
      <3> QED BY <1>1, <3>1,
           AddSerializedBusyOwnerPreservesOwnership
    <2>3. CASE candidate.kind = "SignTimeout"
      <3> DEFINE Request == TimeoutSign(node, candidate.evidence)
      <3>1. /\ Request.node = node
             /\ SerializedBusyOwners' =
                  SerializedBusyOwners \cup {Request}
             /\ PendingNodes' = PendingNodes
             /\ SigningNodes' = SigningNodes \cup {node}
        BY <1>1, <2>3, Isa
           DEF RecoveryCoreReplay, ResumeTimeout, Request,
               TimeoutSign, SerializedBusyOwners, PendingNodes,
               SigningNodes, AllPendingRequests
      <3> QED BY <1>1, <3>1,
           AddSerializedBusyOwnerPreservesOwnership
    <2>4. CASE candidate.kind \notin
                 {"SignProposal", "SignVote", "SignTimeout"}
      BY <1>1, <2>4 DEF RecoveryCoreReplay
    <2> QED BY <2>1, <2>2, <2>3, <2>4, Isa
  <1> QED BY <1>1

THEOREM RestartSignatureReplayCandidateShape ==
  \A node:
  \A candidate \in SequenceSet(RestartSignatureReplay(node)):
    /\ candidate.node = node
    /\ candidate.class = "Completion"
    /\ candidate.height = context.height
    /\ CandidateConsumerCurrent(candidate)
    /\ candidate.item = NoAsyncItem
    /\ \/ /\ candidate.kind = "SignProposal"
           /\ candidate.view = candidate.evidence.view
           /\ candidate.subject = candidate.evidence.subject
       \/ /\ candidate.kind = "SignVote"
           /\ candidate.view = candidate.evidence.view
           /\ candidate.subject = candidate.evidence.subject
       \/ /\ candidate.kind = "SignTimeout"
           /\ candidate.view = candidate.evidence.view
           /\ candidate.subject = candidate.evidence.highSubject
BY Isa
   DEF RestartSignatureReplay, RestartTimeoutOrProposalReplay,
       RestartPrepareReplayIfActive,
       RestartLockedCommitReplayIfActive,
       RestartTimeoutReplay, RestartProposalReplay,
       RestartPrepareReplay, RestartLockedCommitReplay,
       RestartCandidate, AsyncCandidateAtConsumer,
       AsyncCandidateWithIdentity, SequenceSet

THEOREM RecoveryCoreReplayCandidateIsBusyWhenCarried ==
  \A node, candidate:
    /\ candidate \in SequenceSet(RestartSignatureReplay(node))
    /\ RecoveryCoreReplay(node, candidate)
    /\ candidate \in ActiveBusyCompletionCarrier'
    => candidate \in BusyCompletionCandidates(node)'
PROOF
  <1>1. ASSUME NEW node, NEW candidate,
                candidate \in
                  SequenceSet(RestartSignatureReplay(node)),
                RecoveryCoreReplay(node, candidate),
                candidate \in ActiveBusyCompletionCarrier'
         PROVE candidate \in BusyCompletionCandidates(node)'
    <2>1. /\ candidate.node = node
           /\ candidate.class = "Completion"
           /\ candidate.height = context.height
           /\ CandidateConsumerCurrent(candidate)
           /\ candidate.item = NoAsyncItem
           /\ \/ /\ candidate.kind = "SignProposal"
                  /\ candidate.view = candidate.evidence.view
                  /\ candidate.subject = candidate.evidence.subject
              \/ /\ candidate.kind = "SignVote"
                  /\ candidate.view = candidate.evidence.view
                  /\ candidate.subject = candidate.evidence.subject
              \/ /\ candidate.kind = "SignTimeout"
                  /\ candidate.view = candidate.evidence.view
                  /\ candidate.subject =
                       candidate.evidence.highSubject
      BY <1>1, RestartSignatureReplayCandidateShape
    <2>2. CASE candidate.kind = "SignProposal"
      <3>1. ProposalSign(node, candidate.evidence)
                 \in signProposals'
        BY <1>1, <2>2
           DEF RecoveryCoreReplay, ResumeProposal
      <3> QED BY <1>1, <2>1, <2>2, <3>1, Isa
           DEF BusyCompletionCandidates, CandidateConsumerCurrent
    <2>3. CASE candidate.kind = "SignVote"
      <3>1. VoteSign(node, candidate.evidence) \in signVotes'
        BY <1>1, <2>3
           DEF RecoveryCoreReplay, ResumeVote
      <3> QED BY <1>1, <2>1, <2>3, <3>1, Isa
           DEF BusyCompletionCandidates, CandidateConsumerCurrent, VoteSign
    <2>4. CASE candidate.kind = "SignTimeout"
      <3>1. TimeoutSign(node, candidate.evidence)
                 \in signTimeouts'
        BY <1>1, <2>4
           DEF RecoveryCoreReplay, ResumeTimeout
      <3> QED BY <1>1, <2>1, <2>4, <3>1, Isa
           DEF BusyCompletionCandidates, CandidateConsumerCurrent,
               TimeoutSign
    <2> QED BY <2>1, <2>2, <2>3, <2>4, Isa
  <1> QED BY <1>1

THEOREM RecoveryCoreReplayKeepsBusyWitnessWhenCarried ==
  \A node, replayCandidate, owner, witness:
    /\ RecoveryCoreReplay(node, replayCandidate)
    /\ witness \in BusyCompletionCandidates(owner)
    /\ witness \in ActiveBusyCompletionCarrier'
    => witness \in BusyCompletionCandidates(owner)'
PROOF
  <1>1. ASSUME NEW node, NEW replayCandidate, NEW owner, NEW witness,
                RecoveryCoreReplay(node, replayCandidate),
                witness \in BusyCompletionCandidates(owner),
                witness \in ActiveBusyCompletionCarrier'
         PROVE witness \in BusyCompletionCandidates(owner)'
    <2>1. /\ context' = context
           /\ nodeView' = nodeView
           /\ generation' = generation
      BY <1>1, Isa
         DEF RecoveryCoreReplay, ResumeProposal, ResumeVote,
             ResumeTimeout
    <2>2. /\ pendingProposal \subseteq pendingProposal'
           /\ pendingPrepare \subseteq pendingPrepare'
           /\ pendingObservePrepare \subseteq pendingObservePrepare'
           /\ pendingLockCommit \subseteq pendingLockCommit'
           /\ pendingTimeout \subseteq pendingTimeout'
           /\ pendingInstallTC \subseteq pendingInstallTC'
           /\ pendingDecision \subseteq pendingDecision'
           /\ signProposals \subseteq signProposals'
           /\ signVotes \subseteq signVotes'
           /\ signTimeouts \subseteq signTimeouts'
      <3>1. CASE replayCandidate.kind = "SignProposal"
        BY <1>1, <3>1, Isa
           DEF RecoveryCoreReplay, ResumeProposal
      <3>2. CASE replayCandidate.kind = "SignVote"
        BY <1>1, <3>2, Isa
           DEF RecoveryCoreReplay, ResumeVote
      <3>3. CASE replayCandidate.kind = "SignTimeout"
        BY <1>1, <3>3, Isa
           DEF RecoveryCoreReplay, ResumeTimeout
      <3>4. CASE replayCandidate.kind \notin
                   {"SignProposal", "SignVote", "SignTimeout"}
        BY <1>1, <3>4 DEF RecoveryCoreReplay
      <3> QED BY <3>1, <3>2, <3>3, <3>4, Isa
    <2> QED BY <1>1, <2>1, <2>2, Isa
         DEF BusyCompletionCandidates, CandidateConsumerCurrent
  <1> QED BY <1>1

THEOREM ResetNodeSchedulerRetainsOtherActiveCandidate ==
  \A node \in ValidatorIds:
  \A replay:
  \A other \in ValidatorIds \ {node}:
  \A candidate:
    /\ AsyncSchedulerTypeInvariant
    /\ ResetNodeSchedulerForRestart(node, replay)
    /\ candidate \in ActiveBusyCompletionCarrier
    /\ candidate.node = other
    => candidate \in ActiveBusyCompletionCarrier'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, NEW replay,
                NEW other \in ValidatorIds \ {node}, NEW candidate,
                AsyncSchedulerTypeInvariant,
                ResetNodeSchedulerForRestart(node, replay),
                candidate \in ActiveBusyCompletionCarrier,
                candidate.node = other
         PROVE candidate \in ActiveBusyCompletionCarrier'
    <2>1. /\ asyncCommandQueues'[other] =
                  asyncCommandQueues[other]
           /\ asyncCausalQueues'[other] = asyncCausalQueues[other]
           /\ asyncOutstandingWork'[other] =
                  asyncOutstandingWork[other]
      BY <1>1, FunctionalUpdateAwayFromKey
         DEF ResetNodeSchedulerForRestart,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncCausalTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant
    <2>2. CASE candidate \in QueuedCandidates
      <3>1. PICK owner \in ValidatorIds:
                 candidate \in SequenceSet(asyncCommandQueues[owner])
        BY <2>2 DEF QueuedCandidates
      <3>2. candidate.node = owner
        BY <1>1, <3>1
           DEF AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant,
               AsyncRuntimeScalarTypeInvariant,
               AsyncCommandQueueOwnership
      <3>3. owner = other
        BY <1>1, <3>2
      <3> QED BY <2>1, <3>1, <3>3, Isa
           DEF ActiveBusyCompletionCarrier, QueuedCandidates
    <2>3. CASE candidate \in CausalCandidates
      <3>1. PICK owner \in ValidatorIds:
                 candidate \in SequenceSet(asyncCausalQueues[owner])
        BY <2>3 DEF CausalCandidates
      <3>2. candidate.node = owner
        BY <1>1, <3>1
           DEF AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncCausalTypeInvariant,
               AsyncCausalQueueOwnership
      <3>3. owner = other
        BY <1>1, <3>2
      <3> QED BY <2>1, <3>1, <3>3, Isa
           DEF ActiveBusyCompletionCarrier, CausalCandidates
    <2>4. CASE candidate \in TrackedWorkCandidates
      <3>1. PICK owner \in ValidatorIds:
                 candidate \in asyncOutstandingWork[owner]
        BY <2>4 DEF TrackedWorkCandidates
      <3>2. candidate.node = owner
        BY <1>1, <3>1
           DEF AsyncSchedulerTypeInvariant,
               AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
               AsyncIoWorkContentTypeInvariant
      <3>3. owner = other
        BY <1>1, <3>2
      <3> QED BY <2>1, <3>1, <3>3, Isa
           DEF ActiveBusyCompletionCarrier, TrackedWorkCandidates
    <2> QED BY <1>1, <2>2, <2>3, <2>4
         DEF ActiveBusyCompletionCarrier
  <1> QED BY <1>1

THEOREM PreGstResponsiveReplayPreservesProgressOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ PreGstResponsiveReplay
  => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              PreGstResponsiveReplay
         PROVE AsyncProgressOwnershipInvariant'
    <2> DEFINE Node == asyncRecoveryNode
    <2> DEFINE Signatures == RestartSignatureReplay(Node)
    <2> DEFINE Replay == RestartReplay(Node)
    <2>1. /\ StrongInductiveInvariant
           /\ TypeInvariant
           /\ AsyncSchedulerTypeInvariant
           /\ Node \in ValidatorIds
           /\ NodeIdle(Node)
      BY <1>1, SMT
         DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety,
             AsyncRecoveryTypeInvariant, PreGstResponsiveReplay, Node
    <2>2. /\ AsyncQueueTyped(Replay)
           /\ AsyncCausalQueueOwnership(Node, Replay)
           /\ SequenceHasUniqueValues(Replay)
           /\ Len(Replay) <= 2
      BY <2>1, RestartReplayIsTypedOwnedAndUnique DEF Replay
    <2>3. ResetNodeSchedulerForRestart(Node, Replay)
      BY <1>1 DEF PreGstResponsiveReplay, Node, Replay
    <2>4. AsyncLogicalCandidateOwnershipInvariant'
      BY <1>1, <2>1, <2>2, <2>3,
         RestartResetPreservesLogicalOwnership
         DEF AsyncProgressOwnershipInvariant
    <2>5. AsyncOutstandingCarrierInvariant'
      BY <1>1, <2>1, <2>3,
         ResetNodeSchedulerPreservesOutstandingCarrier
         DEF AsyncProgressOwnershipInvariant
    <2>6. CASE Len(Signatures) = 0
      <3>1. /\ UNCHANGED AsyncProgressOwnershipCoreVars
             /\ UNCHANGED AsyncBusyConsumerVars
        BY <1>1, <2>6, Isa
           DEF PreGstResponsiveReplay, Signatures,
               AsyncProgressOwnershipCoreVars, AsyncBusyConsumerVars,
               vars
      <3>2. /\ SerializedBusyOwners' = SerializedBusyOwners
             /\ \A other: NodeIdle(other)' <=> NodeIdle(other)
        BY <3>1, ProgressCoreStutterFacts
      <3>3. SerializedBusyOwnershipInvariant'
        BY <1>1, <3>2
           DEF AsyncProgressOwnershipInvariant,
               SerializedBusyOwnershipInvariant
      <3>4. NodeIdle(Node)'
        BY <2>1, <3>2
      <3>5. \A other \in ValidatorIds \ {Node}:
               BusyCompletionCandidates(other)
                 \subseteq BusyCompletionCandidates(other)'
        <4>1. ASSUME NEW other \in ValidatorIds \ {Node},
                    NEW candidate \in
                      BusyCompletionCandidates(other)
               PROVE candidate \in BusyCompletionCandidates(other)'
          <5>1. /\ candidate \in ActiveBusyCompletionCarrier
                 /\ candidate.node = other
            BY <4>1 DEF BusyCompletionCandidates
          <5>2. candidate \in ActiveBusyCompletionCarrier'
            BY <2>1, <2>3, <4>1, <5>1,
               ResetNodeSchedulerRetainsOtherActiveCandidate
          <5> QED BY <3>1, <4>1, <5>2,
               ProgressCoreStutterKeepsBusyWitnessWhenCarried
        <4> QED BY <4>1
      <3>6. BusyCompletionWitnessInvariant'
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE ~NodeIdle(other)'
                       => \/ BusyCompletionCandidates(other)' # {}
                          \/ InstallGenerationExhausted(other)'
          <5>1. CASE other = Node
            <6>1. NodeIdle(other)'
              BY <3>4, <5>1
            <6> QED BY <6>1
          <5>2. CASE other # Node
            <6>1. other \in ValidatorIds \ {Node}
              BY <4>1, <5>2
            <6>2. ASSUME ~NodeIdle(other)'
                   PROVE \/ BusyCompletionCandidates(other)' # {}
                         \/ InstallGenerationExhausted(other)'
              <7>1. ~NodeIdle(other)
                BY <3>2, <6>2
              <7>2. \/ BusyCompletionCandidates(other) # {}
                     \/ InstallGenerationExhausted(other)
                BY <1>1, <7>1
                   DEF AsyncProgressOwnershipInvariant,
                       BusyCompletionWitnessInvariant
              <7>3. CASE BusyCompletionCandidates(other) # {}
                <8>1. PICK candidate \in
                             BusyCompletionCandidates(other): TRUE
                  BY <7>3
                <8>2. candidate \in BusyCompletionCandidates(other)'
                  BY <3>5, <6>1, <8>1
                <8> QED BY <8>2
              <7>4. CASE InstallGenerationExhausted(other)
                <8>1. /\ generation' = generation
                       /\ pendingInstallTC' = pendingInstallTC
                  BY <3>1, Isa
                     DEF AsyncProgressOwnershipCoreVars,
                         AsyncBusyConsumerVars
                <8>2. InstallGenerationExhausted(other)'
                  BY <7>4, <8>1 DEF InstallGenerationExhausted
                <8> QED BY <8>2
              <7> QED BY <7>2, <7>3, <7>4
            <6> QED BY <6>2
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1 DEF BusyCompletionWitnessInvariant
      <3> QED BY <2>4, <2>5, <3>3, <3>6
           DEF AsyncProgressOwnershipInvariant
    <2>7. CASE Len(Signatures) > 0
      <3>1. /\ Signatures \in Seq(Range(Signatures))
             /\ Signatures # <<>>
        BY <2>1, <2>7, RestartSignatureReplayProperties,
           PositiveSequenceIsNonempty
           DEF Signatures, AsyncQueueTyped
      <3>2. Head(Signatures) \in SequenceSet(Signatures)
        BY <3>1, NonemptySequenceHeadIsFirst DEF SequenceSet
      <3>3. /\ ~NodeHasApplication(Node)
             /\ RestartDecisions(Node) = {}
        BY <3>1, Isa DEF Signatures, RestartSignatureReplay
      <3>4. Head(Signatures) \in SequenceSet(Replay)
        BY <2>7, <3>3, RangeConcatenation, Isa
           DEF Replay, RestartReplay, Signatures, SequenceSet
      <3>5. RecoveryCoreReplay(Node, Head(Signatures))
        BY <1>1, <2>7
           DEF PreGstResponsiveReplay, Node, Signatures
      <3>6. /\ SerializedBusyOwnershipInvariant'
             /\ ~NodeIdle(Node)'
             /\ \A other \in ValidatorIds \ {Node}:
                  NodeIdle(other)' <=> NodeIdle(other)
        BY <1>1, <2>1, <3>5,
           RecoveryCoreReplayPreservesBusyOwnership
           DEF AsyncProgressOwnershipInvariant
      <3>7. asyncCausalQueues'[Node] = Replay
        BY <2>1, <2>3, FunctionalReplaceUpdateAtKey
           DEF AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncCausalTypeInvariant
      <3>8. Head(Signatures)
               \in SequenceSet(asyncCausalQueues'[Node])
        BY <3>4, <3>7
      <3>9. Head(Signatures) \in ActiveBusyCompletionCarrier'
        BY <3>8
           DEF ActiveBusyCompletionCarrier, CausalCandidates
      <3>10. Head(Signatures) \in BusyCompletionCandidates(Node)'
        BY <3>2, <3>5, <3>9,
           RecoveryCoreReplayCandidateIsBusyWhenCarried
           DEF Signatures
      <3>10a. /\ generation' = generation
               /\ pendingInstallTC' = pendingInstallTC
        BY <3>5, Isa
           DEF RecoveryCoreReplay, ResumeProposal, ResumeVote,
               ResumeTimeout
      <3>11. \A other \in ValidatorIds \ {Node}:
                BusyCompletionCandidates(other)
                  \subseteq BusyCompletionCandidates(other)'
        <4>1. ASSUME NEW other \in ValidatorIds \ {Node},
                    NEW candidate \in
                      BusyCompletionCandidates(other)
               PROVE candidate \in BusyCompletionCandidates(other)'
          <5>1. /\ candidate \in ActiveBusyCompletionCarrier
                 /\ candidate.node = other
            BY <4>1 DEF BusyCompletionCandidates
          <5>2. candidate \in ActiveBusyCompletionCarrier'
            BY <2>1, <2>3, <4>1, <5>1,
               ResetNodeSchedulerRetainsOtherActiveCandidate
          <5> QED BY <3>5, <4>1, <5>2,
               RecoveryCoreReplayKeepsBusyWitnessWhenCarried
        <4> QED BY <4>1
      <3>12. BusyCompletionWitnessInvariant'
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE ~NodeIdle(other)'
                       => \/ BusyCompletionCandidates(other)' # {}
                          \/ InstallGenerationExhausted(other)'
          <5>1. CASE other = Node
            BY <3>10, <5>1
          <5>2. CASE other # Node
            <6>1. other \in ValidatorIds \ {Node}
              BY <4>1, <5>2
            <6>2. ASSUME ~NodeIdle(other)'
                   PROVE \/ BusyCompletionCandidates(other)' # {}
                         \/ InstallGenerationExhausted(other)'
              <7>1. ~NodeIdle(other)
                BY <3>6, <6>1, <6>2
              <7>2. \/ BusyCompletionCandidates(other) # {}
                     \/ InstallGenerationExhausted(other)
                BY <1>1, <7>1
                   DEF AsyncProgressOwnershipInvariant,
                       BusyCompletionWitnessInvariant
              <7>3. CASE BusyCompletionCandidates(other) # {}
                <8>1. PICK candidate \in
                             BusyCompletionCandidates(other): TRUE
                  BY <7>3
                <8>2. candidate \in BusyCompletionCandidates(other)'
                  BY <3>11, <6>1, <8>1
                <8> QED BY <8>2
              <7>4. CASE InstallGenerationExhausted(other)
                <8>1. InstallGenerationExhausted(other)'
                  BY <3>10a, <7>4 DEF InstallGenerationExhausted
                <8> QED BY <8>1
              <7> QED BY <7>2, <7>3, <7>4
            <6> QED BY <6>2
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1 DEF BusyCompletionWitnessInvariant
      <3> QED BY <2>4, <2>5, <3>6, <3>12
           DEF AsyncProgressOwnershipInvariant
    <2>8. Len(Signatures) = 0 \/ Len(Signatures) > 0
      BY <2>1, RestartSignatureReplayProperties, SMT
         DEF Signatures, AsyncQueueTyped
    <2> QED BY <2>6, <2>7, <2>8
  <1> QED BY <1>1

THEOREM DriveResponsiveReplayHeadPreservesProgressOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DriveResponsiveReplayHead
  => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              DriveResponsiveReplayHead
         PROVE AsyncProgressOwnershipInvariant'
    <2> DEFINE Node == asyncRecoveryNode
    <2> DEFINE Candidate == Head(asyncRecoveryReplayQueue)
    <2> DEFINE Fresh == FreshCandidateSequence(Candidate)
    <2>1. /\ StrongInductiveInvariant
           /\ AsyncSchedulerTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncRecoveryTypeInvariant
           /\ AsyncRecoveryExecutionInvariant
           /\ AsyncQueueTyped(asyncRecoveryReplayQueue)
           /\ Len(asyncRecoveryReplayQueue) > 0
           /\ Node \in ValidatorIds
      BY <1>1
         DEF AsyncStrongTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRecoveryTypeInvariant,
             DriveResponsiveReplayHead, Node
    <2>2. /\ Candidate \in
                SequenceSet(asyncRecoveryReplayQueue)
           /\ AsyncCandidateTyped(Candidate)
           /\ Candidate.node = Node
           /\ ~CandidateScheduled(Candidate)
           /\ Fresh = <<Candidate>>
      BY <1>1, <2>1, ReplayingRecoveryHeadIsFresh,
         NonemptyTypedQueueHeadIsTyped
         DEF Candidate, Fresh, Node, DriveResponsiveReplayHead
    <2>3. /\ AsyncQueueTyped(Fresh)
           /\ AsyncCausalQueueOwnership(Node, Fresh)
           /\ SequenceHasUniqueValues(Fresh)
           /\ Len(Fresh) <= 1
      BY <2>2, FreshTypedOwnedReplayCandidateProperties DEF Fresh
    <2>4. SequenceSet(Fresh) \cap
             (QueuedCandidates \cup DeferredCandidates \cup
                CausalCandidates \cup TrackedWorkCandidates) = {}
      BY FreshReplayCandidateIsDisjointFromScheduled DEF Fresh
    <2>5. /\ NodeIdle(Node)
           /\ RecoveryCoreReplay(Node, Candidate)
      BY <1>1 DEF DriveResponsiveReplayHead, Node, Candidate
    <2>6. /\ asyncCausalQueues' =
                  [asyncCausalQueues EXCEPT ![Node] = @ \o Fresh]
           /\ asyncRecoveryReplayQueue' =
                  Tail(asyncRecoveryReplayQueue)
           /\ UNCHANGED
                <<asyncCommandQueues, asyncOutstandingWork,
                  asyncIoQueues, asyncIoReadyCompletions,
                  asyncLocalReadyCompletions,
                  asyncDeferredCompletionQueues,
                  asyncDeferredProgressQueues,
                  asyncDeferredNormalQueues>>
      BY <1>1 DEF DriveResponsiveReplayHead, Node, Candidate, Fresh
    <2>7. /\ AsyncLogicalCandidateOwnershipInvariant'
           /\ CausalCandidates' =
                CausalCandidates \cup {Candidate}
      BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>6,
         FreshCausalAppendPreservesLogicalOwnership
         DEF AsyncProgressOwnershipInvariant, SequenceSet, Fresh
    <2>8. AsyncOutstandingCarrierInvariant'
      BY <1>1, <2>6, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncOutstandingCarrierInvariant,
             ConsensusIoCandidates, SequenceSet
    <2>9. /\ SerializedBusyOwnershipInvariant'
           /\ ~NodeIdle(Node)'
           /\ \A other \in ValidatorIds \ {Node}:
                NodeIdle(other)' <=> NodeIdle(other)
      BY <1>1, <2>5,
         RecoveryCoreReplayPreservesBusyOwnership
         DEF AsyncProgressOwnershipInvariant
    <2>10. /\ QueuedCandidates' = QueuedCandidates
            /\ TrackedWorkCandidates' = TrackedWorkCandidates
      BY <2>6, Isa
         DEF QueuedCandidates, TrackedWorkCandidates, SequenceSet
    <2>11. ActiveBusyCompletionCarrier' =
             ActiveBusyCompletionCarrier \cup {Candidate}
      BY <2>7, <2>10, Isa DEF ActiveBusyCompletionCarrier
    <2>12. Candidate \in
              SequenceSet(RestartSignatureReplay(Node))
      BY <2>1, <2>2
         DEF AsyncRecoveryTypeInvariant, Node, Candidate
    <2>13. Candidate \in BusyCompletionCandidates(Node)'
      <3>1. Candidate \in ActiveBusyCompletionCarrier'
        BY <2>11, Isa
      <3> QED BY <2>5, <2>12, <3>1,
           RecoveryCoreReplayCandidateIsBusyWhenCarried
    <2>13a. /\ generation' = generation
              /\ pendingInstallTC' = pendingInstallTC
      BY <2>5, Isa
         DEF RecoveryCoreReplay, ResumeProposal, ResumeVote,
             ResumeTimeout
    <2>14. \A other \in ValidatorIds \ {Node}:
              BusyCompletionCandidates(other)
                \subseteq BusyCompletionCandidates(other)'
      <3>1. ASSUME NEW other \in ValidatorIds \ {Node},
                  NEW candidate \in BusyCompletionCandidates(other)
             PROVE candidate \in BusyCompletionCandidates(other)'
        <4>1. candidate \in ActiveBusyCompletionCarrier'
          BY <2>11, <3>1, Isa DEF BusyCompletionCandidates
        <4> QED BY <2>5, <3>1, <4>1,
             RecoveryCoreReplayKeepsBusyWitnessWhenCarried
      <3> QED BY <3>1
    <2>15. BusyCompletionWitnessInvariant'
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE ~NodeIdle(other)'
                     => \/ BusyCompletionCandidates(other)' # {}
                        \/ InstallGenerationExhausted(other)'
        <4>1. CASE other = Node
          BY <2>13, <4>1
        <4>2. CASE other # Node
          <5>1. other \in ValidatorIds \ {Node}
            BY <3>1, <4>2
          <5>2. ASSUME ~NodeIdle(other)'
                 PROVE \/ BusyCompletionCandidates(other)' # {}
                       \/ InstallGenerationExhausted(other)'
            <6>1. ~NodeIdle(other)
              BY <2>9, <5>1, <5>2
            <6>2. \/ BusyCompletionCandidates(other) # {}
                   \/ InstallGenerationExhausted(other)
              BY <1>1, <6>1
                 DEF AsyncProgressOwnershipInvariant,
                     BusyCompletionWitnessInvariant
            <6>3. CASE BusyCompletionCandidates(other) # {}
              <7>1. PICK candidate \in
                           BusyCompletionCandidates(other): TRUE
                BY <6>3
              <7>2. candidate \in BusyCompletionCandidates(other)'
                BY <2>14, <5>1, <7>1
              <7> QED BY <7>2
            <6>4. CASE InstallGenerationExhausted(other)
              <7>1. InstallGenerationExhausted(other)'
                BY <2>13a, <6>4 DEF InstallGenerationExhausted
              <7> QED BY <7>1
            <6> QED BY <6>2, <6>3, <6>4
          <5> QED BY <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1 DEF BusyCompletionWitnessInvariant
    <2> QED BY <2>7, <2>8, <2>9, <2>15
         DEF AsyncProgressOwnershipInvariant
  <1> QED BY <1>1

THEOREM FinishResponsiveReplayPreservesProgressOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ FinishResponsiveReplay
  => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              FinishResponsiveReplay
         PROVE AsyncProgressOwnershipInvariant'
    <2> DEFINE Node == asyncRecoveryNode
    <2> DEFINE Runner == RestartRunnerAssembly(Node)
    <2>1. /\ Node \in ValidatorIds
           /\ TypeInvariant
      <3>1. AsyncRecoveryTypeInvariant
        BY <1>1 DEF AsyncStrongTypeInvariant
      <3>2. Node \in ValidatorIds
        BY <3>1 DEF AsyncRecoveryTypeInvariant, Node
      <3>3. TypeInvariant
        BY <1>1
           DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety
      <3> QED BY <3>2, <3>3
    <2>2. /\ AsyncQueueTyped(Runner)
           /\ AsyncCausalQueueOwnership(Node, Runner)
           /\ SequenceHasUniqueValues(Runner)
           /\ Len(Runner) <= 1
      BY <2>1, RestartRunnerAssemblyProperties DEF Runner
    <2>2a. Len(Runner) \in Nat
      BY <2>2, LenProperties DEF AsyncQueueTyped
    <2>2b. Len(Runner) = 0 \/ Len(Runner) > 0
      BY <2>2a, SMT
    <2>3. CASE Len(Runner) = 0
      <3>1. UNCHANGED asyncCausalQueues
        BY <1>1, <2>3 DEF FinishResponsiveReplay, Node, Runner
      <3>2. UNCHANGED AsyncProgressOwnershipVars
        BY <1>1, <3>1, Isa
           DEF FinishResponsiveReplay, Runner, Node,
               AsyncProgressOwnershipVars,
               AsyncProgressOwnershipCoreVars,
               AsyncProgressOwnershipSchedulerVars,
               AsyncLocalAdmissionVars, AsyncIoVars,
               AsyncDeferredVars, vars
      <3> QED BY <1>1, <3>2, AsyncProgressOwnershipStutter
    <2>4. CASE Len(Runner) > 0
      <3> DEFINE Candidate == Runner[1]
      <3> DEFINE Fresh == FreshCandidateSequence(Candidate)
      <3>1. /\ Len(Runner) = 1
             /\ AsyncCandidateTyped(Candidate)
             /\ Candidate.node = Node
        BY <2>2, <2>4, SMT
           DEF Candidate, AsyncQueueTyped,
               AsyncCausalQueueOwnership, SequenceSet
      <3>2. /\ AsyncQueueTyped(Fresh)
             /\ AsyncCausalQueueOwnership(Node, Fresh)
             /\ SequenceHasUniqueValues(Fresh)
             /\ Len(Fresh) <= 1
        BY <3>1, FreshTypedOwnedReplayCandidateProperties DEF Fresh
      <3>3. SequenceSet(Fresh) \cap
               (QueuedCandidates \cup DeferredCandidates \cup
                  CausalCandidates \cup TrackedWorkCandidates) = {}
        BY FreshReplayCandidateIsDisjointFromScheduled DEF Fresh
      <3>4. asyncCausalQueues' =
               [asyncCausalQueues EXCEPT ![Node] = @ \o Fresh]
        BY <1>1, <3>1
           DEF FinishResponsiveReplay, Node, Runner, Candidate, Fresh
      <3>5. UNCHANGED
               <<AsyncProgressOwnershipCoreVars,
                 AsyncBusyConsumerVars,
                 asyncCommandQueues, asyncIoQueues,
                 asyncOutstandingWork, asyncIoReadyCompletions,
                 asyncLocalReadyCompletions,
                 asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues,
                 asyncDeferredNormalQueues>>
        BY <1>1, Isa
           DEF FinishResponsiveReplay,
               AsyncProgressOwnershipCoreVars,
               AsyncBusyConsumerVars,
               AsyncLocalAdmissionVars, AsyncIoVars,
               AsyncDeferredVars, vars
      <3>6. AsyncCausalTypeInvariant
        BY <1>1
           DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant
      <3>7. /\ AsyncLogicalCandidateOwnershipInvariant'
             /\ CausalCandidates' =
                  CausalCandidates \cup SequenceSet(Fresh)
        BY <1>1, <2>1, <3>2, <3>3, <3>4, <3>5, <3>6,
           FreshCausalAppendPreservesLogicalOwnership
           DEF AsyncProgressOwnershipInvariant
      <3>8. /\ QueuedCandidates' = QueuedCandidates
             /\ TrackedWorkCandidates' = TrackedWorkCandidates
        BY <3>5, Isa
           DEF QueuedCandidates, TrackedWorkCandidates, SequenceSet
      <3>9. ActiveBusyCompletionCarrier' =
               ActiveBusyCompletionCarrier \cup SequenceSet(Fresh)
        BY <3>7, <3>8, Isa
           DEF ActiveBusyCompletionCarrier
      <3>10. AsyncOutstandingCarrierInvariant'
        BY <1>1, <3>5, Isa
           DEF AsyncProgressOwnershipInvariant,
               AsyncOutstandingCarrierInvariant,
               ConsensusIoCandidates, SequenceSet
      <3>11. /\ SerializedBusyOwners' = SerializedBusyOwners
              /\ \A other: NodeIdle(other)' <=> NodeIdle(other)
        BY <3>5, ProgressCoreStutterFacts
      <3>12. SerializedBusyOwnershipInvariant'
        BY <1>1, <3>11
           DEF AsyncProgressOwnershipInvariant,
               SerializedBusyOwnershipInvariant
      <3>13. \A other \in ValidatorIds:
                BusyCompletionCandidates(other)
                  \subseteq BusyCompletionCandidates(other)'
        BY <3>5, <3>9,
           ProgressCoreStutterAndCarrierGrowthRetainsBusyCandidates,
           Isa
      <3>14. BusyCompletionWitnessInvariant'
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE ~NodeIdle(other)'
                       => \/ BusyCompletionCandidates(other)' # {}
                          \/ InstallGenerationExhausted(other)'
          <5>1. ASSUME ~NodeIdle(other)'
                 PROVE \/ BusyCompletionCandidates(other)' # {}
                       \/ InstallGenerationExhausted(other)'
            <6>1. ~NodeIdle(other)
              BY <3>11, <5>1
            <6>2. \/ BusyCompletionCandidates(other) # {}
                   \/ InstallGenerationExhausted(other)
              BY <1>1, <6>1
                 DEF AsyncProgressOwnershipInvariant,
                     BusyCompletionWitnessInvariant
            <6>3. CASE BusyCompletionCandidates(other) # {}
              <7>1. PICK candidate \in
                           BusyCompletionCandidates(other): TRUE
                BY <6>3
              <7>2. candidate \in BusyCompletionCandidates(other)'
                BY <3>13, <4>1, <7>1
              <7> QED BY <7>2
            <6>4. CASE InstallGenerationExhausted(other)
              <7>1. /\ generation' = generation
                     /\ pendingInstallTC' = pendingInstallTC
                BY <3>5, Isa
                   DEF AsyncProgressOwnershipCoreVars,
                       AsyncBusyConsumerVars
              <7>2. InstallGenerationExhausted(other)'
                BY <6>4, <7>1 DEF InstallGenerationExhausted
              <7> QED BY <7>2
            <6> QED BY <6>2, <6>3, <6>4
          <5> QED BY <5>1
        <4> QED BY <4>1 DEF BusyCompletionWitnessInvariant
      <3> QED BY <3>7, <3>10, <3>12, <3>14
           DEF AsyncProgressOwnershipInvariant
    <2> QED BY <2>2b, <2>3, <2>4
  <1> QED BY <1>1

(***************************************************************************
Small ownership-carrier lemmas used by the local I/O and ingress actions.

`AsyncProgressOwnershipCoreVars` records the serialized Busy owner, while
`AsyncBusyConsumerVars` records the height/view/generation projection which
makes its completion executable.  Consequently those Core frames together
with monotonicity of the active completion carrier preserve both Busy
conjuncts without reopening the scheduler's unrelated state.
***************************************************************************)

THEOREM ProgressCoreFramePreservesBusyOwnership ==
  /\ AsyncProgressOwnershipInvariant
  /\ UNCHANGED AsyncProgressOwnershipCoreVars
  /\ UNCHANGED AsyncBusyConsumerVars
  /\ ActiveBusyCompletionCarrier \subseteq
       ActiveBusyCompletionCarrier'
  => /\ SerializedBusyOwnershipInvariant'
     /\ BusyCompletionWitnessInvariant'
PROOF
  <1>1. ASSUME AsyncProgressOwnershipInvariant,
              UNCHANGED AsyncProgressOwnershipCoreVars,
              UNCHANGED AsyncBusyConsumerVars,
              ActiveBusyCompletionCarrier \subseteq
                ActiveBusyCompletionCarrier'
         PROVE /\ SerializedBusyOwnershipInvariant'
               /\ BusyCompletionWitnessInvariant'
    <2>1. /\ SerializedBusyOwners' = SerializedBusyOwners
           /\ \A node: NodeIdle(node)' <=> NodeIdle(node)
      BY <1>1, ProgressCoreStutterFacts
    <2>2. SerializedBusyOwnershipInvariant'
      BY <1>1, <2>1
         DEF AsyncProgressOwnershipInvariant,
             SerializedBusyOwnershipInvariant
    <2>3. \A node \in ValidatorIds:
             BusyCompletionCandidates(node) \subseteq
               BusyCompletionCandidates(node)'
      BY <1>1,
         ProgressCoreStutterAndCarrierGrowthRetainsBusyCandidates,
         Isa
    <2>4. BusyCompletionWitnessInvariant'
      <3>1. ASSUME NEW node \in ValidatorIds
             PROVE ~NodeIdle(node)' =>
                     \/ BusyCompletionCandidates(node)' # {}
                        \/ InstallGenerationExhausted(node)'
        <4>1. ASSUME ~NodeIdle(node)'
               PROVE \/ BusyCompletionCandidates(node)' # {}
                     \/ InstallGenerationExhausted(node)'
          <5>1. ~NodeIdle(node)
            BY <2>1, <4>1
          <5>2. \/ BusyCompletionCandidates(node) # {}
                 \/ InstallGenerationExhausted(node)
            BY <1>1, <5>1
               DEF AsyncProgressOwnershipInvariant,
                   BusyCompletionWitnessInvariant
          <5>3. CASE BusyCompletionCandidates(node) # {}
            <6>1. PICK candidate \in
                         BusyCompletionCandidates(node): TRUE
              BY <5>3
            <6>2. candidate \in BusyCompletionCandidates(node)'
              BY <2>3, <3>1, <6>1
            <6> QED BY <6>2
          <5>4. CASE InstallGenerationExhausted(node)
            <6>1. /\ generation' = generation
                   /\ pendingInstallTC' = pendingInstallTC
              BY <1>1, Isa
                 DEF AsyncProgressOwnershipCoreVars,
                     AsyncBusyConsumerVars
            <6>2. InstallGenerationExhausted(node)'
              BY <5>4, <6>1 DEF InstallGenerationExhausted
            <6> QED BY <6>2
          <5> QED BY <5>2, <5>3, <5>4
        <4> QED BY <4>1
      <3> QED BY <3>1 DEF BusyCompletionWitnessInvariant
    <2> QED BY <2>2, <2>4
  <1> QED BY <1>1

THEOREM SequenceSetHeadTailDecomposition ==
  \A sequence:
    /\ sequence \in Seq(Range(sequence))
    /\ Len(sequence) > 0
    => SequenceSet(sequence) =
         {Head(sequence)} \cup SequenceSet(Tail(sequence))
PROOF
  <1>1. ASSUME NEW sequence,
                sequence \in Seq(Range(sequence)),
                Len(sequence) > 0
         PROVE SequenceSet(sequence) =
                 {Head(sequence)} \cup SequenceSet(Tail(sequence))
    <2>1. /\ sequence # <<>>
           /\ sequence = <<Head(sequence)>> \o Tail(sequence)
           /\ Tail(sequence) \in Seq(Range(sequence))
      BY <1>1, EmptySeq, HeadTailProperties, SMT
    <2>2. Head(sequence) \in Range(sequence)
      BY <1>1, <2>1, NonemptySequenceHeadIsFirst,
         RangeEquality
    <2>3. <<Head(sequence)>> \in Seq(Range(sequence))
      BY <2>2, SingletonSequenceFacts, SeqMonotonic, Isa
    <2>4. Range(<<Head(sequence)>> \o Tail(sequence)) =
             Range(<<Head(sequence)>>) \cup Range(Tail(sequence))
      BY <2>1, <2>3, RangeConcatenation
    <2>5. Range(<<Head(sequence)>>) = {Head(sequence)}
      BY SingletonSequenceFacts
    <2>6. Range(sequence) =
             {Head(sequence)} \cup Range(Tail(sequence))
      BY <2>1, <2>4, <2>5
    <2>7. /\ SequenceSet(sequence) = Range(sequence)
           /\ SequenceSet(Tail(sequence)) = Range(Tail(sequence))
      BY <1>1, <2>1, RangeEquality DEF SequenceSet
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM NonConsensusIoAppendPreservesConsensusCandidates ==
  \A node \in ValidatorIds:
  \A job:
    /\ AsyncIoTopologyTypeInvariant
    /\ AsyncIoSequenceTyped(asyncIoQueues[node])
    /\ job.class # "Consensus"
    /\ asyncIoQueues' =
         [asyncIoQueues EXCEPT ![node] = Append(@, job)]
    => \A other \in ValidatorIds:
         ConsensusIoCandidates(other)' = ConsensusIoCandidates(other)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, NEW job,
                AsyncIoTopologyTypeInvariant,
                AsyncIoSequenceTyped(asyncIoQueues[node]),
                job.class # "Consensus",
                asyncIoQueues' =
                  [asyncIoQueues EXCEPT ![node] = Append(@, job)]
         PROVE \A other \in ValidatorIds:
                 ConsensusIoCandidates(other)' =
                   ConsensusIoCandidates(other)
    <2>1. asyncIoQueues[node] \in Seq(Range(asyncIoQueues[node]))
      BY <1>1 DEF AsyncIoSequenceTyped
    <2>2. SequenceSet(Append(asyncIoQueues[node], job)) =
             SequenceSet(asyncIoQueues[node]) \cup {job}
      BY <2>1, SequenceSetAfterAppend
    <2>3. ASSUME NEW other \in ValidatorIds
           PROVE ConsensusIoCandidates(other)' =
                   ConsensusIoCandidates(other)
      <3>1. CASE other = node
        <4>1. asyncIoQueues'[other] =
                 Append(asyncIoQueues[node], job)
          BY <1>1, <3>1, FunctionalAppendUpdateAtKey
             DEF AsyncIoTopologyTypeInvariant
        <4> QED BY <1>1, <2>2, <3>1, <4>1, Isa
             DEF ConsensusIoCandidates
      <3>2. CASE other # node
        <4>1. asyncIoQueues'[other] = asyncIoQueues[other]
          BY <1>1, <3>2, <2>3,
             FunctionalAppendUpdateAwayFromKey
             DEF AsyncIoTopologyTypeInvariant
        <4> QED BY <4>1 DEF ConsensusIoCandidates
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM NonConsensusIoAppendPreservesProgressOwnership ==
  \A node \in ValidatorIds:
  \A job:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ job.class # "Consensus"
    /\ asyncIoQueues' =
         [asyncIoQueues EXCEPT ![node] = Append(@, job)]
    /\ UNCHANGED
         <<AsyncProgressOwnershipCoreVars, AsyncBusyConsumerVars,
           asyncCommandQueues,
           asyncOutstandingWork, asyncIoReadyCompletions,
           asyncLocalReadyCompletions,
           asyncDeferredCompletionQueues,
           asyncDeferredProgressQueues,
           asyncDeferredNormalQueues, asyncCausalQueues>>
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, NEW job,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                job.class # "Consensus",
                asyncIoQueues' =
                  [asyncIoQueues EXCEPT ![node] = Append(@, job)],
                UNCHANGED
                  <<AsyncProgressOwnershipCoreVars, AsyncBusyConsumerVars,
                    asyncCommandQueues,
                    asyncOutstandingWork, asyncIoReadyCompletions,
                    asyncLocalReadyCompletions,
                    asyncDeferredCompletionQueues,
                    asyncDeferredProgressQueues,
                    asyncDeferredNormalQueues, asyncCausalQueues>>
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoSequenceTyped(asyncIoQueues[node])
      BY <1>1
         DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant
    <2>2. \A other \in ValidatorIds:
             ConsensusIoCandidates(other)' =
               ConsensusIoCandidates(other)
      BY <1>1, <2>1,
         NonConsensusIoAppendPreservesConsensusCandidates
    <2>3. AsyncLogicalCandidateOwnershipInvariant'
      BY <1>1, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant,
             QueuedCandidates, DeferredCandidates, CausalCandidates,
             TrackedWorkCandidates, SequenceHasUniqueValues,
             SequenceSet
    <2>4. AsyncOutstandingCarrierInvariant'
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE asyncOutstandingWork'[other] =
                     ConsensusIoCandidates(other)'
                       \cup SequenceSet(
                            asyncIoReadyCompletions'[other])
                       \cup SequenceSet(
                            asyncLocalReadyCompletions'[other])
        <4>1. /\ asyncOutstandingWork'[other] =
                       asyncOutstandingWork[other]
               /\ asyncIoReadyCompletions'[other] =
                       asyncIoReadyCompletions[other]
               /\ asyncLocalReadyCompletions'[other] =
                       asyncLocalReadyCompletions[other]
          BY <1>1, Isa
        <4>2. asyncOutstandingWork[other] =
                 ConsensusIoCandidates(other)
                   \cup SequenceSet(asyncIoReadyCompletions[other])
                   \cup SequenceSet(
                        asyncLocalReadyCompletions[other])
          BY <1>1 DEF AsyncProgressOwnershipInvariant,
             AsyncOutstandingCarrierInvariant
        <4> QED BY <2>2, <3>1, <4>1, <4>2
      <3> QED BY <3>1 DEF AsyncOutstandingCarrierInvariant
    <2>5. ActiveBusyCompletionCarrier' =
             ActiveBusyCompletionCarrier
      BY <1>1, Isa
         DEF ActiveBusyCompletionCarrier, QueuedCandidates,
             CausalCandidates, TrackedWorkCandidates, SequenceSet
    <2>6. /\ SerializedBusyOwnershipInvariant'
           /\ BusyCompletionWitnessInvariant'
      BY <1>1, <2>5, ProgressCoreFramePreservesBusyOwnership
    <2> QED BY <2>3, <2>4, <2>6
         DEF AsyncProgressOwnershipInvariant
  <1> QED BY <1>1

THEOREM EnqueueIoControlPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ EnqueueIoLocalControlWork(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                EnqueueIoLocalControlWork(node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. AsyncIoControlJob.class # "Consensus"
      BY DEF AsyncIoControlJob, AsyncIoJob
    <2>2. asyncIoQueues' =
             [asyncIoQueues EXCEPT
                ![node] = Append(@, AsyncIoControlJob)]
      BY <1>1 DEF EnqueueIoLocalControlWork
    <2>3. UNCHANGED
             <<AsyncProgressOwnershipCoreVars, AsyncBusyConsumerVars,
               asyncCommandQueues,
               asyncOutstandingWork, asyncIoReadyCompletions,
               asyncLocalReadyCompletions,
               asyncDeferredCompletionQueues,
               asyncDeferredProgressQueues,
               asyncDeferredNormalQueues, asyncCausalQueues>>
      BY <1>1, Isa
         DEF EnqueueIoLocalControlWork,
             AsyncProgressOwnershipCoreVars,
             AsyncBusyConsumerVars,
             AsyncDeferredVars, LeaveCausalQueues,
             AsyncLocalAdmissionVars, vars
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
         NonConsensusIoAppendPreservesProgressOwnership
  <1> QED BY <1>1

THEOREM HistoricalRunnerPreservesProgressOwnership ==
  \A node \in AsyncResponsiveAppliedArchiveServers:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ RunHistoricalServer(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncResponsiveAppliedArchiveServers,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                RunHistoricalServer(node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. TypeInvariant
      BY <1>1
         DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety
    <2>2. AsyncResponsiveAppliedArchiveServers \subseteq ValidatorIds
      BY <2>1, AsyncResponsiveAppliedArchiveServersAreValidators
    <2>3. node \in ValidatorIds
      BY <1>1, <2>2
    <2>4. CASE HistoricalDrainableIngressIndices(node) = {}
      <3>1. HistoricalIdleStep
        BY <1>1, <2>4 DEF RunHistoricalServer
      <3>2. UNCHANGED AsyncProgressOwnershipVars
        BY <1>1, <3>1, Isa
           DEF RunHistoricalServer, HistoricalIdleStep,
               AsyncProgressOwnershipVars,
               AsyncProgressOwnershipCoreVars,
               AsyncProgressOwnershipSchedulerVars,
               AsyncIoVars, AsyncDeferredVars,
               AsyncLocalAdmissionVars, vars
      <3> QED BY <1>1, <3>2, AsyncProgressOwnershipStutter
    <2>5. CASE HistoricalDrainableIngressIndices(node) # {}
      <3>1. DrainHistoricalIngressSelected(node)
        BY <1>1, <2>5 DEF RunHistoricalServer
      <3>2. CASE HistoricalSelectedRequestAuthorized(node)
        <4> DEFINE Job ==
               AsyncIoCertifiedServeJob(
                 node, DeliveryCandidate(HistoricalSelectedItem(node)))
        <4>1. Job.class # "Consensus"
          BY DEF Job, AsyncIoCertifiedServeJob, AsyncIoJob
        <4>2. asyncIoQueues' =
                 [asyncIoQueues EXCEPT ![node] = Append(@, Job)]
          BY <3>1, <3>2, HistoricalAuthorizedRequestFrame DEF Job
        <4>3. UNCHANGED
                 <<AsyncProgressOwnershipCoreVars, asyncCommandQueues,
                   asyncOutstandingWork, asyncIoReadyCompletions,
                   asyncLocalReadyCompletions,
                   asyncDeferredCompletionQueues,
                   asyncDeferredProgressQueues,
                   asyncDeferredNormalQueues, asyncCausalQueues>>
          BY <3>1, <3>2, HistoricalAuthorizedRequestFrame, Isa
             DEF DrainHistoricalIngressSelected,
                 AsyncProgressOwnershipCoreVars,
                 AsyncDeferredVars, AsyncIoVars, vars
        <4> QED BY <1>1, <2>3, <4>1, <4>2, <4>3,
             NonConsensusIoAppendPreservesProgressOwnership
      <3>3. CASE ~HistoricalSelectedRequestAuthorized(node)
        <4>1. UNCHANGED AsyncIoVars
          BY <3>1, <3>3, Isa
             DEF DrainHistoricalIngressSelected,
                 HistoricalSelectedRequestAuthorized,
                 HistoricalSelectedItem
        <4>2. UNCHANGED AsyncProgressOwnershipVars
          BY <3>1, <4>1, Isa
             DEF DrainHistoricalIngressSelected,
                 AsyncProgressOwnershipVars,
                 AsyncProgressOwnershipCoreVars,
                 AsyncProgressOwnershipSchedulerVars,
                 AsyncDeferredVars, AsyncIoVars, vars
        <4> QED BY <1>1, <4>2, AsyncProgressOwnershipStutter
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

ConsensusCandidatesInIoQueue(queue) ==
  {job.candidate:
     job \in {entry \in SequenceSet(queue):
                entry.class = "Consensus"}}

THEOREM ServiceHeadPreservesIoCarrierSet ==
  \A queue, ioReadyQueue:
    /\ AsyncIoSequenceTyped(queue)
    /\ Len(queue) > 0
    /\ ioReadyQueue \in Seq(Range(ioReadyQueue))
    => ConsensusCandidatesInIoQueue(Tail(queue))
         \cup SequenceSet(
               AsyncIoReadyAfterService(queue, ioReadyQueue))
       = ConsensusCandidatesInIoQueue(queue)
           \cup SequenceSet(ioReadyQueue)
PROOF
  <1>1. ASSUME NEW queue, NEW ioReadyQueue,
                AsyncIoSequenceTyped(queue),
                Len(queue) > 0,
                ioReadyQueue \in Seq(Range(ioReadyQueue))
         PROVE ConsensusCandidatesInIoQueue(Tail(queue))
                   \cup SequenceSet(
                         AsyncIoReadyAfterService(queue, ioReadyQueue))
                 = ConsensusCandidatesInIoQueue(queue)
                     \cup SequenceSet(ioReadyQueue)
    <2>1. SequenceSet(queue) =
             {Head(queue)} \cup SequenceSet(Tail(queue))
      BY <1>1, SequenceSetHeadTailDecomposition
         DEF AsyncIoSequenceTyped
    <2>2. CASE Head(queue).class = "Consensus"
      <3>1. SequenceSet(
               AsyncIoReadyAfterService(queue, ioReadyQueue)) =
                 SequenceSet(ioReadyQueue) \cup
                   {Head(queue).candidate}
        BY <1>1, <2>2, SequenceSetAfterAppend
           DEF AsyncIoReadyAfterService
      <3>2. ConsensusCandidatesInIoQueue(queue) =
               {Head(queue).candidate} \cup
                 ConsensusCandidatesInIoQueue(Tail(queue))
        BY <2>1, <2>2, Isa DEF ConsensusCandidatesInIoQueue
      <3> QED BY <3>1, <3>2, Isa
           DEF ConsensusCandidatesInIoQueue
    <2>3. CASE Head(queue).class # "Consensus"
      <3>1. AsyncIoReadyAfterService(queue, ioReadyQueue) =
               ioReadyQueue
        BY <2>3 DEF AsyncIoReadyAfterService
      <3> QED BY <2>1, <2>3, <3>1, Isa
           DEF ConsensusCandidatesInIoQueue
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ServiceIoWorkerPreservesOutstandingCarrier ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncOutstandingCarrierInvariant
    /\ ServiceIoWorkerWork(node)
    => AsyncOutstandingCarrierInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncOutstandingCarrierInvariant,
                ServiceIoWorkerWork(node)
         PROVE AsyncOutstandingCarrierInvariant'
    <2>1. /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoSequenceTyped(asyncIoQueues[node])
           /\ AsyncCompletionSequenceTyped(
                asyncIoReadyCompletions[node])
           /\ Len(asyncIoQueues[node]) > 0
      BY <1>1
         DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant,
             AsyncIoWorkContentTypeInvariant,
             ServiceIoWorkerWork, AsyncIoQueueDepth
    <2>2. asyncIoReadyCompletions[node] \in
             Seq(Range(asyncIoReadyCompletions[node]))
      BY <2>1 DEF AsyncCompletionSequenceTyped
    <2>3. /\ asyncIoQueues'[node] = Tail(asyncIoQueues[node])
           /\ asyncIoReadyCompletions'[node] =
                AsyncIoReadyAfterService(
                  asyncIoQueues[node], asyncIoReadyCompletions[node])
           /\ asyncOutstandingWork' = asyncOutstandingWork
           /\ asyncLocalReadyCompletions' =
                asyncLocalReadyCompletions
      BY <1>1, <2>1, FunctionalTailUpdateAtKey,
         FunctionalAppendUpdateAtKey, Isa
         DEF ServiceIoWorkerWork, AsyncIoReadyAfterService,
             AsyncIoTopologyTypeInvariant
    <2>4. ConsensusIoCandidates(node)'
               \cup SequenceSet(asyncIoReadyCompletions'[node]) =
             ConsensusIoCandidates(node)
               \cup SequenceSet(asyncIoReadyCompletions[node])
      BY <2>1, <2>2, <2>3,
         ServiceHeadPreservesIoCarrierSet
         DEF ConsensusIoCandidates, ConsensusCandidatesInIoQueue
    <2>5. ASSUME NEW other \in ValidatorIds
           PROVE asyncOutstandingWork'[other] =
                   ConsensusIoCandidates(other)'
                     \cup SequenceSet(asyncIoReadyCompletions'[other])
                     \cup SequenceSet(
                          asyncLocalReadyCompletions'[other])
      <3>1. CASE other = node
        BY <1>1, <2>3, <2>4, <3>1
           DEF AsyncOutstandingCarrierInvariant
      <3>2. CASE other # node
        <4>1. /\ asyncIoQueues'[other] = asyncIoQueues[other]
               /\ asyncIoReadyCompletions'[other] =
                    asyncIoReadyCompletions[other]
          BY <1>1, <2>1, <2>5, <3>2,
             FunctionalTailUpdateAwayFromKey,
             FunctionalAppendUpdateAwayFromKey, Isa
             DEF ServiceIoWorkerWork, AsyncIoTopologyTypeInvariant
        <4> QED BY <1>1, <2>3, <3>2, <4>1
             DEF AsyncOutstandingCarrierInvariant,
                 ConsensusIoCandidates
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>5 DEF AsyncOutstandingCarrierInvariant
  <1> QED BY <1>1

THEOREM ServiceIoWorkerPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ServiceIoWorkerWork(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                ServiceIoWorkerWork(node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. AsyncLogicalCandidateOwnershipInvariant'
      BY <1>1, Isa
         DEF ServiceIoWorkerWork,
             AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant,
             QueuedCandidates, DeferredCandidates, CausalCandidates,
             TrackedWorkCandidates, SequenceHasUniqueValues,
             SequenceSet, AsyncDeferredVars, LeaveCausalQueues, vars
    <2>2. AsyncOutstandingCarrierInvariant'
      BY <1>1, ServiceIoWorkerPreservesOutstandingCarrier
         DEF AsyncProgressOwnershipInvariant
    <2>3. /\ UNCHANGED AsyncProgressOwnershipCoreVars
           /\ UNCHANGED AsyncBusyConsumerVars
           /\ ActiveBusyCompletionCarrier' =
                ActiveBusyCompletionCarrier
      BY <1>1, Isa
         DEF ServiceIoWorkerWork,
             AsyncProgressOwnershipCoreVars,
             AsyncBusyConsumerVars,
             ActiveBusyCompletionCarrier, QueuedCandidates,
             CausalCandidates, TrackedWorkCandidates,
             AsyncDeferredVars, LeaveCausalQueues, vars
    <2>4. /\ SerializedBusyOwnershipInvariant'
           /\ BusyCompletionWitnessInvariant'
      BY <1>1, <2>3, ProgressCoreFramePreservesBusyOwnership
    <2> QED BY <2>1, <2>2, <2>4
         DEF AsyncProgressOwnershipInvariant
  <1> QED BY <1>1

THEOREM LocalPhaseAdvancePreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncProgressOwnershipInvariant
    /\ LocalAdmissionStep(node)
    /\ ~LocalAdmissionCanAdvance(node)
    => AsyncProgressOwnershipInvariant'
BY AsyncProgressOwnershipStutter, Isa
   DEF LocalAdmissionStep, LeaveCausalQueues, AsyncIoVars,
       AsyncDeferredVars, AsyncLocalAdmissionVars,
       AsyncProgressOwnershipVars, AsyncProgressOwnershipCoreVars,
       AsyncProgressOwnershipSchedulerVars, vars

THEOREM FreshAppendPreservesUniqueSequence ==
  \A sequence, value:
    /\ sequence \in Seq(Range(sequence))
    /\ SequenceHasUniqueValues(sequence)
    /\ value \notin SequenceSet(sequence)
    => SequenceHasUniqueValues(Append(sequence, value))
PROOF
  <1>1. ASSUME NEW sequence, NEW value,
                sequence \in Seq(Range(sequence)),
                SequenceHasUniqueValues(sequence),
                value \notin SequenceSet(sequence)
         PROVE SequenceHasUniqueValues(Append(sequence, value))
    <2>1. IsInjective(sequence)
      BY <1>1, UniqueSequenceLengthImpliesInjective
    <2>2. value \notin Range(sequence)
      BY <1>1, RangeEquality DEF SequenceSet
    <2>3. /\ sequence \in Seq(Range(sequence) \cup {value})
           /\ value \in Range(sequence) \cup {value}
      <3>1. Range(sequence) \subseteq Range(sequence) \cup {value}
        BY Isa
      <3> QED BY <1>1, <3>1, SeqMonotonic, Isa
    <2>4. IsInjective(Append(sequence, value))
      BY <2>1, <2>2, <2>3, AppendInjectiveSeq
    <2>5. Append(sequence, value) \in
             Seq(Range(sequence) \cup {value})
      BY <2>3, AppendProperties
    <2>6. Append(sequence, value) \in
             Seq(Range(Append(sequence, value)))
      BY <2>5, SeqOfRange
    <2>7. Len(Append(sequence, value)) =
             Cardinality(SequenceSet(Append(sequence, value)))
      BY <2>4, <2>6,
         InjectiveSequenceLengthMatchesSetCardinality
    <2> QED BY <2>7 DEF SequenceHasUniqueValues
  <1> QED BY <1>1

THEOREM UnionOfSequenceSetsAfterAppendAtKey ==
  \A keys, mapping, key, value:
    /\ key \in keys
    /\ DOMAIN mapping = keys
    /\ mapping[key] \in Seq(Range(mapping[key]))
    => (UNION
          {SequenceSet(
             [mapping EXCEPT ![key] = Append(@, value)][other]):
             other \in keys})
         = (UNION {SequenceSet(mapping[other]): other \in keys})
             \cup {value}
PROOF
  <1>1. ASSUME NEW keys, NEW mapping, NEW key, NEW value,
                key \in keys,
                DOMAIN mapping = keys,
                mapping[key] \in Seq(Range(mapping[key]))
         PROVE (UNION
                  {SequenceSet(
                     [mapping EXCEPT ![key] = Append(@, value)][other]):
                     other \in keys})
                 = (UNION
                      {SequenceSet(mapping[other]): other \in keys})
                     \cup {value}
    <2>1. SequenceSet(Append(mapping[key], value)) =
             SequenceSet(mapping[key]) \cup {value}
      BY <1>1, SequenceSetAfterAppend
    <2>2. [mapping EXCEPT ![key] = Append(@, value)][key] =
             Append(mapping[key], value)
      BY <1>1, FunctionalAppendUpdateAtKey
    <2>3. \A other \in keys \ {key}:
             [mapping EXCEPT ![key] = Append(@, value)][other] =
               mapping[other]
      BY <1>1, FunctionalAppendUpdateAwayFromKey
    <2>4. UNION
             {SequenceSet(
                [mapping EXCEPT ![key] = Append(@, value)][other]):
                other \in keys}
             \subseteq
               UNION {SequenceSet(mapping[other]): other \in keys}
                 \cup {value}
      <3>1. ASSUME NEW candidate \in
                    UNION
                      {SequenceSet(
                         [mapping EXCEPT
                            ![key] = Append(@, value)][other]):
                         other \in keys}
             PROVE candidate \in
                     UNION
                       {SequenceSet(mapping[other]): other \in keys}
                       \cup {value}
        <4>1. PICK other \in keys:
                   candidate \in
                     SequenceSet(
                       [mapping EXCEPT
                          ![key] = Append(@, value)][other])
          BY <3>1
        <4>2. CASE other = key
          BY <2>1, <2>2, <4>1, <4>2, Isa
        <4>3. CASE other # key
          <5>1. other \in keys \ {key}
            BY <4>1, <4>3
          <5> QED BY <2>3, <4>1, <5>1, Isa
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>5. UNION {SequenceSet(mapping[other]): other \in keys}
              \cup {value}
              \subseteq
                UNION
                  {SequenceSet(
                     [mapping EXCEPT
                        ![key] = Append(@, value)][other]):
                     other \in keys}
      <3>1. ASSUME NEW candidate \in
                    UNION
                      {SequenceSet(mapping[other]): other \in keys}
                      \cup {value}
             PROVE candidate \in
                     UNION
                       {SequenceSet(
                          [mapping EXCEPT
                             ![key] = Append(@, value)][other]):
                          other \in keys}
        <4>1. CASE candidate = value
          BY <1>1, <2>1, <2>2, <4>1, Isa
        <4>2. CASE candidate \in
                     UNION
                       {SequenceSet(mapping[other]): other \in keys}
          <5>1. PICK other \in keys:
                     candidate \in SequenceSet(mapping[other])
            BY <4>2
          <5>2. CASE other = key
            BY <2>1, <2>2, <5>1, <5>2, Isa
          <5>3. CASE other # key
            <6>1. other \in keys \ {key}
              BY <5>1, <5>3
            <6> QED BY <2>3, <5>1, <6>1, Isa
          <5> QED BY <5>2, <5>3
        <4> QED BY <3>1, <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>4, <2>5, Isa
  <1> QED BY <1>1

THEOREM UnionOfSequenceSetsAfterTailAtKey ==
  \A keys, mapping, key, value:
    /\ key \in keys
    /\ DOMAIN mapping = keys
    /\ value.node = key
    /\ SequenceSet(Tail(mapping[key])) =
         SequenceSet(mapping[key]) \ {value}
    /\ \A owner \in keys:
         \A candidate \in SequenceSet(mapping[owner]):
           candidate.node = owner
    => UNION
         {SequenceSet(
            [mapping EXCEPT ![key] = Tail(@)][owner]):
            owner \in keys}
         = (UNION
              {SequenceSet(mapping[owner]): owner \in keys})
             \ {value}
PROOF
  <1>1. ASSUME NEW keys, NEW mapping, NEW key, NEW value,
                key \in keys,
                DOMAIN mapping = keys,
                value.node = key,
                SequenceSet(Tail(mapping[key])) =
                  SequenceSet(mapping[key]) \ {value},
                \A owner \in keys:
                  \A candidate \in SequenceSet(mapping[owner]):
                    candidate.node = owner
         PROVE UNION
                 {SequenceSet(
                    [mapping EXCEPT ![key] = Tail(@)][owner]):
                    owner \in keys}
                 = (UNION
                      {SequenceSet(mapping[owner]): owner \in keys})
                     \ {value}
    <2>1. [mapping EXCEPT ![key] = Tail(@)][key] =
             Tail(mapping[key])
      BY <1>1, FunctionalTailUpdateAtKey
    <2>2. \A owner \in keys \ {key}:
             [mapping EXCEPT ![key] = Tail(@)][owner] =
               mapping[owner]
      BY <1>1, FunctionalTailUpdateAwayFromKey
    <2>3. UNION
             {SequenceSet(
                [mapping EXCEPT ![key] = Tail(@)][owner]):
                owner \in keys}
             \subseteq
               (UNION
                  {SequenceSet(mapping[owner]): owner \in keys})
                 \ {value}
      <3>1. ASSUME NEW candidate \in
                    UNION
                      {SequenceSet(
                         [mapping EXCEPT ![key] = Tail(@)][owner]):
                         owner \in keys}
             PROVE candidate \in
                     (UNION
                        {SequenceSet(mapping[owner]): owner \in keys})
                       \ {value}
        <4>1. PICK owner \in keys:
                   candidate \in
                     SequenceSet(
                       [mapping EXCEPT ![key] = Tail(@)][owner])
          BY <3>1
        <4>2. CASE owner = key
          BY <1>1, <2>1, <4>1, <4>2, Isa
        <4>3. CASE owner # key
          <5>1. owner \in keys \ {key}
            BY <4>1, <4>3
          <5>2. candidate \in SequenceSet(mapping[owner])
            BY <2>2, <4>1, <5>1
          <5>3. candidate.node = owner
            BY <1>1, <4>1, <5>2
          <5>4. candidate # value
            BY <1>1, <4>3, <5>3
          <5> QED BY <5>2, <5>4, Isa
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>4. (UNION
              {SequenceSet(mapping[owner]): owner \in keys})
             \ {value}
             \subseteq
               UNION
                 {SequenceSet(
                    [mapping EXCEPT ![key] = Tail(@)][owner]):
                    owner \in keys}
      <3>1. ASSUME NEW candidate \in
                    (UNION
                       {SequenceSet(mapping[owner]): owner \in keys})
                      \ {value}
             PROVE candidate \in
                     UNION
                       {SequenceSet(
                          [mapping EXCEPT ![key] = Tail(@)][owner]):
                          owner \in keys}
        <4>1. PICK owner \in keys:
                   candidate \in SequenceSet(mapping[owner])
          BY <3>1
        <4>2. CASE owner = key
          BY <1>1, <2>1, <3>1, <4>1, <4>2, Isa
        <4>3. CASE owner # key
          <5>1. owner \in keys \ {key}
            BY <4>1, <4>3
          <5> QED BY <2>2, <4>1, <5>1, Isa
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2> QED BY <2>3, <2>4, Isa
  <1> QED BY <1>1

THEOREM FreshCommandAppendPreservesLogicalOwnership ==
  \A node \in ValidatorIds:
  \A candidate:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ candidate.node = node
    /\ ~CandidateScheduled(candidate)
    /\ asyncCommandQueues' =
         [asyncCommandQueues EXCEPT ![node] = Append(@, candidate)]
    /\ UNCHANGED
         <<asyncOutstandingWork, asyncDeferredCompletionQueues,
           asyncDeferredProgressQueues, asyncDeferredNormalQueues,
           asyncCausalQueues>>
    => /\ AsyncLogicalCandidateOwnershipInvariant'
       /\ QueuedCandidates' = QueuedCandidates \cup {candidate}
       /\ ActiveBusyCompletionCarrier \subseteq
            ActiveBusyCompletionCarrier'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, NEW candidate,
                AsyncRuntimeScalarTypeInvariant,
                AsyncLogicalCandidateOwnershipInvariant,
                candidate.node = node,
                ~CandidateScheduled(candidate),
                asyncCommandQueues' =
                  [asyncCommandQueues EXCEPT
                     ![node] = Append(@, candidate)],
                UNCHANGED
                  <<asyncOutstandingWork,
                    asyncDeferredCompletionQueues,
                    asyncDeferredProgressQueues,
                    asyncDeferredNormalQueues, asyncCausalQueues>>
         PROVE /\ AsyncLogicalCandidateOwnershipInvariant'
               /\ QueuedCandidates' = QueuedCandidates \cup {candidate}
               /\ ActiveBusyCompletionCarrier \subseteq
                    ActiveBusyCompletionCarrier'
    <2>1. /\ DOMAIN asyncCommandQueues = ValidatorIds
           /\ AsyncQueueTyped(asyncCommandQueues[node])
           /\ asyncCommandQueues[node] \in
                Seq(Range(asyncCommandQueues[node]))
           /\ SequenceHasUniqueValues(asyncCommandQueues[node])
           /\ candidate \notin SequenceSet(asyncCommandQueues[node])
      BY <1>1, Isa
         DEF AsyncRuntimeScalarTypeInvariant,
             AsyncLogicalCandidateOwnershipInvariant,
             CandidateScheduled, QueuedCandidates, AsyncQueueTyped
    <2>2. SequenceHasUniqueValues(
             Append(asyncCommandQueues[node], candidate))
      BY <2>1, FreshAppendPreservesUniqueSequence
    <2>3. SequenceSet(Append(asyncCommandQueues[node], candidate)) =
             SequenceSet(asyncCommandQueues[node]) \cup {candidate}
      BY <2>1, SequenceSetAfterAppend
    <2>4. \A other \in ValidatorIds:
             SequenceHasUniqueValues(asyncCommandQueues'[other])
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE SequenceHasUniqueValues(asyncCommandQueues'[other])
        <4>1. CASE other = node
          <5>1. asyncCommandQueues'[other] =
                   Append(asyncCommandQueues[node], candidate)
            BY <1>1, <2>1, <4>1,
               FunctionalAppendUpdateAtKey
          <5> QED BY <2>2, <5>1
        <4>2. CASE other # node
          <5>1. asyncCommandQueues'[other] =
                   asyncCommandQueues[other]
            BY <1>1, <2>1, <3>1, <4>2,
               FunctionalAppendUpdateAwayFromKey
          <5> QED BY <1>1, <5>1
               DEF AsyncLogicalCandidateOwnershipInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>5. UNION
             {SequenceSet(asyncCommandQueues'[other]):
                other \in ValidatorIds} =
             UNION
               {SequenceSet(asyncCommandQueues[other]):
                  other \in ValidatorIds}
               \cup {candidate}
      BY <1>1, <2>1,
         UnionOfSequenceSetsAfterAppendAtKey
    <2>6. QueuedCandidates' = QueuedCandidates \cup {candidate}
      BY <2>5 DEF QueuedCandidates
    <2>7. /\ DeferredCandidates' = DeferredCandidates
           /\ CausalCandidates' = CausalCandidates
           /\ TrackedWorkCandidates' = TrackedWorkCandidates
      BY <1>1, Isa
         DEF DeferredCandidates, CausalCandidates,
             TrackedWorkCandidates, SequenceSet
    <2>8. AsyncLogicalCandidateOwnershipInvariant'
      BY <1>1, <2>4, <2>6, <2>7, Isa
         DEF AsyncLogicalCandidateOwnershipInvariant,
             CandidateScheduled
    <2>9. ActiveBusyCompletionCarrier \subseteq
             ActiveBusyCompletionCarrier'
      BY <2>6, <2>7, Isa DEF ActiveBusyCompletionCarrier
    <2> QED BY <2>6, <2>8, <2>9
  <1> QED BY <1>1

THEOREM UnionOfSetsAfterAddAtKey ==
  \A keys, mapping, key, value:
    /\ key \in keys
    /\ DOMAIN mapping = keys
    => UNION
         {[mapping EXCEPT ![key] = @ \cup {value}][owner]:
            owner \in keys}
         = (UNION {mapping[owner]: owner \in keys}) \cup {value}
PROOF
  <1>1. ASSUME NEW keys, NEW mapping, NEW key, NEW value,
                key \in keys,
                DOMAIN mapping = keys
         PROVE UNION
                 {[mapping EXCEPT ![key] = @ \cup {value}][owner]:
                    owner \in keys}
                 = (UNION {mapping[owner]: owner \in keys})
                     \cup {value}
    <2>1. [mapping EXCEPT ![key] = @ \cup {value}][key] =
             mapping[key] \cup {value}
      BY <1>1, Isa
    <2>2. \A owner \in keys \ {key}:
             [mapping EXCEPT ![key] = @ \cup {value}][owner] =
               mapping[owner]
      BY <1>1, FunctionalUpdateAwayFromKey
    <2>3. UNION
             {[mapping EXCEPT ![key] = @ \cup {value}][owner]:
                owner \in keys}
             \subseteq
               (UNION {mapping[owner]: owner \in keys})
                 \cup {value}
      <3>1. ASSUME NEW candidate \in
                    UNION
                      {[mapping EXCEPT
                         ![key] = @ \cup {value}][owner]:
                         owner \in keys}
             PROVE candidate \in
                     (UNION {mapping[owner]: owner \in keys})
                       \cup {value}
        <4>1. PICK owner \in keys:
                   candidate \in
                     [mapping EXCEPT ![key] = @ \cup {value}][owner]
          BY <3>1
        <4>2. CASE owner = key
          BY <2>1, <4>1, <4>2, Isa
        <4>3. CASE owner # key
          <5>1. owner \in keys \ {key}
            BY <4>1, <4>3
          <5> QED BY <2>2, <4>1, <5>1, Isa
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>4. (UNION {mapping[owner]: owner \in keys}) \cup {value}
             \subseteq
               UNION
                 {[mapping EXCEPT ![key] = @ \cup {value}][owner]:
                    owner \in keys}
      <3>1. ASSUME NEW candidate \in
                    (UNION {mapping[owner]: owner \in keys})
                      \cup {value}
             PROVE candidate \in
                     UNION
                       {[mapping EXCEPT
                          ![key] = @ \cup {value}][owner]:
                          owner \in keys}
        <4>1. CASE candidate = value
          BY <1>1, <2>1, <4>1, Isa
        <4>2. CASE candidate \in
                     UNION {mapping[owner]: owner \in keys}
          <5>1. PICK owner \in keys:
                     candidate \in mapping[owner]
            BY <4>2
          <5>2. CASE owner = key
            BY <2>1, <5>1, <5>2, Isa
          <5>3. CASE owner # key
            <6>1. owner \in keys \ {key}
              BY <5>1, <5>3
            <6> QED BY <2>2, <5>1, <6>1, Isa
          <5> QED BY <5>2, <5>3
        <4> QED BY <3>1, <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>3, <2>4, Isa
  <1> QED BY <1>1

THEOREM UnionOfOwnedSetsAfterRemoveAtKey ==
  \A keys, mapping, key, value:
    /\ key \in keys
    /\ DOMAIN mapping = keys
    /\ value.node = key
    /\ value \in mapping[key]
    /\ \A owner \in keys:
         \A candidate \in mapping[owner]: candidate.node = owner
    => UNION
         {[mapping EXCEPT ![key] = @ \ {value}][owner]:
            owner \in keys}
         = (UNION {mapping[owner]: owner \in keys}) \ {value}
PROOF
  <1>1. ASSUME NEW keys, NEW mapping, NEW key, NEW value,
                key \in keys,
                DOMAIN mapping = keys,
                value.node = key,
                value \in mapping[key],
                \A owner \in keys:
                  \A candidate \in mapping[owner]:
                    candidate.node = owner
         PROVE UNION
                 {[mapping EXCEPT ![key] = @ \ {value}][owner]:
                    owner \in keys}
                 = (UNION {mapping[owner]: owner \in keys}) \ {value}
    <2>1. [mapping EXCEPT ![key] = @ \ {value}][key] =
             mapping[key] \ {value}
      BY <1>1, Isa
    <2>2. \A owner \in keys \ {key}:
             [mapping EXCEPT ![key] = @ \ {value}][owner] =
               mapping[owner]
      BY <1>1, FunctionalUpdateAwayFromKey
    <2>3. UNION
             {[mapping EXCEPT ![key] = @ \ {value}][owner]:
                owner \in keys}
             \subseteq
               (UNION {mapping[owner]: owner \in keys}) \ {value}
      <3>1. ASSUME NEW candidate \in
                    UNION
                      {[mapping EXCEPT ![key] = @ \ {value}][owner]:
                         owner \in keys}
             PROVE candidate \in
                     (UNION {mapping[owner]: owner \in keys}) \ {value}
        <4>1. PICK owner \in keys:
                   candidate \in
                     [mapping EXCEPT ![key] = @ \ {value}][owner]
          BY <3>1
        <4>2. CASE owner = key
          BY <2>1, <4>1, <4>2, Isa
        <4>3. CASE owner # key
          <5>1. owner \in keys \ {key}
            BY <4>1, <4>3
          <5>2. candidate \in mapping[owner]
            BY <2>2, <4>1, <5>1
          <5>3. candidate.node = owner
            BY <1>1, <4>1, <5>2
          <5>4. candidate # value
            BY <1>1, <4>3, <5>3
          <5> QED BY <4>1, <5>2, <5>4, Isa
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>4. (UNION {mapping[owner]: owner \in keys}) \ {value}
              \subseteq
                UNION
                  {[mapping EXCEPT ![key] = @ \ {value}][owner]:
                     owner \in keys}
      <3>1. ASSUME NEW candidate \in
                    (UNION {mapping[owner]: owner \in keys}) \ {value}
             PROVE candidate \in
                     UNION
                       {[mapping EXCEPT ![key] = @ \ {value}][owner]:
                          owner \in keys}
        <4>1. PICK owner \in keys: candidate \in mapping[owner]
          BY <3>1
        <4>2. CASE owner = key
          BY <2>1, <3>1, <4>1, <4>2, Isa
        <4>3. CASE owner # key
          <5>1. owner \in keys \ {key}
            BY <4>1, <4>3
          <5> QED BY <2>2, <4>1, <5>1, Isa
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2> QED BY <2>3, <2>4, Isa
  <1> QED BY <1>1

THEOREM ConsensusIoCandidatesDisjointFromReady ==
  \A node \in ValidatorIds:
    AsyncIoQueueContentTypeInvariant
      => ConsensusIoCandidates(node) \cap
           (SequenceSet(asyncIoReadyCompletions[node])
             \cup SequenceSet(asyncLocalReadyCompletions[node])) = {}
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncIoQueueContentTypeInvariant
         PROVE ConsensusIoCandidates(node) \cap
                 (SequenceSet(asyncIoReadyCompletions[node])
                   \cup SequenceSet(
                        asyncLocalReadyCompletions[node])) = {}
    <2>1. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
           /\ AsyncIoConsensusQueueOwnership(
                asyncIoQueues[node],
                asyncIoReadyCompletions[node],
                asyncLocalReadyCompletions[node])
      BY <1>1
         DEF AsyncIoQueueContentTypeInvariant,
             AsyncIoConsensusCandidateOwnership
    <2>2. ASSUME NEW candidate \in
                    ConsensusIoCandidates(node) \cap
                      (SequenceSet(asyncIoReadyCompletions[node])
                        \cup SequenceSet(
                             asyncLocalReadyCompletions[node]))
           PROVE FALSE
      <3>1. PICK job \in SequenceSet(asyncIoQueues[node]):
                 /\ job.class = "Consensus"
                 /\ candidate = job.candidate
        BY <2>2 DEF ConsensusIoCandidates
      <3>2. PICK index \in 1..Len(asyncIoQueues[node]):
                 asyncIoQueues[node][index] = job
        BY <3>1 DEF SequenceSet
      <3>3. index \in AsyncIoConsensusIndices(asyncIoQueues[node])
        BY <3>1, <3>2 DEF AsyncIoConsensusIndices
      <3>4. /\ job.candidate
                      \notin SequenceSet(asyncIoReadyCompletions[node])
             /\ job.candidate
                      \notin SequenceSet(
                           asyncLocalReadyCompletions[node])
        BY <2>1, <3>2, <3>3
           DEF AsyncIoConsensusQueueOwnership
      <3> QED BY <2>2, <3>1, <3>4
    <2> QED BY <2>2
  <1> QED BY <1>1

THEOREM ProducerAdmissionPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ SelectedLocalAdmissionAdvance(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Producer"
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                SelectedLocalAdmissionAdvance(node),
                LocalAdmissionCanAdvance(node),
                SelectedLocalSource(node) = "Producer"
         PROVE AsyncProgressOwnershipInvariant'
    <2> DEFINE Candidate == SelectedCompletionCandidate(node)
    <2> DEFINE Selected == ProducerSelectedReadyQueue(node)
    <2> DEFINE Other == ProducerOtherReadyQueue(node)
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
      BY <1>1
         DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant
    <2>2. /\ ProducerCompletionCanAdmit(node)
           /\ AdmitProducerCompletion(node)
           /\ LeaveCausalQueues
      BY <1>1, SelectedProducerCanAdmit
         DEF SelectedLocalAdmissionAdvance
    <2>3. /\ SelectedCompletionSource(node) \in {"Io", "Local"}
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
      BY <2>1, <2>2, ProducerSelectedCompletionFacts
         DEF Candidate, Selected, Other
    <2>4. Candidate \in TrackedWorkCandidates
      BY <2>3, Isa DEF TrackedWorkCandidates
    <2>4a. /\ Candidate \notin QueuedCandidates
            /\ Candidate \notin DeferredCandidates
            /\ Candidate \notin CausalCandidates
      BY <1>1, <2>4, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>5. /\ asyncCommandQueues' =
                [asyncCommandQueues EXCEPT
                   ![node] = Append(@, Candidate)]
           /\ asyncOutstandingWork' =
                [asyncOutstandingWork EXCEPT
                   ![node] = @ \ {Candidate}]
           /\ UNCHANGED
                <<asyncIoQueues, asyncDeferredCompletionQueues,
                  asyncDeferredProgressQueues,
                  asyncDeferredNormalQueues, asyncCausalQueues>>
      BY <2>2, Isa
         DEF AdmitProducerCompletion, EnqueueCandidate,
             Candidate, AsyncDeferredVars
    <2>6. /\ asyncCommandQueues[node] \in
                  Seq(Range(asyncCommandQueues[node]))
           /\ DOMAIN asyncOutstandingWork = ValidatorIds
           /\ \A owner \in ValidatorIds:
                \A candidate \in asyncOutstandingWork[owner]:
                  candidate.node = owner
      BY <2>1
         DEF AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped,
             AsyncIoTopologyTypeInvariant,
             AsyncIoWorkContentTypeInvariant
    <2>7. /\ QueuedCandidates' = QueuedCandidates \cup {Candidate}
           /\ TrackedWorkCandidates' =
                TrackedWorkCandidates \ {Candidate}
           /\ DeferredCandidates' = DeferredCandidates
           /\ CausalCandidates' = CausalCandidates
      <3>1. QueuedCandidates' =
               QueuedCandidates \cup {Candidate}
        BY <1>1, <2>5, <2>6,
           UnionOfSequenceSetsAfterAppendAtKey
           DEF AsyncRuntimeScalarTypeInvariant, QueuedCandidates
      <3>2. TrackedWorkCandidates' =
               TrackedWorkCandidates \ {Candidate}
        BY <2>3, <2>5, <2>6,
           UnionOfOwnedSetsAfterRemoveAtKey
           DEF TrackedWorkCandidates
      <3>3. /\ DeferredCandidates' = DeferredCandidates
             /\ CausalCandidates' = CausalCandidates
        BY <2>5, Isa
           DEF DeferredCandidates, CausalCandidates, SequenceSet
      <3> QED BY <3>1, <3>2, <3>3
    <2>8. \A otherNode \in ValidatorIds:
             SequenceHasUniqueValues(
               asyncCommandQueues'[otherNode])
      <3>1. ASSUME NEW otherNode \in ValidatorIds
             PROVE SequenceHasUniqueValues(
                     asyncCommandQueues'[otherNode])
        <4>1. CASE otherNode = node
          <5>1. /\ asyncCommandQueues[node] \in
                         Seq(Range(asyncCommandQueues[node]))
                 /\ SequenceHasUniqueValues(
                      asyncCommandQueues[node])
                 /\ Candidate \notin
                      SequenceSet(asyncCommandQueues[node])
            BY <1>1, <2>1, <2>4a
               DEF AsyncRuntimeScalarTypeInvariant,
                   AsyncProgressOwnershipInvariant,
                   AsyncLogicalCandidateOwnershipInvariant,
                   QueuedCandidates, AsyncQueueTyped
          <5>2. SequenceHasUniqueValues(
                   Append(asyncCommandQueues[node], Candidate))
            BY <5>1, FreshAppendPreservesUniqueSequence
          <5>3. asyncCommandQueues'[otherNode] =
                   Append(asyncCommandQueues[node], Candidate)
            BY <2>1, <2>5, <4>1,
               FunctionalAppendUpdateAtKey
               DEF AsyncRuntimeScalarTypeInvariant
          <5> QED BY <5>2, <5>3
        <4>2. CASE otherNode # node
          <5>1. asyncCommandQueues'[otherNode] =
                   asyncCommandQueues[otherNode]
            BY <2>1, <2>5, <3>1, <4>2,
               FunctionalAppendUpdateAwayFromKey
               DEF AsyncRuntimeScalarTypeInvariant
          <5> QED BY <1>1, <5>1
               DEF AsyncProgressOwnershipInvariant,
                   AsyncLogicalCandidateOwnershipInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>9. \A otherNode \in ValidatorIds:
             /\ SequenceHasUniqueValues(asyncCausalQueues'[otherNode])
             /\ SequenceHasUniqueValues(
                  asyncDeferredCompletionQueues'[otherNode])
             /\ SequenceHasUniqueValues(
                  asyncDeferredProgressQueues'[otherNode])
             /\ SequenceHasUniqueValues(
                  asyncDeferredNormalQueues'[otherNode])
      BY <1>1, <2>5, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>10. QueuedCandidates' \cap DeferredCandidates' = {}
      BY <1>1, <2>4a, <2>7, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>11. QueuedCandidates' \cap CausalCandidates' = {}
      BY <1>1, <2>4a, <2>7, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>12. QueuedCandidates' \cap TrackedWorkCandidates' = {}
      BY <1>1, <2>7, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>13. DeferredCandidates' \cap CausalCandidates' = {}
      BY <1>1, <2>7, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>14. DeferredCandidates' \cap TrackedWorkCandidates' = {}
      BY <1>1, <2>7, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>15. CausalCandidates' \cap TrackedWorkCandidates' = {}
      BY <1>1, <2>7, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>16. AsyncLogicalCandidateOwnershipInvariant'
      BY <2>8, <2>9, <2>10, <2>11, <2>12, <2>13, <2>14,
         <2>15
         DEF AsyncLogicalCandidateOwnershipInvariant
    <2>17. AsyncOutstandingCarrierInvariant'
      <3>1. /\ AsyncCompletionSequenceTyped(Other)
             /\ SequenceSet(Other) \subseteq
                  asyncOutstandingWork[node]
             /\ SequenceSet(Selected) \subseteq
                  asyncOutstandingWork[node]
             /\ SequenceSet(Selected) \cap SequenceSet(Other) = {}
        <4>1. CASE SelectedCompletionSource(node) = "Io"
          BY <2>1, <4>1
             DEF Selected, Other, ProducerSelectedReadyQueue,
                 ProducerOtherReadyQueue,
                 AsyncIoWorkContentTypeInvariant
        <4>2. CASE SelectedCompletionSource(node) = "Local"
          BY <2>1, <4>2, Isa
             DEF Selected, Other, ProducerSelectedReadyQueue,
                 ProducerOtherReadyQueue,
                 AsyncIoWorkContentTypeInvariant
        <4> QED BY <2>3, <4>1, <4>2
      <3>2. /\ SequenceSet(Tail(Selected)) =
                    SequenceSet(Selected) \ {Candidate}
             /\ Len(Tail(Selected)) =
                    Cardinality(SequenceSet(Tail(Selected)))
        BY <2>3, UniqueCompletionTailFacts
      <3>3. Candidate \notin ConsensusIoCandidates(node)
        <4>1. Candidate \in
                 SequenceSet(asyncIoReadyCompletions[node])
                   \cup SequenceSet(asyncLocalReadyCompletions[node])
          BY <2>3, Isa
             DEF Selected, ProducerSelectedReadyQueue
        <4>2. ConsensusIoCandidates(node) \cap
                 (SequenceSet(asyncIoReadyCompletions[node])
                   \cup SequenceSet(
                        asyncLocalReadyCompletions[node])) = {}
          BY <2>1, ConsensusIoCandidatesDisjointFromReady
        <4> QED BY <4>1, <4>2
      <3>4. \A otherNode \in ValidatorIds:
               ConsensusIoCandidates(otherNode)' =
                 ConsensusIoCandidates(otherNode)
        <4>1. ASSUME NEW otherNode \in ValidatorIds
               PROVE ConsensusIoCandidates(otherNode)' =
                       ConsensusIoCandidates(otherNode)
          <5>1. asyncIoQueues'[otherNode] =
                   asyncIoQueues[otherNode]
            BY <2>5
          <5> QED BY <5>1 DEF ConsensusIoCandidates
        <4> QED BY <4>1
      <3>5. (SequenceSet(asyncIoReadyCompletions'[node])
                \cup SequenceSet(
                     asyncLocalReadyCompletions'[node])) =
               (SequenceSet(asyncIoReadyCompletions[node])
                 \cup SequenceSet(
                      asyncLocalReadyCompletions[node]))
                 \ {Candidate}
        <4>1. CASE SelectedCompletionSource(node) = "Io"
          <5>1. /\ asyncIoReadyCompletions'[node] =
                         Tail(Selected)
                 /\ asyncLocalReadyCompletions'[node] = Other
            BY <2>1, <2>2, <4>1,
               FunctionalTailUpdateAtKey
               DEF AsyncIoTopologyTypeInvariant,
                   AdmitProducerCompletion, Selected, Other,
                   ProducerSelectedReadyQueue,
                   ProducerOtherReadyQueue
          <5>2. /\ asyncIoReadyCompletions[node] = Selected
                 /\ asyncLocalReadyCompletions[node] = Other
            BY <4>1
               DEF Selected, Other, ProducerSelectedReadyQueue,
                   ProducerOtherReadyQueue
          <5> QED BY <2>3, <3>2, <5>1, <5>2, Isa
        <4>2. CASE SelectedCompletionSource(node) = "Local"
          <5>1. /\ asyncIoReadyCompletions'[node] = Other
                 /\ asyncLocalReadyCompletions'[node] =
                         Tail(Selected)
            BY <2>1, <2>2, <4>2,
               FunctionalTailUpdateAtKey
               DEF AsyncIoTopologyTypeInvariant,
                   AdmitProducerCompletion, Selected, Other,
                   ProducerSelectedReadyQueue,
                   ProducerOtherReadyQueue
          <5>2. /\ asyncIoReadyCompletions[node] = Other
                 /\ asyncLocalReadyCompletions[node] = Selected
            BY <4>2
               DEF Selected, Other, ProducerSelectedReadyQueue,
                   ProducerOtherReadyQueue
          <5> QED BY <2>3, <3>2, <5>1, <5>2, Isa
        <4> QED BY <2>3, <4>1, <4>2
      <3>6. ASSUME NEW otherNode \in ValidatorIds
             PROVE asyncOutstandingWork'[otherNode] =
                     ConsensusIoCandidates(otherNode)'
                       \cup SequenceSet(
                            asyncIoReadyCompletions'[otherNode])
                       \cup SequenceSet(
                            asyncLocalReadyCompletions'[otherNode])
        <4>1. CASE otherNode = node
          <5>1. asyncOutstandingWork[node] =
                   ConsensusIoCandidates(node)
                     \cup SequenceSet(
                          asyncIoReadyCompletions[node])
                     \cup SequenceSet(
                          asyncLocalReadyCompletions[node])
            BY <1>1
               DEF AsyncProgressOwnershipInvariant,
                   AsyncOutstandingCarrierInvariant
          <5>2. asyncOutstandingWork'[node] =
                   asyncOutstandingWork[node] \ {Candidate}
            BY <2>1, <2>5, Isa
               DEF AsyncIoTopologyTypeInvariant
          <5> QED BY <3>3, <3>4, <3>5, <4>1, <5>1, <5>2, Isa
        <4>2. CASE otherNode # node
          <5>1. CASE SelectedCompletionSource(node) = "Io"
            <6>1. /\ asyncOutstandingWork'[otherNode] =
                            asyncOutstandingWork[otherNode]
                   /\ asyncIoReadyCompletions'[otherNode] =
                            asyncIoReadyCompletions[otherNode]
                   /\ asyncLocalReadyCompletions'[otherNode] =
                            asyncLocalReadyCompletions[otherNode]
              BY <2>1, <2>2, <3>6, <4>2, <5>1,
                 FunctionalUpdateAwayFromKey,
                 FunctionalTailUpdateAwayFromKey
                 DEF AsyncIoTopologyTypeInvariant,
                     AdmitProducerCompletion
            <6> QED BY <1>1, <3>4, <3>6, <6>1
                 DEF AsyncProgressOwnershipInvariant,
                     AsyncOutstandingCarrierInvariant
          <5>2. CASE SelectedCompletionSource(node) = "Local"
            <6>1. /\ asyncOutstandingWork'[otherNode] =
                            asyncOutstandingWork[otherNode]
                   /\ asyncIoReadyCompletions'[otherNode] =
                            asyncIoReadyCompletions[otherNode]
                   /\ asyncLocalReadyCompletions'[otherNode] =
                            asyncLocalReadyCompletions[otherNode]
              BY <2>1, <2>2, <3>6, <4>2, <5>2,
                 FunctionalUpdateAwayFromKey,
                 FunctionalTailUpdateAwayFromKey
                 DEF AsyncIoTopologyTypeInvariant,
                     AdmitProducerCompletion
            <6> QED BY <1>1, <3>4, <3>6, <6>1
                 DEF AsyncProgressOwnershipInvariant,
                     AsyncOutstandingCarrierInvariant
          <5> QED BY <2>3, <5>1, <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>6 DEF AsyncOutstandingCarrierInvariant
    <2>18. /\ UNCHANGED AsyncProgressOwnershipCoreVars
            /\ UNCHANGED AsyncBusyConsumerVars
            /\ ActiveBusyCompletionCarrier' =
                 ActiveBusyCompletionCarrier
      BY <2>2, <2>4, <2>7, Isa
         DEF SelectedLocalAdmissionAdvance, AdmitProducerCompletion,
             AsyncProgressOwnershipCoreVars,
             AsyncBusyConsumerVars,
             ActiveBusyCompletionCarrier, vars
    <2>19. /\ SerializedBusyOwnershipInvariant'
            /\ BusyCompletionWitnessInvariant'
      BY <1>1, <2>18, ProgressCoreFramePreservesBusyOwnership
    <2> QED BY <2>16, <2>17, <2>19
         DEF AsyncProgressOwnershipInvariant
  <1> QED BY <1>1

THEOREM UniqueSequenceTailSetFacts ==
  \A sequence:
    /\ sequence \in Seq(Range(sequence))
    /\ SequenceHasUniqueValues(sequence)
    /\ Len(sequence) > 0
    => /\ SequenceSet(Tail(sequence)) =
              SequenceSet(sequence) \ {Head(sequence)}
       /\ SequenceHasUniqueValues(Tail(sequence))
PROOF
  <1>1. ASSUME NEW sequence,
                sequence \in Seq(Range(sequence)),
                SequenceHasUniqueValues(sequence),
                Len(sequence) > 0
         PROVE /\ SequenceSet(Tail(sequence)) =
                      SequenceSet(sequence) \ {Head(sequence)}
               /\ SequenceHasUniqueValues(Tail(sequence))
    <2>1. Len(sequence) \in Nat
      BY <1>1, LenProperties
    <2>2. sequence # <<>>
      BY <1>1, <2>1, EmptySeq, SMT
    <2>3. IsInjective(sequence)
      BY <1>1, UniqueSequenceLengthImpliesInjective
    <2>4. /\ Tail(sequence) \in Seq(Range(sequence))
           /\ Range(Tail(sequence)) =
                Range(sequence) \ {Head(sequence)}
      BY <1>1, <2>2, <2>3, TailInjectiveSeq
    <2>5. /\ SequenceSet(sequence) = Range(sequence)
           /\ SequenceSet(Tail(sequence)) = Range(Tail(sequence))
      BY <1>1, <2>4, RangeEquality DEF SequenceSet
    <2>6. SequenceHasUniqueValues(Tail(sequence))
      BY <1>1, UniqueReplayTailPreservesUniqueValues
    <2> QED BY <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM ConsensusIoAppendAddsCandidate ==
  \A node \in ValidatorIds:
  \A job, candidate:
    /\ AsyncIoTopologyTypeInvariant
    /\ AsyncIoSequenceTyped(asyncIoQueues[node])
    /\ job.class = "Consensus"
    /\ job.candidate = candidate
    /\ asyncIoQueues' =
         [asyncIoQueues EXCEPT ![node] = Append(@, job)]
    => /\ ConsensusIoCandidates(node)' =
              ConsensusIoCandidates(node) \cup {candidate}
       /\ \A other \in ValidatorIds \ {node}:
            ConsensusIoCandidates(other)' =
              ConsensusIoCandidates(other)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, NEW job, NEW candidate,
                AsyncIoTopologyTypeInvariant,
                AsyncIoSequenceTyped(asyncIoQueues[node]),
                job.class = "Consensus",
                job.candidate = candidate,
                asyncIoQueues' =
                  [asyncIoQueues EXCEPT ![node] = Append(@, job)]
         PROVE /\ ConsensusIoCandidates(node)' =
                      ConsensusIoCandidates(node) \cup {candidate}
               /\ \A other \in ValidatorIds \ {node}:
                    ConsensusIoCandidates(other)' =
                      ConsensusIoCandidates(other)
    <2>1. asyncIoQueues[node] \in Seq(Range(asyncIoQueues[node]))
      BY <1>1 DEF AsyncIoSequenceTyped
    <2>2. SequenceSet(Append(asyncIoQueues[node], job)) =
             SequenceSet(asyncIoQueues[node]) \cup {job}
      BY <2>1, SequenceSetAfterAppend
    <2>3. ConsensusIoCandidates(node)' =
             ConsensusIoCandidates(node) \cup {candidate}
      BY <1>1, <2>2, FunctionalAppendUpdateAtKey, Isa
         DEF AsyncIoTopologyTypeInvariant, ConsensusIoCandidates
    <2>4. \A other \in ValidatorIds \ {node}:
             ConsensusIoCandidates(other)' =
               ConsensusIoCandidates(other)
      <3>1. ASSUME NEW other \in ValidatorIds \ {node}
             PROVE ConsensusIoCandidates(other)' =
                     ConsensusIoCandidates(other)
        <4>1. asyncIoQueues'[other] = asyncIoQueues[other]
          BY <1>1, <3>1, FunctionalAppendUpdateAwayFromKey
             DEF AsyncIoTopologyTypeInvariant
        <4> QED BY <4>1 DEF ConsensusIoCandidates
      <3> QED BY <3>1
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM CausalAdmissionPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ SelectedLocalAdmissionAdvance(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Causal"
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                SelectedLocalAdmissionAdvance(node),
                LocalAdmissionCanAdvance(node),
                SelectedLocalSource(node) = "Causal"
         PROVE AsyncProgressOwnershipInvariant'
    <2> DEFINE Candidate == HeadCausalCandidate(node)
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
      BY <1>1
         DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant
    <2>2. /\ CausalHeadCanAdvance(node)
           /\ CausalQueueNonempty(node)
           /\ AdmitCausalHead(node)
      <3>1. CausalHeadCanAdvance(node)
        BY <1>1, SelectedCausalCanAdvance
      <3>2. CausalQueueNonempty(node)
        BY <3>1 DEF CausalHeadCanAdvance
      <3>3. AdmitCausalHead(node)
        BY <1>1, <3>1, Isa DEF SelectedLocalAdmissionAdvance
      <3> QED BY <3>1, <3>2, <3>3
    <2>3. /\ AsyncCandidateTyped(Candidate)
           /\ Candidate.node = node
           /\ Candidate \in SequenceSet(asyncCausalQueues[node])
           /\ Candidate \in CausalCandidates
           /\ Candidate \notin QueuedCandidates
           /\ Candidate \notin DeferredCandidates
           /\ Candidate \notin TrackedWorkCandidates
           /\ ~CandidateInFlight(Candidate)
      <3>1. /\ AsyncCandidateTyped(Candidate)
             /\ Candidate.node = node
        BY <2>1, <2>2, CausalHeadCandidateIsTyped,
           CausalHeadCandidateIsOwned DEF Candidate
      <3>2. Candidate \in SequenceSet(asyncCausalQueues[node])
        <4>1. /\ asyncCausalQueues[node] \in
                       Seq(Range(asyncCausalQueues[node]))
               /\ Len(asyncCausalQueues[node]) > 0
          BY <2>1, <2>2
             DEF AsyncCausalTypeInvariant, AsyncQueueTyped,
                 CausalQueueNonempty
        <4>2. asyncCausalQueues[node] # <<>>
          BY <4>1, EmptySeq, SMT
        <4>3. Head(asyncCausalQueues[node]) \in
                 Range(asyncCausalQueues[node])
          BY <4>1, <4>2, HeadTailProperties
        <4>4. SequenceSet(asyncCausalQueues[node]) =
                 Range(asyncCausalQueues[node])
          BY <4>1, RangeEquality DEF SequenceSet
        <4> QED BY <4>3, <4>4
             DEF Candidate, HeadCausalCandidate
      <3>3. Candidate \in CausalCandidates
        BY <3>2 DEF CausalCandidates
      <3>4. /\ Candidate \notin QueuedCandidates
             /\ Candidate \notin DeferredCandidates
             /\ Candidate \notin TrackedWorkCandidates
        BY <1>1, <3>3, Isa
           DEF AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant
      <3>5. ~CandidateInFlight(Candidate)
        BY <3>4 DEF CandidateInFlight
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
    <2>4. /\ SequenceSet(Tail(asyncCausalQueues[node])) =
                  SequenceSet(asyncCausalQueues[node]) \ {Candidate}
           /\ SequenceHasUniqueValues(Tail(asyncCausalQueues[node]))
      <3>1. /\ asyncCausalQueues[node] \in
                       Seq(Range(asyncCausalQueues[node]))
             /\ SequenceHasUniqueValues(asyncCausalQueues[node])
             /\ Len(asyncCausalQueues[node]) > 0
        BY <1>1, <2>1, <2>2
           DEF AsyncCausalTypeInvariant,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncQueueTyped, CausalQueueNonempty
      <3>2. /\ SequenceSet(Tail(asyncCausalQueues[node])) =
                    SequenceSet(asyncCausalQueues[node])
                      \ {Head(asyncCausalQueues[node])}
             /\ SequenceHasUniqueValues(
                  Tail(asyncCausalQueues[node]))
        BY <3>1, UniqueSequenceTailSetFacts
      <3> QED BY <3>2 DEF Candidate, HeadCausalCandidate
    <2>5. /\ asyncCausalQueues' =
                [asyncCausalQueues EXCEPT ![node] = Tail(@)]
           /\ UNCHANGED
                <<AsyncProgressOwnershipCoreVars, AsyncBusyConsumerVars,
                  asyncDeferredCompletionQueues,
                  asyncDeferredProgressQueues,
                  asyncDeferredNormalQueues>>
      <3>1. asyncCausalQueues' =
               [asyncCausalQueues EXCEPT ![node] = Tail(@)]
        BY <2>2 DEF AdmitCausalHead
      <3>2. /\ UNCHANGED AsyncProgressOwnershipCoreVars
             /\ UNCHANGED AsyncBusyConsumerVars
        BY <2>2, Isa
           DEF AdmitCausalHead, AsyncProgressOwnershipCoreVars,
               AsyncBusyConsumerVars, vars
      <3>3. UNCHANGED
               <<asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues,
                 asyncDeferredNormalQueues>>
        BY <1>1 DEF SelectedLocalAdmissionAdvance, AsyncDeferredVars
      <3> QED BY <3>1, <3>2, <3>3
    <2>6. /\ CausalCandidates' = CausalCandidates \ {Candidate}
           /\ \A other \in ValidatorIds:
                SequenceHasUniqueValues(asyncCausalQueues'[other])
      <3>1. CausalCandidates' = CausalCandidates \ {Candidate}
        <4>1. /\ DOMAIN asyncCausalQueues = ValidatorIds
               /\ Candidate.node = node
               /\ \A owner \in ValidatorIds:
                    \A candidate \in
                         SequenceSet(asyncCausalQueues[owner]):
                      candidate.node = owner
          BY <2>1, <2>3
             DEF AsyncCausalTypeInvariant,
                 AsyncCausalQueueOwnership
        <4>2. UNION
                 {SequenceSet(asyncCausalQueues'[owner]):
                    owner \in ValidatorIds} =
                 (UNION
                    {SequenceSet(asyncCausalQueues[owner]):
                       owner \in ValidatorIds})
                   \ {Candidate}
          BY <1>1, <2>4, <2>5, <4>1,
             UnionOfSequenceSetsAfterTailAtKey
        <4> QED BY <4>2 DEF CausalCandidates
      <3>2. \A other \in ValidatorIds:
               SequenceHasUniqueValues(asyncCausalQueues'[other])
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE SequenceHasUniqueValues(asyncCausalQueues'[other])
          <5>1. CASE other = node
            BY <2>1, <2>4, <2>5, <5>1,
               FunctionalTailUpdateAtKey
               DEF AsyncCausalTypeInvariant
          <5>2. CASE other # node
            BY <1>1, <2>1, <2>5, <4>1, <5>2,
               FunctionalTailUpdateAwayFromKey
               DEF AsyncCausalTypeInvariant,
                   AsyncProgressOwnershipInvariant,
                   AsyncLogicalCandidateOwnershipInvariant
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>1, <3>2
    <2>7. CASE Candidate.class = "Completion"
      <3> DEFINE Job == AsyncIoConsensusJob(Candidate)
      <3>1. /\ asyncCommandQueues' = asyncCommandQueues
             /\ asyncIoQueues' =
                  [asyncIoQueues EXCEPT ![node] = Append(@, Job)]
             /\ asyncOutstandingWork' =
                  [asyncOutstandingWork EXCEPT
                     ![node] = @ \cup {Candidate}]
             /\ UNCHANGED
                  <<asyncIoReadyCompletions,
                    asyncLocalReadyCompletions>>
        <4>1. asyncCommandQueues' = asyncCommandQueues
          BY <2>2, <2>3, <2>7, Isa
             DEF AdmitCausalHead, Candidate
        <4>2. asyncIoQueues' =
                 [asyncIoQueues EXCEPT ![node] = Append(@, Job)]
          BY <2>2, <2>3, <2>7, Isa
             DEF AdmitCausalHead, Candidate, Job
        <4>3. asyncOutstandingWork' =
                 [asyncOutstandingWork EXCEPT
                    ![node] = @ \cup {Candidate}]
          BY <2>2, <2>3, <2>7, Isa
             DEF AdmitCausalHead, Candidate
        <4>4. UNCHANGED
                 <<asyncIoReadyCompletions,
                   asyncLocalReadyCompletions>>
          BY <2>2, <2>3, <2>7, Isa
             DEF AdmitCausalHead, Candidate
        <4> QED BY <4>1, <4>2, <4>3, <4>4
      <3>2. /\ QueuedCandidates' = QueuedCandidates
             /\ DeferredCandidates' = DeferredCandidates
             /\ TrackedWorkCandidates' =
                  TrackedWorkCandidates \cup {Candidate}
        <4>1. QueuedCandidates' = QueuedCandidates
          BY <3>1 DEF QueuedCandidates
        <4>2. DeferredCandidates' = DeferredCandidates
          BY <2>5 DEF DeferredCandidates
        <4>3. TrackedWorkCandidates' =
                 TrackedWorkCandidates \cup {Candidate}
          <5>1. DOMAIN asyncOutstandingWork = ValidatorIds
            BY <2>1 DEF AsyncIoTopologyTypeInvariant
          <5>2. UNION
                   {asyncOutstandingWork'[owner]:
                      owner \in ValidatorIds} =
                   (UNION
                      {asyncOutstandingWork[owner]:
                         owner \in ValidatorIds})
                     \cup {Candidate}
            BY <1>1, <3>1, <5>1, UnionOfSetsAfterAddAtKey
          <5> QED BY <5>2 DEF TrackedWorkCandidates
        <4> QED BY <4>1, <4>2, <4>3
      <3>3. AsyncLogicalCandidateOwnershipInvariant'
        <4>1. \A other \in ValidatorIds:
                 SequenceHasUniqueValues(
                   asyncCommandQueues'[other])
          BY <1>1, <3>1
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>2. \A other \in ValidatorIds:
                 SequenceHasUniqueValues(asyncCausalQueues'[other])
          BY <2>6
        <4>3. \A other \in ValidatorIds:
                 /\ SequenceHasUniqueValues(
                      asyncDeferredCompletionQueues'[other])
                 /\ SequenceHasUniqueValues(
                      asyncDeferredProgressQueues'[other])
                 /\ SequenceHasUniqueValues(
                      asyncDeferredNormalQueues'[other])
          BY <1>1, <2>5
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>4. QueuedCandidates' \cap DeferredCandidates' = {}
          BY <1>1, <3>2
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>5. QueuedCandidates' \cap CausalCandidates' = {}
          BY <1>1, <2>6, <3>2, Isa
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>6. QueuedCandidates' \cap TrackedWorkCandidates' = {}
          BY <1>1, <2>3, <3>2, Isa
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>7. DeferredCandidates' \cap CausalCandidates' = {}
          BY <1>1, <2>6, <3>2, Isa
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>8. DeferredCandidates' \cap TrackedWorkCandidates' = {}
          BY <1>1, <2>3, <3>2, Isa
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>9. CausalCandidates' \cap TrackedWorkCandidates' = {}
          BY <1>1, <2>3, <2>6, <3>2, Isa
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4> QED BY <4>1, <4>2, <4>3, <4>4, <4>5, <4>6,
           <4>7, <4>8, <4>9
           DEF AsyncLogicalCandidateOwnershipInvariant
      <3>4. /\ ConsensusIoCandidates(node)' =
                    ConsensusIoCandidates(node) \cup {Candidate}
             /\ \A other \in ValidatorIds \ {node}:
                  ConsensusIoCandidates(other)' =
                    ConsensusIoCandidates(other)
        <4>1. AsyncIoSequenceTyped(asyncIoQueues[node])
          BY <2>1 DEF AsyncIoQueueContentTypeInvariant
        <4>2. /\ Job.class = "Consensus"
               /\ Job.candidate = Candidate
          BY DEF Job, AsyncIoConsensusJob, AsyncIoJob
        <4> QED BY <1>1, <2>1, <3>1, <4>1, <4>2,
             ConsensusIoAppendAddsCandidate
      <3>5. AsyncOutstandingCarrierInvariant'
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE asyncOutstandingWork'[other] =
                       ConsensusIoCandidates(other)'
                         \cup SequenceSet(
                              asyncIoReadyCompletions'[other])
                         \cup SequenceSet(
                              asyncLocalReadyCompletions'[other])
          <5>1. CASE other = node
            <6>1. asyncOutstandingWork[node] =
                     ConsensusIoCandidates(node)
                       \cup SequenceSet(
                            asyncIoReadyCompletions[node])
                       \cup SequenceSet(
                            asyncLocalReadyCompletions[node])
              BY <1>1
                 DEF AsyncProgressOwnershipInvariant,
                     AsyncOutstandingCarrierInvariant
            <6>2. asyncOutstandingWork'[node] =
                     asyncOutstandingWork[node] \cup {Candidate}
              BY <2>1, <3>1, Isa
                 DEF AsyncIoTopologyTypeInvariant
            <6>3. /\ asyncIoReadyCompletions'[node] =
                            asyncIoReadyCompletions[node]
                   /\ asyncLocalReadyCompletions'[node] =
                            asyncLocalReadyCompletions[node]
              BY <3>1
            <6> QED BY <3>4, <5>1, <6>1, <6>2, <6>3, Isa
          <5>2. CASE other # node
            <6>1. other \in ValidatorIds \ {node}
              BY <4>1, <5>2
            <6>2. /\ asyncOutstandingWork'[other] =
                            asyncOutstandingWork[other]
                   /\ asyncIoReadyCompletions'[other] =
                            asyncIoReadyCompletions[other]
                   /\ asyncLocalReadyCompletions'[other] =
                            asyncLocalReadyCompletions[other]
              BY <2>1, <3>1, <4>1, <5>2,
                 FunctionalUpdateAwayFromKey
                 DEF AsyncIoTopologyTypeInvariant
            <6>3. ConsensusIoCandidates(other)' =
                     ConsensusIoCandidates(other)
              BY <3>4, <6>1
            <6>4. asyncOutstandingWork[other] =
                     ConsensusIoCandidates(other)
                       \cup SequenceSet(
                            asyncIoReadyCompletions[other])
                       \cup SequenceSet(
                            asyncLocalReadyCompletions[other])
              BY <1>1, <4>1
                 DEF AsyncProgressOwnershipInvariant,
                     AsyncOutstandingCarrierInvariant
            <6> QED BY <6>2, <6>3, <6>4
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1 DEF AsyncOutstandingCarrierInvariant
      <3>6. ActiveBusyCompletionCarrier' =
               ActiveBusyCompletionCarrier
        BY <2>3, <2>6, <3>2, Isa
           DEF ActiveBusyCompletionCarrier
      <3>7. /\ SerializedBusyOwnershipInvariant'
             /\ BusyCompletionWitnessInvariant'
        BY <1>1, <2>5, <3>6,
           ProgressCoreFramePreservesBusyOwnership
      <3> QED BY <3>3, <3>5, <3>7
           DEF AsyncProgressOwnershipInvariant
    <2>8. CASE Candidate.class # "Completion"
      <3>1. /\ asyncCommandQueues' =
                  [asyncCommandQueues EXCEPT
                     ![node] = Append(@, Candidate)]
             /\ UNCHANGED AsyncIoVars
        <4>1. asyncCommandQueues' =
                 [asyncCommandQueues EXCEPT
                    ![node] = Append(@, Candidate)]
          BY <2>2, <2>3, <2>8, Isa
             DEF AdmitCausalHead, Candidate, EnqueueCandidate
        <4>2. UNCHANGED AsyncIoVars
          BY <2>2, <2>3, <2>8, Isa
             DEF AdmitCausalHead, Candidate
        <4> QED BY <4>1, <4>2
      <3>2. /\ QueuedCandidates' = QueuedCandidates \cup {Candidate}
             /\ DeferredCandidates' = DeferredCandidates
             /\ TrackedWorkCandidates' = TrackedWorkCandidates
        <4>1. QueuedCandidates' =
                 QueuedCandidates \cup {Candidate}
          <5>1. /\ DOMAIN asyncCommandQueues = ValidatorIds
                 /\ asyncCommandQueues[node] \in
                      Seq(Range(asyncCommandQueues[node]))
            BY <2>1
               DEF AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped
          <5>2. UNION
                   {SequenceSet(asyncCommandQueues'[owner]):
                      owner \in ValidatorIds} =
                   (UNION
                      {SequenceSet(asyncCommandQueues[owner]):
                         owner \in ValidatorIds})
                     \cup {Candidate}
            BY <1>1, <3>1, <5>1,
               UnionOfSequenceSetsAfterAppendAtKey
          <5> QED BY <5>2 DEF QueuedCandidates
        <4>2. DeferredCandidates' = DeferredCandidates
          BY <2>5 DEF DeferredCandidates
        <4>3. TrackedWorkCandidates' = TrackedWorkCandidates
          BY <3>1 DEF AsyncIoVars, TrackedWorkCandidates
        <4> QED BY <4>1, <4>2, <4>3
      <3>3. \A other \in ValidatorIds:
               SequenceHasUniqueValues(asyncCommandQueues'[other])
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE SequenceHasUniqueValues(
                       asyncCommandQueues'[other])
          <5>1. CASE other = node
            <6>1. /\ asyncCommandQueues[node] \in
                           Seq(Range(asyncCommandQueues[node]))
                   /\ SequenceHasUniqueValues(
                        asyncCommandQueues[node])
                   /\ Candidate \notin
                        SequenceSet(asyncCommandQueues[node])
              BY <1>1, <2>1, <2>3
                 DEF AsyncRuntimeScalarTypeInvariant,
                     AsyncProgressOwnershipInvariant,
                     AsyncLogicalCandidateOwnershipInvariant,
                     QueuedCandidates, AsyncQueueTyped
            <6>2. SequenceHasUniqueValues(
                     Append(asyncCommandQueues[node], Candidate))
              BY <6>1, FreshAppendPreservesUniqueSequence
            <6>3. asyncCommandQueues'[other] =
                     Append(asyncCommandQueues[node], Candidate)
              BY <2>1, <3>1, <5>1,
                 FunctionalAppendUpdateAtKey
                 DEF AsyncRuntimeScalarTypeInvariant
            <6> QED BY <6>2, <6>3
          <5>2. CASE other # node
            BY <1>1, <2>1, <3>1, <4>1, <5>2,
               FunctionalAppendUpdateAwayFromKey
               DEF AsyncRuntimeScalarTypeInvariant,
                   AsyncProgressOwnershipInvariant,
                   AsyncLogicalCandidateOwnershipInvariant
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3>4. AsyncLogicalCandidateOwnershipInvariant'
        <4>1. \A other \in ValidatorIds:
                 SequenceHasUniqueValues(
                   asyncCommandQueues'[other])
          BY <3>3
        <4>2. \A other \in ValidatorIds:
                 SequenceHasUniqueValues(asyncCausalQueues'[other])
          BY <2>6
        <4>3. \A other \in ValidatorIds:
                 /\ SequenceHasUniqueValues(
                      asyncDeferredCompletionQueues'[other])
                 /\ SequenceHasUniqueValues(
                      asyncDeferredProgressQueues'[other])
                 /\ SequenceHasUniqueValues(
                      asyncDeferredNormalQueues'[other])
          BY <1>1, <2>5
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>4. QueuedCandidates' \cap DeferredCandidates' = {}
          BY <1>1, <2>3, <3>2, Isa
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>5. QueuedCandidates' \cap CausalCandidates' = {}
          BY <1>1, <2>6, <3>2, Isa
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>6. QueuedCandidates' \cap TrackedWorkCandidates' = {}
          BY <1>1, <2>3, <3>2, Isa
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>7. DeferredCandidates' \cap CausalCandidates' = {}
          BY <1>1, <2>6, <3>2, Isa
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>8. DeferredCandidates' \cap TrackedWorkCandidates' = {}
          BY <1>1, <3>2
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4>9. CausalCandidates' \cap TrackedWorkCandidates' = {}
          BY <1>1, <2>6, <3>2, Isa
             DEF AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant
        <4> QED BY <4>1, <4>2, <4>3, <4>4, <4>5, <4>6,
           <4>7, <4>8, <4>9
           DEF AsyncLogicalCandidateOwnershipInvariant
      <3>5. AsyncOutstandingCarrierInvariant'
        <4>1. /\ asyncIoQueues' = asyncIoQueues
               /\ asyncOutstandingWork' = asyncOutstandingWork
               /\ asyncIoReadyCompletions' =
                    asyncIoReadyCompletions
               /\ asyncLocalReadyCompletions' =
                    asyncLocalReadyCompletions
          BY <3>1 DEF AsyncIoVars
        <4>2. \A other \in ValidatorIds:
                 ConsensusIoCandidates(other)' =
                   ConsensusIoCandidates(other)
          BY <4>1 DEF ConsensusIoCandidates
        <4>3. ASSUME NEW other \in ValidatorIds
               PROVE asyncOutstandingWork'[other] =
                       ConsensusIoCandidates(other)'
                         \cup SequenceSet(
                              asyncIoReadyCompletions'[other])
                         \cup SequenceSet(
                              asyncLocalReadyCompletions'[other])
          <5>1. asyncOutstandingWork[other] =
                   ConsensusIoCandidates(other)
                     \cup SequenceSet(
                          asyncIoReadyCompletions[other])
                     \cup SequenceSet(
                          asyncLocalReadyCompletions[other])
            BY <1>1, <4>3
               DEF AsyncProgressOwnershipInvariant,
                   AsyncOutstandingCarrierInvariant
          <5> QED BY <4>1, <4>2, <4>3, <5>1
        <4> QED BY <4>3 DEF AsyncOutstandingCarrierInvariant
      <3>6. ActiveBusyCompletionCarrier' =
               ActiveBusyCompletionCarrier
        BY <2>3, <2>6, <3>2, Isa
           DEF ActiveBusyCompletionCarrier
      <3>7. /\ SerializedBusyOwnershipInvariant'
             /\ BusyCompletionWitnessInvariant'
        BY <1>1, <2>5, <3>6,
           ProgressCoreFramePreservesBusyOwnership
      <3> QED BY <3>4, <3>5, <3>7
           DEF AsyncProgressOwnershipInvariant
    <2> QED BY <2>7, <2>8
  <1> QED BY <1>1

THEOREM LocalAdmissionPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ LocalAdmissionStep(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                LocalAdmissionStep(node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. CASE ~LocalAdmissionCanAdvance(node)
      BY <1>1, <2>1,
         LocalPhaseAdvancePreservesProgressOwnership
    <2>2. CASE LocalAdmissionCanAdvance(node)
               /\ SelectedLocalSource(node) = "Producer"
      <3>1. SelectedLocalAdmissionAdvance(node)
        BY <1>1, <2>2, LocalAdmissionAdvanceSelectsAtomicWork
      <3> QED BY <1>1, <2>2, <3>1,
           ProducerAdmissionPreservesProgressOwnership
    <2>3. CASE LocalAdmissionCanAdvance(node)
               /\ SelectedLocalSource(node) = "Causal"
      <3>1. SelectedLocalAdmissionAdvance(node)
        BY <1>1, <2>3, LocalAdmissionAdvanceSelectsAtomicWork
      <3> QED BY <1>1, <2>3, <3>1,
           CausalAdmissionPreservesProgressOwnership
    <2> QED BY <2>1, <2>2, <2>3, Isa
         DEF SelectedLocalSource, PreferredLocalSource,
             OtherLocalSource, AsyncLocalSources
  <1> QED BY <1>1

THEOREM SerializedLocalPredecessorPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ SerializedLocalPrecedesServeIngressStep(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                SerializedLocalPrecedesServeIngressStep(node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. /\ SelectedLocalAdmissionAdvance(node)
           /\ LocalAdmissionCanAdvance(node)
      BY <1>1
         DEF SerializedLocalPrecedesServeIngressStep,
             SelectedLocalAdmissionAdvance,
             AsyncOlderLocalLifecyclePrecedesServeIngress
    <2>2. CASE SelectedLocalSource(node) = "Producer"
      BY <1>1, <2>1, <2>2,
         ProducerAdmissionPreservesProgressOwnership
    <2>3. CASE SelectedLocalSource(node) = "Causal"
      BY <1>1, <2>1, <2>3,
         CausalAdmissionPreservesProgressOwnership
    <2> QED BY <1>1, <2>2, <2>3, SMT
         DEF SelectedLocalSource, PreferredLocalSource,
             OtherLocalSource, AsyncLocalSources
  <1> QED BY <1>1

THEOREM IngressPhaseAdvancePreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncProgressOwnershipInvariant
    /\ IngressDrainStep(node)
    /\ ~(asyncRunnerBudget[node] > 0
           /\ asyncIngressReady[node] # <<>>
           /\ DrainableIngressIndices(node) # {})
    => AsyncProgressOwnershipInvariant'
BY AsyncProgressOwnershipStutter, Isa
   DEF IngressDrainStep, LeaveCausalQueues, AsyncIoVars,
       AsyncDeferredVars, AsyncLocalAdmissionVars,
       AsyncProgressOwnershipVars, AsyncProgressOwnershipCoreVars,
       AsyncProgressOwnershipSchedulerVars, vars

THEOREM FreshTrackedAddPreservesLogicalOwnership ==
  \A node \in ValidatorIds:
  \A candidate:
    /\ AsyncIoTopologyTypeInvariant
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ candidate.node = node
    /\ ~CandidateScheduled(candidate)
    /\ asyncOutstandingWork' =
         [asyncOutstandingWork EXCEPT ![node] = @ \cup {candidate}]
    /\ UNCHANGED
         <<asyncCommandQueues, asyncDeferredCompletionQueues,
           asyncDeferredProgressQueues, asyncDeferredNormalQueues,
           asyncCausalQueues>>
    => /\ AsyncLogicalCandidateOwnershipInvariant'
       /\ TrackedWorkCandidates' =
            TrackedWorkCandidates \cup {candidate}
       /\ ActiveBusyCompletionCarrier \subseteq
            ActiveBusyCompletionCarrier'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, NEW candidate,
                AsyncIoTopologyTypeInvariant,
                AsyncLogicalCandidateOwnershipInvariant,
                candidate.node = node,
                ~CandidateScheduled(candidate),
                asyncOutstandingWork' =
                  [asyncOutstandingWork EXCEPT
                     ![node] = @ \cup {candidate}],
                UNCHANGED
                  <<asyncCommandQueues,
                    asyncDeferredCompletionQueues,
                    asyncDeferredProgressQueues,
                    asyncDeferredNormalQueues, asyncCausalQueues>>
         PROVE /\ AsyncLogicalCandidateOwnershipInvariant'
               /\ TrackedWorkCandidates' =
                    TrackedWorkCandidates \cup {candidate}
               /\ ActiveBusyCompletionCarrier \subseteq
                    ActiveBusyCompletionCarrier'
    <2>1. TrackedWorkCandidates' =
             TrackedWorkCandidates \cup {candidate}
      <3>1. DOMAIN asyncOutstandingWork = ValidatorIds
        BY <1>1 DEF AsyncIoTopologyTypeInvariant
      <3>2. UNION
               {asyncOutstandingWork'[owner]:
                  owner \in ValidatorIds} =
               (UNION
                  {asyncOutstandingWork[owner]:
                     owner \in ValidatorIds})
                 \cup {candidate}
        BY <1>1, <3>1, UnionOfSetsAfterAddAtKey
      <3> QED BY <3>2 DEF TrackedWorkCandidates
    <2>2. /\ QueuedCandidates' = QueuedCandidates
           /\ DeferredCandidates' = DeferredCandidates
           /\ CausalCandidates' = CausalCandidates
      <3>1. QueuedCandidates' = QueuedCandidates
        BY <1>1 DEF QueuedCandidates
      <3>2. DeferredCandidates' = DeferredCandidates
        BY <1>1 DEF DeferredCandidates
      <3>3. CausalCandidates' = CausalCandidates
        BY <1>1 DEF CausalCandidates
      <3> QED BY <3>1, <3>2, <3>3
    <2>3. AsyncLogicalCandidateOwnershipInvariant'
      <3>1. \A owner \in ValidatorIds:
               /\ SequenceHasUniqueValues(
                    asyncCommandQueues'[owner])
               /\ SequenceHasUniqueValues(
                    asyncCausalQueues'[owner])
               /\ SequenceHasUniqueValues(
                    asyncDeferredCompletionQueues'[owner])
               /\ SequenceHasUniqueValues(
                    asyncDeferredProgressQueues'[owner])
               /\ SequenceHasUniqueValues(
                    asyncDeferredNormalQueues'[owner])
        BY <1>1 DEF AsyncLogicalCandidateOwnershipInvariant
      <3>2. QueuedCandidates' \cap DeferredCandidates' = {}
        BY <1>1, <2>2
           DEF AsyncLogicalCandidateOwnershipInvariant
      <3>3. QueuedCandidates' \cap CausalCandidates' = {}
        BY <1>1, <2>2
           DEF AsyncLogicalCandidateOwnershipInvariant
      <3>4. QueuedCandidates' \cap TrackedWorkCandidates' = {}
        BY <1>1, <2>1, <2>2, Isa
           DEF AsyncLogicalCandidateOwnershipInvariant,
               CandidateScheduled
      <3>5. DeferredCandidates' \cap CausalCandidates' = {}
        BY <1>1, <2>2
           DEF AsyncLogicalCandidateOwnershipInvariant
      <3>6. DeferredCandidates' \cap TrackedWorkCandidates' = {}
        BY <1>1, <2>1, <2>2, Isa
           DEF AsyncLogicalCandidateOwnershipInvariant,
               CandidateScheduled
      <3>7. CausalCandidates' \cap TrackedWorkCandidates' = {}
        BY <1>1, <2>1, <2>2, Isa
           DEF AsyncLogicalCandidateOwnershipInvariant,
               CandidateScheduled
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5, <3>6, <3>7
           DEF AsyncLogicalCandidateOwnershipInvariant
    <2>4. ActiveBusyCompletionCarrier \subseteq
             ActiveBusyCompletionCarrier'
      BY <2>1, <2>2, Isa DEF ActiveBusyCompletionCarrier
    <2> QED BY <2>1, <2>3, <2>4
  <1> QED BY <1>1

THEOREM AsyncOutstandingCarrierStutter ==
  /\ AsyncOutstandingCarrierInvariant
  /\ UNCHANGED
       <<asyncIoQueues, asyncOutstandingWork,
         asyncIoReadyCompletions, asyncLocalReadyCompletions>>
  => AsyncOutstandingCarrierInvariant'
PROOF
  <1>1. ASSUME AsyncOutstandingCarrierInvariant,
                UNCHANGED
                  <<asyncIoQueues, asyncOutstandingWork,
                    asyncIoReadyCompletions,
                    asyncLocalReadyCompletions>>
         PROVE AsyncOutstandingCarrierInvariant'
    <2>1. /\ asyncIoQueues' = asyncIoQueues
           /\ asyncOutstandingWork' = asyncOutstandingWork
           /\ asyncIoReadyCompletions' =
                asyncIoReadyCompletions
           /\ asyncLocalReadyCompletions' =
                asyncLocalReadyCompletions
      BY <1>1
    <2>2. \A node \in ValidatorIds:
             ConsensusIoCandidates(node)' =
               ConsensusIoCandidates(node)
      BY <2>1 DEF ConsensusIoCandidates
    <2>3. ASSUME NEW node \in ValidatorIds
           PROVE asyncOutstandingWork'[node] =
                   ConsensusIoCandidates(node)'
                     \cup SequenceSet(
                          asyncIoReadyCompletions'[node])
                     \cup SequenceSet(
                          asyncLocalReadyCompletions'[node])
      <3>1. asyncOutstandingWork[node] =
               ConsensusIoCandidates(node)
                 \cup SequenceSet(asyncIoReadyCompletions[node])
                 \cup SequenceSet(
                      asyncLocalReadyCompletions[node])
        BY <1>1, <2>3 DEF AsyncOutstandingCarrierInvariant
      <3> QED BY <2>1, <2>2, <2>3, <3>1
    <2> QED BY <2>3 DEF AsyncOutstandingCarrierInvariant
  <1> QED BY <1>1

THEOREM FreshLocalReadyTrackedAddPreservesOutstandingCarrier ==
  \A node \in ValidatorIds:
  \A candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncOutstandingCarrierInvariant
    /\ candidate.node = node
    /\ ~CandidateScheduled(candidate)
    /\ asyncOutstandingWork' =
         [asyncOutstandingWork EXCEPT ![node] = @ \cup {candidate}]
    /\ asyncLocalReadyCompletions' =
         [asyncLocalReadyCompletions EXCEPT
            ![node] = Append(@, candidate)]
    /\ UNCHANGED <<asyncIoQueues, asyncIoReadyCompletions>>
    => AsyncOutstandingCarrierInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, NEW candidate,
                AsyncStrongTypeInvariant,
                AsyncOutstandingCarrierInvariant,
                candidate.node = node,
                ~CandidateScheduled(candidate),
                asyncOutstandingWork' =
                  [asyncOutstandingWork EXCEPT
                     ![node] = @ \cup {candidate}],
                asyncLocalReadyCompletions' =
                  [asyncLocalReadyCompletions EXCEPT
                     ![node] = Append(@, candidate)],
                UNCHANGED <<asyncIoQueues,
                            asyncIoReadyCompletions>>
         PROVE AsyncOutstandingCarrierInvariant'
    <2>1. /\ AsyncIoTopologyTypeInvariant
           /\ AsyncCompletionSequenceTyped(
                asyncLocalReadyCompletions[node])
           /\ candidate \notin TrackedWorkCandidates
      BY <1>1
         DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoWorkContentTypeInvariant, CandidateScheduled
    <2>2. asyncLocalReadyCompletions[node] \in
             Seq(Range(asyncLocalReadyCompletions[node]))
      BY <2>1 DEF AsyncCompletionSequenceTyped
    <2>3. SequenceSet(
             Append(asyncLocalReadyCompletions[node], candidate)) =
               SequenceSet(asyncLocalReadyCompletions[node])
                 \cup {candidate}
      BY <2>2, SequenceSetAfterAppend
    <2>4. ASSUME NEW other \in ValidatorIds
           PROVE asyncOutstandingWork'[other] =
                   ConsensusIoCandidates(other)'
                     \cup SequenceSet(asyncIoReadyCompletions'[other])
                     \cup SequenceSet(
                          asyncLocalReadyCompletions'[other])
      <3>1. CASE other = node
        <4>1. asyncOutstandingWork'[other] =
                 asyncOutstandingWork[node] \cup {candidate}
          BY <1>1, <2>1, <3>1, Isa
             DEF AsyncIoTopologyTypeInvariant
        <4>2. asyncLocalReadyCompletions'[other] =
                 Append(asyncLocalReadyCompletions[node], candidate)
          BY <1>1, <2>1, <3>1,
             FunctionalAppendUpdateAtKey
             DEF AsyncIoTopologyTypeInvariant
        <4>3. /\ ConsensusIoCandidates(other)' =
                       ConsensusIoCandidates(node)
               /\ asyncIoReadyCompletions'[other] =
                       asyncIoReadyCompletions[node]
          BY <1>1, <3>1 DEF ConsensusIoCandidates
        <4>4. asyncOutstandingWork[node] =
                 ConsensusIoCandidates(node)
                   \cup SequenceSet(asyncIoReadyCompletions[node])
                   \cup SequenceSet(
                        asyncLocalReadyCompletions[node])
          BY <1>1 DEF AsyncOutstandingCarrierInvariant
        <4> QED BY <2>3, <3>1, <4>1, <4>2, <4>3, <4>4, Isa
      <3>2. CASE other # node
        <4>1. /\ asyncOutstandingWork'[other] =
                        asyncOutstandingWork[other]
               /\ asyncLocalReadyCompletions'[other] =
                        asyncLocalReadyCompletions[other]
          BY <1>1, <2>1, <2>4, <3>2,
             FunctionalUpdateAwayFromKey,
             FunctionalAppendUpdateAwayFromKey
             DEF AsyncIoTopologyTypeInvariant
        <4>2. /\ ConsensusIoCandidates(other)' =
                        ConsensusIoCandidates(other)
               /\ asyncIoReadyCompletions'[other] =
                        asyncIoReadyCompletions[other]
          BY <1>1 DEF ConsensusIoCandidates
        <4>3. asyncOutstandingWork[other] =
                 ConsensusIoCandidates(other)
                   \cup SequenceSet(asyncIoReadyCompletions[other])
                   \cup SequenceSet(
                        asyncLocalReadyCompletions[other])
          BY <1>1, <2>4 DEF AsyncOutstandingCarrierInvariant
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>4 DEF AsyncOutstandingCarrierInvariant
  <1> QED BY <1>1

THEOREM DrainedIngressPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ IngressDrainStep(node)
    /\ asyncRunnerBudget[node] > 0
    /\ asyncIngressReady[node] # <<>>
    /\ DrainableIngressIndices(node) # {}
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                IngressDrainStep(node),
                asyncRunnerBudget[node] > 0,
                asyncIngressReady[node] # <<>>,
                DrainableIngressIndices(node) # {}
         PROVE AsyncProgressOwnershipInvariant'
    <2> DEFINE DrainIndex == FirstDrainableIngressIndex(node)
    <2> DEFINE DrainSource == asyncIngressReady[node][DrainIndex]
    <2> DEFINE DrainLaneIndex ==
           SelectedIngressLaneIndex(node, DrainIndex)
    <2> DEFINE DrainItem == SelectedIngressItemAt(node, DrainIndex)
    <2> DEFINE Candidate == DeliveryCandidate(DrainItem)
    <2> DEFINE CertifiedCandidate ==
           CertifiedResponseCandidate(DrainItem)
    <2> DEFINE CommitCandidate ==
           CommitCertificateResponseCandidate(DrainItem)
    <2> DEFINE ServeAccepted ==
           /\ DrainItem \in asyncSentItems
           /\ DrainItem.kind
                \in {"CertifiedRequest", "CommitCertificateRequest"}
           /\ IF DrainItem.kind = "CertifiedRequest"
              THEN CertifiedRequestAuthorized(DrainItem)
              ELSE CommitCertificateRequestAuthorized(DrainItem)
    <2> DEFINE CertifiedAccepted ==
           CertifiedResponseClaimAuthorized(DrainItem)
    <2> DEFINE CommitAccepted ==
           /\ DrainItem \in asyncSentItems
           /\ CommitCertificateResponseAuthorized(DrainItem)
    <2> DEFINE ImportAccepted ==
           /\ DrainItem.kind = "CommitCertificateResponse"
           /\ DrainItem \in asyncSentItems
           /\ CommitCertificateResponseAuthorized(DrainItem)
           /\ DrainItem.envelope \notin qcNetwork
    <2> DEFINE OrdinaryAccepted ==
           /\ DrainItem \in asyncSentItems
           /\ DrainItem.kind # "Noise"
           /\ DrainItem.kind
                \notin {"CertifiedRequest", "CommitCertificateRequest"}
           /\ DrainItem.kind # "Chunk"
           /\ DrainItem.kind # "CertifiedResponse"
           /\ DrainItem.kind # "CommitCertificateResponse"
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
           /\ AsyncIngressTypeInvariant
      BY <1>1
         DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant
    <2>2. /\ DrainFairIngressSelected(node)
           /\ LeaveCausalQueues
           /\ UNCHANGED AsyncDeferredVars
           /\ UNCHANGED AsyncLocalAdmissionVars
      BY <1>1, Isa DEF IngressDrainStep
    <2>3. DrainIndex \in DrainableIngressIndices(node)
      BY <1>1, FirstDrainableIngressIndexIsDrainable DEF DrainIndex
    <2>4. /\ DrainIndex \in 1..Len(asyncIngressReady[node])
           /\ IngressSourceCanDrain(node, DrainSource)
           /\ DrainLaneIndex \in
                DrainableIngressLaneIndices(node, DrainSource)
           /\ DrainLaneIndex \in
                1..Len(IngressLane(node, DrainSource))
           /\ IngressItemCanDrain(node, DrainItem)
      <3>1. /\ DrainIndex \in 1..Len(asyncIngressReady[node])
             /\ IngressSourceCanDrain(node, DrainSource)
        BY <2>3 DEF DrainableIngressIndices, DrainSource
      <3>2. DrainLaneIndex \in
               DrainableIngressLaneIndices(node, DrainSource)
        BY <3>1, FirstDrainableIngressLaneIndexIsDrainable
           DEF IngressSourceCanDrain, DrainLaneIndex,
               SelectedIngressLaneIndex
      <3> QED BY <3>1, <3>2
           DEF DrainableIngressLaneIndices, DrainItem,
               SelectedIngressItemAt, DrainSource, DrainLaneIndex,
               SelectedIngressLaneIndex
    <2>5. /\ AsyncItemTyped(DrainItem)
           /\ DrainItem.envelope.recipient = node
           /\ IngressResourceSource(DrainItem) = DrainSource
      BY <1>1, <2>1, <2>4, SelectedIngressItemIsTyped,
         SelectedIngressItemHasLaneOwnership
         DEF DrainItem, SelectedIngressItemAt, DrainLaneIndex,
             DrainSource
    <2>6. /\ AsyncCandidateTyped(Candidate)
           /\ Candidate.node = node
           /\ Candidate.class # "Completion"
      <3>1. AsyncTypeInvariant
        BY <1>1 DEF AsyncStrongTypeInvariant
      <3>2. /\ AsyncCandidateTyped(DeliveryCandidate(DrainItem))
             /\ DeliveryCandidate(DrainItem).node = node
             /\ DeliveryCandidate(DrainItem).class # "Completion"
        BY <1>1, <2>5, <3>1,
           TypedIngressDeliveryCandidateFacts
      <3> QED BY <3>2 DEF Candidate
    <2>7. /\ UNCHANGED AsyncProgressOwnershipCoreVars
           /\ UNCHANGED AsyncBusyConsumerVars
           /\ UNCHANGED
                <<asyncDeferredCompletionQueues,
                  asyncDeferredProgressQueues,
                  asyncDeferredNormalQueues, asyncCausalQueues>>
      <3>1. /\ UNCHANGED AsyncProgressOwnershipCoreVars
             /\ UNCHANGED AsyncBusyConsumerVars
        <4>1. CASE ImportAccepted
          <5>1. ImportAuthenticatedCommitCertificate(
                   DrainItem.envelope)
            BY <2>2, <4>1, Isa
               DEF DrainFairIngressSelected, ImportAccepted,
                   DrainItem, DrainIndex
          <5> QED BY <5>1
               DEF ImportAuthenticatedCommitCertificate,
                   AsyncProgressOwnershipCoreVars,
                   AsyncBusyConsumerVars
        <4>2. CASE ~ImportAccepted
          <5>1. UNCHANGED vars
            BY <2>2, <4>2, Isa
               DEF DrainFairIngressSelected, ImportAccepted,
                   DrainItem, DrainIndex
          <5> QED BY <5>1
               DEF AsyncProgressOwnershipCoreVars,
                   AsyncBusyConsumerVars, vars
        <4> QED BY <4>1, <4>2
      <3>2. UNCHANGED
               <<asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues,
                 asyncDeferredNormalQueues>>
        BY <2>2 DEF AsyncDeferredVars
      <3>3. UNCHANGED asyncCausalQueues
        BY <2>2 DEF LeaveCausalQueues
      <3> QED BY <3>1, <3>2, <3>3
    <2>8. CASE ServeAccepted
      <3> DEFINE Job ==
             AsyncIoCertifiedServeJob(node, Candidate)
      <3>1. /\ DrainFairIngressSelected(node)
             /\ DrainItem = SelectedDrainItem(node)
             /\ Candidate = SelectedDrainCandidate(node)
        BY <2>2 DEF SelectedDrainItem, SelectedDrainCandidate,
             DrainIndex, DrainItem, Candidate
      <3>2. Job.class # "Consensus"
        BY DEF Job, AsyncIoCertifiedServeJob, AsyncIoJob
      <3>3. asyncIoQueues' =
               [asyncIoQueues EXCEPT ![node] = Append(@, Job)]
        BY <2>8, <3>1, AuthorizedIngressServeFrame
           DEF ServeAccepted, Job
      <3>4. UNCHANGED
               <<AsyncProgressOwnershipCoreVars, asyncCommandQueues,
                 asyncOutstandingWork, asyncIoReadyCompletions,
                 asyncLocalReadyCompletions,
                 asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues,
                 asyncDeferredNormalQueues, asyncCausalQueues>>
        BY <2>7, <2>8, <3>1,
           AuthorizedIngressServeFrame, Isa
           DEF ServeAccepted
      <3> QED BY <1>1, <3>2, <3>3, <3>4,
           NonConsensusIoAppendPreservesProgressOwnership
    <2>9. CASE CertifiedAccepted
      <3>1. /\ DrainItem.kind = "CertifiedResponse"
             /\ AsyncCandidateTyped(CertifiedCandidate)
             /\ CertifiedCandidate.node = node
             /\ CertifiedCandidate.class = "Completion"
        <4>1. AsyncTypeInvariant
          BY <1>1 DEF AsyncStrongTypeInvariant
        <4>2. DrainItem.kind = "CertifiedResponse"
          BY <2>9
             DEF CertifiedAccepted, CertifiedResponseClaimAuthorized,
                 CertifiedResponseAuthorized
        <4>3. /\ AsyncCandidateTyped(
                        CertifiedResponseCandidate(DrainItem))
               /\ CertifiedResponseCandidate(DrainItem).node = node
               /\ CertifiedResponseCandidate(DrainItem).class =
                    "Completion"
          BY <1>1, <2>5, <4>1, <4>2,
             TypedCertifiedResponseCandidateFacts
        <4> QED BY <4>2, <4>3 DEF CertifiedCandidate
      <3>2. CASE CandidateScheduled(CertifiedCandidate)
        <4>0. CandidateScheduled(CertifiedCandidate)
          OBVIOUS
        <4>1. UNCHANGED <<asyncCommandQueues, AsyncIoVars>>
          BY <2>2, <2>9, <4>0,
             ScheduledAuthorizedCertifiedResponseSchedulerFrame
             DEF SelectedDrainItem, DrainIndex, DrainItem,
                 CertifiedAccepted, CertifiedCandidate
        <4>2. UNCHANGED AsyncProgressOwnershipVars
          BY <2>7, <4>1, Isa
             DEF AsyncProgressOwnershipVars,
                 AsyncProgressOwnershipSchedulerVars, AsyncIoVars
        <4> QED BY <1>1, <4>2, AsyncProgressOwnershipStutter
      <3>3. CASE ~CandidateScheduled(CertifiedCandidate)
        <4>0. ~CandidateScheduled(CertifiedCandidate)
          OBVIOUS
        <4>1. /\ asyncCommandQueues' =
                    [asyncCommandQueues EXCEPT
                       ![node] = Append(@, CertifiedCandidate)]
               /\ UNCHANGED AsyncIoVars
          BY <2>2, <2>9, <4>0,
             FreshAuthorizedCertifiedResponseSchedulerFrame
             DEF SelectedDrainItem, DrainIndex, DrainItem,
                 CertifiedAccepted, CertifiedCandidate
        <4>2. /\ AsyncLogicalCandidateOwnershipInvariant'
               /\ QueuedCandidates' =
                    QueuedCandidates \cup {CertifiedCandidate}
               /\ ActiveBusyCompletionCarrier \subseteq
                    ActiveBusyCompletionCarrier'
          <5>1. AsyncLogicalCandidateOwnershipInvariant
            BY <1>1 DEF AsyncProgressOwnershipInvariant
          <5>2. UNCHANGED
                   <<asyncOutstandingWork,
                     asyncDeferredCompletionQueues,
                     asyncDeferredProgressQueues,
                     asyncDeferredNormalQueues, asyncCausalQueues>>
            BY <2>7, <4>1 DEF AsyncIoVars
          <5> QED BY <1>1, <2>1, <3>1, <4>0, <4>1,
             <5>1, <5>2,
             FreshCommandAppendPreservesLogicalOwnership
        <4>3. AsyncOutstandingCarrierInvariant'
          BY <1>1, <4>1, AsyncOutstandingCarrierStutter
             DEF AsyncProgressOwnershipInvariant, AsyncIoVars
        <4>4. /\ SerializedBusyOwnershipInvariant'
               /\ BusyCompletionWitnessInvariant'
          BY <1>1, <2>7, <4>2,
             ProgressCoreFramePreservesBusyOwnership
        <4> QED BY <4>2, <4>3, <4>4
             DEF AsyncProgressOwnershipInvariant
      <3> QED BY <3>2, <3>3
    <2>10. CASE CommitAccepted
      <3>1. /\ DrainItem.kind = "CommitCertificateResponse"
             /\ AsyncCandidateTyped(CommitCandidate)
             /\ CommitCandidate.node = node
             /\ CommitCandidate.class # "Completion"
        <4>1. AsyncTypeInvariant
          BY <1>1 DEF AsyncStrongTypeInvariant
        <4>2. DrainItem.kind = "CommitCertificateResponse"
          BY <2>10 DEF CommitAccepted,
             CommitCertificateResponseAuthorized
        <4>3. /\ AsyncCandidateTyped(
                        CommitCertificateResponseCandidate(DrainItem))
               /\ CommitCertificateResponseCandidate(DrainItem).node =
                    node
               /\ CommitCertificateResponseCandidate(DrainItem).class #
                    "Completion"
          BY <1>1, <2>5, <4>1, <4>2,
             TypedCommitCertificateResponseCandidateFacts
        <4> QED BY <4>2, <4>3 DEF CommitCandidate
      <3>2. /\ DrainItem = SelectedDrainItem(node)
             /\ CommitCandidate =
                  SelectedDrainCommitCandidate(node)
        BY DEF SelectedDrainItem, SelectedDrainCommitCandidate,
               DrainIndex, DrainItem, CommitCandidate
      <3>3. CASE CandidateScheduled(CommitCandidate)
        <4>0. CandidateScheduled(CommitCandidate)
          OBVIOUS
        <4>1. UNCHANGED <<asyncCommandQueues,
                           asyncNextCommandClass>>
          BY <2>2, <2>10, <3>1, <3>2, <4>0,
             ScheduledAuthorizedCommitResponseCommandFrame
             DEF CommitAccepted
        <4>2. UNCHANGED AsyncIoVars
          BY <2>2, <2>10, <3>2, AuthorizedCommitResponseFrame
             DEF CommitAccepted
        <4>3. UNCHANGED AsyncProgressOwnershipVars
          BY <2>7, <4>1, <4>2, Isa
             DEF AsyncProgressOwnershipVars,
                 AsyncProgressOwnershipSchedulerVars, AsyncIoVars
        <4> QED BY <1>1, <4>3, AsyncProgressOwnershipStutter
      <3>4. CASE ~CandidateScheduled(CommitCandidate)
        <4>0. ~CandidateScheduled(CommitCandidate)
          OBVIOUS
        <4>1. /\ asyncCommandQueues' =
                    [asyncCommandQueues EXCEPT
                       ![node] = Append(@, CommitCandidate)]
               /\ UNCHANGED AsyncIoVars
          <5>1. asyncCommandQueues' =
                   [asyncCommandQueues EXCEPT
                      ![node] = Append(@, CommitCandidate)]
            BY <2>2, <2>10, <3>1, <3>2, <4>0,
               FreshAuthorizedCommitResponseCommandFrame
               DEF CommitAccepted
          <5>2. UNCHANGED AsyncIoVars
            BY <2>2, <2>10, <3>2, AuthorizedCommitResponseFrame
               DEF CommitAccepted
          <5> QED BY <5>1, <5>2
        <4>2. /\ AsyncLogicalCandidateOwnershipInvariant'
               /\ QueuedCandidates' =
                    QueuedCandidates \cup {CommitCandidate}
               /\ ActiveBusyCompletionCarrier \subseteq
                    ActiveBusyCompletionCarrier'
          <5>1. AsyncLogicalCandidateOwnershipInvariant
            BY <1>1 DEF AsyncProgressOwnershipInvariant
          <5>2. UNCHANGED
                   <<asyncOutstandingWork,
                     asyncDeferredCompletionQueues,
                     asyncDeferredProgressQueues,
                     asyncDeferredNormalQueues, asyncCausalQueues>>
            BY <2>7, <4>1 DEF AsyncIoVars
          <5> QED BY <1>1, <2>1, <3>1, <4>0, <4>1,
             <5>1, <5>2,
             FreshCommandAppendPreservesLogicalOwnership
        <4>3. AsyncOutstandingCarrierInvariant'
          BY <1>1, <4>1, AsyncOutstandingCarrierStutter
             DEF AsyncProgressOwnershipInvariant,
                 AsyncIoVars
        <4>4. /\ SerializedBusyOwnershipInvariant'
               /\ BusyCompletionWitnessInvariant'
          BY <1>1, <2>7, <4>2,
             ProgressCoreFramePreservesBusyOwnership
        <4> QED BY <4>2, <4>3, <4>4
             DEF AsyncProgressOwnershipInvariant
      <3> QED BY <3>3, <3>4
    <2>11. CASE OrdinaryAccepted
      <3>1. CASE CandidateScheduled(Candidate)
        <4>0. CandidateScheduled(Candidate)
          OBVIOUS
        <4>1. UNCHANGED
                 <<asyncCommandQueues, asyncNextCommandClass,
                   AsyncIoVars>>
          BY <2>2, <2>11, <4>0, OrdinaryScheduledIngressFrame
             DEF SelectedDrainItem, SelectedDrainCandidate,
                 DrainIndex, DrainItem, OrdinaryAccepted, Candidate
        <4>2. UNCHANGED AsyncProgressOwnershipVars
          BY <2>7, <4>1, Isa
             DEF AsyncProgressOwnershipVars,
                 AsyncProgressOwnershipSchedulerVars, AsyncIoVars
        <4> QED BY <1>1, <4>2, AsyncProgressOwnershipStutter
      <3>2. CASE ~CandidateScheduled(Candidate)
        <4>0. ~CandidateScheduled(Candidate)
          OBVIOUS
        <4>1. /\ asyncCommandQueues' =
                    [asyncCommandQueues EXCEPT
                       ![node] = Append(@, Candidate)]
               /\ UNCHANGED AsyncIoVars
          BY <2>2, <2>4, <2>6, <2>11, <4>0,
             OrdinaryFreshIngressFrame
             DEF SelectedDrainItem, SelectedDrainCandidate,
                 DrainIndex, DrainItem, OrdinaryAccepted, Candidate
        <4>2. /\ AsyncLogicalCandidateOwnershipInvariant'
               /\ QueuedCandidates' =
                    QueuedCandidates \cup {Candidate}
               /\ ActiveBusyCompletionCarrier \subseteq
                    ActiveBusyCompletionCarrier'
          <5>1. AsyncLogicalCandidateOwnershipInvariant
            BY <1>1 DEF AsyncProgressOwnershipInvariant
          <5>2. UNCHANGED
                   <<asyncOutstandingWork,
                     asyncDeferredCompletionQueues,
                     asyncDeferredProgressQueues,
                     asyncDeferredNormalQueues, asyncCausalQueues>>
            BY <2>7, <4>1 DEF AsyncIoVars
          <5> QED BY <1>1, <2>1, <2>6, <4>0, <4>1,
             <5>1, <5>2,
             FreshCommandAppendPreservesLogicalOwnership
        <4>3. AsyncOutstandingCarrierInvariant'
          BY <1>1, <4>1, AsyncOutstandingCarrierStutter
             DEF AsyncProgressOwnershipInvariant,
                 AsyncIoVars
        <4>4. /\ SerializedBusyOwnershipInvariant'
               /\ BusyCompletionWitnessInvariant'
          BY <1>1, <2>7, <4>2,
             ProgressCoreFramePreservesBusyOwnership
        <4> QED BY <4>2, <4>3, <4>4
             DEF AsyncProgressOwnershipInvariant
      <3> QED BY <3>1, <3>2
    <2>12. CASE ~(ServeAccepted \/ CertifiedAccepted \/
                   CommitAccepted \/ OrdinaryAccepted)
      <3>1. UNCHANGED
               <<asyncCommandQueues, asyncNextCommandClass,
                 AsyncIoVars>>
        BY <2>2, <2>12, RejectedIngressSchedulerFrame
           DEF SelectedDrainItem, DrainIndex, DrainItem,
               ServeAccepted, CertifiedAccepted, CommitAccepted,
               OrdinaryAccepted
      <3>2. UNCHANGED AsyncProgressOwnershipVars
        BY <2>7, <3>1, Isa
           DEF AsyncProgressOwnershipVars,
               AsyncProgressOwnershipSchedulerVars, AsyncIoVars
      <3> QED BY <1>1, <3>2, AsyncProgressOwnershipStutter
    <2> QED BY <2>8, <2>9, <2>10, <2>11, <2>12
  <1> QED BY <1>1

THEOREM IngressDrainPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ IngressDrainStep(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                IngressDrainStep(node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. CASE asyncRunnerBudget[node] > 0
               /\ asyncIngressReady[node] # <<>>
               /\ DrainableIngressIndices(node) # {}
      BY <1>1, <2>1,
         DrainedIngressPreservesProgressOwnership
    <2>2. CASE ~(asyncRunnerBudget[node] > 0
                 /\ asyncIngressReady[node] # <<>>
                 /\ DrainableIngressIndices(node) # {})
      BY <1>1, <2>2,
         IngressPhaseAdvancePreservesProgressOwnership
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM RuntimeFrameBranchPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncProgressOwnershipInvariant
    /\ UNCHANGED AsyncIoVars
    /\ \/ DirectRetransmitStep(node)
       \/ DeferredRetransmitStep(node)
       \/ IdleRuntimeStep(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncProgressOwnershipInvariant,
                UNCHANGED AsyncIoVars,
                \/ DirectRetransmitStep(node)
                 \/ DeferredRetransmitStep(node)
                 \/ IdleRuntimeStep(node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. CASE DirectRetransmitStep(node)
      <3>1. /\ UNCHANGED AsyncProgressOwnershipCoreVars
             /\ UNCHANGED AsyncBusyConsumerVars
        BY <2>1
           DEF DirectRetransmitStep,
               AsyncProgressOwnershipCoreVars,
               AsyncBusyConsumerVars, vars
      <3>2. UNCHANGED AsyncProgressOwnershipSchedulerVars
        BY <1>1, <2>1, Isa
           DEF DirectRetransmitStep, LeaveCausalQueues,
               AsyncProgressOwnershipSchedulerVars, AsyncIoVars
      <3>3. UNCHANGED AsyncProgressOwnershipVars
        BY <3>1, <3>2 DEF AsyncProgressOwnershipVars
      <3> QED BY <1>1, <3>3,
           AsyncProgressOwnershipStutter
    <2>2. CASE DeferredRetransmitStep(node)
      <3>1. /\ UNCHANGED AsyncProgressOwnershipCoreVars
             /\ UNCHANGED AsyncBusyConsumerVars
        BY <2>2
           DEF DeferredRetransmitStep,
               AsyncProgressOwnershipCoreVars,
               AsyncBusyConsumerVars, vars
      <3>2. UNCHANGED AsyncProgressOwnershipSchedulerVars
        BY <1>1, <2>2, Isa
           DEF DeferredRetransmitStep, LeaveCausalQueues,
               AsyncProgressOwnershipSchedulerVars, AsyncIoVars
      <3>3. UNCHANGED AsyncProgressOwnershipVars
        BY <3>1, <3>2 DEF AsyncProgressOwnershipVars
      <3> QED BY <1>1, <3>3,
           AsyncProgressOwnershipStutter
    <2>3. CASE IdleRuntimeStep(node)
      <3>1. /\ UNCHANGED AsyncProgressOwnershipCoreVars
             /\ UNCHANGED AsyncBusyConsumerVars
        BY <2>3
           DEF IdleRuntimeStep,
               AsyncProgressOwnershipCoreVars,
               AsyncBusyConsumerVars, vars
      <3>2. UNCHANGED AsyncProgressOwnershipSchedulerVars
        BY <1>1, <2>3
           DEF IdleRuntimeStep, LeaveCausalQueues,
               AsyncProgressOwnershipSchedulerVars,
               AsyncIoVars, AsyncDeferredVars
      <3>3. UNCHANGED AsyncProgressOwnershipVars
        BY <3>1, <3>2 DEF AsyncProgressOwnershipVars
      <3> QED BY <1>1, <3>3,
           AsyncProgressOwnershipStutter
    <2> QED BY <1>1, <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM DirectCommitDiscoveryPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncProgressOwnershipInvariant
    /\ CommitCertificateDiscoveryStepWork(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncProgressOwnershipInvariant,
                CommitCertificateDiscoveryStepWork(node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. /\ UNCHANGED AsyncProgressOwnershipCoreVars
           /\ UNCHANGED AsyncBusyConsumerVars
      BY <1>1
         DEF CommitCertificateDiscoveryStepWork,
             AsyncProgressOwnershipCoreVars,
             AsyncBusyConsumerVars, vars
    <2>2. UNCHANGED AsyncProgressOwnershipSchedulerVars
      BY <1>1
         DEF CommitCertificateDiscoveryStepWork,
             AsyncProgressOwnershipSchedulerVars,
             AsyncIoVars, AsyncDeferredVars, LeaveCausalQueues
    <2>3. UNCHANGED AsyncProgressOwnershipVars
      BY <2>1, <2>2 DEF AsyncProgressOwnershipVars
    <2> QED BY <1>1, <2>3,
         AsyncProgressOwnershipStutter
  <1> QED BY <1>1

THEOREM TimeoutRuntimeBranchPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ UNCHANGED AsyncIoVars
    /\ \/ DirectTimeoutStep(node)
       \/ DeferredTimeoutStep(node)
    => AsyncProgressOwnershipInvariant'
BY AsyncStrongTypeProjectsAsyncType,
   FreshTimeoutCausalSuccessorsTypedAndOwned,
   FreshCommandSuccessorsHaveUniqueValues,
   FreshCommandSuccessorsAreUnscheduled,
   AppendOwnedCausalSuccessorsPreservesCausalType,
   RangeConcatenation, Isa
   DEF DirectTimeoutStep, DeferredTimeoutStep, BeginTimeoutEnabled,
       TimeoutCausalCommand, AppendCausalSuccessors,
       FreshCommandSuccessors, FreshCandidateSequence,
       CommandSuccessors, CausalCandidate, NoItemCandidate,
       AsyncCandidate, BeginTimeout,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       SerializedBusyOwnershipInvariant, BusyCompletionWitnessInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncCausalTypeInvariant,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, ConsensusIoCandidates,
       SerializedBusyOwners, BusyCompletionCandidates,
       ActiveBusyCompletionCarrier, CandidateConsumerCurrent,
       SequenceHasUniqueValues, SequenceSet,
       NodeIdle, PendingNodes, SigningNodes, AllPendingRequests,
       RequestNodeSet, RequestsUniqueByNode, LeaveCausalQueues,
       AsyncDeferredVars, AsyncIoVars, vars

THEOREM DeferredTagPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ UNCHANGED AsyncIoVars
    /\ DeferredTagStep(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                UNCHANGED AsyncIoVars,
                DeferredTagStep(node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. CASE DeferredTimeoutExecutable(node)
      <3>1. DeferredTimeoutStep(node)
        BY <1>1, <2>1 DEF DeferredTagStep
      <3> QED BY <1>1, <3>1,
           TimeoutRuntimeBranchPreservesProgressOwnership
    <2>2. CASE ~DeferredTimeoutExecutable(node)
      <3>1. DeferredRetransmitStep(node)
        BY <1>1, <2>2 DEF DeferredTagStep
      <3> QED BY <1>1, <3>1,
           RuntimeFrameBranchPreservesProgressOwnership
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Generic Busy-completion dispatch bridge.

Membership supplies an active, current, Completion-class scheduler candidate;
the serialized Busy kernel supplies the exact Core action guard for the
matching pending/signature owner.  This leaf is intentionally independent of
`AsyncProgressOwnershipInvariant`, so FIFO preservation cannot assume the
very Busy-witness property it is proving.  Each finite dispatch arm expands
ENABLED independently, keeping the scheduler-carrier and publication facts
separate from the final ten-way owner match.
***************************************************************************)
THEOREM TypedItemIsInNetworkCarrier ==
  \A item:
    AsyncItemTyped(item) => item \in AsyncNetworkItems
BY Isa
   DEF AsyncItemTyped, AsyncNetworkItems, AsyncNetworkItem,
       AsyncCertifiedRequestItems, AsyncCommitCertificateRequestItems,
       AsyncBodyEnvelopeTyped,
       AsyncCommitCertificateRequestEnvelopeTyped,
       AsyncReplyRequestItemTyped,
       AsyncCertifiedResponseEnvelopeTyped,
       AsyncCommitCertificateResponseEnvelopeTyped,
       AsyncCertifiedResponseEnvelope,
       AsyncCommitCertificateResponseEnvelope,
       AsyncTcEnvelopeTyped, AsyncTcRecordTyped,
       AsyncBodyEnvelopeSet, AsyncCommitCertificateRequestEnvelopeSet,
       TcEnvelopeSet, TcRecordSet

THEOREM RetainableControlBatchIsPublishable ==
  \A items, voters:
    RetainableControlBatch(items, voters)
      => items \subseteq
           {item \in AsyncNetworkItems:
              item.kind \in AsyncControlKinds}
BY TypedItemIsInNetworkCarrier, Isa
   DEF RetainableControlBatch

THEOREM ActiveBusyCompletionCarrierIsTyped ==
  \A candidate:
    /\ AsyncSchedulerTypeInvariant
    /\ candidate \in ActiveBusyCompletionCarrier
    => AsyncCandidateTyped(candidate)
BY Isa
   DEF ActiveBusyCompletionCarrier,
       QueuedCandidates, CausalCandidates, TrackedWorkCandidates,
       SequenceSet,
       AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncIoWorkContentTypeInvariant,
       AsyncQueueTyped

THEOREM PersistProposalBusyArmEnabled ==
  \A command, request:
    /\ command.kind = "PersistProposal"
    /\ request \in pendingProposal
    /\ request.proposal \notin proposalIntents
    /\ CommandMatches(command, request.node, request.proposal.view,
                      request.proposal.subject)
    => ENABLED ExecuteRegularCommand(command)
BY ExpandENABLED, Isa
   DEF ExecuteRegularCommand, RegularCoreCommand,
       PersistProposal, AsyncAuxVars

THEOREM PersistPrepareBusyArmEnabled ==
  \A command, request:
    /\ command.kind = "PersistPrepare"
    /\ request \in pendingPrepare
    /\ request.vote \notin prepareIntents
    /\ CommandMatches(command, request.node, request.vote.view,
                      request.vote.subject)
    => ENABLED ExecuteRegularCommand(command)
BY ExpandENABLED, Isa
   DEF ExecuteRegularCommand, RegularCoreCommand,
       PersistPrepare, AsyncAuxVars

THEOREM PersistObservePrepareBusyArmEnabled ==
  \A command, request:
    /\ command.kind = "PersistObservePrepare"
    /\ request \in pendingObservePrepare
    /\ CommandMatches(command, request.node, request.qc.view,
                      request.qc.subject)
    => ENABLED ExecuteRegularCommand(command)
BY ExpandENABLED, Isa
   DEF ExecuteRegularCommand, RegularCoreCommand,
       PersistObservePrepare, AsyncAuxVars

THEOREM PersistLockCommitBusyArmEnabled ==
  \A command, request:
    /\ command.kind = "PersistLockCommit"
    /\ request \in pendingLockCommit
    /\ request.vote \notin commitIntents
    /\ BodyHeldBy(durableBodies, request.node, request.qc.context,
                  request.qc.view, request.qc.subject)
    /\ RetainedLockedBodyRecord(
         request.node, request.qc.context, request.qc.subject)
         \in RetainedLockedBodyRecordSet
    /\ CommandMatches(command, request.node, request.qc.view,
                      request.qc.subject)
    => ENABLED ExecuteRegularCommand(command)
BY ExpandENABLED, Isa
   DEF ExecuteRegularCommand, RegularCoreCommand,
       PersistLockCommit, AsyncAuxVars

THEOREM PersistTimeoutBusyArmEnabled ==
  \A command, request:
    /\ command.kind = "PersistTimeout"
    /\ request \in pendingTimeout
    /\ request.vote \notin timeoutIntents
    /\ CommandMatches(command, request.node, request.vote.view,
                      request.vote.highSubject)
    => ENABLED ExecuteRegularCommand(command)
BY ExpandENABLED, Isa
   DEF ExecuteRegularCommand, RegularCoreCommand,
       PersistTimeout, AsyncAuxVars

THEOREM PersistInstallBusyArmEnabled ==
  \A command, request:
    /\ command.kind = "PersistInstallTC"
    /\ request \in pendingInstallTC
    /\ command.node = request.node
    /\ command.view = request.tc.view
    /\ request.tc.view >= nodeView[request.node]
    => ENABLED ExecutePersistInstall(command)
BY ExpandENABLED, Isa
   DEF ExecutePersistInstall, PersistInstallTC,
       PersistInstalledControlAfterInstall

THEOREM PersistDecisionBusyArmEnabled ==
  \A command, request:
    /\ command.kind = "PersistDecision"
    /\ request \in pendingDecision
    /\ CommandMatches(command, request.node, request.qc.view,
                      request.qc.subject)
    => ENABLED ExecutePersistDecision(command)
BY ExpandENABLED, Isa
   DEF ExecutePersistDecision, PersistDecision,
       PersistDecisionControl

THEOREM SignProposalBusyArmEnabled ==
  \A command, request:
    /\ AsyncTypeInvariant
    /\ command.kind = "SignProposal"
    /\ request \in signProposals
    /\ request.proposal.proposer = request.node
    /\ request.proposal \in proposalIntents
    /\ CommandMatches(command, request.node, request.proposal.view,
                      request.proposal.subject)
    => ENABLED ExecuteSignProposal(command)
PROOF
  <1>1. ASSUME NEW command, NEW request,
                AsyncTypeInvariant,
                command.kind = "SignProposal",
                request \in signProposals,
                request.proposal.proposer = request.node,
                request.proposal \in proposalIntents,
                CommandMatches(command, request.node,
                               request.proposal.view,
                               request.proposal.subject)
         PROVE ENABLED ExecuteSignProposal(command)
    <2>1. request \in ProposalSignSet
      BY <1>1 DEF AsyncTypeInvariant, TypeInvariant
    <2>2. RetainableControlBatch(
             ProposalOutbox(request), CurrentVoters)
      BY <1>1, <2>1, ProposalOutboxIsRetainable
    <2>3. ProposalOutbox(request) \subseteq
             {item \in AsyncNetworkItems:
                item.kind \in AsyncControlKinds}
      BY <2>2, RetainableControlBatchIsPublishable
    <2> QED BY <1>1, <2>3, ExpandENABLED, Isa
         DEF ExecuteSignProposal, CompleteProposalSignature,
             PublishControlAndEphemeralItems
  <1> QED BY <1>1

THEOREM SignVoteBusyArmEnabled ==
  \A command, request:
    /\ AsyncTypeInvariant
    /\ command.kind = "SignVote"
    /\ request \in signVotes
    /\ request.vote.signer = request.node
    /\ (request.vote \in prepareIntents
          \/ request.vote \in commitIntents)
    /\ VoteRoundAdmissible(request.node, request.vote)
    /\ CommandMatches(command, request.node, request.vote.view,
                      request.vote.subject)
    => ENABLED ExecuteSignVote(command)
PROOF
  <1>1. ASSUME NEW command, NEW request,
                AsyncTypeInvariant,
                command.kind = "SignVote",
                request \in signVotes,
                request.vote.signer = request.node,
                request.vote \in prepareIntents
                  \/ request.vote \in commitIntents,
                VoteRoundAdmissible(request.node, request.vote),
                CommandMatches(command, request.node,
                               request.vote.view,
                               request.vote.subject)
         PROVE ENABLED ExecuteSignVote(command)
    <2>1. request \in VoteSignSet
      BY <1>1 DEF AsyncTypeInvariant, TypeInvariant
    <2>2. RetainableControlBatch(
             VoteOutbox(request), CurrentVoters)
      BY <1>1, <2>1, VoteOutboxIsRetainable
    <2>3. VoteOutbox(request) \subseteq
             {item \in AsyncNetworkItems:
                item.kind \in AsyncControlKinds}
      BY <2>2, RetainableControlBatchIsPublishable
    <2> QED BY <1>1, <2>3, ExpandENABLED, Isa
         DEF ExecuteSignVote, CompleteVoteSignature,
             PublishControlItems
  <1> QED BY <1>1

THEOREM SignTimeoutBusyArmEnabled ==
  \A command, request:
    /\ AsyncTypeInvariant
    /\ command.kind = "SignTimeout"
    /\ request \in signTimeouts
    /\ request.vote.signer = request.node
    /\ request.vote \in timeoutIntents
    /\ CommandMatches(command, request.node, request.vote.view,
                      request.vote.highSubject)
    => ENABLED ExecuteSignTimeout(command)
PROOF
  <1>1. ASSUME NEW command, NEW request,
                AsyncTypeInvariant,
                command.kind = "SignTimeout",
                request \in signTimeouts,
                request.vote.signer = request.node,
                request.vote \in timeoutIntents,
                CommandMatches(command, request.node,
                               request.vote.view,
                               request.vote.highSubject)
         PROVE ENABLED ExecuteSignTimeout(command)
    <2>1. request \in TimeoutSignSet
      BY <1>1 DEF AsyncTypeInvariant, TypeInvariant
    <2>2. RetainableControlBatch(
             TimeoutOutbox(request), CurrentVoters)
      BY <1>1, <2>1, TimeoutOutboxIsRetainable
    <2>3. TimeoutOutbox(request) \subseteq
             {item \in AsyncNetworkItems:
                item.kind \in AsyncControlKinds}
      BY <2>2, RetainableControlBatchIsPublishable
    <2> QED BY <1>1, <2>3, ExpandENABLED, Isa
         DEF ExecuteSignTimeout, CompleteTimeoutSignature,
             PublishControlItems
  <1> QED BY <1>1

THEOREM EnabledBusyArmImpliesCommandExecutionReady ==
  \A command:
    (\/ ENABLED ExecuteRegularCommand(command)
     \/ ENABLED ExecutePersistInstall(command)
     \/ ENABLED ExecutePersistDecision(command)
     \/ ENABLED ExecuteSignProposal(command)
     \/ ENABLED ExecuteSignVote(command)
     \/ ENABLED ExecuteSignTimeout(command))
      => CommandExecutionReady(command)
BY Isa DEF CommandExecutionReady

THEOREM InstallRankInvariantExcludesGenerationExhaustion ==
  \A node \in ValidatorIds:
    AsyncStrongTypeInvariant
      => ~InstallGenerationExhausted(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant
         PROVE ~InstallGenerationExhausted(node)
    <2>1. StrongInductiveInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2. TypeInvariant
      BY <2>1 DEF StrongInductiveInvariant, Safety
    <2>3. ASSUME InstallGenerationExhausted(node)
           PROVE FALSE
      <3>1. PICK request \in pendingInstallTC:
               /\ request.node = node
               /\ StrictSameRoundTcUpgrade(node, request.tc)
               /\ ~GenerationCanIncrement(generation[node])
        BY <2>3 DEF InstallGenerationExhausted
      <3>2. /\ TcHighRank(request.tc) \in Ranks
             /\ TcHighRank(request.tc) <= request.tc.view
        BY <2>1, <3>1,
           ValidInstallSelectedRankDoesNotExceedTcView
      <3>3. FALSE
        BY <2>1, <2>2, <3>1, <3>2, SMT
           DEF StrongInductiveInvariant, Safety,
               ReducerProvenanceInvariant,
               PendingCertificateWritesAuthorized,
               StrictSameRoundTcUpgrade, GenerationCanIncrement,
               TypeInvariant, ModelConfiguration, Generations,
               Ranks, Views, NoRank
      <3> QED BY <3>3
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM BusyCompletionCandidateExecutionIsEnabled ==
  \A node \in ValidatorIds:
    \A candidate:
      /\ AsyncTypeInvariant
      /\ AsyncBusyReadinessInvariant
      /\ candidate \in BusyCompletionCandidates(node)
      => \/ CommandExecutionReady(candidate)
         \/ InstallGenerationExhausted(node)
BY PersistProposalBusyArmEnabled,
   PersistPrepareBusyArmEnabled,
   PersistObservePrepareBusyArmEnabled,
   PersistLockCommitBusyArmEnabled,
   PersistTimeoutBusyArmEnabled,
   PersistInstallBusyArmEnabled,
   PersistDecisionBusyArmEnabled,
   SignProposalBusyArmEnabled,
   SignVoteBusyArmEnabled,
   SignTimeoutBusyArmEnabled,
   EnabledBusyArmImpliesCommandExecutionReady,
   Isa
   DEF BusyCompletionCandidates, InstallGenerationExhausted,
       CommandMatches, TypeInvariant, Generations

THEOREM BusyCompletionCandidateIsDispatchable ==
  \A node \in ValidatorIds:
    \A candidate:
      /\ AsyncStrongTypeInvariant
      /\ candidate \in BusyCompletionCandidates(node)
      => CommandDispatchable(candidate)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW candidate,
                AsyncStrongTypeInvariant,
                candidate \in BusyCompletionCandidates(node)
         PROVE CommandDispatchable(candidate)
    <2>1. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>2. AsyncSchedulerTypeInvariant
      BY <2>1 DEF AsyncTypeInvariant
    <2>3. AsyncBusyReadinessInvariant
      BY <1>1
         DEF AsyncStrongTypeInvariant,
             AsyncSerializedBusyKernelInvariant
    <2>4. AsyncCandidateTyped(candidate)
      BY <1>1, <2>2, ActiveBusyCompletionCarrierIsTyped
         DEF BusyCompletionCandidates
    <2>5. /\ CandidateConsumerCurrent(candidate)
           /\ candidate.class = "Completion"
      BY <1>1 DEF BusyCompletionCandidates
    <2>6. \/ CommandExecutionReady(candidate)
           \/ InstallGenerationExhausted(node)
      BY <1>1, <2>1, <2>3,
         BusyCompletionCandidateExecutionIsEnabled
    <2>7. CASE CommandExecutionReady(candidate)
      BY <2>4, <2>5, <2>7 DEF CommandDispatchable
    <2>8. CASE InstallGenerationExhausted(node)
      BY <1>1, <2>8,
         InstallRankInvariantExcludesGenerationExhaustion
    <2> QED BY <2>6, <2>7, <2>8
  <1> QED BY <1>1

THEOREM FifoRuntimePreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ UNCHANGED AsyncIoVars
    /\ FifoRuntimeStep(node)
    => AsyncProgressOwnershipInvariant'
BY BusyCompletionCandidateIsDispatchable,
   AsyncStrongTypeProjectsAsyncType,
   RuntimeSelectedCommandsAreTyped, NextNodeCommandIndexFacts,
   SequenceWithoutIndexFacts, TypedOwnedSequenceWithoutIndexFacts,
   ExecutedFreshCommandSuccessorsTypedAndOwned,
   ExecutedInstallProposalSuccessorMatchesPostState,
   ExecutedInstallLockedFetchSuccessorMatchesPostState,
   ExecutedInstallCommitSignSuccessorMatchesPostState,
   FreshCommandSuccessorsHaveUniqueValues,
   FreshCommandSuccessorsAreUnscheduled,
   RangeConcatenation, SequenceSetAfterAppend, Isa
   DEF FifoRuntimeStep, RemoveNextNodeCommand, NextNodeCommand,
       NextNodeCommandIndex, SequenceWithoutIndex, DeferCommand,
       DeferredProgressAfter, DiscardCommand, ExecuteCommand,
       ExecuteRegularCommand, ExecuteSignProposal, ExecuteSignVote,
       ExecuteFormPrepareQC, ExecuteSignTimeout, ExecutePersistInstall,
       ExecutePersistDecision, ExecuteRequestCertifiedBody, ExecuteApply,
       ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk, AppendCausalSuccessors,
       FreshCommandSuccessors, FreshCandidateSequence,
       InstallCommandSuccessors, InstallLockedFetchSuccessors,
       InstallCommitSignSuccessors, InstallLockedFetchSuccessor,
       InstallCommitSignSuccessor, InstallProposalSuccessor,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       SerializedBusyOwnershipInvariant, BusyCompletionWitnessInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant, AsyncDeferredTypeInvariant,
       AsyncDeferredContentTypeInvariant,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, ConsensusIoCandidates,
       SerializedBusyOwners, BusyCompletionCandidates,
       ActiveBusyCompletionCarrier, CandidateConsumerCurrent,
       SequenceHasUniqueValues, SequenceSet,
       NodeIdle, PendingNodes, SigningNodes, AllPendingRequests,
       RequestNodeSet, RequestsUniqueByNode, AllPendingRequests,
       AsyncIoVars, AsyncDeferredVars, AsyncAuxVars, vars

THEOREM DeferredDrainPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ UNCHANGED AsyncIoVars
    /\ DeferredDrainStep(node)
    => AsyncProgressOwnershipInvariant'
BY AsyncStrongTypeProjectsAsyncType,
   RuntimeSelectedCommandsAreTyped,
   RemoveNextDeferredCommandPreservesDeferredContentType,
   TypedQueueTailFacts, ExecutedFreshCommandSuccessorsTypedAndOwned,
   ExecutedInstallProposalSuccessorMatchesPostState,
   ExecutedInstallLockedFetchSuccessorMatchesPostState,
   ExecutedInstallCommitSignSuccessorMatchesPostState,
   FreshCommandSuccessorsHaveUniqueValues,
   FreshCommandSuccessorsAreUnscheduled,
   RangeConcatenation, Isa
   DEF DeferredDrainStep, DeferredQueueNonempty,
       NextDeferredCommand, RemoveNextDeferredCommand,
       DeferredClassQueue, AdvanceNextDeferredClass,
       DiscardCommand, ExecuteCommand, ExecuteRegularCommand,
       ExecuteSignProposal, ExecuteSignVote, ExecuteFormPrepareQC,
       ExecuteSignTimeout, ExecutePersistInstall,
       ExecutePersistDecision, ExecuteRequestCertifiedBody, ExecuteApply,
       ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk, AppendCausalSuccessors,
       FreshCommandSuccessors, FreshCandidateSequence,
       InstallCommandSuccessors, InstallLockedFetchSuccessors,
       InstallCommitSignSuccessors, InstallLockedFetchSuccessor,
       InstallCommitSignSuccessor, InstallProposalSuccessor,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       SerializedBusyOwnershipInvariant, BusyCompletionWitnessInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant, AsyncDeferredTypeInvariant,
       AsyncDeferredContentTypeInvariant,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, ConsensusIoCandidates,
       SerializedBusyOwners, BusyCompletionCandidates,
       ActiveBusyCompletionCarrier, CandidateConsumerCurrent,
       SequenceHasUniqueValues, SequenceSet,
       NodeIdle, PendingNodes, SigningNodes, AllPendingRequests,
       RequestNodeSet, RequestsUniqueByNode,
       AsyncIoVars, AsyncDeferredVars, AsyncAuxVars,
       LeaveCausalQueues, vars

THEOREM SerializedRuntimePreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ (SerializedRuntimeStep(node)
          \/ SerializedRuntimePrecedesServeIngressStep(node))
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                SerializedRuntimeStep(node)
                  \/ SerializedRuntimePrecedesServeIngressStep(node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. \/ DeferredDrainStep(node)
           \/ DeferredTagStep(node)
           \/ DirectTimeoutStep(node)
           \/ FifoRuntimeStep(node)
           \/ DirectRetransmitStep(node)
           \/ IdleRuntimeStep(node)
      BY <1>1, Isa
         DEF SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep, RuntimeStep
    <2>1a. UNCHANGED AsyncIoVars
      BY <1>1, Isa
         DEF SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep
    <2>2. CASE DirectRetransmitStep(node)
                   \/ IdleRuntimeStep(node)
      BY <1>1, <2>1a, <2>2,
         RuntimeFrameBranchPreservesProgressOwnership
    <2>3. CASE DeferredDrainStep(node)
      BY <1>1, <2>1a, <2>3,
         DeferredDrainPreservesProgressOwnership
    <2>4. CASE DeferredTagStep(node)
      BY <1>1, <2>1a, <2>4,
         DeferredTagPreservesProgressOwnership
    <2>5. CASE DirectTimeoutStep(node)
      BY <1>1, <2>1a, <2>5,
         TimeoutRuntimeBranchPreservesProgressOwnership
    <2>6. CASE FifoRuntimeStep(node)
      BY <1>1, <2>1a, <2>6,
         FifoRuntimePreservesProgressOwnership
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM ExactLocalContinuationReplayPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ReplayRunNodeCandidateProducerContinuation(node)
    /\ AsyncCandidateProducerContinuationExactLocalReplayStep(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                ReplayRunNodeCandidateProducerContinuation(node),
                AsyncCandidateProducerContinuationExactLocalReplayStep(
                  node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. LET candidate ==
                  AsyncCandidateProducerContinuationSelectedLocalCandidate(
                    node)
           IN /\ AsyncRuntimeScalarTypeInvariant
              /\ candidate.node = node
              /\ ~CandidateScheduled(candidate)
      BY <1>1, IsaT(300)
         DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
             ReplayRunNodeCandidateProducerContinuation,
             AsyncCandidateProducerContinuationExactLocalReplayStep,
             AsyncCandidateProducerContinuationExactReplayIdentity,
             AsyncCandidateProducerContinuationSelectedLocalCandidate,
             AsyncCandidateProducerContinuationSelectedReplayRecord,
             AsyncCandidateProducerContinuationSelectedResolutionRecord,
             AsyncCandidateProducerContinuationResolutionRequired,
             AsyncCandidateProducerContinuationResolutionReady,
             AsyncCandidateProducerContinuationResolutionRecordsForNode,
             AsyncCandidateProducerContinuationConcreteSuccessorOwned,
             AsyncCandidateProducerContinuationHandoffOwned,
             AsyncCandidateProducerContinuationLocalReplayCarrier,
             CandidateScheduled
    <2>2. LET candidate ==
                  AsyncCandidateProducerContinuationSelectedLocalCandidate(
                    node)
           IN /\ asyncCommandQueues' =
                    [asyncCommandQueues EXCEPT
                       ![node] = Append(@, candidate)]
              /\ UNCHANGED
                   <<asyncOutstandingWork,
                     asyncDeferredCompletionQueues,
                     asyncDeferredProgressQueues,
                     asyncDeferredNormalQueues, asyncCausalQueues>>
              /\ UNCHANGED
                   <<asyncIoQueues, asyncOutstandingWork,
                     asyncIoReadyCompletions,
                     asyncLocalReadyCompletions>>
              /\ UNCHANGED AsyncProgressOwnershipCoreVars
              /\ UNCHANGED AsyncBusyConsumerVars
      BY <1>1, Isa
         DEF AsyncCandidateProducerContinuationExactLocalReplayStep,
             AsyncCandidateProducerContinuationSelectedLocalCandidate,
             EnqueueCandidate,
             AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
             AsyncProgressOwnershipCoreVars, AsyncBusyConsumerVars,
             AsyncIoVars, vars
    <2>3. LET candidate ==
                  AsyncCandidateProducerContinuationSelectedLocalCandidate(
                    node)
           IN /\ AsyncLogicalCandidateOwnershipInvariant'
              /\ QueuedCandidates' = QueuedCandidates \cup {candidate}
              /\ ActiveBusyCompletionCarrier \subseteq
                   ActiveBusyCompletionCarrier'
      BY <1>1, <2>1, <2>2,
         FreshCommandAppendPreservesLogicalOwnership
         DEF AsyncProgressOwnershipInvariant
    <2>4. AsyncOutstandingCarrierInvariant'
      BY <1>1, <2>2, AsyncOutstandingCarrierStutter
         DEF AsyncProgressOwnershipInvariant
    <2>5. /\ SerializedBusyOwnershipInvariant'
           /\ BusyCompletionWitnessInvariant'
      BY <1>1, <2>2, <2>3,
         ProgressCoreFramePreservesBusyOwnership
    <2> QED BY <2>3, <2>4, <2>5
         DEF AsyncProgressOwnershipInvariant
  <1> QED BY <1>1

THEOREM ExactRuntimeContinuationReplayPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ReplayRunNodeCandidateProducerContinuation(node)
    /\ AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                ReplayRunNodeCandidateProducerContinuation(node),
                AsyncCandidateProducerContinuationExactRuntimeReplayStep(
                  node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. AsyncPersistInstallCommandsForNodeThisStep(node) = {}
      BY <1>1, IsaT(300)
         DEF ReplayRunNodeCandidateProducerContinuation,
             AsyncCandidateProducerContinuationExactRuntimeReplayStep,
             AsyncCandidateProducerContinuationExactReplayIdentity,
             AsyncCandidateProducerContinuationSelectedRuntimeCandidate,
             AsyncCandidateProducerContinuationSelectedReplayRecord,
             AsyncCandidateProducerContinuationSelectedResolutionRecord,
             AsyncCandidateProducerContinuationResolutionRequired,
             AsyncCandidateProducerContinuationResolutionRecordsForNode,
             AsyncCandidateProducerContinuationLocallyReconstructibleKinds,
             AsyncCandidateProducerContinuationSourceClass,
             AsyncPersistInstallCommandsForNodeThisStep,
             AsyncPersistInstallCommandsThisStep,
             AsyncPersistInstallCommandThisStep,
             FifoRuntimeStep, DeferredDrainStep,
             DeferredWorkOwnsRuntimeTurn, NextNodeCommand,
             NextDeferredCommand,
             RemoveNextNodeCommand, RemoveNextDeferredCommand,
             DeferCommand, DiscardCommand,
             ExecuteCommand, ExecuteRegularCommand,
             ExecuteDecisionFetch, ExecuteSignProposal,
             ExecuteSignVote, ExecuteFormPrepareQC,
             ExecuteSignTimeout, ExecutePersistInstall,
             ExecutePersistDecision, ExecuteRequestCertifiedBody,
             ExecuteApply, ExecuteCoreDelivery, ExecuteChunkDelivery,
             ExecuteRejectAuthenticatedJunk
    <2>2. UNCHANGED AsyncIoVars
      BY <1>1, <2>1
         DEF AsyncCandidateProducerContinuationExactRuntimeReplayStep,
             AsyncIoTimeoutLifecycleRetirementTransition
    <2>3. CASE DeferredDrainStep(node)
      BY <1>1, <2>2, <2>3,
         DeferredDrainPreservesProgressOwnership
    <2>4. CASE FifoRuntimeStep(node)
      BY <1>1, <2>2, <2>4,
         FifoRuntimePreservesProgressOwnership
    <2> QED BY <1>1, <2>3, <2>4
         DEF AsyncCandidateProducerContinuationExactRuntimeReplayStep
  <1> QED BY <1>1

THEOREM ReplayRunNodeContinuationPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ReplayRunNodeCandidateProducerContinuation(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                ReplayRunNodeCandidateProducerContinuation(node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. CASE
              AsyncCandidateProducerContinuationExactLocalReplayStep(node)
      BY <1>1, <2>1,
         ExactLocalContinuationReplayPreservesProgressOwnership
    <2>2. CASE
              AsyncCandidateProducerContinuationReplayTargetOnlyTurn(node)
      <3>1. UNCHANGED AsyncProgressOwnershipVars
        BY <2>2, Isa
           DEF AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
               AsyncProgressOwnershipVars,
               AsyncProgressOwnershipCoreVars,
               AsyncBusyConsumerVars,
               AsyncProgressOwnershipSchedulerVars,
               AsyncIoVars, AsyncDeferredVars, vars
      <3> QED BY <1>1, <3>1, AsyncProgressOwnershipStutter
    <2>3. CASE
              AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)
      BY <1>1, <2>3,
         ExactRuntimeContinuationReplayPreservesProgressOwnership
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF ReplayRunNodeCandidateProducerContinuation
  <1> QED BY <1>1

THEOREM RunNodeWorkPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ RunNodeWork(node)
    => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                RunNodeWork(node)
         PROVE AsyncProgressOwnershipInvariant'
    <2>1r. CASE
              ResolveRunNodeCandidateProducerContinuation(node)
      <3>2. UNCHANGED AsyncProgressOwnershipVars
        BY <2>1r, Isa
           DEF ResolveRunNodeCandidateProducerContinuation,
               AsyncSchedulerExceptCausalControlAndNodeService,
               AsyncProgressOwnershipVars,
               AsyncProgressOwnershipCoreVars,
               AsyncProgressOwnershipSchedulerVars, vars
      <3> QED BY <1>1, <3>2, AsyncProgressOwnershipStutter
    <2>1p. CASE
              ReplayRunNodeCandidateProducerContinuation(node)
      BY <1>1, <2>1p,
         ReplayRunNodeContinuationPreservesProgressOwnership
    <2>1. CASE LocalAdmissionStep(node)
      BY <1>1, <2>1, LocalAdmissionPreservesProgressOwnership
    <2>2. CASE IngressDrainStep(node)
      BY <1>1, <2>2, IngressDrainPreservesProgressOwnership
    <2>3. CASE SerializedRuntimeStep(node)
                  \/ SerializedRuntimePrecedesServeIngressStep(node)
      BY <1>1, <2>3,
         SerializedRuntimePreservesProgressOwnership
    <2>4. CASE AsyncServeIngressTargetOnlyTurn(node)
      <3>1. UNCHANGED AsyncProgressOwnershipVars
        BY <2>4, Isa
           DEF AsyncServeIngressTargetOnlyTurn,
               AsyncProgressOwnershipVars,
               AsyncProgressOwnershipCoreVars,
               AsyncBusyConsumerVars,
               AsyncProgressOwnershipSchedulerVars,
               AsyncIoVars, AsyncDeferredVars, vars
      <3> QED BY <1>1, <3>1, AsyncProgressOwnershipStutter
    <2>5. CASE SerializedLocalPrecedesServeIngressStep(node)
      BY <1>1, <2>5,
         SerializedLocalPredecessorPreservesProgressOwnership
    <2> QED BY <1>1, <2>1r, <2>1p, <2>1, <2>2, <2>3, <2>4,
                 <2>5
         DEF RunNodeWork
  <1> QED BY <1>1

THEOREM AsyncFaultPreservesProgressOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncFaultStep
  => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              AsyncFaultStep
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. CASE \E node \in ValidatorIds: PreGstCrash(node)
      <3>1. PICK node \in ValidatorIds: PreGstCrash(node)
        BY <2>1
      <3> QED BY <1>1, <3>1,
           PreGstCrashPreservesProgressOwnership
    <2>2. CASE \E packet \in asyncTransport: PreGstLosePacket(packet)
      <3>1. PICK packet \in asyncTransport: PreGstLosePacket(packet)
        BY <2>2
      <3> QED BY <1>1, <3>1,
           TransportOnlyFaultPreservesProgressOwnership
    <2>3. CASE \E source \in AsyncIngressSources,
                    recipient \in ValidatorIds,
                    nonce \in 0..(AsyncIngressCapacity - 1):
                    InjectByzantineNoise(source, recipient, nonce)
      BY <1>1, <2>3, TransportOnlyFaultPreservesProgressOwnership
    <2>3c. CASE \E kind \in IngressTransportCompletionKinds,
                     recipient \in ValidatorIds,
                     nonce \in 0..(AsyncIngressCapacity - 1):
                     InjectUntrustedTransportCompletion(
                       kind, recipient, nonce)
      BY <1>1, <2>3c, TransportOnlyFaultPreservesProgressOwnership
    <2>4. CASE \E kind \in {"NormalJunk", "ProgressJunk"},
                    source \in ValidatorIds, recipient \in ValidatorIds,
                    nonce \in 0..(AsyncIngressCapacity - 1):
                    InjectAuthenticatedJunk(
                      kind, source, recipient, nonce)
      BY <1>1, <2>4, TransportOnlyFaultPreservesProgressOwnership
    <2>5. CASE \E source \in ValidatorIds,
                    recipient \in ValidatorIds, qc \in commitQCs,
                    nonce \in 0..(AsyncIngressCapacity - 1):
                    InjectByzantineCertifiedRequest(
                      source, recipient, qc, nonce)
      BY <1>1, <2>5, TransportOnlyFaultPreservesProgressOwnership
    <2>6. CASE \E signer \in ValidatorIds, roundView \in Views,
                    subject \in Subjects,
                    timeoutCertificate \in TimeoutCertificateOptionSet,
                    highestPrepare \in PrepareQcOptionSet:
                    AsyncByzantineProposal(
                      signer, roundView, subject,
                      timeoutCertificate, highestPrepare)
      BY <1>1, <2>6, TransportOnlyFaultPreservesProgressOwnership
    <2>7. CASE \E signer \in ValidatorIds, roundView \in Views,
                    phase \in Phases, subject \in Subjects:
                    AsyncByzantineVote(
                      signer, roundView, phase, subject)
      BY <1>1, <2>7, TransportOnlyFaultPreservesProgressOwnership
    <2>8. CASE \E signer \in ValidatorIds, roundView \in Views,
                    highestPrepare \in PrepareQcOptionSet:
                    AsyncByzantineTimeout(
                      signer, roundView, highestPrepare)
      BY <1>1, <2>8, TransportOnlyFaultPreservesProgressOwnership
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>3c, <2>4,
                <2>5, <2>6, <2>7, <2>8 DEF AsyncFaultStep
  <1> QED BY <1>1

THEOREM OpenHistoricalRecoveryPreservesProgressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncProgressOwnershipInvariant
    /\ OpenHistoricalRecovery(node)
    => AsyncProgressOwnershipInvariant'
BY AsyncProgressOwnershipStutter, Isa
   DEF OpenHistoricalRecovery, AsyncProgressOwnershipVars,
       AsyncProgressOwnershipCoreVars,
       AsyncProgressOwnershipSchedulerVars, vars

THEOREM AsyncNonRunnerPreservesProgressOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncNonRunnerStep
  => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              AsyncNonRunnerStep
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. CASE AsyncSetGST
      BY <1>1, <2>1, AsyncSetGstPreservesProgressOwnership
    <2>2. CASE AsyncTick
      BY <1>1, <2>2, AsyncTickPreservesProgressOwnership
    <2>3. CASE \E node \in ValidatorIds:
                  OpenHistoricalRecovery(node)
      BY <1>1, <2>3,
         OpenHistoricalRecoveryPreservesProgressOwnership
    <2>4. CASE \E node \in AsyncCurrentResponsiveVoters:
                  DirectCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>4, AsyncCurrentResponsiveVotersAreValidators,
         DirectCommitDiscoveryPreservesProgressOwnership
         DEF DirectCommitCertificateDiscoveryStep
    <2>5. CASE \E node \in asyncHistoricalRecoveryTargets:
                  DirectHistoricalCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>5, HistoricalRecoveryTargetsAreValidators,
         DirectCommitDiscoveryPreservesProgressOwnership
         DEF DirectHistoricalCommitCertificateDiscoveryStep
    <2>6. CASE \E node \in AsyncArchiveIoServiceNodes:
                  ServiceIoWorker(node)
      BY <1>1, <2>6, AsyncArchiveIoServiceNodesAreValidators,
         ServiceIoWorkerPreservesProgressOwnership
         DEF ServiceIoWorker
    <2>7. CASE \E node \in asyncHistoricalRecoveryTargets:
                  ServiceHistoricalRecoveryIoWorker(node)
      BY <1>1, <2>7, HistoricalRecoveryTargetsAreValidators,
         ServiceIoWorkerPreservesProgressOwnership
         DEF ServiceHistoricalRecoveryIoWorker
    <2>8. CASE \E node \in AsyncCurrentResponsiveVoters:
                  EnqueueIoLocalControl(node)
      BY <1>1, <2>8, AsyncCurrentResponsiveVotersAreValidators,
         EnqueueIoControlPreservesProgressOwnership
         DEF EnqueueIoLocalControl
    <2>9. CASE \E node \in asyncHistoricalRecoveryTargets:
                  EnqueueHistoricalRecoveryIoLocalControl(node)
      BY <1>1, <2>9, HistoricalRecoveryTargetsAreValidators,
         EnqueueIoControlPreservesProgressOwnership
         DEF EnqueueHistoricalRecoveryIoLocalControl
    <2>10. CASE AsyncNetworkStep
      BY <1>1, <2>10, AdmitIngressPacketPreservesProgressOwnership
         DEF AsyncNetworkStep
    <2>11. CASE AsyncFaultStep
      BY <1>1, <2>11, AsyncFaultPreservesProgressOwnership
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11
         DEF AsyncNonRunnerStep
  <1> QED BY <1>1

THEOREM AsyncNextPreservesProgressOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncNext
  => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              AsyncNext
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. CASE AsyncNonCrashStep
      <3>1. CASE AsyncRunnerStep
        <4>1. CASE \E node \in AsyncCurrentResponsiveVoters:
                      RunNode(node)
          BY <1>1, <2>1, <3>1, <4>1,
             AsyncCurrentResponsiveVotersAreValidators,
             RunNodeWorkPreservesProgressOwnership
             DEF RunNode
        <4>2. CASE \E node \in asyncHistoricalRecoveryTargets:
                      RunHistoricalRecoveryNode(node)
          BY <1>1, <2>1, <3>1, <4>2,
             HistoricalRecoveryTargetsAreValidators,
             RunNodeWorkPreservesProgressOwnership
             DEF RunHistoricalRecoveryNode
        <4>3. CASE \E node \in AsyncResponsiveAppliedArchiveServers:
                      RunHistoricalServer(node)
          BY <1>1, <2>1, <3>1, <4>3,
             HistoricalRunnerPreservesProgressOwnership
        <4> QED BY <3>1, <4>1, <4>2, <4>3 DEF AsyncRunnerStep
      <3>2. CASE AsyncNonRunnerStep
        BY <1>1, <2>1, <3>2,
           AsyncNonRunnerPreservesProgressOwnership
      <3>3. CASE DriveResponsiveReplayHead
        BY <1>1, <3>3,
           DriveResponsiveReplayHeadPreservesProgressOwnership
      <3>4. CASE FinishResponsiveReplay
        BY <1>1, <3>4,
           FinishResponsiveReplayPreservesProgressOwnership
      <3>5. CASE RearmResponsiveRecovery
        BY <1>1, <3>5, AsyncProgressOwnershipStutter, Isa
           DEF RearmResponsiveRecovery,
               AsyncProgressOwnershipVars,
               AsyncProgressOwnershipCoreVars,
               AsyncProgressOwnershipSchedulerVars
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4, <3>5
           DEF AsyncNonCrashStep
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <1>1, <2>2, PreGstCrashPreservesProgressOwnership
    <2>3. CASE \E node \in ValidatorIds:
                  PreGstResponsiveCrash(node)
      BY <1>1, <2>3,
         PreGstResponsiveCrashPreservesProgressOwnership
    <2>4. CASE PreGstResponsiveRestart
      BY <1>1, <2>4,
         PreGstResponsiveRestartPreservesProgressOwnership
    <2>5. CASE PreGstResponsiveReplay
      BY <1>1, <2>5,
         PreGstResponsiveReplayPreservesProgressOwnership
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5 DEF AsyncNext
  <1> QED BY <1>1

THEOREM AsyncBracketNextPreservesProgressOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ [AsyncNext]_AsyncAllVars
  => AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              [AsyncNext]_AsyncAllVars
         PROVE AsyncProgressOwnershipInvariant'
    <2>1. CASE AsyncNext
      BY <1>1, <2>1, AsyncNextPreservesProgressOwnership
    <2>2. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>2,
         AsyncAllVarsStutterPreservesProgressOwnership
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncSpecAlwaysProgressOwnershipInvariant ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []AsyncProgressOwnershipInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []AsyncProgressOwnershipInvariant
    <2>1. AsyncInitAt(initialContext)
             => /\ AsyncStrongTypeInvariant
                /\ AsyncProgressOwnershipInvariant
      BY AsyncInitEstablishesStrongTypeInvariant,
         AsyncInitEstablishesProgressOwnership
    <2>2. /\ AsyncStrongTypeInvariant
             /\ AsyncProgressOwnershipInvariant
             /\ [AsyncNext]_AsyncAllVars
            => /\ AsyncStrongTypeInvariant'
               /\ AsyncProgressOwnershipInvariant'
      BY AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesProgressOwnership
    <2>3. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant)
      BY <2>1, <2>2, PTL DEF AsyncSpecAt
    <2> QED BY <2>3, PTL
  <1> QED BY <1>1

=============================================================================
