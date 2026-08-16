---- MODULE SumeragiV2ChainEpochRefinementShard11 ----
EXTENDS SumeragiV2ChainEpochRefinementShard10

THEOREM IndexedFairActionsRemainEnabledInProduct ==
  \A initialContext \in AdmissibleContextRecords:
    (IndexedCompositionInvariant
      /\ initialContext \in JoinedContexts)
      => /\ (ENABLED IndexedAsync(initialContext)!AsyncSetGST
                => ENABLED IndexedSetGstStep(initialContext))
         /\ (ENABLED IndexedAsync(initialContext)!AsyncTick
                => ENABLED IndexedTickStep(initialContext))
         /\ \A node \in IndexedAsync(initialContext)!
                       AsyncVotersAt(initialContext):
              node \in joinedByContext[initialContext]
                => /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstRunNode(node)
                          => ENABLED
                               IndexedRunNodeStep(initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstCommitCertificateDiscovery(node)
                          => ENABLED
                               IndexedCommitCertificateDiscoveryStep(
                                 initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstResolveLocalCandidateProducerContinuation(
                                    node)
                          => ENABLED
                               IndexedResolveLocalProducerContinuationStep(
                                 initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstServiceConditionalTransportProducerContinuation(
                                    node)
                          => ENABLED
                               IndexedServiceConditionalProducerContinuationStep(
                                 initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstServiceVolatileBodyProducerContinuation(
                                    node)
                          => ENABLED
                               IndexedServiceVolatileProducerContinuationStep(
                                 initialContext, node))
         /\ \A node \in Responsive:
              node \in joinedByContext[initialContext]
                => /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstRunHistoricalServer(node)
                          => ENABLED IndexedHistoricalServerStep(
                               initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstServiceIoWorker(node)
                          => ENABLED
                               IndexedIoWorkerStep(initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstOpenHistoricalRecovery(node)
                          => ENABLED IndexedOpenHistoricalRecoveryStep(
                               initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstRunHistoricalRecoveryNode(node)
                          => ENABLED IndexedRunHistoricalRecoveryStep(
                               initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstHistoricalCommitCertificateDiscovery(
                                    node)
                          => ENABLED
                               IndexedHistoricalCommitCertificateDiscoveryStep(
                                 initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstServiceHistoricalRecoveryIoWorker(node)
                          => ENABLED
                               IndexedHistoricalRecoveryIoWorkerStep(
                                 initialContext, node))
         /\ \A slot \in IndexedAsync(initialContext)!
                       AsyncLeaderWireLifecycleSlotSet:
              ENABLED IndexedAsync(initialContext)!
                        PostGstRetireLeaderWireLifecycleSlot(slot)
                => ENABLED IndexedRetireLeaderWireLifecycleStep(
                     initialContext, slot)
         /\ \A recipient \in Responsive,
               source \in IndexedAsync(initialContext)!
                         AsyncIngressSources:
              ENABLED IndexedAsync(initialContext)!
                        PostGstAdmitHiddenPacket(recipient, source)
                => ENABLED IndexedAdmitPacketStep(
                     initialContext, recipient, source)
         /\ \A recipient \in ValidatorIds,
               source \in IndexedAsync(initialContext)!
                          AsyncIngressSources:
              ENABLED IndexedAsync(initialContext)!
                        PostGstAdmitHistoricalRecoveryPacket(
                          recipient, source)
                => ENABLED IndexedAdmitHistoricalRecoveryPacketStep(
                     initialContext, recipient, source)
BY Isa, IndexedJoinedActionHasProductExtension,
   JoinedNonCurrentDisablesExactRunNode,
   ExactHistoricalRecoveryTargetOwnsCurrentLocation
   DEF IndexedSetGstStep, IndexedTickStep, IndexedRunNodeStep,
       IndexedOpenHistoricalRecoveryStep,
       IndexedRunHistoricalRecoveryStep,
       IndexedCommitCertificateDiscoveryStep,
       IndexedHistoricalCommitCertificateDiscoveryStep,
       IndexedHistoricalServerStep, IndexedIoWorkerStep,
       IndexedHistoricalRecoveryIoWorkerStep,
       IndexedResolveLocalProducerContinuationStep,
       IndexedServiceConditionalProducerContinuationStep,
       IndexedServiceVolatileProducerContinuationStep,
       IndexedRetireLeaderWireLifecycleStep,
       IndexedAdmitPacketStep, IndexedChainNext,
       IndexedAdmitHistoricalRecoveryPacketStep,
       IndexedOpenHistoricalRecovery,
       IndexedHistoricalRecoveryTargetReady,
       IndexedHistoricalRecoverySourceReady,
       IndexedProductActionAt, IndexedJoinedAsyncNext,
       IndexedJoinedNonCrashStep, IndexedJoinedRunnerStep,
       IndexedJoinedNonRunnerStep, IndexedNodeCurrentAt,
       IndexedAsync!PostGstRunNode,
       IndexedAsync!PostGstOpenHistoricalRecovery,
       IndexedAsync!PostGstRunHistoricalRecoveryNode,
       IndexedAsync!PostGstCommitCertificateDiscovery,
       IndexedAsync!PostGstResolveLocalCandidateProducerContinuation,
       IndexedAsync!PostGstServiceConditionalTransportProducerContinuation,
       IndexedAsync!PostGstServiceVolatileBodyProducerContinuation,
       IndexedAsync!PostGstRetireLeaderWireLifecycleSlot,
       IndexedAsync!PostGstHistoricalCommitCertificateDiscovery,
       IndexedAsync!PostGstRunHistoricalServer,
       IndexedAsync!PostGstServiceIoWorker,
       IndexedAsync!PostGstServiceHistoricalRecoveryIoWorker,
       IndexedAsync!PostGstAdmitHiddenPacket,
       IndexedAsync!PostGstAdmitHistoricalRecoveryPacket,
       IndexedAsync!AsyncNonCrashStep,
       IndexedAsync!AsyncRunnerStep,
       IndexedAsync!AsyncNonRunnerStep,
       IndexedAsync!AsyncNext

THEOREM IndexedChainSpecRefinesChainEpochSpec ==
  IndexedChainSpec => Chain!ChainEpochSpec
PROOF
  <1>1. IndexedChainInit => Chain!ChainEpochInit
    BY DEF IndexedChainInit
  <1>2. IndexedChainNext
           => [Chain!ChainEpochNext]_Chain!ChainEpochVars
    BY IndexedStepProjectsChainEpochStep
  <1> QED BY <1>1, <1>2, PTL
     DEF IndexedChainSpec, Chain!ChainEpochSpec

THEOREM IndexedChainSafety ==
  IndexedChainSpec => []Chain!ChainEpochSafety
PROOF
  <1>1. IndexedChainSpec => Chain!ChainEpochSpec
    BY IndexedChainSpecRefinesChainEpochSpec
  <1>2. Chain!ChainEpochSpec => []Chain!ChainEpochSafety
    BY Chain!ChainPrefixAndEpochSafety
  <1> QED BY <1>1, <1>2, PTL

(***************************************************************************
Temporal induction interface.

IndexedInstanceActivationObligation is the suffix argument: once the finite
prior-height application induction has eventually joined every responsive
validator, the already-running restricted behavior satisfies the exact
AsyncSpecAt fairness obligations. Early joined work is part of that same
behavior and is never blocked. IndexedFairActionsRemainEnabledInProduct proves
that the receipt wrapper does not hide enabled exact actions. Once a joined
node is no longer current, JoinedNonCurrentDisablesExactRunNode makes its exact
RunNode fairness obligation vacuous while historical service stays fair.
Historical recovery is owned by the exact Async target and its ordinary
decision/body/store/validate/apply corridor; its remaining temporal debt is
the explicit IndexedExactHistoricalRecoveryProgress premise of the conditional
height kernel. Terminal application has no successor join.
VerificationOneHeightCompletion is the exact fixed-context expansion of the
one-height completion property over the parameterized production-network
instance. Its wrapper is supplied by the exact asynchronous temporal closure,
after rotating-leader convergence and exact Decision-stage application service
have closed.  The conditional final proof composes explicit premises over
finite Heights; it does not hide them as a new protocol relation.
***************************************************************************)
IndexedAllResponsiveJoined(initialContext) ==
  Responsive \subseteq joinedByContext[initialContext]

THEOREM IndexedResponsiveVoterSetIsNonempty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!AsyncVotersAt(initialContext) # {}
BY Isa DEF AdmissibleContextRecords, FrozenContextAdmissible,
           IndexedAsync!AsyncVotersAt, ModelConfiguration,
           DualQuorum, CountQuorum, QuorumConfiguration,
           ContextRecords, LineagesAt, Heights

THEOREM IndexedAllResponsiveJoinedMakesContextJoined ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAllResponsiveJoined(initialContext)
      => initialContext \in JoinedContexts
BY Isa, IndexedResponsiveVoterSetIsNonempty
   DEF IndexedAllResponsiveJoined, JoinedContexts,
       IndexedAsync!AsyncVotersAt

THEOREM IndexedAllResponsiveJoinedIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAllResponsiveJoined(initialContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedAllResponsiveJoined(initialContext)'
BY Isa, JoinedMembershipIsMonotone
   DEF IndexedAllResponsiveJoined, IndexedChainVars

THEOREM IndexedAllResponsiveJoinedHasActiveRoster ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedCompositionInvariant
    /\ IndexedAllResponsiveJoined(initialContext)
    => Responsive \subseteq
         IndexedAsync(initialContext)!AsyncActiveServiceNodes
BY Isa
   DEF IndexedCompositionInvariant,
       IndexedAllResponsiveJoined,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes

THEOREM IndexedChainSpecKeepsGenesisResponsiveRosterActive ==
  IndexedChainSpec
    => [](Responsive \subseteq
          IndexedAsync(GenesisContext)!AsyncActiveServiceNodes)
BY IndexedChainSpecEstablishesCompositionInvariant,
   IndexedAllResponsiveJoinedHasActiveRoster, PTL, Isa
   DEF IndexedChainInit, IndexedChainSpec,
       IndexedAllResponsiveJoined, GenesisContext,
       AdmissibleContextRecords, FrozenContextAdmissible,
       ContextRecords, LineagesAt, Heights,
       ModelConfiguration, ValidatorIds

IndexedActivationStable(initialContext) ==
  /\ IndexedCompositionInvariant
  /\ IndexedAllResponsiveJoined(initialContext)
  /\ initialContext \in JoinedContexts

THEOREM IndexedActivationEventuallyStabilizes ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedChainSpec
     /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      => <>[]IndexedActivationStable(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => <>[]IndexedActivationStable(initialContext)
    <2>1. IndexedChainSpec => []IndexedCompositionInvariant
      BY IndexedChainSpecEstablishesCompositionInvariant
    <2>2. IndexedAllResponsiveJoined(initialContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedAllResponsiveJoined(initialContext)'
      BY <1>1, IndexedAllResponsiveJoinedIsStable
    <2>3. IndexedAllResponsiveJoined(initialContext)
             => initialContext \in JoinedContexts
      BY <1>1, IndexedAllResponsiveJoinedMakesContextJoined
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF IndexedChainSpec, IndexedActivationStable
  <1> QED BY <1>1

(***************************************************************************
The product never leaves the Eligible recovery phase, so the six pre-GST
restart/replay actions required by AsyncFairnessAt are permanently disabled.
Their weak-fairness clauses are therefore satisfied semantically, rather than
being dropped from the exact AsyncSpecAt projection.
***************************************************************************)
IndexedResponsiveRecoveryActionsDisabled ==
  \A initialContext \in AdmissibleContextRecords:
    /\ ~ENABLED
          <<IndexedAsync(initialContext)!PreGstResponsiveRestart>>_(
            IndexedAsyncStateAt(initialContext))
    /\ ~ENABLED
          <<IndexedAsync(initialContext)!PreGstResponsiveReplay>>_(
            IndexedAsyncStateAt(initialContext))
    /\ ~ENABLED
          <<IndexedAsync(initialContext)!ResponsiveReplayRunNode>>_(
            IndexedAsyncStateAt(initialContext))
    /\ ~ENABLED
          <<IndexedAsync(initialContext)!
              ResponsiveReplayServiceIoWorker>>_(
            IndexedAsyncStateAt(initialContext))
    /\ ~ENABLED
          <<IndexedAsync(initialContext)!DriveResponsiveReplayHead>>_(
            IndexedAsyncStateAt(initialContext))
    /\ ~ENABLED
          <<IndexedAsync(initialContext)!FinishResponsiveReplay>>_(
            IndexedAsyncStateAt(initialContext))

THEOREM IndexedResponsiveRecoveryDormancyDisablesFairActions ==
  IndexedResponsiveRecoveryDormant
    => IndexedResponsiveRecoveryActionsDisabled
BY ExpandENABLED, Isa
   DEF IndexedResponsiveRecoveryDormant,
       IndexedResponsiveRecoveryActionsDisabled,
       IndexedAsyncStateAt, IndexedRecovery,
       IndexedAsync!PreGstResponsiveRestart,
       IndexedAsync!PreGstResponsiveReplay,
       IndexedAsync!ResponsiveReplayRunNode,
       IndexedAsync!ResponsiveReplayServiceIoWorker,
       IndexedAsync!DriveResponsiveReplayHead,
       IndexedAsync!FinishResponsiveReplay,
       IndexedAsync!ResponsiveReplayDraining

THEOREM IndexedChainSpecAlwaysDisablesResponsiveRecoveryActions ==
  IndexedChainSpec => []IndexedResponsiveRecoveryActionsDisabled
BY IndexedChainSpecKeepsResponsiveRecoveryDormant,
   IndexedResponsiveRecoveryDormancyDisablesFairActions, PTL

THEOREM IndexedResponsiveRecoveryFairnessIsVacuous ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         /\ WF_(IndexedAsyncStateAt(initialContext))(
              IndexedAsync(initialContext)!PreGstResponsiveRestart)
         /\ WF_(IndexedAsyncStateAt(initialContext))(
              IndexedAsync(initialContext)!PreGstResponsiveReplay)
         /\ WF_(IndexedAsyncStateAt(initialContext))(
              IndexedAsync(initialContext)!ResponsiveReplayRunNode)
         /\ WF_(IndexedAsyncStateAt(initialContext))(
              IndexedAsync(initialContext)!
                ResponsiveReplayServiceIoWorker)
         /\ WF_(IndexedAsyncStateAt(initialContext))(
              IndexedAsync(initialContext)!DriveResponsiveReplayHead)
         /\ WF_(IndexedAsyncStateAt(initialContext))(
              IndexedAsync(initialContext)!FinishResponsiveReplay)
PROOF
  <1>1. ASSUME IndexedChainSpec,
               NEW initialContext \in AdmissibleContextRecords
         PROVE
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!PreGstResponsiveRestart)
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!PreGstResponsiveReplay)
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!ResponsiveReplayRunNode)
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!
                  ResponsiveReplayServiceIoWorker)
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!DriveResponsiveReplayHead)
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!FinishResponsiveReplay)
    <2>1. /\ []~ENABLED
                  <<IndexedAsync(initialContext)!
                      PreGstResponsiveRestart>>_(
                    IndexedAsyncStateAt(initialContext))
            /\ []~ENABLED
                  <<IndexedAsync(initialContext)!
                      PreGstResponsiveReplay>>_(
                    IndexedAsyncStateAt(initialContext))
            /\ []~ENABLED
                  <<IndexedAsync(initialContext)!
                      ResponsiveReplayRunNode>>_(
                    IndexedAsyncStateAt(initialContext))
            /\ []~ENABLED
                  <<IndexedAsync(initialContext)!
                      ResponsiveReplayServiceIoWorker>>_(
                    IndexedAsyncStateAt(initialContext))
            /\ []~ENABLED
                  <<IndexedAsync(initialContext)!
                      DriveResponsiveReplayHead>>_(
                    IndexedAsyncStateAt(initialContext))
            /\ []~ENABLED
                  <<IndexedAsync(initialContext)!
                      FinishResponsiveReplay>>_(
                    IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedChainSpecAlwaysDisablesResponsiveRecoveryActions, PTL
         DEF IndexedResponsiveRecoveryActionsDisabled
    <2> QED BY <2>1, PTL
  <1> QED BY <1>1

(***************************************************************************
The standalone Async specification weakly fairly rearms every Responsive
service owner.  In the indexed product, rearm is fused into the corresponding
successor publication.  Once the activation premise has joined every
Responsive node, component-46 coherence makes every standalone rearm action
permanently disabled, so those exact weak-fairness clauses hold vacuously.
***************************************************************************)
IndexedResponsiveServiceActivationActionsDisabledAt(initialContext) ==
  \A node \in Responsive:
    ~ENABLED
       <<IndexedAsync(initialContext)!AsyncActivateServiceNode(node)>>_(
         IndexedAsyncStateAt(initialContext))

THEOREM IndexedActivationStableDisablesResponsiveServiceActivation ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedActivationStable(initialContext)
      => IndexedResponsiveServiceActivationActionsDisabledAt(
           initialContext)
BY ExpandENABLED, Isa
   DEF IndexedActivationStable,
       IndexedAllResponsiveJoined,
       IndexedCompositionInvariant,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedResponsiveServiceActivationActionsDisabledAt,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedAsyncStateAt

THEOREM IndexedResponsiveServiceActivationFairnessIsVacuous ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedChainSpec
     /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      => \A node \in Responsive:
           WF_(IndexedAsyncStateAt(initialContext))(
             IndexedAsync(initialContext)!AsyncActivateServiceNode(node))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
               /\ IndexedChainSpec
                  /\ TRUE ~> IndexedAllResponsiveJoined(initialContext)
         PROVE \A node \in Responsive:
                 WF_(IndexedAsyncStateAt(initialContext))(
                   IndexedAsync(initialContext)!
                     AsyncActivateServiceNode(node))
    <2>1. <>[]IndexedActivationStable(initialContext)
      BY <1>1, IndexedActivationEventuallyStabilizes
    <2>2. [](IndexedActivationStable(initialContext)
               => IndexedResponsiveServiceActivationActionsDisabledAt(
                    initialContext))
      BY IndexedActivationStableDisablesResponsiveServiceActivation, PTL
    <2>3. \A node \in Responsive:
             <>[]~ENABLED
               <<IndexedAsync(initialContext)!
                    AsyncActivateServiceNode(node)>>_(
                 IndexedAsyncStateAt(initialContext))
      BY <2>1, <2>2, PTL
         DEF IndexedResponsiveServiceActivationActionsDisabledAt
    <2> QED BY <2>3, PTL
  <1> QED BY <1>1

(***************************************************************************
Every weakly fair product action is an exact nonstuttering action of the
selected Async instance.  Conversely, after activation the product
enabledness theorem extends every exact nonstuttering witness to the paired
receipt/ChainEpoch transition.  These two directions are the concrete
fairness-refinement argument; no fairness clause is added to the projection.
***************************************************************************)
THEOREM IndexedFairProductStepsProjectExactOccurrences ==
  \A initialContext \in AdmissibleContextRecords:
    /\ (IndexedSetGstStep(initialContext)
          => <<IndexedAsync(initialContext)!AsyncSetGST>>_(
               IndexedAsyncStateAt(initialContext)))
    /\ (IndexedTickStep(initialContext)
          => <<IndexedAsync(initialContext)!AsyncTick>>_(
               IndexedAsyncStateAt(initialContext)))
    /\ \A node \in IndexedAsync(initialContext)!
                    AsyncVotersAt(initialContext):
         /\ (IndexedRunNodeStep(initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstRunNode(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedCommitCertificateDiscoveryStep(
                  initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstCommitCertificateDiscovery(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedResolveLocalProducerContinuationStep(
                  initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstResolveLocalCandidateProducerContinuation(
                         node)>>_(IndexedAsyncStateAt(initialContext)))
         /\ (IndexedServiceConditionalProducerContinuationStep(
                  initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstServiceConditionalTransportProducerContinuation(
                         node)>>_(IndexedAsyncStateAt(initialContext)))
         /\ (IndexedServiceVolatileProducerContinuationStep(
                  initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstServiceVolatileBodyProducerContinuation(
                         node)>>_(IndexedAsyncStateAt(initialContext)))
    /\ \A node \in Responsive:
         /\ (IndexedHistoricalServerStep(initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstRunHistoricalServer(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedIoWorkerStep(initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstServiceIoWorker(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedOpenHistoricalRecoveryStep(initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstOpenHistoricalRecovery(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedRunHistoricalRecoveryStep(initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstRunHistoricalRecoveryNode(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedHistoricalCommitCertificateDiscoveryStep(
                  initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstHistoricalCommitCertificateDiscovery(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedHistoricalRecoveryIoWorkerStep(
                  initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstServiceHistoricalRecoveryIoWorker(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
    /\ \A slot \in IndexedAsync(initialContext)!
                  AsyncLeaderWireLifecycleSlotSet:
         IndexedRetireLeaderWireLifecycleStep(initialContext, slot)
           => <<IndexedAsync(initialContext)!
                   PostGstRetireLeaderWireLifecycleSlot(slot)>>_(
                IndexedAsyncStateAt(initialContext))
    /\ \A recipient \in Responsive,
          source \in IndexedAsync(initialContext)!
                    AsyncIngressSources:
         IndexedAdmitPacketStep(initialContext, recipient, source)
           => <<IndexedAsync(initialContext)!
                   PostGstAdmitHiddenPacket(recipient, source)>>_(
                IndexedAsyncStateAt(initialContext))
    /\ \A recipient \in ValidatorIds,
          source \in IndexedAsync(initialContext)!AsyncIngressSources:
         IndexedAdmitHistoricalRecoveryPacketStep(
           initialContext, recipient, source)
           => <<IndexedAsync(initialContext)!
                   PostGstAdmitHistoricalRecoveryPacket(
                     recipient, source)>>_(
                IndexedAsyncStateAt(initialContext))
BY Isa DEF IndexedSetGstStep, IndexedTickStep,
           IndexedRunNodeStep, IndexedHistoricalServerStep,
           IndexedOpenHistoricalRecoveryStep,
           IndexedRunHistoricalRecoveryStep,
           IndexedCommitCertificateDiscoveryStep,
           IndexedHistoricalCommitCertificateDiscoveryStep,
           IndexedIoWorkerStep, IndexedHistoricalRecoveryIoWorkerStep,
           IndexedResolveLocalProducerContinuationStep,
           IndexedServiceConditionalProducerContinuationStep,
           IndexedServiceVolatileProducerContinuationStep,
           IndexedRetireLeaderWireLifecycleStep,
           IndexedAdmitPacketStep,
           IndexedAdmitHistoricalRecoveryPacketStep,
           IndexedChainNext, IndexedProductActionAt,
           IndexedReceiptClassification, IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedChainVars, IndexedAsyncStateAt,
           IndexedAsync!AsyncSetGST, IndexedAsync!SetGST,
           IndexedAsync!AsyncTick,
           IndexedAsync!PostGstRunNode, IndexedAsync!RunNode,
           IndexedAsync!PostGstOpenHistoricalRecovery,
           IndexedAsync!OpenHistoricalRecovery,
           IndexedAsync!PostGstRunHistoricalRecoveryNode,
           IndexedAsync!RunHistoricalRecoveryNode,
           IndexedAsync!PostGstCommitCertificateDiscovery,
           IndexedAsync!DirectCommitCertificateDiscoveryStep,
           IndexedAsync!PostGstHistoricalCommitCertificateDiscovery,
           IndexedAsync!DirectHistoricalCommitCertificateDiscoveryStep,
           IndexedAsync!PostGstRunHistoricalServer,
           IndexedAsync!RunHistoricalServer,
           IndexedAsync!PostGstServiceIoWorker,
           IndexedAsync!ServiceIoWorker,
           IndexedAsync!PostGstServiceHistoricalRecoveryIoWorker,
           IndexedAsync!ServiceHistoricalRecoveryIoWorker,
           IndexedAsync!PostGstResolveLocalCandidateProducerContinuation,
           IndexedAsync!PostGstServiceConditionalTransportProducerContinuation,
           IndexedAsync!PostGstServiceVolatileBodyProducerContinuation,
           IndexedAsync!PostGstRetireLeaderWireLifecycleSlot,
           IndexedAsync!PostGstAdmitHiddenPacket,
           IndexedAsync!PostGstAdmitHistoricalRecoveryPacket,
           IndexedAsync!AdmitHiddenPacket

(***************************************************************************
Activation-local historical non-packet fairness.

Unlike the aggregate AsyncSpecAt projection below, these bridges do not wait
for all Responsive peers.  Each post-GST exact action exposes its own active
owner in its guard.  The composition invariant maps that owner to joined
membership, which is exactly the enabledness premise of the corresponding
weakly fair product action.  Tick is guarded explicitly by GST because bare
AsyncTick is intentionally enabled in dormant pre-GST instances.
***************************************************************************)
THEOREM IndexedPostGstHistoricalFairOccurrencesEnableProduct ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      => /\ (ENABLED
                <<IndexedPostGstTick(initialContext)>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedTickStep(initialContext)>>_(
                       IndexedChainVars))
         /\ \A node \in IndexedAsync(initialContext)!
                       AsyncVotersAt(initialContext):
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstRunNode(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedRunNodeStep(initialContext, node)>>_(
                           IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstResolveLocalCandidateProducerContinuation(
                          node)>>_(IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedResolveLocalProducerContinuationStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceConditionalTransportProducerContinuation(
                          node)>>_(IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedServiceConditionalProducerContinuationStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceVolatileBodyProducerContinuation(
                          node)>>_(IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedServiceVolatileProducerContinuationStep(
                             initialContext, node)>>_(IndexedChainVars))
         /\ \A slot \in IndexedAsync(initialContext)!
                       AsyncLeaderWireLifecycleSlotSet:
              ENABLED
                <<IndexedAsync(initialContext)!
                    PostGstRetireLeaderWireLifecycleSlot(slot)>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedRetireLeaderWireLifecycleStep(
                         initialContext, slot)>>_(IndexedChainVars)
         /\ \A recipient \in Responsive,
               source \in IndexedAsync(initialContext)!
                         AsyncIngressSources:
              ENABLED
                <<IndexedAsync(initialContext)!
                    PostGstAdmitHiddenPacket(recipient, source)>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedAdmitPacketStep(
                         initialContext, recipient, source)>>_(
                       IndexedChainVars)
         /\ \A node \in Responsive:
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstRunHistoricalServer(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedHistoricalServerStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceIoWorker(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedIoWorkerStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstRunHistoricalRecoveryNode(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedRunHistoricalRecoveryStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceHistoricalRecoveryIoWorker(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedHistoricalRecoveryIoWorkerStep(
                             initialContext, node)>>_(IndexedChainVars))
BY IndexedPostGstContextHasJoinedProductInstance,
   IndexedPostGstActiveServiceOwnerHasJoinedProductInstance,
   IndexedFairActionsRemainEnabledInProduct,
   IndexedFairProductStepsProjectExactOccurrences,
   ExpandENABLED, Isa
   DEF IndexedPostGstTick,
       IndexedResolveLocalProducerContinuationStep,
       IndexedServiceConditionalProducerContinuationStep,
       IndexedServiceVolatileProducerContinuationStep,
       IndexedRetireLeaderWireLifecycleStep,
       IndexedAdmitPacketStep,
       IndexedAsync!PostGstRunNode,
       IndexedAsync!PostGstResolveLocalCandidateProducerContinuation,
       IndexedAsync!PostGstServiceConditionalTransportProducerContinuation,
       IndexedAsync!PostGstServiceVolatileBodyProducerContinuation,
       IndexedAsync!PostGstRetireLeaderWireLifecycleSlot,
       IndexedAsync!PostGstAdmitHiddenPacket,
       IndexedAsync!PostGstRunHistoricalServer,
       IndexedAsync!PostGstServiceIoWorker,
       IndexedAsync!PostGstRunHistoricalRecoveryNode,
       IndexedAsync!PostGstServiceHistoricalRecoveryIoWorker,
       IndexedAsync!RunNode, IndexedAsync!RunNodeWork,
       IndexedAsync!LocalAdmissionStep,
       IndexedAsync!IngressDrainStep,
       IndexedAsync!SerializedRuntimeStep,
       IndexedAsync!SerializedRuntimePrecedesServeIngressStep,
       IndexedAsync!SerializedLocalPrecedesServeIngressStep,
       IndexedAsync!AsyncServeIngressTargetOnlyTurn,
       IndexedAsync!SelectedLocalAdmissionAdvance,
       IndexedAsync!RunHistoricalServer,
       IndexedAsync!RunHistoricalRecoveryNode,
       IndexedAsync!ServiceIoWorker,
       IndexedAsync!ServiceHistoricalRecoveryIoWorker,
       IndexedAsync!ServiceIoWorkerWork,
       IndexedAsync!AsyncArchiveIoServiceNodes,
       IndexedAsync!AsyncTimedServiceNodes

=============================================================================
