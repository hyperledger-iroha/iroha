---- MODULE SumeragiV2AutonomousReservationCarrier ----
EXTENDS Naturals

(***************************************************************************
Bounded ownership model for one autonomous transaction reservation carried
unchanged from the ordinary queue through a control-only global anchor, lane
certification, durable full-candidate authorization, canonical application,
and reservation finalization.

The restart path models the production startup gate for a globally finalized
autonomous anchor whose canonical body is not local:

  1. close ordinary Queue selection while retaining the exact reservation;
  2. recover one canonical body from a single authenticated Commit-QC signer;
  3. durably install the exact historical route/incarnation/context task;
  4. preflight every reservation group before mutating Queue ownership;
  5. reopen the Queue gate before historical committee certification.

The losing-owner path models the durable cross-store release protocol:

  1. retire the exact autonomous slot and move its Kura claim to
     ReleasePending;
  2. durably prepare the queue release barrier while the reservation remains
     excluded from ordinary selection;
  3. move the Kura claim to Released only after that barrier;
  4. durably complete the queue release and restore FIFO ownership.

The terminal-outcome path models the durable Kura/Queue join and startup cut:

  1. Kura Pending records source identity but grants no terminal ownership;
  2. canonical catch-up reconstructs the complete carrier outcome set,
     authenticates each ApplyCarrier group independently, and runs one
     all-group Queue preflight before any cleanup;
  3. release completion consumes exact retirement/finalization authority;
  4. Kura Complete consumes positive Queue terminal evidence; and
  5. restart first captures one immutable Queue ownership receipt, preflights
     complete canonical units against that receipt, completes only all-empty
     units, and defers an entire mixed unit when any member is Queue-owned;
  6. the ordinary carrier planner consumes that same receipt, applies every
     deferred member atomically, and only then may publication reopen.

The retained-attempt recovery cut distinguishes the local producer from an
observer. A producer must retain its exact current Queue reservation group
before Crash/Recover; an observer may recover from exact local Kura custody
without a local Queue group.

The production refinement is source-bound separately to the queue reservation
and release-barrier APIs, Kura autonomous slot claims, full merge-candidate
signing authorization, `StateBlock::stage_certified_merge_entry`, and
`State::validate_merge_execution_batch`, including its route/incarnation-first
canonical order key, plus startup reservation reconciliation through bounded
Kura indexes. The carrier surface also abstracts the move-only authorization's
exact encoded autonomous external-event prefix and the separately bound full
deterministic carrier event surface. Pre-vote validation must retain the exact
autonomous prefix while binding the carrier suffix; final application must
compare that complete surface before adding the ordinary Applied block event,
drain the live buffer, and reconstruct the certified write-set root from only
the retained autonomous bytes at metadata mint and State commit. The
observer-only diagnostic rank is
source-bound to
`State::autonomous_lane_execution_diagnostics_inner`, the exact durable Queue
ownership/barrier observer, `AutonomousLaneDiagnosticEvidence::finish`, the
Torii queue-aware projection, and data-model validation. It may only report an
exact identity's independently durable stage chain and never authorize state.
This model is finite mutation evidence, not a proof of those Rust trace
mappings.
***************************************************************************)

CONSTANTS
  \* @type: Str;
  Mode,
  \* @type: Str;
  ExactReservationIdentity,
  \* @type: Str;
  DriftedReservationIdentity,
  \* @type: Str;
  RecreatedReservationIdentity,
  \* @type: Str;
  IncarnationA,
  \* @type: Str;
  IncarnationB

ReservationModes ==
  {"Fixed", "CarrierIdentityDrift", "DuplicateApplication",
   "ReleaseAfterApplication", "ReleaseBeforeBarrier", "AbaRelease",
   "DigestOnlyAuthorization", "OrdinaryAnchorExecution",
   "ReserveBeforeDurable", "NonCanonicalMergePrefix",
   "SkipCanonicalReexecution", "PreVoteCommitSurfaceDrift",
   "AutonomousEventPrefixDrift", "PostValidationEventSurfaceDrift",
   "RestartDropsOwnership",
   "VolatileStageDiagnostics", "UnauthenticatedRecoveryBody",
   "MixedSignerRecoveryBody", "InflatedRecoveryWireLength",
   "HistoricalContextDrift",
   "PartialRecoveryGroupPreflight", "OpenQueueBeforeRecoveryInstall",
   "PendingOnlyCanonicalTerminal", "ReleaseWithoutFinalizationAuthority",
   "CompleteWithoutQueueEvidence", "OwnedGroupMutationBeforePlanner",
   "OpenQueueBeforeDeferredCarrierApply", "PartialTerminalUnitSweep",
   "ProducerRecoveryWithoutQueueOwner"}

ReservationStages ==
  {"Queued", "Reserved", "Anchored", "Certified", "CandidateDurable",
   "CandidateAuthorized", "PreVoteAuthorized", "CarrierFinalized",
   "ReleasePending", "Released", "Applied", "Forgotten"}

RetainedAttemptStages ==
  {"Reserved", "Anchored", "Certified", "CandidateDurable"}

CarrierCommitSurfaces ==
  {"None", "Pristine", "PostBlockPreVote", "FinalizedCarrier",
   "InvalidPostBlockPreVote", "InvalidAutonomousEventPrefix",
   "InvalidPublicationEventSurface"}

ClaimStates == {"None", "Active", "ReleasePending", "Released", "Committed"}

RecoveryStages ==
  {"Normal", "NeedBody", "BodyVerified", "BodyAcceptedUnauthenticated",
   "TaskExact", "TaskUnauthenticated", "TaskDrifted",
   "UnauthenticatedPreflight", "ContextDriftPreflight",
   "PartialPreflight", "GroupsPreflight", "HistoricalCertified",
   "LocalProducerRetained", "ObserverKuraRetained",
   "LocalProducerRecovering", "ObserverKuraRecovering"}

TerminalOutcomeStages == {"None", "Pending", "Complete"}

TerminalOutcomeSources == {"None", "Canonical", "Release"}

ReservationIdentities ==
  {"None", ExactReservationIdentity, DriftedReservationIdentity,
   RecreatedReservationIdentity}

Incarnations == {"None", IncarnationA, IncarnationB}

ReservationConfiguration ==
  /\ Mode \in ReservationModes
  /\ ExactReservationIdentity # DriftedReservationIdentity
  /\ ExactReservationIdentity # RecreatedReservationIdentity
  /\ DriftedReservationIdentity # RecreatedReservationIdentity
  /\ IncarnationA # IncarnationB

BoolNat(value) == IF value THEN 1 ELSE 0

VARIABLES
  \* @type: Str;
  stage,
  \* @type: Str;
  reservationIdentity,
  \* @type: Str;
  carrierIdentity,
  \* @type: Str;
  incarnation,
  \* @type: Str;
  claimState,
  \* @type: Bool;
  queueOwns,
  \* @type: Bool;
  laneOwns,
  \* @type: Bool;
  mergeOwns,
  \* @type: Bool;
  releaseOwns,
  \* @type: Bool;
  committedOwner,
  \* @type: Int;
  executionCount,
  \* @type: Bool;
  controlOnlyAnchor,
  \* @type: Bool;
  candidateBodyDurable,
  \* @type: Bool;
  candidateAuthorized,
  \* @type: Bool;
  slotRetired,
  \* @type: Bool;
  releaseBarrier,
  \* @type: Bool;
  releaseCompletion,
  \* @type: Bool;
  released,
  \* @type: Bool;
  releaseAfterApply,
  \* @type: Bool;
  recreated,
  \* @type: Bool;
  staleRelease,
  \* @type: Bool;
  reservationDurable,
  \* @type: Bool;
  mergeCandidateExact,
  \* @type: Bool;
  canonicalReexecuted,
  \* @type: Str;
  recoveryStage,
  \* @type: Bool;
  queueGateOpen,
  \* @type: Bool;
  recoverySignerStable,
  \* @type: Bool;
  recoveryWireLengthExact,
  \* @type: Bool;
  localQueueReservationGroupExact,
  \* @type: Bool;
  networkIngressStartupFenced,
  \* @type: Bool;
  queueOwnerQuarantinePending,
  \* @type: Int;
  durableStageRank,
  \* @type: Int;
  diagnosticStageRank,
  \* @type: Bool;
  diagnosticIdentityExact,
  \* @type: Bool;
  diagnosticsAuthorizeState,
  \* @type: Str;
  carrierCommitSurface,
  \* @type: Str;
  terminalOutcomeStage,
  \* @type: Str;
  terminalOutcomeSource,
  \* @type: Bool;
  canonicalCarrierCleanupAuthorized,
  \* @type: Bool;
  releaseFinalizationAuthorized,
  \* @type: Bool;
  queueTerminalPhysical,
  \* @type: Bool;
  positiveQueueTerminalEvidence,
  \* @type: Bool;
  terminalStartupGateClosed,
  \* @type: Bool;
  terminalSweepStarted,
  \* @type: Bool;
  terminalSweepCompleted,
  \* @type: Bool;
  queueOwnershipSnapshotTaken,
  \* @type: Bool;
  queueOwnershipSnapshotReceiptValid,
  \* @type: Bool;
  canonicalGroupAQueueOwned,
  \* @type: Bool;
  canonicalGroupBQueueOwned,
  \* @type: Bool;
  snapshotGroupAQueueOwned,
  \* @type: Bool;
  snapshotGroupBQueueOwned,
  \* @type: Bool;
  canonicalGroupATerminalPublished,
  \* @type: Bool;
  canonicalGroupBTerminalPublished,
  \* @type: Bool;
  canonicalCarrierUnitDeferred,
  \* @type: Bool;
  deferredCarrierPlannedFromSnapshot,
  \* @type: Bool;
  normalCarrierApplyCompleted,
  \* @type: Bool;
  canonicalOutcomeSetComplete,
  \* @type: Bool;
  canonicalCarrierBatchPreflighted,
  \* @type: Bool;
  partialCanonicalCleanup

carrierVars ==
  <<stage, reservationIdentity, carrierIdentity, incarnation, claimState,
    queueOwns, laneOwns, mergeOwns, releaseOwns, committedOwner,
    executionCount, controlOnlyAnchor, candidateBodyDurable,
    candidateAuthorized, slotRetired, releaseBarrier, releaseCompletion,
    released, releaseAfterApply, recreated, staleRelease,
    reservationDurable, mergeCandidateExact, canonicalReexecuted,
    carrierCommitSurface>>

diagnosticVars ==
  <<durableStageRank, diagnosticStageRank, diagnosticIdentityExact,
    diagnosticsAuthorizeState>>

recoveryVars ==
  <<recoveryStage, queueGateOpen, recoverySignerStable,
    recoveryWireLengthExact, localQueueReservationGroupExact,
    networkIngressStartupFenced, queueOwnerQuarantinePending>>

\* @type: <<Bool, Bool, Bool>>;
canonicalTerminalBatchVars ==
  <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted,
    partialCanonicalCleanup>>

\* @type: <<Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
startupTerminalUnitVars ==
  <<queueOwnershipSnapshotReceiptValid,
    canonicalGroupAQueueOwned, canonicalGroupBQueueOwned,
    snapshotGroupAQueueOwned, snapshotGroupBQueueOwned,
    canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished,
    canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot,
    normalCarrierApplyCompleted>>

terminalVars ==
  <<terminalOutcomeStage, terminalOutcomeSource,
    canonicalCarrierCleanupAuthorized, releaseFinalizationAuthorized,
    queueTerminalPhysical, positiveQueueTerminalEvidence,
    terminalStartupGateClosed, terminalSweepStarted,
    terminalSweepCompleted, queueOwnershipSnapshotTaken,
    canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup, queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

vars ==
  <<stage, reservationIdentity, carrierIdentity, incarnation, claimState,
    queueOwns, laneOwns, mergeOwns, releaseOwns, committedOwner,
    executionCount, controlOnlyAnchor, candidateBodyDurable,
    candidateAuthorized, slotRetired, releaseBarrier, releaseCompletion,
    released, releaseAfterApply, recreated, staleRelease,
    reservationDurable, mergeCandidateExact, canonicalReexecuted,
    recoveryStage, queueGateOpen, recoverySignerStable,
    recoveryWireLengthExact, localQueueReservationGroupExact,
    networkIngressStartupFenced, queueOwnerQuarantinePending,
    durableStageRank, diagnosticStageRank, diagnosticIdentityExact,
    diagnosticsAuthorizeState, carrierCommitSurface,
    terminalOutcomeStage, terminalOutcomeSource,
    canonicalCarrierCleanupAuthorized, releaseFinalizationAuthorized,
    queueTerminalPhysical, positiveQueueTerminalEvidence,
    terminalStartupGateClosed, terminalSweepStarted,
    terminalSweepCompleted, queueOwnershipSnapshotTaken,
    queueOwnershipSnapshotReceiptValid,
    canonicalGroupAQueueOwned, canonicalGroupBQueueOwned,
    snapshotGroupAQueueOwned, snapshotGroupBQueueOwned,
    canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished,
    canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot,
    normalCarrierApplyCompleted,
    canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted,
    partialCanonicalCleanup>>

Init ==
  /\ ReservationConfiguration
  /\ stage = "Queued"
  /\ reservationIdentity = "None"
  /\ carrierIdentity = "None"
  /\ incarnation = "None"
  /\ claimState = "None"
  /\ queueOwns = TRUE
  /\ laneOwns = FALSE
  /\ mergeOwns = FALSE
  /\ releaseOwns = FALSE
  /\ committedOwner = FALSE
  /\ executionCount = 0
  /\ controlOnlyAnchor = FALSE
  /\ candidateBodyDurable = FALSE
  /\ candidateAuthorized = FALSE
  /\ slotRetired = FALSE
  /\ releaseBarrier = FALSE
  /\ releaseCompletion = FALSE
  /\ released = FALSE
  /\ releaseAfterApply = FALSE
  /\ recreated = FALSE
  /\ staleRelease = FALSE
  /\ reservationDurable = FALSE
  /\ mergeCandidateExact = FALSE
  /\ canonicalReexecuted = FALSE
  /\ recoveryStage = "Normal"
  /\ queueGateOpen = TRUE
  /\ recoverySignerStable = TRUE
  /\ recoveryWireLengthExact = TRUE
  /\ localQueueReservationGroupExact = FALSE
  /\ networkIngressStartupFenced = FALSE
  /\ queueOwnerQuarantinePending = FALSE
  /\ durableStageRank = 0
  /\ diagnosticStageRank = 0
  /\ diagnosticIdentityExact = TRUE
  /\ diagnosticsAuthorizeState = FALSE
  /\ carrierCommitSurface = "None"
  /\ terminalOutcomeStage = "None"
  /\ terminalOutcomeSource = "None"
  /\ canonicalCarrierCleanupAuthorized = FALSE
  /\ releaseFinalizationAuthorized = FALSE
  /\ queueTerminalPhysical = FALSE
  /\ positiveQueueTerminalEvidence = FALSE
  /\ terminalStartupGateClosed = FALSE
  /\ terminalSweepStarted = FALSE
  /\ terminalSweepCompleted = FALSE
  /\ queueOwnershipSnapshotTaken = FALSE
  /\ queueOwnershipSnapshotReceiptValid = FALSE
  /\ canonicalGroupAQueueOwned = FALSE
  /\ canonicalGroupBQueueOwned = FALSE
  /\ snapshotGroupAQueueOwned = FALSE
  /\ snapshotGroupBQueueOwned = FALSE
  /\ canonicalGroupATerminalPublished = FALSE
  /\ canonicalGroupBTerminalPublished = FALSE
  /\ canonicalCarrierUnitDeferred = FALSE
  /\ deferredCarrierPlannedFromSnapshot = FALSE
  /\ normalCarrierApplyCompleted = FALSE
  /\ canonicalOutcomeSetComplete = FALSE
  /\ canonicalCarrierBatchPreflighted = FALSE
  /\ partialCanonicalCleanup = FALSE

ReserveFifoTransaction ==
  /\ stage = "Queued"
  /\ recoveryStage = "Normal"
  /\ queueGateOpen
  /\ queueOwns
  /\ ~released
  /\ ~recreated
  /\ executionCount = 0
  /\ stage' = "Reserved"
  /\ reservationIdentity' = ExactReservationIdentity
  /\ carrierIdentity' = ExactReservationIdentity
  /\ incarnation' = IncarnationA
  /\ claimState' = "Active"
  /\ queueOwns' = FALSE
  /\ laneOwns' = TRUE
  /\ mergeOwns' = FALSE
  /\ releaseOwns' = FALSE
  /\ committedOwner' = FALSE
  /\ controlOnlyAnchor' = FALSE
  /\ candidateBodyDurable' = FALSE
  /\ candidateAuthorized' = FALSE
  /\ slotRetired' = FALSE
  /\ releaseBarrier' = FALSE
  /\ releaseCompletion' = FALSE
  /\ released' = FALSE
  /\ releaseAfterApply' = FALSE
  /\ staleRelease' = FALSE
  /\ reservationDurable' = (Mode # "ReserveBeforeDurable")
  /\ mergeCandidateExact' = FALSE
  /\ canonicalReexecuted' = FALSE
  /\ durableStageRank' =
       IF Mode = "ReserveBeforeDurable" THEN 0 ELSE 1
  /\ UNCHANGED <<executionCount, recreated, carrierCommitSurface>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticIdentityExact,
                 diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

AnchorAutonomousControl ==
  /\ stage = "Reserved"
  /\ laneOwns
  /\ claimState = "Active"
  /\ stage' = "Anchored"
  /\ controlOnlyAnchor' = (Mode # "OrdinaryAnchorExecution")
  /\ executionCount' =
       IF Mode = "OrdinaryAnchorExecution"
       THEN executionCount + 1
       ELSE executionCount
  /\ durableStageRank' = 2
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, laneOwns, mergeOwns, releaseOwns,
                 committedOwner, candidateBodyDurable, candidateAuthorized,
                 slotRetired, releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticIdentityExact,
                 diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

CertifyAutonomousBundle ==
  /\ stage = "Anchored"
  /\ recoveryStage = "Normal"
  /\ laneOwns
  /\ stage' = "Certified"
  /\ carrierIdentity' =
       IF Mode = "CarrierIdentityDrift"
       THEN DriftedReservationIdentity
       ELSE reservationIdentity
  /\ durableStageRank' = 4
  /\ UNCHANGED <<reservationIdentity, incarnation, claimState, queueOwns,
                 laneOwns, mergeOwns, releaseOwns, committedOwner,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, released, releaseAfterApply, recreated,
                 staleRelease, reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticIdentityExact,
                 diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

PersistFullMergeCandidate ==
  /\ stage = "Certified"
  /\ laneOwns
  /\ stage' = "CandidateDurable"
  /\ candidateBodyDurable' = TRUE
  /\ durableStageRank' = 5
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, laneOwns, mergeOwns, releaseOwns,
                 committedOwner, executionCount, controlOnlyAnchor,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, released, releaseAfterApply, recreated,
                 staleRelease, reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticIdentityExact,
                 diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

AuthorizeExactMergeCandidate ==
  /\ \/ /\ stage = "CandidateDurable"
        /\ candidateBodyDurable
     \/ /\ Mode = "DigestOnlyAuthorization"
        /\ stage = "Certified"
        /\ ~candidateBodyDurable
  /\ laneOwns
  /\ stage' = "CandidateAuthorized"
  /\ laneOwns' = FALSE
  /\ mergeOwns' = TRUE
  /\ candidateAuthorized' = TRUE
  /\ mergeCandidateExact' = (Mode # "NonCanonicalMergePrefix")
  /\ carrierCommitSurface' = "Pristine"
  /\ durableStageRank' =
       IF candidateBodyDurable THEN 6 ELSE durableStageRank
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, releaseOwns, committedOwner,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 slotRetired, releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, canonicalReexecuted>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticIdentityExact,
                 diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

\* `Pristine` abstracts an empty block-hash overlay and no staged canonical
\* transaction-height row. Post-block/pre-vote validation must then observe no
\* pending block hash, exactly one empty row at the carrier height, the exact
\* encoded autonomous external-event prefix retained by the move-only commit
\* authorization, and one separately bound complete deterministic carrier
\* event surface.
ValidatePostBlockPreVoteCarrierSurface ==
  /\ stage = "CandidateAuthorized"
  /\ mergeOwns
  /\ candidateAuthorized
  /\ carrierCommitSurface = "Pristine"
  /\ stage' = "PreVoteAuthorized"
  /\ carrierCommitSurface' =
       CASE Mode = "PreVoteCommitSurfaceDrift"
              -> "InvalidPostBlockPreVote"
         [] Mode = "AutonomousEventPrefixDrift"
              -> "InvalidAutonomousEventPrefix"
         [] OTHER -> "PostBlockPreVote"
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, laneOwns, mergeOwns, releaseOwns,
                 committedOwner, executionCount, controlOnlyAnchor,
                 candidateBodyDurable, candidateAuthorized, slotRetired,
                 releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 canonicalReexecuted>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

\* Final application replaces the absent hash with the exact singleton
\* finalized carrier hash while retaining the exact empty transaction row. It
\* first byte-compares the complete deterministic carrier event surface bound
\* before voting, then drains the live event buffer after appending the
\* ordinary Applied block event; metadata mint and State commit reconstruct the
\* certified write-set root from retained autonomous-prefix bytes.
FinalizeCarrierCommitSurface ==
  /\ stage = "PreVoteAuthorized"
  /\ mergeOwns
  /\ candidateAuthorized
  /\ carrierCommitSurface \in
       {"PostBlockPreVote", "InvalidPostBlockPreVote",
        "InvalidAutonomousEventPrefix"}
  /\ stage' = "CarrierFinalized"
  /\ carrierCommitSurface' =
       IF Mode = "PostValidationEventSurfaceDrift"
       THEN "InvalidPublicationEventSurface"
       ELSE "FinalizedCarrier"
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, laneOwns, mergeOwns, releaseOwns,
                 committedOwner, executionCount, controlOnlyAnchor,
                 candidateBodyDurable, candidateAuthorized, slotRetired,
                 releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 canonicalReexecuted>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

ApplyCanonicalCarrier ==
  /\ stage = "CarrierFinalized"
  /\ mergeOwns
  /\ candidateAuthorized
  /\ carrierIdentity = reservationIdentity
  /\ carrierCommitSurface = "FinalizedCarrier"
  /\ executionCount' = executionCount + 1
  /\ claimState' = "Committed"
  /\ canonicalReexecuted' = (Mode # "SkipCanonicalReexecution")
  /\ durableStageRank' = 8
  /\ IF Mode = "DuplicateApplication"
     THEN /\ stage' = "CarrierFinalized"
          /\ mergeOwns' = TRUE
          /\ committedOwner' = FALSE
     ELSE /\ stage' = "Applied"
          /\ mergeOwns' = FALSE
          /\ committedOwner' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 queueOwns, laneOwns, releaseOwns, controlOnlyAnchor,
                 candidateBodyDurable, candidateAuthorized, slotRetired,
                 releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 carrierCommitSurface>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticIdentityExact,
                 diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

\* Kura first fsyncs a source-revalidated outcome. This record names the exact
\* reservation group but deliberately carries no Queue ownership authority.
PersistCanonicalTerminalOutcomePending ==
  /\ stage = "Applied"
  /\ committedOwner
  /\ executionCount = 1
  /\ terminalOutcomeStage = "None"
  /\ terminalOutcomeStage' = "Pending"
  /\ terminalOutcomeSource' = "Canonical"
  /\ canonicalCarrierCleanupAuthorized' = FALSE
  /\ queueTerminalPhysical' = FALSE
  /\ positiveQueueTerminalEvidence' = FALSE
  /\ canonicalOutcomeSetComplete' = FALSE
  /\ canonicalCarrierBatchPreflighted' = FALSE
  /\ partialCanonicalCleanup' = FALSE
  \* The bounded canonical unit has two groups. Group A still has the exact
  \* Queue owner; group B is already physically absent. Neither member has
  \* published terminal evidence yet.
  /\ canonicalGroupAQueueOwned' = TRUE
  /\ canonicalGroupBQueueOwned' = FALSE
  /\ canonicalGroupATerminalPublished' = FALSE
  /\ canonicalGroupBTerminalPublished' = FALSE
  /\ canonicalCarrierUnitDeferred' = FALSE
  /\ deferredCarrierPlannedFromSnapshot' = FALSE
  /\ normalCarrierApplyCompleted' = FALSE
  /\ queueOwnershipSnapshotReceiptValid' = FALSE
  /\ snapshotGroupAQueueOwned' = FALSE
  /\ snapshotGroupBQueueOwned' = FALSE
  /\ UNCHANGED <<stage, reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, laneOwns, mergeOwns, releaseOwns,
                 committedOwner, executionCount, controlOnlyAnchor,
                 candidateBodyDurable, candidateAuthorized, slotRetired,
                 releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface,
                 durableStageRank>>
  /\ UNCHANGED <<releaseFinalizationAuthorized,
                 terminalStartupGateClosed, terminalSweepStarted,
                 terminalSweepCompleted, queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticIdentityExact,
                 diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars

\* A crash may interrupt the per-attempt Pending fsync loop. Before Queue is
\* touched, recovery reconstructs and durably validates the complete carrier
\* outcome set, including groups not present in the observed prefix.
ReconstructCompleteCanonicalTerminalOutcomeSet ==
  /\ terminalOutcomeStage = "Pending"
  /\ terminalOutcomeSource = "Canonical"
  /\ ~queueTerminalPhysical
  /\ ~canonicalOutcomeSetComplete
  /\ canonicalOutcomeSetComplete' = TRUE
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken,
                 canonicalCarrierBatchPreflighted,
                 partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

\* The checked ApplyCarrier capability is reconstructed independently from
\* the committed merge entry, carrier block, State membership, exact source
\* group, and authenticated source-bundle projection.
ReconstructCanonicalCarrierCleanupAuthorization ==
  /\ terminalOutcomeStage = "Pending"
  /\ terminalOutcomeSource = "Canonical"
  /\ canonicalOutcomeSetComplete
  /\ ~canonicalCarrierCleanupAuthorized
  /\ canonicalCarrierCleanupAuthorized' = TRUE
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

\* Every independently authenticated ApplyCarrier group enters one Queue
\* all-group preflight. No group cleanup is visible before this succeeds.
PreflightCanonicalCarrierTerminalBatch ==
  /\ terminalOutcomeStage = "Pending"
  /\ terminalOutcomeSource = "Canonical"
  /\ canonicalOutcomeSetComplete
  /\ canonicalCarrierCleanupAuthorized
  /\ ~canonicalCarrierBatchPreflighted
  /\ canonicalCarrierBatchPreflighted' = TRUE
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken, canonicalOutcomeSetComplete,
                 partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

\* Queue either completes a live/partial exact owner or reauthenticates an
\* already-empty physical terminal after restart. Only the independently
\* reconstructed carrier capability can mint this positive evidence.
PublishCanonicalQueueTerminalEvidence ==
  /\ terminalOutcomeStage = "Pending"
  /\ terminalOutcomeSource = "Canonical"
  /\ canonicalCarrierCleanupAuthorized
  /\ canonicalOutcomeSetComplete
  /\ canonicalCarrierBatchPreflighted
  /\ ~partialCanonicalCleanup
  \* The startup sweep may reauthenticate an all-empty unit. A unit with any
  \* Queue owner is instead deferred intact, and normal carrier application
  \* may consume it only after planning from the original immutable receipt.
  /\ IF terminalStartupGateClosed
        THEN /\ terminalSweepStarted
             /\ queueOwnershipSnapshotTaken
             /\ queueOwnershipSnapshotReceiptValid
             /\ IF terminalSweepCompleted
                   THEN /\ canonicalCarrierUnitDeferred
                        /\ deferredCarrierPlannedFromSnapshot
                   ELSE /\ ~snapshotGroupAQueueOwned
                        /\ ~snapshotGroupBQueueOwned
        ELSE TRUE
  /\ \/ /\ ~queueTerminalPhysical
        /\ stage = "Applied"
        /\ committedOwner
     \/ /\ queueTerminalPhysical
        /\ stage = "Forgotten"
        /\ ~committedOwner
  /\ stage' = "Forgotten"
  /\ claimState' = "None"
  /\ committedOwner' = FALSE
  /\ durableStageRank' = 9
  /\ queueTerminalPhysical' = TRUE
  /\ positiveQueueTerminalEvidence' = TRUE
  /\ canonicalGroupAQueueOwned' = FALSE
  /\ canonicalGroupBQueueOwned' = FALSE
  /\ canonicalGroupATerminalPublished' = TRUE
  /\ canonicalGroupBTerminalPublished' = TRUE
  /\ normalCarrierApplyCompleted' =
       (normalCarrierApplyCompleted \/ canonicalCarrierUnitDeferred)
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 queueOwns, laneOwns, mergeOwns, releaseOwns,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, released, releaseAfterApply, recreated,
                 staleRelease, reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid,
                 snapshotGroupAQueueOwned, snapshotGroupBQueueOwned,
                 canonicalCarrierUnitDeferred,
                 deferredCarrierPlannedFromSnapshot>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticIdentityExact,
                 diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars

\* Kura may replace Pending with Complete only after consuming Queue's
\* move-only positive terminal projection and revalidating the source again.
PromoteTerminalOutcomeComplete ==
  /\ terminalOutcomeStage = "Pending"
  /\ positiveQueueTerminalEvidence
  /\ queueTerminalPhysical
  /\ terminalOutcomeStage' = "Complete"
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

BeginLosingSlotRetirement ==
  /\ stage \in {"Reserved", "Anchored", "Certified", "CandidateDurable"}
  /\ recoveryStage = "Normal"
  /\ laneOwns
  /\ claimState = "Active"
  /\ stage' = "ReleasePending"
  /\ claimState' = "ReleasePending"
  /\ laneOwns' = FALSE
  /\ releaseOwns' = TRUE
  /\ slotRetired' = TRUE
  /\ candidateBodyDurable' = FALSE
  /\ candidateAuthorized' = FALSE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 queueOwns, mergeOwns, committedOwner, executionCount,
                 controlOnlyAnchor, releaseBarrier, releaseCompletion,
                 released, releaseAfterApply, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

PrepareQueueReleaseBarrier ==
  /\ stage = "ReleasePending"
  /\ releaseOwns
  /\ claimState = "ReleasePending"
  /\ ~releaseBarrier
  /\ releaseBarrier' = TRUE
  /\ UNCHANGED <<stage, reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, laneOwns, mergeOwns, releaseOwns,
                 committedOwner, executionCount, controlOnlyAnchor,
                 candidateBodyDurable, candidateAuthorized, slotRetired,
                 releaseCompletion, released, releaseAfterApply, recreated,
                 staleRelease, reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

PublishReleasedClaim ==
  /\ stage = "ReleasePending"
  /\ releaseOwns
  /\ claimState = "ReleasePending"
  /\ (releaseBarrier \/ Mode = "ReleaseBeforeBarrier")
  /\ stage' = "Released"
  /\ claimState' = "Released"
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 queueOwns, laneOwns, mergeOwns, releaseOwns,
                 committedOwner, executionCount, controlOnlyAnchor,
                 candidateBodyDurable, candidateAuthorized, slotRetired,
                 releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

\* Kura derives this move-only authorization from the exact retirement,
\* prepared Queue barrier, and durable Released claim. A restart may rebuild
\* it after physical FIFO restoration, but Pending never supplies it.
AuthorizeExactReleaseFinalization ==
  /\ claimState = "Released"
  /\ slotRetired
  /\ releaseBarrier
  /\ ~releaseFinalizationAuthorized
  /\ \/ /\ terminalOutcomeStage \in {"None", "Pending"}
        /\ (terminalOutcomeStage = "None"
             \/ terminalOutcomeSource = "Release")
        /\ stage = "Released"
        /\ releaseOwns
     \/ /\ terminalOutcomeStage = "Pending"
        /\ terminalOutcomeSource = "Release"
        /\ queueTerminalPhysical
        /\ stage = "Queued"
        /\ queueOwns
  /\ releaseFinalizationAuthorized' = TRUE
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

PersistReleaseTerminalOutcomePending ==
  /\ stage = "Released"
  /\ releaseOwns
  /\ claimState = "Released"
  /\ releaseFinalizationAuthorized
  /\ terminalOutcomeStage = "None"
  /\ terminalOutcomeStage' = "Pending"
  /\ terminalOutcomeSource' = "Release"
  /\ queueTerminalPhysical' = FALSE
  /\ positiveQueueTerminalEvidence' = FALSE
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

CompleteQueueRelease ==
  /\ terminalOutcomeStage = "Pending"
  /\ terminalOutcomeSource = "Release"
  /\ claimState = "Released"
  /\ releaseFinalizationAuthorized
  /\ \/ /\ ~queueTerminalPhysical
        /\ stage = "Released"
        /\ releaseOwns
     \/ /\ queueTerminalPhysical
        /\ stage = "Queued"
        /\ queueOwns
  /\ stage' = "Queued"
  /\ queueOwns' = TRUE
  /\ releaseOwns' = FALSE
  /\ releaseCompletion' = TRUE
  /\ released' = TRUE
  /\ queueTerminalPhysical' = TRUE
  /\ positiveQueueTerminalEvidence' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, laneOwns, mergeOwns, committedOwner,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

ReserveRecreatedIncarnation ==
  /\ stage = "Queued"
  /\ recoveryStage = "Normal"
  /\ queueGateOpen
  /\ queueOwns
  /\ released
  /\ releaseCompletion
  /\ terminalOutcomeStage = "Complete"
  /\ terminalOutcomeSource = "Release"
  /\ positiveQueueTerminalEvidence
  /\ ~recreated
  /\ executionCount = 0
  /\ stage' = "Reserved"
  /\ reservationIdentity' = RecreatedReservationIdentity
  /\ carrierIdentity' = RecreatedReservationIdentity
  /\ incarnation' = IncarnationB
  /\ claimState' = "Active"
  /\ queueOwns' = FALSE
  /\ laneOwns' = TRUE
  /\ mergeOwns' = FALSE
  /\ releaseOwns' = FALSE
  /\ committedOwner' = FALSE
  /\ controlOnlyAnchor' = FALSE
  /\ candidateBodyDurable' = FALSE
  /\ candidateAuthorized' = FALSE
  /\ slotRetired' = FALSE
  /\ released' = FALSE
  /\ releaseAfterApply' = FALSE
  /\ recreated' = TRUE
  /\ staleRelease' = FALSE
  /\ reservationDurable' = TRUE
  /\ mergeCandidateExact' = FALSE
  /\ canonicalReexecuted' = FALSE
  /\ durableStageRank' = 1
  /\ diagnosticStageRank' = 0
  /\ diagnosticIdentityExact' = TRUE
  /\ diagnosticsAuthorizeState' = FALSE
  /\ terminalOutcomeStage' = "None"
  /\ terminalOutcomeSource' = "None"
  /\ canonicalCarrierCleanupAuthorized' = FALSE
  /\ releaseFinalizationAuthorized' = FALSE
  /\ queueTerminalPhysical' = FALSE
  /\ positiveQueueTerminalEvidence' = FALSE
  /\ terminalStartupGateClosed' = FALSE
  /\ terminalSweepStarted' = FALSE
  /\ terminalSweepCompleted' = FALSE
  /\ queueOwnershipSnapshotTaken' = FALSE
  /\ queueOwnershipSnapshotReceiptValid' = FALSE
  /\ canonicalGroupAQueueOwned' = FALSE
  /\ canonicalGroupBQueueOwned' = FALSE
  /\ snapshotGroupAQueueOwned' = FALSE
  /\ snapshotGroupBQueueOwned' = FALSE
  /\ canonicalGroupATerminalPublished' = FALSE
  /\ canonicalGroupBTerminalPublished' = FALSE
  /\ canonicalCarrierUnitDeferred' = FALSE
  /\ deferredCarrierPlannedFromSnapshot' = FALSE
  /\ normalCarrierApplyCompleted' = FALSE
  /\ canonicalOutcomeSetComplete' = FALSE
  /\ canonicalCarrierBatchPreflighted' = FALSE
  /\ partialCanonicalCleanup' = FALSE
  /\ UNCHANGED <<executionCount, releaseBarrier, releaseCompletion,
                 carrierCommitSurface>>
  /\ UNCHANGED recoveryVars

ReplayStaleReleaseMutation ==
  /\ Mode = "AbaRelease"
  /\ stage = "Reserved"
  /\ recreated
  /\ incarnation = IncarnationB
  /\ laneOwns
  /\ releaseBarrier
  /\ releaseCompletion
  /\ stage' = "Queued"
  /\ claimState' = "Released"
  /\ queueOwns' = TRUE
  /\ laneOwns' = FALSE
  /\ released' = TRUE
  /\ staleRelease' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 mergeOwns, releaseOwns, committedOwner, executionCount,
                 controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, releaseAfterApply, recreated>>
  /\ UNCHANGED <<reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

ReleaseCommittedMutation ==
  /\ Mode = "ReleaseAfterApplication"
  /\ stage = "Applied"
  /\ committedOwner
  /\ stage' = "Queued"
  /\ claimState' = "Released"
  /\ queueOwns' = TRUE
  /\ committedOwner' = FALSE
  /\ released' = TRUE
  /\ releaseAfterApply' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 laneOwns, mergeOwns, releaseOwns, executionCount,
                 controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

RestartDropsOwnershipMutation ==
  /\ Mode = "RestartDropsOwnership"
  /\ stage \in
       {"Reserved", "Anchored", "Certified", "CandidateDurable",
        "CandidateAuthorized", "PreVoteAuthorized", "CarrierFinalized",
        "Applied"}
  /\ (queueOwns \/ laneOwns \/ mergeOwns \/ releaseOwns \/ committedOwner)
  /\ queueOwns' = FALSE
  /\ laneOwns' = FALSE
  /\ mergeOwns' = FALSE
  /\ releaseOwns' = FALSE
  /\ committedOwner' = FALSE
  /\ UNCHANGED <<stage, reservationIdentity, carrierIdentity, incarnation,
                 claimState, executionCount, controlOnlyAnchor,
                 candidateBodyDurable, candidateAuthorized, slotRetired,
                 releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

\* Restart discards move-only Queue/Kura capabilities but retains both the
\* hash-protected Pending record and any physical Queue terminal already
\* crossed. The shared Queue startup gate closes before recovery begins.
RestartWithPendingTerminalOutcome ==
  /\ terminalOutcomeStage = "Pending"
  /\ queueGateOpen
  /\ ~terminalStartupGateClosed
  /\ queueGateOpen' = FALSE
  /\ canonicalCarrierCleanupAuthorized' =
       IF queueTerminalPhysical THEN canonicalCarrierCleanupAuthorized ELSE FALSE
  /\ releaseFinalizationAuthorized' = FALSE
  /\ positiveQueueTerminalEvidence' = FALSE
  /\ canonicalCarrierBatchPreflighted' =
       IF queueTerminalPhysical THEN canonicalCarrierBatchPreflighted ELSE FALSE
  /\ terminalStartupGateClosed' = TRUE
  /\ terminalSweepStarted' = FALSE
  /\ terminalSweepCompleted' = FALSE
  /\ queueOwnershipSnapshotTaken' = FALSE
  /\ queueOwnershipSnapshotReceiptValid' = FALSE
  /\ snapshotGroupAQueueOwned' = FALSE
  /\ snapshotGroupBQueueOwned' = FALSE
  /\ canonicalCarrierUnitDeferred' = FALSE
  /\ deferredCarrierPlannedFromSnapshot' = FALSE
  /\ UNCHANGED <<carrierVars, diagnosticVars>>
  /\ UNCHANGED <<recoveryStage, recoverySignerStable,
                 recoveryWireLengthExact, localQueueReservationGroupExact,
                 networkIngressStartupFenced, queueOwnerQuarantinePending>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 queueTerminalPhysical, canonicalOutcomeSetComplete,
                 partialCanonicalCleanup>>
  /\ UNCHANGED <<canonicalGroupAQueueOwned, canonicalGroupBQueueOwned,
                 canonicalGroupATerminalPublished,
                 canonicalGroupBTerminalPublished,
                 normalCarrierApplyCompleted>>

\* Production takes exactly one immutable Queue reconciliation snapshot before
\* inspecting any Pending terminal unit. In the bounded mixed unit, A is
\* Queue-owned and B is already absent.
TakeInitialQueueOwnershipSnapshot ==
  /\ terminalStartupGateClosed
  /\ ~queueGateOpen
  /\ terminalOutcomeStage = "Pending"
  /\ ~terminalSweepStarted
  /\ ~queueOwnershipSnapshotTaken
  /\ queueOwnershipSnapshotTaken' = TRUE
  /\ queueOwnershipSnapshotReceiptValid' = TRUE
  /\ snapshotGroupAQueueOwned' = canonicalGroupAQueueOwned
  /\ snapshotGroupBQueueOwned' = canonicalGroupBQueueOwned
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence,
                 terminalStartupGateClosed, terminalSweepStarted,
                 terminalSweepCompleted>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<canonicalGroupAQueueOwned, canonicalGroupBQueueOwned,
                 canonicalGroupATerminalPublished,
                 canonicalGroupBTerminalPublished,
                 canonicalCarrierUnitDeferred,
                 deferredCarrierPlannedFromSnapshot,
                 normalCarrierApplyCompleted>>

BeginTerminalOutcomeStartupSweep ==
  /\ terminalStartupGateClosed
  /\ ~queueGateOpen
  /\ terminalOutcomeStage = "Pending"
  /\ ~terminalSweepStarted
  /\ queueOwnershipSnapshotTaken
  /\ queueOwnershipSnapshotReceiptValid
  /\ terminalSweepStarted' = TRUE
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence,
                 terminalStartupGateClosed, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

\* Pre-sweep classifies the complete canonical unit with an ANY-owned
\* predicate. A Queue owner on A therefore defers A+B together even though B
\* is absent. No Queue/Kura terminal state changes in this action.
DeferCanonicalTerminalUnitWithQueueOwner ==
  /\ terminalStartupGateClosed
  /\ ~queueGateOpen
  /\ terminalSweepStarted
  /\ ~terminalSweepCompleted
  /\ terminalOutcomeStage = "Pending"
  /\ terminalOutcomeSource = "Canonical"
  /\ canonicalOutcomeSetComplete
  /\ canonicalCarrierCleanupAuthorized
  /\ canonicalCarrierBatchPreflighted
  /\ queueOwnershipSnapshotTaken
  /\ queueOwnershipSnapshotReceiptValid
  /\ (snapshotGroupAQueueOwned \/ snapshotGroupBQueueOwned)
  /\ ~canonicalCarrierUnitDeferred
  /\ ~canonicalGroupATerminalPublished
  /\ ~canonicalGroupBTerminalPublished
  /\ canonicalCarrierUnitDeferred' = TRUE
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid,
                 canonicalGroupAQueueOwned, canonicalGroupBQueueOwned,
                 snapshotGroupAQueueOwned, snapshotGroupBQueueOwned,
                 canonicalGroupATerminalPublished,
                 canonicalGroupBTerminalPublished,
                 deferredCarrierPlannedFromSnapshot,
                 normalCarrierApplyCompleted>>

FinishTerminalOutcomeStartupSweep ==
  /\ terminalStartupGateClosed
  /\ ~queueGateOpen
  /\ terminalSweepStarted
  /\ ~terminalSweepCompleted
  /\ \/ /\ terminalOutcomeStage = "Complete"
          /\ positiveQueueTerminalEvidence
     \/ /\ terminalOutcomeStage = "Pending"
          /\ terminalOutcomeSource = "Canonical"
          /\ canonicalCarrierUnitDeferred
          /\ queueOwnershipSnapshotTaken
          /\ queueOwnershipSnapshotReceiptValid
          /\ ~canonicalGroupATerminalPublished
          /\ ~canonicalGroupBTerminalPublished
  /\ terminalSweepCompleted' = TRUE
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence,
                 terminalStartupGateClosed, terminalSweepStarted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

\* The ordinary planner receives the deferred unit and the exact first
\* reconciliation receipt. This does not retake Queue state or weaken the
\* receipt; normal carrier application consumes the planned unit later.
PlanDeferredCanonicalCarrierFromInitialSnapshot ==
  /\ terminalStartupGateClosed
  /\ ~queueGateOpen
  /\ terminalSweepCompleted
  /\ terminalOutcomeStage = "Pending"
  /\ terminalOutcomeSource = "Canonical"
  /\ canonicalCarrierUnitDeferred
  /\ queueOwnershipSnapshotTaken
  /\ queueOwnershipSnapshotReceiptValid
  /\ ~deferredCarrierPlannedFromSnapshot
  /\ deferredCarrierPlannedFromSnapshot' = TRUE
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence,
                 terminalStartupGateClosed, terminalSweepStarted,
                 terminalSweepCompleted, queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid,
                 canonicalGroupAQueueOwned, canonicalGroupBQueueOwned,
                 snapshotGroupAQueueOwned, snapshotGroupBQueueOwned,
                 canonicalGroupATerminalPublished,
                 canonicalGroupBTerminalPublished,
                 canonicalCarrierUnitDeferred, normalCarrierApplyCompleted>>

OpenQueueAfterTerminalOutcomePlanning ==
  /\ terminalStartupGateClosed
  /\ ~queueGateOpen
  /\ terminalSweepCompleted
  /\ queueOwnershipSnapshotTaken
  /\ queueOwnershipSnapshotReceiptValid
  /\ terminalOutcomeStage # "Pending"
  /\ (terminalOutcomeSource = "Canonical" =>
        /\ canonicalGroupATerminalPublished
        /\ canonicalGroupBTerminalPublished)
  /\ (canonicalCarrierUnitDeferred =>
        /\ deferredCarrierPlannedFromSnapshot
        /\ normalCarrierApplyCompleted)
  /\ queueGateOpen' = TRUE
  /\ terminalStartupGateClosed' = FALSE
  /\ terminalSweepStarted' = FALSE
  /\ terminalSweepCompleted' = FALSE
  /\ UNCHANGED <<carrierVars, diagnosticVars>>
  /\ UNCHANGED <<recoveryStage, recoverySignerStable,
                 recoveryWireLengthExact, localQueueReservationGroupExact,
                 networkIngressStartupFenced, queueOwnerQuarantinePending>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

\* Pending is wrongly treated as the second ApplyCarrier capability and Queue
\* publishes canonical terminality without an independent authorization.
PendingOnlyCanonicalTerminalMutation ==
  /\ Mode = "PendingOnlyCanonicalTerminal"
  /\ terminalOutcomeStage = "Pending"
  /\ terminalOutcomeSource = "Canonical"
  /\ ~canonicalCarrierCleanupAuthorized
  /\ ~queueTerminalPhysical
  /\ ~positiveQueueTerminalEvidence
  /\ stage = "Applied"
  /\ committedOwner
  /\ stage' = "Forgotten"
  /\ claimState' = "None"
  /\ committedOwner' = FALSE
  /\ durableStageRank' = 9
  /\ queueTerminalPhysical' = TRUE
  /\ positiveQueueTerminalEvidence' = TRUE
  /\ canonicalOutcomeSetComplete' = TRUE
  /\ canonicalCarrierBatchPreflighted' = FALSE
  /\ partialCanonicalCleanup' = FALSE
  /\ canonicalGroupAQueueOwned' = FALSE
  /\ canonicalGroupBQueueOwned' = FALSE
  /\ canonicalGroupATerminalPublished' = TRUE
  /\ canonicalGroupBTerminalPublished' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 queueOwns, laneOwns, mergeOwns, releaseOwns,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, released, releaseAfterApply, recreated,
                 staleRelease, reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticIdentityExact,
                 diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid,
                 snapshotGroupAQueueOwned, snapshotGroupBQueueOwned,
                 canonicalCarrierUnitDeferred,
                 deferredCarrierPlannedFromSnapshot,
                 normalCarrierApplyCompleted>>

\* ML-MUT-AUT-13: a broken empty-only sweep examines group B in isolation.
\* Because B is absent it publishes B, even though sibling A is Queue-owned.
\* Production must defer the complete A+B unit before publishing either.
SweepOnlyAbsentCanonicalGroupMutation ==
  /\ Mode = "PartialTerminalUnitSweep"
  /\ terminalStartupGateClosed
  /\ ~queueGateOpen
  /\ terminalSweepStarted
  /\ ~terminalSweepCompleted
  /\ queueOwnershipSnapshotTaken
  /\ queueOwnershipSnapshotReceiptValid
  /\ terminalOutcomeStage = "Pending"
  /\ terminalOutcomeSource = "Canonical"
  /\ canonicalOutcomeSetComplete
  /\ canonicalCarrierCleanupAuthorized
  /\ canonicalCarrierBatchPreflighted
  /\ ~partialCanonicalCleanup
  /\ ~queueTerminalPhysical
  /\ ~positiveQueueTerminalEvidence
  /\ snapshotGroupAQueueOwned
  /\ ~snapshotGroupBQueueOwned
  /\ canonicalGroupAQueueOwned
  /\ ~canonicalGroupBQueueOwned
  /\ ~canonicalGroupATerminalPublished
  /\ ~canonicalGroupBTerminalPublished
  /\ canonicalGroupBTerminalPublished' = TRUE
  /\ partialCanonicalCleanup' = TRUE
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken, canonicalOutcomeSetComplete,
                 canonicalCarrierBatchPreflighted>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid,
                 canonicalGroupAQueueOwned, canonicalGroupBQueueOwned,
                 snapshotGroupAQueueOwned, snapshotGroupBQueueOwned,
                 canonicalGroupATerminalPublished,
                 canonicalCarrierUnitDeferred,
                 deferredCarrierPlannedFromSnapshot,
                 normalCarrierApplyCompleted>>

\* Release cleanup accepts Pending after discarding the exact retirement and
\* finalization capability which production reconstructs from Kura.
ReleaseWithoutFinalizationAuthorityMutation ==
  /\ Mode = "ReleaseWithoutFinalizationAuthority"
  /\ terminalOutcomeStage = "Pending"
  /\ terminalOutcomeSource = "Release"
  /\ releaseFinalizationAuthorized
  /\ ~queueTerminalPhysical
  /\ ~positiveQueueTerminalEvidence
  /\ stage = "Released"
  /\ releaseOwns
  /\ releaseFinalizationAuthorized' = FALSE
  /\ stage' = "Queued"
  /\ queueOwns' = TRUE
  /\ releaseOwns' = FALSE
  /\ releaseCompletion' = TRUE
  /\ released' = TRUE
  /\ queueTerminalPhysical' = TRUE
  /\ positiveQueueTerminalEvidence' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, laneOwns, mergeOwns, committedOwner,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface,
                 durableStageRank>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

CompleteWithoutQueueEvidenceMutation ==
  /\ Mode = "CompleteWithoutQueueEvidence"
  /\ terminalOutcomeStage = "Pending"
  /\ ~positiveQueueTerminalEvidence
  /\ terminalOutcomeStage' = "Complete"
  /\ UNCHANGED <<carrierVars, diagnosticVars, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence, terminalStartupGateClosed,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

\* ML-MUT-AUT-14a: Queue ownership is changed after the first receipt and
\* before the deferred carrier is planned from that receipt. The persisted
\* replay receipt is consequently no longer an exact immutable witness.
MutateOwnedRecoveryGroupBeforePlannerMutation ==
  /\ Mode = "OwnedGroupMutationBeforePlanner"
  /\ terminalStartupGateClosed
  /\ ~queueGateOpen
  /\ terminalSweepStarted
  /\ terminalSweepCompleted
  /\ terminalOutcomeStage = "Pending"
  /\ terminalOutcomeSource = "Canonical"
  /\ queueOwnershipSnapshotTaken
  /\ queueOwnershipSnapshotReceiptValid
  /\ snapshotGroupAQueueOwned
  /\ ~snapshotGroupBQueueOwned
  /\ canonicalGroupAQueueOwned
  /\ ~canonicalGroupBQueueOwned
  /\ canonicalCarrierUnitDeferred
  /\ ~deferredCarrierPlannedFromSnapshot
  /\ stage = "Applied"
  /\ committedOwner
  /\ stage' = "Forgotten"
  /\ claimState' = "None"
  /\ committedOwner' = FALSE
  /\ durableStageRank' = 9
  /\ canonicalGroupAQueueOwned' = FALSE
  /\ queueOwnershipSnapshotReceiptValid' = FALSE
  /\ partialCanonicalCleanup' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 queueOwns, laneOwns, mergeOwns, releaseOwns,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, released, releaseAfterApply, recreated,
                 staleRelease, reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticIdentityExact,
                 diagnosticsAuthorizeState, recoveryVars>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence,
                 terminalStartupGateClosed, terminalSweepStarted,
                 terminalSweepCompleted, queueOwnershipSnapshotTaken,
                 canonicalOutcomeSetComplete,
                 canonicalCarrierBatchPreflighted>>
  /\ UNCHANGED <<canonicalGroupBQueueOwned,
                 snapshotGroupAQueueOwned, snapshotGroupBQueueOwned,
                 canonicalGroupATerminalPublished,
                 canonicalGroupBTerminalPublished,
                 canonicalCarrierUnitDeferred,
                 deferredCarrierPlannedFromSnapshot,
                 normalCarrierApplyCompleted>>

\* ML-MUT-AUT-14b: publication reopens after planning but before normal
\* carrier application has atomically completed both deferred members.
OpenQueueBeforeDeferredCarrierApplyMutation ==
  /\ Mode = "OpenQueueBeforeDeferredCarrierApply"
  /\ terminalStartupGateClosed
  /\ ~queueGateOpen
  /\ terminalSweepStarted
  /\ terminalSweepCompleted
  /\ terminalOutcomeStage = "Pending"
  /\ terminalOutcomeSource = "Canonical"
  /\ queueOwnershipSnapshotTaken
  /\ queueOwnershipSnapshotReceiptValid
  /\ canonicalCarrierUnitDeferred
  /\ deferredCarrierPlannedFromSnapshot
  /\ ~normalCarrierApplyCompleted
  /\ queueGateOpen' = TRUE
  /\ terminalStartupGateClosed' = FALSE
  /\ UNCHANGED <<carrierVars, diagnosticVars>>
  /\ UNCHANGED <<recoveryStage, recoverySignerStable,
                 recoveryWireLengthExact, localQueueReservationGroupExact,
                 networkIngressStartupFenced, queueOwnerQuarantinePending>>
  /\ UNCHANGED <<terminalOutcomeStage, terminalOutcomeSource,
                 canonicalCarrierCleanupAuthorized,
                 releaseFinalizationAuthorized, queueTerminalPhysical,
                 positiveQueueTerminalEvidence,
                 terminalSweepStarted, terminalSweepCompleted,
                 queueOwnershipSnapshotTaken>>
  /\ UNCHANGED <<canonicalOutcomeSetComplete, canonicalCarrierBatchPreflighted, partialCanonicalCleanup>>
  /\ UNCHANGED <<queueOwnershipSnapshotReceiptValid, canonicalGroupAQueueOwned, canonicalGroupBQueueOwned, snapshotGroupAQueueOwned, snapshotGroupBQueueOwned, canonicalGroupATerminalPublished, canonicalGroupBTerminalPublished, canonicalCarrierUnitDeferred, deferredCarrierPlannedFromSnapshot, normalCarrierApplyCompleted>>

\* The local producer classification carries exact current Queue ownership
\* into the closed-gate Crash/Recover cut. This is stronger than Kura payload
\* custody and cannot be reconstructed from an observer's local files.
ClassifyLocalProducerRetainedAttempt ==
  /\ stage \in RetainedAttemptStages
  /\ laneOwns
  /\ reservationDurable
  /\ recoveryStage = "Normal"
  /\ queueGateOpen
  /\ recoveryStage' = "LocalProducerRetained"
  /\ queueGateOpen' = FALSE
  /\ localQueueReservationGroupExact' = TRUE
  /\ networkIngressStartupFenced' = TRUE
  /\ queueOwnerQuarantinePending' = TRUE
  /\ UNCHANGED <<carrierVars, diagnosticVars, terminalVars>>
  /\ UNCHANGED <<recoverySignerStable, recoveryWireLengthExact>>

\* An observer has exact Kura custody but no local Queue reservation group.
\* That absence is legal because it never held the producer's Queue owner.
ClassifyObserverKuraRetainedAttempt ==
  /\ stage \in RetainedAttemptStages
  /\ laneOwns
  /\ reservationDurable
  /\ recoveryStage = "Normal"
  /\ queueGateOpen
  /\ recoveryStage' = "ObserverKuraRetained"
  /\ localQueueReservationGroupExact' = FALSE
  /\ networkIngressStartupFenced' = TRUE
  /\ queueOwnerQuarantinePending' = FALSE
  /\ UNCHANGED <<carrierVars, diagnosticVars, terminalVars>>
  /\ UNCHANGED <<queueGateOpen, recoverySignerStable,
                 recoveryWireLengthExact>>

BeginLocalRetainedAttemptRecovery ==
  /\ stage \in RetainedAttemptStages
  /\ laneOwns
  /\ networkIngressStartupFenced
  /\ \/ /\ recoveryStage = "LocalProducerRetained"
        /\ localQueueReservationGroupExact
        /\ queueOwnerQuarantinePending
        /\ ~queueGateOpen
        /\ recoveryStage' = "LocalProducerRecovering"
     \/ /\ recoveryStage = "ObserverKuraRetained"
        /\ ~localQueueReservationGroupExact
        /\ ~queueOwnerQuarantinePending
        /\ recoveryStage' = "ObserverKuraRecovering"
  /\ UNCHANGED <<carrierVars, diagnosticVars, terminalVars>>
  /\ UNCHANGED <<queueGateOpen, recoverySignerStable,
                 recoveryWireLengthExact, localQueueReservationGroupExact,
                 networkIngressStartupFenced, queueOwnerQuarantinePending>>

CompleteLocalRetainedAttemptRecovery ==
  /\ recoveryStage \in
       {"LocalProducerRecovering", "ObserverKuraRecovering"}
  /\ networkIngressStartupFenced
  /\ ~terminalStartupGateClosed
  /\ recoveryStage' = "Normal"
  /\ queueGateOpen' = TRUE
  /\ localQueueReservationGroupExact' = FALSE
  /\ networkIngressStartupFenced' = FALSE
  /\ queueOwnerQuarantinePending' = FALSE
  /\ UNCHANGED <<carrierVars, diagnosticVars, terminalVars>>
  /\ UNCHANGED <<recoverySignerStable, recoveryWireLengthExact>>

\* ML-MUT-AUT-15: Crash/Recover starts for the local producer after dropping
\* the exact current Queue group. Kura custody alone cannot replace that owner.
RecoverLocalProducerWithoutQueueOwnerMutation ==
  /\ Mode = "ProducerRecoveryWithoutQueueOwner"
  /\ stage \in RetainedAttemptStages
  /\ laneOwns
  /\ recoveryStage = "LocalProducerRetained"
  /\ ~queueGateOpen
  /\ localQueueReservationGroupExact
  /\ networkIngressStartupFenced
  /\ queueOwnerQuarantinePending
  /\ recoveryStage' = "LocalProducerRecovering"
  /\ localQueueReservationGroupExact' = FALSE
  /\ UNCHANGED <<carrierVars, diagnosticVars, terminalVars>>
  /\ UNCHANGED <<queueGateOpen, recoverySignerStable,
                 recoveryWireLengthExact, networkIngressStartupFenced,
                 queueOwnerQuarantinePending>>

\* A globally finalized autonomous control survived, but this peer pruned or
\* never received its canonical body. Queue selection closes before recovery;
\* the exact reservation remains lane-owned throughout the startup repair.
RestartNeedsCanonicalCarrierBody ==
  /\ stage = "Anchored"
  /\ laneOwns
  /\ claimState = "Active"
  /\ reservationDurable
  /\ recoveryStage = "Normal"
  /\ queueGateOpen
  /\ recoveryStage' = "NeedBody"
  /\ queueGateOpen' = FALSE
  /\ recoverySignerStable' = TRUE
  /\ recoveryWireLengthExact' = TRUE
  /\ UNCHANGED <<carrierVars, diagnosticVars>>
  /\ UNCHANGED <<localQueueReservationGroupExact,
                 networkIngressStartupFenced, queueOwnerQuarantinePending>>
  /\ UNCHANGED terminalVars

\* One signer owns a whole fixed-chunk assembly. Changing signer or accepting a
\* body not authenticated by the retained Commit QC is a modeled mutation.
AcceptRecoveredCanonicalCarrierBody ==
  /\ stage = "Anchored"
  /\ laneOwns
  /\ recoveryStage = "NeedBody"
  /\ ~queueGateOpen
  /\ recoveryStage' =
       IF Mode = "UnauthenticatedRecoveryBody"
       THEN "BodyAcceptedUnauthenticated"
       ELSE "BodyVerified"
  /\ recoverySignerStable' = (Mode # "MixedSignerRecoveryBody")
  /\ recoveryWireLengthExact' = (Mode # "InflatedRecoveryWireLength")
  /\ UNCHANGED queueGateOpen
  /\ UNCHANGED <<carrierVars, diagnosticVars>>
  /\ UNCHANGED <<localQueueReservationGroupExact,
                 networkIngressStartupFenced, queueOwnerQuarantinePending>>
  /\ UNCHANGED terminalVars

\* Installation abstracts one versioned, fsynced, read-back Kura task carrying
\* the canonical finality/body binding, exact historical route/incarnation,
\* predecessor, proposal, committee/quorum, and validator-aligned PoPs.
InstallHistoricalAutonomousRecovery ==
  /\ stage = "Anchored"
  /\ laneOwns
  /\ recoveryStage \in {"BodyVerified", "BodyAcceptedUnauthenticated"}
  /\ ~queueGateOpen
  /\ recoveryStage' =
       IF recoveryStage = "BodyAcceptedUnauthenticated"
       THEN "TaskUnauthenticated"
       ELSE IF Mode = "HistoricalContextDrift"
            THEN "TaskDrifted"
            ELSE "TaskExact"
  /\ UNCHANGED <<queueGateOpen, recoverySignerStable,
                 recoveryWireLengthExact, localQueueReservationGroupExact,
                 networkIngressStartupFenced, queueOwnerQuarantinePending>>
  /\ UNCHANGED <<carrierVars, diagnosticVars>>
  /\ UNCHANGED terminalVars

\* The production planner validates every reservation group before applying
\* any Queue transition. A partial prefix never becomes a publishable owner.
PreflightAllHistoricalReservationGroups ==
  /\ stage = "Anchored"
  /\ laneOwns
  /\ recoveryStage \in
       {"TaskExact", "TaskUnauthenticated", "TaskDrifted"}
  /\ ~queueGateOpen
  /\ recoveryStage' =
       CASE recoveryStage = "TaskUnauthenticated"
              -> "UnauthenticatedPreflight"
         [] recoveryStage = "TaskDrifted"
              -> "ContextDriftPreflight"
         [] Mode = "PartialRecoveryGroupPreflight"
              -> "PartialPreflight"
         [] OTHER -> "GroupsPreflight"
  /\ UNCHANGED <<queueGateOpen, recoverySignerStable,
                 recoveryWireLengthExact, localQueueReservationGroupExact,
                 networkIngressStartupFenced, queueOwnerQuarantinePending>>
  /\ UNCHANGED <<carrierVars, diagnosticVars>>
  /\ UNCHANGED terminalVars

OpenQueueAfterHistoricalInstall ==
  /\ stage = "Anchored"
  /\ laneOwns
  /\ recoveryStage \in
       {"GroupsPreflight", "UnauthenticatedPreflight",
        "ContextDriftPreflight", "PartialPreflight"}
  /\ ~queueGateOpen
  /\ queueGateOpen' = TRUE
  /\ UNCHANGED <<recoveryStage, recoverySignerStable,
                 recoveryWireLengthExact, localQueueReservationGroupExact,
                 networkIngressStartupFenced, queueOwnerQuarantinePending>>
  /\ UNCHANGED <<carrierVars, diagnosticVars>>
  /\ UNCHANGED terminalVars

\* ML-MUT-AUT-06: ordinary selection becomes visible before authenticated
\* recovery, durable historical-task installation, and all-group preflight.
OpenQueueBeforeHistoricalInstallMutation ==
  /\ Mode = "OpenQueueBeforeRecoveryInstall"
  /\ stage = "Anchored"
  /\ laneOwns
  /\ recoveryStage \in {"NeedBody", "BodyVerified", "TaskExact"}
  /\ ~queueGateOpen
  /\ queueGateOpen' = TRUE
  /\ UNCHANGED <<recoveryStage, recoverySignerStable,
                 recoveryWireLengthExact, localQueueReservationGroupExact,
                 networkIngressStartupFenced, queueOwnerQuarantinePending>>
  /\ UNCHANGED <<carrierVars, diagnosticVars>>
  /\ UNCHANGED terminalVars

\* Historical certification is deliberately after Queue reopening: startup
\* must not deadlock waiting for the old committee before ordinary work starts.
CertifyInstalledHistoricalAutonomousBundle ==
  /\ stage = "Anchored"
  /\ laneOwns
  /\ queueGateOpen
  /\ recoveryStage \in
       {"GroupsPreflight", "UnauthenticatedPreflight",
        "ContextDriftPreflight", "PartialPreflight"}
  /\ stage' = "Certified"
  /\ recoveryStage' = "HistoricalCertified"
  /\ durableStageRank' = 4
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, laneOwns, mergeOwns, releaseOwns,
                 committedOwner, executionCount, controlOnlyAnchor,
                 candidateBodyDurable, candidateAuthorized, slotRetired,
                 releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 canonicalReexecuted, carrierCommitSurface>>
  /\ UNCHANGED <<queueGateOpen, recoverySignerStable,
                 recoveryWireLengthExact, localQueueReservationGroupExact,
                 networkIngressStartupFenced, queueOwnerQuarantinePending>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticIdentityExact,
                 diagnosticsAuthorizeState>>
  /\ UNCHANGED terminalVars

\* Diagnostics may catch up to the highest revalidated durable stage for the
\* exact route/incarnation/proposal identity. They are observers: publishing a
\* row cannot change any ownership, authorization, or application state.
PublishDurableStageDiagnostic ==
  /\ diagnosticStageRank < durableStageRank
  /\ diagnosticStageRank' = durableStageRank
  /\ diagnosticIdentityExact' = TRUE
  /\ diagnosticsAuthorizeState' = FALSE
  /\ UNCHANGED <<carrierVars, durableStageRank>>
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

\* ML-MUT-LIFE-05: report one stage beyond durable evidence and let that
\* volatile projection fabricate the exact reservation-group identity and
\* become an authorization input.
PublishVolatileStageDiagnosticMutation ==
  /\ Mode = "VolatileStageDiagnostics"
  /\ durableStageRank < 9
  /\ diagnosticStageRank <= durableStageRank
  /\ diagnosticStageRank' = durableStageRank + 1
  /\ diagnosticIdentityExact' = FALSE
  /\ diagnosticsAuthorizeState' = TRUE
  /\ UNCHANGED <<carrierVars, durableStageRank>>
  /\ UNCHANGED recoveryVars
  /\ UNCHANGED terminalVars

Next ==
  \/ ReserveFifoTransaction
  \/ AnchorAutonomousControl
  \/ CertifyAutonomousBundle
  \/ PersistFullMergeCandidate
  \/ AuthorizeExactMergeCandidate
  \/ ValidatePostBlockPreVoteCarrierSurface
  \/ FinalizeCarrierCommitSurface
  \/ ApplyCanonicalCarrier
  \/ PersistCanonicalTerminalOutcomePending
  \/ ReconstructCompleteCanonicalTerminalOutcomeSet
  \/ ReconstructCanonicalCarrierCleanupAuthorization
  \/ PreflightCanonicalCarrierTerminalBatch
  \/ PublishCanonicalQueueTerminalEvidence
  \/ PromoteTerminalOutcomeComplete
  \/ BeginLosingSlotRetirement
  \/ PrepareQueueReleaseBarrier
  \/ PublishReleasedClaim
  \/ AuthorizeExactReleaseFinalization
  \/ PersistReleaseTerminalOutcomePending
  \/ CompleteQueueRelease
  \/ ReserveRecreatedIncarnation
  \/ ReplayStaleReleaseMutation
  \/ ReleaseCommittedMutation
  \/ RestartDropsOwnershipMutation
  \/ RestartWithPendingTerminalOutcome
  \/ TakeInitialQueueOwnershipSnapshot
  \/ BeginTerminalOutcomeStartupSweep
  \/ DeferCanonicalTerminalUnitWithQueueOwner
  \/ FinishTerminalOutcomeStartupSweep
  \/ PlanDeferredCanonicalCarrierFromInitialSnapshot
  \/ OpenQueueAfterTerminalOutcomePlanning
  \/ PendingOnlyCanonicalTerminalMutation
  \/ SweepOnlyAbsentCanonicalGroupMutation
  \/ ReleaseWithoutFinalizationAuthorityMutation
  \/ CompleteWithoutQueueEvidenceMutation
  \/ MutateOwnedRecoveryGroupBeforePlannerMutation
  \/ OpenQueueBeforeDeferredCarrierApplyMutation
  \/ ClassifyLocalProducerRetainedAttempt
  \/ ClassifyObserverKuraRetainedAttempt
  \/ BeginLocalRetainedAttemptRecovery
  \/ CompleteLocalRetainedAttemptRecovery
  \/ RecoverLocalProducerWithoutQueueOwnerMutation
  \/ RestartNeedsCanonicalCarrierBody
  \/ AcceptRecoveredCanonicalCarrierBody
  \/ InstallHistoricalAutonomousRecovery
  \/ PreflightAllHistoricalReservationGroups
  \/ OpenQueueAfterHistoricalInstall
  \/ OpenQueueBeforeHistoricalInstallMutation
  \/ CertifyInstalledHistoricalAutonomousBundle
  \/ PublishDurableStageDiagnostic
  \/ PublishVolatileStageDiagnosticMutation

ReservationCarrierTypeInvariant ==
  /\ ReservationConfiguration
  /\ stage \in ReservationStages
  /\ reservationIdentity \in ReservationIdentities
  /\ carrierIdentity \in ReservationIdentities
  /\ incarnation \in Incarnations
  /\ claimState \in ClaimStates
  /\ queueOwns \in BOOLEAN
  /\ laneOwns \in BOOLEAN
  /\ mergeOwns \in BOOLEAN
  /\ releaseOwns \in BOOLEAN
  /\ committedOwner \in BOOLEAN
  /\ executionCount \in Nat
  /\ controlOnlyAnchor \in BOOLEAN
  /\ candidateBodyDurable \in BOOLEAN
  /\ candidateAuthorized \in BOOLEAN
  /\ slotRetired \in BOOLEAN
  /\ releaseBarrier \in BOOLEAN
  /\ releaseCompletion \in BOOLEAN
  /\ released \in BOOLEAN
  /\ releaseAfterApply \in BOOLEAN
  /\ recreated \in BOOLEAN
  /\ staleRelease \in BOOLEAN
  /\ reservationDurable \in BOOLEAN
  /\ mergeCandidateExact \in BOOLEAN
  /\ canonicalReexecuted \in BOOLEAN
  /\ recoveryStage \in RecoveryStages
  /\ queueGateOpen \in BOOLEAN
  /\ recoverySignerStable \in BOOLEAN
  /\ recoveryWireLengthExact \in BOOLEAN
  /\ localQueueReservationGroupExact \in BOOLEAN
  /\ networkIngressStartupFenced \in BOOLEAN
  /\ queueOwnerQuarantinePending \in BOOLEAN
  /\ durableStageRank \in 0..9
  /\ diagnosticStageRank \in 0..9
  /\ diagnosticsAuthorizeState \in BOOLEAN
  /\ carrierCommitSurface \in CarrierCommitSurfaces
  /\ terminalOutcomeStage \in TerminalOutcomeStages
  /\ terminalOutcomeSource \in TerminalOutcomeSources
  /\ canonicalCarrierCleanupAuthorized \in BOOLEAN
  /\ releaseFinalizationAuthorized \in BOOLEAN
  /\ queueTerminalPhysical \in BOOLEAN
  /\ positiveQueueTerminalEvidence \in BOOLEAN
  /\ terminalStartupGateClosed \in BOOLEAN
  /\ terminalSweepStarted \in BOOLEAN
  /\ terminalSweepCompleted \in BOOLEAN
  /\ queueOwnershipSnapshotTaken \in BOOLEAN
  /\ queueOwnershipSnapshotReceiptValid \in BOOLEAN
  /\ canonicalGroupAQueueOwned \in BOOLEAN
  /\ canonicalGroupBQueueOwned \in BOOLEAN
  /\ snapshotGroupAQueueOwned \in BOOLEAN
  /\ snapshotGroupBQueueOwned \in BOOLEAN
  /\ canonicalGroupATerminalPublished \in BOOLEAN
  /\ canonicalGroupBTerminalPublished \in BOOLEAN
  /\ canonicalCarrierUnitDeferred \in BOOLEAN
  /\ deferredCarrierPlannedFromSnapshot \in BOOLEAN
  /\ normalCarrierApplyCompleted \in BOOLEAN
  /\ canonicalOutcomeSetComplete \in BOOLEAN
  /\ canonicalCarrierBatchPreflighted \in BOOLEAN
  /\ partialCanonicalCleanup \in BOOLEAN

SingleOwnershipInvariant ==
  BoolNat(queueOwns) + BoolNat(laneOwns) + BoolNat(mergeOwns)
    + BoolNat(releaseOwns) + BoolNat(committedOwner) <= 1

ExactCarrierIdentityInvariant ==
  stage \in
    {"Reserved", "Anchored", "Certified", "CandidateDurable",
     "CandidateAuthorized", "PreVoteAuthorized", "CarrierFinalized",
     "Applied", "Forgotten"} =>
    /\ reservationIdentity \in
         {ExactReservationIdentity, RecreatedReservationIdentity}
    /\ carrierIdentity = reservationIdentity

ControlOnlyAnchorInvariant ==
  stage \in
    {"Anchored", "Certified", "CandidateDurable", "CandidateAuthorized",
     "PreVoteAuthorized", "CarrierFinalized", "Applied", "Forgotten"} =>
    controlOnlyAnchor

CandidateAuthorizationInvariant ==
  candidateAuthorized =>
    /\ candidateBodyDurable
    /\ stage \in
         {"CandidateAuthorized", "PreVoteAuthorized", "CarrierFinalized",
          "Applied", "Forgotten"}

ReleaseOrderingInvariant ==
  claimState = "Released" /\ releaseOwns =>
    /\ slotRetired
    /\ releaseBarrier

QueueReleaseCompletionInvariant ==
  released /\ queueOwns /\ ~releaseAfterApply =>
    /\ releaseCompletion
    /\ claimState = "Released"

AtMostOnceApplicationInvariant == executionCount <= 1

NoReleaseAfterApplicationInvariant ==
  executionCount > 0 =>
    /\ ~queueOwns
    /\ ~released
    /\ ~releaseAfterApply

NoStaleIncarnationReleaseInvariant == ~staleRelease

ForgottenOnlyAfterApplicationInvariant ==
  stage = "Forgotten" => executionCount = 1

\* The exact merge flag abstracts the production tuple:
\* (lane_id, dataspace_id, incarnation, lane_height, proposal context),
\* a contiguous source prefix, and the current base WSV commitment.
MLReservationSingleOwner ==
  /\ (stage = "Forgotten" =>
        BoolNat(queueOwns) + BoolNat(laneOwns) + BoolNat(mergeOwns)
          + BoolNat(releaseOwns) + BoolNat(committedOwner) = 0)
  /\ (stage # "Forgotten" =>
        BoolNat(queueOwns) + BoolNat(laneOwns) + BoolNat(mergeOwns)
          + BoolNat(releaseOwns) + BoolNat(committedOwner) = 1)

MLReservationIdentityStable ==
  /\ ExactCarrierIdentityInvariant
  /\ ((laneOwns \/ mergeOwns \/ committedOwner) => reservationDurable)

\* candidateBodyDurable abstracts one exact durable payload plus matching
\* availability, Prepare, and Commit evidence, not a digest-only projection.
MLCertifiedBundleDurable == CandidateAuthorizationInvariant

MLMergeCandidateExactPrefix ==
  candidateAuthorized =>
    /\ mergeCandidateExact
    /\ carrierIdentity = reservationIdentity
    /\ incarnation \in {IncarnationA, IncarnationB}

\* The three values abstract the exact production carrier predicates:
\* Pristine has no pending block hash or staged transaction row;
\* PostBlockPreVote has no pending hash, one exact empty row at the carrier
\* height, the retained exact autonomous event prefix, and one bound complete
\* deterministic carrier event surface. FinalizedCarrier has that row plus the
\* exact singleton carrier hash, an unchanged bound publication surface before
\* drain, an empty live event buffer after drain, and a certified write-set root
\* reconstructed from the retained autonomous-prefix bytes.
MLCarrierCommitSurfaceExact ==
  /\ (stage \in
        {"Queued", "Reserved", "Anchored", "Certified", "CandidateDurable",
         "ReleasePending", "Released"} =>
        carrierCommitSurface = "None")
  /\ (stage = "CandidateAuthorized" =>
        carrierCommitSurface = "Pristine")
  /\ (stage = "PreVoteAuthorized" =>
        carrierCommitSurface = "PostBlockPreVote")
  /\ (stage \in {"CarrierFinalized", "Applied", "Forgotten"} =>
        carrierCommitSurface = "FinalizedCarrier")

MLCarrierExactlyOnce ==
  /\ MLReservationSingleOwner
  /\ ControlOnlyAnchorInvariant
  /\ MLCarrierCommitSurfaceExact
  /\ ReleaseOrderingInvariant
  /\ QueueReleaseCompletionInvariant
  /\ AtMostOnceApplicationInvariant
  /\ NoReleaseAfterApplicationInvariant
  /\ ForgottenOnlyAfterApplicationInvariant
  /\ (executionCount > 0 => canonicalReexecuted)

MLRestartOwnershipPartition ==
  /\ MLReservationSingleOwner
  /\ NoStaleIncarnationReleaseInvariant
  /\ QueueReleaseCompletionInvariant
  /\ (recreated => incarnation = IncarnationB)

\* A recovered canonical body is usable only when one retained Commit-QC
\* signer supplied the whole bounded assembly and the final body revalidated.
MLRecoveredCarrierBodyAuthenticated ==
  /\ recoverySignerStable
  /\ recoveryStage \notin
       {"BodyAcceptedUnauthenticated", "TaskUnauthenticated",
        "UnauthenticatedPreflight"}

\* The complete-wire length is part of the Commit-QC-signed execution
\* commitment. A recovery signer cannot choose a larger assembly length even
\* when it remains below the generic canonical block ceiling.
MLRecoveredCarrierLengthAuthenticated == recoveryWireLengthExact

\* The durable historical task cannot project a stale route, incarnation,
\* predecessor, proposal context, committee, quorum, or validator PoP set.
MLHistoricalRecoveryContextExact ==
  recoveryStage \notin {"TaskDrifted", "ContextDriftPreflight"}

\* Queue visibility follows the startup publication order. It remains closed
\* through body recovery and durable task installation, and may open after the
\* all-group preflight but before quorum certification.
MLHistoricalQueueGateOrder ==
  /\ (recoveryStage = "Normal" /\ ~terminalStartupGateClosed => queueGateOpen)
  /\ (recoveryStage \in
        {"NeedBody", "BodyVerified", "TaskExact"} =>
        ~queueGateOpen)
  /\ (queueGateOpen /\ recoveryStage # "Normal" =>
        recoveryStage \in
          {"GroupsPreflight", "HistoricalCertified",
           "ObserverKuraRetained", "ObserverKuraRecovering"})

\* No prefix of a multi-group reconciliation is publishable. The exact
\* reservation remains the sole durable lane owner until all groups preflight.
MLHistoricalAllGroupsPreflight ==
  /\ recoveryStage # "PartialPreflight"
  /\ (recoveryStage \in
        {"NeedBody", "BodyVerified", "TaskExact", "GroupsPreflight"} =>
        /\ stage = "Anchored"
        /\ laneOwns
        /\ claimState = "Active"
        /\ reservationDurable
        /\ reservationIdentity =
             IF recreated
             THEN RecreatedReservationIdentity
             ELSE ExactReservationIdentity
        /\ incarnation = IF recreated THEN IncarnationB ELSE IncarnationA)

\* Crash/Recover for the local producer is reachable only from a retained
\* attempt carrying its exact current Queue group. The network-ingress startup
\* fence is independent of Queue's observed owner-quarantine bit: an initially
\* empty observer Queue may report quarantine false while ingress stays fenced
\* and exact Kura custody recovers. Producer/nonempty recovery retains both.
MLLocalProducerRecoveryRequiresQueueOwner ==
  /\ (recoveryStage \in
        {"LocalProducerRetained", "LocalProducerRecovering"} =>
        /\ localQueueReservationGroupExact
        /\ networkIngressStartupFenced
        /\ queueOwnerQuarantinePending
        /\ ~queueGateOpen)
  /\ (recoveryStage \in
        {"ObserverKuraRetained", "ObserverKuraRecovering"} =>
        /\ ~localQueueReservationGroupExact
        /\ networkIngressStartupFenced
        /\ ~queueOwnerQuarantinePending)

\* Pending is durable sequencing evidence only. Queue terminal state is
\* either the physical result of exact cleanup or an independently
\* reauthenticated observation of that result. Canonical evidence additionally
\* requires the reconstructed ApplyCarrier capability; release evidence
\* requires the exact retirement/finalization capability. Kura Complete can
\* therefore follow only a positive Queue token for the same source.
MLTerminalOutcomeJoinAuthenticated ==
  /\ (terminalOutcomeStage = "None" =>
        /\ terminalOutcomeSource = "None"
        /\ ~queueTerminalPhysical
        /\ ~positiveQueueTerminalEvidence)
  /\ (terminalOutcomeStage # "None" => terminalOutcomeSource # "None")
  /\ (terminalOutcomeStage = "Pending"
       /\ terminalOutcomeSource = "Canonical"
       /\ ~queueTerminalPhysical =>
        /\ stage = "Applied"
        /\ committedOwner)
  /\ (terminalOutcomeStage = "Pending"
       /\ terminalOutcomeSource = "Release"
       /\ ~queueTerminalPhysical =>
        /\ stage = "Released"
        /\ releaseOwns)
  /\ (queueTerminalPhysical /\ terminalOutcomeSource = "Canonical" =>
        /\ stage = "Forgotten"
        /\ ~queueOwns
        /\ ~laneOwns
        /\ ~mergeOwns
        /\ ~releaseOwns
        /\ ~committedOwner)
  /\ (queueTerminalPhysical /\ terminalOutcomeSource = "Release" =>
        /\ stage = "Queued"
        /\ queueOwns
        /\ ~laneOwns
        /\ ~mergeOwns
        /\ ~releaseOwns
        /\ ~committedOwner
        /\ releaseCompletion)
  /\ (positiveQueueTerminalEvidence => queueTerminalPhysical)
  /\ (positiveQueueTerminalEvidence
       /\ terminalOutcomeSource = "Canonical" =>
        /\ canonicalCarrierCleanupAuthorized
        /\ canonicalGroupATerminalPublished
        /\ canonicalGroupBTerminalPublished)
  /\ (positiveQueueTerminalEvidence
       /\ terminalOutcomeSource = "Release" =>
        releaseFinalizationAuthorized)
  /\ (terminalOutcomeStage = "Complete" =>
        positiveQueueTerminalEvidence)

\* The complete carrier outcome set is reconstructed before any cleanup.
\* Every group receives an independently authenticated ApplyCarrier
\* capability, then Queue runs one all-group preflight. Publishing just the
\* absent B member of the mixed A-owned/B-absent unit is therefore forbidden.
MLCanonicalTerminalBatchAtomic ==
  /\ ~partialCanonicalCleanup
  /\ (canonicalGroupATerminalPublished =
        canonicalGroupBTerminalPublished)
  /\ (canonicalCarrierCleanupAuthorized
       /\ terminalOutcomeSource = "Canonical" =>
        canonicalOutcomeSetComplete)
  /\ (canonicalCarrierBatchPreflighted =>
        /\ canonicalOutcomeSetComplete
        /\ canonicalCarrierCleanupAuthorized
        /\ terminalOutcomeSource = "Canonical")
  /\ (queueTerminalPhysical
       /\ terminalOutcomeSource = "Canonical" =>
        /\ canonicalOutcomeSetComplete
        /\ canonicalCarrierBatchPreflighted
        /\ canonicalGroupATerminalPublished
        /\ canonicalGroupBTerminalPublished)
  /\ (normalCarrierApplyCompleted =>
        /\ canonicalCarrierUnitDeferred
        /\ deferredCarrierPlannedFromSnapshot
        /\ canonicalGroupATerminalPublished
        /\ canonicalGroupBTerminalPublished)

\* Startup first owns one immutable Queue reconciliation receipt. Pre-sweep
\* may complete an all-empty unit, but the mixed bounded unit (A owned, B
\* absent) is deferred as one unit without changing either member. The normal
\* carrier planner and apply path consume that same receipt; only after both
\* members publish and Kura has no Pending record may Queue publication open.
MLTerminalStartupSweepOrder ==
  /\ (terminalStartupGateClosed => ~queueGateOpen)
  /\ (queueOwnershipSnapshotTaken =>
        queueOwnershipSnapshotReceiptValid)
  /\ (terminalSweepStarted =>
        /\ terminalStartupGateClosed
        /\ queueOwnershipSnapshotTaken
        /\ queueOwnershipSnapshotReceiptValid)
  /\ (terminalSweepCompleted =>
        /\ terminalSweepStarted
        /\ queueOwnershipSnapshotTaken
        /\ queueOwnershipSnapshotReceiptValid
        /\ \/ /\ terminalOutcomeStage = "Complete"
                 /\ positiveQueueTerminalEvidence
            \/ /\ terminalOutcomeStage = "Pending"
                 /\ terminalOutcomeSource = "Canonical"
                 /\ canonicalCarrierUnitDeferred)
  /\ (canonicalCarrierUnitDeferred =>
        /\ terminalOutcomeSource = "Canonical"
        /\ queueOwnershipSnapshotTaken
        /\ queueOwnershipSnapshotReceiptValid
        /\ snapshotGroupAQueueOwned
        /\ ~snapshotGroupBQueueOwned)
  /\ (queueOwnershipSnapshotTaken
       /\ terminalOutcomeStage = "Pending"
       /\ terminalOutcomeSource = "Canonical"
       /\ ~queueTerminalPhysical
       /\ ~normalCarrierApplyCompleted =>
        /\ canonicalGroupAQueueOwned = snapshotGroupAQueueOwned
        /\ canonicalGroupBQueueOwned = snapshotGroupBQueueOwned
        /\ ~canonicalGroupATerminalPublished
        /\ ~canonicalGroupBTerminalPublished)
  /\ (deferredCarrierPlannedFromSnapshot =>
        /\ canonicalCarrierUnitDeferred
        /\ (terminalSweepCompleted \/ queueGateOpen)
        /\ queueOwnershipSnapshotTaken
        /\ queueOwnershipSnapshotReceiptValid)
  /\ (normalCarrierApplyCompleted =>
        /\ deferredCarrierPlannedFromSnapshot
        /\ queueOwnershipSnapshotReceiptValid
        /\ ~canonicalGroupAQueueOwned
        /\ ~canonicalGroupBQueueOwned
        /\ canonicalGroupATerminalPublished
        /\ canonicalGroupBTerminalPublished)
  /\ (queueOwnershipSnapshotTaken
       /\ terminalOutcomeStage = "Pending" =>
        ~queueGateOpen)
  /\ (queueGateOpen /\ canonicalCarrierUnitDeferred =>
        /\ terminalOutcomeStage # "Pending"
        /\ normalCarrierApplyCompleted)

\* Ranks 1..9 abstract the ordered durable diagnostics chain from exact
\* reservations through Queue finalization. Rank 0 means that no row is
\* publishable. Every published row keeps the exact durable reservation owner,
\* provisional proposal-slot identity, FIFO group digest, and route/incarnation;
\* a finalized proposal/descriptor identity appears only once payload evidence
\* makes it exact. A decrease is permitted only when the modeled identity
\* changes to the recreated incarnation, whose independent chain restarts at
\* rank 1.
MLStageEvidenceMonotonic ==
  /\ diagnosticStageRank <= durableStageRank
  /\ (diagnosticStageRank = 0 \/ diagnosticIdentityExact)
  /\ ~diagnosticsAuthorizeState

AutonomousReservationCarrierSafetyInvariant ==
  /\ ReservationCarrierTypeInvariant
  /\ SingleOwnershipInvariant
  /\ ExactCarrierIdentityInvariant
  /\ ControlOnlyAnchorInvariant
  /\ CandidateAuthorizationInvariant
  /\ ReleaseOrderingInvariant
  /\ QueueReleaseCompletionInvariant
  /\ AtMostOnceApplicationInvariant
  /\ NoReleaseAfterApplicationInvariant
  /\ NoStaleIncarnationReleaseInvariant
  /\ ForgottenOnlyAfterApplicationInvariant
  /\ MLReservationSingleOwner
  /\ MLReservationIdentityStable
  /\ MLCertifiedBundleDurable
  /\ MLMergeCandidateExactPrefix
  /\ MLCarrierCommitSurfaceExact
  /\ MLCarrierExactlyOnce
  /\ MLRestartOwnershipPartition
  /\ MLRecoveredCarrierBodyAuthenticated
  /\ MLRecoveredCarrierLengthAuthenticated
  /\ MLHistoricalRecoveryContextExact
  /\ MLHistoricalQueueGateOrder
  /\ MLHistoricalAllGroupsPreflight
  /\ MLLocalProducerRecoveryRequiresQueueOwner
  /\ MLTerminalOutcomeJoinAuthenticated
  /\ MLCanonicalTerminalBatchAtomic
  /\ MLTerminalStartupSweepOrder
  /\ MLStageEvidenceMonotonic

ReservationCarrierSpec == Init /\ [][Next]_vars

AutonomousReservationCarrierProductionRefinementObligation ==
  ReservationCarrierSpec => []AutonomousReservationCarrierSafetyInvariant

====
