---- MODULE SumeragiV2AsyncRuntimeAdmissionTypeProofs ----
EXTENDS SumeragiV2AsyncIngressRunnerTypeProofs

THEOREM RunHistoricalServerPreservesSchedulerType ==
  \A node \in AsyncResponsiveAppliedArchiveServers:
    AsyncTypeInvariant /\ RunHistoricalServer(node)
      => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncResponsiveAppliedArchiveServers,
                AsyncTypeInvariant,
                RunHistoricalServer(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE HistoricalDrainableIngressIndices(node) = {}
      BY <1>1, <2>1, HistoricalIdleRunnerPreservesSchedulerType
    <2>2. CASE HistoricalDrainableIngressIndices(node) # {}
      BY <1>1, <2>2, HistoricalDrainRunnerPreservesSchedulerType
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM DrainHistoricalIngressSelectedPreservesClaimIngressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ DrainHistoricalIngressSelected(node)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                AsyncCertifiedResponseClaimIngressOwnershipInvariant,
                DrainHistoricalIngressSelected(node)
         PROVE AsyncCertifiedResponseClaimIngressOwnershipInvariant'
    <2> DEFINE DrainIndex ==
          FirstHistoricalDrainableIngressIndex(node)
    <2> DEFINE DrainSource == asyncIngressReady[node][DrainIndex]
    <2> DEFINE DrainLaneIndex ==
          HistoricalSelectedIngressLaneIndex(node, DrainIndex)
    <2> DEFINE DrainItem ==
          IngressLane(node, DrainSource)[DrainLaneIndex]
    <2>1. /\ AsyncIngressTypeInvariant
           /\ DrainIndex \in
                HistoricalDrainableIngressIndices(node)
      BY <1>1, FirstHistoricalDrainableIndexIsDrainable
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             DrainHistoricalIngressSelected, DrainIndex
    <2>2. /\ DrainIndex \in 1..Len(asyncIngressReady[node])
           /\ DrainSource \in AsyncIngressSources
           /\ DrainLaneIndex \in
                1..IngressLaneDepth(node, DrainSource)
      BY <2>1,
         FirstHistoricalDrainableIngressLaneIndexIsDrainable, SMT
         DEF HistoricalDrainableIngressIndices,
             HistoricalIngressSourceCanDrain,
             HistoricalDrainableIngressLaneIndices,
             HistoricalSelectedIngressLaneIndex,
             DrainSource, DrainLaneIndex
    <2>3. /\ asyncIngressLanes' =
                  [asyncIngressLanes EXCEPT
                     ![node][DrainSource] =
                       SequenceWithoutIndex(@, DrainLaneIndex)]
           /\ DrainItem =
                HistoricalSelectedIngressItemAt(node, DrainIndex)
      BY <1>1, <2>2, Isa
         DEF DrainHistoricalIngressSelected, PopSelectedIngress,
             HistoricalSelectedIngressItemAt,
             DrainIndex, DrainSource, DrainLaneIndex, DrainItem
    <2>4. /\ asyncCertifiedResponseClaim'
                  \subseteq asyncCertifiedResponseClaim
           /\ (DrainItem.kind = "CertifiedResponse"
                 /\ CertifiedResponseClaimMatches(DrainItem)
                 => AsyncCertifiedResponseCanonicalWireIdentity(DrainItem)
                      \notin asyncCertifiedResponseClaim')
      BY <1>1, <2>3, FS_Subset, SMT
         DEF DrainHistoricalIngressSelected, DrainItem
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4,
         PopIngressLanePreservesCertifiedResponseClaimIngressOwnership
         DEF DrainItem
  <1> QED BY <1>1


THEOREM LocalAdmissionPreservesHistoricalRecoveryType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ LocalAdmissionStep(node)
    => AsyncHistoricalRecoveryTypeInvariant'
BY SMTT(30)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
       EnqueueCandidate, LeaveCausalQueues, RecordBlockedCausalDebt,
       UpdateLocalAdmissionMetadata, AsyncHistoricalRecoveryTypeInvariant,
       NodeHasApplication, AsyncDeferredVars, AsyncIoVars,
       AsyncLocalAdmissionVars, AsyncAuxVars, vars

THEOREM SelectedLocalAdmissionAdvancePreservesHistoricalRecoveryType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ SelectedLocalAdmissionAdvance(node)
    => AsyncHistoricalRecoveryTypeInvariant'
BY SMTT(30)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       SelectedLocalAdmissionAdvance,
       AdmitProducerCompletion, AdmitCausalHead,
       EnqueueCandidate, LeaveCausalQueues,
       UpdateLocalAdmissionMetadata,
       AsyncHistoricalRecoveryTypeInvariant,
       NodeHasApplication, AsyncDeferredVars, AsyncIoVars,
       AsyncLocalAdmissionVars, AsyncAuxVars, vars

THEOREM IngressDrainPreservesHistoricalRecoveryType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ IngressDrainStep(node)
    => AsyncHistoricalRecoveryTypeInvariant'
BY SMTT(60)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       IngressDrainStep, DrainFairIngressSelected,
       ImportAuthenticatedCommitCertificate, PopSelectedIngress,
       AsyncHistoricalRecoveryTypeInvariant, NodeHasApplication,
       AsyncDeferredVars, AsyncLocalAdmissionVars, AsyncIoVars,
       AsyncAuxVars, vars

THEOREM ExecuteApplyPreservesHistoricalRecoveryType ==
  \A command:
    /\ AsyncHistoricalRecoveryTypeInvariant
    /\ ExecuteApply(command)
    => AsyncHistoricalRecoveryTypeInvariant'
BY SMTT(30)
   DEF AsyncHistoricalRecoveryTypeInvariant, ExecuteApply,
       ApplyDecision, NodeHasApplication, CommandMatches

THEOREM NonApplyExecutePreservesHistoricalRecoveryType ==
  \A command:
    /\ AsyncHistoricalRecoveryTypeInvariant
    /\ ExecuteCommand(command)
    /\ ~ExecuteApply(command)
    => AsyncHistoricalRecoveryTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                AsyncHistoricalRecoveryTypeInvariant,
                ExecuteCommand(command),
                ~ExecuteApply(command)
         PROVE AsyncHistoricalRecoveryTypeInvariant'
    <2>1. CASE ExecuteRegularCommand(command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <2>1, Isa
           DEF AsyncHistoricalRecoveryFrameVars,
               ExecuteRegularCommand, RegularCoreCommand,
               AssembleLocalBody, BeginLocalProposal, PersistProposal,
               FetchBody, RebindRetainedBody, StoreBody, ValidateBody,
               RejectBody, ValidateDecidedBody, ValidateLockedBody,
               BeginPrepare,
               PersistPrepare, BeginObservePrepare, PersistObservePrepare,
               BeginLockCommit, PersistLockCommit, FormCommitQC,
               BeginDecision, PersistTimeout, BeginInstallTC,
               FetchCertifiedBody, AsyncAuxVars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
    <2>2. CASE ExecuteDecisionFetch(command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <2>2, Isa
           DEF ExecuteDecisionFetch, AsyncHistoricalRecoveryFrameVars,
               PublishCertifiedRequests, vars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
    <2>3. CASE ExecuteSignProposal(command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <2>3, Isa
           DEF ExecuteSignProposal, CompleteProposalSignature,
               PublishControlAndEphemeralItems,
               AsyncHistoricalRecoveryFrameVars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
    <2>4. CASE ExecuteSignVote(command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <2>4, Isa
           DEF ExecuteSignVote, CompleteVoteSignature,
               PublishControlItems, AsyncHistoricalRecoveryFrameVars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
    <2>5. CASE ExecuteFormPrepareQC(command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <2>5, Isa
           DEF ExecuteFormPrepareQC, FormPrepareQC,
               PublishControlItems, AsyncHistoricalRecoveryFrameVars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
    <2>6. CASE ExecuteSignTimeout(command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <2>6, Isa
           DEF ExecuteSignTimeout, CompleteTimeoutSignature,
               PublishControlItems, AsyncHistoricalRecoveryFrameVars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
    <2>7. CASE ExecutePersistInstall(command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <2>7, Isa
           DEF ExecutePersistInstall, PersistInstallTC,
               PersistInstalledControl,
               PersistInstalledControlAfterInstall,
               AsyncHistoricalRecoveryFrameVars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
    <2>8. CASE ExecutePersistDecision(command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <2>8, Isa
           DEF ExecutePersistDecision, PersistDecision,
               PersistDecisionControl, AsyncHistoricalRecoveryFrameVars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
    <2>9. CASE ExecuteRequestCertifiedBody(command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <2>9, Isa
           DEF ExecuteRequestCertifiedBody, PublishCertifiedRequests,
               AsyncHistoricalRecoveryFrameVars, vars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
    <2>10. CASE ExecuteCoreDelivery(command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <2>10, Isa
           DEF ExecuteCoreDelivery, DeliverProposal, DeliverVote,
               DeliverQC, DeliverTimeout, DeliverTC,
               AsyncHistoricalRecoveryFrameVars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
    <2>11. CASE ExecuteChunkDelivery(command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <2>11, Isa
           DEF ExecuteChunkDelivery, AsyncHistoricalRecoveryFrameVars,
               vars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
    <2>12. CASE ExecuteRejectAuthenticatedJunk(command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <2>12, Isa
           DEF ExecuteRejectAuthenticatedJunk,
               AsyncHistoricalRecoveryFrameVars, vars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                  <2>7, <2>8, <2>9, <2>10, <2>11, <2>12
         DEF ExecuteCommand
  <1> QED BY <1>1

THEOREM NonCommandRuntimeLeafPreservesHistoricalRecoveryType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ (DeferredTagStep(node)
          \/ DirectTimeoutStep(node)
          \/ DirectRetransmitStep(node)
          \/ IdleRuntimeStep(node))
    => AsyncHistoricalRecoveryTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                DeferredTagStep(node)
                  \/ DirectTimeoutStep(node)
                  \/ DirectRetransmitStep(node)
                  \/ IdleRuntimeStep(node)
         PROVE AsyncHistoricalRecoveryTypeInvariant'
    <2>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
      BY <1>1, Isa
         DEF AsyncHistoricalRecoveryFrameVars,
             DeferredTagStep, DeferredTimeoutStep,
             DeferredRetransmitStep, DirectTimeoutStep,
             DirectRetransmitStep, IdleRuntimeStep,
             BeginTimeout, SendNodeRetransmissions, NoSendItem,
             LeaveCausalQueues, AppendCausalSuccessors,
             AsyncAuxVars, AsyncDeferredVars, vars
    <2> QED BY <1>1, <2>1, HistoricalRecoveryFramePreservesType
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant
  <1> QED BY <1>1

THEOREM FifoRuntimePreservesHistoricalRecoveryType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ FifoRuntimeStep(node)
    => AsyncHistoricalRecoveryTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                FifoRuntimeStep(node)
         PROVE AsyncHistoricalRecoveryTypeInvariant'
    <2> DEFINE Command == NextNodeCommand(node)
    <2>1. CASE CommandDispatchable(Command)
      <3>1. ExecuteCommand(Command)
        BY <1>1, <2>1 DEF FifoRuntimeStep, Command
      <3>2. CASE ExecuteApply(Command)
        BY <1>1, <3>2, ExecuteApplyPreservesHistoricalRecoveryType
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant
      <3>3. CASE ~ExecuteApply(Command)
        BY <1>1, <3>1, <3>3,
           NonApplyExecutePreservesHistoricalRecoveryType
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant
      <3> QED BY <3>2, <3>3
    <2>2. CASE ~CommandDispatchable(Command)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <1>1, <2>2, Isa
           DEF FifoRuntimeStep, Command, DeferCommand, DiscardCommand,
               LeaveCausalQueues, AsyncHistoricalRecoveryFrameVars,
               AsyncDeferredVars, vars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM DeferredDrainPreservesHistoricalRecoveryType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ DeferredDrainStep(node)
    => AsyncHistoricalRecoveryTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                DeferredDrainStep(node)
         PROVE AsyncHistoricalRecoveryTypeInvariant'
    <2>1. CASE ~DeferredQueueNonempty(node)
      <3>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
        BY <1>1, <2>1, Isa
           DEF DeferredDrainStep, DeferredWorkServiceable,
               LeaveCausalQueues,
               AsyncHistoricalRecoveryFrameVars, vars
      <3> QED BY <1>1, <3>1, HistoricalRecoveryFramePreservesType
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant
    <2>2. CASE DeferredQueueNonempty(node)
      <3> DEFINE Command == NextDeferredCommand(node)
      <3>1. CASE DeferredHandoffAllowsExecution(node, Command)
        <4>1. ExecuteCommand(Command)
          BY <1>1, <2>2, <3>1 DEF DeferredDrainStep, Command
        <4>2. CASE ExecuteApply(Command)
          BY <1>1, <4>2, ExecuteApplyPreservesHistoricalRecoveryType
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant
        <4>3. CASE ~ExecuteApply(Command)
          BY <1>1, <4>1, <4>3,
             NonApplyExecutePreservesHistoricalRecoveryType
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant
        <4> QED BY <4>2, <4>3
      <3>2. CASE ~DeferredHandoffAllowsExecution(node, Command)
        <4>1. UNCHANGED AsyncHistoricalRecoveryFrameVars
          BY <1>1, <2>2, <3>2, Isa
             DEF DeferredDrainStep, Command, DeferCommand, DiscardCommand,
                 LeaveCausalQueues, AdvanceNextDeferredClass,
                 AsyncHistoricalRecoveryFrameVars, vars
        <4> QED BY <1>1, <4>1, HistoricalRecoveryFramePreservesType
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM SerializedRuntimePreservesHistoricalRecoveryType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ SerializedRuntimeStep(node)
    => AsyncHistoricalRecoveryTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                SerializedRuntimeStep(node)
         PROVE AsyncHistoricalRecoveryTypeInvariant'
    <2>1. CASE DeferredDrainStep(node)
      BY <1>1, <2>1, DeferredDrainPreservesHistoricalRecoveryType
    <2>2. CASE FifoRuntimeStep(node)
      BY <1>1, <2>2, FifoRuntimePreservesHistoricalRecoveryType
    <2>3. CASE DeferredTagStep(node)
                   \/ DirectTimeoutStep(node)
                   \/ DirectRetransmitStep(node)
                   \/ IdleRuntimeStep(node)
      BY <1>1, <2>3,
         NonCommandRuntimeLeafPreservesHistoricalRecoveryType
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF SerializedRuntimeStep, RuntimeStep
  <1> QED BY <1>1


THEOREM RunnerScalarClockAndSchedulerStutterPreservesType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunnerServiceFrame(node)
    /\ asyncRunnerPhase'
         \in [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
    /\ asyncRunnerBudget'
         \in [ValidatorIds ->
               0..(AsyncQueueCapacity + AsyncIngressCapacity)]
    /\ asyncCausalAdmissionOwed' \in [ValidatorIds -> BOOLEAN]
    /\ asyncNextLocalSource' \in [ValidatorIds -> AsyncLocalSources]
    /\ AsyncHistoricalRecoveryTypeInvariant'
    /\ UNCHANGED <<context, asyncCommandQueues,
                    asyncNextCommandClass, asyncFifoOwed,
                    asyncTimeoutEmitted, asyncCausalQueues,
                    AsyncIoVars, AsyncDeferredVars,
                    asyncOutstandingTags, asyncNodeDeadlines,
                    asyncRetransmitDeadlines, asyncSentItems,
                    asyncRetainedControl, asyncActiveRequests,
                    asyncCertifiedResponseClaim,
                    asyncTransport, asyncIngressLanes,
                    asyncIngressReady, asyncHeldChunks>>
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                RunnerServiceFrame(node),
                asyncRunnerPhase'
                  \in [ValidatorIds -> {"Local", "Ingress", "Runtime"}],
                asyncRunnerBudget'
                  \in [ValidatorIds ->
                        0..(AsyncQueueCapacity + AsyncIngressCapacity)],
                asyncCausalAdmissionOwed' \in [ValidatorIds -> BOOLEAN],
                asyncNextLocalSource' \in
                  [ValidatorIds -> AsyncLocalSources],
                AsyncHistoricalRecoveryTypeInvariant',
                UNCHANGED <<context, asyncCommandQueues,
                            asyncNextCommandClass, asyncFifoOwed,
                            asyncTimeoutEmitted, asyncCausalQueues,
                            AsyncIoVars, AsyncDeferredVars,
                            asyncOutstandingTags, asyncNodeDeadlines,
                            asyncRetransmitDeadlines, asyncSentItems,
                            asyncRetainedControl, asyncActiveRequests,
                            asyncCertifiedResponseClaim,
                            asyncTransport, asyncIngressLanes,
                            asyncIngressReady, asyncHeldChunks>>
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
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
    <2>2. AsyncRuntimeScalarTypeInvariant'
      BY <1>1, <2>1, Isa
         DEF RunnerServiceFrame, AsyncRuntimeScalarTypeInvariant,
             AsyncConfiguration, AsyncIoVars, AsyncDeferredVars
    <2>3. AsyncTransportClockTypeInvariant'
      BY <1>1, <2>1, RunnerServiceFramePreservesClockType,
         Isa DEF AsyncIoVars, AsyncDeferredVars
    <2>4. /\ UNCHANGED asyncCausalQueues
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
      BY <1>1, Isa
         DEF AsyncIoVars, AsyncDeferredVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportContentTypeVars,
             AsyncCertifiedResponseClaimAuthorityVars,
             AsyncIngressTopologyTypeVars
    <2>5. /\ AsyncCausalTypeInvariant'
           /\ AsyncIoTopologyTypeInvariant'
           /\ AsyncIoContentTypeInvariant'
           /\ AsyncIoCapacityTypeInvariant'
           /\ AsyncDeferredTopologyTypeInvariant'
           /\ AsyncDeferredContentTypeInvariant'
           /\ AsyncTransportContentTypeInvariant'
           /\ AsyncIngressTopologyTypeInvariant'
           /\ AsyncIngressCapacityTypeInvariant'
           /\ AsyncIngressContentTypeInvariant'
      BY <2>1, <2>4, AsyncCausalTypeStutter,
         AsyncIoTopologyTypeStutter, AsyncIoContentTypeStutter,
         AsyncIoCapacityTypeStutter, AsyncDeferredTopologyTypeStutter,
         AsyncDeferredContentTypeStutter,
         AsyncTransportContentTypeStutter,
         AsyncIngressTopologyTypeStutter,
         AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter
    <2> QED BY <1>1, <2>2, <2>3, <2>5
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM LocalAdmissionPhaseAdvancePreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunNodeWork(node)
    /\ LocalAdmissionStep(node)
    /\ ~LocalAdmissionCanAdvance(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                RunNodeWork(node),
                LocalAdmissionStep(node),
                ~LocalAdmissionCanAdvance(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1
    <2>2. /\ RunnerServiceFrame(node)
           /\ asyncRunnerPhase' =
                [asyncRunnerPhase EXCEPT ![node] = "Ingress"]
           /\ asyncRunnerBudget' =
                [asyncRunnerBudget EXCEPT
                   ![node] = AsyncIngressCapacity]
           /\ RecordBlockedCausalDebt(node)
           /\ UNCHANGED <<context, asyncCommandQueues,
                          asyncNextCommandClass, asyncFifoOwed,
                          asyncTimeoutEmitted, asyncCausalQueues,
                          AsyncIoVars, AsyncDeferredVars,
                          asyncOutstandingTags, asyncNodeDeadlines,
                          asyncRetransmitDeadlines, asyncSentItems,
                          asyncRetainedControl, asyncActiveRequests,
                          asyncCertifiedResponseClaim,
                          asyncTransport, asyncIngressLanes,
                          asyncIngressReady, asyncHeldChunks>>
      BY <1>1, Isa
         DEF RunNodeWork, RunnerServiceFrame, LocalAdmissionStep,
             LeaveCausalQueues, vars
    <2>3. /\ asyncCausalAdmissionOwed
                    \in [ValidatorIds -> BOOLEAN]
           /\ asyncNextLocalSource
                    \in [ValidatorIds -> AsyncLocalSources]
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant
    <2>4. /\ asyncCausalAdmissionOwed'
                    \in [ValidatorIds -> BOOLEAN]
           /\ asyncNextLocalSource'
                    \in [ValidatorIds -> AsyncLocalSources]
      BY <2>1, <2>2, <2>3, FunctionalUpdatePreservesType, SMT
         DEF RecordBlockedCausalDebt
    <2>5. /\ asyncRunnerPhase
                    \in [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
           /\ asyncRunnerBudget
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
           /\ AsyncIngressCapacity
                    \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
      BY <1>1, SMT
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncConfiguration
    <2>6. /\ asyncRunnerPhase'
                    \in [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
           /\ asyncRunnerBudget'
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
      BY <2>1, <2>2, <2>5, FunctionalUpdatePreservesType
    <2> QED BY <1>1, <2>1, <2>2, <2>4, <2>6,
                    LocalAdmissionPreservesHistoricalRecoveryType,
                    RunnerScalarClockAndSchedulerStutterPreservesType
  <1> QED BY <1>1

THEOREM AdmitProducerCompletionPreservesIoTopologyType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ ProducerCompletionCanAdmit(node)
    /\ AdmitProducerCompletion(node)
    => AsyncIoTopologyTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                ProducerCompletionCanAdmit(node),
                AdmitProducerCompletion(node)
         PROVE AsyncIoTopologyTypeInvariant'
    <2>1. AsyncIoTopologyTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant
    <2>2. SelectedCompletionSource(node) \in {"Io", "Local"}
      BY <2>1, SMT
         DEF AsyncIoTopologyTypeInvariant, SelectedCompletionSource
    <2>3. /\ asyncIoQueues' = asyncIoQueues
           /\ asyncOutstandingWork' =
                [asyncOutstandingWork EXCEPT
                   ![node] = @ \ {SelectedCompletionCandidate(node)}]
           /\ asyncIoReadyCompletions' =
                (IF SelectedCompletionSource(node) = "Io"
                 THEN [asyncIoReadyCompletions EXCEPT
                         ![node] = Tail(@)]
                 ELSE asyncIoReadyCompletions)
           /\ asyncLocalReadyCompletions' =
                (IF SelectedCompletionSource(node) = "Local"
                 THEN [asyncLocalReadyCompletions EXCEPT
                         ![node] = Tail(@)]
                 ELSE asyncLocalReadyCompletions)
           /\ asyncNextCompletionSource' =
                [asyncNextCompletionSource EXCEPT
                   ![node] =
                     IF SelectedCompletionSource(node) = "Io"
                     THEN "Local" ELSE "Io"]
           /\ asyncIoControlAvailable' = asyncIoControlAvailable
      BY <1>1, Isa
         DEF AdmitProducerCompletion, EnqueueCandidate
    <2>4. /\ DOMAIN asyncOutstandingWork' = ValidatorIds
           /\ DOMAIN asyncIoReadyCompletions' = ValidatorIds
           /\ DOMAIN asyncLocalReadyCompletions' = ValidatorIds
           /\ asyncNextCompletionSource'
                \in [ValidatorIds -> {"Io", "Local"}]
      BY <1>1, <2>1, <2>2, <2>3,
         FunctionalUpdatePreservesType, Isa
         DEF AsyncIoTopologyTypeInvariant
    <2> QED BY <2>1, <2>3, <2>4
         DEF AsyncIoTopologyTypeInvariant
  <1> QED BY <1>1, Isa

THEOREM AdmitProducerCompletionPreservesIoQueueContentType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ ProducerCompletionCanAdmit(node)
    /\ AdmitProducerCompletion(node)
    => AsyncIoQueueContentTypeInvariant'
BY ProducerSelectedCompletionFacts, UniqueCompletionTailFacts,
   FunctionalUpdateAwayFromKey, SMTT(30)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
       AsyncIoContentTypeInvariant, AsyncIoTopologyTypeInvariant,
       AsyncIoQueueContentTypeInvariant,
       AsyncIoWorkContentTypeInvariant,
       AsyncIoConsensusCandidateOwnership,
       AsyncIoConsensusQueueOwnership, AsyncIoConsensusIndices,
       AdmitProducerCompletion, EnqueueCandidate,
       ProducerSelectedReadyQueue, ProducerOtherReadyQueue,
       SelectedCompletionSource, SequenceSet

THEOREM ProducerCompletionMovePreservesWorkFacts ==
  \A node, commandQueue, selectedQueue, otherQueue,
     outstanding, candidate:
    /\ node \in ValidatorIds
    /\ AsyncQueueTyped(commandQueue)
    /\ IsFiniteSet(outstanding)
    /\ \A work \in outstanding:
         /\ AsyncCandidateTyped(work)
         /\ work.class = "Completion"
         /\ work.node = node
    /\ AsyncCompletionSequenceTyped(selectedQueue)
    /\ AsyncCompletionSequenceTyped(otherQueue)
    /\ Len(selectedQueue) =
         Cardinality(SequenceSet(selectedQueue))
    /\ Len(otherQueue) =
         Cardinality(SequenceSet(otherQueue))
    /\ SequenceSet(selectedQueue) \subseteq outstanding
    /\ SequenceSet(otherQueue) \subseteq outstanding
    /\ SequenceSet(selectedQueue) \cap
         SequenceSet(otherQueue) = {}
    /\ SequenceSet(commandQueue) \cap outstanding = {}
    /\ Len(selectedQueue) > 0
    /\ candidate = Head(selectedQueue)
    /\ candidate \in SequenceSet(selectedQueue)
    => LET remaining == outstanding \ {candidate}
           selectedTail == Tail(selectedQueue)
           commandWithCandidate == Append(commandQueue, candidate)
       IN /\ IsFiniteSet(remaining)
          /\ \A work \in remaining:
               /\ AsyncCandidateTyped(work)
               /\ work.class = "Completion"
               /\ work.node = node
          /\ AsyncCompletionSequenceTyped(selectedTail)
          /\ AsyncCompletionSequenceTyped(otherQueue)
          /\ Len(selectedTail) =
               Cardinality(SequenceSet(selectedTail))
          /\ Len(otherQueue) =
               Cardinality(SequenceSet(otherQueue))
          /\ SequenceSet(selectedTail) \subseteq remaining
          /\ SequenceSet(otherQueue) \subseteq remaining
          /\ SequenceSet(selectedTail) \cap
               SequenceSet(otherQueue) = {}
          /\ AsyncQueueTyped(commandWithCandidate)
          /\ SequenceSet(commandWithCandidate) \cap remaining = {}
PROOF
  <1>1. ASSUME NEW node, NEW commandQueue, NEW selectedQueue,
                NEW otherQueue, NEW outstanding, NEW candidate,
                node \in ValidatorIds,
                AsyncQueueTyped(commandQueue),
                IsFiniteSet(outstanding),
                \A work \in outstanding:
                  /\ AsyncCandidateTyped(work)
                  /\ work.class = "Completion"
                  /\ work.node = node,
                AsyncCompletionSequenceTyped(selectedQueue),
                AsyncCompletionSequenceTyped(otherQueue),
                Len(selectedQueue) =
                  Cardinality(SequenceSet(selectedQueue)),
                Len(otherQueue) =
                  Cardinality(SequenceSet(otherQueue)),
                SequenceSet(selectedQueue) \subseteq outstanding,
                SequenceSet(otherQueue) \subseteq outstanding,
                SequenceSet(selectedQueue) \cap
                  SequenceSet(otherQueue) = {},
                SequenceSet(commandQueue) \cap outstanding = {},
                Len(selectedQueue) > 0,
                candidate = Head(selectedQueue),
                candidate \in SequenceSet(selectedQueue)
         PROVE LET remaining == outstanding \ {candidate}
                   selectedTail == Tail(selectedQueue)
                   commandWithCandidate ==
                     Append(commandQueue, candidate)
               IN /\ IsFiniteSet(remaining)
                  /\ \A work \in remaining:
                       /\ AsyncCandidateTyped(work)
                       /\ work.class = "Completion"
                       /\ work.node = node
                  /\ AsyncCompletionSequenceTyped(selectedTail)
                  /\ AsyncCompletionSequenceTyped(otherQueue)
                  /\ Len(selectedTail) =
                       Cardinality(SequenceSet(selectedTail))
                  /\ Len(otherQueue) =
                       Cardinality(SequenceSet(otherQueue))
                  /\ SequenceSet(selectedTail) \subseteq remaining
                  /\ SequenceSet(otherQueue) \subseteq remaining
                  /\ SequenceSet(selectedTail) \cap
                       SequenceSet(otherQueue) = {}
                  /\ AsyncQueueTyped(commandWithCandidate)
                  /\ SequenceSet(commandWithCandidate) \cap
                       remaining = {}
    <2>1. /\ AsyncCandidateTyped(candidate)
           /\ candidate.class = "Completion"
           /\ candidate.node = node
      BY <1>1, SMT
    <2>2. /\ AsyncCompletionSequenceTyped(Tail(selectedQueue))
           /\ SequenceSet(Tail(selectedQueue)) =
                SequenceSet(selectedQueue) \ {candidate}
           /\ Len(Tail(selectedQueue)) =
                Cardinality(SequenceSet(Tail(selectedQueue)))
      BY <1>1, UniqueCompletionTailFacts
    <2>3. AsyncQueueTyped(Append(commandQueue, candidate))
      BY <1>1, <2>1, TypedCandidateAppendPreservesQueueType
    <2>4. SequenceSet(Append(commandQueue, candidate)) =
             SequenceSet(commandQueue) \cup {candidate}
      BY <1>1, SequenceSetAfterAppend DEF AsyncQueueTyped
    <2>5. /\ IsFiniteSet(outstanding \ {candidate})
           /\ \A work \in outstanding \ {candidate}:
                /\ AsyncCandidateTyped(work)
                /\ work.class = "Completion"
                /\ work.node = node
      BY <1>1, FS_RemoveElement, SMT
    <2>6. candidate \notin SequenceSet(otherQueue)
      BY <1>1, SMT
    <2>7. /\ SequenceSet(Tail(selectedQueue)) \subseteq
                  outstanding \ {candidate}
           /\ SequenceSet(otherQueue) \subseteq
                  outstanding \ {candidate}
           /\ SequenceSet(Tail(selectedQueue)) \cap
                  SequenceSet(otherQueue) = {}
      BY <1>1, <2>2, <2>6, SMT
    <2>8. SequenceSet(Append(commandQueue, candidate)) \cap
             (outstanding \ {candidate}) = {}
      BY <1>1, <2>4, SMT
    <2> QED BY <1>1, <2>2, <2>3, <2>5, <2>7, <2>8
  <1> QED BY <1>1

THEOREM AdmitProducerCompletionPreservesIoWorkContentType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ ProducerCompletionCanAdmit(node)
    /\ AdmitProducerCompletion(node)
    => AsyncIoWorkContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                ProducerCompletionCanAdmit(node),
                AdmitProducerCompletion(node)
         PROVE AsyncIoWorkContentTypeInvariant'
    <2> DEFINE Candidate == SelectedCompletionCandidate(node)
    <2> DEFINE Selected == ProducerSelectedReadyQueue(node)
    <2> DEFINE Other == ProducerOtherReadyQueue(node)
    <2>1. /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>2. /\ SelectedCompletionSource(node) \in {"Io", "Local"}
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
      BY <1>1, <2>1, ProducerSelectedCompletionFacts
         DEF Candidate, Selected, Other
    <2>3. /\ DOMAIN asyncCommandQueues = ValidatorIds
           /\ AsyncQueueTyped(asyncCommandQueues[node])
           /\ IsFiniteSet(asyncOutstandingWork[node])
           /\ (\A work \in asyncOutstandingWork[node]:
                 /\ AsyncCandidateTyped(work)
                 /\ work.class = "Completion"
                 /\ work.node = node)
           /\ SequenceSet(asyncCommandQueues[node]) \cap
                asyncOutstandingWork[node] = {}
      BY <1>1, <2>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncIoWorkContentTypeInvariant
    <2>4. /\ AsyncCompletionSequenceTyped(
                    asyncIoReadyCompletions[node])
           /\ AsyncCompletionSequenceTyped(
                    asyncLocalReadyCompletions[node])
           /\ Len(asyncIoReadyCompletions[node]) =
                Cardinality(SequenceSet(
                  asyncIoReadyCompletions[node]))
           /\ Len(asyncLocalReadyCompletions[node]) =
                Cardinality(SequenceSet(
                  asyncLocalReadyCompletions[node]))
           /\ SequenceSet(asyncIoReadyCompletions[node]) \subseteq
                asyncOutstandingWork[node]
           /\ SequenceSet(asyncLocalReadyCompletions[node]) \subseteq
                asyncOutstandingWork[node]
           /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                SequenceSet(asyncLocalReadyCompletions[node]) = {}
      BY <2>1 DEF AsyncIoWorkContentTypeInvariant
    <2>5. /\ AsyncCompletionSequenceTyped(Other)
           /\ Len(Other) = Cardinality(SequenceSet(Other))
           /\ SequenceSet(Selected) \subseteq
                asyncOutstandingWork[node]
           /\ SequenceSet(Other) \subseteq
                asyncOutstandingWork[node]
           /\ SequenceSet(Selected) \cap SequenceSet(Other) = {}
      <3>1. CASE SelectedCompletionSource(node) = "Io"
        BY <2>4, <3>1
           DEF Selected, Other, ProducerSelectedReadyQueue,
               ProducerOtherReadyQueue
      <3>2. CASE SelectedCompletionSource(node) = "Local"
        BY <2>4, <3>2, SMT
           DEF Selected, Other, ProducerSelectedReadyQueue,
               ProducerOtherReadyQueue
      <3> QED BY <2>2, <3>1, <3>2
    <2>6. LET Remaining ==
                     asyncOutstandingWork[node] \ {Candidate}
               SelectedTail == Tail(Selected)
               CommandWithCandidate ==
                     Append(asyncCommandQueues[node], Candidate)
           IN /\ IsFiniteSet(Remaining)
              /\ \A work \in Remaining:
                   /\ AsyncCandidateTyped(work)
                   /\ work.class = "Completion"
                   /\ work.node = node
              /\ AsyncCompletionSequenceTyped(SelectedTail)
              /\ AsyncCompletionSequenceTyped(Other)
              /\ Len(SelectedTail) =
                   Cardinality(SequenceSet(SelectedTail))
              /\ Len(Other) = Cardinality(SequenceSet(Other))
              /\ SequenceSet(SelectedTail) \subseteq Remaining
              /\ SequenceSet(Other) \subseteq Remaining
              /\ SequenceSet(SelectedTail) \cap
                   SequenceSet(Other) = {}
              /\ AsyncQueueTyped(CommandWithCandidate)
              /\ SequenceSet(CommandWithCandidate) \cap Remaining = {}
      BY <1>1, <2>2, <2>3, <2>5,
         ProducerCompletionMovePreservesWorkFacts
         DEF Candidate, Selected, Other
    <2>7. \A otherNode \in ValidatorIds:
             /\ IsFiniteSet(asyncOutstandingWork'[otherNode])
             /\ \A candidate \in asyncOutstandingWork'[otherNode]:
                  /\ AsyncCandidateTyped(candidate)
                  /\ candidate.class = "Completion"
                  /\ candidate.node = otherNode
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions'[otherNode])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions'[otherNode])
             /\ Len(asyncIoReadyCompletions'[otherNode]) =
                  Cardinality(SequenceSet(
                    asyncIoReadyCompletions'[otherNode]))
             /\ Len(asyncLocalReadyCompletions'[otherNode]) =
                  Cardinality(SequenceSet(
                    asyncLocalReadyCompletions'[otherNode]))
             /\ SequenceSet(asyncIoReadyCompletions'[otherNode])
                  \subseteq asyncOutstandingWork'[otherNode]
             /\ SequenceSet(asyncLocalReadyCompletions'[otherNode])
                  \subseteq asyncOutstandingWork'[otherNode]
             /\ SequenceSet(asyncIoReadyCompletions'[otherNode]) \cap
                  SequenceSet(asyncLocalReadyCompletions'[otherNode]) = {}
             /\ SequenceSet(asyncCommandQueues'[otherNode]) \cap
                  asyncOutstandingWork'[otherNode] = {}
      <3>1. ASSUME NEW otherNode \in ValidatorIds
             PROVE /\ IsFiniteSet(asyncOutstandingWork'[otherNode])
                   /\ \A candidate \in
                          asyncOutstandingWork'[otherNode]:
                        /\ AsyncCandidateTyped(candidate)
                        /\ candidate.class = "Completion"
                        /\ candidate.node = otherNode
                   /\ AsyncCompletionSequenceTyped(
                        asyncIoReadyCompletions'[otherNode])
                   /\ AsyncCompletionSequenceTyped(
                        asyncLocalReadyCompletions'[otherNode])
                   /\ Len(asyncIoReadyCompletions'[otherNode]) =
                        Cardinality(SequenceSet(
                          asyncIoReadyCompletions'[otherNode]))
                   /\ Len(asyncLocalReadyCompletions'[otherNode]) =
                        Cardinality(SequenceSet(
                          asyncLocalReadyCompletions'[otherNode]))
                   /\ SequenceSet(
                        asyncIoReadyCompletions'[otherNode])
                        \subseteq asyncOutstandingWork'[otherNode]
                   /\ SequenceSet(
                        asyncLocalReadyCompletions'[otherNode])
                        \subseteq asyncOutstandingWork'[otherNode]
                   /\ SequenceSet(
                        asyncIoReadyCompletions'[otherNode]) \cap
                        SequenceSet(
                          asyncLocalReadyCompletions'[otherNode]) = {}
                   /\ SequenceSet(asyncCommandQueues'[otherNode]) \cap
                        asyncOutstandingWork'[otherNode] = {}
        <4>1. CASE otherNode = node
          <5>1. CASE SelectedCompletionSource(node) = "Io"
            <6>1. /\ asyncOutstandingWork'[node] =
                              asyncOutstandingWork[node] \ {Candidate}
                   /\ asyncCommandQueues'[node] =
                              Append(asyncCommandQueues[node], Candidate)
                   /\ asyncIoReadyCompletions'[node] = Tail(Selected)
                   /\ asyncLocalReadyCompletions'[node] = Other
              BY <1>1, <2>1, <2>2, <2>3, <5>1,
                 FunctionalAppendUpdateAtKey,
                 FunctionalTailUpdateAtKey, Isa
                 DEF AsyncIoTopologyTypeInvariant,
                     AdmitProducerCompletion, EnqueueCandidate,
                     Candidate, Selected, Other,
                     ProducerSelectedReadyQueue,
                     ProducerOtherReadyQueue
            <6> QED BY <2>6, <4>1, <6>1
                 DEF Candidate, Selected, Other
          <5>2. CASE SelectedCompletionSource(node) = "Local"
            <6>1. /\ asyncOutstandingWork'[node] =
                              asyncOutstandingWork[node] \ {Candidate}
                   /\ asyncCommandQueues'[node] =
                              Append(asyncCommandQueues[node], Candidate)
                   /\ asyncIoReadyCompletions'[node] = Other
                   /\ asyncLocalReadyCompletions'[node] = Tail(Selected)
              BY <1>1, <2>1, <2>2, <2>3, <5>2,
                 FunctionalAppendUpdateAtKey,
                 FunctionalTailUpdateAtKey, Isa
                 DEF AsyncIoTopologyTypeInvariant,
                     AdmitProducerCompletion, EnqueueCandidate,
                     Candidate, Selected, Other,
                     ProducerSelectedReadyQueue,
                     ProducerOtherReadyQueue
            <6> QED BY <2>6, <4>1, <6>1
                 DEF Candidate, Selected, Other
          <5> QED BY <2>2, <5>1, <5>2
        <4>2. CASE otherNode # node
          <5>1. CASE SelectedCompletionSource(node) = "Io"
            <6>1. /\ asyncOutstandingWork'[otherNode] =
                              asyncOutstandingWork[otherNode]
                   /\ asyncCommandQueues'[otherNode] =
                              asyncCommandQueues[otherNode]
                   /\ asyncIoReadyCompletions'[otherNode] =
                              asyncIoReadyCompletions[otherNode]
                   /\ asyncLocalReadyCompletions'[otherNode] =
                              asyncLocalReadyCompletions[otherNode]
              BY <1>1, <2>1, <2>2, <2>3, <3>1, <4>2, <5>1,
                 FunctionalUpdateAwayFromKey,
                 FunctionalAppendUpdateAwayFromKey,
                 FunctionalTailUpdateAwayFromKey, Isa
                 DEF AsyncIoTopologyTypeInvariant,
                     AdmitProducerCompletion, EnqueueCandidate,
                     Candidate
            <6> QED BY <2>1, <3>1, <6>1
                 DEF AsyncIoWorkContentTypeInvariant
          <5>2. CASE SelectedCompletionSource(node) = "Local"
            <6>1. /\ asyncOutstandingWork'[otherNode] =
                              asyncOutstandingWork[otherNode]
                   /\ asyncCommandQueues'[otherNode] =
                              asyncCommandQueues[otherNode]
                   /\ asyncIoReadyCompletions'[otherNode] =
                              asyncIoReadyCompletions[otherNode]
                   /\ asyncLocalReadyCompletions'[otherNode] =
                              asyncLocalReadyCompletions[otherNode]
              BY <1>1, <2>1, <2>2, <2>3, <3>1, <4>2, <5>2,
                 FunctionalUpdateAwayFromKey,
                 FunctionalAppendUpdateAwayFromKey,
                 FunctionalTailUpdateAwayFromKey, Isa
                 DEF AsyncIoTopologyTypeInvariant,
                     AdmitProducerCompletion, EnqueueCandidate,
                     Candidate
            <6> QED BY <2>1, <3>1, <6>1
                 DEF AsyncIoWorkContentTypeInvariant
          <5> QED BY <2>1, <2>2, <3>1, <5>1, <5>2
               DEF AsyncIoWorkContentTypeInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>7 DEF AsyncIoWorkContentTypeInvariant
  <1> QED BY <1>1

THEOREM AdmitProducerCompletionWithDeferredFramePreservesIoCapacityType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ ProducerCompletionCanAdmit(node)
    /\ AdmitProducerCompletion(node)
    /\ UNCHANGED asyncDeferredCompletionQueues
    => AsyncIoCapacityTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                ProducerCompletionCanAdmit(node),
                AdmitProducerCompletion(node),
                UNCHANGED asyncDeferredCompletionQueues
         PROVE AsyncIoCapacityTypeInvariant'
    <2> DEFINE Candidate == SelectedCompletionCandidate(node)
    <2>1. /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>2. /\ Candidate \in asyncOutstandingWork[node]
           /\ Candidate.class = "Completion"
           /\ Candidate.node = node
           /\ DOMAIN asyncCommandQueues = ValidatorIds
           /\ AsyncQueueTyped(asyncCommandQueues[node])
           /\ IsFiniteSet(asyncOutstandingWork[node])
      BY <1>1, <2>1, ProducerSelectedCompletionFacts
         DEF Candidate, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoWorkContentTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped
    <2>3. /\ IsFiniteSet(
                    asyncOutstandingWork[node] \ {Candidate})
           /\ Cardinality(asyncOutstandingWork[node]) \in Nat
           /\ Cardinality(asyncOutstandingWork[node]) # 0
           /\ Cardinality(
                asyncOutstandingWork[node] \ {Candidate}) =
                Cardinality(asyncOutstandingWork[node]) - 1
      BY <2>2, FS_RemoveElement, FS_CardinalityType,
         FS_EmptySet, SMT
    <2>4. Cardinality(
              asyncOutstandingWork[node] \ {Candidate}) + 1 =
            Cardinality(asyncOutstandingWork[node])
      BY <2>3, SMT
    <2>5. Cardinality(AsyncCompletionIndices(
              Append(asyncCommandQueues[node], Candidate))) =
            Cardinality(AsyncCompletionIndices(
              asyncCommandQueues[node])) + 1
      BY <2>2, CompletionAppendCountIncreasesByOne
    <2>6. /\ asyncCommandQueues'[node] =
                    Append(asyncCommandQueues[node], Candidate)
           /\ asyncOutstandingWork'[node] =
                    asyncOutstandingWork[node] \ {Candidate}
           /\ asyncIoQueues'[node] = asyncIoQueues[node]
           /\ asyncDeferredCompletionQueues'[node] =
                    asyncDeferredCompletionQueues[node]
      BY <1>1, <2>1, <2>2, FunctionalAppendUpdateAtKey, Isa
         DEF AsyncIoTopologyTypeInvariant,
             AdmitProducerCompletion, EnqueueCandidate, Candidate
    <2>7. Len(Append(asyncCommandQueues[node], Candidate)) =
             Len(asyncCommandQueues[node]) + 1
      BY <2>2, AppendSequenceFacts DEF AsyncQueueTyped
    <2>8. AsyncQueueDepth(node)' = AsyncQueueDepth(node) + 1
      BY <2>6, <2>7, Isa DEF AsyncQueueDepth
    <2>9. AsyncIoQueueDepth(node)' = AsyncIoQueueDepth(node)
      BY <2>6 DEF AsyncIoQueueDepth
    <2>10. AsyncOutstandingWorkCount(node)' + 1 =
             AsyncOutstandingWorkCount(node)
      BY <2>4, <2>6 DEF AsyncOutstandingWorkCount
    <2>11. QueuedCompletionCount(node)' =
              QueuedCompletionCount(node) + 1
      BY <2>5, <2>6
         DEF QueuedCompletionCount, QueuedCompletionIndices,
             AsyncCompletionIndices
    <2>12. DeferredCompletionCount(node)' =
              DeferredCompletionCount(node)
      BY <2>6 DEF DeferredCompletionCount
    <2>13. AsyncOutstandingWorkCount(node) \in Nat
      BY <2>2, FS_CardinalityType DEF AsyncOutstandingWorkCount
    <2>14. /\ Len(asyncCommandQueues[node]) \in Nat
            /\ IsFiniteSet(1..Len(asyncCommandQueues[node]))
      BY <2>2, LenProperties, FS_Interval, SMT DEF AsyncQueueTyped
    <2>15. /\ QueuedCompletionIndices(node)
                  \subseteq 1..Len(asyncCommandQueues[node])
            /\ IsFiniteSet(QueuedCompletionIndices(node))
      <3>1. QueuedCompletionIndices(node)
                 \subseteq 1..Len(asyncCommandQueues[node])
        BY DEF QueuedCompletionIndices
      <3>2. IsFiniteSet(QueuedCompletionIndices(node))
        BY <2>14, <3>1, FS_Subset
      <3> QED BY <3>1, <3>2
    <2>16. QueuedCompletionCount(node) \in Nat
      BY <2>15, FS_CardinalityType DEF QueuedCompletionCount
    <2>17. asyncDeferredCompletionQueues[node]
                \in Seq(Range(asyncDeferredCompletionQueues[node]))
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncDeferredTypeInvariant,
             AsyncDeferredContentTypeInvariant,
             AsyncCompletionSequenceTyped
    <2>18. DeferredCompletionCount(node) \in Nat
      BY <2>17, LenProperties DEF DeferredCompletionCount
    <2>19. Cardinality(
                asyncOutstandingWork[node] \ {Candidate}) \in Nat
      BY <2>3, FS_CardinalityType
    <2>20. AsyncOutstandingWorkCount(node)' \in Nat
      BY <2>6, <2>19 DEF AsyncOutstandingWorkCount
    <2>21. QueuedCompletionCount(node)' \in Nat
      BY <2>11, <2>16, SMT
    <2>22. DeferredCompletionCount(node)' \in Nat
      BY <2>12, <2>18, SMT
    <2>23. AsyncCompletionLoad(node)' = AsyncCompletionLoad(node)
      BY <2>10, <2>11, <2>12, <2>13, <2>16, <2>18,
         <2>20, <2>21, <2>22, SMT
         DEF AsyncCompletionLoad
    <2>24. AsyncQueueDepth(node) < AsyncQueueCapacity
      BY <1>1
         DEF ProducerCompletionCanAdmit, CanEnqueueClass,
             CanEnqueueWithCertifiedFenceCredit
    <2>25. /\ AsyncQueueDepth(node) \in Nat
            /\ AsyncQueueCapacity \in Nat
      BY <1>1, <2>2, LenProperties, SMT
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncConfiguration, AsyncQueueDepth, AsyncQueueTyped
    <2>26. /\ AsyncOutstandingWorkCount(node) <= AsyncIoWorkCapacity
            /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
      BY <2>1 DEF AsyncIoCapacityTypeInvariant
    <2>27. AsyncQueueDepth(node)' <= AsyncQueueCapacity
      BY <2>8, <2>24, <2>25, SMT
    <2>28. \A otherNode \in ValidatorIds:
             /\ AsyncQueueDepth(otherNode)' <= AsyncQueueCapacity
             /\ AsyncIoQueueDepth(otherNode)' <= AsyncIoCapacity
             /\ AsyncOutstandingWorkCount(otherNode)' <=
                  AsyncIoWorkCapacity
      <3>1. ASSUME NEW otherNode \in ValidatorIds
             PROVE /\ AsyncQueueDepth(otherNode)' <=
                          AsyncQueueCapacity
                   /\ AsyncIoQueueDepth(otherNode)' <= AsyncIoCapacity
                   /\ AsyncOutstandingWorkCount(otherNode)' <=
                          AsyncIoWorkCapacity
        <4>1. CASE otherNode = node
          BY <2>9, <2>10, <2>13, <2>20, <2>26, <2>27,
             <3>1, <4>1, SMT
        <4>2. CASE otherNode # node
          <5>1. /\ asyncCommandQueues'[otherNode] =
                            asyncCommandQueues[otherNode]
                 /\ asyncOutstandingWork'[otherNode] =
                            asyncOutstandingWork[otherNode]
                 /\ asyncIoQueues'[otherNode] = asyncIoQueues[otherNode]
                 /\ asyncDeferredCompletionQueues'[otherNode] =
                            asyncDeferredCompletionQueues[otherNode]
            BY <1>1, <2>1, <2>2, <3>1, <4>2,
               FunctionalUpdateAwayFromKey, Isa
               DEF AsyncIoTopologyTypeInvariant,
                   AdmitProducerCompletion, EnqueueCandidate
          <5>2. AsyncQueueDepth(otherNode)' =
                   AsyncQueueDepth(otherNode)
            BY <5>1 DEF AsyncQueueDepth
          <5>3. AsyncOutstandingWorkCount(otherNode)' =
                   AsyncOutstandingWorkCount(otherNode)
            BY <5>1 DEF AsyncOutstandingWorkCount
          <5>4. AsyncIoQueueDepth(otherNode)' =
                   AsyncIoQueueDepth(otherNode)
            BY <5>1 DEF AsyncIoQueueDepth
          <5> QED BY <2>1, <3>1, <5>2, <5>3, <5>4
               DEF AsyncIoCapacityTypeInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>28 DEF AsyncIoCapacityTypeInvariant
  <1> QED BY <1>1

THEOREM AdmitProducerCompletionWithDeferredFramePreservesIoType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ ProducerCompletionCanAdmit(node)
    /\ AdmitProducerCompletion(node)
    /\ UNCHANGED asyncDeferredCompletionQueues
    => AsyncIoTypeInvariant'
BY AdmitProducerCompletionPreservesIoTopologyType,
   AdmitProducerCompletionPreservesIoQueueContentType,
   AdmitProducerCompletionPreservesIoWorkContentType,
   AdmitProducerCompletionWithDeferredFramePreservesIoCapacityType
   DEF AsyncIoTypeInvariant, AsyncIoContentTypeInvariant

THEOREM ProducerAdmissionRunnerPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunNodeWork(node)
    /\ SelectedLocalAdmissionAdvance(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Producer"
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                RunNodeWork(node),
                SelectedLocalAdmissionAdvance(node),
                LocalAdmissionCanAdvance(node),
                SelectedLocalSource(node) = "Producer"
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1
    <2>2. /\ AdmitProducerCompletion(node)
           /\ UNCHANGED asyncDeferredCompletionQueues
           /\ UpdateLocalAdmissionMetadata(node, "Producer")
      BY <1>1 DEF SelectedLocalAdmissionAdvance, AsyncDeferredVars
    <2>3. AsyncIoTypeInvariant'
      BY <1>1, <2>1, <2>2,
         AdmitProducerCompletionWithDeferredFramePreservesIoType
    <2>4. AsyncRuntimeScalarTypeInvariant'
      BY <1>1, <2>1, <2>2, ProducerSelectedCompletionFacts,
         TypedCandidateAppendPreservesQueueType,
         AppendOwnedCandidatePreservesCommandQueueOwnership,
         FunctionalUpdatePreservesType, FunctionalUpdateAwayFromKey,
         LocalAdmissionMetadataUpdatePreservesType,
         SMTT(30)
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoTopologyTypeInvariant,
             AsyncIoWorkContentTypeInvariant, RunNodeWork,
             SelectedLocalAdmissionAdvance, AdmitProducerCompletion,
             EnqueueCandidate, ProducerSelectedReadyQueue,
             ProducerOtherReadyQueue, SelectedCompletionSource,
             AsyncConfiguration, AsyncLocalSources,
             OtherLocalSource, vars
    <2>5. /\ AsyncCausalTypeInvariant
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
    <2>6. /\ UNCHANGED asyncCausalQueues
           /\ UNCHANGED AsyncDeferredTopologyTypeVars
           /\ UNCHANGED <<asyncDeferredCompletionQueues,
                          asyncDeferredProgressQueues,
                          asyncDeferredNormalQueues>>
           /\ UNCHANGED AsyncTransportContentTypeVars
           /\ UNCHANGED AsyncIngressTopologyTypeVars
           /\ UNCHANGED asyncIngressLanes
      BY <1>1, <2>2, Isa
         DEF RunNodeWork, SelectedLocalAdmissionAdvance,
             AdmitProducerCompletion,
             LeaveCausalQueues, AsyncDeferredVars,
             AsyncDeferredTopologyTypeVars,
             AsyncTransportContentTypeVars,
             AsyncCertifiedResponseClaimAuthorityVars,
             AsyncIngressTopologyTypeVars, vars
    <2>7. /\ AsyncCausalTypeInvariant'
           /\ AsyncDeferredTopologyTypeInvariant'
           /\ AsyncDeferredContentTypeInvariant'
           /\ AsyncTransportContentTypeInvariant'
           /\ AsyncIngressTopologyTypeInvariant'
           /\ AsyncIngressCapacityTypeInvariant'
           /\ AsyncIngressContentTypeInvariant'
      BY <2>5, <2>6, AsyncCausalTypeStutter,
         AsyncDeferredTopologyTypeStutter,
         AsyncDeferredContentTypeStutter,
         AsyncTransportContentTypeStutter,
         AsyncIngressTopologyTypeStutter,
         AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter
    <2>8. /\ RunnerServiceFrame(node)
           /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                          asyncRetransmitDeadlines>>
      BY <1>1, <2>2, Isa
         DEF RunNodeWork, RunnerServiceFrame,
             SelectedLocalAdmissionAdvance,
             AdmitProducerCompletion, vars
    <2>9. AsyncTransportClockTypeInvariant'
      BY <1>1, <2>1, <2>5, <2>8,
         RunnerServiceFramePreservesClockType
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant
    <2>10. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1,
         SelectedLocalAdmissionAdvancePreservesHistoricalRecoveryType
    <2> QED BY <2>3, <2>4, <2>7, <2>9, <2>10
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
  <1> QED BY <1>1

THEOREM CausalTailUpdatePreservesCausalType ==
  \A node \in ValidatorIds:
    /\ AsyncCausalTypeInvariant
    /\ CausalQueueNonempty(node)
    /\ asyncCausalQueues' =
         [asyncCausalQueues EXCEPT ![node] = Tail(@)]
    => AsyncCausalTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncCausalTypeInvariant,
                CausalQueueNonempty(node),
                asyncCausalQueues' =
                  [asyncCausalQueues EXCEPT ![node] = Tail(@)]
         PROVE AsyncCausalTypeInvariant'
    <2>1. /\ DOMAIN asyncCausalQueues = ValidatorIds
           /\ AsyncQueueTyped(asyncCausalQueues[node])
           /\ AsyncCausalQueueOwnership(node, asyncCausalQueues[node])
           /\ Len(asyncCausalQueues[node]) > 0
      BY <1>1 DEF AsyncCausalTypeInvariant, CausalQueueNonempty
    <2>2. /\ AsyncQueueTyped(Tail(asyncCausalQueues[node]))
           /\ SequenceSet(Tail(asyncCausalQueues[node]))
                \subseteq SequenceSet(asyncCausalQueues[node])
      BY <2>1, TypedQueueTailFacts
    <2>3. AsyncCausalQueueOwnership(
             node, Tail(asyncCausalQueues[node]))
      BY <2>1, <2>2 DEF AsyncCausalQueueOwnership
    <2>4. DOMAIN asyncCausalQueues' = ValidatorIds
      BY <1>1, <2>1, Isa
    <2>5. \A other \in ValidatorIds:
             /\ AsyncQueueTyped(asyncCausalQueues'[other])
             /\ AsyncCausalQueueOwnership(
                  other, asyncCausalQueues'[other])
      <3>1. ASSUME NEW other \in ValidatorIds
             PROVE /\ AsyncQueueTyped(asyncCausalQueues'[other])
                   /\ AsyncCausalQueueOwnership(
                        other, asyncCausalQueues'[other])
        <4>1. CASE other = node
          <5>1. asyncCausalQueues'[other] =
                   Tail(asyncCausalQueues[node])
            BY <1>1, <2>1, <4>1, FunctionalTailUpdateAtKey
          <5> QED BY <2>2, <2>3, <4>1, <5>1
        <4>2. CASE other # node
          <5>1. asyncCausalQueues'[other] = asyncCausalQueues[other]
            BY <1>1, <2>1, <3>1, <4>2,
               FunctionalTailUpdateAwayFromKey
          <5>2. /\ AsyncQueueTyped(asyncCausalQueues[other])
                 /\ AsyncCausalQueueOwnership(
                      other, asyncCausalQueues[other])
            BY <1>1, <3>1 DEF AsyncCausalTypeInvariant
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>4, <2>5 DEF AsyncCausalTypeInvariant
  <1> QED BY <1>1

THEOREM CausalHeadCandidateIsTyped ==
  \A node \in ValidatorIds:
    AsyncCausalTypeInvariant /\ CausalQueueNonempty(node)
      => AsyncCandidateTyped(HeadCausalCandidate(node))
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncCausalTypeInvariant,
                CausalQueueNonempty(node)
         PROVE AsyncCandidateTyped(HeadCausalCandidate(node))
    <2>1. /\ AsyncQueueTyped(asyncCausalQueues[node])
           /\ Len(asyncCausalQueues[node]) > 0
      BY <1>1 DEF AsyncCausalTypeInvariant, CausalQueueNonempty
    <2>2. /\ 1 \in 1..Len(asyncCausalQueues[node])
           /\ Head(asyncCausalQueues[node]) = asyncCausalQueues[node][1]
      BY <2>1, NonemptySequenceHeadIsFirst, SMT
         DEF AsyncQueueTyped
    <2> QED BY <2>1, <2>2
         DEF AsyncQueueTyped, HeadCausalCandidate
  <1> QED BY <1>1

THEOREM CausalHeadCandidateIsOwned ==
  \A node \in ValidatorIds:
    AsyncCausalTypeInvariant /\ CausalQueueNonempty(node)
      => HeadCausalCandidate(node).node = node
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncCausalTypeInvariant,
                CausalQueueNonempty(node)
         PROVE HeadCausalCandidate(node).node = node
    <2>1. /\ AsyncQueueTyped(asyncCausalQueues[node])
           /\ AsyncCausalQueueOwnership(node, asyncCausalQueues[node])
           /\ Len(asyncCausalQueues[node]) > 0
      BY <1>1 DEF AsyncCausalTypeInvariant, CausalQueueNonempty
    <2>2. /\ 1 \in 1..Len(asyncCausalQueues[node])
           /\ Head(asyncCausalQueues[node]) = asyncCausalQueues[node][1]
      BY <2>1, NonemptySequenceHeadIsFirst, SMT
         DEF AsyncQueueTyped
    <2>3. HeadCausalCandidate(node)
             \in SequenceSet(asyncCausalQueues[node])
      BY <2>2 DEF HeadCausalCandidate, SequenceSet
    <2> QED BY <2>1, <2>3 DEF AsyncCausalQueueOwnership
  <1> QED BY <1>1

THEOREM CausalUntrackedCandidateFacts ==
  \A node \in ValidatorIds:
    \A candidate:
      ~CandidateInFlight(candidate)
        => /\ candidate \notin asyncOutstandingWork[node]
           /\ candidate \notin QueuedCandidates
           /\ candidate \notin DeferredCandidates
BY SMTT(30)
   DEF CandidateInFlight, TrackedWorkCandidates

THEOREM ConsensusIndicesAfterConsensusAppend ==
  \A queue, job:
    /\ AsyncIoSequenceTyped(queue)
    /\ job.class = "Consensus"
    => AsyncIoConsensusIndices(Append(queue, job)) =
         AsyncIoConsensusIndices(queue) \cup {Len(queue) + 1}
PROOF
  <1>1. ASSUME NEW queue, NEW job,
                AsyncIoSequenceTyped(queue),
                job.class = "Consensus"
         PROVE AsyncIoConsensusIndices(Append(queue, job)) =
                 AsyncIoConsensusIndices(queue) \cup {Len(queue) + 1}
    <2>1. queue \in Seq(Range(queue))
      BY <1>1 DEF AsyncIoSequenceTyped
    <2>2. /\ Len(queue) \in Nat
           /\ Len(Append(queue, job)) = Len(queue) + 1
           /\ \A index \in 1..Len(queue):
                Append(queue, job)[index] = queue[index]
           /\ Append(queue, job)[Len(queue) + 1] = job
      BY <2>1, AppendSequenceFacts, LenProperties
    <2> QED BY <1>1, <2>2, SMT DEF AsyncIoConsensusIndices
  <1> QED BY <1>1

THEOREM AppendFreshConsensusJobPreservesQueueFacts ==
  \A queue, outstanding, ioReadyQueue, localReadyQueue, candidate:
    /\ AsyncConfiguration
    /\ AsyncIoSequenceTyped(queue)
    /\ AsyncIoServeNonceOwnership(queue)
    /\ (\A job \in SequenceSet(queue):
          job.class = "Consensus" => job.candidate \in outstanding)
    /\ AsyncIoConsensusQueueOwnership(
         queue, ioReadyQueue, localReadyQueue)
    /\ AsyncCandidateTyped(candidate)
    /\ candidate.class = "Completion"
    /\ candidate \notin outstanding
    /\ candidate \notin SequenceSet(ioReadyQueue)
    /\ candidate \notin SequenceSet(localReadyQueue)
    => /\ AsyncIoSequenceTyped(
             Append(queue, AsyncIoConsensusJob(candidate)))
       /\ AsyncIoServeNonceOwnership(
            Append(queue, AsyncIoConsensusJob(candidate)))
       /\ (\A job \in
                  SequenceSet(
                    Append(queue, AsyncIoConsensusJob(candidate))):
             job.class = "Consensus" =>
               job.candidate \in outstanding \cup {candidate})
       /\ AsyncIoConsensusQueueOwnership(
            Append(queue, AsyncIoConsensusJob(candidate)),
            ioReadyQueue, localReadyQueue)
PROOF
  <1>1. ASSUME NEW queue, NEW outstanding,
                NEW ioReadyQueue, NEW localReadyQueue, NEW candidate,
                AsyncConfiguration,
                AsyncIoSequenceTyped(queue),
                AsyncIoServeNonceOwnership(queue),
                (\A job \in SequenceSet(queue):
                   job.class = "Consensus" =>
                     job.candidate \in outstanding),
                AsyncIoConsensusQueueOwnership(
                  queue, ioReadyQueue, localReadyQueue),
                AsyncCandidateTyped(candidate),
                candidate.class = "Completion",
                candidate \notin outstanding,
                candidate \notin SequenceSet(ioReadyQueue),
                candidate \notin SequenceSet(localReadyQueue)
         PROVE /\ AsyncIoSequenceTyped(
                      Append(queue, AsyncIoConsensusJob(candidate)))
                /\ AsyncIoServeNonceOwnership(
                     Append(queue, AsyncIoConsensusJob(candidate)))
                /\ (\A job \in
                           SequenceSet(
                             Append(queue, AsyncIoConsensusJob(candidate))):
                      job.class = "Consensus" =>
                        job.candidate \in outstanding \cup {candidate})
                /\ AsyncIoConsensusQueueOwnership(
                     Append(queue, AsyncIoConsensusJob(candidate)),
                     ioReadyQueue, localReadyQueue)
    <2> DEFINE NewJob == AsyncIoConsensusJob(candidate)
    <2>1. /\ AsyncIoJobTyped(NewJob)
           /\ NewJob.class = "Consensus"
           /\ NewJob.candidate = candidate
      BY <1>1, TypedCompletionCandidateMakesConsensusJob, SMT
         DEF NewJob, AsyncIoConsensusJob, AsyncIoJob
    <2>2. /\ queue \in Seq(Range(queue))
           /\ SequenceSet(Append(queue, NewJob)) =
                SequenceSet(queue) \cup {NewJob}
           /\ AsyncIoConsensusIndices(Append(queue, NewJob)) =
                AsyncIoConsensusIndices(queue) \cup {Len(queue) + 1}
           /\ (\A index \in 1..Len(queue):
                 Append(queue, NewJob)[index] = queue[index])
           /\ Append(queue, NewJob)[Len(queue) + 1] = NewJob
      BY <1>1, <2>1, SequenceSetAfterAppend,
         ConsensusIndicesAfterConsensusAppend, AppendSequenceFacts
         DEF AsyncIoSequenceTyped
    <2>3. AsyncIoSequenceTyped(Append(queue, NewJob))
      BY <1>1, <2>1, TypedIoAppendPreservesSequenceType
    <2>3s. AsyncIoServeNonceOwnership(Append(queue, NewJob))
      BY <1>1, <2>1, AppendNonServeJobPreservesNonceOwnership
    <2>4. \A job \in SequenceSet(Append(queue, NewJob)):
             job.class = "Consensus" =>
               job.candidate \in outstanding \cup {candidate}
      <3>1. ASSUME NEW job \in SequenceSet(Append(queue, NewJob)),
                    job.class = "Consensus"
             PROVE job.candidate \in outstanding \cup {candidate}
        <4>1. job \in SequenceSet(queue) \cup {NewJob}
          BY <2>2, <3>1
        <4>2. CASE job = NewJob
          BY <2>1, <4>2
        <4>3. CASE job # NewJob
          <5>1. job \in SequenceSet(queue)
            BY <4>1, <4>3
          <5>2. job.candidate \in outstanding
            BY <1>1, <3>1, <5>1
          <5> QED BY <5>2
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>5. /\ (\A index \in AsyncIoConsensusIndices(queue):
                    /\ queue[index].candidate
                         \notin SequenceSet(ioReadyQueue)
                    /\ queue[index].candidate
                         \notin SequenceSet(localReadyQueue))
           /\ (\A left, right \in AsyncIoConsensusIndices(queue):
                 queue[left].candidate = queue[right].candidate =>
                   left = right)
      BY <1>1 DEF AsyncIoConsensusQueueOwnership
    <2>6. \A index \in AsyncIoConsensusIndices(Append(queue, NewJob)):
             \/ /\ index \in AsyncIoConsensusIndices(queue)
                   /\ queue[index] \in SequenceSet(queue)
                   /\ queue[index].class = "Consensus"
                   /\ Append(queue, NewJob)[index] = queue[index]
             \/ /\ index = Len(queue) + 1
                   /\ Append(queue, NewJob)[index] = NewJob
      <3>1. ASSUME NEW index \in
                      AsyncIoConsensusIndices(Append(queue, NewJob))
             PROVE \/ /\ index \in AsyncIoConsensusIndices(queue)
                          /\ queue[index] \in SequenceSet(queue)
                          /\ queue[index].class = "Consensus"
                          /\ Append(queue, NewJob)[index] = queue[index]
                    \/ /\ index = Len(queue) + 1
                          /\ Append(queue, NewJob)[index] = NewJob
        BY <2>2, <3>1, SMT
           DEF AsyncIoConsensusIndices, SequenceSet
      <3> QED BY <3>1
    <2>7. \A index \in AsyncIoConsensusIndices(Append(queue, NewJob)):
             /\ Append(queue, NewJob)[index].candidate
                  \notin SequenceSet(ioReadyQueue)
             /\ Append(queue, NewJob)[index].candidate
                  \notin SequenceSet(localReadyQueue)
      BY <1>1, <2>1, <2>5, <2>6, SMTT(20)
    <2>8. \A left, right \in
                    AsyncIoConsensusIndices(Append(queue, NewJob)):
             Append(queue, NewJob)[left].candidate =
               Append(queue, NewJob)[right].candidate => left = right
      BY <1>1, <2>1, <2>5, <2>6, SMTT(30)
    <2>9. AsyncIoConsensusQueueOwnership(
             Append(queue, NewJob), ioReadyQueue, localReadyQueue)
      BY <2>7, <2>8 DEF AsyncIoConsensusQueueOwnership
    <2> QED BY <2>3, <2>3s, <2>4, <2>9 DEF NewJob
  <1> QED BY <1>1

THEOREM AddFreshCompletionPreservesNodeWorkFacts ==
  \A node, commandQueue, outstanding, ioReadyQueue, localReadyQueue,
     candidate:
    /\ IsFiniteSet(outstanding)
    /\ (\A work \in outstanding:
          /\ AsyncCandidateTyped(work)
          /\ work.class = "Completion"
          /\ work.node = node)
    /\ AsyncCompletionSequenceTyped(ioReadyQueue)
    /\ AsyncCompletionSequenceTyped(localReadyQueue)
    /\ Len(ioReadyQueue) = Cardinality(SequenceSet(ioReadyQueue))
    /\ Len(localReadyQueue) = Cardinality(SequenceSet(localReadyQueue))
    /\ SequenceSet(ioReadyQueue) \subseteq outstanding
    /\ SequenceSet(localReadyQueue) \subseteq outstanding
    /\ SequenceSet(ioReadyQueue) \cap SequenceSet(localReadyQueue) = {}
    /\ SequenceSet(commandQueue) \cap outstanding = {}
    /\ AsyncCandidateTyped(candidate)
    /\ candidate.class = "Completion"
    /\ candidate.node = node
    /\ candidate \notin outstanding
    /\ candidate \notin SequenceSet(commandQueue)
    => /\ IsFiniteSet(outstanding \cup {candidate})
       /\ (\A work \in outstanding \cup {candidate}:
             /\ AsyncCandidateTyped(work)
             /\ work.class = "Completion"
             /\ work.node = node)
       /\ AsyncCompletionSequenceTyped(ioReadyQueue)
       /\ AsyncCompletionSequenceTyped(localReadyQueue)
       /\ Len(ioReadyQueue) = Cardinality(SequenceSet(ioReadyQueue))
       /\ Len(localReadyQueue) = Cardinality(SequenceSet(localReadyQueue))
       /\ SequenceSet(ioReadyQueue) \subseteq outstanding \cup {candidate}
       /\ SequenceSet(localReadyQueue) \subseteq outstanding \cup {candidate}
       /\ SequenceSet(ioReadyQueue) \cap SequenceSet(localReadyQueue) = {}
       /\ SequenceSet(commandQueue) \cap (outstanding \cup {candidate}) = {}
BY FS_AddElement, SMTT(30)

THEOREM AppendFreshLocalCompletionPreservesNodeWorkFacts ==
  \A node, commandQueue, outstanding, ioReadyQueue, localReadyQueue,
     candidate:
    /\ IsFiniteSet(outstanding)
    /\ (\A work \in outstanding:
          /\ AsyncCandidateTyped(work)
          /\ work.class = "Completion"
          /\ work.node = node)
    /\ AsyncCompletionSequenceTyped(ioReadyQueue)
    /\ AsyncCompletionSequenceTyped(localReadyQueue)
    /\ Len(ioReadyQueue) = Cardinality(SequenceSet(ioReadyQueue))
    /\ Len(localReadyQueue) = Cardinality(SequenceSet(localReadyQueue))
    /\ SequenceSet(ioReadyQueue) \subseteq outstanding
    /\ SequenceSet(localReadyQueue) \subseteq outstanding
    /\ SequenceSet(ioReadyQueue) \cap SequenceSet(localReadyQueue) = {}
    /\ SequenceSet(commandQueue) \cap outstanding = {}
    /\ AsyncCandidateTyped(candidate)
    /\ candidate.class = "Completion"
    /\ candidate.node = node
    /\ candidate \notin outstanding
    /\ candidate \notin SequenceSet(commandQueue)
    => /\ IsFiniteSet(outstanding \cup {candidate})
       /\ (\A work \in outstanding \cup {candidate}:
             /\ AsyncCandidateTyped(work)
             /\ work.class = "Completion"
             /\ work.node = node)
       /\ AsyncCompletionSequenceTyped(ioReadyQueue)
       /\ AsyncCompletionSequenceTyped(Append(localReadyQueue, candidate))
       /\ Len(ioReadyQueue) = Cardinality(SequenceSet(ioReadyQueue))
       /\ Len(Append(localReadyQueue, candidate)) =
            Cardinality(SequenceSet(Append(localReadyQueue, candidate)))
       /\ SequenceSet(ioReadyQueue) \subseteq outstanding \cup {candidate}
       /\ SequenceSet(Append(localReadyQueue, candidate))
            \subseteq outstanding \cup {candidate}
       /\ SequenceSet(ioReadyQueue) \cap
            SequenceSet(Append(localReadyQueue, candidate)) = {}
       /\ SequenceSet(commandQueue) \cap
            (outstanding \cup {candidate}) = {}
PROOF
  <1>1. ASSUME NEW node, NEW commandQueue, NEW outstanding,
                NEW ioReadyQueue, NEW localReadyQueue, NEW candidate,
                IsFiniteSet(outstanding),
                \A work \in outstanding:
                  /\ AsyncCandidateTyped(work)
                  /\ work.class = "Completion"
                  /\ work.node = node,
                AsyncCompletionSequenceTyped(ioReadyQueue),
                AsyncCompletionSequenceTyped(localReadyQueue),
                Len(ioReadyQueue) =
                  Cardinality(SequenceSet(ioReadyQueue)),
                Len(localReadyQueue) =
                  Cardinality(SequenceSet(localReadyQueue)),
                SequenceSet(ioReadyQueue) \subseteq outstanding,
                SequenceSet(localReadyQueue) \subseteq outstanding,
                SequenceSet(ioReadyQueue) \cap
                  SequenceSet(localReadyQueue) = {},
                SequenceSet(commandQueue) \cap outstanding = {},
                AsyncCandidateTyped(candidate),
                candidate.class = "Completion",
                candidate.node = node,
                candidate \notin outstanding,
                candidate \notin SequenceSet(commandQueue)
         PROVE /\ IsFiniteSet(outstanding \cup {candidate})
               /\ (\A work \in outstanding \cup {candidate}:
                     /\ AsyncCandidateTyped(work)
                     /\ work.class = "Completion"
                     /\ work.node = node)
               /\ AsyncCompletionSequenceTyped(ioReadyQueue)
               /\ AsyncCompletionSequenceTyped(
                    Append(localReadyQueue, candidate))
               /\ Len(ioReadyQueue) =
                    Cardinality(SequenceSet(ioReadyQueue))
               /\ Len(Append(localReadyQueue, candidate)) =
                    Cardinality(
                      SequenceSet(Append(localReadyQueue, candidate)))
               /\ SequenceSet(ioReadyQueue)
                    \subseteq outstanding \cup {candidate}
               /\ SequenceSet(Append(localReadyQueue, candidate))
                    \subseteq outstanding \cup {candidate}
               /\ SequenceSet(ioReadyQueue) \cap
                    SequenceSet(Append(localReadyQueue, candidate)) = {}
               /\ SequenceSet(commandQueue) \cap
                    (outstanding \cup {candidate}) = {}
    <2>1. /\ IsFiniteSet(outstanding \cup {candidate})
           /\ (\A work \in outstanding \cup {candidate}:
                 /\ AsyncCandidateTyped(work)
                 /\ work.class = "Completion"
                 /\ work.node = node)
           /\ SequenceSet(ioReadyQueue)
                \subseteq outstanding \cup {candidate}
           /\ SequenceSet(localReadyQueue)
                \subseteq outstanding \cup {candidate}
           /\ SequenceSet(commandQueue) \cap
                (outstanding \cup {candidate}) = {}
      BY <1>1, AddFreshCompletionPreservesNodeWorkFacts
    <2>2. /\ localReadyQueue \in Seq(Range(localReadyQueue))
           /\ candidate \notin SequenceSet(localReadyQueue)
           /\ candidate \notin SequenceSet(ioReadyQueue)
      BY <1>1, SMT DEF AsyncCompletionSequenceTyped
    <2>3. /\ SequenceSet(Append(localReadyQueue, candidate)) =
                    SequenceSet(localReadyQueue) \cup {candidate}
           /\ Len(Append(localReadyQueue, candidate)) =
                    Len(localReadyQueue) + 1
           /\ AsyncCompletionSequenceTyped(
                    Append(localReadyQueue, candidate))
      BY <1>1, <2>2, SequenceSetAfterAppend, AppendSequenceFacts,
         TypedCompletionAppendPreservesSequenceType
    <2>4. IsFiniteSet(SequenceSet(localReadyQueue))
      BY <1>1, FS_Subset
    <2>5. Cardinality(
               SequenceSet(Append(localReadyQueue, candidate))) =
             Cardinality(SequenceSet(localReadyQueue)) + 1
      BY <2>2, <2>3, <2>4, FS_AddElement
    <2>6. /\ Len(Append(localReadyQueue, candidate)) =
                    Cardinality(
                      SequenceSet(Append(localReadyQueue, candidate)))
           /\ SequenceSet(Append(localReadyQueue, candidate))
                \subseteq outstanding \cup {candidate}
           /\ SequenceSet(ioReadyQueue) \cap
                SequenceSet(Append(localReadyQueue, candidate)) = {}
      BY <1>1, <2>2, <2>3, <2>5, SMT
    <2> QED BY <1>1, <2>1, <2>3, <2>6
  <1> QED BY <1>1

THEOREM AppendFreshLocalReadyPreservesConsensusOwnership ==
  \A queue, outstanding, ioReadyQueue, localReadyQueue, candidate:
    /\ AsyncIoSequenceTyped(queue)
    /\ (\A job \in SequenceSet(queue):
          job.class = "Consensus" => job.candidate \in outstanding)
    /\ AsyncIoConsensusQueueOwnership(
         queue, ioReadyQueue, localReadyQueue)
    /\ localReadyQueue \in Seq(Range(localReadyQueue))
    /\ candidate \notin outstanding
    => AsyncIoConsensusQueueOwnership(
         queue, ioReadyQueue, Append(localReadyQueue, candidate))
PROOF
  <1>1. ASSUME NEW queue, NEW outstanding, NEW ioReadyQueue,
                NEW localReadyQueue, NEW candidate,
                AsyncIoSequenceTyped(queue),
                \A job \in SequenceSet(queue):
                  job.class = "Consensus" => job.candidate \in outstanding,
                AsyncIoConsensusQueueOwnership(
                  queue, ioReadyQueue, localReadyQueue),
                localReadyQueue \in Seq(Range(localReadyQueue)),
                candidate \notin outstanding
         PROVE AsyncIoConsensusQueueOwnership(
                 queue, ioReadyQueue,
                 Append(localReadyQueue, candidate))
    <2>1. SequenceSet(Append(localReadyQueue, candidate)) =
             SequenceSet(localReadyQueue) \cup {candidate}
      BY <1>1, SequenceSetAfterAppend
    <2>2. /\ (\A index \in AsyncIoConsensusIndices(queue):
                    /\ queue[index].candidate
                         \notin SequenceSet(ioReadyQueue)
                    /\ queue[index].candidate
                         \notin SequenceSet(localReadyQueue))
           /\ (\A left, right \in AsyncIoConsensusIndices(queue):
                 queue[left].candidate = queue[right].candidate =>
                   left = right)
      BY <1>1 DEF AsyncIoConsensusQueueOwnership
    <2>3. \A index \in AsyncIoConsensusIndices(queue):
             queue[index] \in SequenceSet(queue)
               /\ queue[index].class = "Consensus"
      BY <1>1, SMT
         DEF AsyncIoConsensusIndices, AsyncIoSequenceTyped, SequenceSet
    <2>4. \A index \in AsyncIoConsensusIndices(queue):
             /\ queue[index].candidate \notin SequenceSet(ioReadyQueue)
             /\ queue[index].candidate
                  \notin SequenceSet(Append(localReadyQueue, candidate))
      BY <1>1, <2>1, <2>2, <2>3, SMT
    <2> QED BY <2>2, <2>4 DEF AsyncIoConsensusQueueOwnership
  <1> QED BY <1>1

THEOREM AdmitFreshLocalCompletionPreservesIoType ==
  \A node \in ValidatorIds:
    \A candidate:
      /\ AsyncTypeInvariant
      /\ AsyncCandidateTyped(candidate)
      /\ candidate.class = "Completion"
      /\ candidate.node = node
      /\ ~CandidateInFlight(candidate)
      /\ AsyncOutstandingWorkCount(node) < AsyncIoWorkCapacity
      /\ asyncLocalReadyCompletions' =
           [asyncLocalReadyCompletions EXCEPT
              ![node] = Append(@, candidate)]
      /\ asyncOutstandingWork' =
           [asyncOutstandingWork EXCEPT ![node] = @ \cup {candidate}]
      /\ UNCHANGED <<asyncCommandQueues, asyncIoQueues,
                      asyncIoReadyCompletions, asyncNextCompletionSource,
                      asyncIoControlAvailable,
                      asyncDeferredCompletionQueues>>
      => AsyncIoTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW candidate,
                AsyncTypeInvariant,
                AsyncCandidateTyped(candidate),
                candidate.class = "Completion",
                candidate.node = node,
                ~CandidateInFlight(candidate),
                AsyncOutstandingWorkCount(node) < AsyncIoWorkCapacity,
                asyncLocalReadyCompletions' =
                  [asyncLocalReadyCompletions EXCEPT
                     ![node] = Append(@, candidate)],
                asyncOutstandingWork' =
                  [asyncOutstandingWork EXCEPT
                     ![node] = @ \cup {candidate}],
                UNCHANGED <<asyncCommandQueues, asyncIoQueues,
                            asyncIoReadyCompletions,
                            asyncNextCompletionSource,
                            asyncIoControlAvailable,
                            asyncDeferredCompletionQueues>>
         PROVE AsyncIoTypeInvariant'
    <2>1. /\ AsyncConfiguration
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>2. /\ candidate \notin asyncOutstandingWork[node]
           /\ candidate \notin SequenceSet(asyncCommandQueues[node])
      BY <1>1, CausalUntrackedCandidateFacts, SMT
         DEF QueuedCandidates
    <2>3. AsyncIoTopologyTypeInvariant'
      BY <1>1, <2>1, Isa DEF AsyncIoTopologyTypeInvariant
    <2>4. AsyncIoQueueContentTypeInvariant'
      <3>1. /\ asyncIoQueues' = asyncIoQueues
             /\ asyncIoReadyCompletions' = asyncIoReadyCompletions
        BY <1>1
      <3>2. /\ asyncOutstandingWork'[node] =
                    asyncOutstandingWork[node] \cup {candidate}
             /\ asyncLocalReadyCompletions'[node] =
                    Append(asyncLocalReadyCompletions[node], candidate)
        BY <1>1, <2>1, Isa DEF AsyncIoTopologyTypeInvariant
      <3>3. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
             /\ AsyncIoServeNonceOwnership(asyncIoQueues[node])
             /\ (\A job \in SequenceSet(asyncIoQueues[node]):
                   job.class = "Consensus" =>
                     job.candidate \in asyncOutstandingWork[node])
             /\ AsyncIoConsensusQueueOwnership(
                  asyncIoQueues[node], asyncIoReadyCompletions[node],
                  asyncLocalReadyCompletions[node])
             /\ asyncLocalReadyCompletions[node]
                  \in Seq(Range(asyncLocalReadyCompletions[node]))
        BY <2>1
           DEF AsyncIoQueueContentTypeInvariant,
               AsyncIoWorkContentTypeInvariant,
               AsyncIoConsensusCandidateOwnership,
               AsyncCompletionSequenceTyped
      <3>4. AsyncIoConsensusQueueOwnership(
               asyncIoQueues[node], asyncIoReadyCompletions[node],
               Append(asyncLocalReadyCompletions[node], candidate))
        BY <1>1, <2>2, <3>3,
           AppendFreshLocalReadyPreservesConsensusOwnership
      <3>5. /\ AsyncIoSequenceTyped(asyncIoQueues'[node])
             /\ AsyncIoServeNonceOwnership(asyncIoQueues'[node])
             /\ (\A job \in SequenceSet(asyncIoQueues'[node]):
                   job.class = "Consensus" =>
                     job.candidate \in asyncOutstandingWork'[node])
             /\ AsyncIoConsensusCandidateOwnership(
                  node, asyncIoQueues', asyncIoReadyCompletions',
                  asyncLocalReadyCompletions')
        BY <3>1, <3>2, <3>3, <3>4, Isa
           DEF AsyncIoConsensusCandidateOwnership
      <3>6. \A other \in ValidatorIds:
               /\ AsyncIoSequenceTyped(asyncIoQueues'[other])
               /\ AsyncIoServeNonceOwnership(asyncIoQueues'[other])
               /\ (\A job \in SequenceSet(asyncIoQueues'[other]):
                     job.class = "Consensus" =>
                       job.candidate \in asyncOutstandingWork'[other])
               /\ AsyncIoConsensusCandidateOwnership(
                    other, asyncIoQueues', asyncIoReadyCompletions',
                    asyncLocalReadyCompletions')
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ AsyncIoSequenceTyped(asyncIoQueues'[other])
                     /\ AsyncIoServeNonceOwnership(asyncIoQueues'[other])
                     /\ (\A job \in SequenceSet(asyncIoQueues'[other]):
                           job.class = "Consensus" =>
                             job.candidate \in asyncOutstandingWork'[other])
                     /\ AsyncIoConsensusCandidateOwnership(
                          other, asyncIoQueues',
                          asyncIoReadyCompletions',
                          asyncLocalReadyCompletions')
          <5>1. CASE other = node
            BY <3>5, <5>1
          <5>2. CASE other # node
            <6>1. /\ asyncOutstandingWork'[other] =
                           asyncOutstandingWork[other]
                   /\ asyncLocalReadyCompletions'[other] =
                           asyncLocalReadyCompletions[other]
              BY <1>1, <2>1, <4>1, <5>2,
                 FunctionalUpdateAwayFromKey
                 DEF AsyncIoTopologyTypeInvariant
            <6> QED BY <2>1, <3>1, <4>1, <6>1, Isa
                 DEF AsyncIoQueueContentTypeInvariant,
                     AsyncIoConsensusCandidateOwnership
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>6 DEF AsyncIoQueueContentTypeInvariant
    <2>5. AsyncIoWorkContentTypeInvariant'
      <3>1. /\ asyncOutstandingWork'[node] =
                    asyncOutstandingWork[node] \cup {candidate}
             /\ asyncLocalReadyCompletions'[node] =
                    Append(asyncLocalReadyCompletions[node], candidate)
             /\ asyncIoReadyCompletions' = asyncIoReadyCompletions
             /\ asyncCommandQueues' = asyncCommandQueues
        BY <1>1, <2>1, Isa DEF AsyncIoTopologyTypeInvariant
      <3>2. /\ IsFiniteSet(asyncOutstandingWork[node])
             /\ (\A work \in asyncOutstandingWork[node]:
                   /\ AsyncCandidateTyped(work)
                   /\ work.class = "Completion"
                   /\ work.node = node)
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions[node])
             /\ Len(asyncIoReadyCompletions[node]) =
                  Cardinality(
                    SequenceSet(asyncIoReadyCompletions[node]))
             /\ Len(asyncLocalReadyCompletions[node]) =
                  Cardinality(
                    SequenceSet(asyncLocalReadyCompletions[node]))
             /\ SequenceSet(asyncIoReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncLocalReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions[node]) = {}
             /\ SequenceSet(asyncCommandQueues[node]) \cap
                  asyncOutstandingWork[node] = {}
        BY <2>1 DEF AsyncIoWorkContentTypeInvariant
      <3>3. /\ IsFiniteSet(asyncOutstandingWork'[node])
             /\ (\A work \in asyncOutstandingWork'[node]:
                   /\ AsyncCandidateTyped(work)
                   /\ work.class = "Completion"
                   /\ work.node = node)
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions'[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions'[node])
             /\ Len(asyncIoReadyCompletions'[node]) =
                  Cardinality(
                    SequenceSet(asyncIoReadyCompletions'[node]))
             /\ Len(asyncLocalReadyCompletions'[node]) =
                  Cardinality(
                    SequenceSet(asyncLocalReadyCompletions'[node]))
             /\ SequenceSet(asyncIoReadyCompletions'[node])
                  \subseteq asyncOutstandingWork'[node]
             /\ SequenceSet(asyncLocalReadyCompletions'[node])
                  \subseteq asyncOutstandingWork'[node]
             /\ SequenceSet(asyncIoReadyCompletions'[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions'[node]) = {}
             /\ SequenceSet(asyncCommandQueues'[node]) \cap
                  asyncOutstandingWork'[node] = {}
        BY <1>1, <2>2, <3>1, <3>2,
           AppendFreshLocalCompletionPreservesNodeWorkFacts
      <3>4. \A other \in ValidatorIds:
               /\ IsFiniteSet(asyncOutstandingWork'[other])
               /\ (\A work \in asyncOutstandingWork'[other]:
                     /\ AsyncCandidateTyped(work)
                     /\ work.class = "Completion"
                     /\ work.node = other)
               /\ AsyncCompletionSequenceTyped(
                    asyncIoReadyCompletions'[other])
               /\ AsyncCompletionSequenceTyped(
                    asyncLocalReadyCompletions'[other])
               /\ Len(asyncIoReadyCompletions'[other]) =
                    Cardinality(
                      SequenceSet(asyncIoReadyCompletions'[other]))
               /\ Len(asyncLocalReadyCompletions'[other]) =
                    Cardinality(
                      SequenceSet(asyncLocalReadyCompletions'[other]))
               /\ SequenceSet(asyncIoReadyCompletions'[other])
                    \subseteq asyncOutstandingWork'[other]
               /\ SequenceSet(asyncLocalReadyCompletions'[other])
                    \subseteq asyncOutstandingWork'[other]
               /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                    SequenceSet(asyncLocalReadyCompletions'[other]) = {}
               /\ SequenceSet(asyncCommandQueues'[other]) \cap
                    asyncOutstandingWork'[other] = {}
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ IsFiniteSet(asyncOutstandingWork'[other])
                     /\ (\A work \in asyncOutstandingWork'[other]:
                           /\ AsyncCandidateTyped(work)
                           /\ work.class = "Completion"
                           /\ work.node = other)
                     /\ AsyncCompletionSequenceTyped(
                          asyncIoReadyCompletions'[other])
                     /\ AsyncCompletionSequenceTyped(
                          asyncLocalReadyCompletions'[other])
                     /\ Len(asyncIoReadyCompletions'[other]) =
                          Cardinality(
                            SequenceSet(asyncIoReadyCompletions'[other]))
                     /\ Len(asyncLocalReadyCompletions'[other]) =
                          Cardinality(
                            SequenceSet(asyncLocalReadyCompletions'[other]))
                     /\ SequenceSet(asyncIoReadyCompletions'[other])
                          \subseteq asyncOutstandingWork'[other]
                     /\ SequenceSet(asyncLocalReadyCompletions'[other])
                          \subseteq asyncOutstandingWork'[other]
                     /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                          SequenceSet(asyncLocalReadyCompletions'[other]) = {}
                     /\ SequenceSet(asyncCommandQueues'[other]) \cap
                          asyncOutstandingWork'[other] = {}
          <5>1. CASE other = node
            BY <3>3, <5>1
          <5>2. CASE other # node
            <6>1. /\ asyncOutstandingWork'[other] =
                           asyncOutstandingWork[other]
                   /\ asyncLocalReadyCompletions'[other] =
                           asyncLocalReadyCompletions[other]
              BY <1>1, <2>1, <4>1, <5>2,
                 FunctionalUpdateAwayFromKey
                 DEF AsyncIoTopologyTypeInvariant
            <6>2. /\ IsFiniteSet(asyncOutstandingWork[other])
                   /\ (\A work \in asyncOutstandingWork[other]:
                         /\ AsyncCandidateTyped(work)
                         /\ work.class = "Completion"
                         /\ work.node = other)
                   /\ AsyncCompletionSequenceTyped(
                        asyncIoReadyCompletions[other])
                   /\ AsyncCompletionSequenceTyped(
                        asyncLocalReadyCompletions[other])
                   /\ Len(asyncIoReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncIoReadyCompletions[other]))
                   /\ Len(asyncLocalReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncLocalReadyCompletions[other]))
                   /\ SequenceSet(asyncIoReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncLocalReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncIoReadyCompletions[other]) \cap
                        SequenceSet(asyncLocalReadyCompletions[other]) = {}
                   /\ SequenceSet(asyncCommandQueues[other]) \cap
                        asyncOutstandingWork[other] = {}
              BY <2>1, <4>1 DEF AsyncIoWorkContentTypeInvariant
            <6>3. /\ asyncIoReadyCompletions'[other] =
                           asyncIoReadyCompletions[other]
                   /\ asyncCommandQueues'[other] =
                           asyncCommandQueues[other]
              BY <3>1
            <6> QED BY <6>1, <6>2, <6>3
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>4 DEF AsyncIoWorkContentTypeInvariant
    <2>6. AsyncIoCapacityTypeInvariant'
      <3>1. /\ IsFiniteSet(asyncOutstandingWork[node])
             /\ candidate \notin asyncOutstandingWork[node]
             /\ AsyncOutstandingWorkCount(node) \in Nat
        BY <2>1, <2>2, FS_CardinalityType
           DEF AsyncIoWorkContentTypeInvariant,
               AsyncOutstandingWorkCount
      <3>2. Cardinality(
                   asyncOutstandingWork[node] \cup {candidate}) =
                 Cardinality(asyncOutstandingWork[node]) + 1
        BY <3>1, FS_AddElement
      <3>3. /\ asyncCommandQueues' = asyncCommandQueues
             /\ asyncIoQueues' = asyncIoQueues
             /\ asyncOutstandingWork'[node] =
                    asyncOutstandingWork[node] \cup {candidate}
             /\ asyncDeferredCompletionQueues' =
                    asyncDeferredCompletionQueues
        BY <1>1, <2>1, Isa
           DEF AsyncIoTopologyTypeInvariant
      <3>4. /\ AsyncQueueDepth(node)' = AsyncQueueDepth(node)
             /\ AsyncIoQueueDepth(node)' = AsyncIoQueueDepth(node)
             /\ AsyncOutstandingWorkCount(node)' =
                    AsyncOutstandingWorkCount(node) + 1
             /\ QueuedCompletionCount(node)' =
                    QueuedCompletionCount(node)
             /\ DeferredCompletionCount(node)' =
                    DeferredCompletionCount(node)
        BY <3>2, <3>3, Isa
           DEF AsyncQueueDepth, AsyncIoQueueDepth,
               AsyncOutstandingWorkCount, QueuedCompletionCount,
               QueuedCompletionIndices, DeferredCompletionCount
      <3>5. AsyncCompletionLoad(node)' =
               AsyncCompletionLoad(node) + 1
        BY <3>4, Isa DEF AsyncCompletionLoad
      <3>6. /\ AsyncQueueDepth(node) <= AsyncQueueCapacity
             /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
             /\ AsyncOutstandingWorkCount(node) <= AsyncIoWorkCapacity
        BY <2>1 DEF AsyncIoCapacityTypeInvariant
      <3>7. /\ AsyncIoWorkCapacity \in Nat
             /\ AsyncCompletionReserve \in Nat
             /\ AsyncIoWorkCapacity <= AsyncCompletionReserve
        BY <2>1 DEF AsyncConfiguration
      <3>8. AsyncCompletionLoad(node) \in Nat
        <4>1. /\ AsyncQueueTyped(asyncCommandQueues[node])
               /\ asyncDeferredCompletionQueues[node]
                    \in Seq(Range(asyncDeferredCompletionQueues[node]))
          BY <1>1
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncRuntimeTypeInvariant,
                 AsyncRuntimeScalarTypeInvariant,
                 AsyncDeferredTypeInvariant,
                 AsyncDeferredContentTypeInvariant,
                 AsyncCompletionSequenceTyped
        <4>2. /\ Len(asyncCommandQueues[node]) \in Nat
               /\ IsFiniteSet(1..Len(asyncCommandQueues[node]))
          BY <4>1, LenProperties, FS_Interval, SMT
             DEF AsyncQueueTyped
        <4>3. /\ QueuedCompletionIndices(node)
                         \subseteq 1..Len(asyncCommandQueues[node])
               /\ IsFiniteSet(QueuedCompletionIndices(node))
          BY <4>2, FS_Subset DEF QueuedCompletionIndices
        <4> QED BY <3>1, <4>1, <4>3,
             FS_CardinalityType, LenProperties, SMT
             DEF AsyncOutstandingWorkCount, QueuedCompletionCount,
                 DeferredCompletionCount, AsyncCompletionLoad
      <3>9. AsyncOutstandingWorkCount(node)' <= AsyncIoWorkCapacity
        BY <1>1, <3>4, <3>7, SMT
      <3>10. /\ AsyncQueueDepth(node)' <= AsyncQueueCapacity
              /\ AsyncIoQueueDepth(node)' <= AsyncIoCapacity
              /\ AsyncOutstandingWorkCount(node)' <= AsyncIoWorkCapacity
        BY <3>4, <3>6, <3>9
      <3>11. \A other \in ValidatorIds:
               /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
               /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
               /\ AsyncOutstandingWorkCount(other)' <= AsyncIoWorkCapacity
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
                     /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
                     /\ AsyncOutstandingWorkCount(other)' <=
                          AsyncIoWorkCapacity
          <5>1. CASE other = node
            BY <3>10, <5>1
          <5>2. CASE other # node
            <6>1. /\ asyncOutstandingWork'[other] =
                           asyncOutstandingWork[other]
                   /\ asyncLocalReadyCompletions'[other] =
                           asyncLocalReadyCompletions[other]
              BY <1>1, <2>1, <4>1, <5>2,
                 FunctionalUpdateAwayFromKey
                 DEF AsyncIoTopologyTypeInvariant
            <6>2. /\ AsyncQueueDepth(other)' =
                           AsyncQueueDepth(other)
                   /\ AsyncIoQueueDepth(other)' =
                           AsyncIoQueueDepth(other)
                   /\ AsyncOutstandingWorkCount(other)' =
                           AsyncOutstandingWorkCount(other)
              BY <1>1, <6>1, Isa
                 DEF AsyncQueueDepth, AsyncIoQueueDepth,
                     AsyncOutstandingWorkCount
            <6> QED BY <2>1, <4>1, <6>2
                 DEF AsyncIoCapacityTypeInvariant
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>11 DEF AsyncIoCapacityTypeInvariant
    <2> QED BY <2>3, <2>4, <2>5, <2>6
         DEF AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
  <1> QED BY <1>1

THEOREM CausalCompletionAdmissionIoFrame ==
  \A node \in ValidatorIds:
    /\ AdmitCausalHead(node)
    /\ ~CandidateInFlight(HeadCausalCandidate(node))
    /\ HeadCausalCandidate(node).class = "Completion"
    => LET candidate == HeadCausalCandidate(node)
       IN /\ asyncIoQueues' =
               [asyncIoQueues EXCEPT
                  ![node] = Append(@, AsyncIoConsensusJob(candidate))]
          /\ asyncOutstandingWork' =
               [asyncOutstandingWork EXCEPT
                  ![node] = @ \cup {candidate}]
          /\ UNCHANGED <<asyncCommandQueues,
                          asyncIoReadyCompletions,
                          asyncLocalReadyCompletions,
                          asyncNextCompletionSource,
                          asyncIoControlAvailable>>
BY Isa
   DEF AdmitCausalHead, CandidateInFlight, HeadCausalCandidate

THEOREM CausalCompletionAdmissionPreservesIoType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AdmitCausalHead(node)
    /\ ~CandidateInFlight(HeadCausalCandidate(node))
    /\ HeadCausalCandidate(node).class = "Completion"
    /\ UNCHANGED asyncDeferredCompletionQueues
    => AsyncIoTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                AdmitCausalHead(node),
                ~CandidateInFlight(HeadCausalCandidate(node)),
                HeadCausalCandidate(node).class = "Completion",
                UNCHANGED asyncDeferredCompletionQueues
         PROVE AsyncIoTypeInvariant'
    <2>1. AsyncIoTopologyTypeInvariant'
      BY <1>1, FunctionalUpdatePreservesType, SMTT(30)
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant,
             AdmitCausalHead, HeadCausalCandidate, CandidateInFlight
    <2>2. AsyncIoQueueContentTypeInvariant'
      <3> DEFINE Candidate == HeadCausalCandidate(node)
      <3> DEFINE NewJob == AsyncIoConsensusJob(Candidate)
      <3>1. /\ AsyncCausalTypeInvariant
             /\ AsyncIoQueueContentTypeInvariant
             /\ AsyncIoWorkContentTypeInvariant
        BY <1>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
               AsyncIoContentTypeInvariant
      <3>2. /\ AsyncCandidateTyped(Candidate)
             /\ Candidate \notin asyncOutstandingWork[node]
             /\ Candidate \notin QueuedCandidates
             /\ Candidate \notin DeferredCandidates
        BY <1>1, <3>1, CausalHeadCandidateIsTyped,
           CausalUntrackedCandidateFacts
           DEF AdmitCausalHead, CausalHeadCanAdvance, Candidate
      <3>3. /\ AsyncIoJobTyped(NewJob)
             /\ NewJob.class = "Consensus"
             /\ NewJob.candidate = Candidate
        BY <1>1, <3>1, <3>2,
           TypedCompletionCandidateMakesConsensusJob, SMT
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
               NewJob, AsyncIoConsensusJob, AsyncIoJob, Candidate
      <3>4. /\ asyncIoQueues' =
                    [asyncIoQueues EXCEPT ![node] = Append(@, NewJob)]
             /\ asyncOutstandingWork' =
                    [asyncOutstandingWork EXCEPT
                       ![node] = @ \cup {Candidate}]
             /\ UNCHANGED <<asyncCommandQueues,
                            asyncIoReadyCompletions,
                            asyncLocalReadyCompletions,
                            asyncNextCompletionSource,
                            asyncIoControlAvailable>>
        BY <1>1, CausalCompletionAdmissionIoFrame
           DEF Candidate, NewJob
      <3>5. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
             /\ AsyncIoServeNonceOwnership(asyncIoQueues[node])
             /\ \A job \in SequenceSet(asyncIoQueues[node]):
                  job.class = "Consensus" =>
                    job.candidate \in asyncOutstandingWork[node]
             /\ AsyncIoConsensusCandidateOwnership(
                  node, asyncIoQueues, asyncIoReadyCompletions,
                  asyncLocalReadyCompletions)
             /\ SequenceSet(asyncIoReadyCompletions[node])
                    \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncLocalReadyCompletions[node])
                    \subseteq asyncOutstandingWork[node]
        BY <3>1
           DEF AsyncIoQueueContentTypeInvariant,
               AsyncIoWorkContentTypeInvariant
      <3>6. /\ AsyncIoSequenceTyped(
                    Append(asyncIoQueues[node], NewJob))
             /\ SequenceSet(Append(asyncIoQueues[node], NewJob)) =
                    SequenceSet(asyncIoQueues[node]) \cup {NewJob}
             /\ AsyncIoConsensusIndices(
                    Append(asyncIoQueues[node], NewJob)) =
                    AsyncIoConsensusIndices(asyncIoQueues[node])
                      \cup {Len(asyncIoQueues[node]) + 1}
        BY <3>3, <3>5, TypedIoAppendPreservesSequenceType,
           SequenceSetAfterAppend, ConsensusIndicesAfterConsensusAppend
           DEF AsyncIoSequenceTyped
      <3>7. /\ Candidate \notin
                    SequenceSet(asyncIoReadyCompletions[node])
             /\ Candidate \notin
                    SequenceSet(asyncLocalReadyCompletions[node])
             /\ \A index \in AsyncIoConsensusIndices(
                              asyncIoQueues[node]):
                  asyncIoQueues[node][index].candidate # Candidate
        <4>1. /\ Candidate \notin
                        SequenceSet(asyncIoReadyCompletions[node])
               /\ Candidate \notin
                        SequenceSet(asyncLocalReadyCompletions[node])
          <5>1. SequenceSet(asyncIoReadyCompletions[node])
                    \subseteq asyncOutstandingWork[node]
            BY <3>1 DEF AsyncIoWorkContentTypeInvariant
          <5>2. SequenceSet(asyncLocalReadyCompletions[node])
                    \subseteq asyncOutstandingWork[node]
            BY <3>1 DEF AsyncIoWorkContentTypeInvariant
          <5>3. Candidate \notin asyncOutstandingWork[node]
            BY <3>2
          <5> QED BY <5>1, <5>2, <5>3, Isa
        <4>2. \A index \in AsyncIoConsensusIndices(
                                 asyncIoQueues[node]):
                    asyncIoQueues[node][index].candidate # Candidate
          <5>1. ASSUME NEW index \in AsyncIoConsensusIndices(
                                      asyncIoQueues[node])
                 PROVE asyncIoQueues[node][index].candidate # Candidate
            <6>1. asyncIoQueues[node][index].class = "Consensus"
              BY <5>1 DEF AsyncIoConsensusIndices
            <6>2. asyncIoQueues[node][index]
                        \in SequenceSet(asyncIoQueues[node])
              BY <5>1 DEF AsyncIoConsensusIndices, SequenceSet
            <6>3. asyncIoQueues[node][index].candidate
                        \in asyncOutstandingWork[node]
              BY <3>5, <6>1, <6>2
            <6> QED BY <3>2, <6>3
          <5> QED BY <5>1
        <4> QED BY <4>1, <4>2
      <3>8. /\ AsyncIoSequenceTyped(asyncIoQueues'[node])
             /\ AsyncIoServeNonceOwnership(asyncIoQueues'[node])
             /\ (\A job \in SequenceSet(asyncIoQueues'[node]):
                   job.class = "Consensus" =>
                     job.candidate \in asyncOutstandingWork'[node])
             /\ AsyncIoConsensusCandidateOwnership(
                  node, asyncIoQueues', asyncIoReadyCompletions',
                  asyncLocalReadyCompletions')
        <4>1. /\ asyncIoQueues'[node] =
                      Append(asyncIoQueues[node], NewJob)
               /\ asyncOutstandingWork'[node] =
                      asyncOutstandingWork[node] \cup {Candidate}
               /\ asyncIoReadyCompletions'[node] =
                      asyncIoReadyCompletions[node]
               /\ asyncLocalReadyCompletions'[node] =
                      asyncLocalReadyCompletions[node]
          BY <1>1, <3>4, FunctionalAppendUpdateAtKey, Isa
             DEF AsyncIoTopologyTypeInvariant, AsyncTypeInvariant,
                 AsyncSchedulerTypeInvariant, AsyncIoTypeInvariant
        <4>2. AsyncConfiguration
          BY <1>1
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant
        <4>3. /\ (\A job \in SequenceSet(asyncIoQueues[node]):
                        job.class = "Consensus" =>
                          job.candidate \in asyncOutstandingWork[node])
               /\ AsyncIoConsensusQueueOwnership(
                    asyncIoQueues[node],
                    asyncIoReadyCompletions[node],
                    asyncLocalReadyCompletions[node])
          BY <3>1 DEF AsyncIoQueueContentTypeInvariant,
                       AsyncIoConsensusCandidateOwnership
        <4>4. /\ Candidate \notin
                        SequenceSet(asyncIoReadyCompletions[node])
               /\ Candidate \notin
                        SequenceSet(asyncLocalReadyCompletions[node])
          BY <3>7
        <4>5. /\ AsyncIoSequenceTyped(
                       Append(asyncIoQueues[node], NewJob))
               /\ AsyncIoServeNonceOwnership(
                    Append(asyncIoQueues[node], NewJob))
               /\ (\A job \in
                          SequenceSet(Append(asyncIoQueues[node], NewJob)):
                     job.class = "Consensus" =>
                       job.candidate \in
                         asyncOutstandingWork[node] \cup {Candidate})
               /\ AsyncIoConsensusQueueOwnership(
                    Append(asyncIoQueues[node], NewJob),
                    asyncIoReadyCompletions[node],
                    asyncLocalReadyCompletions[node])
          BY <1>1, <3>2, <3>3, <3>5, <4>2, <4>3, <4>4,
             AppendFreshConsensusJobPreservesQueueFacts
             DEF Candidate, NewJob
        <4> QED BY <4>1, <4>5, Isa
             DEF AsyncIoConsensusCandidateOwnership
      <3>9. \A other \in ValidatorIds:
               /\ AsyncIoSequenceTyped(asyncIoQueues'[other])
               /\ AsyncIoServeNonceOwnership(asyncIoQueues'[other])
               /\ (\A job \in SequenceSet(asyncIoQueues'[other]):
                     job.class = "Consensus" =>
                       job.candidate \in asyncOutstandingWork'[other])
               /\ AsyncIoConsensusCandidateOwnership(
                    other, asyncIoQueues', asyncIoReadyCompletions',
                    asyncLocalReadyCompletions')
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ AsyncIoSequenceTyped(asyncIoQueues'[other])
                     /\ AsyncIoServeNonceOwnership(asyncIoQueues'[other])
                     /\ (\A job \in SequenceSet(asyncIoQueues'[other]):
                           job.class = "Consensus" =>
                             job.candidate \in
                               asyncOutstandingWork'[other])
                     /\ AsyncIoConsensusCandidateOwnership(
                          other, asyncIoQueues',
                          asyncIoReadyCompletions',
                          asyncLocalReadyCompletions')
          <5>1. CASE other = node
            BY <3>8, <5>1, Isa
          <5>2. CASE other # node
            <6>1. /\ asyncIoQueues'[other] =
                          asyncIoQueues[other]
                   /\ asyncOutstandingWork'[other] =
                          asyncOutstandingWork[other]
                   /\ asyncIoReadyCompletions' =
                          asyncIoReadyCompletions
                   /\ asyncLocalReadyCompletions' =
                          asyncLocalReadyCompletions
              BY <1>1, <3>4, <4>1, <5>2,
                 FunctionalAppendUpdateAwayFromKey,
                 FunctionalUpdateAwayFromKey, Isa
                 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                     AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant
            <6>2. /\ AsyncIoSequenceTyped(asyncIoQueues[other])
                   /\ AsyncIoServeNonceOwnership(asyncIoQueues[other])
                   /\ (\A job \in SequenceSet(asyncIoQueues[other]):
                         job.class = "Consensus" =>
                           job.candidate \in asyncOutstandingWork[other])
                   /\ AsyncIoConsensusCandidateOwnership(
                        other, asyncIoQueues, asyncIoReadyCompletions,
                        asyncLocalReadyCompletions)
              BY <3>1, <4>1 DEF AsyncIoQueueContentTypeInvariant
            <6> QED BY <6>1, <6>2, Isa
                 DEF AsyncIoConsensusCandidateOwnership
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>9 DEF AsyncIoQueueContentTypeInvariant
    <2>3. AsyncIoWorkContentTypeInvariant'
      <3> DEFINE Candidate == HeadCausalCandidate(node)
      <3>1. AsyncIoWorkContentTypeInvariant
        BY <1>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
      <3>2. /\ AsyncCandidateTyped(Candidate)
             /\ Candidate.class = "Completion"
             /\ Candidate.node = node
             /\ Candidate \notin asyncOutstandingWork[node]
             /\ Candidate \notin QueuedCandidates
        BY <1>1, CausalHeadCandidateIsTyped,
           CausalHeadCandidateIsOwned, CausalUntrackedCandidateFacts
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AdmitCausalHead,
               CausalHeadCanAdvance, Candidate
      <3>3. Candidate \notin
               SequenceSet(asyncCommandQueues[node])
        BY <1>1, <3>2, SMT DEF QueuedCandidates
      <3>4. /\ asyncOutstandingWork' =
                    [asyncOutstandingWork EXCEPT
                       ![node] = @ \cup {Candidate}]
             /\ UNCHANGED <<asyncCommandQueues,
                            asyncIoReadyCompletions,
                            asyncLocalReadyCompletions>>
        BY <1>1, CausalCompletionAdmissionIoFrame DEF Candidate
      <3>5. /\ IsFiniteSet(asyncOutstandingWork[node])
             /\ (\A work \in asyncOutstandingWork[node]:
                   /\ AsyncCandidateTyped(work)
                   /\ work.class = "Completion"
                   /\ work.node = node)
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions[node])
             /\ Len(asyncIoReadyCompletions[node]) =
                  Cardinality(SequenceSet(asyncIoReadyCompletions[node]))
             /\ Len(asyncLocalReadyCompletions[node]) =
                  Cardinality(SequenceSet(asyncLocalReadyCompletions[node]))
             /\ SequenceSet(asyncIoReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncLocalReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions[node]) = {}
             /\ SequenceSet(asyncCommandQueues[node]) \cap
                  asyncOutstandingWork[node] = {}
        BY <3>1 DEF AsyncIoWorkContentTypeInvariant
      <3>6. /\ IsFiniteSet(
                       asyncOutstandingWork[node] \cup {Candidate})
             /\ (\A work \in
                        asyncOutstandingWork[node] \cup {Candidate}:
                   /\ AsyncCandidateTyped(work)
                   /\ work.class = "Completion"
                   /\ work.node = node)
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions[node])
             /\ Len(asyncIoReadyCompletions[node]) =
                  Cardinality(SequenceSet(asyncIoReadyCompletions[node]))
             /\ Len(asyncLocalReadyCompletions[node]) =
                  Cardinality(SequenceSet(asyncLocalReadyCompletions[node]))
             /\ SequenceSet(asyncIoReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node] \cup {Candidate}
             /\ SequenceSet(asyncLocalReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node] \cup {Candidate}
             /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions[node]) = {}
             /\ SequenceSet(asyncCommandQueues[node]) \cap
                  (asyncOutstandingWork[node] \cup {Candidate}) = {}
        BY <3>2, <3>3, <3>5,
           AddFreshCompletionPreservesNodeWorkFacts
      <3>7. /\ IsFiniteSet(asyncOutstandingWork'[node])
             /\ (\A work \in asyncOutstandingWork'[node]:
                   /\ AsyncCandidateTyped(work)
                   /\ work.class = "Completion"
                   /\ work.node = node)
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions'[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions'[node])
             /\ Len(asyncIoReadyCompletions'[node]) =
                  Cardinality(SequenceSet(asyncIoReadyCompletions'[node]))
             /\ Len(asyncLocalReadyCompletions'[node]) =
                  Cardinality(SequenceSet(asyncLocalReadyCompletions'[node]))
             /\ SequenceSet(asyncIoReadyCompletions'[node])
                  \subseteq asyncOutstandingWork'[node]
             /\ SequenceSet(asyncLocalReadyCompletions'[node])
                  \subseteq asyncOutstandingWork'[node]
             /\ SequenceSet(asyncIoReadyCompletions'[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions'[node]) = {}
             /\ SequenceSet(asyncCommandQueues'[node]) \cap
                  asyncOutstandingWork'[node] = {}
        BY <1>1, <3>4, <3>6, Isa
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant
      <3>8. \A other \in ValidatorIds:
               /\ IsFiniteSet(asyncOutstandingWork'[other])
               /\ (\A work \in asyncOutstandingWork'[other]:
                     /\ AsyncCandidateTyped(work)
                     /\ work.class = "Completion"
                     /\ work.node = other)
               /\ AsyncCompletionSequenceTyped(
                    asyncIoReadyCompletions'[other])
               /\ AsyncCompletionSequenceTyped(
                    asyncLocalReadyCompletions'[other])
               /\ Len(asyncIoReadyCompletions'[other]) =
                    Cardinality(
                      SequenceSet(asyncIoReadyCompletions'[other]))
               /\ Len(asyncLocalReadyCompletions'[other]) =
                    Cardinality(
                      SequenceSet(asyncLocalReadyCompletions'[other]))
               /\ SequenceSet(asyncIoReadyCompletions'[other])
                    \subseteq asyncOutstandingWork'[other]
               /\ SequenceSet(asyncLocalReadyCompletions'[other])
                    \subseteq asyncOutstandingWork'[other]
               /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                    SequenceSet(asyncLocalReadyCompletions'[other]) = {}
               /\ SequenceSet(asyncCommandQueues'[other]) \cap
                    asyncOutstandingWork'[other] = {}
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ IsFiniteSet(asyncOutstandingWork'[other])
                     /\ (\A work \in asyncOutstandingWork'[other]:
                           /\ AsyncCandidateTyped(work)
                           /\ work.class = "Completion"
                           /\ work.node = other)
                     /\ AsyncCompletionSequenceTyped(
                          asyncIoReadyCompletions'[other])
                     /\ AsyncCompletionSequenceTyped(
                          asyncLocalReadyCompletions'[other])
                     /\ Len(asyncIoReadyCompletions'[other]) =
                          Cardinality(
                            SequenceSet(asyncIoReadyCompletions'[other]))
                     /\ Len(asyncLocalReadyCompletions'[other]) =
                          Cardinality(
                            SequenceSet(asyncLocalReadyCompletions'[other]))
                     /\ SequenceSet(asyncIoReadyCompletions'[other])
                          \subseteq asyncOutstandingWork'[other]
                     /\ SequenceSet(asyncLocalReadyCompletions'[other])
                          \subseteq asyncOutstandingWork'[other]
                     /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                          SequenceSet(asyncLocalReadyCompletions'[other]) = {}
                     /\ SequenceSet(asyncCommandQueues'[other]) \cap
                          asyncOutstandingWork'[other] = {}
          <5>1. CASE other = node
            BY <3>7, <5>1
          <5>2. CASE other # node
            <6>1. /\ asyncOutstandingWork'[other] =
                          asyncOutstandingWork[other]
                   /\ asyncCommandQueues' = asyncCommandQueues
                   /\ asyncIoReadyCompletions' =
                          asyncIoReadyCompletions
                   /\ asyncLocalReadyCompletions' =
                          asyncLocalReadyCompletions
              BY <1>1, <3>4, <4>1, <5>2,
                 FunctionalUpdateAwayFromKey, Isa
                 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                     AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant
            <6>2. /\ IsFiniteSet(asyncOutstandingWork[other])
                   /\ (\A work \in asyncOutstandingWork[other]:
                         /\ AsyncCandidateTyped(work)
                         /\ work.class = "Completion"
                         /\ work.node = other)
                   /\ AsyncCompletionSequenceTyped(
                        asyncIoReadyCompletions[other])
                   /\ AsyncCompletionSequenceTyped(
                        asyncLocalReadyCompletions[other])
                   /\ Len(asyncIoReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncIoReadyCompletions[other]))
                   /\ Len(asyncLocalReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncLocalReadyCompletions[other]))
                   /\ SequenceSet(asyncIoReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncLocalReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncIoReadyCompletions[other]) \cap
                        SequenceSet(asyncLocalReadyCompletions[other]) = {}
                   /\ SequenceSet(asyncCommandQueues[other]) \cap
                        asyncOutstandingWork[other] = {}
              BY <3>1, <4>1 DEF AsyncIoWorkContentTypeInvariant
            <6> QED BY <6>1, <6>2, Isa
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>8 DEF AsyncIoWorkContentTypeInvariant
    <2>4. AsyncIoCapacityTypeInvariant'
      <3> DEFINE Candidate == HeadCausalCandidate(node)
      <3> DEFINE NewJob == AsyncIoConsensusJob(Candidate)
      <3>1. /\ AsyncConfiguration
             /\ AsyncIoTopologyTypeInvariant
             /\ AsyncIoQueueContentTypeInvariant
             /\ AsyncIoWorkContentTypeInvariant
             /\ AsyncIoCapacityTypeInvariant
        BY <1>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
               AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
      <3>2. /\ Candidate \notin asyncOutstandingWork[node]
             /\ IsFiniteSet(asyncOutstandingWork[node])
             /\ AsyncIoSequenceTyped(asyncIoQueues[node])
             /\ AsyncQueueTyped(asyncCommandQueues[node])
             /\ DOMAIN asyncIoQueues = ValidatorIds
             /\ DOMAIN asyncOutstandingWork = ValidatorIds
             /\ DOMAIN asyncCommandQueues = ValidatorIds
        BY <1>1, <3>1, CausalUntrackedCandidateFacts
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
               AsyncIoQueueContentTypeInvariant,
               AsyncIoWorkContentTypeInvariant,
               AsyncIoTopologyTypeInvariant, AdmitCausalHead,
               CausalHeadCanAdvance, Candidate
      <3>3. /\ IsFiniteSet(
                       asyncOutstandingWork[node] \cup {Candidate})
             /\ Cardinality(
                  asyncOutstandingWork[node] \cup {Candidate}) =
                    Cardinality(asyncOutstandingWork[node]) + 1
        BY <3>2, FS_AddElement
      <3>4. /\ asyncCommandQueues' = asyncCommandQueues
             /\ asyncIoQueues' =
                    [asyncIoQueues EXCEPT
                       ![node] = Append(@, NewJob)]
             /\ asyncOutstandingWork' =
                    [asyncOutstandingWork EXCEPT
                       ![node] = @ \cup {Candidate}]
             /\ asyncDeferredCompletionQueues' =
                    asyncDeferredCompletionQueues
        BY <1>1, CausalCompletionAdmissionIoFrame, Isa
           DEF Candidate, NewJob
      <3>5. /\ asyncIoQueues'[node] =
                    Append(asyncIoQueues[node], NewJob)
             /\ asyncOutstandingWork'[node] =
                    asyncOutstandingWork[node] \cup {Candidate}
             /\ asyncDeferredCompletionQueues'[node] =
                    asyncDeferredCompletionQueues[node]
        BY <1>1, <3>2, <3>4, FunctionalAppendUpdateAtKey, Isa
      <3>6. Len(Append(asyncIoQueues[node], NewJob)) =
               Len(asyncIoQueues[node]) + 1
        BY <3>2, AppendSequenceFacts DEF AsyncIoSequenceTyped
      <3>7. /\ AsyncQueueDepth(node)' = AsyncQueueDepth(node)
             /\ AsyncIoQueueDepth(node)' = AsyncIoQueueDepth(node) + 1
             /\ AsyncOutstandingWorkCount(node)' =
                    AsyncOutstandingWorkCount(node) + 1
             /\ QueuedCompletionCount(node)' =
                    QueuedCompletionCount(node)
             /\ DeferredCompletionCount(node)' =
                    DeferredCompletionCount(node)
        BY <3>3, <3>4, <3>5, <3>6, Isa
           DEF AsyncQueueDepth, AsyncIoQueueDepth,
               AsyncOutstandingWorkCount, QueuedCompletionCount,
               QueuedCompletionIndices, DeferredCompletionCount
      <3>8. /\ AsyncOutstandingWorkCount(node) \in Nat
             /\ AsyncOutstandingWorkCount(node)' \in Nat
             /\ QueuedCompletionCount(node) \in Nat
             /\ QueuedCompletionCount(node)' \in Nat
             /\ DeferredCompletionCount(node) \in Nat
             /\ DeferredCompletionCount(node)' \in Nat
             /\ AsyncQueueDepth(node) \in Nat
             /\ AsyncIoQueueDepth(node) \in Nat
             /\ AsyncCompletionLoad(node) \in Nat
             /\ AsyncCompletionLoad(node)' \in Nat
        <4>1. /\ Cardinality(asyncOutstandingWork[node]) \in Nat
               /\ Cardinality(
                    asyncOutstandingWork[node] \cup {Candidate}) \in Nat
          BY <3>2, <3>3, FS_CardinalityType
        <4>2. /\ Len(asyncCommandQueues[node]) \in Nat
               /\ IsFiniteSet(1..Len(asyncCommandQueues[node]))
               /\ Len(asyncIoQueues[node]) \in Nat
          BY <3>2, LenProperties, FS_Interval, SMT
             DEF AsyncQueueTyped, AsyncIoSequenceTyped
        <4>3. /\ QueuedCompletionIndices(node)
                         \subseteq 1..Len(asyncCommandQueues[node])
               /\ IsFiniteSet(QueuedCompletionIndices(node))
          BY <4>2, FS_Subset DEF QueuedCompletionIndices
        <4>4. asyncDeferredCompletionQueues[node]
                    \in Seq(Range(asyncDeferredCompletionQueues[node]))
          BY <1>1
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncDeferredTypeInvariant,
                 AsyncDeferredContentTypeInvariant,
                 AsyncCompletionSequenceTyped
        <4> QED BY <3>4, <3>5, <3>7, <4>1, <4>2, <4>3, <4>4,
             FS_CardinalityType, LenProperties, SMT
             DEF AsyncOutstandingWorkCount, QueuedCompletionCount,
                 DeferredCompletionCount, AsyncQueueDepth,
                 AsyncIoQueueDepth, AsyncCompletionLoad
      <3>9. AsyncCompletionLoad(node)' =
               AsyncCompletionLoad(node) + 1
        BY <3>7, <3>8, SMT DEF AsyncCompletionLoad
      <3>10. /\ AsyncOutstandingWorkCount(node) < AsyncIoWorkCapacity
              /\ AsyncIoQueueDepth(node) <
                   AsyncIoAuxCapacity + AsyncIoWorkCapacity
        BY <1>1 DEF AdmitCausalHead, CausalHeadCanAdvance,
                       CanEnqueueIoClass, AsyncIoAdmissionLimit
      <3>11. /\ AsyncQueueDepth(node) <= AsyncQueueCapacity
              /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
              /\ AsyncOutstandingWorkCount(node) <= AsyncIoWorkCapacity
        BY <3>1 DEF AsyncIoCapacityTypeInvariant
      <3>12. /\ AsyncQueueDepth(node)' <= AsyncQueueCapacity
              /\ AsyncIoQueueDepth(node)' <= AsyncIoCapacity
              /\ AsyncOutstandingWorkCount(node)' <= AsyncIoWorkCapacity
        <4>1. AsyncQueueDepth(node)' <= AsyncQueueCapacity
          BY <3>7, <3>11
        <4>2. AsyncOutstandingWorkCount(node)' <= AsyncIoWorkCapacity
          BY <3>1, <3>7, <3>8, <3>10, SMT
             DEF AsyncConfiguration
        <4>6. AsyncIoQueueDepth(node)' <= AsyncIoCapacity
          BY <3>1, <3>7, <3>8, <3>10, SMT
             DEF AsyncConfiguration, AsyncIoCapacity
        <4> QED BY <4>1, <4>2, <4>6
      <3>13. \A other \in ValidatorIds:
                /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
                /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
                /\ AsyncOutstandingWorkCount(other)' <= AsyncIoWorkCapacity
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
                     /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
                     /\ AsyncOutstandingWorkCount(other)' <=
                          AsyncIoWorkCapacity
          <5>1. CASE other = node
            BY <3>12, <5>1
          <5>2. CASE other # node
            <6>1. /\ asyncCommandQueues'[other] =
                          asyncCommandQueues[other]
                   /\ asyncIoQueues'[other] = asyncIoQueues[other]
                   /\ asyncOutstandingWork'[other] =
                          asyncOutstandingWork[other]
                   /\ asyncDeferredCompletionQueues'[other] =
                          asyncDeferredCompletionQueues[other]
              BY <3>2, <3>4, <4>1, <5>2,
                 FunctionalAppendUpdateAwayFromKey,
                 FunctionalUpdateAwayFromKey, Isa
            <6>2. /\ AsyncQueueDepth(other)' = AsyncQueueDepth(other)
                   /\ AsyncOutstandingWorkCount(other)' =
                          AsyncOutstandingWorkCount(other)
                   /\ AsyncIoQueueDepth(other)' = AsyncIoQueueDepth(other)
              BY <6>1, Isa
                 DEF AsyncQueueDepth, AsyncIoQueueDepth,
                     AsyncOutstandingWorkCount
            <6> QED BY <3>1, <4>1, <6>2
                 DEF AsyncIoCapacityTypeInvariant
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>13 DEF AsyncIoCapacityTypeInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncAdmissionLimitsBelowQueueCapacity ==
  AsyncConfiguration
    => /\ AsyncNormalLimit < AsyncQueueCapacity
       /\ AsyncProgressLimit < AsyncQueueCapacity
BY SMT
   DEF AsyncConfiguration, AsyncNormalLimit, AsyncProgressLimit

THEOREM NaturalIncrementWithinBound ==
  \A value, bound \in Nat:
    value < bound => value + 1 <= bound
BY SMT

THEOREM StrictLessTransitive ==
  \A lower, middle, upper:
    lower < middle /\ middle < upper => lower < upper
BY SMT

THEOREM EnqueueCandidatePreservesIoType ==
  \A node \in ValidatorIds:
    \A candidate:
      /\ AsyncTypeInvariant
      /\ AsyncCandidateTyped(candidate)
      /\ candidate.node = node
      /\ CanEnqueueClass(node, candidate.class)
      /\ asyncCommandQueues' =
           [asyncCommandQueues EXCEPT ![node] = Append(@, candidate)]
      /\ UNCHANGED <<AsyncIoVars, asyncDeferredCompletionQueues>>
      => AsyncIoTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW candidate,
                AsyncTypeInvariant,
                AsyncCandidateTyped(candidate),
                candidate.node = node,
                CanEnqueueClass(node, candidate.class),
                asyncCommandQueues' =
                  [asyncCommandQueues EXCEPT
                     ![node] = Append(@, candidate)],
                UNCHANGED <<AsyncIoVars,
                            asyncDeferredCompletionQueues>>
         PROVE AsyncIoTypeInvariant'
    <2>1. /\ AsyncConfiguration
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoQueueContentTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>2. AsyncIoTopologyTypeInvariant'
      BY <1>1, <2>1, AsyncIoTopologyTypeStutter
         DEF AsyncIoVars, AsyncIoTopologyTypeVars
    <2>3. AsyncIoQueueContentTypeInvariant'
      BY <1>1, <2>1, AsyncIoQueueContentTypeStutter
         DEF AsyncIoVars, AsyncIoQueueContentTypeVars
    <2>4. AsyncIoWorkContentTypeInvariant'
      <3>1. /\ DOMAIN asyncCommandQueues = ValidatorIds
             /\ AsyncQueueTyped(asyncCommandQueues[node])
             /\ asyncCommandQueues[node] \in
                  Seq(Range(asyncCommandQueues[node]))
        BY <1>1, <2>1
           DEF AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped
      <3>2. /\ asyncOutstandingWork' = asyncOutstandingWork
             /\ asyncIoReadyCompletions' = asyncIoReadyCompletions
             /\ asyncLocalReadyCompletions' =
                  asyncLocalReadyCompletions
        BY <1>1 DEF AsyncIoVars
      <3>3. candidate \notin asyncOutstandingWork[node]
        BY <1>1, <2>1, SMT
           DEF AsyncIoWorkContentTypeInvariant
      <3>4. /\ asyncCommandQueues'[node] =
                    Append(asyncCommandQueues[node], candidate)
             /\ SequenceSet(asyncCommandQueues'[node]) =
                  SequenceSet(asyncCommandQueues[node]) \cup {candidate}
        BY <1>1, <3>1, FunctionalAppendUpdateAtKey,
           SequenceSetAfterAppend
      <3>5. SequenceSet(asyncCommandQueues'[node]) \cap
               asyncOutstandingWork'[node] = {}
        BY <2>1, <3>2, <3>3, <3>4, SMT
           DEF AsyncIoWorkContentTypeInvariant
      <3>6. \A other \in ValidatorIds:
               /\ IsFiniteSet(asyncOutstandingWork'[other])
               /\ (\A work \in asyncOutstandingWork'[other]:
                     /\ AsyncCandidateTyped(work)
                     /\ work.class = "Completion"
                     /\ work.node = other)
               /\ AsyncCompletionSequenceTyped(
                    asyncIoReadyCompletions'[other])
               /\ AsyncCompletionSequenceTyped(
                    asyncLocalReadyCompletions'[other])
               /\ Len(asyncIoReadyCompletions'[other]) =
                    Cardinality(
                      SequenceSet(asyncIoReadyCompletions'[other]))
               /\ Len(asyncLocalReadyCompletions'[other]) =
                    Cardinality(
                      SequenceSet(asyncLocalReadyCompletions'[other]))
               /\ SequenceSet(asyncIoReadyCompletions'[other])
                    \subseteq asyncOutstandingWork'[other]
               /\ SequenceSet(asyncLocalReadyCompletions'[other])
                    \subseteq asyncOutstandingWork'[other]
               /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                    SequenceSet(asyncLocalReadyCompletions'[other]) = {}
               /\ SequenceSet(asyncCommandQueues'[other]) \cap
                    asyncOutstandingWork'[other] = {}
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ IsFiniteSet(asyncOutstandingWork'[other])
                     /\ (\A work \in asyncOutstandingWork'[other]:
                           /\ AsyncCandidateTyped(work)
                           /\ work.class = "Completion"
                           /\ work.node = other)
                     /\ AsyncCompletionSequenceTyped(
                          asyncIoReadyCompletions'[other])
                     /\ AsyncCompletionSequenceTyped(
                          asyncLocalReadyCompletions'[other])
                     /\ Len(asyncIoReadyCompletions'[other]) =
                          Cardinality(
                            SequenceSet(asyncIoReadyCompletions'[other]))
                     /\ Len(asyncLocalReadyCompletions'[other]) =
                          Cardinality(
                            SequenceSet(asyncLocalReadyCompletions'[other]))
                     /\ SequenceSet(asyncIoReadyCompletions'[other])
                          \subseteq asyncOutstandingWork'[other]
                     /\ SequenceSet(asyncLocalReadyCompletions'[other])
                          \subseteq asyncOutstandingWork'[other]
                     /\ SequenceSet(asyncIoReadyCompletions'[other]) \cap
                          SequenceSet(asyncLocalReadyCompletions'[other]) = {}
                     /\ SequenceSet(asyncCommandQueues'[other]) \cap
                          asyncOutstandingWork'[other] = {}
          <5>1. CASE other = node
            <6>1. /\ asyncOutstandingWork'[other] =
                           asyncOutstandingWork[other]
                   /\ asyncIoReadyCompletions'[other] =
                           asyncIoReadyCompletions[other]
                   /\ asyncLocalReadyCompletions'[other] =
                           asyncLocalReadyCompletions[other]
              BY <3>2, Isa
            <6>2. SequenceSet(asyncCommandQueues'[other]) \cap
                       asyncOutstandingWork'[other] = {}
              BY <3>5, <5>1
            <6>3. /\ IsFiniteSet(asyncOutstandingWork[other])
                   /\ (\A work \in asyncOutstandingWork[other]:
                         /\ AsyncCandidateTyped(work)
                         /\ work.class = "Completion"
                         /\ work.node = other)
                   /\ AsyncCompletionSequenceTyped(
                        asyncIoReadyCompletions[other])
                   /\ AsyncCompletionSequenceTyped(
                        asyncLocalReadyCompletions[other])
                   /\ Len(asyncIoReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncIoReadyCompletions[other]))
                   /\ Len(asyncLocalReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncLocalReadyCompletions[other]))
                   /\ SequenceSet(asyncIoReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncLocalReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncIoReadyCompletions[other]) \cap
                        SequenceSet(asyncLocalReadyCompletions[other]) = {}
              BY <2>1, <4>1 DEF AsyncIoWorkContentTypeInvariant
            <6> QED BY <6>1, <6>2, <6>3, Isa
          <5>2. CASE other # node
            <6>1. asyncCommandQueues'[other] =
                     asyncCommandQueues[other]
              BY <1>1, <3>1, <4>1, <5>2,
                 FunctionalAppendUpdateAwayFromKey
            <6>2. /\ asyncOutstandingWork'[other] =
                           asyncOutstandingWork[other]
                   /\ asyncIoReadyCompletions'[other] =
                           asyncIoReadyCompletions[other]
                   /\ asyncLocalReadyCompletions'[other] =
                           asyncLocalReadyCompletions[other]
              BY <3>2, Isa
            <6>3. /\ IsFiniteSet(asyncOutstandingWork[other])
                   /\ (\A work \in asyncOutstandingWork[other]:
                         /\ AsyncCandidateTyped(work)
                         /\ work.class = "Completion"
                         /\ work.node = other)
                   /\ AsyncCompletionSequenceTyped(
                        asyncIoReadyCompletions[other])
                   /\ AsyncCompletionSequenceTyped(
                        asyncLocalReadyCompletions[other])
                   /\ Len(asyncIoReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncIoReadyCompletions[other]))
                   /\ Len(asyncLocalReadyCompletions[other]) =
                        Cardinality(
                          SequenceSet(asyncLocalReadyCompletions[other]))
                   /\ SequenceSet(asyncIoReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncLocalReadyCompletions[other])
                        \subseteq asyncOutstandingWork[other]
                   /\ SequenceSet(asyncIoReadyCompletions[other]) \cap
                        SequenceSet(asyncLocalReadyCompletions[other]) = {}
                   /\ SequenceSet(asyncCommandQueues[other]) \cap
                        asyncOutstandingWork[other] = {}
              BY <2>1, <4>1 DEF AsyncIoWorkContentTypeInvariant
            <6> QED BY <6>1, <6>2, <6>3, Isa
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>6 DEF AsyncIoWorkContentTypeInvariant
    <2>5. AsyncIoCapacityTypeInvariant'
      <3>1. /\ DOMAIN asyncCommandQueues = ValidatorIds
             /\ AsyncQueueTyped(asyncCommandQueues[node])
             /\ asyncCommandQueues[node] \in
                  Seq(Range(asyncCommandQueues[node]))
             /\ AsyncQueueDepth(node) \in Nat
        BY <1>1, <2>1, LenProperties
           DEF AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped,
               AsyncQueueDepth
      <3>2. /\ asyncOutstandingWork' = asyncOutstandingWork
             /\ asyncIoQueues' = asyncIoQueues
             /\ asyncDeferredCompletionQueues' =
                  asyncDeferredCompletionQueues
        BY <1>1 DEF AsyncIoVars
      <3>3. /\ asyncCommandQueues'[node] =
                    Append(asyncCommandQueues[node], candidate)
             /\ AsyncQueueDepth(node)' = AsyncQueueDepth(node) + 1
        BY <1>1, <3>1, FunctionalAppendUpdateAtKey,
           AppendSequenceFacts, Isa
           DEF AsyncQueueDepth
      <3>4. /\ AsyncOutstandingWorkCount(node)' =
                    AsyncOutstandingWorkCount(node)
             /\ AsyncIoQueueDepth(node)' = AsyncIoQueueDepth(node)
        BY <1>1, <3>2, <3>3, Isa
           DEF AsyncOutstandingWorkCount, AsyncIoQueueDepth
      <3>5. AsyncQueueDepth(node) < AsyncQueueCapacity
        BY <1>1 DEF CanEnqueueClass, CanEnqueueWithCertifiedFenceCredit
      <3>6. /\ AsyncQueueDepth(node)' <= AsyncQueueCapacity
             /\ AsyncIoQueueDepth(node)' <= AsyncIoCapacity
             /\ AsyncOutstandingWorkCount(node)' <=
                    AsyncIoWorkCapacity
        <4>1. AsyncQueueDepth(node)' <= AsyncQueueCapacity
          <5>1. /\ AsyncQueueDepth(node) \in Nat
                 /\ AsyncQueueCapacity \in Nat
            BY <2>1, <3>1 DEF AsyncConfiguration
          <5>2. AsyncQueueDepth(node) + 1 <= AsyncQueueCapacity
            BY <3>5, <5>1, NaturalIncrementWithinBound
          <5> QED BY <3>3, <5>2
        <4>2. /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
               /\ AsyncOutstandingWorkCount(node) <=
                      AsyncIoWorkCapacity
          BY <2>1 DEF AsyncIoCapacityTypeInvariant
        <4> QED BY <3>4, <4>1, <4>2
      <3>7. \A other \in ValidatorIds:
               /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
               /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
               /\ AsyncOutstandingWorkCount(other)' <=
                      AsyncIoWorkCapacity
        <4>1. ASSUME NEW other \in ValidatorIds
               PROVE /\ AsyncQueueDepth(other)' <= AsyncQueueCapacity
                     /\ AsyncIoQueueDepth(other)' <= AsyncIoCapacity
                     /\ AsyncOutstandingWorkCount(other)' <=
                            AsyncIoWorkCapacity
          <5>1. CASE other = node
            BY <3>6, <5>1
          <5>2. CASE other # node
            <6>1. asyncCommandQueues'[other] =
                     asyncCommandQueues[other]
              BY <1>1, <3>1, <4>1, <5>2,
                 FunctionalAppendUpdateAwayFromKey
            <6>2. /\ AsyncQueueDepth(other)' =
                           AsyncQueueDepth(other)
                   /\ AsyncOutstandingWorkCount(other)' =
                           AsyncOutstandingWorkCount(other)
                   /\ AsyncIoQueueDepth(other)' =
                           AsyncIoQueueDepth(other)
              BY <3>2, <6>1, Isa
                 DEF AsyncQueueDepth, AsyncOutstandingWorkCount,
                     AsyncIoQueueDepth
            <6> QED BY <2>1, <4>1, <6>2
                 DEF AsyncIoCapacityTypeInvariant
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>7 DEF AsyncIoCapacityTypeInvariant
    <2> QED BY <2>2, <2>3, <2>4, <2>5
         DEF AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
  <1> QED BY <1>1

THEOREM EnqueueNonCompletionCandidatePreservesIoType ==
  \A node \in ValidatorIds:
    \A candidate:
      /\ AsyncTypeInvariant
      /\ AsyncCandidateTyped(candidate)
      /\ candidate.node = node
      /\ candidate.class # "Completion"
      /\ CanEnqueueClass(node, candidate.class)
      /\ asyncCommandQueues' =
           [asyncCommandQueues EXCEPT ![node] = Append(@, candidate)]
      /\ UNCHANGED <<AsyncIoVars, asyncDeferredCompletionQueues>>
      => AsyncIoTypeInvariant'
BY EnqueueCandidatePreservesIoType

THEOREM CausalAdmissionRunnerPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunNodeWork(node)
    /\ SelectedLocalAdmissionAdvance(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Causal"
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                RunNodeWork(node),
                SelectedLocalAdmissionAdvance(node),
                LocalAdmissionCanAdvance(node),
                SelectedLocalSource(node) = "Causal"
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE Candidate == HeadCausalCandidate(node)
    <2>1. CASE CandidateInFlight(Candidate)
      <3>1. node \in ValidatorIds
        BY <1>1
      <3>2. /\ AsyncRuntimeScalarTypeInvariant
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
      <3>3. CausalQueueNonempty(node)
        BY <1>1, SelectedCausalCanAdvance DEF CausalHeadCanAdvance
      <3>4. AdmitCausalHead(node)
        BY <1>1 DEF SelectedLocalAdmissionAdvance
      <3>5. /\ asyncCausalQueues' =
                    [asyncCausalQueues EXCEPT ![node] = Tail(@)]
             /\ UNCHANGED vars
             /\ UNCHANGED <<asyncCommandQueues,
                            asyncNextCommandClass, asyncFifoOwed,
                            asyncTimeoutEmitted, AsyncIoVars,
                            asyncOutstandingTags, asyncNodeDeadlines,
                            asyncRetransmitDeadlines, asyncSentItems,
                            asyncRetainedControl, asyncActiveRequests,
                            asyncCertifiedResponseClaim,
                            asyncTransport, asyncIngressLanes,
                            asyncIngressReady, asyncHeldChunks>>
        <4>1. CandidateInFlight(HeadCausalCandidate(node))
          BY <2>1 DEF Candidate
        <4> QED BY <3>4, <4>1, Isa
             DEF AdmitCausalHead, CandidateInFlight,
                 HeadCausalCandidate, AsyncIoVars,
                 LeaveCausalQueues, vars
      <3>6. /\ asyncRunnerPhase' = asyncRunnerPhase
             /\ asyncRunnerBudget' =
                    [asyncRunnerBudget EXCEPT ![node] = @ - 1]
             /\ UNCHANGED AsyncDeferredVars
             /\ UpdateLocalAdmissionMetadata(node, "Causal")
        BY <1>1 DEF SelectedLocalAdmissionAdvance
      <3>7. RunnerServiceFrame(node)
        BY <1>1 DEF RunNodeWork, RunnerServiceFrame
      <3>8. /\ asyncRunnerBudget
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
             /\ asyncRunnerBudget[node] \in Nat
             /\ asyncRunnerBudget[node] <=
                    AsyncQueueCapacity + AsyncIngressCapacity
             /\ AsyncQueueCapacity \in Nat
             /\ AsyncIngressCapacity \in Nat
        BY <1>1, <3>1, <3>2, SMT
           DEF AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
      <3>9. asyncRunnerBudget[node] - 1
                 \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
        BY <1>1, <3>8, SMT DEF LocalAdmissionCanAdvance
      <3>10. /\ asyncRunnerBudget'
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
              /\ asyncCausalAdmissionOwed'
                    \in [ValidatorIds -> BOOLEAN]
              /\ asyncNextLocalSource'
                    \in [ValidatorIds -> AsyncLocalSources]
        <4>1. asyncRunnerBudget'
                 \in [ValidatorIds ->
                       0..(AsyncQueueCapacity + AsyncIngressCapacity)]
          BY <3>1, <3>6, <3>8, <3>9,
             FunctionalUpdatePreservesType
             DEF AsyncRuntimeScalarTypeInvariant
        <4>2. /\ asyncCausalAdmissionOwed'
                       \in [ValidatorIds -> BOOLEAN]
                /\ asyncNextLocalSource'
                       \in [ValidatorIds -> AsyncLocalSources]
          BY <3>1, <3>2, <3>6,
             LocalAdmissionMetadataUpdatePreservesType
             DEF AsyncRuntimeScalarTypeInvariant, AsyncLocalSources
        <4> QED BY <4>1, <4>2
      <3>11. AsyncRuntimeScalarTypeInvariant'
        BY <3>2, <3>5, <3>6, <3>7, <3>10, Isa
           DEF RunnerServiceFrame, AsyncRuntimeScalarTypeInvariant,
               AsyncConfiguration, AsyncIoVars, AsyncDeferredVars
      <3>12. AsyncCausalTypeInvariant'
        BY <3>1, <3>2, <3>3, <3>5,
           CausalTailUpdatePreservesCausalType
      <3>13. /\ UNCHANGED AsyncIoTopologyTypeVars
              /\ UNCHANGED AsyncIoContentTypeVars
              /\ UNCHANGED AsyncIoCapacityTypeVars
              /\ UNCHANGED AsyncDeferredTopologyTypeVars
              /\ UNCHANGED <<asyncDeferredCompletionQueues,
                             asyncDeferredProgressQueues,
                             asyncDeferredNormalQueues>>
              /\ UNCHANGED AsyncTransportContentTypeVars
              /\ UNCHANGED AsyncIngressTopologyTypeVars
              /\ UNCHANGED asyncIngressLanes
        BY <3>5, <3>6, Isa
           DEF AsyncIoVars, AsyncDeferredVars,
               AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
               AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
               AsyncTransportContentTypeVars,
               AsyncCertifiedResponseClaimAuthorityVars,
               AsyncIngressTopologyTypeVars, vars
      <3>14. /\ AsyncIoTopologyTypeInvariant'
              /\ AsyncIoContentTypeInvariant'
              /\ AsyncIoCapacityTypeInvariant'
              /\ AsyncDeferredTopologyTypeInvariant'
              /\ AsyncDeferredContentTypeInvariant'
              /\ AsyncTransportContentTypeInvariant'
              /\ AsyncIngressTopologyTypeInvariant'
              /\ AsyncIngressCapacityTypeInvariant'
              /\ AsyncIngressContentTypeInvariant'
        BY <3>2, <3>13, AsyncIoTopologyTypeStutter,
           AsyncIoContentTypeStutter, AsyncIoCapacityTypeStutter,
           AsyncDeferredTopologyTypeStutter,
           AsyncDeferredContentTypeStutter,
           AsyncTransportContentTypeStutter,
           AsyncIngressTopologyTypeStutter,
           AsyncIngressCapacityTypeStutter,
           AsyncIngressContentTypeStutter
      <3>15. AsyncTransportClockTypeInvariant'
        BY <3>1, <3>2, <3>5, <3>7,
           RunnerServiceFramePreservesClockType
      <3> QED BY <1>1, <3>11, <3>12, <3>14, <3>15,
                   SelectedLocalAdmissionAdvancePreservesHistoricalRecoveryType
           DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
               AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
               AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
    <2>2. CASE ~CandidateInFlight(Candidate)
                /\ Candidate.class = "Completion"
      <3>1. node \in ValidatorIds
        BY <1>1
      <3>2. /\ AsyncRuntimeScalarTypeInvariant
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
      <3>3. CausalQueueNonempty(node)
        BY <1>1, SelectedCausalCanAdvance DEF CausalHeadCanAdvance
      <3>4. AdmitCausalHead(node)
        BY <1>1 DEF SelectedLocalAdmissionAdvance
      <3>5. /\ asyncCausalQueues' =
                    [asyncCausalQueues EXCEPT ![node] = Tail(@)]
             /\ UNCHANGED vars
             /\ UNCHANGED <<asyncCommandQueues,
                            asyncNextCommandClass, asyncFifoOwed,
                            asyncTimeoutEmitted,
                            asyncOutstandingTags, asyncNodeDeadlines,
                            asyncRetransmitDeadlines, asyncSentItems,
                            asyncRetainedControl, asyncActiveRequests,
                            asyncCertifiedResponseClaim,
                            asyncTransport, asyncIngressLanes,
                            asyncIngressReady, asyncHeldChunks>>
        BY <2>2, <3>4, Isa
           DEF AdmitCausalHead, Candidate, vars
      <3>6. /\ asyncRunnerPhase' = asyncRunnerPhase
             /\ asyncRunnerBudget' =
                    [asyncRunnerBudget EXCEPT ![node] = @ - 1]
             /\ UNCHANGED AsyncDeferredVars
             /\ UpdateLocalAdmissionMetadata(node, "Causal")
        BY <1>1 DEF SelectedLocalAdmissionAdvance
      <3>7. RunnerServiceFrame(node)
        BY <1>1 DEF RunNodeWork, RunnerServiceFrame
      <3>8. /\ asyncRunnerBudget
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
             /\ asyncRunnerBudget[node] \in Nat
             /\ asyncRunnerBudget[node] <=
                    AsyncQueueCapacity + AsyncIngressCapacity
             /\ AsyncQueueCapacity \in Nat
             /\ AsyncIngressCapacity \in Nat
        BY <1>1, <3>1, <3>2, SMT
           DEF AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
      <3>9. asyncRunnerBudget[node] - 1
                 \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
        BY <1>1, <3>8, SMT DEF LocalAdmissionCanAdvance
      <3>10. /\ asyncRunnerBudget'
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
              /\ asyncCausalAdmissionOwed'
                    \in [ValidatorIds -> BOOLEAN]
              /\ asyncNextLocalSource'
                    \in [ValidatorIds -> AsyncLocalSources]
        <4>1. asyncRunnerBudget'
                 \in [ValidatorIds ->
                       0..(AsyncQueueCapacity + AsyncIngressCapacity)]
          BY <3>1, <3>6, <3>8, <3>9,
             FunctionalUpdatePreservesType
             DEF AsyncRuntimeScalarTypeInvariant
        <4>2. /\ asyncCausalAdmissionOwed'
                       \in [ValidatorIds -> BOOLEAN]
                /\ asyncNextLocalSource'
                       \in [ValidatorIds -> AsyncLocalSources]
          BY <3>1, <3>2, <3>6,
             LocalAdmissionMetadataUpdatePreservesType
             DEF AsyncRuntimeScalarTypeInvariant, AsyncLocalSources
        <4> QED BY <4>1, <4>2
      <3>11. AsyncRuntimeScalarTypeInvariant'
        BY <3>2, <3>5, <3>6, <3>7, <3>10, Isa
           DEF RunnerServiceFrame, AsyncRuntimeScalarTypeInvariant,
               AsyncConfiguration
      <3>12. AsyncCausalTypeInvariant'
        BY <3>1, <3>2, <3>3, <3>5,
           CausalTailUpdatePreservesCausalType
      <3>13. AsyncIoTypeInvariant'
        BY <1>1, <2>2, <3>1, <3>4, <3>6,
           CausalCompletionAdmissionPreservesIoType
           DEF Candidate, AsyncDeferredVars
      <3>14. /\ UNCHANGED AsyncDeferredTopologyTypeVars
              /\ UNCHANGED <<asyncDeferredCompletionQueues,
                             asyncDeferredProgressQueues,
                             asyncDeferredNormalQueues>>
              /\ UNCHANGED AsyncTransportContentTypeVars
              /\ UNCHANGED AsyncIngressTopologyTypeVars
              /\ UNCHANGED asyncIngressLanes
        BY <3>5, <3>6, Isa
           DEF AsyncDeferredVars, AsyncDeferredTopologyTypeVars,
               AsyncTransportContentTypeVars,
               AsyncCertifiedResponseClaimAuthorityVars,
               AsyncIngressTopologyTypeVars, vars
      <3>15. /\ AsyncDeferredTopologyTypeInvariant'
              /\ AsyncDeferredContentTypeInvariant'
              /\ AsyncTransportContentTypeInvariant'
              /\ AsyncIngressTopologyTypeInvariant'
              /\ AsyncIngressCapacityTypeInvariant'
              /\ AsyncIngressContentTypeInvariant'
        BY <3>2, <3>14, AsyncDeferredTopologyTypeStutter,
           AsyncDeferredContentTypeStutter,
           AsyncTransportContentTypeStutter,
           AsyncIngressTopologyTypeStutter,
           AsyncIngressCapacityTypeStutter,
           AsyncIngressContentTypeStutter
      <3>16. AsyncTransportClockTypeInvariant'
        BY <3>1, <3>2, <3>5, <3>7,
           RunnerServiceFramePreservesClockType
      <3> QED BY <1>1, <3>11, <3>12, <3>13, <3>15, <3>16,
                   SelectedLocalAdmissionAdvancePreservesHistoricalRecoveryType
           DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
               AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
               AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
    <2>3. CASE ~CandidateInFlight(Candidate)
                /\ Candidate.class # "Completion"
      <3>1. node \in ValidatorIds
        BY <1>1
      <3>2. /\ AsyncRuntimeScalarTypeInvariant
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
      <3>3. /\ CausalQueueNonempty(node)
             /\ CanEnqueueClass(node, Candidate.class)
        BY <1>1, <2>3, SelectedCausalCanAdvance
           DEF CausalHeadCanAdvance, Candidate
      <3>4. AdmitCausalHead(node)
        BY <1>1 DEF SelectedLocalAdmissionAdvance
      <3>5. /\ AsyncCandidateTyped(Candidate)
             /\ Candidate.node = node
             /\ CanEnqueueClass(node, Candidate.class)
        BY <2>3, <3>1, <3>2, <3>3,
           CausalHeadCandidateIsTyped, CausalHeadCandidateIsOwned
           DEF Candidate
      <3>6. /\ asyncCausalQueues' =
                    [asyncCausalQueues EXCEPT ![node] = Tail(@)]
             /\ asyncCommandQueues' =
                    [asyncCommandQueues EXCEPT
                       ![node] = Append(@, Candidate)]
             /\ UNCHANGED <<vars, asyncNextCommandClass,
                            asyncFifoOwed, asyncTimeoutEmitted, AsyncIoVars,
                            asyncOutstandingTags, asyncNodeDeadlines,
                            asyncRetransmitDeadlines, asyncSentItems,
                            asyncRetainedControl, asyncActiveRequests,
                            asyncCertifiedResponseClaim,
                            asyncTransport, asyncIngressLanes,
                            asyncIngressReady, asyncHeldChunks>>
        BY <2>3, <3>4, <3>5, Isa
           DEF AdmitCausalHead, EnqueueCandidate, Candidate, vars
      <3>7. /\ asyncRunnerPhase' = asyncRunnerPhase
             /\ asyncRunnerBudget' =
                    [asyncRunnerBudget EXCEPT ![node] = @ - 1]
             /\ UNCHANGED AsyncDeferredVars
             /\ UpdateLocalAdmissionMetadata(node, "Causal")
        BY <1>1 DEF SelectedLocalAdmissionAdvance
      <3>8. RunnerServiceFrame(node)
        BY <1>1 DEF RunNodeWork, RunnerServiceFrame
      <3>9. /\ asyncRunnerBudget
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
             /\ asyncRunnerBudget[node] \in Nat
             /\ asyncRunnerBudget[node] <=
                    AsyncQueueCapacity + AsyncIngressCapacity
             /\ AsyncQueueCapacity \in Nat
             /\ AsyncIngressCapacity \in Nat
        BY <1>1, <3>1, <3>2, SMT
           DEF AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
      <3>10. asyncRunnerBudget[node] - 1
                  \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
        BY <1>1, <3>9, SMT DEF LocalAdmissionCanAdvance
      <3>11. /\ asyncRunnerBudget'
                     \in [ValidatorIds ->
                           0..(AsyncQueueCapacity + AsyncIngressCapacity)]
               /\ asyncCausalAdmissionOwed'
                     \in [ValidatorIds -> BOOLEAN]
               /\ asyncNextLocalSource'
                     \in [ValidatorIds -> AsyncLocalSources]
        <4>1. asyncRunnerBudget'
                 \in [ValidatorIds ->
                       0..(AsyncQueueCapacity + AsyncIngressCapacity)]
          BY <3>1, <3>7, <3>9, <3>10,
             FunctionalUpdatePreservesType
             DEF AsyncRuntimeScalarTypeInvariant
        <4>2. /\ asyncCausalAdmissionOwed'
                       \in [ValidatorIds -> BOOLEAN]
                /\ asyncNextLocalSource'
                       \in [ValidatorIds -> AsyncLocalSources]
          BY <3>1, <3>2, <3>7,
             LocalAdmissionMetadataUpdatePreservesType
             DEF AsyncRuntimeScalarTypeInvariant, AsyncLocalSources
        <4> QED BY <4>1, <4>2
      <3>12. /\ DOMAIN asyncCommandQueues' = ValidatorIds
              /\ \A other \in ValidatorIds:
                   /\ AsyncQueueTyped(asyncCommandQueues'[other])
                   /\ AsyncCommandQueueOwnership(
                        other, asyncCommandQueues'[other])
        <4>1. DOMAIN asyncCommandQueues' = ValidatorIds
          BY <3>1, <3>2, <3>6, Isa
             DEF AsyncRuntimeScalarTypeInvariant
        <4>2. \A other \in ValidatorIds:
                   /\ AsyncQueueTyped(asyncCommandQueues'[other])
                   /\ AsyncCommandQueueOwnership(
                        other, asyncCommandQueues'[other])
          <5>1. ASSUME NEW other \in ValidatorIds
                 PROVE /\ AsyncQueueTyped(asyncCommandQueues'[other])
                       /\ AsyncCommandQueueOwnership(
                            other, asyncCommandQueues'[other])
            <6>1. CASE other = node
              BY <3>2, <3>5, <3>6, <6>1,
                 TypedCandidateAppendPreservesQueueType,
                 AppendOwnedCandidatePreservesCommandQueueOwnership
                 DEF AsyncRuntimeScalarTypeInvariant
            <6>2. CASE other # node
              <7>1. asyncCommandQueues'[other] =
                       asyncCommandQueues[other]
                BY <3>1, <3>2, <3>6, <5>1, <6>2,
                   FunctionalAppendUpdateAwayFromKey
                   DEF AsyncRuntimeScalarTypeInvariant
              <7> QED BY <3>2, <5>1, <7>1
                   DEF AsyncRuntimeScalarTypeInvariant
            <6> QED BY <6>1, <6>2
          <5> QED BY <5>1
        <4> QED BY <4>1, <4>2
      <3>13. AsyncRuntimeScalarTypeInvariant'
        BY <3>2, <3>6, <3>7, <3>8, <3>11, <3>12, Isa
           DEF RunnerServiceFrame, AsyncRuntimeScalarTypeInvariant,
               AsyncConfiguration, AsyncIoVars, AsyncDeferredVars
      <3>14. AsyncCausalTypeInvariant'
        BY <3>1, <3>2, <3>3, <3>6,
           CausalTailUpdatePreservesCausalType
      <3>15. AsyncIoTypeInvariant'
        BY <1>1, <2>3, <3>1, <3>5, <3>6, <3>7,
           EnqueueNonCompletionCandidatePreservesIoType
           DEF AsyncDeferredVars
      <3>16. /\ UNCHANGED AsyncDeferredTopologyTypeVars
              /\ UNCHANGED <<asyncDeferredCompletionQueues,
                             asyncDeferredProgressQueues,
                             asyncDeferredNormalQueues>>
              /\ UNCHANGED AsyncTransportContentTypeVars
              /\ UNCHANGED AsyncIngressTopologyTypeVars
              /\ UNCHANGED asyncIngressLanes
        BY <3>6, <3>7, Isa
           DEF AsyncDeferredVars, AsyncDeferredTopologyTypeVars,
               AsyncTransportContentTypeVars,
               AsyncCertifiedResponseClaimAuthorityVars,
               AsyncIngressTopologyTypeVars, vars
      <3>17. /\ AsyncDeferredTopologyTypeInvariant'
              /\ AsyncDeferredContentTypeInvariant'
              /\ AsyncTransportContentTypeInvariant'
              /\ AsyncIngressTopologyTypeInvariant'
              /\ AsyncIngressCapacityTypeInvariant'
              /\ AsyncIngressContentTypeInvariant'
        BY <3>2, <3>16, AsyncDeferredTopologyTypeStutter,
           AsyncDeferredContentTypeStutter,
           AsyncTransportContentTypeStutter,
           AsyncIngressTopologyTypeStutter,
           AsyncIngressCapacityTypeStutter,
           AsyncIngressContentTypeStutter
      <3>18. AsyncTransportClockTypeInvariant'
        BY <3>1, <3>2, <3>6, <3>8,
           RunnerServiceFramePreservesClockType
      <3> QED BY <1>1, <3>13, <3>14, <3>15, <3>17, <3>18,
                   SelectedLocalAdmissionAdvancePreservesHistoricalRecoveryType
           DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
               AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
               AsyncTransportTypeInvariant, AsyncIngressTypeInvariant
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM LocalAdmissionRunnerPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunNodeWork(node)
    /\ LocalAdmissionStep(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                RunNodeWork(node),
                LocalAdmissionStep(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE /\ LocalAdmissionCanAdvance(node)
                 /\ SelectedLocalSource(node) = "Producer"
      <3>1. SelectedLocalAdmissionAdvance(node)
        BY <1>1, <2>1, LocalAdmissionAdvanceSelectsAtomicWork
      <3> QED BY <1>1, <2>1, <3>1,
           ProducerAdmissionRunnerPreservesSchedulerType
    <2>2. CASE /\ LocalAdmissionCanAdvance(node)
                 /\ SelectedLocalSource(node) = "Causal"
      <3>1. SelectedLocalAdmissionAdvance(node)
        BY <1>1, <2>2, LocalAdmissionAdvanceSelectsAtomicWork
      <3> QED BY <1>1, <2>2, <3>1,
           CausalAdmissionRunnerPreservesSchedulerType
    <2>3. CASE ~LocalAdmissionCanAdvance(node)
      BY <1>1, <2>3,
         LocalAdmissionPhaseAdvancePreservesSchedulerType
    <2> QED BY <2>1, <2>2, <2>3, SMT
         DEF SelectedLocalSource, PreferredLocalSource,
             OtherLocalSource
  <1> QED BY <1>1

THEOREM SerializedLocalPrecedesServeIngressPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunNodeWork(node)
    /\ SerializedLocalPrecedesServeIngressStep(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                RunNodeWork(node),
                SerializedLocalPrecedesServeIngressStep(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. /\ SelectedLocalAdmissionAdvance(node)
           /\ LocalAdmissionCanAdvance(node)
      BY <1>1
         DEF SerializedLocalPrecedesServeIngressStep,
             SelectedLocalAdmissionAdvance,
             AsyncOlderLocalLifecyclePrecedesServeIngress
    <2>2. CASE SelectedLocalSource(node) = "Producer"
      BY <1>1, <2>1, <2>2,
         ProducerAdmissionRunnerPreservesSchedulerType
    <2>3. CASE SelectedLocalSource(node) = "Causal"
      BY <1>1, <2>1, <2>3,
         CausalAdmissionRunnerPreservesSchedulerType
    <2> QED BY <1>1, <2>2, <2>3, SMT
         DEF SelectedLocalSource, PreferredLocalSource,
             OtherLocalSource
  <1> QED BY <1>1

THEOREM SchedulerIoStutterPreservesIoType ==
  /\ AsyncIoTypeInvariant
  /\ UNCHANGED <<asyncCommandQueues, AsyncIoVars,
                  asyncDeferredCompletionQueues>>
  => AsyncIoTypeInvariant'
PROOF
  <1>1. ASSUME AsyncIoTypeInvariant,
              UNCHANGED <<asyncCommandQueues, AsyncIoVars,
                          asyncDeferredCompletionQueues>>
         PROVE AsyncIoTypeInvariant'
    <2>1. /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoContentTypeInvariant
           /\ AsyncIoCapacityTypeInvariant
      BY <1>1 DEF AsyncIoTypeInvariant
    <2>2. /\ UNCHANGED AsyncIoTopologyTypeVars
           /\ UNCHANGED AsyncIoContentTypeVars
           /\ UNCHANGED AsyncIoCapacityTypeVars
      BY <1>1, Isa
         DEF AsyncIoVars, AsyncIoTopologyTypeVars,
             AsyncIoContentTypeVars, AsyncIoCapacityTypeVars
    <2> QED BY <2>1, <2>2,
         AsyncIoTopologyTypeStutter, AsyncIoContentTypeStutter,
         AsyncIoCapacityTypeStutter
         DEF AsyncIoTypeInvariant
  <1> QED BY <1>1

THEOREM TypedIngressDeliveryCandidateFacts ==
  \A node \in ValidatorIds:
    \A item:
      (AsyncTypeInvariant
        /\ AsyncItemTyped(item)
        /\ item.envelope.recipient = node)
      => /\ AsyncCandidateTyped(DeliveryCandidate(item))
         /\ DeliveryCandidate(item).node = node
         /\ DeliveryCandidate(item).class # "Completion"
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW item,
                AsyncTypeInvariant,
                AsyncItemTyped(item),
                item.envelope.recipient = node
         PROVE /\ AsyncCandidateTyped(DeliveryCandidate(item))
               /\ DeliveryCandidate(item).node = node
               /\ DeliveryCandidate(item).class # "Completion"
    <2>1. TypeInvariant
      BY <1>1 DEF AsyncTypeInvariant
    <2>2. AsyncCandidateTyped(DeliveryCandidate(item))
      BY <1>1, <2>1, TypedItemMakesTypedDeliveryCandidate
    <2>3. DeliveryCandidate(item).node = node
      BY <1>1, DeliveryCandidateShape
    <2>4. DeliveryCandidate(item).class = DeliveryClass(item)
      BY DeliveryCandidateShape
    <2>5. DeliveryClass(item) \in {"Normal", "Progress"}
      BY SMT DEF DeliveryClass
    <2> QED BY <2>2, <2>3, <2>4, <2>5
  <1> QED BY <1>1

THEOREM TypedCertifiedResponseCandidateFacts ==
  \A node \in ValidatorIds:
    \A item:
      (AsyncTypeInvariant
        /\ AsyncItemTyped(item)
        /\ item.kind = "CertifiedResponse"
        /\ item.envelope.recipient = node)
      => /\ AsyncCandidateTyped(CertifiedResponseCandidate(item))
         /\ CertifiedResponseCandidate(item).node = node
         /\ CertifiedResponseCandidate(item).class = "Completion"
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW item,
                AsyncTypeInvariant,
                AsyncItemTyped(item),
                item.kind = "CertifiedResponse",
                item.envelope.recipient = node
         PROVE /\ AsyncCandidateTyped(CertifiedResponseCandidate(item))
               /\ CertifiedResponseCandidate(item).node = node
               /\ CertifiedResponseCandidate(item).class = "Completion"
    <2>1. TypeInvariant
      BY <1>1 DEF AsyncTypeInvariant
    <2>2. AsyncCertifiedResponseEnvelopeTyped(item.envelope)
      BY <1>1, Isa DEF AsyncItemTyped
    <2>3. /\ "Completion" \in AsyncCommandClasses
           /\ "FetchCertifiedBody" \in AsyncWorkKinds
      BY Isa DEF AsyncCommandClasses, AsyncWorkKinds, AsyncReducerKinds
    <2>4. DOMAIN CertifiedResponseCandidate(item) = AsyncCandidateDomain
      BY DEF CertifiedResponseCandidate, AsyncCandidate,
             AsyncCandidateWithIdentity, AsyncCandidateDomain
    <2>4a. /\ CertifiedResponseCandidate(item).class = "Completion"
           /\ CertifiedResponseCandidate(item).kind =
                "FetchCertifiedBody"
           /\ CertifiedResponseCandidate(item).node =
                item.envelope.recipient
           /\ CertifiedResponseCandidate(item).height =
                item.envelope.height
           /\ CertifiedResponseCandidate(item).view = item.envelope.view
           /\ CertifiedResponseCandidate(item).subject =
                item.envelope.subject
           /\ CertifiedResponseCandidate(item).item = item
      BY DEF CertifiedResponseCandidate, AsyncCandidate,
             AsyncCandidateWithIdentity
    <2>4b. /\ CertifiedResponseCandidate(item).consumerContext = context
           /\ CertifiedResponseCandidate(item).consumerView =
                nodeView[item.envelope.recipient]
           /\ CertifiedResponseCandidate(item).consumerGeneration =
                generation[item.envelope.recipient]
           /\ CertifiedResponseCandidate(item).evidence = item
      BY DEF CertifiedResponseCandidate, AsyncCandidate,
             AsyncCandidateWithIdentity
    <2>4c. /\ CertifiedResponseCandidate(item).bodyIdentity =
                item.envelope.subject
           /\ CertifiedResponseCandidate(item).manifestIdentity =
                item.envelope.subject
           /\ CertifiedResponseCandidate(item).commitmentIdentity =
                item.envelope.subject
      BY DEF CertifiedResponseCandidate, AsyncCandidate,
             AsyncCandidateWithIdentity
    <2>5. CertifiedResponseCandidate(item).node = node
      BY <1>1, <2>4a
    <2>6. CertifiedResponseCandidate(item).height \in Heights
      BY <2>2, <2>4a
         DEF AsyncCertifiedResponseEnvelopeTyped,
             AsyncReplyRequestItemTyped, AsyncBodyEnvelopeTyped
    <2>7. CertifiedResponseCandidate(item).view \in Views
      BY <2>2, <2>4a
         DEF AsyncCertifiedResponseEnvelopeTyped,
             AsyncReplyRequestItemTyped, AsyncBodyEnvelopeTyped
    <2>8. ValidSubjects \subseteq Subjects
      BY <2>1 DEF TypeInvariant, ModelConfiguration
    <2>9. CertifiedResponseCandidate(item).subject \in SubjectOrNone
      BY <2>2, <2>4a, <2>8
         DEF AsyncCertifiedResponseEnvelopeTyped,
             AsyncReplyRequestItemTyped, AsyncBodyEnvelopeTyped,
             SubjectOrNone
    <2>10. /\ context \in ContextRecords
            /\ nodeView[item.envelope.recipient] \in Views
            /\ generation[item.envelope.recipient] \in Generations
      BY <1>1, <2>1, SMT DEF TypeInvariant
    <2>11. AsyncEvidenceTyped(item)
      BY <1>1 DEF AsyncEvidenceTyped
    <2>12. AsyncCandidateTyped(CertifiedResponseCandidate(item))
      BY <1>1, <2>3, <2>4, <2>4a, <2>4b, <2>4c,
         <2>5, <2>6, <2>7, <2>9, <2>10, <2>11, SMTT(30)
         DEF AsyncCandidateTyped
    <2> QED BY <2>4a, <2>5, <2>12
  <1> QED BY <1>1

THEOREM TypedCommitCertificateResponseCandidateFacts ==
  \A node \in ValidatorIds:
    \A item:
      (AsyncTypeInvariant
        /\ AsyncItemTyped(item)
        /\ item.kind = "CommitCertificateResponse"
        /\ item.envelope.recipient = node)
      => /\ AsyncItemTyped(DiscoveredCommitQcItem(item))
         /\ AsyncCandidateTyped(
              CommitCertificateResponseCandidate(item))
         /\ CommitCertificateResponseCandidate(item).node = node
         /\ CommitCertificateResponseCandidate(item).class # "Completion"
BY SMTT(90)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncItemTyped, AsyncCommitCertificateResponseEnvelopeTyped,
       AsyncReplyRequestItemTyped, AsyncBodyEnvelopeTyped,
       HistoricalCommitQcSigner, DiscoveredCommitQcItem,
       CommitCertificateResponseCandidate, AsyncCandidateAtConsumer,
       AsyncCandidateWithIdentity, AsyncCandidateTyped,
       AsyncCandidateDomain, AsyncEvidenceTyped, AsyncNetworkItem,
       QcEnvelope, QcEnvelopeSet, DeliveryClass, DeliveryKind,
       DeliveryHeight, DeliveryView, AsyncNetworkKinds,
       AsyncIngressSources, AsyncCommandClasses, AsyncWorkKinds,
       AsyncDeliveryKinds, SubjectOrNone, ValidatorIds,
       TypeInvariant, ModelConfiguration, QuorumConfiguration

THEOREM CommitCertificateCandidatePreservesOuterResponseEvidence ==
  \A item:
    /\ CommitCertificateResponseCandidate(item).class = "Progress"
    /\ CommitCertificateResponseCandidate(item).kind = "DeliverQC"
    /\ CommitCertificateResponseCandidate(item).item =
         DiscoveredCommitQcItem(item)
    /\ CommitCertificateResponseCandidate(item).evidence = item
    /\ CommitCertificateResponseCandidate(item).item.source =
         HistoricalCommitQcSigner(item)
BY DEF CommitCertificateResponseCandidate, DiscoveredCommitQcItem,
       AsyncNetworkItem, DeliveryClass, DeliveryKind,
       AsyncCandidateAtConsumer, AsyncCandidateWithIdentity

THEOREM TypedCommitCertificateMatchingIsExactOutstandingRequest ==
  \A item:
    /\ AsyncItemTyped(item)
    /\ item.kind = "CommitCertificateResponse"
    => MatchingCommitCertificateRequests(item) =
         {request \in asyncActiveRequests:
            /\ request.kind = "CommitCertificateRequest"
            /\ AsyncCommitCertificateRequestRegistrationIdentity(request)
                 = AsyncCommitCertificateRequestRegistrationIdentity(
                     item.envelope.request)}
BY SMT DEF MatchingCommitCertificateRequests,
           AsyncItemTyped,
           AsyncCommitCertificateResponseEnvelopeTyped,
           AsyncReplyRequestItemTyped

THEOREM TypedCertifiedMatchingIsExactOutstandingRequest ==
  \A item:
    /\ AsyncItemTyped(item)
    /\ item.kind = "CertifiedResponse"
    => MatchingCertifiedRequests(item) =
         {request \in asyncActiveRequests:
            /\ request.kind = "CertifiedRequest"
            /\ AsyncCertifiedRequestHash(request) =
                 item.envelope.requestHash}
BY SMT DEF MatchingCertifiedRequests, AsyncItemTyped,
           AsyncCertifiedResponseEnvelopeTyped,
           AsyncReplyRequestItemTyped, AsyncCertifiedRequestHash

THEOREM RemoveRequestsAndAddSentPreservesTransportContentType ==
  \A removed, additions:
    /\ AsyncTransportContentTypeInvariant
    /\ IsFiniteSet(additions)
    /\ \A item \in additions: AsyncItemTyped(item)
    /\ asyncSentItems' = asyncSentItems \cup additions
    /\ asyncActiveRequests' = asyncActiveRequests \ removed
    /\ asyncRetainedControl' = asyncRetainedControl
    /\ AsyncCertifiedResponseClaimInvariant'
    /\ UNCHANGED
         <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
           asyncTransport, asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW removed, NEW additions,
                AsyncTransportContentTypeInvariant,
                IsFiniteSet(additions),
                \A item \in additions: AsyncItemTyped(item),
                asyncSentItems' = asyncSentItems \cup additions,
                asyncActiveRequests' = asyncActiveRequests \ removed,
                asyncRetainedControl' = asyncRetainedControl,
                AsyncCertifiedResponseClaimInvariant',
                UNCHANGED
                  <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
                    asyncTransport, asyncHeldChunks>>
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
    <2>2. AsyncSentItemsType(asyncSentItems')
      <3>1. IsFiniteSet(asyncSentItems')
        BY <1>1, <2>1, FS_Union DEF AsyncSentItemsType
      <3>2. \A item \in asyncSentItems': AsyncItemTyped(item)
        BY <1>1, <2>1 DEF AsyncSentItemsType
      <3> QED BY <3>1, <3>2 DEF AsyncSentItemsType
    <2>3. CurrentVoters' = CurrentVoters
      BY <1>1, Isa DEF CurrentVoters, CurrentEpoch
    <2>4. AsyncRetainedControlType(
             asyncRetainedControl', CurrentVoters')
      BY <1>1, <2>1, <2>3
    <2>5. AsyncActiveRequestsType(
             asyncActiveRequests', asyncSentItems')
      <3>1. asyncActiveRequests' \subseteq asyncActiveRequests
        BY <1>1
      <3>2. IsFiniteSet(asyncActiveRequests')
        BY <2>1, <3>1, FS_Subset DEF AsyncActiveRequestsType
      <3>3. /\ asyncActiveRequests' \subseteq asyncSentItems'
             /\ \A item \in asyncActiveRequests':
                  /\ AsyncItemTyped(item)
                  /\ item.kind \in {"CertifiedRequest",
                                      "CommitCertificateRequest"}
        BY <1>1, <2>1, <3>1 DEF AsyncActiveRequestsType
      <3>4. AsyncCertifiedRequestLogicalIndexConsistent(
               asyncActiveRequests')
        BY <2>1, <3>1,
           CertifiedRequestLogicalIndexConsistencyIsDownwardClosed
           DEF AsyncActiveRequestsType
      <3> QED BY <3>2, <3>3, <3>4 DEF AsyncActiveRequestsType
    <2>6. AsyncTransportHistoryTypeInvariant'
      BY <1>1, <2>2, <2>4, <2>5,
         AsyncTransportHistoryTypeDecomposition
    <2>7. /\ AsyncPacketContentTypeInvariant'
           /\ AsyncHeldChunksTypeInvariant'
      BY <1>1, <2>1
         DEF AsyncPacketContentTypeInvariant,
             AsyncHeldChunksTypeInvariant
    <2> QED BY <2>6, <2>7
         DEF AsyncTransportContentTypeInvariant
  <1> QED BY <1>1

SelectedDrainItem(node) ==
  SelectedIngressItemAt(node, FirstDrainableIngressIndex(node))

SelectedDrainCandidate(node) ==
  DeliveryCandidate(SelectedDrainItem(node))

SelectedDrainCertifiedCandidate(node) ==
  CertifiedResponseCandidate(SelectedDrainItem(node))

SelectedDrainCommitCandidate(node) ==
  CommitCertificateResponseCandidate(SelectedDrainItem(node))

THEOREM CommitCertificateResponseCandidateHasProgressClass ==
  \A item:
    CommitCertificateResponseCandidate(item).class = "Progress"
PROOF
  <1>1. ASSUME NEW item
         PROVE CommitCertificateResponseCandidate(item).class = "Progress"
    <2>1. DiscoveredCommitQcItem(item).kind = "CommitQC"
      BY DEF DiscoveredCommitQcItem, AsyncNetworkItem
    <2>2. DeliveryClass(DiscoveredCommitQcItem(item)) = "Progress"
      BY <2>1 DEF DeliveryClass
    <2> QED BY <2>2
         DEF CommitCertificateResponseCandidate,
             AsyncCandidateAtConsumer, AsyncCandidateWithIdentity
  <1> QED BY <1>1

THEOREM FreshAuthorizedCommitResponseCommandFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ SelectedDrainItem(node).kind = "CommitCertificateResponse"
    /\ SelectedDrainItem(node) \in asyncSentItems
    /\ CommitCertificateResponseAuthorized(SelectedDrainItem(node))
    /\ ~CandidateScheduled(SelectedDrainCommitCandidate(node))
    => /\ UNCHANGED asyncNextCommandClass
       /\ asyncCommandQueues' =
            [asyncCommandQueues EXCEPT
               ![SelectedDrainCommitCandidate(node).node] =
                 Append(@, SelectedDrainCommitCandidate(node))]
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                SelectedDrainItem(node).kind =
                  "CommitCertificateResponse",
                SelectedDrainItem(node) \in asyncSentItems,
                CommitCertificateResponseAuthorized(
                  SelectedDrainItem(node)),
                ~CandidateScheduled(
                  SelectedDrainCommitCandidate(node))
         PROVE /\ UNCHANGED asyncNextCommandClass
               /\ asyncCommandQueues' =
                    [asyncCommandQueues EXCEPT
                       ![SelectedDrainCommitCandidate(node).node] =
                         Append(@, SelectedDrainCommitCandidate(node))]
    <2>1. SelectedDrainItem(node).kind # "Noise"
      BY <1>1
    <2>2. SelectedDrainItem(node).kind
             \notin {"CertifiedRequest", "CommitCertificateRequest"}
      BY <1>1
    <2>3. SelectedDrainItem(node).kind # "CertifiedResponse"
      BY <1>1
    <2> QED BY <1>1, <2>1, <2>2, <2>3, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem,
             SelectedDrainCommitCandidate, EnqueueCandidate
  <1> QED BY <1>1

THEOREM ScheduledAuthorizedCommitResponseCommandFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ SelectedDrainItem(node).kind = "CommitCertificateResponse"
    /\ SelectedDrainItem(node) \in asyncSentItems
    /\ CommitCertificateResponseAuthorized(SelectedDrainItem(node))
    /\ CandidateScheduled(SelectedDrainCommitCandidate(node))
    => UNCHANGED <<asyncCommandQueues, asyncNextCommandClass>>
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                SelectedDrainItem(node).kind =
                  "CommitCertificateResponse",
                SelectedDrainItem(node) \in asyncSentItems,
                CommitCertificateResponseAuthorized(
                  SelectedDrainItem(node)),
                CandidateScheduled(
                  SelectedDrainCommitCandidate(node))
         PROVE UNCHANGED <<asyncCommandQueues,
                            asyncNextCommandClass>>
    <2>1. SelectedDrainItem(node).kind # "Noise"
      BY <1>1
    <2>2. SelectedDrainItem(node).kind
             \notin {"CertifiedRequest", "CommitCertificateRequest"}
      BY <1>1
    <2>3. SelectedDrainItem(node).kind # "CertifiedResponse"
      BY <1>1
    <2> QED BY <1>1, <2>1, <2>2, <2>3, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem,
             SelectedDrainCommitCandidate, EnqueueCandidate
  <1> QED BY <1>1

THEOREM RejectedCommitResponseCommandFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ SelectedDrainItem(node).kind = "CommitCertificateResponse"
    /\ SelectedDrainItem(node) \in asyncSentItems
    /\ ~CommitCertificateResponseAuthorized(SelectedDrainItem(node))
    => /\ UNCHANGED asyncNextCommandClass
       /\ asyncCommandQueues' = asyncCommandQueues
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                SelectedDrainItem(node).kind =
                  "CommitCertificateResponse",
                SelectedDrainItem(node) \in asyncSentItems,
                ~CommitCertificateResponseAuthorized(
                  SelectedDrainItem(node))
         PROVE /\ UNCHANGED asyncNextCommandClass
               /\ asyncCommandQueues' = asyncCommandQueues
    <2>1. SelectedDrainItem(node).kind # "Noise"
      BY <1>1
    <2>2. SelectedDrainItem(node).kind
             \notin {"CertifiedRequest", "CommitCertificateRequest"}
      BY <1>1
    <2>3. SelectedDrainItem(node).kind # "CertifiedResponse"
      BY <1>1
    <2> QED BY <1>1, <2>1, <2>2, <2>3, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem,
             SelectedDrainCommitCandidate, EnqueueCandidate
  <1> QED BY <1>1

THEOREM FreshAuthorizedCertifiedResponseSchedulerFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ CertifiedResponseClaimAuthorized(SelectedDrainItem(node))
    /\ ~CandidateScheduled(
         CertifiedResponseCandidate(SelectedDrainItem(node)))
    => /\ asyncCommandQueues' =
             [asyncCommandQueues EXCEPT
                ![node] = Append(
                  @, CertifiedResponseCandidate(SelectedDrainItem(node)))]
       /\ UNCHANGED <<AsyncIoVars, asyncNextCommandClass>>
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                CertifiedResponseClaimAuthorized(SelectedDrainItem(node)),
                ~CandidateScheduled(
                  CertifiedResponseCandidate(SelectedDrainItem(node)))
         PROVE /\ asyncCommandQueues' =
                    [asyncCommandQueues EXCEPT
                       ![node] = Append(
                         @, CertifiedResponseCandidate(
                              SelectedDrainItem(node)))]
               /\ UNCHANGED <<AsyncIoVars, asyncNextCommandClass>>
    <2>1. SelectedDrainItem(node).kind = "CertifiedResponse"
      BY <1>1
         DEF CertifiedResponseClaimAuthorized,
             CertifiedResponseAuthorized
    <2>2. SelectedDrainItem(node).kind # "Noise"
      BY <2>1
    <2>3. SelectedDrainItem(node).kind
             \notin {"CertifiedRequest", "CommitCertificateRequest"}
      BY <2>1
    <2> QED BY <1>1, <2>1, <2>2, <2>3, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem,
             EnqueueCandidate, vars
  <1> QED BY <1>1

THEOREM ScheduledAuthorizedCertifiedResponseSchedulerFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ CertifiedResponseClaimAuthorized(SelectedDrainItem(node))
    /\ CandidateScheduled(
         CertifiedResponseCandidate(SelectedDrainItem(node)))
    => UNCHANGED <<AsyncIoVars, asyncCommandQueues,
                   asyncNextCommandClass>>
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                CertifiedResponseClaimAuthorized(SelectedDrainItem(node)),
                CandidateScheduled(
                  CertifiedResponseCandidate(SelectedDrainItem(node)))
         PROVE UNCHANGED <<AsyncIoVars, asyncCommandQueues,
                            asyncNextCommandClass>>
    <2>1. SelectedDrainItem(node).kind = "CertifiedResponse"
      BY <1>1
         DEF CertifiedResponseClaimAuthorized,
             CertifiedResponseAuthorized
    <2>2. SelectedDrainItem(node).kind # "Noise"
      BY <2>1
    <2>3. SelectedDrainItem(node).kind
             \notin {"CertifiedRequest", "CommitCertificateRequest"}
      BY <2>1
    <2> QED BY <1>1, <2>1, <2>2, <2>3, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem, vars
  <1> QED BY <1>1

THEOREM DrainFairIngressRuntimeFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ RunnerServiceFrame(node)
    => /\ UNCHANGED asyncNextCommandClass
       /\ UNCHANGED <<asyncNow, asyncFifoOwed,
                      asyncTimeoutEmitted>>
       /\ \/ asyncCommandQueues' = asyncCommandQueues
          \/ asyncCommandQueues' =
               [asyncCommandQueues EXCEPT
                  ![SelectedDrainCandidate(node).node] =
                    Append(@, SelectedDrainCandidate(node))]
          \/ /\ CertifiedResponseClaimAuthorized(
                   SelectedDrainItem(node))
                /\ asyncCommandQueues' =
                     [asyncCommandQueues EXCEPT
                        ![SelectedDrainCertifiedCandidate(node).node] =
                          Append(
                            @, SelectedDrainCertifiedCandidate(node))]
          \/ /\ SelectedDrainItem(node) \in asyncSentItems
                /\ CommitCertificateResponseAuthorized(
                     SelectedDrainItem(node))
                /\ asyncCommandQueues' =
                     [asyncCommandQueues EXCEPT
                        ![SelectedDrainCommitCandidate(node).node] = Append(
                          @, SelectedDrainCommitCandidate(node))]
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                RunnerServiceFrame(node)
         PROVE /\ UNCHANGED asyncNextCommandClass
                /\ UNCHANGED <<asyncNow, asyncFifoOwed,
                              asyncTimeoutEmitted>>
                /\ \/ asyncCommandQueues' = asyncCommandQueues
                   \/ asyncCommandQueues' =
                        [asyncCommandQueues EXCEPT
                           ![SelectedDrainCandidate(node).node] = Append(
                             @, SelectedDrainCandidate(node))]
                   \/ /\ CertifiedResponseClaimAuthorized(
                            SelectedDrainItem(node))
                         /\ asyncCommandQueues' =
                              [asyncCommandQueues EXCEPT
                                 ![SelectedDrainCertifiedCandidate(node).node] =
                                   Append(
                                     @,
                                     SelectedDrainCertifiedCandidate(node))]
                   \/ /\ SelectedDrainItem(node) \in asyncSentItems
                         /\ CommitCertificateResponseAuthorized(
                              SelectedDrainItem(node))
                         /\ asyncCommandQueues' =
                              [asyncCommandQueues EXCEPT
                                 ![SelectedDrainCommitCandidate(node).node] =
                                   Append(
                                   @, SelectedDrainCommitCandidate(node))]
    <2>1. /\ UNCHANGED asyncNextCommandClass
           /\ UNCHANGED <<asyncNow, asyncFifoOwed,
                           asyncTimeoutEmitted>>
      BY <1>1, Isa
         DEF DrainFairIngressSelected, SelectedDrainItem,
             SelectedDrainCandidate, SelectedDrainCertifiedCandidate,
             SelectedDrainCommitCandidate,
             EnqueueCandidate, RunnerServiceFrame, vars
    <2>2. \/ asyncCommandQueues' = asyncCommandQueues
           \/ asyncCommandQueues' =
                [asyncCommandQueues EXCEPT
                   ![SelectedDrainCandidate(node).node] = Append(
                     @, SelectedDrainCandidate(node))]
           \/ /\ CertifiedResponseClaimAuthorized(
                    SelectedDrainItem(node))
                 /\ asyncCommandQueues' =
                      [asyncCommandQueues EXCEPT
                         ![SelectedDrainCertifiedCandidate(node).node] =
                           Append(
                             @, SelectedDrainCertifiedCandidate(node))]
           \/ /\ SelectedDrainItem(node) \in asyncSentItems
                 /\ CommitCertificateResponseAuthorized(
                      SelectedDrainItem(node))
                 /\ asyncCommandQueues' =
                      [asyncCommandQueues EXCEPT
                         ![SelectedDrainCommitCandidate(node).node] = Append(
                           @, SelectedDrainCommitCandidate(node))]
      <3>1. CASE
        \/ SelectedDrainItem(node).kind = "Noise"
        \/ SelectedDrainItem(node) \notin asyncSentItems
        BY <1>1, <3>1, Isa
           DEF DrainFairIngressSelected, SelectedDrainItem,
               SelectedDrainCandidate, SelectedDrainCertifiedCandidate,
               SelectedDrainCommitCandidate,
               EnqueueCandidate
      <3>2. CASE
        /\ SelectedDrainItem(node).kind # "Noise"
        /\ SelectedDrainItem(node) \in asyncSentItems
        /\ SelectedDrainItem(node).kind
             \in {"CertifiedRequest", "CommitCertificateRequest"}
        BY <1>1, <3>2, Isa
           DEF DrainFairIngressSelected, SelectedDrainItem,
               SelectedDrainCandidate, SelectedDrainCertifiedCandidate,
               SelectedDrainCommitCandidate,
               EnqueueCandidate
      <3>3. CASE
        /\ SelectedDrainItem(node).kind # "Noise"
        /\ SelectedDrainItem(node) \in asyncSentItems
        /\ SelectedDrainItem(node).kind = "CertifiedResponse"
        <4>1. CASE CertifiedResponseClaimAuthorized(
                      SelectedDrainItem(node))
          <5>1. CASE CandidateScheduled(
                       SelectedDrainCertifiedCandidate(node))
            BY <1>1, <3>3, <4>1, <5>1, Isa
               DEF DrainFairIngressSelected, SelectedDrainItem,
                   SelectedDrainCandidate, SelectedDrainCertifiedCandidate,
                   EnqueueCandidate, CertifiedResponseClaimAuthorized,
                   CertifiedResponseAuthorized, vars
          <5>2. CASE ~CandidateScheduled(
                       SelectedDrainCertifiedCandidate(node))
            BY <1>1, <3>3, <4>1, <5>2, Isa
               DEF DrainFairIngressSelected, SelectedDrainItem,
                   SelectedDrainCandidate, SelectedDrainCertifiedCandidate,
                   EnqueueCandidate, CertifiedResponseClaimAuthorized,
                   CertifiedResponseAuthorized, vars
          <5> QED BY <5>1, <5>2
        <4>2. CASE ~CertifiedResponseClaimAuthorized(
                      SelectedDrainItem(node))
          BY <1>1, <3>3, <4>2, Isa
             DEF DrainFairIngressSelected, SelectedDrainItem,
                 SelectedDrainCandidate, SelectedDrainCertifiedCandidate,
                 SelectedDrainCommitCandidate, EnqueueCandidate
        <4> QED BY <4>1, <4>2
      <3>4. CASE
        /\ SelectedDrainItem(node).kind # "Noise"
        /\ SelectedDrainItem(node) \in asyncSentItems
        /\ SelectedDrainItem(node).kind = "CommitCertificateResponse"
        <4>1. CASE CommitCertificateResponseAuthorized(
                      SelectedDrainItem(node))
          <5>1. CASE CandidateScheduled(
                       SelectedDrainCommitCandidate(node))
            BY <1>1, <3>4, <4>1, <5>1,
               ScheduledAuthorizedCommitResponseCommandFrame
          <5>2. CASE ~CandidateScheduled(
                       SelectedDrainCommitCandidate(node))
            BY <1>1, <3>4, <4>1, <5>2,
               FreshAuthorizedCommitResponseCommandFrame
          <5> QED BY <5>1, <5>2
        <4>2. CASE ~CommitCertificateResponseAuthorized(
                      SelectedDrainItem(node))
          BY <1>1, <3>4, <4>2,
             RejectedCommitResponseCommandFrame
        <4> QED BY <4>1, <4>2
      <3>5. CASE
        /\ SelectedDrainItem(node).kind # "Noise"
        /\ SelectedDrainItem(node) \in asyncSentItems
        /\ SelectedDrainItem(node).kind
             \notin {"CertifiedRequest", "CommitCertificateRequest"}
        /\ SelectedDrainItem(node).kind # "CertifiedResponse"
        /\ SelectedDrainItem(node).kind # "CommitCertificateResponse"
        <4>1. CASE CandidateScheduled(SelectedDrainCandidate(node))
          BY <1>1, <3>5, <4>1, Isa
             DEF DrainFairIngressSelected, SelectedDrainItem,
                 SelectedDrainCandidate, SelectedDrainCertifiedCandidate,
                 SelectedDrainCommitCandidate,
                 EnqueueCandidate
        <4>2. CASE ~CandidateScheduled(SelectedDrainCandidate(node))
          BY <1>1, <3>5, <4>2, Isa
             DEF DrainFairIngressSelected, SelectedDrainItem,
                 SelectedDrainCandidate, SelectedDrainCertifiedCandidate,
                 SelectedDrainCommitCandidate,
                 EnqueueCandidate
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5, SMT
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM AuthorizedIngressServeFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ SelectedDrainItem(node) \in asyncSentItems
    /\ SelectedDrainItem(node).kind
         \in {"CertifiedRequest", "CommitCertificateRequest"}
    /\ IF SelectedDrainItem(node).kind = "CertifiedRequest"
       THEN CertifiedRequestAuthorized(SelectedDrainItem(node))
       ELSE CommitCertificateRequestAuthorized(SelectedDrainItem(node))
    => /\ asyncIoQueues' =
             [asyncIoQueues EXCEPT
                ![node] = Append(
                  @, AsyncIoCertifiedServeJob(
                       node, SelectedDrainCandidate(node)))]
       /\ UNCHANGED <<asyncOutstandingWork,
                       asyncIoReadyCompletions,
                       asyncLocalReadyCompletions,
                       asyncNextCompletionSource,
                       asyncIoControlAvailable,
                       asyncCommandQueues,
                       asyncNextCommandClass,
                       asyncSentItems,
                       asyncRetainedControl,
                       asyncActiveRequests>>
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                SelectedDrainItem(node) \in asyncSentItems,
                SelectedDrainItem(node).kind
                  \in {"CertifiedRequest", "CommitCertificateRequest"},
                IF SelectedDrainItem(node).kind = "CertifiedRequest"
                THEN CertifiedRequestAuthorized(SelectedDrainItem(node))
                ELSE CommitCertificateRequestAuthorized(
                       SelectedDrainItem(node))
         PROVE /\ asyncIoQueues' =
                    [asyncIoQueues EXCEPT
                       ![node] = Append(
                         @, AsyncIoCertifiedServeJob(
                              node, SelectedDrainCandidate(node)))]
               /\ UNCHANGED <<asyncOutstandingWork,
                               asyncIoReadyCompletions,
                               asyncLocalReadyCompletions,
                               asyncNextCompletionSource,
                               asyncIoControlAvailable,
                               asyncCommandQueues,
                               asyncNextCommandClass,
                               asyncSentItems,
                               asyncRetainedControl,
                               asyncActiveRequests>>
    <2>1. SelectedDrainItem(node).kind # "Noise"
      BY <1>1
    <2> QED BY <1>1, <2>1, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem,
             SelectedDrainCandidate
  <1> QED BY <1>1

THEOREM AuthorizedCertifiedResponseFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ CertifiedResponseClaimAuthorized(SelectedDrainItem(node))
    => /\ asyncActiveRequests' =
            asyncActiveRequests \
              MatchingCertifiedRequests(SelectedDrainItem(node))
       /\ asyncCertifiedResponseClaim' =
            CertifiedResponseClaimForRequests(asyncActiveRequests')
       /\ UNCHANGED <<asyncSentItems, asyncRetainedControl,
                       context,
                       AsyncCertifiedResponseClaimCoreAuthorityVars,
                       asyncTransport, asyncHeldChunks>>
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                CertifiedResponseClaimAuthorized(SelectedDrainItem(node))
         PROVE /\ asyncActiveRequests' =
                    asyncActiveRequests \
                      MatchingCertifiedRequests(SelectedDrainItem(node))
               /\ asyncCertifiedResponseClaim' =
                    CertifiedResponseClaimForRequests(
                      asyncActiveRequests')
               /\ UNCHANGED <<asyncSentItems, asyncRetainedControl,
                               context,
                               AsyncCertifiedResponseClaimCoreAuthorityVars,
                               asyncTransport, asyncHeldChunks>>
    <2>1. SelectedDrainItem(node).kind = "CertifiedResponse"
      BY <1>1
         DEF CertifiedResponseClaimAuthorized,
             CertifiedResponseAuthorized
    <2>2. SelectedDrainItem(node).kind # "Noise"
      BY <2>1
    <2>3. SelectedDrainItem(node).kind
             \notin {"CertifiedRequest", "CommitCertificateRequest"}
      BY <2>1
    <2> QED BY <1>1, <2>1, <2>2, <2>3, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem,
             AsyncCertifiedResponseClaimCoreAuthorityVars, vars
  <1> QED BY <1>1

THEOREM DrainFairIngressSelectedClaimPopShape ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ DrainFairIngressSelected(node)
    => LET item == SelectedDrainItem(node)
       IN asyncCertifiedResponseClaim' =
            IF item.kind = "CertifiedResponse"
                 /\ CertifiedResponseClaimMatches(item)
            THEN CertifiedResponseClaimForRequests(asyncActiveRequests')
            ELSE asyncCertifiedResponseClaim
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                DrainFairIngressSelected(node)
         PROVE LET item == SelectedDrainItem(node)
               IN asyncCertifiedResponseClaim' =
                    IF item.kind = "CertifiedResponse"
                         /\ CertifiedResponseClaimMatches(item)
                    THEN CertifiedResponseClaimForRequests(
                           asyncActiveRequests')
                    ELSE asyncCertifiedResponseClaim
    <2> DEFINE Item == SelectedDrainItem(node)
    <2>1. AsyncCertifiedResponseClaimInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant
    <2>2. CASE /\ Item.kind = "CertifiedResponse"
                 /\ CertifiedResponseClaimMatches(Item)
      <3>1. CertifiedResponseClaimAuthorized(Item)
        BY <2>1, <2>2,
           MatchingClaimedCertifiedResponseIsAuthorized
      <3> QED BY <1>1, <2>2, <3>1, SMTT(60), Isa
           DEF DrainFairIngressSelected, SelectedDrainItem, Item
    <2>3. CASE ~(Item.kind = "CertifiedResponse"
                   /\ CertifiedResponseClaimMatches(Item))
      <3>1. ~CertifiedResponseClaimAuthorized(Item)
        BY <2>3 DEF CertifiedResponseClaimAuthorized
      <3> QED BY <1>1, <2>3, <3>1, SMTT(60), Isa
           DEF DrainFairIngressSelected, SelectedDrainItem, Item
    <2> QED BY <2>2, <2>3 DEF Item
  <1> QED BY <1>1

THEOREM DrainFairIngressSelectedPreservesClaimIngressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ DrainFairIngressSelected(node)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                AsyncCertifiedResponseClaimIngressOwnershipInvariant,
                DrainFairIngressSelected(node)
         PROVE AsyncCertifiedResponseClaimIngressOwnershipInvariant'
    <2> DEFINE DrainIndex == FirstDrainableIngressIndex(node)
    <2> DEFINE DrainSource == asyncIngressReady[node][DrainIndex]
    <2> DEFINE DrainLaneIndex ==
          SelectedIngressLaneIndex(node, DrainIndex)
    <2> DEFINE DrainItem ==
          IngressLane(node, DrainSource)[DrainLaneIndex]
    <2>1. /\ AsyncIngressTypeInvariant
           /\ DrainIndex \in DrainableIngressIndices(node)
      BY <1>1, FirstDrainableIngressIndexIsDrainable
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             DrainIndex
    <2>2. /\ DrainIndex \in 1..Len(asyncIngressReady[node])
           /\ DrainSource \in AsyncIngressSources
           /\ DrainLaneIndex \in
                1..IngressLaneDepth(node, DrainSource)
      BY <2>1, FirstDrainableIngressLaneIndexIsDrainable, SMT
         DEF DrainableIngressIndices, IngressSourceCanDrain,
             DrainableIngressLaneIndices, SelectedIngressLaneIndex,
             DrainSource, DrainLaneIndex
    <2>3. /\ asyncIngressLanes' =
                  [asyncIngressLanes EXCEPT
                     ![node][DrainSource] =
                       SequenceWithoutIndex(@, DrainLaneIndex)]
           /\ DrainItem = SelectedDrainItem(node)
      BY <1>1, <2>2, Isa
         DEF DrainFairIngressSelected, PopSelectedIngress,
             SelectedDrainItem, SelectedIngressItemAt,
             DrainIndex, DrainSource, DrainLaneIndex, DrainItem
    <2>4. /\ asyncCertifiedResponseClaim'
                  \subseteq asyncCertifiedResponseClaim
           /\ (DrainItem.kind = "CertifiedResponse"
                 /\ CertifiedResponseClaimMatches(DrainItem)
                 => AsyncCertifiedResponseCanonicalWireIdentity(DrainItem)
                      \notin asyncCertifiedResponseClaim')
      BY <1>1, <2>3, DrainFairIngressSelectedClaimPopShape,
         SMTT(60), Isa
         DEF DrainItem, CertifiedResponseClaimForRequests,
             MatchingCertifiedRequests, ActiveCertifiedRequestHashesIn,
             AsyncCertifiedRequestHash,
             CertifiedResponseClaimMatches,
             AsyncCertifiedResponseCanonicalWireIdentity
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4,
         PopIngressLanePreservesCertifiedResponseClaimIngressOwnership
         DEF DrainItem
  <1> QED BY <1>1

THEOREM AuthorizedCommitResponseFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ SelectedDrainItem(node) \in asyncSentItems
    /\ CommitCertificateResponseAuthorized(SelectedDrainItem(node))
    => /\ UNCHANGED AsyncIoVars
       /\ asyncActiveRequests' =
            asyncActiveRequests \
              MatchingCommitCertificateRequests(SelectedDrainItem(node))
       /\ asyncSentItems' =
            asyncSentItems \cup
              {DiscoveredCommitQcItem(SelectedDrainItem(node))}
       /\ UNCHANGED <<asyncRetainedControl,
                       asyncCertifiedResponseClaim,
                       context,
                       AsyncCertifiedResponseClaimCoreAuthorityVars,
                       asyncTransport, asyncHeldChunks>>
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                SelectedDrainItem(node) \in asyncSentItems,
                CommitCertificateResponseAuthorized(SelectedDrainItem(node))
         PROVE /\ UNCHANGED AsyncIoVars
               /\ asyncActiveRequests' =
                    asyncActiveRequests \
                      MatchingCommitCertificateRequests(
                        SelectedDrainItem(node))
               /\ asyncSentItems' =
                    asyncSentItems \cup
                      {DiscoveredCommitQcItem(SelectedDrainItem(node))}
               /\ UNCHANGED <<asyncRetainedControl,
                               asyncCertifiedResponseClaim,
                               context,
                               AsyncCertifiedResponseClaimCoreAuthorityVars,
                               asyncTransport, asyncHeldChunks>>
    <2>1. SelectedDrainItem(node).kind = "CommitCertificateResponse"
      BY <1>1 DEF CommitCertificateResponseAuthorized
    <2>2. SelectedDrainItem(node).kind # "Noise"
      BY <2>1
    <2>3. SelectedDrainItem(node).kind
             \notin {"CertifiedRequest", "CommitCertificateRequest"}
      BY <2>1
    <2>4. SelectedDrainItem(node).kind # "CertifiedResponse"
      BY <2>1
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem,
             AsyncCertifiedResponseClaimCoreAuthorityVars, vars
  <1> QED BY <1>1

THEOREM OrdinaryScheduledIngressFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ SelectedDrainItem(node) \in asyncSentItems
    /\ SelectedDrainItem(node).kind # "Noise"
    /\ SelectedDrainItem(node).kind
         \notin {"CertifiedRequest", "CommitCertificateRequest"}
    /\ SelectedDrainItem(node).kind # "Chunk"
    /\ SelectedDrainItem(node).kind # "CertifiedResponse"
    /\ SelectedDrainItem(node).kind # "CommitCertificateResponse"
    /\ CandidateScheduled(SelectedDrainCandidate(node))
    => UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                   AsyncIoVars>>
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                SelectedDrainItem(node) \in asyncSentItems,
                SelectedDrainItem(node).kind # "Noise",
                SelectedDrainItem(node).kind
                  \notin {"CertifiedRequest", "CommitCertificateRequest"},
                SelectedDrainItem(node).kind # "Chunk",
                SelectedDrainItem(node).kind # "CertifiedResponse",
                SelectedDrainItem(node).kind #
                  "CommitCertificateResponse",
                CandidateScheduled(SelectedDrainCandidate(node))
         PROVE UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                            AsyncIoVars>>
    <2> QED BY <1>1, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem,
             SelectedDrainCandidate
  <1> QED BY <1>1

THEOREM OrdinaryFreshIngressFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ IngressItemCanDrain(node, SelectedDrainItem(node))
    /\ SelectedDrainItem(node) \in asyncSentItems
    /\ SelectedDrainItem(node).kind # "Noise"
    /\ SelectedDrainItem(node).kind
         \notin {"CertifiedRequest", "CommitCertificateRequest"}
    /\ SelectedDrainItem(node).kind # "Chunk"
    /\ SelectedDrainItem(node).kind # "CertifiedResponse"
    /\ SelectedDrainItem(node).kind # "CommitCertificateResponse"
    /\ ~CandidateScheduled(SelectedDrainCandidate(node))
    /\ SelectedDrainCandidate(node).node = node
    => /\ CanEnqueueClass(node, SelectedDrainCandidate(node).class)
       /\ asyncCommandQueues' =
            [asyncCommandQueues EXCEPT
               ![node] = Append(@, SelectedDrainCandidate(node))]
       /\ UNCHANGED AsyncIoVars
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                IngressItemCanDrain(node, SelectedDrainItem(node)),
                SelectedDrainItem(node) \in asyncSentItems,
                SelectedDrainItem(node).kind # "Noise",
                SelectedDrainItem(node).kind
                  \notin {"CertifiedRequest", "CommitCertificateRequest"},
                SelectedDrainItem(node).kind # "Chunk",
                SelectedDrainItem(node).kind # "CertifiedResponse",
                SelectedDrainItem(node).kind #
                  "CommitCertificateResponse",
                ~CandidateScheduled(SelectedDrainCandidate(node)),
                SelectedDrainCandidate(node).node = node
         PROVE /\ CanEnqueueClass(
                      node, SelectedDrainCandidate(node).class)
               /\ asyncCommandQueues' =
                    [asyncCommandQueues EXCEPT
                       ![node] = Append(@, SelectedDrainCandidate(node))]
               /\ UNCHANGED AsyncIoVars
    <2> QED BY <1>1, SMTT(60)
         DEF DrainFairIngressSelected, IngressItemCanDrain,
             SelectedDrainItem, SelectedDrainCandidate,
             EnqueueCandidate
  <1> QED BY <1>1

THEOREM RejectedIngressSchedulerFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ ~( /\ SelectedDrainItem(node) \in asyncSentItems
           /\ SelectedDrainItem(node).kind
                \in {"CertifiedRequest", "CommitCertificateRequest"}
           /\ IF SelectedDrainItem(node).kind = "CertifiedRequest"
              THEN CertifiedRequestAuthorized(SelectedDrainItem(node))
              ELSE CommitCertificateRequestAuthorized(
                     SelectedDrainItem(node)))
    /\ ~CertifiedResponseClaimAuthorized(SelectedDrainItem(node))
    /\ ~( /\ SelectedDrainItem(node) \in asyncSentItems
           /\ CommitCertificateResponseAuthorized(
                SelectedDrainItem(node)))
    /\ ~( /\ SelectedDrainItem(node) \in asyncSentItems
           /\ SelectedDrainItem(node).kind # "Noise"
           /\ SelectedDrainItem(node).kind
                \notin {"CertifiedRequest", "CommitCertificateRequest"}
           /\ SelectedDrainItem(node).kind # "Chunk"
           /\ SelectedDrainItem(node).kind # "CertifiedResponse"
           /\ SelectedDrainItem(node).kind #
                "CommitCertificateResponse")
    => UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                   AsyncIoVars>>
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                ~( /\ SelectedDrainItem(node) \in asyncSentItems
                    /\ SelectedDrainItem(node).kind
                         \in {"CertifiedRequest",
                              "CommitCertificateRequest"}
                    /\ IF SelectedDrainItem(node).kind =
                              "CertifiedRequest"
                       THEN CertifiedRequestAuthorized(
                              SelectedDrainItem(node))
                       ELSE CommitCertificateRequestAuthorized(
                              SelectedDrainItem(node))),
                ~CertifiedResponseClaimAuthorized(
                  SelectedDrainItem(node)),
                ~( /\ SelectedDrainItem(node) \in asyncSentItems
                    /\ CommitCertificateResponseAuthorized(
                         SelectedDrainItem(node))),
                ~( /\ SelectedDrainItem(node) \in asyncSentItems
                    /\ SelectedDrainItem(node).kind # "Noise"
                    /\ SelectedDrainItem(node).kind
                         \notin {"CertifiedRequest",
                                 "CommitCertificateRequest"}
                    /\ SelectedDrainItem(node).kind # "Chunk"
                    /\ SelectedDrainItem(node).kind #
                         "CertifiedResponse"
                    /\ SelectedDrainItem(node).kind #
                         "CommitCertificateResponse")
         PROVE UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                           AsyncIoVars>>
    <2> QED BY <1>1, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem,
             SelectedDrainCandidate, SelectedDrainCommitCandidate,
             EnqueueCandidate
  <1> QED BY <1>1

THEOREM NonResponseIngressTransportFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ ~CertifiedResponseClaimAuthorized(SelectedDrainItem(node))
    /\ ~( /\ SelectedDrainItem(node) \in asyncSentItems
           /\ CommitCertificateResponseAuthorized(
                SelectedDrainItem(node)))
    => UNCHANGED
         <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
           asyncSentItems, asyncRetainedControl,
           asyncActiveRequests, asyncCertifiedResponseClaim,
           asyncTransport>>
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                ~CertifiedResponseClaimAuthorized(
                  SelectedDrainItem(node)),
                ~( /\ SelectedDrainItem(node) \in asyncSentItems
                    /\ CommitCertificateResponseAuthorized(
                         SelectedDrainItem(node)))
         PROVE UNCHANGED
                 <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
                   asyncSentItems, asyncRetainedControl,
                   asyncActiveRequests, asyncCertifiedResponseClaim,
                   asyncTransport>>
    <2> QED BY <1>1, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem,
             SelectedDrainCandidate, SelectedDrainCommitCandidate,
             EnqueueCandidate,
             AsyncCertifiedResponseClaimCoreAuthorityVars, vars
  <1> QED BY <1>1


THEOREM DirectChunkIngressTransportFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ SelectedDrainItem(node) \in asyncSentItems
    /\ SelectedDrainItem(node).kind = "Chunk"
    /\ SelectedDrainItem(node).envelope.chunk \in AsyncChunks
    => /\ UNCHANGED AsyncTransportHistoryTypeVars
       /\ UNCHANGED asyncTransport
       /\ asyncHeldChunks' =
            asyncHeldChunks \cup
              {AsyncChunkReceipt(
                 node, SelectedDrainItem(node).envelope.view,
                 SelectedDrainItem(node).envelope.subject,
                 SelectedDrainItem(node).envelope.chunk)}
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                SelectedDrainItem(node) \in asyncSentItems,
                SelectedDrainItem(node).kind = "Chunk",
                SelectedDrainItem(node).envelope.chunk \in AsyncChunks
         PROVE /\ UNCHANGED AsyncTransportHistoryTypeVars
               /\ UNCHANGED asyncTransport
               /\ asyncHeldChunks' =
                    asyncHeldChunks \cup
                      {AsyncChunkReceipt(
                         node, SelectedDrainItem(node).envelope.view,
                         SelectedDrainItem(node).envelope.subject,
                         SelectedDrainItem(node).envelope.chunk)}
    <2> QED BY <1>1, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem,
             IngressItemHasAuthenticatedHistory,
             AsyncTransportHistoryTypeVars,
             AsyncCertifiedResponseClaimAuthorityVars, vars
  <1> QED BY <1>1

THEOREM DirectChunkIngressPreservesTransportContentType ==
  \A node \in ValidatorIds:
    /\ AsyncTransportContentTypeInvariant
    /\ AsyncItemTyped(SelectedDrainItem(node))
    /\ DrainFairIngressSelected(node)
    /\ SelectedDrainItem(node) \in asyncSentItems
    /\ SelectedDrainItem(node).kind = "Chunk"
    /\ SelectedDrainItem(node).envelope.chunk \in AsyncChunks
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTransportContentTypeInvariant,
                AsyncItemTyped(SelectedDrainItem(node)),
                DrainFairIngressSelected(node),
                SelectedDrainItem(node) \in asyncSentItems,
                SelectedDrainItem(node).kind = "Chunk",
                SelectedDrainItem(node).envelope.chunk \in AsyncChunks
         PROVE AsyncTransportContentTypeInvariant'
    <2> DEFINE Receipt ==
           AsyncChunkReceipt(
             node, SelectedDrainItem(node).envelope.view,
             SelectedDrainItem(node).envelope.subject,
             SelectedDrainItem(node).envelope.chunk)
    <2>1. /\ AsyncTransportHistoryTypeInvariant
           /\ AsyncPacketContentTypeInvariant
           /\ AsyncHeldChunksTypeInvariant
      BY <1>1 DEF AsyncTransportContentTypeInvariant
    <2>2. AsyncBodyEnvelopeTyped(SelectedDrainItem(node).envelope)
      BY <1>1, SMT DEF AsyncItemTyped
    <2>3. /\ SelectedDrainItem(node).envelope.view \in Views
           /\ SelectedDrainItem(node).envelope.subject \in Subjects
      BY <2>2 DEF AsyncBodyEnvelopeTyped
    <2>4. Receipt \in AsyncChunkReceiptSet
      BY <1>1, <2>3, Isa
         DEF AsyncChunkReceipt, AsyncChunkReceiptSet, Receipt
    <2>5. /\ UNCHANGED AsyncTransportHistoryTypeVars
           /\ UNCHANGED asyncTransport
           /\ asyncHeldChunks' = asyncHeldChunks \cup {Receipt}
      BY <1>1, DirectChunkIngressTransportFrame DEF Receipt
    <2>6. AsyncTransportHistoryTypeInvariant'
      BY <2>1, <2>5, AsyncTransportHistoryTypeStutter
    <2>7. AsyncPacketContentTypeInvariant'
      BY <2>1, <2>5 DEF AsyncPacketContentTypeInvariant
    <2>8. AsyncHeldChunksTypeInvariant'
      BY <2>1, <2>4, <2>5, Isa
         DEF AsyncHeldChunksTypeInvariant
    <2> QED BY <2>6, <2>7, <2>8
         DEF AsyncTransportContentTypeInvariant
  <1> QED BY <1>1


THEOREM NonRecordingIngressTransportFrame ==
  \A node:
    /\ DrainFairIngressSelected(node)
    /\ ~CertifiedResponseClaimAuthorized(SelectedDrainItem(node))
    /\ ~( /\ SelectedDrainItem(node) \in asyncSentItems
           /\ CommitCertificateResponseAuthorized(
                SelectedDrainItem(node)))
    /\ ~( /\ SelectedDrainItem(node) \in asyncSentItems
           /\ SelectedDrainItem(node).kind = "Chunk"
           /\ SelectedDrainItem(node).envelope.chunk \in AsyncChunks)
    => UNCHANGED AsyncTransportContentTypeVars
PROOF
  <1>1. ASSUME NEW node,
                DrainFairIngressSelected(node),
                ~CertifiedResponseClaimAuthorized(
                  SelectedDrainItem(node)),
                ~( /\ SelectedDrainItem(node) \in asyncSentItems
                    /\ CommitCertificateResponseAuthorized(
                         SelectedDrainItem(node))),
                ~( /\ SelectedDrainItem(node) \in asyncSentItems
                    /\ SelectedDrainItem(node).kind = "Chunk"
                    /\ SelectedDrainItem(node).envelope.chunk
                         \in AsyncChunks)
         PROVE UNCHANGED AsyncTransportContentTypeVars
    <2> QED BY <1>1, SMTT(60)
         DEF DrainFairIngressSelected, SelectedDrainItem,
             IngressItemHasAuthenticatedHistory,
             AsyncTransportContentTypeVars,
             AsyncCertifiedResponseClaimAuthorityVars,
             AsyncCertifiedResponseClaimCoreAuthorityVars, vars
  <1> QED BY <1>1


THEOREM IngressSelectedPreservesRuntimeScalarType ==
  \A node \in ValidatorIds:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ asyncRunnerBudget[node] > 0
    /\ asyncRunnerPhase' = asyncRunnerPhase
    /\ asyncRunnerBudget' =
         [asyncRunnerBudget EXCEPT ![node] = @ - 1]
    /\ UNCHANGED asyncNextCommandClass
    /\ UNCHANGED <<asyncNow, asyncFifoOwed, asyncTimeoutEmitted>>
    /\ UNCHANGED AsyncLocalAdmissionVars
    /\ \/ asyncCommandQueues' = asyncCommandQueues
       \/ \E candidate:
            /\ AsyncCandidateTyped(candidate)
            /\ candidate.node = node
            /\ asyncCommandQueues' =
                 [asyncCommandQueues EXCEPT
                    ![node] = Append(@, candidate)]
    => AsyncRuntimeScalarTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncRuntimeScalarTypeInvariant,
                asyncRunnerBudget[node] > 0,
                asyncRunnerPhase' = asyncRunnerPhase,
                asyncRunnerBudget' =
                  [asyncRunnerBudget EXCEPT ![node] = @ - 1],
                UNCHANGED asyncNextCommandClass,
                UNCHANGED <<asyncNow, asyncFifoOwed,
                            asyncTimeoutEmitted>>,
                UNCHANGED AsyncLocalAdmissionVars,
                \/ asyncCommandQueues' = asyncCommandQueues
                \/ \E candidate:
                     /\ AsyncCandidateTyped(candidate)
                     /\ candidate.node = node
                     /\ asyncCommandQueues' =
                          [asyncCommandQueues EXCEPT
                             ![node] = Append(@, candidate)]
         PROVE AsyncRuntimeScalarTypeInvariant'
    <2>1. /\ DOMAIN asyncCommandQueues = ValidatorIds
           /\ \A other \in ValidatorIds:
                /\ AsyncQueueTyped(asyncCommandQueues[other])
                /\ AsyncCommandQueueOwnership(
                     other, asyncCommandQueues[other])
      BY <1>1 DEF AsyncRuntimeScalarTypeInvariant
    <2>2. asyncRunnerBudget \in
             [ValidatorIds ->
               0..(AsyncQueueCapacity + AsyncIngressCapacity)]
      BY <1>1 DEF AsyncRuntimeScalarTypeInvariant
    <2>3. AsyncConfiguration
      BY <1>1 DEF AsyncRuntimeScalarTypeInvariant
    <2>4. AsyncQueueCapacity + AsyncIngressCapacity \in Nat
      BY <2>3, SMT DEF AsyncConfiguration
    <2>5. asyncRunnerBudget[node]
               \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
      BY <1>1, <2>2
    <2>6. asyncRunnerBudget[node] > 0
      BY <1>1
    <2>7. asyncRunnerBudget[node] - 1
               \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
      BY <2>4, <2>5, <2>6, BoundedNaturalPredecessor
    <2>8. asyncRunnerBudget' \in
             [ValidatorIds ->
               0..(AsyncQueueCapacity + AsyncIngressCapacity)]
      BY <1>1, <2>2, <2>7, FunctionalUpdatePreservesType
    <2>9. /\ DOMAIN asyncCommandQueues' = ValidatorIds
           /\ \A other \in ValidatorIds:
                /\ AsyncQueueTyped(asyncCommandQueues'[other])
                /\ AsyncCommandQueueOwnership(
                     other, asyncCommandQueues'[other])
      <3>1. CASE asyncCommandQueues' = asyncCommandQueues
        BY <2>1, <3>1
      <3>2. CASE \E candidate:
                       /\ AsyncCandidateTyped(candidate)
                       /\ candidate.node = node
                       /\ asyncCommandQueues' =
                            [asyncCommandQueues EXCEPT
                               ![node] = Append(@, candidate)]
        <4>1. PICK candidate:
                       /\ AsyncCandidateTyped(candidate)
                       /\ candidate.node = node
                       /\ asyncCommandQueues' =
                            [asyncCommandQueues EXCEPT
                               ![node] = Append(@, candidate)]
          BY <3>2
        <4>2. /\ AsyncQueueTyped(
                       Append(asyncCommandQueues[node], candidate))
               /\ AsyncCommandQueueOwnership(
                    node, Append(asyncCommandQueues[node], candidate))
          BY <2>1, <4>1, TypedCandidateAppendPreservesQueueType,
             AppendOwnedCandidatePreservesCommandQueueOwnership
        <4>3. \A other \in ValidatorIds:
                 /\ AsyncQueueTyped(asyncCommandQueues'[other])
                 /\ AsyncCommandQueueOwnership(
                      other, asyncCommandQueues'[other])
          <5>1. ASSUME NEW other \in ValidatorIds
                 PROVE /\ AsyncQueueTyped(asyncCommandQueues'[other])
                       /\ AsyncCommandQueueOwnership(
                            other, asyncCommandQueues'[other])
            <6>1. CASE other = node
              BY <2>1, <4>1, <4>2, <6>1,
                 FunctionalAppendUpdateAtKey
            <6>2. CASE other # node
              BY <2>1, <4>1, <5>1, <6>2,
                 FunctionalUpdateAwayFromKey
            <6> QED BY <6>1, <6>2
          <5> QED BY <5>1
        <4> QED BY <2>1, <4>1, <4>3, Isa
      <3> QED BY <1>1, <3>1, <3>2
    <2> QED BY <1>1, <2>8, <2>9
         DEF AsyncRuntimeScalarTypeInvariant, AsyncLocalAdmissionVars
  <1> QED BY <1>1

THEOREM IngressPhaseAdvancePreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunNodeWork(node)
    /\ IngressDrainStep(node)
    /\ ~(asyncRunnerBudget[node] > 0
           /\ asyncIngressReady[node] # <<>>
           /\ DrainableIngressIndices(node) # {})
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                RunNodeWork(node),
                IngressDrainStep(node),
                ~(asyncRunnerBudget[node] > 0
                    /\ asyncIngressReady[node] # <<>>
                    /\ DrainableIngressIndices(node) # {})
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1
    <2>2. /\ RunnerServiceFrame(node)
           /\ asyncRunnerPhase' =
                [asyncRunnerPhase EXCEPT ![node] = "Runtime"]
           /\ asyncRunnerBudget' =
                [asyncRunnerBudget EXCEPT ![node] = 1]
           /\ UNCHANGED AsyncLocalAdmissionVars
           /\ UNCHANGED <<context, asyncCommandQueues,
                          asyncNextCommandClass, asyncFifoOwed,
                          asyncTimeoutEmitted, asyncCausalQueues,
                          AsyncIoVars, AsyncDeferredVars,
                          asyncOutstandingTags, asyncNodeDeadlines,
                          asyncRetransmitDeadlines, asyncSentItems,
                          asyncRetainedControl, asyncActiveRequests,
                          asyncCertifiedResponseClaim,
                          asyncTransport, asyncIngressLanes,
                          asyncIngressReady, asyncHeldChunks>>
      BY <1>1, Isa
         DEF RunNodeWork, RunnerServiceFrame, IngressDrainStep,
             LeaveCausalQueues, vars
    <2>3. /\ asyncCausalAdmissionOwed'
                    \in [ValidatorIds -> BOOLEAN]
           /\ asyncNextLocalSource'
                    \in [ValidatorIds -> AsyncLocalSources]
      BY <1>1, <2>2, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncLocalAdmissionVars
    <2>4. /\ asyncRunnerPhase
                    \in [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
           /\ asyncRunnerBudget
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
           /\ 1 \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
      BY <1>1, SMT
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncConfiguration
    <2>5. /\ asyncRunnerPhase'
                    \in [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
           /\ asyncRunnerBudget'
                    \in [ValidatorIds ->
                          0..(AsyncQueueCapacity + AsyncIngressCapacity)]
      BY <2>1, <2>2, <2>4, FunctionalUpdatePreservesType
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>5,
                    IngressDrainPreservesHistoricalRecoveryType,
                    RunnerScalarClockAndSchedulerStutterPreservesType
  <1> QED BY <1>1

THEOREM IngressDrainRunnerPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunNodeWork(node)
    /\ IngressDrainStep(node)
    /\ asyncRunnerBudget[node] > 0
    /\ asyncIngressReady[node] # <<>>
    /\ DrainableIngressIndices(node) # {}
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                RunNodeWork(node),
                IngressDrainStep(node),
                asyncRunnerBudget[node] > 0,
                asyncIngressReady[node] # <<>>,
                DrainableIngressIndices(node) # {}
         PROVE AsyncSchedulerTypeInvariant'
    <2> DEFINE DrainIndex == FirstDrainableIngressIndex(node)
    <2> DEFINE DrainSource == asyncIngressReady[node][DrainIndex]
    <2> DEFINE DrainLaneIndex == SelectedIngressLaneIndex(node, DrainIndex)
    <2> DEFINE DrainItem == SelectedIngressItemAt(node, DrainIndex)
    <2> DEFINE Candidate == DeliveryCandidate(DrainItem)
    <2> DEFINE CertifiedCandidate == CertifiedResponseCandidate(DrainItem)
    <2> DEFINE DiscoveredItem == DiscoveredCommitQcItem(DrainItem)
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
    <2> DEFINE ChunkAccepted ==
           /\ DrainItem \in asyncSentItems
           /\ DrainItem.kind = "Chunk"
           /\ DrainItem.envelope.chunk \in AsyncChunks
    <2> DEFINE OrdinaryAccepted ==
           /\ DrainItem \in asyncSentItems
           /\ DrainItem.kind # "Noise"
           /\ DrainItem.kind
                \notin {"CertifiedRequest", "CommitCertificateRequest"}
           /\ DrainItem.kind # "Chunk"
           /\ DrainItem.kind # "CertifiedResponse"
           /\ DrainItem.kind # "CommitCertificateResponse"
    <2>1. node \in ValidatorIds
      BY <1>1
    <2>2. /\ DrainFairIngressSelected(node)
           /\ PopSelectedIngress(node, DrainIndex, DrainLaneIndex)
           /\ RunnerServiceFrame(node)
           /\ LeaveCausalQueues
           /\ UNCHANGED AsyncDeferredVars
           /\ UNCHANGED AsyncLocalAdmissionVars
           /\ asyncRunnerPhase' = asyncRunnerPhase
           /\ asyncRunnerBudget' =
                [asyncRunnerBudget EXCEPT ![node] = @ - 1]
      <3>1. /\ DrainFairIngressSelected(node)
             /\ LeaveCausalQueues
             /\ UNCHANGED AsyncDeferredVars
             /\ UNCHANGED AsyncLocalAdmissionVars
             /\ asyncRunnerPhase' = asyncRunnerPhase
             /\ asyncRunnerBudget' =
                  [asyncRunnerBudget EXCEPT ![node] = @ - 1]
        BY <1>1, Isa DEF IngressDrainStep
      <3>2. PopSelectedIngress(node, DrainIndex, DrainLaneIndex)
        BY <3>1
           DEF DrainFairIngressSelected, DrainIndex, DrainLaneIndex
      <3>3. RunnerServiceFrame(node)
        BY <1>1 DEF RunNodeWork, RunnerServiceFrame
      <3> QED BY <3>1, <3>2, <3>3
    <2>3. DrainIndex \in DrainableIngressIndices(node)
      BY <1>1, FirstDrainableIngressIndexIsDrainable DEF DrainIndex
    <2>4. /\ DrainIndex \in 1..Len(asyncIngressReady[node])
           /\ IngressSourceCanDrain(node, DrainSource)
           /\ DrainLaneIndex \in
                DrainableIngressLaneIndices(node, DrainSource)
           /\ DrainLaneIndex \in 1..Len(IngressLane(node, DrainSource))
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
    <2>5. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncIoTypeInvariant
           /\ AsyncDeferredTopologyTypeInvariant
           /\ AsyncDeferredContentTypeInvariant
           /\ AsyncTransportClockTypeInvariant
           /\ AsyncTransportContentTypeInvariant
           /\ AsyncConfiguration
           /\ ModelConfiguration
           /\ AsyncIngressTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, TypeInvariant
    <2>6. /\ AsyncItemTyped(DrainItem)
           /\ DrainItem.envelope.recipient = node
           /\ AsyncIngressItemSourceBinding(DrainItem, DrainSource)
      BY <2>1, <2>4, <2>5, SelectedIngressItemIsTyped,
         SelectedIngressItemHasLaneOwnership
         DEF DrainItem, SelectedIngressItemAt, DrainLaneIndex,
             DrainSource
    <2>7. /\ AsyncCandidateTyped(Candidate)
           /\ Candidate.node = node
           /\ Candidate.class # "Completion"
      BY <1>1, <2>1, <2>6,
         TypedIngressDeliveryCandidateFacts DEF Candidate
    <2>8. CommitAccepted
             => /\ AsyncItemTyped(DiscoveredItem)
                /\ AsyncCandidateTyped(CommitCandidate)
                /\ CommitCandidate.node = node
                /\ CommitCandidate.class # "Completion"
      BY <1>1, <2>1, <2>6,
         TypedCommitCertificateResponseCandidateFacts
         DEF CommitAccepted, CommitCandidate, DiscoveredItem,
             CommitCertificateResponseAuthorized
    <2>8a. CertifiedAccepted
              => /\ AsyncCandidateTyped(CertifiedCandidate)
                 /\ CertifiedCandidate.node = node
                 /\ CertifiedCandidate.class = "Completion"
      <3>1. ASSUME CertifiedAccepted
             PROVE /\ AsyncCandidateTyped(CertifiedCandidate)
                   /\ CertifiedCandidate.node = node
                   /\ CertifiedCandidate.class = "Completion"
        <4>1. DrainItem.kind = "CertifiedResponse"
          BY <3>1
             DEF CertifiedAccepted, CertifiedResponseClaimAuthorized,
                 CertifiedResponseAuthorized
        <4> QED BY <1>1, <2>1, <2>6, <4>1,
             TypedCertifiedResponseCandidateFacts
             DEF CertifiedCandidate
      <3> QED BY <3>1
    <2>9. AsyncRuntimeScalarTypeInvariant'
      <3>1. /\ UNCHANGED asyncNextCommandClass
             /\ UNCHANGED <<asyncNow, asyncFifoOwed,
                            asyncTimeoutEmitted>>
             /\ \/ asyncCommandQueues' = asyncCommandQueues
                \/ asyncCommandQueues' =
                     [asyncCommandQueues EXCEPT
                        ![Candidate.node] = Append(@, Candidate)]
                \/ /\ CertifiedAccepted
                      /\ asyncCommandQueues' =
                           [asyncCommandQueues EXCEPT
                              ![CertifiedCandidate.node] =
                                Append(@, CertifiedCandidate)]
                \/ /\ CommitAccepted
                      /\ asyncCommandQueues' =
                           [asyncCommandQueues EXCEPT
                              ![CommitCandidate.node] =
                                Append(@, CommitCandidate)]
        BY <2>2, DrainFairIngressRuntimeFrame
           DEF SelectedDrainItem, SelectedDrainCandidate,
               SelectedDrainCertifiedCandidate,
               SelectedDrainCommitCandidate, DrainIndex, DrainItem,
               Candidate, CertifiedCandidate, CommitCandidate,
               CertifiedAccepted, CommitAccepted
      <3>2. /\ UNCHANGED asyncNextCommandClass
             /\ UNCHANGED <<asyncNow, asyncFifoOwed,
                            asyncTimeoutEmitted>>
             /\ \/ asyncCommandQueues' = asyncCommandQueues
                \/ asyncCommandQueues' =
                     [asyncCommandQueues EXCEPT
                        ![node] = Append(@, Candidate)]
                \/ /\ CertifiedAccepted
                      /\ asyncCommandQueues' =
                           [asyncCommandQueues EXCEPT
                              ![node] = Append(@, CertifiedCandidate)]
                \/ /\ CommitAccepted
                      /\ asyncCommandQueues' =
                           [asyncCommandQueues EXCEPT
                              ![node] = Append(@, CommitCandidate)]
        BY <2>7, <2>8, <2>8a, <3>1, Zenon
      <3>3. \/ asyncCommandQueues' = asyncCommandQueues
             \/ \E admitted:
                  /\ AsyncCandidateTyped(admitted)
                  /\ admitted.node = node
                  /\ asyncCommandQueues' =
                       [asyncCommandQueues EXCEPT
                          ![node] = Append(@, admitted)]
        BY <2>7, <2>8, <2>8a, <3>2, Zenon
      <3> QED BY <1>1, <2>1, <2>2, <2>5, <3>2, <3>3,
           IngressSelectedPreservesRuntimeScalarType
    <2>10. AsyncCausalTypeInvariant'
      BY <2>2, <2>5, AsyncCausalTypeStutter DEF LeaveCausalQueues
    <2>11. AsyncDeferredTypeInvariant'
      <3>1. /\ UNCHANGED AsyncDeferredTopologyTypeVars
             /\ UNCHANGED <<asyncDeferredCompletionQueues,
                            asyncDeferredProgressQueues,
                            asyncDeferredNormalQueues>>
        BY <2>2, Isa
           DEF AsyncDeferredVars, AsyncDeferredTopologyTypeVars
      <3> QED BY <2>5, <3>1,
           AsyncDeferredTopologyTypeStutter,
           AsyncDeferredContentTypeStutter
           DEF AsyncDeferredTypeInvariant
    <2>12. AsyncIngressTypeInvariant'
      BY <2>1, <2>2, <2>4, <2>5,
         PopSelectedIngressPreservesIngressType
    <2>13. AsyncIoTypeInvariant'
      <3>1. CASE ServeAccepted
        <4>1. DrainItem.kind
                 \in {"CertifiedRequest", "CommitCertificateRequest"}
          BY <3>1 DEF ServeAccepted
        <4>2. CanEnqueueIoClass(node, "Serve")
          BY <2>4, <3>1, SMTT(60)
             DEF IngressItemCanDrain, ServeAccepted
        <4>3. /\ asyncIoQueues' =
                    [asyncIoQueues EXCEPT
                       ![node] = Append(
                         @, AsyncIoCertifiedServeJob(node, Candidate))]
               /\ UNCHANGED <<asyncOutstandingWork,
                               asyncIoReadyCompletions,
                               asyncLocalReadyCompletions,
                               asyncNextCompletionSource,
                               asyncIoControlAvailable,
                               asyncCommandQueues>>
          BY <2>2, <3>1, AuthorizedIngressServeFrame
             DEF SelectedDrainItem, SelectedDrainCandidate,
                 DrainIndex, DrainItem, Candidate, ServeAccepted
        <4>4. UNCHANGED asyncDeferredCompletionQueues
          BY <2>2 DEF AsyncDeferredVars
        <4> QED BY <1>1, <2>1, <2>5, <2>6,
             <4>1, <4>2, <4>3, <4>4,
             AppendTypedServeJobPreservesIoType
             DEF Candidate
      <3>2. CASE CertifiedAccepted
        <4>1. DrainItem.kind = "CertifiedResponse"
          BY <3>2
             DEF CertifiedAccepted, CertifiedResponseClaimAuthorized,
                 CertifiedResponseAuthorized
        <4>2. /\ AsyncCandidateTyped(CertifiedCandidate)
               /\ CertifiedCandidate.node = node
               /\ CertifiedCandidate.class = "Completion"
          BY <2>8a, <3>2
        <4>3. CASE CandidateScheduled(CertifiedCandidate)
          <5>1. UNCHANGED <<AsyncIoVars, asyncCommandQueues,
                             asyncNextCommandClass>>
            BY <2>2, <3>2, <4>3,
               ScheduledAuthorizedCertifiedResponseSchedulerFrame
               DEF SelectedDrainItem, DrainIndex, DrainItem,
                   CertifiedAccepted, CertifiedCandidate
          <5>2. UNCHANGED asyncDeferredCompletionQueues
            BY <2>2 DEF AsyncDeferredVars
          <5> QED BY <2>5, <5>1, <5>2,
               SchedulerIoStutterPreservesIoType
        <4>4. CASE ~CandidateScheduled(CertifiedCandidate)
          <5>1. /\ CanEnqueueCertifiedResponse(node)
                 /\ ~CandidateInFlight(CertifiedCandidate)
            BY <2>4, <3>2, <4>4, Isa
               DEF IngressItemCanDrain, DrainItem,
                   CertifiedAccepted, CertifiedCandidate,
                   CertifiedResponseClaimAuthorized,
                   CertifiedResponseAuthorized,
                   CandidateScheduled, CandidateInFlight
          <5>2. /\ asyncCommandQueues' =
                      [asyncCommandQueues EXCEPT
                         ![node] = Append(@, CertifiedCandidate)]
                 /\ UNCHANGED <<AsyncIoVars,
                                 asyncDeferredCompletionQueues>>
            BY <2>2, <3>2, <4>4,
               FreshAuthorizedCertifiedResponseSchedulerFrame
               DEF SelectedDrainItem, DrainIndex, DrainItem,
                   CertifiedAccepted, CertifiedCandidate,
                   AsyncDeferredVars
          <5> QED BY <1>1, <2>1, <4>2, <5>1, <5>2,
               EnqueueCandidatePreservesIoType
        <4> QED BY <4>3, <4>4
      <3>3. CASE CommitAccepted
        <4>1. CommitCandidate.class = "Progress"
          BY CommitCertificateResponseCandidateHasProgressClass
             DEF CommitCandidate
        <4>2. DrainItem.kind = "CommitCertificateResponse"
          BY <3>3
             DEF CommitAccepted, CommitCertificateResponseAuthorized
        <4>2a. /\ DrainItem \in asyncSentItems
                /\ CommitCertificateResponseAuthorized(DrainItem)
          BY <3>3 DEF CommitAccepted
        <4>3. CASE CandidateScheduled(CommitCandidate)
          <5>1. UNCHANGED <<asyncCommandQueues,
                             asyncNextCommandClass>>
            BY <2>2, <3>3, <4>2, <4>3,
               ScheduledAuthorizedCommitResponseCommandFrame
               DEF SelectedDrainItem, SelectedDrainCommitCandidate,
                   DrainIndex, DrainItem, CommitCandidate, CommitAccepted
          <5>2. UNCHANGED AsyncIoVars
            BY <2>2, <3>3, AuthorizedCommitResponseFrame
               DEF SelectedDrainItem, DrainIndex, DrainItem,
                   CommitAccepted
          <5>3. UNCHANGED asyncDeferredCompletionQueues
            BY <2>2 DEF AsyncDeferredVars
          <5> QED BY <2>5, <5>1, <5>2, <5>3,
               SchedulerIoStutterPreservesIoType
        <4>4. CASE ~CandidateScheduled(CommitCandidate)
          <5>1. CanEnqueueClass(node, "Progress")
            BY <2>4, <4>2, <4>2a, <4>4, SMTT(30)
               DEF IngressItemCanDrain
          <5>2. /\ asyncCommandQueues' =
                      [asyncCommandQueues EXCEPT
                         ![node] = Append(@, CommitCandidate)]
                 /\ UNCHANGED <<AsyncIoVars,
                                 asyncDeferredCompletionQueues>>
            <6>1. /\ UNCHANGED asyncNextCommandClass
                   /\ asyncCommandQueues' =
                        [asyncCommandQueues EXCEPT
                           ![CommitCandidate.node] =
                             Append(@, CommitCandidate)]
              BY <2>2, <3>3, <4>2, <4>4,
                 FreshAuthorizedCommitResponseCommandFrame
                 DEF SelectedDrainItem, SelectedDrainCommitCandidate,
                     DrainIndex, DrainItem, CommitCandidate,
                     CommitAccepted
            <6>2. UNCHANGED AsyncIoVars
              BY <2>2, <3>3, AuthorizedCommitResponseFrame
                 DEF SelectedDrainItem, DrainIndex, DrainItem,
                     CommitAccepted
            <6>3. UNCHANGED asyncDeferredCompletionQueues
              BY <2>2 DEF AsyncDeferredVars
            <6> QED BY <2>8, <3>3, <6>1, <6>2, <6>3, Isa
          <5>3. /\ AsyncCandidateTyped(CommitCandidate)
                 /\ CommitCandidate.node = node
                 /\ CommitCandidate.class # "Completion"
                 /\ CanEnqueueClass(node, CommitCandidate.class)
            BY <2>8, <3>3, <4>1, <5>1, SMT
          <5> QED BY <1>1, <2>1, <5>1, <5>2, <5>3,
               EnqueueNonCompletionCandidatePreservesIoType
        <4> QED BY <4>3, <4>4
      <3>4. CASE OrdinaryAccepted
        <4>1. CASE CandidateScheduled(Candidate)
          <5>1. UNCHANGED <<asyncCommandQueues,
                             asyncNextCommandClass, AsyncIoVars>>
            BY <2>2, <3>4, <4>1, OrdinaryScheduledIngressFrame
               DEF SelectedDrainItem, SelectedDrainCandidate,
                   DrainIndex, DrainItem, OrdinaryAccepted, Candidate
          <5>2. UNCHANGED asyncDeferredCompletionQueues
            BY <2>2 DEF AsyncDeferredVars
          <5> QED BY <2>5, <5>1,
               <5>2,
               SchedulerIoStutterPreservesIoType
        <4>2. CASE ~CandidateScheduled(Candidate)
          <5>1. /\ CanEnqueueClass(node, Candidate.class)
                 /\ asyncCommandQueues' =
                      [asyncCommandQueues EXCEPT
                         ![node] = Append(@, Candidate)]
                 /\ UNCHANGED AsyncIoVars
            BY <2>2, <2>4, <2>7, <3>4, <4>2,
               OrdinaryFreshIngressFrame
               DEF SelectedDrainItem, SelectedDrainCandidate,
                   DrainIndex, DrainItem, Candidate, OrdinaryAccepted
          <5>2. UNCHANGED asyncDeferredCompletionQueues
            BY <2>2 DEF AsyncDeferredVars
          <5> QED BY <1>1, <2>1, <2>7, <5>1, <5>2,
               EnqueueNonCompletionCandidatePreservesIoType
        <4> QED BY <4>1, <4>2
      <3>5. CASE ~(ServeAccepted \/ CertifiedAccepted \/
                     CommitAccepted \/ OrdinaryAccepted)
        <4>1. UNCHANGED <<asyncCommandQueues, asyncNextCommandClass,
                           AsyncIoVars>>
          BY <2>2, <3>5, RejectedIngressSchedulerFrame
             DEF SelectedDrainItem, DrainIndex, DrainItem,
                 ServeAccepted, CertifiedAccepted, CommitAccepted,
                 OrdinaryAccepted
        <4>2. UNCHANGED asyncDeferredCompletionQueues
          BY <2>2 DEF AsyncDeferredVars
        <4> QED BY <2>5, <4>1, <4>2,
             SchedulerIoStutterPreservesIoType
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
    <2>13a. AsyncCertifiedResponseClaimInvariant'
      <3>1. CASE CertifiedAccepted
        <4>1. /\ asyncSentItems' = asyncSentItems \cup {}
               /\ asyncActiveRequests' \subseteq asyncActiveRequests
               /\ asyncCertifiedResponseClaim' =
                    CertifiedResponseClaimForRequests(
                      asyncActiveRequests')
          BY <2>2, <3>1, AuthorizedCertifiedResponseFrame, SMT
             DEF SelectedDrainItem, DrainIndex, DrainItem,
                 CertifiedAccepted
        <4>2. AsyncCertifiedResponseClaimInvariant
          BY <2>5
             DEF AsyncTransportContentTypeInvariant,
                 AsyncTransportHistoryTypeInvariant
        <4> QED BY <4>1, <4>2,
             FilterActiveRequestsAndClaimPreservesInvariant
      <3>2. CASE CommitAccepted
        <4>1. /\ asyncSentItems' =
                        asyncSentItems \cup {DiscoveredItem}
               /\ AsyncCertifiedRequestsIn(asyncActiveRequests') =
                    AsyncCertifiedRequestsIn(asyncActiveRequests)
               /\ UNCHANGED
                    <<AsyncCertifiedResponseClaimCoreAuthorityVars,
                      asyncCertifiedResponseClaim>>
          BY <2>2, <3>2, AuthorizedCommitResponseFrame, SMT
             DEF SelectedDrainItem, DrainIndex, DrainItem,
                 CommitAccepted, DiscoveredItem,
                 MatchingCommitCertificateRequests,
                 AsyncCertifiedRequestsIn
        <4>2. AsyncCertifiedResponseClaimInvariant
          BY <2>5
             DEF AsyncTransportContentTypeInvariant,
                 AsyncTransportHistoryTypeInvariant
        <4> QED BY <4>1, <4>2,
             AppendSentHistoryPreservesCertifiedResponseClaimInvariant
      <3>3. CASE ~(CertifiedAccepted \/ CommitAccepted)
        <4>1. UNCHANGED
                 <<AsyncCertifiedResponseClaimCoreAuthorityVars,
                   asyncSentItems, asyncActiveRequests,
                   asyncCertifiedResponseClaim>>
          BY <2>2, <3>3, NonResponseIngressTransportFrame
             DEF SelectedDrainItem, DrainIndex, DrainItem,
                 CertifiedAccepted, CommitAccepted
        <4>2. AsyncCertifiedResponseClaimInvariant
          BY <2>5
             DEF AsyncTransportContentTypeInvariant,
                 AsyncTransportHistoryTypeInvariant
        <4> QED BY <4>1, <4>2, Isa
             DEF AsyncCertifiedResponseClaimInvariant,
                 AsyncCertifiedResponseClaimCoreAuthorityVars,
               CertifiedResponseClaimProjectionAuthenticated,
               CertifiedResponseAuthorized,
               CertifiedResponseAuthenticatedOccurrence,
               MatchingCertifiedRequests,
               FrozenCertifiedRequestRegistration,
               FrozenCertifiedResponseBinding,
               AsyncCertifiedResponseCanonicalWireIdentity,
               ActiveCertifiedRequestHashes,
                 ActiveCertifiedRequestHashesIn,
                 CertifiedResponseAuthorityReady,
                 CertifiedResponseAuthorityClaimed
      <3> QED BY <3>1, <3>2, <3>3
    <2>14. AsyncTransportContentTypeInvariant'
      <3>1. CASE CertifiedAccepted
        <4>1. /\ asyncSentItems' = asyncSentItems \cup {}
               /\ asyncActiveRequests' =
                    asyncActiveRequests \ MatchingCertifiedRequests(DrainItem)
               /\ asyncRetainedControl' = asyncRetainedControl
               /\ AsyncCertifiedResponseClaimInvariant'
               /\ UNCHANGED
                    <<context,
                      AsyncCertifiedResponseClaimCoreAuthorityVars,
                      asyncTransport, asyncHeldChunks>>
          BY <2>2, <3>1, AuthorizedCertifiedResponseFrame, SMT
             DEF SelectedDrainItem, DrainIndex, DrainItem,
                 CertifiedAccepted
        <4>2. /\ IsFiniteSet({})
               /\ \A item \in {}: AsyncItemTyped(item)
          BY FS_EmptySet
        <4> QED BY <2>5, <2>13a, <4>1, <4>2,
             RemoveRequestsAndAddSentPreservesTransportContentType
      <3>2. CASE CommitAccepted
        <4>1. /\ asyncSentItems' =
                    asyncSentItems \cup {DiscoveredItem}
               /\ asyncActiveRequests' =
                    asyncActiveRequests \
                      MatchingCommitCertificateRequests(DrainItem)
               /\ asyncRetainedControl' = asyncRetainedControl
               /\ AsyncCertifiedResponseClaimInvariant'
               /\ UNCHANGED
                    <<context,
                      AsyncCertifiedResponseClaimCoreAuthorityVars,
                      asyncTransport, asyncHeldChunks>>
          BY <2>2, <3>2, AuthorizedCommitResponseFrame
             DEF SelectedDrainItem, DrainIndex, DrainItem,
                 CommitAccepted, DiscoveredItem
        <4>2. /\ IsFiniteSet({DiscoveredItem})
               /\ \A item \in {DiscoveredItem}: AsyncItemTyped(item)
          BY <2>8, <3>2, FS_Singleton
        <4> QED BY <2>5, <2>13a, <4>1, <4>2,
             RemoveRequestsAndAddSentPreservesTransportContentType
      <3>3. CASE ChunkAccepted
        <4> QED BY <2>1, <2>2, <2>5, <2>6, <3>3,
             DirectChunkIngressPreservesTransportContentType
             DEF SelectedDrainItem, DrainIndex, DrainItem,
                 ChunkAccepted
      <3>4. CASE ~(CertifiedAccepted \/ CommitAccepted \/
                     ChunkAccepted)
        <4>1. UNCHANGED AsyncTransportContentTypeVars
          BY <2>2, <3>4, NonRecordingIngressTransportFrame
             DEF SelectedDrainItem, DrainIndex, DrainItem,
                 CertifiedAccepted, CommitAccepted, ChunkAccepted
        <4> QED BY <2>5, <4>1,
             AsyncTransportContentTypeStutter
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2>15. /\ RunnerServiceFrame(node)
            /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                           asyncRetransmitDeadlines>>
      BY <1>1, <2>2, Isa
         DEF DrainFairIngressSelected, RunnerServiceFrame, vars
    <2>16. AsyncTransportClockTypeInvariant'
      BY <2>1, <2>5, <2>15,
         RunnerServiceFramePreservesClockType
    <2>17. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, IngressDrainPreservesHistoricalRecoveryType
    <2> QED BY <2>9, <2>10, <2>11, <2>12, <2>13, <2>14, <2>16,
                  <2>17
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncDeferredTypeInvariant, AsyncTransportTypeInvariant
  <1> QED BY <1>1

=============================================================================
