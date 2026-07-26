---- MODULE SumeragiV2ProgressWitnessFinalClosureProofs ----
EXTENDS SumeragiV2HistoricalLockedBodyWitnessPreservationProofs

(***************************************************************************
Final progress-witness preservation closure.

This module is deliberately above the exact Decision and historical
locked-body lineage leaves.  That ordering lets the final induction consume
both strengthened source invariants without feeding either one back through
the lower dependency chain.

The authenticated CertifiedResponse authority used below is route-neutral:
the response retains the exact signed-request hash and authenticated sent
occurrence, its archive server owns the response signature, and its cited
responder belongs to the frozen QC signer set.  Request recipients remain a
routing concern in the imported request-owner predicate; no theorem below
uses a response archive server's membership in the request route set.

No production action is redefined.  FinalWitnessMonotoneCarrierFrame is a
proof-only summary of append-only authentication/request history and
carrier-set preservation for actions which do not execute a selected owner.
***************************************************************************)

FinalWitnessSourceRetentionInvariant ==
  /\ DecisionExactSourceRetentionInvariant
  /\ HistoricalLockedBodyLineageSourceRetentionInvariant

FinalProgressWitnessClosureInvariant ==
  /\ FinalWitnessSourceRetentionInvariant
  /\ ResponsiveReplayLockedBodyLineagedCarrierInvariant

(***************************************************************************
Projection to the lower open kernel.

The lineaged historical stage supplies both lower historical obligations.
In the validated/no-higher-conflict partition its lineaged Commit owner
projects to the exact lower HistoricalLockedCommitRecoveryWitness.  In every
other partition the existing lineaged-stage projection supplies the lower
locked-body stage directly.  The exact Decision source invariant supplies the
remaining open-kernel conjunct.
***************************************************************************)

THEOREM HistoricalLineageProjectsOpenHistoricalKernel ==
  HistoricalLockedBodyLineageSourceRetentionInvariant
    => /\ HistoricalLockedCommitRecoveryProgress
       /\ HistoricalLockedBodyRecoveryStageInvariant
BY LineagedInvariantProjectsReleaseInvariant,
   LineagedCommitProjectsReleaseWitness, IsaT(180)
   DEF HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedCommitRecoveryProgress,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource

THEOREM FinalWitnessSourceProjectsOpenProgressKernel ==
  FinalWitnessSourceRetentionInvariant
    => OpenProgressWitnessKernelInvariant
BY HistoricalLineageProjectsOpenHistoricalKernel,
   ExactDecisionSourceRetentionProjectsAsyncWitness, Isa
   DEF FinalWitnessSourceRetentionInvariant,
       OpenProgressWitnessKernelInvariant

THEOREM FinalClosureProjectsOpenProgressKernel ==
  FinalProgressWitnessClosureInvariant
    => OpenProgressWitnessKernelInvariant
BY FinalWitnessSourceProjectsOpenProgressKernel
   DEF FinalProgressWitnessClosureInvariant

(***************************************************************************
Base case.
***************************************************************************)

THEOREM AsyncInitEstablishesResponsiveReplayLineagedCarrier ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => ResponsiveReplayLockedBodyLineagedCarrierInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt,
       ResponsiveReplayLockedBodyLineagedCarrierInvariant

THEOREM AsyncInitEstablishesFinalProgressWitnessClosure ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => FinalProgressWitnessClosureInvariant
BY AsyncInitEstablishesDecisionExactSourceRetention,
   AsyncInitEstablishesHistoricalLockedBodyLineageSourceRetention,
   AsyncInitEstablishesResponsiveReplayLineagedCarrier
   DEF FinalProgressWitnessClosureInvariant,
       FinalWitnessSourceRetentionInvariant

(***************************************************************************
Carrier-neutral monotone frame.

OpenProgressWitnessCarrierFrame fixes every semantic Core/recovery component
read by either strengthened stage, retains active requests and authenticated
sent occurrences monotonically, and retains the union of every scheduler
carrier.  The explicit inclusion for the union of responsive voters and
historical recovery targets permits either source class to shrink while
preventing a new quantified Decision source from appearing.  Opening a new
historical target is proved separately from this monotone frame.
***************************************************************************)

FinalWitnessMonotoneCarrierFrame ==
  /\ OpenProgressWitnessCarrierFrame
  /\ (AsyncCurrentResponsiveVoters'
        \cup asyncHistoricalRecoveryTargets')
       \subseteq
         (AsyncCurrentResponsiveVoters
            \cup asyncHistoricalRecoveryTargets)

THEOREM FinalMonotoneCarrierFrameEstablishesDecisionExactFrame ==
  FinalWitnessMonotoneCarrierFrame
    => DecisionExactRetentionFrame
BY IsaT(120)
   DEF FinalWitnessMonotoneCarrierFrame,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars,
       ScheduledCandidateSet,
       DecisionExactRetentionFrame,
       DecisionExactAuthenticatedHistoryRetained,
       DecisionExactCertifiedRequestsRetained,
       DecisionExactScheduledCandidatesRetained,
       DecisionCertifiedRequestActiveExact,
       DecisionExecutableStageOwner,
       CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet

THEOREM FinalMonotoneCarrierFrameEstablishesHistoricalLineageFrame ==
  FinalWitnessMonotoneCarrierFrame
    => HistoricalLockedBodyLineageRetentionFrame
BY IsaT(150)
   DEF FinalWitnessMonotoneCarrierFrame,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars,
       ScheduledCandidateSet,
       HistoricalLockedBodyLineageRetentionFrame,
       HistoricalLockedBodyLineageSemanticVars,
       HistoricalLockedAuthenticatedHistoryRetained,
       HistoricalLockedLineagedRequestsRetained,
       HistoricalLockedLineagedCandidatesRetained,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet

THEOREM HistoricalLineageFramePreservesResponsiveReplayCarrier ==
  /\ ResponsiveReplayLockedBodyLineagedCarrierInvariant
  /\ HistoricalLockedBodyLineageRetentionFrame
  => ResponsiveReplayLockedBodyLineagedCarrierInvariant'
BY IsaT(240)
   DEF ResponsiveReplayLockedBodyLineagedCarrierInvariant,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLockedBodyFetchOwned,
       HistoricalLockedBodyCertifiedFetchOwned,
       HistoricalLockedBodyStoreOwned,
       HistoricalLockedBodyValidateOwned,
       HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, RestartLockedPrepareQCs,
       HistoricalLockedBodyLineageRetentionFrame,
       HistoricalLockedBodyLineageSemanticVars,
       HistoricalLockedAuthenticatedHistoryRetained,
       HistoricalLockedLineagedRequestsRetained,
       HistoricalLockedLineagedCandidatesRetained,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       CertifiedRequestOutbox, CandidateScheduled,
       CandidateConsumerCurrent

THEOREM FinalMonotoneCarrierFramePreservesClosure ==
  /\ FinalProgressWitnessClosureInvariant
  /\ FinalWitnessMonotoneCarrierFrame
  => FinalProgressWitnessClosureInvariant'
BY FinalMonotoneCarrierFrameEstablishesDecisionExactFrame,
   FinalMonotoneCarrierFrameEstablishesHistoricalLineageFrame,
   DecisionExactRetentionFramePreservesSource,
   HistoricalLockedBodyLineageFramePreservesSourceRetention,
   HistoricalLineageFramePreservesResponsiveReplayCarrier
   DEF FinalProgressWitnessClosureInvariant,
       FinalWitnessSourceRetentionInvariant

(***************************************************************************
Ordinary non-runner actions.

The inner worker/publication/network actions do not bind the recovery
controller.  Their aggregate theorem therefore requires the exact outer
UNCHANGED AsyncRecoveryVars frame supplied by AsyncNonCrashStep.
***************************************************************************)

THEOREM AsyncSetGstEstablishesFinalMonotoneCarrierFrame ==
  AsyncSetGST => FinalWitnessMonotoneCarrierFrame
BY Isa
   DEF FinalWitnessMonotoneCarrierFrame,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       AsyncSetGST, SetGST, AsyncSchedulerVars, AsyncRecoveryVars,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, vars

THEOREM AsyncTickEstablishesFinalMonotoneCarrierFrame ==
  AsyncTick => FinalWitnessMonotoneCarrierFrame
BY Isa
   DEF FinalWitnessMonotoneCarrierFrame,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       AsyncTick, AsyncNonClockVars, AsyncIoVars, AsyncDeferredVars,
       AsyncLocalAdmissionVars, AsyncRecoveryVars,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, vars

THEOREM StrongDecisionRecordsAreCommit ==
  AsyncStrongTypeInvariant
    => \A decision \in decisions:
         decision.qc.phase = "Commit"
BY Isa
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, DecisionAgreement,
       LineageInvariant, CertificatePhasesCorrect

(***************************************************************************
Opening historical recovery is the sole non-runner action which grows the
Decision source-owner set.  Its new owner is decisionless by construction.
Every current-context durable Decision is Commit-only under the strong
inductive invariant, so the `~NodeHasDecision(node)` guard excludes a
Decision at the newly added target.  All pre-existing owners retain their
exact stage because the action frames Core, recovery, authentication history,
active requests, and every scheduler carrier except the target set itself.
***************************************************************************)

THEOREM OpenHistoricalRecoveryPreservesDecisionExactSource ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ DecisionExactSourceRetentionInvariant
    /\ OpenHistoricalRecovery(node)
    => DecisionExactSourceRetentionInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                DecisionExactSourceRetentionInvariant,
                OpenHistoricalRecovery(node)
         PROVE DecisionExactSourceRetentionInvariant'
    <2>1. \A decision \in decisions:
             decision.qc.phase = "Commit"
      BY <1>1, StrongDecisionRecordsAreCommit
    <2>2. /\ ~NodeHasDecision(node)
           /\ decisions' = decisions
           /\ context' = context
           /\ asyncHistoricalRecoveryTargets' =
                asyncHistoricalRecoveryTargets \cup {node}
      BY <1>1
         DEF OpenHistoricalRecovery, HistoricalRecoverySourceReady,
             vars
    <2>3. ASSUME NEW decision \in decisions',
                  /\ DecisionExactSourceOwner(decision.node)'
                     /\ decision.qc.context = context'
           PROVE AsyncDecisionRecoveryStageExact(
                   decision.node, decision.qc)'
      <3>1. /\ decision \in decisions
             /\ decision.qc.context = context
             /\ decision.qc.phase = "Commit"
        BY <2>1, <2>2, <2>3
      <3>2. CASE decision.node = node
        BY <2>2, <3>1, <3>2 DEF NodeHasDecision
      <3>3. CASE decision.node # node
        <4>1. DecisionExactSourceOwner(decision.node)
          BY <1>1, <2>2, <2>3, <3>3, Isa
             DEF DecisionExactSourceOwner, HistoricalRecoveryTarget,
                 OpenHistoricalRecovery,
                 AsyncCurrentResponsiveVoters,
                 CurrentVoters, CurrentEpoch, vars
        <4>2. AsyncDecisionRecoveryStageExact(
                 decision.node, decision.qc)
          BY <1>1, <3>1, <4>1
             DEF DecisionExactSourceRetentionInvariant
        <4> QED BY <1>1, <3>1, <4>2, IsaT(300)
             DEF AsyncDecisionRecoveryStageExact,
                 DecisionRecoveryStageExact,
                 DecisionFetchBodyOwnedExact,
                 DecisionCertifiedRequestActiveExact,
                 DecisionCertifiedResponseLineageExact,
                 DecisionCertifiedFetchOwnedExact,
                 DecisionStoreBodyOwned, DecisionValidateBodyOwned,
                 DecisionApplyOwned, DecisionPipelineKindOwned,
                 DecisionPipelineCandidate,
                 DecisionValidationHeld, DecisionBody,
                 DecisionRecoveryAuthority,
                 DurableDecisionRecoveryAuthority,
                 DurableDecisionRecoveryExecutorCurrent,
                 CertifiedResponseAuthenticatedOccurrence,
                 CertifiedResponseCapabilityAuthorized,
                 MatchingSentCertifiedRequests,
                 FrozenCertifiedResponseBinding,
                 FrozenCertifiedRequestRegistration,
                 AsyncCertifiedResponseAuthProjection,
                 CandidateConsumerCurrent, CandidateScheduled,
                 OpenHistoricalRecovery,
                 AsyncSchedulerExceptHistoricalRecoveryTargets,
                 QueuedCandidates, DeferredCandidates, CausalCandidates,
                 TrackedWorkCandidates, SequenceSet, vars
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>3 DEF DecisionExactSourceRetentionInvariant
  <1> QED BY <1>1

THEOREM OpenHistoricalRecoveryEstablishesHistoricalLineageFrame ==
  \A node \in ValidatorIds:
    OpenHistoricalRecovery(node)
      => HistoricalLockedBodyLineageRetentionFrame
BY IsaT(150)
   DEF HistoricalLockedBodyLineageRetentionFrame,
       HistoricalLockedBodyLineageSemanticVars,
       HistoricalLockedAuthenticatedHistoryRetained,
       HistoricalLockedLineagedRequestsRetained,
       HistoricalLockedLineagedCandidatesRetained,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       CandidateScheduled,
       OpenHistoricalRecovery, AsyncSchedulerExceptHistoricalRecoveryTargets,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, vars

THEOREM OpenHistoricalRecoveryPreservesFinalProgressWitnessClosure ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ OpenHistoricalRecovery(node)
    => FinalProgressWitnessClosureInvariant'
BY OpenHistoricalRecoveryPreservesDecisionExactSource,
   OpenHistoricalRecoveryEstablishesHistoricalLineageFrame,
   HistoricalLockedBodyLineageFramePreservesSourceRetention,
   HistoricalLineageFramePreservesResponsiveReplayCarrier
   DEF FinalProgressWitnessClosureInvariant,
       FinalWitnessSourceRetentionInvariant

THEOREM CommitDiscoveryEstablishesFinalMonotoneCarrierFrame ==
  \A node \in ValidatorIds:
    /\ CommitCertificateDiscoveryStepWork(node)
    /\ UNCHANGED AsyncRecoveryVars
    => FinalWitnessMonotoneCarrierFrame
BY IsaT(120)
   DEF FinalWitnessMonotoneCarrierFrame,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       CommitCertificateDiscoveryStepWork,
       PublishCommitCertificateRequests,
       CommitCertificateRequestOutbox,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, vars

THEOREM ServiceIoEstablishesFinalMonotoneCarrierFrame ==
  \A node \in ValidatorIds:
    /\ ServiceIoWorkerWork(node)
    /\ UNCHANGED AsyncRecoveryVars
    => FinalWitnessMonotoneCarrierFrame
BY IsaT(150)
   DEF FinalWitnessMonotoneCarrierFrame,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       ServiceIoWorkerWork, PublishEphemeralItems,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, vars

THEOREM EnqueueIoControlEstablishesFinalMonotoneCarrierFrame ==
  \A node \in ValidatorIds:
    /\ EnqueueIoLocalControlWork(node)
    /\ UNCHANGED AsyncRecoveryVars
    => FinalWitnessMonotoneCarrierFrame
BY Isa
   DEF FinalWitnessMonotoneCarrierFrame,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       EnqueueIoLocalControlWork,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, vars

THEOREM AdmitIngressEstablishesFinalMonotoneCarrierFrame ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AdmitIngressPacket(recipient, source)
    /\ UNCHANGED AsyncRecoveryVars
    => FinalWitnessMonotoneCarrierFrame
BY Isa
   DEF FinalWitnessMonotoneCarrierFrame,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       AsyncIoVars, AsyncDeferredVars, LeaveCausalQueues,
       AsyncLocalAdmissionVars,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, vars

THEOREM TransportFaultEstablishesFinalMonotoneCarrierFrame ==
  /\ TransportOnlyProgressWitnessStep
  /\ UNCHANGED AsyncRecoveryVars
  => FinalWitnessMonotoneCarrierFrame
BY IsaT(180)
   DEF FinalWitnessMonotoneCarrierFrame,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       TransportOnlyProgressWitnessStep,
       PreGstLosePacket, InjectByzantineNoise,
       InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk, InjectByzantineCertifiedRequest,
       AsyncByzantineProposal, AsyncByzantineVote,
       AsyncByzantineTimeout, ByzantineBroadcastProposal,
       ByzantineBroadcastVote, ByzantineBroadcastTimeout,
       PublishEphemeralItems, PacketsForItems, NoSendItem,
       AsyncIoVars, AsyncDeferredVars, LeaveCausalQueues,
       AsyncLocalAdmissionVars, AsyncAuxVars,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, vars

THEOREM NonResponsiveCrashPreservesFinalProgressWitnessClosure ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ PreGstCrash(node)
    => FinalProgressWitnessClosureInvariant'
BY IsaT(420)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       FinalProgressWitnessClosureInvariant,
       FinalWitnessSourceRetentionInvariant,
       DecisionExactSourceRetentionInvariant,
       DecisionExactSourceOwner, HistoricalRecoveryTarget,
       AsyncDecisionRecoveryStageExact,
       DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedResponseLineageExact,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyRecoveryAuthority,
       ResponsiveReplayLockedBodyLineagedCarrierInvariant,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       RestartLockedPrepareQCs,
       PreGstCrash, Crash, AsyncSchedulerVars,
       AsyncRecoveryVars, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, vars

THEOREM AsyncFaultPreservesFinalProgressWitnessClosure ==
  /\ AsyncStrongTypeInvariant
  /\ FinalProgressWitnessClosureInvariant
  /\ AsyncFaultStep
  /\ UNCHANGED AsyncRecoveryVars
  => FinalProgressWitnessClosureInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              FinalProgressWitnessClosureInvariant,
              AsyncFaultStep,
              UNCHANGED AsyncRecoveryVars
         PROVE FinalProgressWitnessClosureInvariant'
    <2>1. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <1>1, <2>1,
         NonResponsiveCrashPreservesFinalProgressWitnessClosure
    <2>2. CASE TransportOnlyProgressWitnessStep
      BY <1>1, <2>2,
         TransportFaultEstablishesFinalMonotoneCarrierFrame,
         FinalMonotoneCarrierFramePreservesClosure
    <2> QED BY <1>1, <2>1, <2>2
         DEF AsyncFaultStep, TransportOnlyProgressWitnessStep
  <1> QED BY <1>1

THEOREM AsyncNonRunnerPreservesFinalProgressWitnessClosure ==
  /\ AsyncStrongTypeInvariant
  /\ FinalProgressWitnessClosureInvariant
  /\ AsyncNonRunnerStep
  /\ UNCHANGED AsyncRecoveryVars
  => FinalProgressWitnessClosureInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              FinalProgressWitnessClosureInvariant,
              AsyncNonRunnerStep,
              UNCHANGED AsyncRecoveryVars
         PROVE FinalProgressWitnessClosureInvariant'
    <2>1. CASE AsyncSetGST
      BY <1>1, <2>1,
         AsyncSetGstEstablishesFinalMonotoneCarrierFrame,
         FinalMonotoneCarrierFramePreservesClosure
    <2>2. CASE AsyncTick
      BY <1>1, <2>2,
         AsyncTickEstablishesFinalMonotoneCarrierFrame,
         FinalMonotoneCarrierFramePreservesClosure
    <2>3. CASE \E node \in ValidatorIds: OpenHistoricalRecovery(node)
      BY <1>1, <2>3,
         OpenHistoricalRecoveryPreservesFinalProgressWitnessClosure
    <2>4. CASE \E node \in ValidatorIds:
                    DirectCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>4,
         CommitDiscoveryEstablishesFinalMonotoneCarrierFrame,
         FinalMonotoneCarrierFramePreservesClosure
         DEF DirectCommitCertificateDiscoveryStep
    <2>5. CASE \E node \in asyncHistoricalRecoveryTargets:
                    DirectHistoricalCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>5,
         CommitDiscoveryEstablishesFinalMonotoneCarrierFrame,
         FinalMonotoneCarrierFramePreservesClosure
         DEF DirectHistoricalCommitCertificateDiscoveryStep
    <2>6. CASE \E node \in ValidatorIds:
                    ServiceIoWorker(node)
      BY <1>1, <2>6,
         ServiceIoEstablishesFinalMonotoneCarrierFrame,
         FinalMonotoneCarrierFramePreservesClosure
         DEF ServiceIoWorker
    <2>7. CASE \E node \in asyncHistoricalRecoveryTargets:
                    ServiceHistoricalRecoveryIoWorker(node)
      BY <1>1, <2>7,
         ServiceIoEstablishesFinalMonotoneCarrierFrame,
         FinalMonotoneCarrierFramePreservesClosure
         DEF ServiceHistoricalRecoveryIoWorker
    <2>8. CASE \E node \in ValidatorIds:
                    EnqueueIoLocalControl(node)
      BY <1>1, <2>8,
         EnqueueIoControlEstablishesFinalMonotoneCarrierFrame,
         FinalMonotoneCarrierFramePreservesClosure
         DEF EnqueueIoLocalControl
    <2>9. CASE \E node \in asyncHistoricalRecoveryTargets:
                    EnqueueHistoricalRecoveryIoLocalControl(node)
      BY <1>1, <2>9,
         EnqueueIoControlEstablishesFinalMonotoneCarrierFrame,
         FinalMonotoneCarrierFramePreservesClosure
         DEF EnqueueHistoricalRecoveryIoLocalControl
    <2>10. CASE AsyncNetworkStep
      BY <1>1, <2>10,
         AdmitIngressEstablishesFinalMonotoneCarrierFrame,
         FinalMonotoneCarrierFramePreservesClosure
         DEF AsyncNetworkStep
    <2>11. CASE AsyncFaultStep
      BY <1>1, <2>11,
         AsyncFaultPreservesFinalProgressWitnessClosure
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11
         DEF AsyncNonRunnerStep
  <1> QED BY <1>1

(***************************************************************************
Replaying owner-moving runner actions.

Unlike the source-retention invariant, the scoped replay carrier may not use
the Replaying authority disjunct.  The following three theorems therefore
repeat the exact carrier handoff analysis at the stronger no-authority
boundary.  Local admission preserves the scheduled set, ingress replaces one
exact request with its authenticated FetchCertifiedBody owner, and serialized
dispatch uses the already-proved exact semantic handoffs and successor
scheduling facts.
***************************************************************************)

THEOREM LocalAdmissionPreservesResponsiveReplayLineagedCarrier ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ResponsiveReplayLockedBodyLineagedCarrierInvariant
    /\ LocalAdmissionStep(node)
    /\ UNCHANGED AsyncRecoveryVars
    => ResponsiveReplayLockedBodyLineagedCarrierInvariant'
BY LocalAdmissionPreservesScheduledCandidateSet,
   HistoricalLineageFramePreservesResponsiveReplayCarrier,
   IsaT(120)
   DEF HistoricalLockedBodyLineageRetentionFrame,
       HistoricalLockedBodyLineageSemanticVars,
       HistoricalLockedAuthenticatedHistoryRetained,
       HistoricalLockedLineagedRequestsRetained,
       HistoricalLockedLineagedCandidatesRetained,
       LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
       LeaveCausalQueues, AsyncIoVars, AsyncDeferredVars,
       AsyncLocalAdmissionVars, vars

THEOREM IngressDrainPreservesResponsiveReplayLineagedCarrier ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ResponsiveReplayLockedBodyLineagedCarrierInvariant
    /\ IngressDrainStep(node)
    /\ UNCHANGED AsyncRecoveryVars
    => ResponsiveReplayLockedBodyLineagedCarrierInvariant'
BY AuthorizedLineagedResponseCandidateCarriesExactEvidence,
   SequenceWithoutIndexRetainsOtherValue,
   SequenceSetAfterAppend, IsaT(420)
   DEF ResponsiveReplayLockedBodyLineagedCarrierInvariant,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLockedBodyFetchOwned,
       HistoricalLockedBodyCertifiedFetchOwned,
       HistoricalLockedBodyStoreOwned,
       HistoricalLockedBodyValidateOwned,
       HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, RestartLockedPrepareQCs,
       DrainFairIngressSelected, PopSelectedIngress,
       IngressItemCanDrain, DeliveryCandidate,
       CertifiedResponseAuthorized, MatchingCertifiedRequests,
       CertifiedRequestAuthorized,
       CertifiedBodyRecoveryAuthority,
       CertifiedResponseCandidate, CertifiedRequestOutbox,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       AsyncCertifiedRequestHash, AsyncCertifiedRequestHashOf,
       AsyncCandidate, AsyncCandidateWithIdentity,
       EnqueueCandidate, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       IngressDrainStep, AsyncIoVars, AsyncRecoveryVars,
       SequenceSet, vars

THEOREM SerializedRuntimePreservesResponsiveReplayLineagedCarrier ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ResponsiveReplayLockedBodyLineagedCarrierInvariant
    /\ SerializedRuntimeStep(node)
    /\ UNCHANGED AsyncRecoveryVars
    => ResponsiveReplayLockedBodyLineagedCarrierInvariant'
BY HistoricalLockedBodyExecutableCandidateIsDispatchable,
   FifoSuccessfulExecutionSchedulesEverySuccessor,
   DeferredSuccessfulExecutionSchedulesEverySuccessor,
   HistoricalLockedFetchMissingBodyOpensLineagedRequest,
   HistoricalLockedFetchHeldBodySchedulesLineagedValidation,
   HistoricalCertifiedFetchStagesBodyAndSchedulesLineagedStore,
   HistoricalStoreSchedulesLineagedValidation,
   HistoricalValidationSchedulesLineagedBeginLockOrTerminal,
   LineagedHistoricalBeginLockExecutionCreatesSameRefPending,
   HistoricalPersistLockCommitCreatesExactCommitWitness,
   PersistInstallEstablishesTargetLineagedStage,
   CertifiedRequestOutboxDecisionSurvivalIsExactTarget,
   PersistDecisionControlRetainsExactlySurvivingRequests,
   SequenceWithoutIndexRetainsOtherValue,
   TailRetainsNonHeadValue, SequenceSetAfterAppend, IsaT(1200)
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
       ResponsiveReplayLockedBodyLineagedCarrierInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLockedBodyFetchOwned,
       HistoricalLockedBodyCertifiedFetchOwned,
       HistoricalLockedBodyStoreOwned,
       HistoricalLockedBodyValidateOwned,
       HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedBodyScheduledCandidate,
       HistoricalLockedBodyExecutableCandidate,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, RestartLockedPrepareQCs,
       CommandSuccessorsScheduledAfter,
       SerializedRuntimeStep, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DeferredTimeoutStep,
       DeferredRetransmitStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       DeferCommand, DiscardCommand,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       ExecuteDecisionFetch, ExecutePersistInstall,
       ExecutePersistDecision, PersistDecisionControl,
       CertifiedRequestSurvivesDecision,
       FilterCertifiedResponseAuthority, ExecuteApply,
       AppendCausalSuccessors, AppendHistoricalLockedRetransmitSuccessors,
       HistoricalLockedRetransmitSuccessors,
       FreshCommandSuccessors, FreshCandidateSequence,
       CommandSuccessors, CandidateScheduled, CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncRecoveryVars, SequenceSet, vars

THEOREM RunNodeWorkPreservesResponsiveReplayLineagedCarrier ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ResponsiveReplayLockedBodyLineagedCarrierInvariant
    /\ RunNodeWork(node)
    /\ UNCHANGED AsyncRecoveryVars
    => ResponsiveReplayLockedBodyLineagedCarrierInvariant'
BY LocalAdmissionPreservesResponsiveReplayLineagedCarrier,
   IngressDrainPreservesResponsiveReplayLineagedCarrier,
   SerializedRuntimePreservesResponsiveReplayLineagedCarrier, Isa
   DEF RunNodeWork

THEOREM RunHistoricalServerEstablishesFinalMonotoneCarrierFrame ==
  \A node \in ValidatorIds:
    /\ RunHistoricalServer(node)
    /\ UNCHANGED AsyncRecoveryVars
    => FinalWitnessMonotoneCarrierFrame
BY IsaT(120)
   DEF FinalWitnessMonotoneCarrierFrame,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       RunHistoricalServer, DrainHistoricalIngressSelected,
       HistoricalIdleStep, PopSelectedIngress,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, vars

THEOREM AsyncRunnerPreservesHistoricalLineageSource ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalLockedBodyLineageSourceRetentionInvariant
  /\ AsyncRunnerStep
  /\ UNCHANGED AsyncRecoveryVars
  => HistoricalLockedBodyLineageSourceRetentionInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              HistoricalLockedBodyLineageSourceRetentionInvariant,
              AsyncRunnerStep,
              UNCHANGED AsyncRecoveryVars
         PROVE HistoricalLockedBodyLineageSourceRetentionInvariant'
    <2>1. CASE \E node \in ValidatorIds: RunNode(node)
      BY <1>1, <2>1,
         RunNodePreservesHistoricalLockedBodyLineageSourceRetention
    <2>2. CASE \E node \in asyncHistoricalRecoveryTargets:
                    RunHistoricalRecoveryNode(node)
      BY <1>1, <2>2,
         RunNodeWorkPreservesHistoricalLockedBodyLineageSourceRetention
         DEF RunHistoricalRecoveryNode
    <2>3. CASE \E node \in ValidatorIds:
                    RunHistoricalServer(node)
      BY <1>1, <2>3,
         RunHistoricalServerEstablishesFinalMonotoneCarrierFrame,
         FinalMonotoneCarrierFrameEstablishesHistoricalLineageFrame,
         HistoricalLockedBodyLineageFramePreservesSourceRetention
    <2> QED BY <1>1, <2>1, <2>2, <2>3 DEF AsyncRunnerStep
  <1> QED BY <1>1

THEOREM AsyncRunnerPreservesResponsiveReplayLineagedCarrier ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ ResponsiveReplayLockedBodyLineagedCarrierInvariant
  /\ AsyncRunnerStep
  /\ UNCHANGED AsyncRecoveryVars
  => ResponsiveReplayLockedBodyLineagedCarrierInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              ResponsiveReplayLockedBodyLineagedCarrierInvariant,
              AsyncRunnerStep,
              UNCHANGED AsyncRecoveryVars
         PROVE ResponsiveReplayLockedBodyLineagedCarrierInvariant'
    <2>1. CASE \E node \in ValidatorIds: RunNode(node)
      BY <1>1, <2>1,
         RunNodeWorkPreservesResponsiveReplayLineagedCarrier
         DEF RunNode
    <2>2. CASE \E node \in asyncHistoricalRecoveryTargets:
                    RunHistoricalRecoveryNode(node)
      BY <1>1, <2>2,
         RunNodeWorkPreservesResponsiveReplayLineagedCarrier
         DEF RunHistoricalRecoveryNode
    <2>3. CASE \E node \in ValidatorIds:
                    RunHistoricalServer(node)
      BY <1>1, <2>3,
         RunHistoricalServerEstablishesFinalMonotoneCarrierFrame,
         FinalMonotoneCarrierFrameEstablishesHistoricalLineageFrame,
         HistoricalLineageFramePreservesResponsiveReplayCarrier
    <2> QED BY <1>1, <2>1, <2>2, <2>3 DEF AsyncRunnerStep
  <1> QED BY <1>1

THEOREM AsyncRunnerPreservesFinalProgressWitnessClosure ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ FinalProgressWitnessClosureInvariant
  /\ AsyncRunnerStep
  /\ UNCHANGED AsyncRecoveryVars
  => FinalProgressWitnessClosureInvariant'
BY AsyncRunnerPreservesDecisionExactSourceRetention,
   AsyncRunnerPreservesHistoricalLineageSource,
   AsyncRunnerPreservesResponsiveReplayLineagedCarrier
   DEF FinalProgressWitnessClosureInvariant,
       FinalWitnessSourceRetentionInvariant

(***************************************************************************
Responsive recovery lifecycle.
***************************************************************************)

THEOREM ResponsiveCrashPreservesFinalProgressWitnessClosure ==
  \A crashNode \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ FinalProgressWitnessClosureInvariant
    /\ PreGstResponsiveCrash(crashNode)
    => FinalProgressWitnessClosureInvariant'
BY IsaT(360)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       FinalProgressWitnessClosureInvariant,
       FinalWitnessSourceRetentionInvariant,
       DecisionExactSourceRetentionInvariant,
       DecisionExactSourceOwner, HistoricalRecoveryTarget,
       AsyncDecisionRecoveryStageExact,
       DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedResponseLineageExact,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyRecoveryAuthority,
       ResponsiveReplayLockedBodyLineagedCarrierInvariant,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       PreGstResponsiveCrash, Crash

THEOREM ResponsiveRestartPreservesFinalProgressWitnessClosure ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ FinalProgressWitnessClosureInvariant
  /\ PreGstResponsiveRestart
  => FinalProgressWitnessClosureInvariant'
BY ResponsiveRestartAdvancesExactDurableDecisionAuthority, IsaT(420)
   DEF ResponsiveRecoveryValidationClearedInvariant,
       RecoveryNodeValidationCleared,
       AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       FinalProgressWitnessClosureInvariant,
       FinalWitnessSourceRetentionInvariant,
       DecisionExactSourceRetentionInvariant,
       DecisionExactSourceOwner, HistoricalRecoveryTarget,
       AsyncDecisionRecoveryStageExact,
       DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedResponseLineageExact,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyRecoveryAuthority,
       ResponsiveReplayLockedBodyLineagedCarrierInvariant,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       RestartDecisions, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch,
       PreGstResponsiveRestart, Restart

(***************************************************************************
All restart-authorized locked Prepare QCs for one node share one stable
production CertificateRef.  The signer sets may differ, but authenticated QC
validity supplies canonical height=context.height while the recovery-source
predicate fixes context, phase, view, and subject.
***************************************************************************)

THEOREM RestartLockedPrepareSourcesShareRecoveryReference ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    => \A left \in RestartLockedPrepareQCs(node):
         \A right \in RestartLockedPrepareQCs(node):
           SamePrepareRecoveryRef(left, right)
BY SMTT(120)
   DEF StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant, CertificatesBackedByIntents,
       HistoricalQcValid, RestartLockedPrepareQCs,
       LockedPrepareRecoverySource,
       SamePrepareRecoveryRef, SameCertificateRef, CertificateRefOf

THEOREM ResponsiveReplayEstablishesLineagedLockedBodyOwners ==
  /\ AsyncStrongTypeInvariant
  /\ PreGstResponsiveReplay
  => \A qc \in RestartLockedPrepareQCs(asyncRecoveryNode)':
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority(
         asyncRecoveryNode, qc)'
BY RestartLockedPrepareChoiceIsAvailable,
   RestartLockedPrepareSourcesShareRecoveryReference,
   RestartLockedBodyReplayProperties,
   RestartLockedBodyReplayCandidateShape,
   RangeConcatenation, RangeEquality, IsaT(480)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyFetchOwned,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedBodyValidationHeld,
       RestartLockedPrepareQCs, RestartLockedPrepareQC,
       RestartLockedBodyReplay, RestartLockedBodyPipelineCandidate,
       RestartSignatureReplay, RestartReplay, RestartDecisionReplay,
       RestartRunnerAssembly, RestartCandidate,
       CandidateConsumerCurrent, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncQueueTyped, AsyncCandidateTyped,
       AsyncCandidateSet, SequenceSet,
       PreGstResponsiveReplay, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       ResetNodeSchedulerForRestart

THEOREM ResponsiveReplayEstablishesLineagedReplayCarrier ==
  /\ AsyncStrongTypeInvariant
  /\ PreGstResponsiveReplay
  => ResponsiveReplayLockedBodyLineagedCarrierInvariant'
BY ResponsiveReplayEstablishesLineagedLockedBodyOwners, Isa
   DEF ResponsiveReplayLockedBodyLineagedCarrierInvariant

THEOREM ResponsiveReplayPreservesHistoricalLineageSource ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ HistoricalLockedBodyLineageSourceRetentionInvariant
  /\ PreGstResponsiveReplay
  => HistoricalLockedBodyLineageSourceRetentionInvariant'
BY ResponsiveReplayEstablishesLineagedLockedBodyOwners, IsaT(420)
   DEF ResponsiveRecoveryValidationClearedInvariant,
       RecoveryNodeValidationCleared,
       AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       RestartLockedPrepareQCs,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       PreGstResponsiveReplay, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       ResetNodeSchedulerForRestart

THEOREM ResponsiveReplayPreservesDecisionExactSource ==
  /\ AsyncStrongTypeInvariant
  /\ DecisionsUniqueByNodeContext
  /\ DecisionExactSourceRetentionInvariant
  /\ PreGstResponsiveReplay
  => DecisionExactSourceRetentionInvariant'
BY ResponsiveReplayInstallsExactCurrentDecisionFetchUpdate,
   UniqueUnappliedDecisionExcludesNodeApplication, IsaT(360)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       DecisionExactSourceRetentionInvariant,
       DecisionExactSourceOwner, HistoricalRecoveryTarget,
       AsyncDecisionRecoveryStageExact,
       DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedResponseLineageExact,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       ExactCurrentDecisionFetchUpdate, DecisionFetchCandidateAt,
       DecisionPipelineCandidate, CandidateScheduled,
       CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, NodeHasApplication,
       RestartDecisions, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch,
       PreGstResponsiveReplay, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       ResetNodeSchedulerForRestart, RestartSignatureReplay,
       RestartReplay, RestartDecisionReplay, RestartCandidate

THEOREM ResponsiveReplayPreservesFinalProgressWitnessClosure ==
  /\ AsyncStrongTypeInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ FinalProgressWitnessClosureInvariant
  /\ PreGstResponsiveReplay
  => FinalProgressWitnessClosureInvariant'
BY ResponsiveReplayPreservesDecisionExactSource,
   ResponsiveReplayPreservesHistoricalLineageSource,
   ResponsiveReplayEstablishesLineagedReplayCarrier
   DEF DecisionFrontierUniquenessInvariant,
       FinalProgressWitnessClosureInvariant,
       FinalWitnessSourceRetentionInvariant

THEOREM DriveReplayPreservesHistoricalLineageSource ==
  /\ AsyncStrongTypeInvariant
  /\ HistoricalLockedBodyLineageSourceRetentionInvariant
  /\ DriveResponsiveReplayHead
  => HistoricalLockedBodyLineageSourceRetentionInvariant'
BY IsaT(300)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       DriveResponsiveReplayHead, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       AsyncRecoveryLifecycleVars

THEOREM DriveReplayPreservesDecisionExactSource ==
  /\ AsyncStrongTypeInvariant
  /\ DecisionExactSourceRetentionInvariant
  /\ DriveResponsiveReplayHead
  => DecisionExactSourceRetentionInvariant'
BY IsaT(300)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       DecisionExactSourceRetentionInvariant,
       DecisionExactSourceOwner, HistoricalRecoveryTarget,
       AsyncDecisionRecoveryStageExact,
       DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedResponseLineageExact,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       RestartDecisions, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch,
       DriveResponsiveReplayHead, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       FreshCandidateSequence, AsyncRecoveryLifecycleVars

THEOREM DriveReplayPreservesLineagedCarrier ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveReplayLockedBodyLineagedCarrierInvariant
  /\ DriveResponsiveReplayHead
  => ResponsiveReplayLockedBodyLineagedCarrierInvariant'
BY RestartSignatureReplayProperties,
   RangeConcatenation, RangeEquality, FunctionalConcatUpdateAtKey,
   IsaT(420)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ResponsiveReplayLockedBodyLineagedCarrierInvariant,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLockedBodyFetchOwned,
       HistoricalLockedBodyCertifiedFetchOwned,
       HistoricalLockedBodyStoreOwned,
       HistoricalLockedBodyValidateOwned,
       HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, RestartLockedPrepareQCs,
       CertifiedRequestOutbox, CandidateConsumerCurrent,
       CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       DriveResponsiveReplayHead, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       FreshCandidateSequence, AsyncRecoveryLifecycleVars

THEOREM DriveReplayPreservesFinalProgressWitnessClosure ==
  /\ AsyncStrongTypeInvariant
  /\ FinalProgressWitnessClosureInvariant
  /\ DriveResponsiveReplayHead
  => FinalProgressWitnessClosureInvariant'
BY DriveReplayPreservesDecisionExactSource,
   DriveReplayPreservesHistoricalLineageSource,
   DriveReplayPreservesLineagedCarrier
   DEF FinalProgressWitnessClosureInvariant,
       FinalWitnessSourceRetentionInvariant

THEOREM FinishReplayRetainsLineagedLockedBodyOwners ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveReplayLockedBodyLineagedCarrierInvariant
  /\ FinishResponsiveReplay
  => \A qc \in RestartLockedPrepareQCs(asyncRecoveryNode)':
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority(
         asyncRecoveryNode, qc)'
BY IsaT(420)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ResponsiveReplayLockedBodyLineagedCarrierInvariant,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLockedBodyFetchOwned,
       HistoricalLockedBodyCertifiedFetchOwned,
       HistoricalLockedBodyStoreOwned,
       HistoricalLockedBodyValidateOwned,
       HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, RestartLockedPrepareQCs,
       CertifiedRequestOutbox, CandidateConsumerCurrent,
       CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       FinishResponsiveReplay, RestartRunnerAssembly,
       FreshCandidateSequence, vars

THEOREM FinishReplayPreservesHistoricalLineageSource ==
  /\ AsyncStrongTypeInvariant
  /\ HistoricalLockedBodyLineageSourceRetentionInvariant
  /\ ResponsiveReplayLockedBodyLineagedCarrierInvariant
  /\ FinishResponsiveReplay
  => HistoricalLockedBodyLineageSourceRetentionInvariant'
BY FinishReplayRetainsLineagedLockedBodyOwners, IsaT(300)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       RestartLockedPrepareQCs,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       FinishResponsiveReplay, vars

THEOREM FinishReplayPreservesDecisionExactSource ==
  /\ AsyncStrongTypeInvariant
  /\ DecisionExactSourceRetentionInvariant
  /\ FinishResponsiveReplay
  => DecisionExactSourceRetentionInvariant'
BY IsaT(360)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       DecisionExactSourceRetentionInvariant,
       DecisionExactSourceOwner, HistoricalRecoveryTarget,
       AsyncDecisionRecoveryStageExact,
       DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedResponseLineageExact,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       RestartDecisions,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       FinishResponsiveReplay, RestartRunnerAssembly,
       FreshCandidateSequence, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, vars

THEOREM FinishReplayPreservesFinalProgressWitnessClosure ==
  /\ AsyncStrongTypeInvariant
  /\ FinalProgressWitnessClosureInvariant
  /\ FinishResponsiveReplay
  => FinalProgressWitnessClosureInvariant'
BY FinishReplayPreservesDecisionExactSource,
   FinishReplayPreservesHistoricalLineageSource, Isa
   DEF FinalProgressWitnessClosureInvariant,
       FinalWitnessSourceRetentionInvariant,
       ResponsiveReplayLockedBodyLineagedCarrierInvariant,
       FinishResponsiveReplay

THEOREM RearmPreservesFinalProgressWitnessClosure ==
  /\ FinalProgressWitnessClosureInvariant
  /\ RearmResponsiveRecovery
  => FinalProgressWitnessClosureInvariant'
BY IsaT(180)
   DEF FinalProgressWitnessClosureInvariant,
       FinalWitnessSourceRetentionInvariant,
       DecisionExactSourceRetentionInvariant,
       DecisionExactSourceOwner, HistoricalRecoveryTarget,
       AsyncDecisionRecoveryStageExact,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryAuthority,
       ResponsiveReplayLockedBodyLineagedCarrierInvariant,
       RearmResponsiveRecovery, AsyncSchedulerVars,
       AsyncRecoveryVars, vars

(***************************************************************************
Full AsyncNext and temporal induction.
***************************************************************************)

THEOREM AsyncNextPreservesFinalProgressWitnessClosure ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ FinalProgressWitnessClosureInvariant
  /\ AsyncNext
  => FinalProgressWitnessClosureInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              DecisionFrontierUniquenessInvariant,
              DecisionTimeoutFrontierInvariant,
              ResponsiveRecoveryValidationClearedInvariant,
              FinalProgressWitnessClosureInvariant,
              AsyncNext
         PROVE FinalProgressWitnessClosureInvariant'
    <2>1. CASE AsyncNonCrashStep
      <3>1. CASE /\ AsyncRunnerStep
                   /\ UNCHANGED <<up, AsyncRecoveryVars>>
        BY <1>1, <3>1,
           AsyncRunnerPreservesFinalProgressWitnessClosure
      <3>2. CASE /\ AsyncNonRunnerStep
                   /\ UNCHANGED <<up, AsyncRecoveryVars>>
        BY <1>1, <3>2,
           AsyncNonRunnerPreservesFinalProgressWitnessClosure
      <3>3. CASE /\ DriveResponsiveReplayHead
                   /\ UNCHANGED up
        BY <1>1, <3>3,
           DriveReplayPreservesFinalProgressWitnessClosure
      <3>4. CASE /\ FinishResponsiveReplay
                   /\ UNCHANGED up
        BY <1>1, <3>4,
           FinishReplayPreservesFinalProgressWitnessClosure
      <3>5. CASE /\ RearmResponsiveRecovery
                   /\ UNCHANGED up
        BY <1>1, <3>5,
           RearmPreservesFinalProgressWitnessClosure
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4, <3>5
           DEF AsyncNonCrashStep
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <1>1, <2>2,
         NonResponsiveCrashPreservesFinalProgressWitnessClosure
    <2>3. CASE \E node \in ValidatorIds:
                  PreGstResponsiveCrash(node)
      BY <1>1, <2>3,
         ResponsiveCrashPreservesFinalProgressWitnessClosure
    <2>4. CASE PreGstResponsiveRestart
      BY <1>1, <2>4,
         ResponsiveRestartPreservesFinalProgressWitnessClosure
    <2>5. CASE PreGstResponsiveReplay
      BY <1>1, <2>5,
         ResponsiveReplayPreservesFinalProgressWitnessClosure
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5 DEF AsyncNext
  <1> QED BY <1>1

THEOREM AsyncAllVarsStutterPreservesFinalProgressWitnessClosure ==
  /\ FinalProgressWitnessClosureInvariant
  /\ UNCHANGED AsyncAllVars
  => FinalProgressWitnessClosureInvariant'
BY Isa
   DEF FinalProgressWitnessClosureInvariant,
       FinalWitnessSourceRetentionInvariant,
       AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars, vars

THEOREM AsyncBracketStepLeavesContext ==
  [AsyncNext]_AsyncAllVars => UNCHANGED context
BY AsyncStepRefinementObligation, CoreNextLeavesContext, Isa
   DEF AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars, vars

THEOREM AsyncBracketNextPreservesFinalProgressWitnessClosure ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ FinalProgressWitnessClosureInvariant
  /\ [AsyncNext]_AsyncAllVars
  => FinalProgressWitnessClosureInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncProgressOwnershipInvariant,
              DecisionFrontierUniquenessInvariant,
              DecisionTimeoutFrontierInvariant,
              ResponsiveRecoveryValidationClearedInvariant,
              FinalProgressWitnessClosureInvariant,
              [AsyncNext]_AsyncAllVars
         PROVE FinalProgressWitnessClosureInvariant'
    <2>1. CASE AsyncNext
      BY <1>1, <2>1,
         AsyncNextPreservesFinalProgressWitnessClosure
    <2>2. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>2,
         AsyncAllVarsStutterPreservesFinalProgressWitnessClosure
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM FinalProgressWitnessClosureInvariantObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []FinalProgressWitnessClosureInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []FinalProgressWitnessClosureInvariant
    <2>1. AsyncInitAt(initialContext)
             => FinalProgressWitnessClosureInvariant
      BY AsyncInitEstablishesFinalProgressWitnessClosure
    <2>2. AsyncSpecAt(initialContext)
             => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>3. AsyncSpecAt(initialContext)
             => []AsyncProgressOwnershipInvariant
      BY AsyncSpecAlwaysProgressOwnershipInvariant
    <2>4. AsyncSpecAt(initialContext)
             => []DecisionFrontierUniquenessInvariant
      BY DecisionFrontierUniquenessInvariantFromAsyncSpec
    <2>5. AsyncSpecAt(initialContext)
             => []DecisionTimeoutFrontierInvariant
      BY DecisionTimeoutFrontierInvariantFromAsyncSpec
    <2>6. AsyncSpecAt(initialContext)
             => []ResponsiveRecoveryValidationClearedInvariant
      BY ResponsiveRecoveryValidationClearedInvariantObligation
    <2>7. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ DecisionFrontierUniquenessInvariant
           /\ DecisionTimeoutFrontierInvariant
           /\ ResponsiveRecoveryValidationClearedInvariant
           /\ FinalProgressWitnessClosureInvariant
           /\ [AsyncNext]_AsyncAllVars
          => FinalProgressWitnessClosureInvariant'
      BY AsyncBracketNextPreservesFinalProgressWitnessClosure
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
         PTL DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM OpenProgressWitnessKernelInvariantObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []OpenProgressWitnessKernelInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []OpenProgressWitnessKernelInvariant
    <2>1. AsyncSpecAt(initialContext)
             => []FinalProgressWitnessClosureInvariant
      BY FinalProgressWitnessClosureInvariantObligation
    <2>2. FinalProgressWitnessClosureInvariant
             => OpenProgressWitnessKernelInvariant
      BY FinalClosureProjectsOpenProgressKernel
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

(***************************************************************************
Release interface consumed by SumeragiV2AsyncTemporalClosureProofs.
***************************************************************************)

THEOREM FinalProgressWitnessObligation ==
  \A initialContext:
    AsyncProgressWitnessAndHistoricalRecoveryProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncProgressWitnessAndHistoricalRecoveryProperty(
                   AsyncSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
             => []OpenProgressWitnessKernelInvariant
      BY OpenProgressWitnessKernelInvariantObligation
    <2>2. AsyncProgressWitnessAndHistoricalRecoveryProperty(
             AsyncSpecAt(initialContext))
             <=> (AsyncSpecAt(initialContext)
                    => []OpenProgressWitnessKernelInvariant)
      BY ProgressWitnessClosureEquivalentToOpenKernel
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

=============================================================================
