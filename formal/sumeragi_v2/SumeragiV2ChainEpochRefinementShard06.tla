---- MODULE SumeragiV2ChainEpochRefinementShard06 ----
EXTENDS SumeragiV2ChainEpochRefinementShard05

THEOREM IndexedAsyncInitHasNoCurrentReceipts ==
  (IndexedAsyncStateShape
    /\ \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncInitAt(initialContext))
    => /\ IndexedDecisionEvidence = {}
       /\ IndexedApplicationEvidence = {}
BY Isa DEF IndexedDecisionEvidence, IndexedApplicationEvidence,
           IndexedCurrentDecisions, IndexedCurrentApplications,
           IndexedDecisions, IndexedApplications,
           IndexedAsync!AsyncInitAt, IndexedAsync!AsyncBaseInitAt,
           IndexedAsync!InitAt, IndexedAsync!BootstrapParentDecision

THEOREM IndexedChainInitHasEmptyCurrentReceiptUnion ==
  IndexedChainInit
    => /\ IndexedDecisionEvidence = {}
       /\ IndexedApplicationEvidence = {}
BY IndexedAsyncInitHasNoCurrentReceipts DEF IndexedChainInit

THEOREM JoinedRunnerIsExactAsyncWork ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedJoinedRunnerStep(initialContext)
      => IndexedAsync(initialContext)!AsyncRunnerStep
BY Isa DEF IndexedJoinedRunnerStep,
           IndexedAsync!AsyncRunnerStep,
           IndexedAsync!RunHistoricalRecoveryNode,
           IndexedAsync!HistoricalRecoveryTarget,
           IndexedAsync!RunHistoricalServer

THEOREM JoinedNonRunnerIsExactAsyncWork ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedJoinedNonRunnerStep(initialContext)
      => IndexedAsync(initialContext)!AsyncNonRunnerStep
BY Isa DEF IndexedJoinedNonRunnerStep,
           IndexedAsync!AsyncNonRunnerStep,
           IndexedOpenHistoricalRecovery,
           IndexedAsync!DirectHistoricalCommitCertificateDiscoveryStep,
           IndexedAsync!HistoricalCommitCertificateDiscoveryDue,
           IndexedAsync!ServiceIoWorker,
           IndexedAsync!ServiceHistoricalRecoveryIoWorker,
           IndexedAsync!EnqueueHistoricalRecoveryIoLocalControl,
           IndexedAsync!HistoricalRecoveryTarget

THEOREM JoinedAsyncStepRefinesExactAsyncStep ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedJoinedAsyncNext(initialContext)
      => IndexedAsync(initialContext)!AsyncNext
BY Isa, JoinedRunnerIsExactAsyncWork,
   JoinedNonRunnerIsExactAsyncWork
   DEF IndexedJoinedAsyncNext, IndexedJoinedNonCrashStep,
       IndexedAsync!AsyncNext, IndexedAsync!AsyncNonCrashStep,
       IndexedAsync!AsyncProducerProjectionStep,
       IndexedAsync!AsyncServiceActivationTransition,
       IndexedScheduler

(***************************************************************************
Responsive restart/replay is intentionally outside the indexed chain product.
The normal joined branch now carries the complete production non-crash
recovery-control frame, and the remaining non-responsive crash branch already
frames AsyncRecoveryVars.  Final successor publication changes only the exact
successor instance's internal service-activation state and paired deadlines;
every recovery component still stutters.  These facts make the initialized
Eligible phase inductive without silently adding a favourable restart relation.
***************************************************************************)
THEOREM IndexedInitEstablishesResponsiveRecoveryDormancy ==
  IndexedChainInit => IndexedResponsiveRecoveryDormant
BY Isa DEF IndexedChainInit, IndexedResponsiveRecoveryDormant,
           IndexedAsync!AsyncInitAt, IndexedAsync!AsyncBaseInitAt,
           IndexedAsync!AsyncRecoveryInit, IndexedRecovery

THEOREM IndexedJoinedAsyncStepPreservesResponsiveRecoveryEligibility ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedRecovery(initialContext, 1) = "Eligible"
      /\ IndexedJoinedAsyncNext(initialContext)
      => IndexedRecovery(initialContext, 1)' = "Eligible"
BY Isa DEF IndexedJoinedAsyncNext, IndexedJoinedNonCrashStep,
           IndexedAsync!PreGstCrash,
           IndexedAsync!AsyncRecoveryVars,
           IndexedAsync!AsyncRecoveryControlVars,
           IndexedRecovery

THEOREM IndexedProductActionPreservesResponsiveRecoveryDormancy ==
  \A selectedContext \in AdmissibleContextRecords:
    IndexedResponsiveRecoveryDormant
      /\ IndexedProductActionAt(selectedContext)
      => IndexedResponsiveRecoveryDormant'
BY Isa, IndexedJoinedAsyncStepPreservesResponsiveRecoveryEligibility
   DEF IndexedResponsiveRecoveryDormant, IndexedProductActionAt,
       IndexedAsyncStateAt, IndexedRecovery

THEOREM IndexedSuccessorActivationStepPreservesRecoveryState ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedSuccessorActivationProgressStep(parentContext, node)
      => \A initialContext \in AdmissibleContextRecords:
           UNCHANGED indexedAsyncState[initialContext][4]
BY Isa DEF IndexedSuccessorActivationProgressStep,
           BeginSuccessorActivation,
           BindAppliedSuccessorActivationToken,
           LatchAppliedSuccessorStartupFailure,
           LatchRecoveredSuccessorStartupFailure,
           RehydrateCleanCompleteTipSuccessorStartup,
           RehydrateFailedSuccessorStartup,
           AuthenticateRecoveredSuccessorActivation,
           OpenDeferredSuccessorAdapter,
           ConstructSuccessorRuntime,
           StartSuccessorServices,
           ApplySuccessorStartupEffects,
           ArmSuccessorClocks,
           PrepareSuccessorActivationMarker,
           OpenSuccessorIngress,
           ActivateAppliedSuccessorHeight,
           ActivateRecoveredSuccessorHeight,
           SuccessorActivationEnvironmentStutter,
           SuccessorActivationEnvironmentActivatesNode,
           IndexedAsync!AsyncEnterIndexedServiceActivation,
           IndexedAsync!AsyncActivateServiceNode,
           IndexedAsync!AsyncServiceActivationFrameVars,
           IndexedAsync!AsyncRecoveryVars,
           IndexedAsyncStateAt, IndexedRecovery

THEOREM IndexedSuccessorActivationStepPreservesHistoricalRecoveryTargets ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedSuccessorActivationProgressStep(parentContext, node)
      => \A initialContext \in AdmissibleContextRecords:
           UNCHANGED IndexedScheduler(initialContext, 44)
BY Isa DEF IndexedSuccessorActivationProgressStep,
           BeginSuccessorActivation,
           BindAppliedSuccessorActivationToken,
           LatchAppliedSuccessorStartupFailure,
           LatchRecoveredSuccessorStartupFailure,
           RehydrateCleanCompleteTipSuccessorStartup,
           RehydrateFailedSuccessorStartup,
           AuthenticateRecoveredSuccessorActivation,
           OpenDeferredSuccessorAdapter,
           ConstructSuccessorRuntime,
           StartSuccessorServices,
           ApplySuccessorStartupEffects,
           ArmSuccessorClocks,
           PrepareSuccessorActivationMarker,
           OpenSuccessorIngress,
           ActivateAppliedSuccessorHeight,
           ActivateRecoveredSuccessorHeight,
           SuccessorActivationEnvironmentStutter,
           SuccessorActivationEnvironmentActivatesNode,
           IndexedAsync!AsyncEnterIndexedServiceActivation,
           IndexedAsync!AsyncActivateServiceNode,
           IndexedAsync!AsyncServiceActivationFrameVars,
           IndexedAsync!AsyncSchedulerExceptServiceActivation,
           IndexedAsyncStateAt, IndexedScheduler

THEOREM IndexedActionPreservesResponsiveRecoveryDormancy ==
  IndexedResponsiveRecoveryDormant /\ IndexedChainNext
    => IndexedResponsiveRecoveryDormant'
BY Isa, IndexedProductActionPreservesResponsiveRecoveryDormancy,
   IndexedSuccessorActivationStepPreservesRecoveryState
   DEF IndexedChainNext, JoinedContexts,
       IndexedResponsiveRecoveryDormant,
       IndexedRecovery

THEOREM IndexedStepPreservesResponsiveRecoveryDormancy ==
  IndexedResponsiveRecoveryDormant
    /\ [IndexedChainNext]_IndexedChainVars
    => IndexedResponsiveRecoveryDormant'
BY Isa, IndexedActionPreservesResponsiveRecoveryDormancy
   DEF IndexedChainVars, IndexedResponsiveRecoveryDormant,
       IndexedRecovery

THEOREM IndexedChainSpecKeepsResponsiveRecoveryDormant ==
  IndexedChainSpec => []IndexedResponsiveRecoveryDormant
PROOF
  <1>1. IndexedChainInit => IndexedResponsiveRecoveryDormant
    BY IndexedInitEstablishesResponsiveRecoveryDormancy
  <1>2. IndexedResponsiveRecoveryDormant
           /\ [IndexedChainNext]_IndexedChainVars
           => IndexedResponsiveRecoveryDormant'
    BY IndexedStepPreservesResponsiveRecoveryDormancy
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

(***************************************************************************
Local historical-service activation bridge.

Post-GST work needs only its own joined context and owner; it does not wait
for every Responsive peer.  A timed service owner is active by construction.
Component-44 coherence therefore maps that exact owner to the monotonically
joined product membership.  Historical recovery targets already have the
stronger routing witness in the composition invariant.
***************************************************************************)
THEOREM IndexedPostGstContextHasJoinedProductInstance ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      /\ IndexedCore(initialContext, 7)
      => initialContext \in JoinedContexts
BY DEF IndexedCompositionInvariant,
       IndexedPostGstContextJoinedCoherence

THEOREM IndexedPostGstActiveServiceOwnerHasJoinedProductInstance ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ IndexedCompositionInvariant
    /\ IndexedCore(initialContext, 7)
    /\ node \in IndexedAsync(initialContext)!AsyncActiveServiceNodes
    => /\ initialContext \in JoinedContexts
       /\ node \in joinedByContext[initialContext]
BY Isa, IndexedPostGstContextHasJoinedProductInstance
   DEF IndexedCompositionInvariant,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes

THEOREM IndexedHistoricalRecoveryTargetHasJoinedActiveOwner ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ IndexedCompositionInvariant
    /\ IndexedCore(initialContext, 7)
    /\ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
    => /\ initialContext \in JoinedContexts
       /\ node \in Responsive
       /\ node \in joinedByContext[initialContext]
       /\ node \in IndexedAsync(initialContext)!AsyncActiveServiceNodes
BY Isa, IndexedPostGstContextHasJoinedProductInstance
   DEF IndexedCompositionInvariant,
       IndexedHistoricalRecoveryTargetCoherence,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes

=============================================================================
