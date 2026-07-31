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

The production refinement is source-bound separately to the queue reservation
and release-barrier APIs, Kura autonomous slot claims, full merge-candidate
signing authorization, `StateBlock::stage_certified_merge_entry`, and
`State::validate_merge_execution_batch`, including its route/incarnation-first
canonical order key, plus startup reservation reconciliation through bounded
Kura indexes. The observer-only diagnostic rank is source-bound to
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
   "SkipCanonicalReexecution", "RestartDropsOwnership",
   "VolatileStageDiagnostics", "UnauthenticatedRecoveryBody",
   "MixedSignerRecoveryBody", "InflatedRecoveryWireLength",
   "HistoricalContextDrift",
   "PartialRecoveryGroupPreflight", "OpenQueueBeforeRecoveryInstall"}

ReservationStages ==
  {"Queued", "Reserved", "Anchored", "Certified", "CandidateDurable",
   "CandidateAuthorized", "ReleasePending", "Released", "Applied",
   "Forgotten"}

ClaimStates == {"None", "Active", "ReleasePending", "Released", "Committed"}

RecoveryStages ==
  {"Normal", "NeedBody", "BodyVerified", "BodyAcceptedUnauthenticated",
   "TaskExact", "TaskUnauthenticated", "TaskDrifted",
   "UnauthenticatedPreflight", "ContextDriftPreflight",
   "PartialPreflight", "GroupsPreflight", "HistoricalCertified"}

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
  \* @type: Int;
  durableStageRank,
  \* @type: Int;
  diagnosticStageRank,
  \* @type: Bool;
  diagnosticsAuthorizeState

carrierVars ==
  <<stage, reservationIdentity, carrierIdentity, incarnation, claimState,
    queueOwns, laneOwns, mergeOwns, releaseOwns, committedOwner,
    executionCount, controlOnlyAnchor, candidateBodyDurable,
    candidateAuthorized, slotRetired, releaseBarrier, releaseCompletion,
    released, releaseAfterApply, recreated, staleRelease,
    reservationDurable, mergeCandidateExact, canonicalReexecuted>>

diagnosticVars ==
  <<durableStageRank, diagnosticStageRank, diagnosticsAuthorizeState>>

recoveryVars ==
  <<recoveryStage, queueGateOpen, recoverySignerStable,
    recoveryWireLengthExact>>

vars ==
  <<stage, reservationIdentity, carrierIdentity, incarnation, claimState,
    queueOwns, laneOwns, mergeOwns, releaseOwns, committedOwner,
    executionCount, controlOnlyAnchor, candidateBodyDurable,
    candidateAuthorized, slotRetired, releaseBarrier, releaseCompletion,
    released, releaseAfterApply, recreated, staleRelease,
    reservationDurable, mergeCandidateExact, canonicalReexecuted,
    recoveryStage, queueGateOpen, recoverySignerStable,
    recoveryWireLengthExact,
    durableStageRank, diagnosticStageRank, diagnosticsAuthorizeState>>

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
  /\ durableStageRank = 0
  /\ diagnosticStageRank = 0
  /\ diagnosticsAuthorizeState = FALSE

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
  /\ UNCHANGED <<executionCount, recreated>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars

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
                 canonicalReexecuted>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars

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
                 canonicalReexecuted>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars

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
                 canonicalReexecuted>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars

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
  /\ durableStageRank' =
       IF candidateBodyDurable THEN 6 ELSE durableStageRank
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, releaseOwns, committedOwner,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 slotRetired, releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, canonicalReexecuted>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars

ApplyCanonicalCarrier ==
  /\ stage = "CandidateAuthorized"
  /\ mergeOwns
  /\ candidateAuthorized
  /\ carrierIdentity = reservationIdentity
  /\ executionCount' = executionCount + 1
  /\ claimState' = "Committed"
  /\ canonicalReexecuted' = (Mode # "SkipCanonicalReexecution")
  /\ durableStageRank' = 8
  /\ IF Mode = "DuplicateApplication"
     THEN /\ stage' = "CandidateAuthorized"
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
                 reservationDurable, mergeCandidateExact>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars

ForgetCommittedReservation ==
  /\ stage = "Applied"
  /\ committedOwner
  /\ executionCount = 1
  /\ stage' = "Forgotten"
  /\ claimState' = "None"
  /\ committedOwner' = FALSE
  /\ durableStageRank' = 9
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 queueOwns, laneOwns, mergeOwns, releaseOwns,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, released, releaseAfterApply, recreated,
                 staleRelease, reservationDurable, mergeCandidateExact,
                 canonicalReexecuted>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticsAuthorizeState>>
  /\ UNCHANGED recoveryVars

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
                 canonicalReexecuted>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars

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
                 canonicalReexecuted>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars

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
                 canonicalReexecuted>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars

CompleteQueueRelease ==
  /\ stage = "Released"
  /\ releaseOwns
  /\ claimState = "Released"
  /\ stage' = "Queued"
  /\ queueOwns' = TRUE
  /\ releaseOwns' = FALSE
  /\ releaseCompletion' = TRUE
  /\ released' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, laneOwns, mergeOwns, committedOwner,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseAfterApply, recreated, staleRelease,
                 reservationDurable, mergeCandidateExact,
                 canonicalReexecuted>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars

ReserveRecreatedIncarnation ==
  /\ stage = "Queued"
  /\ recoveryStage = "Normal"
  /\ queueGateOpen
  /\ queueOwns
  /\ released
  /\ releaseCompletion
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
  /\ diagnosticsAuthorizeState' = FALSE
  /\ UNCHANGED <<executionCount, releaseBarrier, releaseCompletion>>
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
                 canonicalReexecuted>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars

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
                 canonicalReexecuted>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars

RestartDropsOwnershipMutation ==
  /\ Mode = "RestartDropsOwnership"
  /\ stage \in
       {"Reserved", "Anchored", "Certified", "CandidateDurable",
        "CandidateAuthorized", "Applied"}
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
                 canonicalReexecuted>>
  /\ UNCHANGED diagnosticVars
  /\ UNCHANGED recoveryVars

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
  /\ UNCHANGED <<queueGateOpen, recoverySignerStable>>
  /\ UNCHANGED <<carrierVars, diagnosticVars>>

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
  /\ UNCHANGED <<queueGateOpen, recoverySignerStable>>
  /\ UNCHANGED <<carrierVars, diagnosticVars>>

OpenQueueAfterHistoricalInstall ==
  /\ stage = "Anchored"
  /\ laneOwns
  /\ recoveryStage \in
       {"GroupsPreflight", "UnauthenticatedPreflight",
        "ContextDriftPreflight", "PartialPreflight"}
  /\ ~queueGateOpen
  /\ queueGateOpen' = TRUE
  /\ UNCHANGED <<recoveryStage, recoverySignerStable>>
  /\ UNCHANGED <<carrierVars, diagnosticVars>>

\* ML-MUT-AUT-06: ordinary selection becomes visible before authenticated
\* recovery, durable historical-task installation, and all-group preflight.
OpenQueueBeforeHistoricalInstallMutation ==
  /\ Mode = "OpenQueueBeforeRecoveryInstall"
  /\ stage = "Anchored"
  /\ laneOwns
  /\ recoveryStage \in {"NeedBody", "BodyVerified", "TaskExact"}
  /\ ~queueGateOpen
  /\ queueGateOpen' = TRUE
  /\ UNCHANGED <<recoveryStage, recoverySignerStable>>
  /\ UNCHANGED <<carrierVars, diagnosticVars>>

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
                 canonicalReexecuted>>
  /\ UNCHANGED <<queueGateOpen, recoverySignerStable>>
  /\ UNCHANGED <<diagnosticStageRank, diagnosticsAuthorizeState>>

\* Diagnostics may catch up to the highest revalidated durable stage for the
\* exact route/incarnation/proposal identity. They are observers: publishing a
\* row cannot change any ownership, authorization, or application state.
PublishDurableStageDiagnostic ==
  /\ diagnosticStageRank < durableStageRank
  /\ diagnosticStageRank' = durableStageRank
  /\ diagnosticsAuthorizeState' = FALSE
  /\ UNCHANGED <<carrierVars, durableStageRank>>
  /\ UNCHANGED recoveryVars

\* ML-MUT-LIFE-05: report one stage beyond durable evidence and let that
\* volatile projection become an authorization input.
PublishVolatileStageDiagnosticMutation ==
  /\ Mode = "VolatileStageDiagnostics"
  /\ durableStageRank < 9
  /\ diagnosticStageRank <= durableStageRank
  /\ diagnosticStageRank' = durableStageRank + 1
  /\ diagnosticsAuthorizeState' = TRUE
  /\ UNCHANGED <<carrierVars, durableStageRank>>
  /\ UNCHANGED recoveryVars

Next ==
  \/ ReserveFifoTransaction
  \/ AnchorAutonomousControl
  \/ CertifyAutonomousBundle
  \/ PersistFullMergeCandidate
  \/ AuthorizeExactMergeCandidate
  \/ ApplyCanonicalCarrier
  \/ ForgetCommittedReservation
  \/ BeginLosingSlotRetirement
  \/ PrepareQueueReleaseBarrier
  \/ PublishReleasedClaim
  \/ CompleteQueueRelease
  \/ ReserveRecreatedIncarnation
  \/ ReplayStaleReleaseMutation
  \/ ReleaseCommittedMutation
  \/ RestartDropsOwnershipMutation
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
  /\ durableStageRank \in 0..9
  /\ diagnosticStageRank \in 0..9
  /\ diagnosticsAuthorizeState \in BOOLEAN

SingleOwnershipInvariant ==
  BoolNat(queueOwns) + BoolNat(laneOwns) + BoolNat(mergeOwns)
    + BoolNat(releaseOwns) + BoolNat(committedOwner) <= 1

ExactCarrierIdentityInvariant ==
  stage \in
    {"Reserved", "Anchored", "Certified", "CandidateDurable",
     "CandidateAuthorized", "Applied", "Forgotten"} =>
    /\ reservationIdentity \in
         {ExactReservationIdentity, RecreatedReservationIdentity}
    /\ carrierIdentity = reservationIdentity

ControlOnlyAnchorInvariant ==
  stage \in
    {"Anchored", "Certified", "CandidateDurable", "CandidateAuthorized",
     "Applied", "Forgotten"} =>
    controlOnlyAnchor

CandidateAuthorizationInvariant ==
  candidateAuthorized =>
    /\ candidateBodyDurable
    /\ stage \in {"CandidateAuthorized", "Applied", "Forgotten"}

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

MLCarrierExactlyOnce ==
  /\ MLReservationSingleOwner
  /\ ControlOnlyAnchorInvariant
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
  /\ (recoveryStage = "Normal" => queueGateOpen)
  /\ (recoveryStage \in
        {"NeedBody", "BodyVerified", "TaskExact"} =>
        ~queueGateOpen)
  /\ (queueGateOpen /\ recoveryStage # "Normal" =>
        recoveryStage \in {"GroupsPreflight", "HistoricalCertified"})

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
        /\ reservationIdentity = ExactReservationIdentity
        /\ incarnation = IncarnationA)

\* Ranks 1..9 abstract the ordered durable diagnostics chain from exact
\* reservations through Queue finalization. Rank 0 means that no row is
\* publishable. A decrease is permitted only when the modeled identity changes
\* to the recreated incarnation, whose independent chain restarts at rank 1.
MLStageEvidenceMonotonic ==
  /\ diagnosticStageRank <= durableStageRank
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
  /\ MLCarrierExactlyOnce
  /\ MLRestartOwnershipPartition
  /\ MLRecoveredCarrierBodyAuthenticated
  /\ MLRecoveredCarrierLengthAuthenticated
  /\ MLHistoricalRecoveryContextExact
  /\ MLHistoricalQueueGateOrder
  /\ MLHistoricalAllGroupsPreflight
  /\ MLStageEvidenceMonotonic

ReservationCarrierSpec == Init /\ [][Next]_vars

AutonomousReservationCarrierProductionRefinementObligation ==
  ReservationCarrierSpec => []AutonomousReservationCarrierSafetyInvariant

====
