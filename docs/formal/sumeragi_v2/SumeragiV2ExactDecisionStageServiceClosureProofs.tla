---- MODULE SumeragiV2ExactDecisionStageServiceClosureProofs ----
EXTENDS SumeragiV2ApplicationCompletionProofs,
        SumeragiV2HeightResetBoundaryClosureProofs

(***************************************************************************
Exact durable-Decision service closure audit.

This module does not restate `ExactDecisionStageServiceProperty` and does not
add a fairness or liveness assumption to `AsyncSpecAt`.  It projects the
already-proved exact Decision source into:

  * one executable exact reducer owner;
  * one active exact certified request; or
  * the terminal application.

For executable owners it specializes the deferred Stage-2 fence to the exact
Decision node.  A durable Decision excludes a same-node pending InstallTC, so
the matching Busy completion is dispatchable without
`AsyncInstallGenerationBudget` for any unrelated validator.  The resulting
local Busy-phase proof is complete in this module.

For active requests it names every concrete ownership handoff: retained
fanout alias, retransmission packet, normal/historical ingress, fresh-nonce
Serve job, authenticated response, response packet, response ingress, the
route-neutral certified-response claim, and the exact FetchCertifiedBody
owner.  The action leaves below establish the identity-preserving handoffs.
The remaining temporal debt is exposed as four strict off-scheduler
residuals:

  * active-request retransmission into an exact packet owner;
  * packet/runner reach into an exact request ingress handoff;
  * Serve FIFO exit into an exact authenticated response packet; and
  * exact response admission through the recipient-local singleton
    authenticated claim and finite normalized physical-completion owner.

These corridors are not discharged by generic packet fairness.  A fresh
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

The generic Stage-2 proof uses `AsyncInstallGenerationBudget` only to turn the
disjunction supplied by `BusyCompletionCandidateIsDispatchable` into the
dispatchable arm.  The exact Decision source supplies that fact locally.
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
       AdmitCausalHead, IngressDrainStep,
       SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
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
       CommandExecutionEnabled, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
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
      AsyncSpecAt(initialContext)
        => ExactDecisionStage2BusyAtPhase(
             node, qc, target, phase)
             ~> ExactDecisionStage2BusyPhaseGoal(
                  node, qc, target, phase)
PROOF
  <1>1. ASSUME NEW initialContext, NEW node, NEW qc, NEW target,
                NEW phase \in ExactDecisionActiveBusyPhaseCarrier
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
    ExactDecisionStage2BusyClosureProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
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
    ExactDecisionStage2BusyClosureProperty(
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
       ProtectedCandidateOwned

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
       LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
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

ExactDecisionServeOccurrenceOwned(archive, request, job) ==
  /\ job \in SequenceSet(asyncIoQueues[archive])
  /\ job.class = "Serve"
  /\ job.candidate.item = request
  /\ job.nonce \in 0..AsyncIoAuxCapacity

ExactDecisionServeJobOwned(
    node, qc, archive, request, job) ==
  /\ ExactDecisionBodyHoldingAlias(node, qc, archive, request)
  /\ ExactDecisionServeOccurrenceOwned(archive, request, job)

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
   DEF AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars, vars

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
       AsyncRecoveryVars, AsyncIoVars, AsyncDeferredVars,
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

THEOREM ExactDecisionPhysicalCompletionSameNodeRunStrictlyProgresses ==
  \A node, qc, archive, request, response, packet:
    \A rank \in ExactDecisionPhysicalCompletionRunnerCarrier:
    /\ ExactDecisionPhysicalCompletionAtRank(
         node, qc, archive, request, response, packet, rank)
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
      <3>1. LocalAdmissionStep(node)
        BY <1>1
           DEF PostGstRunNode, RunNode, RunNodeWork
      <3>2. DrainableIngressTurnReachRank(node)'
                  < DrainableIngressTurnReachRank(node)
        BY <2>1, <3>1, LocalStepDecreasesDrainableIngressTurnReach
      <3>3. TransportCompletionOwnerDebt(response)' =
                  TransportCompletionOwnerDebt(response)
        BY <3>1, Isa
           DEF LocalAdmissionStep, TransportCompletionOwnerDebt,
               TransportCompletionOwnerIndices, IngressLane
      <3> QED BY <1>1, <2>2, <3>2, <3>3, Isa
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
      <3>1. SerializedRuntimeStep(node)
        BY <1>1
           DEF PostGstRunNode, RunNode, RunNodeWork
      <3>2. DrainableIngressTurnReachRank(node)'
                  < DrainableIngressTurnReachRank(node)
        BY <2>1, <3>1, RuntimeStepDecreasesDrainableIngressTurnReach
      <3>3. TransportCompletionOwnerDebt(response)' =
                  TransportCompletionOwnerDebt(response)
        BY <2>1, <3>1, SerializedRuntimeLeavesIngress, Isa
           DEF TransportCompletionOwnerDebt,
               TransportCompletionOwnerIndices, IngressLane
      <3> QED BY <1>1, <2>2, <3>2, <3>3, Isa
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
       RunHistoricalServer, RunHistoricalRecoveryNode,
       AsyncAllVars, AsyncSchedulerVars

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
       AsyncAllVars, AsyncSchedulerVars

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
       AsyncAllVars, AsyncSchedulerVars

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
       AsyncAllVars, AsyncSchedulerVars

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
       AsyncAllVars, AsyncSchedulerVars

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
             IngressLane, AsyncAllVars, AsyncSchedulerVars
    <2>2. CASE AsyncNext
      <3>1. CASE \E other \in AsyncCurrentResponsiveVoters:
                    RunNode(other)
        <4>1. CASE RunNode(node)
          BY <1>1, <2>2, <4>1,
             ExactDecisionPhysicalCompletionSameNodeRunStrictlyProgresses
             DEF ExactDecisionPhysicalCompletionRunnerStepResult,
                 PostGstRunNode,
                 ExactDecisionPhysicalCompletionAtRank
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
          BY <1>1, <2>2, <4>1,
             ExactDecisionPhysicalCompletionSameNodeRunStrictlyProgresses
             DEF ExactDecisionPhysicalCompletionRunnerStepResult,
                 PostGstRunNode,
                 ExactDecisionPhysicalCompletionAtRank
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
    ExactDecisionPhysicalCompletionAtRank(
      node, qc, archive, request, response, packet, rank)
      => ENABLED <<PostGstRunNode(node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW node, NEW qc, NEW archive, NEW request,
                NEW response, NEW packet, NEW rank,
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
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>2. ExactDecisionPhysicalCompletionAtRank(
             node, qc, archive, request, response, packet, rank)
             /\ ~ExactDecisionPhysicalCompletionRunnerProgress(
                  node, qc, archive, request, response, packet, rank)
            => ENABLED <<PostGstRunNode(node)>>_AsyncAllVars
      BY ExactDecisionPhysicalCompletionEnablesFairRunNode
    <2>3. ExactDecisionPhysicalCompletionAtRank(
             node, qc, archive, request, response, packet, rank)
             /\ ~ExactDecisionPhysicalCompletionRunnerProgress(
                  node, qc, archive, request, response, packet, rank)
             /\ <<PostGstRunNode(node)>>_AsyncAllVars
            => ExactDecisionPhysicalCompletionRunnerProgress(
                 node, qc, archive, request, response, packet, rank)'
      BY ExactDecisionPhysicalCompletionSameNodeRunStrictlyProgresses
         DEF ExactDecisionPhysicalCompletionRunnerStrictResult
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
                     ~> ExactDecisionPhysicalCompletionRunnerProgress(
                          node, qc, archive, request, response,
                          packet, rank))
        BY <2>2, <2>3, <2>4, <3>1, PTL DEF AsyncSpecAt
      <3> QED BY <3>2
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
   DEF AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars, vars

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
       LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
       IngressDrainStep, DrainFairIngressSelected,
       DrainHistoricalIngressSelected,
       SerializedRuntimeStep, RuntimeStep,
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

`DirectRetransmitStep` publishes retryable items only while the Core node is
idle; `DeferredRetransmitStep` already carries that same idle guard.  An exact
active Decision request supplies a responsive body-holding alias in
`ActiveRequestItems(node)`, hence in `RetryableItems(node)`.  Either sending
branch therefore appends the packet for that exact retained alias.  This is an
action-local theorem: it deliberately does not claim that the retransmission
deadline becomes due or that the aggregate fair node runner eventually selects
the sending branch.
***************************************************************************)

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

ExactDecisionRequestRetransmitArmedResidual(node, qc) ==
  /\ ExactDecisionRequestPacketEmissionResidual(node, qc)
  /\ \/ RetransmitDue(node)
     \/ "RetransmitElapsed" \in asyncOutstandingTags[node]

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
  ExactDecisionRequestIngressLaneRunnerConvergenceProperty(
    specification)

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
   ExactDecisionRequestIngressResidualSplitsAtAdmissionReady, PTL
   DEF ExactDecisionRequestHeadGateOwnerConvergenceProperty,
       ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty,
       ExactDecisionRequestAdmissionHandoffConvergenceProperty,
       ExactDecisionRequestIngressLaneRunnerConvergenceProperty,
       ExactDecisionRequestIngressResidualConvergenceProperty,
       ExactDecisionRequestHeadGateOwnerResidual,
       ExactDecisionRequestAdmissionOutcome

(***************************************************************************
Protected Serve FIFO closure under the actual exact-corridor specification.

The generic Serve FIFO rank leaf is already proved under `AsyncSpecAt`; it
does not consume the install-generation budget needed by unrelated Stage-2
candidate work.  The older aggregate starvation bridge was stated only for
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
    ExactDecisionOffSchedulerResidualConvergenceProperty(
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
