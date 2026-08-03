---- MODULE SumeragiV2ExactDecisionStageServiceClosureProofs ----
EXTENDS SumeragiV2ApplicationCompletionProofs,
        SumeragiV2HeightResetBoundaryClosureProofs,
        SumeragiV2AsyncHistoricalRecoveryClockOwnerActionProofs

(***************************************************************************
Exact durable-Decision service closure audit.

This module does not restate `ExactDecisionStageServiceProperty` and does not
add a fairness or liveness assumption to `AsyncSpecAt`.  It projects the
already-proved exact Decision source into:

  * one executable exact reducer owner;
  * one active exact certified request; or
  * the terminal application.

For executable owners it specializes the deferred Stage-2 fence to the exact
Decision node.  A durable Decision excludes a same-node pending InstallTC, and
the global view-local generation/rank invariant makes checked generation
exhaustion unreachable at every validator.  The resulting local Busy-phase
proof is complete in this module.

For active requests it names every concrete ownership handoff: retained
fanout alias, retransmission packet, normal/historical ingress, fresh-nonce
Serve job, authenticated response, response packet, response ingress, the
route-neutral certified-response claim, and the exact FetchCertifiedBody
owner.  The action leaves below establish the identity-preserving handoffs.
The temporal decomposition exposes four strict off-scheduler residuals:

  * active-request retransmission into an exact packet owner;
  * packet/runner reach into an exact request ingress handoff;
  * Serve FIFO exit into an exact authenticated response packet; and
  * exact response admission through the recipient-local singleton
    authenticated claim and finite normalized physical-completion owner.

The request-ingress residual is further decomposed below; its exact
admission/coalescing subcorridor has a source-level lifecycle rank and a
finite coalesced producer episode.  The other named leaves remain explicit,
and none of the new lifecycle theorems is evidence until a fresh strict
TLAPS run succeeds against this exact source.  These corridors are not
discharged by generic packet fairness.  A fresh
authenticated response first acquires its recipient's process-local claim; an
exact retransmission then coalesces by the route-neutral authenticated-envelope
projection.  A distinct local claim retains the target packet for retry.
Both `Chunk` and `CertifiedResponse` still use one physical completion owner
in the normalized resource lane.  Successful response drain atomically
retires every matching request alias and only its matching claim.
***************************************************************************)

(***************************************************************************
Exact source and executable-owner decomposition.
***************************************************************************)

ExactDecisionActiveRequestOwner(node, qc) ==
  /\ ExactDecisionServiceSource(node, qc)
  /\ ~NodeHasApplication(node)
  /\ ~BodyHeldBy(durableBodies, node, qc.context,
                  qc.view, qc.subject)
  /\ ~DecisionValidationHeld(node, qc)
  /\ DecisionCertifiedRequestActiveExact(node, qc)

ExactDecisionExecutableOwner(node, qc, candidate) ==
  /\ ExactDecisionServiceSource(node, qc)
  /\ DecisionExecutableStageOwner(node, qc, candidate)

THEOREM ExactDecisionServiceSourceDecomposition ==
  \A node, qc:
    ExactDecisionServiceSource(node, qc)
      => \/ NodeHasApplication(node)
         \/ ExactDecisionActiveRequestOwner(node, qc)
         \/ \E candidate \in AsyncCandidateSet:
              ExactDecisionExecutableOwner(node, qc, candidate)
BY ExactDecisionStageDecomposition, Isa
   DEF ExactDecisionServiceSource, ExactDecisionRecord,
       ExactDecisionActiveRequestOwner,
       ExactDecisionExecutableOwner

ExactDecisionPipelineRank(candidate) ==
  CASE candidate.kind = "FetchBody" -> 5
    [] candidate.kind = "FetchCertifiedBody" -> 4
    [] candidate.kind = "StoreBody" -> 3
    [] candidate.kind = "ValidateBody" -> 2
    [] candidate.kind = "Apply" -> 1
    [] OTHER -> 0

ExactDecisionRankedExecutableOwner(node, qc, candidate, rank) ==
  /\ ExactDecisionExecutableOwner(node, qc, candidate)
  /\ rank = ExactDecisionPipelineRank(candidate)
  /\ rank \in 1..5

THEOREM ExactDecisionExecutableOwnerHasPipelineRank ==
  \A node, qc, candidate:
    ExactDecisionExecutableOwner(node, qc, candidate)
      => \E rank \in 1..5:
           ExactDecisionRankedExecutableOwner(
             node, qc, candidate, rank)
BY Isa
   DEF ExactDecisionExecutableOwner,
       ExactDecisionRankedExecutableOwner,
       ExactDecisionPipelineRank,
       DecisionExecutableStageOwner

(***************************************************************************
Decision-local Stage-2 specialization.

The generic Stage-2 decomposition uses the strong view-local rank invariant to
exclude checked generation exhaustion.  The exact Decision source additionally
supplies the narrower same-node exclusion used by this projection.
The residual below is therefore temporal Busy-owner persistence/service, not
generation exhaustion.
***************************************************************************)

ExactDecisionStage2Owner(node, qc, candidate, position) ==
  /\ DecisionTimeoutFrontierInvariant
  /\ ExactDecisionExecutableOwner(node, qc, candidate)
  /\ CandidateServiceRank(candidate) = <<2, position>>
  /\ position \in Nat

THEOREM ExactDecisionStage2OwnerIsProtectedDeferred ==
  \A node, qc, candidate, position:
    ExactDecisionStage2Owner(node, qc, candidate, position)
      => /\ ResponsiveProtectedCandidateOwned(candidate)
         /\ candidate \in DeferredCandidates
         /\ candidate.node = node
         /\ ~InstallGenerationExhausted(node)
BY ExactDecisionExecutableOwnerIsResponsiveProtected,
   ExactDecisionSourceExcludesLocalInstallExhaustion, Isa
   DEF ExactDecisionStage2Owner,
       ExactDecisionExecutableOwner,
       ExactDecisionServiceSource,
       ExactDecisionRecord,
       DecisionExecutableStageOwner,
       DecisionPipelineCandidate,
       CandidateServiceRank

(***************************************************************************
Decision-local projections of the split Stage-2 kernel.

The production Stage-2 module owns the generic rank and exact deferred
handoff proof.  This module keeps only the projections needed to specialize
that proof to one durable Decision; it does not recreate the retired scratch
kernel or a second handoff model.
***************************************************************************)

ExactDecisionActiveBusyPhaseCarrier == 1..2

ExactDecisionBusyOrdering == OpToRel(<, Nat)

ExactDecisionProtectedBusyExit(target) ==
  \/ ProtectedServiceOwnershipExit(target)
  \/ NodeIdle(target.node)

THEOREM ExactDecisionStage2ReachableBusyPhaseCarrierObligation ==
  \A node:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ node \in ValidatorIds
    /\ ~NodeIdle(node)
    => BusyPhaseRank(node) \in ExactDecisionActiveBusyPhaseCarrier
BY BusyPhaseOwnerPartitionObligation, Isa
   DEF Stage2BusyKernelInvariant,
       ExactDecisionActiveBusyPhaseCarrier

THEOREM ExactDecisionProtectedStage2OwnedUnlessBusyExitObligation ==
  \A target:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ ProtectedStage2Owned(target)
    /\ ~ExactDecisionProtectedBusyExit(target)
    /\ [AsyncNext]_AsyncAllVars
    => \/ ProtectedStage2Owned(target)'
       \/ ExactDecisionProtectedBusyExit(target)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   BusyPhaseOwnerPartitionObligation,
   HeadTailProperties, IsaT(240)
   DEF Stage2BusyKernelInvariant,
       ExactDecisionProtectedBusyExit,
       ProtectedStage2Owned, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       ProtectedServiceCandidate, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, NodeIdle,
       RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer,
       LocalAdmissionStep, AdmitProducerCompletion,
       AdmitCausalHead, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, DeferredTagStep,
       DirectTimeoutStep, DirectRetransmitStep,
       IdleRuntimeStep, RemoveNextNodeCommand,
       RemoveNextDeferredCommand, DeferCommand, DiscardCommand,
       ExecuteCommand, ExecuteRegularCommand,
       ExecuteDecisionFetch, ExecuteSignProposal, ExecuteSignVote,
       ExecuteFormPrepareQC, ExecuteSignTimeout,
       ExecutePersistInstall, ExecutePersistDecision,
       ExecuteRequestCertifiedBody, ExecuteApply,
       ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, OpenHistoricalRecovery,
       CommitCertificateDiscoveryStepWork,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstCrash,
       PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery,
       AsyncTick, AsyncSetGST, AsyncNext,
       AsyncNonCrashStep, AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncAllVars

THEOREM ExactDecisionBusyOrderingWellFoundedObligation ==
  IsWellFoundedOn(
    ExactDecisionBusyOrdering,
    ExactDecisionActiveBusyPhaseCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset, Isa
   DEF ExactDecisionBusyOrdering,
       ExactDecisionActiveBusyPhaseCarrier

THEOREM ExactDecisionBusyCompletionIsLocallyDispatchable ==
  \A node, qc, target, position, witness:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionStage2Owner(node, qc, target, position)
    /\ witness \in BusyCompletionCandidates(node)
    => CommandDispatchable(witness)
BY BusyCompletionCandidateIsDispatchable,
   ExactDecisionStage2OwnerIsProtectedDeferred, Isa

ExactDecisionBusyWitness(node, qc, target, position, witness) ==
  /\ ExactDecisionStage2Owner(node, qc, target, position)
  /\ BusyPhaseRank(node) \in ExactDecisionActiveBusyPhaseCarrier
  /\ witness \in BusyCompletionCandidates(node)
  /\ ProtectedBusyCompletionWitness(target, witness)

THEOREM ExactDecisionBusyWitnessHasPostDeferredRank ==
  \A node, qc, target, position, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ExactDecisionBusyWitness(
         node, qc, target, position, witness)
    => /\ ResponsiveProtectedCandidateOwned(witness)
       /\ CandidateServiceRank(witness)
            \in PostDeferredServiceRankCarrier
BY ProtectedBusyWitnessHasPostDeferredRankObligation
   DEF ExactDecisionBusyWitness

(***************************************************************************
This one-step lemma is the exact replacement for the global-budget premise
in `BusyWitnessOwnershipPersistsUntilTargetExitOrPhaseDrop`.  It uses the
durable-Decision exclusion above for the target node only.
***************************************************************************)

THEOREM ExactDecisionBusyWitnessPersistsLocally ==
  \A node, qc, target, position, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ ExactDecisionBusyWitness(
         node, qc, target, position, witness)
    /\ [AsyncNext]_AsyncAllVars
    /\ ExactDecisionExecutableOwner(node, qc, target)'
    /\ BusyPhaseRank(node)' >= BusyPhaseRank(node)
    => ProtectedBusyCompletionWitness(target, witness)'
BY BusyPhaseOwnerPartitionObligation,
   BusyCompletionExecutionDropsPhaseObligation,
   ExactDecisionBusyCompletionIsLocallyDispatchable,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   RuntimeSelectedCommandsAreTyped,
   ProgressCoreStutterAndCarrierGrowthRetainsBusyCandidates,
   ProgressCoreStutterKeepsBusyWitnessWhenCarried,
   HeadTailProperties, IsaT(240)
   DEF ExactDecisionBusyWitness,
       ExactDecisionStage2Owner,
       ExactDecisionExecutableOwner,
       ProtectedBusyCompletionWitness,
       ProtectedStage2Owned,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateScheduled, CandidateServiceRank,
       BusyPhaseRank, Stage2TwoStepBusyNodes,
       Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
       Stage2OneStepBusyOwners, BusyCompletionCandidates,
       ActiveBusyCompletionCarrier, SerializedBusyOwners,
       SerializedBusyOwnershipInvariant, RequestsUniqueByNode,
       RequestNodeSet, NodeIdle, PendingNodes, SigningNodes,
       AllPendingRequests, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       CandidateConsumerCurrent, CommandDispatchable,
       CommandExecutionReady, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       DeferCommand, DiscardCommand, ExecuteCommand,
       ExecuteRegularCommand, ExecuteDecisionFetch,
       ExecuteSignProposal, ExecuteSignVote, ExecuteFormPrepareQC,
       ExecuteSignTimeout, ExecutePersistInstall,
       ExecutePersistDecision, ExecuteRequestCertifiedBody,
       ExecuteApply, ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk, AppendCausalSuccessors,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork, AsyncNetworkStep,
       AdmitIngressPacket, AsyncFaultStep, PreGstCrash,
       PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery,
       AsyncTick, AsyncSetGST, AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

ExactDecisionStage2BusyExit(node, qc, target) ==
  \/ ~ExactDecisionExecutableOwner(node, qc, target)
  \/ NodeIdle(node)

(***************************************************************************
Narrow temporal Stage-2 kernel.  This is not the application target: it says
only that one exact deferred candidate either leaves exact ownership or
reaches the already-proved idle deferred-drain corridor.  No live-budget
predicate appears in its statement.
***************************************************************************)

ExactDecisionStage2BusyClosureProperty(specification) ==
  specification
    => \A node, qc, target, position:
         (ExactDecisionStage2Owner(node, qc, target, position)
            /\ ~NodeIdle(node))
           ~> ExactDecisionStage2BusyExit(node, qc, target)

ExactDecisionStage2BusyAtPhase(node, qc, target, phase) ==
  /\ \E position \in Nat:
       ExactDecisionStage2Owner(node, qc, target, position)
  /\ ~NodeIdle(node)
  /\ BusyPhaseRank(node) = phase

ExactDecisionStage2BusyPhaseGoal(node, qc, target, phase) ==
  \/ ExactDecisionStage2BusyExit(node, qc, target)
  \/ \E lower \in SetLessThan(
       phase, ExactDecisionBusyOrdering,
       ExactDecisionActiveBusyPhaseCarrier):
       ExactDecisionStage2BusyAtPhase(
         node, qc, target, lower)

ExactDecisionBusyWitnessBlocked(
    node, qc, target, phase, witness) ==
  /\ BusyPhaseRank(node) = phase
  /\ \E position \in Nat:
       ExactDecisionBusyWitness(
         node, qc, target, position, witness)

(***************************************************************************
The generic Stage-2 temporal proof needs the global Install-generation
budget only to eliminate the exhausted-Install arm of
`BusyCompletionWitnessInvariant`.  The exact durable Decision eliminates that
arm at this node, so the same concrete Busy witness exists without assuming
anything about unrelated validators.
***************************************************************************)

THEOREM ExactDecisionStage2BusyHasLocalWitness ==
  \A node, qc, target, position:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ ExactDecisionStage2Owner(node, qc, target, position)
    /\ ~NodeIdle(node)
    => \E witness \in AsyncCandidateSet:
         ExactDecisionBusyWitnessBlocked(
           node, qc, target, BusyPhaseRank(node), witness)
BY ExactDecisionStage2OwnerIsProtectedDeferred,
   ActiveBusyCompletionCarrierIsTyped,
   AsyncCurrentResponsiveVotersAreValidators,
   BusyPhaseOwnerPartitionObligation,
   ExactDecisionStage2ReachableBusyPhaseCarrierObligation, Isa
   DEF ExactDecisionBusyWitnessBlocked,
       ExactDecisionBusyWitness,
       Stage2BusyKernelInvariant,
       AsyncProgressOwnershipInvariant,
       BusyCompletionWitnessInvariant,
       BusyCompletionCandidates,
       ActiveBusyCompletionCarrier,
       AsyncStrongTypeInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant

THEOREM ExactDecisionProtectedStage2ReconstructsPosition ==
  \A node, qc, target:
    /\ AsyncStrongTypeInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ ExactDecisionExecutableOwner(node, qc, target)
    /\ ProtectedStage2Owned(target)
    => \E position \in Nat:
         ExactDecisionStage2Owner(
           node, qc, target, position)
BY AsyncStrongTypeProjectsAsyncType,
   ScheduledCandidateServiceRankInCarrier, Isa
   DEF ExactDecisionStage2Owner,
       ExactDecisionExecutableOwner,
       ProtectedStage2Owned,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned,
       CandidateServiceRank, DeferredCandidates,
       OwnedServiceRankCarrier

THEOREM ExactDecisionBlockedBusyWitnessPersistsOrLowersPhase ==
  \A node, qc, target, phase, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ ExactDecisionBusyWitnessBlocked(
         node, qc, target, phase, witness)
    /\ [AsyncNext]_AsyncAllVars
    => \/ ExactDecisionStage2BusyPhaseGoal(
             node, qc, target, phase)'
       \/ ExactDecisionBusyWitnessBlocked(
            node, qc, target, phase, witness)'
PROOF
  <1>1. ASSUME NEW node, NEW qc, NEW target, NEW phase, NEW witness,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                Stage2BusyKernelInvariant,
                DecisionTimeoutFrontierInvariant,
                ExactDecisionBusyWitnessBlocked(
                  node, qc, target, phase, witness),
                [AsyncNext]_AsyncAllVars
         PROVE \/ ExactDecisionStage2BusyPhaseGoal(
                      node, qc, target, phase)'
               \/ ExactDecisionBusyWitnessBlocked(
                    node, qc, target, phase, witness)'
    <2>1. PICK position \in Nat:
           ExactDecisionBusyWitness(
             node, qc, target, position, witness)
      BY <1>1 DEF ExactDecisionBusyWitnessBlocked
    <2>2. /\ ProtectedStage2Owned(target)
           /\ ~ExactDecisionProtectedBusyExit(target)
           /\ BusyPhaseRank(node)
                \in ExactDecisionActiveBusyPhaseCarrier
      BY <1>1, <2>1,
         ExactDecisionStage2OwnerIsProtectedDeferred,
         BusyPhaseOwnerPartitionObligation, Isa
         DEF ExactDecisionBusyWitness,
             ProtectedBusyCompletionWitness,
             ExactDecisionProtectedBusyExit,
             ProtectedServiceOwnershipExit
    <2>3. /\ AsyncStrongTypeInvariant'
           /\ AsyncProgressOwnershipInvariant'
           /\ Stage2BusyKernelInvariant'
           /\ DecisionTimeoutFrontierInvariant'
      BY <1>1,
         AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesProgressOwnership,
         Stage2BusyKernelNextObligation,
         AsyncBracketPreservesDecisionTimeoutFrontier
    <2>4. CASE ExactDecisionStage2BusyPhaseGoal(
                  node, qc, target, phase)'
      BY <2>4
    <2>5. CASE ~ExactDecisionStage2BusyPhaseGoal(
                   node, qc, target, phase)'
      <3>1. /\ ExactDecisionExecutableOwner(
                   node, qc, target)'
             /\ ~NodeIdle(node)'
        BY <2>5
           DEF ExactDecisionStage2BusyPhaseGoal,
               ExactDecisionStage2BusyExit
      <3>2. ~ExactDecisionProtectedBusyExit(target)'
        BY <3>1, ExactDecisionExecutableOwnerIsResponsiveProtected
           DEF ExactDecisionProtectedBusyExit,
               ProtectedServiceOwnershipExit
      <3>3. ProtectedStage2Owned(target)'
        BY <1>1, <2>2, <3>2,
           ExactDecisionProtectedStage2OwnedUnlessBusyExitObligation
      <3>4. BusyPhaseRank(node)' <= phase
        BY <1>1, <2>2, <3>1,
           BusyPhaseCannotIncreaseWhileProtected
      <3>5. \E nextPosition \in Nat:
               ExactDecisionStage2Owner(
                 node, qc, target, nextPosition)'
        BY <2>3, <3>1, <3>3,
           ExactDecisionProtectedStage2ReconstructsPosition
      <3>6. BusyPhaseRank(node)' >= phase
        <4>1. CASE BusyPhaseRank(node)' < phase
          <5>1. BusyPhaseRank(node)'
                   \in ExactDecisionActiveBusyPhaseCarrier
            BY <2>3, <3>3, <3>1,
               ExactDecisionStage2ReachableBusyPhaseCarrierObligation,
               Isa
               DEF ProtectedStage2Owned,
                   ResponsiveProtectedCandidateOwned,
                   ProtectedCandidateOwned
          <5>2. BusyPhaseRank(node)'
                   \in SetLessThan(
                        phase, ExactDecisionBusyOrdering,
                        ExactDecisionActiveBusyPhaseCarrier)
            BY <4>1, <5>1
               DEF SetLessThan, ExactDecisionBusyOrdering, OpToRel
          <5>3. ExactDecisionStage2BusyAtPhase(
                   node, qc, target, BusyPhaseRank(node))'
            BY <3>1, <3>5
               DEF ExactDecisionStage2BusyAtPhase
          <5>4. ExactDecisionStage2BusyPhaseGoal(
                   node, qc, target, phase)'
            BY <5>2, <5>3
               DEF ExactDecisionStage2BusyPhaseGoal
          <5> QED BY <2>5, <5>4
        <4>2. CASE BusyPhaseRank(node)' >= phase
          BY <4>2
        <4> QED BY <4>1, <4>2
      <3>7. ProtectedBusyCompletionWitness(target, witness)'
        BY <1>1, <2>1, <3>1, <3>6,
           ExactDecisionBusyWitnessPersistsLocally
      <3>8. ExactDecisionBusyWitnessBlocked(
               node, qc, target, phase, witness)'
        BY <2>3, <3>1, <3>4, <3>5, <3>6, <3>7, Isa
           DEF ExactDecisionBusyWitnessBlocked,
               ExactDecisionBusyWitness,
               ExactDecisionStage2Owner,
               ProtectedBusyCompletionWitness
      <3> QED BY <3>8
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM ExactDecisionBusyPhaseTakesWellFoundedStep ==
  \A initialContext, node, qc, target:
    \A phase \in ExactDecisionActiveBusyPhaseCarrier:
      ProtectedServiceFiniteRunnerEpisodeClosureProperty(
        AsyncSpecAt(initialContext))
        => (AsyncSpecAt(initialContext)
              => ExactDecisionStage2BusyAtPhase(
                   node, qc, target, phase)
                   ~> ExactDecisionStage2BusyPhaseGoal(
                        node, qc, target, phase))
PROOF
  <1>1. ASSUME NEW initialContext, NEW node, NEW qc, NEW target,
                NEW phase \in ExactDecisionActiveBusyPhaseCarrier,
                ProtectedServiceFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => ExactDecisionStage2BusyAtPhase(
                      node, qc, target, phase)
                      ~> ExactDecisionStage2BusyPhaseGoal(
                           node, qc, target, phase)
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE ExactDecisionStage2BusyAtPhase(
                   node, qc, target, phase)
                   ~> ExactDecisionStage2BusyPhaseGoal(
                        node, qc, target, phase)
      <3>1. [](AsyncStrongTypeInvariant
                /\ AsyncProgressOwnershipInvariant
                /\ Stage2BusyKernelInvariant
                /\ DecisionTimeoutFrontierInvariant)
        BY <2>1, AsyncSpecAlwaysStrongTypeInvariant,
           AsyncSpecAlwaysProgressOwnershipInvariant,
           AsyncSpecAlwaysStage2BusyKernelObligation,
           DecisionTimeoutFrontierInvariantFromAsyncSpec, PTL
      <3>2. ProtectedPostDeferredRankProgressProperty(
               AsyncSpecAt(initialContext))
        BY FairProtectedStage3RankProgress,
           ProtectedStage4RankProgressFromFairScheduler,
           ProtectedStage5RankProgressFromFairFifo,
           FairProtectedStage6RankProgress,
           ProtectedPostDeferredRanksComposeFromLeavesObligation
           DEF ProtectedServiceFiniteRunnerEpisodeClosureProperty,
               Stage6FiniteRunnerEpisodeClosureProperty
      <3>3. \A witness \in AsyncCandidateSet:
               ExactDecisionBusyWitnessBlocked(
                 node, qc, target, phase, witness)
                 ~> ProtectedPostDeferredExit(witness)
        <4>1. ASSUME NEW witness \in AsyncCandidateSet
               PROVE ExactDecisionBusyWitnessBlocked(
                       node, qc, target, phase, witness)
                       ~> ProtectedPostDeferredExit(witness)
          <5>1. [](ExactDecisionBusyWitnessBlocked(
                     node, qc, target, phase, witness)
                     => /\ ResponsiveProtectedCandidateOwned(witness)
                        /\ CandidateServiceRank(witness)
                             \in PostDeferredServiceRankCarrier)
            BY <3>1,
               ExactDecisionBusyWitnessHasPostDeferredRank, PTL
               DEF ExactDecisionBusyWitnessBlocked
          <5>2. (gst
                    /\ ResponsiveProtectedCandidateOwned(witness)
                    /\ CandidateServiceRank(witness)[1] \in 3..6)
                   ~> ProtectedPostDeferredExit(witness)
            BY <2>1, <3>2,
               PostDeferredRankProgressConvergesObligation
          <5> QED BY <5>1, <5>2, PTL
               DEF ExactDecisionBusyWitnessBlocked,
                   ExactDecisionBusyWitness,
                   ProtectedBusyCompletionWitness,
                   PostDeferredServiceRankCarrier
        <4> QED BY <4>1
      <3>4. \A witness \in AsyncCandidateSet:
               ExactDecisionBusyWitnessBlocked(
                 node, qc, target, phase, witness)
                 ~> ExactDecisionStage2BusyPhaseGoal(
                      node, qc, target, phase)
        <4>1. ASSUME NEW witness \in AsyncCandidateSet
               PROVE ExactDecisionBusyWitnessBlocked(
                       node, qc, target, phase, witness)
                       ~> ExactDecisionStage2BusyPhaseGoal(
                            node, qc, target, phase)
          <5>1. ExactDecisionBusyWitnessBlocked(
                   node, qc, target, phase, witness)
                   /\ [AsyncNext]_AsyncAllVars
                  => \/ ExactDecisionStage2BusyPhaseGoal(
                           node, qc, target, phase)'
                     \/ ExactDecisionBusyWitnessBlocked(
                          node, qc, target, phase, witness)'
            BY <3>1,
               ExactDecisionBlockedBusyWitnessPersistsOrLowersPhase
          <5>2. [](ExactDecisionBusyWitnessBlocked(
                     node, qc, target, phase, witness)
                     /\ ProtectedPostDeferredExit(witness)
                    => FALSE)
            BY <3>1,
               ExactDecisionBusyWitnessHasPostDeferredRank, PTL
               DEF ExactDecisionBusyWitnessBlocked,
                   ProtectedPostDeferredExit,
                   PostDeferredServiceRankCarrier
          <5> QED BY <3>3, <5>1, <5>2, PTL
               DEF AsyncSpecAt
        <4> QED BY <4>1
      <3>5. [](ExactDecisionStage2BusyAtPhase(
                 node, qc, target, phase)
                 => \E witness \in AsyncCandidateSet:
                      ExactDecisionBusyWitnessBlocked(
                        node, qc, target, phase, witness))
        BY <3>1, ExactDecisionStage2BusyHasLocalWitness, PTL
           DEF ExactDecisionStage2BusyAtPhase
      <3> QED BY <3>4, <3>5, PTL
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ExactDecisionStage2BusyClosure ==
  \A initialContext:
    ProtectedServiceFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => ExactDecisionStage2BusyClosureProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ProtectedServiceFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext))
         PROVE ExactDecisionStage2BusyClosureProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node, qc, target, position:
             (ExactDecisionStage2Owner(
                node, qc, target, position)
                /\ ~NodeIdle(node))
               ~> ExactDecisionStage2BusyExit(
                    node, qc, target)
      <3>1. ASSUME NEW node, NEW qc, NEW target, NEW position
             PROVE (ExactDecisionStage2Owner(
                      node, qc, target, position)
                      /\ ~NodeIdle(node))
                     ~> ExactDecisionStage2BusyExit(
                          node, qc, target)
        <4>1. \A phase \in ExactDecisionActiveBusyPhaseCarrier:
                 ExactDecisionStage2BusyAtPhase(
                   node, qc, target, phase)
                   ~> ExactDecisionStage2BusyPhaseGoal(
                        node, qc, target, phase)
          BY <2>1, ExactDecisionBusyPhaseTakesWellFoundedStep
        <4>2. \A phase \in ExactDecisionActiveBusyPhaseCarrier:
                 ExactDecisionStage2BusyAtPhase(
                   node, qc, target, phase)
                   ~> ExactDecisionStage2BusyExit(
                        node, qc, target)
          BY <4>1, ExactDecisionBusyOrderingWellFoundedObligation,
             WellFoundedLeadsTo
             DEF ExactDecisionStage2BusyPhaseGoal
        <4>3. AsyncSpecAt(initialContext)
                 => [](AsyncStrongTypeInvariant
                       /\ AsyncProgressOwnershipInvariant
                       /\ Stage2BusyKernelInvariant)
          BY AsyncSpecAlwaysStrongTypeInvariant,
             AsyncSpecAlwaysProgressOwnershipInvariant,
             AsyncSpecAlwaysStage2BusyKernelObligation, PTL
        <4>4. [](ExactDecisionStage2Owner(
                   node, qc, target, position)
                   /\ ~NodeIdle(node)
                  => \E phase
                       \in ExactDecisionActiveBusyPhaseCarrier:
                       ExactDecisionStage2BusyAtPhase(
                         node, qc, target, phase))
          BY <4>3, ExactDecisionStage2OwnerIsProtectedDeferred,
             ExactDecisionStage2ReachableBusyPhaseCarrierObligation,
             BusyPhaseOwnerPartitionObligation, Isa, PTL
             DEF ExactDecisionStage2BusyAtPhase,
                 ProtectedStage2Owned
        <4> QED BY <4>2, <4>4, PTL
      <3> QED BY <3>1
    <2> QED BY <2>1
         DEF ExactDecisionStage2BusyClosureProperty
  <1> QED BY <1>1

ExactDecisionCandidateExitProperty(specification) ==
  specification
    => \A node, qc, candidate:
         ExactDecisionExecutableOwner(node, qc, candidate)
           ~> ~ExactDecisionExecutableOwner(node, qc, candidate)

THEOREM ExactDecisionLocalStage2ClosesPhysicalOwnerExit ==
  \A initialContext:
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ ExactDecisionStage2BusyClosureProperty(
         AsyncSpecAt(initialContext))
      => ExactDecisionCandidateExitProperty(
           AsyncSpecAt(initialContext))
BY FairProtectedStage3RankProgress,
   ProtectedStage4RankProgressFromFairScheduler,
   ProtectedStage5RankProgressFromFairFifo,
   FairProtectedStage6RankProgress,
   ProtectedPostDeferredRanksComposeFromLeavesObligation,
   AsyncSpecAlwaysStage2BusyKernelObligation,
   ProtectedStage2RankProgressWithExactHandoffObligation,
   ExactDecisionExecutableOwnerIsResponsiveProtected,
   ScheduledCandidateServiceRankInCarrier,
   ProtectedRankExitHasWellFoundedSuccessor,
   OwnedServiceRankOrderingWellFounded,
   WellFoundedLeadsTo, PTL
   DEF ExactDecisionStage2BusyClosureProperty,
       ExactDecisionStage2BusyExit,
       ExactDecisionCandidateExitProperty,
       ExactDecisionExecutableOwner,
       ExactDecisionStage2Owner,
       ProtectedPostDeferredRankProgressProperty,
       ProtectedOwnedAtServiceRank,
       ProtectedServiceOwnershipExit,
       OwnedServiceRankCarrier,
       Stage2RankProgressExit,
       Stage2HandoffRankBlocked,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned,
       ProtectedServiceFiniteRunnerEpisodeClosureProperty,
       Stage6FiniteRunnerEpisodeClosureProperty

(***************************************************************************
Reducer semantic handoffs after physical ownership exit.
***************************************************************************)

ExactDecisionCandidatePostMilestone(node, qc, candidate) ==
  \/ NodeHasApplication(node)
  \/ /\ candidate.kind = "FetchBody"
        /\ ExactDecisionActiveRequestOwner(node, qc)
  \/ \E successor \in AsyncCandidateSet:
       /\ ExactDecisionExecutableOwner(node, qc, successor)
       /\ ExactDecisionPipelineRank(successor)
            < ExactDecisionPipelineRank(candidate)

ExactDecisionOwnerExitStep(node, qc, candidate) ==
  /\ gst
  /\ ExactDecisionExecutableOwner(node, qc, candidate)
  /\ AsyncNext
  /\ ~ExactDecisionExecutableOwner(node, qc, candidate)'

UnexplainedExactDecisionOwnerExit(node, qc, candidate) ==
  /\ ExactDecisionOwnerExitStep(node, qc, candidate)
  /\ ~ExactDecisionCandidatePostMilestone(node, qc, candidate)'

THEOREM ExactDecisionOwnerExitHasSemanticHandoff ==
  \A node, qc, candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ ExactDecisionOwnerExitStep(node, qc, candidate)
    => ExactDecisionCandidatePostMilestone(node, qc, candidate)'
BY DecisionExecutableStageOwnerIsDispatchable,
   SelectedExactFifoOwnerCannotDeferOrDiscard,
   SelectedExactDeferredOwnerCannotDiscard,
   FifoSuccessfulExecutionSchedulesEverySuccessor,
   DeferredSuccessfulExecutionSchedulesEverySuccessor,
   ExactDecisionFetchMissingBodyOpensCertifiedRequest,
   ExactDecisionFetchHeldBodySchedulesValidation,
   ExactCertifiedFetchStagesBodyAndSchedulesStore,
   DecisionStoreSchedulesValidation,
   DecisionValidationSchedulesApply,
   DecisionApplyCreatesTerminalStage,
   ExactDecisionStageDecomposition, IsaT(300)
   DEF ExactDecisionOwnerExitStep,
       ExactDecisionCandidatePostMilestone,
       ExactDecisionExecutableOwner,
       ExactDecisionActiveRequestOwner,
       ExactDecisionPipelineRank,
       ExactDecisionServiceSource,
       ExactDecisionRecord,
       DecisionExecutableStageOwner,
       CommandSuccessorsScheduledAfter,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunNodeWork,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, DiscardCommand, DeferCommand,
       ExecuteCommand, ExecuteRegularCommand,
       ExecuteDecisionFetch, ExecuteApply,
       CandidateScheduled, AsyncAllVars

THEOREM NoUnexplainedExactDecisionOwnerExit ==
  \A node, qc, candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ FinalProgressWitnessClosureInvariant
    => ~UnexplainedExactDecisionOwnerExit(node, qc, candidate)
BY ExactDecisionOwnerExitHasSemanticHandoff
   DEF UnexplainedExactDecisionOwnerExit

ExactDecisionCandidateTerminal(node, qc, candidate) ==
  \/ NodeHasApplication(node)
  \/ /\ candidate.kind = "FetchBody"
        /\ ExactDecisionActiveRequestOwner(node, qc)

ExactDecisionCandidatePipelineProperty(specification) ==
  specification
    => \A node, qc, candidate:
         ExactDecisionExecutableOwner(node, qc, candidate)
           ~> ExactDecisionCandidateTerminal(node, qc, candidate)

THEOREM ExactDecisionPhysicalExitClosesCandidatePipeline ==
  \A initialContext:
    ExactDecisionCandidateExitProperty(AsyncSpecAt(initialContext))
      => ExactDecisionCandidatePipelineProperty(
           AsyncSpecAt(initialContext))
BY ExactDecisionOwnerExitHasSemanticHandoff,
   ExactDecisionExecutableOwnerHasPipelineRank,
   FinalProgressWitnessClosureInvariantObligation,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   DecisionFrontierUniquenessInvariantFromAsyncSpec,
   DecisionTimeoutFrontierInvariantFromAsyncSpec,
   NatLessThanWellFounded, WellFoundedLeadsTo, PTL
   DEF ExactDecisionCandidateExitProperty,
       ExactDecisionCandidatePipelineProperty,
       ExactDecisionCandidateTerminal,
       ExactDecisionCandidatePostMilestone,
       ExactDecisionRankedExecutableOwner,
       ExactDecisionPipelineRank

(***************************************************************************
Exact fanout alias and wire milestones.
***************************************************************************)

ExactDecisionFanoutRetentionInvariant ==
  \A node, qc:
    /\ ExactDecisionRecord(node, qc)
    /\ DecisionCertifiedRequestActiveExact(node, qc)
    => CertifiedRequestOutbox(node, qc) \subseteq asyncActiveRequests

(***************************************************************************
A durable Decision also isolates its request authority. PersistDecision
filters older local body requests before the exact recovery successor is
published, and every later fanout alias carries the one frozen signed request
hash. Thus a competing response at this recipient may come from another
archive relay, but it cannot belong to a different Decision request. Draining
such an archive alternative is the same exact recovery handoff.
***************************************************************************)

ExactDecisionRequestAuthorityIsolationInvariant ==
  \A node, qc:
    /\ ExactDecisionRecord(node, qc)
    /\ DecisionCertifiedRequestActiveExact(node, qc)
    => \A request \in asyncActiveRequests:
         /\ request.kind = "CertifiedRequest"
         /\ request.source = node
         => AsyncCertifiedRequestHash(request)
              = AsyncCertifiedRequestHashOf(node, qc, 0)

THEOREM AsyncInitEstablishesExactDecisionFanoutRetention ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => ExactDecisionFanoutRetentionInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       ExactDecisionFanoutRetentionInvariant,
       DecisionCertifiedRequestActiveExact

THEOREM AsyncInitEstablishesExactDecisionRequestAuthorityIsolation ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => ExactDecisionRequestAuthorityIsolationInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       ExactDecisionRequestAuthorityIsolationInvariant,
       ExactDecisionRecord

THEOREM AsyncNextPreservesExactDecisionFanoutRetention ==
  /\ AsyncStrongTypeInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ ExactDecisionFanoutRetentionInvariant
  /\ [AsyncNext]_AsyncAllVars
  => ExactDecisionFanoutRetentionInvariant'
BY ExactDecisionCertifiedRequestBindsHashAndArchiveRoute,
   DurableDecisionNodeCannotOwnPendingInstall,
   ExactCertifiedResponseMatchesDecisionRequestHash,
   CertifiedArchiveRoutesStableUnderContextFrame, IsaT(300)
   DEF ExactDecisionFanoutRetentionInvariant,
       DecisionCertifiedRequestActiveExact,
       CertifiedRequestOutbox, MatchingCertifiedRequests,
       PublishCertifiedRequests, PersistInstalledControl,
       CertifiedRequestSurvivesInstall,
       DrainFairIngressSelected, PreGstResponsiveCrash, AsyncNext,
       AsyncNonCrashStep, AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncAllVars

THEOREM AsyncNextPreservesExactDecisionRequestAuthorityIsolation ==
  /\ AsyncStrongTypeInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ ExactDecisionFanoutRetentionInvariant
  /\ ExactDecisionRequestAuthorityIsolationInvariant
  /\ [AsyncNext]_AsyncAllVars
  => ExactDecisionRequestAuthorityIsolationInvariant'
BY ExactDecisionCertifiedRequestBindsHashAndArchiveRoute,
   ExactCertifiedResponseMatchesDecisionRequestHash,
   DurableDecisionNodeCannotOwnPendingInstall,
   CertifiedArchiveRoutesStableUnderContextFrame, IsaT(300)
   DEF ExactDecisionRequestAuthorityIsolationInvariant,
       ExactDecisionFanoutRetentionInvariant,
       DecisionCertifiedRequestActiveExact,
       CertifiedRequestOutbox,
       AsyncCertifiedRequestHashOf,
       PublishCertifiedRequests,
       PersistDecisionControl,
       CertifiedRequestSurvivesDecision,
       PersistInstalledControlAfterInstall,
       CertifiedRequestSurvivesInstall,
       FilterCertifiedResponseAuthority,
       DrainFairIngressSelected,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncAllVars

THEOREM AsyncSpecAlwaysRetainsExactDecisionFanout ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []ExactDecisionFanoutRetentionInvariant
BY AsyncInitEstablishesExactDecisionFanoutRetention,
   AsyncNextPreservesExactDecisionFanoutRetention,
   AsyncSpecAlwaysStrongTypeInvariant,
   DecisionTimeoutFrontierInvariantFromAsyncSpec,
   PTL DEF AsyncSpecAt

THEOREM AsyncSpecAlwaysIsolatesExactDecisionRequestAuthority ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []ExactDecisionRequestAuthorityIsolationInvariant
BY AsyncInitEstablishesExactDecisionRequestAuthorityIsolation,
   AsyncNextPreservesExactDecisionRequestAuthorityIsolation,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   DecisionFrontierUniquenessInvariantFromAsyncSpec,
   DecisionTimeoutFrontierInvariantFromAsyncSpec,
   AsyncSpecAlwaysRetainsExactDecisionFanout,
   PTL DEF AsyncSpecAt

ExactDecisionBodyHoldingAlias(node, qc, archive, request) ==
  /\ ExactDecisionActiveRequestOwner(node, qc)
  /\ archive \in AsyncCurrentResponsiveVoters \ {node}
  /\ BodyHeldBy(durableBodies, archive, qc.context,
                qc.view, qc.subject)
  /\ request \in CertifiedRequestOutbox(node, qc)
  /\ request.envelope.recipient = archive
  /\ request \in asyncActiveRequests
  /\ CertifiedServeCanRespond(archive, request)

THEOREM ExactDecisionRequestHasResponsiveBodyHoldingAlias ==
  \A node, qc:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionFanoutRetentionInvariant
    /\ ExactDecisionActiveRequestOwner(node, qc)
    => \E archive \in AsyncCurrentResponsiveVoters,
          request \in asyncActiveRequests:
         ExactDecisionBodyHoldingAlias(
           node, qc, archive, request)
BY DecisionRecoveryCertificateHasResponsiveRemoteBodySource,
   ExactDecisionCertifiedRequestBindsHashAndArchiveRoute, IsaT(180)
   DEF ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionExecutableOwner,
       ExactDecisionServiceSource, ExactDecisionRecord,
       DecisionRecoveryCertificate,
       DecisionCertifiedRequestActiveExact,
       ExactDecisionFanoutRetentionInvariant,
       CertifiedRequestOutbox, CertifiedArchiveRoutes,
       AsyncResponsiveArchiveServers,
       CertifiedServeCanRespond,
       AsyncStrongTypeInvariant, StrongInductiveInvariant

ExactDecisionRequestPacketOwned(
    node, qc, archive, request, packet) ==
  /\ ExactDecisionBodyHoldingAlias(node, qc, archive, request)
  /\ packet \in asyncTransport
  /\ packet.item = request

ExactDecisionRequestIngressOwned(
    node, qc, archive, request) ==
  /\ ExactDecisionBodyHoldingAlias(node, qc, archive, request)
  /\ request \in
       SequenceSet(
         IngressLane(archive, IngressResourceSource(request)))

ExactDecisionServeLifecycleIdentity(archive, request) ==
  AsyncServeLogicalRequestIdentity(archive, request)

ExactDecisionServeAdmissionOwned(archive, request) ==
  LET identity ==
        ExactDecisionServeLifecycleIdentity(archive, request)
  IN /\ AsyncServeLiveReservationOwned(archive, identity)
     /\ AsyncServeAdmissionOrdinal(archive, identity) \in Nat \ {0}

ExactDecisionServeOccurrenceOwned(archive, request, job) ==
  LET identity ==
        ExactDecisionServeLifecycleIdentity(archive, request)
  IN /\ ExactDecisionServeAdmissionOwned(archive, request)
     /\ AsyncServeJobQueued(archive, identity)
     /\ job \in SequenceSet(asyncIoQueues[archive])
     /\ job.class = "Serve"
     /\ job.candidate.item = request
     /\ AsyncIoServeJobIdentity(archive, job) = identity
     /\ job.nonce \in 0..AsyncIoAuxCapacity

ExactDecisionServeJobOwned(
    node, qc, archive, request, job) ==
  /\ ExactDecisionBodyHoldingAlias(node, qc, archive, request)
  /\ ExactDecisionServeOccurrenceOwned(archive, request, job)

ExactDecisionServeTombstoneOwned(
    node, qc, archive, request) ==
  LET identity ==
        ExactDecisionServeLifecycleIdentity(archive, request)
  IN /\ ExactDecisionBodyHoldingAlias(node, qc, archive, request)
     /\ AsyncServeLifecycleTombstone(archive, identity)
     /\ AsyncServeTombstoneOutputs(archive, identity) # {}
     /\ \A response \in
          AsyncServeTombstoneOutputs(archive, identity):
          DecisionCertifiedResponseLineageExact(node, qc, response)

ExactDecisionAuthenticatedResponse(
    node, qc, archive, request, response) ==
  /\ ExactDecisionBodyHoldingAlias(node, qc, archive, request)
  /\ response =
       CertifiedResponseItem(AsyncUntrustedSource, archive, request)
  /\ DecisionCertifiedResponseLineageExact(node, qc, response)

ExactDecisionResponsePacketOwned(
    node, qc, archive, request, response, packet) ==
  /\ ExactDecisionAuthenticatedResponse(
       node, qc, archive, request, response)
  /\ packet \in asyncTransport
  /\ packet.item = response

ExactDecisionExecutableFrontier(node, qc) ==
  \/ NodeHasApplication(node)
  \/ \E candidate \in AsyncCandidateSet:
       ExactDecisionExecutableOwner(node, qc, candidate)

ExactDecisionResponseAdmissionGoal(node, qc) ==
  ExactDecisionExecutableFrontier(node, qc)

ExactDecisionResponseAdmissionResidual(
    node, qc, archive, request, response, packet) ==
  /\ ExactDecisionResponsePacketOwned(
       node, qc, archive, request, response, packet)
  /\ ~ExactDecisionResponseAdmissionGoal(node, qc)

ExactDecisionResponsePhysicalCompletionResidual(
    node, qc, archive, request, response, packet) ==
  /\ ExactDecisionResponseAdmissionResidual(
       node, qc, archive, request, response, packet)
  /\ packet = OldestDueSourcePacket(node, response.source)
  /\ CertifiedResponseFreshClaimGateAllows(response)
  /\ ~AsyncTransportCompletionOwnerGateAllows(response)

THEOREM ExactDecisionResponsePacketIsAuthorized ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)
    => CertifiedResponseAuthorized(response)
BY ExactOutstandingCertifiedBodyResponseIsAuthorized,
   ExactDecisionCertifiedRequestBindsHashAndArchiveRoute,
   ExactCertifiedResponseMatchesDecisionRequestHash, IsaT(180)
   DEF ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       FrozenCertifiedRequestRegistration,
       CertifiedResponseItem, AsyncCertifiedCitedResponder,
       AsyncCurrentResponsiveVoters, CurrentVoters,
       AsyncArchiveServerIds

THEOREM CoreBracketStepRetainsExactDecisionRecord ==
  \A node, qc:
    /\ ExactDecisionRecord(node, qc)
    /\ [Next]_vars
    => ExactDecisionRecord(node, qc)'
BY CoreNextLeavesContext,
   NextDurableReceiptActionClassification, Isa
   DEF ExactDecisionRecord, PersistDecision, ApplyDecision, vars

THEOREM AsyncBracketStepRetainsExactDecisionRecord ==
  \A node, qc:
    /\ ExactDecisionRecord(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    => ExactDecisionRecord(node, qc)'
BY AsyncStepRefinementObligation,
   CoreBracketStepRetainsExactDecisionRecord, Isa
   DEF AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars,
       AsyncProducerVars, vars

ExactDecisionRouteNeutralResponseClaimOwned(
    node, qc, archive, request, response) ==
  /\ ExactDecisionAuthenticatedResponse(
       node, qc, archive, request, response)
  /\ IngressHasCoalescingOwner(response)
  /\ CertifiedResponseClaimMatches(response)

ExactDecisionClaimedResponseIngressOwned(node, qc, response) ==
  /\ ExactDecisionServiceSource(node, qc)
  /\ DecisionCertifiedResponseLineageExact(node, qc, response)
  /\ CertifiedResponseClaimAuthorized(response)
  /\ response \in
       SequenceSet(
         IngressLane(node, IngressResourceSource(response)))

ExactDecisionResponseIngressOwned(
    node, qc, archive, request, response) ==
  /\ ExactDecisionRouteNeutralResponseClaimOwned(
       node, qc, archive, request, response)
  /\ ExactDecisionClaimedResponseIngressOwned(node, qc, response)

(***************************************************************************
Coalescing is intentionally route-neutral: the queued physical occurrence
may have a different outer relay source from the packet which observes it.
The claim and its mandatory ingress owner are therefore the stable handoff;
the exact queued occurrence is recovered from their shared canonical signed
wire identity when the normal runner drains it.
***************************************************************************)

ExactDecisionRouteNeutralClaimIngressOwned(node, qc, response) ==
  /\ ExactDecisionServiceSource(node, qc)
  /\ DecisionCertifiedResponseLineageExact(node, qc, response)
  /\ CertifiedResponseClaimMatches(response)
  /\ CertifiedResponseClaimIngressOwner(
       AsyncCertifiedResponseAuthProjection(response))

ExactDecisionCertifiedFetchOwner(node, qc, response) ==
  /\ DecisionCertifiedResponseLineageExact(node, qc, response)
  /\ ExactDecisionExecutableOwner(
       node, qc, CertifiedResponseCandidate(response))

THEOREM ExactDecisionResponseLineageTransfersAcrossRouteNeutralIdentity ==
  \A node, qc, response, admitted:
    /\ DecisionCertifiedResponseLineageExact(node, qc, response)
    /\ AsyncCertifiedResponseAuthProjection(admitted)
         = AsyncCertifiedResponseAuthProjection(response)
    => DecisionCertifiedResponseLineageExact(node, qc, admitted)
BY Isa
   DEF DecisionCertifiedResponseLineageExact,
       CertifiedResponseCapabilityAuthorized,
       CertifiedResponseAuthenticatedOccurrence,
       MatchingSentCertifiedRequests,
       FrozenCertifiedResponseBinding,
       AsyncCertifiedResponseAuthProjection,
       AsyncCertifiedResponseCanonicalWireIdentity

THEOREM ExactDecisionRouteNeutralClaimHasExactIngressOccurrence ==
  \A node, qc, response:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRouteNeutralClaimIngressOwned(node, qc, response)
    => \E admitted:
         /\ AsyncCertifiedResponseAuthProjection(admitted)
              = AsyncCertifiedResponseAuthProjection(response)
         /\ ExactDecisionClaimedResponseIngressOwned(
              node, qc, admitted)
BY ExactDecisionResponseLineageTransfersAcrossRouteNeutralIdentity,
   MatchingClaimedCertifiedResponseIsAuthorized, IsaT(180)
   DEF ExactDecisionRouteNeutralClaimIngressOwned,
       ExactDecisionClaimedResponseIngressOwned,
       CertifiedResponseClaimIngressOwner,
       CertifiedResponseClaimMatches,
       AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       IngressResourceSource, IngressLaneDepth, SequenceSet

THEOREM ExactDecisionServeJobProjectsProtectedOwner ==
  \A node, qc, archive, request, job:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionServeJobOwned(
         node, qc, archive, request, job)
    => /\ gst
       /\ archive \in Responsive
       /\ job \in AsyncServeJobSet
       /\ ResponsiveProtectedServeJobOwned(archive, job)
BY TypedCandidateIsInCarrier, IsaT(180)
   DEF ExactDecisionServeJobOwned,
       ExactDecisionServeOccurrenceOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ResponsiveProtectedServeJobOwned,
       AsyncServeJobSet, AsyncIoJob,
       AsyncArchiveIoServiceNodes,
       AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant,
       AsyncIoContentTypeInvariant,
       AsyncIoQueueContentTypeInvariant,
       AsyncIoSequenceTyped, AsyncIoJobTyped,
       SequenceSet

(***************************************************************************
Concrete action handoffs.  These are deliberately action-local.  They do not
turn weak fairness into admission while the selected packet/action is
disabled by an earlier lane owner.
***************************************************************************)

THEOREM ExactRequestPacketAdmissionCreatesIngressOwner ==
  \A node, qc, archive, request, packet:
    /\ ExactDecisionRequestPacketOwned(
         node, qc, archive, request, packet)
    /\ packet = OldestDueSourcePacket(archive, request.source)
    /\ AdmitIngressPacket(archive, request.source)
    => ExactDecisionRequestIngressOwned(
         node, qc, archive, request)'
BY Isa
   DEF ExactDecisionRequestPacketOwned,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       IngressPacketPolicyRejected,
       CertifiedResponsePacketPolicyRejected,
       UntrustedGenericCompletionPacketPolicyRejected,
       IngressResourceSource, IngressLane, SequenceSet

THEOREM NormalExactRequestIngressCreatesFreshServeOwner ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressOwned(
         node, qc, archive, request)
    /\ ~NodeHasApplication(archive)
    /\ SelectedIngressItemAt(
         archive, FirstDrainableIngressIndex(archive)) = request
    /\ DrainFairIngressSelected(archive)
    => \E job \in SequenceSet(asyncIoQueues'[archive]):
         ExactDecisionServeJobOwned(
           node, qc, archive, request, job)'
BY FreshAsyncIoServeNonceFacts,
   TypedRequestMakesTypedServeJob, IsaT(180)
   DEF ExactDecisionRequestIngressOwned,
       ExactDecisionServeJobOwned,
       ExactDecisionServeOccurrenceOwned,
       ExactDecisionBodyHoldingAlias,
       DrainFairIngressSelected, AsyncIoCertifiedServeJob,
       CertifiedRequestAuthorized, CertifiedRequestAuthority,
       FreshAsyncIoServeNonce, SequenceSet

THEOREM HistoricalExactRequestIngressCreatesFreshServeOwner ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressOwned(
         node, qc, archive, request)
    /\ NodeHasApplication(archive)
    /\ HistoricalSelectedIngressItemAt(
         archive,
         FirstHistoricalDrainableIngressIndex(archive)) = request
    /\ DrainHistoricalIngressSelected(archive)
    => \E job \in SequenceSet(asyncIoQueues'[archive]):
         ExactDecisionServeJobOwned(
           node, qc, archive, request, job)'
BY FreshAsyncIoServeNonceFacts,
   TypedRequestMakesTypedServeJob, IsaT(180)
   DEF ExactDecisionRequestIngressOwned,
       ExactDecisionServeJobOwned,
       ExactDecisionServeOccurrenceOwned,
       ExactDecisionBodyHoldingAlias,
       DrainHistoricalIngressSelected,
       AsyncIoCertifiedServeJob,
       CertifiedRequestAuthorized, CertifiedRequestAuthority,
       FreshAsyncIoServeNonce, SequenceSet

THEOREM ExactServeHeadCreatesAuthenticatedResponsePacket ==
  \A node, qc, archive, request, job:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionServeJobOwned(
         node, qc, archive, request, job)
    /\ Head(asyncIoQueues[archive]) = job
    /\ ServiceIoWorkerWork(archive)
    => \E response, packet:
         /\ ExactDecisionAuthenticatedResponse(
              node, qc, archive, request, response)'
         /\ ExactDecisionResponsePacketOwned(
              node, qc, archive, request, response, packet)'
BY SentCertifiedResponseAuthenticatesEveryRelayOccurrence,
   ExactCertifiedResponseMatchesDecisionRequestHash, IsaT(180)
   DEF ExactDecisionServeJobOwned,
       ExactDecisionServeOccurrenceOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionResponsePacketOwned,
       ExactDecisionBodyHoldingAlias,
       ServiceIoWorkerWork, CertifiedServeCanRespond,
       CertifiedResponseItem, PublishEphemeralItems,
       PacketsForItems, DecisionCertifiedResponseLineageExact

THEOREM ExactDecisionAdmittedServeProducesNonemptyTombstone ==
  \A node, qc, archive, request, job:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionServeJobOwned(
         node, qc, archive, request, job)
    /\ Head(asyncIoQueues[archive]) = job
    /\ ServiceIoWorkerWork(archive)
    => /\ AsyncServeLifecycleTombstone(
            archive,
            ExactDecisionServeLifecycleIdentity(
              archive, request))'
       /\ AsyncServeTombstoneOutputs(
            archive,
            ExactDecisionServeLifecycleIdentity(
              archive, request))' # {}
BY ExactServeHeadCreatesAuthenticatedResponsePacket, IsaT(180)
   DEF ExactDecisionServeJobOwned,
       ExactDecisionServeOccurrenceOwned,
       ExactDecisionServeAdmissionOwned,
       ExactDecisionServeLifecycleIdentity,
       ServiceIoWorkerWork,
       AsyncServeLifecycleTombstone,
       AsyncServeTombstoneOutputs,
       AsyncServeTombstoneRecords,
       AsyncServeReservationRecord,
       AsyncServeTombstonesWithoutFamily,
       AsyncServeTombstone

THEOREM FreshExactResponsePacketAdmissionAcquiresRecipientClaim ==
  \A node, qc, archive, request, response, packet:
    /\ ExactDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)
    /\ packet =
         OldestDueSourcePacket(node, response.source)
    /\ AdmitFreshHiddenPacket(node, response.source)
    => /\ ExactDecisionResponseIngressOwned(
            node, qc, archive, request, response)'
       /\ AsyncCertifiedResponseAuthProjection(response)
            \in asyncCertifiedResponseClaim'
       /\ CertifiedResponseClaimsAt(node)' =
            {AsyncCertifiedResponseAuthProjection(response)}
BY Isa
   DEF ExactDecisionResponsePacketOwned,
       ExactDecisionResponseIngressOwned,
       ExactDecisionRouteNeutralResponseClaimOwned,
       ExactDecisionClaimedResponseIngressOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       AdmitFreshHiddenPacket, AdmitHiddenPacket,
       IngressHasCoalescingOwner, IngressCoalescingIdentity,
       CertifiedResponseClaimMatches,
       IngressResourceSource, IngressLane, SequenceSet

THEOREM ExactResponsePacketCoalescingRetainsRouteNeutralClaim ==
  \A node, qc, archive, request, response, packet:
    /\ ExactDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)
    /\ packet =
         OldestDueSourcePacket(node, response.source)
    /\ CoalesceHiddenPacket(node, response.source)
    => /\ asyncCertifiedResponseClaim' =
            asyncCertifiedResponseClaim
       /\ ExactDecisionRouteNeutralResponseClaimOwned(
            node, qc, archive, request, response)'
BY Isa
   DEF ExactDecisionResponsePacketOwned,
       ExactDecisionRouteNeutralResponseClaimOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       CoalesceHiddenPacket, IngressHasCoalescingOwner,
       IngressCoalescingIdentity, CertifiedResponseClaimMatches

THEOREM ExactDecisionRouteNeutralClaimProjectsIngressOwner ==
  \A node, qc, archive, request, response:
    ExactDecisionRouteNeutralResponseClaimOwned(
      node, qc, archive, request, response)
      => ExactDecisionRouteNeutralClaimIngressOwned(
           node, qc, response)
BY Isa
   DEF ExactDecisionRouteNeutralResponseClaimOwned,
       ExactDecisionRouteNeutralClaimIngressOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       IngressHasCoalescingOwner, IngressCoalescingIdentity,
       CertifiedResponseClaimIngressOwner,
       IngressResourceSource, IngressLaneDepth, SequenceSet

THEOREM FreshExactResponseAdmissionCreatesRouteNeutralIngressOwner ==
  \A node, qc, archive, request, response, packet:
    /\ ExactDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)
    /\ packet =
         OldestDueSourcePacket(node, response.source)
    /\ AdmitFreshHiddenPacket(node, response.source)
    => ExactDecisionRouteNeutralClaimIngressOwned(
         node, qc, response)'
BY FreshExactResponsePacketAdmissionAcquiresRecipientClaim,
   ExactDecisionRouteNeutralClaimProjectsIngressOwner, Isa
   DEF ExactDecisionResponseIngressOwned

THEOREM CoalescedExactResponseCreatesRouteNeutralIngressOwner ==
  \A node, qc, archive, request, response, packet:
    /\ ExactDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)
    /\ packet =
         OldestDueSourcePacket(node, response.source)
    /\ CoalesceHiddenPacket(node, response.source)
    => ExactDecisionRouteNeutralClaimIngressOwned(
         node, qc, response)'
BY ExactResponsePacketCoalescingRetainsRouteNeutralClaim,
   ExactDecisionRouteNeutralClaimProjectsIngressOwner

THEOREM ExactResponseIngressDrainAtomicallyRetiresAliasesAndCreatesFetchOwner ==
  \A node, qc, response:
    /\ AsyncStrongTypeInvariant
    /\ DecisionsUniqueByNodeContext
    /\ ExactDecisionClaimedResponseIngressOwned(node, qc, response)
    /\ SelectedIngressItemAt(
         node, FirstDrainableIngressIndex(node)) = response
    /\ DrainFairIngressSelected(node)
    => /\ asyncActiveRequests' =
            asyncActiveRequests \ MatchingCertifiedRequests(response)
       /\ asyncCertifiedResponseClaim' =
            CertifiedResponseClaimForRequests(asyncActiveRequests')
       /\ \/ ExactDecisionCertifiedFetchOwner(node, qc, response)'
          \/ NodeHasApplication(node)'
BY ExactCertifiedResponseCandidateRetainsOuterItem, IsaT(180)
   DEF ExactDecisionClaimedResponseIngressOwned,
       ExactDecisionCertifiedFetchOwner,
       DrainFairIngressSelected,
       CertifiedResponseClaimAuthorized,
       CertifiedResponseClaimMatches,
       CertifiedResponseAuthorized,
       CertifiedResponseCandidate,
       DecisionExecutableStageOwner,
       ExactDecisionExecutableOwner,
       ExactDecisionServiceSource, ExactDecisionRecord,
       DecisionPipelineCandidate

THEOREM ExactDecisionClaimedResponseDrainCreatesExecutableFrontier ==
  \A node, qc, response:
    /\ AsyncStrongTypeInvariant
    /\ DecisionsUniqueByNodeContext
    /\ ExactDecisionClaimedResponseIngressOwned(node, qc, response)
    /\ SelectedIngressItemAt(
         node, FirstDrainableIngressIndex(node)) = response
    /\ DrainFairIngressSelected(node)
    => \/ NodeHasApplication(node)'
       \/ \E candidate \in AsyncCandidateSet':
            ExactDecisionExecutableOwner(node, qc, candidate)'
BY ExactResponseIngressDrainAtomicallyRetiresAliasesAndCreatesFetchOwner,
   Isa
   DEF ExactDecisionCertifiedFetchOwner

(***************************************************************************
Packet and runner residuals.

The source-specific admission action always selects
`OldestDueSourcePacket`.  A later exact response therefore receives no weak
fairness help while an earlier, non-timed packet on the same outer relay lane
is inadmissible.  The runner rank below gives Runtime an explicit value above
Local and Ingress; unlike `RuntimeReachRank`, the Runtime->Local reset is a
strict decrease rather than a zero-to-large increase.
***************************************************************************)

ExactDecisionWireItem(item) ==
  /\ item \in AsyncNetworkItems
  /\ item.kind \in {"CertifiedRequest", "CertifiedResponse"}
  /\ item.envelope.recipient \in AsyncTimedServiceNodes
  /\ \E node, qc:
       /\ ExactDecisionServiceSource(node, qc)
       /\ IF item.kind = "CertifiedRequest"
          THEN \E archive:
                 ExactDecisionBodyHoldingAlias(
                   node, qc, archive, item)
          ELSE DecisionCertifiedResponseLineageExact(node, qc, item)

ExactDecisionWireIngressOwned(item) ==
  item \in SequenceSet(
    IngressLane(
      item.envelope.recipient, IngressResourceSource(item)))

ExactDecisionWireNextOwner(item) ==
  IF item.kind = "CertifiedRequest"
  THEN \/ NodeHasApplication(item.envelope.requester)
       \/ \E node, qc, archive, request, job:
            /\ item = request
            /\ ExactDecisionServeJobOwned(
                 node, qc, archive, request, job)
  ELSE \/ NodeHasApplication(item.envelope.recipient)
       \/ \E node, qc:
            ExactDecisionCertifiedFetchOwner(node, qc, item)

ExactDecisionWireRunnerReachRank(item) ==
  IF ExactDecisionWireNextOwner(item)
  THEN 0
  ELSE IF ExactDecisionWireIngressOwned(item)
       THEN CASE
              asyncRunnerPhase[item.envelope.recipient] = "Ingress" -> 1
            [] asyncRunnerPhase[item.envelope.recipient] = "Local" -> 2
            [] OTHER -> 3
       ELSE IF ItemHasPacket(item) THEN 4 ELSE 5

ExactDecisionWirePacketExitReady(packet) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
      head == OldestDueSourcePacket(recipient, source)
  IN /\ packet = head
     /\ IngressPacketCanLeaveTransport(head.item)

ExactDecisionWirePacketHeadOfLineShadowed(packet) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
  IN /\ DueSourcePackets(recipient, source) # {}
     /\ packet # OldestDueSourcePacket(recipient, source)

ExactDecisionWirePacketGateBlocked(packet) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
      head == OldestDueSourcePacket(recipient, source)
  IN /\ packet = head
     /\ ~IngressPacketCanLeaveTransport(head.item)

ExactDecisionWireTransportResidual(packet) ==
  /\ packet \in OverdueResponsivePackets
  /\ ExactDecisionWireItem(packet.item)
  /\ \/ ExactDecisionWirePacketHeadOfLineShadowed(packet)
     \/ ExactDecisionWirePacketGateBlocked(packet)

ExactDecisionWireRunnerResidual(item) ==
  /\ ExactDecisionWireItem(item)
  /\ ExactDecisionWireIngressOwned(item)
  /\ ~ExactDecisionWireNextOwner(item)
  /\ \/ asyncRunnerPhase[item.envelope.recipient] = "Runtime"
     \/ /\ asyncRunnerPhase[item.envelope.recipient] = "Local"
           /\ ~LocalAdmissionCanAdvance(item.envelope.recipient)
     \/ /\ asyncRunnerPhase[item.envelope.recipient] = "Ingress"
           /\ ~IngressItemCanDrain(
                item.envelope.recipient, item)

THEOREM DueExactDecisionWirePacketHasExitOrResidual ==
  \A packet \in OverdueResponsivePackets:
    ExactDecisionWireItem(packet.item)
      => \/ ExactDecisionWirePacketExitReady(packet)
         \/ ExactDecisionWireTransportResidual(packet)
BY Isa
   DEF ExactDecisionWirePacketExitReady,
       ExactDecisionWireTransportResidual,
       ExactDecisionWirePacketHeadOfLineShadowed,
       ExactDecisionWirePacketGateBlocked,
       IngressPacketCanLeaveTransport

THEOREM ExitReadyExactDecisionWireHeadEnablesRemoval ==
  \A packet \in OverdueResponsivePackets:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ ExactDecisionWireItem(packet.item)
    /\ ExactDecisionWirePacketExitReady(packet)
    => ENABLED (
         PostGstAdmitHiddenPacket(
           packet.item.envelope.recipient, packet.item.source)
           /\ packet \notin asyncTransport')
BY AsyncStrongTypeProjectsAsyncType,
   GstResponsiveNodesAreUp,
   GstExcludesResponsiveReplayQuarantine,
   OldestDueSourcePacketFacts,
   ExpandENABLED, Isa
   DEF ExactDecisionWireItem,
       ExactDecisionWirePacketExitReady,
       IngressPacketCanLeaveTransport,
       IngressCoalescingGateAllows,
       PostGstAdmitHiddenPacket, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       DueSourcePackets, AsyncNonRunnerOuterFrame,
       AsyncNonCrashOuterFrame, AsyncCoreOuterFrame,
       AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars,
       AsyncProducerVars,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
       LeaveCausalQueues, vars

THEOREM RuntimeResetLowersExactDecisionWireReachRank ==
  \A item \in AsyncNetworkItems,
     node \in AsyncCurrentResponsiveVoters:
    /\ ExactDecisionWireItem(item)
    /\ item.envelope.recipient = node
    /\ ExactDecisionWireIngressOwned(item)
    /\ ~ExactDecisionWireNextOwner(item)
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ SerializedRuntimeStep(node)
    => ExactDecisionWireRunnerReachRank(item)'
         < ExactDecisionWireRunnerReachRank(item)
BY Isa
   DEF ExactDecisionWireRunnerReachRank,
       ExactDecisionWireIngressOwned,
       ExactDecisionWireNextOwner,
       SerializedRuntimeStep, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       IdleRuntimeStep, LeaveCausalQueues

ExactResponseNonTimedBlockedHead(packet) ==
  LET response == packet.item
      recipient == response.envelope.recipient
      source == response.source
      head == OldestDueSourcePacket(recipient, source)
  IN /\ packet \in OverdueResponsivePackets
     /\ response.kind = "CertifiedResponse"
     /\ DecisionCertifiedResponseLineageExact(
          recipient,
          CHOOSE qc:
            ExactDecisionRecord(recipient, qc)
              /\ response.envelope.requestHash =
                   AsyncCertifiedRequestHashOf(recipient, qc, 0),
          response)
     /\ DueSourcePackets(recipient, source) # {}
     /\ head # packet
     /\ head \notin OverdueResponsivePackets
     /\ ~IngressPacketCanLeaveTransport(head.item)

THEOREM NonTimedBlockedHeadDisablesExactResponseLaneAdmission ==
  \A packet:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ ExactResponseNonTimedBlockedHead(packet)
    => ~ENABLED PostGstAdmitHiddenPacket(
         packet.item.envelope.recipient, packet.item.source)
BY ExpandENABLED, Isa
   DEF ExactResponseNonTimedBlockedHead,
       IngressPacketCanLeaveTransport,
       IngressCoalescingGateAllows,
       PostGstAdmitHiddenPacket, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       DueSourcePackets, AsyncNonRunnerOuterFrame,
       AsyncNonCrashOuterFrame, AsyncCoreOuterFrame,
       AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars,
       AsyncProducerVars,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
       LeaveCausalQueues, vars

(***************************************************************************
Exact response claim and normalized physical-completion debt.

The model now contains the repair contract directly.  A certified response is
charged to `IngressResourceSource(item)`, acquires the recipient-local
singleton authenticated claim only on fresh admission, and coalesces by its
route-neutral authenticated-envelope projection.  Claims at other recipients
are independent.  That logical claim is not a second physical queue slot:
both Chunk and CertifiedResponse continue to satisfy
`IngressUsesPhysicalCompletionOwner`.

A pre-existing physical completion can therefore delay a fresh exact response,
but it is a finite lane owner rather than an archive-route mismatch.  A
distinct live claim at this recipient is retryable finite backpressure: the
target packet stays in transport until that local claim drains.  An exact
matching claim is handled by the coalescing action above.
***************************************************************************)

ExactResponseRegisteredAndAuthenticated(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ CertifiedResponseAuthenticatedOccurrence(item)
  /\ item.envelope.archiveServer \in AsyncArchiveServerIds
  /\ MatchingCertifiedRequests(item) # {}

ExactDecisionResponseRegisteredAndAuthenticated(item) ==
  /\ ExactResponseRegisteredAndAuthenticated(item)
  /\ \E node, qc:
       /\ ExactDecisionServiceSource(node, qc)
       /\ DecisionCertifiedResponseLineageExact(node, qc, item)

THEOREM ExactDecisionResponseUsesNormalizedPhysicalOwner ==
  \A item:
    ExactDecisionResponseRegisteredAndAuthenticated(item)
      => /\ IngressResourceSource(item) = AsyncUntrustedSource
         /\ IngressAdmissionClass(item) = "CertifiedResponse"
         /\ IngressUsesPhysicalCompletionOwner(item)
BY DEF ExactDecisionResponseRegisteredAndAuthenticated,
       ExactResponseRegisteredAndAuthenticated,
       IngressResourceSource, IngressAdmissionClass,
       IngressUsesPhysicalCompletionOwner

ExactDecisionFreshResponsePhysicalCompletionResidual(item) ==
  /\ ExactDecisionResponseRegisteredAndAuthenticated(item)
  /\ CertifiedResponseFreshClaimGateAllows(item)
  /\ ~AsyncTransportCompletionOwnerGateAllows(item)

THEOREM ExactDecisionFreshResponsePhysicalCompletionDebtIsFinite ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ AsyncItemTyped(item)
    /\ ExactDecisionFreshResponsePhysicalCompletionResidual(item)
    => /\ IngressResourceSource(item) = AsyncUntrustedSource
       /\ IngressUsesPhysicalCompletionOwner(item)
       /\ TransportCompletionOwnerDebt(item) \in Nat \ {0}
       /\ TransportCompletionOwnerDebt(item)
            <= AsyncIngressCapacity
BY ExactDecisionResponseUsesNormalizedPhysicalOwner,
   IngressGateOwnerDebtsAreFiniteNaturals,
   TransportCompletionGateHasExactFiniteOwner, Isa
   DEF ExactDecisionFreshResponsePhysicalCompletionResidual

THEOREM ExactDecisionResponsePhysicalCompletionDebtIsFinite ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponsePhysicalCompletionResidual(
         node, qc, archive, request, response, packet)
    => /\ IngressResourceSource(response) = AsyncUntrustedSource
       /\ IngressUsesPhysicalCompletionOwner(response)
       /\ TransportCompletionOwnerDebt(response) \in Nat \ {0}
       /\ TransportCompletionOwnerDebt(response)
            <= AsyncIngressCapacity
BY ExactDecisionFreshResponsePhysicalCompletionDebtIsFinite,
   ExactDecisionResponsePacketIsAuthorized, Isa
   DEF ExactDecisionResponsePhysicalCompletionResidual,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponsePacketOwned,
       ExactDecisionResponseRegisteredAndAuthenticated,
       ExactResponseRegisteredAndAuthenticated,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncPacketContentTypeInvariant,
       AsyncPacketTyped,
       ExactDecisionFreshResponsePhysicalCompletionResidual

(***************************************************************************
The response's fresh-claim gate proves that this recipient has no existing
response claim.  Every shared physical owner is therefore drainable: Chunk
uses the direct transport-local ingress path, while CertifiedResponse either
has no local claim and is discarded as stale or has the impossible empty-set
claim witness.  Thus every old owner falls into the recipient-fenced priority
set.  The blocked predicate remains named only as a diagnostic partition and
is proved unreachable below; no liveness conclusion relies on finiteness
alone.
***************************************************************************)

THEOREM ChunkIngressIsTransportLocalDrainable ==
  \A node, item:
    item.kind = "Chunk"
      => IngressItemCanDrain(node, item)
BY DEF IngressItemCanDrain

THEOREM EmptyRecipientClaimPhysicalCompletionLaneIsDrainable ==
  \A node \in ValidatorIds:
    \A source \in AsyncIngressSources:
      \A index \in 1..IngressLaneDepth(node, source):
        /\ AsyncStrongTypeInvariant
        /\ CertifiedResponseClaimsAt(node) = {}
        /\ IngressUsesPhysicalCompletionOwner(
             IngressLane(node, source)[index])
        => IngressItemCanDrain(
             node, IngressLane(node, source)[index])
BY ChunkIngressIsTransportLocalDrainable, IsaT(180)
   DEF AsyncStrongTypeInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant, AsyncIngressTypeInvariant,
       AsyncIngressContentTypeInvariant,
       IngressLaneDepth, IngressUsesPhysicalCompletionOwner,
       IngressAdmissionClass, IngressTransportCompletionKinds,
       IngressItemCanDrain,
       CertifiedResponseClaimAuthorized,
       CertifiedResponseClaimMatches, CertifiedResponseClaimsAt,
       AsyncCertifiedResponseCanonicalWireIdentity

ExactDecisionDrainablePhysicalCompletionResidual(
    node, qc, archive, request, response, packet) ==
  /\ ExactDecisionResponsePhysicalCompletionResidual(
       node, qc, archive, request, response, packet)
  /\ DrainableRequestFencedCompletionLaneIndices(
       node, AsyncUntrustedSource) # {}

ExactDecisionBlockedPhysicalCompletionRunnerResidual(
    node, qc, archive, request, response, packet) ==
  /\ ExactDecisionResponsePhysicalCompletionResidual(
       node, qc, archive, request, response, packet)
  /\ DrainableRequestFencedCompletionLaneIndices(
       node, AsyncUntrustedSource) = {}

THEOREM ExactDecisionPhysicalCompletionResidualSplitsAtDrainability ==
  \A node, qc, archive, request, response, packet:
    ExactDecisionResponsePhysicalCompletionResidual(
      node, qc, archive, request, response, packet)
      => \/ ExactDecisionDrainablePhysicalCompletionResidual(
              node, qc, archive, request, response, packet)
         \/ ExactDecisionBlockedPhysicalCompletionRunnerResidual(
              node, qc, archive, request, response, packet)
BY Isa
   DEF ExactDecisionDrainablePhysicalCompletionResidual,
       ExactDecisionBlockedPhysicalCompletionRunnerResidual

THEOREM ExactDecisionFreshResponseHasLocalFenceAndNoClaimPriority ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponsePhysicalCompletionResidual(
         node, qc, archive, request, response, packet)
    => /\ ActiveCertifiedRequestHashesAt(node) # {}
       /\ CertifiedResponseClaimsAt(node) = {}
       /\ DrainableClaimedResponseReadyIndices(node) = {}
BY EmptyRecipientClaimHasNoClaimedResponsePriority, IsaT(180)
   DEF ExactDecisionResponsePhysicalCompletionResidual,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       CertifiedResponseFreshClaimGateAllows,
       CertifiedResponseRecipientClaimAvailable,
       CertifiedResponseClaimsAt,
       ActiveCertifiedRequestHashesAt,
       AsyncCertifiedRequestHash,
       CertifiedResponseItem,
       AsyncCertifiedResponseEnvelope

THEOREM ExactDecisionPhysicalCompletionResidualIsDrainable ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponsePhysicalCompletionResidual(
         node, qc, archive, request, response, packet)
    => ExactDecisionDrainablePhysicalCompletionResidual(
         node, qc, archive, request, response, packet)
PROOF
  <1>1. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                NEW response, NEW packet,
                AsyncStrongTypeInvariant,
                ExactDecisionResponsePhysicalCompletionResidual(
                  node, qc, archive, request, response, packet)
         PROVE ExactDecisionDrainablePhysicalCompletionResidual(
                  node, qc, archive, request, response, packet)
    <2>1. /\ node \in ValidatorIds
           /\ response.envelope.recipient = node
           /\ ActiveCertifiedRequestHashesAt(node) # {}
           /\ CertifiedResponseClaimsAt(node) = {}
      BY <1>1,
         ExactDecisionFreshResponseHasLocalFenceAndNoClaimPriority,
         IsaT(180)
         DEF ExactDecisionResponsePhysicalCompletionResidual,
             ExactDecisionResponseAdmissionResidual,
             ExactDecisionResponsePacketOwned,
             ExactDecisionAuthenticatedResponse,
             ExactDecisionBodyHoldingAlias,
             ExactDecisionActiveRequestOwner,
             ExactDecisionServiceSource,
             CertifiedResponseItem,
             AsyncCertifiedResponseEnvelope,
             AsyncCurrentResponsiveVoters, CurrentVoters
    <2>2. /\ IngressResourceSource(response) =
                  AsyncUntrustedSource
           /\ TransportCompletionOwnerIndices(response) # {}
      BY <1>1,
         ExactDecisionResponsePhysicalCompletionDebtIsFinite,
         FS_CardinalityType, IsaT(180)
         DEF TransportCompletionOwnerDebt
    <2>3. PICK index \in TransportCompletionOwnerIndices(response):
             TRUE
      BY <2>2
    <2>4. /\ index \in
                  1..IngressLaneDepth(node, AsyncUntrustedSource)
           /\ IngressUsesPhysicalCompletionOwner(
                IngressLane(node, AsyncUntrustedSource)[index])
      BY <2>1, <2>2, <2>3, Isa
         DEF TransportCompletionOwnerIndices,
             IngressLaneDepth
    <2>5. IngressItemCanDrain(
             node, IngressLane(node, AsyncUntrustedSource)[index])
      BY <1>1, <2>1, <2>4,
         EmptyRecipientClaimPhysicalCompletionLaneIsDrainable
         DEF AsyncIngressSources, AsyncArchiveServerIds
    <2>6. index \in
             DrainableRequestFencedCompletionLaneIndices(
               node, AsyncUntrustedSource)
      BY <2>1, <2>4, <2>5, Isa
         DEF DrainableRequestFencedCompletionLaneIndices,
             DrainableIngressLaneIndices
    <2> QED BY <1>1, <2>6, Isa
         DEF ExactDecisionDrainablePhysicalCompletionResidual
  <1> QED BY <1>1

THEOREM ExactDecisionBlockedPhysicalCompletionRunnerResidualIsImpossible ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionBlockedPhysicalCompletionRunnerResidual(
         node, qc, archive, request, response, packet)
    => FALSE
BY ExactDecisionPhysicalCompletionResidualIsDrainable, Isa
   DEF ExactDecisionDrainablePhysicalCompletionResidual,
       ExactDecisionBlockedPhysicalCompletionRunnerResidual

THEOREM ExactDecisionDrainablePhysicalCompletionCreatesPrioritySource ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionDrainablePhysicalCompletionResidual(
         node, qc, archive, request, response, packet)
    => /\ DrainableClaimedResponseReadyIndices(node) = {}
       /\ DrainableRequestFencedCompletionReadyIndices(node) # {}
BY ExactDecisionFreshResponseHasLocalFenceAndNoClaimPriority,
   DrainableRequestFencedCompletionLaneCreatesPrioritySource, Isa
   DEF ExactDecisionDrainablePhysicalCompletionResidual

THEOREM ExactDecisionFencedCompletionDrainLowersPhysicalDebt ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionDrainablePhysicalCompletionResidual(
         node, qc, archive, request, response, packet)
    /\ DrainFairIngressSelected(node)
    => TransportCompletionOwnerDebt(response)' + 1 =
         TransportCompletionOwnerDebt(response)
BY ExactDecisionDrainablePhysicalCompletionCreatesPrioritySource,
   PrioritySourceSelectsRequestFencedCompletion,
   FirstDrainableIngressIndexIsDrainable,
   FirstDrainableIngressLaneIndexIsDrainable,
   IngressPhysicalCompletionCountDropsAfterOwnerRemoval, IsaT(300)
   DEF ExactDecisionDrainablePhysicalCompletionResidual,
       ExactDecisionResponsePhysicalCompletionResidual,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       DrainFairIngressSelected, PopSelectedIngress,
       SelectedIngressItemAt, SelectedIngressLaneIndex,
       FirstDrainableIngressIndex,
       FirstDrainableIngressLaneIndex,
       TransportCompletionOwnerDebt,
       TransportCompletionOwnerIndices,
       IngressPhysicalCompletionPositions,
       IngressResourceSource, IngressLane, IngressLaneDepth,
       SequenceSet

ExactDecisionDrainablePhysicalCompletionIngressReady(
    node, qc, archive, request, response, packet) ==
  /\ ExactDecisionDrainablePhysicalCompletionResidual(
       node, qc, archive, request, response, packet)
  /\ asyncRunnerPhase[node] = "Ingress"
  /\ asyncRunnerBudget[node] > 0

THEOREM ExactDecisionIngressTurnDrainsFencedPhysicalOwner ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionDrainablePhysicalCompletionIngressReady(
         node, qc, archive, request, response, packet)
    /\ PostGstRunNode(node)
    => TransportCompletionOwnerDebt(response)' + 1 =
         TransportCompletionOwnerDebt(response)
PROOF
  <1>1. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                NEW response, NEW packet,
                AsyncStrongTypeInvariant,
                ExactDecisionDrainablePhysicalCompletionIngressReady(
                  node, qc, archive, request, response, packet),
                PostGstRunNode(node)
         PROVE TransportCompletionOwnerDebt(response)' + 1 =
                 TransportCompletionOwnerDebt(response)
    <2>1. /\ DrainableClaimedResponseReadyIndices(node) = {}
           /\ DrainableRequestFencedCompletionReadyIndices(node) # {}
           /\ DrainableIngressIndices(node) # {}
      BY <1>1,
         ExactDecisionDrainablePhysicalCompletionCreatesPrioritySource
         DEF ExactDecisionDrainablePhysicalCompletionIngressReady,
             DrainableRequestFencedCompletionReadyIndices
    <2>2. /\ IngressDrainStep(node)
           /\ DrainFairIngressSelected(node)
      BY <1>1, <2>1, Isa
         DEF ExactDecisionDrainablePhysicalCompletionIngressReady,
             PostGstRunNode, RunNode, RunNodeWork,
             IngressDrainStep
    <2> QED BY <1>1, <2>2,
         ExactDecisionFencedCompletionDrainLowersPhysicalDebt
         DEF ExactDecisionDrainablePhysicalCompletionIngressReady
  <1> QED BY <1>1

(***************************************************************************
Narrow temporal kernels and exact reduction.

The four off-scheduler ownership corridors follow the concrete ownership
boundaries rather than assuming the whole certified-request corridor.  The
request-emission corridor is split into pre-deadline clock-owner and armed
Runtime prefixes; request ingress is split at exact admission/coalescing; and
the generic nonce-owned Serve FIFO exit is proved from its existing rank,
leaving only exact alias/occurrence exit safety:

  * active registration to one body-holding alias packet;
  * that request packet/ingress occurrence to one fresh Serve job;
  * that nonce-owned Serve job to one authenticated response packet; and
  * that response packet through fresh recipient-local claim acquisition or
    route-neutral coalescing, finite physical-completion debt, and exact
    FetchCertifiedBody admission.

The candidate pipeline is already reduced to the Decision-local Stage-2 Busy
kernel above.
***************************************************************************)

ExactDecisionRequestPacketEmissionGoal(node, qc) ==
  \/ ExactDecisionExecutableFrontier(node, qc)
  \/ \E archive, request, packet:
       ExactDecisionRequestPacketOwned(
         node, qc, archive, request, packet)

ExactDecisionRequestIngressGoal(node, qc, archive, request) ==
  \/ ExactDecisionExecutableFrontier(node, qc)
  \/ \E job:
       ExactDecisionServeJobOwned(
         node, qc, archive, request, job)
  \/ \E response, packet:
       ExactDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)

ExactDecisionServeResponseGoal(node, qc, archive, request) ==
  \/ ExactDecisionExecutableFrontier(node, qc)
  \/ \E response, packet:
       ExactDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)

ExactDecisionRequestPacketEmissionKernelProperty(specification) ==
  specification
    => \A node, qc:
         ExactDecisionActiveRequestOwner(node, qc)
           ~> ExactDecisionRequestPacketEmissionGoal(node, qc)

ExactDecisionRequestIngressKernelProperty(specification) ==
  specification
    => \A node, qc, archive, request, packet:
         ExactDecisionRequestPacketOwned(
           node, qc, archive, request, packet)
           ~> ExactDecisionRequestIngressGoal(
                node, qc, archive, request)

ExactDecisionServeResponseKernelProperty(specification) ==
  specification
    => \A node, qc, archive, request, job:
         ExactDecisionServeJobOwned(
           node, qc, archive, request, job)
           ~> ExactDecisionServeResponseGoal(
                node, qc, archive, request)

ExactDecisionResponseAdmissionKernelProperty(specification) ==
  specification
    => \A node, qc, archive, request, response, packet:
         ExactDecisionResponsePacketOwned(
           node, qc, archive, request, response, packet)
           ~> ExactDecisionResponseAdmissionGoal(node, qc)

(***************************************************************************
Strict off-scheduler residuals.

Each predicate is the exact owner with its semantic handoff absent.  This is
strictly narrower than restating the four broad kernels: it records only the
states in which the current corridor has not already succeeded.  The request
emission residual is split below at the exact retransmission-authority
boundary, and the request ingress residual is split at the exact packet
admission/coalescing boundary.  The Serve FIFO liveness leaf is closed below
and leaves only exact exit safety; response admission remains one temporal
kernel.  The combined property is deliberately an operator, not an asserted
theorem.  A release-facing module can expose precisely this remaining
temporal debt and derive the broad Decision service theorem through the
checked reduction below.
***************************************************************************)

ExactDecisionRequestPacketEmissionResidual(node, qc) ==
  /\ ExactDecisionActiveRequestOwner(node, qc)
  /\ ~ExactDecisionRequestPacketEmissionGoal(node, qc)

ExactDecisionRequestIngressResidual(
    node, qc, archive, request, packet) ==
  /\ ExactDecisionRequestPacketOwned(
       node, qc, archive, request, packet)
  /\ ~ExactDecisionRequestIngressGoal(
       node, qc, archive, request)

ExactDecisionRequestIngressLaneResidual(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressOwned(
       node, qc, archive, request)
  /\ ~ExactDecisionRequestIngressGoal(
       node, qc, archive, request)

ExactDecisionRequestPacketAdmissionReady(
    node, qc, archive, request, packet) ==
  /\ ExactDecisionRequestIngressResidual(
       node, qc, archive, request, packet)
  /\ packet = OldestDueSourcePacket(
       archive, request.source)
  /\ ENABLED (
       PostGstAdmitHiddenPacket(archive, request.source)
         /\ packet \notin asyncTransport')

ExactDecisionRequestHeadGateOwnerResidual(
    node, qc, archive, request, packet) ==
  /\ ExactDecisionRequestIngressResidual(
       node, qc, archive, request, packet)
  /\ ~ExactDecisionRequestPacketAdmissionReady(
       node, qc, archive, request, packet)

ExactDecisionRequestAdmissionOutcome(
    node, qc, archive, request) ==
  \/ ExactDecisionRequestIngressGoal(
       node, qc, archive, request)
  \/ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)

THEOREM ExactDecisionRequestIngressResidualSplitsAtAdmissionReady ==
  \A node, qc, archive, request, packet:
    ExactDecisionRequestIngressResidual(
      node, qc, archive, request, packet)
      => \/ ExactDecisionRequestPacketAdmissionReady(
              node, qc, archive, request, packet)
         \/ ExactDecisionRequestHeadGateOwnerResidual(
              node, qc, archive, request, packet)
BY Isa DEF ExactDecisionRequestHeadGateOwnerResidual

ExactDecisionServeResponseResidual(
    node, qc, archive, request, job) ==
  /\ ExactDecisionServeJobOwned(
       node, qc, archive, request, job)
  /\ ~ExactDecisionServeResponseGoal(
       node, qc, archive, request)

ExactDecisionResponsePacketAdmissionReady(
    node, qc, archive, request, response, packet) ==
  /\ ExactDecisionResponseAdmissionResidual(
       node, qc, archive, request, response, packet)
  /\ packet = OldestDueSourcePacket(node, response.source)
  /\ ENABLED (
       PostGstAdmitHiddenPacket(node, response.source)
         /\ packet \notin asyncTransport')

ExactDecisionResponseHeadGateOwnerResidual(
    node, qc, archive, request, response, packet) ==
  /\ ExactDecisionResponseAdmissionResidual(
       node, qc, archive, request, response, packet)
  /\ ~ExactDecisionResponsePacketAdmissionReady(
       node, qc, archive, request, response, packet)

ExactDecisionResponseClaimIngressResidual(node, qc, response) ==
  /\ ExactDecisionRouteNeutralClaimIngressOwned(node, qc, response)
  /\ ~ExactDecisionResponseAdmissionGoal(node, qc)

ExactDecisionResponseClaimContentionResidual(
    node, qc, archive, request, response, packet) ==
  /\ ExactDecisionResponseHeadGateOwnerResidual(
       node, qc, archive, request, response, packet)
  /\ packet = OldestDueSourcePacket(node, response.source)
  /\ CertifiedResponseAuthorized(response)
  /\ ~CertifiedResponseRecipientClaimAvailable(response)
  /\ ~CertifiedResponseClaimMatches(response)

THEOREM ExactDecisionClaimContentionOwnsExactClaimResidual ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestAuthorityIsolationInvariant
    /\ ExactDecisionResponseClaimContentionResidual(
         node, qc, archive, request, response, packet)
    => \E claimed \in AsyncCertifiedResponseItems:
         ExactDecisionResponseClaimIngressResidual(
           node, qc, claimed)
BY MatchingClaimedCertifiedResponseIsAuthorized,
   ExactDecisionCertifiedRequestBindsHashAndArchiveRoute, IsaT(300)
   DEF ExactDecisionResponseClaimContentionResidual,
       ExactDecisionResponseHeadGateOwnerResidual,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionRequestAuthorityIsolationInvariant,
       ExactDecisionResponseClaimIngressResidual,
       ExactDecisionRouteNeutralClaimIngressOwned,
       DecisionCertifiedResponseLineageExact,
       CertifiedResponseClaimIngressOwner,
       CertifiedResponseClaimAuthorized,
       CertifiedResponseClaimMatches,
       CertifiedResponseRecipientClaimAvailable,
       CertifiedResponseClaimsAt,
       CertifiedResponseClaimProjectionAuthenticated,
       CertifiedResponseCapabilityAuthorized,
       CertifiedResponseAuthenticatedOccurrence,
       MatchingCertifiedRequests,
       MatchingSentCertifiedRequests,
       FrozenCertifiedResponseBinding,
       FrozenCertifiedRequestRegistration,
       AsyncCertifiedRequestHash,
       AsyncCertifiedRequestHashOf,
       AsyncCertifiedSignedRequest,
       AsyncCertifiedRequestSignature,
       AsyncCertifiedRequestPreimage,
       AsyncCertifiedResponseItems,
       AsyncCertifiedResponseClaimValues,
       AsyncCertifiedResponseCanonicalWireIdentity,
       AsyncCertifiedResponseAuthProjection,
       AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       AsyncCertifiedResponseClaimInvariant,
       AsyncCertifiedResponseClaimIngressOwnershipInvariant,
       IngressResourceSource,
       IngressLane, IngressLaneDepth,
       SequenceSet

ExactDecisionResponseAdmissionOutcome(node, qc, response) ==
  \/ ExactDecisionResponseAdmissionGoal(node, qc)
  \/ ExactDecisionResponseClaimIngressResidual(node, qc, response)
  \/ \E claimed \in AsyncCertifiedResponseItems:
       ExactDecisionResponseClaimIngressResidual(node, qc, claimed)

ExactDecisionResponseNonPhysicalHeadGateOwnerResidual(
    node, qc, archive, request, response, packet) ==
  /\ ExactDecisionResponseHeadGateOwnerResidual(
       node, qc, archive, request, response, packet)
  /\ ~ExactDecisionResponsePhysicalCompletionResidual(
       node, qc, archive, request, response, packet)

ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual(
    node, qc, archive, request, response, packet) ==
  /\ ExactDecisionResponseNonPhysicalHeadGateOwnerResidual(
       node, qc, archive, request, response, packet)
  /\ ~ExactDecisionResponseClaimContentionResidual(
       node, qc, archive, request, response, packet)

THEOREM ExactDecisionResponseAdmissionResidualSplitsAtReady ==
  \A node, qc, archive, request, response, packet:
    ExactDecisionResponseAdmissionResidual(
      node, qc, archive, request, response, packet)
      => \/ ExactDecisionResponsePacketAdmissionReady(
              node, qc, archive, request, response, packet)
         \/ ExactDecisionResponseHeadGateOwnerResidual(
              node, qc, archive, request, response, packet)
BY Isa DEF ExactDecisionResponseHeadGateOwnerResidual

THEOREM ExactDecisionResponsePacketIsNeverPolicyRejected ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)
    => ~IngressPacketPolicyRejected(response)
BY ExactDecisionResponsePacketIsAuthorized, IsaT(180)
   DEF IngressPacketPolicyRejected,
       CertifiedResponsePacketPolicyRejected,
       UntrustedGenericCompletionPacketPolicyRejected,
       IngressAdmissionClass,
       AsyncStrongTypeInvariant,
       IngressResourceSource

(***************************************************************************
The protected ingress geometry eliminates aggregate capacity as an exact
response blocker.  At the selected due head, a matching recipient-local
claim supplies the coalescing owner.  Otherwise exclusion of exact claim
contention supplies an empty local claim; the missing physical owner supplies
the dedicated reserved slot proved in the height-reset leaf.  Timeout-byte
and generic-untrusted gates do not apply to the distinct CertifiedResponse
class.  Consequently a non-physical, non-claim due head can always leave
transport.
***************************************************************************)

THEOREM ExactDecisionResponseNonPhysicalNonClaimDueHeadCanLeaveTransport ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual(
         node, qc, archive, request, response, packet)
    /\ packet = OldestDueSourcePacket(node, response.source)
    => IngressPacketCanLeaveTransport(response)
PROOF
  <1>1. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                NEW response, NEW packet,
                AsyncStrongTypeInvariant,
                ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual(
                  node, qc, archive, request, response, packet),
                packet = OldestDueSourcePacket(node, response.source)
         PROVE IngressPacketCanLeaveTransport(response)
    <2>1. /\ AsyncItemTyped(response)
           /\ response.kind = "CertifiedResponse"
           /\ response.envelope.recipient = node
           /\ CertifiedResponseAuthorized(response)
           /\ ~IngressPacketPolicyRejected(response)
      BY <1>1,
         ExactDecisionResponsePacketIsAuthorized,
         ExactDecisionResponsePacketIsNeverPolicyRejected,
         IsaT(180)
         DEF ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual,
             ExactDecisionResponseNonPhysicalHeadGateOwnerResidual,
             ExactDecisionResponseHeadGateOwnerResidual,
             ExactDecisionResponseAdmissionResidual,
             ExactDecisionResponsePacketOwned,
             ExactDecisionAuthenticatedResponse,
             ExactDecisionBodyHoldingAlias,
             CertifiedResponseItem,
             AsyncCertifiedResponseEnvelope,
             AsyncStrongTypeInvariant,
             AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant,
             AsyncPacketContentTypeInvariant,
             AsyncPacketTyped
    <2>2. CASE CertifiedResponseClaimMatches(response)
      <3>1. IngressHasCoalescingOwner(response)
        BY <1>1, <2>1, <2>2, IsaT(180)
           DEF AsyncStrongTypeInvariant,
               AsyncCertifiedResponseClaimIngressOwnershipInvariant,
               CertifiedResponseClaimIngressOwner,
               CertifiedResponseClaimMatches,
               CertifiedResponseClaimsAt,
               IngressHasCoalescingOwner,
               IngressCoalescingIdentity,
               IngressResourceSource,
               IngressLaneDepth, SequenceSet
      <3>2. IngressCoalescingGateAllows(response)
        BY <2>1, <2>2, <3>1
           DEF IngressCoalescingGateAllows
      <3> QED BY <3>2 DEF IngressPacketCanLeaveTransport
    <2>3. CASE ~CertifiedResponseClaimMatches(response)
      <3>1. CertifiedResponseRecipientClaimAvailable(response)
        BY <1>1, <2>1, <2>3, Isa
           DEF ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual,
               ExactDecisionResponseClaimContentionResidual
      <3>2. /\ CertifiedResponseAuthorityReady(
                      response.envelope.requestHash)
             /\ CertifiedResponseFreshClaimGateAllows(response)
        BY <1>1, <2>1, <3>1, IsaT(180)
           DEF ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual,
               ExactDecisionResponseNonPhysicalHeadGateOwnerResidual,
               ExactDecisionResponseHeadGateOwnerResidual,
               ExactDecisionResponseAdmissionResidual,
               ExactDecisionResponsePacketOwned,
               ExactDecisionAuthenticatedResponse,
               ExactDecisionBodyHoldingAlias,
               ExactDecisionActiveRequestOwner,
               DecisionCertifiedRequestActiveExact,
               CertifiedResponseFreshClaimGateAllows,
               CertifiedResponseAuthorityReady,
               CertifiedResponseAuthorityClaimed,
               CertifiedResponseRecipientClaimAvailable,
               ActiveCertifiedRequestHashes,
               ActiveCertifiedRequestHashesAt,
               AsyncCertifiedRequestHash
      <3>3. AsyncTransportCompletionOwnerGateAllows(response)
        BY <1>1, <3>2, Isa
           DEF ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual,
               ExactDecisionResponseNonPhysicalHeadGateOwnerResidual,
               ExactDecisionResponsePhysicalCompletionResidual
      <3>4. IngressDepth(response.envelope.recipient)
               < IngressUsableCapacityAfterAdmission(response)
        BY <1>1, <2>1, <3>3,
           FreshCertifiedResponsePhysicalGateSuppliesIngressCapacity
      <3>5. /\ AsyncTimeoutVoteByteGateAllows(response)
             /\ AsyncUntrustedGenericCompletionGateAllows(response)
        BY <2>1
           DEF AsyncTimeoutVoteByteGateAllows,
               AsyncUntrustedGenericCompletionGateAllows,
               IngressAdmissionClass
      <3>6. CanAdmitIngressItem(response)
        BY <3>2, <3>3, <3>4, <3>5
           DEF CanAdmitIngressItem
      <3> QED BY <3>6 DEF IngressPacketCanLeaveTransport
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ExactDecisionResponseDueHeadIsAdmissionReady ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual(
         node, qc, archive, request, response, packet)
    /\ packet.deadline <= asyncNow
    /\ packet = OldestDueSourcePacket(node, response.source)
    => ExactDecisionResponsePacketAdmissionReady(
         node, qc, archive, request, response, packet)
BY ExactDecisionResponseNonPhysicalNonClaimDueHeadCanLeaveTransport,
   ExactDecisionResponsePacketIsNeverPolicyRejected,
   GstResponsiveNodesAreUp,
   GstExcludesResponsiveReplayQuarantine,
   OldestDueSourcePacketFacts,
   ExpandENABLED, IsaT(300)
   DEF ExactDecisionResponsePacketAdmissionReady,
       ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual,
       ExactDecisionResponseNonPhysicalHeadGateOwnerResidual,
       ExactDecisionResponseHeadGateOwnerResidual,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionResponseAdmissionGoal,
       ExactDecisionExecutableFrontier,
       ExactDecisionExecutableOwner,
       IngressPacketCanLeaveTransport,
       IngressCoalescingGateAllows,
       PostGstAdmitHiddenPacket, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       DueSourcePackets,
       AsyncNonRunnerOuterFrame, AsyncNonCrashOuterFrame,
       AsyncCoreOuterFrame, AsyncAllVars, AsyncSchedulerVars,
       AsyncRecoveryVars, AsyncProducerVars,
       AsyncIoVars, AsyncDeferredVars,
       AsyncLocalAdmissionVars, LeaveCausalQueues, vars

THEOREM ExactDecisionResponseRemainingHeadGateIsDeadlineOrShadow ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual(
         node, qc, archive, request, response, packet)
    => \/ packet.deadline > asyncNow
       \/ packet # OldestDueSourcePacket(node, response.source)
BY ExactDecisionResponseDueHeadIsAdmissionReady, Isa
   DEF ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual,
       ExactDecisionResponseNonPhysicalHeadGateOwnerResidual,
       ExactDecisionResponseHeadGateOwnerResidual

THEOREM ExactDecisionResponseAdmissionCreatesExactOutcome ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponseAdmissionResidual(
         node, qc, archive, request, response, packet)
    /\ packet = OldestDueSourcePacket(node, response.source)
    /\ PostGstAdmitHiddenPacket(node, response.source)
    => ExactDecisionResponseAdmissionOutcome(node, qc, response)'
BY FreshExactResponseAdmissionCreatesRouteNeutralIngressOwner,
   CoalescedExactResponseCreatesRouteNeutralIngressOwner,
   ExactDecisionResponsePacketIsNeverPolicyRejected, Isa
   DEF ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponseAdmissionOutcome,
       ExactDecisionResponseClaimIngressResidual,
       PostGstAdmitHiddenPacket, AdmitIngressPacket,
       AdmitFreshHiddenPacket

(***************************************************************************
Exact ready-admission fairness interface.

The active-request fence prevents a fresh generic untrusted completion from
refilling the normalized owner once an old completion drains.  Exact
Decision fanout retention also freezes the response authority while the
packet is ready.  Consequently the fair per-(recipient, outer source)
admission action is continuously enabled until it removes this exact packet
and creates either the route-neutral claim owner or the terminal executable
frontier.
***************************************************************************)

ExactDecisionResponseAdmissionReadyPersistsOrOutcome(
    node, qc, archive, request, response, packet) ==
  ExactDecisionResponsePacketAdmissionReady(
    node, qc, archive, request, response, packet)
    => \/ ExactDecisionResponsePacketAdmissionReady(
            node, qc, archive, request, response, packet)'
       \/ ExactDecisionResponseAdmissionOutcome(node, qc, response)'

THEOREM ExactDecisionResponseReadyEnablesFairAdmission ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponsePacketAdmissionReady(
         node, qc, archive, request, response, packet)
    /\ ~ExactDecisionResponseAdmissionOutcome(node, qc, response)
    => ENABLED
         <<PostGstAdmitHiddenPacket(
             node, response.source)>>_AsyncAllVars
BY ENABLEDaxioms, Isa
   DEF ExactDecisionResponsePacketAdmissionReady,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponseAdmissionOutcome,
       ExactDecisionResponseAdmissionGoal,
       PostGstAdmitHiddenPacket,
       AsyncAllVars

THEOREM ExactDecisionResponseFairAdmissionCreatesOutcome ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponsePacketAdmissionReady(
         node, qc, archive, request, response, packet)
    /\ ~ExactDecisionResponseAdmissionOutcome(node, qc, response)
    /\ <<PostGstAdmitHiddenPacket(
           node, response.source)>>_AsyncAllVars
    => ExactDecisionResponseAdmissionOutcome(node, qc, response)'
BY ExactDecisionResponseAdmissionCreatesExactOutcome, Isa
   DEF ExactDecisionResponsePacketAdmissionReady,
       ExactDecisionResponseAdmissionOutcome,
       ExactDecisionResponseAdmissionResidual

THEOREM ExactDecisionResponseAdmissionReadyStepIsSafe ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ ResponsiveRecoveryValidationClearedInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ ExactDecisionFanoutRetentionInvariant
    /\ ExactDecisionRequestAuthorityIsolationInvariant
    /\ [AsyncNext]_AsyncAllVars
    => ExactDecisionResponseAdmissionReadyPersistsOrOutcome(
         node, qc, archive, request, response, packet)
BY ExactDecisionResponseAdmissionCreatesExactOutcome,
   ExactDecisionResponsePacketIsNeverPolicyRejected,
   AsyncBracketStepRetainsExactDecisionRecord,
   AsyncBracketStepLeavesContext,
   AsyncNextPreservesExactDecisionFanoutRetention,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesFinalProgressWitnessClosure,
   GstAsyncStepIsMonotone, ExpandENABLED, IsaT(300)
   DEF ExactDecisionResponseAdmissionReadyPersistsOrOutcome,
       ExactDecisionResponsePacketAdmissionReady,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponseAdmissionOutcome,
       ExactDecisionResponseClaimIngressResidual,
       ExactDecisionResponseAdmissionGoal,
       ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionRecord,
       ExactDecisionRequestAuthorityIsolationInvariant,
       ExactDecisionRequestPacketOwned,
       CertifiedResponseFreshClaimGateAllows,
       CertifiedResponseRecipientClaimAvailable,
       CertifiedResponseAuthorityReady,
       CertifiedResponseAuthorityClaimed,
       AsyncTransportCompletionOwnerGateAllows,
       AsyncUntrustedGenericCompletionGateAllows,
       CanAdmitIngressItem,
       PostGstAdmitHiddenPacket,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DueSourcePackets, OldestDueSourcePacket,
       IngressHasCoalescingOwner,
       IngressPacketPolicyRejected,
       IngressResourceSource,
       IngressLane, IngressLaneDepth,
       SequenceSet

ExactDecisionResponseHeadGateResidualPersistsOrGoals(
    node, qc, archive, request, response, packet) ==
  ExactDecisionResponseHeadGateOwnerResidual(
    node, qc, archive, request, response, packet)
    => \/ ExactDecisionResponseHeadGateOwnerResidual(
            node, qc, archive, request, response, packet)'
       \/ ExactDecisionResponseAdmissionGoal(node, qc)'
       \/ ExactDecisionResponsePacketAdmissionReady(
            node, qc, archive, request, response, packet)'

THEOREM ExactDecisionResponseHeadGateResidualStepIsSafe ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ ResponsiveRecoveryValidationClearedInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ ExactDecisionFanoutRetentionInvariant
    /\ ExactDecisionRequestAuthorityIsolationInvariant
    /\ [AsyncNext]_AsyncAllVars
    => ExactDecisionResponseHeadGateResidualPersistsOrGoals(
         node, qc, archive, request, response, packet)
BY AsyncBracketStepRetainsExactDecisionRecord,
   AsyncBracketStepLeavesContext,
   AsyncNextPreservesExactDecisionFanoutRetention,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesFinalProgressWitnessClosure,
   GstAsyncStepIsMonotone, ExpandENABLED, IsaT(300)
   DEF ExactDecisionResponseHeadGateResidualPersistsOrGoals,
       ExactDecisionResponseHeadGateOwnerResidual,
       ExactDecisionResponsePacketAdmissionReady,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponseAdmissionGoal,
       ExactDecisionResponseClaimIngressResidual,
       ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionRecord,
       ExactDecisionRequestAuthorityIsolationInvariant,
       ExactDecisionRequestPacketOwned,
       ExactDecisionExecutableFrontier,
       ExactDecisionExecutableOwner,
       CertifiedResponseFreshClaimGateAllows,
       CertifiedResponseRecipientClaimAvailable,
       CertifiedResponseAuthorityReady,
       CertifiedResponseAuthorityClaimed,
       AsyncTransportCompletionOwnerGateAllows,
       AsyncUntrustedGenericCompletionGateAllows,
       CanAdmitIngressItem,
       PostGstAdmitHiddenPacket,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DueSourcePackets, OldestDueSourcePacket,
       IngressHasCoalescingOwner,
       IngressPacketPolicyRejected,
       IngressResourceSource,
       IngressLane, IngressLaneDepth,
       SequenceSet

(***************************************************************************
Fair fenced physical-completion service.

Finiteness is not a temporal argument.  The rank below orders the exact
physical owner count before a target-aware scheduler distance.  Its scheduler
component treats exhausted Ingress, Runtime, Local, and a positive-budget
Ingress turn as one acyclic path.  This avoids the invalid shortcut of using
`RuntimeReachRank` across its deliberate Runtime-to-Local reset.

While the physical count is positive no ingress admission can append another
physical completion to the same normalized lane.  A fair turn of the exact
recipient therefore either lowers the scheduler component or, at positive
Ingress budget, removes exactly one old owner and lowers the outer component.
***************************************************************************)

ExactDecisionPhysicalCompletionRunnerRank(node, response) ==
  <<TransportCompletionOwnerDebt(response),
    DrainableIngressTurnReachRank(node)>>

ExactDecisionPhysicalCompletionRunnerCarrier == Nat \X Nat

ExactDecisionPhysicalCompletionRunnerOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)

THEOREM ExactDecisionPhysicalCompletionRunnerOrderingIsWellFounded ==
  IsWellFoundedOn(
    ExactDecisionPhysicalCompletionRunnerOrdering,
    ExactDecisionPhysicalCompletionRunnerCarrier)
BY NatLessThanWellFounded, WFLexPairOrdering
   DEF ExactDecisionPhysicalCompletionRunnerOrdering,
       ExactDecisionPhysicalCompletionRunnerCarrier

ExactDecisionPhysicalCompletionRunnerGoal(
    node, qc, archive, request, response, packet) ==
  \/ ExactDecisionResponseAdmissionGoal(node, qc)
  \/ /\ ExactDecisionResponseAdmissionResidual(
          node, qc, archive, request, response, packet)
     /\ ~ExactDecisionResponsePhysicalCompletionResidual(
          node, qc, archive, request, response, packet)

ExactDecisionPhysicalCompletionAtRank(
    node, qc, archive, request, response, packet, rank) ==
  /\ AsyncStrongTypeInvariant
  /\ ExactDecisionResponsePhysicalCompletionResidual(
       node, qc, archive, request, response, packet)
  /\ ExactDecisionPhysicalCompletionRunnerRank(node, response) = rank

ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual(
    node, qc, archive, request, response, packet, rank) ==
  /\ ExactDecisionPhysicalCompletionAtRank(
       node, qc, archive, request, response, packet, rank)
  /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)

ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
    node, qc, archive, request, response, packet, rank) ==
  /\ ExactDecisionPhysicalCompletionAtRank(
       node, qc, archive, request, response, packet, rank)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(node)

ExactDecisionPhysicalCompletionRunnerProgress(
    node, qc, archive, request, response, packet, rank) ==
  \/ ExactDecisionPhysicalCompletionRunnerGoal(
       node, qc, archive, request, response, packet)
  \/ \E lower \in SetLessThan(
       rank,
       ExactDecisionPhysicalCompletionRunnerOrdering,
       ExactDecisionPhysicalCompletionRunnerCarrier):
       ExactDecisionPhysicalCompletionAtRank(
         node, qc, archive, request, response, packet, lower)

ExactDecisionPhysicalCompletionRunnerStrictResult(
    node, qc, archive, request, response, packet, rank) ==
  ExactDecisionPhysicalCompletionRunnerProgress(
    node, qc, archive, request, response, packet, rank)'

ExactDecisionPhysicalCompletionRunnerStepResult(
    node, qc, archive, request, response, packet, rank) ==
  \/ ExactDecisionPhysicalCompletionRunnerStrictResult(
       node, qc, archive, request, response, packet, rank)
  \/ ExactDecisionPhysicalCompletionAtRank(
       node, qc, archive, request, response, packet, rank)'

THEOREM ExactDecisionPhysicalCompletionRunnerRankInCarrier ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponsePhysicalCompletionResidual(
         node, qc, archive, request, response, packet)
    => ExactDecisionPhysicalCompletionRunnerRank(node, response)
         \in ExactDecisionPhysicalCompletionRunnerCarrier
BY ExactDecisionResponsePhysicalCompletionDebtIsFinite,
   AsyncStrongTypeProjectsAsyncType,
   DrainableIngressTurnReachRankIsNatural, Isa
   DEF ExactDecisionResponsePhysicalCompletionResidual,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionPhysicalCompletionRunnerRank,
       ExactDecisionPhysicalCompletionRunnerCarrier

THEOREM ExactDecisionPhysicalCompletionNonIngressRunLowersRank ==
  \A node \in ValidatorIds, response \in AsyncNetworkItems:
    /\ AsyncTypeInvariant
    /\ \/ LocalAdmissionStep(node)
       \/ SerializedRunnerRuntimeStep(node)
       \/ SerializedLocalPrecedesServeIngressStep(node)
       \/ AsyncServeIngressTargetOnlyTurn(node)
    => /\ DrainableIngressTurnReachRank(node)'
              < DrainableIngressTurnReachRank(node)
       /\ TransportCompletionOwnerDebt(response)'
              = TransportCompletionOwnerDebt(response)
BY LocalStepDecreasesDrainableIngressTurnReach,
   SerializedRunnerRuntimeDecreasesDrainableIngressTurnReach,
   SerializedLocalPredecessorDecreasesDrainableIngressTurnReach,
   ExactTicketTurnDecreasesDrainableIngressTurnReach,
   SerializedRuntimeLeavesIngress, IsaT(180)
   DEF LocalAdmissionStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       TransportCompletionOwnerDebt,
       TransportCompletionOwnerIndices, IngressLane

THEOREM ExactDecisionPhysicalCompletionSameNodeRunStrictlyProgresses ==
  \A node, qc, archive, request, response, packet:
    \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
    /\ ExactDecisionPhysicalCompletionAtRank(
         node, qc, archive, request, response, packet, rank)
    /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
    /\ PostGstRunNode(node)
    => ExactDecisionPhysicalCompletionRunnerStrictResult(
         node, qc, archive, request, response, packet, rank)
PROOF
  <1>1. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                NEW response, NEW packet,
                NEW rank
                  \in ExactDecisionPhysicalCompletionRunnerCarrier,
                ExactDecisionPhysicalCompletionAtRank(
                  node, qc, archive, request, response, packet, rank),
                ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
                  node),
                PostGstRunNode(node)
         PROVE ExactDecisionPhysicalCompletionRunnerStrictResult(
                 node, qc, archive, request, response, packet, rank)
    <2>1. /\ node \in ValidatorIds
           /\ AsyncTypeInvariant
           /\ asyncRunnerPhase[node]
                \in {"Local", "Ingress", "Runtime"}
           /\ asyncRunnerBudget[node] \in Nat
      BY <1>1, AsyncStrongTypeProjectsAsyncType, Isa
         DEF ExactDecisionPhysicalCompletionAtRank,
             ExactDecisionResponsePhysicalCompletionResidual,
             ExactDecisionResponseAdmissionResidual,
             ExactDecisionResponsePacketOwned,
             ExactDecisionAuthenticatedResponse,
             ExactDecisionBodyHoldingAlias,
             ExactDecisionActiveRequestOwner,
             ExactDecisionServiceSource,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant
    <2>2. AsyncStrongTypeInvariant'
      BY <1>1, AsyncBracketNextPreservesStrongTypeInvariant
         DEF PostGstRunNode
    <2>3. CASE asyncRunnerPhase[node] = "Local"
      <3>1. \/ LocalAdmissionStep(node)
             \/ SerializedLocalPrecedesServeIngressStep(node)
             \/ AsyncServeIngressTargetOnlyTurn(node)
        BY <1>1, <2>3, RunNodeWorkConcreteActionCaseSplit, Isa
           DEF PostGstRunNode, RunNode,
               IngressDrainStep, SerializedRunnerRuntimeStep,
               SerializedRuntimeStep,
               SerializedRuntimePrecedesServeIngressStep,
               SerializedLocalPrecedesServeIngressStep,
               SelectedLocalAdmissionAdvance
      <3>2. /\ DrainableIngressTurnReachRank(node)'
                       < DrainableIngressTurnReachRank(node)
              /\ TransportCompletionOwnerDebt(response)' =
                       TransportCompletionOwnerDebt(response)
        BY <2>1, <3>1,
           ExactDecisionPhysicalCompletionNonIngressRunLowersRank
      <3> QED BY <1>1, <2>2, <3>2, Isa
           DEF ExactDecisionPhysicalCompletionRunnerStrictResult,
               ExactDecisionPhysicalCompletionRunnerProgress,
               ExactDecisionPhysicalCompletionRunnerGoal,
               ExactDecisionPhysicalCompletionAtRank,
               ExactDecisionPhysicalCompletionRunnerRank,
               ExactDecisionPhysicalCompletionRunnerOrdering,
               ExactDecisionPhysicalCompletionRunnerCarrier,
               SetLessThan, LexPairOrdering, OpToRel
    <2>4. CASE asyncRunnerPhase[node] = "Ingress"
      <3>1. IngressDrainStep(node)
        BY <1>1
           DEF PostGstRunNode, RunNode, RunNodeWork
      <3>2. CASE asyncRunnerBudget[node] > 0
        <4>1. ExactDecisionDrainablePhysicalCompletionIngressReady(
                 node, qc, archive, request, response, packet)
          BY <1>1, <3>1, <3>2,
             ExactDecisionPhysicalCompletionResidualIsDrainable
             DEF ExactDecisionPhysicalCompletionAtRank,
                 ExactDecisionDrainablePhysicalCompletionIngressReady
        <4>2. TransportCompletionOwnerDebt(response)' + 1 =
                    TransportCompletionOwnerDebt(response)
          BY <1>1, <4>1,
             ExactDecisionIngressTurnDrainsFencedPhysicalOwner
             DEF ExactDecisionPhysicalCompletionAtRank
        <4> QED BY <1>1, <2>2, <4>2,
             ExactDecisionPhysicalCompletionRunnerRankInCarrier,
             DrainableIngressTurnReachRankIsNatural, Isa
             DEF ExactDecisionPhysicalCompletionRunnerStrictResult,
                 ExactDecisionPhysicalCompletionRunnerProgress,
                 ExactDecisionPhysicalCompletionRunnerGoal,
                 ExactDecisionPhysicalCompletionAtRank,
                 ExactDecisionPhysicalCompletionRunnerRank,
                 ExactDecisionPhysicalCompletionRunnerOrdering,
                 ExactDecisionPhysicalCompletionRunnerCarrier,
                 SetLessThan, LexPairOrdering, OpToRel
      <3>3. CASE ~(asyncRunnerBudget[node] > 0)
        <4>1. asyncRunnerBudget[node] = 0
          BY <2>1, <3>3, SMT
        <4>2. DrainableIngressTurnReachRank(node)'
                    < DrainableIngressTurnReachRank(node)
          BY <2>1, <3>1, <4>1,
             ExhaustedIngressStepDecreasesDrainableIngressTurnReach
        <4>3. TransportCompletionOwnerDebt(response)' =
                    TransportCompletionOwnerDebt(response)
          BY <3>1, <4>1, Isa
             DEF IngressDrainStep, TransportCompletionOwnerDebt,
                 TransportCompletionOwnerIndices, IngressLane
        <4> QED BY <1>1, <2>2, <4>2, <4>3, Isa
             DEF ExactDecisionPhysicalCompletionRunnerStrictResult,
                 ExactDecisionPhysicalCompletionRunnerProgress,
                 ExactDecisionPhysicalCompletionRunnerGoal,
                 ExactDecisionPhysicalCompletionAtRank,
                 ExactDecisionPhysicalCompletionRunnerRank,
                 ExactDecisionPhysicalCompletionRunnerOrdering,
                 ExactDecisionPhysicalCompletionRunnerCarrier,
                 SetLessThan, LexPairOrdering, OpToRel
      <3> QED BY <3>2, <3>3
    <2>5. CASE asyncRunnerPhase[node] = "Runtime"
      <3>1. \/ SerializedRunnerRuntimeStep(node)
             \/ AsyncServeIngressTargetOnlyTurn(node)
        BY <1>1, <2>5, RunNodeWorkConcreteActionCaseSplit, Isa
           DEF PostGstRunNode, RunNode,
               LocalAdmissionStep, IngressDrainStep,
               SerializedRunnerRuntimeStep,
               SerializedRuntimeStep,
               SerializedRuntimePrecedesServeIngressStep,
               SerializedLocalPrecedesServeIngressStep,
               SelectedLocalAdmissionAdvance
      <3>2. /\ DrainableIngressTurnReachRank(node)'
                       < DrainableIngressTurnReachRank(node)
              /\ TransportCompletionOwnerDebt(response)' =
                       TransportCompletionOwnerDebt(response)
        BY <2>1, <3>1,
           ExactDecisionPhysicalCompletionNonIngressRunLowersRank
      <3> QED BY <1>1, <2>2, <3>2, Isa
           DEF ExactDecisionPhysicalCompletionRunnerStrictResult,
               ExactDecisionPhysicalCompletionRunnerProgress,
               ExactDecisionPhysicalCompletionRunnerGoal,
               ExactDecisionPhysicalCompletionAtRank,
               ExactDecisionPhysicalCompletionRunnerRank,
               ExactDecisionPhysicalCompletionRunnerOrdering,
               ExactDecisionPhysicalCompletionRunnerCarrier,
               SetLessThan, LexPairOrdering, OpToRel
    <2> QED BY <2>1, <2>3, <2>4, <2>5
  <1> QED BY <1>1

THEOREM ExactDecisionPhysicalCompletionContinuationRunFramesTargetRank ==
  \A node, qc, archive, request, response, packet:
    \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
    /\ ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual(
         node, qc, archive, request, response, packet, rank)
    /\ PostGstRunNode(node)
    => \/ ExactDecisionPhysicalCompletionRunnerProgress(
            node, qc, archive, request, response, packet, rank)'
       \/ ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual(
            node, qc, archive, request, response, packet, rank)'
       \/ ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
            node, qc, archive, request, response, packet, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   ExactDecisionPhysicalCompletionRunnerRankInCarrier,
   HeadTailProperties, SequenceSetAfterAppend, IsaT(900)
   DEF ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual,
       ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry,
       ExactDecisionPhysicalCompletionRunnerProgress,
       ExactDecisionPhysicalCompletionRunnerGoal,
       ExactDecisionPhysicalCompletionAtRank,
       ExactDecisionPhysicalCompletionRunnerRank,
       ExactDecisionPhysicalCompletionRunnerOrdering,
       ExactDecisionPhysicalCompletionRunnerCarrier,
       ExactDecisionResponsePhysicalCompletionResidual,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       EnqueueCandidate, DrainableIngressTurnReachRank,
       TransportCompletionOwnerDebt,
       TransportCompletionOwnerIndices, IngressLane,
       SetLessThan, LexPairOrdering, OpToRel, AsyncAllVars

THEOREM ExactDecisionPhysicalCompletionOtherRunnerPreservesRank ==
  \A node, qc, archive, request, response, packet:
    \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
    /\ ExactDecisionPhysicalCompletionAtRank(
         node, qc, archive, request, response, packet, rank)
    /\ \/ \E other \in AsyncCurrentResponsiveVoters:
              /\ other # node
              /\ RunNode(other)
       \/ \E other \in AsyncResponsiveAppliedArchiveServers:
              RunHistoricalServer(other)
       \/ \E other \in asyncHistoricalRecoveryTargets:
              /\ other # node
              /\ RunHistoricalRecoveryNode(other)
    => ExactDecisionPhysicalCompletionRunnerStepResult(
         node, qc, archive, request, response, packet, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant, IsaT(300)
   DEF ExactDecisionPhysicalCompletionRunnerStepResult,
       ExactDecisionPhysicalCompletionRunnerStrictResult,
       ExactDecisionPhysicalCompletionRunnerProgress,
       ExactDecisionPhysicalCompletionRunnerGoal,
       ExactDecisionPhysicalCompletionAtRank,
       ExactDecisionPhysicalCompletionRunnerRank,
       DrainableIngressTurnReachRank,
       ExactDecisionResponsePhysicalCompletionResidual,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       TransportCompletionOwnerDebt,
       TransportCompletionOwnerIndices,
       IngressLane,
       RunNode, RunNodeWork,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RunHistoricalServer, RunHistoricalRecoveryNode,
       AsyncAllVars, AsyncSchedulerVars, AsyncProducerVars

THEOREM ExactDecisionPhysicalCompletionClockPreservesRank ==
  \A node, qc, archive, request, response, packet:
    \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
    /\ ExactDecisionPhysicalCompletionAtRank(
         node, qc, archive, request, response, packet, rank)
    /\ AsyncTick
    => ExactDecisionPhysicalCompletionRunnerStepResult(
         node, qc, archive, request, response, packet, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant, IsaT(180)
   DEF ExactDecisionPhysicalCompletionRunnerStepResult,
       ExactDecisionPhysicalCompletionRunnerStrictResult,
       ExactDecisionPhysicalCompletionRunnerProgress,
       ExactDecisionPhysicalCompletionRunnerGoal,
       ExactDecisionPhysicalCompletionAtRank,
       ExactDecisionPhysicalCompletionRunnerRank,
       DrainableIngressTurnReachRank,
       TransportCompletionOwnerDebt,
       TransportCompletionOwnerIndices,
       IngressLane, AsyncTick, AsyncNonClockVars,
       AsyncAllVars, AsyncSchedulerVars, AsyncProducerVars

THEOREM ExactDecisionPhysicalCompletionIoPreservesRank ==
  \A node, qc, archive, request, response, packet:
    \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
    /\ ExactDecisionPhysicalCompletionAtRank(
         node, qc, archive, request, response, packet, rank)
    /\ \/ \E ioNode \in AsyncArchiveIoServiceNodes:
              ServiceIoWorker(ioNode)
       \/ \E ioNode \in asyncHistoricalRecoveryTargets:
              ServiceHistoricalRecoveryIoWorker(ioNode)
       \/ \E ioNode \in AsyncCurrentResponsiveVoters:
              EnqueueIoLocalControl(ioNode)
       \/ \E ioNode \in asyncHistoricalRecoveryTargets:
              EnqueueHistoricalRecoveryIoLocalControl(ioNode)
    => ExactDecisionPhysicalCompletionRunnerStepResult(
         node, qc, archive, request, response, packet, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant, IsaT(300)
   DEF ExactDecisionPhysicalCompletionRunnerStepResult,
       ExactDecisionPhysicalCompletionRunnerStrictResult,
       ExactDecisionPhysicalCompletionRunnerProgress,
       ExactDecisionPhysicalCompletionRunnerGoal,
       ExactDecisionPhysicalCompletionAtRank,
       ExactDecisionPhysicalCompletionRunnerRank,
       DrainableIngressTurnReachRank,
       TransportCompletionOwnerDebt,
       TransportCompletionOwnerIndices,
       IngressLane,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncAllVars, AsyncSchedulerVars, AsyncProducerVars

THEOREM ExactDecisionPhysicalCompletionOuterPrefixPreservesRank ==
  \A node, qc, archive, request, response, packet:
    \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
    /\ ExactDecisionPhysicalCompletionAtRank(
         node, qc, archive, request, response, packet, rank)
    /\ \/ \E other \in ValidatorIds: OpenHistoricalRecovery(other)
       \/ \E other \in AsyncCurrentResponsiveVoters:
              DirectCommitCertificateDiscoveryStep(other)
       \/ \E other \in asyncHistoricalRecoveryTargets:
              DirectHistoricalCommitCertificateDiscoveryStep(other)
    => ExactDecisionPhysicalCompletionRunnerStepResult(
         node, qc, archive, request, response, packet, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant, IsaT(300)
   DEF ExactDecisionPhysicalCompletionRunnerStepResult,
       ExactDecisionPhysicalCompletionRunnerStrictResult,
       ExactDecisionPhysicalCompletionRunnerProgress,
       ExactDecisionPhysicalCompletionRunnerGoal,
       ExactDecisionPhysicalCompletionAtRank,
       ExactDecisionPhysicalCompletionRunnerRank,
       DrainableIngressTurnReachRank,
       TransportCompletionOwnerDebt,
       TransportCompletionOwnerIndices,
       IngressLane,
       OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       AsyncAllVars, AsyncSchedulerVars, AsyncProducerVars

THEOREM ExactDecisionPhysicalCompletionNetworkOrFaultPreservesRank ==
  \A node, qc, archive, request, response, packet:
    \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
    /\ ExactDecisionPhysicalCompletionAtRank(
         node, qc, archive, request, response, packet, rank)
    /\ (AsyncNetworkStep \/ AsyncFaultStep)
    => ExactDecisionPhysicalCompletionRunnerStepResult(
         node, qc, archive, request, response, packet, rank)
BY PositivePhysicalCompletionDebtAdmissionPreservesDebt,
   AsyncBracketNextPreservesStrongTypeInvariant,
   ExactDecisionResponsePhysicalCompletionDebtIsFinite, IsaT(300)
   DEF ExactDecisionPhysicalCompletionRunnerStepResult,
       ExactDecisionPhysicalCompletionRunnerStrictResult,
       ExactDecisionPhysicalCompletionRunnerProgress,
       ExactDecisionPhysicalCompletionRunnerGoal,
       ExactDecisionPhysicalCompletionAtRank,
       ExactDecisionPhysicalCompletionRunnerRank,
       DrainableIngressTurnReachRank,
       ExactDecisionResponsePhysicalCompletionResidual,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       TransportCompletionOwnerDebt,
       TransportCompletionOwnerIndices,
       IngressLane,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstCrash,
       InjectUntrustedTransportCompletion,
       AsyncAllVars, AsyncSchedulerVars, AsyncProducerVars

THEOREM ExactDecisionPhysicalCompletionRunnerStep ==
  \A node, qc, archive, request, response, packet:
    \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
    /\ ExactDecisionPhysicalCompletionAtRank(
         node, qc, archive, request, response, packet, rank)
    /\ [AsyncNext]_AsyncAllVars
    => ExactDecisionPhysicalCompletionRunnerStepResult(
         node, qc, archive, request, response, packet, rank)
PROOF
  <1>1. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                NEW response, NEW packet,
                NEW rank
                  \in ExactDecisionPhysicalCompletionRunnerCarrier,
                ExactDecisionPhysicalCompletionAtRank(
                  node, qc, archive, request, response, packet, rank),
                [AsyncNext]_AsyncAllVars
         PROVE ExactDecisionPhysicalCompletionRunnerStepResult(
                 node, qc, archive, request, response, packet, rank)
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>1, Isa
         DEF ExactDecisionPhysicalCompletionRunnerStepResult,
             ExactDecisionPhysicalCompletionAtRank,
             ExactDecisionPhysicalCompletionRunnerRank,
             DrainableIngressTurnReachRank,
             TransportCompletionOwnerDebt,
             TransportCompletionOwnerIndices,
             IngressLane, AsyncAllVars, AsyncSchedulerVars,
             AsyncProducerVars
    <2>2. CASE AsyncNext
      <3>1. CASE \E other \in AsyncCurrentResponsiveVoters:
                    RunNode(other)
        <4>1. CASE RunNode(node)
          <5>1. CASE
                   AsyncCandidateProducerContinuationRunnerResolutionRequired(
                     node)
            BY <1>1, <2>2, <4>1, <5>1,
               ExactDecisionPhysicalCompletionContinuationRunFramesTargetRank,
               Isa
               DEF ExactDecisionPhysicalCompletionRunnerStepResult,
                   ExactDecisionPhysicalCompletionRunnerStrictResult,
                   ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual,
                   ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry,
                   PostGstRunNode
          <5>2. CASE
                   ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
                     node)
            BY <1>1, <2>2, <4>1, <5>2,
               ExactDecisionPhysicalCompletionSameNodeRunStrictlyProgresses
               DEF ExactDecisionPhysicalCompletionRunnerStepResult,
                   PostGstRunNode,
                   ExactDecisionPhysicalCompletionAtRank
          <5> QED BY <5>1, <5>2
        <4>2. CASE ~RunNode(node)
          BY <1>1, <3>1, <4>2,
             ExactDecisionPhysicalCompletionOtherRunnerPreservesRank
        <4> QED BY <4>1, <4>2
      <3>2. CASE \E other \in AsyncResponsiveAppliedArchiveServers:
                    RunHistoricalServer(other)
        BY <1>1, <3>2,
           ExactDecisionPhysicalCompletionOtherRunnerPreservesRank
      <3>3. CASE \E other \in asyncHistoricalRecoveryTargets:
                    RunHistoricalRecoveryNode(other)
        <4>1. CASE RunNode(node)
          <5>1. CASE
                   AsyncCandidateProducerContinuationRunnerResolutionRequired(
                     node)
            BY <1>1, <2>2, <4>1, <5>1,
               ExactDecisionPhysicalCompletionContinuationRunFramesTargetRank,
               Isa
               DEF ExactDecisionPhysicalCompletionRunnerStepResult,
                   ExactDecisionPhysicalCompletionRunnerStrictResult,
                   ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual,
                   ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry,
                   PostGstRunNode
          <5>2. CASE
                   ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
                     node)
            BY <1>1, <2>2, <4>1, <5>2,
               ExactDecisionPhysicalCompletionSameNodeRunStrictlyProgresses
               DEF ExactDecisionPhysicalCompletionRunnerStepResult,
                   PostGstRunNode,
                   ExactDecisionPhysicalCompletionAtRank
          <5> QED BY <5>1, <5>2
        <4>2. CASE ~RunNode(node)
          BY <1>1, <3>3, <4>2,
             ExactDecisionPhysicalCompletionOtherRunnerPreservesRank
             DEF RunNode, RunHistoricalRecoveryNode
        <4> QED BY <4>1, <4>2
      <3>4. CASE AsyncTick
        BY <1>1, <3>4,
           ExactDecisionPhysicalCompletionClockPreservesRank
      <3>5. CASE \E other \in ValidatorIds:
                    OpenHistoricalRecovery(other)
        BY <1>1, <3>5,
           ExactDecisionPhysicalCompletionOuterPrefixPreservesRank
      <3>6. CASE \/ \E discoveryNode \in AsyncCurrentResponsiveVoters:
                          DirectCommitCertificateDiscoveryStep(discoveryNode)
                   \/ \E historicalNode \in asyncHistoricalRecoveryTargets:
                          DirectHistoricalCommitCertificateDiscoveryStep(
                            historicalNode)
        BY <1>1, <3>6,
           ExactDecisionPhysicalCompletionOuterPrefixPreservesRank
      <3>7. CASE \/ \E ioNode \in AsyncArchiveIoServiceNodes:
                          ServiceIoWorker(ioNode)
                   \/ \E recoveryIoNode \in asyncHistoricalRecoveryTargets:
                          ServiceHistoricalRecoveryIoWorker(recoveryIoNode)
                   \/ \E enqueueNode \in AsyncCurrentResponsiveVoters:
                          EnqueueIoLocalControl(enqueueNode)
                   \/ \E recoveryEnqueueNode
                          \in asyncHistoricalRecoveryTargets:
                          EnqueueHistoricalRecoveryIoLocalControl(
                            recoveryEnqueueNode)
        BY <1>1, <3>7,
           ExactDecisionPhysicalCompletionIoPreservesRank
      <3>8. CASE AsyncNetworkStep \/ AsyncFaultStep
        BY <1>1, <3>8,
           ExactDecisionPhysicalCompletionNetworkOrFaultPreservesRank
      <3>9. CASE AsyncSetGST
        BY <1>1, <3>9
           DEF ExactDecisionPhysicalCompletionAtRank,
               ExactDecisionResponsePhysicalCompletionResidual,
               ExactDecisionResponseAdmissionResidual,
               ExactDecisionResponsePacketOwned,
               ExactDecisionAuthenticatedResponse,
               ExactDecisionBodyHoldingAlias,
               ExactDecisionActiveRequestOwner,
               ExactDecisionServiceSource, AsyncSetGST
      <3>10. CASE \E other \in ValidatorIds: PreGstCrash(other)
        BY <1>1, <3>10
           DEF ExactDecisionPhysicalCompletionAtRank,
               ExactDecisionResponsePhysicalCompletionResidual,
               ExactDecisionResponseAdmissionResidual,
               ExactDecisionResponsePacketOwned,
               ExactDecisionAuthenticatedResponse,
               ExactDecisionBodyHoldingAlias,
               ExactDecisionActiveRequestOwner,
               ExactDecisionServiceSource, PreGstCrash
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
           <3>7, <3>8, <3>9, <3>10
           DEF AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
               AsyncNonRunnerStep
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM ExactDecisionPhysicalCompletionEnablesFairRunNode ==
  \A node, qc, archive, request, response, packet, rank:
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ ExactDecisionPhysicalCompletionAtRank(
         node, qc, archive, request, response, packet, rank)
      => ENABLED <<PostGstRunNode(node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                NEW response, NEW packet, NEW rank,
                AsyncCandidateProducerContinuationExternalCoverageInvariant,
                AsyncCandidateProducerContinuationLocalReplayCapacityInvariant,
                ExactDecisionPhysicalCompletionAtRank(
                  node, qc, archive, request, response, packet, rank)
         PROVE ENABLED <<PostGstRunNode(node)>>_AsyncAllVars
    <2>1. /\ node \in AsyncCurrentResponsiveVoters
           /\ node \in ValidatorIds
           /\ gst
           /\ ~NodeHasApplication(node)
           /\ AsyncStrongTypeInvariant
           /\ AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType,
         AsyncCurrentResponsiveVotersAreValidators, Isa
         DEF ExactDecisionPhysicalCompletionAtRank,
             ExactDecisionResponsePhysicalCompletionResidual,
             ExactDecisionResponseAdmissionResidual,
             ExactDecisionResponsePacketOwned,
             ExactDecisionAuthenticatedResponse,
             ExactDecisionBodyHoldingAlias,
             ExactDecisionActiveRequestOwner,
             ExactDecisionServiceSource
    <2>2. ENABLED PostGstRunNode(node)
      BY <2>1, GstResponsiveUnappliedRunNodeIsEnabled
    <2>3. PostGstRunNode(node)
             => <<PostGstRunNode(node)>>_AsyncAllVars
      BY <2>1, RunNodeIsNonstuttering, Isa
         DEF PostGstRunNode
    <2> QED BY <2>2, <2>3, ENABLEDaxioms
  <1> QED BY <1>1

ExactDecisionPhysicalCompletionContinuationClosureProperty(
    specification, initialContext) ==
  specification
    => \A node, qc, archive, request, response, packet:
         \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
           ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual(
             node, qc, archive, request, response, packet, rank)
             ~> (ExactDecisionPhysicalCompletionRunnerProgress(
                   node, qc, archive, request, response, packet, rank)
                  \/ ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
                       node, qc, archive, request, response, packet, rank))

THEOREM AsyncSpecClosesExactDecisionPhysicalCompletionContinuation ==
  \A initialContext:
    ExactDecisionPhysicalCompletionContinuationClosureProperty(
      AsyncSpecAt(initialContext), initialContext)
BY AsyncSpecProvidesVoterCandidateProducerContinuationResolutionClosure,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   ExactDecisionPhysicalCompletionRunnerStep,
   PTL, IsaT(900)
   DEF ExactDecisionPhysicalCompletionContinuationClosureProperty,
       ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual,
       ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry,
       ExactDecisionPhysicalCompletionRunnerStepResult,
       ExactDecisionPhysicalCompletionRunnerStrictResult,
       ExactDecisionPhysicalCompletionAtRank,
       ExactDecisionResponsePhysicalCompletionResidual,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       AsyncVoterCandidateProducerContinuationResolutionClosureProperty,
       AsyncVoterCandidateProducerContinuationEpisodePending

THEOREM FairExactDecisionPhysicalCompletionClearedPrefixOneStep ==
  \A initialContext:
    \A node, qc, archive, request, response, packet:
      \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
        AsyncSpecAt(initialContext)
          => (ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
                node, qc, archive, request, response, packet, rank)
                ~> ExactDecisionPhysicalCompletionRunnerProgress(
                     node, qc, archive, request, response, packet, rank))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node, NEW qc, NEW archive, NEW request,
                NEW response, NEW packet,
                NEW rank
                  \in ExactDecisionPhysicalCompletionRunnerCarrier
         PROVE AsyncSpecAt(initialContext)
                 => (ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
                       node, qc, archive, request, response, packet, rank)
                       ~> ExactDecisionPhysicalCompletionRunnerProgress(
                            node, qc, archive, request, response,
                            packet, rank))
    <2>1. AsyncSpecAt(initialContext)
             => /\ [](AsyncCurrentResponsiveVoters
                       = AsyncVotersAt(initialContext))
                /\ []AsyncCandidateProducerContinuationExternalCoverageInvariant
                /\ []AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters,
         AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
         AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity
    <2>2. ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
             node, qc, archive, request, response, packet, rank)
             /\ ~ExactDecisionPhysicalCompletionRunnerProgress(
                  node, qc, archive, request, response, packet, rank)
            => ENABLED <<PostGstRunNode(node)>>_AsyncAllVars
      BY <2>1, ExactDecisionPhysicalCompletionEnablesFairRunNode, PTL
         DEF ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry
    <2>3. ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
             node, qc, archive, request, response, packet, rank)
             /\ ~ExactDecisionPhysicalCompletionRunnerProgress(
                  node, qc, archive, request, response, packet, rank)
             /\ <<PostGstRunNode(node)>>_AsyncAllVars
            => ExactDecisionPhysicalCompletionRunnerProgress(
                 node, qc, archive, request, response, packet, rank)'
      BY ExactDecisionPhysicalCompletionSameNodeRunStrictlyProgresses
         DEF ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry,
             ExactDecisionPhysicalCompletionRunnerStrictResult
    <2>4. ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
             node, qc, archive, request, response, packet, rank)
             /\ [AsyncNext]_AsyncAllVars
            => \/ ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
                    node, qc, archive, request, response, packet, rank)'
               \/ ExactDecisionPhysicalCompletionRunnerProgress(
                    node, qc, archive, request, response, packet, rank)'
      BY ExactDecisionPhysicalCompletionRunnerStep,
         AsyncSharedSchedulerHighWatermarkIsMonotone,
         AsyncCandidateProducerContinuationLaterOrdinalCannotOwnRunnerTurn,
         IsaT(900)
         DEF ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry,
             ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual,
             ExactDecisionPhysicalCompletionRunnerStepResult,
             ExactDecisionPhysicalCompletionRunnerStrictResult
    <2>5. CASE node \in AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(PostGstRunNode(node))
        BY <2>5 DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
                     node, qc, archive, request, response, packet, rank)
                     ~> ExactDecisionPhysicalCompletionRunnerProgress(
                          node, qc, archive, request, response,
                          packet, rank))
        BY <2>2, <2>3, <2>4, <3>1, PTL DEF AsyncSpecAt
      <3> QED BY <3>2
    <2>6. CASE node \notin AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => []~ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
                      node, qc, archive, request, response, packet, rank)
        BY <2>1, <2>6, PTL
           DEF ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry,
               ExactDecisionPhysicalCompletionAtRank,
               ExactDecisionResponsePhysicalCompletionResidual,
               ExactDecisionResponseAdmissionResidual,
               ExactDecisionResponsePacketOwned,
               ExactDecisionAuthenticatedResponse,
               ExactDecisionBodyHoldingAlias
      <3> QED BY <3>1, PTL
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM FairExactDecisionPhysicalCompletionRunnerOneStep ==
  \A initialContext:
    \A node, qc, archive, request, response, packet:
      \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
        AsyncSpecAt(initialContext)
          => (ExactDecisionPhysicalCompletionAtRank(
                node, qc, archive, request, response, packet, rank)
                ~> ExactDecisionPhysicalCompletionRunnerProgress(
                     node, qc, archive, request, response, packet, rank))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node, NEW qc, NEW archive, NEW request,
                NEW response, NEW packet,
                NEW rank
                  \in ExactDecisionPhysicalCompletionRunnerCarrier
         PROVE AsyncSpecAt(initialContext)
                 => (ExactDecisionPhysicalCompletionAtRank(
                       node, qc, archive, request, response, packet, rank)
                       ~> ExactDecisionPhysicalCompletionRunnerProgress(
                            node, qc, archive, request, response,
                            packet, rank))
    <2>1. AsyncSpecAt(initialContext)
             => /\ [](AsyncCurrentResponsiveVoters
                       = AsyncVotersAt(initialContext))
                /\ []AsyncCandidateProducerContinuationExternalCoverageInvariant
                /\ []AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters,
         AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
         AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity
    <2>2. ExactDecisionPhysicalCompletionAtRank(
             node, qc, archive, request, response, packet, rank)
             /\ ~ExactDecisionPhysicalCompletionRunnerProgress(
                  node, qc, archive, request, response, packet, rank)
            => ENABLED <<PostGstRunNode(node)>>_AsyncAllVars
      BY <2>1, ExactDecisionPhysicalCompletionEnablesFairRunNode, PTL
    <2>3. ExactDecisionPhysicalCompletionAtRank(
             node, qc, archive, request, response, packet, rank)
             /\ ~(ExactDecisionPhysicalCompletionRunnerProgress(
                    node, qc, archive, request, response, packet, rank)
                  \/ ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual(
                       node, qc, archive, request,
                       response, packet, rank))
             /\ <<PostGstRunNode(node)>>_AsyncAllVars
            => ExactDecisionPhysicalCompletionRunnerProgress(
                 node, qc, archive, request, response, packet, rank)'
      BY ExactDecisionPhysicalCompletionSameNodeRunStrictlyProgresses
         DEF ExactDecisionPhysicalCompletionRunnerStrictResult,
             ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual
    <2>4. ExactDecisionPhysicalCompletionAtRank(
             node, qc, archive, request, response, packet, rank)
             /\ [AsyncNext]_AsyncAllVars
            => \/ ExactDecisionPhysicalCompletionAtRank(
                    node, qc, archive, request, response, packet, rank)'
               \/ ExactDecisionPhysicalCompletionRunnerProgress(
                    node, qc, archive, request, response, packet, rank)'
      BY ExactDecisionPhysicalCompletionRunnerStep
         DEF ExactDecisionPhysicalCompletionRunnerStepResult,
             ExactDecisionPhysicalCompletionRunnerStrictResult
    <2>5. CASE node \in AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(PostGstRunNode(node))
        BY <2>5 DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (ExactDecisionPhysicalCompletionAtRank(
                     node, qc, archive, request, response, packet, rank)
                     ~> (ExactDecisionPhysicalCompletionRunnerProgress(
                           node, qc, archive, request, response,
                           packet, rank)
                          \/ ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual(
                               node, qc, archive, request,
                               response, packet, rank)))
        BY <2>2, <2>3, <2>4, <3>1, PTL DEF AsyncSpecAt
      <3>3. AsyncSpecAt(initialContext)
               => (ExactDecisionPhysicalCompletionCandidateProducerContinuationResidual(
                     node, qc, archive, request, response, packet, rank)
                     ~> (ExactDecisionPhysicalCompletionRunnerProgress(
                           node, qc, archive, request, response,
                           packet, rank)
                          \/ ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
                               node, qc, archive, request,
                               response, packet, rank)))
        BY AsyncSpecClosesExactDecisionPhysicalCompletionContinuation
           DEF ExactDecisionPhysicalCompletionContinuationClosureProperty
      <3>4. AsyncSpecAt(initialContext)
               => (ExactDecisionPhysicalCompletionCandidateProducerContinuationReentry(
                     node, qc, archive, request, response, packet, rank)
                     ~> ExactDecisionPhysicalCompletionRunnerProgress(
                          node, qc, archive, request, response,
                          packet, rank))
        BY FairExactDecisionPhysicalCompletionClearedPrefixOneStep
      <3>5. AsyncSpecAt(initialContext)
               => (ExactDecisionPhysicalCompletionAtRank(
                     node, qc, archive, request, response, packet, rank)
                     ~> ExactDecisionPhysicalCompletionRunnerProgress(
                          node, qc, archive, request, response,
                          packet, rank))
        BY <3>2, <3>3, <3>4, PTL
      <3> QED BY <3>5
    <2>6. CASE node \notin AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => []~ExactDecisionPhysicalCompletionAtRank(
                     node, qc, archive, request, response, packet, rank)
        BY <2>1, <2>6, PTL
           DEF ExactDecisionPhysicalCompletionAtRank,
               ExactDecisionResponsePhysicalCompletionResidual,
               ExactDecisionResponseAdmissionResidual,
               ExactDecisionResponsePacketOwned,
               ExactDecisionAuthenticatedResponse,
               ExactDecisionBodyHoldingAlias,
               ExactDecisionActiveRequestOwner,
               ExactDecisionServiceSource
      <3>2. AsyncSpecAt(initialContext)
               => (ExactDecisionPhysicalCompletionAtRank(
                     node, qc, archive, request, response, packet, rank)
                     ~> ExactDecisionPhysicalCompletionRunnerProgress(
                          node, qc, archive, request, response,
                          packet, rank))
        BY <3>1, PTL
      <3> QED BY <3>2
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1
       DEF ExactDecisionPhysicalCompletionRunnerProgress

THEOREM FairExactDecisionPhysicalCompletionRankDescent ==
  \A initialContext:
    \A node, qc, archive, request, response, packet:
      AsyncSpecAt(initialContext)
        => \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
             ExactDecisionPhysicalCompletionAtRank(
               node, qc, archive, request, response, packet, rank)
               ~> ExactDecisionPhysicalCompletionRunnerGoal(
                    node, qc, archive, request, response, packet)
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node, NEW qc, NEW archive, NEW request,
                NEW response, NEW packet
         PROVE AsyncSpecAt(initialContext)
                 => \A rank
                       \in ExactDecisionPhysicalCompletionRunnerCarrier:
                      ExactDecisionPhysicalCompletionAtRank(
                        node, qc, archive, request, response,
                        packet, rank)
                        ~> ExactDecisionPhysicalCompletionRunnerGoal(
                             node, qc, archive, request,
                             response, packet)
    <2>1. ASSUME NEW rank
                  \in ExactDecisionPhysicalCompletionRunnerCarrier
           PROVE AsyncSpecAt(initialContext)
                   => (ExactDecisionPhysicalCompletionAtRank(
                         node, qc, archive, request, response,
                         packet, rank)
                         ~> (ExactDecisionPhysicalCompletionRunnerGoal(
                               node, qc, archive, request,
                               response, packet)
                              \/ \E lower \in SetLessThan(
                                   rank,
                                   ExactDecisionPhysicalCompletionRunnerOrdering,
                                   ExactDecisionPhysicalCompletionRunnerCarrier):
                                   ExactDecisionPhysicalCompletionAtRank(
                                     node, qc, archive, request,
                                     response, packet, lower)))
      BY FairExactDecisionPhysicalCompletionRunnerOneStep
         DEF ExactDecisionPhysicalCompletionRunnerProgress
    <2>2. AsyncSpecAt(initialContext)
             => \A rank
                   \in ExactDecisionPhysicalCompletionRunnerCarrier:
                  ExactDecisionPhysicalCompletionAtRank(
                    node, qc, archive, request, response, packet, rank)
                    ~> (ExactDecisionPhysicalCompletionRunnerGoal(
                          node, qc, archive, request, response, packet)
                         \/ \E lower \in SetLessThan(
                              rank,
                              ExactDecisionPhysicalCompletionRunnerOrdering,
                              ExactDecisionPhysicalCompletionRunnerCarrier):
                              ExactDecisionPhysicalCompletionAtRank(
                                node, qc, archive, request,
                                response, packet, lower))
      BY <2>1
    <2> QED BY <2>2,
         ExactDecisionPhysicalCompletionRunnerOrderingIsWellFounded,
         WellFoundedLeadsTo
  <1> QED BY <1>1

ExactDecisionResponsePhysicalCompletionConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, archive, request, response, packet:
         ExactDecisionResponsePhysicalCompletionResidual(
           node, qc, archive, request, response, packet)
           ~> ExactDecisionPhysicalCompletionRunnerGoal(
                node, qc, archive, request, response, packet)

THEOREM ExactDecisionResponsePhysicalCompletionConvergence ==
  \A initialContext:
    ExactDecisionResponsePhysicalCompletionConvergenceProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ExactDecisionResponsePhysicalCompletionConvergenceProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node, qc, archive, request, response, packet:
                    ExactDecisionResponsePhysicalCompletionResidual(
                      node, qc, archive, request, response, packet)
                      ~> ExactDecisionPhysicalCompletionRunnerGoal(
                           node, qc, archive, request, response, packet)
      <3>1. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
        BY AsyncSpecAlwaysStrongTypeInvariant
      <3>2. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                    NEW response, NEW packet
             PROVE ExactDecisionResponsePhysicalCompletionResidual(
                     node, qc, archive, request, response, packet)
                     ~> ExactDecisionPhysicalCompletionRunnerGoal(
                          node, qc, archive, request, response, packet)
        <4>1. AsyncSpecAt(initialContext)
                 => \A rank
                       \in ExactDecisionPhysicalCompletionRunnerCarrier:
                      ExactDecisionPhysicalCompletionAtRank(
                        node, qc, archive, request, response,
                        packet, rank)
                        ~> ExactDecisionPhysicalCompletionRunnerGoal(
                             node, qc, archive, request,
                             response, packet)
          BY FairExactDecisionPhysicalCompletionRankDescent
        <4>2. /\ AsyncStrongTypeInvariant
               /\ ExactDecisionResponsePhysicalCompletionResidual(
                    node, qc, archive, request, response, packet)
              => \E rank
                    \in ExactDecisionPhysicalCompletionRunnerCarrier:
                   ExactDecisionPhysicalCompletionAtRank(
                     node, qc, archive, request, response, packet, rank)
          BY ExactDecisionPhysicalCompletionRunnerRankInCarrier
             DEF ExactDecisionPhysicalCompletionAtRank
        <4> QED BY <2>1, <3>1, <4>1, <4>2, PTL
             DEF ExactDecisionPhysicalCompletionRunnerGoal
      <3> QED BY <3>2
    <2> QED BY <2>1
         DEF ExactDecisionResponsePhysicalCompletionConvergenceProperty
  <1> QED BY <1>1

ExactDecisionResponseNormalDrainAction(node, response) ==
  \E admitted:
    /\ AsyncCertifiedResponseAuthProjection(admitted)
         = AsyncCertifiedResponseAuthProjection(response)
    /\ SelectedIngressItemAt(
         node, FirstDrainableIngressIndex(node)) = admitted
    /\ DrainFairIngressSelected(node)

THEOREM ExactDecisionNormalResponseDrainCreatesAdmissionGoal ==
  \A node, qc, response:
    /\ AsyncStrongTypeInvariant
    /\ DecisionsUniqueByNodeContext
    /\ ExactDecisionResponseClaimIngressResidual(node, qc, response)
    /\ ExactDecisionResponseNormalDrainAction(node, response)
    => ExactDecisionResponseAdmissionGoal(node, qc)'
BY ExactDecisionResponseLineageTransfersAcrossRouteNeutralIdentity,
   MatchingClaimedCertifiedResponseIsAuthorized,
   ExactDecisionClaimedResponseDrainCreatesExecutableFrontier, IsaT(180)
   DEF ExactDecisionResponseClaimIngressResidual,
       ExactDecisionRouteNeutralClaimIngressOwned,
       ExactDecisionResponseNormalDrainAction,
       ExactDecisionClaimedResponseIngressOwned,
       ExactDecisionResponseAdmissionGoal,
       ExactDecisionExecutableFrontier,
       CertifiedResponseClaimMatches,
       IngressResourceSource

THEOREM ExactDecisionServeResidualHeadExitCreatesGoal ==
  \A node, qc, archive, request, job:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionServeResponseResidual(
         node, qc, archive, request, job)
    /\ Head(asyncIoQueues[archive]) = job
    /\ ServiceIoWorkerWork(archive)
    => ExactDecisionServeResponseGoal(
         node, qc, archive, request)'
BY ExactServeHeadCreatesAuthenticatedResponsePacket, Isa
   DEF ExactDecisionServeResponseResidual,
       ExactDecisionServeResponseGoal

THEOREM ExactDecisionServeResidualProjectsProtectedOwner ==
  \A node, qc, archive, request, job:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionServeResponseResidual(
         node, qc, archive, request, job)
    => /\ gst
       /\ archive \in Responsive
       /\ job \in AsyncServeJobSet
       /\ ResponsiveProtectedServeJobOwned(archive, job)
BY ExactDecisionServeJobProjectsProtectedOwner
   DEF ExactDecisionServeResponseResidual

(***************************************************************************
Serve safety needs two state facts which are implicit in the Core action
vocabulary but were not previously exported at the asynchronous boundary:
durable bodies only grow, and an exact durable Decision record is never
retired.  The small bracket lemmas below expose exactly those facts without
adding an invariant or a temporal assumption.
***************************************************************************)

THEOREM CoreBracketStepRetainsDurableBodies ==
  [Next]_vars => durableBodies \subseteq durableBodies'
BY IsaM("blast")
   DEF Next, SetGST, AssembleLocalBody, BeginLocalProposal,
       PersistProposal, CompleteProposalSignature,
       ByzantineBroadcastProposal, DeliverProposal,
       FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, ValidateDecidedBody, ValidateLockedBody, RejectBody,
       BeginPrepare, PersistPrepare,
       CompleteVoteSignature, ByzantineBroadcastVote, DeliverVote,
       FormPrepareQC, ImportAuthenticatedCommitCertificate, DeliverQC,
       BeginObservePrepare, PersistObservePrepare,
       BeginLockCommit, PersistLockCommit, FormCommitQC,
       BeginDecision, PersistDecision, BeginTimeout, PersistTimeout,
       CompleteTimeoutSignature, ByzantineBroadcastTimeout,
       DeliverTimeout, FormTC, DeliverTC,
       BeginInstallTC, PersistInstallTC,
       FetchCertifiedBody, AcceptCertifiedResponseCapability,
       InstallCertifiedBodyEffect, ApplyDecision,
       Crash, Restart, ResumeProposal, ResumeVote, ResumeTimeout,
       DropProposal, vars

THEOREM AsyncBracketStepRetainsDurableBodies ==
  [AsyncNext]_AsyncAllVars
    => durableBodies \subseteq durableBodies'
BY AsyncStepRefinementObligation,
   CoreBracketStepRetainsDurableBodies, Isa
   DEF AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars,
       AsyncProducerVars, vars

(***************************************************************************
Split (a): preservation of the semantic alias.

The final exact-source invariant reconstructs the post-state recovery stage
from the retained durable Decision.  If no executable/application frontier
has appeared, exact-stage decomposition forces the active-request arm.
Fanout retention then restores this same concrete request alias, while Core
durability monotonicity retains the addressed archive body.
***************************************************************************)

THEOREM ExactDecisionBodyHoldingAliasPersistsOrFrontier ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ ResponsiveRecoveryValidationClearedInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ ExactDecisionFanoutRetentionInvariant
    /\ ExactDecisionBodyHoldingAlias(
         node, qc, archive, request)
    /\ [AsyncNext]_AsyncAllVars
    => \/ ExactDecisionBodyHoldingAlias(
            node, qc, archive, request)'
       \/ ExactDecisionExecutableFrontier(node, qc)'
PROOF
  <1>1. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                DecisionFrontierUniquenessInvariant,
                DecisionTimeoutFrontierInvariant,
                ResponsiveRecoveryValidationClearedInvariant,
                FinalProgressWitnessClosureInvariant,
                ExactDecisionFanoutRetentionInvariant,
                ExactDecisionBodyHoldingAlias(
                  node, qc, archive, request),
                [AsyncNext]_AsyncAllVars
         PROVE \/ ExactDecisionBodyHoldingAlias(
                     node, qc, archive, request)'
                \/ ExactDecisionExecutableFrontier(node, qc)'
    <2>1. AsyncStrongTypeInvariant'
      BY <1>1, AsyncBracketNextPreservesStrongTypeInvariant
    <2>2. FinalProgressWitnessClosureInvariant'
      BY <1>1, AsyncBracketNextPreservesFinalProgressWitnessClosure
    <2>3. ExactDecisionFanoutRetentionInvariant'
      BY <1>1, AsyncNextPreservesExactDecisionFanoutRetention
    <2>4. ExactDecisionRecord(node, qc)'
      BY <1>1, AsyncBracketStepRetainsExactDecisionRecord
         DEF ExactDecisionBodyHoldingAlias,
             ExactDecisionActiveRequestOwner,
             ExactDecisionServiceSource
    <2>5. gst'
      BY <1>1, GstAsyncStepIsMonotone
         DEF ExactDecisionBodyHoldingAlias,
             ExactDecisionActiveRequestOwner,
             ExactDecisionServiceSource
    <2>6. UNCHANGED context
      BY <1>1, AsyncBracketStepLeavesContext
    <2>7. /\ archive
                  \in (AsyncCurrentResponsiveVoters \ {node})'
           /\ node \in AsyncCurrentResponsiveVoters'
           /\ request \in CertifiedRequestOutbox(node, qc)'
           /\ request.envelope.recipient = archive
      BY <1>1, <2>6,
         CertifiedArchiveRoutesStableUnderContextFrame, Isa
         DEF ExactDecisionBodyHoldingAlias,
             ExactDecisionActiveRequestOwner,
             ExactDecisionServiceSource,
             ExactDecisionRecord,
             CertifiedRequestOutbox,
             AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch
    <2>8. BodyHeldBy(
             durableBodies', archive, qc.context',
             qc.view, qc.subject)
      BY <1>1, <2>6, AsyncBracketStepRetainsDurableBodies,
         BodyHeldIsMonotone, Isa
         DEF ExactDecisionBodyHoldingAlias
    <2>9. CASE ExactDecisionExecutableFrontier(node, qc)'
      BY <2>9
    <2>10. CASE ~ExactDecisionExecutableFrontier(node, qc)'
      <3>1. DecisionExactSourceRetentionInvariant'
        BY <2>2
           DEF FinalProgressWitnessClosureInvariant,
               FinalWitnessSourceRetentionInvariant
      <3>2. DecisionRecoveryStageExact(node, qc)'
        BY <2>1, <2>3, <2>4, <2>5, <2>7, <3>1,
           ExactDecisionSourceProjectsPostGstServiceStage
      <3>3. ExactDecisionServiceSource(node, qc)'
        BY <2>4, <2>5, <2>7, <3>2
           DEF ExactDecisionServiceSource
      <3>4. /\ ~NodeHasApplication(node)'
             /\ ~BodyHeldBy(
                  durableBodies', node, qc.context',
                  qc.view, qc.subject)
             /\ ~DecisionValidationHeld(node, qc)'
             /\ DecisionCertifiedRequestActiveExact(node, qc)'
        BY <2>10, <3>2, <3>3,
           ExactDecisionStageDecomposition, Isa
           DEF ExactDecisionExecutableFrontier,
               ExactDecisionExecutableOwner
      <3>5. ExactDecisionActiveRequestOwner(node, qc)'
        BY <3>3, <3>4 DEF ExactDecisionActiveRequestOwner
      <3>6. request \in asyncActiveRequests'
        BY <2>3, <2>4, <2>7, <3>4, Isa
           DEF ExactDecisionFanoutRetentionInvariant,
               ExactDecisionRecord
      <3>7. CertifiedServeCanRespond(archive, request)'
        BY <2>6, <2>7, <2>8, Isa
           DEF CertifiedServeCanRespond
      <3>8. ExactDecisionBodyHoldingAlias(
               node, qc, archive, request)'
        BY <2>7, <2>8, <3>5, <3>6, <3>7
           DEF ExactDecisionBodyHoldingAlias
      <3> QED BY <3>8
    <2> QED BY <2>9, <2>10
  <1> QED BY <1>1

(***************************************************************************
Split (b): preservation of the nonce-owned FIFO occurrence.

Nonce uniqueness makes a target Serve occurrence a linear queue owner.  Every
non-target action frames or appends that queue.  A worker for another head
removes only that head and leaves the unique target occurrence in the tail;
the only action which can remove the target itself is the exact target-head
`ServiceIoWorkerWork` action.
***************************************************************************)

THEOREM ExactDecisionServeOccurrencePersistsOrHeadServiced ==
  \A node, qc, archive, request, job:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionServeJobOwned(
         node, qc, archive, request, job)
    /\ [AsyncNext]_AsyncAllVars
    => \/ ExactDecisionServeOccurrenceOwned(
            archive, request, job)'
       \/ /\ Head(asyncIoQueues[archive]) = job
             /\ ServiceIoWorkerWork(archive)
BY ExactDecisionServeJobProjectsProtectedOwner,
   AsyncBracketNextPreservesStrongTypeInvariant,
   ServeOccurrenceIndexAfterNonTargetHead,
   TailRemovesUniqueServeOccurrence,
   AppendProperties, HeadTailProperties, IsaT(300)
   DEF ExactDecisionServeJobOwned,
       ExactDecisionServeOccurrenceOwned,
       ResponsiveProtectedServeJobOwned,
       AsyncServeJobSet, AsyncIoJob,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer, OpenHistoricalRecovery,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep,
       AdmitProducerCompletion, AdmitCausalHead,
       IngressDrainStep, DrainFairIngressSelected,
       DrainHistoricalIngressSelected,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncNetworkStep, AdmitIngressPacket, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       AsyncIoCertifiedServeJob,
       AsyncIoServeNonceOwnership, SequenceSet,
       AsyncAllVars

(***************************************************************************
The exact physical request handoff is closed action-locally.

For `CertifiedRequest`, policy rejection is impossible.  Thus the selected
exact packet either appends the exact request or coalesces with an equal
request already in the normalized ingress lane.  The earlier admission
theorem proves that exact ingress owner in both cases; splitting on the
post-state goal yields precisely the outcome used by the temporal kernels
below.
***************************************************************************)

THEOREM ExactDecisionRequestAdmissionCreatesExactOutcome ==
  \A node, qc, archive, request, packet:
    /\ ExactDecisionRequestIngressResidual(
         node, qc, archive, request, packet)
    /\ packet = OldestDueSourcePacket(
         archive, request.source)
    /\ PostGstAdmitHiddenPacket(
         archive, request.source)
    => ExactDecisionRequestAdmissionOutcome(
         node, qc, archive, request)'
BY ExactRequestPacketAdmissionCreatesIngressOwner, Isa
   DEF ExactDecisionRequestIngressResidual,
       ExactDecisionRequestAdmissionOutcome,
       ExactDecisionRequestIngressLaneResidual,
       PostGstAdmitHiddenPacket

(***************************************************************************
The exact request ready edge is fair for the same reason, but it owns a
validator-scoped Progress reservation rather than the untrusted response
owner.  Once this packet is the selected due head, no other outer source can
consume its source-isolated reservation; the per-source admission action
therefore remains enabled until the exact request is appended/coalesced or a
different exact pipeline handoff makes the request goal true.
***************************************************************************)

ExactDecisionRequestAdmissionReadyPersistsOrOutcome(
    node, qc, archive, request, packet) ==
  ExactDecisionRequestPacketAdmissionReady(
    node, qc, archive, request, packet)
    => \/ ExactDecisionRequestPacketAdmissionReady(
            node, qc, archive, request, packet)'
       \/ ExactDecisionRequestAdmissionOutcome(
            node, qc, archive, request)'

THEOREM ExactDecisionRequestReadyEnablesFairAdmission ==
  \A node, qc, archive, request, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestPacketAdmissionReady(
         node, qc, archive, request, packet)
    /\ ~ExactDecisionRequestAdmissionOutcome(
         node, qc, archive, request)
    => ENABLED
         <<PostGstAdmitHiddenPacket(
             archive, request.source)>>_AsyncAllVars
BY ENABLEDaxioms, Isa
   DEF ExactDecisionRequestPacketAdmissionReady,
       ExactDecisionRequestIngressResidual,
       ExactDecisionRequestAdmissionOutcome,
       ExactDecisionRequestIngressGoal,
       PostGstAdmitHiddenPacket,
       AsyncAllVars

THEOREM ExactDecisionRequestFairAdmissionCreatesOutcome ==
  \A node, qc, archive, request, packet:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestPacketAdmissionReady(
         node, qc, archive, request, packet)
    /\ ~ExactDecisionRequestAdmissionOutcome(
         node, qc, archive, request)
    /\ <<PostGstAdmitHiddenPacket(
           archive, request.source)>>_AsyncAllVars
    => ExactDecisionRequestAdmissionOutcome(
         node, qc, archive, request)'
BY ExactDecisionRequestAdmissionCreatesExactOutcome, Isa
   DEF ExactDecisionRequestPacketAdmissionReady,
       ExactDecisionRequestAdmissionOutcome,
       ExactDecisionRequestIngressResidual

THEOREM ExactDecisionRequestAdmissionReadyStepIsSafe ==
  \A node, qc, archive, request, packet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ ResponsiveRecoveryValidationClearedInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ ExactDecisionFanoutRetentionInvariant
    /\ [AsyncNext]_AsyncAllVars
    => ExactDecisionRequestAdmissionReadyPersistsOrOutcome(
         node, qc, archive, request, packet)
BY ExactDecisionRequestAdmissionCreatesExactOutcome,
   AsyncBracketStepRetainsExactDecisionRecord,
   AsyncBracketStepLeavesContext,
   AsyncNextPreservesExactDecisionFanoutRetention,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesFinalProgressWitnessClosure,
   GstAsyncStepIsMonotone, ExpandENABLED, IsaT(300)
   DEF ExactDecisionRequestAdmissionReadyPersistsOrOutcome,
       ExactDecisionRequestPacketAdmissionReady,
       ExactDecisionRequestIngressResidual,
       ExactDecisionRequestAdmissionOutcome,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressGoal,
       ExactDecisionRequestPacketOwned,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionRecord,
       ExactDecisionServeJobOwned,
       ExactDecisionServeOccurrenceOwned,
       CanAdmitIngressItem,
       PostGstAdmitHiddenPacket,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DueSourcePackets, OldestDueSourcePacket,
       IngressHasCoalescingOwner,
       IngressPacketPolicyRejected,
       IngressResourceSource,
       IngressLane, IngressLaneDepth,
       SequenceSet

(***************************************************************************
The exact retransmission handoff itself is closed.

`DirectRetransmitStep` publishes retryable items immediately when the Core
node is idle.  If the fixed production run-loop point first observes a busy
executor, it arms `AsyncRetransmitProgramCounter(node) = "DriveDue"`; the
subsequent `DeferredRetransmitStep` mirrors the unconditional
`drive_block_sync` call and therefore does not wait for reducer idleness.  An
exact active Decision request supplies a responsive body-holding alias in
`ActiveRequestItems(node)`, hence in `RetryableItems(node)`.  Either sending
branch therefore appends the packet for that exact retained alias.  This is an
action-local theorem: it deliberately does not claim that the retransmission
deadline becomes due or that the aggregate fair node runner eventually selects
the sending branch.
***************************************************************************)

THEOREM ExactDecisionRequestPeriodicEpisodeCannotResurrectDrainedOrdinal ==
  \A state, node, qc, drainedOrdinal:
    LET after ==
          AsyncCandidateLifecycleStateAfterServeIngressAdmission(state)
    IN /\ ExactDecisionRequestPacketEmissionResidual(node, qc)
       /\ node \in ValidatorIds
       /\ drainedOrdinal \in Nat
       /\ drainedOrdinal < state.candidateLifecycleNextOrdinal[node]
       /\ IsFiniteSet(
            AsyncFreshServeIngressAdmissionsForNodeThisStep(node))
       /\ AsyncRetransmitLifecycleConsumesFreshOrdinal(state, node)
         => /\ after.candidateLifecycleNextOrdinal[node]
                  = state.candidateLifecycleNextOrdinal[node]
                      + Cardinality(
                          AsyncFreshServeIngressAdmissionsForNodeThisStep(
                            node))
                      + 1
            /\ drainedOrdinal
                 < AsyncRetransmitLifecycleFreshOrdinalForStep(
                     state, node)
BY AsyncRetransmitFreshEpisodeAdvancesSharedHighWatermark,
   AsyncRetransmitFreshEpisodeCannotReuseDrainedPosition, Isa

ExactDecisionSendingRetransmitStep(node) ==
  \/ /\ DirectRetransmitStep(node)
        /\ NodeIdle(node)
  \/ DeferredRetransmitStep(node)

THEOREM ExactDecisionSendingRetransmitPublishesExactAlias ==
  \A node, qc:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionFanoutRetentionInvariant
    /\ ExactDecisionRequestPacketEmissionResidual(node, qc)
    /\ ExactDecisionSendingRetransmitStep(node)
    => ExactDecisionRequestPacketEmissionGoal(node, qc)'
BY ExactDecisionRequestHasResponsiveBodyHoldingAlias,
   ExactDecisionCertifiedRequestBindsHashAndArchiveRoute, IsaT(240)
   DEF ExactDecisionSendingRetransmitStep,
       ExactDecisionRequestPacketEmissionResidual,
       ExactDecisionRequestPacketEmissionGoal,
       ExactDecisionExecutableFrontier,
       ExactDecisionRequestPacketOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       RetryableItems, ActiveRequestItems,
       DirectRetransmitStep, DeferredRetransmitStep,
       SendNodeRetransmissions, NoSendItem,
       PacketsForItems, AsyncPacket, LeaveCausalQueues,
       AsyncDeferredVars, vars

(***************************************************************************
Non-circular request-emission temporal decomposition.

The clock-owner kernel covers exactly the prefix before either the absolute
retransmission deadline is due or a previously deferred retransmission tag is
owned by the node.  Proving it requires service of whichever overdue packet,
node, or I/O owner currently disables `AsyncTick`; weak fairness of
`AsyncTick` alone is insufficient while that owner remains.

The Runtime-prefix kernel starts only after that concrete retransmission
authority exists.  Its missing rank must preserve the active exact request
while Local/Ingress turns consume `RuntimeReachRank`, then order the finite
deferred/tag/FIFO prefix selected by `RuntimeStep`.  The only successful
non-stage exit is `ExactDecisionSendingRetransmitStep`, whose exact packet
handoff is proved above.  Neither kernel assumes the broad Decision-stage
service theorem or the residual convergence property which they discharge.
***************************************************************************)

(***************************************************************************
The retransmission clock frame used by the exact request corridor records
monotone time and preservation of both target-local clock owners.  Tick is
factored through a structural projection of `AsyncNonClockVars` and a scalar
natural-number successor leaf so callers do not unfold the full scheduler
tuple.
***************************************************************************)

ExactDecisionRequestClockFrame(node) ==
  /\ asyncNow' >= asyncNow
  /\ asyncRetransmitDeadlines[node]' =
       asyncRetransmitDeadlines[node]
  /\ ("RetransmitElapsed" \in asyncOutstandingTags[node]
        => "RetransmitElapsed" \in asyncOutstandingTags[node]')

THEOREM ExactDecisionRequestAsyncNonClockVarsStuttersClockPayload ==
  UNCHANGED AsyncNonClockVars
    => UNCHANGED <<asyncRetransmitDeadlines, asyncOutstandingTags>>
BY ONLY Isa DEF AsyncNonClockVars

THEOREM ExactDecisionRequestNaturalSuccessorIsMonotone ==
  \A now \in Nat: now + 1 >= now
BY ONLY SMT

THEOREM ExactDecisionRequestTypedTickAdvancesClock ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncTick
  => asyncNow' >= asyncNow
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
                AsyncTick
         PROVE asyncNow' >= asyncNow
    <2>1. asyncNow \in Nat
      BY <1>1
         DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant
    <2>2. asyncNow' = asyncNow + 1
      BY <1>1 DEF AsyncTick
    <2> QED BY <2>1, <2>2,
                 ExactDecisionRequestNaturalSuccessorIsMonotone
  <1> QED BY <1>1

THEOREM ExactDecisionRequestTickStuttersClockPayload ==
  AsyncTick
    => UNCHANGED <<asyncRetransmitDeadlines, asyncOutstandingTags>>
BY ExactDecisionRequestAsyncNonClockVarsStuttersClockPayload
   DEF AsyncTick

THEOREM ExactDecisionRequestTypedTickFramesClock ==
  \A node:
    /\ AsyncStrongTypeInvariant
    /\ AsyncTick
    => ExactDecisionRequestClockFrame(node)
PROOF
  <1>1. ASSUME NEW node,
                AsyncStrongTypeInvariant,
                AsyncTick
         PROVE ExactDecisionRequestClockFrame(node)
    <2>1. asyncNow' >= asyncNow
      BY <1>1, ExactDecisionRequestTypedTickAdvancesClock
    <2>2. UNCHANGED
             <<asyncRetransmitDeadlines, asyncOutstandingTags>>
      BY <1>1, ExactDecisionRequestTickStuttersClockPayload
    <2>3. asyncRetransmitDeadlines[node]' =
             asyncRetransmitDeadlines[node]
      BY <2>2, Isa
    <2>4. "RetransmitElapsed" \in asyncOutstandingTags[node]
             => "RetransmitElapsed" \in asyncOutstandingTags[node]'
      BY <2>2, Isa
    <2> QED BY <2>1, <2>3, <2>4
               DEF ExactDecisionRequestClockFrame
  <1> QED BY <1>1

\* Wall-clock expiry is deliberately not clock ownership.  `AsyncTick` may
\* make this predicate true, but Tick does not allocate a shared scheduler
\* ordinal.  The exact request therefore enters a separate acquisition
\* corridor before any Runtime service rank is available.
ExactDecisionRequestClockElapsedResidual(node, qc) ==
  /\ ExactDecisionRequestPacketEmissionResidual(node, qc)
  /\ asyncNow >= asyncRetransmitDeadlines[node]

THEOREM ExactDecisionRequestClockElapsedExcludesTimeoutClockDue ==
  \A node, qc:
    ExactDecisionRequestClockElapsedResidual(node, qc)
      => ~AsyncTimeoutClockDue(node)
BY Isa
   DEF ExactDecisionRequestClockElapsedResidual,
       ExactDecisionRequestPacketEmissionResidual,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource, ExactDecisionRecord,
       AsyncTimeoutClockDue, AsyncTimeoutClockDueIn,
       AsyncNodeHasDecisionIn

ExactDecisionRequestOwnedRetransmitEpisode(
    node, qc, ownerOrdinal) ==
  /\ ExactDecisionRequestPacketEmissionResidual(node, qc)
  /\ ownerOrdinal \in Nat \ {0}
  /\ AsyncRetransmitLifecycleOwned(node)
  /\ AsyncRetransmitLifecycleOrdinal(node) = ownerOrdinal

\* Compatibility/public kernel name.  The armed state now means one
\* immutable actor-local periodic lifecycle, never merely a raw deadline or
\* an unowned completion tag.
ExactDecisionRequestRetransmitArmedResidual(node, qc) ==
  \E ownerOrdinal \in Nat \ {0}:
    ExactDecisionRequestOwnedRetransmitEpisode(
      node, qc, ownerOrdinal)

ExactDecisionRequestSendingRetransmitReady(node, qc) ==
  /\ ExactDecisionRequestPacketEmissionResidual(node, qc)
  /\ ENABLED (
       PostGstRunNode(node)
         /\ ExactDecisionSendingRetransmitStep(node))

ExactDecisionRequestClockOwnerConvergenceProperty(specification) ==
  specification
    => \A node, qc:
         ExactDecisionRequestPacketEmissionResidual(node, qc)
           ~> (ExactDecisionRequestPacketEmissionGoal(node, qc)
                \/ ExactDecisionRequestRetransmitArmedResidual(
                     node, qc))

ExactDecisionRequestRuntimePrefixConvergenceProperty(specification) ==
  specification
    => \A node, qc:
         /\ ExactDecisionRequestRetransmitArmedResidual(node, qc)
              ~> (ExactDecisionRequestPacketEmissionGoal(node, qc)
                   \/ ExactDecisionRequestSendingRetransmitReady(
                        node, qc))
         /\ ExactDecisionRequestSendingRetransmitReady(node, qc)
              ~> ExactDecisionRequestPacketEmissionGoal(node, qc)

ExactDecisionRequestPacketEmissionResidualConvergenceProperty(
    specification) ==
  specification
    => \A node, qc:
         ExactDecisionRequestPacketEmissionResidual(node, qc)
           ~> ExactDecisionRequestPacketEmissionGoal(node, qc)

THEOREM ExactDecisionRequestEmissionKernelsDischargeResidual ==
  \A initialContext:
    /\ ExactDecisionRequestClockOwnerConvergenceProperty(
         AsyncSpecAt(initialContext))
    /\ ExactDecisionRequestRuntimePrefixConvergenceProperty(
         AsyncSpecAt(initialContext))
    => ExactDecisionRequestPacketEmissionResidualConvergenceProperty(
         AsyncSpecAt(initialContext))
BY PTL
   DEF ExactDecisionRequestClockOwnerConvergenceProperty,
       ExactDecisionRequestRuntimePrefixConvergenceProperty,
       ExactDecisionRequestPacketEmissionResidualConvergenceProperty

(***************************************************************************
Rigid clock-owner and Runtime corridor.

The old proof used `ReadyRunAuxRank` as if every FIFO/deferred occurrence
were a predecessor of the exact periodic owner.  That is false: later Local
work may raise that global rank while remaining strictly behind the admitted
periodic lifecycle.  This replacement freezes the receiver's physical cut
and the exact source records below one scheduler ceiling.  Candidate
successors, Serve reservations and producer continuations are charged by
their retained sources.  Only after that finite prefix is drained do we use
the target-specific periodic/tag/runner suffix.

Raw elapsed time has ceiling `AsyncNextCandidateLifecycleOrdinal(node)`.
Every fair Local or Ingress turn which precedes Runtime strictly lowers
`RuntimeReachRank`; the lower cell takes a fresh rigid snapshot and therefore
precharges the finite work admitted by that turn.  Once Runtime freezes the
clock, the immutable periodic ordinal becomes the ceiling.  Work admitted
after that point cannot enter the strict source prefix.
***************************************************************************)

ExactDecisionRequestRuntimeCandidateOriginsAt(
    node, schedulerCeiling, physicalCut) ==
  {record.origin:
     record \in
       {owned \in AsyncCandidateLifecycleAdmissions:
          /\ owned.node = node
          /\ owned.ordinal < schedulerCeiling
          /\ owned.sourcePhysicalOrdinal < physicalCut}}
    \cup
  {record.causalOrigin:
     record \in
       {owned \in asyncLeaderWireLifecycles:
          /\ owned.recipient = node
          /\ owned.schedulerOrdinal < schedulerCeiling
          /\ owned.physicalAdmissionOrdinal < physicalCut
          /\ AsyncLeaderWireLifecycleActive(owned)}}

ExactDecisionRequestRuntimeContinuationSourcesAt(
    node, schedulerCeiling, physicalCut) ==
  {AsyncCandidateLifecycleSource(record.origin, record.ordinal):
     record \in
       {owned \in AsyncCandidateLifecycleAdmissions:
          /\ owned.node = node
          /\ owned.ordinal < schedulerCeiling
          /\ owned.sourcePhysicalOrdinal < physicalCut}}
    \cup
  {AsyncCandidateLifecycleSource(
     record.causalOrigin, record.schedulerOrdinal):
     record \in
       {owned \in asyncLeaderWireLifecycles:
          /\ owned.recipient = node
          /\ owned.schedulerOrdinal < schedulerCeiling
          /\ owned.physicalAdmissionOrdinal < physicalCut
          /\ AsyncLeaderWireLifecycleActive(owned)}}

ExactDecisionRequestRuntimeServeSourcesAt(
    node, schedulerCeiling, physicalCut) ==
  {AsyncServeIngressSourceFor(admission):
     admission \in
       {owned \in asyncServeIngressAdmissions:
          /\ owned.node = node
          /\ owned.schedulerOrdinal < schedulerCeiling
          /\ owned.ordinal < physicalCut}}

ExactDecisionRequestRuntimeLeaderWireIdentitiesAt(
    node, schedulerCeiling, physicalCut) ==
  {AsyncLeaderWirePotentialOwnerIdentity(record):
     record \in
       {owned \in asyncLeaderWireLifecycles:
          /\ owned.recipient = node
          /\ owned.schedulerOrdinal < schedulerCeiling
          /\ owned.physicalAdmissionOrdinal < physicalCut
          /\ AsyncLeaderWireLifecycleActive(owned)}}

ExactDecisionRequestRuntimePrefixSnapshot(
    node, schedulerCeiling, physicalCut) ==
  [node |-> node,
   schedulerCeiling |-> schedulerCeiling,
   physicalCut |-> physicalCut,
   candidateOrigins |->
     ExactDecisionRequestRuntimeCandidateOriginsAt(
       node, schedulerCeiling, physicalCut),
   serveSources |->
     ExactDecisionRequestRuntimeServeSourcesAt(
       node, schedulerCeiling, physicalCut),
   continuationSources |->
     ExactDecisionRequestRuntimeContinuationSourcesAt(
       node, schedulerCeiling, physicalCut),
   leaderWireIdentities |->
     ExactDecisionRequestRuntimeLeaderWireIdentitiesAt(
       node, schedulerCeiling, physicalCut)]

ExactDecisionRequestRuntimePrefixSnapshotActive(snapshot, node) ==
  /\ snapshot.node = node
  /\ snapshot.schedulerCeiling \in Nat \ {0}
  /\ snapshot.physicalCut \in Nat \ {0}
  /\ snapshot.schedulerCeiling
       <= AsyncNextCandidateLifecycleOrdinal(node)
  /\ snapshot.physicalCut
       <= AsyncNextIngressPhysicalOrdinal(node)
  /\ IsFiniteSet(snapshot.candidateOrigins)
  /\ IsFiniteSet(snapshot.serveSources)
  /\ IsFiniteSet(snapshot.continuationSources)
  /\ IsFiniteSet(snapshot.leaderWireIdentities)
  /\ snapshot.candidateOrigins
       \subseteq AsyncCandidateCausalOriginSet
  /\ snapshot.serveSources \subseteq AsyncServeIngressSourceSet
  /\ snapshot.continuationSources
       \subseteq AsyncCandidateLifecycleSourceSet
  /\ snapshot.leaderWireIdentities
       \subseteq AsyncLeaderWirePotentialOwnerIdentitySet
  /\ \A origin \in snapshot.candidateOrigins:
       origin.target = node
  /\ \A source \in snapshot.serveSources:
       /\ source.ordinal < snapshot.physicalCut
       /\ source.schedulerOrdinal < snapshot.schedulerCeiling
       /\ \/ source.lifecycleOrdinal = 0
          \/ source.lifecycleOrdinal
               < asyncNextServeAdmissionOrdinal[node]
  /\ \A source \in snapshot.continuationSources:
       /\ source.origin \in snapshot.candidateOrigins
       /\ source.ordinal < snapshot.schedulerCeiling
       /\ \/ \E admission \in AsyncCandidateLifecycleAdmissions:
                  /\ admission.node = node
                  /\ admission.origin = source.origin
                  /\ admission.ordinal = source.ordinal
                  /\ admission.sourcePhysicalOrdinal
                       < snapshot.physicalCut
          \/ \E owner \in snapshot.leaderWireIdentities:
               \E record \in asyncLeaderWireLifecycles:
                 /\ AsyncLeaderWirePotentialOwnerIdentity(record) = owner
                 /\ record.recipient = node
                 /\ record.causalOrigin = source.origin
                 /\ record.schedulerOrdinal = source.ordinal
                 /\ record.physicalAdmissionOrdinal
                      < snapshot.physicalCut
  /\ ExactDecisionRequestRuntimeCandidateOriginsAt(
       node, snapshot.schedulerCeiling, snapshot.physicalCut)
       \subseteq snapshot.candidateOrigins
  /\ ExactDecisionRequestRuntimeServeSourcesAt(
       node, snapshot.schedulerCeiling, snapshot.physicalCut)
       \subseteq snapshot.serveSources
  /\ ExactDecisionRequestRuntimeContinuationSourcesAt(
       node, snapshot.schedulerCeiling, snapshot.physicalCut)
       \subseteq snapshot.continuationSources
  /\ ExactDecisionRequestRuntimeLeaderWireIdentitiesAt(
       node, snapshot.schedulerCeiling, snapshot.physicalCut)
       \subseteq snapshot.leaderWireIdentities
  /\ \A record \in asyncLeaderWireLifecycles:
       /\ record.recipient = node
       /\ AsyncLeaderWireLifecycleActive(record)
       /\ record.schedulerOrdinal < snapshot.schedulerCeiling
       /\ record.physicalAdmissionOrdinal < snapshot.physicalCut
         => /\ AsyncLeaderWirePotentialOwnerIdentity(record)
                  \in snapshot.leaderWireIdentities
            /\ record.causalOrigin \in snapshot.candidateOrigins
            /\ AsyncCandidateLifecycleSource(
                 record.causalOrigin, record.schedulerOrdinal)
                  \in snapshot.continuationSources
  /\ \A owner \in snapshot.leaderWireIdentities:
       /\ owner.schedulerOrdinal < snapshot.schedulerCeiling
       /\ \E record \in asyncLeaderWireLifecycles:
            /\ AsyncLeaderWirePotentialOwnerIdentity(record) = owner
            /\ record.recipient = node
            /\ record.physicalAdmissionOrdinal < snapshot.physicalCut
       /\ \A record \in asyncLeaderWireLifecycles:
            AsyncLeaderWirePotentialOwnerIdentity(record) = owner
              => /\ record.recipient = node
                 /\ record.physicalAdmissionOrdinal < snapshot.physicalCut
                 /\ ~AsyncLeaderWireLifecycleDormant(record)

\* A Decision may be recorded one transition after an already-frozen timeout
\* episode.  That owner is older than the raw retransmit cut and is charged as
\* a separate outer cell.  If it transfers a BeginTimeout candidate while
\* clearing, the immutable continuation source was already captured below the
\* same ceiling, so the lexicographic tail may grow only while this outer cell
\* strictly descends.
ExactDecisionRequestRuntimeOlderTimeoutStage(snapshot) ==
  IF /\ AsyncTimeoutLifecycleOwned(snapshot.node)
     /\ AsyncTimeoutLifecycleOrdinal(snapshot.node)
          < snapshot.schedulerCeiling
  THEN 1
  ELSE 0

ExactDecisionRequestRuntimeFrozenSourceRank(snapshot) ==
  AsyncCandidateProducerContinuationFrozenSourcePrefixRank(
    snapshot.node,
    snapshot.candidateOrigins,
    snapshot.serveSources,
    snapshot.continuationSources)

ExactDecisionRequestRuntimeFrozenIngressStage(snapshot) ==
  AsyncFrozenLeaderWireBarrierStageBudget(
    snapshot.node, snapshot.schedulerCeiling - 1,
    snapshot.physicalCut, "Logical")

ExactDecisionRequestRuntimeFrozenIngressDependencyRank(snapshot) ==
  AsyncFrozenLeaderWireIngressDependencyRank(
    snapshot.node, snapshot.schedulerCeiling - 1,
    snapshot.physicalCut, "Logical")

ExactDecisionRequestRuntimeFrozenPrefixRank(snapshot) ==
  <<ExactDecisionRequestRuntimeOlderTimeoutStage(snapshot),
    <<ExactDecisionRequestRuntimeFrozenIngressStage(snapshot),
      <<ExactDecisionRequestRuntimeFrozenSourceRank(snapshot),
        ExactDecisionRequestRuntimeFrozenIngressDependencyRank(snapshot)>>>>>>

ExactDecisionRequestRuntimeFrozenPrefixTailCarrier ==
  AsyncCandidateProducerContinuationFrozenPrefixRankCarrier
    \X AsyncFrozenLeaderWireIngressDependencyRankCarrier

ExactDecisionRequestRuntimeFrozenBarrierCarrier ==
  Nat \X ExactDecisionRequestRuntimeFrozenPrefixTailCarrier

ExactDecisionRequestRuntimeFrozenPrefixCarrier ==
  (0..1) \X ExactDecisionRequestRuntimeFrozenBarrierCarrier

ExactDecisionRequestRuntimeFrozenPrefixTailOrdering ==
  LexPairOrdering(
    AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
    AsyncFrozenLeaderWireIngressDependencyRankOrdering,
    AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
    AsyncFrozenLeaderWireIngressDependencyRankCarrier)

ExactDecisionRequestRuntimeFrozenBarrierOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    ExactDecisionRequestRuntimeFrozenPrefixTailOrdering,
    Nat, ExactDecisionRequestRuntimeFrozenPrefixTailCarrier)

ExactDecisionRequestRuntimeFrozenPrefixOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    ExactDecisionRequestRuntimeFrozenBarrierOrdering,
    0..1, ExactDecisionRequestRuntimeFrozenBarrierCarrier)

ExactDecisionRequestRuntimeFrozenSourceBottom == <<0, <<0, 0>>>>

ExactDecisionRequestRuntimeFrozenPrefixDrained(snapshot) ==
  /\ ExactDecisionRequestRuntimeOlderTimeoutStage(snapshot) = 0
  /\ ExactDecisionRequestRuntimeFrozenIngressStage(snapshot) = 0
  /\ ExactDecisionRequestRuntimeFrozenSourceRank(snapshot)
       = ExactDecisionRequestRuntimeFrozenSourceBottom
  /\ AsyncFrozenLeaderWireBarrierRecords(
       snapshot.node, snapshot.schedulerCeiling - 1,
       snapshot.physicalCut, "Logical") = {}
  /\ AsyncFrozenOrdinaryIngressBarrierRecords(
       snapshot.node, snapshot.schedulerCeiling - 1,
       snapshot.physicalCut, "Logical") = {}

THEOREM ExactDecisionRequestRuntimeFrozenPrefixOrderingIsWellFounded ==
  IsWellFoundedOn(
    ExactDecisionRequestRuntimeFrozenPrefixOrdering,
    ExactDecisionRequestRuntimeFrozenPrefixCarrier)
BY CandidateProducerContinuationFrozenPrefixRankOrderingIsWellFounded,
   AsyncFrozenLeaderWireIngressDependencyOrderingIsWellFounded,
   WFLexPairOrdering
   DEF ExactDecisionRequestRuntimeFrozenPrefixOrdering,
       ExactDecisionRequestRuntimeFrozenPrefixCarrier,
       ExactDecisionRequestRuntimeFrozenBarrierOrdering,
       ExactDecisionRequestRuntimeFrozenBarrierCarrier,
       ExactDecisionRequestRuntimeFrozenPrefixTailOrdering,
       ExactDecisionRequestRuntimeFrozenPrefixTailCarrier

THEOREM ExactDecisionRequestRuntimePrefixSnapshotIsFinite ==
  \A node \in ValidatorIds,
     schedulerCeiling \in Nat \ {0},
     physicalCut \in Nat \ {0}:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ schedulerCeiling
         <= AsyncNextCandidateLifecycleOrdinal(node)
    /\ physicalCut <= AsyncNextIngressPhysicalOrdinal(node)
      => LET snapshot ==
               ExactDecisionRequestRuntimePrefixSnapshot(
                 node, schedulerCeiling, physicalCut)
         IN /\ ExactDecisionRequestRuntimePrefixSnapshotActive(
                  snapshot, node)
            /\ ExactDecisionRequestRuntimeFrozenPrefixRank(snapshot)
                 \in ExactDecisionRequestRuntimeFrozenPrefixCarrier
BY CandidateProducerContinuationFrozenCandidateCarrierHasConfiguredBound,
   AsyncFrozenLeaderWireBarrierRankIsFinite,
   RuntimeValidatorIdsAreFinite,
   FS_Image, FS_Union, FS_Subset, FS_Intersection,
   FS_CardinalityType, IsaT(1200)
   DEF ExactDecisionRequestRuntimePrefixSnapshot,
       ExactDecisionRequestRuntimePrefixSnapshotActive,
       ExactDecisionRequestRuntimeCandidateOriginsAt,
       ExactDecisionRequestRuntimeContinuationSourcesAt,
       ExactDecisionRequestRuntimeServeSourcesAt,
       ExactDecisionRequestRuntimeLeaderWireIdentitiesAt,
       ExactDecisionRequestRuntimeFrozenPrefixRank,
       ExactDecisionRequestRuntimeOlderTimeoutStage,
       ExactDecisionRequestRuntimeFrozenIngressStage,
       ExactDecisionRequestRuntimeFrozenSourceRank,
       ExactDecisionRequestRuntimeFrozenIngressDependencyRank,
       ExactDecisionRequestRuntimeFrozenPrefixCarrier,
       ExactDecisionRequestRuntimeFrozenBarrierCarrier,
       ExactDecisionRequestRuntimeFrozenPrefixTailCarrier,
       AsyncCandidateProducerContinuationFrozenSourcePrefixRank,
       AsyncCandidateProducerContinuationFrozenSourceProducerBudget,
       AsyncCandidateProducerContinuationFrozenSourceProducerTokens,
       AsyncFrozenServeWorkBudget, AsyncFrozenServeWorkTokens,
       AsyncFrozenServeReachDebt,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
       AsyncCausalEpisodeStructuralRankCarrier,
       AsyncCausalEpisodeServeRankCarrier

THEOREM ExactDecisionRequestRuntimeActiveSnapshotRankIsInCarrier ==
  \A snapshot, node:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ ExactDecisionRequestRuntimePrefixSnapshotActive(snapshot, node)
      => ExactDecisionRequestRuntimeFrozenPrefixRank(snapshot)
           \in ExactDecisionRequestRuntimeFrozenPrefixCarrier
BY CandidateProducerContinuationFrozenCandidateCarrierHasConfiguredBound,
   AsyncFrozenLeaderWireBarrierRankIsFinite,
   FS_CardinalityType, FS_Subset, IsaT(1200)
   DEF ExactDecisionRequestRuntimePrefixSnapshotActive,
       ExactDecisionRequestRuntimeFrozenPrefixRank,
       ExactDecisionRequestRuntimeOlderTimeoutStage,
       ExactDecisionRequestRuntimeFrozenIngressStage,
       ExactDecisionRequestRuntimeFrozenSourceRank,
       ExactDecisionRequestRuntimeFrozenIngressDependencyRank,
       ExactDecisionRequestRuntimeFrozenPrefixCarrier,
       ExactDecisionRequestRuntimeFrozenBarrierCarrier,
       ExactDecisionRequestRuntimeFrozenPrefixTailCarrier,
       AsyncCandidateProducerContinuationFrozenSourcePrefixRank,
       AsyncCandidateProducerContinuationFrozenSourceProducerBudget,
       AsyncCandidateProducerContinuationFrozenSourceProducerTokens,
       AsyncFrozenServeWorkBudget, AsyncFrozenServeWorkTokens,
       AsyncFrozenServeReachDebt,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
       AsyncCausalEpisodeStructuralRankCarrier,
       AsyncCausalEpisodeServeRankCarrier

ExactDecisionRequestClockPrefixKinds == {"Acquire", "Owned"}

ExactDecisionRequestClockPrefixResidual(
    kind, node, qc, ownerOrdinal) ==
  CASE kind = "Acquire" ->
         /\ ownerOrdinal = 0
         /\ ExactDecisionRequestClockElapsedResidual(node, qc)
         /\ ~ExactDecisionRequestRetransmitArmedResidual(node, qc)
    [] kind = "Owned" ->
         ExactDecisionRequestOwnedRetransmitEpisode(
           node, qc, ownerOrdinal)
    [] OTHER -> FALSE

ExactDecisionRequestClockPrefixEndpoint(
    kind, snapshot, node, qc) ==
  \/ ExactDecisionRequestPacketEmissionGoal(node, qc)
  \/ /\ kind = "Acquire"
        /\ ExactDecisionRequestRetransmitArmedResidual(node, qc)
  \/ ExactDecisionRequestRuntimeFrozenPrefixDrained(snapshot)

ExactDecisionRequestClockPrefixSnapshotBinding(
    kind, snapshot, node, ownerOrdinal) ==
  CASE kind = "Acquire" ->
         /\ ownerOrdinal = 0
         /\ snapshot.schedulerCeiling
              <= AsyncNextCandidateLifecycleOrdinal(node)
    [] kind = "Owned" ->
         /\ snapshot.schedulerCeiling = ownerOrdinal
         /\ snapshot.physicalCut
              = AsyncRetransmitLifecyclePhysicalCut(node)
    [] OTHER -> FALSE

ExactDecisionRequestClockPrefixAtRank(
    kind, snapshot, node, qc, ownerOrdinal, rank) ==
  /\ kind \in ExactDecisionRequestClockPrefixKinds
  /\ ExactDecisionRequestClockPrefixResidual(
       kind, node, qc, ownerOrdinal)
  /\ ExactDecisionRequestRuntimePrefixSnapshotActive(snapshot, node)
  /\ ExactDecisionRequestClockPrefixSnapshotBinding(
       kind, snapshot, node, ownerOrdinal)
  /\ ~ExactDecisionRequestClockPrefixEndpoint(
       kind, snapshot, node, qc)
  /\ rank \in ExactDecisionRequestRuntimeFrozenPrefixCarrier
  /\ ExactDecisionRequestRuntimeFrozenPrefixRank(snapshot) = rank

ExactDecisionRequestClockPrefixRankGoal(
    kind, snapshot, node, qc, ownerOrdinal, rank) ==
  \/ ExactDecisionRequestClockPrefixEndpoint(
       kind, snapshot, node, qc)
  \/ \E lowerRank \in
       SetLessThan(
         rank,
         ExactDecisionRequestRuntimeFrozenPrefixOrdering,
         ExactDecisionRequestRuntimeFrozenPrefixCarrier):
       ExactDecisionRequestClockPrefixAtRank(
         kind, snapshot, node, qc, ownerOrdinal, lowerRank)

ExactDecisionRequestClockPrefixContinuationAtRank(
    kind, snapshot, node, qc, ownerOrdinal, rank) ==
  /\ ExactDecisionRequestClockPrefixAtRank(
       kind, snapshot, node, qc, ownerOrdinal, rank)
  /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)

ExactDecisionRequestClockPrefixResolvedAtRank(
    kind, snapshot, node, qc, ownerOrdinal, rank) ==
  /\ ExactDecisionRequestClockPrefixAtRank(
       kind, snapshot, node, qc, ownerOrdinal, rank)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(node)

ExactDecisionRequestClockPrefixIoOwnerRequired(snapshot) ==
  AsyncFrozenServeIoOwnerRequired(
    snapshot.node, snapshot.serveSources)

ExactDecisionRequestClockPrefixFairOwner(snapshot) ==
  IF ExactDecisionRequestClockPrefixIoOwnerRequired(snapshot)
  THEN "IoWorker"
  ELSE "Runner"

ExactDecisionRequestClockPrefixFairAction(snapshot) ==
  AsyncCausalEpisodeFairAction(
    snapshot.node,
    ExactDecisionRequestClockPrefixFairOwner(snapshot))

(***************************************************************************
Generic seam 1: a rigid source-record prefix cannot replenish.  Successor
replacement consumes the precharged causal weight, Serve retries retain the
same source, and post-cut ingress/leader-wire work is physically behind this
cell.  The theorem is action-local and advertises no producer action as
progress by itself.
***************************************************************************)
THEOREM ExactDecisionRequestClockPrefixStepIsDescentOrFrame ==
  \A kind \in ExactDecisionRequestClockPrefixKinds:
    \A snapshot, node, qc, ownerOrdinal:
      \A rank \in ExactDecisionRequestRuntimeFrozenPrefixCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ ExactDecisionRequestClockPrefixAtRank(
         kind, snapshot, node, qc, ownerOrdinal, rank)
    /\ [AsyncNext]_AsyncAllVars
      => \/ ExactDecisionRequestClockPrefixRankGoal(
              kind, snapshot, node, qc, ownerOrdinal, rank)'
         \/ ExactDecisionRequestClockPrefixAtRank(
              kind, snapshot, node, qc, ownerOrdinal, rank)'
BY CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight,
   CandidateProducerContinuationDormantLocalReplayReplacementConsumesFrozenCausalCharge,
   CandidateProducerContinuationExactLocalReplayReplacesFrozenCharge,
   CandidateProducerContinuationFrozenLeaderWireChargeCannotAppearAtGst,
   CandidateProducerContinuationFrozenOrdinaryIngressChargeCannotAppearAtGst,
   CandidateProducerContinuationActionInertDormantHasZeroFrozenStage,
   CandidateProducerContinuationPostCutAdmissionCannotEnterFrozenPrefix,
   CandidateProducerContinuationPostCutOrdinaryAdmissionCannotEnterFrozenPrefix,
   CandidateProducerContinuationDropPolicyRejectedIsFrozenPhysicalPrefixFrame,
   CandidateProducerContinuationPreCutIngressToRuntimeConsumesBarrierStage,
   CandidateProducerContinuationPreCutOrdinaryIngressConsumesBarrierStage,
   AsyncFrozenServeSourceCannotResurrectAtGst,
   AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   AsyncTimeoutLifecycleOrdinalPersistsUntilEndpoint,
   AsyncTimeoutLifecycleOrdinalClearsOnlyAtEndpoint,
   AsyncTimeoutLifecycleNewOwnershipUsesRecordedOrFreshOrdinal,
   AsyncRetransmitLifecycleOwnerAndPhysicalCutPersistUntilEndpoint,
   AsyncRetransmitLifecycleOwnerAndPhysicalCutClearAtEndpoint,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   ExactDecisionBodyHoldingAliasPersistsOrFrontier,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   AsyncNextPreservesExactDecisionFanoutRetention,
   IsaT(3600)
   DEF ExactDecisionRequestClockPrefixRankGoal,
       ExactDecisionRequestClockPrefixAtRank,
       ExactDecisionRequestClockPrefixEndpoint,
       ExactDecisionRequestClockPrefixResidual,
       ExactDecisionRequestClockPrefixSnapshotBinding,
       ExactDecisionRequestRuntimePrefixSnapshotActive,
       ExactDecisionRequestRuntimeCandidateOriginsAt,
       ExactDecisionRequestRuntimeContinuationSourcesAt,
       ExactDecisionRequestRuntimeServeSourcesAt,
       ExactDecisionRequestRuntimeLeaderWireIdentitiesAt,
       ExactDecisionRequestRuntimeFrozenPrefixDrained,
       ExactDecisionRequestRuntimeFrozenPrefixRank,
       ExactDecisionRequestRuntimeOlderTimeoutStage,
       ExactDecisionRequestRuntimeFrozenIngressStage,
       ExactDecisionRequestRuntimeFrozenSourceRank,
       ExactDecisionRequestRuntimeFrozenIngressDependencyRank,
       ExactDecisionRequestRuntimeFrozenPrefixOrdering,
       ExactDecisionRequestRuntimeFrozenPrefixCarrier,
       ExactDecisionRequestRuntimeFrozenPrefixTailOrdering,
       ExactDecisionRequestRuntimeFrozenPrefixTailCarrier,
       AsyncCandidateProducerContinuationFrozenSourcePrefixRank,
       AsyncCandidateProducerContinuationFrozenSourceProducerBudget,
       AsyncCandidateProducerContinuationFrozenSourceProducerTokens,
       AsyncCandidateProducerContinuationFrozenSourceCandidateTokens,
       AsyncCandidateProducerContinuationFrozenSourceStatusTokens,
       AsyncCandidateProducerContinuationFrozenSourceRecords,
       AsyncFrozenServeWorkBudget, AsyncFrozenServeWorkTokens,
       AsyncFrozenServeReachDebt,
       AsyncFrozenLeaderWireBarrierStageBudget,
       AsyncFrozenLeaderWireBarrierRecords,
       AsyncFrozenOrdinaryIngressBarrierRecords,
       AsyncFrozenLeaderWireIngressDependencyRank,
       AsyncTimeoutLifecycleOwned,
       AsyncTimeoutLifecycleOrdinal,
       AsyncTimeoutLifecycleOrigin,
       AsyncTimeoutLifecycleResetThisStep,
       AsyncTimeoutLifecycleTransfersThisStep,
       AsyncRetransmitLifecycleOwned,
       AsyncRetransmitLifecycleOrdinal,
       AsyncRetransmitLifecyclePhysicalCut,
       AsyncAllVars, SetLessThan, OpToRel

(***************************************************************************
Generic seam 2: an unresolved producer continuation is a separate fair,
finite episode.  Resolution may return to this exact rank cell, strictly
lower it, or reach the exact endpoint; it is never counted as periodic
progress.
***************************************************************************)
ExactDecisionRequestClockPrefixContinuationClosureProperty(
    specification, initialContext) ==
  specification
    => \A kind \in ExactDecisionRequestClockPrefixKinds:
         \A snapshot, qc, ownerOrdinal:
           \A node \in AsyncVotersAt(initialContext):
             \A rank \in ExactDecisionRequestRuntimeFrozenPrefixCarrier:
         ExactDecisionRequestClockPrefixContinuationAtRank(
           kind, snapshot, node, qc, ownerOrdinal, rank)
           ~> \/ ExactDecisionRequestClockPrefixRankGoal(
                   kind, snapshot, node, qc, ownerOrdinal, rank)
              \/ ExactDecisionRequestClockPrefixResolvedAtRank(
                   kind, snapshot, node, qc, ownerOrdinal, rank)

THEOREM AsyncSpecClosesExactDecisionRequestClockPrefixContinuation ==
  \A initialContext:
    ExactDecisionRequestClockPrefixContinuationClosureProperty(
      AsyncSpecAt(initialContext), initialContext)
BY AsyncSpecProvidesVoterCandidateProducerContinuationResolutionClosure,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   ExactDecisionAsyncSpecAlwaysCandidateTombstones,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   ExactDecisionRequestClockPrefixStepIsDescentOrFrame,
   PTL, IsaT(1800)
   DEF ExactDecisionRequestClockPrefixContinuationClosureProperty,
       ExactDecisionRequestClockPrefixContinuationAtRank,
       ExactDecisionRequestClockPrefixResolvedAtRank,
       ExactDecisionRequestClockPrefixRankGoal,
       AsyncVoterCandidateProducerContinuationEpisodePending,
       AsyncVoterCandidateProducerContinuationResolutionClosureProperty

THEOREM ExactDecisionRequestClockPrefixResolvedOwnerIsEnabled ==
  \A kind \in ExactDecisionRequestClockPrefixKinds:
    \A snapshot, node, qc, ownerOrdinal:
      \A rank \in ExactDecisionRequestRuntimeFrozenPrefixCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ ExactDecisionRequestClockPrefixResolvedAtRank(
         kind, snapshot, node, qc, ownerOrdinal, rank)
      => /\ ExactDecisionRequestClockPrefixFairOwner(snapshot)
               \in AsyncCausalEpisodeFairOwnerKinds
         /\ ENABLED
              <<ExactDecisionRequestClockPrefixFairAction(
                  snapshot)>>_AsyncAllVars
BY QueuedIoEnablesPostGstService,
   QueuedIoServiceIsNonstuttering,
   ResponsiveUnappliedRunNodeIsEnabled,
   EnabledRunNodeLiftsPostGst,
   ExpandENABLED, ENABLEDaxioms, IsaT(1800)
   DEF ExactDecisionRequestClockPrefixResolvedAtRank,
       ExactDecisionRequestClockPrefixAtRank,
       ExactDecisionRequestClockPrefixResidual,
       ExactDecisionRequestClockPrefixFairAction,
       ExactDecisionRequestClockPrefixFairOwner,
       ExactDecisionRequestClockPrefixIoOwnerRequired,
       AsyncFrozenServeIoOwnerRequired,
       AsyncCausalEpisodeFairOwnerKinds,
       AsyncCausalEpisodeFairAction,
       ExactDecisionRequestClockElapsedResidual,
       ExactDecisionRequestPacketEmissionResidual,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       AsyncAllVars

THEOREM ExactDecisionRequestClockPrefixResolvedOwnerConsumesRankCell ==
  \A kind \in ExactDecisionRequestClockPrefixKinds:
    \A snapshot, node, qc, ownerOrdinal:
      \A rank \in ExactDecisionRequestRuntimeFrozenPrefixCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ ExactDecisionRequestClockPrefixResolvedAtRank(
         kind, snapshot, node, qc, ownerOrdinal, rank)
    /\ <<ExactDecisionRequestClockPrefixFairAction(
            snapshot)>>_AsyncAllVars
      => ExactDecisionRequestClockPrefixRankGoal(
           kind, snapshot, node, qc, ownerOrdinal, rank)'
BY ExactDecisionRequestClockPrefixStepIsDescentOrFrame,
   CandidateProducerContinuationFrozenSourceFairResolutionStrictlyDescends,
   ExhaustedIngressStepDecreasesDrainableIngressTurnReach,
   LocalStepDecreasesDrainableIngressTurnReach,
   SerializedLocalPredecessorDecreasesDrainableIngressTurnReach,
   RuntimeStepDecreasesDrainableIngressTurnReach,
   OlderRuntimeInterleaveDecreasesDrainableIngressTurnReach,
   ServiceIoWorkerDropsQueueDepth,
   ExactDecisionSendingRetransmitPublishesExactAlias,
   IsaT(3600)
   DEF ExactDecisionRequestClockPrefixResolvedAtRank,
       ExactDecisionRequestClockPrefixRankGoal,
       ExactDecisionRequestClockPrefixFairAction,
       ExactDecisionRequestClockPrefixFairOwner,
       AsyncCausalEpisodeFairAction,
       AsyncAllVars

THEOREM ExactDecisionRequestClockPrefixFairOwnerUsesExistingFairness ==
  \A initialContext, snapshot:
    (/\ snapshot.node \in AsyncVotersAt(initialContext)
     /\ ExactDecisionRequestClockPrefixFairOwner(snapshot)
          \in AsyncCausalEpisodeFairOwnerKinds)
      => (AsyncSpecAt(initialContext)
            => WF_AsyncAllVars(
                 ExactDecisionRequestClockPrefixFairAction(snapshot)))
BY Isa, PTL
   DEF ExactDecisionRequestClockPrefixFairAction,
       ExactDecisionRequestClockPrefixFairOwner,
       AsyncCausalEpisodeFairOwnerKinds,
       AsyncCausalEpisodeFairAction,
       AsyncSpecAt, AsyncFairnessAt

THEOREM ExactDecisionRequestClockPrefixResolvedFairOwnerIsStable ==
  \A kind \in ExactDecisionRequestClockPrefixKinds:
    \A snapshot, node, qc, ownerOrdinal:
      \A rank \in ExactDecisionRequestRuntimeFrozenPrefixCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ ExactDecisionRequestClockPrefixResolvedAtRank(
         kind, snapshot, node, qc, ownerOrdinal, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ ExactDecisionRequestClockPrefixResolvedAtRank(
         kind, snapshot, node, qc, ownerOrdinal, rank)'
      => ExactDecisionRequestClockPrefixFairOwner(snapshot)'
           = ExactDecisionRequestClockPrefixFairOwner(snapshot)
BY AsyncFrozenServeSourceCannotResurrectAtGst,
   AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal,
   AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   IsaT(2400)
   DEF ExactDecisionRequestClockPrefixResolvedAtRank,
       ExactDecisionRequestClockPrefixAtRank,
       ExactDecisionRequestClockPrefixFairOwner,
       ExactDecisionRequestClockPrefixIoOwnerRequired,
       ExactDecisionRequestRuntimePrefixSnapshotActive,
       ExactDecisionRequestRuntimeFrozenPrefixRank,
       ExactDecisionRequestRuntimeFrozenSourceRank,
       AsyncCandidateProducerContinuationFrozenSourcePrefixRank,
       AsyncFrozenServeIoOwnerRequired,
       AsyncFrozenServeLifecycleSources,
       AsyncFrozenServeSourceOwned,
       AsyncAllVars

(***************************************************************************
Generic seam 3: after continuation resolution, the concrete Runner/I/O owner
is stable in an equal rank cell and its reviewed weak fairness closes that
cell.  `AsyncVoterRuntimeReadyHasNoNonRunnerContinuationInsertion` is used at
the boundary so an arbitrary non-runner step cannot silently recreate an
absolute-priority continuation ahead of the frozen target.
***************************************************************************)
ExactDecisionRequestClockPrefixResolvedRankStepProperty(
    specification, initialContext) ==
  specification
    => \A kind \in ExactDecisionRequestClockPrefixKinds:
         \A snapshot, qc, ownerOrdinal:
           \A node \in AsyncVotersAt(initialContext):
             \A rank \in ExactDecisionRequestRuntimeFrozenPrefixCarrier:
         ExactDecisionRequestClockPrefixResolvedAtRank(
           kind, snapshot, node, qc, ownerOrdinal, rank)
           ~> ExactDecisionRequestClockPrefixRankGoal(
                kind, snapshot, node, qc, ownerOrdinal, rank)

THEOREM AsyncSpecClosesExactDecisionRequestClockPrefixResolvedRankStep ==
  \A initialContext:
    ExactDecisionRequestClockPrefixResolvedRankStepProperty(
      AsyncSpecAt(initialContext), initialContext)
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   ExactDecisionAsyncSpecAlwaysCandidateTombstones,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   ExactDecisionRequestClockPrefixStepIsDescentOrFrame,
   ExactDecisionRequestClockPrefixResolvedOwnerIsEnabled,
   ExactDecisionRequestClockPrefixResolvedOwnerConsumesRankCell,
   ExactDecisionRequestClockPrefixFairOwnerUsesExistingFairness,
   ExactDecisionRequestClockPrefixResolvedFairOwnerIsStable,
   WF1, PTL, IsaT(1800)
   DEF ExactDecisionRequestClockPrefixResolvedRankStepProperty,
       ExactDecisionRequestClockPrefixResolvedAtRank,
       ExactDecisionRequestClockPrefixRankGoal

ExactDecisionRequestClockPrefixRankStepProperty(
    specification, initialContext) ==
  specification
    => \A kind \in ExactDecisionRequestClockPrefixKinds:
         \A snapshot, qc, ownerOrdinal:
           \A node \in AsyncVotersAt(initialContext):
             \A rank \in ExactDecisionRequestRuntimeFrozenPrefixCarrier:
         ExactDecisionRequestClockPrefixAtRank(
           kind, snapshot, node, qc, ownerOrdinal, rank)
           ~> ExactDecisionRequestClockPrefixRankGoal(
                kind, snapshot, node, qc, ownerOrdinal, rank)

THEOREM AsyncSpecClosesExactDecisionRequestClockPrefixRankStep ==
  \A initialContext:
    ExactDecisionRequestClockPrefixRankStepProperty(
      AsyncSpecAt(initialContext), initialContext)
BY AsyncSpecClosesExactDecisionRequestClockPrefixContinuation,
   AsyncSpecClosesExactDecisionRequestClockPrefixResolvedRankStep,
   PTL, IsaT(900)
   DEF ExactDecisionRequestClockPrefixRankStepProperty,
       ExactDecisionRequestClockPrefixContinuationClosureProperty,
       ExactDecisionRequestClockPrefixResolvedRankStepProperty,
       ExactDecisionRequestClockPrefixContinuationAtRank,
       ExactDecisionRequestClockPrefixResolvedAtRank

ExactDecisionRequestClockPrefixClosureProperty(
    specification, initialContext) ==
  specification
    => \A kind \in ExactDecisionRequestClockPrefixKinds:
         \A snapshot, qc, ownerOrdinal:
           \A node \in AsyncVotersAt(initialContext):
         /\ ExactDecisionRequestClockPrefixResidual(
              kind, node, qc, ownerOrdinal)
         /\ ExactDecisionRequestRuntimePrefixSnapshotActive(
              snapshot, node)
         /\ ExactDecisionRequestClockPrefixSnapshotBinding(
              kind, snapshot, node, ownerOrdinal)
         /\ ~ExactDecisionRequestClockPrefixEndpoint(
              kind, snapshot, node, qc)
           ~> ExactDecisionRequestClockPrefixEndpoint(
                kind, snapshot, node, qc)

THEOREM AsyncSpecClosesExactDecisionRequestClockPrefix ==
  \A initialContext:
    ExactDecisionRequestClockPrefixClosureProperty(
      AsyncSpecAt(initialContext), initialContext)
BY AsyncSpecClosesExactDecisionRequestClockPrefixRankStep,
   AsyncSpecAlwaysStrongTypeInvariant,
   ExactDecisionAsyncSpecAlwaysCandidateTombstones,
   ExactDecisionRequestRuntimeActiveSnapshotRankIsInCarrier,
   ExactDecisionRequestRuntimeFrozenPrefixOrderingIsWellFounded,
   WellFoundedLeadsTo, PTL, IsaT(900)
   DEF ExactDecisionRequestClockPrefixClosureProperty,
       ExactDecisionRequestClockPrefixRankStepProperty,
       ExactDecisionRequestClockPrefixAtRank,
       ExactDecisionRequestClockPrefixRankGoal,
       ExactDecisionRequestClockPrefixSnapshotBinding

(***************************************************************************
Elapsed -> immutable owner acquisition.

Each `RuntimeReachRank` cell first closes the rigid prefix captured at that
cell.  A Local/Ingress fair turn may admit finite work, but it also strictly
lowers the outer runner distance; the lower cell snapshots that exact work.
The Runtime boundary either emits directly or allocates the one immutable
periodic ordinal.  Tick is a frame and is never credited with ownership.
***************************************************************************)
ExactDecisionRequestClockAcquisitionAtDistance(node, qc, distance) ==
  /\ ExactDecisionRequestClockElapsedResidual(node, qc)
  /\ ~AsyncTimeoutClockDue(node)
  /\ ~ExactDecisionRequestRetransmitArmedResidual(node, qc)
  /\ ~ExactDecisionRequestPacketEmissionGoal(node, qc)
  /\ distance \in Nat
  /\ RuntimeReachRank(node) = distance

ExactDecisionRequestClockAcquisitionProgress(node, qc, distance) ==
  \/ ExactDecisionRequestPacketEmissionGoal(node, qc)
  \/ ExactDecisionRequestRetransmitArmedResidual(node, qc)
  \/ \E lower \in SetLessThan(
       distance, OpToRel(<, Nat), Nat):
       ExactDecisionRequestClockAcquisitionAtDistance(
         node, qc, lower)

THEOREM ExactDecisionRequestClockAcquisitionRankCoversElapsed ==
  \A node, qc:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestClockElapsedResidual(node, qc)
    /\ ~ExactDecisionRequestRetransmitArmedResidual(node, qc)
    /\ ~ExactDecisionRequestPacketEmissionGoal(node, qc)
      => \E distance \in Nat:
           ExactDecisionRequestClockAcquisitionAtDistance(
             node, qc, distance)
BY ExactDecisionRequestClockElapsedExcludesTimeoutClockDue,
   RuntimeReachRankWithinRunnerCycle, Isa
   DEF ExactDecisionRequestClockAcquisitionAtDistance,
       ExactDecisionRequestClockElapsedResidual,
       ExactDecisionRequestPacketEmissionResidual,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource

THEOREM ExactDecisionRequestElapsedCellHasRigidPrefixSnapshot ==
  \A node, qc, distance:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ ExactDecisionRequestClockAcquisitionAtDistance(
         node, qc, distance)
      => \E snapshot:
           /\ snapshot =
                ExactDecisionRequestRuntimePrefixSnapshot(
                  node,
                  AsyncNextCandidateLifecycleOrdinal(node),
                  AsyncNextIngressPhysicalOrdinal(node))
           /\ ExactDecisionRequestRuntimePrefixSnapshotActive(
                snapshot, node)
BY ExactDecisionRequestRuntimePrefixSnapshotIsFinite, Isa
   DEF ExactDecisionRequestClockAcquisitionAtDistance

THEOREM ExactDecisionRequestOwnedEpisodeHasRigidPrefixSnapshot ==
  \A node, qc, ownerOrdinal:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ ExactDecisionRequestOwnedRetransmitEpisode(
         node, qc, ownerOrdinal)
      => \E snapshot:
           /\ snapshot =
                ExactDecisionRequestRuntimePrefixSnapshot(
                  node, ownerOrdinal,
                  AsyncRetransmitLifecyclePhysicalCut(node))
           /\ ExactDecisionRequestRuntimePrefixSnapshotActive(
                snapshot, node)
           /\ ExactDecisionRequestClockPrefixSnapshotBinding(
                "Owned", snapshot, node, ownerOrdinal)
BY ExactDecisionRequestRuntimePrefixSnapshotIsFinite, Isa
   DEF ExactDecisionRequestOwnedRetransmitEpisode,
       ExactDecisionRequestClockPrefixSnapshotBinding,
       AsyncRetransmitLifecyclePhysicalCut,
       AsyncRetransmitLifecycleOwned

THEOREM ExactDecisionRequestDrainedElapsedPrefixEnablesFairRunner ==
  \A snapshot, node, qc, distance:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ ExactDecisionRequestClockAcquisitionAtDistance(
         node, qc, distance)
    /\ ExactDecisionRequestRuntimePrefixSnapshotActive(snapshot, node)
    /\ ExactDecisionRequestRuntimeFrozenPrefixDrained(snapshot)
      => ENABLED <<PostGstRunNode(node)>>_AsyncAllVars
BY ResponsiveUnappliedRunNodeIsEnabled,
   EnabledRunNodeLiftsPostGst,
   GstExcludesResponsiveReplayQuarantine,
   ExpandENABLED, ENABLEDaxioms, IsaT(1800)
   DEF ExactDecisionRequestClockAcquisitionAtDistance,
       ExactDecisionRequestClockElapsedResidual,
       ExactDecisionRequestPacketEmissionResidual,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionRequestRuntimeFrozenPrefixDrained,
       AsyncCurrentResponsiveVoters,
       PostGstRunNode, RunNode, RunNodeWork,
       AsyncAllVars

THEOREM ExactDecisionRequestDrainedElapsedFairRunAcquiresOrDescends ==
  \A snapshot, node, qc, distance:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ ExactDecisionFanoutRetentionInvariant
    /\ ExactDecisionRequestClockAcquisitionAtDistance(
         node, qc, distance)
    /\ ExactDecisionRequestRuntimePrefixSnapshotActive(snapshot, node)
    /\ ExactDecisionRequestRuntimeFrozenPrefixDrained(snapshot)
    /\ PostGstRunNode(node)
      => ExactDecisionRequestClockAcquisitionProgress(
           node, qc, distance)'
BY ExactDecisionSendingRetransmitPublishesExactAlias,
   AsyncRetransmitFreshEpisodeConsumesSharedLifecycleOrdinal,
   AsyncRetransmitFreshEpisodeAdvancesSharedHighWatermark,
   AsyncRetransmitFreshLiveEpisodeRetainsSharedLifecycleOrdinal,
   AsyncRetransmitCompletedOwnedEpisodeDefersFreshAcquisition,
   ExactDecisionRequestPeriodicEpisodeCannotResurrectDrainedOrdinal,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   IngressDrainStrictlyDecreasesRuntimeReach,
   AsyncVoterRuntimeReadyHasNoNonRunnerContinuationInsertion,
   ExactDecisionBodyHoldingAliasPersistsOrFrontier,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   AsyncNextPreservesExactDecisionFanoutRetention,
   IsaT(3600)
   DEF ExactDecisionRequestClockAcquisitionProgress,
       ExactDecisionRequestClockAcquisitionAtDistance,
       ExactDecisionRequestClockElapsedResidual,
       ExactDecisionRequestRetransmitArmedResidual,
       ExactDecisionRequestOwnedRetransmitEpisode,
       ExactDecisionRequestPacketEmissionGoal,
       ExactDecisionSendingRetransmitStep,
       ExactDecisionRequestRuntimeFrozenPrefixDrained,
       ExactDecisionRequestRuntimePrefixSnapshotActive,
       RuntimeReachRank,
       PostGstRunNode, RunNode, RunNodeWork,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       AsyncClockLifecycleFreezeBoundaryThisStep,
       AsyncRetransmitLifecycleCanAcquireThisStep,
       AsyncRetransmitLifecycleConsumesFreshOrdinal,
       DirectRetransmitStep, DeferredRetransmitStep,
       AsyncAllVars, SetLessThan, OpToRel

THEOREM FairExactDecisionRequestClockAcquisitionDistanceStep ==
  \A initialContext, node, qc:
    \A distance \in Nat:
    AsyncSpecAt(initialContext)
      => (ExactDecisionRequestClockAcquisitionAtDistance(
            node, qc, distance)
            ~> ExactDecisionRequestClockAcquisitionProgress(
                 node, qc, distance))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   ExactDecisionAsyncSpecAlwaysCandidateTombstones,
   DecisionFrontierUniquenessInvariantFromAsyncSpec,
   DecisionTimeoutFrontierInvariantFromAsyncSpec,
   FinalProgressWitnessClosureInvariantObligation,
   AsyncSpecAlwaysRetainsExactDecisionFanout,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   ExactDecisionRequestElapsedCellHasRigidPrefixSnapshot,
   AsyncSpecClosesExactDecisionRequestClockPrefix,
   ExactDecisionRequestDrainedElapsedPrefixEnablesFairRunner,
   ExactDecisionRequestDrainedElapsedFairRunAcquiresOrDescends,
   WF1, PTL, IsaT(1800)
   DEF ExactDecisionRequestClockPrefixClosureProperty,
       ExactDecisionRequestClockPrefixResidual,
       ExactDecisionRequestClockPrefixEndpoint,
       ExactDecisionRequestClockAcquisitionAtDistance,
       ExactDecisionRequestClockAcquisitionProgress,
       AsyncSpecAt, AsyncFairnessAt

THEOREM FairExactDecisionRequestClockElapsedAcquiresOwner ==
  \A initialContext, node, qc:
    AsyncSpecAt(initialContext)
      => ExactDecisionRequestClockElapsedResidual(node, qc)
           ~> (ExactDecisionRequestPacketEmissionGoal(node, qc)
                \/ ExactDecisionRequestRetransmitArmedResidual(
                     node, qc))
BY ExactDecisionRequestClockAcquisitionRankCoversElapsed,
   FairExactDecisionRequestClockAcquisitionDistanceStep,
   NatLessThanWellFounded, WellFoundedLeadsTo, PTL
   DEF ExactDecisionRequestClockAcquisitionProgress

(***************************************************************************
Owned periodic suffix.

The source prefix is frozen at the immutable periodic ordinal and the
receiver-local physical ingress cut captured with that owner.  Once it is
drained, the only target-local cells are: direct periodic ownership (2), the
owned `RetransmitElapsed` program point (1), an optional already-existing
timeout tag, and `RuntimeReachRank`.  A durable Decision cannot create a new
timeout owner.  Later FIFO/deferred work has a greater shared ordinal and is
not part of this rank.
***************************************************************************)
ExactDecisionRequestPeriodicStage(node, ownerOrdinal) ==
  IF /\ AsyncRetransmitLifecycleOwned(node)
     /\ AsyncRetransmitLifecycleOrdinal(node) = ownerOrdinal
  THEN IF "RetransmitElapsed" \in asyncOutstandingTags[node]
       THEN 1
       ELSE 2
  ELSE 0

ExactDecisionRequestOwnedTimeoutTagDebt(node) ==
  IF "TimeoutElapsed" \in asyncOutstandingTags[node]
  THEN 1
  ELSE 0

ExactDecisionRequestOwnedRuntimeTailRank(node) ==
  <<ExactDecisionRequestOwnedTimeoutTagDebt(node),
    RuntimeReachRank(node)>>

ExactDecisionRequestOwnedRuntimeRank(node, ownerOrdinal) ==
  <<ExactDecisionRequestPeriodicStage(node, ownerOrdinal),
    ExactDecisionRequestOwnedRuntimeTailRank(node)>>

ExactDecisionRequestOwnedRuntimeTailCarrier == (0..1) \X Nat
ExactDecisionRequestOwnedRuntimeCarrier ==
  (1..2) \X ExactDecisionRequestOwnedRuntimeTailCarrier

ExactDecisionRequestOwnedRuntimeTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), 0..1, Nat)

ExactDecisionRequestOwnedRuntimeOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    ExactDecisionRequestOwnedRuntimeTailOrdering,
    1..2, ExactDecisionRequestOwnedRuntimeTailCarrier)

THEOREM ExactDecisionRequestOwnedRuntimeOrderingIsWellFounded ==
  IsWellFoundedOn(
    ExactDecisionRequestOwnedRuntimeOrdering,
    ExactDecisionRequestOwnedRuntimeCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset,
   WFLexPairOrdering
   DEF ExactDecisionRequestOwnedRuntimeOrdering,
       ExactDecisionRequestOwnedRuntimeCarrier,
       ExactDecisionRequestOwnedRuntimeTailOrdering,
       ExactDecisionRequestOwnedRuntimeTailCarrier

ExactDecisionRequestRuntimeGoal(node, qc) ==
  \/ ExactDecisionRequestPacketEmissionGoal(node, qc)
  \/ ExactDecisionRequestSendingRetransmitReady(node, qc)

ExactDecisionRequestOwnedRuntimeAtRank(
    snapshot, node, qc, ownerOrdinal, rank) ==
  /\ ExactDecisionRequestOwnedRetransmitEpisode(
       node, qc, ownerOrdinal)
  /\ ~ExactDecisionRequestRuntimeGoal(node, qc)
  /\ ExactDecisionRequestRuntimePrefixSnapshotActive(snapshot, node)
  /\ ExactDecisionRequestClockPrefixSnapshotBinding(
       "Owned", snapshot, node, ownerOrdinal)
  /\ ExactDecisionRequestRuntimeFrozenPrefixDrained(snapshot)
  /\ rank \in ExactDecisionRequestOwnedRuntimeCarrier
  /\ ExactDecisionRequestOwnedRuntimeRank(node, ownerOrdinal) = rank

ExactDecisionRequestOwnedRuntimeRankProgress(
    snapshot, node, qc, ownerOrdinal, rank) ==
  \/ ExactDecisionRequestRuntimeGoal(node, qc)
  \/ \E lower \in
       SetLessThan(
         rank,
         ExactDecisionRequestOwnedRuntimeOrdering,
         ExactDecisionRequestOwnedRuntimeCarrier):
       ExactDecisionRequestOwnedRuntimeAtRank(
         snapshot, node, qc, ownerOrdinal, lower)

\* Compatibility projection retained for downstream theorem inventories.
ExactDecisionRequestRuntimeBlockedAtRank(node, qc, rank) ==
  \E snapshot:
    \E ownerOrdinal \in Nat \ {0}:
      ExactDecisionRequestOwnedRuntimeAtRank(
        snapshot, node, qc, ownerOrdinal, rank)

ExactDecisionRequestRuntimeRankProgress(node, qc, rank) ==
  \/ ExactDecisionRequestRuntimeGoal(node, qc)
  \/ \E lower \in
       SetLessThan(
         rank,
         ExactDecisionRequestOwnedRuntimeOrdering,
         ExactDecisionRequestOwnedRuntimeCarrier):
       ExactDecisionRequestRuntimeBlockedAtRank(node, qc, lower)

THEOREM ExactDecisionRequestOwnedPrefixHasRuntimeRank ==
  \A snapshot, node, qc, ownerOrdinal:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestOwnedRetransmitEpisode(
         node, qc, ownerOrdinal)
    /\ ~ExactDecisionRequestRuntimeGoal(node, qc)
    /\ ExactDecisionRequestRuntimePrefixSnapshotActive(snapshot, node)
    /\ ExactDecisionRequestClockPrefixSnapshotBinding(
         "Owned", snapshot, node, ownerOrdinal)
    /\ ExactDecisionRequestRuntimeFrozenPrefixDrained(snapshot)
      => \E rank \in ExactDecisionRequestOwnedRuntimeCarrier:
           ExactDecisionRequestOwnedRuntimeAtRank(
             snapshot, node, qc, ownerOrdinal, rank)
BY RuntimeReachRankWithinRunnerCycle, Isa
   DEF ExactDecisionRequestOwnedRuntimeAtRank,
       ExactDecisionRequestOwnedRuntimeRank,
       ExactDecisionRequestPeriodicStage,
       ExactDecisionRequestOwnedRuntimeTailRank,
       ExactDecisionRequestOwnedTimeoutTagDebt,
       ExactDecisionRequestOwnedRuntimeCarrier,
       ExactDecisionRequestOwnedRuntimeTailCarrier

THEOREM ExactDecisionRequestRuntimeRankCoversArmedResidual ==
  \A snapshot, node, qc, ownerOrdinal:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestOwnedRetransmitEpisode(
         node, qc, ownerOrdinal)
    /\ ~ExactDecisionRequestRuntimeGoal(node, qc)
    /\ ExactDecisionRequestRuntimePrefixSnapshotActive(snapshot, node)
    /\ ExactDecisionRequestClockPrefixSnapshotBinding(
         "Owned", snapshot, node, ownerOrdinal)
    /\ ExactDecisionRequestRuntimeFrozenPrefixDrained(snapshot)
      => \E rank \in ExactDecisionRequestOwnedRuntimeCarrier:
           ExactDecisionRequestRuntimeBlockedAtRank(
             node, qc, rank)
BY ExactDecisionRequestOwnedPrefixHasRuntimeRank, Isa
   DEF ExactDecisionRequestRuntimeBlockedAtRank

THEOREM ExactDecisionRequestRuntimeOwnerEnablesFairRunNode ==
  \A snapshot, node, qc, ownerOrdinal:
    \A rank \in ExactDecisionRequestOwnedRuntimeCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
      /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
      /\ ExactDecisionRequestOwnedRuntimeAtRank(
           snapshot, node, qc, ownerOrdinal, rank)
        => ENABLED <<PostGstRunNode(node)>>_AsyncAllVars
BY ResponsiveUnappliedRunNodeIsEnabled,
   EnabledRunNodeLiftsPostGst,
   GstExcludesResponsiveReplayQuarantine,
   ExpandENABLED, ENABLEDaxioms, IsaT(1800)
   DEF ExactDecisionRequestOwnedRuntimeAtRank,
       ExactDecisionRequestOwnedRetransmitEpisode,
       ExactDecisionRequestPacketEmissionResidual,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionRequestRuntimeFrozenPrefixDrained,
       PostGstRunNode, RunNode, RunNodeWork,
       AsyncAllVars

THEOREM ExactDecisionRequestSameNodeRunConsumesRuntimePrefix ==
  \A snapshot, node, qc, ownerOrdinal:
    \A rank \in ExactDecisionRequestOwnedRuntimeCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ DecisionFrontierUniquenessInvariant
      /\ DecisionTimeoutFrontierInvariant
      /\ ResponsiveRecoveryValidationClearedInvariant
      /\ FinalProgressWitnessClosureInvariant
      /\ ExactDecisionFanoutRetentionInvariant
      /\ ExactDecisionRequestOwnedRuntimeAtRank(
           snapshot, node, qc, ownerOrdinal, rank)
      /\ PostGstRunNode(node)
      => ExactDecisionRequestOwnedRuntimeRankProgress(
           snapshot, node, qc, ownerOrdinal, rank)'
BY ExactDecisionSendingRetransmitPublishesExactAlias,
   ExactDecisionRequestPeriodicEpisodeCannotResurrectDrainedOrdinal,
   AsyncRetransmitCompletedEpisodeClearsActiveOwner,
   AsyncRetransmitCompletedOwnedEpisodeDefersFreshAcquisition,
   AsyncRetransmitLifecycleOwnerAndPhysicalCutPersistUntilEndpoint,
   AsyncRetransmitLifecycleOwnerAndPhysicalCutClearAtEndpoint,
   DeferredRetransmitConsumesDriveProgramCounter,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   IngressDrainStrictlyDecreasesRuntimeReach,
   AsyncVoterRuntimeReadyHasNoNonRunnerContinuationInsertion,
   ExactDecisionBodyHoldingAliasPersistsOrFrontier,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   AsyncBracketNextPreservesFinalProgressWitnessClosure,
   AsyncNextPreservesExactDecisionFanoutRetention,
   IsaT(3600)
   DEF ExactDecisionRequestOwnedRuntimeRankProgress,
       ExactDecisionRequestOwnedRuntimeAtRank,
       ExactDecisionRequestRuntimeGoal,
       ExactDecisionRequestSendingRetransmitReady,
       ExactDecisionRequestOwnedRetransmitEpisode,
       ExactDecisionRequestPacketEmissionResidual,
       ExactDecisionRequestPacketEmissionGoal,
       ExactDecisionSendingRetransmitStep,
       ExactDecisionRequestOwnedRuntimeRank,
       ExactDecisionRequestPeriodicStage,
       ExactDecisionRequestOwnedRuntimeTailRank,
       ExactDecisionRequestOwnedTimeoutTagDebt,
       ExactDecisionRequestOwnedRuntimeOrdering,
       ExactDecisionRequestOwnedRuntimeCarrier,
       ExactDecisionRequestOwnedRuntimeTailOrdering,
       ExactDecisionRequestOwnedRuntimeTailCarrier,
       ExactDecisionRequestRuntimeFrozenPrefixDrained,
       RuntimeReachRank,
       PostGstRunNode, RunNode, RunNodeWork,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       DeferredTagStep, DeferredTimeoutStep,
       DeferredRetransmitStep, DirectRetransmitStep,
       DeferredTagExecutable, DeferredTimeoutExecutable,
       RetransmitDue, AsyncRetransmitProgramCounter,
       AsyncAllVars, SetLessThan, OpToRel

THEOREM ExactDecisionRequestRuntimeBlockedStepIsSafe ==
  \A snapshot, node, qc, ownerOrdinal:
    \A rank \in ExactDecisionRequestOwnedRuntimeCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ DecisionFrontierUniquenessInvariant
      /\ DecisionTimeoutFrontierInvariant
      /\ ResponsiveRecoveryValidationClearedInvariant
      /\ FinalProgressWitnessClosureInvariant
      /\ ExactDecisionFanoutRetentionInvariant
      /\ ExactDecisionRequestOwnedRuntimeAtRank(
           snapshot, node, qc, ownerOrdinal, rank)
      /\ [AsyncNext]_AsyncAllVars
      => \/ ExactDecisionRequestOwnedRuntimeAtRank(
              snapshot, node, qc, ownerOrdinal, rank)'
         \/ ExactDecisionRequestOwnedRuntimeRankProgress(
              snapshot, node, qc, ownerOrdinal, rank)'
BY ExactDecisionRequestSameNodeRunConsumesRuntimePrefix,
   ExactDecisionBodyHoldingAliasPersistsOrFrontier,
   ExactDecisionSendingRetransmitPublishesExactAlias,
   AsyncRetransmitFreshLiveEpisodeRetainsSharedLifecycleOrdinal,
   AsyncRetransmitCompletedOwnedEpisodeDefersFreshAcquisition,
   AsyncRetransmitLifecycleOwnerAndPhysicalCutPersistUntilEndpoint,
   AsyncRetransmitLifecycleOwnerAndPhysicalCutClearAtEndpoint,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   AsyncBracketNextPreservesFinalProgressWitnessClosure,
   AsyncNextPreservesExactDecisionFanoutRetention,
   IsaT(3600)
   DEF ExactDecisionRequestOwnedRuntimeRankProgress,
       ExactDecisionRequestOwnedRuntimeAtRank,
       ExactDecisionRequestRuntimeGoal,
       ExactDecisionRequestOwnedRuntimeRank,
       ExactDecisionRequestPeriodicStage,
       ExactDecisionRequestOwnedRuntimeTailRank,
       ExactDecisionRequestOwnedTimeoutTagDebt,
       ExactDecisionRequestRuntimePrefixSnapshotActive,
       ExactDecisionRequestRuntimeCandidateOriginsAt,
       ExactDecisionRequestRuntimeContinuationSourcesAt,
       ExactDecisionRequestRuntimeServeSourcesAt,
       ExactDecisionRequestRuntimeLeaderWireIdentitiesAt,
       ExactDecisionRequestRuntimeFrozenPrefixDrained,
       AsyncNext, AsyncAllVars, SetLessThan, OpToRel

THEOREM FairExactDecisionRequestRuntimeRankStep ==
  \A initialContext, snapshot, node, qc, ownerOrdinal:
    \A rank \in ExactDecisionRequestOwnedRuntimeCarrier:
      AsyncSpecAt(initialContext)
        => (ExactDecisionRequestOwnedRuntimeAtRank(
              snapshot, node, qc, ownerOrdinal, rank)
              ~> ExactDecisionRequestOwnedRuntimeRankProgress(
                   snapshot, node, qc, ownerOrdinal, rank))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   DecisionFrontierUniquenessInvariantFromAsyncSpec,
   DecisionTimeoutFrontierInvariantFromAsyncSpec,
   ResponsiveRecoveryValidationClearedInvariantObligation,
   FinalProgressWitnessClosureInvariantObligation,
   AsyncSpecAlwaysRetainsExactDecisionFanout,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   ExactDecisionRequestRuntimeBlockedStepIsSafe,
   ExactDecisionRequestRuntimeOwnerEnablesFairRunNode,
   ExactDecisionRequestSameNodeRunConsumesRuntimePrefix,
   WF1, PTL, IsaT(1800)
   DEF ExactDecisionRequestOwnedRuntimeAtRank,
       ExactDecisionRequestOwnedRuntimeRankProgress,
       ExactDecisionRequestOwnedRetransmitEpisode,
       ExactDecisionRequestPacketEmissionResidual,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       AsyncSpecAt, AsyncFairnessAt

THEOREM FairExactDecisionRequestRuntimeRankConverges ==
  \A initialContext, snapshot, node, qc, ownerOrdinal:
    AsyncSpecAt(initialContext)
      => \A rank \in ExactDecisionRequestOwnedRuntimeCarrier:
           ExactDecisionRequestOwnedRuntimeAtRank(
             snapshot, node, qc, ownerOrdinal, rank)
             ~> ExactDecisionRequestRuntimeGoal(node, qc)
BY FairExactDecisionRequestRuntimeRankStep,
   ExactDecisionRequestOwnedRuntimeOrderingIsWellFounded,
   WellFoundedLeadsTo
   DEF ExactDecisionRequestOwnedRuntimeRankProgress

THEOREM FairExactDecisionRequestOwnedPrefixAndRuntimeConverges ==
  \A initialContext, node, qc:
    AsyncSpecAt(initialContext)
      => ExactDecisionRequestRetransmitArmedResidual(node, qc)
           ~> ExactDecisionRequestRuntimeGoal(node, qc)
BY AsyncSpecAlwaysStrongTypeInvariant,
   ExactDecisionAsyncSpecAlwaysCandidateTombstones,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   ExactDecisionRequestRuntimePrefixSnapshotIsFinite,
   ExactDecisionRequestOwnedEpisodeHasRigidPrefixSnapshot,
   AsyncSpecClosesExactDecisionRequestClockPrefix,
   ExactDecisionRequestOwnedPrefixHasRuntimeRank,
   FairExactDecisionRequestRuntimeRankConverges,
   PTL, IsaT(1800)
   DEF ExactDecisionRequestRetransmitArmedResidual,
       ExactDecisionRequestClockPrefixClosureProperty,
       ExactDecisionRequestClockPrefixResidual,
       ExactDecisionRequestClockPrefixEndpoint,
       ExactDecisionRequestOwnedRuntimeAtRank,
       ExactDecisionRequestRuntimeGoal

ExactDecisionRequestSendingReadyPersistsOrGoal(node, qc) ==
  ExactDecisionRequestSendingRetransmitReady(node, qc)
    => \/ ExactDecisionRequestSendingRetransmitReady(node, qc)'
       \/ ExactDecisionRequestPacketEmissionGoal(node, qc)'

THEOREM ExactDecisionRequestSendingReadyStepIsSafe ==
  \A node, qc:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ ResponsiveRecoveryValidationClearedInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ ExactDecisionFanoutRetentionInvariant
    /\ [AsyncNext]_AsyncAllVars
    => ExactDecisionRequestSendingReadyPersistsOrGoal(node, qc)
BY ExactDecisionSendingRetransmitPublishesExactAlias,
   ExactDecisionBodyHoldingAliasPersistsOrFrontier,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   AsyncBracketNextPreservesFinalProgressWitnessClosure,
   AsyncNextPreservesExactDecisionFanoutRetention,
   ExpandENABLED, IsaT(600)
   DEF ExactDecisionRequestSendingReadyPersistsOrGoal,
       ExactDecisionRequestSendingRetransmitReady,
       ExactDecisionRequestPacketEmissionResidual,
       ExactDecisionRequestPacketEmissionGoal,
       ExactDecisionSendingRetransmitStep,
       PostGstRunNode, RunNode, RunNodeWork,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       AsyncAllVars

THEOREM FairExactDecisionRequestSendingHandoff ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => \A node, qc:
           ExactDecisionRequestSendingRetransmitReady(node, qc)
             ~> ExactDecisionRequestPacketEmissionGoal(node, qc)
PROOF
  <1>1. ASSUME NEW initialContext, AsyncSpecAt(initialContext),
                NEW node, NEW qc
         PROVE ExactDecisionRequestSendingRetransmitReady(node, qc)
                 ~> ExactDecisionRequestPacketEmissionGoal(node, qc)
    <2>1. [](/\ AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ DecisionFrontierUniquenessInvariant
              /\ DecisionTimeoutFrontierInvariant
              /\ ResponsiveRecoveryValidationClearedInvariant
              /\ FinalProgressWitnessClosureInvariant
              /\ ExactDecisionFanoutRetentionInvariant)
      BY <1>1,
         AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         DecisionFrontierUniquenessInvariantFromAsyncSpec,
         DecisionTimeoutFrontierInvariantFromAsyncSpec,
         ResponsiveRecoveryValidationClearedInvariantObligation,
         FinalProgressWitnessClosureInvariantObligation,
         AsyncSpecAlwaysRetainsExactDecisionFanout, PTL
    <2>2. ExactDecisionRequestSendingRetransmitReady(node, qc)
              /\ [AsyncNext]_AsyncAllVars
            => \/ ExactDecisionRequestSendingRetransmitReady(
                    node, qc)'
               \/ ExactDecisionRequestPacketEmissionGoal(node, qc)'
      BY <2>1, ExactDecisionRequestSendingReadyStepIsSafe
    <2>3. /\ ExactDecisionRequestSendingRetransmitReady(node, qc)
             /\ ~ExactDecisionRequestPacketEmissionGoal(node, qc)
            => ENABLED <<PostGstRunNode(node)>>_AsyncAllVars
      BY ENABLEDaxioms, Isa
         DEF ExactDecisionRequestSendingRetransmitReady,
             AsyncAllVars
    <2>4. /\ ExactDecisionRequestSendingRetransmitReady(node, qc)
             /\ ~ExactDecisionRequestPacketEmissionGoal(node, qc)
             /\ <<PostGstRunNode(node)>>_AsyncAllVars
            => ExactDecisionRequestPacketEmissionGoal(node, qc)'
      BY <2>1, ExactDecisionSendingRetransmitPublishesExactAlias, Isa
         DEF ExactDecisionRequestSendingRetransmitReady,
             ExactDecisionSendingRetransmitStep
    <2>5. CASE node \in AsyncVotersAt(initialContext)
      <3>1. WF_AsyncAllVars(PostGstRunNode(node))
        BY <1>1, <2>5 DEF AsyncSpecAt, AsyncFairnessAt
      <3> QED BY <2>2, <2>3, <2>4, <3>1, PTL
    <2>6. CASE node \notin AsyncVotersAt(initialContext)
      <3>1. []~ExactDecisionRequestSendingRetransmitReady(node, qc)
        BY <1>1, <2>6,
           AsyncSpecAlwaysUsesFixedResponsiveVoters, PTL
           DEF ExactDecisionRequestSendingRetransmitReady,
               ExactDecisionRequestPacketEmissionResidual,
               ExactDecisionActiveRequestOwner,
               ExactDecisionServiceSource
      <3> QED BY <3>1, PTL
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM ExactDecisionRequestRuntimePrefixConvergence ==
  \A initialContext:
    ExactDecisionRequestRuntimePrefixConvergenceProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext, AsyncSpecAt(initialContext)
         PROVE \A node, qc:
                  /\ ExactDecisionRequestRetransmitArmedResidual(node, qc)
                       ~> (ExactDecisionRequestPacketEmissionGoal(node, qc)
                            \/ ExactDecisionRequestSendingRetransmitReady(
                                 node, qc))
                  /\ ExactDecisionRequestSendingRetransmitReady(node, qc)
                       ~> ExactDecisionRequestPacketEmissionGoal(node, qc)
    <2>1. ASSUME NEW node, NEW qc
           PROVE
             ExactDecisionRequestRetransmitArmedResidual(node, qc)
               ~> ExactDecisionRequestRuntimeGoal(node, qc)
      BY <1>1,
         FairExactDecisionRequestOwnedPrefixAndRuntimeConverges
    <2>2. \A node, qc:
             ExactDecisionRequestSendingRetransmitReady(node, qc)
               ~> ExactDecisionRequestPacketEmissionGoal(node, qc)
      BY <1>1, FairExactDecisionRequestSendingHandoff
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1
       DEF ExactDecisionRequestRuntimePrefixConvergenceProperty,
           ExactDecisionRequestRuntimeGoal

(***************************************************************************
Non-circular request-ingress temporal decomposition.

The height-reset development supplies exact static classification and enabled
action leaves for an overdue admissible lane head.  It does not establish
convergence for this particular packet: its Runtime-reset convergence theorem
is stated under `AsyncLiveSpecAt` and reaches generic immediate productivity
or aggregate Decision, whereas this corridor is required under `AsyncSpecAt`
and must preserve the exact request alias.  Moreover, the archive may apply
while the request is queued, changing the fair runner from `PostGstRunNode`
to `PostGstRunHistoricalServer`.

The first kernel therefore owns only the finite transport prefix: delivery
deadline, older due source-lane heads, and finite admission-gate owners.  Its
successful non-goal exit is the exact selected packet with its fair admission
action enabled.

The exact enabled admission/coalescing handoff is proved below from its
per-source weak-fair action.  The remaining named property orders only the
normal-or-historical ingress runner prefix until the exact request creates a
fresh Serve owner.  It assumes neither the broad request-ingress kernel, the
combined off-scheduler property, nor whole-stage convergence.
***************************************************************************)

THEOREM FairExactDecisionRequestAdmissionHandoff ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => \A node, qc, archive, request, packet:
           ExactDecisionRequestPacketAdmissionReady(
             node, qc, archive, request, packet)
             ~> ExactDecisionRequestAdmissionOutcome(
                  node, qc, archive, request)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => \A node, qc, archive, request, packet:
                      ExactDecisionRequestPacketAdmissionReady(
                        node, qc, archive, request, packet)
                        ~> ExactDecisionRequestAdmissionOutcome(
                             node, qc, archive, request)
    <2>1. AsyncSpecAt(initialContext)
             => [](/\ AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ DecisionFrontierUniquenessInvariant
                    /\ DecisionTimeoutFrontierInvariant
                    /\ ResponsiveRecoveryValidationClearedInvariant
                    /\ FinalProgressWitnessClosureInvariant
                    /\ ExactDecisionFanoutRetentionInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         DecisionFrontierUniquenessInvariantFromAsyncSpec,
         DecisionTimeoutFrontierInvariantFromAsyncSpec,
         ResponsiveRecoveryValidationClearedInvariantObligation,
         FinalProgressWitnessClosureInvariantObligation,
         AsyncSpecAlwaysRetainsExactDecisionFanout, PTL
    <2>2. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node, qc, archive, request, packet:
                    ExactDecisionRequestPacketAdmissionReady(
                      node, qc, archive, request, packet)
                      ~> ExactDecisionRequestAdmissionOutcome(
                           node, qc, archive, request)
      <3>1. ASSUME NEW node, NEW qc, NEW archive,
                    NEW request, NEW packet
             PROVE ExactDecisionRequestPacketAdmissionReady(
                     node, qc, archive, request, packet)
                     ~> ExactDecisionRequestAdmissionOutcome(
                          node, qc, archive, request)
        <4>1. [](ExactDecisionRequestPacketAdmissionReady(
                   node, qc, archive, request, packet)
                  => /\ archive \in Responsive
                     /\ request.source \in AsyncIngressSources)
          BY <2>1, <2>2, Isa, PTL
             DEF ExactDecisionRequestPacketAdmissionReady,
                 ExactDecisionRequestIngressResidual,
                 ExactDecisionRequestPacketOwned,
                 ExactDecisionBodyHoldingAlias,
                 ExactDecisionActiveRequestOwner,
                 ExactDecisionServiceSource,
                 AsyncCurrentResponsiveVoters,
                 AsyncIngressSources,
                 AsyncStrongTypeInvariant,
                 AsyncSchedulerTypeInvariant,
                 AsyncTransportTypeInvariant,
                 AsyncTransportContentTypeInvariant,
                 AsyncPacketContentTypeInvariant,
                 AsyncPacketTyped, AsyncItemTyped
        <4>2. ExactDecisionRequestPacketAdmissionReady(
                 node, qc, archive, request, packet)
                   /\ [AsyncNext]_AsyncAllVars
                => \/ ExactDecisionRequestPacketAdmissionReady(
                        node, qc, archive, request, packet)'
                   \/ ExactDecisionRequestAdmissionOutcome(
                        node, qc, archive, request)'
          BY <2>1, ExactDecisionRequestAdmissionReadyStepIsSafe
        <4>3. CASE /\ archive \in Responsive
                     /\ request.source \in AsyncIngressSources
          <5>1. AsyncSpecAt(initialContext)
                   => WF_AsyncAllVars(
                        PostGstAdmitHiddenPacket(
                          archive, request.source))
            BY <4>3 DEF AsyncSpecAt, AsyncFairnessAt
          <5>2. /\ ExactDecisionRequestPacketAdmissionReady(
                       node, qc, archive, request, packet)
                   /\ ~ExactDecisionRequestAdmissionOutcome(
                        node, qc, archive, request)
                  => ENABLED
                       <<PostGstAdmitHiddenPacket(
                           archive, request.source)>>_AsyncAllVars
            BY ExactDecisionRequestReadyEnablesFairAdmission
          <5>3. /\ ExactDecisionRequestPacketAdmissionReady(
                       node, qc, archive, request, packet)
                   /\ ~ExactDecisionRequestAdmissionOutcome(
                        node, qc, archive, request)
                   /\ <<PostGstAdmitHiddenPacket(
                         archive, request.source)>>_AsyncAllVars
                  => ExactDecisionRequestAdmissionOutcome(
                       node, qc, archive, request)'
            BY ExactDecisionRequestFairAdmissionCreatesOutcome
          <5> QED BY <4>2, <5>1, <5>2, <5>3, PTL
               DEF AsyncSpecAt
        <4>4. CASE \/ archive \notin Responsive
                     \/ request.source \notin AsyncIngressSources
          <5>1. AsyncSpecAt(initialContext)
                   => []~ExactDecisionRequestPacketAdmissionReady(
                         node, qc, archive, request, packet)
            BY <4>1, <4>4, PTL
          <5> QED BY <5>1, PTL
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>1
    <2> QED BY <2>2
  <1> QED BY <1>1

ExactDecisionRequestHeadGateOwnerConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, archive, request, packet:
         ExactDecisionRequestHeadGateOwnerResidual(
           node, qc, archive, request, packet)
           ~> (ExactDecisionRequestIngressGoal(
                 node, qc, archive, request)
                \/ ExactDecisionRequestPacketAdmissionReady(
                     node, qc, archive, request, packet))

ExactDecisionRequestAdmissionHandoffConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, archive, request, packet:
         ExactDecisionRequestPacketAdmissionReady(
           node, qc, archive, request, packet)
           ~> ExactDecisionRequestAdmissionOutcome(
                node, qc, archive, request)

THEOREM ExactDecisionRequestAdmissionHandoffConvergence ==
  \A initialContext:
    ExactDecisionRequestAdmissionHandoffConvergenceProperty(
      AsyncSpecAt(initialContext))
BY FairExactDecisionRequestAdmissionHandoff
   DEF ExactDecisionRequestAdmissionHandoffConvergenceProperty

(***************************************************************************
Exact request-ingress runner boundary.

The lane residual alone is not an enabled exact drain.  Before Apply, the
archive runner may still be in Local or Runtime, the exact request may be
blocked by Completion causal debt or the Serve reservation, and a claimed
response or request-fenced physical completion may own selector priority.
After Apply, the historical server has no runner-phase prefix, but the exact
request may still be blocked by the Serve reservation or a different
historically drainable item may be selected.

The two readiness predicates below therefore state the complete concrete
head/action guards.  Their exact selected actions are enabled and create a
fresh nonce-owned Serve occurrence.  The broad lane residual does not
immediately imply either readiness predicate: the lifecycle section below
supplies the per-item well-founded rank that orders selector priority,
earlier source/lane owners, causal debt, and Serve-capacity owners.  Weak
fairness of the per-archive runner alone remains insufficient because a fair
runner occurrence may drain another item (and an idle historical-server
occurrence may stutter while the Serve reservation is physically full).
***************************************************************************)

ExactDecisionNormalRequestIngressRunnerReady(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ ~NodeHasApplication(archive)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(archive)
  /\ asyncRunnerPhase[archive] = "Ingress"
  /\ asyncRunnerBudget[archive] > 0
  /\ IngressItemCanDrain(archive, request)
  /\ DrainableIngressIndices(archive) # {}
  /\ SelectedIngressItemAt(
       archive, FirstDrainableIngressIndex(archive)) = request

ExactDecisionHistoricalRequestIngressRunnerReady(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ NodeHasApplication(archive)
  /\ HistoricalIngressItemCanDrain(archive, request)
  /\ HistoricalDrainableIngressIndices(archive) # {}
  /\ HistoricalSelectedIngressItemAt(
       archive,
       FirstHistoricalDrainableIngressIndex(archive)) = request

ExactDecisionRequestIngressRunnerReady(
    node, qc, archive, request) ==
  \/ ExactDecisionNormalRequestIngressRunnerReady(
       node, qc, archive, request)
  \/ ExactDecisionHistoricalRequestIngressRunnerReady(
       node, qc, archive, request)

ExactDecisionRequestIngressRunnerBlocked(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ ~ExactDecisionRequestIngressRunnerReady(
       node, qc, archive, request)

THEOREM ExactDecisionRequestIngressLaneSplitsAtRunnerReadiness ==
  \A node, qc, archive, request:
    ExactDecisionRequestIngressLaneResidual(
      node, qc, archive, request)
      => \/ ExactDecisionRequestIngressRunnerReady(
              node, qc, archive, request)
         \/ ExactDecisionRequestIngressRunnerBlocked(
              node, qc, archive, request)
BY Isa DEF ExactDecisionRequestIngressRunnerBlocked

ExactDecisionNormalRequestIngressRunnerAction(archive, request) ==
  /\ PostGstRunNode(archive)
  /\ DrainFairIngressSelected(archive)
  /\ SelectedIngressItemAt(
       archive, FirstDrainableIngressIndex(archive)) = request

ExactDecisionHistoricalRequestIngressRunnerAction(archive, request) ==
  /\ PostGstRunHistoricalServer(archive)
  /\ DrainHistoricalIngressSelected(archive)
  /\ HistoricalSelectedIngressItemAt(
       archive,
       FirstHistoricalDrainableIngressIndex(archive)) = request

ExactDecisionRequestIngressRunnerAction(archive, request) ==
  \/ ExactDecisionNormalRequestIngressRunnerAction(archive, request)
  \/ ExactDecisionHistoricalRequestIngressRunnerAction(archive, request)

THEOREM ExactDecisionNormalRequestIngressRunnerCreatesFreshServeOwner ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ ExactDecisionServeAdmissionOwned(archive, request)
    /\ ExactDecisionNormalRequestIngressRunnerAction(archive, request)
    => \E job \in SequenceSet(asyncIoQueues'[archive]):
         ExactDecisionServeJobOwned(
           node, qc, archive, request, job)'
BY NormalExactRequestIngressCreatesFreshServeOwner, Isa
   DEF ExactDecisionRequestIngressLaneResidual,
       ExactDecisionNormalRequestIngressRunnerAction,
       PostGstRunNode, RunNode

THEOREM ExactDecisionHistoricalRequestIngressRunnerCreatesFreshServeOwner ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ ExactDecisionServeAdmissionOwned(archive, request)
    /\ ExactDecisionHistoricalRequestIngressRunnerAction(
         archive, request)
    => \E job \in SequenceSet(asyncIoQueues'[archive]):
         ExactDecisionServeJobOwned(
           node, qc, archive, request, job)'
BY HistoricalExactRequestIngressCreatesFreshServeOwner, Isa
   DEF ExactDecisionRequestIngressLaneResidual,
       ExactDecisionHistoricalRequestIngressRunnerAction,
       PostGstRunHistoricalServer, RunHistoricalServer

THEOREM ExactDecisionCachedRequestIngressRunnerCreatesResponseOwner ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ ExactDecisionServeTombstoneOwned(
         node, qc, archive, request)
    /\ ExactDecisionRequestIngressRunnerAction(archive, request)
    => \E response, packet:
         ExactDecisionResponsePacketOwned(
           node, qc, archive, request, response, packet)'
BY IsaT(180)
   DEF ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressRunnerAction,
       ExactDecisionNormalRequestIngressRunnerAction,
       ExactDecisionHistoricalRequestIngressRunnerAction,
       ExactDecisionServeTombstoneOwned,
       ExactDecisionResponsePacketOwned,
       ExactDecisionAuthenticatedResponse,
       ExactDecisionBodyHoldingAlias,
       AsyncServeCachedReplayItems,
       AsyncServeTombstoneOutputs,
       DrainFairIngressSelected,
       DrainHistoricalIngressSelected,
       PublishEphemeralItems, PacketsForItems

THEOREM ExactDecisionCachedRequestIngressRunnerBypassesServeLifecycle ==
  \A node, qc, archive, request:
    LET identity ==
          ExactDecisionServeLifecycleIdentity(archive, request)
    IN /\ AsyncStrongTypeInvariant
       /\ ExactDecisionRequestIngressLaneResidual(
            node, qc, archive, request)
       /\ ExactDecisionServeTombstoneOwned(
            node, qc, archive, request)
       /\ ExactDecisionRequestIngressRunnerAction(archive, request)
       => /\ asyncIoQueues'[archive] = asyncIoQueues[archive]
          /\ AsyncServeLifecycleTombstone(archive, identity)'
          /\ ~AsyncServeLiveReservationOwned(archive, identity)'
          /\ ~AsyncServeJobQueued(archive, identity)'
          /\ ~AsyncServeIngressAdmissionOwned(archive, identity)'
          /\ \E response, packet:
               ExactDecisionResponsePacketOwned(
                 node, qc, archive, request, response, packet)'
BY ExactDecisionCachedRequestIngressRunnerCreatesResponseOwner, IsaT(240)
   DEF ExactDecisionRequestIngressRunnerAction,
       ExactDecisionNormalRequestIngressRunnerAction,
       ExactDecisionHistoricalRequestIngressRunnerAction,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionServeTombstoneOwned,
       ExactDecisionServeLifecycleIdentity,
       ExactDecisionResponsePacketOwned,
       AsyncServeLifecycleTombstone,
       AsyncServeLiveReservationOwned, AsyncServeJobQueued,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeLifecyclePartitionInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncServeLifecycleTypeInvariant,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       AcceptOrCoalesceExactServeRequest,
       CoalesceExactServeCapacity,
       AsyncServeCachedReplayItems, PublishEphemeralItems,
       AsyncAllVars

THEOREM ExactDecisionRequestIngressRunnerActionCreatesGoal ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestIngressRunnerAction(archive, request)
    => ExactDecisionRequestIngressGoal(
         node, qc, archive, request)'
BY ExactDecisionNormalRequestIngressRunnerCreatesFreshServeOwner,
   ExactDecisionHistoricalRequestIngressRunnerCreatesFreshServeOwner,
   ExactDecisionCachedRequestIngressRunnerBypassesServeLifecycle, IsaT(180)
   DEF ExactDecisionRequestIngressRunnerAction,
       ExactDecisionRequestIngressGoal,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionServeAdmissionOwned,
       ExactDecisionServeTombstoneOwned,
       AsyncServeLifecyclePartitionInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncServeLifecycleTypeInvariant

THEOREM ExactDecisionRequestIngressRunnerActionPersistsOrGoals ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ [ExactDecisionRequestIngressRunnerAction(
          archive, request)]_AsyncAllVars
    => \/ ExactDecisionRequestIngressLaneResidual(
            node, qc, archive, request)'
       \/ ExactDecisionRequestIngressGoal(
            node, qc, archive, request)'
BY ExactDecisionRequestIngressRunnerActionCreatesGoal, Isa
   DEF ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressRunnerAction,
       AsyncAllVars

THEOREM ExactDecisionNormalRequestIngressReadyEnablesExactAction ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ ExactDecisionNormalRequestIngressRunnerReady(
         node, qc, archive, request)
    => ENABLED
         <<ExactDecisionNormalRequestIngressRunnerAction(
             archive, request)>>_AsyncAllVars
BY GstResponsiveUnappliedRunNodeIsEnabled,
   AsyncStrongTypeProjectsAsyncType,
   RunNodeIsNonstuttering, ENABLEDaxioms, IsaT(180)
   DEF ExactDecisionNormalRequestIngressRunnerReady,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionNormalRequestIngressRunnerAction,
       PostGstRunNode, RunNode, RunNodeWork, IngressDrainStep,
       AsyncAllVars

THEOREM ExactDecisionHistoricalRequestIngressReadyEnablesExactAction ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionHistoricalRequestIngressRunnerReady(
         node, qc, archive, request)
    => ENABLED
         <<ExactDecisionHistoricalRequestIngressRunnerAction(
             archive, request)>>_AsyncAllVars
BY GstHistoricalServerIsEnabled,
   GstResponsiveNodesAreUp,
   HistoricalExactRequestIngressCreatesFreshServeOwner,
   ENABLEDaxioms, ExpandENABLED, IsaT(300)
   DEF ExactDecisionHistoricalRequestIngressRunnerReady,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressGoal,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionServeJobOwned,
       ExactDecisionServeOccurrenceOwned,
       ExactDecisionHistoricalRequestIngressRunnerAction,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       AsyncArchiveServerIds,
       PostGstRunHistoricalServer, RunHistoricalServer,
       DrainHistoricalIngressSelected, PopSelectedIngress,
       AsyncAllVars, AsyncSchedulerVars, AsyncProducerVars,
       SequenceSet

(***************************************************************************
Small exact per-item ingress rank interface.

This rank is an ownership interface, not a convergence claim.  Apply is the
outer mode decrease.  Serve capacity is target-specific: it is nonzero only
for this exact request's live off-queue reservation, never for a cached replay
or an already-queued Serve job.  The acyclic runner-turn reach precedes
selector priority, so a Local or Runtime step may expose a pre-cutoff owner
without turning that exposure into rank ascent.  Selector-priority
occurrences precede the exact lane and source positions because a priority
drain may rotate a source which follows the target.  The target lane position
precedes the target source position because draining an earlier item from the
target lane rotates that source to the ready-list tail.

Completion causal debt is intentionally not a component: it does not gate
either exact reply-request kind in `IngressItemCanDrain`.  Charging it here
would let a Local producer manufacture a false rank ascent while the exact
target remained drainable.

The priority component counts concrete lane occurrences, rather than merely
ready sources: one source may retain several request-fenced physical owners
after one drain.  The request lane itself is not known to have unique values,
so its component is the least matching occurrence rather than an arbitrary
value-based CHOOSE.  All components are state-based and every exact lane
residual has a rank in the explicit well-founded carrier.
***************************************************************************)

ExactDecisionRequestIngressModeRank(archive) ==
  IF NodeHasApplication(archive) THEN 0 ELSE 1

ExactDecisionRequestIngressCausalDebt(archive) ==
  IF ~NodeHasApplication(archive)
       /\ CompletionCausalAdmissionDebt(archive)
  THEN 1
  ELSE 0

(***************************************************************************
This component measures the nonnegative physical Serve backlog.  Effective
depth is intentionally not used here: one off-queue reservation saturates
effective capacity even when the physical queue is shallow, and subtracting
`AsyncIoAuxCapacity` from that shallow depth would leave the natural-number
rank carrier.  The outer lifecycle debt separately owns the reservation's
frozen I/O jobs, including the full-to-resumable I/O-worker handoff.
***************************************************************************)
ExactDecisionRequestIngressServeCapacityDebt(archive) ==
  IF AsyncIoQueueDepth(archive) < AsyncIoAuxCapacity
  THEN 0
  ELSE AsyncIoQueueDepth(archive) - AsyncIoAuxCapacity + 1

ExactDecisionRequestIngressTargetServeCapacityDebt(archive, request) ==
  LET identity ==
        ExactDecisionServeLifecycleIdentity(archive, request)
  IN IF /\ AsyncServeLiveReservationOwned(archive, identity)
           /\ ~AsyncServeJobQueued(archive, identity)
     THEN ExactDecisionRequestIngressServeCapacityDebt(archive)
     ELSE 0

ExactDecisionRequestIngressPriorityOwners(archive) ==
  {pair \in AsyncIngressSources \X (1..AsyncIngressCapacity):
     \/ pair[2] \in
          DrainableClaimedResponseLaneIndices(archive, pair[1])
     \/ pair[2] \in
          DrainableRequestFencedCompletionLaneIndices(
            archive, pair[1])}

ExactDecisionRequestIngressPriorityDebt(archive) ==
  Cardinality(ExactDecisionRequestIngressPriorityOwners(archive))

ExactDecisionRequestIngressLaneIndices(archive, request) ==
  {index \in
       1..Len(IngressLane(
                archive, IngressResourceSource(request))):
     IngressLane(
       archive, IngressResourceSource(request))[index] = request}

ExactDecisionRequestIngressLanePosition(archive, request) ==
  CHOOSE least \in
      ExactDecisionRequestIngressLaneIndices(archive, request):
    \A other \in
        ExactDecisionRequestIngressLaneIndices(archive, request):
      least <= other

ExactDecisionRequestIngressOccurrenceMultiplicityResidual(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ Cardinality(
       ExactDecisionRequestIngressLaneIndices(archive, request)) > 1

ExactDecisionRequestIngressSourcePosition(archive, request) ==
  IngressSourceServiceRank(
    archive, IngressResourceSource(request))

ExactDecisionRequestIngressReachRank(archive) ==
  IF NodeHasApplication(archive)
  THEN 0
  ELSE DrainableIngressTurnReachRank(archive)

ExactDecisionRequestIngressLaneRank(archive, request) ==
  <<ExactDecisionRequestIngressLanePosition(archive, request),
    ExactDecisionRequestIngressSourcePosition(archive, request)>>

ExactDecisionRequestIngressSelectorRank(archive, request) ==
  <<ExactDecisionRequestIngressPriorityDebt(archive),
    ExactDecisionRequestIngressLaneRank(archive, request)>>

ExactDecisionRequestIngressReachSelectorRank(archive, request) ==
  <<ExactDecisionRequestIngressReachRank(archive),
    ExactDecisionRequestIngressSelectorRank(archive, request)>>

ExactDecisionRequestIngressCapacityRank(archive, request) ==
  <<ExactDecisionRequestIngressTargetServeCapacityDebt(archive, request),
    ExactDecisionRequestIngressReachSelectorRank(archive, request)>>

ExactDecisionRequestIngressRank(archive, request) ==
  <<ExactDecisionRequestIngressModeRank(archive),
    ExactDecisionRequestIngressCapacityRank(archive, request)>>

ExactDecisionRequestIngressLaneCarrier ==
  Nat \X Nat
ExactDecisionRequestIngressSelectorCarrier ==
  Nat \X ExactDecisionRequestIngressLaneCarrier
ExactDecisionRequestIngressReachSelectorCarrier ==
  Nat \X ExactDecisionRequestIngressSelectorCarrier
ExactDecisionRequestIngressCapacityCarrier ==
  Nat \X ExactDecisionRequestIngressReachSelectorCarrier
ExactDecisionRequestIngressRankCarrier ==
  (0..1) \X ExactDecisionRequestIngressCapacityCarrier

ExactDecisionRequestIngressLaneOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)

ExactDecisionRequestIngressSelectorOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    ExactDecisionRequestIngressLaneOrdering,
    Nat, ExactDecisionRequestIngressLaneCarrier)

ExactDecisionRequestIngressReachSelectorOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    ExactDecisionRequestIngressSelectorOrdering,
    Nat, ExactDecisionRequestIngressSelectorCarrier)

ExactDecisionRequestIngressCapacityOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    ExactDecisionRequestIngressReachSelectorOrdering,
    Nat, ExactDecisionRequestIngressReachSelectorCarrier)

ExactDecisionRequestIngressRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    ExactDecisionRequestIngressCapacityOrdering,
    0..1, ExactDecisionRequestIngressCapacityCarrier)

THEOREM ExactDecisionRequestIngressRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    ExactDecisionRequestIngressRankOrdering,
    ExactDecisionRequestIngressRankCarrier)
PROOF
  <1>1. IsWellFoundedOn(
           ExactDecisionRequestIngressLaneOrdering,
           ExactDecisionRequestIngressLaneCarrier)
    BY NatLessThanWellFounded, WFLexPairOrdering
       DEF ExactDecisionRequestIngressLaneOrdering,
           ExactDecisionRequestIngressLaneCarrier
  <1>2. IsWellFoundedOn(
           ExactDecisionRequestIngressSelectorOrdering,
           ExactDecisionRequestIngressSelectorCarrier)
    BY NatLessThanWellFounded, <1>1, WFLexPairOrdering
       DEF ExactDecisionRequestIngressSelectorOrdering,
           ExactDecisionRequestIngressSelectorCarrier
  <1>3. IsWellFoundedOn(
           ExactDecisionRequestIngressReachSelectorOrdering,
           ExactDecisionRequestIngressReachSelectorCarrier)
    BY NatLessThanWellFounded, <1>2, WFLexPairOrdering
       DEF ExactDecisionRequestIngressReachSelectorOrdering,
           ExactDecisionRequestIngressReachSelectorCarrier
  <1>4. IsWellFoundedOn(
           ExactDecisionRequestIngressCapacityOrdering,
           ExactDecisionRequestIngressCapacityCarrier)
    BY NatLessThanWellFounded, <1>3, WFLexPairOrdering
       DEF ExactDecisionRequestIngressCapacityOrdering,
           ExactDecisionRequestIngressCapacityCarrier
  <1> QED BY NatLessThanWellFounded, IsWellFoundedOnSubset,
       <1>4, WFLexPairOrdering, Isa
       DEF ExactDecisionRequestIngressRankOrdering,
           ExactDecisionRequestIngressRankCarrier

THEOREM ExactDecisionRequestIngressPriorityDebtIsNatural ==
  \A archive \in ValidatorIds:
    AsyncStrongTypeInvariant
      => /\ IsFiniteSet(
              ExactDecisionRequestIngressPriorityOwners(archive))
         /\ ExactDecisionRequestIngressPriorityDebt(archive) \in Nat
BY FS_Product, FS_Interval, FS_Subset, FS_CardinalityType, Isa
   DEF ExactDecisionRequestIngressPriorityOwners,
       ExactDecisionRequestIngressPriorityDebt,
       AsyncStrongTypeInvariant, AsyncConfiguration

THEOREM ExactDecisionRequestIngressServeCapacityDebtIsNatural ==
  \A archive \in ValidatorIds:
    AsyncStrongTypeInvariant
      => ExactDecisionRequestIngressServeCapacityDebt(archive) \in Nat
BY AsyncStrongTypeProjectsAsyncType, SMT
   DEF ExactDecisionRequestIngressServeCapacityDebt,
       AsyncIoQueueDepth, AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant, AsyncIoTypeInvariant,
       AsyncIoContentTypeInvariant, AsyncConfiguration

THEOREM ExactDecisionRequestIngressLanePositionIsEarliestOccurrence ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    => /\ ExactDecisionRequestIngressLaneIndices(archive, request) # {}
       /\ ExactDecisionRequestIngressLanePosition(archive, request)
            \in ExactDecisionRequestIngressLaneIndices(archive, request)
       /\ ExactDecisionRequestIngressLanePosition(archive, request)
            \in 1..Len(
                 IngressLane(
                   archive, IngressResourceSource(request)))
       /\ IngressLane(
            archive, IngressResourceSource(request))[
              ExactDecisionRequestIngressLanePosition(
                archive, request)] = request
       /\ \A other \in
              ExactDecisionRequestIngressLaneIndices(archive, request):
            ExactDecisionRequestIngressLanePosition(archive, request)
              <= other
PROOF
  <1>1. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                AsyncStrongTypeInvariant,
                ExactDecisionRequestIngressLaneResidual(
                  node, qc, archive, request)
         PROVE /\ ExactDecisionRequestIngressLaneIndices(
                    archive, request) # {}
               /\ ExactDecisionRequestIngressLanePosition(
                    archive, request)
                    \in ExactDecisionRequestIngressLaneIndices(
                          archive, request)
               /\ ExactDecisionRequestIngressLanePosition(
                    archive, request)
                    \in 1..Len(
                         IngressLane(
                           archive, IngressResourceSource(request)))
               /\ IngressLane(
                    archive, IngressResourceSource(request))[
                      ExactDecisionRequestIngressLanePosition(
                        archive, request)] = request
               /\ \A other \in
                      ExactDecisionRequestIngressLaneIndices(
                        archive, request):
                    ExactDecisionRequestIngressLanePosition(
                      archive, request) <= other
    <2> DEFINE Indices ==
           ExactDecisionRequestIngressLaneIndices(archive, request)
    <2>1. PICK witness \in
                    1..Len(
                         IngressLane(
                           archive, IngressResourceSource(request))):
             IngressLane(
               archive, IngressResourceSource(request))[witness] = request
      BY <1>1
         DEF ExactDecisionRequestIngressLaneResidual,
             ExactDecisionRequestIngressOwned,
             ExactDecisionBodyHoldingAlias,
             ExactDecisionActiveRequestOwner,
             ExactDecisionServiceSource,
             SequenceSet
    <2>2. /\ witness \in Indices
           /\ witness \in Nat
           /\ Indices # {}
      BY <2>1, FS_EmptySet, Isa
         DEF Indices, ExactDecisionRequestIngressLaneIndices
    <2>3. \E least \in Nat:
             /\ least \in Indices
             /\ \A prior \in 0..(least - 1): prior \notin Indices
      BY <2>2, SmallestNatural, SMTT(30)
    <2>4. PICK least \in Nat:
             /\ least \in Indices
             /\ \A prior \in 0..(least - 1): prior \notin Indices
      BY <2>3
    <2>5. \A other \in Indices: least <= other
      BY <2>4, SMT
         DEF Indices, ExactDecisionRequestIngressLaneIndices
    <2>6. /\ ExactDecisionRequestIngressLanePosition(
                  archive, request) \in Indices
           /\ \A other \in Indices:
                ExactDecisionRequestIngressLanePosition(
                  archive, request) <= other
      BY <2>4, <2>5, Zenon
         DEF ExactDecisionRequestIngressLanePosition, Indices
    <2> QED BY <2>2, <2>6
         DEF Indices, ExactDecisionRequestIngressLaneIndices
  <1> QED BY <1>1

THEOREM ExactDecisionRequestIngressRankComponentsAreTyped ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    => /\ archive \in ValidatorIds
       /\ ExactDecisionRequestIngressModeRank(archive) \in 0..1
       /\ ExactDecisionRequestIngressCausalDebt(archive) \in 0..1
       /\ ExactDecisionRequestIngressServeCapacityDebt(archive) \in Nat
       /\ ExactDecisionRequestIngressTargetServeCapacityDebt(
            archive, request) \in Nat
       /\ ExactDecisionRequestIngressPriorityDebt(archive) \in Nat
       /\ ExactDecisionRequestIngressLanePosition(archive, request)
            \in Nat \ {0}
       /\ ExactDecisionRequestIngressSourcePosition(archive, request)
            \in Nat \ {0}
       /\ ExactDecisionRequestIngressReachRank(archive) \in Nat
BY AsyncStrongTypeProjectsAsyncType,
   AsyncCurrentResponsiveVotersAreValidators,
   ExactDecisionRequestIngressPriorityDebtIsNatural,
   ExactDecisionRequestIngressServeCapacityDebtIsNatural,
   ExactDecisionRequestIngressLanePositionIsEarliestOccurrence,
   CandidateSequenceIndexIsPosition,
   DrainableIngressTurnReachRankIsNatural, IsaT(180)
   DEF ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressCausalDebt,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionServeLifecycleIdentity,
       AsyncServeLiveReservationOwned, AsyncServeJobQueued,
       ExactDecisionRequestIngressSourcePosition,
       IngressSourceServiceRank,
       ExactDecisionRequestIngressReachRank,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIngressTypeInvariant, AsyncIngressTopologyTypeInvariant,
       AsyncIngressContentTypeInvariant,
       IngressResourceSource, IngressLane, SequenceSet

THEOREM ExactDecisionRequestIngressRankInCarrier ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    => ExactDecisionRequestIngressRank(archive, request)
         \in ExactDecisionRequestIngressRankCarrier
BY ExactDecisionRequestIngressRankComponentsAreTyped, Isa
   DEF ExactDecisionRequestIngressRank,
       ExactDecisionRequestIngressCapacityRank,
       ExactDecisionRequestIngressReachSelectorRank,
       ExactDecisionRequestIngressSelectorRank,
       ExactDecisionRequestIngressLaneRank,
       ExactDecisionRequestIngressRankCarrier,
       ExactDecisionRequestIngressCapacityCarrier,
       ExactDecisionRequestIngressReachSelectorCarrier,
       ExactDecisionRequestIngressSelectorCarrier,
       ExactDecisionRequestIngressLaneCarrier

(***************************************************************************
Admission-lifecycle rank.

The old per-lane rank remains the nested runner component.  The outer rank
now follows the immutable Serve lifecycle identity across the atomic hidden
admission cut, its logical future-slot ticket, the frozen I/O and ingress
predecessor prefixes, the queued Serve occurrence, and cached-output replay.
For every target occurrence, the outer predecessor set snapshots all smaller
ingress ordinals and each owner's immutable pre-cutoff source prefixes.  The
singular off-queue Rust Serve barrier additionally contributes its frozen
physical I/O jobs.  Prefix service, earlier-owner drain, I/O service, and
barrier materialization each lower the outer component before any change to
the target's nested rank is considered.  The target's separate immutable
ingress ordinal blocks Reserve/Advance from installing a later barrier until
this occurrence drains; a duplicate coalesces behind the same owner.  A
tombstone is not requester success: it only owns exact response bytes.  After
packet loss the active request retransmits with the same logical identity,
reaches the tombstone through ordinary ingress, and re-emits those bytes
without recreating a Serve lifecycle.
***************************************************************************)

ExactDecisionRequestLifecycleGoal(node, qc, archive, request) ==
  ExactDecisionRequestIngressGoal(
    node, qc, archive, request)

ExactDecisionRequestLifecycleResidual(
    node, qc, archive, request) ==
  ExactDecisionRequestIngressLaneResidual(
    node, qc, archive, request)

ExactDecisionRequestLifecycleStage(
    node, qc, archive, request) ==
  IF ExactDecisionRequestLifecycleGoal(
       node, qc, archive, request)
  THEN 0
  ELSE IF ExactDecisionServeTombstoneOwned(
            node, qc, archive, request)
       THEN 1
       ELSE IF ExactDecisionServeAdmissionOwned(
                 archive, request)
            THEN 2
            ELSE 3

ExactDecisionRequestLifecycleFrozenPredecessorSet(
    archive, request) ==
  LET identity ==
        ExactDecisionServeLifecycleIdentity(archive, request)
  IN ({"Io"} \X AsyncServeFrozenPredecessorSet(
                    archive, identity))
       \cup
     ({"Ingress"} \X
        AsyncServeIngressAdmissionPredecessorDebtSlots(
          archive, identity))
       \cup
     AsyncServePreexistingIngressOwnerPredecessorDebtSet(
       archive, identity)
       \cup
     AsyncServePreexistingIngressBarrierPredecessorDebtSet(
       archive, identity)

ExactDecisionRequestLifecycleFrozenPredecessorDebt(
    archive, request) ==
  Cardinality(
    ExactDecisionRequestLifecycleFrozenPredecessorSet(
      archive, request))

ExactDecisionRequestIngressZeroLaneRank == <<0, 0>>
ExactDecisionRequestIngressZeroSelectorRank ==
  <<0, ExactDecisionRequestIngressZeroLaneRank>>
ExactDecisionRequestIngressZeroReachSelectorRank ==
  <<0, ExactDecisionRequestIngressZeroSelectorRank>>
ExactDecisionRequestIngressZeroCapacityRank ==
  <<0, ExactDecisionRequestIngressZeroReachSelectorRank>>
ExactDecisionRequestIngressZeroRank ==
  <<0, ExactDecisionRequestIngressZeroCapacityRank>>

ExactDecisionRequestLifecycleNestedIngressRank(
    node, qc, archive, request) ==
  IF ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  THEN ExactDecisionRequestIngressRank(archive, request)
  ELSE ExactDecisionRequestIngressZeroRank

ExactDecisionRequestLifecycleIngressRank(
    node, qc, archive, request) ==
  <<ExactDecisionRequestLifecycleStage(
      node, qc, archive, request),
    <<ExactDecisionRequestLifecycleFrozenPredecessorDebt(
        archive, request),
      ExactDecisionRequestLifecycleNestedIngressRank(
        node, qc, archive, request)>>>>

ExactDecisionRequestLifecycleDebtCarrier ==
  Nat \X ExactDecisionRequestIngressRankCarrier

ExactDecisionRequestLifecycleIngressRankCarrier ==
  (0..3) \X ExactDecisionRequestLifecycleDebtCarrier

ExactDecisionRequestLifecycleDebtOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    ExactDecisionRequestIngressRankOrdering,
    Nat, ExactDecisionRequestIngressRankCarrier)

ExactDecisionRequestLifecycleIngressRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    ExactDecisionRequestLifecycleDebtOrdering,
    0..3, ExactDecisionRequestLifecycleDebtCarrier)

THEOREM ExactDecisionRequestLifecycleIngressRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    ExactDecisionRequestLifecycleIngressRankOrdering,
    ExactDecisionRequestLifecycleIngressRankCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset,
   ExactDecisionRequestIngressRankOrderingIsWellFounded,
   WFLexPairOrdering, Isa
   DEF ExactDecisionRequestLifecycleIngressRankOrdering,
       ExactDecisionRequestLifecycleIngressRankCarrier,
       ExactDecisionRequestLifecycleDebtOrdering,
       ExactDecisionRequestLifecycleDebtCarrier

ExactDecisionRequestIngressAtRank(
    node, qc, archive, request, rank) ==
  /\ AsyncStrongTypeInvariant
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ rank = ExactDecisionRequestIngressRank(archive, request)
  /\ rank \in ExactDecisionRequestIngressRankCarrier

THEOREM ExactDecisionRequestIngressRankCoversEveryLaneResidual ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    => \E rank \in ExactDecisionRequestIngressRankCarrier:
         ExactDecisionRequestIngressAtRank(
           node, qc, archive, request, rank)
BY ExactDecisionRequestIngressRankInCarrier
   DEF ExactDecisionRequestIngressAtRank

(***************************************************************************
Exact blocked-case decomposition.

The guards are ordered so the cases are disjoint within each runner mode.
The ordinary runner first owes its phase prefix, then target-specific Serve
capacity, then claimed-response/request-fenced priority, then the source and
lane positions.  Completion causal debt is absent because it does not gate
either exact reply-request kind.  The historical server has only
target-specific Serve capacity and its minimum source/lane positions.  Lane
position means the least equal request occurrence, so this decomposition does
not assume that retransmitted request values are unique.
***************************************************************************)

THEOREM ExactDecisionNormalRequestIngressDrainability ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ ~NodeHasApplication(archive)
    => (IngressItemCanDrain(archive, request)
          <=> ExactServeIngressCanAdvance(archive, request))
BY IsaT(180)
   DEF ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       IngressItemCanDrain,
       AsyncServeLifecycleDrainRequired,
       CertifiedRequestAuthorized, CertifiedRequestAuthority

THEOREM ExactDecisionHistoricalRequestIngressDrainability ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ NodeHasApplication(archive)
    => (HistoricalIngressItemCanDrain(archive, request)
          <=> ExactServeIngressCanAdvance(archive, request))
BY IsaT(180)
   DEF ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       HistoricalIngressItemCanDrain,
       AsyncServeLifecycleDrainRequired,
       CertifiedRequestAuthorized, CertifiedRequestAuthority

ExactDecisionNormalRequestIngressPhaseBlocked(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ ~NodeHasApplication(archive)
  /\ ~(/\ asyncRunnerPhase[archive] = "Ingress"
       /\ asyncRunnerBudget[archive] > 0)

ExactDecisionNormalRequestIngressContinuationPrefixBlocked(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ ~NodeHasApplication(archive)
  /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(archive)

ExactDecisionRequestIngressContinuationPrefixCleared(archive) ==
  \/ NodeHasApplication(archive)
  \/ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(archive)

ExactDecisionRequestIngressContinuationPrefixGoal(
    node, qc, archive, request) ==
  \/ ExactDecisionRequestLifecycleGoal(node, qc, archive, request)
  \/ ExactDecisionRequestIngressContinuationPrefixCleared(archive)

ExactDecisionRequestIngressContinuationPrefixAtBudget(
    node, qc, archive, request, record, status, budget) ==
  /\ ExactDecisionNormalRequestIngressContinuationPrefixBlocked(
       node, qc, archive, request)
  /\ record =
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord(
         archive)
  /\ status = record.status
  /\ status \in {"Reserved", "Materialized"}
  /\ AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
       archive, record.identity, record.ordinal, record.address.stage,
       status, budget)

ExactDecisionRequestIngressContinuationPrefixClosureProperty(
    specification) ==
  specification
    => \A node, qc, archive, request:
         ExactDecisionNormalRequestIngressContinuationPrefixBlocked(
           node, qc, archive, request)
           ~> ExactDecisionRequestIngressContinuationPrefixGoal(
                node, qc, archive, request)

THEOREM AsyncSpecClosesExactDecisionRequestIngressContinuationPrefix ==
  \A initialContext:
    ExactDecisionRequestIngressContinuationPrefixClosureProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesVoterCandidateProducerContinuationResolutionClosure,
   AsyncSpecAlwaysUsesFixedResponsiveVoters, PTL, IsaT(600)
   DEF ExactDecisionRequestIngressContinuationPrefixClosureProperty,
       ExactDecisionNormalRequestIngressContinuationPrefixBlocked,
       ExactDecisionRequestIngressContinuationPrefixGoal,
       ExactDecisionRequestIngressContinuationPrefixCleared,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       AsyncVoterCandidateProducerContinuationResolutionClosureProperty,
       AsyncVoterCandidateProducerContinuationEpisodePending

(***************************************************************************
The exact ingress rank does not service this prefix.  `RunNodeWork` first
drains only producer continuations whose inherited lifecycle ordinal is no
later than the immutable ingress scheduler cut.  A later continuation remains
durably owned but cannot preempt this request.  Readiness excludes the finite
earlier/equal prefix, whose frozen rank supplies the source witness below and
whose voter-runner composite rank supplies temporal closure.  Replay itself
is not classified as request-ingress progress.
***************************************************************************)
THEOREM ExactDecisionRequestIngressContinuationPrefixHasFiniteRankWitness ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ ExactDecisionNormalRequestIngressContinuationPrefixBlocked(
         node, qc, archive, request)
    => \E record \in AsyncCandidateProducerContinuationRecordSet,
          status \in {"Reserved", "Materialized"},
          budget
            \in AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
         ExactDecisionRequestIngressContinuationPrefixAtBudget(
           node, qc, archive, request, record, status, budget)
BY CandidateProducerContinuationFrozenPrefixRankIsFiniteAndPositive,
   IsaT(600)
   DEF ExactDecisionRequestIngressContinuationPrefixAtBudget,
       ExactDecisionNormalRequestIngressContinuationPrefixBlocked,
       AsyncCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
       AsyncCandidateProducerContinuationRunnerMayPrecedeIngress,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRecordSet,
       AsyncCandidateProducerContinuationRecord,
       AsyncCandidateProducerContinuationSourceClasses,
       AsyncCandidateProducerContinuations,
       AsyncControlServiceStateTypeInvariant

ExactDecisionNormalRequestIngressCausalBlocked(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ ~NodeHasApplication(archive)
  /\ asyncRunnerPhase[archive] = "Ingress"
  /\ asyncRunnerBudget[archive] > 0
  /\ CompletionCausalAdmissionDebt(archive)

ExactDecisionNormalRequestIngressServeCapacityBlocked(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ ~NodeHasApplication(archive)
  /\ asyncRunnerPhase[archive] = "Ingress"
  /\ asyncRunnerBudget[archive] > 0
  /\ ~IngressItemCanDrain(archive, request)

ExactDecisionNormalRequestIngressSelectable(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ ~NodeHasApplication(archive)
  /\ asyncRunnerPhase[archive] = "Ingress"
  /\ asyncRunnerBudget[archive] > 0
  /\ IngressItemCanDrain(archive, request)

ExactDecisionNormalRequestIngressClaimedPriorityBlocked(
    node, qc, archive, request) ==
  /\ ExactDecisionNormalRequestIngressSelectable(
       node, qc, archive, request)
  /\ DrainableClaimedResponseReadyIndices(archive) # {}

ExactDecisionNormalRequestIngressFencedPriorityBlocked(
    node, qc, archive, request) ==
  /\ ExactDecisionNormalRequestIngressSelectable(
       node, qc, archive, request)
  /\ DrainableClaimedResponseReadyIndices(archive) = {}
  /\ DrainableRequestFencedCompletionReadyIndices(archive) # {}

ExactDecisionNormalRequestIngressSourcePositionBlocked(
    node, qc, archive, request) ==
  /\ ExactDecisionNormalRequestIngressSelectable(
       node, qc, archive, request)
  /\ DrainableClaimedResponseReadyIndices(archive) = {}
  /\ DrainableRequestFencedCompletionReadyIndices(archive) = {}
  /\ FirstDrainableIngressIndex(archive)
       < ExactDecisionRequestIngressSourcePosition(archive, request)

ExactDecisionNormalRequestIngressLanePositionBlocked(
    node, qc, archive, request) ==
  /\ ExactDecisionNormalRequestIngressSelectable(
       node, qc, archive, request)
  /\ DrainableClaimedResponseReadyIndices(archive) = {}
  /\ DrainableRequestFencedCompletionReadyIndices(archive) = {}
  /\ FirstDrainableIngressIndex(archive)
       = ExactDecisionRequestIngressSourcePosition(archive, request)
  /\ SelectedIngressLaneIndex(
       archive, FirstDrainableIngressIndex(archive))
       < ExactDecisionRequestIngressLanePosition(archive, request)

ExactDecisionHistoricalRequestIngressServeCapacityBlocked(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ NodeHasApplication(archive)
  /\ ~HistoricalIngressItemCanDrain(archive, request)

ExactDecisionHistoricalRequestIngressSelectable(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ NodeHasApplication(archive)
  /\ HistoricalIngressItemCanDrain(archive, request)

ExactDecisionHistoricalRequestIngressSourcePositionBlocked(
    node, qc, archive, request) ==
  /\ ExactDecisionHistoricalRequestIngressSelectable(
       node, qc, archive, request)
  /\ FirstHistoricalDrainableIngressIndex(archive)
       < ExactDecisionRequestIngressSourcePosition(archive, request)

ExactDecisionHistoricalRequestIngressLanePositionBlocked(
    node, qc, archive, request) ==
  /\ ExactDecisionHistoricalRequestIngressSelectable(
       node, qc, archive, request)
  /\ FirstHistoricalDrainableIngressIndex(archive)
       = ExactDecisionRequestIngressSourcePosition(archive, request)
  /\ HistoricalSelectedIngressLaneIndex(
       archive, FirstHistoricalDrainableIngressIndex(archive))
       < ExactDecisionRequestIngressLanePosition(archive, request)

ExactDecisionRequestIngressConcreteBlockedCase(
    node, qc, archive, request) ==
  \/ ExactDecisionNormalRequestIngressContinuationPrefixBlocked(
       node, qc, archive, request)
  \/ ExactDecisionNormalRequestIngressPhaseBlocked(
       node, qc, archive, request)
  \/ ExactDecisionNormalRequestIngressServeCapacityBlocked(
       node, qc, archive, request)
  \/ ExactDecisionNormalRequestIngressClaimedPriorityBlocked(
       node, qc, archive, request)
  \/ ExactDecisionNormalRequestIngressFencedPriorityBlocked(
       node, qc, archive, request)
  \/ ExactDecisionNormalRequestIngressSourcePositionBlocked(
       node, qc, archive, request)
  \/ ExactDecisionNormalRequestIngressLanePositionBlocked(
       node, qc, archive, request)
  \/ ExactDecisionHistoricalRequestIngressServeCapacityBlocked(
       node, qc, archive, request)
  \/ ExactDecisionHistoricalRequestIngressSourcePositionBlocked(
       node, qc, archive, request)
  \/ ExactDecisionHistoricalRequestIngressLanePositionBlocked(
       node, qc, archive, request)

THEOREM ExactDecisionRequestIngressBlockedCaseDecomposition ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    => (ExactDecisionRequestIngressRunnerBlocked(
          node, qc, archive, request)
          <=> ExactDecisionRequestIngressConcreteBlockedCase(
                node, qc, archive, request))
BY ExactDecisionNormalRequestIngressDrainability,
   ExactDecisionHistoricalRequestIngressDrainability,
   ExactDecisionRequestIngressLanePositionIsEarliestOccurrence,
   ExactDecisionRequestIngressRankComponentsAreTyped,
   FirstDrainableSourceNeverFollowsAnotherDrainableSource,
   FirstDrainableIngressIndexIsDrainable,
   FirstDrainableIngressLaneIndexIsDrainable, IsaT(300)
   DEF ExactDecisionRequestIngressRunnerBlocked,
       ExactDecisionRequestIngressRunnerReady,
       ExactDecisionNormalRequestIngressRunnerReady,
       ExactDecisionHistoricalRequestIngressRunnerReady,
       ExactDecisionRequestIngressConcreteBlockedCase,
       ExactDecisionNormalRequestIngressContinuationPrefixBlocked,
       ExactDecisionNormalRequestIngressPhaseBlocked,
       ExactDecisionNormalRequestIngressServeCapacityBlocked,
       ExactDecisionNormalRequestIngressSelectable,
       ExactDecisionNormalRequestIngressClaimedPriorityBlocked,
       ExactDecisionNormalRequestIngressFencedPriorityBlocked,
       ExactDecisionNormalRequestIngressSourcePositionBlocked,
       ExactDecisionNormalRequestIngressLanePositionBlocked,
       ExactDecisionHistoricalRequestIngressServeCapacityBlocked,
       ExactDecisionHistoricalRequestIngressSelectable,
       ExactDecisionHistoricalRequestIngressSourcePositionBlocked,
       ExactDecisionHistoricalRequestIngressLanePositionBlocked,
       ExactDecisionRequestIngressLanePosition,
       ExactDecisionRequestIngressLaneIndices,
       ExactDecisionRequestIngressSourcePosition,
       HistoricalDrainableIngressIndices,
       HistoricalDrainableIngressLaneIndices,
       FirstHistoricalDrainableIngressIndex,
       FirstHistoricalDrainableIngressLaneIndex,
       HistoricalSelectedIngressLaneIndex,
       HistoricalSelectedIngressItemAt,
       DrainableIngressIndices, DrainableIngressLaneIndices,
       DrainableClaimedResponseReadyIndices,
       DrainableRequestFencedCompletionReadyIndices,
       SelectedIngressLaneIndex, SelectedIngressItemAt,
       IngressResourceSource, IngressLane, SequenceSet

(***************************************************************************
Sound local rank edges.

The structural lemma below is the only generic decrease rule: one component
strictly decreases while every earlier component is unchanged.  The concrete
local runner-turn and I/O-worker leaves then establish two actual owner
transfers.  They are deliberately stated only while the exact lane residual
persists; the selected exact drain instead reaches the goal by
`ExactDecisionRequestIngressRunnerActionCreatesGoal`.
***************************************************************************)

ExactDecisionRequestIngressStrictComponentDecrease(archive, request) ==
  \/ ExactDecisionRequestIngressModeRank(archive)'
       < ExactDecisionRequestIngressModeRank(archive)
  \/ /\ ExactDecisionRequestIngressModeRank(archive)'
          = ExactDecisionRequestIngressModeRank(archive)
     /\ ExactDecisionRequestIngressTargetServeCapacityDebt(
          archive, request)'
          < ExactDecisionRequestIngressTargetServeCapacityDebt(
              archive, request)
  \/ /\ ExactDecisionRequestIngressModeRank(archive)'
          = ExactDecisionRequestIngressModeRank(archive)
     /\ ExactDecisionRequestIngressTargetServeCapacityDebt(
          archive, request)'
          = ExactDecisionRequestIngressTargetServeCapacityDebt(
              archive, request)
     /\ ExactDecisionRequestIngressReachRank(archive)'
          < ExactDecisionRequestIngressReachRank(archive)
  \/ /\ ExactDecisionRequestIngressModeRank(archive)'
          = ExactDecisionRequestIngressModeRank(archive)
     /\ ExactDecisionRequestIngressTargetServeCapacityDebt(
          archive, request)'
          = ExactDecisionRequestIngressTargetServeCapacityDebt(
              archive, request)
     /\ ExactDecisionRequestIngressReachRank(archive)'
          = ExactDecisionRequestIngressReachRank(archive)
     /\ ExactDecisionRequestIngressPriorityDebt(archive)'
          < ExactDecisionRequestIngressPriorityDebt(archive)
  \/ /\ ExactDecisionRequestIngressModeRank(archive)'
          = ExactDecisionRequestIngressModeRank(archive)
     /\ ExactDecisionRequestIngressTargetServeCapacityDebt(
          archive, request)'
          = ExactDecisionRequestIngressTargetServeCapacityDebt(
              archive, request)
     /\ ExactDecisionRequestIngressReachRank(archive)'
          = ExactDecisionRequestIngressReachRank(archive)
     /\ ExactDecisionRequestIngressPriorityDebt(archive)'
          = ExactDecisionRequestIngressPriorityDebt(archive)
     /\ ExactDecisionRequestIngressLanePosition(archive, request)'
          < ExactDecisionRequestIngressLanePosition(archive, request)
  \/ /\ ExactDecisionRequestIngressModeRank(archive)'
          = ExactDecisionRequestIngressModeRank(archive)
     /\ ExactDecisionRequestIngressTargetServeCapacityDebt(
          archive, request)'
          = ExactDecisionRequestIngressTargetServeCapacityDebt(
              archive, request)
     /\ ExactDecisionRequestIngressReachRank(archive)'
          = ExactDecisionRequestIngressReachRank(archive)
     /\ ExactDecisionRequestIngressPriorityDebt(archive)'
          = ExactDecisionRequestIngressPriorityDebt(archive)
     /\ ExactDecisionRequestIngressLanePosition(archive, request)'
          = ExactDecisionRequestIngressLanePosition(archive, request)
     /\ ExactDecisionRequestIngressSourcePosition(archive, request)'
          < ExactDecisionRequestIngressSourcePosition(archive, request)

THEOREM ExactDecisionRequestIngressStrictComponentLowersRank ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ AsyncStrongTypeInvariant'
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)'
    /\ ExactDecisionRequestIngressStrictComponentDecrease(
         archive, request)
    => <<ExactDecisionRequestIngressRank(archive, request)',
          ExactDecisionRequestIngressRank(archive, request)>>
         \in ExactDecisionRequestIngressRankOrdering
BY ExactDecisionRequestIngressRankInCarrier, Isa
   DEF ExactDecisionRequestIngressStrictComponentDecrease,
       ExactDecisionRequestIngressRank,
       ExactDecisionRequestIngressCapacityRank,
       ExactDecisionRequestIngressReachSelectorRank,
       ExactDecisionRequestIngressSelectorRank,
       ExactDecisionRequestIngressLaneRank,
       ExactDecisionRequestIngressRankOrdering,
       ExactDecisionRequestIngressCapacityOrdering,
       ExactDecisionRequestIngressReachSelectorOrdering,
       ExactDecisionRequestIngressSelectorOrdering,
       ExactDecisionRequestIngressLaneOrdering,
       LexPairOrdering, OpToRel

THEOREM ExactDecisionRequestIngressStutterPreservesRank ==
  \A node, qc, archive, request:
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ UNCHANGED AsyncAllVars
    => /\ ExactDecisionRequestIngressLaneResidual(
            node, qc, archive, request)'
       /\ ExactDecisionRequestIngressRank(archive, request)'
            = ExactDecisionRequestIngressRank(archive, request)
BY Isa
   DEF ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressRank,
       ExactDecisionRequestIngressCapacityRank,
       ExactDecisionRequestIngressReachSelectorRank,
       ExactDecisionRequestIngressSelectorRank,
       ExactDecisionRequestIngressLaneRank,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionServeLifecycleIdentity,
       AsyncServeLiveReservationOwned, AsyncServeJobQueued,
       ExactDecisionRequestIngressServeCapacityDebt,
       ExactDecisionRequestIngressPriorityDebt,
       ExactDecisionRequestIngressPriorityOwners,
       ExactDecisionRequestIngressLanePosition,
       ExactDecisionRequestIngressLaneIndices,
       ExactDecisionRequestIngressSourcePosition,
       IngressSourceServiceRank,
       ExactDecisionRequestIngressReachRank,
       AsyncAllVars, AsyncSchedulerVars, AsyncProducerVars, vars

THEOREM ExactDecisionRequestIngressLaneOwnsPriorityTicket ==
  \A node, qc, archive, request:
    LET identity ==
          ExactDecisionServeLifecycleIdentity(archive, request)
    IN /\ AsyncStrongTypeInvariant
       /\ ExactDecisionRequestIngressLaneResidual(
            node, qc, archive, request)
       => identity
            \in AsyncServeIngressLifecycleOwnerIdentities(archive)
BY Isa
   DEF ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionServeLifecycleIdentity,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionIdentities,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant

THEOREM ExactDecisionRequestIngressTicketDisablesLaterRunnerWork ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    => /\ ~LocalAdmissionStep(archive)
       /\ ~SerializedRuntimeStep(archive)
       /\ ~EnqueueIoLocalControlWork(archive)
       /\ ~CommitCertificateDiscoveryStepWork(archive)
BY ExactDecisionRequestIngressLaneOwnsPriorityTicket,
   AsyncServeIngressTicketExcludesLaterLocalWork

THEOREM ExactDecisionRequestIngressTargetOnlyTurnStrictlyLowersRank ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ AsyncServeIngressTargetOnlyTurn(archive)
    /\ AsyncNext
    => /\ ExactDecisionRequestIngressLaneResidual(
            node, qc, archive, request)'
       /\ ExactDecisionRequestIngressStrictComponentDecrease(
            archive, request)
       /\ <<ExactDecisionRequestIngressRank(archive, request)',
             ExactDecisionRequestIngressRank(archive, request)>>
            \in ExactDecisionRequestIngressRankOrdering
BY AsyncStrongTypeProjectsAsyncType,
   ExactTicketTurnDecreasesDrainableIngressTurnReach,
   ExactDecisionRequestIngressStrictComponentLowersRank, IsaT(300)
   DEF ExactDecisionRequestIngressStrictComponentDecrease,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionRequestIngressReachRank,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressGoal,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       AsyncServeIngressTargetOnlyTurn,
       AsyncNext

ExactDecisionRequestIngressCausalAdmissionAction(archive) ==
  /\ ~NodeHasApplication(archive)
  /\ PostGstRunNode(archive)
  /\ LocalAdmissionStep(archive)
  /\ AdmitCausalHead(archive)
  /\ UpdateLocalAdmissionMetadata(archive, "Causal")

THEOREM ExactDecisionRequestIngressCausalAdmissionPersistsAndLowers ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ CompletionCausalAdmissionDebt(archive)
    /\ ExactDecisionRequestIngressCausalAdmissionAction(archive)
    /\ [AsyncNext]_AsyncAllVars
    => /\ ExactDecisionRequestIngressLaneResidual(
            node, qc, archive, request)'
       /\ ExactDecisionRequestIngressStrictComponentDecrease(
            archive, request)
       /\ <<ExactDecisionRequestIngressRank(archive, request)',
             ExactDecisionRequestIngressRank(archive, request)>>
            \in ExactDecisionRequestIngressRankOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   LocalStepDecreasesDrainableIngressTurnReach,
   ExactDecisionRequestIngressStrictComponentLowersRank, IsaT(300)
   DEF ExactDecisionRequestIngressStrictComponentDecrease,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionRequestIngressReachRank,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressGoal,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionServeJobOwned,
       ExactDecisionServeOccurrenceOwned,
       CompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       ExactDecisionRequestIngressCausalAdmissionAction,
       AdmitCausalHead, UpdateLocalAdmissionMetadata,
       IngressResourceSource, IngressLane, SequenceSet

THEOREM ExactDecisionRequestIngressLocalProducerCannotAscendRank ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ PostGstRunNode(archive)
    /\ LocalAdmissionStep(archive)
    /\ [AsyncNext]_AsyncAllVars
    => /\ ExactDecisionRequestIngressLaneResidual(
            node, qc, archive, request)'
       /\ <<ExactDecisionRequestIngressRank(archive, request)',
             ExactDecisionRequestIngressRank(archive, request)>>
            \in ExactDecisionRequestIngressRankOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   LocalStepDecreasesDrainableIngressTurnReach,
   ExactDecisionRequestIngressStrictComponentLowersRank, IsaT(300)
   DEF ExactDecisionRequestIngressStrictComponentDecrease,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionRequestIngressReachRank,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressGoal,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       PostGstRunNode, RunNode, RunNodeWork, LocalAdmissionStep,
       AdmitProducerCompletion, AdmitCausalHead,
       EnqueueCandidate, AsyncAllVars

THEOREM ExactDecisionRequestIngressRuntimeActivationCannotAscendRank ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ PostGstRunNode(archive)
    /\ SerializedRuntimeStep(archive)
    /\ [AsyncNext]_AsyncAllVars
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)'
    => <<ExactDecisionRequestIngressRank(archive, request)',
          ExactDecisionRequestIngressRank(archive, request)>>
         \in ExactDecisionRequestIngressRankOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   RuntimeStepDecreasesDrainableIngressTurnReach,
   ExactDecisionRequestIngressStrictComponentLowersRank, IsaT(300)
   DEF ExactDecisionRequestIngressStrictComponentDecrease,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionRequestIngressReachRank,
       PostGstRunNode, RunNode, RunNodeWork, SerializedRuntimeStep,
       AsyncAllVars

THEOREM ExactDecisionRequestIngressOlderRuntimeInterleaveCannotAscendRank ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ PostGstRunNode(archive)
    /\ SerializedRuntimePrecedesServeIngressStep(archive)
    /\ [AsyncNext]_AsyncAllVars
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)'
    => <<ExactDecisionRequestIngressRank(archive, request)',
          ExactDecisionRequestIngressRank(archive, request)>>
         \in ExactDecisionRequestIngressRankOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   OlderRuntimeInterleaveDecreasesDrainableIngressTurnReach,
   ExactDecisionRequestIngressStrictComponentLowersRank, IsaT(300)
   DEF ExactDecisionRequestIngressStrictComponentDecrease,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionRequestIngressReachRank,
       PostGstRunNode, RunNode, RunNodeWork,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncAllVars

THEOREM ExactDecisionRequestIngressOlderLocalInterleaveCannotAscendRank ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ PostGstRunNode(archive)
    /\ SerializedLocalPrecedesServeIngressStep(archive)
    /\ [AsyncNext]_AsyncAllVars
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)'
    => <<ExactDecisionRequestIngressRank(archive, request)',
          ExactDecisionRequestIngressRank(archive, request)>>
         \in ExactDecisionRequestIngressRankOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   SerializedLocalPredecessorDecreasesDrainableIngressTurnReach,
   ExactDecisionRequestIngressStrictComponentLowersRank, IsaT(300)
   DEF ExactDecisionRequestIngressStrictComponentDecrease,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionRequestIngressReachRank,
       PostGstRunNode, RunNode, RunNodeWork,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncAllVars

THEOREM ExactDecisionRequestIngressIoServiceLowersCapacityDebt ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestIngressServeCapacityDebt(archive) > 0
    /\ ServiceIoWorkerWork(archive)
    => /\ ExactDecisionRequestIngressLaneResidual(
            node, qc, archive, request)'
       /\ ExactDecisionRequestIngressServeCapacityDebt(archive)' + 1
            = ExactDecisionRequestIngressServeCapacityDebt(archive)
BY AsyncStrongTypeProjectsAsyncType,
   AsyncCurrentResponsiveVotersAreValidators,
   ServiceIoWorkerDropsQueueDepth,
   HeadTailProperties, SMT, IsaT(300)
   DEF ExactDecisionRequestIngressServeCapacityDebt,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressGoal,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionServeJobOwned,
       ExactDecisionServeOccurrenceOwned,
       AsyncIoQueueDepth,
       ServiceIoWorkerWork, PublishEphemeralItems,
       IngressResourceSource, IngressLane, SequenceSet

THEOREM ExactDecisionRequestIngressIoServicePersistsAndLowers ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestIngressTargetServeCapacityDebt(
         archive, request) > 0
    /\ ServiceIoWorkerWork(archive)
    /\ [AsyncNext]_AsyncAllVars
    => /\ ExactDecisionRequestIngressLaneResidual(
            node, qc, archive, request)'
       /\ ExactDecisionRequestIngressStrictComponentDecrease(
            archive, request)
       /\ <<ExactDecisionRequestIngressRank(archive, request)',
             ExactDecisionRequestIngressRank(archive, request)>>
            \in ExactDecisionRequestIngressRankOrdering
BY ExactDecisionRequestIngressIoServiceLowersCapacityDebt,
   AsyncBracketNextPreservesStrongTypeInvariant,
   ExactDecisionRequestIngressStrictComponentLowersRank, IsaT(180)
   DEF ExactDecisionRequestIngressStrictComponentDecrease,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionServeLifecycleIdentity,
       AsyncServeLiveReservationOwned, AsyncServeJobQueued

(***************************************************************************
Physical producer diagnostics.

These state predicates retain the three executable counter-transition seams
used by the mutation suite.  They are deliberately not the components of the
final ingress rank.  Completion causal debt does not gate an exact request;
capacity is charged only while this target owns an unmaterialized reservation;
and runner-turn reach precedes volatile selector activation.  Consequently a
local causal producer, unrelated physical Serve refill, or newly active
selector owner may change one of these diagnostic counters without ascending
the target's full rank.  The lifecycle section below instead freezes the
immutable predecessor/ordinal owners and composes their finite service budget
with the target-specific mode/capacity/reach/selector/lane/source rank.
***************************************************************************)

ExactDecisionRequestIngressCausalReplenishmentAction(
    node, qc, archive, request) ==
  /\ AsyncNext
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)'
  /\ ExactDecisionRequestIngressModeRank(archive)'
       = ExactDecisionRequestIngressModeRank(archive)
  /\ ExactDecisionRequestIngressCausalDebt(archive)'
       > ExactDecisionRequestIngressCausalDebt(archive)

ExactDecisionRequestIngressServeReplenishmentAction(
    node, qc, archive, request) ==
  /\ AsyncNext
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)'
  /\ ExactDecisionRequestIngressModeRank(archive)'
       = ExactDecisionRequestIngressModeRank(archive)
  /\ ExactDecisionRequestIngressCausalDebt(archive)'
       = ExactDecisionRequestIngressCausalDebt(archive)
  /\ ExactDecisionRequestIngressServeCapacityDebt(archive)'
       > ExactDecisionRequestIngressServeCapacityDebt(archive)

ExactDecisionRequestIngressPriorityReplenishmentAction(
    node, qc, archive, request) ==
  /\ AsyncNext
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)'
  /\ ExactDecisionRequestIngressModeRank(archive)'
       = ExactDecisionRequestIngressModeRank(archive)
  /\ ExactDecisionRequestIngressCausalDebt(archive)'
       = ExactDecisionRequestIngressCausalDebt(archive)
  /\ ExactDecisionRequestIngressServeCapacityDebt(archive)'
       = ExactDecisionRequestIngressServeCapacityDebt(archive)
  /\ ExactDecisionRequestIngressPriorityDebt(archive)'
       > ExactDecisionRequestIngressPriorityDebt(archive)

ExactDecisionRequestIngressCausalReplenishmentResidual(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ ENABLED
       <<ExactDecisionRequestIngressCausalReplenishmentAction(
           node, qc, archive, request)>>_AsyncAllVars

ExactDecisionRequestIngressServeReplenishmentResidual(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ ENABLED
       <<ExactDecisionRequestIngressServeReplenishmentAction(
           node, qc, archive, request)>>_AsyncAllVars

ExactDecisionRequestIngressPriorityReplenishmentResidual(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestIngressLaneResidual(
       node, qc, archive, request)
  /\ ENABLED
       <<ExactDecisionRequestIngressPriorityReplenishmentAction(
           node, qc, archive, request)>>_AsyncAllVars

(***************************************************************************
Concrete replenishment-producer audit.

The three residuals above are now classified against the executable
`AsyncNext` inventory.  The classifications are intentionally action-local:

  * Completion causal debt can be created by the two Local-admission bit
    setters.  `AsyncStrongTypeInvariant` does not contain the stronger
    reachable-state fact `owed => causal queue nonempty`; without that fact,
    a serialized runtime successor append can also turn an already-owed empty
    queue into Completion debt.  The local Init/Next induction below proves
    the fact for `AsyncSpecAt`, eliminating that successor arm on reachable
    traces while retaining it as an explicit Strong-type counterexample.
  * Serve-capacity debt can be increased only by an ordinary/historical
    request drain, a fresh causal Completion admission, or the one-slot local
    Control producer.  Every such action appends one I/O job.
  * selector priority can gain only a concrete claimed or request-fenced lane
    occurrence.  Network admission can create the claimed occurrence;
    only the same archive's normal, historical-recovery, or historical-server
    runner can change its remaining priority inputs.  The historical-server
    witness is not discarded under `AsyncStrongTypeInvariant`: that invariant
    deliberately permits stale duplicate response occurrences, so removing
    the singleton claim owner can make a remaining duplicate unauthorized,
    drainable, and request-fenced.

Apart from the two Control-producer wrappers, the remaining non-runner arms
(`AsyncSetGST`, `AsyncTick`, historical opening and certificate discovery,
both I/O workers, and `AsyncFaultStep`) frame the relevant producer state.
The pre-GST crash/restart/replay arms are inconsistent with the `gst` fact
carried by the exact lane residual.  The source theorems below therefore
cover every concrete `AsyncNext` arm without asserting that any witness
action is fair or that the number of witness episodes is finite.
***************************************************************************)

ExactDecisionRequestIngressCausalLocalDebtProducerAction(archive) ==
  /\ \/ RunNode(archive)
     \/ RunHistoricalRecoveryNode(archive)
  /\ LocalAdmissionStep(archive)
  /\ CausalQueueNonempty(archive)
  /\ HeadCausalCandidate(archive).class = "Completion"
  /\ ~asyncCausalAdmissionOwed[archive]
  /\ asyncCausalAdmissionOwed'[archive]
  /\ \/ /\ LocalAdmissionCanAdvance(archive)
        /\ SelectedLocalSource(archive) = "Producer"
        /\ AdmitProducerCompletion(archive)
        /\ UpdateLocalAdmissionMetadata(archive, "Producer")
     \/ /\ ~LocalAdmissionCanAdvance(archive)
        /\ RecordBlockedCausalDebt(archive)

ExactDecisionRequestIngressCausalSuccessorProducerAction(archive) ==
  /\ \/ RunNode(archive)
     \/ RunHistoricalRecoveryNode(archive)
  /\ SerializedRuntimeStep(archive)
  /\ asyncCausalAdmissionOwed[archive]
  /\ ~CausalQueueNonempty(archive)
  /\ CompletionCausalAdmissionDebt(archive)'
  /\ \/ DirectTimeoutStep(archive)
     \/ DirectRetransmitStep(archive)
     \/ DeferredTimeoutStep(archive)
     \/ DeferredRetransmitStep(archive)
     \/ FifoRuntimeStep(archive)
     \/ DeferredDrainStep(archive)

ExactDecisionRequestIngressCausalConcreteProducerAction(archive) ==
  \/ ExactDecisionRequestIngressCausalLocalDebtProducerAction(archive)
  \/ ExactDecisionRequestIngressCausalSuccessorProducerAction(archive)

ExactDecisionRequestIngressCausalOwedQueueConsistency(archive) ==
  asyncCausalAdmissionOwed[archive] => CausalQueueNonempty(archive)

ExactDecisionRequestIngressCausalOwedQueueInvariant ==
  \A archive \in ValidatorIds:
    ExactDecisionRequestIngressCausalOwedQueueConsistency(archive)

THEOREM AsyncInitEstablishesExactDecisionRequestIngressCausalOwedQueue ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => ExactDecisionRequestIngressCausalOwedQueueInvariant
BY Isa
   DEF ExactDecisionRequestIngressCausalOwedQueueInvariant,
       ExactDecisionRequestIngressCausalOwedQueueConsistency,
       AsyncInitAt, AsyncBaseInitAt, AsyncRuntimeInit

THEOREM AsyncBracketNextPreservesExactDecisionRequestIngressCausalOwedQueue ==
  /\ AsyncStrongTypeInvariant
  /\ ExactDecisionRequestIngressCausalOwedQueueInvariant
  /\ [AsyncNext]_AsyncAllVars
  => ExactDecisionRequestIngressCausalOwedQueueInvariant'
BY IsaT(300)
   DEF ExactDecisionRequestIngressCausalOwedQueueInvariant,
       ExactDecisionRequestIngressCausalOwedQueueConsistency,
       CausalQueueNonempty,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncSetGST, AsyncTick,
       OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncNetworkStep, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       AsyncFaultStep, PreGstLosePacket,
       InjectByzantineNoise, InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk, InjectByzantineCertifiedRequest,
       AsyncByzantineProposal, AsyncByzantineVote,
       AsyncByzantineTimeout,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       DirectTimeoutStep, DirectRetransmitStep,
       DeferredTagStep, DeferredTimeoutStep, DeferredRetransmitStep,
       FifoRuntimeStep, DeferredDrainStep, IdleRuntimeStep,
       AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
       AppendCausalSuccessors,
       AppendHistoricalLockedRetransmitSuccessors,
       LeaveCausalQueues,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       RearmResponsiveRecovery,
       AsyncAllVars, AsyncSchedulerVars, AsyncProducerVars,
       AsyncLocalAdmissionVars, AsyncIoVars

THEOREM AsyncSpecAlwaysExactDecisionRequestIngressCausalOwedQueue ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []ExactDecisionRequestIngressCausalOwedQueueInvariant
BY AsyncInitEstablishesExactDecisionRequestIngressCausalOwedQueue,
   AsyncBracketNextPreservesExactDecisionRequestIngressCausalOwedQueue,
   AsyncSpecAlwaysStrongTypeInvariant, PTL
   DEF AsyncSpecAt

THEOREM ExactDecisionRequestIngressCausalReplenishmentHasConcreteProducer ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestIngressCausalReplenishmentAction(
         node, qc, archive, request)
    => ExactDecisionRequestIngressCausalConcreteProducerAction(archive)
BY AsyncBracketStepPreservesNodeApplication, IsaT(300)
   DEF ExactDecisionRequestIngressCausalReplenishmentAction,
       ExactDecisionRequestIngressCausalConcreteProducerAction,
       ExactDecisionRequestIngressCausalLocalDebtProducerAction,
       ExactDecisionRequestIngressCausalSuccessorProducerAction,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressCausalDebt,
       CompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       CausalQueueNonempty, HeadCausalCandidate,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunHistoricalServer, RunNodeWork,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep,
       IngressDrainStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       DirectTimeoutStep, DirectRetransmitStep,
       DeferredTagStep, DeferredTimeoutStep, DeferredRetransmitStep,
       FifoRuntimeStep, DeferredDrainStep, IdleRuntimeStep,
       UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
       ResetNodeSchedulerForRestart, DriveResponsiveReplayHead,
       FinishResponsiveReplay, PreGstResponsiveReplay

THEOREM ExactDecisionRequestIngressCausalConsistencyExcludesSuccessorProducer ==
  \A archive:
    /\ ExactDecisionRequestIngressCausalOwedQueueConsistency(archive)
    /\ ExactDecisionRequestIngressCausalSuccessorProducerAction(archive)
    => FALSE
BY Isa
   DEF ExactDecisionRequestIngressCausalOwedQueueConsistency,
       ExactDecisionRequestIngressCausalSuccessorProducerAction

THEOREM ExactDecisionRequestIngressReachableCausalReplenishmentIsLocal ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressCausalOwedQueueInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestIngressCausalReplenishmentAction(
         node, qc, archive, request)
    => ExactDecisionRequestIngressCausalLocalDebtProducerAction(archive)
BY ExactDecisionRequestIngressCausalReplenishmentHasConcreteProducer,
   ExactDecisionRequestIngressCausalConsistencyExcludesSuccessorProducer,
   AsyncCurrentResponsiveVotersAreValidators, Isa
   DEF ExactDecisionRequestIngressCausalConcreteProducerAction,
       ExactDecisionRequestIngressCausalOwedQueueInvariant,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias

THEOREM ExactDecisionRequestIngressReachableCausalResidualHasLocalWitness ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressCausalOwedQueueInvariant
    /\ ExactDecisionRequestIngressCausalReplenishmentResidual(
         node, qc, archive, request)
    => ENABLED
         <<ExactDecisionRequestIngressCausalReplenishmentAction(
             node, qc, archive, request)
           /\ ExactDecisionRequestIngressCausalLocalDebtProducerAction(
                archive)>>_AsyncAllVars
BY ExactDecisionRequestIngressReachableCausalReplenishmentIsLocal,
   ExpandENABLED, Isa
   DEF ExactDecisionRequestIngressCausalReplenishmentResidual,
       AsyncAllVars

THEOREM AsyncSpecAlwaysExactDecisionRequestIngressCausalResidualIsLocal ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => [](\A node, qc, archive, request:
             ExactDecisionRequestIngressCausalReplenishmentResidual(
               node, qc, archive, request)
               => ENABLED
                    <<ExactDecisionRequestIngressCausalReplenishmentAction(
                        node, qc, archive, request)
                      /\ ExactDecisionRequestIngressCausalLocalDebtProducerAction(
                           archive)>>_AsyncAllVars)
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysExactDecisionRequestIngressCausalOwedQueue,
   ExactDecisionRequestIngressReachableCausalResidualHasLocalWitness, PTL

THEOREM ExactDecisionRequestIngressCausalNonProducerDoesNotIncrease ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ AsyncNext
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)'
    /\ ExactDecisionRequestIngressModeRank(archive)'
         = ExactDecisionRequestIngressModeRank(archive)
    /\ ~ExactDecisionRequestIngressCausalConcreteProducerAction(archive)
    => ExactDecisionRequestIngressCausalDebt(archive)'
         <= ExactDecisionRequestIngressCausalDebt(archive)
BY ExactDecisionRequestIngressCausalReplenishmentHasConcreteProducer, SMT
   DEF ExactDecisionRequestIngressCausalReplenishmentAction

THEOREM ExactDecisionRequestIngressFreshCommandSuccessorBatchIsBounded ==
  \A command:
    Len(FreshCommandSuccessors(command)) \in 0..3
BY CommandSuccessorsHaveBoundedLength, Isa
   DEF FreshCommandSuccessors, FreshCandidateSequence

THEOREM ExactDecisionRequestIngressHistoricalSuccessorBatchIsBounded ==
  \A archive:
    Len(HistoricalLockedRetransmitSuccessors(archive)) \in 0..1
BY Isa
   DEF HistoricalLockedRetransmitSuccessors, FreshCandidateSequence

ExactDecisionRequestIngressServeIngressProducerAction(archive) ==
  /\ AsyncIoQueueDepth(archive)' = AsyncIoQueueDepth(archive) + 1
  /\ \/ /\ RunNode(archive)
        /\ DrainFairIngressSelected(archive)
     \/ /\ RunHistoricalRecoveryNode(archive)
        /\ DrainFairIngressSelected(archive)
     \/ /\ RunHistoricalServer(archive)
        /\ DrainHistoricalIngressSelected(archive)

ExactDecisionRequestIngressServeCausalProducerAction(archive) ==
  /\ AsyncIoQueueDepth(archive)' = AsyncIoQueueDepth(archive) + 1
  /\ \/ RunNode(archive)
     \/ RunHistoricalRecoveryNode(archive)
  /\ LocalAdmissionStep(archive)
  /\ AdmitCausalHead(archive)

ExactDecisionRequestIngressServeControlProducerAction(archive) ==
  /\ AsyncIoQueueDepth(archive)' = AsyncIoQueueDepth(archive) + 1
  /\ \/ EnqueueIoLocalControl(archive)
     \/ EnqueueHistoricalRecoveryIoLocalControl(archive)

ExactDecisionRequestIngressServeConcreteProducerAction(archive) ==
  \/ ExactDecisionRequestIngressServeIngressProducerAction(archive)
  \/ ExactDecisionRequestIngressServeCausalProducerAction(archive)
  \/ ExactDecisionRequestIngressServeControlProducerAction(archive)

THEOREM ExactDecisionRequestIngressServeCapacityDebtIsBounded ==
  \A archive \in ValidatorIds:
    AsyncStrongTypeInvariant
      => ExactDecisionRequestIngressServeCapacityDebt(archive)
           <= AsyncIoWorkCapacity + 2
BY AsyncStrongTypeProjectsAsyncType, SMT
   DEF ExactDecisionRequestIngressServeCapacityDebt,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoCapacityTypeInvariant,
       AsyncIoCapacity, AsyncIoQueueDepth, AsyncConfiguration

THEOREM ExactDecisionRequestIngressServeReplenishmentHasConcreteProducer ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestIngressServeReplenishmentAction(
         node, qc, archive, request)
    => ExactDecisionRequestIngressServeConcreteProducerAction(archive)
BY IsaT(300)
   DEF ExactDecisionRequestIngressServeReplenishmentAction,
       ExactDecisionRequestIngressServeConcreteProducerAction,
       ExactDecisionRequestIngressServeIngressProducerAction,
       ExactDecisionRequestIngressServeCausalProducerAction,
       ExactDecisionRequestIngressServeControlProducerAction,
       ExactDecisionRequestIngressServeCapacityDebt,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       AsyncIoQueueDepth,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunHistoricalServer, RunNodeWork,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep,
       IngressDrainStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       AdmitCausalHead, ServiceIoWorker, ServiceIoWorkerWork,
       ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueIoLocalControlWork,
       EnqueueHistoricalRecoveryIoLocalControl,
       ResetNodeSchedulerForRestart

THEOREM ExactDecisionRequestIngressServeReplenishmentAddsOneJob ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestIngressServeReplenishmentAction(
         node, qc, archive, request)
    => /\ AsyncIoQueueDepth(archive)'
            = AsyncIoQueueDepth(archive) + 1
       /\ ExactDecisionRequestIngressServeCapacityDebt(archive)'
            = ExactDecisionRequestIngressServeCapacityDebt(archive) + 1
BY ExactDecisionRequestIngressServeReplenishmentHasConcreteProducer, SMT
   DEF ExactDecisionRequestIngressServeConcreteProducerAction,
       ExactDecisionRequestIngressServeIngressProducerAction,
       ExactDecisionRequestIngressServeCausalProducerAction,
       ExactDecisionRequestIngressServeControlProducerAction,
       ExactDecisionRequestIngressServeReplenishmentAction,
       ExactDecisionRequestIngressServeCapacityDebt,
       AsyncIoQueueDepth

THEOREM ExactDecisionRequestIngressServeNonProducerDoesNotIncrease ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ AsyncNext
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)'
    /\ ExactDecisionRequestIngressModeRank(archive)'
         = ExactDecisionRequestIngressModeRank(archive)
    /\ ExactDecisionRequestIngressCausalDebt(archive)'
         = ExactDecisionRequestIngressCausalDebt(archive)
    /\ ~ExactDecisionRequestIngressServeConcreteProducerAction(archive)
    => ExactDecisionRequestIngressServeCapacityDebt(archive)'
         <= ExactDecisionRequestIngressServeCapacityDebt(archive)
BY ExactDecisionRequestIngressServeReplenishmentHasConcreteProducer, SMT
   DEF ExactDecisionRequestIngressServeReplenishmentAction

ExactDecisionRequestIngressClaimedPriorityOwners(archive) ==
  {pair \in AsyncIngressSources \X (1..AsyncIngressCapacity):
     pair[2] \in
       DrainableClaimedResponseLaneIndices(archive, pair[1])}

ExactDecisionRequestIngressFencedPriorityOwners(archive) ==
  {pair \in AsyncIngressSources \X (1..AsyncIngressCapacity):
     pair[2] \in
       DrainableRequestFencedCompletionLaneIndices(
         archive, pair[1])}

ExactDecisionRequestIngressInsertedPriorityOwners(archive) ==
  ExactDecisionRequestIngressPriorityOwners(archive)'
    \ ExactDecisionRequestIngressPriorityOwners(archive)

ExactDecisionRequestIngressInsertedClaimedPriorityOwners(archive) ==
  ExactDecisionRequestIngressInsertedPriorityOwners(archive)
    \cap ExactDecisionRequestIngressClaimedPriorityOwners(archive)'

ExactDecisionRequestIngressInsertedFencedPriorityOwners(archive) ==
  ExactDecisionRequestIngressInsertedPriorityOwners(archive)
    \cap ExactDecisionRequestIngressFencedPriorityOwners(archive)'

(*
For a runner, a fenced insertion has exactly two state-level sources.  An
empty local fence can become nonempty only through
`PublishCertifiedRequests`, reached by successful `FifoRuntimeStep` or
`DeferredDrainStep` execution of `ExecuteRequestCertifiedBody` /
`ExecuteDecisionFetch`.  If the fence was already active, the insertion is a
drainability opening: a normal/recovery runner can open the command slot or
retire a claim while an independent fence remains, while the historical
server can remove the singleton claim and expose permitted stale duplicates
as unauthorized, drainable fenced occurrences.
*)
ExactDecisionRequestIngressPriorityFenceActivationWitnessAction(archive) ==
  /\ ActiveCertifiedRequestHashesAt(archive) = {}
  /\ ActiveCertifiedRequestHashesAt(archive)' # {}
  /\ ExactDecisionRequestIngressInsertedFencedPriorityOwners(archive)
       # {}

ExactDecisionRequestIngressPriorityExistingFenceWitnessAction(archive) ==
  /\ ActiveCertifiedRequestHashesAt(archive) # {}
  /\ ExactDecisionRequestIngressInsertedFencedPriorityOwners(archive)
       # {}

ExactDecisionRequestIngressPriorityNetworkClaimProducerAction(archive) ==
  /\ AsyncNetworkStep
  /\ ExactDecisionRequestIngressInsertedClaimedPriorityOwners(archive)
       # {}
  /\ \E source \in AsyncIngressSources:
       AdmitHiddenPacket(archive, source)

ExactDecisionRequestIngressPriorityNormalRunnerWitnessAction(archive) ==
  /\ RunNode(archive)
  /\ \/ ExactDecisionRequestIngressInsertedClaimedPriorityOwners(archive)
          # {}
     \/ ExactDecisionRequestIngressPriorityFenceActivationWitnessAction(
          archive)
     \/ ExactDecisionRequestIngressPriorityExistingFenceWitnessAction(
          archive)

ExactDecisionRequestIngressPriorityRecoveryRunnerWitnessAction(archive) ==
  /\ RunHistoricalRecoveryNode(archive)
  /\ \/ ExactDecisionRequestIngressInsertedClaimedPriorityOwners(archive)
          # {}
     \/ ExactDecisionRequestIngressPriorityFenceActivationWitnessAction(
          archive)
     \/ ExactDecisionRequestIngressPriorityExistingFenceWitnessAction(
          archive)

ExactDecisionRequestIngressPriorityHistoricalRunnerWitnessAction(archive) ==
  /\ RunHistoricalServer(archive)
  /\ \/ ExactDecisionRequestIngressInsertedClaimedPriorityOwners(archive)
          # {}
     \/ ExactDecisionRequestIngressPriorityExistingFenceWitnessAction(
          archive)

ExactDecisionRequestIngressPriorityConcreteProducerAction(archive) ==
  \/ ExactDecisionRequestIngressPriorityNetworkClaimProducerAction(archive)
  \/ ExactDecisionRequestIngressPriorityNormalRunnerWitnessAction(archive)
  \/ ExactDecisionRequestIngressPriorityRecoveryRunnerWitnessAction(archive)
  \/ ExactDecisionRequestIngressPriorityHistoricalRunnerWitnessAction(archive)

THEOREM ExactDecisionRequestIngressPriorityDebtIsCapacityBounded ==
  \A archive \in ValidatorIds:
    AsyncStrongTypeInvariant
      => ExactDecisionRequestIngressPriorityDebt(archive)
           <= IngressDepth(archive)
         /\ IngressDepth(archive) <= AsyncIngressCapacity
         /\ Cardinality(CertifiedResponseClaimsAt(archive)) <= 1
BY FS_Product, FS_Interval, FS_Subset, FS_CardinalityType, IsaT(180)
   DEF ExactDecisionRequestIngressPriorityDebt,
       ExactDecisionRequestIngressPriorityOwners,
       IngressDepth, IngressLaneDepth, IngressLane,
       DrainableClaimedResponseLaneIndices,
       DrainableRequestFencedCompletionLaneIndices,
       DrainableIngressLaneIndices,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant, AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       AsyncCertifiedResponseClaimInvariant,
       AsyncIngressTypeInvariant, AsyncIngressCapacityTypeInvariant,
       AsyncIngressContentTypeInvariant

THEOREM ExactDecisionRequestIngressPriorityIncreaseHasInsertedWitness ==
  \A archive \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ ExactDecisionRequestIngressPriorityDebt(archive)'
         > ExactDecisionRequestIngressPriorityDebt(archive)
    => /\ ExactDecisionRequestIngressInsertedPriorityOwners(archive)
            # {}
       /\ \/ ExactDecisionRequestIngressInsertedClaimedPriorityOwners(
                 archive) # {}
          \/ ExactDecisionRequestIngressInsertedFencedPriorityOwners(
                 archive) # {}
BY ExactDecisionRequestIngressPriorityDebtIsNatural,
   FS_CardinalityType, FS_Subset, Isa
   DEF ExactDecisionRequestIngressPriorityDebt,
       ExactDecisionRequestIngressPriorityOwners,
       ExactDecisionRequestIngressClaimedPriorityOwners,
       ExactDecisionRequestIngressFencedPriorityOwners,
       ExactDecisionRequestIngressInsertedPriorityOwners,
       ExactDecisionRequestIngressInsertedClaimedPriorityOwners,
       ExactDecisionRequestIngressInsertedFencedPriorityOwners

THEOREM ExactDecisionRequestIngressFencedInsertionSplitsAtActiveFence ==
  \A archive:
    ExactDecisionRequestIngressInsertedFencedPriorityOwners(archive) # {}
      => \/ ExactDecisionRequestIngressPriorityFenceActivationWitnessAction(
              archive)
         \/ ExactDecisionRequestIngressPriorityExistingFenceWitnessAction(
              archive)
BY Isa
   DEF ExactDecisionRequestIngressPriorityFenceActivationWitnessAction,
       ExactDecisionRequestIngressPriorityExistingFenceWitnessAction,
       ExactDecisionRequestIngressInsertedFencedPriorityOwners,
       ExactDecisionRequestIngressFencedPriorityOwners,
       DrainableRequestFencedCompletionLaneIndices

THEOREM ExactDecisionRequestIngressPriorityReplenishmentHasConcreteProducer ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestIngressPriorityReplenishmentAction(
         node, qc, archive, request)
    => ExactDecisionRequestIngressPriorityConcreteProducerAction(archive)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   ExactDecisionRequestIngressPriorityIncreaseHasInsertedWitness,
   ExactDecisionRequestIngressFencedInsertionSplitsAtActiveFence, IsaT(300)
   DEF ExactDecisionRequestIngressPriorityReplenishmentAction,
       ExactDecisionRequestIngressPriorityConcreteProducerAction,
       ExactDecisionRequestIngressPriorityNetworkClaimProducerAction,
       ExactDecisionRequestIngressPriorityNormalRunnerWitnessAction,
       ExactDecisionRequestIngressPriorityRecoveryRunnerWitnessAction,
       ExactDecisionRequestIngressPriorityHistoricalRunnerWitnessAction,
       ExactDecisionRequestIngressPriorityFenceActivationWitnessAction,
       ExactDecisionRequestIngressPriorityExistingFenceWitnessAction,
       ExactDecisionRequestIngressInsertedClaimedPriorityOwners,
       ExactDecisionRequestIngressInsertedFencedPriorityOwners,
       ExactDecisionRequestIngressInsertedPriorityOwners,
       ExactDecisionRequestIngressClaimedPriorityOwners,
       ExactDecisionRequestIngressFencedPriorityOwners,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncNetworkStep, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       OpenHistoricalRecovery, DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       AsyncFaultStep, PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       RearmResponsiveRecovery, ResetNodeSchedulerForRestart

THEOREM ExactDecisionRequestIngressPriorityNonProducerDoesNotIncrease ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)
    /\ AsyncNext
    /\ ExactDecisionRequestIngressLaneResidual(
         node, qc, archive, request)'
    /\ ExactDecisionRequestIngressModeRank(archive)'
         = ExactDecisionRequestIngressModeRank(archive)
    /\ ExactDecisionRequestIngressCausalDebt(archive)'
         = ExactDecisionRequestIngressCausalDebt(archive)
    /\ ExactDecisionRequestIngressServeCapacityDebt(archive)'
         = ExactDecisionRequestIngressServeCapacityDebt(archive)
    /\ ~ExactDecisionRequestIngressPriorityConcreteProducerAction(archive)
    => ExactDecisionRequestIngressPriorityDebt(archive)'
         <= ExactDecisionRequestIngressPriorityDebt(archive)
BY ExactDecisionRequestIngressPriorityReplenishmentHasConcreteProducer, SMT
   DEF ExactDecisionRequestIngressPriorityReplenishmentAction

(***************************************************************************
Finite owner-episode accounting.

Hidden admission now owns a logical future I/O slot even when the physical
queue is full.  Its immutable ordinal freezes the current I/O jobs, every
earlier admitted exact owner, and one pre-cutoff prefix length for every
ingress source.  Off-queue ownership saturates ordinary I/O admission; the
normal and historical selectors admit only those frozen prefixes or the least
ordinal exact owner.  Work admitted after the cutoff cannot acquire a frozen
predecessor position.

The episode owner set below is entirely state-derived.  It combines those
frozen occurrences with the target-specific mode/capacity/selector/lane/
source/runner components.  Volatile causal and generic physical-capacity
diagnostics above are intentionally absent.  Service can consume an owner,
preserve the exact target while consuming one finite owner, or reach the
lifecycle goal.  A cached tombstone remains an outstanding replay stage until
its exact bytes are back in transport; it is never counted as requester
completion.
***************************************************************************)

ExactDecisionRequestIngressProducerClasses ==
  {"Causal", "Serve", "Priority"}

ExactDecisionRequestIngressConcreteReplenishmentAction(
    node, qc, archive, request, producerClass) ==
  CASE producerClass = "Causal" ->
         /\ ExactDecisionRequestIngressCausalReplenishmentAction(
              node, qc, archive, request)
         /\ ExactDecisionRequestIngressCausalConcreteProducerAction(
              archive)
    [] producerClass = "Serve" ->
         /\ ExactDecisionRequestIngressServeReplenishmentAction(
              node, qc, archive, request)
         /\ ExactDecisionRequestIngressServeConcreteProducerAction(
              archive)
    [] producerClass = "Priority" ->
         /\ ExactDecisionRequestIngressPriorityReplenishmentAction(
              node, qc, archive, request)
         /\ ExactDecisionRequestIngressPriorityConcreteProducerAction(
              archive)
    [] OTHER -> FALSE

ExactDecisionRequestIngressProducerEpisodeOwnerSet(
    node, qc, archive, request) ==
  LET laneOwners ==
        IF ExactDecisionRequestIngressLaneResidual(
             node, qc, archive, request)
        THEN ({"Lane"} \X
                (1..ExactDecisionRequestIngressLanePosition(
                      archive, request)))
             \cup
             ({"Source"} \X
                (1..ExactDecisionRequestIngressSourcePosition(
                      archive, request)))
             \cup
             ({"Runner"} \X
                (1..ExactDecisionRequestIngressReachRank(archive)))
        ELSE {}
  IN ExactDecisionRequestLifecycleFrozenPredecessorSet(
       archive, request)
       \cup
     ({"Mode"} \X
        (1..ExactDecisionRequestIngressModeRank(archive)))
       \cup
     ({"Capacity"} \X
        (1..ExactDecisionRequestIngressTargetServeCapacityDebt(
              archive, request)))
       \cup
     ({"Selector"} \X
        (1..ExactDecisionRequestIngressPriorityDebt(archive)))
       \cup laneOwners

ExactDecisionRequestIngressProducerEpisodeBudget(
    node, qc, archive, request) ==
  Cardinality(
    ExactDecisionRequestIngressProducerEpisodeOwnerSet(
      node, qc, archive, request))

ExactDecisionRequestIngressProducerEpisodeStaticBound ==
  3 * AsyncIoCapacity
    + 8 * Cardinality(AsyncIngressSources) * AsyncIngressCapacity
    + AsyncServeLifecycleFamilyBudget
    + AsyncRunnerCycleBudget + 8

THEOREM ExactDecisionRequestIngressProducerEpisodeBudgetIsFinite ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    => /\ IsFiniteSet(
            ExactDecisionRequestIngressProducerEpisodeOwnerSet(
              node, qc, archive, request))
       /\ ExactDecisionRequestIngressProducerEpisodeBudget(
            node, qc, archive, request) \in Nat
       /\ ExactDecisionRequestIngressProducerEpisodeBudget(
            node, qc, archive, request)
            <= ExactDecisionRequestIngressProducerEpisodeStaticBound
BY FS_Union, FS_Product, FS_Interval, FS_Subset,
   FS_CardinalityType, IsaT(180)
   DEF ExactDecisionRequestIngressProducerEpisodeOwnerSet,
       ExactDecisionRequestIngressProducerEpisodeBudget,
       ExactDecisionRequestIngressProducerEpisodeStaticBound,
       ExactDecisionRequestLifecycleFrozenPredecessorSet,
       ExactDecisionRequestLifecycleResidual,
       ExactDecisionServeAdmissionOwned,
       ExactDecisionServeLifecycleIdentity,
       ExactDecisionRequestIngressLaneResidual,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncServeLifecycleTypeInvariant,
       AsyncServeReservationTyped,
       AsyncServeFrozenPredecessorSet,
       AsyncServeFrozenIngressPredecessorSet,
       AsyncServeFrozenIngressPredecessorDebtSlots,
       AsyncServeFrozenIngressPredecessorCounts,
       AsyncServeIngressAdmissionPredecessorDebtSlots,
       AsyncServeIngressAdmissionPredecessorCounts,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServePreexistingIngressOwnerPredecessorDebtSet,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressBarrierPredecessorDebtSet,
       AsyncServeIngressIdentityFrozenByReservation,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionRequestIngressServeCapacityDebt,
       ExactDecisionRequestIngressPriorityDebt,
       ExactDecisionRequestIngressPriorityOwners,
       ExactDecisionRequestIngressLanePosition,
       ExactDecisionRequestIngressLaneIndices,
       ExactDecisionRequestIngressSourcePosition,
       IngressSourceServiceRank,
       ExactDecisionRequestIngressReachRank,
       AsyncConfiguration

THEOREM ExactDecisionRequestLifecycleIngressRankInCarrier ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    => ExactDecisionRequestLifecycleIngressRank(
         node, qc, archive, request)
         \in ExactDecisionRequestLifecycleIngressRankCarrier
BY ExactDecisionRequestIngressRankInCarrier,
   FS_Union, FS_Product, FS_CardinalityType, IsaT(180)
   DEF ExactDecisionRequestLifecycleIngressRank,
       ExactDecisionRequestLifecycleIngressRankCarrier,
       ExactDecisionRequestLifecycleDebtCarrier,
       ExactDecisionRequestLifecycleStage,
       ExactDecisionRequestLifecycleFrozenPredecessorDebt,
       ExactDecisionRequestLifecycleFrozenPredecessorSet,
       ExactDecisionRequestLifecycleNestedIngressRank,
       ExactDecisionRequestIngressZeroRank,
       ExactDecisionRequestIngressZeroCapacityRank,
       ExactDecisionRequestIngressZeroReachSelectorRank,
       ExactDecisionRequestIngressZeroSelectorRank,
       ExactDecisionRequestIngressZeroLaneRank,
       ExactDecisionRequestIngressRankCarrier,
       ExactDecisionRequestIngressCapacityCarrier,
       ExactDecisionRequestIngressReachSelectorCarrier,
       ExactDecisionRequestIngressSelectorCarrier,
       ExactDecisionRequestIngressLaneCarrier

ExactDecisionRequestFrozenServeBarrierIdentities(archive, request) ==
  AsyncServePreexistingIngressBarrierIdentities(
    archive,
    ExactDecisionServeLifecycleIdentity(archive, request))

ExactDecisionRequestFrozenServeBarrierIdentity(archive, request) ==
  CHOOSE identity \in
    ExactDecisionRequestFrozenServeBarrierIdentities(archive, request):
      TRUE

ExactDecisionRequestIngressAdmissionOrdinal(archive, request) ==
  AsyncServeIngressAdmissionOrdinal(
    archive,
    ExactDecisionServeLifecycleIdentity(archive, request))

THEOREM AsyncServeNewReservationRequiresEmptyIngressOwnerSet ==
  \A archive, candidate:
    \/ ReserveExactServeCapacity(archive, candidate)
    \/ AdvanceExactServeCapacity(archive, candidate)
    => AsyncServeIngressLifecycleOwnerIdentities(archive) = {}
BY Isa
   DEF ReserveExactServeCapacity, AdvanceExactServeCapacity

THEOREM ExactDecisionRequestIngressOrdinalExcludesLaterServeBarrier ==
  \A node, qc, archive, request:
    LET identity ==
          ExactDecisionServeLifecycleIdentity(archive, request)
    IN /\ AsyncStrongTypeInvariant
       /\ ExactDecisionRequestLifecycleResidual(
            node, qc, archive, request)
       => /\ AsyncServeIngressAdmissionOwned(archive, identity)
          /\ \A reservation \in
               AsyncServeOffQueueReservations(archive):
               AsyncServeIngressAdmissionOrdinal(
                 archive, reservation.identity)
                 <=
               ExactDecisionRequestIngressAdmissionOrdinal(
                 archive, request)
BY IsaT(240)
   DEF ExactDecisionRequestIngressAdmissionOrdinal,
       ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionServeLifecycleIdentity,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionOrdinal,
       AsyncServeIngressAdmissionInvariant,
       AsyncServeBarrierOwnsEarliestIngressOrdinalInvariant,
       AsyncServeLifecycleTypeInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant

THEOREM ExactDecisionRequestIngressOrdinalPersistsUntilDrain ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    /\ AsyncNext
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)'
    => ExactDecisionRequestIngressAdmissionOrdinal(
         archive, request)'
         =
       ExactDecisionRequestIngressAdmissionOrdinal(
         archive, request)
BY IsaT(300)
   DEF ExactDecisionRequestIngressAdmissionOrdinal,
       ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionServeLifecycleIdentity,
       AsyncServeIngressAdmissionOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       ReserveExactServeCapacity, AdvanceExactServeCapacity,
       CoalesceExactServeIngressCapacity,
       AcceptOrReserveExactServeIngress,
       PopSelectedIngress,
       ResetNodeSchedulerForRestart,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncNetworkStep, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay

THEOREM AsyncServeIngressPopRetiresSelectedAdmission ==
  \A archive, readyIndex, laneIndex:
    LET source == asyncIngressReady[archive][readyIndex]
        item == asyncIngressLanes[archive][source][laneIndex]
        identity == AsyncServeLogicalRequestIdentity(archive, item)
    IN /\ item.kind \in AsyncReplyRequestKinds
       /\ AsyncServeIngressAdmissionOwned(archive, identity)
       /\ PopSelectedIngress(archive, readyIndex, laneIndex)
       => ~AsyncServeIngressAdmissionOwned(archive, identity)'
BY Isa
   DEF PopSelectedIngress,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout

THEOREM AsyncServeIngressDuplicateDoesNotAllocateOrdinal ==
  \A recipient, source:
    CoalesceHiddenPacket(recipient, source)
      => UNCHANGED AsyncServeIngressAdmissionVars
BY Isa
   DEF CoalesceHiddenPacket, AsyncIoVars,
       AsyncServeIngressAdmissionVars

THEOREM AsyncServeIngressOwnerFencesLaterControlProducer ==
  \A archive:
    AsyncServeIngressLifecycleOwnerIdentities(archive) # {}
      => ~EnqueueIoLocalControlWork(archive)
BY Isa
   DEF EnqueueIoLocalControlWork

THEOREM ExactDecisionRequestIngressOrdinalRejectsLaterPriorityBypass ==
  \A node, qc, archive, request, source, index:
    LET identity ==
          ExactDecisionServeLifecycleIdentity(archive, request)
        item == asyncIngressLanes[archive][source][index]
    IN /\ AsyncStrongTypeInvariant
       /\ ExactDecisionRequestLifecycleResidual(
            node, qc, archive, request)
       /\ identity =
            AsyncServeEarliestIngressLifecycleOwnerIdentity(archive)
       /\ item.kind \notin AsyncReplyRequestKinds
       /\ ~AsyncCertifiedFenceEscapeItem(item)
       /\ index >
            AsyncServeIngressAdmissionPredecessorCounts(
              archive, identity)[source]
       => ~AsyncServeIngressIndexMayPrecedeAdmittedTarget(
             archive, source, index)
BY Isa
   DEF ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionServeLifecycleIdentity,
       AsyncServeIngressIndexMayPrecedeAdmittedTarget,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionPredecessorCounts

THEOREM ExactDecisionRequestIngressOrdinalRejectsLaterExactRequestBypass ==
  \A node, qc, archive, request, source, index:
    LET identity ==
          ExactDecisionServeLifecycleIdentity(archive, request)
        item == asyncIngressLanes[archive][source][index]
        itemIdentity ==
          AsyncServeLogicalRequestIdentity(archive, item)
    IN /\ AsyncStrongTypeInvariant
       /\ ExactDecisionRequestLifecycleResidual(
            node, qc, archive, request)
       /\ identity =
            AsyncServeEarliestIngressLifecycleOwnerIdentity(archive)
       /\ item.kind \in AsyncReplyRequestKinds
       /\ itemIdentity # identity
       /\ AsyncServeIngressAdmissionOwned(archive, itemIdentity)
       /\ ExactDecisionRequestIngressAdmissionOrdinal(
            archive, request)
            < AsyncServeIngressAdmissionOrdinal(
                archive, itemIdentity)
       /\ index >
            AsyncServeIngressAdmissionPredecessorCounts(
              archive, identity)[source]
       => ~AsyncServeIngressIndexMayPrecedeAdmittedTarget(
             archive, source, index)
BY Isa
   DEF ExactDecisionRequestIngressAdmissionOrdinal,
       ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionServeLifecycleIdentity,
       AsyncServeIngressIndexMayPrecedeAdmittedTarget,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionPredecessorCounts

THEOREM AsyncServeIngressFrozenPrefixCutoffAllowsPreexisting ==
  \A archive, source, index:
    LET identity ==
          AsyncServeEarliestIngressLifecycleOwnerIdentity(archive)
    IN /\ AsyncServeIngressLifecycleOwnerIdentities(archive) # {}
       /\ source \in AsyncIngressSources
       /\ index \in 1..Len(asyncIngressLanes[archive][source])
       /\ index <=
            AsyncServeIngressAdmissionPredecessorCounts(
              archive, identity)[source]
       => AsyncServeIngressIndexMayPrecedeAdmittedTarget(
            archive, source, index)
BY Isa
   DEF AsyncServeIngressIndexMayPrecedeAdmittedTarget

THEOREM AsyncServeIngressFrozenPrefixCutoffRejectsPostCutoff ==
  \A archive, source, index:
    LET identity ==
          AsyncServeEarliestIngressLifecycleOwnerIdentity(archive)
        item == asyncIngressLanes[archive][source][index]
    IN /\ AsyncServeIngressLifecycleOwnerIdentities(archive) # {}
       /\ source \in AsyncIngressSources
       /\ index \in 1..Len(asyncIngressLanes[archive][source])
       /\ index >
            AsyncServeIngressAdmissionPredecessorCounts(
              archive, identity)[source]
       /\ ~AsyncCertifiedFenceEscapeItem(item)
       /\ \/ item.kind \notin AsyncReplyRequestKinds
          \/ AsyncServeLogicalRequestIdentity(archive, item) # identity
       => ~AsyncServeIngressIndexMayPrecedeAdmittedTarget(
             archive, source, index)
BY Isa
   DEF AsyncServeIngressIndexMayPrecedeAdmittedTarget

THEOREM ExactDecisionRequestFrozenServeBarrierIsSingleton ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    => /\ IsFiniteSet(
            ExactDecisionRequestFrozenServeBarrierIdentities(
              archive, request))
       /\ Cardinality(
            ExactDecisionRequestFrozenServeBarrierIdentities(
              archive, request)) <= 1
       /\ (ExactDecisionRequestFrozenServeBarrierIdentities(
              archive, request) # {}
             =>
           ExactDecisionRequestFrozenServeBarrierIdentity(
             archive, request)
             \in
               ExactDecisionRequestFrozenServeBarrierIdentities(
                 archive, request))
BY FS_Subset, FS_CardinalityType, IsaT(180)
   DEF ExactDecisionRequestFrozenServeBarrierIdentities,
       ExactDecisionRequestFrozenServeBarrierIdentity,
       ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServeIngressLiveReservations,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeSingularOffQueueBarrierInvariant,
       AsyncServeBarrierOwnsEarliestIngressOrdinalInvariant,
       AsyncServeLifecycleTypeInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant

ExactDecisionRequestFrozenServeBarrierMaterializationAction(
    archive, request) ==
  LET normalItem ==
        SelectedIngressItemAt(
          archive, FirstDrainableIngressIndex(archive))
      historicalItem ==
        HistoricalSelectedIngressItemAt(
          archive, FirstHistoricalDrainableIngressIndex(archive))
  IN \E barrierIdentity \in
       ExactDecisionRequestFrozenServeBarrierIdentities(
         archive, request):
       \/ /\ PostGstRunNode(archive)
             /\ DrainFairIngressSelected(archive)
             /\ normalItem.kind \in AsyncReplyRequestKinds
             /\ AsyncServeLogicalRequestIdentity(
                  archive, normalItem) = barrierIdentity
          \/ /\ PostGstRunHistoricalServer(archive)
                /\ DrainHistoricalIngressSelected(archive)
                /\ historicalItem.kind \in AsyncReplyRequestKinds
                /\ AsyncServeLogicalRequestIdentity(
                     archive, historicalItem) = barrierIdentity

ExactDecisionRequestEarlierIngressOwnerServiceAction(
    archive, request) ==
  LET targetIdentity ==
        ExactDecisionServeLifecycleIdentity(archive, request)
      selectedItem ==
        IF NodeHasApplication(archive)
        THEN HistoricalSelectedIngressItemAt(
               archive,
               FirstHistoricalDrainableIngressIndex(archive))
        ELSE SelectedIngressItemAt(
               archive, FirstDrainableIngressIndex(archive))
  IN \E ownerIdentity \in
       AsyncServePreexistingIngressOwnerIdentities(
         archive, targetIdentity):
       /\ \/ DrainFairIngressSelected(archive)
          \/ DrainHistoricalIngressSelected(archive)
       /\ \/ /\ selectedItem.kind \in AsyncReplyRequestKinds
             /\ AsyncServeLogicalRequestIdentity(
                  archive, selectedItem) = ownerIdentity
          \/ \E source \in AsyncIngressSources:
               \E index \in
                    1..AsyncServeIngressAdmissionPredecessorCounts(
                         archive, ownerIdentity)[source]:
                 asyncIngressLanes[archive][source][index]
                   = selectedItem

ExactDecisionRequestFrozenServeBarrierPredecessorServiceAction(
    archive, request) ==
  LET selectedItem ==
        IF NodeHasApplication(archive)
        THEN HistoricalSelectedIngressItemAt(
               archive,
               FirstHistoricalDrainableIngressIndex(archive))
        ELSE SelectedIngressItemAt(
               archive, FirstDrainableIngressIndex(archive))
      head == Head(asyncIoQueues[archive])
  IN \E barrierIdentity \in
       ExactDecisionRequestFrozenServeBarrierIdentities(
         archive, request):
       \/ /\ ServiceIoWorkerWork(archive)
             /\ head
                  \in AsyncServeFrozenPredecessorSet(
                       archive, barrierIdentity)
          \/ \E source \in AsyncIngressSources:
               \E index \in
                    1..AsyncServeFrozenIngressPredecessorCounts(
                         archive, barrierIdentity)[source]:
                 /\ \/ DrainFairIngressSelected(archive)
                    \/ DrainHistoricalIngressSelected(archive)
                 /\ asyncIngressLanes[archive][source][index]
                      = selectedItem

\* Compatibility name retained for source-fidelity consumers.  "Earlier"
\* means the smaller immutable ingress-admission ordinal, not a comparison
\* with a terminal lifecycle's retained Serve ordinal.
ExactDecisionRequestEarlierServeMaterializationAction(
    archive, request) ==
  ExactDecisionRequestFrozenServeBarrierMaterializationAction(
    archive, request)

ExactDecisionRequestLifecycleFrozenOwnerServiceAction(
    archive, request) ==
  LET identity ==
        ExactDecisionServeLifecycleIdentity(archive, request)
      head == Head(asyncIoQueues[archive])
  IN \/ /\ ServiceIoWorkerWork(archive)
           /\ head
                \in AsyncServeFrozenPredecessorSet(
                     archive, identity)
     \/ \E source \in AsyncIngressSources:
          \E index \in
               1..AsyncServeFrozenIngressPredecessorCounts(
                    archive, identity)[source]:
            /\ \/ DrainFairIngressSelected(archive)
               \/ DrainHistoricalIngressSelected(archive)
            /\ asyncIngressLanes[archive][source][index]
                 = IF NodeHasApplication(archive)
                   THEN HistoricalSelectedIngressItemAt(
                          archive,
                          FirstHistoricalDrainableIngressIndex(archive))
                   ELSE SelectedIngressItemAt(
                          archive,
                          FirstDrainableIngressIndex(archive))

THEOREM ExactDecisionRequestLifecycleFrozenOwnerServiceLowersRank ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestLifecycleFrozenOwnerServiceAction(
         archive, request)
    /\ [AsyncNext]_AsyncAllVars
    => /\ ExactDecisionRequestLifecycleResidual(
            node, qc, archive, request)'
       /\ ExactDecisionRequestLifecycleFrozenPredecessorDebt(
            archive, request)'
            <
          ExactDecisionRequestLifecycleFrozenPredecessorDebt(
            archive, request)
       /\ <<ExactDecisionRequestLifecycleIngressRank(
               node, qc, archive, request)',
             ExactDecisionRequestLifecycleIngressRank(
               node, qc, archive, request)>>
            \in ExactDecisionRequestLifecycleIngressRankOrdering
BY IsaT(300)
   DEF ExactDecisionRequestLifecycleFrozenOwnerServiceAction,
       ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestLifecycleFrozenPredecessorDebt,
       ExactDecisionRequestLifecycleFrozenPredecessorSet,
       ExactDecisionRequestLifecycleIngressRank,
       ExactDecisionRequestLifecycleStage,
       ExactDecisionRequestLifecycleNestedIngressRank,
       ExactDecisionRequestLifecycleIngressRankOrdering,
       ExactDecisionRequestLifecycleDebtOrdering,
       AsyncServeFrozenPredecessorSet,
       AsyncServeFrozenIngressPredecessorSet,
       AsyncServeFrozenIngressPredecessorDebtSlots,
       AsyncServeFrozenIngressPredecessorCounts,
       AsyncServeIngressAdmissionPredecessorDebtSlots,
       AsyncServeIngressAdmissionPredecessorCounts,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServePreexistingIngressOwnerPredecessorDebtSet,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressBarrierPredecessorDebtSet,
       AsyncServeIngressIdentityFrozenByReservation,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       ServiceIoWorkerWork, PopSelectedIngress,
       SetLessThan, LexPairOrdering, OpToRel, AsyncAllVars

THEOREM ExactDecisionRequestLifecycleFrozenOwnerServiceConsumesBudget ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestLifecycleFrozenOwnerServiceAction(
         archive, request)
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)'
    => ExactDecisionRequestIngressProducerEpisodeBudget(
         node, qc, archive, request)'
         <
       ExactDecisionRequestIngressProducerEpisodeBudget(
         node, qc, archive, request)
BY FS_CardinalityType, FS_Subset, IsaT(600)
   DEF ExactDecisionRequestLifecycleFrozenOwnerServiceAction,
       ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestIngressProducerEpisodeBudget,
       ExactDecisionRequestIngressProducerEpisodeOwnerSet,
       ExactDecisionRequestLifecycleFrozenPredecessorSet,
       ExactDecisionServeLifecycleIdentity,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionRequestIngressServeCapacityDebt,
       ExactDecisionRequestIngressPriorityDebt,
       ExactDecisionRequestIngressPriorityOwners,
       ExactDecisionRequestIngressLanePosition,
       ExactDecisionRequestIngressLaneIndices,
       ExactDecisionRequestIngressSourcePosition,
       ExactDecisionRequestIngressReachRank,
       AsyncServeFrozenPredecessorSet,
       AsyncServeFrozenIngressPredecessorSet,
       AsyncServeFrozenIngressPredecessorDebtSlots,
       AsyncServeFrozenIngressPredecessorCounts,
       AsyncServeIngressAdmissionPredecessorDebtSlots,
       AsyncServeIngressAdmissionPredecessorCounts,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServePreexistingIngressOwnerPredecessorDebtSet,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressBarrierPredecessorDebtSet,
       AsyncServeIngressIdentityFrozenByReservation,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       ServiceIoWorkerWork,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       PopSelectedIngress,
       IngressSourceServiceRank

THEOREM ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    /\ AsyncNext
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)'
    => ExactDecisionRequestLifecycleFrozenPredecessorSet(
         archive, request)'
         \subseteq
           ExactDecisionRequestLifecycleFrozenPredecessorSet(
             archive, request)
BY IsaT(300)
   DEF ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestLifecycleFrozenPredecessorSet,
       ExactDecisionServeLifecycleIdentity,
       AsyncServeFrozenPredecessorSet,
       AsyncServeFrozenIngressPredecessorSet,
       AsyncServeFrozenIngressPredecessorDebtSlots,
       AsyncServeFrozenIngressPredecessorCounts,
       AsyncServeIngressAdmissionPredecessorDebtSlots,
       AsyncServeIngressAdmissionPredecessorCounts,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServePreexistingIngressOwnerPredecessorDebtSet,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressBarrierPredecessorDebtSet,
       AsyncServeIngressIdentityFrozenByReservation,
       AsyncServeIngressLiveReservations,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeSingularOffQueueBarrierInvariant,
       AsyncServeBarrierOwnsEarliestIngressOrdinalInvariant,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       ReserveExactServeCapacity, AdvanceExactServeCapacity,
       CoalesceExactServeIngressCapacity,
       AcceptOrReserveExactServeIngress,
       ExactServeTransportAdmissionCanAdvance,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncNetworkStep, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart

THEOREM ExactDecisionRequestEarlierIngressOwnerServiceLowersRank ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestEarlierIngressOwnerServiceAction(
         archive, request)
    /\ [AsyncNext]_AsyncAllVars
    => /\ ExactDecisionRequestLifecycleResidual(
            node, qc, archive, request)'
       /\ ExactDecisionRequestLifecycleFrozenPredecessorDebt(
            archive, request)'
            <
          ExactDecisionRequestLifecycleFrozenPredecessorDebt(
            archive, request)
       /\ <<ExactDecisionRequestLifecycleIngressRank(
               node, qc, archive, request)',
             ExactDecisionRequestLifecycleIngressRank(
               node, qc, archive, request)>>
            \in ExactDecisionRequestLifecycleIngressRankOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish,
   FS_CardinalityType, IsaT(300)
   DEF ExactDecisionRequestEarlierIngressOwnerServiceAction,
       ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestLifecycleFrozenPredecessorDebt,
       ExactDecisionRequestLifecycleFrozenPredecessorSet,
       ExactDecisionRequestLifecycleIngressRank,
       ExactDecisionRequestLifecycleStage,
       ExactDecisionRequestLifecycleNestedIngressRank,
       ExactDecisionRequestLifecycleIngressRankOrdering,
       ExactDecisionRequestLifecycleDebtOrdering,
       ExactDecisionServeLifecycleIdentity,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServePreexistingIngressOwnerPredecessorDebtSet,
       AsyncServeIngressAdmissionPredecessorCounts,
       AsyncServeIngressAdmissionPredecessorDebtSlots,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       PopSelectedIngress,
       SetLessThan, LexPairOrdering, OpToRel, AsyncAllVars

THEOREM ExactDecisionRequestFrozenServeBarrierPredecessorServiceLowersRank ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestFrozenServeBarrierPredecessorServiceAction(
         archive, request)
    /\ [AsyncNext]_AsyncAllVars
    => /\ ExactDecisionRequestLifecycleResidual(
            node, qc, archive, request)'
       /\ ExactDecisionRequestFrozenServeBarrierIdentities(
            archive, request)' =
          ExactDecisionRequestFrozenServeBarrierIdentities(
            archive, request)
       /\ ExactDecisionRequestLifecycleFrozenPredecessorDebt(
            archive, request)'
            <
          ExactDecisionRequestLifecycleFrozenPredecessorDebt(
            archive, request)
       /\ <<ExactDecisionRequestLifecycleIngressRank(
               node, qc, archive, request)',
             ExactDecisionRequestLifecycleIngressRank(
               node, qc, archive, request)>>
            \in ExactDecisionRequestLifecycleIngressRankOrdering
BY ExactDecisionRequestFrozenServeBarrierIsSingleton,
   AsyncBracketNextPreservesStrongTypeInvariant,
   ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish,
   FS_CardinalityType, IsaT(300)
   DEF ExactDecisionRequestFrozenServeBarrierPredecessorServiceAction,
       ExactDecisionRequestFrozenServeBarrierIdentities,
       ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestLifecycleFrozenPredecessorDebt,
       ExactDecisionRequestLifecycleFrozenPredecessorSet,
       ExactDecisionRequestLifecycleIngressRank,
       ExactDecisionRequestLifecycleStage,
       ExactDecisionRequestLifecycleNestedIngressRank,
       ExactDecisionRequestLifecycleIngressRankOrdering,
       ExactDecisionRequestLifecycleDebtOrdering,
       AsyncServeFrozenPredecessorSet,
       AsyncServeFrozenIngressPredecessorSet,
       AsyncServeFrozenIngressPredecessorDebtSlots,
       AsyncServeFrozenIngressPredecessorCounts,
       AsyncServeIngressAdmissionPredecessorDebtSlots,
       AsyncServeIngressAdmissionPredecessorCounts,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServePreexistingIngressOwnerPredecessorDebtSet,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressBarrierPredecessorDebtSet,
       AsyncServeIngressIdentityFrozenByReservation,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       ServiceIoWorkerWork, DrainFairIngressSelected,
       DrainHistoricalIngressSelected, PopSelectedIngress,
       SetLessThan, LexPairOrdering, OpToRel, AsyncAllVars

THEOREM ExactDecisionRequestFrozenServeBarrierMaterializationLowersRank ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    /\ ExactDecisionRequestFrozenServeBarrierMaterializationAction(
         archive, request)
    /\ [AsyncNext]_AsyncAllVars
    => /\ ExactDecisionRequestLifecycleResidual(
            node, qc, archive, request)'
       /\ ExactDecisionRequestFrozenServeBarrierIdentities(
            archive, request)' = {}
       /\ ExactDecisionRequestLifecycleFrozenPredecessorDebt(
            archive, request)'
            <
          ExactDecisionRequestLifecycleFrozenPredecessorDebt(
            archive, request)
       /\ <<ExactDecisionRequestLifecycleIngressRank(
               node, qc, archive, request)',
             ExactDecisionRequestLifecycleIngressRank(
               node, qc, archive, request)>>
            \in ExactDecisionRequestLifecycleIngressRankOrdering
BY ExactDecisionRequestFrozenServeBarrierIsSingleton,
   AsyncBracketNextPreservesStrongTypeInvariant,
   ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish,
   FS_CardinalityType, IsaT(300)
   DEF ExactDecisionRequestFrozenServeBarrierMaterializationAction,
       ExactDecisionRequestFrozenServeBarrierIdentities,
       ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestLifecycleFrozenPredecessorDebt,
       ExactDecisionRequestLifecycleFrozenPredecessorSet,
       ExactDecisionRequestLifecycleIngressRank,
       ExactDecisionRequestLifecycleStage,
       ExactDecisionRequestLifecycleNestedIngressRank,
       ExactDecisionRequestLifecycleIngressRankOrdering,
       ExactDecisionRequestLifecycleDebtOrdering,
       AsyncServeIngressAdmissionPredecessorDebtSlots,
       AsyncServeIngressAdmissionPredecessorCounts,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServePreexistingIngressOwnerPredecessorDebtSet,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressBarrierPredecessorDebtSet,
       AsyncServeFrozenPredecessorSet,
       AsyncServeFrozenIngressPredecessorDebtSlots,
       AsyncServeFrozenIngressPredecessorCounts,
       AsyncServeIngressIdentityFrozenByReservation,
       AsyncServeIngressLiveReservations,
       AsyncServeSingularOffQueueBarrierInvariant,
       AsyncServeLifecycleTypeInvariant,
       AsyncServeReservationsAfterIngressDrain,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       PopSelectedIngress, ResumeExactServeCapacity,
       AcceptOrCoalesceExactServeRequest,
       PostGstRunNode, PostGstRunHistoricalServer,
       SetLessThan, LexPairOrdering, OpToRel, AsyncAllVars

THEOREM ExactDecisionRequestFrozenServeBarrierPreservesTargetIngressCoalescing ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    /\ ExactDecisionServeTombstoneOwned(
         node, qc, archive, request)
    /\ ExactDecisionRequestFrozenServeBarrierMaterializationAction(
         archive, request)
    /\ [AsyncNext]_AsyncAllVars
    => /\ ExactDecisionRequestIngressOwned(
            node, qc, archive, request)'
       /\ ExactDecisionServeTombstoneOwned(
            node, qc, archive, request)'
       /\ AsyncServeIngressAdmissionOwned(
            archive,
            ExactDecisionServeLifecycleIdentity(
              archive, request))'
       /\ request
            \in SequenceSet(
                 IngressLane(
                   archive, IngressResourceSource(request)))'
       /\ ExactDecisionRequestFrozenServeBarrierIdentities(
            archive, request)' = {}
BY ExactDecisionRequestFrozenServeBarrierMaterializationLowersRank,
   AsyncBracketNextPreservesStrongTypeInvariant, IsaT(240)
   DEF ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionServeTombstoneOwned,
       ExactDecisionServeLifecycleIdentity,
       ExactDecisionRequestFrozenServeBarrierMaterializationAction,
       ExactDecisionRequestFrozenServeBarrierIdentities,
       AsyncServeLifecycleTombstone,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeTombstoneOutputs,
       AsyncServeReservationsAfterIngressDrain,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       PopSelectedIngress, ResumeExactServeCapacity,
       AcceptOrCoalesceExactServeRequest,
       IngressResourceSource, IngressLane, SequenceSet,
       AsyncAllVars

THEOREM ExactDecisionRequestLifecycleOrdinalCannotResurrect ==
  \A node, qc, archive, request:
    LET identity ==
          ExactDecisionServeLifecycleIdentity(archive, request)
    IN /\ AsyncStrongTypeInvariant
       /\ ExactDecisionRequestLifecycleResidual(
            node, qc, archive, request)
       /\ AsyncServeLifecycleOwned(archive, identity)
       /\ AsyncNext
       /\ AsyncServeLifecycleOwned(archive, identity)'
       => AsyncServeAdmissionOrdinal(archive, identity)'
            = AsyncServeAdmissionOrdinal(archive, identity)
BY IsaT(300)
   DEF ExactDecisionRequestLifecycleResidual,
       ExactDecisionServeLifecycleIdentity,
       AsyncServeAdmissionOrdinal,
       AsyncServeLifecycleOwned,
       AsyncServeLiveReservationOwned,
       AsyncServeLifecycleTombstone,
       AsyncServeReservationRecord,
       AsyncServeTombstoneRecord,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncNetworkStep, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart

THEOREM ExactDecisionServeTombstoneSurvivesSameHeightReplay ==
  \A archive, request:
    LET identity ==
          ExactDecisionServeLifecycleIdentity(archive, request)
        family == AsyncServeLifecycleFamily(archive, request)
    IN /\ AsyncServeFamilyHighWatermarkInvariant
       /\ AsyncServeLifecycleTombstone(archive, identity)
       /\ PreGstResponsiveReplay
       /\ asyncRecoveryNode = archive
       /\ AsyncNext
       => /\ AsyncServeLifecycleTombstone(archive, identity)'
          /\ AsyncServeAdmissionOrdinal(archive, identity)'
               = AsyncServeAdmissionOrdinal(archive, identity)
BY SameHeightRestartRetainsOrConvertsUnreplacedServeTombstone, Isa
   DEF ExactDecisionServeLifecycleIdentity,
       AsyncServeLifecycleTombstone,
       AsyncServeTombstoneRecords,
       AsyncServeFamilyAdmissionRecords,
       AsyncServeRestartAdmissionFamilies,
       AsyncServeFamilyHighWatermarkInvariant,
       AsyncServeAdmissionOrdinal,
       AsyncServeLiveReservationOwned,
       AsyncServeTombstoneRecord,
       PreGstResponsiveReplay, AsyncNext

ExactDecisionRequestIngressFiniteProducerEpisodeAction(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestLifecycleResidual(
       node, qc, archive, request)
  /\ AsyncNext
  /\ ExactDecisionRequestLifecycleResidual(
       node, qc, archive, request)'
  /\ ExactDecisionRequestLifecycleIngressRank(
       node, qc, archive, request)'
       = ExactDecisionRequestLifecycleIngressRank(
           node, qc, archive, request)
  /\ ExactDecisionRequestIngressProducerEpisodeBudget(
       node, qc, archive, request)'
       < ExactDecisionRequestIngressProducerEpisodeBudget(
           node, qc, archive, request)

ExactDecisionRequestLifecycleNoninterferenceAction(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestLifecycleResidual(
       node, qc, archive, request)
  /\ AsyncNext
  /\ ExactDecisionRequestLifecycleResidual(
       node, qc, archive, request)'
  /\ ExactDecisionRequestLifecycleIngressRank(
       node, qc, archive, request)'
       = ExactDecisionRequestLifecycleIngressRank(
           node, qc, archive, request)
  /\ ExactDecisionRequestIngressProducerEpisodeBudget(
       node, qc, archive, request)'
       = ExactDecisionRequestIngressProducerEpisodeBudget(
           node, qc, archive, request)

ExactDecisionRequestLifecycleStepClassification(
    node, qc, archive, request) ==
  /\ ExactDecisionRequestLifecycleResidual(
       node, qc, archive, request)
  /\ AsyncNext
  => \/ ExactDecisionRequestLifecycleGoal(
          node, qc, archive, request)'
     \/ <<ExactDecisionRequestLifecycleIngressRank(
             node, qc, archive, request)',
           ExactDecisionRequestLifecycleIngressRank(
             node, qc, archive, request)>>
          \in ExactDecisionRequestLifecycleIngressRankOrdering
     \/ ExactDecisionRequestIngressFiniteProducerEpisodeAction(
          node, qc, archive, request)
     \/ ExactDecisionRequestLifecycleNoninterferenceAction(
          node, qc, archive, request)

THEOREM ExactDecisionRequestLifecycleStepClassificationIsExhaustive ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ExactDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    /\ AsyncNext
    => ExactDecisionRequestLifecycleStepClassification(
         node, qc, archive, request)
BY ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish,
   ExactDecisionRequestLifecycleFrozenOwnerServiceConsumesBudget,
   ExactDecisionRequestLifecycleFrozenOwnerServiceLowersRank,
   ExactDecisionRequestEarlierIngressOwnerServiceLowersRank,
   ExactDecisionRequestFrozenServeBarrierPredecessorServiceLowersRank,
   ExactDecisionRequestFrozenServeBarrierMaterializationLowersRank,
   ExactDecisionRequestIngressOlderRuntimeInterleaveCannotAscendRank,
   ExactDecisionRequestIngressOlderLocalInterleaveCannotAscendRank,
   ExactDecisionRequestLifecycleOrdinalCannotResurrect,
   ExactDecisionRequestIngressOrdinalPersistsUntilDrain, IsaT(300)
   DEF ExactDecisionRequestLifecycleStepClassification,
       ExactDecisionRequestIngressFiniteProducerEpisodeAction,
       ExactDecisionRequestLifecycleNoninterferenceAction,
       ExactDecisionRequestLifecycleIngressRank,
       ExactDecisionRequestLifecycleStage,
       ExactDecisionRequestLifecycleFrozenPredecessorDebt,
       ExactDecisionRequestLifecycleNestedIngressRank

ExactDecisionRequestLifecycleAtRank(
    node, qc, archive, request, rank) ==
  /\ ExactDecisionRequestLifecycleResidual(
       node, qc, archive, request)
  /\ ExactDecisionRequestLifecycleIngressRank(
       node, qc, archive, request) = rank

ExactDecisionRequestLifecycleAtRankAndBudget(
    node, qc, archive, request, rank, budget) ==
  /\ ExactDecisionRequestLifecycleAtRank(
       node, qc, archive, request, rank)
  /\ ExactDecisionRequestIngressProducerEpisodeBudget(
       node, qc, archive, request) = budget

ExactDecisionRequestLifecycleRankGoal(
    node, qc, archive, request, rank) ==
  \/ ExactDecisionRequestLifecycleGoal(
       node, qc, archive, request)
  \/ <<ExactDecisionRequestLifecycleIngressRank(
          node, qc, archive, request),
        rank>>
       \in ExactDecisionRequestLifecycleIngressRankOrdering

ExactDecisionRequestLifecycleConcreteFairOwnerKinds ==
  {"NormalRunner", "HistoricalServer", "IoWorker"}

\* The I/O worker owns exactly two physical-full cuts.  The first is this
\* target's live off-queue reservation.  The second is the exact singleton
\* Serve barrier frozen ahead of a terminal retry.  When either reservation
\* has a physical slot, the normal/historical runner materializes it after
\* AsyncServeMaterializationPredecessorIndices.  A cached replay with no
\* frozen barrier is always runner-owned and never consults effective Serve
\* capacity.
ExactDecisionRequestLifecycleIoOwnerRequired(archive, request) ==
  LET identity ==
        ExactDecisionServeLifecycleIdentity(archive, request)
      barriers ==
        ExactDecisionRequestFrozenServeBarrierIdentities(
          archive, request)
  IN \/ /\ AsyncServeLiveReservationOwned(archive, identity)
           /\ ~AsyncServeJobQueued(archive, identity)
           /\ ~CanResumeExactServeCapacity(archive, identity)
     \/ /\ barriers # {}
           /\ ~CanResumeExactServeCapacity(
                archive,
                ExactDecisionRequestFrozenServeBarrierIdentity(
                  archive, request))

ExactDecisionRequestLifecycleConcreteFairOwner(archive, request) ==
  IF ExactDecisionRequestLifecycleIoOwnerRequired(archive, request)
  THEN "IoWorker"
  ELSE IF NodeHasApplication(archive)
       THEN "HistoricalServer"
       ELSE "NormalRunner"

ExactDecisionRequestLifecycleConcreteFairAction(archive, ownerKind) ==
  CASE ownerKind = "NormalRunner" ->
         PostGstRunNode(archive)
    [] ownerKind = "HistoricalServer" ->
         PostGstRunHistoricalServer(archive)
    [] ownerKind = "IoWorker" ->
         PostGstServiceIoWorker(archive)
    [] OTHER -> FALSE

ExactDecisionRequestLifecycleSelectedConcreteFairAction(
    archive, request) ==
  ExactDecisionRequestLifecycleConcreteFairAction(
    archive,
    ExactDecisionRequestLifecycleConcreteFairOwner(archive, request))

ExactDecisionRequestLifecycleRankCellOutcome(
    node, qc, archive, request, rank, budget) ==
  \/ ExactDecisionRequestLifecycleRankGoal(
       node, qc, archive, request, rank)
  \/ \E lowerBudget \in SetLessThan(
       budget, OpToRel(<, Nat), Nat):
       ExactDecisionRequestLifecycleAtRankAndBudget(
         node, qc, archive, request, rank, lowerBudget)

ExactDecisionRequestLifecycleRankCellClosureProperty(
    specification) ==
  specification
    => \A node, qc, archive, request:
         \A rank \in ExactDecisionRequestLifecycleIngressRankCarrier:
         ExactDecisionRequestLifecycleAtRank(
           node, qc, archive, request, rank)
           ~> (ExactDecisionRequestLifecycleGoal(
                 node, qc, archive, request)
                \/ <<ExactDecisionRequestLifecycleIngressRank(
                       node, qc, archive, request),
                     rank>>
                     \in
                       ExactDecisionRequestLifecycleIngressRankOrdering)

ExactDecisionRequestLifecycleFiniteProducerEpisodeClosureProperty(
    specification) ==
  specification
    => \A node, qc, archive, request:
         \A rank \in ExactDecisionRequestLifecycleIngressRankCarrier,
            budget \in Nat:
         ExactDecisionRequestLifecycleAtRankAndBudget(
           node, qc, archive, request, rank, budget)
           ~> (ExactDecisionRequestLifecycleRankGoal(
                 node, qc, archive, request, rank)
                \/ \E lowerBudget \in SetLessThan(
                     budget, OpToRel(<, Nat), Nat):
                     ExactDecisionRequestLifecycleAtRankAndBudget(
                       node, qc, archive, request,
                       rank, lowerBudget))

ExactDecisionRequestLifecycleConcreteActionOriginProperty(specification) ==
  /\ specification
       => [](\A node, qc, archive, request:
              \A rank \in ExactDecisionRequestLifecycleIngressRankCarrier,
                 budget \in Nat:
              /\ ExactDecisionRequestLifecycleAtRankAndBudget(
                   node, qc, archive, request, rank, budget)
              /\ ~ExactDecisionRequestLifecycleRankGoal(
                   node, qc, archive, request, rank)
              /\ ExactDecisionRequestIngressContinuationPrefixCleared(
                   archive)
              => /\ ExactDecisionRequestLifecycleConcreteFairOwner(
                       archive, request)
                       \in
                         ExactDecisionRequestLifecycleConcreteFairOwnerKinds
                 /\ ENABLED
                      <<ExactDecisionRequestLifecycleSelectedConcreteFairAction(
                          archive, request)>>_AsyncAllVars)
  /\ specification
       => [](\A node, qc, archive, request:
              \A rank \in ExactDecisionRequestLifecycleIngressRankCarrier,
                 budget \in Nat:
              /\ ExactDecisionRequestLifecycleAtRankAndBudget(
                   node, qc, archive, request, rank, budget)
              /\ ~ExactDecisionRequestLifecycleRankGoal(
                   node, qc, archive, request, rank)
              /\ [AsyncNext]_AsyncAllVars
              => \/ ExactDecisionRequestLifecycleRankCellOutcome(
                      node, qc, archive, request, rank, budget)'
                 \/ /\ ExactDecisionRequestLifecycleAtRankAndBudget(
                           node, qc, archive, request, rank, budget)'
                    /\ ExactDecisionRequestLifecycleConcreteFairOwner(
                         archive, request)'
                         =
                       ExactDecisionRequestLifecycleConcreteFairOwner(
                         archive, request))
  /\ specification
       => [](\A node, qc, archive, request:
              \A rank \in ExactDecisionRequestLifecycleIngressRankCarrier,
                 budget \in Nat:
              /\ ExactDecisionRequestLifecycleAtRankAndBudget(
                   node, qc, archive, request, rank, budget)
              /\ ~ExactDecisionRequestLifecycleRankGoal(
                   node, qc, archive, request, rank)
              /\ ExactDecisionRequestIngressContinuationPrefixCleared(
                   archive)
              /\ <<ExactDecisionRequestLifecycleSelectedConcreteFairAction(
                     archive, request)>>_AsyncAllVars
              => ExactDecisionRequestLifecycleRankCellOutcome(
                   node, qc, archive, request, rank, budget)')

ExactDecisionRequestLifecycleRankDescentProperty(specification) ==
  /\ specification
       => [](\A node, qc, archive, request:
              ExactDecisionRequestLifecycleStepClassification(
                node, qc, archive, request))
  /\ ExactDecisionRequestLifecycleConcreteActionOriginProperty(specification)

(***************************************************************************
The episode closure is derived, not assumed.  Classification excludes rank
ascent and partitions every bracket step into goal, strict rank descent,
same-rank finite-budget consumption, or exact noninterference.  While one
rank/budget cell remains open, the immutable owner geometry selects exactly
one concrete archive action.  Selection persists while that cell persists.
The action is one of the three actions already named by `AsyncFairnessAt`;
there is no weak-fairness premise for an action/progress intersection.
***************************************************************************)
THEOREM ExactDecisionRequestLifecycleSelectedActionEnabledAtEpisode ==
  \A node, qc, archive, request, rank, budget:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ ExactDecisionRequestIngressContinuationPrefixCleared(archive)
    /\ ExactDecisionRequestLifecycleAtRankAndBudget(
         node, qc, archive, request, rank, budget)
    /\ ~ExactDecisionRequestLifecycleRankGoal(
         node, qc, archive, request, rank)
    => ENABLED
         <<ExactDecisionRequestLifecycleSelectedConcreteFairAction(
             archive, request)>>_AsyncAllVars
BY QueuedIoEnablesPostGstService,
   QueuedIoServiceIsNonstuttering,
   GstResponsiveUnappliedRunNodeIsEnabled,
   RunNodeIsNonstuttering,
   GstHistoricalServerIsEnabled,
   ExpandENABLED, ENABLEDaxioms, IsaT(300)
   DEF ExactDecisionRequestLifecycleSelectedConcreteFairAction,
       ExactDecisionRequestLifecycleConcreteFairAction,
       ExactDecisionRequestLifecycleConcreteFairOwner,
       ExactDecisionRequestLifecycleConcreteFairOwnerKinds,
       ExactDecisionRequestLifecycleIoOwnerRequired,
       ExactDecisionRequestFrozenServeBarrierIdentities,
       ExactDecisionRequestFrozenServeBarrierIdentity,
       ExactDecisionRequestLifecycleAtRankAndBudget,
       ExactDecisionRequestLifecycleAtRank,
       ExactDecisionRequestLifecycleRankGoal,
       ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       CanResumeExactServeCapacity,
       AsyncServeJobQueued,
       AsyncServeLiveReservationOwned,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServeIngressIdentityFrozenByReservation,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeEarliestIngressLifecycleOwnerIdentity,
       AsyncServeBarrierOwnsEarliestIngressOrdinalInvariant,
       AsyncServeIngressIndexMayPrecedeAdmittedTarget,
       AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncCurrentResponsiveVoters,
       AsyncIoQueueDepth, AsyncAllVars

THEOREM ExactDecisionRequestLifecycleBracketStepPreservesEpisodeOrGoal ==
  \A node, qc, archive, request, rank, budget:
    /\ ExactDecisionRequestLifecycleStepClassification(
         node, qc, archive, request)
    /\ ExactDecisionRequestLifecycleAtRankAndBudget(
         node, qc, archive, request, rank, budget)
    /\ [AsyncNext]_AsyncAllVars
    => \/ ExactDecisionRequestLifecycleAtRankAndBudget(
            node, qc, archive, request, rank, budget)'
       \/ ExactDecisionRequestLifecycleRankGoal(
            node, qc, archive, request, rank)'
       \/ \E lowerBudget \in SetLessThan(
            budget, OpToRel(<, Nat), Nat):
            ExactDecisionRequestLifecycleAtRankAndBudget(
              node, qc, archive, request, rank, lowerBudget)'
BY IsaT(300)
   DEF ExactDecisionRequestLifecycleStepClassification,
       ExactDecisionRequestIngressFiniteProducerEpisodeAction,
       ExactDecisionRequestLifecycleNoninterferenceAction,
       ExactDecisionRequestLifecycleAtRankAndBudget,
       ExactDecisionRequestLifecycleAtRank,
       ExactDecisionRequestLifecycleRankGoal,
       SetLessThan, AsyncAllVars

THEOREM ExactDecisionRequestLifecycleConcreteOwnerPersistsInRankCell ==
  \A node, qc, archive, request, rank, budget:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ExactDecisionRequestLifecycleAtRankAndBudget(
         node, qc, archive, request, rank, budget)
    /\ ~ExactDecisionRequestLifecycleRankGoal(
         node, qc, archive, request, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ ExactDecisionRequestLifecycleAtRankAndBudget(
         node, qc, archive, request, rank, budget)'
    => ExactDecisionRequestLifecycleConcreteFairOwner(
         archive, request)'
         =
       ExactDecisionRequestLifecycleConcreteFairOwner(
         archive, request)
BY ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish,
   ExactDecisionRequestLifecycleOrdinalCannotResurrect,
   ExactDecisionRequestIngressOrdinalPersistsUntilDrain, IsaT(300)
   DEF ExactDecisionRequestLifecycleConcreteFairOwner,
       ExactDecisionRequestLifecycleIoOwnerRequired,
       ExactDecisionRequestFrozenServeBarrierIdentities,
       ExactDecisionRequestFrozenServeBarrierIdentity,
       ExactDecisionRequestLifecycleAtRankAndBudget,
       ExactDecisionRequestLifecycleAtRank,
       ExactDecisionRequestLifecycleRankGoal,
       ExactDecisionRequestLifecycleIngressRank,
       ExactDecisionRequestLifecycleStage,
       ExactDecisionRequestLifecycleFrozenPredecessorDebt,
       ExactDecisionRequestLifecycleFrozenPredecessorSet,
       ExactDecisionRequestIngressProducerEpisodeBudget,
       ExactDecisionRequestIngressProducerEpisodeOwnerSet,
       CanResumeExactServeCapacity,
       AsyncServeJobQueued,
       AsyncServeLiveReservationOwned,
       AsyncServeFrozenPredecessorSet,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServeIngressIdentityFrozenByReservation,
       AsyncServeIngressAdmissionOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeEarliestIngressLifecycleOwnerIdentity,
       AsyncServeBarrierOwnsEarliestIngressOrdinalInvariant,
       AsyncAllVars

THEOREM ExactDecisionRequestLifecycleSelectedActionConsumesEpisode ==
  \A node, qc, archive, request, rank, budget:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ ExactDecisionRequestIngressContinuationPrefixCleared(archive)
    /\ ExactDecisionRequestLifecycleAtRankAndBudget(
         node, qc, archive, request, rank, budget)
    /\ ~ExactDecisionRequestLifecycleRankGoal(
         node, qc, archive, request, rank)
    /\ <<ExactDecisionRequestLifecycleSelectedConcreteFairAction(
           archive, request)>>_AsyncAllVars
    => ExactDecisionRequestLifecycleRankCellOutcome(
         node, qc, archive, request, rank, budget)'
BY ExactDecisionRequestIngressIoServicePersistsAndLowers,
   ExactDecisionRequestIngressLocalProducerCannotAscendRank,
   ExactDecisionRequestIngressOlderLocalInterleaveCannotAscendRank,
   ExactDecisionRequestIngressRuntimeActivationCannotAscendRank,
   ExactDecisionRequestIngressRunnerActionCreatesGoal,
   ExactDecisionRequestEarlierIngressOwnerServiceLowersRank,
   ExactDecisionRequestFrozenServeBarrierPredecessorServiceLowersRank,
   ExactDecisionRequestFrozenServeBarrierMaterializationLowersRank,
   ExactDecisionRequestLifecycleFrozenOwnerServiceConsumesBudget,
   ExactDecisionRequestLifecycleFrozenOwnerServiceLowersRank,
   ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish,
   IsaT(600)
   DEF ExactDecisionRequestLifecycleSelectedConcreteFairAction,
       ExactDecisionRequestLifecycleConcreteFairAction,
       ExactDecisionRequestLifecycleConcreteFairOwner,
       ExactDecisionRequestLifecycleIoOwnerRequired,
       ExactDecisionRequestFrozenServeBarrierIdentities,
       ExactDecisionRequestFrozenServeBarrierIdentity,
       ExactDecisionRequestLifecycleRankCellOutcome,
       ExactDecisionRequestLifecycleAtRankAndBudget,
       ExactDecisionRequestLifecycleAtRank,
       ExactDecisionRequestLifecycleRankGoal,
       ExactDecisionRequestLifecycleIngressRank,
       ExactDecisionRequestLifecycleStage,
       ExactDecisionRequestLifecycleFrozenOwnerServiceAction,
       ExactDecisionRequestEarlierIngressOwnerServiceAction,
       ExactDecisionRequestIngressRunnerAction,
       ExactDecisionNormalRequestIngressRunnerAction,
       ExactDecisionHistoricalRequestIngressRunnerAction,
       ExactDecisionRequestEarlierServeMaterializationAction,
       ExactDecisionRequestFrozenServeBarrierPredecessorServiceAction,
       ExactDecisionRequestFrozenServeBarrierMaterializationAction,
       ExactDecisionRequestFrozenServeBarrierIdentities,
       ExactDecisionRequestFrozenServeBarrierIdentity,
       CanResumeExactServeCapacity,
       ExactDecisionRequestIngressServeCapacityDebt,
       AsyncIoEffectiveQueueDepth,
       AsyncServeOffQueueReservations,
       AsyncServeJobQueued, AsyncServeLiveReservationOwned,
       AsyncServeMaterializationPredecessorIndices,
       AsyncServeEarlierOrdinalIndices,
       AsyncServeRemainingPredecessorIndices,
       SetLessThan, AsyncAllVars

THEOREM ExactDecisionRequestLifecycleConcreteOwnerUsesAsyncFairness ==
  \A initialContext, archive, ownerKind:
    (/\ archive \in AsyncVotersAt(initialContext)
     /\ archive \in Responsive
     /\ ownerKind
          \in ExactDecisionRequestLifecycleConcreteFairOwnerKinds)
      => (AsyncSpecAt(initialContext)
            => WF_AsyncAllVars(
                 ExactDecisionRequestLifecycleConcreteFairAction(
                   archive, ownerKind)))
BY Isa, PTL
   DEF AsyncSpecAt, AsyncFairnessAt,
       ExactDecisionRequestLifecycleConcreteFairOwnerKinds,
       ExactDecisionRequestLifecycleConcreteFairAction

THEOREM AsyncSpecProvidesExactDecisionRequestLifecycleConcreteActionOrigin ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => ExactDecisionRequestLifecycleConcreteActionOriginProperty(
           AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   ExactDecisionRequestLifecycleSelectedActionEnabledAtEpisode,
   ExactDecisionRequestLifecycleBracketStepPreservesEpisodeOrGoal,
   ExactDecisionRequestLifecycleConcreteOwnerPersistsInRankCell,
   ExactDecisionRequestLifecycleSelectedActionConsumesEpisode,
   Isa, PTL
   DEF ExactDecisionRequestLifecycleConcreteActionOriginProperty,
       ExactDecisionRequestLifecycleRankCellOutcome

THEOREM AsyncSpecProvidesExactDecisionRequestLifecycleRankDescent ==
  \A initialContext:
    ExactDecisionRequestLifecycleRankDescentProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesExactDecisionRequestLifecycleConcreteActionOrigin,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   ExactDecisionRequestLifecycleStepClassificationIsExhaustive,
   PTL
   DEF ExactDecisionRequestLifecycleRankDescentProperty

THEOREM ExactDecisionRequestRankDescentDerivesFiniteEpisodeClosure ==
  \A initialContext:
    /\ ExactDecisionRequestLifecycleRankDescentProperty(
         AsyncSpecAt(initialContext))
    /\ ExactDecisionRequestIngressContinuationPrefixClosureProperty(
         AsyncSpecAt(initialContext))
      => ExactDecisionRequestLifecycleFiniteProducerEpisodeClosureProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ExactDecisionRequestLifecycleRankDescentProperty(
                  AsyncSpecAt(initialContext)),
                ExactDecisionRequestIngressContinuationPrefixClosureProperty(
                  AsyncSpecAt(initialContext))
         PROVE
           ExactDecisionRequestLifecycleFiniteProducerEpisodeClosureProperty(
             AsyncSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                   /\ AsyncProgressOwnershipInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
    <2>2. ASSUME AsyncSpecAt(initialContext),
                  NEW node, NEW qc, NEW archive, NEW request,
                  NEW rank
                    \in ExactDecisionRequestLifecycleIngressRankCarrier,
                  NEW budget \in Nat
           PROVE
             ExactDecisionRequestLifecycleAtRankAndBudget(
               node, qc, archive, request, rank, budget)
               ~> (ExactDecisionRequestLifecycleRankGoal(
                     node, qc, archive, request, rank)
                    \/ \E lowerBudget \in SetLessThan(
                         budget, OpToRel(<, Nat), Nat):
                         ExactDecisionRequestLifecycleAtRankAndBudget(
                           node, qc, archive, request,
                           rank, lowerBudget))
      BY <1>1, <2>1,
         AsyncSpecAlwaysUsesFixedResponsiveVoters,
         ExactDecisionRequestLifecycleConcreteOwnerUsesAsyncFairness,
         PTL
         DEF ExactDecisionRequestLifecycleRankDescentProperty,
             ExactDecisionRequestLifecycleConcreteActionOriginProperty,
             ExactDecisionRequestIngressContinuationPrefixClosureProperty,
             ExactDecisionRequestIngressContinuationPrefixGoal,
             ExactDecisionRequestIngressContinuationPrefixCleared,
             ExactDecisionNormalRequestIngressContinuationPrefixBlocked,
             ExactDecisionRequestLifecycleAtRankAndBudget,
             ExactDecisionRequestLifecycleAtRank,
             ExactDecisionRequestLifecycleResidual,
             ExactDecisionRequestIngressLaneResidual,
             ExactDecisionRequestIngressOwned,
             ExactDecisionBodyHoldingAlias,
             ExactDecisionActiveRequestOwner,
             ExactDecisionServiceSource,
             ExactDecisionRequestLifecycleRankCellOutcome
    <2> QED BY <2>2
         DEF ExactDecisionRequestLifecycleFiniteProducerEpisodeClosureProperty
  <1> QED BY <1>1

THEOREM ExactDecisionRequestFiniteProducerEpisodeClosesAtRank ==
  \A initialContext:
    /\ ExactDecisionRequestLifecycleRankDescentProperty(
         AsyncSpecAt(initialContext))
    /\ ExactDecisionRequestIngressContinuationPrefixClosureProperty(
         AsyncSpecAt(initialContext))
      => ExactDecisionRequestLifecycleRankCellClosureProperty(
           AsyncSpecAt(initialContext))
BY ExactDecisionRequestRankDescentDerivesFiniteEpisodeClosure,
   ExactDecisionRequestIngressProducerEpisodeBudgetIsFinite,
   ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish,
   ExactDecisionRequestLifecycleOrdinalCannotResurrect,
   NatLessThanWellFounded, WellFoundedLeadsTo, PTL
   DEF ExactDecisionRequestLifecycleFiniteProducerEpisodeClosureProperty,
       ExactDecisionRequestLifecycleRankCellClosureProperty,
       ExactDecisionRequestLifecycleAtRankAndBudget,
       ExactDecisionRequestLifecycleRankGoal

ExactDecisionRequestLifecycleConvergenceProperty(specification) ==
  specification
    => \A node, qc, archive, request:
         ExactDecisionRequestLifecycleResidual(
           node, qc, archive, request)
           ~> ExactDecisionRequestLifecycleGoal(
                node, qc, archive, request)

THEOREM ExactDecisionRequestLifecycleRankDescentClosesLifecycle ==
  \A initialContext:
    /\ ExactDecisionRequestLifecycleRankDescentProperty(
         AsyncSpecAt(initialContext))
    /\ ExactDecisionRequestIngressContinuationPrefixClosureProperty(
         AsyncSpecAt(initialContext))
      => ExactDecisionRequestLifecycleConvergenceProperty(
           AsyncSpecAt(initialContext))
BY ExactDecisionRequestFiniteProducerEpisodeClosesAtRank,
   ExactDecisionRequestLifecycleIngressRankOrderingIsWellFounded,
   ExactDecisionRequestLifecycleIngressRankInCarrier,
   WellFoundedLeadsTo, PTL
   DEF ExactDecisionRequestLifecycleConvergenceProperty,
       ExactDecisionRequestLifecycleRankCellClosureProperty,
       ExactDecisionRequestLifecycleAtRank

ExactDecisionRequestIngressRankReplenishmentResidual(
    node, qc, archive, request) ==
  \/ ExactDecisionRequestIngressCausalReplenishmentResidual(
       node, qc, archive, request)
  \/ ExactDecisionRequestIngressServeReplenishmentResidual(
       node, qc, archive, request)
  \/ ExactDecisionRequestIngressPriorityReplenishmentResidual(
       node, qc, archive, request)

THEOREM ExactDecisionRequestIngressReplenishmentHasConcreteActionWitness ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionRequestIngressRankReplenishmentResidual(
         node, qc, archive, request)
    => \E producerClass
           \in ExactDecisionRequestIngressProducerClasses:
         ENABLED
           <<ExactDecisionRequestIngressConcreteReplenishmentAction(
               node, qc, archive, request,
               producerClass)>>_AsyncAllVars
BY ExactDecisionRequestIngressCausalReplenishmentHasConcreteProducer,
   ExactDecisionRequestIngressServeReplenishmentHasConcreteProducer,
   ExactDecisionRequestIngressPriorityReplenishmentHasConcreteProducer,
   ExpandENABLED, IsaT(300)
   DEF ExactDecisionRequestIngressRankReplenishmentResidual,
       ExactDecisionRequestIngressCausalReplenishmentResidual,
       ExactDecisionRequestIngressServeReplenishmentResidual,
       ExactDecisionRequestIngressPriorityReplenishmentResidual,
       ExactDecisionRequestIngressConcreteReplenishmentAction,
       ExactDecisionRequestIngressProducerClasses,
       AsyncAllVars

ExactDecisionRequestIngressLaneRunnerConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, archive, request:
         ExactDecisionRequestIngressLaneResidual(
           node, qc, archive, request)
           ~> ExactDecisionRequestIngressGoal(
                node, qc, archive, request)

ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty(
    specification) ==
  /\ ExactDecisionRequestLifecycleRankDescentProperty(specification)
  /\ ExactDecisionRequestIngressContinuationPrefixClosureProperty(
       specification)

THEOREM ExactDecisionRequestAdmissionCoalescingOutcomeIsDischarged ==
  \A initialContext:
    ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesExactDecisionRequestLifecycleRankDescent,
   AsyncSpecClosesExactDecisionRequestIngressContinuationPrefix
   DEF ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty

THEOREM ExactDecisionRequestAdmissionCoalescingClosesLaneRunner ==
  \A initialContext:
    ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty(
      AsyncSpecAt(initialContext))
      => ExactDecisionRequestIngressLaneRunnerConvergenceProperty(
           AsyncSpecAt(initialContext))
BY ExactDecisionRequestLifecycleRankDescentClosesLifecycle, PTL
   DEF ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty,
       ExactDecisionRequestIngressLaneRunnerConvergenceProperty,
       ExactDecisionRequestLifecycleConvergenceProperty,
       ExactDecisionRequestLifecycleResidual,
       ExactDecisionRequestLifecycleGoal,
       ExactDecisionRequestIngressLaneResidual,
       ExactDecisionRequestIngressGoal

THEOREM ExactDecisionRequestAdmissionCoalescingLaneRunnerConverges ==
  \A initialContext:
    ExactDecisionRequestIngressLaneRunnerConvergenceProperty(
      AsyncSpecAt(initialContext))
BY ExactDecisionRequestAdmissionCoalescingOutcomeIsDischarged,
   ExactDecisionRequestAdmissionCoalescingClosesLaneRunner

ExactDecisionRequestIngressResidualConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, archive, request, packet:
         ExactDecisionRequestIngressResidual(
           node, qc, archive, request, packet)
           ~> ExactDecisionRequestIngressGoal(
                node, qc, archive, request)

THEOREM ExactDecisionRequestIngressKernelsDischargeResidual ==
  \A initialContext:
    /\ ExactDecisionRequestHeadGateOwnerConvergenceProperty(
         AsyncSpecAt(initialContext))
    /\ ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty(
         AsyncSpecAt(initialContext))
    => ExactDecisionRequestIngressResidualConvergenceProperty(
         AsyncSpecAt(initialContext))
BY ExactDecisionRequestAdmissionHandoffConvergence,
   ExactDecisionRequestAdmissionCoalescingClosesLaneRunner,
   ExactDecisionRequestIngressResidualSplitsAtAdmissionReady, PTL
   DEF ExactDecisionRequestHeadGateOwnerConvergenceProperty,
       ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty,
       ExactDecisionRequestAdmissionHandoffConvergenceProperty,
       ExactDecisionRequestIngressResidualConvergenceProperty,
       ExactDecisionRequestHeadGateOwnerResidual,
       ExactDecisionRequestAdmissionOutcome

(***************************************************************************
Protected Serve FIFO closure under the actual exact-corridor specification.

The generic Serve FIFO rank leaf is already proved under `AsyncSpecAt`; it
uses the live `Nat` generation domain.  Checked exhaustion in bounded TLC
instances is diagnostic only and adds no liveness premise for unrelated
Stage-2 candidate work.  The older aggregate starvation bridge was stated only for
`AsyncLiveSpecAt`, so the two lemmas below specialize its well-founded
natural-position argument to the weaker specification used here.
***************************************************************************)

THEOREM ProtectedServeRankStepAtAsyncSpec ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => \A node \in Responsive,
            job \in AsyncServeJobSet:
           \A position \in Nat:
             ProtectedServeOwnedAtServiceRank(
               node, job, <<5, position>>)
               ~> (ProtectedServeOwnershipExit(node, job)
                    \/ \E lower \in SetLessThan(
                         position, OpToRel(<, Nat), Nat):
                         ProtectedServeOwnedAtServiceRank(
                           node, job, <<5, lower>>))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext)
         PROVE \A node \in Responsive,
                   job \in AsyncServeJobSet:
                 \A position \in Nat:
                   ProtectedServeOwnedAtServiceRank(
                     node, job, <<5, position>>)
                     ~> (ProtectedServeOwnershipExit(node, job)
                          \/ \E lower \in SetLessThan(
                               position, OpToRel(<, Nat), Nat):
                               ProtectedServeOwnedAtServiceRank(
                                 node, job, <<5, lower>>))
    <2>1. ProtectedServeRankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedServeRankProgressFromFairFifo
    <2>2. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>3. [](gst => []gst)
      BY <1>1, AsyncSpecKeepsGstOnceSet
    <2>4. ASSUME NEW node \in Responsive,
                    NEW job \in AsyncServeJobSet
           PROVE \A position \in Nat:
                   ProtectedServeOwnedAtServiceRank(
                     node, job, <<5, position>>)
                     ~> (ProtectedServeOwnershipExit(node, job)
                          \/ \E lower \in SetLessThan(
                               position, OpToRel(<, Nat), Nat):
                               ProtectedServeOwnedAtServiceRank(
                                 node, job, <<5, lower>>))
      <3>1. ASSUME NEW position \in Nat
             PROVE ProtectedServeOwnedAtServiceRank(
                       node, job, <<5, position>>)
                       ~> (ProtectedServeOwnershipExit(node, job)
                            \/ \E lower \in SetLessThan(
                                 position, OpToRel(<, Nat), Nat):
                                 ProtectedServeOwnedAtServiceRank(
                                   node, job, <<5, lower>>))
        <4>1. ProtectedServeOwnedAtServiceRank(
                 node, job, <<5, position>>)
                 ~> (ProtectedServeOwnershipExit(node, job)
                      \/ ServiceRankLess(
                           ServeJobRank(node, job), <<5, position>>))
          BY <1>1, <2>1, <2>4
             DEF ProtectedServeRankProgressProperty,
                 ProtectedServeOwnedAtServiceRank,
                 ProtectedServeOwnershipExit
        <4>2. /\ AsyncStrongTypeInvariant
                 /\ gst
                 /\ ~ProtectedServeOwnershipExit(node, job)
                 /\ ServiceRankLess(
                      ServeJobRank(node, job), <<5, position>>)
                => \E lower \in SetLessThan(
                     position, OpToRel(<, Nat), Nat):
                     ProtectedServeOwnedAtServiceRank(
                       node, job, <<5, lower>>)
          BY <3>1, ProtectedServeRankExitHasWellFoundedSuccessor
        <4> QED BY <2>2, <2>3, <4>1, <4>2, PTL
      <3> QED BY <3>1
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM ProtectedServeStarvationAtAsyncSpec ==
  \A initialContext:
    ProtectedServeStarvationProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ProtectedServeStarvationProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node \in Responsive,
                     job \in AsyncServeJobSet:
                  (gst
                    /\ ResponsiveProtectedServeJobOwned(node, job))
                    ~> ~ResponsiveProtectedServeJobOwned(node, job)
      <3>1. ASSUME NEW node \in Responsive,
                    NEW job \in AsyncServeJobSet
             PROVE (gst
                      /\ ResponsiveProtectedServeJobOwned(node, job))
                      ~> ~ResponsiveProtectedServeJobOwned(node, job)
        <4>1. \A position \in Nat:
                 ProtectedServeOwnedAtServiceRank(
                   node, job, <<5, position>>)
                   ~> (ProtectedServeOwnershipExit(node, job)
                        \/ \E lower \in SetLessThan(
                             position, OpToRel(<, Nat), Nat):
                             ProtectedServeOwnedAtServiceRank(
                               node, job, <<5, lower>>))
          BY <2>1, <3>1, ProtectedServeRankStepAtAsyncSpec
        <4>2. \A position \in Nat:
                 ProtectedServeOwnedAtServiceRank(
                   node, job, <<5, position>>)
                   ~> ProtectedServeOwnershipExit(node, job)
          BY ONLY <4>1, ProtectedServeWellFoundedRankConvergence,
             SMT
        <4>3. (\E position \in Nat:
                   ProtectedServeOwnedAtServiceRank(
                     node, job, <<5, position>>))
                   ~> ProtectedServeOwnershipExit(node, job)
          BY ONLY <4>2, ProtectedServeRankExistentialLift, SMT
        <4>4. []AsyncStrongTypeInvariant
          BY <2>1, AsyncSpecAlwaysStrongTypeInvariant
        <4>5. [](gst
                   /\ ResponsiveProtectedServeJobOwned(node, job)
                  => \E position \in Nat:
                       ProtectedServeOwnedAtServiceRank(
                         node, job, <<5, position>>))
          BY <4>4, ResponsiveProtectedServeJobHasRankPositionAt,
             PTL
        <4> QED BY <4>3, <4>5, PTL
             DEF ProtectedServeOwnershipExit
      <3> QED BY <3>1
    <2> QED BY <2>1 DEF ProtectedServeStarvationProperty
  <1> QED BY <1>1

(***************************************************************************
Exact Serve exit safety.

The protected FIFO theorem now guarantees that the nonce-owned occurrence
leaves the queue under `AsyncSpecAt`.  It intentionally says nothing about
the exact Decision alias carried by that occurrence.  The remaining narrow
kernel is therefore safety, not another scheduler-liveness assumption: until
the exact response goal appears, the exact alias and nonce-owned occurrence
must persist together.

The physical target-head exit is already covered by
`ExactDecisionServeResidualHeadExitCreatesGoal`; it executes
`ServiceIoWorkerWork` and publishes the authenticated exact response packet.
Proving the kernel below requires only classifying the other actions as exact
owner-preserving or semantic-goal-producing.  It does not assume broad Serve,
off-scheduler, or stage convergence.
***************************************************************************)

ExactDecisionServeResidualPersistsOrGoals(
    node, qc, archive, request, job) ==
  ExactDecisionServeResponseResidual(
    node, qc, archive, request, job)
    => \/ ExactDecisionServeResponseResidual(
            node, qc, archive, request, job)'
       \/ ExactDecisionServeResponseGoal(
            node, qc, archive, request)'

THEOREM ExactDecisionServeResidualStepIsSafe ==
  \A node, qc, archive, request, job:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ ResponsiveRecoveryValidationClearedInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ ExactDecisionFanoutRetentionInvariant
    /\ [AsyncNext]_AsyncAllVars
    => ExactDecisionServeResidualPersistsOrGoals(
         node, qc, archive, request, job)
BY ExactDecisionBodyHoldingAliasPersistsOrFrontier,
   ExactDecisionServeOccurrencePersistsOrHeadServiced,
   ExactDecisionServeResidualHeadExitCreatesGoal, Isa
   DEF ExactDecisionServeResidualPersistsOrGoals,
       ExactDecisionServeResponseResidual,
       ExactDecisionServeResponseGoal,
       ExactDecisionServeJobOwned

ExactDecisionServeExitSafetyKernelProperty(specification) ==
  specification
    => \A node, qc, archive, request, job:
         [][ExactDecisionServeResidualPersistsOrGoals(
              node, qc, archive, request, job)]_AsyncAllVars

ExactDecisionServeResponseResidualConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, archive, request, job:
         ExactDecisionServeResponseResidual(
           node, qc, archive, request, job)
           ~> ExactDecisionServeResponseGoal(
                node, qc, archive, request)

THEOREM ExactDecisionServeExitSafetyKernel ==
  \A initialContext:
    ExactDecisionServeExitSafetyKernelProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ExactDecisionServeExitSafetyKernelProperty(
                 AsyncSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
             => [](/\ AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ DecisionFrontierUniquenessInvariant
                    /\ DecisionTimeoutFrontierInvariant
                    /\ ResponsiveRecoveryValidationClearedInvariant
                    /\ FinalProgressWitnessClosureInvariant
                    /\ ExactDecisionFanoutRetentionInvariant
                    /\ ExactDecisionRequestAuthorityIsolationInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         DecisionFrontierUniquenessInvariantFromAsyncSpec,
         DecisionTimeoutFrontierInvariantFromAsyncSpec,
         ResponsiveRecoveryValidationClearedInvariantObligation,
         FinalProgressWitnessClosureInvariantObligation,
         AsyncSpecAlwaysRetainsExactDecisionFanout,
         AsyncSpecAlwaysIsolatesExactDecisionRequestAuthority, PTL
    <2>2. \A node, qc, archive, request, job:
             /\ AsyncStrongTypeInvariant
             /\ AsyncProgressOwnershipInvariant
             /\ DecisionFrontierUniquenessInvariant
             /\ DecisionTimeoutFrontierInvariant
             /\ ResponsiveRecoveryValidationClearedInvariant
             /\ FinalProgressWitnessClosureInvariant
             /\ ExactDecisionFanoutRetentionInvariant
             /\ [AsyncNext]_AsyncAllVars
             => [ExactDecisionServeResidualPersistsOrGoals(
                   node, qc, archive, request, job)]_AsyncAllVars
      BY ExactDecisionServeResidualStepIsSafe
    <2>3. AsyncSpecAt(initialContext)
             => [][AsyncNext]_AsyncAllVars
      BY DEF AsyncSpecAt
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF ExactDecisionServeExitSafetyKernelProperty
  <1> QED BY <1>1

THEOREM ExactDecisionServeExitSafetyDischargesResidual ==
  \A initialContext:
    ExactDecisionServeExitSafetyKernelProperty(
      AsyncSpecAt(initialContext))
      => ExactDecisionServeResponseResidualConvergenceProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ExactDecisionServeExitSafetyKernelProperty(
                  AsyncSpecAt(initialContext))
         PROVE ExactDecisionServeResponseResidualConvergenceProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ProtectedServeStarvationProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedServeStarvationAtAsyncSpec
    <2>2. AsyncSpecAt(initialContext)
            => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>3. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node, qc, archive, request, job:
                    ExactDecisionServeResponseResidual(
                      node, qc, archive, request, job)
                      ~> ExactDecisionServeResponseGoal(
                           node, qc, archive, request)
      <3>1. ASSUME NEW node, NEW qc, NEW archive,
                    NEW request, NEW job
             PROVE ExactDecisionServeResponseResidual(
                     node, qc, archive, request, job)
                     ~> ExactDecisionServeResponseGoal(
                          node, qc, archive, request)
        <4>1. [](ExactDecisionServeResponseResidual(
                   node, qc, archive, request, job)
                  => /\ gst
                     /\ archive \in Responsive
                     /\ job \in AsyncServeJobSet
                     /\ ResponsiveProtectedServeJobOwned(
                          archive, job))
          BY <2>2, ExactDecisionServeResidualProjectsProtectedOwner,
             PTL
        <4>2. ExactDecisionServeResponseResidual(
                 node, qc, archive, request, job)
                 ~> ~ResponsiveProtectedServeJobOwned(archive, job)
          BY <2>1, <2>3, <4>1, PTL
             DEF ProtectedServeStarvationProperty
        <4>3. [][ExactDecisionServeResidualPersistsOrGoals(
                    node, qc, archive, request, job)]_AsyncAllVars
          BY <1>1, <2>3
             DEF ExactDecisionServeExitSafetyKernelProperty
        <4> QED BY <4>1, <4>2, <4>3, PTL
             DEF ExactDecisionServeResidualPersistsOrGoals
      <3> QED BY <3>1
    <2> QED BY <2>3
         DEF ExactDecisionServeResponseResidualConvergenceProperty
  <1> QED BY <1>1

THEOREM ExactDecisionServeResponseResidualConvergence ==
  \A initialContext:
    ExactDecisionServeResponseResidualConvergenceProperty(
      AsyncSpecAt(initialContext))
BY ExactDecisionServeExitSafetyKernel,
   ExactDecisionServeExitSafetyDischargesResidual

(***************************************************************************
Exact claimed-response exit safety.

The generic recipient-local retry theorem proves that a physical claim is
eventually retired or the node applies.  This exact leaf additionally retains
the frozen Decision lineage while that physical owner exists.  Therefore the
generic exit cannot silently discharge an unrelated response: consuming the
claim creates the exact FetchCertifiedBody frontier, while every lifecycle
exit which can retire the request is already an exact executable/application
goal.
***************************************************************************)

THEOREM ExactDecisionResponseClaimResidualProjectsRunnerOwned ==
  \A node, qc, response:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionResponseClaimIngressResidual(node, qc, response)
    => /\ node \in ValidatorIds
       /\ gst
       /\ CertifiedResponseClaimRunnerOwned(node)
BY AsyncCurrentResponsiveVotersAreValidators, IsaT(180)
   DEF ExactDecisionResponseClaimIngressResidual,
       ExactDecisionRouteNeutralClaimIngressOwned,
       ExactDecisionResponseAdmissionGoal,
       ExactDecisionExecutableFrontier,
       ExactDecisionServiceSource,
       CertifiedResponseClaimRunnerOwned,
       CertifiedResponseClaimMatches,
       CertifiedResponseClaimsAt,
       DecisionCertifiedResponseLineageExact

ExactDecisionResponseClaimResidualPersistsOrGoals(node, qc, response) ==
  ExactDecisionResponseClaimIngressResidual(node, qc, response)
    => \/ ExactDecisionResponseClaimIngressResidual(
            node, qc, response)'
       \/ ExactDecisionResponseAdmissionGoal(node, qc)'

THEOREM ExactDecisionResponseClaimResidualStepIsSafe ==
  \A node, qc, response:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ ResponsiveRecoveryValidationClearedInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ ExactDecisionFanoutRetentionInvariant
    /\ [AsyncNext]_AsyncAllVars
    => ExactDecisionResponseClaimResidualPersistsOrGoals(
         node, qc, response)
BY ExactDecisionNormalResponseDrainCreatesAdmissionGoal,
   AsyncBracketStepRetainsExactDecisionRecord,
   AsyncBracketStepLeavesContext,
   AsyncNextPreservesExactDecisionFanoutRetention,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesFinalProgressWitnessClosure,
   GstAsyncStepIsMonotone, IsaT(300)
   DEF ExactDecisionResponseClaimResidualPersistsOrGoals,
       ExactDecisionResponseClaimIngressResidual,
       ExactDecisionRouteNeutralClaimIngressOwned,
       ExactDecisionResponseAdmissionGoal,
       ExactDecisionExecutableFrontier,
       ExactDecisionExecutableOwner,
       ExactDecisionServiceSource,
       ExactDecisionRecord,
       DecisionCertifiedResponseLineageExact,
       CertifiedResponseClaimMatches,
       CertifiedResponseClaimIngressOwner,
       AsyncCertifiedResponseClaimIngressOwnershipInvariant,
       DrainFairIngressSelected, ExactDecisionResponseNormalDrainAction,
       CertifiedResponseClaimsAt, CertifiedResponseClaimForRequests,
       MatchingCertifiedRequests, AsyncCertifiedRequestHash,
       AsyncCertifiedResponseCanonicalWireIdentity,
       AsyncCertifiedResponseAuthProjection,
       IngressResourceSource, IngressLane, IngressLaneDepth,
       SequenceSet

ExactDecisionResponseClaimExitSafetyKernelProperty(specification) ==
  specification
    => \A node, qc, response:
         [][ExactDecisionResponseClaimResidualPersistsOrGoals(
              node, qc, response)]_AsyncAllVars

THEOREM ExactDecisionResponseClaimExitSafetyKernel ==
  \A initialContext:
    ExactDecisionResponseClaimExitSafetyKernelProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ExactDecisionResponseClaimExitSafetyKernelProperty(
                 AsyncSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
             => [](/\ AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ DecisionFrontierUniquenessInvariant
                    /\ DecisionTimeoutFrontierInvariant
                    /\ ResponsiveRecoveryValidationClearedInvariant
                    /\ FinalProgressWitnessClosureInvariant
                    /\ ExactDecisionFanoutRetentionInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         DecisionFrontierUniquenessInvariantFromAsyncSpec,
         DecisionTimeoutFrontierInvariantFromAsyncSpec,
         ResponsiveRecoveryValidationClearedInvariantObligation,
         FinalProgressWitnessClosureInvariantObligation,
         AsyncSpecAlwaysRetainsExactDecisionFanout, PTL
    <2>2. \A node, qc, response:
             /\ AsyncStrongTypeInvariant
             /\ AsyncProgressOwnershipInvariant
             /\ DecisionFrontierUniquenessInvariant
             /\ DecisionTimeoutFrontierInvariant
             /\ ResponsiveRecoveryValidationClearedInvariant
             /\ FinalProgressWitnessClosureInvariant
             /\ ExactDecisionFanoutRetentionInvariant
             /\ [AsyncNext]_AsyncAllVars
             => [ExactDecisionResponseClaimResidualPersistsOrGoals(
                   node, qc, response)]_AsyncAllVars
      BY ExactDecisionResponseClaimResidualStepIsSafe
    <2>3. AsyncSpecAt(initialContext)
             => [][AsyncNext]_AsyncAllVars
      BY DEF AsyncSpecAt
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF ExactDecisionResponseClaimExitSafetyKernelProperty
  <1> QED BY <1>1

ExactDecisionResponseClaimIngressRunnerConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, response:
         ExactDecisionResponseClaimIngressResidual(node, qc, response)
           ~> ExactDecisionResponseAdmissionGoal(node, qc)

THEOREM ExactDecisionResponseClaimExitSafetyDischargesResidual ==
  \A initialContext:
    ExactDecisionResponseClaimExitSafetyKernelProperty(
      AsyncSpecAt(initialContext))
      => ExactDecisionResponseClaimIngressRunnerConvergenceProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ExactDecisionResponseClaimExitSafetyKernelProperty(
                  AsyncSpecAt(initialContext))
         PROVE ExactDecisionResponseClaimIngressRunnerConvergenceProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node, qc, response:
                    ExactDecisionResponseClaimIngressResidual(
                      node, qc, response)
                      ~> ExactDecisionResponseAdmissionGoal(node, qc)
      <3>1. AsyncSpecAt(initialContext)
               => []AsyncStrongTypeInvariant
        BY AsyncSpecAlwaysStrongTypeInvariant
      <3>2. ASSUME NEW node, NEW qc, NEW response
             PROVE ExactDecisionResponseClaimIngressResidual(
                     node, qc, response)
                     ~> ExactDecisionResponseAdmissionGoal(node, qc)
        <4>1. [](ExactDecisionResponseClaimIngressResidual(
                   node, qc, response)
                  => /\ node \in ValidatorIds
                     /\ gst
                     /\ CertifiedResponseClaimRunnerOwned(node))
          BY <2>1, <3>1,
             ExactDecisionResponseClaimResidualProjectsRunnerOwned, PTL
        <4>2. ExactDecisionResponseClaimIngressResidual(
                 node, qc, response)
                 ~> CertifiedResponseClaimRunnerGoal(node)
          BY <2>1, <4>1,
             GstCertifiedResponseClaimRunnerConvergence, PTL
        <4>3. [][ExactDecisionResponseClaimResidualPersistsOrGoals(
                    node, qc, response)]_AsyncAllVars
          BY <1>1, <2>1
             DEF ExactDecisionResponseClaimExitSafetyKernelProperty
        <4>4. /\ ExactDecisionResponseClaimIngressResidual(
                      node, qc, response)
                 /\ CertifiedResponseClaimRunnerGoal(node)
                => ExactDecisionResponseAdmissionGoal(node, qc)
          BY Isa
             DEF ExactDecisionResponseClaimIngressResidual,
                 ExactDecisionRouteNeutralClaimIngressOwned,
                 ExactDecisionResponseAdmissionGoal,
                 ExactDecisionExecutableFrontier,
                 CertifiedResponseClaimRunnerGoal,
                 CertifiedResponseClaimMatches,
                 CertifiedResponseClaimsAt,
                 DecisionCertifiedResponseLineageExact
        <4> QED BY <4>2, <4>3, <4>4, PTL
             DEF ExactDecisionResponseClaimResidualPersistsOrGoals
      <3> QED BY <3>2
    <2> QED BY <2>1
         DEF ExactDecisionResponseClaimIngressRunnerConvergenceProperty
  <1> QED BY <1>1

THEOREM ExactDecisionResponseClaimIngressRunnerConvergence ==
  \A initialContext:
    ExactDecisionResponseClaimIngressRunnerConvergenceProperty(
      AsyncSpecAt(initialContext))
BY ExactDecisionResponseClaimExitSafetyKernel,
   ExactDecisionResponseClaimExitSafetyDischargesResidual

ExactDecisionResponseClaimContentionConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, archive, request, response, packet:
         ExactDecisionResponseClaimContentionResidual(
           node, qc, archive, request, response, packet)
           ~> ExactDecisionResponseAdmissionGoal(node, qc)

THEOREM ExactDecisionResponseClaimContentionConvergence ==
  \A initialContext:
    ExactDecisionResponseClaimContentionConvergenceProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ExactDecisionResponseClaimContentionConvergenceProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node, qc, archive, request, response, packet:
                    ExactDecisionResponseClaimContentionResidual(
                      node, qc, archive, request, response, packet)
                      ~> ExactDecisionResponseAdmissionGoal(node, qc)
      <3>1. AsyncSpecAt(initialContext)
               => [](/\ AsyncStrongTypeInvariant
                      /\ ExactDecisionRequestAuthorityIsolationInvariant)
        BY AsyncSpecAlwaysStrongTypeInvariant,
           AsyncSpecAlwaysIsolatesExactDecisionRequestAuthority, PTL
      <3>2. ExactDecisionResponseClaimIngressRunnerConvergenceProperty(
               AsyncSpecAt(initialContext))
        BY ExactDecisionResponseClaimIngressRunnerConvergence
      <3>3. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                    NEW response, NEW packet
             PROVE ExactDecisionResponseClaimContentionResidual(
                     node, qc, archive, request, response, packet)
                     ~> ExactDecisionResponseAdmissionGoal(node, qc)
        <4>1. [](ExactDecisionResponseClaimContentionResidual(
                   node, qc, archive, request, response, packet)
                  => \E claimed \in AsyncCertifiedResponseItems:
                       ExactDecisionResponseClaimIngressResidual(
                         node, qc, claimed))
          BY <2>1, <3>1,
             ExactDecisionClaimContentionOwnsExactClaimResidual, PTL
        <4>2. \A claimed:
                 ExactDecisionResponseClaimIngressResidual(
                   node, qc, claimed)
                   ~> ExactDecisionResponseAdmissionGoal(node, qc)
          BY <2>1, <3>2
             DEF ExactDecisionResponseClaimIngressRunnerConvergenceProperty
        <4> QED BY <4>1, <4>2, PTL
      <3> QED BY <3>3
    <2> QED BY <2>1
         DEF ExactDecisionResponseClaimContentionConvergenceProperty
  <1> QED BY <1>1

(***************************************************************************
Exact response-admission temporal decomposition.

The action leaves above close every semantic handoff:

  * fresh admission acquires the recipient-local canonical claim and ingress
    owner;
  * a route-neutral duplicate coalesces onto that same physical owner;
  * exact authenticated packets are never policy-retired;
  * a different live claim at the same recipient retains the exact packet;
  * the shared completion owner has an exact finite natural debt; and
  * selecting the claimed occurrence in the normal runner creates the exact
    FetchCertifiedBody frontier (or observes terminal application).

The fair claim runner, exact claim-contention kernel, finite physical-owner
descent, and exact enabled-admission handoff are proved below without importing
aggregate stage service.  The remaining response debt is therefore the
non-physical, non-claim transport prefix: delivery deadline or older packets
in the same outer-source lane before the retained target packet becomes
continuously admission-ready.  The protected-slot invariant rules out a
separate aggregate-capacity owner for the fresh response.
***************************************************************************)

THEOREM FairExactDecisionResponseAdmissionHandoff ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => \A node, qc, archive, request, response, packet:
           ExactDecisionResponsePacketAdmissionReady(
             node, qc, archive, request, response, packet)
             ~> ExactDecisionResponseAdmissionOutcome(
                  node, qc, response)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => \A node, qc, archive, request, response, packet:
                      ExactDecisionResponsePacketAdmissionReady(
                        node, qc, archive, request, response, packet)
                        ~> ExactDecisionResponseAdmissionOutcome(
                             node, qc, response)
    <2>1. AsyncSpecAt(initialContext)
             => [](/\ AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ DecisionFrontierUniquenessInvariant
                    /\ DecisionTimeoutFrontierInvariant
                    /\ ResponsiveRecoveryValidationClearedInvariant
                    /\ FinalProgressWitnessClosureInvariant
                    /\ ExactDecisionFanoutRetentionInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         DecisionFrontierUniquenessInvariantFromAsyncSpec,
         DecisionTimeoutFrontierInvariantFromAsyncSpec,
         ResponsiveRecoveryValidationClearedInvariantObligation,
         FinalProgressWitnessClosureInvariantObligation,
         AsyncSpecAlwaysRetainsExactDecisionFanout, PTL
    <2>2. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node, qc, archive, request, response, packet:
                    ExactDecisionResponsePacketAdmissionReady(
                      node, qc, archive, request, response, packet)
                      ~> ExactDecisionResponseAdmissionOutcome(
                           node, qc, response)
      <3>1. ASSUME NEW node, NEW qc, NEW archive,
                    NEW request, NEW response, NEW packet
             PROVE ExactDecisionResponsePacketAdmissionReady(
                     node, qc, archive, request, response, packet)
                     ~> ExactDecisionResponseAdmissionOutcome(
                          node, qc, response)
        <4>1. [](ExactDecisionResponsePacketAdmissionReady(
                   node, qc, archive, request, response, packet)
                  => /\ node \in Responsive
                     /\ response.source \in AsyncIngressSources)
          BY <2>1, <2>2, Isa, PTL
             DEF ExactDecisionResponsePacketAdmissionReady,
                 ExactDecisionResponseAdmissionResidual,
                 ExactDecisionResponsePacketOwned,
                 ExactDecisionAuthenticatedResponse,
                 ExactDecisionBodyHoldingAlias,
                 ExactDecisionActiveRequestOwner,
                 ExactDecisionServiceSource,
                 AsyncCurrentResponsiveVoters,
                 AsyncStrongTypeInvariant,
                 AsyncSchedulerTypeInvariant,
                 AsyncTransportTypeInvariant,
                 AsyncTransportContentTypeInvariant,
                 AsyncPacketContentTypeInvariant,
                 AsyncPacketTyped, AsyncItemTyped
        <4>2. ExactDecisionResponsePacketAdmissionReady(
                 node, qc, archive, request, response, packet)
                   /\ [AsyncNext]_AsyncAllVars
                => \/ ExactDecisionResponsePacketAdmissionReady(
                        node, qc, archive, request, response, packet)'
                   \/ ExactDecisionResponseAdmissionOutcome(
                        node, qc, response)'
          BY <2>1, ExactDecisionResponseAdmissionReadyStepIsSafe
        <4>3. CASE /\ node \in Responsive
                     /\ response.source \in AsyncIngressSources
          <5>1. AsyncSpecAt(initialContext)
                   => WF_AsyncAllVars(
                        PostGstAdmitHiddenPacket(
                          node, response.source))
            BY <4>3 DEF AsyncSpecAt, AsyncFairnessAt
          <5>2. /\ ExactDecisionResponsePacketAdmissionReady(
                       node, qc, archive, request, response, packet)
                   /\ ~ExactDecisionResponseAdmissionOutcome(
                        node, qc, response)
                  => ENABLED
                       <<PostGstAdmitHiddenPacket(
                           node, response.source)>>_AsyncAllVars
            BY ExactDecisionResponseReadyEnablesFairAdmission
          <5>3. /\ ExactDecisionResponsePacketAdmissionReady(
                       node, qc, archive, request, response, packet)
                   /\ ~ExactDecisionResponseAdmissionOutcome(
                        node, qc, response)
                   /\ <<PostGstAdmitHiddenPacket(
                         node, response.source)>>_AsyncAllVars
                  => ExactDecisionResponseAdmissionOutcome(
                       node, qc, response)'
            BY ExactDecisionResponseFairAdmissionCreatesOutcome
          <5> QED BY <4>2, <5>1, <5>2, <5>3, PTL
               DEF AsyncSpecAt
        <4>4. CASE \/ node \notin Responsive
                     \/ response.source \notin AsyncIngressSources
          <5>1. AsyncSpecAt(initialContext)
                   => []~ExactDecisionResponsePacketAdmissionReady(
                         node, qc, archive, request, response, packet)
            BY <4>1, <4>4, PTL
          <5> QED BY <5>1, PTL
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>1
    <2> QED BY <2>2
  <1> QED BY <1>1

ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, archive, request, response, packet:
         ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual(
           node, qc, archive, request, response, packet)
           ~> (ExactDecisionResponseAdmissionGoal(node, qc)
                \/ ExactDecisionResponsePacketAdmissionReady(
                     node, qc, archive, request, response, packet))

ExactDecisionResponseNonPhysicalHeadGateOwnerConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, archive, request, response, packet:
         ExactDecisionResponseNonPhysicalHeadGateOwnerResidual(
           node, qc, archive, request, response, packet)
           ~> (ExactDecisionResponseAdmissionGoal(node, qc)
                \/ ExactDecisionResponsePacketAdmissionReady(
                     node, qc, archive, request, response, packet))

THEOREM ExactDecisionResponseClaimKernelNarrowsNonPhysicalResidual ==
  \A initialContext:
    ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty(
      AsyncSpecAt(initialContext))
      => ExactDecisionResponseNonPhysicalHeadGateOwnerConvergenceProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty(
                  AsyncSpecAt(initialContext))
         PROVE ExactDecisionResponseNonPhysicalHeadGateOwnerConvergenceProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node, qc, archive, request, response, packet:
                    ExactDecisionResponseNonPhysicalHeadGateOwnerResidual(
                      node, qc, archive, request, response, packet)
                      ~> (ExactDecisionResponseAdmissionGoal(node, qc)
                           \/ ExactDecisionResponsePacketAdmissionReady(
                                node, qc, archive, request,
                                response, packet))
      <3>1. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                    NEW response, NEW packet
             PROVE ExactDecisionResponseNonPhysicalHeadGateOwnerResidual(
                     node, qc, archive, request, response, packet)
                     ~> (ExactDecisionResponseAdmissionGoal(node, qc)
                          \/ ExactDecisionResponsePacketAdmissionReady(
                               node, qc, archive, request,
                               response, packet))
        <4>1. ExactDecisionResponseClaimContentionResidual(
                 node, qc, archive, request, response, packet)
                 ~> ExactDecisionResponseAdmissionGoal(node, qc)
          BY <2>1,
             ExactDecisionResponseClaimContentionConvergence
             DEF ExactDecisionResponseClaimContentionConvergenceProperty
        <4>2. ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual(
                 node, qc, archive, request, response, packet)
                 ~> (ExactDecisionResponseAdmissionGoal(node, qc)
                      \/ ExactDecisionResponsePacketAdmissionReady(
                           node, qc, archive, request, response, packet))
          BY <1>1, <2>1
             DEF ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty
        <4>3. ExactDecisionResponseNonPhysicalHeadGateOwnerResidual(
                 node, qc, archive, request, response, packet)
                => \/ ExactDecisionResponseClaimContentionResidual(
                        node, qc, archive, request, response, packet)
                   \/ ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual(
                        node, qc, archive, request, response, packet)
          BY Isa
             DEF ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual
        <4> QED BY <4>1, <4>2, <4>3, PTL
      <3> QED BY <3>1
    <2> QED BY <2>1
  <1> QED BY <1>1
       DEF ExactDecisionResponseNonPhysicalHeadGateOwnerConvergenceProperty

ExactDecisionResponseHeadGateOwnerConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, archive, request, response, packet:
         ExactDecisionResponseHeadGateOwnerResidual(
           node, qc, archive, request, response, packet)
           ~> (ExactDecisionResponseAdmissionGoal(node, qc)
                \/ ExactDecisionResponsePacketAdmissionReady(
                     node, qc, archive, request, response, packet))

THEOREM ExactDecisionResponsePhysicalKernelNarrowsHeadGateResidual ==
  \A initialContext:
    ExactDecisionResponseNonPhysicalHeadGateOwnerConvergenceProperty(
      AsyncSpecAt(initialContext))
      => ExactDecisionResponseHeadGateOwnerConvergenceProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ExactDecisionResponseNonPhysicalHeadGateOwnerConvergenceProperty(
                  AsyncSpecAt(initialContext))
         PROVE ExactDecisionResponseHeadGateOwnerConvergenceProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node, qc, archive, request, response, packet:
                    ExactDecisionResponseHeadGateOwnerResidual(
                      node, qc, archive, request, response, packet)
                      ~> (ExactDecisionResponseAdmissionGoal(node, qc)
                           \/ ExactDecisionResponsePacketAdmissionReady(
                                node, qc, archive, request,
                                response, packet))
      <3>1. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                    NEW response, NEW packet
             PROVE ExactDecisionResponseHeadGateOwnerResidual(
                     node, qc, archive, request, response, packet)
                     ~> (ExactDecisionResponseAdmissionGoal(node, qc)
                          \/ ExactDecisionResponsePacketAdmissionReady(
                               node, qc, archive, request,
                               response, packet))
        <4>1. ExactDecisionResponsePhysicalCompletionResidual(
                 node, qc, archive, request, response, packet)
                 ~> ExactDecisionPhysicalCompletionRunnerGoal(
                      node, qc, archive, request, response, packet)
          BY <2>1,
             ExactDecisionResponsePhysicalCompletionConvergence
             DEF ExactDecisionResponsePhysicalCompletionConvergenceProperty
        <4>2. ExactDecisionResponseNonPhysicalHeadGateOwnerResidual(
                 node, qc, archive, request, response, packet)
                 ~> (ExactDecisionResponseAdmissionGoal(node, qc)
                      \/ ExactDecisionResponsePacketAdmissionReady(
                           node, qc, archive, request, response, packet))
          BY <1>1, <2>1
             DEF ExactDecisionResponseNonPhysicalHeadGateOwnerConvergenceProperty
        <4>3. ExactDecisionPhysicalCompletionRunnerGoal(
                 node, qc, archive, request, response, packet)
                => \/ ExactDecisionResponseAdmissionGoal(node, qc)
                   \/ ExactDecisionResponsePacketAdmissionReady(
                        node, qc, archive, request, response, packet)
                   \/ ExactDecisionResponseNonPhysicalHeadGateOwnerResidual(
                        node, qc, archive, request, response, packet)
          BY ExactDecisionResponseAdmissionResidualSplitsAtReady, Isa
             DEF ExactDecisionPhysicalCompletionRunnerGoal,
                 ExactDecisionResponseNonPhysicalHeadGateOwnerResidual,
                 ExactDecisionResponseHeadGateOwnerResidual
        <4>4. ExactDecisionResponsePhysicalCompletionResidual(
                 node, qc, archive, request, response, packet)
                 ~> (ExactDecisionResponseAdmissionGoal(node, qc)
                      \/ ExactDecisionResponsePacketAdmissionReady(
                           node, qc, archive, request, response, packet))
          BY <4>1, <4>2, <4>3, PTL
        <4>5. ExactDecisionResponseHeadGateOwnerResidual(
                 node, qc, archive, request, response, packet)
                => \/ ExactDecisionResponsePhysicalCompletionResidual(
                        node, qc, archive, request, response, packet)
                   \/ ExactDecisionResponseNonPhysicalHeadGateOwnerResidual(
                        node, qc, archive, request, response, packet)
          BY Isa
             DEF ExactDecisionResponseNonPhysicalHeadGateOwnerResidual
        <4> QED BY <4>2, <4>4, <4>5, PTL
      <3> QED BY <3>1
    <2> QED BY <2>1
  <1> QED BY <1>1
       DEF ExactDecisionResponseHeadGateOwnerConvergenceProperty

ExactDecisionResponseAdmissionHandoffConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, archive, request, response, packet:
         ExactDecisionResponsePacketAdmissionReady(
           node, qc, archive, request, response, packet)
           ~> ExactDecisionResponseAdmissionOutcome(node, qc, response)

THEOREM ExactDecisionResponseAdmissionHandoffConvergence ==
  \A initialContext:
    ExactDecisionResponseAdmissionHandoffConvergenceProperty(
      AsyncSpecAt(initialContext))
BY FairExactDecisionResponseAdmissionHandoff
   DEF ExactDecisionResponseAdmissionHandoffConvergenceProperty

ExactDecisionResponseAdmissionCorridorConvergenceProperty(
    specification) ==
  /\ ExactDecisionResponseAdmissionHandoffConvergenceProperty(
       specification)
  /\ ExactDecisionResponseClaimIngressRunnerConvergenceProperty(
       specification)

ExactDecisionResponseAdmissionResidualConvergenceProperty(
    specification) ==
  specification
    => \A node, qc, archive, request, response, packet:
         ExactDecisionResponseAdmissionResidual(
           node, qc, archive, request, response, packet)
           ~> ExactDecisionResponseAdmissionGoal(node, qc)

THEOREM ExactDecisionResponseAdmissionKernelsDischargeResidual ==
  \A initialContext:
    ExactDecisionResponseHeadGateOwnerConvergenceProperty(
      AsyncSpecAt(initialContext))
    => ExactDecisionResponseAdmissionResidualConvergenceProperty(
         AsyncSpecAt(initialContext))
BY ExactDecisionResponseAdmissionHandoffConvergence,
   ExactDecisionResponseClaimIngressRunnerConvergence,
   ExactDecisionResponseAdmissionResidualSplitsAtReady, PTL
   DEF ExactDecisionResponseHeadGateOwnerConvergenceProperty,
       ExactDecisionResponseAdmissionCorridorConvergenceProperty,
       ExactDecisionResponseAdmissionHandoffConvergenceProperty,
       ExactDecisionResponseClaimIngressRunnerConvergenceProperty,
       ExactDecisionResponseAdmissionResidualConvergenceProperty,
       ExactDecisionResponseHeadGateOwnerResidual,
       ExactDecisionResponseAdmissionOutcome

(***************************************************************************
Target-neutral fixed-clock and transport-head closure.

The three remaining exact Decision leaves share one scheduler obstruction:
an immutable target deadline or packet cannot advance while an arbitrary
overdue packet, due runner, or due I/O worker disables `AsyncTick`.  The rank
below is deliberately independent of the target's historical/current-voter
role.  It ranges over every responsive timed owner and selects only actions
which are individually quantified by `AsyncFairnessAt`.

At a frozen clock the outer prefix charges every already-due packet and every
responsive validator which may still become a timed owner.  The selected
packet dependency retains the existing lane-shadow, capacity, selector,
runner, candidate, and Serve components.  Candidate and Serve tails count
logical occurrences before their minimum service rank, so removing one of
several equal owners is visible.

Serving an owner may replace it with deterministic causal children or admit a
Serve lifecycle.  That replacement is not called progress.  A source-sealed
episode freezes the immutable packet, candidate, and Serve predecessor
identities and precharges the candidate and Serve admission ordinals.  Runner
and I/O handoffs remain explicit coordinates of the well-founded lexicographic
rank; they are not conflated with immutable producer identities.  Exact retry
coalescing, the same-generation candidate marker, the durable terminal
tombstone, and the Serve tombstone make that episode finite: a non-descent
step consumes an ordinal token, and a serviced logical identity cannot return
at its old stage while GST fixes the generation.  Pre-GST responsive restart
clears transient markers and may reconstruct nonterminal work; it is outside
this fixed-GST episode and is never counted as progress.  Later causal,
Control, Completion, or priority work is outside the frozen identity cohort
and must consume that finite ordinal budget before it can affect the producer
tail.

The clock leaf adds a natural distance to the retained retransmit deadline.
The two head-gate leaves add the same distance to the packet's immutable
delivery deadline.  Once either target packet is due it is itself an overdue
responsive occurrence, so a retained non-ready packet disables Tick.  The
fixed-clock closure must therefore expose the exact admission-ready goal; it
cannot escape by advancing the clock.
***************************************************************************)

ExactDecisionRequestHeadGateResidualPersistsOrGoals(
    node, qc, archive, request, packet) ==
  ExactDecisionRequestHeadGateOwnerResidual(
    node, qc, archive, request, packet)
    => \/ ExactDecisionRequestHeadGateOwnerResidual(
            node, qc, archive, request, packet)'
       \/ ExactDecisionRequestIngressGoal(
            node, qc, archive, request)'
       \/ ExactDecisionRequestPacketAdmissionReady(
            node, qc, archive, request, packet)'

THEOREM ExactDecisionRequestHeadGateResidualStepIsSafe ==
  \A node, qc, archive, request, packet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ ResponsiveRecoveryValidationClearedInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ ExactDecisionFanoutRetentionInvariant
    /\ [AsyncNext]_AsyncAllVars
    => ExactDecisionRequestHeadGateResidualPersistsOrGoals(
         node, qc, archive, request, packet)
BY AsyncBracketStepRetainsExactDecisionRecord,
   AsyncBracketStepLeavesContext,
   AsyncNextPreservesExactDecisionFanoutRetention,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesFinalProgressWitnessClosure,
   GstAsyncStepIsMonotone, ExpandENABLED, IsaT(300)
   DEF ExactDecisionRequestHeadGateResidualPersistsOrGoals,
       ExactDecisionRequestHeadGateOwnerResidual,
       ExactDecisionRequestPacketAdmissionReady,
       ExactDecisionRequestIngressResidual,
       ExactDecisionRequestIngressGoal,
       ExactDecisionRequestPacketOwned,
       ExactDecisionRequestIngressOwned,
       ExactDecisionBodyHoldingAlias,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionRecord,
       ExactDecisionExecutableFrontier,
       ExactDecisionExecutableOwner,
       CanAdmitIngressItem,
       PostGstAdmitHiddenPacket,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DueSourcePackets, OldestDueSourcePacket,
       IngressHasCoalescingOwner,
       IngressPacketPolicyRejected,
       IngressResourceSource,
       IngressLane, IngressLaneDepth,
       SequenceSet

ExactDecisionTargetNeutralCandidateOwnerIdentity(candidate) ==
  [ownerKind |-> "Candidate",
   identity |-> AsyncCandidateAdmissionIdentity(candidate)]

ExactDecisionTargetNeutralServeOwnerIdentity(node, job) ==
  [ownerKind |-> "Serve",
   identity |-> AsyncIoServeJobIdentity(node, job)]

ExactDecisionTargetNeutralServeRequestOwnerIdentity(node, request) ==
  [ownerKind |-> "Serve",
   identity |-> AsyncServeLogicalRequestIdentity(node, request)]

ExactDecisionTargetNeutralCandidateOwnerIdentitySet ==
  {[ownerKind |-> "Candidate", identity |-> identity]:
     identity \in AsyncCandidateAdmissionIdentitySet}

ExactDecisionTargetNeutralServeOwnerIdentitySet ==
  {[ownerKind |-> "Serve", identity |-> identity]:
     identity \in AsyncServeLogicalRequestIdentities}

ExactDecisionTargetNeutralCausalRoot(node, origin) ==
  [node |-> node, origin |-> origin]

ExactDecisionTargetNeutralCausalRootSet ==
  [node: ValidatorIds, origin: AsyncCandidateCausalOriginSet]

ExactDecisionTargetNeutralLiveCandidateCausalRoots ==
  {ExactDecisionTargetNeutralCausalRoot(
     candidate.node, candidate.causalOrigin):
     candidate \in
       {scheduled \in ActiveScheduledCandidates:
          scheduled.node \in Responsive}}

ExactDecisionTargetNeutralLifecycleRecordCausalRoots ==
  {ExactDecisionTargetNeutralCausalRoot(record.node, record.origin):
     record \in
       {owned \in AsyncCandidateLifecycleAdmissions:
          /\ owned.node \in Responsive
          /\ ~owned.retired}}

ExactDecisionTargetNeutralProducerContinuationCausalRoots ==
  {ExactDecisionTargetNeutralCausalRoot(
     continuation.node, continuation.causalOrigin):
     continuation \in
       {owned \in AsyncCandidateProducerContinuations:
          /\ owned.node \in Responsive
          /\ owned.status \in {"Reserved", "Materialized"}}}

ExactDecisionTargetNeutralOrdinaryIngressCausalRoots ==
  {ExactDecisionTargetNeutralCausalRoot(
     carrier.node, carrier.origin):
     carrier
       \in {owned \in
             asyncControlServiceState.ordinaryIngressCarrierEvidence:
             owned.node \in Responsive}}

ExactDecisionTargetNeutralLeaderWireCausalRoots ==
  {ExactDecisionTargetNeutralCausalRoot(
     owner.recipient, owner.causalOrigin):
     owner \in
       {owned \in asyncLeaderWireLifecycles:
          /\ owned.recipient \in Responsive
          /\ AsyncLeaderWireLifecycleActive(owned)}}

ExactDecisionTargetNeutralDurableReplayCausalRoots ==
  {ExactDecisionTargetNeutralCausalRoot(node, origin):
     node \in Responsive,
     origin \in AsyncCandidateLifecycleDurableReplayOriginsForNode(node)}

ExactDecisionTargetNeutralTimeoutReservationCausalRoots ==
  {ExactDecisionTargetNeutralCausalRoot(
     node, asyncControlServiceState.timeoutLifecycleOrigin[node]):
     node \in
       {owner \in Responsive:
          AsyncUnmaterializedTimeoutLifecycleReservationIn(
            asyncControlServiceState, owner)}}

ExactDecisionTargetNeutralRetainedCandidateCausalRoots ==
  ExactDecisionTargetNeutralLiveCandidateCausalRoots
    \cup ExactDecisionTargetNeutralLifecycleRecordCausalRoots
    \cup ExactDecisionTargetNeutralProducerContinuationCausalRoots
    \cup ExactDecisionTargetNeutralOrdinaryIngressCausalRoots
    \cup ExactDecisionTargetNeutralLeaderWireCausalRoots
    \cup ExactDecisionTargetNeutralDurableReplayCausalRoots
    \cup ExactDecisionTargetNeutralTimeoutReservationCausalRoots

ExactDecisionTargetNeutralDuePacketCausalRoots(clockValue) ==
  {ExactDecisionTargetNeutralCausalRoot(
     packet.item.envelope.recipient,
     (DeliveryCandidate(packet.item)).causalOrigin):
     packet \in
       {due \in HistoricalDiscoveryDuePacketsAt(clockValue):
          due.item.envelope.recipient \in Responsive}}

ExactDecisionTargetNeutralFrozenCausalRoots(clockValue) ==
  ExactDecisionTargetNeutralRetainedCandidateCausalRoots
    \cup ExactDecisionTargetNeutralDuePacketCausalRoots(clockValue)

ExactDecisionTargetNeutralCandidateEpisodeUniverse(clockValue) ==
  {ExactDecisionTargetNeutralCandidateOwnerIdentity(candidate):
     candidate \in
       {scheduled \in ActiveScheduledCandidates:
          ExactDecisionTargetNeutralCausalRoot(
            scheduled.node, scheduled.causalOrigin)
            \in ExactDecisionTargetNeutralFrozenCausalRoots(clockValue)}}

\* A fixed-clock episode freezes the predecessor of the shared scheduler
\* high-watermark at every responsive process.  This is a past cut, not a
\* future allowance: every fresh Candidate root and every fresh Serve ticket
\* receives the current high-watermark and is therefore strictly above it.
\* A child of an already admitted Candidate retains its parent's immutable
\* lifecycle ordinal and remains below the cut, so its complete successor
\* subtree must be prepaid by the causal occurrence budget rather than by
\* enumerating the child's not-yet-materialized identity.
ExactDecisionTargetNeutralCurrentPastSchedulerCut(node) ==
  AsyncNextCandidateLifecycleOrdinal(node) - 1

ExactDecisionTargetNeutralCurrentPastSchedulerCuts ==
  [node \in Responsive |->
     ExactDecisionTargetNeutralCurrentPastSchedulerCut(node)]

\* The physical cut is the current receiver-local admission high-watermark.
\* Every carrier already active at construction has a strictly smaller
\* ordinal.  A Dormant replay admitted afterwards receives the cut itself,
\* so logical age cannot place it back into this frozen physical prefix.
ExactDecisionTargetNeutralCurrentPhysicalCuts ==
  [node \in Responsive |-> AsyncNextIngressPhysicalOrdinal(node)]

ExactDecisionTargetNeutralSchedulerCutToken(node, cutoffOrdinal) ==
  [node |-> node, cutoffOrdinal |-> cutoffOrdinal]

ExactDecisionTargetNeutralSchedulerCutTokens(snapshot) ==
  {ExactDecisionTargetNeutralSchedulerCutToken(
     node, snapshot.schedulerCuts[node]):
     node \in Responsive}

ExactDecisionTargetNeutralCandidateRootPrecedesPhysicalCut(
    snapshot, candidate) ==
  /\ candidate.node \in Responsive
  /\ AsyncCandidateLifecycleRecorded(
       candidate.node, candidate.causalOrigin)
  /\ (AsyncCandidateLifecycleRecordFor(
        candidate.node, candidate.causalOrigin)).sourcePhysicalOrdinal
       < snapshot.physicalCuts[candidate.node]

ExactDecisionTargetNeutralCandidateOwnersForSnapshot(snapshot, packet) ==
  LET recipient == packet.item.envelope.recipient
  IN {candidate \in ActiveScheduledCandidates:
        /\ candidate.node = recipient
        /\ candidate.node \in AsyncTimedServiceNodes
        /\ AsyncCandidateLifecycleOrdinal(candidate)
             <= snapshot.schedulerCuts[recipient]
        /\ ExactDecisionTargetNeutralCandidateRootPrecedesPhysicalCut(
             snapshot, candidate)
        /\ ExactDecisionTargetNeutralCausalRoot(
             candidate.node, candidate.causalOrigin)
             \in snapshot.candidateRoots
        /\ ProtectedCandidateOwned(candidate)}

ExactDecisionTargetNeutralServeOwnersForSnapshot(snapshot, packet) ==
  LET recipient == packet.item.envelope.recipient
  IN {job \in ActiveIoJobs:
        /\ job \in SequenceSet(asyncIoQueues[recipient])
        /\ job.class = "Serve"
        /\ ExactDecisionTargetNeutralServeOwnerIdentity(recipient, job)
             \in snapshot.serveIdentities}

ExactDecisionTargetNeutralCandidateRanksForSnapshot(snapshot, packet) ==
  {CandidateServiceRank(candidate):
     candidate
       \in ExactDecisionTargetNeutralCandidateOwnersForSnapshot(
             snapshot, packet)}

ExactDecisionTargetNeutralServeRanksForSnapshot(snapshot, packet) ==
  LET recipient == packet.item.envelope.recipient
  IN {ServeJobRank(recipient, job):
        job \in ExactDecisionTargetNeutralServeOwnersForSnapshot(
                   snapshot, packet)}

ExactDecisionTargetNeutralCandidateDebtRankForSnapshot(snapshot, packet) ==
  LET ranks ==
        ExactDecisionTargetNeutralCandidateRanksForSnapshot(
          snapshot, packet)
  IN IF ranks = {}
     THEN HistoricalDiscoveryCandidateDebtBottom
     ELSE HistoricalDiscoveryOwnedRankMinimum(ranks)

ExactDecisionTargetNeutralServeDebtRankForSnapshot(snapshot, packet) ==
  LET ranks ==
        ExactDecisionTargetNeutralServeRanksForSnapshot(snapshot, packet)
  IN IF ranks = {}
     THEN HistoricalDiscoveryServeDebtBottom
     ELSE HistoricalDiscoveryOwnedRankMinimum(ranks)

ExactDecisionTargetNeutralCandidateOccurrenceRankForSnapshot(
    snapshot, packet) ==
  <<Cardinality(
       ExactDecisionTargetNeutralCandidateOwnersForSnapshot(
         snapshot, packet)),
    ExactDecisionTargetNeutralCandidateDebtRankForSnapshot(
      snapshot, packet)>>

ExactDecisionTargetNeutralServeOccurrenceRankForSnapshot(
    snapshot, packet) ==
  <<Cardinality(
       ExactDecisionTargetNeutralServeOwnersForSnapshot(snapshot, packet)),
    ExactDecisionTargetNeutralServeDebtRankForSnapshot(snapshot, packet)>>

ExactDecisionTargetNeutralPacketDependencyRankForSnapshot(
    snapshot, packet) ==
  LET recipient == packet.item.envelope.recipient
  IN <<OlderDueNonOverdueShadowDebt(packet),
       <<FreshIngressCapacityOwnerDebt(packet.item),
         <<TimeoutVoteByteOwnerDebt(packet.item),
           <<TransportCompletionOwnerDebt(packet.item),
             <<BoundedTransportServiceRank(
                  packet.item.envelope.recipient, packet.item.source),
               <<ResetAwareIngressReachRank(recipient),
                 <<ReadyRunAuxRank(recipient),
                   <<Stage4CapacityRank(recipient),
                     <<ExactDecisionTargetNeutralCandidateOccurrenceRankForSnapshot(
                          snapshot, packet),
                       ExactDecisionTargetNeutralServeOccurrenceRankForSnapshot(
                         snapshot, packet)>>>>>>>>>>>>>>>>>>

ExactDecisionTargetNeutralSelectedOverduePacket ==
  CHOOSE packet \in OverdueResponsivePackets: TRUE

ExactDecisionTargetNeutralSelectedPacketDependencyRankForSnapshot(snapshot) ==
  ExactDecisionTargetNeutralPacketDependencyRankForSnapshot(
    snapshot, ExactDecisionTargetNeutralSelectedOverduePacket)

ExactDecisionTargetNeutralConcreteBlockerStage(clockValue) ==
  IF OverdueResponsivePackets # {}
  THEN 1
  ELSE IF HistoricalDiscoveryNodeBlockersAt(clockValue) # {}
       THEN 3
       ELSE IF HistoricalDiscoveryActiveIoBlockersAt(clockValue) # {}
            THEN 2
            ELSE 0

ExactDecisionTargetNeutralConcreteDependencyRankForSnapshot(
    snapshot, clockValue) ==
  IF OverdueResponsivePackets # {}
  THEN ExactDecisionTargetNeutralSelectedPacketDependencyRankForSnapshot(
         snapshot)
  ELSE IF HistoricalDiscoveryNodeBlockersAt(clockValue) # {}
       THEN HistoricalDiscoveryIngressCounterRank(
              HistoricalDiscoveryNodeBlockerDebt(clockValue))
       ELSE IF HistoricalDiscoveryActiveIoBlockersAt(clockValue) # {}
            THEN HistoricalDiscoveryIngressCounterRank(
                   HistoricalDiscoveryActiveIoBlockerDebt(clockValue))
            ELSE HistoricalDiscoveryIngressCounterRank(0)

ExactDecisionTargetNeutralLiveCandidateIdentitySet ==
  {ExactDecisionTargetNeutralCandidateOwnerIdentity(candidate):
     candidate \in
       {scheduledCandidate \in ActiveScheduledCandidates:
          scheduledCandidate.node \in Responsive}}

ExactDecisionTargetNeutralLiveServeIdentitySet ==
  {ExactDecisionTargetNeutralServeOwnerIdentity(node, job):
     node \in Responsive,
     job \in
       {serveJob \in SequenceSet(asyncIoQueues[node]):
          serveJob.class = "Serve"}}

ExactDecisionTargetNeutralRetainedServeLifecycleRecords ==
  {record \in asyncServeAdmissions \cup asyncServeTombstones:
     record.node \in Responsive}

ExactDecisionTargetNeutralRetainedServeIdentitySet ==
  {[ownerKind |-> "Serve", identity |-> record.identity]:
     record \in ExactDecisionTargetNeutralRetainedServeLifecycleRecords}

ExactDecisionTargetNeutralLiveProducerIdentitySet ==
  ExactDecisionTargetNeutralLiveCandidateIdentitySet
    \cup ExactDecisionTargetNeutralLiveServeIdentitySet

ExactDecisionTargetNeutralFrozenProducerRequests(clockValue) ==
  {AsyncProducerIngressRequest(packet.item):
     packet \in HistoricalDiscoveryDuePacketsAt(clockValue)}

ExactDecisionTargetNeutralIngressEpisodeUniverse(clockValue) ==
  {AsyncProducerEpisode(
     request, authenticatedSource, AsyncProducerIngressStage):
     request
       \in ExactDecisionTargetNeutralFrozenProducerRequests(clockValue),
     authenticatedSource \in AsyncIngressSources}

ExactDecisionTargetNeutralServeEpisodeUniverse(clockValue) ==
  ExactDecisionTargetNeutralRetainedServeIdentitySet
    \cup
  {ExactDecisionTargetNeutralServeRequestOwnerIdentity(
     packet.item.envelope.recipient, packet.item):
     packet \in
       {requestPacket \in HistoricalDiscoveryDuePacketsAt(clockValue):
          requestPacket.item.kind \in AsyncReplyRequestKinds}}

ExactDecisionTargetNeutralCandidateIdentityCoalesced(owner) ==
  /\ owner.ownerKind = "Candidate"
  /\ \/ AsyncCandidateTransientServiceIdentityMarked(
          owner.identity.service)
     \/ AsyncCandidateTerminalIdentityTombstoned(
          owner.identity.service)

ExactDecisionTargetNeutralCandidateIdentityObsolete(owner) ==
  /\ owner.ownerKind = "Candidate"
  /\ AsyncCandidateAdmissionIdentityObsolete(owner.identity)

ExactDecisionTargetNeutralServeIdentityRetired(owner) ==
  /\ owner.ownerKind = "Serve"
  /\ AsyncServeLogicalIdentityRetiredOrSuperseded(
       owner.identity.owner, owner.identity)

\* Compatibility names now range over the immutable semantic owner universe,
\* rather than over only the DeliverChunk owners live at the snapshot edge.
ExactDecisionTargetNeutralFrozenCandidateOwnerIdentitySet ==
  ExactDecisionTargetNeutralCandidateOwnerIdentitySet

ExactDecisionTargetNeutralFrozenLiveCandidateIdentitySet ==
  ExactDecisionTargetNeutralLiveCandidateIdentitySet

ExactDecisionTargetNeutralCandidateEpisodeRemaining(snapshot) ==
  {owner \in snapshot.candidateIdentities:
     /\ ~ExactDecisionTargetNeutralCandidateIdentityCoalesced(owner)
     /\ ~ExactDecisionTargetNeutralCandidateIdentityObsolete(owner)}

ExactDecisionTargetNeutralServeEpisodeRemaining(snapshot) ==
  {owner \in snapshot.serveIdentities:
     ~ExactDecisionTargetNeutralServeIdentityRetired(owner)}

ExactDecisionTargetNeutralFrozenCandidateLifecycleCovered(snapshot) ==
  \A owner \in snapshot.candidateIdentities:
    \/ owner \in ExactDecisionTargetNeutralLiveCandidateIdentitySet
    \/ owner \in ExactDecisionTargetNeutralCandidateEpisodeRemaining(
                  snapshot)
    \/ ExactDecisionTargetNeutralCandidateIdentityCoalesced(owner)
    \/ ExactDecisionTargetNeutralCandidateIdentityObsolete(owner)

ExactDecisionTargetNeutralFrozenServeLifecycleCovered(snapshot) ==
  \A owner \in snapshot.serveIdentities:
    \/ owner \in ExactDecisionTargetNeutralLiveServeIdentitySet
    \/ owner \in ExactDecisionTargetNeutralServeEpisodeRemaining(snapshot)
    \/ ExactDecisionTargetNeutralServeIdentityRetired(owner)

ExactDecisionTargetNeutralIngressEpisodeRemaining(snapshot) ==
  snapshot.ingressEpisodes \ asyncProducerConsumedEpisodes

\* These identity sets are retained only for carrier-coverage diagnostics.
\* They are not the producer rank: a not-yet-materialized same-origin child
\* has no identity in either set.  The causal occurrence rank below instead
\* prepays the complete closed successor subtree below the frozen scheduler
\* cut.
ExactDecisionTargetNeutralProducerEpisodeSet(snapshot) ==
  ({"Ingress"} \X
     ExactDecisionTargetNeutralIngressEpisodeRemaining(snapshot))
    \cup
  ({"Candidate"} \X
     ExactDecisionTargetNeutralCandidateEpisodeRemaining(snapshot))
    \cup
  ({"Serve"} \X
     ExactDecisionTargetNeutralServeEpisodeRemaining(snapshot))

\* Materialized-identity diagnostics.  In particular, these are not ordinal
\* intervals and are never used to justify finite episode closure.
ExactDecisionTargetNeutralMaterializedCandidateIdentityTokens(snapshot) ==
  {"Candidate"} \X
    ExactDecisionTargetNeutralCandidateEpisodeRemaining(snapshot)

ExactDecisionTargetNeutralMaterializedServeIdentityTokens(snapshot) ==
  {"Serve"} \X
    ExactDecisionTargetNeutralServeEpisodeRemaining(snapshot)

ExactDecisionTargetNeutralIngressEpisodeTokens(snapshot) ==
  {"Ingress"} \X
    ExactDecisionTargetNeutralIngressEpisodeRemaining(snapshot)

ExactDecisionTargetNeutralProducerEpisodeTokens(snapshot) ==
  ExactDecisionTargetNeutralIngressEpisodeTokens(snapshot)
    \cup
      ExactDecisionTargetNeutralMaterializedCandidateIdentityTokens(snapshot)
    \cup
      ExactDecisionTargetNeutralMaterializedServeIdentityTokens(snapshot)

\* The producer episode is an ingress-leading lexicographic rank.  Its causal
\* tail is selected only while a concrete overdue packet owns the fixed-clock
\* dependency rank.  Replacing that packet is already strict dependency-rank
\* progress, so a non-descent cell retains the same recipient and frozen cut.
ExactDecisionTargetNeutralIngressEpisodeRank(snapshot) ==
  Cardinality(
    ExactDecisionTargetNeutralIngressEpisodeRemaining(snapshot))

ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot(
    snapshot, node) ==
  {record.origin:
     record \in
       {owned \in AsyncCandidateLifecycleAdmissions:
          /\ owned.node = node
          /\ owned.ordinal <= snapshot.schedulerCuts[node]
          /\ owned.sourcePhysicalOrdinal < snapshot.physicalCuts[node]}}

ExactDecisionTargetNeutralCausalCandidatesForSnapshot(snapshot, node) ==
  {candidate \in
     AsyncCausalEpisodeCandidates(node, snapshot.schedulerCuts[node]):
     candidate.causalOrigin
       \in ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot(
            snapshot, node)}

ExactDecisionTargetNeutralExactCandidateOccurrenceTokensForSnapshot(
    snapshot, node) ==
  {<<candidate, token>>:
     candidate \in
       ExactDecisionTargetNeutralCausalCandidatesForSnapshot(snapshot, node),
     token
       \in 1..AsyncCausalExactRemainingOccurrenceBudget(candidate.kind)}

ExactDecisionTargetNeutralExactCandidateOccurrenceBudgetForSnapshot(
    snapshot, node) ==
  Cardinality(
    ExactDecisionTargetNeutralExactCandidateOccurrenceTokensForSnapshot(
      snapshot, node))

ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot(
    snapshot, node) ==
  {identity \in
     AsyncCausalEpisodeServeIngressIdentities(
       node, snapshot.schedulerCuts[node]):
     AsyncServeIngressAdmissionOrdinal(node, identity)
       < snapshot.physicalCuts[node]}

ExactDecisionTargetNeutralServeWorkTokensForSnapshot(snapshot, node) ==
  {token \in
     AsyncCausalEpisodeServeWorkTokens(
       node, snapshot.schedulerCuts[node]):
     token[2]
       \in ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot(
            snapshot, node)}

ExactDecisionTargetNeutralServeWorkBudgetForSnapshot(snapshot, node) ==
  Cardinality(
    ExactDecisionTargetNeutralServeWorkTokensForSnapshot(snapshot, node))

ExactDecisionTargetNeutralServeReachDebtForSnapshot(snapshot, node) ==
  IF ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot(
       snapshot, node) = {}
  THEN 0
  ELSE DrainableIngressTurnReachRank(node)

ExactDecisionTargetNeutralCausalEpisodeRankForSnapshot(snapshot, node) ==
  <<ExactDecisionTargetNeutralExactCandidateOccurrenceBudgetForSnapshot(
      snapshot, node),
    <<ExactDecisionTargetNeutralServeWorkBudgetForSnapshot(snapshot, node),
      ExactDecisionTargetNeutralServeReachDebtForSnapshot(
        snapshot, node)>>>>

ExactDecisionTargetNeutralCausalEpisodeBottom ==
  <<0, <<0, 0>>>>

\* Proofless carrier replacement is charged outside the physical occurrence
\* rank.  The rigid physical cut contains exactly the leader-wire and
\* ordinary aggregate carriers already admitted when the snapshot was built.
\* Dormant records contribute
\* zero: their exact due packet is paid by the outer fixed-clock packet rank,
\* and any later admission receives a physical ordinal at or above this cut.
\* The continuation coordinate covers Reserved and Materialized local/external
\* handoffs plus their latent Candidate.  Ingress-to-Runtime replacement
\* consumes the stage coordinate before the inner occurrence rank can change.
ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot(
    snapshot, node) ==
  {record \in asyncLeaderWireLifecycles:
  /\ record.recipient = node
  /\ record.schedulerOrdinal <= snapshot.schedulerCuts[node]
  /\ AsyncLeaderWireLifecycleActive(record)
  /\ record.physicalAdmissionOrdinal < snapshot.physicalCuts[node]}

ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot(
    snapshot, node) ==
  {carrier \in
     asyncControlServiceState.ordinaryIngressCarrierEvidence:
     /\ carrier.node = node
     /\ carrier.status = "Ingress"
     /\ carrier.schedulerOrdinal <= snapshot.schedulerCuts[node]
     /\ carrier.physicalOrdinal < snapshot.physicalCuts[node]}

ExactDecisionTargetNeutralLeaderWireRemainingStageForSnapshot(
    snapshot, node, record) ==
  CASE /\ record
              \in ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot(
                   snapshot, node)
       /\ AsyncLeaderWireLifecycleIngressProtected(record) -> 1
    [] OTHER -> 0

ExactDecisionTargetNeutralLeaderWireStageTokensForSnapshot(snapshot, node) ==
  {<<"LeaderWireBarrier", AsyncLeaderWirePotentialOwnerIdentity(record),
     token>>:
     record
       \in ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot(
            snapshot, node),
     token
       \in
         1..ExactDecisionTargetNeutralLeaderWireRemainingStageForSnapshot(
              snapshot, node, record)}
    \cup
  {<<"OrdinaryIngressBarrier", carrier.carrierIdentity, 1>>:
     carrier \in
       ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot(
         snapshot, node)}

ExactDecisionTargetNeutralLeaderWireStageBudgetForSnapshot(snapshot, node) ==
  Cardinality(
    ExactDecisionTargetNeutralLeaderWireStageTokensForSnapshot(
      snapshot, node))

ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot(
    snapshot, node) ==
  {AsyncLeaderWireRuntimeCandidate(record.item):
     record
       \in ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot(
            snapshot, node)}

ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot(
    snapshot, node) ==
  {DeliveryCandidate(carrier.item):
     carrier \in
       ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot(
         snapshot, node)}

ExactDecisionTargetNeutralFrozenContinuationRecordsForSnapshot(
    snapshot, node) ==
  {record \in
     AsyncCandidateProducerContinuationResolutionRecordsForNode(node):
     /\ record.ordinal <= snapshot.schedulerCuts[node]
     /\ record.sourcePhysicalOrdinal < snapshot.physicalCuts[node]}

ExactDecisionTargetNeutralFrozenDormantLocalReplayCandidatesForSnapshot(
    snapshot, node) ==
  {candidate \in AsyncCandidateSet:
     \E record \in
          ExactDecisionTargetNeutralFrozenContinuationRecordsForSnapshot(
            snapshot, node):
       /\ record.status = "Reserved"
       /\ record.sourceClass = "Local"
       /\ ~AsyncCandidateProducerContinuationConcreteSuccessorOwned(record)
       /\ ~AsyncCandidateProducerContinuationHandoffRetired(record)
       /\ candidate = record.candidate}

ExactDecisionTargetNeutralFrozenContinuationStatusTokensForSnapshot(
    snapshot, node) ==
  {<<"Continuation", record.identity, token>>:
     record \in
       ExactDecisionTargetNeutralFrozenContinuationRecordsForSnapshot(
         snapshot, node),
     token
       \in 1..AsyncCandidateProducerContinuationStatusRank(record.status)}

ExactDecisionTargetNeutralProoflessCandidateOwnersForSnapshot(
    snapshot, node) ==
  ExactDecisionTargetNeutralCausalCandidatesForSnapshot(snapshot, node)
    \cup
      ExactDecisionTargetNeutralFrozenDormantLocalReplayCandidatesForSnapshot(
        snapshot, node)
    \cup
      ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot(
        snapshot, node)
    \cup
      ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot(
        snapshot, node)

ExactDecisionTargetNeutralProoflessCandidateTokensForSnapshot(
    snapshot, node) ==
  {<<"Candidate", candidate, token>>:
     candidate
       \in ExactDecisionTargetNeutralProoflessCandidateOwnersForSnapshot(
            snapshot, node),
     token
       \in
         1..AsyncCandidateProducerContinuationCausalWeight(candidate.kind)}

ExactDecisionTargetNeutralProoflessContinuationTokensForSnapshot(
    snapshot, node) ==
  ExactDecisionTargetNeutralProoflessCandidateTokensForSnapshot(
    snapshot, node)
    \cup
      ExactDecisionTargetNeutralFrozenContinuationStatusTokensForSnapshot(
        snapshot, node)

ExactDecisionTargetNeutralProoflessContinuationBudgetForSnapshot(
    snapshot, node) ==
  Cardinality(
    ExactDecisionTargetNeutralProoflessContinuationTokensForSnapshot(
      snapshot, node))

ExactDecisionTargetNeutralProoflessProducerRankForSnapshot(
    snapshot, node) ==
  <<ExactDecisionTargetNeutralLeaderWireStageBudgetForSnapshot(
      snapshot, node),
    ExactDecisionTargetNeutralProoflessContinuationBudgetForSnapshot(
      snapshot, node)>>

ExactDecisionTargetNeutralProoflessProducerCarrier ==
  Nat \X Nat

ExactDecisionTargetNeutralProoflessProducerOrdering ==
  LexPairOrdering(OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)

ExactDecisionTargetNeutralProoflessProducerBottom ==
  <<0, 0>>

ExactDecisionTargetNeutralComposedCausalEpisodeRankForSnapshot(
    snapshot, node) ==
  <<ExactDecisionTargetNeutralProoflessProducerRankForSnapshot(
       snapshot, node),
    ExactDecisionTargetNeutralCausalEpisodeRankForSnapshot(snapshot, node)>>

ExactDecisionTargetNeutralComposedCausalEpisodeCarrier ==
  ExactDecisionTargetNeutralProoflessProducerCarrier
    \X AsyncCausalEpisodeStructuralRankCarrier

ExactDecisionTargetNeutralComposedCausalEpisodeOrdering ==
  LexPairOrdering(
    ExactDecisionTargetNeutralProoflessProducerOrdering,
    AsyncCausalEpisodeStructuralRankOrdering,
    ExactDecisionTargetNeutralProoflessProducerCarrier,
    AsyncCausalEpisodeStructuralRankCarrier)

ExactDecisionTargetNeutralComposedCausalEpisodeBottom ==
  <<ExactDecisionTargetNeutralProoflessProducerBottom,
    ExactDecisionTargetNeutralCausalEpisodeBottom>>

ExactDecisionTargetNeutralSelectedCausalEpisodeRankForSnapshot(snapshot) ==
  IF OverdueResponsivePackets = {}
  THEN ExactDecisionTargetNeutralComposedCausalEpisodeBottom
  ELSE LET packet == ExactDecisionTargetNeutralSelectedOverduePacket
           node == packet.item.envelope.recipient
       IN ExactDecisionTargetNeutralComposedCausalEpisodeRankForSnapshot(
            snapshot, node)

ExactDecisionTargetNeutralProducerEpisodeRank(snapshot) ==
  <<ExactDecisionTargetNeutralIngressEpisodeRank(snapshot),
    ExactDecisionTargetNeutralSelectedCausalEpisodeRankForSnapshot(
      snapshot)>>

ExactDecisionTargetNeutralProducerEpisodeCarrier ==
  Nat \X ExactDecisionTargetNeutralComposedCausalEpisodeCarrier

ExactDecisionTargetNeutralProducerEpisodeOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    ExactDecisionTargetNeutralComposedCausalEpisodeOrdering,
    Nat,
    ExactDecisionTargetNeutralComposedCausalEpisodeCarrier)

ExactDecisionTargetNeutralProducerEpisodeBottom ==
  <<0, ExactDecisionTargetNeutralComposedCausalEpisodeBottom>>

ExactDecisionTargetNeutralFixedPredecessorSetForSnapshot(
    snapshot, clockValue) ==
  ({"Packet"} \X
     (HistoricalDiscoveryDuePacketsAt(clockValue)
        \cap snapshot.packets))
    \cup
  ({"Candidate"} \X
     (ExactDecisionTargetNeutralLiveCandidateIdentitySet
        \cap snapshot.candidateIdentities))
    \cup
  ({"Serve"} \X
     (ExactDecisionTargetNeutralLiveServeIdentitySet
        \cap snapshot.serveIdentities))
    \cup
  ({"SchedulerCut"} \X
     ExactDecisionTargetNeutralSchedulerCutTokens(snapshot))

ExactDecisionTargetNeutralRetainedCausalRootsForSnapshot(snapshot) ==
  ExactDecisionTargetNeutralRetainedCandidateCausalRoots
    \cap snapshot.candidateRoots

ExactDecisionTargetNeutralFixedPredecessorSet(clockValue) ==
  ({"Packet"} \X HistoricalDiscoveryDuePacketsAt(clockValue))
    \cup
  ({"Candidate"} \X
     ExactDecisionTargetNeutralCandidateEpisodeUniverse(clockValue))
    \cup
  ({"Serve"} \X
     ExactDecisionTargetNeutralServeEpisodeUniverse(clockValue))
    \cup
  ({"SchedulerCut"} \X
     ExactDecisionTargetNeutralSchedulerCutTokens(
       [schedulerCuts |->
          ExactDecisionTargetNeutralCurrentPastSchedulerCuts]))

\* The predecessor fields freeze every currently materialized identity and,
\* separately, the complete causal-root universe which may still materialize
\* a descendant ahead of the target.  These fields never grant a future ordinal
\* interval: an identity rooted after the scheduler cut is excluded by
\* `candidateRoots`, while an admission at or after the physical cut is behind
\* the target and every due request/source coordinate is precharged by the
\* journal.
ExactDecisionTargetNeutralFixedClockSnapshot(clockValue) ==
  LET schedulerCuts ==
        ExactDecisionTargetNeutralCurrentPastSchedulerCuts
      physicalCuts == ExactDecisionTargetNeutralCurrentPhysicalCuts
  IN [clock |-> clockValue,
   packets |-> HistoricalDiscoveryDuePacketsAt(clockValue),
   producerRequests |->
     ExactDecisionTargetNeutralFrozenProducerRequests(clockValue),
   ingressEpisodes |->
     ExactDecisionTargetNeutralIngressEpisodeUniverse(clockValue),
   candidateRoots |->
     ExactDecisionTargetNeutralFrozenCausalRoots(clockValue),
   schedulerCuts |-> schedulerCuts,
   physicalCuts |-> physicalCuts,
   predecessors |->
     ({"Packet"} \X HistoricalDiscoveryDuePacketsAt(clockValue))
       \cup
     ({"Candidate"} \X
        ExactDecisionTargetNeutralCandidateEpisodeUniverse(clockValue))
       \cup
     ({"Serve"} \X
        ExactDecisionTargetNeutralServeEpisodeUniverse(clockValue))
       \cup
     ({"SchedulerCut"} \X
        ExactDecisionTargetNeutralSchedulerCutTokens(
          [schedulerCuts |-> schedulerCuts])),
   candidateIdentities |->
     ExactDecisionTargetNeutralCandidateEpisodeUniverse(clockValue),
   serveIdentities |->
     ExactDecisionTargetNeutralServeEpisodeUniverse(clockValue)]

\* The constructor above reads the source state exactly once.  Temporal
\* arguments carry the resulting record as a rigid quantified value; these
\* projections make that state-freezing boundary explicit.
ExactDecisionTargetNeutralFrozenSnapshotCarriers(snapshot) ==
  [clock |-> snapshot.clock,
   packets |-> snapshot.packets,
   producerRequests |-> snapshot.producerRequests,
   ingressEpisodes |-> snapshot.ingressEpisodes,
   candidateRoots |-> snapshot.candidateRoots,
   schedulerCuts |-> snapshot.schedulerCuts,
   physicalCuts |-> snapshot.physicalCuts,
   predecessors |-> snapshot.predecessors,
   candidateIdentities |-> snapshot.candidateIdentities,
   serveIdentities |-> snapshot.serveIdentities]

THEOREM ExactDecisionTargetNeutralFrozenSnapshotCarriersArePrimeInvariant ==
  \A snapshot:
    ExactDecisionTargetNeutralFrozenSnapshotCarriers(snapshot)'
      = ExactDecisionTargetNeutralFrozenSnapshotCarriers(snapshot)
BY Isa
   DEF ExactDecisionTargetNeutralFrozenSnapshotCarriers

ExactDecisionTargetNeutralSnapshotActive(snapshot, clockValue) ==
  /\ snapshot.clock = clockValue
  /\ AsyncCandidateServiceLifecycleInvariant
  /\ IsFiniteSet(snapshot.packets)
  /\ IsFiniteSet(snapshot.predecessors)
  /\ IsFiniteSet(snapshot.producerRequests)
  /\ IsFiniteSet(snapshot.ingressEpisodes)
  /\ IsFiniteSet(snapshot.candidateRoots)
  /\ IsFiniteSet(snapshot.candidateIdentities)
  /\ IsFiniteSet(snapshot.serveIdentities)
  /\ snapshot.schedulerCuts \in [Responsive -> Nat]
  /\ snapshot.physicalCuts \in [Responsive -> Nat]
  /\ \A node \in Responsive:
       snapshot.schedulerCuts[node]
         < AsyncNextCandidateLifecycleOrdinal(node)
  /\ \A node \in Responsive:
       snapshot.physicalCuts[node]
         <= AsyncNextIngressPhysicalOrdinal(node)
  /\ snapshot.candidateIdentities
       \subseteq ExactDecisionTargetNeutralCandidateOwnerIdentitySet
  /\ snapshot.candidateRoots
       \subseteq ExactDecisionTargetNeutralCausalRootSet
  /\ snapshot.serveIdentities
       \subseteq ExactDecisionTargetNeutralServeOwnerIdentitySet
  /\ snapshot.producerRequests \subseteq AsyncProducerIngressRequests
  /\ snapshot.ingressEpisodes =
       {AsyncProducerEpisode(
          request, authenticatedSource, AsyncProducerIngressStage):
          request \in snapshot.producerRequests,
          authenticatedSource \in AsyncIngressSources}
  /\ snapshot.predecessors =
       ({"Packet"} \X snapshot.packets)
         \cup
       ({"Candidate"} \X snapshot.candidateIdentities)
         \cup
       ({"Serve"} \X snapshot.serveIdentities)
         \cup
       ({"SchedulerCut"} \X
          ExactDecisionTargetNeutralSchedulerCutTokens(snapshot))
  /\ ExactDecisionTargetNeutralFixedPredecessorSetForSnapshot(
       snapshot, clockValue)
       \subseteq snapshot.predecessors
  /\ ExactDecisionTargetNeutralRetainedCausalRootsForSnapshot(snapshot)
       \subseteq snapshot.candidateRoots
  /\ ExactDecisionTargetNeutralFrozenCandidateLifecycleCovered(snapshot)
  /\ ExactDecisionTargetNeutralFrozenServeLifecycleCovered(snapshot)
  /\ HistoricalDiscoveryDuePacketsAt(clockValue)
       \subseteq snapshot.packets

ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot(
    snapshot, clockValue) ==
  HistoricalDiscoveryFixedClockRank(
    clockValue,
    ExactDecisionTargetNeutralConcreteBlockerStage(clockValue),
    ExactDecisionTargetNeutralConcreteDependencyRankForSnapshot(
      snapshot, clockValue))

ExactDecisionTargetNeutralFixedClockCarrier ==
  HistoricalDiscoveryFixedClockBlockerCarrier

ExactDecisionTargetNeutralFixedClockOrdering ==
  HistoricalDiscoveryFixedClockBlockerOrdering

ExactDecisionTargetNeutralDependencyProducerPrefix(dependencyRank) ==
  <<dependencyRank[1],
    dependencyRank[2][1],
    dependencyRank[2][2][1],
    dependencyRank[2][2][2][1],
    dependencyRank[2][2][2][2][1],
    dependencyRank[2][2][2][2][2][1],
    dependencyRank[2][2][2][2][2][2][1],
    dependencyRank[2][2][2][2][2][2][2][1]>>

ExactDecisionTargetNeutralProducerPrefix(rank) ==
  <<rank[1],
    rank[2][1],
    rank[2][2][1],
    rank[2][2][2][1],
    ExactDecisionTargetNeutralDependencyProducerPrefix(
      rank[2][2][2][2])>>

ExactDecisionTargetNeutralModeSet ==
  {"RequestClock", "RequestHead", "ResponseHead"}

ExactDecisionTargetNeutralResidual(
    mode, node, qc, archive, request, response, packet) ==
  CASE mode = "RequestClock" ->
         ExactDecisionRequestPacketEmissionResidual(node, qc)
    [] mode = "RequestHead" ->
         ExactDecisionRequestHeadGateOwnerResidual(
           node, qc, archive, request, packet)
    [] mode = "ResponseHead" ->
         ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual(
           node, qc, archive, request, response, packet)
    [] OTHER -> FALSE

ExactDecisionTargetNeutralGoal(
    mode, node, qc, archive, request, response, packet) ==
  CASE mode = "RequestClock" ->
         \/ ExactDecisionRequestPacketEmissionGoal(node, qc)
         \/ ExactDecisionRequestClockElapsedResidual(node, qc)
    [] mode = "RequestHead" ->
         \/ ExactDecisionRequestIngressGoal(
              node, qc, archive, request)
         \/ ExactDecisionRequestPacketAdmissionReady(
              node, qc, archive, request, packet)
    [] mode = "ResponseHead" ->
         \/ ExactDecisionResponseAdmissionGoal(node, qc)
         \/ ExactDecisionResponsePacketAdmissionReady(
              node, qc, archive, request, response, packet)
    [] OTHER -> FALSE

ExactDecisionTargetNeutralDeadline(
    mode, node, packet) ==
  IF mode = "RequestClock"
  THEN asyncRetransmitDeadlines[node]
  ELSE packet.deadline

ExactDecisionTargetNeutralFixedClockPending(
    snapshot, mode, node, qc, archive,
    request, response, packet, clockValue) ==
  /\ mode \in ExactDecisionTargetNeutralModeSet
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ gst
  /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, clockValue)
  /\ ExactDecisionTargetNeutralResidual(
       mode, node, qc, archive, request, response, packet)
  /\ ~ExactDecisionTargetNeutralGoal(
       mode, node, qc, archive, request, response, packet)
  /\ asyncNow = clockValue

ExactDecisionTargetNeutralFixedClockExit(
    mode, node, qc, archive, request, response, packet, clockValue) ==
  \/ ExactDecisionTargetNeutralGoal(
       mode, node, qc, archive, request, response, packet)
  \/ asyncNow > clockValue

ExactDecisionTargetNeutralFixedClockBlockedAtRank(
    snapshot, mode, node, qc, archive, request, response,
    packet, clockValue, rank) ==
  /\ ExactDecisionTargetNeutralFixedClockPending(
       snapshot, mode, node, qc, archive,
       request, response, packet, clockValue)
  /\ rank \in ExactDecisionTargetNeutralFixedClockCarrier
  /\ ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot(
       snapshot, clockValue)
       = rank

ExactDecisionTargetNeutralFixedClockStrictRankGoal(
    snapshot, mode, node, qc, archive, request, response,
    packet, clockValue, sourceRank) ==
  \/ ExactDecisionTargetNeutralFixedClockExit(
       mode, node, qc, archive, request, response, packet, clockValue)
  \/ \E lowerRank \in
       SetLessThan(
         sourceRank,
         ExactDecisionTargetNeutralFixedClockOrdering,
         ExactDecisionTargetNeutralFixedClockCarrier):
       ExactDecisionTargetNeutralFixedClockBlockedAtRank(
         snapshot, mode, node, qc, archive, request, response,
         packet, clockValue, lowerRank)

\* The episode budget is a genuine well-founded rank, not an ordinal
\* allowance.  Its ingress head coalesces exact retransmissions through the
\* producer journal; its causal tail prepays all remaining same-origin
\* occurrences and every Serve predecessor below the frozen scheduler cut.
ExactDecisionTargetNeutralProducerEpisodeBudget(snapshot) ==
  ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)

ExactDecisionTargetNeutralProducerEpisodeAtBudget(
    snapshot, mode, node, qc, archive, request, response,
    packet, clockValue, sourceRank, budget) ==
  LET currentRank ==
        ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot(
          snapshot, clockValue)
  IN /\ ExactDecisionTargetNeutralFixedClockPending(
           snapshot, mode, node, qc, archive,
           request, response, packet, clockValue)
     /\ sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier
     /\ currentRank \in ExactDecisionTargetNeutralFixedClockCarrier
     /\ budget \in ExactDecisionTargetNeutralProducerEpisodeCarrier
     /\ ~ExactDecisionTargetNeutralFixedClockStrictRankGoal(
          snapshot, mode, node, qc, archive, request, response,
          packet, clockValue, sourceRank)
     /\ ExactDecisionTargetNeutralProducerPrefix(currentRank)
          = ExactDecisionTargetNeutralProducerPrefix(sourceRank)
     /\ budget = ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)

ExactDecisionTargetNeutralRankCellOutcome(
    snapshot, mode, node, qc, archive, request, response,
    packet, clockValue, sourceRank, budget) ==
  \/ ExactDecisionTargetNeutralFixedClockStrictRankGoal(
       snapshot, mode, node, qc, archive, request, response,
       packet, clockValue, sourceRank)
  \/ \E lowerBudget \in
       SetLessThan(
         budget,
         ExactDecisionTargetNeutralProducerEpisodeOrdering,
         ExactDecisionTargetNeutralProducerEpisodeCarrier):
       ExactDecisionTargetNeutralProducerEpisodeAtBudget(
         snapshot, mode, node, qc, archive, request, response,
         packet, clockValue, sourceRank, lowerBudget)

ExactDecisionTargetNeutralFairOwner(
    ownerKind, node, source) ==
  [ownerKind |-> ownerKind, node |-> node, source |-> source]

\* Every owner is individually weakly fair.  The three producer-continuation
\* owners are distinct non-runner source-class actions in AsyncFairnessAt;
\* RunNode does not subsume them.  `Admit` is the single atomic physical
\* packet/lifecycle handoff.
ExactDecisionTargetNeutralFairOwnerSet(initialContext) ==
  {ExactDecisionTargetNeutralFairOwner(
     "Tick", 0, AsyncUntrustedSource)}
  \cup
  {ExactDecisionTargetNeutralFairOwner(
     "RunNode", node, AsyncUntrustedSource):
     node \in AsyncVotersAt(initialContext)}
  \cup
  {ExactDecisionTargetNeutralFairOwner(
     "ResolveLocalCandidateProducerContinuation",
     node, AsyncUntrustedSource):
     node \in AsyncVotersAt(initialContext)}
  \cup
  {ExactDecisionTargetNeutralFairOwner(
     "ServiceConditionalTransportProducerContinuation",
     node, AsyncUntrustedSource):
     node \in AsyncVotersAt(initialContext)}
  \cup
  {ExactDecisionTargetNeutralFairOwner(
     "ServiceVolatileBodyProducerContinuation",
     node, AsyncUntrustedSource):
     node \in AsyncVotersAt(initialContext)}
  \cup
  {ExactDecisionTargetNeutralFairOwner(
     ownerKind, node, AsyncUntrustedSource):
     ownerKind
       \in {"RunHistoricalRecovery", "RunHistoricalServer",
            "ServiceIo", "ServiceHistoricalIo"},
     node \in Responsive}
  \cup
  {ExactDecisionTargetNeutralFairOwner(
     "Admit", recipient, source):
     recipient \in Responsive,
     source \in AsyncIngressSources}
  \cup
  {ExactDecisionTargetNeutralFairOwner(
     "AdmitHistorical", recipient, source):
     recipient \in ValidatorIds,
     source \in AsyncIngressSources}

ExactDecisionTargetNeutralFairAction(owner) ==
  CASE owner.ownerKind = "Tick" -> AsyncTick
    [] owner.ownerKind = "RunNode" ->
         PostGstRunNode(owner.node)
    [] owner.ownerKind =
         "ResolveLocalCandidateProducerContinuation" ->
         PostGstResolveLocalCandidateProducerContinuation(owner.node)
    [] owner.ownerKind =
         "ServiceConditionalTransportProducerContinuation" ->
         PostGstServiceConditionalTransportProducerContinuation(owner.node)
    [] owner.ownerKind =
         "ServiceVolatileBodyProducerContinuation" ->
         PostGstServiceVolatileBodyProducerContinuation(owner.node)
    [] owner.ownerKind = "RunHistoricalRecovery" ->
         PostGstRunHistoricalRecoveryNode(owner.node)
    [] owner.ownerKind = "RunHistoricalServer" ->
         PostGstRunHistoricalServer(owner.node)
    [] owner.ownerKind = "ServiceIo" ->
         PostGstServiceIoWorker(owner.node)
    [] owner.ownerKind = "ServiceHistoricalIo" ->
         PostGstServiceHistoricalRecoveryIoWorker(owner.node)
    [] owner.ownerKind = "Admit" ->
         PostGstAdmitHiddenPacket(owner.node, owner.source)
    [] owner.ownerKind = "AdmitHistorical" ->
         PostGstAdmitHistoricalRecoveryPacket(
           owner.node, owner.source)
    [] OTHER -> FALSE

ExactDecisionTargetNeutralOwnerReadyForRankCell(
    initialContext, snapshot, mode, node, qc, archive,
    request, response, packet, clockValue, sourceRank, budget, owner) ==
  /\ owner \in
       ExactDecisionTargetNeutralFairOwnerSet(initialContext)
  /\ ENABLED
       (ExactDecisionTargetNeutralFairAction(owner)
          /\ ExactDecisionTargetNeutralRankCellOutcome(
               snapshot, mode, node, qc, archive, request, response,
               packet, clockValue, sourceRank, budget)')

ExactDecisionTargetNeutralSelectedFairOwner(
    initialContext, snapshot, mode, node, qc, archive,
    request, response, packet, clockValue, sourceRank, budget) ==
  CHOOSE owner \in
    ExactDecisionTargetNeutralFairOwnerSet(initialContext):
      ExactDecisionTargetNeutralOwnerReadyForRankCell(
        initialContext, snapshot, mode, node, qc, archive,
        request, response, packet, clockValue,
        sourceRank, budget, owner)

ExactDecisionTargetNeutralProducerEpisodeOwnedBy(
    initialContext, snapshot, mode, node, qc, archive,
    request, response, packet, clockValue,
    sourceRank, budget, owner) ==
  /\ ExactDecisionTargetNeutralProducerEpisodeAtBudget(
       snapshot, mode, node, qc, archive, request, response,
       packet, clockValue, sourceRank, budget)
  /\ owner =
       ExactDecisionTargetNeutralSelectedFairOwner(
         initialContext, snapshot, mode, node, qc, archive,
         request, response, packet, clockValue, sourceRank, budget)

(***************************************************************************
The source transition installs a same-generation transient service marker or
a restart-durable terminal-discard tombstone independently of adequate-leader
convergence.  These local theorems repeat only the
initialization/preservation bridge needed by the fixed-GST rank episode;
they do not import the adequate-leader module.  The restart bridge is stated
separately so pre-GST reconstruction cannot be mistaken for rank progress.
***************************************************************************)

THEOREM ExactDecisionAsyncInitEstablishesCandidateTombstones ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncCandidateServiceLifecycleInvariant
BY AsyncInitEstablishesLeaderWireContinuationSharedOrdinalNoCollision,
   Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       AsyncRuntimeInit, AsyncIoInit, AsyncDeferredInit,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateProducerSemanticHandoffCoverageInvariant,
       AsyncCandidateLifecycleAdmissions,
       AsyncInitialCandidateLifecycleAdmissions,
       AsyncCandidateLifecycleAdmission,
       AsyncCandidateProducerContinuationScheduledExclusionInvariant,
       AsyncCandidateProducerContinuationBlocks,
       AsyncCandidateProducerContinuations,
       AsyncControlServiceStateTypeInvariant,
       AsyncCandidateServiceTombstones,
       AsyncCandidateServiceRecordsFor,
       AsyncCandidateServiceRecordsForIdentity,
       QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       SequenceSet

THEOREM ExactDecisionAsyncNextPreservesCandidateTombstones ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateServiceLifecycleInvariant
  /\ AsyncNext
  => AsyncCandidateServiceLifecycleInvariant'
BY AsyncNextPreservesControlServiceStateTypeInvariant,
   AsyncNextPreservesLeaderWireContinuationSharedOrdinalNoCollision,
   AsyncControlServiceTransitionPreservesSemanticHandoffCoverage,
   AsyncNextPreservesCandidateProducerContinuationScheduledExclusion,
   AsyncCandidateServicesThisStepIsSingleton,
   AsyncCandidateSuccessfulServiceInstallsTransientMarker,
   AsyncCandidateCausalAdmissionTransfersSameOwner,
   AsyncCandidateIoCompletionTransfersSameOwner,
   AsyncCandidateProducerCompletionTransfersSameOwner,
   AsyncCandidateBusyDeferralTransfersSameOwner,
   AsyncCandidateDeferredHandoffRetainsSameOwner,
   AsyncCandidateDiscardIsNotSemanticService,
   AsyncCandidateTerminalRetirementsThisStepIsSingleton,
   AsyncCandidateDiscardInstallsTerminalTombstone,
   AsyncCandidateDiscardRetiresLogicalLifecycle,
   AsyncCandidateTransientMarkerCoalescesFreshCandidate,
   AsyncCandidateTerminalTombstoneCoalescesFreshCandidate,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateSameHeightRestartPreservesTombstone,
   AsyncCandidateResponsiveRestartPermitsNonterminalReconstruction,
   IsaT(600)
   DEF AsyncCandidateServiceLifecycleInvariant,
       AsyncStrongTypeInvariant,
       AsyncProgressOwnershipInvariant,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       DrainFairIngressSelected, AdmitCausalHead,
       AdmitProducerCompletion, ServiceIoWorkerWork,
       FifoRuntimeStep, DeferredDrainStep,
       AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CandidateAdmissionCoalesced,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       FreshRestartCandidateSequence,
       CandidateScheduled, CandidateScheduledAfter

THEOREM ExactDecisionAsyncSpecAlwaysCandidateTombstones ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []AsyncCandidateServiceLifecycleInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext)
         PROVE []AsyncCandidateServiceLifecycleInvariant
    <2>1. AsyncInitAt(initialContext)
             => AsyncCandidateServiceLifecycleInvariant
      BY ExactDecisionAsyncInitEstablishesCandidateTombstones
    <2>2. [](/\ AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
    <2>3. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ AsyncCandidateServiceLifecycleInvariant
           /\ [AsyncNext]_AsyncAllVars
          => AsyncCandidateServiceLifecycleInvariant'
      BY ExactDecisionAsyncNextPreservesCandidateTombstones, Isa
         DEF AsyncAllVars, AsyncProducerVars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, PTL
         DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM ExactDecisionSameGenerationCandidateServiceCannotReactivateAtGst ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCandidateTransientServiceActive(candidate)
    /\ candidate.consumerGeneration = generation[candidate.node]
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    /\ ~AsyncCandidateTransientMarkerExitThisStep(candidate)
    => /\ AsyncCandidateTransientServiceActive(candidate)'
       /\ ~CandidateScheduled(candidate)'
BY AsyncCandidateSameGenerationServicedIdentityCannotReactivateAtGst

THEOREM ExactDecisionTerminalCandidateDiscardCannotReactivateAtGst ==
  \A identity \in AsyncCandidateAdmissionIdentitySet:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCandidateTerminalIdentityTombstoned(identity.service)
    /\ identity \notin AsyncScheduledCandidateAdmissionIdentities
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => /\ AsyncCandidateAdmissionIdentityTerminallyCovered(identity)'
       /\ identity \notin AsyncScheduledCandidateAdmissionIdentities'
BY AsyncCandidateTerminalIdentityCannotReactivateAtGst

THEOREM ExactDecisionResponsiveRestartPermitsNonterminalCandidateReconstruction ==
  \A item \in AsyncNetworkItems:
    LET candidate == DeliveryCandidate(item)
    IN /\ candidate.node = asyncRecoveryNode
       /\ AsyncCandidateTransientServiceActive(candidate)
       /\ ~AsyncCandidateTerminalTombstoned(candidate)
       /\ PreGstResponsiveReplay
       /\ AsyncControlServiceSlotTransition
       => /\ ~AsyncCandidateTransientServiceMarked(candidate)'
          /\ ~AsyncCandidateServicePacketRetired(item)'
BY AsyncCandidateResponsiveRestartPermitsNonterminalReconstruction

THEOREM ExactDecisionTargetNeutralSnapshotIsFinite ==
  \A clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
      => LET snapshot ==
               ExactDecisionTargetNeutralFixedClockSnapshot(clockValue)
         IN /\ IsFiniteSet(snapshot.packets)
            /\ IsFiniteSet(snapshot.predecessors)
            /\ IsFiniteSet(snapshot.producerRequests)
            /\ IsFiniteSet(snapshot.ingressEpisodes)
            /\ IsFiniteSet(snapshot.candidateRoots)
            /\ IsFiniteSet(snapshot.candidateIdentities)
            /\ IsFiniteSet(snapshot.serveIdentities)
            /\ snapshot.schedulerCuts \in [Responsive -> Nat]
            /\ snapshot.physicalCuts \in [Responsive -> Nat]
            /\ ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)
                 \in ExactDecisionTargetNeutralProducerEpisodeCarrier
BY StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   StrongTypeHasFiniteHistoricalDiscoveryRankOwners,
   AsyncCausalEpisodeStructuralRankAtFiniteCutIsFinite,
   CandidateProducerContinuationFrozenCandidateCarrierHasConfiguredBound,
   RuntimeValidatorIdsAreFinite,
   FS_Image, FS_Union, FS_Product, FS_Subset, FS_Intersection,
   FS_CardinalityType, Isa
   DEF ExactDecisionTargetNeutralFixedClockSnapshot,
       ExactDecisionTargetNeutralFrozenProducerRequests,
       ExactDecisionTargetNeutralIngressEpisodeUniverse,
       AsyncProducerEpisode, AsyncProducerIngressStage,
       ExactDecisionTargetNeutralCandidateEpisodeUniverse,
       ExactDecisionTargetNeutralFrozenCausalRoots,
       ExactDecisionTargetNeutralRetainedCandidateCausalRoots,
       ExactDecisionTargetNeutralLiveCandidateCausalRoots,
       ExactDecisionTargetNeutralLifecycleRecordCausalRoots,
       ExactDecisionTargetNeutralProducerContinuationCausalRoots,
       ExactDecisionTargetNeutralOrdinaryIngressCausalRoots,
       ExactDecisionTargetNeutralLeaderWireCausalRoots,
       ExactDecisionTargetNeutralDurableReplayCausalRoots,
       ExactDecisionTargetNeutralTimeoutReservationCausalRoots,
       ExactDecisionTargetNeutralDuePacketCausalRoots,
       ExactDecisionTargetNeutralCurrentPastSchedulerCut,
       ExactDecisionTargetNeutralCurrentPastSchedulerCuts,
       ExactDecisionTargetNeutralCurrentPhysicalCuts,
       ExactDecisionTargetNeutralSchedulerCutToken,
       ExactDecisionTargetNeutralSchedulerCutTokens,
       ExactDecisionTargetNeutralServeEpisodeUniverse,
       ExactDecisionTargetNeutralRetainedServeLifecycleRecords,
       ExactDecisionTargetNeutralRetainedServeIdentitySet,
       ExactDecisionTargetNeutralLiveCandidateIdentitySet,
       ExactDecisionTargetNeutralLiveServeIdentitySet,
       ExactDecisionTargetNeutralCandidateOwnerIdentity,
       ExactDecisionTargetNeutralCausalRoot,
       ExactDecisionTargetNeutralServeOwnerIdentity,
       ExactDecisionTargetNeutralServeRequestOwnerIdentity,
       ExactDecisionTargetNeutralProducerEpisodeRank,
       ExactDecisionTargetNeutralProducerEpisodeSet,
       ExactDecisionTargetNeutralIngressEpisodeRemaining,
       ExactDecisionTargetNeutralCandidateEpisodeRemaining,
       ExactDecisionTargetNeutralServeEpisodeRemaining,
       ExactDecisionTargetNeutralIngressEpisodeRank,
       ExactDecisionTargetNeutralSelectedCausalEpisodeRankForSnapshot,
       ExactDecisionTargetNeutralCausalEpisodeRankForSnapshot,
       ExactDecisionTargetNeutralCausalEpisodeBottom,
       ExactDecisionTargetNeutralProoflessProducerRankForSnapshot,
       ExactDecisionTargetNeutralProoflessProducerCarrier,
       ExactDecisionTargetNeutralProoflessProducerBottom,
       ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot,
       ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot,
       ExactDecisionTargetNeutralLeaderWireRemainingStageForSnapshot,
       ExactDecisionTargetNeutralLeaderWireStageTokensForSnapshot,
       ExactDecisionTargetNeutralLeaderWireStageBudgetForSnapshot,
       ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot,
       ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenContinuationRecordsForSnapshot,
       ExactDecisionTargetNeutralFrozenDormantLocalReplayCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenContinuationStatusTokensForSnapshot,
       ExactDecisionTargetNeutralCausalCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot,
       ExactDecisionTargetNeutralExactCandidateOccurrenceTokensForSnapshot,
       ExactDecisionTargetNeutralExactCandidateOccurrenceBudgetForSnapshot,
       ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot,
       ExactDecisionTargetNeutralServeWorkTokensForSnapshot,
       ExactDecisionTargetNeutralServeWorkBudgetForSnapshot,
       ExactDecisionTargetNeutralServeReachDebtForSnapshot,
       ExactDecisionTargetNeutralProoflessCandidateOwnersForSnapshot,
       ExactDecisionTargetNeutralProoflessCandidateTokensForSnapshot,
       ExactDecisionTargetNeutralProoflessContinuationTokensForSnapshot,
       ExactDecisionTargetNeutralProoflessContinuationBudgetForSnapshot,
       ExactDecisionTargetNeutralComposedCausalEpisodeRankForSnapshot,
       ExactDecisionTargetNeutralComposedCausalEpisodeCarrier,
       ExactDecisionTargetNeutralComposedCausalEpisodeBottom,
       ExactDecisionTargetNeutralProducerEpisodeCarrier,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCausalEpisodeStructuralRankCarrier,
       AsyncCausalEpisodeServeRankCarrier,
       AsyncCandidateAdmissionIdentity,
       AsyncStrongTypeInvariant, AsyncConfiguration,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ModelConfiguration, QuorumConfiguration

THEOREM ExactDecisionTargetNeutralEpisodeRankIsInCarrier ==
  \A snapshot:
    \A clockValue \in Nat:
      /\ AsyncStrongTypeInvariant
      /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, clockValue)
      => ExactDecisionTargetNeutralProducerEpisodeBudget(snapshot)
           \in ExactDecisionTargetNeutralProducerEpisodeCarrier
BY AsyncCausalEpisodeStructuralRankAtFiniteCutIsFinite,
   CandidateProducerContinuationFrozenCandidateCarrierHasConfiguredBound,
   RuntimeValidatorIdsAreFinite,
   FS_Interval, FS_Image, FS_Union, FS_Product,
   FS_CardinalityType, Isa
   DEF ExactDecisionTargetNeutralProducerEpisodeBudget,
       ExactDecisionTargetNeutralProducerEpisodeRank,
       ExactDecisionTargetNeutralProducerEpisodeCarrier,
       ExactDecisionTargetNeutralIngressEpisodeRank,
       ExactDecisionTargetNeutralSelectedCausalEpisodeRankForSnapshot,
       ExactDecisionTargetNeutralCausalEpisodeRankForSnapshot,
       ExactDecisionTargetNeutralCausalEpisodeBottom,
       ExactDecisionTargetNeutralProoflessProducerRankForSnapshot,
       ExactDecisionTargetNeutralProoflessProducerCarrier,
       ExactDecisionTargetNeutralProoflessProducerBottom,
       ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot,
       ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot,
       ExactDecisionTargetNeutralLeaderWireRemainingStageForSnapshot,
       ExactDecisionTargetNeutralLeaderWireStageTokensForSnapshot,
       ExactDecisionTargetNeutralLeaderWireStageBudgetForSnapshot,
       ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot,
       ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenContinuationRecordsForSnapshot,
       ExactDecisionTargetNeutralFrozenDormantLocalReplayCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenContinuationStatusTokensForSnapshot,
       ExactDecisionTargetNeutralCausalCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot,
       ExactDecisionTargetNeutralExactCandidateOccurrenceTokensForSnapshot,
       ExactDecisionTargetNeutralExactCandidateOccurrenceBudgetForSnapshot,
       ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot,
       ExactDecisionTargetNeutralServeWorkTokensForSnapshot,
       ExactDecisionTargetNeutralServeWorkBudgetForSnapshot,
       ExactDecisionTargetNeutralServeReachDebtForSnapshot,
       ExactDecisionTargetNeutralProoflessCandidateOwnersForSnapshot,
       ExactDecisionTargetNeutralProoflessCandidateTokensForSnapshot,
       ExactDecisionTargetNeutralProoflessContinuationTokensForSnapshot,
       ExactDecisionTargetNeutralProoflessContinuationBudgetForSnapshot,
       ExactDecisionTargetNeutralComposedCausalEpisodeRankForSnapshot,
       ExactDecisionTargetNeutralComposedCausalEpisodeCarrier,
       ExactDecisionTargetNeutralComposedCausalEpisodeBottom,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCausalEpisodeStructuralRankCarrier,
       AsyncCausalEpisodeServeRankCarrier,
       ExactDecisionTargetNeutralSnapshotActive

THEOREM ExactDecisionTargetNeutralProoflessProducerOrderingIsWellFounded ==
  IsWellFoundedOn(
    ExactDecisionTargetNeutralProoflessProducerOrdering,
    ExactDecisionTargetNeutralProoflessProducerCarrier)
BY NatLessThanWellFounded, WFLexPairOrdering
   DEF ExactDecisionTargetNeutralProoflessProducerOrdering,
       ExactDecisionTargetNeutralProoflessProducerCarrier

THEOREM ExactDecisionTargetNeutralComposedCausalEpisodeOrderingIsWellFounded ==
  IsWellFoundedOn(
    ExactDecisionTargetNeutralComposedCausalEpisodeOrdering,
    ExactDecisionTargetNeutralComposedCausalEpisodeCarrier)
BY ExactDecisionTargetNeutralProoflessProducerOrderingIsWellFounded,
   AsyncCausalEpisodeStructuralRankOrderingIsWellFounded,
   WFLexPairOrdering
   DEF ExactDecisionTargetNeutralComposedCausalEpisodeOrdering,
       ExactDecisionTargetNeutralComposedCausalEpisodeCarrier

THEOREM ExactDecisionTargetNeutralProducerEpisodeOrderingIsWellFounded ==
  IsWellFoundedOn(
    ExactDecisionTargetNeutralProducerEpisodeOrdering,
    ExactDecisionTargetNeutralProducerEpisodeCarrier)
BY NatLessThanWellFounded,
   ExactDecisionTargetNeutralComposedCausalEpisodeOrderingIsWellFounded,
   WFLexPairOrdering
   DEF ExactDecisionTargetNeutralProducerEpisodeOrdering,
       ExactDecisionTargetNeutralProducerEpisodeCarrier

THEOREM ExactDecisionTargetNeutralPacketDependencyRankForSnapshotInCarrier ==
  \A snapshot:
    \A packet \in OverdueResponsivePackets:
      /\ AsyncStrongTypeInvariant
      /\ snapshot.candidateIdentities
           \subseteq ExactDecisionTargetNeutralCandidateOwnerIdentitySet
      /\ snapshot.candidateRoots
           \subseteq ExactDecisionTargetNeutralCausalRootSet
      /\ snapshot.schedulerCuts \in [Responsive -> Nat]
      /\ snapshot.serveIdentities
           \subseteq ExactDecisionTargetNeutralServeOwnerIdentitySet
      => ExactDecisionTargetNeutralPacketDependencyRankForSnapshot(
           snapshot, packet)
           \in HistoricalDiscoveryPacketDependencyCarrier
BY StrongTypeHasFiniteHistoricalDiscoveryRankOwners,
   ScheduledCandidateServiceRankInCarrier,
   HistoricalDiscoveryPacketServeOwnerRankInCarrier,
   HistoricalDiscoveryOwnedRankMinimumFacts,
   StrongTypeHasFiniteOlderNonOverdueShadows,
   IngressGateOwnerDebtsAreFiniteNaturals,
   BoundedTransportServiceRankIsNatural,
   ResetAwareIngressReachRankIsNatural,
   ReadyRunAuxRankInCarrier,
   Stage4CapacityRankInCarrier,
   FS_Subset, FS_CardinalityType, Isa
   DEF ExactDecisionTargetNeutralPacketDependencyRankForSnapshot,
       ExactDecisionTargetNeutralCandidateOccurrenceRankForSnapshot,
       ExactDecisionTargetNeutralServeOccurrenceRankForSnapshot,
       ExactDecisionTargetNeutralCandidateDebtRankForSnapshot,
       ExactDecisionTargetNeutralServeDebtRankForSnapshot,
       ExactDecisionTargetNeutralCandidateRanksForSnapshot,
       ExactDecisionTargetNeutralServeRanksForSnapshot,
       ExactDecisionTargetNeutralCandidateOwnersForSnapshot,
       ExactDecisionTargetNeutralCandidateRootPrecedesPhysicalCut,
       ExactDecisionTargetNeutralServeOwnersForSnapshot,
       HistoricalDiscoveryPacketDependencyCarrier,
       HistoricalDiscoveryCapacityTailCarrier,
       HistoricalDiscoveryTimeoutTailCarrier,
       HistoricalDiscoveryCompletionTailCarrier,
       HistoricalDiscoveryTransportTailCarrier,
       HistoricalDiscoveryResetTailCarrier,
       HistoricalDiscoveryReadyTailCarrier,
       HistoricalDiscoveryStage4TailCarrier,
       HistoricalDiscoveryCandidateServeTailCarrier,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       OwnedServiceRankCarrier

THEOREM ExactDecisionTargetNeutralActiveSnapshotConcreteRankIsInCarrier ==
  \A snapshot:
    \A clockValue \in Nat:
      /\ AsyncStrongTypeInvariant
      /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, clockValue)
      /\ asyncNow = clockValue
      => ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot(
           snapshot, clockValue)
           \in ExactDecisionTargetNeutralFixedClockCarrier
BY ExactDecisionTargetNeutralPacketDependencyRankForSnapshotInCarrier,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   HistoricalDiscoveryFixedClockRankShapeInCarrier,
   HistoricalDiscoveryIngressCounterRankInCarrier,
   FS_CardinalityType, Isa
   DEF ExactDecisionTargetNeutralSnapshotActive,
       ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot,
       ExactDecisionTargetNeutralConcreteBlockerStage,
       ExactDecisionTargetNeutralConcreteDependencyRankForSnapshot,
       ExactDecisionTargetNeutralSelectedOverduePacket,
       ExactDecisionTargetNeutralSelectedPacketDependencyRankForSnapshot,
       ExactDecisionTargetNeutralFixedClockCarrier,
       HistoricalDiscoveryLatentOwnerDebt,
       HistoricalDiscoveryDuePacketDebt,
       HistoricalDiscoveryDormantIoDebt,
       HistoricalDiscoveryNodeBlockerDebt,
       HistoricalDiscoveryActiveIoBlockerDebt,
       HistoricalDiscoveryBlockerStageCarrier

THEOREM ExactDecisionTargetNeutralConcreteRankForSnapshotInCarrier ==
  \A snapshot, mode, node, qc, archive, request, response, packet:
    \A clockValue \in Nat:
      ExactDecisionTargetNeutralFixedClockPending(
        snapshot, mode, node, qc, archive,
        request, response, packet, clockValue)
        => ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot(
             snapshot, clockValue)
             \in ExactDecisionTargetNeutralFixedClockCarrier
BY ExactDecisionTargetNeutralActiveSnapshotConcreteRankIsInCarrier, Isa
   DEF ExactDecisionTargetNeutralFixedClockPending,
       ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot,
       ExactDecisionTargetNeutralConcreteBlockerStage,
       ExactDecisionTargetNeutralConcreteDependencyRankForSnapshot,
       ExactDecisionTargetNeutralSelectedOverduePacket,
       ExactDecisionTargetNeutralSelectedPacketDependencyRankForSnapshot,
       ExactDecisionTargetNeutralFixedClockCarrier,
       HistoricalDiscoveryLatentOwnerDebt,
       HistoricalDiscoveryDuePacketDebt,
       HistoricalDiscoveryDormantIoDebt,
       HistoricalDiscoveryNodeBlockerDebt,
       HistoricalDiscoveryActiveIoBlockerDebt,
       HistoricalDiscoveryBlockerStageCarrier

THEOREM ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded ==
  IsWellFoundedOn(
    ExactDecisionTargetNeutralFixedClockOrdering,
    ExactDecisionTargetNeutralFixedClockCarrier)
BY HistoricalDiscoveryFixedClockBlockerOrderingIsWellFounded
   DEF ExactDecisionTargetNeutralFixedClockOrdering,
       ExactDecisionTargetNeutralFixedClockCarrier

(***************************************************************************
Action-local retained producer accounting.

At a fixed clock the due set cannot grow.  A first authenticated-source
observation consumes one precharged ingress episode; an exact retransmission
is a journal stutter and coalesces into the same lifecycle.  Candidate
children were already included through their frozen causal root, while
candidate markers and Serve tombstones monotonically retire exact identities.
No action receives credit merely for allocating or replacing an owner.
***************************************************************************)

THEOREM ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets ==
  \A clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ asyncNow = clockValue
    /\ asyncNow' = clockValue
    => HistoricalDiscoveryDuePacketsAt(clockValue)'
         \subseteq HistoricalDiscoveryDuePacketsAt(clockValue)
BY HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame,
   HistoricalDiscoveryBroadcastControlHelpersHaveFixedClockFrame,
   HistoricalDiscoveryRetransmissionHelpersHaveFixedClockFrame,
   HistoricalDiscoveryDirectRequestPublicationHasFixedClockFrame,
   HistoricalDiscoveryResponsePublicationHasFixedClockFrame,
   HistoricalDiscoveryByzantineCertifiedRequestHasFixedClockFrame,
   HistoricalDiscoverySingletonFaultInjectorsHaveFixedClockFrame,
   HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket,
   IsaT(1200)
   DEF HistoricalDiscoveryFixedClockPublicationFrame,
       HistoricalDiscoveryDuePacketsAt,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, AsyncAllVars

ExactDecisionTargetNeutralExactIngressRequest(packet) ==
  AsyncProducerIngressRequest(packet.item)

ExactDecisionTargetNeutralExactIngressRank(packet) ==
  AsyncProducerRemainingIngressRankFor(
    ExactDecisionTargetNeutralExactIngressRequest(packet))

THEOREM ExactDecisionTargetNeutralFrozenRequestUsesExactIngressRank ==
  \A snapshot, packet:
    /\ packet \in snapshot.packets
    /\ ExactDecisionTargetNeutralExactIngressRequest(packet)
         \in snapshot.producerRequests
    /\ snapshot.ingressEpisodes =
         {AsyncProducerEpisode(
            request, authenticatedSource, AsyncProducerIngressStage):
            request \in snapshot.producerRequests,
            authenticatedSource \in AsyncIngressSources}
    => AsyncProducerRemainingIngressEpisodesFor(
         ExactDecisionTargetNeutralExactIngressRequest(packet))
         =
       {episode
          \in ExactDecisionTargetNeutralIngressEpisodeRemaining(snapshot):
          episode.request =
            ExactDecisionTargetNeutralExactIngressRequest(packet)}
BY Isa
   DEF ExactDecisionTargetNeutralExactIngressRequest,
       ExactDecisionTargetNeutralIngressEpisodeRemaining,
       AsyncProducerRemainingIngressEpisodesFor,
       AsyncProducerConsumedIngressEpisodesFor,
       AsyncProducerIngressEpisodeUniverseFor,
       AsyncProducerIngressEpisodeSet,
       AsyncProducerEpisode, AsyncProducerIngressStage

THEOREM ExactDecisionTargetNeutralFirstDistinctIngressConsumesFrozenRank ==
  \A snapshot:
    /\ AsyncConfiguration
    /\ AsyncProducerJournalClosed
    /\ AsyncProducerProjectionStep
    /\ (AsyncProducerAdmittedIngressEpisodes
          \ asyncProducerConsumedEpisodes)
         \cap snapshot.ingressEpisodes # {}
    /\ IsFiniteSet(snapshot.ingressEpisodes)
    /\ ExactDecisionTargetNeutralSelectedCausalEpisodeRankForSnapshot(
         snapshot)
         \in ExactDecisionTargetNeutralComposedCausalEpisodeCarrier
    /\ ExactDecisionTargetNeutralSelectedCausalEpisodeRankForSnapshot(
         snapshot)'
         \in ExactDecisionTargetNeutralComposedCausalEpisodeCarrier
    => <<ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)',
          ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)>>
         \in ExactDecisionTargetNeutralProducerEpisodeOrdering
BY AsyncProducerFirstDistinctEpisodeStrictlyConsumesFiniteRank,
   FS_CardinalityType, Isa
   DEF ExactDecisionTargetNeutralProducerEpisodeRank,
       ExactDecisionTargetNeutralProducerEpisodeOrdering,
       ExactDecisionTargetNeutralProducerEpisodeCarrier,
       ExactDecisionTargetNeutralIngressEpisodeRank,
       ExactDecisionTargetNeutralIngressEpisodeRemaining,
       AsyncProducerProjectionStep,
       AsyncProducerFirstDistinctIngressEpisodeStep,
       LexPairOrdering, SetLessThan, OpToRel

THEOREM ExactDecisionTargetNeutralExactRetransmissionIsProducerStutter ==
  \A snapshot:
    /\ AsyncProducerJournalClosed
    /\ AsyncProducerProjectionStep
    /\ AsyncProducerAdmittedIngressEpisodes # {}
    /\ AsyncProducerAdmittedIngressEpisodes
         \subseteq asyncProducerConsumedEpisodes
    => ExactDecisionTargetNeutralIngressEpisodeRemaining(snapshot)'
         = ExactDecisionTargetNeutralIngressEpisodeRemaining(snapshot)
BY AsyncProducerExactRetransmissionIsJournalStutter, Isa
   DEF ExactDecisionTargetNeutralIngressEpisodeRemaining

(***************************************************************************
Atomic ingress is strict physical progress.

The individually fair Admit action removes the selected source head in the
same transition that it appends the lane occurrence and publishes any
leader-wire Ingress lifecycle.  Thus the existing bounded transport
coordinate strictly descends; no proof-only reservation bit is needed.
***************************************************************************)
THEOREM ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank ==
  \A snapshot:
    \A packet \in OverdueResponsivePackets,
       recipient \in Responsive,
       source \in AsyncIngressSources:
    /\ AsyncStrongTypeInvariant
    /\ snapshot.candidateIdentities
         \subseteq ExactDecisionTargetNeutralCandidateOwnerIdentitySet
    /\ snapshot.candidateRoots
         \subseteq ExactDecisionTargetNeutralCausalRootSet
    /\ snapshot.schedulerCuts \in [Responsive -> Nat]
    /\ snapshot.serveIdentities
         \subseteq ExactDecisionTargetNeutralServeOwnerIdentitySet
    /\ packet.item.envelope.recipient = recipient
    /\ packet.authenticatedSource = source
    /\ PostGstAdmitHiddenPacket(recipient, source)
    => ExactDecisionTargetNeutralPacketDependencyRankForSnapshot(
         snapshot, packet)'
         \in SetLessThan(
              ExactDecisionTargetNeutralPacketDependencyRankForSnapshot(
                snapshot, packet),
              HistoricalDiscoveryPacketDependencyOrdering,
              HistoricalDiscoveryPacketDependencyCarrier)
BY HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket,
   ExactDecisionTargetNeutralPacketDependencyRankForSnapshotInCarrier,
   IsaT(600)
   DEF ExactDecisionTargetNeutralPacketDependencyRankForSnapshot,
       ExactDecisionTargetNeutralCandidateOccurrenceRankForSnapshot,
       ExactDecisionTargetNeutralServeOccurrenceRankForSnapshot,
       ExactDecisionTargetNeutralCandidateOwnersForSnapshot,
       ExactDecisionTargetNeutralCandidateRootPrecedesPhysicalCut,
       ExactDecisionTargetNeutralServeOwnersForSnapshot,
       ExactDecisionTargetNeutralCandidateRanksForSnapshot,
       ExactDecisionTargetNeutralServeRanksForSnapshot,
       ExactDecisionTargetNeutralCandidateDebtRankForSnapshot,
       ExactDecisionTargetNeutralServeDebtRankForSnapshot,
       HistoricalDiscoveryPacketDependencyOrdering,
       HistoricalDiscoveryPacketDependencyCarrier,
       HistoricalDiscoveryCapacityTailOrdering,
       HistoricalDiscoveryTimeoutTailOrdering,
       HistoricalDiscoveryCompletionTailOrdering,
       HistoricalDiscoveryTransportTailOrdering,
       HistoricalDiscoveryResetTailOrdering,
       HistoricalDiscoveryReadyTailOrdering,
       HistoricalDiscoveryStage4TailOrdering,
       HistoricalDiscoveryCandidateServeTailOrdering,
       HistoricalDiscoveryOccurrenceDebtOrdering,
       PostGstAdmitHiddenPacket, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket, DropExactActiveLeaderWireRetry,
       BoundedTransportServiceRank, IngressSourceServiceRank,
       LexPairOrdering, SetLessThan, OpToRel

THEOREM ExactDecisionTargetNeutralMaterializedEpisodeIdentitiesDoNotResurrect ==
  \A snapshot:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncProducerJournalClosed
    /\ gst
    /\ snapshot.candidateIdentities
         \subseteq ExactDecisionTargetNeutralCandidateOwnerIdentitySet
    /\ snapshot.serveIdentities
         \subseteq ExactDecisionTargetNeutralServeOwnerIdentitySet
    /\ [AsyncNext]_AsyncAllVars
    => ExactDecisionTargetNeutralProducerEpisodeSet(snapshot)'
         \subseteq ExactDecisionTargetNeutralProducerEpisodeSet(snapshot)
BY AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   ExactDecisionSameGenerationCandidateServiceCannotReactivateAtGst,
   ExactDecisionTerminalCandidateDiscardCannotReactivateAtGst,
   AsyncNextProjectsMonotoneProducerJournal,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeRetiredIdentityCannotRequeueAtGst,
   FS_Product, FS_Union, FS_Subset,
   IsaT(900)
   DEF ExactDecisionTargetNeutralProducerEpisodeSet,
       ExactDecisionTargetNeutralIngressEpisodeRemaining,
       ExactDecisionTargetNeutralCandidateEpisodeRemaining,
       ExactDecisionTargetNeutralServeEpisodeRemaining,
       ExactDecisionTargetNeutralCandidateIdentityCoalesced,
       ExactDecisionTargetNeutralCandidateIdentityObsolete,
       ExactDecisionTargetNeutralServeIdentityRetired,
       ExactDecisionTargetNeutralCandidateOwnerIdentitySet,
       ExactDecisionTargetNeutralServeOwnerIdentitySet,
       ExactDecisionTargetNeutralLiveCandidateIdentitySet,
       ExactDecisionTargetNeutralLiveServeIdentitySet,
       ExactDecisionTargetNeutralCandidateOwnerIdentity,
       ExactDecisionTargetNeutralServeOwnerIdentity,
       AsyncProducerJournalMonotoneStep,
       AsyncCandidateAdmissionIdentity,
       AsyncCandidateAdmissionIdentityObsolete,
       AsyncCandidateAdmissionIdentityLifecycleCovered,
       AsyncCandidateAdmissionIdentityTerminallyCovered,
       AsyncCandidateAdmissionIdentitySet,
       AsyncCandidateTransientServiceIdentityMarked,
       AsyncCandidateTerminalIdentityTombstoned,
       AsyncScheduledCandidateAdmissionIdentities,
       AsyncScheduledCandidateServiceIdentities,
       AsyncServeLogicalIdentityRetiredOrSuperseded,
       AsyncServeJobQueued,
       ActiveScheduledCandidates, ActiveIoJobs,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncServeLifecycleTypeInvariant,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncAllVars

(***************************************************************************
Frozen past-cut producer accounting.

The next four theorems are the action-specialized bridge which the old
materialized-identity complement could not supply.  A fixed snapshot stores a
strict past cut.  Fresh roots and Serve tickets are allocated at the shared
high-watermark and cannot enter below that cut.  Servicing a Candidate below
the cut removes its exact remaining occurrence weight and replaces it only
with the strictly smaller closed successor batch.  The proofless
continuation/leader-wire handoff is ranked separately and may expose a
physical Candidate only while consuming that outer rank.
***************************************************************************)

THEOREM ExactDecisionTargetNeutralFrozenPastCutsRemainPast ==
  \A snapshot:
    /\ AsyncStrongTypeInvariant
    /\ snapshot.schedulerCuts \in [Responsive -> Nat]
    /\ \A node \in Responsive:
         snapshot.schedulerCuts[node]
           < AsyncNextCandidateLifecycleOrdinal(node)
    /\ [AsyncNext]_AsyncAllVars
    => \A node \in Responsive:
         snapshot.schedulerCuts[node]
           < AsyncNextCandidateLifecycleOrdinal(node)'
BY AsyncSharedSchedulerHighWatermarkIsMonotone, IsaT(600)
   DEF AsyncAllVars

THEOREM ExactDecisionTargetNeutralFrozenPhysicalCutsRemainPastOrCurrent ==
  \A snapshot:
    /\ AsyncStrongTypeInvariant
    /\ snapshot.physicalCuts \in [Responsive -> Nat]
    /\ \A node \in Responsive:
         snapshot.physicalCuts[node]
           <= AsyncNextIngressPhysicalOrdinal(node)
    /\ [AsyncNext]_AsyncAllVars
    => \A node \in Responsive:
         snapshot.physicalCuts[node]
           <= AsyncNextIngressPhysicalOrdinal(node)'
BY AsyncIngressPhysicalHighWatermarkIsMonotone, IsaT(600)
   DEF AsyncAllVars

THEOREM ExactDecisionTargetNeutralFrozenActiveLeaderWireCandidatesMatchPhysicalCut ==
  \A snapshot:
    \A node \in Responsive:
      ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot(
        snapshot, node)
        =
      AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
        node, snapshot.schedulerCuts[node] + 1,
        snapshot.physicalCuts[node])
BY Isa
   DEF ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot,
       ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates

THEOREM ExactDecisionTargetNeutralActionInertDormantHasZeroProoflessCharge ==
  \A snapshot:
    \A node \in Responsive,
       record \in asyncLeaderWireLifecycles:
    AsyncLeaderWireActionInertDormant(record)
      => /\ record
              \notin
                ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot(
                  snapshot, node)
         /\ ExactDecisionTargetNeutralLeaderWireRemainingStageForSnapshot(
              snapshot, node, record) = 0
BY CandidateProducerContinuationActionInertDormantHasZeroFrozenStage, Isa
   DEF ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot,
       ExactDecisionTargetNeutralLeaderWireRemainingStageForSnapshot,
       AsyncLeaderWireActionInertDormant,
       AsyncLeaderWireLifecycleActive,
       AsyncLeaderWireLifecycleDormant

THEOREM ExactDecisionTargetNeutralPostCutLeaderWireAdmissionCannotEnterFrozenPrefix ==
  \A snapshot:
    \A node \in Responsive,
       recipient \in ValidatorIds,
       source \in AsyncIngressSources:
    /\ snapshot.physicalCuts \in [Responsive -> Nat]
    /\ snapshot.physicalCuts[node]
         <= AsyncNextIngressPhysicalOrdinal(node)
    /\ recipient = node
    /\ AdmitHiddenPacket(recipient, source)
      => \A record \in asyncLeaderWireLifecycles':
           record.physicalAdmissionOrdinal >= snapshot.physicalCuts[node]
             => record
                  \notin
                    ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot(
                      snapshot, node)'
BY CandidateProducerContinuationPostCutAdmissionCannotEnterFrozenPrefix,
   Isa
   DEF ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot

THEOREM ExactDecisionTargetNeutralPostCutOrdinaryAdmissionCannotEnterFrozenPrefix ==
  \A snapshot:
    \A node \in Responsive,
       recipient \in ValidatorIds,
       source \in AsyncIngressSources:
    /\ snapshot.physicalCuts \in [Responsive -> Nat]
    /\ snapshot.physicalCuts[node]
         <= AsyncNextIngressPhysicalOrdinal(node)
    /\ recipient = node
    /\ AdmitHiddenPacket(recipient, source)
      => \A carrier \in
           asyncControlServiceState'.ordinaryIngressCarrierEvidence:
           carrier.physicalOrdinal >= snapshot.physicalCuts[node]
             => carrier
                  \notin
                    ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot(
                      snapshot, node)'
BY CandidateProducerContinuationPostCutOrdinaryAdmissionCannotEnterFrozenPrefix,
   Isa
   DEF ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot

THEOREM ExactDecisionTargetNeutralPostCutCausalRootCannotEnterFrozenPrefix ==
  \A snapshot:
    \A node \in Responsive, candidate \in AsyncCandidateSet:
    LET lifecycle ==
          AsyncCandidateLifecycleRecordFor(
            candidate.node, candidate.causalOrigin)
    IN /\ AsyncControlServiceStateTypeInvariant
       /\ snapshot.physicalCuts \in [Responsive -> Nat]
       /\ candidate.node = node
       /\ AsyncCandidateLifecycleRecorded(
            candidate.node, candidate.causalOrigin)
       /\ lifecycle.sourcePhysicalOrdinal
            >= snapshot.physicalCuts[node]
       => candidate
            \notin
              ExactDecisionTargetNeutralCausalCandidatesForSnapshot(
                snapshot, node)
BY Isa
   DEF ExactDecisionTargetNeutralCausalCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot,
       AsyncCandidateLifecycleRecorded,
       AsyncCandidateLifecycleRecordsFor,
       AsyncCandidateLifecycleRecordFor,
       AsyncControlServiceStateTypeInvariant

THEOREM ExactDecisionTargetNeutralPostCutContinuationCannotEnterFrozenPrefix ==
  \A snapshot:
    \A node \in Responsive,
       record \in
         AsyncCandidateProducerContinuationResolutionRecordsForNode(node):
    /\ snapshot.physicalCuts \in [Responsive -> Nat]
    /\ record.sourcePhysicalOrdinal >= snapshot.physicalCuts[node]
      => record
           \notin
             ExactDecisionTargetNeutralFrozenContinuationRecordsForSnapshot(
               snapshot, node)
BY Isa
   DEF ExactDecisionTargetNeutralFrozenContinuationRecordsForSnapshot

THEOREM ExactDecisionTargetNeutralPostCutServeCannotEnterFrozenPrefix ==
  \A snapshot:
    \A node \in Responsive,
       identity \in AsyncCausalEpisodeServeIngressIdentities(
                     node, snapshot.schedulerCuts[node]):
    /\ snapshot.physicalCuts \in [Responsive -> Nat]
    /\ AsyncServeIngressAdmissionOrdinal(node, identity)
         >= snapshot.physicalCuts[node]
      => identity
           \notin
             ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot(
               snapshot, node)
BY Isa
   DEF ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot

THEOREM ExactDecisionTargetNeutralFrozenActiveLeaderWireCandidatesCannotReplenish ==
  \A snapshot:
    \A node \in Responsive:
    /\ gst
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ snapshot.schedulerCuts \in [Responsive -> Nat]
    /\ snapshot.physicalCuts \in [Responsive -> Nat]
    /\ snapshot.schedulerCuts[node]
         < AsyncNextCandidateLifecycleOrdinal(node)
    /\ snapshot.physicalCuts[node]
         <= AsyncNextIngressPhysicalOrdinal(node)
    /\ [AsyncNext]_AsyncAllVars
      => (ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot(
             snapshot, node))'
           \subseteq
             ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot(
               snapshot, node)
BY ExactDecisionTargetNeutralFrozenActiveLeaderWireCandidatesMatchPhysicalCut,
   CandidateProducerContinuationFrozenLeaderWireChargeCannotAppearAtGst,
   IsaT(900)
   DEF AsyncAllVars

THEOREM ExactDecisionTargetNeutralFrozenOrdinaryIngressCandidatesCannotReplenish ==
  \A snapshot:
    \A node \in Responsive:
    /\ gst
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ snapshot.schedulerCuts \in [Responsive -> Nat]
    /\ snapshot.physicalCuts \in [Responsive -> Nat]
    /\ snapshot.schedulerCuts[node]
         < AsyncNextCandidateLifecycleOrdinal(node)
    /\ snapshot.physicalCuts[node]
         <= AsyncNextIngressPhysicalOrdinal(node)
    /\ [AsyncNext]_AsyncAllVars
      => (ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot(
             snapshot, node))'
           \subseteq
             ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot(
               snapshot, node)
BY ExactDecisionTargetNeutralPostCutOrdinaryAdmissionCannotEnterFrozenPrefix,
   AsyncFreshOrdinaryIngressCarrierAdmissionsAreSingularThisStep,
   LaterAcceptedOrdinaryCarrierCannotOvertakeFrozenCarrier,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   FS_Image, FS_Subset, IsaT(1200)
   DEF ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot,
       AsyncControlServiceSlotTransition,
       AsyncCandidateLifecycleStateAfterOrdinaryIngressAdmission,
       AsyncFreshOrdinaryIngressCarrierEvidenceForNodeIn,
       AsyncOrdinaryIngressCarrierEvidence,
       AsyncOrdinaryIngressCarrierStateAfterTransition,
       AsyncOrdinaryIngressCarrierEvidenceAfterPhysicalTransition,
       AsyncOrdinaryIngressCarrierAfterPhysicalTransition,
       AsyncAllVars

THEOREM ExactDecisionTargetNeutralDormantLocalReplayReplacementConsumesFrozenCausalCharge ==
  \A snapshot:
    \A node \in Responsive:
    /\ gst
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ snapshot.schedulerCuts \in [Responsive -> Nat]
    /\ snapshot.physicalCuts \in [Responsive -> Nat]
    /\ snapshot.schedulerCuts[node]
         < AsyncNextCandidateLifecycleOrdinal(node)
    /\ snapshot.physicalCuts[node]
         <= AsyncNextIngressPhysicalOrdinal(node)
    /\ [AsyncNext]_AsyncAllVars
      => ((ExactDecisionTargetNeutralFrozenDormantLocalReplayCandidatesForSnapshot(
              snapshot, node))'
            \ ExactDecisionTargetNeutralFrozenDormantLocalReplayCandidatesForSnapshot(
                snapshot, node))
           \subseteq
             ExactDecisionTargetNeutralCausalCandidatesForSnapshot(
               snapshot, node)
BY AsyncCandidateProducerContinuationGstExcludesResetReplay,
   AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck,
   AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   AsyncCandidateProducerSemanticHandoffRetirementRequiresAck,
   ExactDecisionTargetNeutralPostCutContinuationCannotEnterFrozenPrefix,
   FS_Subset, IsaT(1800)
   DEF ExactDecisionTargetNeutralFrozenDormantLocalReplayCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenContinuationRecordsForSnapshot,
       ExactDecisionTargetNeutralCausalCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot,
       AsyncCandidateProducerContinuationConcreteSuccessorOwned,
       AsyncCandidateProducerContinuationHandoffOwned,
       AsyncCandidateProducerContinuationHandoffRetired,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuations,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       AsyncAllVars

THEOREM ExactDecisionTargetNeutralExactLocalReplayReplacesFrozenCharge ==
  \A snapshot:
    \A node \in Responsive:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ snapshot.schedulerCuts \in [Responsive -> Nat]
    /\ snapshot.physicalCuts \in [Responsive -> Nat]
    /\ AsyncNext
    /\ AsyncControlServiceSlotTransition
    /\ AsyncCandidateProducerContinuationExactLocalReplayStep(node)
      => (ExactDecisionTargetNeutralProoflessCandidateOwnersForSnapshot(
             snapshot, node))'
           = ExactDecisionTargetNeutralProoflessCandidateOwnersForSnapshot(
               snapshot, node)
BY AsyncCandidateProducerContinuationExactLocalReplayRetainsReservation,
   AsyncCandidateProducerContinuationExactLocalReplayPublishesStoredCarrier,
   AsyncNextPreservesCandidateProducerContinuationScheduledExclusion,
   AsyncCandidateCausalSuccessorInheritsContinuationPhysicalOwnership,
   IsaT(1800)
   DEF ExactDecisionTargetNeutralProoflessCandidateOwnersForSnapshot,
       ExactDecisionTargetNeutralCausalCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot,
       ExactDecisionTargetNeutralFrozenDormantLocalReplayCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenContinuationRecordsForSnapshot,
       ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot,
       ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot,
       AsyncCandidateProducerContinuationConcreteSuccessorOwned,
       AsyncCandidateProducerContinuationHandoffOwned,
       AsyncCandidateProducerContinuationHandoffRetired,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuations,
       AsyncCausalEpisodeCandidates,
       AsyncControlServiceSlotTransition,
       CandidateScheduled, CandidateScheduledAfter

THEOREM ExactDecisionTargetNeutralDropPolicyRejectedIsFrozenPhysicalPrefixFrame ==
  \A snapshot:
    \A node \in Responsive,
       recipient \in ValidatorIds,
       source \in AsyncIngressSources:
    DropPolicyRejectedHiddenPacket(recipient, source)
      => /\ (ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot(
                snapshot, node))'
              = ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot(
                  snapshot, node)
         /\ (ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot(
                snapshot, node))'
              = ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot(
                  snapshot, node)
         /\ (ExactDecisionTargetNeutralLeaderWireStageTokensForSnapshot(
                snapshot, node))'
              = ExactDecisionTargetNeutralLeaderWireStageTokensForSnapshot(
                  snapshot, node)
BY CandidateProducerContinuationDropPolicyRejectedIsFrozenPhysicalPrefixFrame,
   Isa
   DEF ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot,
       ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot,
       ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot,
       ExactDecisionTargetNeutralLeaderWireStageTokensForSnapshot,
       ExactDecisionTargetNeutralLeaderWireRemainingStageForSnapshot,
       DropPolicyRejectedHiddenPacket

THEOREM ExactDecisionTargetNeutralFrozenPastCutOriginsCannotReplenish ==
  \A snapshot:
    \A node \in Responsive:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ snapshot.schedulerCuts \in [Responsive -> Nat]
    /\ snapshot.schedulerCuts[node]
         < AsyncNextCandidateLifecycleOrdinal(node)
    /\ snapshot.physicalCuts \in [Responsive -> Nat]
    /\ snapshot.physicalCuts[node]
         <= AsyncNextIngressPhysicalOrdinal(node)
    /\ [AsyncNext]_AsyncAllVars
    => (ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot(
           snapshot, node))'
         \subseteq
           ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot(
             snapshot, node)
BY AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   IsaT(1200)
   DEF ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot,
       AsyncAllVars

THEOREM ExactDecisionTargetNeutralFrozenPastCutServeCannotReplenish ==
  \A snapshot:
    \A node \in Responsive:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ snapshot.schedulerCuts \in [Responsive -> Nat]
    /\ snapshot.schedulerCuts[node]
         < AsyncNextCandidateLifecycleOrdinal(node)
    /\ snapshot.physicalCuts \in [Responsive -> Nat]
    /\ snapshot.physicalCuts[node]
         <= AsyncNextIngressPhysicalOrdinal(node)
    /\ [AsyncNext]_AsyncAllVars
    => (ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot(
           snapshot, node))'
         \subseteq
           ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot(
             snapshot, node)
BY AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal,
   AsyncServeIngressAdmissionConsumesSharedSchedulerOrdinal,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   IsaT(1200)
   DEF ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot,
       AsyncCausalEpisodeServeIngressIdentities,
       AsyncFreshServeIngressAdmissionsForNodeThisStep,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionSchedulerOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncAllVars

THEOREM ExactDecisionTargetNeutralFrozenPastCutCandidateServiceConsumesExactOccurrence ==
  \A snapshot:
    \A node \in Responsive, serviced \in AsyncCandidateSet:
    LET cutoffOrdinal == snapshot.schedulerCuts[node]
    IN /\ gst
       /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateServiceLifecycleInvariant
       /\ snapshot.schedulerCuts \in [Responsive -> Nat]
       /\ snapshot.physicalCuts \in [Responsive -> Nat]
       /\ cutoffOrdinal < AsyncNextCandidateLifecycleOrdinal(node)
       /\ snapshot.physicalCuts[node]
            <= AsyncNextIngressPhysicalOrdinal(node)
       /\ serviced \in
            ExactDecisionTargetNeutralCausalCandidatesForSnapshot(
              snapshot, node)
       /\ [AsyncNext]_AsyncAllVars
       /\ ~CandidateScheduled(serviced)'
       => ExactDecisionTargetNeutralExactCandidateOccurrenceBudgetForSnapshot(
            snapshot, node)'
            < ExactDecisionTargetNeutralExactCandidateOccurrenceBudgetForSnapshot(
                snapshot, node)
BY ExactDecisionTargetNeutralFrozenPastCutOriginsCannotReplenish,
   AsyncCommandExactSuccessorBatchStrictlyConsumesOccurrenceBudget,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   FS_CardinalityType, FS_Subset, IsaT(1800)
   DEF ExactDecisionTargetNeutralExactCandidateOccurrenceBudgetForSnapshot,
       ExactDecisionTargetNeutralExactCandidateOccurrenceTokensForSnapshot,
       ExactDecisionTargetNeutralCausalCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot,
       AsyncCausalEpisodeCandidates,
       CandidateScheduled, AsyncAllVars

THEOREM ExactDecisionTargetNeutralExactOccurrenceStructuralStepIsDescentOrFrame ==
  \A snapshot:
    \A node \in Responsive:
    LET rank ==
          ExactDecisionTargetNeutralCausalEpisodeRankForSnapshot(
            snapshot, node)
        prooflessRank ==
          ExactDecisionTargetNeutralProoflessProducerRankForSnapshot(
            snapshot, node)
    IN /\ gst
       /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateServiceLifecycleInvariant
       /\ snapshot.schedulerCuts \in [Responsive -> Nat]
       /\ snapshot.physicalCuts \in [Responsive -> Nat]
       /\ snapshot.schedulerCuts[node]
            < AsyncNextCandidateLifecycleOrdinal(node)
       /\ snapshot.physicalCuts[node]
            <= AsyncNextIngressPhysicalOrdinal(node)
       /\ [AsyncNext]_AsyncAllVars
       /\ ExactDecisionTargetNeutralProoflessProducerRankForSnapshot(
            snapshot, node)' = prooflessRank
       => \/ <<ExactDecisionTargetNeutralCausalEpisodeRankForSnapshot(
                   snapshot, node)', rank>>
                  \in AsyncCausalEpisodeStructuralRankOrdering
          \/ ExactDecisionTargetNeutralCausalEpisodeRankForSnapshot(
               snapshot, node)' = rank
BY ExactDecisionTargetNeutralFrozenPastCutOriginsCannotReplenish,
   ExactDecisionTargetNeutralFrozenPastCutServeCannotReplenish,
   ExactDecisionTargetNeutralFrozenPastCutCandidateServiceConsumesExactOccurrence,
   AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   AsyncServeIngressTargetOnlyCannotOvertakeOlderRuntimeLifecycle,
   AsyncServeIngressTargetOnlyCannotOvertakeOlderLocalLifecycle,
   ExactTicketTurnDecreasesDrainableIngressTurnReach,
   ExhaustedIngressStepDecreasesDrainableIngressTurnReach,
   LocalStepDecreasesDrainableIngressTurnReach,
   SerializedLocalPredecessorDecreasesDrainableIngressTurnReach,
   RuntimeStepDecreasesDrainableIngressTurnReach,
   OlderRuntimeInterleaveDecreasesDrainableIngressTurnReach,
   FS_CardinalityType, FS_Subset, IsaT(3000)
   DEF ExactDecisionTargetNeutralCausalEpisodeRankForSnapshot,
       ExactDecisionTargetNeutralExactCandidateOccurrenceBudgetForSnapshot,
       ExactDecisionTargetNeutralExactCandidateOccurrenceTokensForSnapshot,
       ExactDecisionTargetNeutralCausalCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot,
       AsyncCausalEpisodeCandidates,
       ExactDecisionTargetNeutralServeWorkBudgetForSnapshot,
       ExactDecisionTargetNeutralServeWorkTokensForSnapshot,
       ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot,
       ExactDecisionTargetNeutralServeReachDebtForSnapshot,
       AsyncCausalEpisodeServeWorkTokens,
       AsyncCausalEpisodeServeOccurrenceTokens,
       AsyncCausalEpisodeServeIngressPrefixTokens,
       AsyncCausalEpisodeServeIoPredecessorTokens,
       AsyncCausalEpisodeServeReachDebt,
       AsyncCausalEpisodeStructuralRankOrdering,
       AsyncCausalEpisodeServeRankOrdering,
       AsyncServeIngressTargetOnlyTurn,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       ServiceIoWorkerWork, PopSelectedIngress,
       DrainFairIngressSelected,
       LocalAdmissionStep, IngressDrainStep,
       SerializedLocalPrecedesServeIngressStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncNext, AsyncAllVars,
       LexPairOrdering, OpToRel

THEOREM ExactDecisionTargetNeutralProoflessProducerStepIsDescentOrFrame ==
  \A snapshot:
    \A node \in Responsive:
    LET rank ==
          ExactDecisionTargetNeutralProoflessProducerRankForSnapshot(
            snapshot, node)
    IN /\ gst
       /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateServiceLifecycleInvariant
       /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
       /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
       /\ snapshot.schedulerCuts \in [Responsive -> Nat]
       /\ snapshot.physicalCuts \in [Responsive -> Nat]
       /\ snapshot.schedulerCuts[node]
            < AsyncNextCandidateLifecycleOrdinal(node)
       /\ snapshot.physicalCuts[node]
            <= AsyncNextIngressPhysicalOrdinal(node)
       /\ [AsyncNext]_AsyncAllVars
       => \/ <<ExactDecisionTargetNeutralProoflessProducerRankForSnapshot(
                   snapshot, node)', rank>>
                  \in ExactDecisionTargetNeutralProoflessProducerOrdering
          \/ ExactDecisionTargetNeutralProoflessProducerRankForSnapshot(
               snapshot, node)' = rank
BY CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight,
   CandidateProducerContinuationFrozenLeaderWireChargeCannotAppearAtGst,
   CandidateProducerContinuationActionInertDormantHasZeroFrozenStage,
   CandidateProducerContinuationPostCutAdmissionCannotEnterFrozenPrefix,
   CandidateProducerContinuationPostCutOrdinaryAdmissionCannotEnterFrozenPrefix,
   CandidateProducerContinuationDropPolicyRejectedIsFrozenPhysicalPrefixFrame,
   ExactDecisionTargetNeutralFrozenActiveLeaderWireCandidatesCannotReplenish,
   ExactDecisionTargetNeutralFrozenOrdinaryIngressCandidatesCannotReplenish,
   ExactDecisionTargetNeutralDormantLocalReplayReplacementConsumesFrozenCausalCharge,
   ExactDecisionTargetNeutralActionInertDormantHasZeroProoflessCharge,
   ExactDecisionTargetNeutralPostCutLeaderWireAdmissionCannotEnterFrozenPrefix,
   ExactDecisionTargetNeutralPostCutOrdinaryAdmissionCannotEnterFrozenPrefix,
   ExactDecisionTargetNeutralPostCutCausalRootCannotEnterFrozenPrefix,
   ExactDecisionTargetNeutralPostCutContinuationCannotEnterFrozenPrefix,
   ExactDecisionTargetNeutralPostCutServeCannotEnterFrozenPrefix,
   ExactDecisionTargetNeutralDropPolicyRejectedIsFrozenPhysicalPrefixFrame,
   CandidateProducerContinuationPreCutIngressToRuntimeConsumesBarrierStage,
   CandidateProducerContinuationPreCutOrdinaryIngressConsumesBarrierStage,
   ExactDecisionTargetNeutralExactLocalReplayReplacesFrozenCharge,
   AsyncCandidateCausalSuccessorInheritsContinuationPhysicalOwnership,
   ExternalContinuationPersistsOrDescendsOrReplayExits,
   LocalContinuationPersistsOrDescendsOrReplayExits,
   AsyncCandidateProducerContinuationGstExcludesResetReplay,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck,
   AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   AsyncCandidateProducerSemanticHandoffRetirementRequiresAck,
   RetireLeaderWireLifecycleRetainsTerminalTombstone,
   FS_CardinalityType, FS_Subset, IsaT(3000)
   DEF ExactDecisionTargetNeutralProoflessProducerRankForSnapshot,
       ExactDecisionTargetNeutralProoflessProducerOrdering,
       ExactDecisionTargetNeutralProoflessProducerCarrier,
       ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot,
       ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot,
       ExactDecisionTargetNeutralLeaderWireRemainingStageForSnapshot,
       ExactDecisionTargetNeutralLeaderWireStageTokensForSnapshot,
       ExactDecisionTargetNeutralLeaderWireStageBudgetForSnapshot,
       ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot,
       ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenContinuationRecordsForSnapshot,
       ExactDecisionTargetNeutralFrozenDormantLocalReplayCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenContinuationStatusTokensForSnapshot,
       ExactDecisionTargetNeutralCausalCandidatesForSnapshot,
       ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot,
       ExactDecisionTargetNeutralProoflessCandidateOwnersForSnapshot,
       ExactDecisionTargetNeutralProoflessCandidateTokensForSnapshot,
       ExactDecisionTargetNeutralProoflessContinuationTokensForSnapshot,
       ExactDecisionTargetNeutralProoflessContinuationBudgetForSnapshot,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCandidateProducerContinuationStatusRank,
       AsyncNext, AsyncAllVars,
       LexPairOrdering, SetLessThan, OpToRel

THEOREM ExactDecisionTargetNeutralComposedCausalEpisodeStepIsDescentOrFrame ==
  \A snapshot:
    \A node \in Responsive:
    LET rank ==
          ExactDecisionTargetNeutralComposedCausalEpisodeRankForSnapshot(
            snapshot, node)
    IN /\ gst
       /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateServiceLifecycleInvariant
       /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
       /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
       /\ snapshot.schedulerCuts \in [Responsive -> Nat]
       /\ snapshot.physicalCuts \in [Responsive -> Nat]
       /\ snapshot.schedulerCuts[node]
            < AsyncNextCandidateLifecycleOrdinal(node)
       /\ snapshot.physicalCuts[node]
            <= AsyncNextIngressPhysicalOrdinal(node)
       /\ [AsyncNext]_AsyncAllVars
       => \/ <<ExactDecisionTargetNeutralComposedCausalEpisodeRankForSnapshot(
                   snapshot, node)', rank>>
                  \in
                    ExactDecisionTargetNeutralComposedCausalEpisodeOrdering
          \/ ExactDecisionTargetNeutralComposedCausalEpisodeRankForSnapshot(
               snapshot, node)' = rank
BY ExactDecisionTargetNeutralProoflessProducerStepIsDescentOrFrame,
   ExactDecisionTargetNeutralExactOccurrenceStructuralStepIsDescentOrFrame,
   IsaT(900)
   DEF ExactDecisionTargetNeutralComposedCausalEpisodeRankForSnapshot,
       ExactDecisionTargetNeutralComposedCausalEpisodeOrdering,
       ExactDecisionTargetNeutralComposedCausalEpisodeCarrier,
       LexPairOrdering

\* Residual-free structural interface for other fixed-clock liveness
\* corridors.  The caller supplies only a rigid active snapshot and proves
\* that the step remains in that fixed-clock cell; no exact-Decision residual
\* or goal is invented to obtain the producer-rank result.
THEOREM ExactDecisionTargetNeutralSnapshotPredecessorsDoNotReplenishAtFixedClock ==
  \A snapshot:
    \A clockValue \in Nat:
      /\ gst
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ AsyncProducerJournalClosed
      /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, clockValue)
      /\ asyncNow = clockValue
      /\ [AsyncNext]_AsyncAllVars
      /\ asyncNow' = clockValue
      => /\ HistoricalDiscoveryDuePacketsAt(clockValue)'
               \subseteq snapshot.packets
         /\ ExactDecisionTargetNeutralFixedPredecessorSetForSnapshot(
              snapshot, clockValue)'
               \subseteq snapshot.predecessors
         /\ ExactDecisionTargetNeutralProducerEpisodeSet(snapshot)'
               \subseteq
                 ExactDecisionTargetNeutralProducerEpisodeSet(snapshot)
BY ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets,
   ExactDecisionTargetNeutralMaterializedEpisodeIdentitiesDoNotResurrect,
   ExactDecisionTargetNeutralFrozenSnapshotCarriersArePrimeInvariant,
   AsyncServeIngressTicketExcludesLaterLocalWork,
   AsyncOrdinaryIngressTicketExcludesLaterLocalWork,
   AsyncSelectedOrdinaryPhysicalCarrierDefinesIngressScheduler,
   AsyncServeIngressReservationPrecedesSameStepCandidateAllocation,
   AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal,
   AsyncFreshServeIngressSchedulerOrdinalInjectsAgainstPriorOwners,
   AsyncCandidateProducerContinuationLaterOrdinalCannotOwnRunnerTurn,
   AsyncCandidateProducerContinuationPostCutIngressCannotBlockRunnerTurn,
   AsyncCandidateProducerContinuationOnlyPreCutIngressCanBlockRunnerTurn,
   AsyncCandidateProducerContinuationPostCutSourceCannotPrecede,
   AsyncCandidateProducerContinuationFrozenOwnerPrecedesPostCutReplay,
   AsyncStrongTypeProjectsControlServiceStateType,
   AsyncCandidateProducerContinuationRunnerSelectionIsTwoStageLogicalMinimum,
   AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain,
   AsyncCandidateDiscardRetiresLogicalLifecycle,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   ExactDecisionTerminalCandidateDiscardCannotReactivateAtGst,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   AsyncServeRetiredIdentityCannotRequeueAtGst,
   AsyncServeIngressDuplicateDoesNotAllocateOrdinal,
   SameHeightRestartPreservesServeHighWatermarks,
   AsyncCandidateTransientMarkerCoalescesFreshCandidate,
   AsyncCandidateTerminalTombstoneCoalescesFreshCandidate,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   ExactDecisionSameGenerationCandidateServiceCannotReactivateAtGst,
   AsyncBracketNextPreservesStrongTypeInvariant,
   FS_Subset, IsaT(1200)
   DEF ExactDecisionTargetNeutralSnapshotActive,
       ExactDecisionTargetNeutralFixedPredecessorSetForSnapshot,
       ExactDecisionTargetNeutralSchedulerCutTokens,
       ExactDecisionTargetNeutralSchedulerCutToken,
       ExactDecisionTargetNeutralProducerEpisodeSet,
       ExactDecisionTargetNeutralLiveCandidateIdentitySet,
       ExactDecisionTargetNeutralLiveServeIdentitySet,
       ExactDecisionTargetNeutralCandidateOwnerIdentitySet,
       ExactDecisionTargetNeutralServeOwnerIdentitySet,
       ExactDecisionTargetNeutralCandidateOwnerIdentity,
       ExactDecisionTargetNeutralServeOwnerIdentity,
       ExactDecisionTargetNeutralCandidateIdentityCoalesced,
       ExactDecisionTargetNeutralCandidateIdentityObsolete,
       ExactDecisionTargetNeutralServeIdentityRetired,
       HistoricalDiscoveryDuePacketsAt, AsyncAllVars

THEOREM ExactDecisionTargetNeutralSnapshotRemainsActiveAtFixedClock ==
  \A snapshot:
    \A clockValue \in Nat:
      /\ gst
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ AsyncProducerJournalClosed
      /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, clockValue)
      /\ asyncNow = clockValue
      /\ [AsyncNext]_AsyncAllVars
      /\ asyncNow' = clockValue
      => (ExactDecisionTargetNeutralSnapshotActive(
            snapshot, clockValue))'
BY ExactDecisionTargetNeutralSnapshotPredecessorsDoNotReplenishAtFixedClock,
   ExactDecisionTargetNeutralMaterializedEpisodeIdentitiesDoNotResurrect,
   ExactDecisionAsyncNextPreservesCandidateTombstones,
   ExactDecisionTargetNeutralFrozenSnapshotCarriersArePrimeInvariant,
   ExactDecisionTargetNeutralFrozenPastCutsRemainPast,
   ExactDecisionTargetNeutralFrozenPhysicalCutsRemainPastOrCurrent,
   ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets,
   FS_CardinalityType, FS_Subset, IsaT(1200)
   DEF ExactDecisionTargetNeutralSnapshotActive,
       ExactDecisionTargetNeutralFixedPredecessorSetForSnapshot,
       ExactDecisionTargetNeutralRetainedCausalRootsForSnapshot,
       ExactDecisionTargetNeutralFrozenCandidateLifecycleCovered,
       ExactDecisionTargetNeutralFrozenServeLifecycleCovered,
       ExactDecisionTargetNeutralCandidateEpisodeRemaining,
       ExactDecisionTargetNeutralServeEpisodeRemaining,
       ExactDecisionTargetNeutralCandidateIdentityCoalesced,
       ExactDecisionTargetNeutralCandidateIdentityObsolete,
       ExactDecisionTargetNeutralServeIdentityRetired,
       ExactDecisionTargetNeutralProducerEpisodeSet,
       AsyncAllVars

THEOREM ExactDecisionTargetNeutralSnapshotProducerEpisodeStepIsDescentOrFrame ==
  \A snapshot:
    \A clockValue \in Nat:
      LET budget == ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)
      IN /\ gst
         /\ AsyncStrongTypeInvariant
         /\ AsyncProgressOwnershipInvariant
         /\ AsyncCandidateServiceLifecycleInvariant
         /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
         /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
         /\ AsyncProducerJournalClosed
         /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, clockValue)
         /\ asyncNow = clockValue
         /\ [AsyncNext]_AsyncAllVars
         /\ asyncNow' = clockValue
         => /\ (ExactDecisionTargetNeutralSnapshotActive(
                   snapshot, clockValue))'
            /\ \/ <<ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)',
                     budget>>
                    \in ExactDecisionTargetNeutralProducerEpisodeOrdering
               \/ ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)'
                    = budget
BY HistoricalDiscoveryRetainedPacketMinimumStepCases,
   ExactDecisionTargetNeutralSnapshotRemainsActiveAtFixedClock,
   ExactDecisionTargetNeutralEpisodeRankIsInCarrier,
   ExactDecisionTargetNeutralFirstDistinctIngressConsumesFrozenRank,
   ExactDecisionTargetNeutralExactRetransmissionIsProducerStutter,
   ExactDecisionTargetNeutralComposedCausalEpisodeStepIsDescentOrFrame,
   ExactDecisionTargetNeutralFrozenPastCutsRemainPast,
   ExactDecisionTargetNeutralFrozenPhysicalCutsRemainPastOrCurrent,
   AsyncNextProjectsMonotoneProducerJournal,
   FS_CardinalityType, FS_Subset, IsaT(2400)
   DEF ExactDecisionTargetNeutralSnapshotActive,
       ExactDecisionTargetNeutralProducerEpisodeRank,
       ExactDecisionTargetNeutralProducerEpisodeOrdering,
       ExactDecisionTargetNeutralProducerEpisodeCarrier,
       ExactDecisionTargetNeutralIngressEpisodeRank,
       ExactDecisionTargetNeutralIngressEpisodeRemaining,
       ExactDecisionTargetNeutralSelectedCausalEpisodeRankForSnapshot,
       ExactDecisionTargetNeutralComposedCausalEpisodeRankForSnapshot,
       AsyncProducerJournalMonotoneStep,
       HistoricalDiscoveryFixedClockLexStep,
       LexPairOrdering, SetLessThan, OpToRel, AsyncAllVars

THEOREM ExactDecisionTargetNeutralSnapshotProducerEpisodeDoesNotReplenish ==
  \A snapshot:
    \A clockValue \in Nat:
      LET budget == ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)
      IN /\ gst
         /\ AsyncStrongTypeInvariant
         /\ AsyncProgressOwnershipInvariant
         /\ AsyncCandidateServiceLifecycleInvariant
         /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
         /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
         /\ AsyncProducerJournalClosed
         /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, clockValue)
         /\ asyncNow = clockValue
         /\ [AsyncNext]_AsyncAllVars
         /\ asyncNow' = clockValue
         => /\ (ExactDecisionTargetNeutralSnapshotActive(
                   snapshot, clockValue))'
            /\ ExactDecisionTargetNeutralProducerEpisodeSet(snapshot)'
                  \subseteq
                    ExactDecisionTargetNeutralProducerEpisodeSet(snapshot)
            /\ ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)'
                 \in
                   {budget}
                     \cup
                   SetLessThan(
                     budget,
                     ExactDecisionTargetNeutralProducerEpisodeOrdering,
                     ExactDecisionTargetNeutralProducerEpisodeCarrier)
BY ExactDecisionTargetNeutralSnapshotProducerEpisodeStepIsDescentOrFrame,
   ExactDecisionTargetNeutralSnapshotPredecessorsDoNotReplenishAtFixedClock,
   ExactDecisionTargetNeutralSnapshotRemainsActiveAtFixedClock,
   ExactDecisionTargetNeutralEpisodeRankIsInCarrier,
   ExactDecisionTargetNeutralMaterializedEpisodeIdentitiesDoNotResurrect,
   ExactDecisionTargetNeutralFrozenPastCutsRemainPast,
   ExactDecisionTargetNeutralFrozenPhysicalCutsRemainPastOrCurrent,
   AsyncNextProjectsMonotoneProducerJournal,
   FS_Subset, IsaT(1200)
   DEF ExactDecisionTargetNeutralSnapshotActive,
       ExactDecisionTargetNeutralProducerEpisodeRank,
       ExactDecisionTargetNeutralProducerEpisodeSet,
       ExactDecisionTargetNeutralProducerEpisodeOrdering,
       ExactDecisionTargetNeutralProducerEpisodeCarrier,
       SetLessThan, AsyncAllVars

THEOREM ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor ==
  \A snapshot, mode, node, qc, archive, request, response, packet:
    \A clockValue \in Nat:
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ ExactDecisionTargetNeutralFixedClockPending(
           snapshot, mode, node, qc, archive,
           request, response, packet, clockValue)
      /\ [AsyncNext]_AsyncAllVars
      /\ ExactDecisionTargetNeutralFixedClockPending(
           snapshot, mode, node, qc, archive,
           request, response, packet, clockValue)'
      => /\ HistoricalDiscoveryDuePacketsAt(clockValue)'
              \subseteq snapshot.packets
         /\ ExactDecisionTargetNeutralFixedPredecessorSetForSnapshot(
              snapshot, clockValue)'
              \subseteq snapshot.predecessors
         /\ ExactDecisionTargetNeutralProducerEpisodeSet(snapshot)'
              \subseteq
                ExactDecisionTargetNeutralProducerEpisodeSet(snapshot)
BY ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets,
   ExactDecisionTargetNeutralMaterializedEpisodeIdentitiesDoNotResurrect,
   ExactDecisionTargetNeutralFrozenSnapshotCarriersArePrimeInvariant,
   AsyncServeIngressTicketExcludesLaterLocalWork,
   AsyncOrdinaryIngressTicketExcludesLaterLocalWork,
   AsyncSelectedOrdinaryPhysicalCarrierDefinesIngressScheduler,
   AsyncServeIngressReservationPrecedesSameStepCandidateAllocation,
   AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal,
   AsyncFreshServeIngressSchedulerOrdinalInjectsAgainstPriorOwners,
   AsyncCandidateProducerContinuationLaterOrdinalCannotOwnRunnerTurn,
   AsyncCandidateProducerContinuationPostCutIngressCannotBlockRunnerTurn,
   AsyncCandidateProducerContinuationOnlyPreCutIngressCanBlockRunnerTurn,
   AsyncCandidateProducerContinuationPostCutSourceCannotPrecede,
   AsyncCandidateProducerContinuationFrozenOwnerPrecedesPostCutReplay,
   AsyncStrongTypeProjectsControlServiceStateType,
   AsyncCandidateProducerContinuationRunnerSelectionIsTwoStageLogicalMinimum,
   AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain,
   AsyncCandidateDiscardRetiresLogicalLifecycle,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   ExactDecisionTerminalCandidateDiscardCannotReactivateAtGst,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   AsyncServeRetiredIdentityCannotRequeueAtGst,
   AsyncServeIngressTicketExcludesLaterLocalWork,
   AsyncServeIngressDuplicateDoesNotAllocateOrdinal,
   SameHeightRestartPreservesServeHighWatermarks,
   AsyncCandidateTransientMarkerCoalescesFreshCandidate,
   AsyncCandidateTerminalTombstoneCoalescesFreshCandidate,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   ExactDecisionSameGenerationCandidateServiceCannotReactivateAtGst,
   AsyncBracketNextPreservesStrongTypeInvariant,
   IsaT(600)
   DEF ExactDecisionTargetNeutralFixedClockPending,
       ExactDecisionTargetNeutralSnapshotActive,
       ExactDecisionTargetNeutralFixedPredecessorSetForSnapshot,
       ExactDecisionTargetNeutralSchedulerCutTokens,
       ExactDecisionTargetNeutralSchedulerCutToken,
       ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot,
       ExactDecisionTargetNeutralProducerEpisodeSet,
       ExactDecisionTargetNeutralLiveCandidateIdentitySet,
       ExactDecisionTargetNeutralLiveServeIdentitySet,
       ExactDecisionTargetNeutralCandidateOwnerIdentitySet,
       ExactDecisionTargetNeutralServeOwnerIdentitySet,
       ExactDecisionTargetNeutralCandidateOwnerIdentity,
       ExactDecisionTargetNeutralServeOwnerIdentity,
       ExactDecisionTargetNeutralCandidateIdentityCoalesced,
       ExactDecisionTargetNeutralCandidateIdentityObsolete,
       ExactDecisionTargetNeutralServeIdentityRetired,
       HistoricalDiscoveryDuePacketsAt,
       AsyncAllVars

\* `asyncNextServeAdmissionOrdinal` is a cumulative high-watermark and may
\* advance repeatedly for one family at higher views.  Its configured live
\* family bound is therefore not used to bound such advances.  Post-GST
\* allocation occurs only inside the physical hidden-ingress admission that
\* removes one frozen due packet.  Hence a Serve high-watermark advance is
\* already strict fixed-clock rank descent and is outside the producer
\* non-descent episode.
THEOREM ExactDecisionTargetNeutralServeOrdinalAdvanceLowersFrozenPacketRank ==
  \A snapshot, mode, node, qc, archive, request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier,
       budget \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ DecisionFrontierUniquenessInvariant
      /\ DecisionTimeoutFrontierInvariant
      /\ ResponsiveRecoveryValidationClearedInvariant
      /\ FinalProgressWitnessClosureInvariant
      /\ ExactDecisionFanoutRetentionInvariant
      /\ ExactDecisionRequestAuthorityIsolationInvariant
      /\ ExactDecisionTargetNeutralProducerEpisodeAtBudget(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank, budget)
      /\ [AsyncNext]_AsyncAllVars
      /\ \E recipient \in Responsive:
           asyncNextServeAdmissionOrdinal[recipient]'
             > asyncNextServeAdmissionOrdinal[recipient]
      => /\ \E recipient \in Responsive,
               source \in AsyncIngressSources:
              AdmitIngressPacket(recipient, source)
         /\ (ExactDecisionTargetNeutralFixedClockStrictRankGoal(
               snapshot, mode, node, qc, archive, request, response,
               packet, clockValue, sourceRank))'
BY HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket,
   HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock,
   HistoricalDiscoveryFixedClockLexStepStrictlyDescends,
   ExactDecisionRequestHeadGateResidualStepIsSafe,
   ExactDecisionResponseHeadGateResidualStepIsSafe,
   AsyncBracketStepRetainsExactDecisionRecord,
   AsyncBracketStepLeavesContext,
   AsyncNextPreservesExactDecisionFanoutRetention,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   AsyncBracketNextPreservesFinalProgressWitnessClosure,
   ExactDecisionTargetNeutralConcreteRankForSnapshotInCarrier,
   IsaT(1200)
   DEF ExactDecisionTargetNeutralProducerEpisodeAtBudget,
       ExactDecisionTargetNeutralFixedClockStrictRankGoal,
       ExactDecisionTargetNeutralFixedClockBlockedAtRank,
       ExactDecisionTargetNeutralFixedClockPending,
       ExactDecisionTargetNeutralFixedClockExit,
       ExactDecisionTargetNeutralResidual,
       ExactDecisionTargetNeutralGoal,
       ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot,
       ExactDecisionTargetNeutralConcreteBlockerStage,
       ExactDecisionTargetNeutralConcreteDependencyRankForSnapshot,
       HistoricalDiscoveryFixedClockLexStep,
       HistoricalDiscoveryFixedClockRank,
       HistoricalDiscoveryDuePacketDebt,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncNetworkStep, PostGstAdmitHiddenPacket,
       AdmitIngressPacket, AdmitHiddenPacket,
       AsyncAllVars

\* At a non-strict fixed-clock step, the due-packet prefix either descends or
\* keeps the same selected recipient.  In the latter case a first distinct
\* ingress episode lowers the outer journal coordinate; otherwise the
\* proofless handoff and exact occurrence/Serve ranks are descent-or-frame.
THEOREM ExactDecisionTargetNeutralProducerEpisodeStepIsDescentOrFrame ==
  \A snapshot, mode, node, qc, archive, request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier,
       budget \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
      /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
      /\ AsyncProducerJournalClosed
      /\ ExactDecisionTargetNeutralProducerEpisodeAtBudget(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank, budget)
      /\ [AsyncNext]_AsyncAllVars
      /\ ~(ExactDecisionTargetNeutralFixedClockStrictRankGoal(
             snapshot, mode, node, qc, archive, request, response,
             packet, clockValue, sourceRank))'
      => \/ <<ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)',
                budget>>
               \in ExactDecisionTargetNeutralProducerEpisodeOrdering
         \/ ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)'
              = budget
BY HistoricalDiscoveryRetainedPacketMinimumStepCases,
   HistoricalDiscoveryFixedClockLexStepStrictlyDescends,
   ExactDecisionTargetNeutralFirstDistinctIngressConsumesFrozenRank,
   ExactDecisionTargetNeutralExactRetransmissionIsProducerStutter,
   ExactDecisionTargetNeutralComposedCausalEpisodeStepIsDescentOrFrame,
   ExactDecisionTargetNeutralFrozenPastCutsRemainPast,
   ExactDecisionTargetNeutralFrozenPhysicalCutsRemainPastOrCurrent,
   AsyncNextProjectsMonotoneProducerJournal,
   FS_CardinalityType, FS_Subset, IsaT(2400)
   DEF ExactDecisionTargetNeutralProducerEpisodeAtBudget,
       ExactDecisionTargetNeutralProducerEpisodeRank,
       ExactDecisionTargetNeutralProducerEpisodeOrdering,
       ExactDecisionTargetNeutralProducerEpisodeCarrier,
       ExactDecisionTargetNeutralIngressEpisodeRank,
       ExactDecisionTargetNeutralIngressEpisodeRemaining,
       ExactDecisionTargetNeutralSelectedCausalEpisodeRankForSnapshot,
       ExactDecisionTargetNeutralComposedCausalEpisodeRankForSnapshot,
       ExactDecisionTargetNeutralFixedClockStrictRankGoal,
       ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot,
       AsyncProducerJournalMonotoneStep,
       HistoricalDiscoveryFixedClockLexStep,
       LexPairOrdering, SetLessThan, OpToRel, AsyncAllVars

\* Every non-goal step retains the immutable snapshot and stays in the same
\* producer cell or a strictly smaller one.  This is the non-replenishment
\* statement consumed by temporal induction; materialized-identity subset is
\* retained only as an independent non-resurrection diagnostic.
THEOREM ExactDecisionTargetNeutralRetainedEpisodesDoNotReplenish ==
  \A snapshot, mode, node, qc, archive, request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier,
       budget \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ ExactDecisionTargetNeutralProducerEpisodeAtBudget(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank, budget)
      /\ [AsyncNext]_AsyncAllVars
      /\ ~(ExactDecisionTargetNeutralFixedClockStrictRankGoal(
             snapshot, mode, node, qc, archive, request, response,
             packet, clockValue, sourceRank))'
      => /\ (ExactDecisionTargetNeutralSnapshotActive(
                 snapshot, clockValue))'
         /\ ExactDecisionTargetNeutralProducerEpisodeSet(snapshot)'
              \subseteq
                ExactDecisionTargetNeutralProducerEpisodeSet(snapshot)
         /\ ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)'
              \in
                {budget}
                  \cup
                SetLessThan(
                  budget,
                  ExactDecisionTargetNeutralProducerEpisodeOrdering,
                  ExactDecisionTargetNeutralProducerEpisodeCarrier)
BY ExactDecisionTargetNeutralProducerEpisodeStepIsDescentOrFrame,
   ExactDecisionTargetNeutralMaterializedEpisodeIdentitiesDoNotResurrect,
   ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor,
   ExactDecisionAsyncNextPreservesCandidateTombstones,
   ExactDecisionTargetNeutralFrozenPastCutsRemainPast,
   ExactDecisionTargetNeutralFrozenPhysicalCutsRemainPastOrCurrent,
   ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets,
   FS_CardinalityType, FS_Subset, IsaT(1200)
   DEF ExactDecisionTargetNeutralProducerEpisodeAtBudget,
       ExactDecisionTargetNeutralSnapshotActive,
       ExactDecisionTargetNeutralFixedPredecessorSetForSnapshot,
       ExactDecisionTargetNeutralFrozenCandidateLifecycleCovered,
       ExactDecisionTargetNeutralFrozenServeLifecycleCovered,
       ExactDecisionTargetNeutralCandidateEpisodeRemaining,
       ExactDecisionTargetNeutralServeEpisodeRemaining,
       ExactDecisionTargetNeutralCandidateIdentityCoalesced,
       ExactDecisionTargetNeutralCandidateIdentityObsolete,
       ExactDecisionTargetNeutralServeIdentityRetired,
       ExactDecisionTargetNeutralProducerEpisodeRank,
       ExactDecisionTargetNeutralProducerEpisodeSet,
       ExactDecisionTargetNeutralProducerEpisodeBudget,
       ExactDecisionTargetNeutralProducerEpisodeTokens,
       ExactDecisionTargetNeutralMaterializedCandidateIdentityTokens,
       ExactDecisionTargetNeutralIngressEpisodeTokens,
       ExactDecisionTargetNeutralMaterializedServeIdentityTokens,
       ExactDecisionTargetNeutralProducerEpisodeOrdering,
       ExactDecisionTargetNeutralProducerEpisodeCarrier,
       AsyncAllVars

\* The rank remains in its exact product carrier, including its explicit
\* nested bottom.  No positive-slack or numeric-ordinal conclusion is exposed.
THEOREM ExactDecisionTargetNeutralNonGoalEpisodeRankRemainsInCarrier ==
  \A snapshot, mode, node, qc, archive, request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier,
       budget \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ ExactDecisionTargetNeutralProducerEpisodeAtBudget(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank, budget)
      /\ [AsyncNext]_AsyncAllVars
      /\ ~(ExactDecisionTargetNeutralFixedClockStrictRankGoal(
             snapshot, mode, node, qc, archive, request, response,
             packet, clockValue, sourceRank))'
      => ExactDecisionTargetNeutralProducerEpisodeBudget(snapshot)'
           \in ExactDecisionTargetNeutralProducerEpisodeCarrier
BY ExactDecisionTargetNeutralEpisodeRankIsInCarrier,
   ExactDecisionTargetNeutralRetainedEpisodesDoNotReplenish, IsaT(600)
   DEF ExactDecisionTargetNeutralProducerEpisodeAtBudget,
       ExactDecisionTargetNeutralSnapshotActive,
       ExactDecisionTargetNeutralProducerEpisodeBudget,
       ExactDecisionTargetNeutralProducerEpisodeRank,
       ExactDecisionTargetNeutralProducerEpisodeCarrier

THEOREM ExactDecisionTargetNeutralRetainedEpisodeConsumptionLowersRank ==
  \A snapshot, mode, node, qc, archive, request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier,
       budget \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ ExactDecisionTargetNeutralProducerEpisodeAtBudget(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank, budget)
      /\ [AsyncNext]_AsyncAllVars
      /\ ~ExactDecisionTargetNeutralFixedClockStrictRankGoal(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank)'
      /\ ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)'
           # budget
      => ExactDecisionTargetNeutralProducerEpisodeBudget(snapshot)'
           \in
             SetLessThan(
               budget,
               ExactDecisionTargetNeutralProducerEpisodeOrdering,
               ExactDecisionTargetNeutralProducerEpisodeCarrier)
BY ExactDecisionTargetNeutralRetainedEpisodesDoNotReplenish,
   IsaT(600)
   DEF ExactDecisionTargetNeutralProducerEpisodeAtBudget,
       ExactDecisionTargetNeutralProducerEpisodeBudget,
       ExactDecisionTargetNeutralProducerEpisodeRank,
       ExactDecisionTargetNeutralProducerEpisodeOrdering,
       ExactDecisionTargetNeutralProducerEpisodeCarrier

THEOREM ExactDecisionTargetNeutralRankCellHasConcreteFairOwner ==
  \A initialContext, snapshot, mode, node, qc, archive,
     request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier,
       budget \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
      /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
      /\ PostGstReplayQuarantineExcluded
      /\ initialContext \in ContextRecords
      /\ AsyncCurrentResponsiveVoters
           = AsyncVotersAt(initialContext)
      /\ ExactDecisionTargetNeutralProducerEpisodeAtBudget(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank, budget)
      => \E owner \in
           ExactDecisionTargetNeutralFairOwnerSet(initialContext):
           ExactDecisionTargetNeutralOwnerReadyForRankCell(
             initialContext, snapshot, mode, node, qc, archive,
             request, response, packet, clockValue,
             sourceRank, budget, owner)
BY HistoricalDiscoveryFixedClockBlockerCharacterization,
   AsyncTickEnabledHasConcreteSuccessor,
   CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner,
   ExternalCandidateProducerContinuationSelectionIsReady,
   LocalContinuationReadyEnablesFairResolution,
   ConditionalTransportContinuationReadyEnablesFairService,
   VolatileBodyContinuationReadyEnablesFairService,
   CandidateProducerContinuationFrozenSourceFairResolutionStrictlyDescends,
   OverdueResponsivePacketEnablesConcreteProgress,
   DueNodeServiceEnablesConcreteGateProgress,
   DueIoServiceEnablesConcreteLocalProgress,
   HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket,
   HistoricalDiscoverySelectedNonOverdueShadowStrictlyDescends,
   HistoricalDiscoveryRetainedPacketMinimumStepCases,
   HistoricalDiscoveryLowerCandidateInsertionReselectsLower,
   HistoricalDiscoveryLowerServeInsertionReselectsLower,
   HistoricalDiscoveryCandidateExitClassifiesOccurrenceDebt,
   HistoricalDiscoveryServeExitEitherLowersOrReplenishes,
   HistoricalDiscoveryServeFairActionLowersOccurrenceDebt,
   ExactDecisionTargetNeutralFirstDistinctIngressConsumesFrozenRank,
   ExactDecisionTargetNeutralExactRetransmissionIsProducerStutter,
   ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank,
   ExactDecisionTargetNeutralRetainedEpisodeConsumptionLowersRank,
   ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor,
   ExactDecisionTargetNeutralConcreteRankForSnapshotInCarrier,
   IsaT(1200)
   DEF ExactDecisionTargetNeutralProducerEpisodeAtBudget,
       ExactDecisionTargetNeutralRankCellOutcome,
       ExactDecisionTargetNeutralFixedClockStrictRankGoal,
       ExactDecisionTargetNeutralFixedClockExit,
       ExactDecisionTargetNeutralOwnerReadyForRankCell,
       ExactDecisionTargetNeutralFairOwnerSet,
       ExactDecisionTargetNeutralFairOwner,
       ExactDecisionTargetNeutralFairAction,
       ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot,
       ExactDecisionTargetNeutralConcreteBlockerStage,
       ExactDecisionTargetNeutralConcreteDependencyRankForSnapshot,
       ExactDecisionTargetNeutralPacketDependencyRankForSnapshot,
       ExactDecisionTargetNeutralSelectedOverduePacket,
       ExactDecisionTargetNeutralSelectedPacketDependencyRankForSnapshot,
       ExactDecisionTargetNeutralCandidateOwnersForSnapshot,
       ExactDecisionTargetNeutralCandidateRootPrecedesPhysicalCut,
       ExactDecisionTargetNeutralServeOwnersForSnapshot,
       ExactDecisionTargetNeutralCandidateOccurrenceRankForSnapshot,
       ExactDecisionTargetNeutralServeOccurrenceRankForSnapshot,
       ExactDecisionTargetNeutralProducerPrefix,
       ExactDecisionTargetNeutralProducerEpisodeBudget,
       HistoricalDiscoveryFixedClockLexStep,
       AsyncAllVars

THEOREM ExactDecisionTargetNeutralSelectedOwnerIsReady ==
  \A initialContext, snapshot, mode, node, qc, archive,
     request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier,
       budget \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
      /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
      /\ PostGstReplayQuarantineExcluded
      /\ initialContext \in ContextRecords
      /\ AsyncCurrentResponsiveVoters
           = AsyncVotersAt(initialContext)
      /\ ExactDecisionTargetNeutralProducerEpisodeAtBudget(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank, budget)
      => ExactDecisionTargetNeutralOwnerReadyForRankCell(
           initialContext, snapshot, mode, node, qc, archive,
           request, response, packet, clockValue, sourceRank, budget,
           ExactDecisionTargetNeutralSelectedFairOwner(
             initialContext, snapshot, mode, node, qc, archive,
             request, response, packet, clockValue,
             sourceRank, budget))
BY ExactDecisionTargetNeutralRankCellHasConcreteFairOwner, Isa
   DEF ExactDecisionTargetNeutralSelectedFairOwner

THEOREM ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness ==
  \A initialContext, owner:
    owner \in ExactDecisionTargetNeutralFairOwnerSet(initialContext)
      => (AsyncSpecAt(initialContext)
            => WF_AsyncAllVars(
                 ExactDecisionTargetNeutralFairAction(owner)))
BY LocalCandidateProducerContinuationResolutionUsesReviewedFairAction,
   ConditionalTransportProducerContinuationServiceUsesReviewedFairAction,
   VolatileBodyProducerContinuationServiceUsesReviewedFairAction,
   Isa, PTL
   DEF ExactDecisionTargetNeutralFairOwnerSet,
       ExactDecisionTargetNeutralFairOwner,
       ExactDecisionTargetNeutralFairAction,
       AsyncSpecAt, AsyncFairnessAt

THEOREM ExactDecisionTargetNeutralRankCellStepIsSafe ==
  \A initialContext, snapshot, mode, node, qc, archive,
     request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier,
       budget \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ DecisionFrontierUniquenessInvariant
      /\ DecisionTimeoutFrontierInvariant
      /\ ResponsiveRecoveryValidationClearedInvariant
      /\ FinalProgressWitnessClosureInvariant
      /\ ExactDecisionFanoutRetentionInvariant
      /\ ExactDecisionRequestAuthorityIsolationInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ AsyncCurrentResponsiveVoters
           = AsyncVotersAt(initialContext)
      /\ ExactDecisionTargetNeutralProducerEpisodeAtBudget(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank, budget)
      /\ [AsyncNext]_AsyncAllVars
      => \/ ExactDecisionTargetNeutralRankCellOutcome(
              snapshot, mode, node, qc, archive, request, response,
              packet, clockValue, sourceRank, budget)'
         \/ /\ ExactDecisionTargetNeutralProducerEpisodeAtBudget(
                 snapshot, mode, node, qc, archive, request, response,
                 packet, clockValue, sourceRank, budget)'
            /\ ExactDecisionTargetNeutralSelectedFairOwner(
                 initialContext, snapshot, mode, node, qc, archive,
                 request, response, packet, clockValue,
                 sourceRank, budget)'
                 =
               ExactDecisionTargetNeutralSelectedFairOwner(
                 initialContext, snapshot, mode, node, qc, archive,
                 request, response, packet, clockValue,
                 sourceRank, budget)
BY ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor,
   ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank,
   ExactDecisionTargetNeutralRetainedEpisodeConsumptionLowersRank,
   ExactDecisionRequestHeadGateResidualStepIsSafe,
   ExactDecisionResponseHeadGateResidualStepIsSafe,
   ExactDecisionBodyHoldingAliasPersistsOrFrontier,
   AsyncBracketStepRetainsExactDecisionRecord,
   AsyncBracketStepLeavesContext,
   AsyncNextPreservesExactDecisionFanoutRetention,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   AsyncBracketNextPreservesFinalProgressWitnessClosure,
   ExactDecisionAsyncNextPreservesCandidateTombstones,
   ExactDecisionTargetNeutralFirstDistinctIngressConsumesFrozenRank,
   ExactDecisionTargetNeutralExactRetransmissionIsProducerStutter,
   ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets,
   HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock,
   IsaT(1200)
   DEF ExactDecisionTargetNeutralRankCellOutcome,
       ExactDecisionTargetNeutralProducerEpisodeAtBudget,
       ExactDecisionTargetNeutralFixedClockStrictRankGoal,
       ExactDecisionTargetNeutralFixedClockPending,
       ExactDecisionTargetNeutralFixedClockExit,
       ExactDecisionTargetNeutralResidual,
       ExactDecisionTargetNeutralGoal,
       ExactDecisionTargetNeutralSelectedFairOwner,
       ExactDecisionTargetNeutralOwnerReadyForRankCell,
       ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot,
       ExactDecisionTargetNeutralProducerPrefix,
       ExactDecisionTargetNeutralProducerEpisodeBudget,
       AsyncAllVars

THEOREM ExactDecisionTargetNeutralSelectedOwnerConsumesRankCell ==
  \A initialContext, snapshot, mode, node, qc, archive,
     request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier,
       budget \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
      /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
      /\ PostGstReplayQuarantineExcluded
      /\ initialContext \in ContextRecords
      /\ AsyncCurrentResponsiveVoters
           = AsyncVotersAt(initialContext)
      /\ ExactDecisionTargetNeutralProducerEpisodeAtBudget(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank, budget)
      /\ <<ExactDecisionTargetNeutralFairAction(
             ExactDecisionTargetNeutralSelectedFairOwner(
               initialContext, snapshot, mode, node, qc, archive,
               request, response, packet, clockValue,
               sourceRank, budget))>>_AsyncAllVars
      => ExactDecisionTargetNeutralRankCellOutcome(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank, budget)'
BY ExactDecisionTargetNeutralSelectedOwnerIsReady,
   ExactDecisionTargetNeutralRankCellHasConcreteFairOwner,
   ExactDecisionTargetNeutralRetainedEpisodesDoNotReplenish,
   ExactDecisionTargetNeutralNonGoalEpisodeRankRemainsInCarrier,
   ENABLEDaxioms, IsaT(600)
   DEF ExactDecisionTargetNeutralOwnerReadyForRankCell,
       ExactDecisionTargetNeutralRankCellOutcome,
       ExactDecisionTargetNeutralProducerEpisodeAtBudget,
       ExactDecisionTargetNeutralProducerEpisodeBudget,
       AsyncAllVars

THEOREM ExactDecisionTargetNeutralProducerEpisodeBottomHasNoLowerRank ==
  /\ ExactDecisionTargetNeutralProducerEpisodeBottom
       \in ExactDecisionTargetNeutralProducerEpisodeCarrier
  /\ SetLessThan(
       ExactDecisionTargetNeutralProducerEpisodeBottom,
       ExactDecisionTargetNeutralProducerEpisodeOrdering,
       ExactDecisionTargetNeutralProducerEpisodeCarrier)
       = {}
BY IsaT(600)
   DEF ExactDecisionTargetNeutralProducerEpisodeBottom,
       ExactDecisionTargetNeutralComposedCausalEpisodeBottom,
       ExactDecisionTargetNeutralProoflessProducerBottom,
       ExactDecisionTargetNeutralCausalEpisodeBottom,
       ExactDecisionTargetNeutralProducerEpisodeOrdering,
       ExactDecisionTargetNeutralProducerEpisodeCarrier,
       ExactDecisionTargetNeutralComposedCausalEpisodeOrdering,
       ExactDecisionTargetNeutralComposedCausalEpisodeCarrier,
       ExactDecisionTargetNeutralProoflessProducerOrdering,
       ExactDecisionTargetNeutralProoflessProducerCarrier,
       AsyncCausalEpisodeStructuralRankOrdering,
       AsyncCausalEpisodeStructuralRankCarrier,
       AsyncCausalEpisodeServeRankOrdering,
       AsyncCausalEpisodeServeRankCarrier,
       LexPairOrdering, SetLessThan, OpToRel

\* The nested product bottom has no predecessor.  Therefore the selected fair
\* owner must reach the exact goal or strict composite-rank descent in the
\* same transition; no fabricated last positive ordinal is involved.
THEOREM ExactDecisionTargetNeutralProducerEpisodeBottomForcesStrictRankGoal ==
  \A initialContext, snapshot, mode, node, qc, archive,
     request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
      /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
      /\ PostGstReplayQuarantineExcluded
      /\ initialContext \in ContextRecords
      /\ AsyncCurrentResponsiveVoters
           = AsyncVotersAt(initialContext)
      /\ ExactDecisionTargetNeutralProducerEpisodeAtBudget(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank,
           ExactDecisionTargetNeutralProducerEpisodeBottom)
      /\ <<ExactDecisionTargetNeutralFairAction(
             ExactDecisionTargetNeutralSelectedFairOwner(
               initialContext, snapshot, mode, node, qc, archive,
               request, response, packet, clockValue,
               sourceRank,
               ExactDecisionTargetNeutralProducerEpisodeBottom))>>_AsyncAllVars
      => ExactDecisionTargetNeutralFixedClockStrictRankGoal(
           snapshot, mode, node, qc, archive, request, response,
           packet, clockValue, sourceRank)'
BY ExactDecisionTargetNeutralSelectedOwnerConsumesRankCell,
   ExactDecisionTargetNeutralProducerEpisodeBottomHasNoLowerRank,
   IsaT(600)
   DEF ExactDecisionTargetNeutralRankCellOutcome,
       SetLessThan, OpToRel

THEOREM ExactDecisionTargetNeutralFairEpisodeStep ==
  \A initialContext, snapshot, mode, node, qc, archive,
     request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier,
       budget \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
      AsyncSpecAt(initialContext)
        => (ExactDecisionTargetNeutralProducerEpisodeAtBudget(
              snapshot, mode, node, qc, archive, request, response,
              packet, clockValue, sourceRank, budget)
              ~> ExactDecisionTargetNeutralRankCellOutcome(
                   snapshot, mode, node, qc, archive, request, response,
                   packet, clockValue, sourceRank, budget))
PROOF
  <1>1. ASSUME NEW initialContext, NEW snapshot, NEW mode,
                NEW node, NEW qc, NEW archive, NEW request,
                NEW response, NEW packet, NEW clockValue \in Nat,
                NEW sourceRank
                  \in ExactDecisionTargetNeutralFixedClockCarrier,
                NEW budget
                  \in ExactDecisionTargetNeutralProducerEpisodeCarrier,
                AsyncSpecAt(initialContext)
         PROVE ExactDecisionTargetNeutralProducerEpisodeAtBudget(
                 snapshot, mode, node, qc, archive, request, response,
                 packet, clockValue, sourceRank, budget)
                 ~>
               ExactDecisionTargetNeutralRankCellOutcome(
                 snapshot, mode, node, qc, archive, request, response,
                 packet, clockValue, sourceRank, budget)
    <2>0. initialContext \in ContextRecords
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysKeepsFrozenContext, PTL
         DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
             Safety, TypeInvariant, AsyncFrozenContextAt
    <2>1. [](/\ AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
              /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
              /\ DecisionFrontierUniquenessInvariant
              /\ DecisionTimeoutFrontierInvariant
              /\ ResponsiveRecoveryValidationClearedInvariant
              /\ FinalProgressWitnessClosureInvariant
              /\ ExactDecisionFanoutRetentionInvariant
              /\ ExactDecisionRequestAuthorityIsolationInvariant
              /\ AsyncCandidateServiceLifecycleInvariant
              /\ PostGstReplayQuarantineExcluded
              /\ AsyncCurrentResponsiveVoters
                   = AsyncVotersAt(initialContext))
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
         AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
         DecisionFrontierUniquenessInvariantFromAsyncSpec,
         DecisionTimeoutFrontierInvariantFromAsyncSpec,
         ResponsiveRecoveryValidationClearedInvariantObligation,
         FinalProgressWitnessClosureInvariantObligation,
         AsyncSpecAlwaysRetainsExactDecisionFanout,
         AsyncSpecAlwaysIsolatesExactDecisionRequestAuthority,
         ExactDecisionAsyncSpecAlwaysCandidateTombstones,
         AsyncSpecAlwaysExcludesPostGstReplayQuarantine,
         AsyncSpecAlwaysUsesFixedResponsiveVoters, PTL
    <2>2. \A owner \in
               ExactDecisionTargetNeutralFairOwnerSet(initialContext):
             /\ ExactDecisionTargetNeutralProducerEpisodeOwnedBy(
                  initialContext, snapshot, mode, node, qc, archive,
                  request, response, packet, clockValue,
                  sourceRank, budget, owner)
             /\ [AsyncNext]_AsyncAllVars
            => \/ ExactDecisionTargetNeutralRankCellOutcome(
                    snapshot, mode, node, qc, archive, request, response,
                    packet, clockValue, sourceRank, budget)'
               \/ ExactDecisionTargetNeutralProducerEpisodeOwnedBy(
                    initialContext, snapshot, mode, node, qc, archive,
                    request, response, packet, clockValue,
                    sourceRank, budget, owner)'
      BY <2>1, ExactDecisionTargetNeutralRankCellStepIsSafe, Isa
         DEF ExactDecisionTargetNeutralProducerEpisodeOwnedBy
    <2>3. \A owner \in
               ExactDecisionTargetNeutralFairOwnerSet(initialContext):
             WF_AsyncAllVars(
               ExactDecisionTargetNeutralFairAction(owner))
      BY <1>1,
         ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness
    <2>4. \A owner \in
               ExactDecisionTargetNeutralFairOwnerSet(initialContext):
             ExactDecisionTargetNeutralProducerEpisodeOwnedBy(
               initialContext, snapshot, mode, node, qc, archive,
               request, response, packet, clockValue,
               sourceRank, budget, owner)
               => ENABLED
                    <<ExactDecisionTargetNeutralFairAction(
                        owner)>>_AsyncAllVars
      BY <2>0, <2>1,
         ExactDecisionTargetNeutralSelectedOwnerIsReady,
         ENABLEDaxioms, Isa
         DEF ExactDecisionTargetNeutralProducerEpisodeOwnedBy,
             ExactDecisionTargetNeutralOwnerReadyForRankCell
    <2>5. \A owner \in
               ExactDecisionTargetNeutralFairOwnerSet(initialContext):
             /\ ExactDecisionTargetNeutralProducerEpisodeOwnedBy(
                  initialContext, snapshot, mode, node, qc, archive,
                  request, response, packet, clockValue,
                  sourceRank, budget, owner)
             /\ <<ExactDecisionTargetNeutralFairAction(
                    owner)>>_AsyncAllVars
            => ExactDecisionTargetNeutralRankCellOutcome(
                 snapshot, mode, node, qc, archive, request, response,
                 packet, clockValue, sourceRank, budget)'
      BY <2>0, <2>1,
         ExactDecisionTargetNeutralSelectedOwnerConsumesRankCell, Isa
         DEF ExactDecisionTargetNeutralProducerEpisodeOwnedBy
    <2>6. \A owner \in
               ExactDecisionTargetNeutralFairOwnerSet(initialContext):
             ExactDecisionTargetNeutralProducerEpisodeOwnedBy(
               initialContext, snapshot, mode, node, qc, archive,
               request, response, packet, clockValue,
               sourceRank, budget, owner)
               ~> ExactDecisionTargetNeutralRankCellOutcome(
                    snapshot, mode, node, qc, archive, request, response,
                    packet, clockValue, sourceRank, budget)
      BY <2>2, <2>3, <2>4, <2>5, PTL
    <2>7. ExactDecisionTargetNeutralProducerEpisodeAtBudget(
             snapshot, mode, node, qc, archive, request, response,
             packet, clockValue, sourceRank, budget)
           => \E owner \in
                ExactDecisionTargetNeutralFairOwnerSet(initialContext):
                ExactDecisionTargetNeutralProducerEpisodeOwnedBy(
                  initialContext, snapshot, mode, node, qc, archive,
                  request, response, packet, clockValue,
                  sourceRank, budget, owner)
      BY <2>0, <2>1,
         ExactDecisionTargetNeutralSelectedOwnerIsReady, Isa
         DEF ExactDecisionTargetNeutralProducerEpisodeOwnedBy,
             ExactDecisionTargetNeutralOwnerReadyForRankCell
    <2> QED BY <2>6, <2>7, PTL
  <1> QED BY <1>1

THEOREM ExactDecisionTargetNeutralFiniteEpisodeClosesRankCell ==
  \A initialContext, snapshot, mode, node, qc, archive,
     request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier:
      AsyncSpecAt(initialContext)
        => \A budget
             \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
             ExactDecisionTargetNeutralProducerEpisodeAtBudget(
               snapshot, mode, node, qc, archive, request, response,
               packet, clockValue, sourceRank, budget)
               ~> ExactDecisionTargetNeutralFixedClockStrictRankGoal(
                    snapshot, mode, node, qc, archive, request, response,
                    packet, clockValue, sourceRank)
BY ExactDecisionTargetNeutralFairEpisodeStep,
   ExactDecisionTargetNeutralProducerEpisodeBottomForcesStrictRankGoal,
   ExactDecisionTargetNeutralProducerEpisodeOrderingIsWellFounded,
   WellFoundedLeadsTo
   DEF ExactDecisionTargetNeutralRankCellOutcome

THEOREM ExactDecisionTargetNeutralFixedClockRankStep ==
  \A initialContext, snapshot, mode, node, qc, archive,
     request, response, packet:
    \A clockValue \in Nat,
       sourceRank \in ExactDecisionTargetNeutralFixedClockCarrier:
      AsyncSpecAt(initialContext)
        => (ExactDecisionTargetNeutralFixedClockBlockedAtRank(
              snapshot, mode, node, qc, archive, request, response,
              packet, clockValue, sourceRank)
              ~> ExactDecisionTargetNeutralFixedClockStrictRankGoal(
                   snapshot, mode, node, qc, archive, request, response,
                   packet, clockValue, sourceRank))
PROOF
  <1>1. ASSUME NEW initialContext, NEW snapshot, NEW mode,
                NEW node, NEW qc, NEW archive, NEW request,
                NEW response, NEW packet, NEW clockValue \in Nat,
                NEW sourceRank
                  \in ExactDecisionTargetNeutralFixedClockCarrier,
                AsyncSpecAt(initialContext)
         PROVE ExactDecisionTargetNeutralFixedClockBlockedAtRank(
                 snapshot, mode, node, qc, archive, request, response,
                 packet, clockValue, sourceRank)
                 ~>
               ExactDecisionTargetNeutralFixedClockStrictRankGoal(
                 snapshot, mode, node, qc, archive, request, response,
                 packet, clockValue, sourceRank)
    <2>1. [](/\ AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ DecisionFrontierUniquenessInvariant
              /\ DecisionTimeoutFrontierInvariant
              /\ ResponsiveRecoveryValidationClearedInvariant
              /\ FinalProgressWitnessClosureInvariant
              /\ ExactDecisionFanoutRetentionInvariant
              /\ ExactDecisionRequestAuthorityIsolationInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         DecisionFrontierUniquenessInvariantFromAsyncSpec,
         DecisionTimeoutFrontierInvariantFromAsyncSpec,
         ResponsiveRecoveryValidationClearedInvariantObligation,
         FinalProgressWitnessClosureInvariantObligation,
         AsyncSpecAlwaysRetainsExactDecisionFanout,
         AsyncSpecAlwaysIsolatesExactDecisionRequestAuthority, PTL
    <2>2. [](ExactDecisionTargetNeutralFixedClockBlockedAtRank(
                 snapshot, mode, node, qc, archive, request, response,
                 packet, clockValue, sourceRank)
                =>
              \/ ExactDecisionTargetNeutralFixedClockStrictRankGoal(
                   snapshot, mode, node, qc, archive, request, response,
                   packet, clockValue, sourceRank)
              \/ \E budget
                   \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
                   ExactDecisionTargetNeutralProducerEpisodeAtBudget(
                     snapshot, mode, node, qc, archive, request, response,
                     packet, clockValue, sourceRank, budget))
      BY <2>1, ExactDecisionTargetNeutralEpisodeRankIsInCarrier, PTL
         DEF ExactDecisionTargetNeutralFixedClockBlockedAtRank,
             ExactDecisionTargetNeutralProducerEpisodeAtBudget,
             ExactDecisionTargetNeutralFixedClockStrictRankGoal
    <2>3. \A budget
               \in ExactDecisionTargetNeutralProducerEpisodeCarrier:
             ExactDecisionTargetNeutralProducerEpisodeAtBudget(
               snapshot, mode, node, qc, archive, request, response,
               packet, clockValue, sourceRank, budget)
               ~> ExactDecisionTargetNeutralFixedClockStrictRankGoal(
                    snapshot, mode, node, qc, archive, request, response,
                    packet, clockValue, sourceRank)
      BY <1>1,
         ExactDecisionTargetNeutralFiniteEpisodeClosesRankCell
    <2> QED BY <2>2, <2>3, PTL
  <1> QED BY <1>1

THEOREM ExactDecisionTargetNeutralFixedClockConverges ==
  \A initialContext, snapshot, mode, node, qc, archive,
     request, response, packet:
    \A clockValue \in Nat:
      AsyncSpecAt(initialContext)
        => ExactDecisionTargetNeutralFixedClockPending(
             snapshot,
             mode, node, qc, archive, request, response,
             packet, clockValue)
             ~> ExactDecisionTargetNeutralFixedClockExit(
                  mode, node, qc, archive, request,
                  response, packet, clockValue)
PROOF
  <1>1. ASSUME NEW initialContext, NEW snapshot,
                NEW mode, NEW node, NEW qc,
                NEW archive, NEW request, NEW response, NEW packet,
                NEW clockValue \in Nat,
                AsyncSpecAt(initialContext)
         PROVE ExactDecisionTargetNeutralFixedClockPending(
                 snapshot,
                 mode, node, qc, archive, request, response,
                 packet, clockValue)
                 ~>
               ExactDecisionTargetNeutralFixedClockExit(
                 mode, node, qc, archive, request,
                 response, packet, clockValue)
    <2>1. \A rank \in
               ExactDecisionTargetNeutralFixedClockCarrier:
             ExactDecisionTargetNeutralFixedClockBlockedAtRank(
               snapshot,
               mode, node, qc, archive, request, response,
               packet, clockValue, rank)
               ~>
             (ExactDecisionTargetNeutralFixedClockExit(
                mode, node, qc, archive, request,
                response, packet, clockValue)
              \/ \E lowerRank \in
                   SetLessThan(
                     rank,
                     ExactDecisionTargetNeutralFixedClockOrdering,
                     ExactDecisionTargetNeutralFixedClockCarrier):
                   ExactDecisionTargetNeutralFixedClockBlockedAtRank(
                     snapshot,
                     mode, node, qc, archive, request, response,
                     packet, clockValue, lowerRank))
      BY <1>1, ExactDecisionTargetNeutralFixedClockRankStep
         DEF ExactDecisionTargetNeutralFixedClockStrictRankGoal
    <2>2. \A rank \in
               ExactDecisionTargetNeutralFixedClockCarrier:
             ExactDecisionTargetNeutralFixedClockBlockedAtRank(
               snapshot,
               mode, node, qc, archive, request, response,
               packet, clockValue, rank)
               ~> ExactDecisionTargetNeutralFixedClockExit(
                    mode, node, qc, archive, request,
                    response, packet, clockValue)
      BY <2>1,
         ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded,
         WellFoundedLeadsTo
    <2>3. [](ExactDecisionTargetNeutralFixedClockPending(
                 snapshot,
                 mode, node, qc, archive, request, response,
                 packet, clockValue)
                =>
              \E rank \in
                   ExactDecisionTargetNeutralFixedClockCarrier:
                ExactDecisionTargetNeutralFixedClockBlockedAtRank(
                  snapshot,
                  mode, node, qc, archive, request, response,
                  packet, clockValue, rank))
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         ExactDecisionAsyncSpecAlwaysCandidateTombstones,
         ExactDecisionTargetNeutralSnapshotIsFinite,
         ExactDecisionTargetNeutralConcreteRankForSnapshotInCarrier, PTL
         DEF ExactDecisionTargetNeutralFixedClockBlockedAtRank
    <2> QED BY <2>2, <2>3, PTL
  <1> QED BY <1>1

(***************************************************************************
Every source action other than Tick freezes the target-neutral clock.  This
is kept local so the exact leaves do not import the historical temporal
closure or any target-to-Decision convergence theorem.
***************************************************************************)

THEOREM ExactDecisionTargetNeutralNonTickNonRunnerStepLeavesClock ==
  /\ AsyncNonRunnerStep
  /\ ~AsyncTick
  => asyncNow' = asyncNow
PROOF
  <1>1. ASSUME AsyncNonRunnerStep, ~AsyncTick
         PROVE asyncNow' = asyncNow
    <2>1. CASE AsyncSetGST
      BY <2>1, Isa DEF AsyncSetGST, AsyncSchedulerVars
    <2>2. CASE AsyncTick
      BY <1>1, <2>2
    <2>3. CASE \E node \in ValidatorIds:
                  OpenHistoricalRecovery(node)
      BY <2>3, Isa
         DEF OpenHistoricalRecovery,
             AsyncSchedulerExceptHistoricalRecoveryTargets
    <2>4. CASE \E node \in AsyncCurrentResponsiveVoters:
                  DirectCommitCertificateDiscoveryStep(node)
      BY <2>4, Isa DEF DirectCommitCertificateDiscoveryStep
    <2>5. CASE \E node \in asyncHistoricalRecoveryTargets:
                  DirectHistoricalCommitCertificateDiscoveryStep(node)
      BY <2>5, Isa
         DEF DirectHistoricalCommitCertificateDiscoveryStep,
             CommitCertificateDiscoveryStepWork
    <2>6. CASE \E node \in AsyncArchiveIoServiceNodes:
                  ServiceIoWorker(node)
      BY <2>6, Isa DEF ServiceIoWorker, ServiceIoWorkerWork
    <2>7. CASE \E node \in asyncHistoricalRecoveryTargets:
                  ServiceHistoricalRecoveryIoWorker(node)
      BY <2>7, Isa
         DEF ServiceHistoricalRecoveryIoWorker, ServiceIoWorkerWork
    <2>8. CASE \E node \in AsyncCurrentResponsiveVoters:
                  EnqueueIoLocalControl(node)
      BY <2>8, Isa
         DEF EnqueueIoLocalControl, EnqueueIoLocalControlWork
    <2>9. CASE \E node \in asyncHistoricalRecoveryTargets:
                  EnqueueHistoricalRecoveryIoLocalControl(node)
      BY <2>9, Isa
         DEF EnqueueHistoricalRecoveryIoLocalControl,
             EnqueueIoLocalControlWork
    <2>10. CASE AsyncNetworkStep
      BY <2>10, Isa
         DEF AsyncNetworkStep, AdmitIngressPacket,
             AdmitHiddenPacket, CoalesceHiddenPacket
    <2>11. CASE AsyncFaultStep
      BY <2>11, AsyncFaultStepLeavesDiscoveryClock
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11
         DEF AsyncNonRunnerStep
  <1> QED BY <1>1

THEOREM ExactDecisionTargetNeutralNonTickAsyncNextLeavesClock ==
  /\ AsyncNext
  /\ ~AsyncTick
  => asyncNow' = asyncNow
PROOF
  <1>1. ASSUME AsyncNext, ~AsyncTick
         PROVE asyncNow' = asyncNow
    <2>1. CASE AsyncNonCrashStep
      <3>1. CASE AsyncRunnerStep
        BY <3>1, AsyncRunnerStepLeavesDiscoveryClock
      <3>2. CASE AsyncNonRunnerStep
        BY <1>1, <3>2,
           ExactDecisionTargetNeutralNonTickNonRunnerStepLeavesClock
      <3>3. CASE DriveResponsiveReplayHead \/ FinishResponsiveReplay
        BY <3>3, Isa
           DEF DriveResponsiveReplayHead, FinishResponsiveReplay
      <3>4. CASE RearmResponsiveRecovery
        BY <3>4, Isa DEF RearmResponsiveRecovery, AsyncSchedulerVars
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4
           DEF AsyncNonCrashStep
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <2>2, Isa DEF PreGstCrash, AsyncSchedulerVars
    <2>3. CASE \E node \in ValidatorIds:
                  PreGstResponsiveCrash(node)
      BY <2>3, Isa
         DEF PreGstResponsiveCrash, AsyncSchedulerVars
    <2>4. CASE PreGstResponsiveRestart
      BY <2>4, Isa
         DEF PreGstResponsiveRestart, AsyncSchedulerVars
    <2>5. CASE PreGstResponsiveReplay
      BY <2>5, Isa
         DEF PreGstResponsiveReplay, ResetNodeSchedulerForRestart
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5
         DEF AsyncNext
  <1> QED BY <1>1

THEOREM ExactDecisionTargetNeutralEveryNonTickSourceStepLeavesClock ==
  /\ [AsyncNext]_AsyncAllVars
  /\ ~AsyncTick
  => asyncNow' = asyncNow
PROOF
  <1>1. ASSUME [AsyncNext]_AsyncAllVars, ~AsyncTick
         PROVE asyncNow' = asyncNow
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <2>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars, AsyncProducerVars
    <2>2. CASE AsyncNext
      BY <1>1, <2>2,
         ExactDecisionTargetNeutralNonTickAsyncNextLeavesClock
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Outer immutable deadline rank.
***************************************************************************)

ExactDecisionTargetNeutralClockBudgetFrontier(
    mode, node, qc, archive, request, response, packet, budget) ==
  /\ mode \in ExactDecisionTargetNeutralModeSet
  /\ gst
  /\ ExactDecisionTargetNeutralResidual(
       mode, node, qc, archive, request, response, packet)
  /\ ~ExactDecisionTargetNeutralGoal(
       mode, node, qc, archive, request, response, packet)
  /\ asyncNow \in Nat
  /\ budget \in Nat
  /\ asyncNow + budget
       = ExactDecisionTargetNeutralDeadline(mode, node, packet)

ExactDecisionTargetNeutralClockBudgetGoal(
    mode, node, qc, archive, request, response, packet, budget) ==
  \/ ExactDecisionTargetNeutralGoal(
       mode, node, qc, archive, request, response, packet)
  \/ \E lowerBudget \in
       SetLessThan(budget, OpToRel(<, Nat), Nat):
       ExactDecisionTargetNeutralClockBudgetFrontier(
         mode, node, qc, archive, request, response, packet,
         lowerBudget)

THEOREM ExactDecisionTargetNeutralDueHeadDisablesTick ==
  \A mode \in {"RequestHead", "ResponseHead"}:
    \A node, qc, archive, request, response, packet:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ DecisionFrontierUniquenessInvariant
      /\ DecisionTimeoutFrontierInvariant
      /\ ResponsiveRecoveryValidationClearedInvariant
      /\ FinalProgressWitnessClosureInvariant
      /\ ExactDecisionFanoutRetentionInvariant
      /\ ExactDecisionRequestAuthorityIsolationInvariant
      /\ gst
      /\ ExactDecisionTargetNeutralResidual(
           mode, node, qc, archive, request, response, packet)
      /\ ~ExactDecisionTargetNeutralGoal(
           mode, node, qc, archive, request, response, packet)
      /\ packet.deadline <= asyncNow
      => /\ packet \in OverdueResponsivePackets
         /\ ~AsyncTickEnabled
         /\ ~ENABLED <<AsyncTick>>_AsyncAllVars
BY ExactDecisionRequestHasResponsiveBodyHoldingAlias,
   ExactDecisionResponsePacketIsAuthorized,
   ExactDecisionResponseRemainingHeadGateIsDeadlineOrShadow,
   ExpandENABLED, IsaT(300)
   DEF ExactDecisionTargetNeutralResidual,
       ExactDecisionTargetNeutralGoal,
       ExactDecisionRequestHeadGateOwnerResidual,
       ExactDecisionRequestIngressResidual,
       ExactDecisionRequestPacketOwned,
       ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual,
       ExactDecisionResponseNonPhysicalHeadGateOwnerResidual,
       ExactDecisionResponseHeadGateOwnerResidual,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionResponsePacketOwned,
       ExactDecisionActiveRequestOwner,
       ExactDecisionServiceSource,
       ExactDecisionBodyHoldingAlias,
       OverdueResponsivePackets, AsyncPacketOwnsClockDeadline,
       AsyncServeTransportAdmissionGateAllows,
       AsyncServeRequestAuthorized, AsyncServeRequestServiceable,
       AsyncTickEnabled,
       AsyncTimedServiceNodes, AsyncAllVars

THEOREM ExactDecisionTargetNeutralDueHeadStepLeavesClockOrGoals ==
  \A mode \in {"RequestHead", "ResponseHead"}:
    \A node, qc, archive, request, response, packet:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ DecisionFrontierUniquenessInvariant
      /\ DecisionTimeoutFrontierInvariant
      /\ ResponsiveRecoveryValidationClearedInvariant
      /\ FinalProgressWitnessClosureInvariant
      /\ ExactDecisionFanoutRetentionInvariant
      /\ ExactDecisionRequestAuthorityIsolationInvariant
      /\ gst
      /\ ExactDecisionTargetNeutralResidual(
           mode, node, qc, archive, request, response, packet)
      /\ ~ExactDecisionTargetNeutralGoal(
           mode, node, qc, archive, request, response, packet)
      /\ packet.deadline <= asyncNow
      /\ [AsyncNext]_AsyncAllVars
      => \/ ExactDecisionTargetNeutralGoal(
              mode, node, qc, archive, request, response, packet)'
         \/ /\ ExactDecisionTargetNeutralResidual(
                 mode, node, qc, archive,
                 request, response, packet)'
            /\ asyncNow' = asyncNow
BY ExactDecisionTargetNeutralDueHeadDisablesTick,
   ExactDecisionTargetNeutralEveryNonTickSourceStepLeavesClock,
   ExactDecisionRequestHeadGateResidualStepIsSafe,
   ExactDecisionResponseHeadGateResidualStepIsSafe,
   ExactDecisionRequestTypedTickAdvancesClock,
   ExpandENABLED, IsaT(300)
   DEF ExactDecisionTargetNeutralResidual,
       ExactDecisionTargetNeutralGoal,
       AsyncTick, AsyncAllVars

THEOREM ExactDecisionTargetNeutralFixedClockLowersDeadlineBudget ==
  \A initialContext, mode, node, qc, archive,
     request, response, packet, budget:
    AsyncSpecAt(initialContext)
      => (ExactDecisionTargetNeutralClockBudgetFrontier(
         mode, node, qc, archive, request, response, packet, budget)
         ~> ExactDecisionTargetNeutralClockBudgetGoal(
              mode, node, qc, archive, request, response, packet, budget))
PROOF
  <1>1. ASSUME NEW initialContext, NEW mode, NEW node, NEW qc,
                NEW archive, NEW request, NEW response, NEW packet,
                NEW budget,
                AsyncSpecAt(initialContext)
         PROVE ExactDecisionTargetNeutralClockBudgetFrontier(
                 mode, node, qc, archive, request, response,
                 packet, budget)
                 ~>
               ExactDecisionTargetNeutralClockBudgetGoal(
                 mode, node, qc, archive, request, response,
                 packet, budget)
    <2>1. [](/\ AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ AsyncCandidateServiceLifecycleInvariant
              /\ DecisionFrontierUniquenessInvariant
              /\ DecisionTimeoutFrontierInvariant
              /\ ResponsiveRecoveryValidationClearedInvariant
              /\ FinalProgressWitnessClosureInvariant
              /\ ExactDecisionFanoutRetentionInvariant
              /\ ExactDecisionRequestAuthorityIsolationInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         ExactDecisionAsyncSpecAlwaysCandidateTombstones,
         DecisionFrontierUniquenessInvariantFromAsyncSpec,
         DecisionTimeoutFrontierInvariantFromAsyncSpec,
         ResponsiveRecoveryValidationClearedInvariantObligation,
         FinalProgressWitnessClosureInvariantObligation,
         AsyncSpecAlwaysRetainsExactDecisionFanout,
         AsyncSpecAlwaysIsolatesExactDecisionRequestAuthority, PTL
    <2>2. \A snapshot:
             \A clockValue \in Nat:
               ExactDecisionTargetNeutralFixedClockPending(
                 snapshot, mode, node, qc, archive, request, response,
                 packet, clockValue)
                 ~> ExactDecisionTargetNeutralFixedClockExit(
                      mode, node, qc, archive, request,
                      response, packet, clockValue)
      BY <1>1, ExactDecisionTargetNeutralFixedClockConverges
    <2>3. ExactDecisionTargetNeutralClockBudgetFrontier(
             mode, node, qc, archive, request, response, packet, budget)
             /\ ~ExactDecisionTargetNeutralGoal(
                  mode, node, qc, archive, request, response, packet)
           =>
         \E clockValue \in Nat:
           \E snapshot:
             /\ clockValue = asyncNow
             /\ snapshot =
                  ExactDecisionTargetNeutralFixedClockSnapshot(clockValue)
             /\ ExactDecisionTargetNeutralFixedClockPending(
                  snapshot, mode, node, qc, archive, request, response,
                  packet, clockValue)
      BY <2>1, ExactDecisionTargetNeutralSnapshotIsFinite, Isa
         DEF ExactDecisionTargetNeutralClockBudgetFrontier,
             ExactDecisionTargetNeutralFixedClockPending,
             ExactDecisionTargetNeutralFixedClockSnapshot,
             ExactDecisionTargetNeutralCurrentPhysicalCuts,
             ExactDecisionTargetNeutralSnapshotActive,
             ExactDecisionTargetNeutralFixedPredecessorSetForSnapshot,
             ExactDecisionTargetNeutralCurrentPastSchedulerCut,
             ExactDecisionTargetNeutralCurrentPastSchedulerCuts,
             ExactDecisionTargetNeutralSchedulerCutToken,
             ExactDecisionTargetNeutralSchedulerCutTokens,
             ExactDecisionTargetNeutralRetainedCausalRootsForSnapshot,
             ExactDecisionTargetNeutralFrozenCandidateLifecycleCovered,
             ExactDecisionTargetNeutralFrozenServeLifecycleCovered,
             ExactDecisionTargetNeutralCandidateEpisodeRemaining,
             ExactDecisionTargetNeutralServeEpisodeRemaining,
             ExactDecisionTargetNeutralFrozenProducerRequests,
             ExactDecisionTargetNeutralIngressEpisodeUniverse,
             AsyncProducerEpisode, AsyncProducerIngressStage,
             ExactDecisionTargetNeutralCandidateEpisodeUniverse,
             ExactDecisionTargetNeutralFrozenCausalRoots,
             ExactDecisionTargetNeutralRetainedCandidateCausalRoots,
             ExactDecisionTargetNeutralLiveCandidateCausalRoots,
             ExactDecisionTargetNeutralLifecycleRecordCausalRoots,
             ExactDecisionTargetNeutralProducerContinuationCausalRoots,
             ExactDecisionTargetNeutralOrdinaryIngressCausalRoots,
             ExactDecisionTargetNeutralLeaderWireCausalRoots,
             ExactDecisionTargetNeutralDurableReplayCausalRoots,
             ExactDecisionTargetNeutralTimeoutReservationCausalRoots,
             ExactDecisionTargetNeutralDuePacketCausalRoots,
             ExactDecisionTargetNeutralServeEpisodeUniverse,
             ExactDecisionTargetNeutralRetainedServeLifecycleRecords,
             ExactDecisionTargetNeutralRetainedServeIdentitySet,
             ExactDecisionTargetNeutralLiveCandidateIdentitySet,
             ExactDecisionTargetNeutralLiveServeIdentitySet,
             ExactDecisionTargetNeutralCandidateOwnerIdentitySet,
             ExactDecisionTargetNeutralCausalRootSet,
             ExactDecisionTargetNeutralCausalRoot,
             ExactDecisionTargetNeutralServeOwnerIdentitySet,
             ExactDecisionTargetNeutralCandidateOwnerIdentity,
             ExactDecisionTargetNeutralServeOwnerIdentity,
             AsyncIoServeJobIdentity,
             AsyncServeLogicalRequestIdentities
    <2>4. \A clockValue \in Nat:
             /\ ExactDecisionTargetNeutralClockBudgetFrontier(
                  mode, node, qc, archive, request, response,
                  packet, budget)
             /\ asyncNow = clockValue
             /\ ExactDecisionTargetNeutralFixedClockExit(
                  mode, node, qc, archive, request,
                  response, packet, clockValue)
            => ExactDecisionTargetNeutralClockBudgetGoal(
                 mode, node, qc, archive, request, response,
                 packet, budget)
      BY SMT
         DEF ExactDecisionTargetNeutralClockBudgetFrontier,
             ExactDecisionTargetNeutralClockBudgetGoal,
             ExactDecisionTargetNeutralFixedClockExit,
             ExactDecisionTargetNeutralDeadline,
             SetLessThan, OpToRel
    <2> QED BY <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM ExactDecisionTargetNeutralDeadlineBudgetConverges ==
  \A initialContext, mode, node, qc, archive,
     request, response, packet:
    AsyncSpecAt(initialContext)
      => \A budget \in Nat:
           ExactDecisionTargetNeutralClockBudgetFrontier(
             mode, node, qc, archive, request, response, packet, budget)
             ~> ExactDecisionTargetNeutralGoal(
                  mode, node, qc, archive, request, response, packet)
BY ExactDecisionTargetNeutralFixedClockLowersDeadlineBudget,
   NatLessThanWellFounded, WellFoundedLeadsTo
   DEF ExactDecisionTargetNeutralClockBudgetGoal

THEOREM ExactDecisionTargetNeutralResidualReachesDeadlineOrGoal ==
  \A initialContext, mode, node, qc, archive,
     request, response, packet:
    AsyncSpecAt(initialContext)
      => (ExactDecisionTargetNeutralResidual(
            mode, node, qc, archive, request, response, packet)
            ~> (ExactDecisionTargetNeutralGoal(
                  mode, node, qc, archive, request, response, packet)
                 \/ asyncNow
                      >= ExactDecisionTargetNeutralDeadline(
                           mode, node, packet)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW mode, NEW node, NEW qc,
                NEW archive, NEW request, NEW response, NEW packet,
                AsyncSpecAt(initialContext)
         PROVE ExactDecisionTargetNeutralResidual(
                 mode, node, qc, archive, request, response, packet)
                 ~>
               (ExactDecisionTargetNeutralGoal(
                  mode, node, qc, archive, request, response, packet)
                \/ asyncNow
                     >= ExactDecisionTargetNeutralDeadline(
                          mode, node, packet))
    <2>1. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>2. \A budget \in Nat:
             ExactDecisionTargetNeutralClockBudgetFrontier(
               mode, node, qc, archive, request, response,
               packet, budget)
               ~> ExactDecisionTargetNeutralGoal(
                    mode, node, qc, archive, request, response, packet)
      BY <1>1, ExactDecisionTargetNeutralDeadlineBudgetConverges
    <2>3. [](ExactDecisionTargetNeutralResidual(
                 mode, node, qc, archive, request, response, packet)
                =>
              \/ ExactDecisionTargetNeutralGoal(
                   mode, node, qc, archive, request, response, packet)
              \/ asyncNow
                   >= ExactDecisionTargetNeutralDeadline(
                        mode, node, packet)
              \/ \E budget \in Nat:
                   ExactDecisionTargetNeutralClockBudgetFrontier(
                     mode, node, qc, archive, request, response,
                     packet, budget))
      BY <2>1, SMT, PTL
         DEF ExactDecisionTargetNeutralClockBudgetFrontier,
             ExactDecisionTargetNeutralDeadline,
             ExactDecisionTargetNeutralResidual,
             ExactDecisionRequestPacketEmissionResidual,
             ExactDecisionRequestHeadGateOwnerResidual,
             ExactDecisionRequestIngressResidual,
             ExactDecisionRequestPacketOwned,
             ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual,
             ExactDecisionResponseNonPhysicalHeadGateOwnerResidual,
             ExactDecisionResponseHeadGateOwnerResidual,
             ExactDecisionResponseAdmissionResidual,
             ExactDecisionResponsePacketOwned,
             AsyncStrongTypeInvariant,
             AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant,
             AsyncPacketContentTypeInvariant,
             AsyncPacketTyped
    <2> QED BY <2>2, <2>3, PTL
  <1> QED BY <1>1

THEOREM ExactDecisionTargetNeutralDueHeadReachesReadyGoal ==
  \A initialContext, mode, node, qc, archive,
     request, response, packet:
    /\ mode \in {"RequestHead", "ResponseHead"}
    /\ AsyncSpecAt(initialContext)
    => (/\ ExactDecisionTargetNeutralResidual(
             mode, node, qc, archive, request, response, packet)
         /\ packet.deadline <= asyncNow)
          ~> ExactDecisionTargetNeutralGoal(
               mode, node, qc, archive, request, response, packet)
PROOF
  <1>1. ASSUME NEW initialContext, NEW mode, NEW node, NEW qc,
                NEW archive, NEW request, NEW response, NEW packet,
                mode \in {"RequestHead", "ResponseHead"},
                AsyncSpecAt(initialContext)
         PROVE (/\ ExactDecisionTargetNeutralResidual(
                      mode, node, qc, archive, request, response, packet)
                  /\ packet.deadline <= asyncNow)
                  ~>
               ExactDecisionTargetNeutralGoal(
                 mode, node, qc, archive, request, response, packet)
    <2>1. [](/\ AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ AsyncCandidateServiceLifecycleInvariant
              /\ DecisionFrontierUniquenessInvariant
              /\ DecisionTimeoutFrontierInvariant
              /\ ResponsiveRecoveryValidationClearedInvariant
              /\ FinalProgressWitnessClosureInvariant
              /\ ExactDecisionFanoutRetentionInvariant
              /\ ExactDecisionRequestAuthorityIsolationInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         ExactDecisionAsyncSpecAlwaysCandidateTombstones,
         DecisionFrontierUniquenessInvariantFromAsyncSpec,
         DecisionTimeoutFrontierInvariantFromAsyncSpec,
         ResponsiveRecoveryValidationClearedInvariantObligation,
         FinalProgressWitnessClosureInvariantObligation,
         AsyncSpecAlwaysRetainsExactDecisionFanout,
         AsyncSpecAlwaysIsolatesExactDecisionRequestAuthority, PTL
    <2>2. \A snapshot:
             \A clockValue \in Nat:
               ExactDecisionTargetNeutralFixedClockPending(
                 snapshot, mode, node, qc, archive, request, response,
                 packet, clockValue)
                 ~> ExactDecisionTargetNeutralFixedClockExit(
                      mode, node, qc, archive, request,
                      response, packet, clockValue)
      BY <1>1, ExactDecisionTargetNeutralFixedClockConverges
    <2>3. [](/\ ExactDecisionTargetNeutralResidual(
                  mode, node, qc, archive, request, response, packet)
              /\ ~ExactDecisionTargetNeutralGoal(
                   mode, node, qc, archive, request, response, packet)
              /\ packet.deadline <= asyncNow
             => ~ENABLED <<AsyncTick>>_AsyncAllVars)
      BY <1>1, <2>1,
         ExactDecisionTargetNeutralDueHeadDisablesTick, PTL
    <2>4. [](ExactDecisionTargetNeutralResidual(
                 mode, node, qc, archive, request, response, packet)
                /\ packet.deadline <= asyncNow
               =>
              \/ ExactDecisionTargetNeutralGoal(
                   mode, node, qc, archive, request, response, packet)
              \/ \E clockValue \in Nat:
                   \E snapshot:
                     /\ clockValue = asyncNow
                     /\ snapshot =
                          ExactDecisionTargetNeutralFixedClockSnapshot(
                            clockValue)
                     /\ ExactDecisionTargetNeutralFixedClockPending(
                          snapshot, mode, node, qc, archive, request,
                          response, packet, clockValue))
      BY <2>1, ExactDecisionTargetNeutralSnapshotIsFinite, Isa, PTL
         DEF ExactDecisionTargetNeutralFixedClockPending,
             ExactDecisionTargetNeutralFixedClockSnapshot,
             ExactDecisionTargetNeutralCurrentPhysicalCuts,
             ExactDecisionTargetNeutralSnapshotActive,
             ExactDecisionTargetNeutralFixedPredecessorSetForSnapshot,
             ExactDecisionTargetNeutralCurrentPastSchedulerCut,
             ExactDecisionTargetNeutralCurrentPastSchedulerCuts,
             ExactDecisionTargetNeutralSchedulerCutToken,
             ExactDecisionTargetNeutralSchedulerCutTokens,
             ExactDecisionTargetNeutralRetainedCausalRootsForSnapshot,
             ExactDecisionTargetNeutralFrozenCandidateLifecycleCovered,
             ExactDecisionTargetNeutralFrozenServeLifecycleCovered,
             ExactDecisionTargetNeutralCandidateEpisodeRemaining,
             ExactDecisionTargetNeutralServeEpisodeRemaining,
             ExactDecisionTargetNeutralFrozenProducerRequests,
             ExactDecisionTargetNeutralIngressEpisodeUniverse,
             AsyncProducerEpisode, AsyncProducerIngressStage,
             ExactDecisionTargetNeutralCandidateEpisodeUniverse,
             ExactDecisionTargetNeutralFrozenCausalRoots,
             ExactDecisionTargetNeutralRetainedCandidateCausalRoots,
             ExactDecisionTargetNeutralLiveCandidateCausalRoots,
             ExactDecisionTargetNeutralLifecycleRecordCausalRoots,
             ExactDecisionTargetNeutralProducerContinuationCausalRoots,
             ExactDecisionTargetNeutralOrdinaryIngressCausalRoots,
             ExactDecisionTargetNeutralLeaderWireCausalRoots,
             ExactDecisionTargetNeutralDurableReplayCausalRoots,
             ExactDecisionTargetNeutralTimeoutReservationCausalRoots,
             ExactDecisionTargetNeutralDuePacketCausalRoots,
             ExactDecisionTargetNeutralServeEpisodeUniverse,
             ExactDecisionTargetNeutralRetainedServeLifecycleRecords,
             ExactDecisionTargetNeutralRetainedServeIdentitySet,
             ExactDecisionTargetNeutralLiveCandidateIdentitySet,
             ExactDecisionTargetNeutralLiveServeIdentitySet,
             ExactDecisionTargetNeutralCandidateOwnerIdentitySet,
             ExactDecisionTargetNeutralCausalRootSet,
             ExactDecisionTargetNeutralCausalRoot,
             ExactDecisionTargetNeutralServeOwnerIdentitySet,
             ExactDecisionTargetNeutralCandidateOwnerIdentity,
             ExactDecisionTargetNeutralServeOwnerIdentity,
             AsyncIoServeJobIdentity,
             AsyncServeLogicalRequestIdentities
    <2>5. [](/\ ExactDecisionTargetNeutralResidual(
                  mode, node, qc, archive, request, response, packet)
              /\ ~ExactDecisionTargetNeutralGoal(
                   mode, node, qc, archive, request, response, packet)
              /\ packet.deadline <= asyncNow
              /\ [AsyncNext]_AsyncAllVars
             => \/ ExactDecisionTargetNeutralGoal(
                     mode, node, qc, archive, request, response, packet)'
                \/ /\ ExactDecisionTargetNeutralResidual(
                        mode, node, qc, archive,
                        request, response, packet)'
                   /\ asyncNow' = asyncNow)
      BY <1>1, <2>1,
         ExactDecisionTargetNeutralDueHeadStepLeavesClockOrGoals, PTL
    <2> QED BY <2>2, <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

THEOREM ExactDecisionRequestClockOwnerConvergence ==
  \A initialContext:
    ExactDecisionRequestClockOwnerConvergenceProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext, AsyncSpecAt(initialContext)
         PROVE \A node, qc:
                  ExactDecisionRequestPacketEmissionResidual(node, qc)
                    ~> (ExactDecisionRequestPacketEmissionGoal(node, qc)
                         \/ ExactDecisionRequestRetransmitArmedResidual(
                              node, qc))
    <2>1. ASSUME NEW node, NEW qc
           PROVE ExactDecisionRequestPacketEmissionResidual(node, qc)
                   ~> (ExactDecisionRequestPacketEmissionGoal(node, qc)
                        \/ ExactDecisionRequestRetransmitArmedResidual(
                             node, qc))
      <3>1. ExactDecisionRequestPacketEmissionResidual(node, qc)
               ~>
             (ExactDecisionRequestPacketEmissionGoal(node, qc)
              \/ ExactDecisionRequestClockElapsedResidual(node, qc))
        BY <1>1,
           ExactDecisionTargetNeutralResidualReachesDeadlineOrGoal,
           ExactDecisionBodyHoldingAliasPersistsOrFrontier, PTL
           DEF ExactDecisionTargetNeutralResidual,
               ExactDecisionTargetNeutralGoal,
               ExactDecisionTargetNeutralDeadline,
               ExactDecisionRequestClockElapsedResidual
      <3>2. ExactDecisionRequestClockElapsedResidual(node, qc)
               ~>
             (ExactDecisionRequestPacketEmissionGoal(node, qc)
              \/ ExactDecisionRequestRetransmitArmedResidual(node, qc))
        BY <1>1, FairExactDecisionRequestClockElapsedAcquiresOwner
      <3> QED BY <3>1, <3>2, PTL
    <2> QED BY <2>1
  <1> QED BY <1>1
       DEF ExactDecisionRequestClockOwnerConvergenceProperty

THEOREM ExactDecisionRequestHeadGateOwnerConvergence ==
  \A initialContext:
    ExactDecisionRequestHeadGateOwnerConvergenceProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext, AsyncSpecAt(initialContext)
         PROVE \A node, qc, archive, request, packet:
                  ExactDecisionRequestHeadGateOwnerResidual(
                    node, qc, archive, request, packet)
                    ~> (ExactDecisionRequestIngressGoal(
                          node, qc, archive, request)
                         \/ ExactDecisionRequestPacketAdmissionReady(
                              node, qc, archive, request, packet))
    <2>1. ASSUME NEW node, NEW qc, NEW archive,
                  NEW request, NEW packet
           PROVE ExactDecisionRequestHeadGateOwnerResidual(
                   node, qc, archive, request, packet)
                   ~> (ExactDecisionRequestIngressGoal(
                         node, qc, archive, request)
                        \/ ExactDecisionRequestPacketAdmissionReady(
                             node, qc, archive, request, packet))
      <3>1. ExactDecisionTargetNeutralResidual(
               "RequestHead", node, qc, archive,
               request, NoAsyncItem, packet)
               ~>
             (ExactDecisionTargetNeutralGoal(
                "RequestHead", node, qc, archive,
                request, NoAsyncItem, packet)
              \/ packet.deadline <= asyncNow)
        BY <1>1,
           ExactDecisionTargetNeutralResidualReachesDeadlineOrGoal
           DEF ExactDecisionTargetNeutralDeadline
      <3>2. (/\ ExactDecisionTargetNeutralResidual(
                    "RequestHead", node, qc, archive,
                    request, NoAsyncItem, packet)
                /\ packet.deadline <= asyncNow)
               ~>
             ExactDecisionTargetNeutralGoal(
               "RequestHead", node, qc, archive,
               request, NoAsyncItem, packet)
        BY <1>1, ExactDecisionTargetNeutralDueHeadReachesReadyGoal
      <3> QED BY <3>1, <3>2, PTL
           DEF ExactDecisionTargetNeutralResidual,
               ExactDecisionTargetNeutralGoal
    <2> QED BY <2>1
  <1> QED BY <1>1
       DEF ExactDecisionRequestHeadGateOwnerConvergenceProperty

THEOREM ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergence ==
  \A initialContext:
    ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext, AsyncSpecAt(initialContext)
         PROVE \A node, qc, archive, request, response, packet:
                  ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual(
                    node, qc, archive, request, response, packet)
                    ~> (ExactDecisionResponseAdmissionGoal(node, qc)
                         \/ ExactDecisionResponsePacketAdmissionReady(
                              node, qc, archive, request,
                              response, packet))
    <2>1. ASSUME NEW node, NEW qc, NEW archive,
                  NEW request, NEW response, NEW packet
           PROVE
             ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual(
               node, qc, archive, request, response, packet)
               ~> (ExactDecisionResponseAdmissionGoal(node, qc)
                    \/ ExactDecisionResponsePacketAdmissionReady(
                         node, qc, archive, request, response, packet))
      <3>1. ExactDecisionTargetNeutralResidual(
               "ResponseHead", node, qc, archive,
               request, response, packet)
               ~>
             (ExactDecisionTargetNeutralGoal(
                "ResponseHead", node, qc, archive,
                request, response, packet)
              \/ packet.deadline <= asyncNow)
        BY <1>1,
           ExactDecisionTargetNeutralResidualReachesDeadlineOrGoal
           DEF ExactDecisionTargetNeutralDeadline
      <3>2. (/\ ExactDecisionTargetNeutralResidual(
                    "ResponseHead", node, qc, archive,
                    request, response, packet)
                /\ packet.deadline <= asyncNow)
               ~>
             ExactDecisionTargetNeutralGoal(
               "ResponseHead", node, qc, archive,
               request, response, packet)
        BY <1>1, ExactDecisionTargetNeutralDueHeadReachesReadyGoal
      <3> QED BY <3>1, <3>2, PTL
           DEF ExactDecisionTargetNeutralResidual,
               ExactDecisionTargetNeutralGoal
    <2> QED BY <2>1
  <1> QED BY <1>1
       DEF
         ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty

ExactDecisionOffSchedulerResidualConvergenceProperty(specification) ==
  /\ ExactDecisionRequestClockOwnerConvergenceProperty(specification)
  /\ ExactDecisionRequestRuntimePrefixConvergenceProperty(specification)
  /\ ExactDecisionRequestHeadGateOwnerConvergenceProperty(specification)
  /\ ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty(
       specification)
  /\ ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty(
       specification)

THEOREM ExactDecisionOffSchedulerResidualsDischargeKernels ==
  \A initialContext:
    ExactDecisionOffSchedulerResidualConvergenceProperty(
      AsyncSpecAt(initialContext))
      => /\ ExactDecisionRequestPacketEmissionKernelProperty(
               AsyncSpecAt(initialContext))
         /\ ExactDecisionRequestIngressKernelProperty(
              AsyncSpecAt(initialContext))
         /\ ExactDecisionServeResponseKernelProperty(
              AsyncSpecAt(initialContext))
         /\ ExactDecisionResponseAdmissionKernelProperty(
              AsyncSpecAt(initialContext))
BY ExactDecisionRequestEmissionKernelsDischargeResidual,
   ExactDecisionRequestIngressKernelsDischargeResidual,
   ExactDecisionServeResponseResidualConvergence,
   ExactDecisionResponseClaimKernelNarrowsNonPhysicalResidual,
   ExactDecisionResponsePhysicalKernelNarrowsHeadGateResidual,
   ExactDecisionResponseAdmissionKernelsDischargeResidual, PTL
   DEF ExactDecisionOffSchedulerResidualConvergenceProperty,
       ExactDecisionRequestPacketEmissionResidualConvergenceProperty,
       ExactDecisionRequestIngressResidualConvergenceProperty,
       ExactDecisionServeResponseResidualConvergenceProperty,
       ExactDecisionResponseAdmissionResidualConvergenceProperty,
       ExactDecisionRequestPacketEmissionResidual,
       ExactDecisionRequestIngressResidual,
       ExactDecisionServeResponseResidual,
       ExactDecisionResponseAdmissionResidual,
       ExactDecisionRequestPacketEmissionKernelProperty,
       ExactDecisionRequestIngressKernelProperty,
       ExactDecisionServeResponseKernelProperty,
       ExactDecisionResponseAdmissionKernelProperty

THEOREM ExactDecisionResidualKernelsDischargeStageService ==
  \A initialContext:
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ ExactDecisionStage2BusyClosureProperty(
         AsyncSpecAt(initialContext))
    /\ ExactDecisionRequestPacketEmissionKernelProperty(
         AsyncSpecAt(initialContext))
    /\ ExactDecisionRequestIngressKernelProperty(
         AsyncSpecAt(initialContext))
    /\ ExactDecisionServeResponseKernelProperty(
         AsyncSpecAt(initialContext))
    /\ ExactDecisionResponseAdmissionKernelProperty(
         AsyncSpecAt(initialContext))
    => ExactDecisionStageServiceProperty(
         AsyncSpecAt(initialContext))
BY ExactDecisionServiceSourceDecomposition,
   ExactDecisionLocalStage2ClosesPhysicalOwnerExit,
   ExactDecisionPhysicalExitClosesCandidatePipeline,
   ExactDecisionRequestHasResponsiveBodyHoldingAlias,
   ExactRequestPacketAdmissionCreatesIngressOwner,
   NormalExactRequestIngressCreatesFreshServeOwner,
   HistoricalExactRequestIngressCreatesFreshServeOwner,
   ExactServeHeadCreatesAuthenticatedResponsePacket,
   FreshExactResponsePacketAdmissionAcquiresRecipientClaim,
   ExactResponsePacketCoalescingRetainsRouteNeutralClaim,
   ExactResponseIngressDrainAtomicallyRetiresAliasesAndCreatesFetchOwner,
   ExactDecisionFetchHeldBodySchedulesValidation,
   ExactCertifiedFetchStagesBodyAndSchedulesStore,
   DecisionStoreSchedulesValidation,
   DecisionValidationSchedulesApply,
   DecisionApplyCreatesTerminalStage, PTL
   DEF ExactDecisionStage2BusyClosureProperty,
       ExactDecisionRequestPacketEmissionKernelProperty,
       ExactDecisionRequestIngressKernelProperty,
       ExactDecisionServeResponseKernelProperty,
       ExactDecisionResponseAdmissionKernelProperty,
       ExactDecisionExecutableFrontier,
       ExactDecisionStageServiceProperty,
       ExactDecisionCandidatePipelineProperty,
       ExactDecisionCandidateTerminal,
       ExactDecisionActiveRequestOwner,
       ExactDecisionExecutableOwner

THEOREM ExactDecisionOffSchedulerResidualConvergenceDischargesStageService ==
  \A initialContext:
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ ExactDecisionOffSchedulerResidualConvergenceProperty(
         AsyncSpecAt(initialContext))
      => ExactDecisionStageServiceProperty(
           AsyncSpecAt(initialContext))
BY ExactDecisionStage2BusyClosure,
   ExactDecisionOffSchedulerResidualsDischargeKernels,
   ExactDecisionResidualKernelsDischargeStageService

(***************************************************************************
Complete exact residual inventory.

The Stage-2 Busy owner is absent because
`ExactDecisionStage2BusyClosure` discharges it locally.  The four arms below
are the strict ownership predicates used by
`ExactDecisionOffSchedulerResidualConvergenceProperty`; the request arm is
further partitioned into pre-deadline clock-owner and armed Runtime-prefix
kernels, while the request-ingress arm is partitioned into the finite
head/gate-owner prefix and normal-or-historical ingress-runner prefix; its
exact enabled admission/coalescing handoff is proved above.  The response
claim runner, competing exact-claim kernel, finite physical-owner descent, and
enabled response-admission handoff are proved as well; only the non-physical,
non-claim response head/gate prefix remains open.  The Serve arm uses the proved
protected FIFO starvation result and retains only its exact alias/nonce
exit-safety kernel.
The separate diagnostic debt operator records the concrete transport, runner,
and remaining deadline/source-shadow states which the missing off-scheduler
ranks must order.  Archive rotation and physical route are not residuals:
authorization is bound to `AsyncArchiveServerIds` and the exact signed-request
hash, while ingress resource ownership is normalized independently of the
relay route.
***************************************************************************)

ExactDecisionStageServiceResidual ==
  \/ \E node, qc:
       ExactDecisionRequestPacketEmissionResidual(node, qc)
  \/ \E node, qc, archive, request, packet:
       ExactDecisionRequestIngressResidual(
         node, qc, archive, request, packet)
  \/ \E node, qc, archive, request, job:
       ExactDecisionServeResponseResidual(
         node, qc, archive, request, job)
  \/ \E node, qc, archive, request, response, packet:
       ExactDecisionResponseAdmissionResidual(
         node, qc, archive, request, response, packet)

ExactDecisionPhysicalSchedulerDebt ==
  \/ \E packet \in OverdueResponsivePackets:
       ExactDecisionWireTransportResidual(packet)
  \/ \E item \in AsyncNetworkItems:
       ExactDecisionWireRunnerResidual(item)
  \/ \E item \in AsyncNetworkItems:
       ExactDecisionFreshResponsePhysicalCompletionResidual(item)

THEOREM ExactDecisionRequestEmissionGapIsResidual ==
  \A node, qc:
    ExactDecisionRequestPacketEmissionResidual(node, qc)
    => ExactDecisionStageServiceResidual
BY DEF ExactDecisionStageServiceResidual

THEOREM ExactDecisionRequestIngressGapIsResidual ==
  \A node, qc, archive, request, packet:
    ExactDecisionRequestIngressResidual(
      node, qc, archive, request, packet)
      => ExactDecisionStageServiceResidual
BY DEF ExactDecisionStageServiceResidual

THEOREM ExactDecisionServeResponseGapIsResidual ==
  \A node, qc, archive, request, job:
    ExactDecisionServeResponseResidual(
      node, qc, archive, request, job)
      => ExactDecisionStageServiceResidual
BY DEF ExactDecisionStageServiceResidual

THEOREM ExactDecisionResponseAdmissionGapIsResidual ==
  \A node, qc, archive, request, response, packet:
    ExactDecisionResponseAdmissionResidual(
      node, qc, archive, request, response, packet)
      => ExactDecisionStageServiceResidual
BY DEF ExactDecisionStageServiceResidual

THEOREM ExactDecisionBlockedWirePacketIsPhysicalDebt ==
  \A packet \in OverdueResponsivePackets:
    ExactDecisionWireTransportResidual(packet)
      => ExactDecisionPhysicalSchedulerDebt
BY DEF ExactDecisionPhysicalSchedulerDebt

THEOREM ExactDecisionWireRunnerGapIsPhysicalDebt ==
  \A item \in AsyncNetworkItems:
    ExactDecisionWireRunnerResidual(item)
      => ExactDecisionPhysicalSchedulerDebt
BY DEF ExactDecisionPhysicalSchedulerDebt

THEOREM ExactDecisionFreshResponseDebtIsPhysicalDebt ==
  \A item \in AsyncNetworkItems:
    ExactDecisionFreshResponsePhysicalCompletionResidual(item)
      => ExactDecisionPhysicalSchedulerDebt
BY DEF ExactDecisionPhysicalSchedulerDebt

=============================================================================
