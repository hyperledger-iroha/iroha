---- MODULE SumeragiV2AsyncSchedulerPrimitiveTypeProofs ----
EXTENDS SumeragiV2AsyncRankAndInitContinuationProofs

(***************************************************************************
Scheduler type preservation.  The first boundary records precisely which
Core state the scheduler types can observe: the frozen context determines the
current voting roster, while every other free state variable is contained in
`AsyncSchedulerVars`.  The primitive action proofs below then unfold only the
slice changed by that action.
***************************************************************************)

AsyncRuntimeScalarTypeVars ==
  <<asyncNow, asyncCommandQueues, asyncNextCommandClass,
    asyncFifoOwed, asyncTimeoutEmitted,
    asyncRunnerPhase, asyncRunnerBudget, AsyncLocalAdmissionVars>>

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
    asyncDeferredNormalQueues, asyncDeferredHandoffs,
    asyncNextDeferredClass,
    asyncDeferredDrainOwed>>

AsyncDeferredHandoffOwnershipVars ==
  <<asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncDeferredHandoffs>>

AsyncTransportClockTypeVars ==
  <<asyncOutstandingTags, asyncNodeDeadlines, asyncRetransmitDeadlines,
    asyncNodeServiceDeadlines, asyncIoServiceDeadlines>>

\* The claimed response is an opaque preauthenticated capability.  No mutable
\* Core variable is part of its post-admission provenance frame.
AsyncCertifiedResponseClaimCoreAuthorityVars == <<>>

AsyncCertifiedResponseClaimAuthorityVars ==
  <<asyncSentItems, asyncActiveRequests,
    asyncCertifiedResponseClaim>>

AsyncTransportContentTypeVars ==
  <<context, AsyncCertifiedResponseClaimAuthorityVars, asyncRetainedControl,
    asyncTransport, asyncHeldChunks>>

AsyncTransportHistoryTypeVars ==
  <<context, AsyncCertifiedResponseClaimAuthorityVars,
    asyncRetainedControl>>

AsyncIngressTopologyTypeVars == <<asyncIngressLanes, asyncIngressReady>>

THEOREM AsyncRuntimeScalarTypeStutter ==
  AsyncRuntimeScalarTypeInvariant /\ UNCHANGED AsyncRuntimeScalarTypeVars
    => AsyncRuntimeScalarTypeInvariant'
BY Isa
   DEF AsyncRuntimeScalarTypeInvariant, AsyncRuntimeScalarTypeVars,
       AsyncLocalAdmissionVars

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

THEOREM AsyncDeferredHandoffOwnershipStutter ==
  AsyncDeferredHandoffOwnershipInvariant
    /\ UNCHANGED AsyncDeferredHandoffOwnershipVars
    => AsyncDeferredHandoffOwnershipInvariant'
BY Isa
   DEF AsyncDeferredHandoffOwnershipInvariant,
       AsyncDeferredHandoffOwnershipVars, DeferredHandoffActive,
       DeferredHandoffCandidate, DeferredHandoffQueueHead,
       DeferredClassNonempty, DeferredClassQueue

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
       AsyncTransportContentTypeVars,
       AsyncCertifiedResponseClaimAuthorityVars,
       AsyncCertifiedResponseClaimCoreAuthorityVars,
       CurrentVoters, CurrentEpoch

THEOREM AsyncTransportHistoryTypeStutter ==
  AsyncTransportHistoryTypeInvariant
    /\ UNCHANGED AsyncTransportHistoryTypeVars
    => AsyncTransportHistoryTypeInvariant'
BY Isa
   DEF AsyncTransportHistoryTypeInvariant,
       AsyncTransportHistoryTypeVars,
       AsyncCertifiedResponseClaimAuthorityVars,
       AsyncCertifiedResponseClaimCoreAuthorityVars,
       CurrentVoters, CurrentEpoch

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
       IngressProtectedSourcesFor, IngressLaneHasNonTimeoutProgressIn,
       IngressTimeoutVoteProtectedSourcesFor,
       IngressLaneHasTimeoutVoteIn,
       IngressTransportCompletionProtectedSourcesFor,
       IngressLaneHasTransportCompletionIn,
       IngressContinuationProtectedSourcesFor,
       IngressProtectedSlotCountFor, IngressAdmissionClass,
       IngressTransportCompletionKinds, IngressProgressKinds,
       IngressLaneDepth, IngressLane, SequenceSet

THEOREM AsyncIngressContentTypeStutter ==
  AsyncIngressContentTypeInvariant /\ UNCHANGED asyncIngressLanes
    => AsyncIngressContentTypeInvariant'
BY Isa
   DEF AsyncIngressContentTypeInvariant, IngressLaneDepth, IngressLane

THEOREM AsyncSchedulerStateStutterPreservesType ==
  AsyncSchedulerTypeInvariant
    /\ UNCHANGED <<context, AsyncSchedulerVars>>
    /\ AsyncHistoricalRecoveryTypeInvariant'
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
       AsyncTransportContentTypeVars,
       AsyncCertifiedResponseClaimAuthorityVars,
       AsyncIngressTopologyTypeVars,
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
                          asyncRunnerBudget,
                          AsyncLocalAdmissionVars>>
      BY <1>1, Isa DEF AsyncTick, AsyncNonClockVars, vars
    <2>4. AsyncRuntimeScalarTypeInvariant'
      BY <2>1, <2>3, SMT
         DEF AsyncRuntimeScalarTypeInvariant, AsyncLocalAdmissionVars
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
             AsyncCertifiedResponseClaimAuthorityVars,
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
    <2>11. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, HistoricalRecoveryFramePreservesType, Isa
         DEF AsyncSchedulerTypeInvariant, AsyncTick, AsyncNonClockVars,
             AsyncHistoricalRecoveryFrameVars, vars
    <2> QED BY <2>4, <2>6, <2>10, <2>11
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
             AsyncCertifiedResponseClaimAuthorityVars,
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
    <2>16. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, HistoricalRecoveryFramePreservesType, Isa
         DEF AsyncSchedulerTypeInvariant, PreGstLosePacket,
             AsyncHistoricalRecoveryFrameVars, vars
    <2> QED BY <2>9, <2>15, <2>16
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

THEOREM ResponsiveAreValidators ==
  TypeInvariant => Responsive \subseteq ValidatorIds
BY SMT
   DEF TypeInvariant, ModelConfiguration, QuorumConfiguration

THEOREM AsyncResponsiveAppliedArchiveServersAreValidators ==
  AsyncResponsiveAppliedArchiveServers \subseteq ValidatorIds
BY Isa
   DEF AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers, AsyncArchiveServerIds

THEOREM AsyncResponsiveAppliedArchiveServersAreResponsive ==
  AsyncResponsiveAppliedArchiveServers \subseteq Responsive
BY Isa
   DEF AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers

THEOREM AsyncArchiveIoServiceNodesAreValidators ==
  TypeInvariant => AsyncArchiveIoServiceNodes \subseteq ValidatorIds
BY AsyncCurrentResponsiveVotersAreValidators,
   AsyncResponsiveAppliedArchiveServersAreValidators,
   Isa
   DEF AsyncArchiveIoServiceNodes

THEOREM AsyncArchiveIoServiceNodesAreResponsive ==
  AsyncArchiveIoServiceNodes \subseteq Responsive
BY AsyncResponsiveAppliedArchiveServersAreResponsive, Isa
   DEF AsyncArchiveIoServiceNodes, AsyncCurrentResponsiveVoters

THEOREM AsyncTimedServiceNodesAreValidators ==
  AsyncTypeInvariant => AsyncTimedServiceNodes \subseteq ValidatorIds
PROOF
  <1>1. ASSUME AsyncTypeInvariant
         PROVE AsyncTimedServiceNodes \subseteq ValidatorIds
    <2>1. /\ TypeInvariant
           /\ AsyncArchiveIoServiceNodes \subseteq ValidatorIds
           /\ asyncHistoricalRecoveryTargets
                \subseteq Responsive \cap up
      BY <1>1, AsyncArchiveIoServiceNodesAreValidators
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncHistoricalRecoveryTypeInvariant
    <2>2. Responsive \subseteq ValidatorIds
      BY <2>1, ResponsiveAreValidators
    <2> QED BY <2>1, <2>2, Isa DEF AsyncTimedServiceNodes
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

THEOREM AppendOwnedCandidatePreservesCommandQueueOwnership ==
  \A node, queue, candidate:
    /\ AsyncQueueTyped(queue)
    /\ AsyncCommandQueueOwnership(node, queue)
    /\ candidate.node = node
    => AsyncCommandQueueOwnership(node, Append(queue, candidate))
BY SequenceSetAfterAppend, SMT
   DEF AsyncQueueTyped, AsyncCommandQueueOwnership

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

THEOREM AppendFreshServeJobPreservesNonceOwnership ==
  \A queue, job:
    /\ AsyncIoSequenceTyped(queue)
    /\ AsyncIoServeNonceOwnership(queue)
    /\ AsyncIoJobTyped(job)
    /\ job.class = "Serve"
    /\ job.nonce \notin
         {queue[index].nonce: index \in AsyncIoServeIndices(queue)}
    => AsyncIoServeNonceOwnership(Append(queue, job))
BY AppendSequenceFacts, SMTT(30)
   DEF AsyncIoServeNonceOwnership, AsyncIoServeIndices,
       AsyncIoSequenceTyped

THEOREM AppendNonServeJobPreservesNonceOwnership ==
  \A queue, job:
    /\ AsyncIoSequenceTyped(queue)
    /\ AsyncIoServeNonceOwnership(queue)
    /\ job.class # "Serve"
    => AsyncIoServeNonceOwnership(Append(queue, job))
BY AppendSequenceFacts, SMTT(30)
   DEF AsyncIoServeNonceOwnership, AsyncIoServeIndices,
       AsyncIoSequenceTyped

THEOREM TailPreservesServeNonceOwnership ==
  \A queue:
    /\ AsyncIoSequenceTyped(queue)
    /\ AsyncIoServeNonceOwnership(queue)
    /\ Len(queue) > 0
    => AsyncIoServeNonceOwnership(Tail(queue))
BY TypedIoTailFacts, SMTT(30)
   DEF AsyncIoServeNonceOwnership, AsyncIoServeIndices,
       AsyncIoSequenceTyped

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

AsyncIoResponseItemsAfterService(node, queue) ==
  LET job == Head(queue)
  IN IF job.class # "Serve"
     THEN {}
     ELSE IF CertifiedServeCanRespond(node, job.candidate.item)
          THEN {CertifiedResponseItem(
                  AsyncUntrustedSource, node, job.candidate.item)}
          ELSE IF CommitCertificateServeCanRespond(job.candidate.item)
               THEN CommitCertificateResponseItems(job.candidate.item)
               ELSE {}

THEOREM CertifiedResponseItemIsTyped ==
  \A via, archiveServer, request:
    /\ AsyncConfiguration
    /\ via \in AsyncIngressSources
    /\ archiveServer \in AsyncArchiveServerIds
    /\ AsyncItemTyped(request)
    /\ request.kind = "CertifiedRequest"
    => AsyncItemTyped(
         CertifiedResponseItem(via, archiveServer, request))
BY SMTT(30)
   DEF AsyncConfiguration, AsyncItemTyped, CertifiedResponseItem,
       AsyncNetworkItem, AsyncCertifiedResponseEnvelope,
       AsyncCertifiedResponseEnvelopeTyped, AsyncReplyRequestItemTyped,
       AsyncBodyEnvelopeTyped, AsyncNetworkKinds, AsyncIngressSources,
       AsyncArchiveServerIds, ValidatorIds,
       AsyncHeartbeatSubject, NoAsyncChunk,
       AsyncCertifiedCitedResponder, AsyncCertifiedRequestHash,
       AsyncCertifiedRequestHashes, AsyncCertifiedRequestItems,
       AsyncCertifiedRequestEnvelope

THEOREM CommitCertificateResponseItemIsTyped ==
  \A request, qc:
    /\ AsyncItemTyped(request)
    /\ request.kind = "CommitCertificateRequest"
    /\ qc \in QcRecordSet
    => AsyncItemTyped(CommitCertificateResponseItem(request, qc))
BY SMTT(60)
   DEF AsyncItemTyped, CommitCertificateResponseItem,
       AsyncNetworkItem, AsyncCommitCertificateResponseEnvelope,
       AsyncCommitCertificateResponseEnvelopeTyped,
       AsyncReplyRequestItemTyped, AsyncBodyEnvelopeTyped,
       AsyncNetworkKinds, AsyncIngressSources, ValidatorIds

THEOREM RuntimeCurrentEpochIsTyped ==
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
      BY <2>1, <2>3 DEF ContextRecord, ModelConfiguration
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

THEOREM RuntimeCurrentVotersAreFiniteValidators ==
  TypeInvariant
    => /\ IsFiniteSet(CurrentVoters)
       /\ CurrentVoters \subseteq ValidatorIds
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE /\ IsFiniteSet(CurrentVoters)
               /\ CurrentVoters \subseteq ValidatorIds
    <2>1. /\ QuorumConfiguration
           /\ CurrentEpoch \in Epochs
      BY <1>1, RuntimeCurrentEpochIsTyped
         DEF TypeInvariant, ModelConfiguration
    <2> QED BY <2>1 DEF QuorumConfiguration, CurrentVoters,
                             VotingRoster
  <1> QED BY <1>1

THEOREM HistoricalResponseItemsSeparateRotatedHopFromExactRequest ==
  \A bodyRequest, certificateRequest, qc, archiveServer:
    /\ TypeInvariant
    /\ AsyncItemTyped(bodyRequest)
    /\ bodyRequest.kind = "CertifiedRequest"
    /\ AsyncItemTyped(certificateRequest)
    /\ certificateRequest.kind = "CommitCertificateRequest"
    /\ certificateRequest.envelope.recipient = archiveServer
    /\ qc \in QcRecordSet
    /\ archiveServer \in AsyncArchiveServerIds
    => LET bodyResponse ==
             CertifiedResponseItem(
               AsyncUntrustedSource, archiveServer, bodyRequest)
           certificateResponse ==
             CommitCertificateResponseItem(certificateRequest, qc)
       IN /\ bodyResponse.source = AsyncUntrustedSource
          /\ certificateResponse.source = archiveServer
          /\ bodyResponse.source # certificateResponse.source
          /\ AsyncUntrustedSource \in AsyncIngressSources
          /\ AsyncUntrustedSource \notin CurrentVoters
          /\ bodyResponse.envelope.requestHash =
               AsyncCertifiedRequestHash(bodyRequest)
          /\ bodyResponse.envelope.archiveServer = archiveServer
          /\ bodyResponse.envelope.signatureOwner = archiveServer
          /\ certificateResponse.envelope.request = certificateRequest
PROOF
  <1>1. ASSUME NEW bodyRequest, NEW certificateRequest,
                NEW qc, NEW archiveServer,
                TypeInvariant,
                AsyncItemTyped(bodyRequest),
                bodyRequest.kind = "CertifiedRequest",
                AsyncItemTyped(certificateRequest),
                certificateRequest.kind = "CommitCertificateRequest",
                certificateRequest.envelope.recipient = archiveServer,
                qc \in QcRecordSet,
                archiveServer \in AsyncArchiveServerIds
         PROVE LET bodyResponse ==
                       CertifiedResponseItem(
                         AsyncUntrustedSource, archiveServer, bodyRequest)
                   certificateResponse ==
                     CommitCertificateResponseItem(certificateRequest, qc)
               IN /\ bodyResponse.source = AsyncUntrustedSource
                  /\ certificateResponse.source = archiveServer
                  /\ bodyResponse.source # certificateResponse.source
                  /\ AsyncUntrustedSource \in AsyncIngressSources
                  /\ AsyncUntrustedSource \notin CurrentVoters
                  /\ bodyResponse.envelope.requestHash =
                       AsyncCertifiedRequestHash(bodyRequest)
                  /\ bodyResponse.envelope.archiveServer = archiveServer
                  /\ bodyResponse.envelope.signatureOwner = archiveServer
                  /\ certificateResponse.envelope.request = certificateRequest
    <2>1. CurrentVoters \subseteq ValidatorIds
      BY <1>1, RuntimeCurrentVotersAreFiniteValidators
    <2>2. AsyncUntrustedSource \notin ValidatorIds
      BY SMT DEF AsyncUntrustedSource, ValidatorIds
    <2> QED BY <2>1, <2>2, SMT
         DEF CertifiedResponseItem, CommitCertificateResponseItem,
             AsyncCertifiedResponseEnvelope,
             AsyncCommitCertificateResponseEnvelope, AsyncNetworkItem,
             AsyncIngressSources, AsyncArchiveServerIds,
             AsyncCertifiedRequestHash
  <1> QED BY <1>1

THEOREM CertifiedResponseAuthenticationProjectionIsViaIndependent ==
  \A leftVia, rightVia, archiveServer, request:
    AsyncCertifiedResponseAuthProjection(
      CertifiedResponseItem(leftVia, archiveServer, request))
      =
    AsyncCertifiedResponseAuthProjection(
      CertifiedResponseItem(rightVia, archiveServer, request))
BY DEF AsyncCertifiedResponseAuthProjection, CertifiedResponseItem,
       AsyncCertifiedResponseEnvelope, AsyncNetworkItem

THEOREM SentCertifiedResponseAuthenticatesEveryRelayOccurrence ==
  \A sentVia, relayVia, archiveServer, request:
    CertifiedResponseItem(sentVia, archiveServer, request)
      \in asyncSentItems
      => CertifiedResponseAuthenticatedOccurrence(
           CertifiedResponseItem(relayVia, archiveServer, request))
BY SMT
   DEF CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection, CertifiedResponseItem,
       AsyncCertifiedResponseEnvelope, AsyncNetworkItem

THEOREM ExactOutstandingCommitCertificateResponseIsAuthorized ==
  \A request, qc:
    /\ request \in asyncActiveRequests
    /\ CommitCertificateRequestAuthorized(request)
    /\ qc \in commitQCs
    /\ qc.context = context
    /\ qc.phase = "Commit"
    => CommitCertificateResponseAuthorized(
         CommitCertificateResponseItem(request, qc))
BY SMT
   DEF CommitCertificateResponseAuthorized,
       MatchingCommitCertificateRequests,
       CommitCertificateResponseItem,
       AsyncCommitCertificateResponseEnvelope, AsyncNetworkItem,
       AsyncIngressSources, AsyncUntrustedSource

THEOREM ExactOutstandingCertifiedBodyResponseIsAuthorized ==
  \A via, archiveServer, request:
    /\ request \in asyncActiveRequests
    /\ FrozenCertifiedRequestRegistration(request)
    /\ archiveServer \in AsyncArchiveServerIds
    /\ AsyncCertifiedCitedResponder(request)
         \in request.envelope.certificate.signers
    /\ CertifiedResponseAuthenticatedOccurrence(
         CertifiedResponseItem(via, archiveServer, request))
    => CertifiedResponseAuthorized(
         CertifiedResponseItem(via, archiveServer, request))
BY SMT
   DEF CertifiedResponseAuthorized, MatchingCertifiedRequests,
       CertifiedResponseItem, AsyncCertifiedResponseEnvelope,
       AsyncNetworkItem, CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection, AsyncCertifiedRequestHash,
       FrozenCertifiedRequestRegistration,
       FrozenCertifiedResponseBinding

THEOREM CommitCertificateResponseRejectsWrongContextOrPhase ==
  \A item:
    /\ item.kind = "CommitCertificateResponse"
    /\ \/ item.envelope.qc.context # context
       \/ item.envelope.qc.phase # "Commit"
    => ~CommitCertificateResponseAuthorized(item)
BY DEF CommitCertificateResponseAuthorized

THEOREM StrongInvariantTypesAppliedCertificates ==
  StrongInductiveInvariant
    => \A application \in applied: application.qc \in QcRecordSet
BY SMT
   DEF StrongInductiveInvariant, Safety, TypeInvariant,
       DecisionAgreement, AppliedRequiresDecision

THEOREM ServiceResponseItemsAreFiniteAndTyped ==
  \A node \in AsyncArchiveIoServiceNodes:
    /\ AsyncTypeInvariant
    /\ StrongInductiveInvariant
    /\ AsyncIoQueueDepth(node) > 0
    => /\ IsFiniteSet(
             AsyncIoResponseItemsAfterService(
               node, asyncIoQueues[node]))
       /\ \A item \in
              AsyncIoResponseItemsAfterService(
                node, asyncIoQueues[node]):
            AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW node \in AsyncArchiveIoServiceNodes,
                AsyncTypeInvariant,
                StrongInductiveInvariant,
                AsyncIoQueueDepth(node) > 0
         PROVE /\ IsFiniteSet(
                      AsyncIoResponseItemsAfterService(
                        node, asyncIoQueues[node]))
               /\ \A item \in
                      AsyncIoResponseItemsAfterService(
                        node, asyncIoQueues[node]):
                    AsyncItemTyped(item)
    <2>1. /\ AsyncConfiguration
           /\ node \in ValidatorIds
           /\ AsyncIoSequenceTyped(asyncIoQueues[node])
           /\ AsyncIoJobTyped(Head(asyncIoQueues[node]))
      BY <1>1, AsyncArchiveIoServiceNodesAreValidators,
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
                       node, Head(asyncIoQueues[node]).candidate.item)
      <3>1. AsyncItemTyped(
               CertifiedResponseItem(
                 AsyncUntrustedSource, node,
                 Head(asyncIoQueues[node]).candidate.item))
        BY <2>1, <2>2, <2>5, CertifiedResponseItemIsTyped
           DEF CertifiedServeCanRespond
      <3>2. AsyncIoResponseItemsAfterService(
               node, asyncIoQueues[node]) =
               {CertifiedResponseItem(
                  AsyncUntrustedSource, node,
                  Head(asyncIoQueues[node]).candidate.item)}
        BY <2>5 DEF AsyncIoResponseItemsAfterService
      <3> QED BY <3>1, <3>2, FS_Singleton
    <2>6. CASE /\ Head(asyncIoQueues[node]).class = "Serve"
                  /\ ~CertifiedServeCanRespond(
                       node, Head(asyncIoQueues[node]).candidate.item)
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
      <3>4. AsyncIoResponseItemsAfterService(
               node, asyncIoQueues[node]) =
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
                       node, Head(asyncIoQueues[node]).candidate.item)
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
             /\ {item.envelope.recipient: item \in retainedClass} =
                  ControlRecipients(source, controlClass, voters)
             /\ \A left, right \in retainedClass:
                  ControlView(left) = ControlView(right)

AsyncActiveRequestsType(active, sent) ==
  /\ IsFiniteSet(active)
  /\ active \subseteq sent
  /\ \A item \in active:
       /\ AsyncItemTyped(item)
       /\ item.kind \in {"CertifiedRequest",
                          "CommitCertificateRequest"}
  /\ AsyncCertifiedRequestLogicalIndexConsistent(active)

THEOREM CertifiedRequestLogicalIndexConsistencyIsDownwardClosed ==
  \A existing, remaining:
    /\ remaining \subseteq existing
    /\ AsyncCertifiedRequestLogicalIndexConsistent(existing)
    => AsyncCertifiedRequestLogicalIndexConsistent(remaining)
BY SMT
   DEF AsyncCertifiedRequestLogicalIndexConsistent,
       AsyncCertifiedRequestsIn

THEOREM CompatibleCertifiedRequestUnionIsLogicallyConsistent ==
  \A existing, incoming:
    /\ AsyncCertifiedRequestLogicalIndexConsistent(existing)
    /\ AsyncCertifiedRequestLogicalIndexConsistent(incoming)
    /\ AsyncCertifiedRequestSetsCompatible(existing, incoming)
    => AsyncCertifiedRequestLogicalIndexConsistent(
         existing \cup incoming)
BY SMT
   DEF AsyncCertifiedRequestLogicalIndexConsistent,
       AsyncCertifiedRequestSetsCompatible,
       AsyncCertifiedRequestsIn

THEOREM AsyncTransportHistoryTypeDecomposition ==
  AsyncTransportHistoryTypeInvariant
    <=> /\ AsyncSentItemsType(asyncSentItems)
        /\ AsyncRetainedControlType(
             asyncRetainedControl, CurrentVoters)
        /\ AsyncActiveRequestsType(
             asyncActiveRequests, asyncSentItems)
        /\ AsyncCertifiedResponseClaimInvariant
BY DEF AsyncTransportHistoryTypeInvariant, AsyncSentItemsType,
       AsyncRetainedControlType, AsyncActiveRequestsType

THEOREM AppendSentHistoryPreservesCertifiedResponseClaimInvariant ==
  \A items:
    /\ AsyncCertifiedResponseClaimInvariant
    /\ asyncSentItems' = asyncSentItems \cup items
    /\ AsyncCertifiedRequestsIn(asyncActiveRequests') =
         AsyncCertifiedRequestsIn(asyncActiveRequests)
    /\ UNCHANGED
         <<AsyncCertifiedResponseClaimCoreAuthorityVars,
           asyncCertifiedResponseClaim>>
    => AsyncCertifiedResponseClaimInvariant'
BY SMTT(30), Isa
   DEF AsyncCertifiedResponseClaimInvariant,
       AsyncCertifiedResponseClaimCoreAuthorityVars,
       CertifiedResponseClaimProjectionAuthenticated,
       CertifiedResponseAuthorized,
       CertifiedResponseAuthenticatedOccurrence,
       MatchingCertifiedRequests, FrozenCertifiedRequestRegistration,
       FrozenCertifiedResponseBinding,
       ActiveCertifiedRequestHashes,
       ActiveCertifiedRequestHashesIn,
       AsyncCertifiedRequestsIn,
       CertifiedResponseAuthorityReady,
       CertifiedResponseAuthorityClaimed

THEOREM EmptyCertifiedResponseClaimIsInvariant ==
  asyncCertifiedResponseClaim = {}
    => AsyncCertifiedResponseClaimInvariant
BY FS_EmptySet, SMT, Isa
   DEF AsyncCertifiedResponseClaimInvariant,
       ActiveCertifiedRequestHashes,
       ActiveCertifiedRequestHashesIn,
       CertifiedResponseAuthorityReady,
       CertifiedResponseAuthorityClaimed

THEOREM ExtendActiveRequestsPreservesCertifiedResponseClaimInvariant ==
  \A items:
    /\ AsyncCertifiedResponseClaimInvariant
    /\ asyncSentItems' = asyncSentItems \cup items
    /\ asyncActiveRequests' = asyncActiveRequests \cup items
    /\ UNCHANGED
         <<AsyncCertifiedResponseClaimCoreAuthorityVars,
           asyncCertifiedResponseClaim>>
    => AsyncCertifiedResponseClaimInvariant'
BY SMTT(30), Isa
   DEF AsyncCertifiedResponseClaimInvariant,
       AsyncCertifiedResponseClaimCoreAuthorityVars,
       CertifiedResponseClaimProjectionAuthenticated,
       CertifiedResponseAuthorized,
       CertifiedResponseAuthenticatedOccurrence,
       MatchingCertifiedRequests, FrozenCertifiedRequestRegistration,
       FrozenCertifiedResponseBinding,
       ActiveCertifiedRequestHashes,
       ActiveCertifiedRequestHashesIn,
       CertifiedResponseAuthorityReady,
       CertifiedResponseAuthorityClaimed

THEOREM FilterActiveRequestsAndClaimPreservesInvariant ==
  \A additions:
    /\ AsyncCertifiedResponseClaimInvariant
    /\ asyncSentItems' = asyncSentItems \cup additions
    /\ asyncActiveRequests' \subseteq asyncActiveRequests
    /\ asyncCertifiedResponseClaim' =
         CertifiedResponseClaimForRequests(asyncActiveRequests')
    => AsyncCertifiedResponseClaimInvariant'
BY SMTT(45), Isa
   DEF AsyncCertifiedResponseClaimInvariant,
       CertifiedResponseClaimForRequests,
       CertifiedResponseClaimProjectionAuthenticated,
       CertifiedResponseAuthenticatedOccurrence,
       MatchingCertifiedRequests, FrozenCertifiedRequestRegistration,
       FrozenCertifiedResponseBinding,
       ActiveCertifiedRequestHashes,
       ActiveCertifiedRequestHashesIn,
       AsyncCertifiedRequestHash,
       CertifiedResponseAuthorityReady,
       CertifiedResponseAuthorityClaimed

THEOREM MatchingClaimedCertifiedResponseIsAuthorized ==
  \A item:
    /\ AsyncCertifiedResponseClaimInvariant
    /\ item.kind = "CertifiedResponse"
    /\ CertifiedResponseClaimMatches(item)
    => CertifiedResponseClaimAuthorized(item)
BY SMTT(45), Isa
   DEF AsyncCertifiedResponseClaimInvariant,
       CertifiedResponseClaimMatches,
       CertifiedResponseClaimAuthorized,
       CertifiedResponseClaimProjectionAuthenticated,
       CertifiedResponseAuthenticatedOccurrence,
       MatchingCertifiedRequests, FrozenCertifiedRequestRegistration,
       FrozenCertifiedResponseBinding,
       AsyncCertifiedResponseCanonicalWireIdentity,
       ActiveCertifiedRequestHashes,
       ActiveCertifiedRequestHashesIn,
       CertifiedResponseAuthorityReady,
       CertifiedResponseAuthorityClaimed

THEOREM FrozenCertifiedResponseBindingMakesSentRequestMatch ==
  \A item, request:
    FrozenCertifiedResponseBinding(item, request)
      => request \in MatchingSentCertifiedRequests(item)
BY Isa
   DEF FrozenCertifiedResponseBinding,
       FrozenCertifiedRequestRegistration,
       MatchingSentCertifiedRequests

THEOREM CertifiedResponseAuthorizationSuppliesFrozenCapability ==
  \A item:
    CertifiedResponseAuthorized(item)
      => CertifiedResponseCapabilityAuthorized(item)
BY FrozenCertifiedResponseBindingMakesSentRequestMatch, Isa
   DEF CertifiedResponseAuthorized,
       CertifiedResponseCapabilityAuthorized

THEOREM CertifiedResponseClaimAuthorizationSuppliesFrozenCapability ==
  \A item:
    CertifiedResponseClaimAuthorized(item)
      => CertifiedResponseCapabilityAuthorized(item)
BY FrozenCertifiedResponseBindingMakesSentRequestMatch, Isa
   DEF CertifiedResponseClaimAuthorized,
       CertifiedResponseCapabilityAuthorized

THEOREM PacketForTypedItemIsTyped ==
  \A item:
    /\ AsyncConfiguration
    /\ asyncNow \in Nat
    /\ AsyncItemTyped(item)
    => AsyncPacketTyped(PacketForItem(item))
BY SMT
   DEF AsyncConfiguration, AsyncPacketTyped, PacketForItem, AsyncPacket

THEOREM PacketForItemHasStrictlyFutureDeadline ==
  \A item:
    /\ AsyncConfiguration
    /\ asyncNow \in Nat
    => PacketForItem(item).deadline > asyncNow
BY SMT DEF AsyncConfiguration, PacketForItem, AsyncPacket

THEOREM PacketsForItemsHaveStrictlyFutureDeadlines ==
  \A items:
    /\ AsyncConfiguration
    /\ asyncNow \in Nat
    => \A packet \in PacketsForItems(items):
         packet.deadline > asyncNow
PROOF
  <1>1. ASSUME NEW items,
                AsyncConfiguration,
                asyncNow \in Nat
         PROVE \A packet \in PacketsForItems(items):
                 packet.deadline > asyncNow
    <2>1. ASSUME NEW packet \in PacketsForItems(items)
           PROVE packet.deadline > asyncNow
      <3>1. PICK item \in items:
               packet = PacketForItem(item)
        BY <2>1 DEF PacketsForItems
      <3> QED BY <1>1, <3>1,
                   PacketForItemHasStrictlyFutureDeadline
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM PacketPublicationPreservesCurrentDueSourcePrefix ==
  \A items, recipient, source:
    /\ AsyncConfiguration
    /\ asyncNow \in Nat
    /\ asyncTransport' = asyncTransport \cup PacketsForItems(items)
    /\ UNCHANGED asyncNow
    => DueSourcePackets(recipient, source)' =
         DueSourcePackets(recipient, source)
BY PacketsForItemsHaveStrictlyFutureDeadlines, Isa
   DEF DueSourcePackets

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
                 packet = PacketForItem(item)
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
    /\ UNCHANGED
         <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
           asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW items,
                AsyncRuntimeScalarTypeInvariant,
                AsyncTransportContentTypeInvariant,
                IsFiniteSet(items),
                \A item \in items: AsyncItemTyped(item),
                PublishEphemeralItems(items),
                UNCHANGED
                  <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
                    asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. /\ AsyncSentItemsType(asyncSentItems)
           /\ AsyncRetainedControlType(
                asyncRetainedControl, CurrentVoters)
           /\ AsyncActiveRequestsType(
                asyncActiveRequests, asyncSentItems)
           /\ AsyncCertifiedResponseClaimInvariant
           /\ AsyncPacketContentTypeInvariant
           /\ AsyncHeldChunksTypeInvariant
      BY <1>1, AsyncTransportHistoryTypeDecomposition
         DEF AsyncTransportContentTypeInvariant
    <2>2. /\ asyncSentItems' = asyncSentItems \cup items
           /\ asyncRetainedControl' = asyncRetainedControl
           /\ asyncActiveRequests' = asyncActiveRequests
           /\ asyncCertifiedResponseClaim' =
                asyncCertifiedResponseClaim
           /\ asyncTransport' =
                asyncTransport \cup PacketsForItems(items)
           /\ UNCHANGED
                AsyncCertifiedResponseClaimCoreAuthorityVars
           /\ asyncHeldChunks' = asyncHeldChunks
      BY <1>1
         DEF PublishEphemeralItems,
             AsyncCertifiedResponseClaimCoreAuthorityVars
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
    <2>6a. AsyncCertifiedResponseClaimInvariant'
      BY <2>1, <2>2,
         AppendSentHistoryPreservesCertifiedResponseClaimInvariant
    <2>7. AsyncTransportHistoryTypeInvariant'
      BY <2>3, <2>5, <2>6, <2>6a
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
  \A node \in ValidatorIds:
    AsyncTypeInvariant /\ EnqueueIoLocalControlWork(node)
      => AsyncIoTopologyTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                EnqueueIoLocalControlWork(node)
         PROVE AsyncIoTopologyTypeInvariant'
    <2>1. AsyncIoTopologyTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant
    <2>2. node \in ValidatorIds
      BY <1>1
    <2>3. asyncIoControlAvailable'
               \in [ValidatorIds -> BOOLEAN]
      BY <1>1, <2>1, <2>2, FunctionalUpdatePreservesType
         DEF EnqueueIoLocalControlWork, AsyncIoTopologyTypeInvariant
    <2> QED BY <1>1, <2>1, <2>3, Isa
         DEF EnqueueIoLocalControlWork, AsyncIoTopologyTypeInvariant,
             AsyncDeferredVars, LeaveCausalQueues
  <1> QED BY <1>1

THEOREM EnqueueIoControlPreservesContentType ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant /\ EnqueueIoLocalControlWork(node)
      => AsyncIoContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                EnqueueIoLocalControlWork(node)
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
      BY <1>1
    <2>4. ASSUME NEW other \in ValidatorIds
           PROVE AsyncIoSequenceTyped(asyncIoQueues'[other])
      <3>1. CASE other = node
        <4>1. AsyncIoSequenceTyped(asyncIoQueues[node])
          BY <2>1, <3>1 DEF AsyncIoQueueContentTypeInvariant
        <4>2. asyncIoQueues'[other] =
                 Append(asyncIoQueues[node], AsyncIoControlJob)
          BY <1>1, <2>1, <2>3, <3>1,
             FunctionalAppendUpdateAtKey
             DEF EnqueueIoLocalControlWork, AsyncIoTopologyTypeInvariant
        <4> QED BY <2>2, <4>1, <4>2,
                     TypedIoAppendPreservesSequenceType
      <3>2. CASE other # node
        <4>1. asyncIoQueues'[other] = asyncIoQueues[other]
          BY <1>1, <2>1, <2>4, <3>2,
             FunctionalAppendUpdateAwayFromKey
             DEF EnqueueIoLocalControlWork, AsyncIoTopologyTypeInvariant
        <4>2. AsyncIoSequenceTyped(asyncIoQueues[other])
          BY <2>1, <2>4 DEF AsyncIoQueueContentTypeInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1, <3>2
    <2>4s. ASSUME NEW other \in ValidatorIds
            PROVE AsyncIoServeNonceOwnership(asyncIoQueues'[other])
      <3>1. CASE other = node
        <4>1. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
               /\ AsyncIoServeNonceOwnership(asyncIoQueues[node])
          BY <2>1 DEF AsyncIoQueueContentTypeInvariant
        <4>2. asyncIoQueues'[other] =
                 Append(asyncIoQueues[node], AsyncIoControlJob)
          BY <1>1, <2>1, <2>3, <3>1,
             FunctionalAppendUpdateAtKey
             DEF EnqueueIoLocalControlWork, AsyncIoTopologyTypeInvariant
        <4>3. AsyncIoControlJob.class # "Serve"
          BY DEF AsyncIoControlJob, AsyncIoJob
        <4> QED BY <4>1, <4>2, <4>3,
             AppendNonServeJobPreservesNonceOwnership
      <3>2. CASE other # node
        BY <1>1, <2>1, <2>3, <2>4s, <3>2,
           FunctionalAppendUpdateAwayFromKey
           DEF EnqueueIoLocalControlWork, AsyncIoTopologyTypeInvariant,
               AsyncIoQueueContentTypeInvariant
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
             DEF EnqueueIoLocalControlWork, AsyncIoTopologyTypeInvariant
        <4>3. SequenceSet(asyncIoQueues'[node]) =
                 SequenceSet(asyncIoQueues[node]) \cup {AsyncIoControlJob}
          BY <4>1, <4>2, SequenceSetAfterAppend
        <4>4. asyncOutstandingWork'[node] = asyncOutstandingWork[node]
          BY <1>1 DEF EnqueueIoLocalControlWork
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
             DEF EnqueueIoLocalControlWork, AsyncIoTopologyTypeInvariant
        <4>2. asyncOutstandingWork'[other] =
                 asyncOutstandingWork[other]
          BY <1>1 DEF EnqueueIoLocalControlWork
        <4> QED BY <2>1, <2>5, <4>1, <4>2
             DEF AsyncIoQueueContentTypeInvariant
      <3> QED BY <3>1, <3>2
    <2>6. UNCHANGED
             <<asyncIoReadyCompletions, asyncLocalReadyCompletions>>
      BY <1>1 DEF EnqueueIoLocalControlWork
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
             DEF EnqueueIoLocalControlWork, AsyncIoTopologyTypeInvariant
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
             DEF EnqueueIoLocalControlWork, AsyncIoTopologyTypeInvariant
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
      BY <2>4, <2>4s, <2>5, <2>7
         DEF AsyncIoQueueContentTypeInvariant
    <2>9. UNCHANGED AsyncIoWorkContentTypeVars
      BY <1>1, Isa
         DEF EnqueueIoLocalControlWork, AsyncIoWorkContentTypeVars,
             AsyncDeferredVars, LeaveCausalQueues
    <2>10. AsyncIoWorkContentTypeInvariant'
      BY <2>1, <2>9, AsyncIoWorkContentTypeStutter
    <2> QED BY <2>8, <2>10 DEF AsyncIoContentTypeInvariant
  <1> QED BY <1>1

THEOREM EnqueueIoControlPreservesCapacityType ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant /\ EnqueueIoLocalControlWork(node)
      => AsyncIoCapacityTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                EnqueueIoLocalControlWork(node)
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
      BY <1>1
    <2>3. ASSUME NEW other \in ValidatorIds
           PROVE /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
                 /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
                 /\ AsyncOutstandingWorkCount(other)' <=
                      AsyncIoWorkCapacity
      <3>1. CASE other = node
        <4>1. asyncIoQueues[node] \in Seq(Range(asyncIoQueues[node]))
          BY <2>1, <2>2
             DEF AsyncIoQueueContentTypeInvariant, AsyncIoSequenceTyped
        <4>2. asyncIoQueues'[node] =
                 Append(asyncIoQueues[node], AsyncIoControlJob)
          BY <1>1, <2>1, <2>2, FunctionalAppendUpdateAtKey
             DEF EnqueueIoLocalControlWork, AsyncIoTopologyTypeInvariant
        <4>3. Len(asyncIoQueues'[node]) =
                 Len(asyncIoQueues[node]) + 1
          BY <4>1, <4>2, AppendSequenceFacts
        <4>4. Len(asyncIoQueues[node]) < AsyncIoCapacity
          BY <1>1
             DEF EnqueueIoLocalControlWork, CanEnqueueIoClass,
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
              /\ AsyncOutstandingWorkCount(node)' =
                   AsyncOutstandingWorkCount(node)
          BY <1>1, Isa
             DEF EnqueueIoLocalControlWork, AsyncDeferredVars,
                 AsyncQueueDepth, AsyncOutstandingWorkCount
        <4>10. /\ AsyncQueueDepth(node) <= AsyncQueueCapacity
              /\ AsyncOutstandingWorkCount(node) <= AsyncIoWorkCapacity
          BY <2>1, <2>2 DEF AsyncIoCapacityTypeInvariant
        <4> QED BY <3>1, <4>8, <4>9, <4>10
      <3>2. CASE other # node
        <4>1. asyncIoQueues'[other] = asyncIoQueues[other]
          BY <1>1, <2>1, <2>3, <3>2,
             FunctionalAppendUpdateAwayFromKey
             DEF EnqueueIoLocalControlWork, AsyncIoTopologyTypeInvariant
        <4>2. /\ AsyncQueueDepth(other)' = AsyncQueueDepth(other)
              /\ AsyncOutstandingWorkCount(other)' =
                   AsyncOutstandingWorkCount(other)
              /\ AsyncIoQueueDepth(other)' = AsyncIoQueueDepth(other)
          BY <1>1, <4>1, Isa
             DEF EnqueueIoLocalControlWork, AsyncDeferredVars,
                 AsyncQueueDepth, AsyncIoQueueDepth,
                 AsyncOutstandingWorkCount
        <4>3. /\ AsyncQueueDepth(other) <= AsyncQueueCapacity
              /\ AsyncIoQueueDepth(other) <= AsyncIoCapacity
              /\ AsyncOutstandingWorkCount(other) <=
                   AsyncIoWorkCapacity
          BY <2>1, <2>3 DEF AsyncIoCapacityTypeInvariant
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>3 DEF AsyncIoCapacityTypeInvariant
  <1> QED BY <1>1

THEOREM EnqueueIoControlPreservesNonIoType ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant /\ EnqueueIoLocalControlWork(node)
      => /\ AsyncRuntimeTypeInvariant'
         /\ AsyncDeferredTypeInvariant'
         /\ AsyncTransportTypeInvariant'
         /\ AsyncIngressTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                EnqueueIoLocalControlWork(node)
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
         DEF EnqueueIoLocalControlWork, AsyncRuntimeScalarTypeVars,
             AsyncDeferredVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportClockTypeVars, AsyncTransportContentTypeVars,
             AsyncCertifiedResponseClaimAuthorityVars,
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
  \A node \in ValidatorIds:
    AsyncTypeInvariant /\ EnqueueIoLocalControlWork(node)
      => AsyncSchedulerTypeInvariant'
BY EnqueueIoControlPreservesTopologyType,
   EnqueueIoControlPreservesContentType,
   EnqueueIoControlPreservesCapacityType,
   EnqueueIoControlPreservesNonIoType,
   HistoricalRecoveryFramePreservesType, Isa
   DEF AsyncSchedulerTypeInvariant, AsyncIoTypeInvariant,
       EnqueueIoLocalControlWork, AsyncHistoricalRecoveryFrameVars,
       vars

THEOREM ServiceIoWorkerPreservesTopologyType ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant /\ ServiceIoWorkerWork(node)
      => AsyncIoTopologyTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                ServiceIoWorkerWork(node)
         PROVE AsyncIoTopologyTypeInvariant'
    <2>1. AsyncIoTopologyTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant
    <2>2. node \in ValidatorIds
      BY <1>1
    <2>3. asyncIoControlAvailable'
               \in [ValidatorIds -> BOOLEAN]
      BY <1>1, <2>1, <2>2, FunctionalUpdatePreservesType, Isa
         DEF ServiceIoWorkerWork, AsyncIoTopologyTypeInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3, Isa
         DEF ServiceIoWorkerWork, AsyncIoTopologyTypeInvariant,
             AsyncDeferredVars, LeaveCausalQueues
  <1> QED BY <1>1

THEOREM ServiceIoWorkerPreservesContentType ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant /\ ServiceIoWorkerWork(node)
      => AsyncIoContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                ServiceIoWorkerWork(node)
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
      BY <1>1
    <2>3. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
           /\ AsyncIoServeNonceOwnership(asyncIoQueues[node])
           /\ Len(asyncIoQueues[node]) > 0
           /\ AsyncIoJobTyped(Head(asyncIoQueues[node]))
           /\ AsyncIoConsensusCandidateOwnership(
                node, asyncIoQueues, asyncIoReadyCompletions,
                asyncLocalReadyCompletions)
      BY <1>1, <2>1, <2>2, TypedIoTailFacts
         DEF ServiceIoWorkerWork, AsyncIoQueueDepth,
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
         DEF ServiceIoWorkerWork, AsyncIoReadyAfterService,
             AsyncIoTopologyTypeInvariant, AsyncDeferredVars,
             LeaveCausalQueues, vars
    <2>6. /\ AsyncIoSequenceTyped(asyncIoQueues'[node])
           /\ SequenceSet(asyncIoQueues'[node]) \subseteq
                SequenceSet(asyncIoQueues[node])
      BY <2>3, <2>5, TypedIoTailFacts
    <2>6s. AsyncIoServeNonceOwnership(asyncIoQueues'[node])
      BY <2>3, <2>5, TailPreservesServeNonceOwnership
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
                 /\ AsyncIoServeNonceOwnership(asyncIoQueues'[other])
                 /\ \A job \in SequenceSet(asyncIoQueues'[other]):
                      job.class = "Consensus" =>
                        job.candidate \in asyncOutstandingWork'[other]
                 /\ AsyncIoConsensusCandidateOwnership(
                      other, asyncIoQueues',
                      asyncIoReadyCompletions',
                      asyncLocalReadyCompletions')
      <3>1. CASE other = node
        BY <3>1, <2>6, <2>6s, <2>7, <2>8
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
             DEF ServiceIoWorkerWork, AsyncIoTopologyTypeInvariant,
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
             DEF ServiceIoWorkerWork, AsyncIoTopologyTypeInvariant,
                 AsyncDeferredVars, LeaveCausalQueues, vars
        <4> QED BY <2>1, <2>13, <4>1
             DEF AsyncIoWorkContentTypeInvariant
      <3> QED BY <3>1, <3>2
    <2>14. AsyncIoWorkContentTypeInvariant'
      BY <2>13 DEF AsyncIoWorkContentTypeInvariant
    <2> QED BY <2>10, <2>14 DEF AsyncIoContentTypeInvariant
  <1> QED BY <1>1

THEOREM ServiceIoWorkerPreservesCapacityType ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant /\ ServiceIoWorkerWork(node)
      => AsyncIoCapacityTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                ServiceIoWorkerWork(node)
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
      BY <1>1
    <2>3. /\ asyncCommandQueues' = asyncCommandQueues
           /\ asyncOutstandingWork' = asyncOutstandingWork
           /\ asyncDeferredCompletionQueues' =
                asyncDeferredCompletionQueues
      BY <1>1, Isa
         DEF ServiceIoWorkerWork, AsyncDeferredVars,
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
             DEF ServiceIoWorkerWork, AsyncIoQueueDepth,
                 AsyncIoQueueContentTypeInvariant
        <4>2. asyncIoQueues'[node] = Tail(asyncIoQueues[node])
          BY <1>1, <2>1, <2>2, FunctionalTailUpdateAtKey
             DEF ServiceIoWorkerWork, AsyncIoTopologyTypeInvariant
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
             DEF ServiceIoWorkerWork, AsyncIoTopologyTypeInvariant
        <4>2. AsyncIoQueueDepth(other) <= AsyncIoCapacity
          BY <2>1, <2>5 DEF AsyncIoCapacityTypeInvariant
        <4> QED BY <4>1, <4>2 DEF AsyncIoQueueDepth
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1, <2>4, <2>5
         DEF AsyncIoCapacityTypeInvariant
  <1> QED BY <1>1

THEOREM ServiceIoWorkerPreservesNonIoType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ StrongInductiveInvariant
    /\ ServiceIoWorkerWork(node)
    => /\ AsyncRuntimeTypeInvariant'
       /\ AsyncDeferredTypeInvariant'
       /\ AsyncTransportTypeInvariant'
       /\ AsyncIngressTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                StrongInductiveInvariant,
                ServiceIoWorkerWork(node)
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
      BY <1>1
    <2>3. /\ UNCHANGED AsyncRuntimeScalarTypeVars
           /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, Isa
         DEF ServiceIoWorkerWork, AsyncRuntimeScalarTypeVars,
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
         DEF ServiceIoWorkerWork, AsyncTransportClockTypeInvariant
    <2>8. /\ asyncOutstandingTags' = asyncOutstandingTags
           /\ asyncNodeDeadlines' = asyncNodeDeadlines
           /\ asyncRetransmitDeadlines' = asyncRetransmitDeadlines
           /\ asyncNodeServiceDeadlines' = asyncNodeServiceDeadlines
      BY <1>1, Isa DEF ServiceIoWorkerWork, vars
    <2>9. AsyncTransportClockTypeInvariant'
      BY <2>1, <2>7, <2>8
         DEF AsyncTransportClockTypeInvariant
    <2>10. /\ IsFiniteSet(
                    AsyncIoResponseItemsAfterService(
                      node, asyncIoQueues[node]))
            /\ \A item \in
                   AsyncIoResponseItemsAfterService(
                     node, asyncIoQueues[node]):
                 AsyncItemTyped(item)
      BY <1>1, ServiceResponseItemsAreFiniteAndTyped
         DEF ServiceIoWorkerWork
    <2>11. /\ PublishEphemeralItems(
                    AsyncIoResponseItemsAfterService(
                      node, asyncIoQueues[node]))
            /\ UNCHANGED
                 <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
                   asyncHeldChunks>>
      BY <1>1
         DEF ServiceIoWorkerWork, AsyncIoResponseItemsAfterService,
             AsyncCertifiedResponseClaimCoreAuthorityVars, vars
    <2>12. AsyncTransportContentTypeInvariant'
      BY <2>1, <2>10, <2>11,
         PublishEphemeralItemsPreservesTransportContentType
    <2> QED BY <2>4, <2>9, <2>12
         DEF AsyncRuntimeTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM ServiceIoWorkerPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ StrongInductiveInvariant
    /\ ServiceIoWorkerWork(node)
    => AsyncSchedulerTypeInvariant'
BY ServiceIoWorkerPreservesTopologyType,
   ServiceIoWorkerPreservesContentType,
   ServiceIoWorkerPreservesCapacityType,
   ServiceIoWorkerPreservesNonIoType,
   HistoricalRecoveryFramePreservesType, Isa
   DEF AsyncSchedulerTypeInvariant, AsyncIoTypeInvariant,
       ServiceIoWorkerWork, AsyncHistoricalRecoveryFrameVars, vars

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
          /\ packet.authenticatedSource = source
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncPacketContentTypeInvariant,
                DueSourcePackets(recipient, source) # {}
         PROVE LET packet == OldestDueSourcePacket(recipient, source)
               IN /\ packet \in DueSourcePackets(recipient, source)
                  /\ AsyncPacketTyped(packet)
                  /\ packet.item.envelope.recipient = recipient
                  /\ packet.authenticatedSource = source
    <2>1. DueSourcePackets(recipient, source) \subseteq asyncTransport
      BY Isa DEF DueSourcePackets
    <2>2. IsFiniteSet(asyncTransport)
      BY <1>1 DEF AsyncPacketContentTypeInvariant
    <2>3. IsFiniteSet(DueSourcePackets(recipient, source))
      BY <2>1, <2>2, FS_Subset
    <2>4. \A packet \in DueSourcePackets(recipient, source): AsyncPacketTyped(packet)
      BY <1>1, <2>1, Isa DEF AsyncPacketContentTypeInvariant
    <2>4a. \A packet \in DueSourcePackets(recipient, source): packet.sentAt \in Nat
      BY <2>4, Isa DEF AsyncPacketTyped
    <2>4b. \A packet \in DueSourcePackets(recipient, source):
              /\ packet.item.envelope.recipient = recipient
              /\ packet.authenticatedSource = source
      BY Isa DEF DueSourcePackets
    <2>5. \E packet \in DueSourcePackets(recipient, source):
             \A other \in DueSourcePackets(recipient, source):
               packet.sentAt <= other.sentAt
      BY <1>1, <2>3, <2>4a, FinitePacketSetHasOldestSentAt
    <2>6. OldestDueSourcePacket(recipient, source)
             \in DueSourcePackets(recipient, source)
      BY <2>5, Zenon DEF OldestDueSourcePacket
    <2> QED BY <2>4, <2>4b, <2>6
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

THEOREM IngressOwnerBindingsAppendPreserved ==
  \A sequence, item, recipient, source:
    /\ sequence \in Seq(Range(sequence))
    /\ \A index \in 1..Len(sequence):
         /\ sequence[index].envelope.recipient = recipient
         /\ AsyncIngressItemSourceBinding(sequence[index], source)
    /\ item.envelope.recipient = recipient
    /\ AsyncIngressItemSourceBinding(item, source)
    => \A index \in 1..Len(Append(sequence, item)):
         /\ Append(sequence, item)[index].envelope.recipient = recipient
         /\ AsyncIngressItemSourceBinding(Append(sequence, item)[index], source)
PROOF
  <1>1. ASSUME NEW sequence, NEW item, NEW recipient, NEW source,
                sequence \in Seq(Range(sequence)),
                \A index \in 1..Len(sequence):
                  /\ sequence[index].envelope.recipient = recipient
                  /\ AsyncIngressItemSourceBinding(sequence[index], source),
                item.envelope.recipient = recipient,
                AsyncIngressItemSourceBinding(item, source)
         PROVE \A index \in 1..Len(Append(sequence, item)):
                 /\ Append(sequence, item)[index].envelope.recipient =
                      recipient
                 /\ AsyncIngressItemSourceBinding(
                      Append(sequence, item)[index], source)
    <2>1. /\ Len(Append(sequence, item)) = Len(sequence) + 1
           /\ \A index \in 1..Len(sequence):
                Append(sequence, item)[index] = sequence[index]
           /\ Append(sequence, item)[Len(sequence) + 1] = item
      BY <1>1, AppendSequenceFacts
    <2>2. ASSUME NEW index \in 1..Len(Append(sequence, item))
           PROVE /\ Append(sequence, item)[index].envelope.recipient =
                      recipient
                 /\ AsyncIngressItemSourceBinding(
                      Append(sequence, item)[index], source)
      <3>1. Len(sequence) \in Nat BY <1>1, LenProperties
      <3>2. CASE index \in 1..Len(sequence)
        BY <1>1, <2>1, <3>2
      <3>3. CASE index \notin 1..Len(sequence)
        <4>1. 1..Len(Append(sequence, item)) = 1..Len(sequence) \union {Len(sequence) + 1}
          BY ONLY <2>1, <3>1, Isa
        <4>2. index = Len(sequence) + 1 BY ONLY <3>3, <4>1, Zenon
        <4> QED BY <1>1, <2>1, <4>2
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>2
  <1> QED BY <1>1

THEOREM AdmitHiddenPacketPreservesCertifiedResponseClaimInvariant ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant /\ AdmitHiddenPacket(recipient, source)
      => AsyncCertifiedResponseClaimInvariant'
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                AdmitHiddenPacket(recipient, source)
         PROVE AsyncCertifiedResponseClaimInvariant'
    <2> DEFINE Packet == OldestDueSourcePacket(recipient, source)
    <2> DEFINE Item == Packet.item
    <2>1. /\ AsyncCertifiedResponseClaimInvariant
           /\ AsyncPacketContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant
    <2>2. /\ DueSourcePackets(recipient, source) # {}
           /\ Packet \in asyncTransport
           /\ AsyncPacketTyped(Packet)
           /\ AsyncItemTyped(Item)
      BY <1>1, <2>1, OldestDueSourcePacketFacts, Isa
         DEF AdmitHiddenPacket, Packet, Item, AsyncPacketTyped
    <2>3. /\ UNCHANGED
                 <<context, prepareQCs, decisions, lockRank, lockSubject,
                   installedTCs, commitIntents, asyncSentItems,
                   asyncActiveRequests>>
           /\ asyncCertifiedResponseClaim' =
                IF Item.kind = "CertifiedResponse"
                THEN asyncCertifiedResponseClaim
                       \cup {AsyncCertifiedResponseCanonicalWireIdentity(Item)}
                ELSE asyncCertifiedResponseClaim
      BY <1>1, Isa
         DEF AdmitHiddenPacket, Packet, Item, AsyncSchedulerVars, vars
    <2>4. CASE Item.kind # "CertifiedResponse"
      BY <2>1, <2>3, <2>4, Isa
         DEF AsyncCertifiedResponseClaimInvariant,
             CertifiedResponseClaimProjectionAuthenticated,
             CertifiedResponseAuthorized,
             CertifiedResponseAuthenticatedOccurrence,
             MatchingCertifiedRequests, CertifiedRequestAuthority,
             CertifiedBodyRecoveryAuthority,
             DecisionCertifiedBodyRecoveryAuthority,
             HistoricalLockedPrepareSource,
             HistoricalLockedPrepareRecoveryProvenance,
             InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
             NoDecisionForNode, ActiveCertifiedRequestHashes,
             ActiveCertifiedRequestHashesIn,
             CertifiedResponseAuthorityReady,
             CertifiedResponseAuthorityClaimed
    <2>5. CASE Item.kind = "CertifiedResponse"
      <3>1. /\ CertifiedResponseAuthorized(Item)
             /\ CertifiedResponseAuthorityReady(
                  Item.envelope.requestHash)
             /\ CertifiedResponseRecipientClaimAvailable(Item)
             /\ asyncCertifiedResponseClaim' =
                  asyncCertifiedResponseClaim
                    \cup {AsyncCertifiedResponseCanonicalWireIdentity(Item)}
        BY <1>1, <2>3, <2>5
           DEF AdmitHiddenPacket, Packet, Item, CanAdmitIngressItem,
               CertifiedResponseFreshClaimGateAllows,
               CertifiedResponseRecipientClaimAvailable
      <3>2. Item \in AsyncCertifiedResponseItems
        BY <2>2, <2>5, Isa
           DEF AsyncItemTyped, AsyncCertifiedResponseItems
      <3>3. /\ IsFiniteSet(asyncCertifiedResponseClaim')
             /\ asyncCertifiedResponseClaim'
                  \subseteq AsyncCertifiedResponseClaimValues
        BY <3>1, <3>2, FS_Singleton, SMT
           DEF AsyncCertifiedResponseClaimValues
      <3> QED BY <2>1, <2>3, <3>1, <3>2, <3>3,
                   FS_Singleton, SMTT(30), Isa
           DEF AsyncCertifiedResponseClaimInvariant,
               CertifiedResponseClaimProjectionAuthenticated,
               CertifiedResponseAuthorized,
               CertifiedResponseAuthenticatedOccurrence,
               MatchingCertifiedRequests,
               FrozenCertifiedRequestRegistration,
               FrozenCertifiedResponseBinding,
               AsyncCertifiedResponseCanonicalWireIdentity,
               CertifiedResponseClaimsAt,
               CertifiedResponseRecipientClaimAvailable,
               ActiveCertifiedRequestHashes,
               ActiveCertifiedRequestHashesIn,
               CertifiedResponseAuthorityReady,
               CertifiedResponseAuthorityClaimed
    <2> QED BY <2>4, <2>5
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
           /\ UNCHANGED
                <<vars, asyncSentItems, asyncRetainedControl,
                  asyncActiveRequests>>
           /\ UNCHANGED asyncHeldChunks
      BY <1>1, Isa
         DEF AdmitHiddenPacket, AsyncRuntimeScalarTypeVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredVars,
             AsyncDeferredTopologyTypeVars, AsyncTransportClockTypeVars,
             LeaveCausalQueues, AsyncSchedulerVars, vars
    <2>3. /\ AsyncRuntimeScalarTypeInvariant'
           /\ AsyncCausalTypeInvariant'
           /\ AsyncIoTopologyTypeInvariant'
           /\ AsyncIoContentTypeInvariant'
           /\ AsyncIoCapacityTypeInvariant'
           /\ AsyncDeferredTopologyTypeInvariant'
           /\ AsyncDeferredContentTypeInvariant'
           /\ AsyncTransportClockTypeInvariant'
           /\ AsyncHeldChunksTypeInvariant'
      BY <2>1, <2>2, AsyncRuntimeScalarTypeStutter,
         AsyncCausalTypeStutter, AsyncIoTopologyTypeStutter,
         AsyncIoContentTypeStutter, AsyncIoCapacityTypeStutter,
         AsyncDeferredTopologyTypeStutter,
         AsyncDeferredContentTypeStutter,
         AsyncTransportClockTypeStutter,
         AsyncHeldChunksTypeStutter
    <2>3a. AsyncCertifiedResponseClaimInvariant'
      BY <1>1,
         AdmitHiddenPacketPreservesCertifiedResponseClaimInvariant
    <2>3b. AsyncTransportHistoryTypeInvariant'
      BY <2>1, <2>2, <2>3a, Isa
         DEF AsyncTransportHistoryTypeInvariant,
             AsyncActiveRequestLogicalIndexConsistencyInvariant,
             CurrentVoters, CurrentEpoch
    <2>4. asyncTransport' \subseteq asyncTransport
      BY <1>1, Isa DEF AdmitHiddenPacket
    <2>5. /\ IsFiniteSet(asyncTransport')
           /\ \A packet \in asyncTransport': AsyncPacketTyped(packet)
      BY <2>1, <2>4, FS_Subset, SMT
         DEF AsyncPacketContentTypeInvariant
    <2>6. AsyncPacketContentTypeInvariant'
      BY <2>5 DEF AsyncPacketContentTypeInvariant
    <2> QED BY <2>3, <2>3b, <2>6
         DEF AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncDeferredTypeInvariant, AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant
  <1> QED BY <1>1

(***************************************************************************
The authenticated delivery route selects the due transport occurrence, but it
does not multiply certified-response queue ownership.  Every fully authorized
response is normalized onto the one aggregate untrusted physical-completion
lane before the count/byte gate.  Keep this action-level equality explicit so
an accidental reversion to the outer transport source cannot hide inside the
generic ingress type-preservation proof.
***************************************************************************)
THEOREM AdmitCertifiedResponseNormalizesPhysicalResourceSource ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncTypeInvariant
    /\ DueSourcePackets(recipient, source) # {}
    /\ OldestDueSourcePacket(recipient, source).item.kind =
         "CertifiedResponse"
    /\ AdmitHiddenPacket(recipient, source)
    => LET item == OldestDueSourcePacket(recipient, source).item
       IN /\ IngressResourceSource(item) = AsyncUntrustedSource
          /\ asyncIngressLanes' =
               [asyncIngressLanes EXCEPT
                  ![recipient][AsyncUntrustedSource] = Append(@, item)]
BY Isa
   DEF AdmitHiddenPacket, IngressResourceSource

THEOREM CertifiedResponseClaimIngressOwnershipStutter ==
  /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
  /\ UNCHANGED <<asyncCertifiedResponseClaim, asyncIngressLanes>>
  => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY Isa
   DEF AsyncCertifiedResponseClaimIngressOwnershipInvariant,
       CertifiedResponseClaimIngressOwner

THEOREM CertifiedResponseClaimIngressOwnershipIsDownwardClosed ==
  /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
  /\ asyncCertifiedResponseClaim'
       \subseteq asyncCertifiedResponseClaim
  /\ UNCHANGED asyncIngressLanes
  => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY Isa
   DEF AsyncCertifiedResponseClaimIngressOwnershipInvariant,
       CertifiedResponseClaimIngressOwner

THEOREM EmptyCertifiedResponseClaimHasIngressOwnership ==
  asyncCertifiedResponseClaim = {}
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant
BY Isa DEF AsyncCertifiedResponseClaimIngressOwnershipInvariant

THEOREM AppendIngressLanePreservesCertifiedResponseClaimIngressOwnership ==
  \A recipient \in ValidatorIds:
    \A source \in AsyncIngressSources:
      \A item:
        /\ AsyncIngressTypeInvariant
        /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
        /\ asyncIngressLanes' =
             [asyncIngressLanes EXCEPT
                ![recipient][source] =
                  Append(@, item)]
        /\ UNCHANGED asyncCertifiedResponseClaim
        => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY AppendSequenceFacts, SMTT(60), Isa
   DEF AsyncCertifiedResponseClaimIngressOwnershipInvariant,
       CertifiedResponseClaimIngressOwner,
       AsyncIngressTypeInvariant, AsyncIngressTopologyTypeInvariant,
       AsyncIngressContentTypeInvariant,
       IngressLane, IngressLaneDepth

THEOREM AppendCertifiedResponseEstablishesClaimIngressOwnership ==
  \A recipient \in ValidatorIds:
    \A item:
      /\ AsyncIngressTypeInvariant
      /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
      /\ item.kind = "CertifiedResponse"
      /\ item.envelope.recipient = recipient
      /\ IngressResourceSource(item) = AsyncUntrustedSource
      /\ asyncIngressLanes' =
           [asyncIngressLanes EXCEPT
              ![recipient][AsyncUntrustedSource] =
                Append(@, item)]
      /\ asyncCertifiedResponseClaim' =
           asyncCertifiedResponseClaim
             \cup {AsyncCertifiedResponseCanonicalWireIdentity(item)}
      => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY AppendSequenceFacts, SMTT(60), Isa
   DEF AsyncCertifiedResponseClaimIngressOwnershipInvariant,
       CertifiedResponseClaimIngressOwner,
       AsyncIngressTypeInvariant, AsyncIngressTopologyTypeInvariant,
       AsyncIngressContentTypeInvariant,
       IngressLane, IngressLaneDepth

THEOREM AdmitHiddenPacketPreservesClaimIngressOwnership ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncTypeInvariant
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ AdmitHiddenPacket(recipient, source)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                AsyncCertifiedResponseClaimIngressOwnershipInvariant,
                AdmitHiddenPacket(recipient, source)
         PROVE AsyncCertifiedResponseClaimIngressOwnershipInvariant'
    <2> DEFINE Item ==
          OldestDueSourcePacket(recipient, source).item
    <2> DEFINE ResourceSource == IngressResourceSourceVia(Item, source)
    <2>1. /\ AsyncIngressTypeInvariant
           /\ AsyncPacketContentTypeInvariant
           /\ DueSourcePackets(recipient, source) # {}
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant,
             AdmitHiddenPacket
    <2>2. /\ AsyncItemTyped(Item)
           /\ Item.envelope.recipient = recipient
      BY <1>1, <2>1, OldestDueSourcePacketFacts, Zenon DEF AsyncPacketTyped, Item
    <2>2a. Item.source \in AsyncIngressSources BY <2>2, Zenon DEF AsyncItemTyped
    <2>2b. ResourceSource \in AsyncIngressSources
      BY <1>1, <2>2a, Isa
         DEF ResourceSource, IngressResourceSourceVia, IngressResourceSource, AsyncIngressSources
    <2>3. asyncIngressLanes' =
             [asyncIngressLanes EXCEPT
                ![recipient][ResourceSource] =
                  Append(@, Item)]
      BY <1>1 DEF AdmitHiddenPacket, Item, ResourceSource
    <2>4. CASE Item.kind = "CertifiedResponse"
      <3>1. /\ IngressResourceSource(Item) = AsyncUntrustedSource
             /\ asyncIngressLanes' = [asyncIngressLanes EXCEPT
                  ![recipient][AsyncUntrustedSource] = Append(@, Item)]
        BY <1>1, <2>1, <2>4, AdmitCertifiedResponseNormalizesPhysicalResourceSource DEF Item
      <3>2. asyncCertifiedResponseClaim' = asyncCertifiedResponseClaim
               \cup {AsyncCertifiedResponseCanonicalWireIdentity(Item)}
        BY <1>1, <2>4 DEF AdmitHiddenPacket, Item
      <3> QED BY <1>1, <2>1, <2>2, <2>4, <3>1, <3>2,
           AppendCertifiedResponseEstablishesClaimIngressOwnership
    <2>5. CASE Item.kind # "CertifiedResponse"
      <3>1. UNCHANGED asyncCertifiedResponseClaim
        BY <1>1, <2>5 DEF AdmitHiddenPacket, Item
      <3> QED BY <1>1, <2>1, <2>2, <2>2b, <2>3, <3>1,
           AppendIngressLanePreservesCertifiedResponseClaimIngressOwnership
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM AdmitIngressPacketPreservesClaimIngressOwnership ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncTypeInvariant
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ AdmitIngressPacket(recipient, source)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                AsyncCertifiedResponseClaimIngressOwnershipInvariant,
                AdmitIngressPacket(recipient, source)
         PROVE AsyncCertifiedResponseClaimIngressOwnershipInvariant'
    <2>1. CASE AdmitHiddenPacket(recipient, source)
      BY <1>1, <2>1,
         AdmitHiddenPacketPreservesClaimIngressOwnership
    <2>2. CASE CoalesceHiddenPacket(recipient, source)
      BY <1>1, <2>2,
         CertifiedResponseClaimIngressOwnershipStutter
         DEF CoalesceHiddenPacket
    <2>3. CASE DropPolicyRejectedHiddenPacket(recipient, source)
      BY <1>1, <2>3,
         CertifiedResponseClaimIngressOwnershipStutter
         DEF DropPolicyRejectedHiddenPacket
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF AdmitIngressPacket
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
    <2> DEFINE Item ==
           OldestDueSourcePacket(recipient, source).item
    <2> DEFINE ResourceSource == IngressResourceSourceVia(Item, source)
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
           /\ AsyncItemTyped(Item)
      BY <1>1, <2>1, OldestDueSourcePacketFacts
         DEF AsyncPacketTyped, Item
    <2>2a. /\ Item.envelope.recipient = recipient
            /\ ResourceSource \in AsyncIngressSources
            /\ AsyncIngressItemSourceBinding(Item, ResourceSource)
      BY <2>1, <2>2, OldestDueSourcePacketFacts, SMT
         DEF Item, ResourceSource, IngressResourceSourceVia, IngressResourceSource,
             AsyncIngressItemSourceBinding, AsyncItemTyped, AsyncPacketTyped, AsyncIngressSources
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
                        /\ AsyncItemTyped(
                             asyncIngressLanes'[otherRecipient][otherSource]
                               [index])
                        /\ asyncIngressLanes'[otherRecipient][otherSource]
                             [index].envelope.recipient = otherRecipient
                        /\ AsyncIngressItemSourceBinding(
                             asyncIngressLanes'[otherRecipient][otherSource]
                               [index], otherSource)
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
                          /\ AsyncItemTyped(
                               asyncIngressLanes'[otherRecipient][otherSource]
                                 [index])
                          /\ asyncIngressLanes'[otherRecipient][otherSource]
                               [index].envelope.recipient = otherRecipient
                          /\ AsyncIngressItemSourceBinding(
                               asyncIngressLanes'[otherRecipient][otherSource]
                                 [index], otherSource)
        <4>1. CASE otherRecipient = recipient
                     /\ otherSource = ResourceSource
          <5>1. asyncIngressLanes'[otherRecipient][otherSource] =
                   Append(IngressLane(otherRecipient, otherSource),
                          Item)
            BY <1>1, <2>1, <2>2a, <3>1, <4>1, Isa
               DEF AdmitHiddenPacket, IngressLane,
                   Item, ResourceSource
          <5>2. /\ IngressLane(otherRecipient, otherSource)
                         \in Seq(Range(
                              IngressLane(otherRecipient, otherSource)))
                 /\ DOMAIN IngressLane(otherRecipient, otherSource) =
                      1..Len(IngressLane(otherRecipient, otherSource))
                 /\ \A index \in
                        1..Len(IngressLane(otherRecipient, otherSource)):
                      /\ AsyncItemTyped(
                           IngressLane(otherRecipient, otherSource)[index])
                      /\ IngressLane(otherRecipient, otherSource)
                           [index].envelope.recipient = otherRecipient
                      /\ AsyncIngressItemSourceBinding(
                           IngressLane(otherRecipient, otherSource)[index],
                           otherSource)
            BY <2>1, <3>1
               DEF AsyncIngressContentTypeInvariant, IngressLaneDepth
          <5>3. /\ AsyncItemTyped(Item)
                 /\ Item.envelope.recipient = otherRecipient
                 /\ AsyncIngressItemSourceBinding(Item, otherSource)
            BY <2>2, <2>2a, <4>1
          <5>4. /\ Append(IngressLane(otherRecipient, otherSource),
                              Item)
                           \in Seq(Range(
                                Append(IngressLane(otherRecipient, otherSource),
                                  Item)))
                 /\ DOMAIN Append(IngressLane(otherRecipient, otherSource),
                                   Item)
                        = 1..Len(
                            Append(IngressLane(otherRecipient, otherSource),
                              Item))
                 /\ \A index \in
                        1..Len(Append(
                          IngressLane(otherRecipient, otherSource),
                          Item)):
                      /\ AsyncItemTyped(
                           Append(IngressLane(otherRecipient, otherSource),
                             Item)[index])
                      /\ Append(IngressLane(otherRecipient, otherSource),
                           Item)[index].envelope.recipient = otherRecipient
                      /\ AsyncIngressItemSourceBinding(
                           Append(IngressLane(otherRecipient, otherSource),
                             Item)[index], otherSource)
            BY <5>2, <5>3, TypedIngressAppendPreservesSequence,
               IngressOwnerBindingsAppendPreserved
          <5> QED BY <5>1, <5>4 DEF IngressLaneDepth
        <4>2. CASE ~(otherRecipient = recipient
                       /\ otherSource = ResourceSource)
          <5>1. asyncIngressLanes'[otherRecipient][otherSource] =
                   IngressLane(otherRecipient, otherSource)
            BY <1>1, <2>1, <2>2a, <3>1, <4>2, Isa
               DEF AdmitHiddenPacket, IngressLane,
                   Item, ResourceSource
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
    <2> DEFINE Item ==
           OldestDueSourcePacket(recipient, source).item
    <2> DEFINE ResourceSource == IngressResourceSourceVia(Item, source)
    <2>1. /\ AsyncConfiguration
           /\ ModelConfiguration
           /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, TypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncIngressTypeInvariant
    <2>1a. /\ AsyncItemTyped(Item)
            /\ Item.envelope.recipient = recipient
            /\ ResourceSource \in AsyncIngressSources
      BY <1>1, <2>1, OldestDueSourcePacketFacts, SMT
         DEF AdmitHiddenPacket, AsyncPacketContentTypeInvariant,
             AsyncTransportTypeInvariant, AsyncTransportContentTypeInvariant,
             Item, ResourceSource, IngressResourceSourceVia,
             AsyncItemTyped, AsyncIngressSources
    <2>2. /\ DOMAIN asyncIngressLanes' = ValidatorIds
           /\ \A otherRecipient \in ValidatorIds:
                DOMAIN asyncIngressLanes'[otherRecipient] =
                  AsyncIngressSources
      BY <1>1, <2>1, <2>1a, NestedIngressAppendLaneFacts
         DEF AdmitHiddenPacket, AsyncIngressTopologyTypeInvariant,
             AsyncIngressContentTypeInvariant, IngressLane,
             Item, ResourceSource
    <2>3. /\ AsyncIngressNonemptySourcesFor(
                    asyncIngressLanes', recipient) =
                  AsyncIngressNonemptySourcesFor(
                    asyncIngressLanes, recipient) \cup {ResourceSource}
           /\ \A otherRecipient \in ValidatorIds \ {recipient}:
                AsyncIngressNonemptySourcesFor(
                  asyncIngressLanes', otherRecipient) =
                AsyncIngressNonemptySourcesFor(
                  asyncIngressLanes, otherRecipient)
      BY <1>1, <2>1, <2>1a, NestedIngressAppendSourceSetFacts
         DEF AdmitHiddenPacket, AsyncIngressTopologyTypeInvariant,
             AsyncIngressContentTypeInvariant, IngressLane,
             Item, ResourceSource
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
        <4>1. CASE Len(IngressLane(recipient, ResourceSource)) = 0
          <5>1. asyncIngressReady'[otherRecipient] =
                   Append(asyncIngressReady[otherRecipient], ResourceSource)
            BY <1>1, <2>1a, <2>6, <3>2, <4>1
               DEF AdmitHiddenPacket, Item, ResourceSource
          <5>2. /\ DOMAIN Append(
                              asyncIngressReady[otherRecipient],
                              ResourceSource) =
                         1..(Len(asyncIngressReady[otherRecipient]) + 1)
                 /\ Len(Append(asyncIngressReady[otherRecipient],
                               ResourceSource)) =
                         Len(asyncIngressReady[otherRecipient]) + 1
                 /\ Append(asyncIngressReady[otherRecipient], ResourceSource)
                         \in Seq(Range(asyncIngressReady[otherRecipient])
                                  \cup {ResourceSource})
            BY <3>1, AppendSequenceFacts
          <5>3. /\ Append(asyncIngressReady[otherRecipient], ResourceSource)
                           \in Seq(Range(
                                Append(asyncIngressReady[otherRecipient],
                                       ResourceSource)))
                 /\ DOMAIN Append(asyncIngressReady[otherRecipient],
                                  ResourceSource) =
                           1..Len(Append(
                                asyncIngressReady[otherRecipient],
                                ResourceSource))
                 /\ SequenceSet(
                        Append(asyncIngressReady[otherRecipient],
                               ResourceSource)) =
                           SequenceSet(asyncIngressReady[otherRecipient])
                             \cup {ResourceSource}
            BY <3>1, <5>2, SequenceSetAfterAppend,
               SeqOfRange, LenProperties, Isa
          <5>4. ResourceSource \notin
                   SequenceSet(asyncIngressReady[otherRecipient])
            BY <2>1a, <3>1, <3>2, <4>1, SMT
               DEF AsyncIngressNonemptySourcesFor, IngressLane
          <5>5. IsFiniteSet(
                   SequenceSet(asyncIngressReady[otherRecipient]))
            BY <2>5, <3>1, FS_Subset
          <5>6. Cardinality(
                   SequenceSet(asyncIngressReady[otherRecipient])
                     \cup {ResourceSource}) =
                   Cardinality(
                     SequenceSet(asyncIngressReady[otherRecipient])) + 1
            BY <5>4, <5>5, FS_AddElement
          <5>7. Len(Append(asyncIngressReady[otherRecipient],
                           ResourceSource)) =
                   Cardinality(SequenceSet(
                     Append(asyncIngressReady[otherRecipient],
                            ResourceSource)))
            BY <3>1, <5>2, <5>3, <5>6, SMT
          <5>8. SequenceSet(
                   Append(asyncIngressReady[otherRecipient],
                          ResourceSource)) =
                 AsyncIngressNonemptySourcesFor(
                   asyncIngressLanes', otherRecipient)
            BY <2>3, <2>6, <3>1, <3>2, <5>3
          <5>9. SequenceSet(
                   Append(asyncIngressReady[otherRecipient],
                          ResourceSource))
                     \subseteq AsyncIngressSources
            BY <2>1a, <3>1, <5>3, Isa
          <5> QED BY <5>1, <5>2, <5>3, <5>7, <5>8, <5>9
        <4>2. CASE Len(
                        IngressLane(recipient, ResourceSource)) # 0
          <5>1. Len(IngressLane(recipient, ResourceSource)) \in Nat
            BY <2>1, <1>1, LenProperties
               DEF AsyncIngressContentTypeInvariant, IngressLane
          <5>2. ResourceSource \in
                   AsyncIngressNonemptySourcesFor(
                     asyncIngressLanes, otherRecipient)
            BY <2>1a, <2>6, <3>2, <4>2, <5>1, SMT
               DEF AsyncIngressNonemptySourcesFor, IngressLane
          <5>3. AsyncIngressNonemptySourcesFor(
                    asyncIngressLanes, otherRecipient)
                    \cup {ResourceSource} =
                  AsyncIngressNonemptySourcesFor(
                    asyncIngressLanes, otherRecipient)
            BY <5>2, Isa
          <5>4. /\ asyncIngressReady'[otherRecipient] =
                        asyncIngressReady[otherRecipient]
                 /\ AsyncIngressNonemptySourcesFor(
                       asyncIngressLanes', otherRecipient) =
                        AsyncIngressNonemptySourcesFor(
                          asyncIngressLanes, otherRecipient)
            BY <1>1, <2>1a, <2>3, <2>6, <3>2, <4>2, <5>3
               DEF AdmitHiddenPacket, Item, ResourceSource
          <5> QED BY <3>1, <5>4
        <4> QED BY <4>1, <4>2
      <3>3. CASE otherRecipient # recipient
        <4>1. /\ asyncIngressReady'[otherRecipient] =
                        asyncIngressReady[otherRecipient]
               /\ AsyncIngressNonemptySourcesFor(
                     asyncIngressLanes', otherRecipient) =
                        AsyncIngressNonemptySourcesFor(
                          asyncIngressLanes, otherRecipient)
          BY <1>1, <2>1a, <2>3, <2>6, <3>3, Isa
             DEF AdmitHiddenPacket,
                 AsyncIngressTopologyTypeInvariant,
                 Item, ResourceSource
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

THEOREM IngressProtectedSourcesFinite ==
  \A lanes, recipient:
    IsFiniteSet(AsyncIngressSources)
      => /\ IngressProtectedSourcesFor(lanes, recipient)
               \subseteq AsyncIngressSources
         /\ IsFiniteSet(IngressProtectedSourcesFor(lanes, recipient))
         /\ Cardinality(IngressProtectedSourcesFor(lanes, recipient))
               \in Nat
PROOF
  <1>1. ASSUME NEW lanes, NEW recipient,
                IsFiniteSet(AsyncIngressSources)
         PROVE /\ IngressProtectedSourcesFor(lanes, recipient)
                    \subseteq AsyncIngressSources
               /\ IsFiniteSet(
                    IngressProtectedSourcesFor(lanes, recipient))
               /\ Cardinality(
                    IngressProtectedSourcesFor(lanes, recipient)) \in Nat
    <2>1. IngressProtectedSourcesFor(lanes, recipient)
               \subseteq AsyncIngressSources
      BY Isa DEF IngressProtectedSourcesFor
    <2>2. IsFiniteSet(
             IngressProtectedSourcesFor(lanes, recipient))
      BY <1>1, <2>1, FS_Subset
    <2>3. Cardinality(
             IngressProtectedSourcesFor(lanes, recipient)) \in Nat
      BY <2>2, FS_CardinalityType
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM IngressProtectedSlotCountIsNatural ==
  \A lanes, recipient:
    (AsyncConfiguration /\ ModelConfiguration)
      => IngressProtectedSlotCountFor(lanes, recipient) \in Nat
PROOF
  <1>1. ASSUME NEW lanes, NEW recipient,
                AsyncConfiguration /\ ModelConfiguration
         PROVE IngressProtectedSlotCountFor(lanes, recipient) \in Nat
    <2>1. IsFiniteSet(AsyncIngressSources)
      BY <1>1, AsyncIngressSourcesAreFinite
    <2>2. Cardinality(
             IngressProtectedSourcesFor(lanes, recipient)) \in Nat
      BY <2>1, IngressProtectedSourcesFinite
    <2>3. IngressContinuationProtectedSourcesFor(lanes, recipient)
               \subseteq AsyncIngressSources
      BY Isa DEF IngressContinuationProtectedSourcesFor,
                 AsyncIngressSources
    <2>4. IngressTimeoutVoteProtectedSourcesFor(lanes, recipient)
               \subseteq AsyncIngressSources
      BY Isa DEF IngressTimeoutVoteProtectedSourcesFor,
                 AsyncIngressSources
    <2>4f. IngressCertifiedFenceEscapeProtectedSourcesFor(
                lanes, recipient) \subseteq AsyncIngressSources
      BY Isa DEF IngressCertifiedFenceEscapeProtectedSourcesFor,
                 AsyncIngressSources
    <2>4c. IngressTransportCompletionProtectedSourcesFor(lanes, recipient)
                \subseteq AsyncIngressSources
      BY Isa DEF IngressTransportCompletionProtectedSourcesFor,
                 AsyncIngressSources
    <2>5. IsFiniteSet(
             IngressContinuationProtectedSourcesFor(lanes, recipient))
      BY <2>1, <2>3, FS_Subset
    <2>6. IsFiniteSet(
             IngressTimeoutVoteProtectedSourcesFor(lanes, recipient))
      BY <2>1, <2>4, FS_Subset
    <2>6f. IsFiniteSet(
              IngressCertifiedFenceEscapeProtectedSourcesFor(
                lanes, recipient))
      BY <2>1, <2>4f, FS_Subset
    <2>6c. IsFiniteSet(
              IngressTransportCompletionProtectedSourcesFor(
                lanes, recipient))
      BY <2>1, <2>4c, FS_Subset
    <2>7. Cardinality(
             IngressContinuationProtectedSourcesFor(lanes, recipient)) \in Nat
      BY <2>5, FS_CardinalityType
    <2>8. Cardinality(
             IngressTimeoutVoteProtectedSourcesFor(lanes, recipient)) \in Nat
      BY <2>6, FS_CardinalityType
    <2>8f. Cardinality(
               IngressCertifiedFenceEscapeProtectedSourcesFor(
                 lanes, recipient)) \in Nat
      BY <2>6f, FS_CardinalityType
    <2>8c. Cardinality(
               IngressTransportCompletionProtectedSourcesFor(
                 lanes, recipient)) \in Nat
      BY <2>6c, FS_CardinalityType
    <2> QED BY <2>2, <2>7, <2>8, <2>8f, <2>8c, SMT
         DEF IngressProtectedSlotCountFor
  <1> QED BY <1>1

THEOREM IngressLaneAtCapacityConsumesCapacity ==
  \A lanes, recipient, source:
    /\ IsFiniteSet(AsyncIngressSources)
    /\ source \in AsyncIngressSources
    /\ AsyncIngressCapacity \in Nat
    /\ Len(lanes[recipient][source]) \in Nat
    /\ Len(lanes[recipient][source]) >= AsyncIngressCapacity
    => AsyncIngressDepthFor(lanes, recipient) >= AsyncIngressCapacity
PROOF
  <1>1. ASSUME NEW lanes, NEW recipient, NEW source,
                IsFiniteSet(AsyncIngressSources),
                source \in AsyncIngressSources,
                AsyncIngressCapacity \in Nat,
                Len(lanes[recipient][source]) \in Nat,
                Len(lanes[recipient][source]) >= AsyncIngressCapacity
         PROVE AsyncIngressDepthFor(lanes, recipient)
                 >= AsyncIngressCapacity
    <2>1. /\ IsFiniteSet({source})
           /\ Cardinality({source}) = 1
           /\ IsFiniteSet(1..AsyncIngressCapacity)
           /\ Cardinality(1..AsyncIngressCapacity) =
                AsyncIngressCapacity
      BY <1>1, FS_Singleton, FS_Interval, SMT
    <2>2. /\ IsFiniteSet(
                  {source} \X (1..AsyncIngressCapacity))
           /\ Cardinality(
                  {source} \X (1..AsyncIngressCapacity)) =
                AsyncIngressCapacity
      BY <2>1, FS_Product, SMT
    <2>3. {source} \X (1..AsyncIngressCapacity)
               \subseteq AsyncIngressPairIndicesFor(lanes, recipient)
      BY <1>1, SMT DEF AsyncIngressPairIndicesFor
    <2>4. IsFiniteSet(
             AsyncIngressPairIndicesFor(lanes, recipient))
      <3>1. IsFiniteSet(
               AsyncIngressSources \X (1..AsyncIngressCapacity))
        BY <1>1, <2>1, FS_Product
      <3> QED BY <3>1, FS_Subset
           DEF AsyncIngressPairIndicesFor
    <2>5. Cardinality(
             {source} \X (1..AsyncIngressCapacity)) <=
           Cardinality(
             AsyncIngressPairIndicesFor(lanes, recipient))
      BY <2>2, <2>3, <2>4, FS_Subset
    <2> QED BY <2>2, <2>5 DEF AsyncIngressDepthFor
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
    <2> DEFINE Item ==
           OldestDueSourcePacket(recipient, source).item
    <2> DEFINE ResourceSource == IngressResourceSourceVia(Item, source)
    <2> DEFINE NextLanes == IngressLanesAfterAdmissionVia(Item, source)
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
    <2>2. /\ AsyncItemTyped(Item)
           /\ Item.envelope.recipient = recipient
           /\ ResourceSource \in AsyncIngressSources
      BY <2>1, OldestDueSourcePacketFacts, SMT
         DEF Item, ResourceSource, IngressResourceSourceVia,
             AsyncPacketContentTypeInvariant, AsyncPacketTyped,
             AsyncItemTyped, AsyncIngressSources
    <2>3. /\ NextLanes =
                  [asyncIngressLanes EXCEPT
                     ![recipient][ResourceSource] = Append(@, Item)]
           /\ asyncIngressLanes' = NextLanes
      BY <1>1, <2>2
         DEF AdmitHiddenPacket, NextLanes,
             IngressLanesAfterAdmissionVia, Item, ResourceSource
    <2>4. IngressDepth(recipient) <
             AsyncIngressCapacity -
               IngressProtectedSlotCountFor(NextLanes, recipient)
      BY <1>1, <2>2, <2>3
         DEF AdmitHiddenPacket, CanAdmitIngressItemVia,
             IngressUsableCapacityAfterAdmissionVia,
             IngressProtectedSlotCountAfterAdmissionVia,
             NextLanes, Item
    <2>5. /\ IsFiniteSet(AsyncIngressSources)
           /\ AsyncIngressCapacity \in Nat \ {0}
      BY <2>1, AsyncIngressSourcesAreFinite
         DEF AsyncConfiguration
    <2>6. IngressProtectedSlotCountFor(NextLanes, recipient) \in Nat
      BY <2>1, IngressProtectedSlotCountIsNatural
    <2>7. IsFiniteSet(
             AsyncIngressPairIndicesFor(asyncIngressLanes, recipient))
      <3>1. /\ IsFiniteSet(1..AsyncIngressCapacity)
             /\ IsFiniteSet(
                  AsyncIngressSources \X (1..AsyncIngressCapacity))
        BY <2>5, FS_Interval, FS_Product, SMT
      <3> QED BY <3>1, FS_Subset
           DEF AsyncIngressPairIndicesFor
    <2>8. IngressDepth(recipient) \in Nat
      <3>1. Cardinality(
               AsyncIngressPairIndicesFor(
                 asyncIngressLanes, recipient)) \in Nat
        BY <2>7, FS_CardinalityType
      <3> QED BY <3>1
           DEF IngressDepth, AsyncIngressPairIndicesFor,
               IngressLaneDepth, IngressLane
    <2>9. IngressDepth(recipient) < AsyncIngressCapacity
      BY <2>4, <2>5, <2>6, <2>8, SMT
    <2>10. Len(
              asyncIngressLanes[recipient][ResourceSource]) \in Nat
      BY <2>1, LenProperties
         DEF AsyncIngressContentTypeInvariant, IngressLane
    <2>11. Len(asyncIngressLanes[recipient][ResourceSource]) <
               AsyncIngressCapacity
      <3>1. ASSUME Len(
                      asyncIngressLanes[recipient][ResourceSource]) >=
                       AsyncIngressCapacity
             PROVE FALSE
        <4>1. AsyncIngressDepthFor(
                 asyncIngressLanes, recipient) >=
                   AsyncIngressCapacity
          BY <1>1, <2>2, <2>5, <2>10, <3>1,
             IngressLaneAtCapacityConsumesCapacity
        <4>2. AsyncIngressDepthFor(
                 asyncIngressLanes, recipient) =
               IngressDepth(recipient)
          BY DEF AsyncIngressDepthFor, IngressDepth,
                 AsyncIngressPairIndicesFor,
                 IngressLaneDepth, IngressLane
        <4>3. ~(IngressDepth(recipient) < AsyncIngressCapacity)
          BY <2>5, <2>8, <4>1, <4>2,
             NaturalGreaterOrEqualIsNotLess
        <4> QED BY <2>9, <4>3
      <3> QED BY <2>5, <2>10, <3>1, SMT
    <2>12. LET nextIndex ==
                    <<ResourceSource,
                      Len(asyncIngressLanes[recipient][ResourceSource]) + 1>>
            IN /\ nextIndex \notin
                     AsyncIngressPairIndicesFor(
                       asyncIngressLanes, recipient)
               /\ AsyncIngressPairIndicesFor(NextLanes, recipient) =
                    AsyncIngressPairIndicesFor(
                      asyncIngressLanes, recipient) \cup {nextIndex}
      <3>1. <<ResourceSource,
               Len(asyncIngressLanes[recipient][ResourceSource]) + 1>>
                \notin AsyncIngressPairIndicesFor(
                          asyncIngressLanes, recipient)
        <4>1. asyncIngressLanes[recipient][ResourceSource]
                   \in Seq(Range(
                        asyncIngressLanes[recipient][ResourceSource]))
          BY <2>1, <2>2
             DEF AsyncIngressContentTypeInvariant, IngressLane
        <4> QED BY <4>1, NextIngressIndexIsFresh
      <3>2. LET nextIndex ==
                      <<ResourceSource,
                        Len(
                          asyncIngressLanes[recipient][ResourceSource]) + 1>>
              IN AsyncIngressPairIndicesFor(NextLanes, recipient) =
                   AsyncIngressPairIndicesFor(
                     asyncIngressLanes, recipient) \cup {nextIndex}
        <4>1. AsyncIngressCapacity \in Nat
          BY <2>5, SMT
        <4>2. /\ DOMAIN asyncIngressLanes = ValidatorIds
               /\ \A otherRecipient \in ValidatorIds:
                    /\ DOMAIN asyncIngressLanes[otherRecipient] =
                         AsyncIngressSources
                    /\ \A otherSource \in AsyncIngressSources:
                         asyncIngressLanes[otherRecipient][otherSource]
                           \in Seq(Range(
                                asyncIngressLanes[otherRecipient]
                                  [otherSource]))
          BY <2>1
             DEF AsyncIngressTopologyTypeInvariant,
                 AsyncIngressContentTypeInvariant, IngressLane
        <4>3. LET nextIndex ==
                        <<ResourceSource,
                          Len(
                            asyncIngressLanes[recipient][ResourceSource]) + 1>>
                IN AsyncIngressPairIndicesFor(NextLanes, recipient) =
                     IF Len(
                          asyncIngressLanes[recipient][ResourceSource]) <
                          AsyncIngressCapacity
                     THEN AsyncIngressPairIndicesFor(
                            asyncIngressLanes, recipient) \cup {nextIndex}
                     ELSE AsyncIngressPairIndicesFor(
                            asyncIngressLanes, recipient)
          BY <2>2, <2>3, <4>1, <4>2,
             NestedIngressAppendPairSetFacts
        <4> QED BY <2>11, <4>3, SMT
      <3> QED BY <3>1, <3>2
    <2>13. AsyncIngressDepthFor(NextLanes, recipient) =
               IngressDepth(recipient) + 1
      <3>1. Cardinality(
               AsyncIngressPairIndicesFor(NextLanes, recipient)) =
             Cardinality(
               AsyncIngressPairIndicesFor(
                 asyncIngressLanes, recipient)) + 1
        BY <2>7, <2>12, FS_AddElement
      <3> QED BY <3>1
           DEF AsyncIngressDepthFor, IngressDepth,
               AsyncIngressPairIndicesFor,
               IngressLaneDepth, IngressLane
    <2>14. \A otherRecipient \in ValidatorIds \ {recipient}:
               /\ AsyncIngressDepthFor(
                    NextLanes, otherRecipient) =
                    IngressDepth(otherRecipient)
               /\ IngressProtectedSourcesFor(
                    NextLanes, otherRecipient) =
                    IngressProtectedSourcesFor(
                      asyncIngressLanes, otherRecipient)
               /\ IngressTimeoutVoteProtectedSourcesFor(
                    NextLanes, otherRecipient) =
                    IngressTimeoutVoteProtectedSourcesFor(
                      asyncIngressLanes, otherRecipient)
               /\ IngressTransportCompletionProtectedSourcesFor(
                    NextLanes, otherRecipient) =
                    IngressTransportCompletionProtectedSourcesFor(
                      asyncIngressLanes, otherRecipient)
               /\ IngressContinuationProtectedSourcesFor(
                    NextLanes, otherRecipient) =
                    IngressContinuationProtectedSourcesFor(
                      asyncIngressLanes, otherRecipient)
               /\ IngressProtectedSlotCountFor(
                    NextLanes, otherRecipient) =
                    IngressProtectedSlotCountFor(
                      asyncIngressLanes, otherRecipient)
      <3>1. ASSUME NEW otherRecipient \in
                       ValidatorIds \ {recipient}
             PROVE /\ AsyncIngressDepthFor(
                          NextLanes, otherRecipient) =
                          IngressDepth(otherRecipient)
                   /\ IngressProtectedSourcesFor(
                          NextLanes, otherRecipient) =
                          IngressProtectedSourcesFor(
                            asyncIngressLanes, otherRecipient)
                   /\ IngressTimeoutVoteProtectedSourcesFor(
                          NextLanes, otherRecipient) =
                          IngressTimeoutVoteProtectedSourcesFor(
                            asyncIngressLanes, otherRecipient)
                   /\ IngressTransportCompletionProtectedSourcesFor(
                          NextLanes, otherRecipient) =
                          IngressTransportCompletionProtectedSourcesFor(
                            asyncIngressLanes, otherRecipient)
                   /\ IngressContinuationProtectedSourcesFor(
                          NextLanes, otherRecipient) =
                          IngressContinuationProtectedSourcesFor(
                            asyncIngressLanes, otherRecipient)
                   /\ IngressProtectedSlotCountFor(
                          NextLanes, otherRecipient) =
                          IngressProtectedSlotCountFor(
                            asyncIngressLanes, otherRecipient)
        <4>1. otherRecipient # recipient
          BY <3>1, Isa
        <4>2. DOMAIN asyncIngressLanes = ValidatorIds
          BY <2>1 DEF AsyncIngressTopologyTypeInvariant
        <4>3. [asyncIngressLanes EXCEPT
                   ![recipient][ResourceSource] = Append(@, Item)]
                    [otherRecipient] =
                 asyncIngressLanes[otherRecipient]
          BY <2>2, <3>1, <4>1, <4>2,
             FunctionalUpdateAwayFromKey
        <4>4. NextLanes[otherRecipient] =
                 asyncIngressLanes[otherRecipient]
          BY <2>3, <4>3
        <4>5. AsyncIngressDepthFor(
                 NextLanes, otherRecipient) =
               IngressDepth(otherRecipient)
          BY <4>4, Isa
             DEF AsyncIngressDepthFor, IngressDepth,
                 AsyncIngressPairIndicesFor,
                 IngressLaneDepth, IngressLane
        <4>6. IngressProtectedSourcesFor(
                 NextLanes, otherRecipient) =
               IngressProtectedSourcesFor(
                 asyncIngressLanes, otherRecipient)
          BY <4>4, Isa
             DEF IngressProtectedSourcesFor,
                 IngressLaneHasNonTimeoutProgressIn,
                 IngressLaneHasTimeoutVoteIn, SequenceSet
        <4>7. IngressContinuationProtectedSourcesFor(
                 NextLanes, otherRecipient) =
               IngressContinuationProtectedSourcesFor(
                 asyncIngressLanes, otherRecipient)
          BY <4>4, Isa
             DEF IngressContinuationProtectedSourcesFor,
                 IngressLaneHasNonTimeoutProgressIn,
                 IngressLaneHasTimeoutVoteIn,
                 IngressLaneHasTransportCompletionIn, SequenceSet
        <4>7t. IngressTimeoutVoteProtectedSourcesFor(
                  NextLanes, otherRecipient) =
                IngressTimeoutVoteProtectedSourcesFor(
                  asyncIngressLanes, otherRecipient)
          BY <4>4, Isa
             DEF IngressTimeoutVoteProtectedSourcesFor,
                 IngressLaneHasTimeoutVoteIn, SequenceSet
        <4>7c. IngressTransportCompletionProtectedSourcesFor(
                  NextLanes, otherRecipient) =
                IngressTransportCompletionProtectedSourcesFor(
                  asyncIngressLanes, otherRecipient)
          BY <4>4, Isa
             DEF IngressTransportCompletionProtectedSourcesFor,
                 IngressLaneHasTransportCompletionIn, SequenceSet
        <4>8. IngressProtectedSlotCountFor(
                 NextLanes, otherRecipient) =
               IngressProtectedSlotCountFor(
                 asyncIngressLanes, otherRecipient)
          BY <4>6, <4>7, <4>7t, <4>7c
             DEF IngressProtectedSlotCountFor
        <4> QED BY <4>5, <4>6, <4>7, <4>7t, <4>7c, <4>8
      <3> QED BY <3>1
    <2>15. AsyncIngressDepthFor(NextLanes, recipient)
                + IngressProtectedSlotCountFor(NextLanes, recipient)
              <= AsyncIngressCapacity
      BY <2>4, <2>5, <2>6, <2>8, <2>13,
         NaturalReservedCapacityStep
    <2>16. \A otherRecipient \in ValidatorIds,
                otherSource \in AsyncIngressSources:
               Len(NextLanes[otherRecipient][otherSource]) <=
                 AsyncIngressCapacity
      <3>1. ASSUME NEW otherRecipient \in ValidatorIds,
                     NEW otherSource \in AsyncIngressSources
             PROVE Len(NextLanes[otherRecipient][otherSource]) <=
                     AsyncIngressCapacity
        <4>1. Len(asyncIngressLanes[otherRecipient][otherSource]) <=
                 AsyncIngressCapacity
          BY <2>1, <3>1
             DEF AsyncIngressCapacityTypeInvariant,
                 IngressLaneDepth, IngressLane
        <4>2. Len(NextLanes[otherRecipient][otherSource]) =
                 IF otherRecipient = recipient
                      /\ otherSource = ResourceSource
                 THEN Len(
                        asyncIngressLanes[otherRecipient][otherSource]) + 1
                 ELSE Len(
                        asyncIngressLanes[otherRecipient][otherSource])
          BY <2>1, <2>2, <2>3, <3>1,
             NestedIngressAppendLaneFacts
             DEF AsyncIngressTopologyTypeInvariant,
                 AsyncIngressContentTypeInvariant, IngressLane
        <4>3. CASE otherRecipient = recipient
                     /\ otherSource = ResourceSource
          BY <2>5, <2>10, <2>11, <3>1, <4>2, <4>3, SMT
        <4>4. CASE ~(otherRecipient = recipient
                       /\ otherSource = ResourceSource)
          BY <4>1, <4>2, <4>4
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>1
    <2>17. ASSUME NEW otherRecipient \in ValidatorIds
             PROVE /\ AsyncIngressDepthFor(
                          NextLanes, otherRecipient)
                            <= AsyncIngressCapacity
                   /\ AsyncIngressDepthFor(
                          NextLanes, otherRecipient)
                        + IngressProtectedSlotCountFor(
                            NextLanes, otherRecipient)
                            <= AsyncIngressCapacity
      <3>1. CASE otherRecipient = recipient
        <4>1. AsyncIngressDepthFor(NextLanes, recipient) \in Nat
          BY <2>8, <2>13, SMT
        <4>2. AsyncIngressDepthFor(NextLanes, recipient)
                   <= AsyncIngressCapacity
          BY <2>5, <2>6, <2>15, <4>1,
             NaturalSumBoundProjectsLeft
        <4> QED BY <2>15, <3>1, <4>2
      <3>2. CASE otherRecipient # recipient
        <4>1. otherRecipient \in ValidatorIds \ {recipient}
          BY <2>17, <3>2, Isa
        <4>2. /\ IngressDepth(otherRecipient)
                        <= AsyncIngressCapacity
               /\ IngressDepth(otherRecipient)
                    + IngressProtectedSlotCountFor(
                        asyncIngressLanes, otherRecipient)
                        <= AsyncIngressCapacity
          BY <2>1, <2>17
             DEF AsyncIngressCapacityTypeInvariant
        <4> QED BY <2>14, <4>1, <4>2
      <3> QED BY <3>1, <3>2
    <2>18. \A otherRecipient \in ValidatorIds:
               /\ \A otherSource \in AsyncIngressSources:
                    (IngressLaneDepth(
                       otherRecipient, otherSource))' <=
                      AsyncIngressCapacity
               /\ (IngressDepth(otherRecipient))' <=
                    AsyncIngressCapacity
               /\ (IngressDepth(otherRecipient))'
                    + IngressProtectedSlotCountFor(
                        asyncIngressLanes', otherRecipient)
                      <= AsyncIngressCapacity
      <3>1. ASSUME NEW otherRecipient \in ValidatorIds
             PROVE /\ \A otherSource \in AsyncIngressSources:
                          (IngressLaneDepth(
                             otherRecipient, otherSource))' <=
                            AsyncIngressCapacity
                   /\ (IngressDepth(otherRecipient))' <=
                          AsyncIngressCapacity
                   /\ (IngressDepth(otherRecipient))'
                        + IngressProtectedSlotCountFor(
                            asyncIngressLanes', otherRecipient)
                          <= AsyncIngressCapacity
        <4>1. /\ AsyncIngressDepthFor(
                      NextLanes, otherRecipient) <=
                        AsyncIngressCapacity
               /\ AsyncIngressDepthFor(
                      NextLanes, otherRecipient)
                    + IngressProtectedSlotCountFor(
                        NextLanes, otherRecipient)
                      <= AsyncIngressCapacity
          BY <2>17, <3>1
        <4>2. \A otherSource \in AsyncIngressSources:
                 (IngressLaneDepth(
                    otherRecipient, otherSource))' =
                   Len(NextLanes[otherRecipient][otherSource])
          BY <2>3 DEF IngressLaneDepth, IngressLane
        <4>3. \A otherSource \in AsyncIngressSources:
                 (IngressLaneDepth(
                    otherRecipient, otherSource))' <=
                   AsyncIngressCapacity
          BY <2>16, <3>1, <4>2
        <4>4. (IngressDepth(otherRecipient))' =
                 AsyncIngressDepthFor(NextLanes, otherRecipient)
          BY <2>3
             DEF IngressDepth, AsyncIngressDepthFor,
                 AsyncIngressPairIndicesFor,
                 IngressLaneDepth, IngressLane
        <4>5. IngressProtectedSlotCountFor(
                 asyncIngressLanes', otherRecipient) =
               IngressProtectedSlotCountFor(
                 NextLanes, otherRecipient)
          BY <2>3
        <4> QED BY <4>1, <4>3, <4>4, <4>5
      <3> QED BY <3>1
    <2> QED BY <2>18 DEF AsyncIngressCapacityTypeInvariant
  <1> QED BY <1>1

=============================================================================
