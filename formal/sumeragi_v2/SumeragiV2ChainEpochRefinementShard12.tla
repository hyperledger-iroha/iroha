---- MODULE SumeragiV2ChainEpochRefinementShard12 ----
EXTENDS SumeragiV2ChainEpochRefinementShard11

THEOREM IndexedPostGstTickProductStepProjectsExactOccurrence ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCore(initialContext, 7)
      /\ IndexedTickStep(initialContext)
      => <<IndexedPostGstTick(initialContext)>>_(
           IndexedAsyncStateAt(initialContext))
BY IndexedFairProductStepsProjectExactOccurrences
   DEF IndexedPostGstTick

THEOREM IndexedPostGstTickFairnessTransfersLocally ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainSpec
      => WF_(IndexedAsyncStateAt(initialContext))(
           IndexedPostGstTick(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
               IndexedChainSpec
         PROVE WF_(IndexedAsyncStateAt(initialContext))(
                 IndexedPostGstTick(initialContext))
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [](IndexedCompositionInvariant
               => (ENABLED
                     <<IndexedPostGstTick(initialContext)>>_(
                       IndexedAsyncStateAt(initialContext))
                     => ENABLED
                          <<IndexedTickStep(initialContext)>>_(
                            IndexedChainVars)))
      BY <1>1,
         IndexedPostGstHistoricalFairOccurrencesEnableProduct, PTL
    <2>3. [](ENABLED
               <<IndexedPostGstTick(initialContext)>>_(
                 IndexedAsyncStateAt(initialContext))
               => IndexedCore(initialContext, 7))
      BY ExpandENABLED, PTL DEF IndexedPostGstTick
    <2>4. IndexedCore(initialContext, 7)
             /\ IndexedTickStep(initialContext)
             => <<IndexedPostGstTick(initialContext)>>_(
                  IndexedAsyncStateAt(initialContext))
      BY <1>1,
         IndexedPostGstTickProductStepProjectsExactOccurrence, PTL
    <2>5. WF_IndexedChainVars(IndexedTickStep(initialContext))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

THEOREM IndexedPostGstRunNodeFairnessTransfersLocally ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in IndexedAsync(initialContext)!
                  AsyncVotersAt(initialContext):
      IndexedChainSpec
        => WF_(IndexedAsyncStateAt(initialContext))(
             IndexedAsync(initialContext)!PostGstRunNode(node))
BY IndexedChainSpecEstablishesCompositionInvariant,
   IndexedPostGstHistoricalFairOccurrencesEnableProduct,
   IndexedFairProductStepsProjectExactOccurrences, PTL
   DEF IndexedChainSpec, IndexedFairness

THEOREM IndexedAdequateLeaderNonRunnerFairnessTransfersLocally ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainSpec
      => /\ \A node \in IndexedAsync(initialContext)!
                         AsyncVotersAt(initialContext):
              /\ WF_(IndexedAsyncStateAt(initialContext))(
                   IndexedAsync(initialContext)!
                     PostGstResolveLocalCandidateProducerContinuation(node))
              /\ WF_(IndexedAsyncStateAt(initialContext))(
                   IndexedAsync(initialContext)!
                     PostGstServiceConditionalTransportProducerContinuation(
                       node))
              /\ WF_(IndexedAsyncStateAt(initialContext))(
                   IndexedAsync(initialContext)!
                     PostGstServiceVolatileBodyProducerContinuation(node))
         /\ \A slot \in IndexedAsync(initialContext)!
                       AsyncLeaderWireLifecycleSlotSet:
              WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!
                  PostGstRetireLeaderWireLifecycleSlot(slot))
         /\ \A recipient \in Responsive,
               source \in IndexedAsync(initialContext)!AsyncIngressSources:
              WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!
                  PostGstAdmitHiddenPacket(recipient, source))
BY IndexedChainSpecEstablishesCompositionInvariant,
   IndexedPostGstHistoricalFairOccurrencesEnableProduct,
   IndexedFairProductStepsProjectExactOccurrences, PTL
   DEF IndexedChainSpec, IndexedFairness

THEOREM IndexedHistoricalNonPacketOwnerFairnessTransfersLocally ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in Responsive:
      IndexedChainSpec
        => /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstRunHistoricalServer(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstServiceIoWorker(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstRunHistoricalRecoveryNode(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstServiceHistoricalRecoveryIoWorker(node))
BY IndexedChainSpecEstablishesCompositionInvariant,
   IndexedPostGstHistoricalFairOccurrencesEnableProduct,
   IndexedFairProductStepsProjectExactOccurrences, PTL
   DEF IndexedChainSpec, IndexedFairness

THEOREM IndexedFairExactOccurrencesEnableProductOccurrences ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedActivationStable(initialContext)
      => /\ (ENABLED
                <<IndexedAsync(initialContext)!AsyncSetGST>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedSetGstStep(initialContext)>>_(
                       IndexedChainVars))
         /\ (ENABLED
               <<IndexedAsync(initialContext)!AsyncTick>>_(
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
                        PostGstCommitCertificateDiscovery(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedCommitCertificateDiscoveryStep(
                             initialContext, node)>>_(IndexedChainVars))
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
                         <<IndexedIoWorkerStep(initialContext, node)>>_(
                           IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstOpenHistoricalRecovery(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedOpenHistoricalRecoveryStep(
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
                        PostGstHistoricalCommitCertificateDiscovery(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedHistoricalCommitCertificateDiscoveryStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceHistoricalRecoveryIoWorker(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedHistoricalRecoveryIoWorkerStep(
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
         /\ \A recipient \in ValidatorIds,
               source \in IndexedAsync(initialContext)!
                          AsyncIngressSources:
              ENABLED
                <<IndexedAsync(initialContext)!
                    PostGstAdmitHistoricalRecoveryPacket(
                      recipient, source)>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedAdmitHistoricalRecoveryPacketStep(
                         initialContext, recipient, source)>>_(
                       IndexedChainVars)
BY IndexedFairActionsRemainEnabledInProduct,
   IndexedFairProductStepsProjectExactOccurrences,
   ExpandENABLED, Isa
   DEF IndexedActivationStable

THEOREM IndexedSetGstFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedChainSpec
     /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      => WF_(IndexedAsyncStateAt(initialContext))(
           IndexedAsync(initialContext)!AsyncSetGST)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!AsyncSetGST)
    <2>1. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => <>[]IndexedActivationStable(initialContext)
      BY <1>1, IndexedActivationEventuallyStabilizes
    <2>2. IndexedActivationStable(initialContext)
             => (ENABLED
                   <<IndexedAsync(initialContext)!AsyncSetGST>>_(
                     IndexedAsyncStateAt(initialContext))
                   => ENABLED
                        <<IndexedSetGstStep(initialContext)>>_(
                          IndexedChainVars))
      BY <1>1, IndexedFairExactOccurrencesEnableProductOccurrences
    <2>3. IndexedSetGstStep(initialContext)
             => <<IndexedAsync(initialContext)!AsyncSetGST>>_(
                  IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedFairProductStepsProjectExactOccurrences
    <2>4. IndexedChainSpec
             => WF_IndexedChainVars(IndexedSetGstStep(initialContext))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedTickFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedChainSpec
     /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      => WF_(IndexedAsyncStateAt(initialContext))(
           IndexedAsync(initialContext)!AsyncTick)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!AsyncTick)
    <2>1. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => <>[]IndexedActivationStable(initialContext)
      BY <1>1, IndexedActivationEventuallyStabilizes
    <2>2. IndexedActivationStable(initialContext)
             => (ENABLED
                   <<IndexedAsync(initialContext)!AsyncTick>>_(
                     IndexedAsyncStateAt(initialContext))
                   => ENABLED
                        <<IndexedTickStep(initialContext)>>_(
                          IndexedChainVars))
      BY <1>1, IndexedFairExactOccurrencesEnableProductOccurrences
    <2>3. IndexedTickStep(initialContext)
             => <<IndexedAsync(initialContext)!AsyncTick>>_(
                  IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedFairProductStepsProjectExactOccurrences
    <2>4. IndexedChainSpec
             => WF_IndexedChainVars(IndexedTickStep(initialContext))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedNodeFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in IndexedAsync(initialContext)!AsyncVotersAt(initialContext):
      (/\ IndexedChainSpec
       /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
        => /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!PostGstRunNode(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstCommitCertificateDiscovery(node))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in IndexedAsync(initialContext)!
                            AsyncVotersAt(initialContext)
         PROVE
           (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!PostGstRunNode(node))
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstCommitCertificateDiscovery(node))
    <2>1. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => <>[]IndexedActivationStable(initialContext)
      BY <1>1, IndexedActivationEventuallyStabilizes
    <2>2. IndexedActivationStable(initialContext)
             => /\ (ENABLED
                       <<IndexedAsync(initialContext)!
                           PostGstRunNode(node)>>_(
                         IndexedAsyncStateAt(initialContext))
                       => ENABLED
                            <<IndexedRunNodeStep(
                                initialContext, node)>>_(IndexedChainVars))
                /\ (ENABLED
                       <<IndexedAsync(initialContext)!
                           PostGstCommitCertificateDiscovery(node)>>_(
                         IndexedAsyncStateAt(initialContext))
                       => ENABLED
                            <<IndexedCommitCertificateDiscoveryStep(
                                initialContext, node)>>_(IndexedChainVars))
      BY <1>1, IndexedFairExactOccurrencesEnableProductOccurrences
    <2>3. /\ (IndexedRunNodeStep(initialContext, node)
                   => <<IndexedAsync(initialContext)!
                           PostGstRunNode(node)>>_(
                        IndexedAsyncStateAt(initialContext)))
              /\ (IndexedCommitCertificateDiscoveryStep(
                       initialContext, node)
                   => <<IndexedAsync(initialContext)!
                           PostGstCommitCertificateDiscovery(node)>>_(
                        IndexedAsyncStateAt(initialContext)))
      BY <1>1, IndexedFairProductStepsProjectExactOccurrences
    <2>4. IndexedChainSpec
             => /\ WF_IndexedChainVars(
                       IndexedRunNodeStep(initialContext, node))
                /\ WF_IndexedChainVars(
                       IndexedCommitCertificateDiscoveryStep(
                         initialContext, node))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedResponsiveServiceFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in Responsive:
      (/\ IndexedChainSpec
       /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
        => /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstRunHistoricalServer(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstServiceIoWorker(node))
BY IndexedActivationEventuallyStabilizes,
   IndexedFairExactOccurrencesEnableProductOccurrences,
   IndexedFairProductStepsProjectExactOccurrences, PTL
   DEF IndexedChainSpec, IndexedFairness

THEOREM IndexedHistoricalRecoveryFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in Responsive:
      (/\ IndexedChainSpec
       /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
        => /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstOpenHistoricalRecovery(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstRunHistoricalRecoveryNode(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstHistoricalCommitCertificateDiscovery(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstServiceHistoricalRecoveryIoWorker(node))
BY IndexedActivationEventuallyStabilizes,
   IndexedFairExactOccurrencesEnableProductOccurrences,
   IndexedFairProductStepsProjectExactOccurrences, PTL
   DEF IndexedChainSpec, IndexedFairness

THEOREM IndexedPacketFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    \A recipient \in Responsive,
       source \in IndexedAsync(initialContext)!
                  AsyncIngressSources:
      (/\ IndexedChainSpec
       /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
        => WF_(IndexedAsyncStateAt(initialContext))(
             IndexedAsync(initialContext)!
               PostGstAdmitHiddenPacket(recipient, source))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW recipient \in Responsive,
              NEW source \in IndexedAsync(initialContext)!
                               AsyncIngressSources
         PROVE
           (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstAdmitHiddenPacket(recipient, source))
    <2>1. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => <>[]IndexedActivationStable(initialContext)
      BY <1>1, IndexedActivationEventuallyStabilizes
    <2>2. IndexedActivationStable(initialContext)
             => (ENABLED
                   <<IndexedAsync(initialContext)!
                       PostGstAdmitHiddenPacket(recipient, source)>>_(
                     IndexedAsyncStateAt(initialContext))
                   => ENABLED
                        <<IndexedAdmitPacketStep(
                            initialContext, recipient, source)>>_(
                          IndexedChainVars))
      BY <1>1, IndexedFairExactOccurrencesEnableProductOccurrences
    <2>3. IndexedAdmitPacketStep(initialContext, recipient, source)
             => <<IndexedAsync(initialContext)!
                     PostGstAdmitHiddenPacket(recipient, source)>>_(
                  IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedFairProductStepsProjectExactOccurrences
    <2>4. IndexedChainSpec
             => WF_IndexedChainVars(
                  IndexedAdmitPacketStep(initialContext, recipient, source))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedHistoricalRecoveryPacketFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    \A recipient \in ValidatorIds,
       source \in IndexedAsync(initialContext)!AsyncIngressSources:
      (/\ IndexedChainSpec
       /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
        => WF_(IndexedAsyncStateAt(initialContext))(
             IndexedAsync(initialContext)!
               PostGstAdmitHistoricalRecoveryPacket(recipient, source))
BY IndexedActivationEventuallyStabilizes,
   IndexedFairExactOccurrencesEnableProductOccurrences,
   IndexedFairProductStepsProjectExactOccurrences, PTL
   DEF IndexedChainSpec, IndexedFairness

THEOREM IndexedInstanceActivationObligation ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedChainSpec
     /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      => IndexedAsync(initialContext)!AsyncSpecAt(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => IndexedAsync(initialContext)!AsyncSpecAt(initialContext)
    <2>1. IndexedChainInit
             => IndexedAsync(initialContext)!AsyncInitAt(initialContext)
      BY <1>1, IndexedInitProjectsEveryAsyncInit
    <2>2. IndexedChainSpec
             => [][IndexedAsync(initialContext)!AsyncNext]_(
                  IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedStepProjectsEveryAsyncStep, PTL
         DEF IndexedChainSpec, IndexedChainVars
    <2>3. IndexedChainSpec
             => [](IndexedAsync(initialContext)!AsyncAllVars
                    = IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant,
         IndexedInstanceVariablesAreExact, PTL
         DEF IndexedCompositionInvariant
    <2>4. IndexedChainSpec
             => /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PreGstResponsiveRestart)
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PreGstResponsiveReplay)
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         ResponsiveReplayRunNode)
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         ResponsiveReplayServiceIoWorker)
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         DriveResponsiveReplayHead)
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         FinishResponsiveReplay)
      BY <1>1, IndexedResponsiveRecoveryFairnessIsVacuous
    <2>4a. (/\ IndexedChainSpec
             /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
              => \A node \in Responsive:
                   WF_(IndexedAsyncStateAt(initialContext))(
                     IndexedAsync(initialContext)!
                       AsyncActivateServiceNode(node))
      BY <1>1,
         IndexedResponsiveServiceActivationFairnessIsVacuous
    <2>5. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!AsyncSetGST)
      BY <1>1, IndexedSetGstFairnessTransfers
    <2>6. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!AsyncTick)
      BY <1>1, IndexedTickFairnessTransfers
    <2>7. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => \A node \in IndexedAsync(initialContext)!
                               AsyncVotersAt(initialContext):
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!PostGstRunNode(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstCommitCertificateDiscovery(node))
      BY <1>1, IndexedNodeFairnessTransfers
    <2>7a. IndexedChainSpec
              => /\ \A node \in IndexedAsync(initialContext)!
                                 AsyncVotersAt(initialContext):
                       /\ WF_(IndexedAsyncStateAt(initialContext))(
                            IndexedAsync(initialContext)!
                              PostGstResolveLocalCandidateProducerContinuation(
                                node))
                       /\ WF_(IndexedAsyncStateAt(initialContext))(
                            IndexedAsync(initialContext)!
                              PostGstServiceConditionalTransportProducerContinuation(
                                node))
                       /\ WF_(IndexedAsyncStateAt(initialContext))(
                            IndexedAsync(initialContext)!
                              PostGstServiceVolatileBodyProducerContinuation(
                                node))
                  /\ \A slot \in IndexedAsync(initialContext)!
                                AsyncLeaderWireLifecycleSlotSet:
                       WF_(IndexedAsyncStateAt(initialContext))(
                         IndexedAsync(initialContext)!
                           PostGstRetireLeaderWireLifecycleSlot(slot))
      BY <1>1, IndexedAdequateLeaderNonRunnerFairnessTransfersLocally
    <2>8. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => \A node \in Responsive:
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstRunHistoricalServer(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstServiceIoWorker(node))
      BY <1>1, IndexedResponsiveServiceFairnessTransfers
    <2>9. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => \A node \in Responsive:
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstOpenHistoricalRecovery(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstRunHistoricalRecoveryNode(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstHistoricalCommitCertificateDiscovery(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstServiceHistoricalRecoveryIoWorker(node))
      BY <1>1, IndexedHistoricalRecoveryFairnessTransfers
    <2>10. (/\ IndexedChainSpec
             /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
              => \A recipient \in Responsive,
                    source \in IndexedAsync(initialContext)!
                              AsyncIngressSources:
                   WF_(IndexedAsyncStateAt(initialContext))(
                     IndexedAsync(initialContext)!
                       PostGstAdmitHiddenPacket(recipient, source))
      BY <1>1, IndexedPacketFairnessTransfers
    <2>11. (/\ IndexedChainSpec
             /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
              => \A recipient \in ValidatorIds,
                    source \in IndexedAsync(initialContext)!
                              AsyncIngressSources:
                   WF_(IndexedAsyncStateAt(initialContext))(
                     IndexedAsync(initialContext)!
                       PostGstAdmitHistoricalRecoveryPacket(
                         recipient, source))
      BY <1>1, IndexedHistoricalRecoveryPacketFairnessTransfers
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>4a, <2>5, <2>6,
                 <2>7, <2>7a, <2>8, <2>9, <2>10, <2>11, PTL
         DEF IndexedAsync!AsyncSpecAt, IndexedAsync!AsyncFairnessAt
  <1> QED BY <1>1

THEOREM IndexedLiveInstanceActivationObligation ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedLiveChainSpec
     /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      => IndexedAsync(initialContext)!AsyncLiveSpecAt(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           (/\ IndexedLiveChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => IndexedAsync(initialContext)!
                  AsyncLiveSpecAt(initialContext)
    <2>1. (/\ IndexedLiveChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => (/\ IndexedChainSpec
                 /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      BY DEF IndexedLiveChainSpec
    <2>2. (/\ IndexedLiveChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => IndexedAsync(initialContext)!AsyncSpecAt(initialContext)
      BY <1>1, <2>1, IndexedInstanceActivationObligation
    <2>3. (/\ IndexedLiveChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => AsyncRepresentativeLiveConfiguration
      BY DEF IndexedLiveChainSpec
    <2> QED BY <2>2, <2>3
         DEF IndexedAsync!AsyncLiveSpecAt
  <1> QED BY <1>1

(***************************************************************************
Exact historical-recovery progress boundary.

Opening and every subsequent recovery transition belong to the exact Async
instance. This product therefore carries no second stage rank. The remaining
temporal debt is stated directly over exact target ownership: once a responsive
node at its frozen context eventually becomes recovery-eligible and acquires
that context's exact application evidence. Eligibility is intentionally
separate: the node must have an authenticated source ready to open, already own
the exact target, or already have the exact durable Decision. A merely joined
node with no source is still waiting for current-height consensus and is not
silently reclassified as an enabled historical-recovery action. Nonterminal
receipt handoff then advances nodeHeight; the terminal horizon intentionally
records application without inventing a successor. The child chain-liveness
module names the eligibility leadsto separately, proves fair target opening,
and exposes the two exact Async temporal prerequisites: target-to-Decision and
responsive Decision-to-application.
***************************************************************************)
HistoricalRecoveryOutstanding(initialContext, node) ==
  /\ node \in Responsive
  /\ node \in joinedByContext[initialContext]
  /\ ExactNodeLocationAt(initialContext, node)
  /\ ~IndexedAsync(initialContext)!NodeHasApplication(node)

HistoricalRecoveryProgressEligible(initialContext, node) ==
  /\ HistoricalRecoveryOutstanding(initialContext, node)
  /\ \/ IndexedHistoricalRecoveryReady(initialContext, node)
     \/ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
     \/ IndexedAsync(initialContext)!NodeHasDecision(node)

HistoricalRecoveryComplete(initialContext, node) ==
  IF initialContext.height = MaxHeight
  THEN IndexedAsync(initialContext)!NodeHasApplication(node)
  ELSE nodeHeight[node] > initialContext.height

IndexedExactHistoricalRecoveryProgress ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    HistoricalRecoveryOutstanding(initialContext, node)
      ~> HistoricalRecoveryComplete(initialContext, node)

VerificationOneHeightCompletion ==
  IndexedAsync(VerificationContext)!AsyncLiveSpecAt(VerificationContext)
    => (IndexedCore(VerificationContext, 7)
          ~> IndexedAsync(VerificationContext)!
               AsyncAllResponsiveAppliedAt(VerificationContext))

THEOREM VerificationOneHeightCompletionObligation ==
  VerificationOneHeightCompletion
PROOF
  <1>1. VerificationAsyncProof!AsyncTemporalClosureOneHeightCompletionObligation
    BY VerificationAsyncProof!AsyncTemporalClosureOneHeightCompletionObligation
  <1> QED BY <1>1
       DEF VerificationOneHeightCompletion,
           VerificationAsyncProof!OneHeightCompletionLiveness,
           VerificationAsyncProof!AsyncLiveSpecAt,
           VerificationAsyncProof!AsyncAllResponsiveAppliedAt,
           IndexedAsync!AsyncLiveSpecAt,
           IndexedAsync!AsyncAllResponsiveAppliedAt,
           IndexedAsyncStateAt, IndexedCore, IndexedRecovery,
           VerificationCore, VerificationScheduler, VerificationRecovery

=============================================================================
