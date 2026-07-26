---- MODULE SumeragiV2AsyncDeadlockProofs ----
EXTENDS SumeragiV2AsyncStage2Proofs

(***************************************************************************
Productive deadlock freedom includes concrete terminating local work.

The top-level candidate/Serve ranks deliberately omit finite prefixes which
do not yet move their owner between scheduler stages.  The child model
therefore supplies the parameterized base property with exact strict-step
witnesses: the proved auxiliary/capacity ranks, Busy and handoff ranks,
completion I/O depth, ingress/transport ownership counts, and the local-runner
contract debt. That last debt is a per-validator ghost projection of the
ledgered `runtime-after-gst` trusted contract. It is not production all-voter
state, and the Rust loop cannot prove the host scheduler or admitted-work
latency bound. `RuntimeInvocationDebt` is used only while an overdue responsive
packet blocks the clock; it records completion of the one current serialized
invocation before the runner returns to Local, rather than classifying an
arbitrary RunNode turn as productive.
***************************************************************************)

RuntimeInvocationDebt(node) ==
  IF asyncRunnerPhase[node] = "Runtime" THEN 1 ELSE 0

LocalRunnerServiceContractDebt(node) ==
  IF node \in LocalRunnerServiceOwners
       /\ asyncNodeServiceDeadlines[node] <= asyncNow
  THEN 1
  ELSE 0

Stage2BusyLocalWorkDecreaseStep ==
  \E target, witness, phase \in 1..2:
    /\ Stage2BusyWitnessBlocked(target, witness, phase)
    /\ Stage2BusyPhaseGoal(target, phase)'

Stage2HandoffLocalWorkDecreaseStep ==
  \E candidate \in AsyncCandidateSet, distance \in 0..2:
    /\ Stage2IdleHandoffAtDistance(candidate, distance)
    /\ Stage2IdleHandoffCursorProgress(candidate, distance)'

Stage3AuxLocalWorkDecreaseStep ==
  \E candidate \in AsyncCandidateSet, position \in Nat,
     rank \in ReadyRunAuxCarrier:
    /\ Stage3AuxBlocked(candidate, position, rank)
    /\ Stage3AuxProgress(candidate, position, rank)'

Stage4AuxLocalWorkDecreaseStep ==
  \E candidate \in AsyncCandidateSet, position \in Nat,
     rank \in ReadyRunAuxCarrier:
    /\ ReadyBlockedAtAux(candidate, position, rank)
    /\ Stage4AuxProgress(candidate, position, rank)'

Stage4CapacityLocalWorkDecreaseStep ==
  \E candidate \in AsyncCandidateSet, position \in Nat,
     rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ Stage4CapacityProgress(candidate, position, rank)'

Stage6NonCompletionCapacityLocalWorkDecreaseStep ==
  \E candidate \in AsyncCandidateSet, position \in Nat,
     rank \in Stage4CapacityCarrier:
    /\ Stage6NonCompletionCapacityBlockedAtRank(
         candidate, position, rank)
    /\ Stage6NonCompletionCapacityProgress(
         candidate, position, rank)'

Stage6CompletionIoLocalWorkDecreaseStep ==
  \E candidate \in AsyncCandidateSet, position, depth \in Nat:
    /\ Stage6CompletionIoBlockedAtDepth(candidate, position, depth)
    /\ Stage6CompletionIoProgress(candidate, position, depth)'

Stage6OwedReadyLocalWorkDecreaseStep ==
  \E candidate \in AsyncCandidateSet, position \in Nat,
     rank \in ReadyRunAuxCarrier:
    /\ Stage6OwedReadyBlockedAtAux(candidate, position, rank)
    /\ Stage6OwedReadyAuxProgress(candidate, position, rank)'

Stage6PreAdmissionLocalWorkDecreaseStep ==
  \E candidate \in AsyncCandidateSet, position \in Nat,
     rank \in ReadyRunAuxCarrier:
    /\ Stage6PreAdmissionBlockedAtAux(candidate, position, rank)
    /\ Stage6PreAdmissionAuxProgress(candidate, position, rank)'

RunnerPrefixLocalWorkDecreaseStep ==
  \E node \in AsyncCurrentResponsiveVoters
                 \cup asyncHistoricalRecoveryTargets:
    RuntimeReachRank(node)' < RuntimeReachRank(node)

OverdueTransportRuntimeInvocationDecreaseStep ==
  \E recipient \in AsyncCurrentResponsiveVoters
                    \cup asyncHistoricalRecoveryTargets,
     source \in AsyncIngressSources:
    /\ DueSourcePackets(recipient, source) # {}
    /\ ~NodeHasApplication(recipient)
    /\ RuntimeInvocationDebt(recipient)'
         < RuntimeInvocationDebt(recipient)

ResponsivePacketPairAt(initialContext, recipient, source) ==
  \/ /\ recipient \in AsyncVotersAt(initialContext)
        /\ source \in AsyncVotersAt(initialContext)
  \/ HistoricalRecoveryPacketCorridor(recipient, source)

DueIngressPacketCanCoalesce(item) ==
  /\ IngressHasCoalescingOwner(item)
  /\ \/ item.kind # "CertifiedResponse"
     \/ CertifiedResponseClaimMatches(item)

DueIngressPacketCanEnter(recipient, source) ==
  LET item == OldestDueSourcePacket(recipient, source).item
  IN \/ DueIngressPacketCanCoalesce(item)
     \/ /\ ~IngressHasCoalescingOwner(item)
           /\ CanAdmitIngressItem(item)

IoDepthLocalWorkDecreaseStep ==
  \E node \in AsyncCurrentResponsiveVoters
                 \cup asyncHistoricalRecoveryTargets:
    AsyncIoQueueDepth(node)' < AsyncIoQueueDepth(node)

IngressDepthLocalWorkDecreaseStep ==
  \E node \in AsyncCurrentResponsiveVoters
                 \cup asyncHistoricalRecoveryTargets:
    IngressDepth(node)' < IngressDepth(node)

TransportOutstandingLocalWorkDecreaseStep ==
  Cardinality(asyncTransport') < Cardinality(asyncTransport)

LocalRunnerServiceContractDecreaseStep ==
  \E node \in LocalRunnerServiceOwners:
    LocalRunnerServiceContractDebt(node)'
      < LocalRunnerServiceContractDebt(node)

AsyncTerminatingLocalWorkDecreaseStep ==
  \/ Stage2BusyLocalWorkDecreaseStep
  \/ Stage2HandoffLocalWorkDecreaseStep
  \/ Stage3AuxLocalWorkDecreaseStep
  \/ Stage4AuxLocalWorkDecreaseStep
  \/ Stage4CapacityLocalWorkDecreaseStep
  \/ Stage6NonCompletionCapacityLocalWorkDecreaseStep
  \/ Stage6CompletionIoLocalWorkDecreaseStep
  \/ Stage6OwedReadyLocalWorkDecreaseStep
  \/ Stage6PreAdmissionLocalWorkDecreaseStep
  \/ RunnerPrefixLocalWorkDecreaseStep
  \/ OverdueTransportRuntimeInvocationDecreaseStep
  \/ IoDepthLocalWorkDecreaseStep
  \/ IngressDepthLocalWorkDecreaseStep
  \/ TransportOutstandingLocalWorkDecreaseStep
  \/ LocalRunnerServiceContractDecreaseStep

THEOREM AsyncTerminatingLocalWorkHasStrictWitness ==
  AsyncTerminatingLocalWorkDecreaseStep
    => \/ Stage2BusyLocalWorkDecreaseStep
       \/ Stage2HandoffLocalWorkDecreaseStep
       \/ Stage3AuxLocalWorkDecreaseStep
       \/ Stage4AuxLocalWorkDecreaseStep
       \/ Stage4CapacityLocalWorkDecreaseStep
       \/ Stage6NonCompletionCapacityLocalWorkDecreaseStep
       \/ Stage6CompletionIoLocalWorkDecreaseStep
       \/ Stage6OwedReadyLocalWorkDecreaseStep
       \/ Stage6PreAdmissionLocalWorkDecreaseStep
       \/ RunnerPrefixLocalWorkDecreaseStep
       \/ OverdueTransportRuntimeInvocationDecreaseStep
       \/ IoDepthLocalWorkDecreaseStep
       \/ IngressDepthLocalWorkDecreaseStep
       \/ TransportOutstandingLocalWorkDecreaseStep
       \/ LocalRunnerServiceContractDecreaseStep
BY DEF AsyncTerminatingLocalWorkDecreaseStep

THEOREM RunNodePrefixStrictlyDecreasesLocalWork ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ node \in AsyncCurrentResponsiveVoters
                   \cup asyncHistoricalRecoveryTargets
    /\ (LocalAdmissionStep(node) \/ IngressDrainStep(node))
    => RunnerPrefixLocalWorkDecreaseStep
BY LocalAdmissionStrictlyDecreasesRuntimeReach,
   IngressDrainStrictlyDecreasesRuntimeReach, Isa
   DEF RunnerPrefixLocalWorkDecreaseStep,
       AsyncHistoricalRecoveryTypeInvariant

THEOREM RuntimeInvocationStrictlyDecreasesOverdueTransportWork ==
  \A recipient \in AsyncCurrentResponsiveVoters
                    \cup asyncHistoricalRecoveryTargets,
     source \in AsyncIngressSources:
    /\ DueSourcePackets(recipient, source) # {}
    /\ ~NodeHasApplication(recipient)
    /\ SerializedRuntimeStep(recipient)
    => OverdueTransportRuntimeInvocationDecreaseStep
BY Isa
   DEF OverdueTransportRuntimeInvocationDecreaseStep,
       RuntimeInvocationDebt, SerializedRuntimeStep

THEOREM RunnerServiceStrictlyClearsDueGate ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ LocalRunnerServiceContractDebt(node) = 1
    /\ (RunNode(node)
          \/ RunHistoricalRecoveryNode(node)
          \/ RunHistoricalServer(node))
    => LocalRunnerServiceContractDecreaseStep
BY SMT
   DEF LocalRunnerServiceContractDecreaseStep,
       LocalRunnerServiceContractDebt, LocalRunnerServiceOwners,
       RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer, AsyncConfiguration

THEOREM IoWorkerStrictlyDecreasesLocalWork ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ node \in AsyncCurrentResponsiveVoters
                   \cup asyncHistoricalRecoveryTargets
    /\ ServiceIoWorkerWork(node)
    => IoDepthLocalWorkDecreaseStep
BY ServiceIoWorkerDropsQueueDepth, SMT
   DEF IoDepthLocalWorkDecreaseStep,
       AsyncHistoricalRecoveryTypeInvariant

THEOREM IngressDrainStrictlyDecreasesLocalWork ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ node \in AsyncCurrentResponsiveVoters
                   \cup asyncHistoricalRecoveryTargets
    /\ (DrainFairIngressSelected(node)
          \/ DrainHistoricalIngressSelected(node))
    => IngressDepthLocalWorkDecreaseStep
BY IngressDepthDropsByOneAfterLaneRemoval,
   AsyncIngressSourcesAreFinite, Isa
   DEF IngressDepthLocalWorkDecreaseStep, IngressDepth,
       AsyncIngressDepthFor, DrainFairIngressSelected,
       DrainHistoricalIngressSelected, PopSelectedIngress,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIngressTypeInvariant, AsyncIngressTopologyTypeInvariant,
       AsyncIngressCapacityTypeInvariant, AsyncConfiguration

THEOREM IngressAdmissionStrictlyDecreasesTransportWork ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncTypeInvariant
    /\ AdmitIngressPacket(recipient, source)
    => TransportOutstandingLocalWorkDecreaseStep
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                AdmitIngressPacket(recipient, source)
         PROVE TransportOutstandingLocalWorkDecreaseStep
    <2>1. /\ IsFiniteSet(asyncTransport)
           /\ DueSourcePackets(recipient, source) # {}
           /\ OldestDueSourcePacket(recipient, source)
                \in asyncTransport
      BY <1>1, OldestDueSourcePacketFacts, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant,
             AsyncPacketContentTypeInvariant,
             AdmitIngressPacket, AdmitHiddenPacket,
             CoalesceHiddenPacket, DueSourcePackets
    <2>2. asyncTransport' =
             asyncTransport
               \ {OldestDueSourcePacket(recipient, source)}
      BY <1>1
         DEF AdmitIngressPacket, AdmitHiddenPacket,
             CoalesceHiddenPacket
    <2>3. Cardinality(asyncTransport') + 1
             = Cardinality(asyncTransport)
      BY <2>1, <2>2, FS_RemoveElement, FS_CardinalityType, SMT
    <2> QED BY <2>3, SMT
         DEF TransportOutstandingLocalWorkDecreaseStep
  <1> QED BY <1>1

(***************************************************************************
The ingress reservation is useful to deadlock freedom only after proving the
converse needed by the full-ingress branch: a fresh typed packet cannot be
rejected by an empty recipient.  Appending the first item consumes exactly
one of that source's four (or, for the untrusted aggregate, two) protected
slots.  Thus a rejected fresh packet witnesses existing recipient-local
ingress work; this is not an assumption about nominal queue capacity.
***************************************************************************)

THEOREM ZeroIngressDepthMeansEveryLaneEmpty ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncTypeInvariant
    /\ IngressDepth(recipient) = 0
    => IngressLane(recipient, source) = <<>>
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                IngressDepth(recipient) = 0
         PROVE IngressLane(recipient, source) = <<>>
    <2>1. /\ AsyncIngressCapacity \in Nat \ {0}
           /\ IngressLane(recipient, source)
                \in Seq(Range(IngressLane(recipient, source)))
           /\ Len(IngressLane(recipient, source)) \in Nat
      BY <1>1, LenProperties
         DEF AsyncTypeInvariant, TypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncIngressTypeInvariant,
             AsyncIngressContentTypeInvariant, AsyncConfiguration
    <2>2. IsFiniteSet(
             AsyncIngressPairIndicesFor(asyncIngressLanes, recipient))
      <3>1. /\ IsFiniteSet(AsyncIngressSources)
             /\ IsFiniteSet(1..AsyncIngressCapacity)
        BY <1>1, AsyncIngressSourcesAreFinite, FS_Interval
           DEF AsyncTypeInvariant, TypeInvariant, AsyncConfiguration
      <3> QED BY <3>1, FS_Product, FS_Subset
           DEF AsyncIngressPairIndicesFor
    <2>3. Cardinality(
             AsyncIngressPairIndicesFor(asyncIngressLanes, recipient)) = 0
      BY <1>1 DEF IngressDepth, AsyncIngressDepthFor
    <2>4. AsyncIngressPairIndicesFor(asyncIngressLanes, recipient) = {}
      BY <2>2, <2>3, FS_EmptySet
    <2>5. Len(IngressLane(recipient, source)) = 0
      <3>1. ASSUME Len(IngressLane(recipient, source)) # 0
             PROVE FALSE
        <4>1. Len(IngressLane(recipient, source)) > 0
          BY <2>1, <3>1, SMT
        <4>2. <<source, 1>> \in
                 AsyncIngressPairIndicesFor(
                   asyncIngressLanes, recipient)
          BY <2>1, <4>1, SMT
             DEF AsyncIngressPairIndicesFor, IngressLane
        <4> QED BY <2>4, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>1, <2>5, EmptySeq
  <1> QED BY <1>1

THEOREM FirstIngressItemConsumesOneProtectedSlot ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    \A item:
      /\ AsyncTypeInvariant
      /\ AsyncItemTyped(item)
      /\ item.envelope.recipient = recipient
      /\ item.source = source
      /\ IngressLane(recipient, source) = <<>>
      => IngressProtectedSlotCountAfterAdmission(item) + 1 =
           IngressProtectedSlotCountFor(asyncIngressLanes, recipient)
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                NEW item,
                AsyncTypeInvariant,
                AsyncItemTyped(item),
                item.envelope.recipient = recipient,
                item.source = source,
                IngressLane(recipient, source) = <<>>
         PROVE IngressProtectedSlotCountAfterAdmission(item) + 1 =
                 IngressProtectedSlotCountFor(
                   asyncIngressLanes, recipient)
    <2> DEFINE After == IngressLanesAfterAdmission(item)
    <2>1. /\ AsyncConfiguration
           /\ ModelConfiguration
      BY <1>1 DEF AsyncTypeInvariant, TypeInvariant
    <2>2. /\ After[recipient][source] = <<item>>
           /\ \A otherSource \in AsyncIngressSources \ {source}:
                After[recipient][otherSource] =
                  asyncIngressLanes[recipient][otherSource]
      BY <1>1, AppendSequenceFacts, Isa
         DEF After, IngressLanesAfterAdmission, IngressLane
    <2>3. IngressSourceProtectionPotential(source, <<item>>) + 1 =
             IngressSourceProtectionPotential(source, <<>>)
      BY <1>1, EmptySequenceFacts, SMT
         DEF IngressSourceProtectionPotential,
             IngressSequenceHasNonTimeoutProgress,
             IngressSequenceHasTimeoutVote,
             IngressSequenceHasTransportCompletion,
             IngressItemIsNonTimeoutProgress,
             IngressItemIsTimeoutVote,
             IngressItemIsTransportCompletion,
             IngressAdmissionClass, IngressProgressKinds,
             IngressTransportCompletionKinds, SequenceSet
    <2>4. IngressProtectedSlotCountWithoutSourceFor(
             After, recipient, source) =
           IngressProtectedSlotCountWithoutSourceFor(
             asyncIngressLanes, recipient, source)
      BY <2>2, IngressProtectedSlotsWithoutSourceAreLocal
    <2>5. /\ IngressProtectedSlotCountFor(After, recipient) =
                  IngressProtectedSlotCountWithoutSourceFor(
                    After, recipient, source)
                    + IngressSourceProtectionPotential(
                        source, After[recipient][source])
           /\ IngressProtectedSlotCountFor(
                  asyncIngressLanes, recipient) =
                  IngressProtectedSlotCountWithoutSourceFor(
                    asyncIngressLanes, recipient, source)
                    + IngressSourceProtectionPotential(
                        source,
                        asyncIngressLanes[recipient][source])
      BY <1>1, <2>1, IngressProtectedSlotCountDecomposesAtSource
    <2>6. IngressProtectedSlotCountFor(After, recipient) + 1 =
             IngressProtectedSlotCountFor(
               asyncIngressLanes, recipient)
      BY <1>1, <2>2, <2>3, <2>4, <2>5, SMT
         DEF IngressLane
    <2> QED BY <1>1, <2>6
         DEF After, IngressProtectedSlotCountAfterAdmission
  <1> QED BY <1>1

THEOREM EmptyIngressAdmitsTypedPacket ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    \A item:
      /\ AsyncTypeInvariant
      /\ AsyncItemTyped(item)
      /\ item.envelope.recipient = recipient
      /\ item.source = source
      /\ IngressDepth(recipient) = 0
      => CanAdmitIngressItem(item)
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                NEW item,
                AsyncTypeInvariant,
                AsyncItemTyped(item),
                item.envelope.recipient = recipient,
                item.source = source,
                IngressDepth(recipient) = 0
         PROVE CanAdmitIngressItem(item)
    <2>1. IngressLane(recipient, source) = <<>>
      BY <1>1, ZeroIngressDepthMeansEveryLaneEmpty
    <2>2. IngressProtectedSlotCountAfterAdmission(item) + 1 =
             IngressProtectedSlotCountFor(
               asyncIngressLanes, recipient)
      BY <1>1, <2>1, FirstIngressItemConsumesOneProtectedSlot
    <2>3. /\ IngressDepth(recipient)
                  + IngressProtectedSlotCountFor(
                      asyncIngressLanes, recipient)
                    <= AsyncIngressCapacity
           /\ AsyncIngressCapacity \in Nat \ {0}
           /\ IngressProtectedSlotCountAfterAdmission(item) \in Nat
      BY <1>1, IngressProtectedSlotCountWithoutSourceIsNatural,
         IngressSourceProtectionPotentialIsNatural, SMT
         DEF AsyncTypeInvariant, TypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncIngressTypeInvariant,
             AsyncIngressCapacityTypeInvariant, AsyncConfiguration,
             IngressProtectedSlotCountAfterAdmission,
             IngressProtectedSlotCountFor
    <2>4. IngressDepth(item.envelope.recipient) <
             IngressUsableCapacityAfterAdmission(item)
      BY <1>1, <2>2, <2>3, SMT
         DEF IngressUsableCapacityAfterAdmission
    <2>5. /\ AsyncTimeoutVoteByteGateAllows(item)
           /\ AsyncTransportCompletionOwnerGateAllows(item)
      BY <1>1, <2>1, Isa
         DEF AsyncTimeoutVoteByteGateAllows,
             AsyncTransportCompletionOwnerGateAllows,
             IngressLaneHasTimeoutVoteIn,
             IngressLaneHasTransportCompletionIn,
             IngressLane, SequenceSet
    <2> QED BY <2>4, <2>5 DEF CanAdmitIngressItem
  <1> QED BY <1>1

THEOREM RejectedFreshPacketWitnessesExistingIngress ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    \A item:
      /\ AsyncTypeInvariant
      /\ AsyncItemTyped(item)
      /\ item.envelope.recipient = recipient
      /\ item.source = source
      /\ item \notin SequenceSet(IngressLane(recipient, source))
      /\ ~CanAdmitIngressItem(item)
      => IngressDepth(recipient) > 0
BY EmptyIngressAdmitsTypedPacket, SMT

THEOREM NonemptyUndrainableHistoricalIngressHasIoWork ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ IngressDepth(node) > 0
    /\ HistoricalDrainableIngressIndices(node) = {}
    => AsyncIoQueueDepth(node) > 0
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                IngressDepth(node) > 0,
                HistoricalDrainableIngressIndices(node) = {}
         PROVE AsyncIoQueueDepth(node) > 0
    <2>1. /\ AsyncConfiguration
           /\ AsyncIngressTopologyTypeInvariant
           /\ AsyncIngressContentTypeInvariant
           /\ AsyncIngressCapacity \in Nat \ {0}
      BY <1>1
         DEF AsyncTypeInvariant, TypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncIngressTypeInvariant,
             AsyncConfiguration
    <2>2. AsyncIngressPairIndicesFor(asyncIngressLanes, node) # {}
      BY <1>1, FS_EmptySet, SMT
         DEF IngressDepth, AsyncIngressDepthFor
    <2>3. PICK pair \in
                   AsyncIngressPairIndicesFor(asyncIngressLanes, node): TRUE
      BY <2>2
    <2>4. /\ pair[1] \in AsyncIngressSources
           /\ IngressLaneDepth(node, pair[1]) > 0
           /\ pair[1] \in SequenceSet(asyncIngressReady[node])
      BY <1>1, <2>1, <2>3, SMT
         DEF AsyncIngressPairIndicesFor,
             AsyncIngressTopologyTypeInvariant
    <2>5. PICK readyIndex \in 1..Len(asyncIngressReady[node]):
             asyncIngressReady[node][readyIndex] = pair[1]
      BY <2>1, <2>4 DEF SequenceSet
    <2>6. HistoricalDrainableIngressLaneIndices(node, pair[1]) = {}
      <3>1. ASSUME
               HistoricalDrainableIngressLaneIndices(node, pair[1]) # {}
             PROVE FALSE
        <4>1. HistoricalIngressSourceCanDrain(node, pair[1])
          BY <3>1 DEF HistoricalIngressSourceCanDrain
        <4>2. readyIndex \in HistoricalDrainableIngressIndices(node)
          BY <2>5, <4>1 DEF HistoricalDrainableIngressIndices
        <4> QED BY <1>1, <4>2
      <3> QED BY <3>1
    <2>7. 1 \in 1..Len(IngressLane(node, pair[1]))
      BY <2>4, SMT DEF IngressLaneDepth
    <2>8. ~HistoricalIngressItemCanDrain(
             node, IngressLane(node, pair[1])[1])
      BY <2>6, <2>7 DEF HistoricalDrainableIngressLaneIndices
    <2>9. ~CanEnqueueIoClass(node, "Serve")
      BY <2>8, SMT DEF HistoricalIngressItemCanDrain
    <2>10. /\ AsyncIoAuxCapacity \in Nat \ {0}
            /\ AsyncIoQueueDepth(node) >= AsyncIoAuxCapacity
      BY <1>1, <2>9, SMT
         DEF AsyncTypeInvariant, TypeInvariant, AsyncConfiguration,
             CanEnqueueIoClass, AsyncIoAdmissionLimit
    <2> QED BY <2>10, SMT
  <1> QED BY <1>1

THEOREM OverdueResponsivePacketUsesFairIngressPair ==
  \A initialContext \in ContextRecords,
     packet \in OverdueResponsivePackets:
    /\ AsyncTypeInvariant
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    => LET recipient == packet.item.envelope.recipient
           source == packet.item.source
       IN /\ recipient \in ValidatorIds
          /\ source \in AsyncIngressSources
          /\ DueSourcePackets(recipient, source) # {}
          /\ ResponsivePacketPairAt(initialContext, recipient, source)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW packet \in OverdueResponsivePackets,
                AsyncTypeInvariant,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext)
         PROVE LET recipient == packet.item.envelope.recipient
                   source == packet.item.source
               IN /\ recipient \in ValidatorIds
                  /\ source \in AsyncIngressSources
                  /\ DueSourcePackets(recipient, source) # {}
                  /\ ResponsivePacketPairAt(
                       initialContext, recipient, source)
    <2>1. /\ packet \in asyncTransport
           /\ AsyncPacketTyped(packet)
      BY <1>1
         DEF OverdueResponsivePackets, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant,
             AsyncPacketContentTypeInvariant
    <2>2. LET recipient == packet.item.envelope.recipient
               source == packet.item.source
           IN /\ recipient \in ValidatorIds
              /\ source \in AsyncIngressSources
              /\ packet \in DueSourcePackets(recipient, source)
      BY <1>1, <2>1, SMT
         DEF AsyncPacketTyped, AsyncItemTyped,
             OverdueResponsivePackets, DueSourcePackets
    <2>3. LET recipient == packet.item.envelope.recipient
               source == packet.item.source
           IN ResponsivePacketPairAt(
                initialContext, recipient, source)
      BY <1>1, SMT
         DEF OverdueResponsivePackets, ResponsivePacketPairAt,
             HistoricalRecoveryPacketCorridor,
             HistoricalRecoveryTarget
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

(***************************************************************************
The restart-authority projection is the only ghost variable whose exact
successor is computed from the post-state.  Clock, ingress, historical-server,
and historical-I/O actions leave every source and exact-FetchBody owner input
unchanged.  The witness below therefore evaluates the same filter in the
pre-state, outside ENABLED's rigid quantification, and the refinement lemma
proves that this concrete witness is exactly the production-facing transition.
***************************************************************************)

HistoricalLockRestartAuthorityObservedVars ==
  <<context, nodeView, generation, prepareQCs, installedTCs,
    lockRank, lockSubject, commitIntents, decisions,
    asyncCommandQueues,
    asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncCausalQueues, asyncOutstandingWork>>

RetainedHistoricalLockRestartAuthorities ==
  {authority \in asyncHistoricalLockRestartAuthorities:
    /\ HistoricalLockRestartAuthoritySource(authority)
    /\ ~HistoricalLockRestartExactCurrentFetchOwner(authority)}

RetainedHistoricalLockRestartAuthorityFrame ==
  /\ UNCHANGED AsyncRecoveryControlVars
  /\ UNCHANGED HistoricalLockRestartAuthorityObservedVars
  /\ asyncHistoricalLockRestartAuthorities' =
       RetainedHistoricalLockRestartAuthorities

THEOREM RetainedHistoricalLockRestartAuthorityFrameRefinesTransition ==
  RetainedHistoricalLockRestartAuthorityFrame
    => AsyncHistoricalLockRestartAuthorityTransition
BY IsaT(300)
   DEF RetainedHistoricalLockRestartAuthorityFrame,
       RetainedHistoricalLockRestartAuthorities,
       HistoricalLockRestartAuthorityObservedVars,
       AsyncRecoveryControlVars,
       AsyncHistoricalLockRestartAuthorityTransition,
       ResponsiveCrashRecoveryRegistration,
       HistoricalLockRestartAuthoritySource,
       HistoricalLockRestartAuthoritySourceAfter,
       HistoricalLockRestartExactCurrentFetchOwner,
       HistoricalLockRestartExactCurrentFetchOwnerAfter, vars

(***************************************************************************
The historical-recovery runner may change an exact FetchBody owner input, so
it cannot use the retained pre-state projection above.  Expose every
post-state input as an explicit pure-kernel parameter instead.  ENABLED can
then rigidly quantify those arguments without expanding the authority
predicates into each scheduler-phase successor.
***************************************************************************)

HistoricalLockRestartAuthoritiesAt(
    currentContext, currentNodeView, currentGeneration,
    currentPrepareQCs, currentInstalledTCs,
    currentLockRank, currentLockSubject,
    currentCommitIntents, currentDecisions,
    currentCommandQueues,
    currentDeferredCompletionQueues,
    currentDeferredProgressQueues,
    currentDeferredNormalQueues,
    currentCausalQueues, currentOutstandingWork) ==
  {authority \in asyncHistoricalLockRestartAuthorities:
    /\ \E qc \in currentPrepareQCs:
         HistoricalLockRestartAuthoritySourceKernel(
           authority, qc, currentContext, currentNodeView,
           currentLockRank, currentLockSubject,
           currentInstalledTCs, currentCommitIntents, currentDecisions)
    /\ ~\E qc \in currentPrepareQCs, candidate \in AsyncCandidateSet:
         /\ HistoricalLockRestartAuthoritySourceKernel(
              authority, qc, currentContext, currentNodeView,
              currentLockRank, currentLockSubject,
              currentInstalledTCs, currentCommitIntents, currentDecisions)
         /\ HistoricalLockRestartExactCurrentFetchKernel(
              authority, qc, candidate, currentContext,
              currentNodeView, currentGeneration,
              currentCommandQueues, currentDeferredCompletionQueues,
              currentDeferredProgressQueues, currentDeferredNormalQueues,
              currentCausalQueues, currentOutstandingWork)}

PostStateHistoricalLockRestartAuthorityFrame ==
  /\ UNCHANGED AsyncRecoveryControlVars
  /\ asyncHistoricalLockRestartAuthorities' =
       HistoricalLockRestartAuthoritiesAt(
         context', nodeView', generation', prepareQCs', installedTCs',
         lockRank', lockSubject', commitIntents', decisions',
         asyncCommandQueues',
         asyncDeferredCompletionQueues',
         asyncDeferredProgressQueues',
         asyncDeferredNormalQueues',
         asyncCausalQueues', asyncOutstandingWork')

THEOREM PostStateHistoricalLockRestartAuthorityFrameRefinesTransition ==
  PostStateHistoricalLockRestartAuthorityFrame
    => AsyncHistoricalLockRestartAuthorityTransition
BY IsaT(300)
   DEF PostStateHistoricalLockRestartAuthorityFrame,
       HistoricalLockRestartAuthoritiesAt,
       AsyncHistoricalLockRestartAuthorityTransition,
       ResponsiveCrashRecoveryRegistration,
       HistoricalLockRestartAuthoritySourceAfter,
       HistoricalLockRestartExactCurrentFetchOwnerAfter,
       AsyncRecoveryControlVars

PostStateHistoricalLockRestartNonCrashOuterFrame ==
  /\ UNCHANGED up
  /\ PostStateHistoricalLockRestartAuthorityFrame
  /\ AsyncCoreOuterFrame

THEOREM PostStateHistoricalLockRestartNonCrashOuterFrameRefinesExact ==
  PostStateHistoricalLockRestartNonCrashOuterFrame
    => AsyncNonCrashOuterFrame
BY PostStateHistoricalLockRestartAuthorityFrameRefinesTransition
   DEF PostStateHistoricalLockRestartNonCrashOuterFrame,
       PostStateHistoricalLockRestartAuthorityFrame,
       AsyncNonCrashOuterFrame

RetainedHistoricalLockRestartNonCrashOuterFrame ==
  /\ UNCHANGED up
  /\ RetainedHistoricalLockRestartAuthorityFrame
  /\ AsyncCoreOuterFrame

THEOREM RetainedHistoricalLockRestartNonCrashOuterFrameRefinesExact ==
  RetainedHistoricalLockRestartNonCrashOuterFrame
    => AsyncNonCrashOuterFrame
BY RetainedHistoricalLockRestartAuthorityFrameRefinesTransition
   DEF RetainedHistoricalLockRestartNonCrashOuterFrame,
       RetainedHistoricalLockRestartAuthorityFrame,
       AsyncNonCrashOuterFrame

RetainedHistoricalLockRestartNonRunnerOuterFrame ==
  /\ UNCHANGED asyncNodeServiceDeadlines
  /\ RetainedHistoricalLockRestartNonCrashOuterFrame

THEOREM RetainedHistoricalLockRestartNonRunnerOuterFrameRefinesExact ==
  RetainedHistoricalLockRestartNonRunnerOuterFrame
    => AsyncNonRunnerOuterFrame
BY RetainedHistoricalLockRestartNonCrashOuterFrameRefinesExact
   DEF RetainedHistoricalLockRestartNonRunnerOuterFrame,
       RetainedHistoricalLockRestartNonCrashOuterFrame,
       AsyncNonRunnerOuterFrame

RetainedHistoricalLockRestartTickWitness ==
  /\ AsyncTickEnabled
  /\ asyncNow' = asyncNow + 1
  /\ UNCHANGED AsyncNonClockVars
  /\ RetainedHistoricalLockRestartNonRunnerOuterFrame

THEOREM RetainedHistoricalLockRestartTickWitnessRefinesExact ==
  RetainedHistoricalLockRestartTickWitness => AsyncTick
BY RetainedHistoricalLockRestartNonRunnerOuterFrameRefinesExact
   DEF RetainedHistoricalLockRestartTickWitness, AsyncTick

THEOREM AsyncTickGuardEnablesRetainedHistoricalLockRestartWitness ==
  AsyncTickEnabled => ENABLED RetainedHistoricalLockRestartTickWitness
BY ExpandENABLED, Isa
   DEF RetainedHistoricalLockRestartTickWitness,
       RetainedHistoricalLockRestartNonRunnerOuterFrame,
       RetainedHistoricalLockRestartNonCrashOuterFrame,
       RetainedHistoricalLockRestartAuthorityFrame,
       HistoricalLockRestartAuthorityObservedVars,
       AsyncNonClockVars, AsyncRecoveryControlVars,
       AsyncLocalAdmissionVars, AsyncIoVars,
       AsyncCoreOuterFrame, vars

RetainedHistoricalLockRestartIngressWitness(recipient, source) ==
  /\ gst
  /\ AdmitIngressPacket(recipient, source)
  /\ RetainedHistoricalLockRestartNonRunnerOuterFrame

RetainedHistoricalLockRestartCoalescingIngressWitness(recipient, source) ==
  /\ gst
  /\ CoalesceHiddenPacket(recipient, source)
  /\ RetainedHistoricalLockRestartNonRunnerOuterFrame

RetainedHistoricalLockRestartFreshIngressWitness(recipient, source) ==
  /\ gst
  /\ AdmitHiddenPacket(recipient, source)
  /\ RetainedHistoricalLockRestartNonRunnerOuterFrame

THEOREM RetainedHistoricalLockRestartIngressWitnessRefinesExact ==
  \A recipient, source:
    RetainedHistoricalLockRestartIngressWitness(recipient, source)
      => PostGstAdmitHiddenPacket(recipient, source)
BY RetainedHistoricalLockRestartNonRunnerOuterFrameRefinesExact
   DEF RetainedHistoricalLockRestartIngressWitness,
       PostGstAdmitHiddenPacket

THEOREM EnabledRetainedHistoricalLockRestartIngressWitnessRefinesExact ==
  \A recipient, source:
    ENABLED RetainedHistoricalLockRestartIngressWitness(recipient, source)
      => ENABLED (PostGstAdmitHiddenPacket(recipient, source)
                    \/ PostGstAdmitHistoricalRecoveryPacket(
                         recipient, source))
PROOF
  <1>1. ASSUME NEW recipient, NEW source,
                ENABLED RetainedHistoricalLockRestartIngressWitness(
                  recipient, source)
         PROVE ENABLED (PostGstAdmitHiddenPacket(recipient, source)
                          \/ PostGstAdmitHistoricalRecoveryPacket(
                               recipient, source))
    <2>1. RetainedHistoricalLockRestartIngressWitness(
              recipient, source) \in BOOLEAN
      BY Isa DEF RetainedHistoricalLockRestartIngressWitness
    <2>2. (PostGstAdmitHiddenPacket(recipient, source)
               \/ PostGstAdmitHistoricalRecoveryPacket(
                    recipient, source)) \in BOOLEAN
      BY Isa
         DEF PostGstAdmitHiddenPacket,
             PostGstAdmitHistoricalRecoveryPacket
    <2>3. RetainedHistoricalLockRestartIngressWitness(
              recipient, source)
             => (PostGstAdmitHiddenPacket(recipient, source)
                   \/ PostGstAdmitHistoricalRecoveryPacket(
                        recipient, source))
      BY RetainedHistoricalLockRestartIngressWitnessRefinesExact
    <2>4. ENABLED RetainedHistoricalLockRestartIngressWitness(
              recipient, source)
             => ENABLED (PostGstAdmitHiddenPacket(recipient, source)
                           \/ PostGstAdmitHistoricalRecoveryPacket(
                                recipient, source))
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED BY <1>1, <2>4
  <1> QED BY <1>1

THEOREM EnabledRetainedHistoricalLockRestartCoalescingIngressWitnessRefinesExact ==
  \A recipient, source:
    ENABLED RetainedHistoricalLockRestartCoalescingIngressWitness(
      recipient, source)
      => ENABLED (PostGstAdmitHiddenPacket(recipient, source)
                    \/ PostGstAdmitHistoricalRecoveryPacket(
                         recipient, source))
PROOF
  <1>1. ASSUME NEW recipient, NEW source,
                ENABLED
                  RetainedHistoricalLockRestartCoalescingIngressWitness(
                    recipient, source)
         PROVE ENABLED (PostGstAdmitHiddenPacket(recipient, source)
                          \/ PostGstAdmitHistoricalRecoveryPacket(
                               recipient, source))
    <2>1. RetainedHistoricalLockRestartCoalescingIngressWitness(
              recipient, source) \in BOOLEAN
      BY Isa
         DEF RetainedHistoricalLockRestartCoalescingIngressWitness
    <2>2. (PostGstAdmitHiddenPacket(recipient, source)
               \/ PostGstAdmitHistoricalRecoveryPacket(
                    recipient, source)) \in BOOLEAN
      BY Isa
         DEF PostGstAdmitHiddenPacket,
             PostGstAdmitHistoricalRecoveryPacket
    <2>3. RetainedHistoricalLockRestartCoalescingIngressWitness(
              recipient, source)
             => (PostGstAdmitHiddenPacket(recipient, source)
                   \/ PostGstAdmitHistoricalRecoveryPacket(
                        recipient, source))
      BY RetainedHistoricalLockRestartNonRunnerOuterFrameRefinesExact
         DEF RetainedHistoricalLockRestartCoalescingIngressWitness,
             PostGstAdmitHiddenPacket, AdmitIngressPacket
    <2>4. ENABLED
              RetainedHistoricalLockRestartCoalescingIngressWitness(
                recipient, source)
             => ENABLED (PostGstAdmitHiddenPacket(recipient, source)
                           \/ PostGstAdmitHistoricalRecoveryPacket(
                                recipient, source))
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED BY <1>1, <2>4
  <1> QED BY <1>1

THEOREM EnabledRetainedHistoricalLockRestartFreshIngressWitnessRefinesExact ==
  \A recipient, source:
    ENABLED RetainedHistoricalLockRestartFreshIngressWitness(
      recipient, source)
      => ENABLED (PostGstAdmitHiddenPacket(recipient, source)
                    \/ PostGstAdmitHistoricalRecoveryPacket(
                         recipient, source))
PROOF
  <1>1. ASSUME NEW recipient, NEW source,
                ENABLED RetainedHistoricalLockRestartFreshIngressWitness(
                  recipient, source)
         PROVE ENABLED (PostGstAdmitHiddenPacket(recipient, source)
                          \/ PostGstAdmitHistoricalRecoveryPacket(
                               recipient, source))
    <2>1. RetainedHistoricalLockRestartFreshIngressWitness(
              recipient, source) \in BOOLEAN
      BY Isa DEF RetainedHistoricalLockRestartFreshIngressWitness
    <2>2. (PostGstAdmitHiddenPacket(recipient, source)
               \/ PostGstAdmitHistoricalRecoveryPacket(
                    recipient, source)) \in BOOLEAN
      BY Isa
         DEF PostGstAdmitHiddenPacket,
             PostGstAdmitHistoricalRecoveryPacket
    <2>3. RetainedHistoricalLockRestartFreshIngressWitness(
              recipient, source)
             => (PostGstAdmitHiddenPacket(recipient, source)
                   \/ PostGstAdmitHistoricalRecoveryPacket(
                        recipient, source))
      BY RetainedHistoricalLockRestartNonRunnerOuterFrameRefinesExact
         DEF RetainedHistoricalLockRestartFreshIngressWitness,
             PostGstAdmitHiddenPacket, AdmitIngressPacket
    <2>4. ENABLED RetainedHistoricalLockRestartFreshIngressWitness(
              recipient, source)
             => ENABLED (PostGstAdmitHiddenPacket(recipient, source)
                           \/ PostGstAdmitHistoricalRecoveryPacket(
                                recipient, source))
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED BY <1>1, <2>4
  <1> QED BY <1>1

RetainedHistoricalLockRestartServerWitness(node) ==
  /\ gst
  /\ RunHistoricalServer(node)
  /\ RetainedHistoricalLockRestartNonCrashOuterFrame

THEOREM RetainedHistoricalLockRestartServerWitnessRefinesExact ==
  \A node:
    RetainedHistoricalLockRestartServerWitness(node)
      => PostGstRunHistoricalServer(node)
BY RetainedHistoricalLockRestartNonCrashOuterFrameRefinesExact
   DEF RetainedHistoricalLockRestartServerWitness,
       PostGstRunHistoricalServer

RetainedHistoricalLockRestartIoWorkerWitness(node) ==
  /\ gst
  /\ ServiceHistoricalRecoveryIoWorker(node)
  /\ RetainedHistoricalLockRestartNonRunnerOuterFrame

THEOREM RetainedHistoricalLockRestartIoWorkerWitnessRefinesExact ==
  \A node:
    RetainedHistoricalLockRestartIoWorkerWitness(node)
      => PostGstServiceHistoricalRecoveryIoWorker(node)
BY RetainedHistoricalLockRestartNonRunnerOuterFrameRefinesExact
   DEF RetainedHistoricalLockRestartIoWorkerWitness,
       PostGstServiceHistoricalRecoveryIoWorker

THEOREM DueIngressPacketAdmissionIsEnabled ==
  \A initialContext \in ContextRecords,
     recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncStrongTypeInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ gst
    /\ recipient \in AsyncCurrentResponsiveVoters
                    \cup asyncHistoricalRecoveryTargets
    /\ ResponsivePacketPairAt(initialContext, recipient, source)
    /\ DueSourcePackets(recipient, source) # {}
    /\ DueIngressPacketCanEnter(recipient, source)
    => ENABLED (PostGstAdmitHiddenPacket(recipient, source)
                  \/ PostGstAdmitHistoricalRecoveryPacket(
                       recipient, source))
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncStrongTypeInvariant,
                PostGstReplayQuarantineExcluded,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                gst,
                recipient \in AsyncCurrentResponsiveVoters
                                \cup asyncHistoricalRecoveryTargets,
                ResponsivePacketPairAt(
                  initialContext, recipient, source),
                DueSourcePackets(recipient, source) # {},
                DueIngressPacketCanEnter(recipient, source)
         PROVE ENABLED (PostGstAdmitHiddenPacket(recipient, source)
                          \/ PostGstAdmitHistoricalRecoveryPacket(
                               recipient, source))
    <2> DEFINE Item == OldestDueSourcePacket(recipient, source).item
    <2>1. /\ AsyncTypeInvariant
           /\ recipient \in up
           /\ ~ResponsiveReplayQuarantined(recipient)
           /\ Item.envelope.recipient = recipient
           /\ Item.source = source
      BY <1>1, AsyncStrongTypeProjectsAsyncType,
         OldestDueSourcePacketFacts, Isa
         DEF Item, PostGstReplayQuarantineExcluded,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncHistoricalRecoveryTypeInvariant,
             AsyncRecoveryTypeInvariant, AsyncCurrentResponsiveVoters
    <2>2. CASE DueIngressPacketCanCoalesce(Item)
      <3>1. ENABLED
               RetainedHistoricalLockRestartCoalescingIngressWitness(
                 recipient, source)
        BY <1>1, <2>1, <2>2,
           AutoUSE, ExpandENABLED, IsaT(300)
           DEF RetainedHistoricalLockRestartCoalescingIngressWitness,
               RetainedHistoricalLockRestartNonRunnerOuterFrame,
               RetainedHistoricalLockRestartNonCrashOuterFrame,
               RetainedHistoricalLockRestartAuthorityFrame,
               HistoricalLockRestartAuthorityObservedVars,
               CoalesceHiddenPacket, DueIngressPacketCanCoalesce,
               Item, AsyncAllVars,
               AsyncSchedulerVars, AsyncIoVars, AsyncDeferredVars,
               AsyncLocalAdmissionVars, AsyncRecoveryControlVars, LeaveCausalQueues,
               AsyncCoreOuterFrame, vars
      <3>2. ENABLED
               (PostGstAdmitHiddenPacket(recipient, source)
                  \/ PostGstAdmitHistoricalRecoveryPacket(
                       recipient, source))
        BY <3>1,
           EnabledRetainedHistoricalLockRestartCoalescingIngressWitnessRefinesExact
      <3> QED BY <3>2
    <2>3. CASE ~DueIngressPacketCanCoalesce(Item)
      <3>1. /\ ~IngressHasCoalescingOwner(Item)
             /\ CanAdmitIngressItem(Item)
        BY <1>1, <2>3
           DEF DueIngressPacketCanEnter, DueIngressPacketCanCoalesce,
               Item
      <3>2. ENABLED
               RetainedHistoricalLockRestartFreshIngressWitness(
                 recipient, source)
        BY <1>1, <2>1, <2>3, <3>1,
           AutoUSE, ExpandENABLED, IsaT(300)
           DEF RetainedHistoricalLockRestartFreshIngressWitness,
               RetainedHistoricalLockRestartNonRunnerOuterFrame,
               RetainedHistoricalLockRestartNonCrashOuterFrame,
               RetainedHistoricalLockRestartAuthorityFrame,
               HistoricalLockRestartAuthorityObservedVars,
               AdmitHiddenPacket, DueIngressPacketCanCoalesce,
               Item, AsyncAllVars,
               AsyncSchedulerVars, AsyncIoVars, AsyncDeferredVars,
               AsyncLocalAdmissionVars, AsyncRecoveryControlVars, LeaveCausalQueues,
               AsyncCoreOuterFrame, vars
      <3>3. ENABLED
               (PostGstAdmitHiddenPacket(recipient, source)
                  \/ PostGstAdmitHistoricalRecoveryPacket(
                       recipient, source))
        BY <3>2,
           EnabledRetainedHistoricalLockRestartFreshIngressWitnessRefinesExact
      <3> QED BY <3>3
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM AppliedResponsiveHistoricalServerEnabledAfterGst ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ gst
    /\ NodeHasApplication(node)
    => ENABLED PostGstRunHistoricalServer(node)
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncStrongTypeInvariant,
                PostGstReplayQuarantineExcluded,
                gst,
                NodeHasApplication(node)
         PROVE ENABLED PostGstRunHistoricalServer(node)
    <2>1a. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>1b. node \in ValidatorIds
      BY <1>1, <2>1a, AsyncCurrentResponsiveVotersAreValidators
    <2>1c. node \in up
      BY <1>1, GstResponsiveNodesAreUp
         DEF AsyncStrongTypeInvariant
    <2>1d. ~ResponsiveReplayQuarantined(node)
      BY <1>1, <2>1b
         DEF PostGstReplayQuarantineExcluded
    <2>1. /\ node \in AsyncResponsiveAppliedArchiveServers
           /\ ~ResponsiveReplayQuarantined(node)
      BY <1>1, <2>1b, <2>1c, <2>1d, Isa
         DEF AsyncCurrentResponsiveVoters,
             AsyncResponsiveAppliedArchiveServers,
             AsyncResponsiveOnlineArchiveServers,
             AsyncResponsiveArchiveServers, AsyncArchiveServerIds
    <2>2. RetainedHistoricalLockRestartServerWitness(node) \in BOOLEAN
      BY Isa DEF RetainedHistoricalLockRestartServerWitness
    <2>3. PostGstRunHistoricalServer(node) \in BOOLEAN
      BY Isa DEF PostGstRunHistoricalServer
    <2>4. RetainedHistoricalLockRestartServerWitness(node)
             => PostGstRunHistoricalServer(node)
      BY RetainedHistoricalLockRestartServerWitnessRefinesExact
    <2>5. ENABLED RetainedHistoricalLockRestartServerWitness(node)
             => ENABLED PostGstRunHistoricalServer(node)
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2>6. CASE HistoricalDrainableIngressIndices(node) # {}
      <3>1a. FirstHistoricalDrainableIngressIndex(node)
                  \in HistoricalDrainableIngressIndices(node)
        BY <2>6, FirstHistoricalDrainableIndexIsDrainable
      <3>1b. /\ FirstHistoricalDrainableIngressIndex(node)
                      \in 1..Len(asyncIngressReady[node])
               /\ HistoricalIngressSourceCanDrain(
                    node,
                    asyncIngressReady[node][
                      FirstHistoricalDrainableIngressIndex(node)])
        BY <3>1a DEF HistoricalDrainableIngressIndices
      <3>1c. HistoricalSelectedIngressLaneIndex(
                  node, FirstHistoricalDrainableIngressIndex(node))
                \in HistoricalDrainableIngressLaneIndices(
                     node,
                     asyncIngressReady[node][
                       FirstHistoricalDrainableIngressIndex(node)])
        BY <3>1b, FirstHistoricalDrainableIngressLaneIndexIsDrainable
           DEF HistoricalIngressSourceCanDrain,
               HistoricalSelectedIngressLaneIndex
      <3>1d. HistoricalSelectedIngressLaneIndex(
                  node, FirstHistoricalDrainableIngressIndex(node))
                \in 1..Len(
                     IngressLane(
                       node,
                       asyncIngressReady[node][
                         FirstHistoricalDrainableIngressIndex(node)]))
        BY <3>1c DEF HistoricalDrainableIngressLaneIndices
      <3>2. ENABLED RetainedHistoricalLockRestartServerWitness(node)
        BY <1>1, <2>1, <2>6, <3>1b, <3>1d,
           FirstHistoricalDrainableIndexIsDrainable,
           FirstHistoricalDrainableIngressLaneIndexIsDrainable,
           AutoUSE, ExpandENABLED, SMTT(300)
           DEF RetainedHistoricalLockRestartServerWitness,
               RetainedHistoricalLockRestartNonCrashOuterFrame,
               RetainedHistoricalLockRestartAuthorityFrame,
               HistoricalLockRestartAuthorityObservedVars,
               RunHistoricalServer, DrainHistoricalIngressSelected,
               PopSelectedIngress,
               AsyncCoreOuterFrame, AsyncAllVars, AsyncSchedulerVars,
               AsyncRecoveryVars, AsyncRecoveryControlVars,
               AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
               LeaveCausalQueues, vars
      <3> QED BY <3>2, <2>5
    <2>7. CASE HistoricalDrainableIngressIndices(node) = {}
      <3>1. ENABLED RetainedHistoricalLockRestartServerWitness(node)
        BY <1>1, <2>1, <2>7,
           AutoUSE, ExpandENABLED, IsaT(300)
           DEF RetainedHistoricalLockRestartServerWitness,
               RetainedHistoricalLockRestartNonCrashOuterFrame,
               RetainedHistoricalLockRestartAuthorityFrame,
               HistoricalLockRestartAuthorityObservedVars,
               RunHistoricalServer, HistoricalIdleStep,
               AsyncCoreOuterFrame, AsyncAllVars, AsyncSchedulerVars,
               AsyncRecoveryVars, AsyncRecoveryControlVars,
               AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
               LeaveCausalQueues, vars
      <3> QED BY <3>1, <2>5
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

PostStateHistoricalRecoveryRunnerEnvelope(node) ==
  /\ HistoricalRecoveryTarget(node)
  /\ node \in up
  /\ ~NodeHasApplication(node)
  /\ ~ResponsiveReplayQuarantined(node)
  /\ NodeServiceFrame(node)
  /\ PostStateHistoricalLockRestartNonCrashOuterFrame

PostStateHistoricalRecoveryLocalRunnerWitness(node) ==
  /\ gst
  /\ LocalAdmissionStep(node)
  /\ PostStateHistoricalRecoveryRunnerEnvelope(node)

PostStateHistoricalRecoveryIngressRunnerWitness(node) ==
  /\ gst
  /\ IngressDrainStep(node)
  /\ PostStateHistoricalRecoveryRunnerEnvelope(node)

PostStateHistoricalRecoveryRuntimeRunnerWitness(node) ==
  /\ gst
  /\ SerializedRuntimeStep(node)
  /\ PostStateHistoricalRecoveryRunnerEnvelope(node)

HistoricalRecoveryRunnerCurrentGuard(node) ==
  /\ node \in asyncHistoricalRecoveryTargets
  /\ gst
  /\ node \in up
  /\ ~NodeHasApplication(node)
  /\ ~ResponsiveReplayQuarantined(node)

(***************************************************************************
Construct each local-runner successor from its exact scheduler guard.  This
avoids expanding `ENABLED LocalAdmissionStep` once as a hypothesis and again
inside the framed target.  The no-admission arm preserves every authority
observation, so it uses the already-proved retained projection while advancing
the runner phase.
***************************************************************************)

HistoricalRecoveryLocalPhaseAdvanceBaseWitness(node) ==
  /\ gst
  /\ asyncRunnerPhase[node] = "Local"
  /\ ~LocalAdmissionCanAdvance(node)
  /\ LocalPhaseAdvanceStep(node)
  /\ HistoricalRecoveryTarget(node)
  /\ node \in up
  /\ ~NodeHasApplication(node)
  /\ ~ResponsiveReplayQuarantined(node)
  /\ NodeServiceFrame(node)
  /\ UNCHANGED AsyncRecoveryControlVars

RetainedHistoricalRecoveryLocalPhaseAdvanceWitness(node) ==
  /\ HistoricalRecoveryLocalPhaseAdvanceBaseWitness(node)
  /\ asyncHistoricalLockRestartAuthorities' =
       RetainedHistoricalLockRestartAuthorities

THEOREM RetainedHistoricalRecoveryLocalPhaseAdvanceProvidesOuterFrame ==
  \A node:
    RetainedHistoricalRecoveryLocalPhaseAdvanceWitness(node)
      => RetainedHistoricalLockRestartNonCrashOuterFrame
BY Isa
   DEF RetainedHistoricalRecoveryLocalPhaseAdvanceWitness,
       HistoricalRecoveryLocalPhaseAdvanceBaseWitness,
       RetainedHistoricalLockRestartNonCrashOuterFrame,
       RetainedHistoricalLockRestartAuthorityFrame,
       HistoricalLockRestartAuthorityObservedVars,
       LocalPhaseAdvanceStep, RecordBlockedCausalDebt,
       LeaveCausalQueues, AsyncCoreOuterFrame,
       AsyncIoVars, AsyncDeferredVars, AsyncRecoveryControlVars, vars

THEOREM RetainedHistoricalRecoveryLocalPhaseAdvanceUsesLocalAdmission ==
  \A node:
    RetainedHistoricalRecoveryLocalPhaseAdvanceWitness(node)
      => LocalAdmissionStep(node)
BY Isa
   DEF RetainedHistoricalRecoveryLocalPhaseAdvanceWitness,
       HistoricalRecoveryLocalPhaseAdvanceBaseWitness,
       LocalPhaseAdvanceStep, LocalAdmissionStep,
       AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
       EnqueueCandidate, LeaveCausalQueues,
       AsyncIoVars, AsyncDeferredVars, vars

THEOREM RetainedHistoricalRecoveryLocalPhaseAdvanceRefinesExact ==
  \A node:
    RetainedHistoricalRecoveryLocalPhaseAdvanceWitness(node)
      => PostGstRunHistoricalRecoveryNode(node)
BY RetainedHistoricalRecoveryLocalPhaseAdvanceUsesLocalAdmission,
   RetainedHistoricalRecoveryLocalPhaseAdvanceProvidesOuterFrame,
   RetainedHistoricalLockRestartNonCrashOuterFrameRefinesExact, Isa
   DEF RetainedHistoricalRecoveryLocalPhaseAdvanceWitness,
       HistoricalRecoveryLocalPhaseAdvanceBaseWitness,
       PostGstRunHistoricalRecoveryNode,
       RunHistoricalRecoveryNode, RunNodeWork, NodeServiceFrame

THEOREM HistoricalRecoveryLocalPhaseAdvanceBaseEnabled ==
  \A node \in ValidatorIds:
    /\ HistoricalRecoveryRunnerCurrentGuard(node)
    /\ asyncRunnerPhase[node] = "Local"
    /\ ~LocalAdmissionCanAdvance(node)
    => ENABLED HistoricalRecoveryLocalPhaseAdvanceBaseWitness(node)
BY AutoUSE, ExpandENABLED, Isa
   DEF HistoricalRecoveryRunnerCurrentGuard,
       HistoricalRecoveryLocalPhaseAdvanceBaseWitness,
       HistoricalRecoveryTarget,
       LocalPhaseAdvanceStep, RecordBlockedCausalDebt,
       LeaveCausalQueues, NodeServiceFrame,
       AsyncIoVars, AsyncDeferredVars, AsyncRecoveryControlVars, vars

THEOREM HistoricalRecoveryLocalPhaseAdvanceEnabled ==
  \A node \in ValidatorIds:
    /\ HistoricalRecoveryRunnerCurrentGuard(node)
    /\ asyncRunnerPhase[node] = "Local"
    /\ ~LocalAdmissionCanAdvance(node)
    => ENABLED RetainedHistoricalRecoveryLocalPhaseAdvanceWitness(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                HistoricalRecoveryRunnerCurrentGuard(node),
                asyncRunnerPhase[node] = "Local",
                ~LocalAdmissionCanAdvance(node)
         PROVE ENABLED
                 RetainedHistoricalRecoveryLocalPhaseAdvanceWitness(node)
    <2>1. ENABLED HistoricalRecoveryLocalPhaseAdvanceBaseWitness(node)
      BY <1>1, HistoricalRecoveryLocalPhaseAdvanceBaseEnabled
    <2>2. ENABLED RetainedHistoricalRecoveryLocalPhaseAdvanceWitness(node)
      BY <2>1, AutoUSE, ExpandENABLED, Isa
         DEF RetainedHistoricalRecoveryLocalPhaseAdvanceWitness
    <2> QED BY <2>2
  <1> QED BY <1>1

PostStateHistoricalRecoveryRunnerWitness(node) ==
  \/ PostStateHistoricalRecoveryLocalRunnerWitness(node)
  \/ PostStateHistoricalRecoveryIngressRunnerWitness(node)
  \/ PostStateHistoricalRecoveryRuntimeRunnerWitness(node)

THEOREM PostStateHistoricalRecoveryRunnerWitnessRefinesExact ==
  \A node:
    PostStateHistoricalRecoveryRunnerWitness(node)
      => PostGstRunHistoricalRecoveryNode(node)
BY PostStateHistoricalLockRestartNonCrashOuterFrameRefinesExact, Isa
   DEF PostStateHistoricalRecoveryRunnerWitness,
       PostStateHistoricalRecoveryLocalRunnerWitness,
       PostStateHistoricalRecoveryIngressRunnerWitness,
       PostStateHistoricalRecoveryRuntimeRunnerWitness,
       PostStateHistoricalRecoveryRunnerEnvelope,
       PostStateHistoricalLockRestartNonCrashOuterFrame,
       PostGstRunHistoricalRecoveryNode,
       RunHistoricalRecoveryNode, RunNodeWork, NodeServiceFrame

THEOREM EnabledPostStateHistoricalRecoveryRunnerWitnessRefinesExact ==
  \A node:
    ENABLED PostStateHistoricalRecoveryRunnerWitness(node)
      => ENABLED PostGstRunHistoricalRecoveryNode(node)
PROOF
  <1>1. ASSUME NEW node,
                ENABLED PostStateHistoricalRecoveryRunnerWitness(node)
         PROVE ENABLED PostGstRunHistoricalRecoveryNode(node)
    <2>1. PostStateHistoricalRecoveryRunnerWitness(node) \in BOOLEAN
      BY Isa DEF PostStateHistoricalRecoveryRunnerWitness,
                 PostStateHistoricalRecoveryLocalRunnerWitness,
                 PostStateHistoricalRecoveryIngressRunnerWitness,
                 PostStateHistoricalRecoveryRuntimeRunnerWitness
    <2>2. PostGstRunHistoricalRecoveryNode(node) \in BOOLEAN
      BY Isa DEF PostGstRunHistoricalRecoveryNode
    <2>3. PostStateHistoricalRecoveryRunnerWitness(node)
             => PostGstRunHistoricalRecoveryNode(node)
      BY PostStateHistoricalRecoveryRunnerWitnessRefinesExact
    <2>4. ENABLED PostStateHistoricalRecoveryRunnerWitness(node)
             => ENABLED PostGstRunHistoricalRecoveryNode(node)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED BY <1>1, <2>4
  <1> QED BY <1>1

THEOREM HistoricalRecoveryRunnerEnabledAfterGst ==
  \A node \in asyncHistoricalRecoveryTargets:
    /\ AsyncStrongTypeInvariant
    /\ gst
    => ENABLED PostGstRunHistoricalRecoveryNode(node)
PROOF
  <1>1. ASSUME NEW node \in asyncHistoricalRecoveryTargets,
                AsyncStrongTypeInvariant,
                gst
         PROVE ENABLED PostGstRunHistoricalRecoveryNode(node)
    <2>1a. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>1b. node \in ValidatorIds
      BY <1>1, <2>1a, HistoricalRecoveryTargetsAreValidators
    <2>1c. /\ node \in up
             /\ ~NodeHasApplication(node)
      BY <1>1
         DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant,
             AsyncHistoricalRecoveryTypeInvariant
    <2>1d. ~ResponsiveReplayQuarantined(node)
      BY <1>1, GstExcludesResponsiveReplayQuarantine
         DEF AsyncStrongTypeInvariant
    <2>1e. asyncRunnerPhase[node]
                \in {"Local", "Ingress", "Runtime"}
      BY <1>1, <2>1b
         DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant
    <2>2. CASE asyncRunnerPhase[node] = "Local"
      <3>1. ENABLED LocalAdmissionStep(node)
        BY <2>1b, <2>2, LocalAdmissionStepIsEnabled
      <3>2. ENABLED
                PostStateHistoricalRecoveryLocalRunnerWitness(node)
        BY <1>1, <2>1c, <2>1d, <3>1,
           AutoUSE, ExpandENABLED, Isa
           DEF PostStateHistoricalRecoveryLocalRunnerWitness,
               PostStateHistoricalRecoveryRunnerEnvelope,
               PostStateHistoricalLockRestartNonCrashOuterFrame,
               PostStateHistoricalLockRestartAuthorityFrame,
               HistoricalRecoveryTarget,
               NodeServiceFrame, AsyncCoreOuterFrame,
               AsyncRecoveryControlVars
      <3>3. ENABLED PostStateHistoricalRecoveryRunnerWitness(node)
        BY <3>2, AutoUSE, ExpandENABLED, Isa
           DEF PostStateHistoricalRecoveryRunnerWitness
      <3> QED BY <3>3,
           EnabledPostStateHistoricalRecoveryRunnerWitnessRefinesExact
    <2>3. CASE asyncRunnerPhase[node] = "Ingress"
      <3>1. ENABLED IngressDrainStep(node)
        BY <2>1b, <2>3, IngressDrainStepIsEnabled
      <3>2. ENABLED
                PostStateHistoricalRecoveryIngressRunnerWitness(node)
        BY <1>1, <2>1c, <2>1d, <3>1,
           AutoUSE, ExpandENABLED, Isa
           DEF PostStateHistoricalRecoveryIngressRunnerWitness,
               PostStateHistoricalRecoveryRunnerEnvelope,
               PostStateHistoricalLockRestartNonCrashOuterFrame,
               PostStateHistoricalLockRestartAuthorityFrame,
               HistoricalRecoveryTarget,
               NodeServiceFrame, AsyncCoreOuterFrame,
               AsyncRecoveryControlVars
      <3>3. ENABLED PostStateHistoricalRecoveryRunnerWitness(node)
        BY <3>2, AutoUSE, ExpandENABLED, Isa
           DEF PostStateHistoricalRecoveryRunnerWitness
      <3> QED BY <3>3,
           EnabledPostStateHistoricalRecoveryRunnerWitnessRefinesExact
    <2>4. CASE asyncRunnerPhase[node] = "Runtime"
      <3>1. ENABLED SerializedRuntimeStep(node)
        BY <2>1b, <2>4, SerializedRuntimeStepIsEnabled
      <3>2. ENABLED
                PostStateHistoricalRecoveryRuntimeRunnerWitness(node)
        BY <1>1, <2>1c, <2>1d, <3>1,
           AutoUSE, ExpandENABLED, Isa
           DEF PostStateHistoricalRecoveryRuntimeRunnerWitness,
               PostStateHistoricalRecoveryRunnerEnvelope,
               PostStateHistoricalLockRestartNonCrashOuterFrame,
               PostStateHistoricalLockRestartAuthorityFrame,
               HistoricalRecoveryTarget,
               NodeServiceFrame, AsyncCoreOuterFrame,
               AsyncRecoveryControlVars
      <3>3. ENABLED PostStateHistoricalRecoveryRunnerWitness(node)
        BY <3>2, AutoUSE, ExpandENABLED, Isa
           DEF PostStateHistoricalRecoveryRunnerWitness
      <3> QED BY <3>3,
           EnabledPostStateHistoricalRecoveryRunnerWitnessRefinesExact
    <2> QED BY <2>1e, <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM HistoricalRecoveryIoWorkerEnabledAfterGst ==
  \A node \in asyncHistoricalRecoveryTargets:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AsyncIoQueueDepth(node) > 0
    => ENABLED PostGstServiceHistoricalRecoveryIoWorker(node)
PROOF
  <1>1. ASSUME NEW node \in asyncHistoricalRecoveryTargets,
                AsyncStrongTypeInvariant,
                gst,
                AsyncIoQueueDepth(node) > 0
         PROVE ENABLED PostGstServiceHistoricalRecoveryIoWorker(node)
    <2>1a. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>1b. node \in up
      BY <1>1, <2>1a, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncHistoricalRecoveryTypeInvariant
    <2>1c. ~ResponsiveReplayQuarantined(node)
      BY <1>1, GstExcludesResponsiveReplayQuarantine
         DEF AsyncStrongTypeInvariant
    <2>1. /\ node \in up
           /\ ~ResponsiveReplayQuarantined(node)
      BY <2>1b, <2>1c
    <2>2. ENABLED RetainedHistoricalLockRestartIoWorkerWitness(node)
      BY <1>1, <2>1, AsyncStrongTypeProjectsAsyncType,
         HistoricalRecoveryTargetsAreValidators,
         GstExcludesResponsiveReplayQuarantine,
         AutoUSE, ExpandENABLED, IsaT(300)
         DEF RetainedHistoricalLockRestartIoWorkerWitness,
             RetainedHistoricalLockRestartNonRunnerOuterFrame,
             RetainedHistoricalLockRestartNonCrashOuterFrame,
             RetainedHistoricalLockRestartAuthorityFrame,
             HistoricalLockRestartAuthorityObservedVars,
             AsyncStrongTypeInvariant, AsyncTypeInvariant,
             AsyncHistoricalRecoveryTypeInvariant,
             ServiceHistoricalRecoveryIoWorker, ServiceIoWorkerWork,
             HistoricalRecoveryTarget,
             ResponsiveReplayExecutorAllowed,
             ResponsiveReplayQuarantined, ResponsiveReplayDraining,
             PublishEphemeralItems, LeaveCausalQueues,
             AsyncIoQueueDepth, AsyncCoreOuterFrame,
             AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars,
             AsyncRecoveryControlVars, AsyncIoVars,
             AsyncDeferredVars, AsyncLocalAdmissionVars, vars
    <2>3. RetainedHistoricalLockRestartIoWorkerWitness(node) \in BOOLEAN
      BY Isa DEF RetainedHistoricalLockRestartIoWorkerWitness
    <2>4. PostGstServiceHistoricalRecoveryIoWorker(node) \in BOOLEAN
      BY Isa DEF PostGstServiceHistoricalRecoveryIoWorker
    <2>5. RetainedHistoricalLockRestartIoWorkerWitness(node)
             => PostGstServiceHistoricalRecoveryIoWorker(node)
      BY RetainedHistoricalLockRestartIoWorkerWitnessRefinesExact
    <2>6. ENABLED RetainedHistoricalLockRestartIoWorkerWitness(node)
             => ENABLED PostGstServiceHistoricalRecoveryIoWorker(node)
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>6
  <1> QED BY <1>1

THEOREM AsyncTickEnabledHasConcreteSuccessor ==
  AsyncTickEnabled => ENABLED AsyncTick
PROOF
  <1>1. ASSUME AsyncTickEnabled
         PROVE ENABLED AsyncTick
    <2>1. ENABLED RetainedHistoricalLockRestartTickWitness
      BY <1>1,
         AsyncTickGuardEnablesRetainedHistoricalLockRestartWitness
    <2>2. RetainedHistoricalLockRestartTickWitness \in BOOLEAN
      BY Isa
         DEF RetainedHistoricalLockRestartTickWitness
    <2>3. AsyncTick \in BOOLEAN
      BY Isa DEF AsyncTick
    <2>4. RetainedHistoricalLockRestartTickWitness => AsyncTick
      BY RetainedHistoricalLockRestartTickWitnessRefinesExact
    <2>5. ENABLED RetainedHistoricalLockRestartTickWitness
             => ENABLED AsyncTick
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

THEOREM AsyncTickStrictlyDecreasesResponsiveServiceDebt ==
  \A node \in AsyncCurrentResponsiveVoters:
    gst /\ AsyncTick => PostGstDeadlineDebtDecreases
BY SMT
   DEF AsyncTick, AsyncTickEnabled,
       PostGstDeadlineDebtDecreases, DeadlineDistance

THEOREM AsyncFairStrictStepIsParameterizedProductive ==
  \A initialContext \in ContextRecords:
    /\ AsyncTypeInvariant
    /\ gst
    /\ AsyncFairActionAt(initialContext)
    /\ \/ HeightProtocolEvidenceGrows
       \/ PostGstDeadlineDebtDecreases
       \/ ProtectedServiceRankDecreaseStep
       \/ ProtectedServeRankDecreaseStep
       \/ AsyncTerminatingLocalWorkDecreaseStep
    => PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
BY AsyncFairActionsRefineAsyncNextObligation
   DEF AsyncTypeInvariant, PostGstProductiveStepWith

THEOREM PostGstIngressAdmissionIsConcreteTransportProgress ==
  \A initialContext \in ContextRecords,
     recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncTypeInvariant
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ ResponsivePacketPairAt(initialContext, recipient, source)
    /\ (PostGstAdmitHiddenPacket(recipient, source)
          \/ PostGstAdmitHistoricalRecoveryPacket(recipient, source))
    => PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                ResponsivePacketPairAt(
                  initialContext, recipient, source),
                PostGstAdmitHiddenPacket(recipient, source)
                  \/ PostGstAdmitHistoricalRecoveryPacket(
                       recipient, source)
         PROVE PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. /\ gst
           /\ AdmitIngressPacket(recipient, source)
      BY <1>1
         DEF PostGstAdmitHiddenPacket,
             PostGstAdmitHistoricalRecoveryPacket
    <2>2. TransportOutstandingLocalWorkDecreaseStep
      BY <1>1, <2>1, IngressAdmissionStrictlyDecreasesTransportWork
    <2>3. AsyncFairActionAt(initialContext)
      BY <1>1, Isa
         DEF AsyncFairActionAt, ResponsivePacketPairAt,
             PostGstAdmitHiddenPacket,
             PostGstAdmitHistoricalRecoveryPacket,
             HistoricalRecoveryPacketCorridor
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
         AsyncFairStrictStepIsParameterizedProductive
         DEF AsyncTerminatingLocalWorkDecreaseStep
  <1> QED BY <1>1

THEOREM AdmissibleResponsivePacketEnablesConcreteProgress ==
  \A initialContext \in ContextRecords,
     recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncStrongTypeInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ gst
    /\ recipient \in AsyncCurrentResponsiveVoters
                    \cup asyncHistoricalRecoveryTargets
    /\ ResponsivePacketPairAt(initialContext, recipient, source)
    /\ DueSourcePackets(recipient, source) # {}
    /\ DueIngressPacketCanEnter(recipient, source)
    => ENABLED PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncStrongTypeInvariant,
                PostGstReplayQuarantineExcluded,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                gst,
                recipient \in AsyncCurrentResponsiveVoters
                                \cup asyncHistoricalRecoveryTargets,
                ResponsivePacketPairAt(
                  initialContext, recipient, source),
                DueSourcePackets(recipient, source) # {},
                DueIngressPacketCanEnter(recipient, source)
         PROVE ENABLED PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>2. ENABLED
             (PostGstAdmitHiddenPacket(recipient, source)
                \/ PostGstAdmitHistoricalRecoveryPacket(
                     recipient, source))
      BY <1>1, DueIngressPacketAdmissionIsEnabled
    <2>3. (PostGstAdmitHiddenPacket(recipient, source)
               \/ PostGstAdmitHistoricalRecoveryPacket(
                    recipient, source))
             => PostGstProductiveStepWith(
                  AsyncTerminatingLocalWorkDecreaseStep)
      BY <1>1, <2>1, PostGstIngressAdmissionIsConcreteTransportProgress
    <2>4. /\ (PostGstAdmitHiddenPacket(recipient, source)
                  \/ PostGstAdmitHistoricalRecoveryPacket(
                       recipient, source)) \in BOOLEAN
           /\ PostGstProductiveStepWith(
                AsyncTerminatingLocalWorkDecreaseStep) \in BOOLEAN
      BY Isa
         DEF PostGstAdmitHiddenPacket,
             PostGstAdmitHistoricalRecoveryPacket,
             AdmitIngressPacket, AdmitHiddenPacket,
             CoalesceHiddenPacket, PostGstProductiveStepWith,
             AsyncTerminatingLocalWorkDecreaseStep
    <2> QED BY <2>2, <2>3, <2>4, ENABLEDaxioms
  <1> QED BY <1>1

THEOREM PostGstTickIsConcreteProductiveAt ==
  \A initialContext \in ContextRecords,
     node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ gst
    /\ AsyncTick
    => PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
BY AsyncTickStrictlyDecreasesResponsiveServiceDebt,
   AsyncFairStrictStepIsParameterizedProductive
   DEF AsyncFairActionAt

THEOREM PostGstPacketRecipientRunnerIsConcreteTransportProgress ==
  \A initialContext \in ContextRecords,
     recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncTypeInvariant
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ recipient \in AsyncCurrentResponsiveVoters
                    \cup asyncHistoricalRecoveryTargets
    /\ DueSourcePackets(recipient, source) # {}
    /\ ~NodeHasApplication(recipient)
    /\ (PostGstRunNode(recipient)
          \/ PostGstRunHistoricalRecoveryNode(recipient))
    => PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                recipient \in AsyncCurrentResponsiveVoters
                                \cup asyncHistoricalRecoveryTargets,
                DueSourcePackets(recipient, source) # {},
                ~NodeHasApplication(recipient),
                PostGstRunNode(recipient)
                  \/ PostGstRunHistoricalRecoveryNode(recipient)
         PROVE PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. /\ gst
           /\ RunNodeWork(recipient)
      BY <1>1
         DEF PostGstRunNode, PostGstRunHistoricalRecoveryNode,
             RunNode, RunHistoricalRecoveryNode
    <2>2. CASE LocalAdmissionStep(recipient)
                    \/ IngressDrainStep(recipient)
      <3>1. RunnerPrefixLocalWorkDecreaseStep
        BY <1>1, <2>2,
           RunNodePrefixStrictlyDecreasesLocalWork
      <3>2. AsyncFairActionAt(initialContext)
        BY <1>1, Isa
           DEF AsyncFairActionAt, PostGstRunNode,
               PostGstRunHistoricalRecoveryNode,
               RunHistoricalRecoveryNode, HistoricalRecoveryTarget,
               AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncHistoricalRecoveryTypeInvariant
      <3> QED BY <1>1, <2>1, <3>1, <3>2,
           AsyncFairStrictStepIsParameterizedProductive
           DEF AsyncTerminatingLocalWorkDecreaseStep
    <2>3. CASE SerializedRuntimeStep(recipient)
      <3>1. OverdueTransportRuntimeInvocationDecreaseStep
        BY <1>1, <2>3,
           RuntimeInvocationStrictlyDecreasesOverdueTransportWork
      <3>2. AsyncFairActionAt(initialContext)
        BY <1>1, Isa
           DEF AsyncFairActionAt, PostGstRunNode,
               PostGstRunHistoricalRecoveryNode,
               RunHistoricalRecoveryNode, HistoricalRecoveryTarget,
               AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncHistoricalRecoveryTypeInvariant
      <3> QED BY <1>1, <2>1, <3>1, <3>2,
           AsyncFairStrictStepIsParameterizedProductive
           DEF AsyncTerminatingLocalWorkDecreaseStep
    <2> QED BY <2>1, <2>2, <2>3 DEF RunNodeWork
  <1> QED BY <1>1

THEOREM UnappliedPacketRecipientEnablesConcreteRunnerProgress ==
  \A initialContext \in ContextRecords,
     recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ gst
    /\ recipient \in AsyncCurrentResponsiveVoters
                    \cup asyncHistoricalRecoveryTargets
    /\ DueSourcePackets(recipient, source) # {}
    /\ ~NodeHasApplication(recipient)
    => ENABLED PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncStrongTypeInvariant,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                gst,
                recipient \in AsyncCurrentResponsiveVoters
                                \cup asyncHistoricalRecoveryTargets,
                DueSourcePackets(recipient, source) # {},
                ~NodeHasApplication(recipient)
         PROVE ENABLED PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>2. CASE recipient \in asyncHistoricalRecoveryTargets
      <3>1. ENABLED PostGstRunHistoricalRecoveryNode(recipient)
        BY <1>1, <2>2, HistoricalRecoveryRunnerEnabledAfterGst
      <3>2. PostGstRunHistoricalRecoveryNode(recipient)
               => PostGstProductiveStepWith(
                    AsyncTerminatingLocalWorkDecreaseStep)
        BY <1>1, <2>1, <2>2,
           PostGstPacketRecipientRunnerIsConcreteTransportProgress
      <3>3. /\ PostGstRunHistoricalRecoveryNode(recipient) \in BOOLEAN
             /\ PostGstProductiveStepWith(
                  AsyncTerminatingLocalWorkDecreaseStep) \in BOOLEAN
        BY Isa
           DEF PostGstRunHistoricalRecoveryNode,
               RunHistoricalRecoveryNode,
               PostGstProductiveStepWith,
               AsyncTerminatingLocalWorkDecreaseStep
      <3> QED BY <3>1, <3>2, <3>3, ENABLEDaxioms
    <2>3. CASE recipient \notin asyncHistoricalRecoveryTargets
      <3>1. recipient \in AsyncCurrentResponsiveVoters
        BY <1>1, <2>3
      <3>2. ENABLED RunNode(recipient)
        BY <2>1, <3>1, <1>1,
           ResponsiveUnappliedRunNodeIsEnabled
      <3>3. ENABLED PostGstRunNode(recipient)
        BY <1>1, <3>1, <3>2, EnabledRunNodeLiftsPostGst
      <3>4. PostGstRunNode(recipient)
               => PostGstProductiveStepWith(
                    AsyncTerminatingLocalWorkDecreaseStep)
        BY <1>1, <2>1, <3>1,
           PostGstPacketRecipientRunnerIsConcreteTransportProgress
      <3>5. /\ PostGstRunNode(recipient) \in BOOLEAN
             /\ PostGstProductiveStepWith(
                  AsyncTerminatingLocalWorkDecreaseStep) \in BOOLEAN
        BY Isa
           DEF PostGstRunNode, RunNode,
               PostGstProductiveStepWith,
               AsyncTerminatingLocalWorkDecreaseStep
      <3> QED BY <3>3, <3>4, <3>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM PostGstRunnerServiceIsConcreteGateProgress ==
  \A initialContext \in ContextRecords, node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ LocalRunnerServiceContractDebt(node) = 1
    /\ (PostGstRunNode(node)
          \/ PostGstRunHistoricalRecoveryNode(node)
          \/ PostGstRunHistoricalServer(node))
    => PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                LocalRunnerServiceContractDebt(node) = 1,
                PostGstRunNode(node)
                  \/ PostGstRunHistoricalRecoveryNode(node)
                  \/ PostGstRunHistoricalServer(node)
         PROVE PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. /\ gst
           /\ (RunNode(node)
                 \/ RunHistoricalRecoveryNode(node)
                 \/ RunHistoricalServer(node))
      BY <1>1
         DEF PostGstRunNode,
             PostGstRunHistoricalRecoveryNode,
             PostGstRunHistoricalServer
    <2>2. LocalRunnerServiceContractDecreaseStep
      BY <1>1, <2>1, RunnerServiceStrictlyClearsDueGate
    <2>3. AsyncFairActionAt(initialContext)
      BY <1>1, <2>1, Isa
         DEF AsyncFairActionAt, PostGstRunNode,
             PostGstRunHistoricalRecoveryNode,
             PostGstRunHistoricalServer,
             RunHistoricalRecoveryNode,
             HistoricalRecoveryTarget,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncHistoricalRecoveryTypeInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
         AsyncFairStrictStepIsParameterizedProductive
         DEF AsyncTerminatingLocalWorkDecreaseStep
  <1> QED BY <1>1

THEOREM PostGstHistoricalIngressDrainIsConcreteLocalProgress ==
  \A initialContext \in ContextRecords,
     node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ HistoricalDrainableIngressIndices(node) # {}
    /\ PostGstRunHistoricalServer(node)
    => PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                HistoricalDrainableIngressIndices(node) # {},
                PostGstRunHistoricalServer(node)
         PROVE PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. /\ gst
           /\ DrainHistoricalIngressSelected(node)
      BY <1>1 DEF PostGstRunHistoricalServer, RunHistoricalServer
    <2>2. IngressDepthLocalWorkDecreaseStep
      BY <1>1, <2>1, IngressDrainStrictlyDecreasesLocalWork
    <2>3. AsyncFairActionAt(initialContext)
      BY <1>1 DEF AsyncFairActionAt
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
         AsyncFairStrictStepIsParameterizedProductive
         DEF AsyncTerminatingLocalWorkDecreaseStep
  <1> QED BY <1>1

THEOREM AppliedDrainableRecipientEnablesConcreteIngressProgress ==
  \A initialContext \in ContextRecords,
     node \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ gst
    /\ NodeHasApplication(node)
    /\ HistoricalDrainableIngressIndices(node) # {}
    => ENABLED PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW node \in AsyncCurrentResponsiveVoters,
                AsyncStrongTypeInvariant,
                PostGstReplayQuarantineExcluded,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                gst,
                NodeHasApplication(node),
                HistoricalDrainableIngressIndices(node) # {}
         PROVE ENABLED PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>2. ENABLED PostGstRunHistoricalServer(node)
      BY <1>1, AppliedResponsiveHistoricalServerEnabledAfterGst
    <2>3. PostGstRunHistoricalServer(node)
             => PostGstProductiveStepWith(
                  AsyncTerminatingLocalWorkDecreaseStep)
      BY <1>1, <2>1,
         PostGstHistoricalIngressDrainIsConcreteLocalProgress
    <2>4. /\ PostGstRunHistoricalServer(node) \in BOOLEAN
           /\ PostGstProductiveStepWith(
                AsyncTerminatingLocalWorkDecreaseStep) \in BOOLEAN
      BY Isa
         DEF PostGstRunHistoricalServer, RunHistoricalServer,
             PostGstProductiveStepWith,
             AsyncTerminatingLocalWorkDecreaseStep
    <2> QED BY <2>2, <2>3, <2>4, ENABLEDaxioms
  <1> QED BY <1>1

THEOREM PostGstIoServiceIsConcreteLocalProgress ==
  \A initialContext \in ContextRecords, node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ (PostGstServiceIoWorker(node)
          \/ PostGstServiceHistoricalRecoveryIoWorker(node))
    => PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                PostGstServiceIoWorker(node)
                  \/ PostGstServiceHistoricalRecoveryIoWorker(node)
         PROVE PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. /\ gst
           /\ ServiceIoWorkerWork(node)
      BY <1>1
         DEF PostGstServiceIoWorker,
             PostGstServiceHistoricalRecoveryIoWorker,
             ServiceIoWorker, ServiceHistoricalRecoveryIoWorker
    <2>2. IoDepthLocalWorkDecreaseStep
      BY <1>1, <2>1, IoWorkerStrictlyDecreasesLocalWork, Isa
         DEF PostGstServiceIoWorker,
             PostGstServiceHistoricalRecoveryIoWorker,
             ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncHistoricalRecoveryTypeInvariant
    <2>3. AsyncFairActionAt(initialContext)
      BY <1>1, Isa
         DEF AsyncFairActionAt, PostGstServiceIoWorker,
             PostGstServiceHistoricalRecoveryIoWorker,
             ServiceHistoricalRecoveryIoWorker,
             HistoricalRecoveryTarget,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncHistoricalRecoveryTypeInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
         AsyncFairStrictStepIsParameterizedProductive
         DEF AsyncTerminatingLocalWorkDecreaseStep
  <1> QED BY <1>1

THEOREM EnabledPostGstTickEnablesConcreteProductiveStep ==
  \A initialContext \in ContextRecords,
     node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ gst
    /\ AsyncTickEnabled
    => ENABLED PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant, gst, AsyncTickEnabled
         PROVE ENABLED PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. ENABLED AsyncTick
      BY <1>1, AsyncTickEnabledHasConcreteSuccessor
    <2>2. AsyncTick
             => PostGstProductiveStepWith(
                  AsyncTerminatingLocalWorkDecreaseStep)
      BY <1>1, PostGstTickIsConcreteProductiveAt
    <2>3. /\ AsyncTick \in BOOLEAN
           /\ PostGstProductiveStepWith(
                AsyncTerminatingLocalWorkDecreaseStep) \in BOOLEAN
      BY Isa
         DEF AsyncTick, AsyncTickEnabled,
             PostGstProductiveStepWith,
             AsyncTerminatingLocalWorkDecreaseStep
    <2> QED BY <2>1, <2>2, <2>3, ENABLEDaxioms
  <1> QED BY <1>1

THEOREM DueNodeServiceEnablesConcreteGateProgress ==
  \A initialContext \in ContextRecords, node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ gst
    /\ node \in LocalRunnerServiceOwners
    /\ LocalRunnerServiceContractDebt(node) = 1
    => ENABLED PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                PostGstReplayQuarantineExcluded,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                gst,
                node \in LocalRunnerServiceOwners,
                LocalRunnerServiceContractDebt(node) = 1
         PROVE ENABLED PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>2. CASE node \in asyncHistoricalRecoveryTargets
      <3>1. ENABLED PostGstRunHistoricalRecoveryNode(node)
        BY <1>1, <2>2, HistoricalRecoveryRunnerEnabledAfterGst
      <3>2. PostGstRunHistoricalRecoveryNode(node)
               => PostGstProductiveStepWith(
                    AsyncTerminatingLocalWorkDecreaseStep)
        BY <1>1, <2>1, <2>2,
           PostGstRunnerServiceIsConcreteGateProgress
      <3>3. /\ PostGstRunHistoricalRecoveryNode(node) \in BOOLEAN
             /\ PostGstProductiveStepWith(
                  AsyncTerminatingLocalWorkDecreaseStep) \in BOOLEAN
        BY Isa
           DEF PostGstRunHistoricalRecoveryNode,
               RunHistoricalRecoveryNode,
               PostGstProductiveStepWith,
               AsyncTerminatingLocalWorkDecreaseStep
      <3> QED BY <3>1, <3>2, <3>3, ENABLEDaxioms
    <2>3. CASE node \notin asyncHistoricalRecoveryTargets
      <3>1. node \in AsyncCurrentResponsiveVoters
        BY <1>1, <2>3
      <3>2. CASE NodeHasApplication(node)
        <4>1. ENABLED PostGstRunHistoricalServer(node)
          BY <1>1, <3>1, <3>2,
             AppliedResponsiveHistoricalServerEnabledAfterGst
        <4>2. PostGstRunHistoricalServer(node)
                 => PostGstProductiveStepWith(
                      AsyncTerminatingLocalWorkDecreaseStep)
          BY <1>1, <2>1, <3>1, <3>2,
             PostGstRunnerServiceIsConcreteGateProgress
        <4>3. /\ PostGstRunHistoricalServer(node) \in BOOLEAN
               /\ PostGstProductiveStepWith(
                    AsyncTerminatingLocalWorkDecreaseStep) \in BOOLEAN
          BY Isa
             DEF PostGstRunHistoricalServer,
                 RunHistoricalServer,
                 PostGstProductiveStepWith,
                 AsyncTerminatingLocalWorkDecreaseStep
        <4> QED BY <4>1, <4>2, <4>3, ENABLEDaxioms
      <3>3. CASE ~NodeHasApplication(node)
        <4>1. ENABLED RunNode(node)
          BY <2>1, <3>1, <3>3,
             ResponsiveUnappliedRunNodeIsEnabled
        <4>2. ENABLED PostGstRunNode(node)
          BY <1>1, <3>1, <4>1, EnabledRunNodeLiftsPostGst
        <4>3. PostGstRunNode(node)
                 => PostGstProductiveStepWith(
                      AsyncTerminatingLocalWorkDecreaseStep)
          BY <1>1, <2>1, <3>1, <3>3,
             PostGstRunnerServiceIsConcreteGateProgress
        <4>4. /\ PostGstRunNode(node) \in BOOLEAN
               /\ PostGstProductiveStepWith(
                    AsyncTerminatingLocalWorkDecreaseStep) \in BOOLEAN
          BY Isa
             DEF PostGstRunNode, RunNode,
                 PostGstProductiveStepWith,
                 AsyncTerminatingLocalWorkDecreaseStep
        <4> QED BY <4>2, <4>3, <4>4, ENABLEDaxioms
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM DueIoServiceEnablesConcreteLocalProgress ==
  \A initialContext \in ContextRecords, node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ gst
    /\ node \in AsyncCurrentResponsiveVoters
                 \cup asyncHistoricalRecoveryTargets
    /\ AsyncIoQueueDepth(node) > 0
    => ENABLED PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                gst,
                node \in AsyncCurrentResponsiveVoters
                         \cup asyncHistoricalRecoveryTargets,
                AsyncIoQueueDepth(node) > 0
         PROVE ENABLED PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>2. CASE node \in asyncHistoricalRecoveryTargets
      <3>1. ENABLED PostGstServiceHistoricalRecoveryIoWorker(node)
        BY <1>1, <2>1, <2>2,
           HistoricalRecoveryIoWorkerEnabledAfterGst
      <3>2. PostGstServiceHistoricalRecoveryIoWorker(node)
               => PostGstProductiveStepWith(
                    AsyncTerminatingLocalWorkDecreaseStep)
        BY <1>1, <2>1, <2>2,
           PostGstIoServiceIsConcreteLocalProgress
      <3>3. /\ PostGstServiceHistoricalRecoveryIoWorker(node)
                   \in BOOLEAN
             /\ PostGstProductiveStepWith(
                  AsyncTerminatingLocalWorkDecreaseStep) \in BOOLEAN
        BY Isa
           DEF PostGstServiceHistoricalRecoveryIoWorker,
               ServiceHistoricalRecoveryIoWorker,
               PostGstProductiveStepWith,
               AsyncTerminatingLocalWorkDecreaseStep
      <3> QED BY <3>1, <3>2, <3>3, ENABLEDaxioms
    <2>3. CASE node \notin asyncHistoricalRecoveryTargets
      <3>1. node \in AsyncCurrentResponsiveVoters
        BY <1>1, <2>3
      <3>2. ENABLED PostGstServiceIoWorker(node)
        BY <1>1, <2>1, <3>1, QueuedIoEnablesPostGstService
      <3>3. PostGstServiceIoWorker(node)
               => PostGstProductiveStepWith(
                    AsyncTerminatingLocalWorkDecreaseStep)
        BY <1>1, <2>1, <3>1,
           PostGstIoServiceIsConcreteLocalProgress
      <3>4. /\ PostGstServiceIoWorker(node) \in BOOLEAN
             /\ PostGstProductiveStepWith(
                  AsyncTerminatingLocalWorkDecreaseStep) \in BOOLEAN
        BY Isa
           DEF PostGstServiceIoWorker, ServiceIoWorker,
               PostGstProductiveStepWith,
               AsyncTerminatingLocalWorkDecreaseStep
      <3> QED BY <3>2, <3>3, <3>4, ENABLEDaxioms
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

(***************************************************************************
The transport blocker is discharged at its authenticated recipient/source,
never by selecting an unrelated undecided validator.  Exact duplicates take
the coalescing admission branch.  A fresh admissible item takes the ordinary
admission branch.  A fresh rejected item proves that recipient ingress is
nonempty: an unapplied recipient advances its own three-phase runner; an
applied recipient either drains a historical-safe item or, if every queued
item is an authorized request blocked on the Serve reservation, services the
nonempty I/O FIFO which is the exact admission blocker.
***************************************************************************)

THEOREM OverdueResponsivePacketEnablesConcreteCorridorProgress ==
  \A initialContext \in ContextRecords:
    /\ AsyncStrongTypeInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ gst
    /\ OverdueResponsivePackets # {}
    => ENABLED PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                AsyncStrongTypeInvariant,
                PostGstReplayQuarantineExcluded,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                gst,
                OverdueResponsivePackets # {}
         PROVE ENABLED PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>2. PICK packet \in OverdueResponsivePackets: TRUE
      BY <1>1
    <2> DEFINE Recipient == packet.item.envelope.recipient
    <2> DEFINE Source == packet.item.source
    <2> DEFINE Item == OldestDueSourcePacket(Recipient, Source).item
    <2>3. /\ Recipient \in ValidatorIds
           /\ Source \in AsyncIngressSources
           /\ DueSourcePackets(Recipient, Source) # {}
           /\ ResponsivePacketPairAt(
                initialContext, Recipient, Source)
           /\ Recipient \in AsyncCurrentResponsiveVoters
                           \cup asyncHistoricalRecoveryTargets
      BY <1>1, <2>1, <2>2,
         OverdueResponsivePacketUsesFairIngressPair, Isa
         DEF Recipient, Source, OverdueResponsivePackets,
             HistoricalRecoveryTarget
    <2>4. /\ AsyncItemTyped(Item)
           /\ Item.envelope.recipient = Recipient
           /\ Item.source = Source
      BY <2>1, <2>3, OldestDueSourcePacketFacts
         DEF Item, AsyncPacketTyped
    <2>5. CASE DueIngressPacketCanEnter(Recipient, Source)
      BY <1>1, <2>3, <2>5,
         AdmissibleResponsivePacketEnablesConcreteProgress
    <2>6. CASE ~DueIngressPacketCanEnter(Recipient, Source)
      <3>1. \/ IngressHasCoalescingOwner(Item)
             \/ ~CanAdmitIngressItem(Item)
        BY <2>6
           DEF DueIngressPacketCanEnter,
               DueIngressPacketCanCoalesce, Item
      <3>2. CASE ~NodeHasApplication(Recipient)
        BY <1>1, <2>3, <3>2,
           UnappliedPacketRecipientEnablesConcreteRunnerProgress
      <3>3. CASE NodeHasApplication(Recipient)
        <4>1. Recipient \in AsyncCurrentResponsiveVoters
          BY <1>1, <2>1, <2>3, <3>3, Isa
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncHistoricalRecoveryTypeInvariant
        <4>2. IngressDepth(Recipient) > 0
          <5>1. CASE IngressHasCoalescingOwner(Item)
            <6>1. IngressResourceSource(Item) \in AsyncIngressSources
              BY <2>4, Isa
                 DEF AsyncItemTyped, AsyncIngressSources
            <6>2. ASSUME IngressDepth(Recipient) = 0
                   PROVE FALSE
              <7>1. IngressLane(
                       Recipient, IngressResourceSource(Item)) = <<>>
                BY <2>1, <2>3, <6>1, <6>2,
                   ZeroIngressDepthMeansEveryLaneEmpty
              <7> QED BY <5>1, <7>1
                   DEF IngressHasCoalescingOwner, SequenceSet,
                       IngressLane
            <6> QED BY <2>1, <6>2, SMT
                 DEF AsyncTypeInvariant, TypeInvariant,
                     AsyncSchedulerTypeInvariant,
                     AsyncIngressTypeInvariant,
                     AsyncIngressCapacityTypeInvariant
          <5>2. CASE ~IngressHasCoalescingOwner(Item)
            <6>1. ~CanAdmitIngressItem(Item)
              BY <3>1, <5>2
            <6> QED BY <2>1, <2>3, <2>4, <6>1,
                 EmptyIngressAdmitsTypedPacket, SMT
          <5> QED BY <5>1, <5>2
        <4>3. CASE HistoricalDrainableIngressIndices(Recipient) # {}
          BY <1>1, <4>1, <3>3, <4>3,
             AppliedDrainableRecipientEnablesConcreteIngressProgress
        <4>4. CASE HistoricalDrainableIngressIndices(Recipient) = {}
          <5>1. AsyncIoQueueDepth(Recipient) > 0
            BY <2>1, <2>3, <4>2, <4>4,
               NonemptyUndrainableHistoricalIngressHasIoWork
          <5> QED BY <1>1, <2>3, <5>1,
               DueIoServiceEnablesConcreteLocalProgress
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

PostGstTickBlocker ==
  \/ OverdueResponsivePackets # {}
  \/ \E node \in LocalRunnerServiceOwners:
       \/ asyncNodeServiceDeadlines[node] <= asyncNow
       \/ /\ AsyncIoQueueDepth(node) > 0
             /\ asyncIoServiceDeadlines[node] <= asyncNow

THEOREM DisabledPostGstTickHasConcreteBlocker ==
  /\ AsyncTypeInvariant
  /\ gst
  /\ ~AsyncTickEnabled
  => PostGstTickBlocker
BY Isa
   DEF PostGstTickBlocker, AsyncTickEnabled,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant,
       AsyncIoQueueDepth

THEOREM PostGstUndecidedEnablesConcreteProductiveStepAt ==
  \A initialContext \in ContextRecords:
    /\ AsyncStrongTypeInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ gst
    /\ ~ResponsiveNodesDecide
    => ENABLED PostGstProductiveStepWith(
         AsyncTerminatingLocalWorkDecreaseStep)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                AsyncStrongTypeInvariant,
                PostGstReplayQuarantineExcluded,
                AsyncCurrentResponsiveVoters =
                  AsyncVotersAt(initialContext),
                gst, ~ResponsiveNodesDecide
         PROVE ENABLED PostGstProductiveStepWith(
                 AsyncTerminatingLocalWorkDecreaseStep)
    <2>1. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>2. PICK undecided \in AsyncCurrentResponsiveVoters:
             ~NodeHasDecision(undecided)
      BY <1>1, Isa DEF ResponsiveNodesDecide
    <2>3. CASE AsyncTickEnabled
      BY <1>1, <2>1, <2>2, <2>3,
         EnabledPostGstTickEnablesConcreteProductiveStep
    <2>4. CASE ~AsyncTickEnabled
      <3>1. PostGstTickBlocker
        BY <1>1, <2>1, <2>4,
           DisabledPostGstTickHasConcreteBlocker
      <3>2. CASE OverdueResponsivePackets # {}
        BY <1>1, <3>2,
           OverdueResponsivePacketEnablesConcreteCorridorProgress
      <3>3. CASE \E node \in LocalRunnerServiceOwners:
                       asyncNodeServiceDeadlines[node] <= asyncNow
        <4>1. PICK serviceNode \in LocalRunnerServiceOwners:
                    asyncNodeServiceDeadlines[serviceNode] <= asyncNow
          BY <3>3
        <4>2. /\ serviceNode \in ValidatorIds
               /\ LocalRunnerServiceContractDebt(serviceNode) = 1
          BY <2>1, <4>1, Isa
             DEF LocalRunnerServiceContractDebt,
                 LocalRunnerServiceOwners, AsyncTypeInvariant,
                 AsyncSchedulerTypeInvariant,
                 AsyncHistoricalRecoveryTypeInvariant,
                 AsyncRuntimeTypeInvariant,
                 AsyncRuntimeScalarTypeInvariant
        <4> QED BY <1>1, <4>1, <4>2,
             DueNodeServiceEnablesConcreteGateProgress
      <3>4. CASE \E node \in AsyncCurrentResponsiveVoters
                              \cup asyncHistoricalRecoveryTargets:
                       /\ AsyncIoQueueDepth(node) > 0
                       /\ asyncIoServiceDeadlines[node] <= asyncNow
        <4>1. PICK ioNode \in AsyncCurrentResponsiveVoters
                              \cup asyncHistoricalRecoveryTargets:
                    /\ AsyncIoQueueDepth(ioNode) > 0
                    /\ asyncIoServiceDeadlines[ioNode] <= asyncNow
          BY <3>4
        <4>2. ioNode \in ValidatorIds
          BY <2>1, <4>1, Isa
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncHistoricalRecoveryTypeInvariant
        <4> QED BY <1>1, <4>1, <4>2,
             DueIoServiceEnablesConcreteLocalProgress
      <3> QED BY <3>1, <3>2, <3>3, <3>4
           DEF PostGstTickBlocker
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

(***************************************************************************
The Entry-38 candidate proof uses the parameterized productive-step property.
The legacy one-argument base wrapper remains semantically identical to the old
four-way predicate; this child supplies only strict, named terminating-local-
work steps.  In particular, an applied voter reaches the historical-server
gate through the proved post-GST quarantine exclusion rather than an assumed
ordinary RunNode action.  The ledger remains `specified_unproved` until this
entire dependency cone passes the pinned strict TLAPS release invocation.
***************************************************************************)

THEOREM DeadlockFreedomObligation ==
  \A initialContext:
    DeadlockFreedomWithLocalWorkProperty(AsyncSpecAt(initialContext),
      ENABLED PostGstProductiveStepWith(
        AsyncTerminatingLocalWorkDecreaseStep))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE DeadlockFreedomWithLocalWorkProperty(
                 AsyncSpecAt(initialContext),
                 ENABLED PostGstProductiveStepWith(
                   AsyncTerminatingLocalWorkDecreaseStep))
    <2>1. AsyncSpecAt(initialContext)
             => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>2. AsyncSpecAt(initialContext)
             => []PostGstReplayQuarantineExcluded
      BY AsyncSpecAlwaysExcludesPostGstReplayQuarantine
    <2>3. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>4. AsyncSpecAt(initialContext)
             => []AsyncFrozenContextAt(initialContext)
      BY AsyncSpecAlwaysKeepsFrozenContext
    <2>5. AsyncSpecAt(initialContext)
             => initialContext \in ContextRecords
      BY <2>1, <2>4, PTL
         DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
             Safety, TypeInvariant, AsyncFrozenContextAt
    <2>6. AsyncSpecAt(initialContext)
             => [](gst /\ ~ResponsiveNodesDecide
                    => ENABLED PostGstProductiveStepWith(
                         AsyncTerminatingLocalWorkDecreaseStep))
      BY <2>1, <2>2, <2>3, <2>5,
         PostGstUndecidedEnablesConcreteProductiveStepAt, PTL
    <2> QED BY <2>6
         DEF DeadlockFreedomWithLocalWorkProperty
  <1> QED BY <1>1

THEOREM StarvationFreedomObligation ==
  \A initialContext:
    StarvationFreedomProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE StarvationFreedomProperty(AsyncSpecAt(initialContext))
    <2>1. ProtectedServiceRanksProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedServiceRankProgressObligation
    <2> QED BY <2>1, ProtectedServiceRankProgressImpliesStarvation,
               PTL
  <1> QED BY <1>1

=============================================================================
