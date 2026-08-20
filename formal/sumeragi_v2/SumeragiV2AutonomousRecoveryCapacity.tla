---- MODULE SumeragiV2AutonomousRecoveryCapacity ----
EXTENDS Naturals

(***************************************************************************
Compact current-behavior kernel for autonomous recovery and capacity cuts.

The model deliberately separates six obligations that cross crash or first-
mutation boundaries:

  1. advancing the route snapshot to N+1 cannot discard the exact bounded
     recovery source for an incomplete pointerless carrier at N; only that
     identity or exact terminal/receipt evidence can discharge the source;
  2. READY-bearing autonomous successor admission accepts only the exact WSV
     frontier or a carrier-revalidated MergeExecution receipt; hash-only lane
     ownership is not economic application evidence;
  3. startup repair cannot consume capacity before carrier envelopes have
     been reconstructed;
  4. certification creates durable pair and bundle capacity obligations, so
     a crash may discard live envelopes only while startup remains closed and
     both envelopes remain reconstructable;
  5. the complete entrypoint-claim-set, canonical-association-stage, and
     canonical-prune peaks are admitted before their respective first durable
     mutations; and
  6. a debug append follows durable carrier reservation and remains accounted
     after restart.

Mutation modes each remove exactly one ordering or durability edge. Source
bindings and unresolved editor-owned gates live in the adjacent versioned
static contract. This finite kernel is not, by itself, a production trace
proof.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Mode

RecoveryCapacityModes ==
  {"Fixed",
   "RouteLatestOnlySkip",
   "StartupRepairBeforeEnvelope",
   "FrontierMissingBundleEnvelope",
   "ClaimPeakAfterMutation",
   "AssociationPeakAfterMutation",
   "PrunePeakAfterMutation",
   "PrunePeakDropsReservationEnvelope",
   "DebugAppendBeforeCarrierReservation",
   "DebugRestartDropsAccounting",
   "HashOnlyAutonomousPredecessor"}

CarrierNStatuses == {"Incomplete", "Recovered", "Terminal", "Receipted"}
CarrierNSources ==
  {"None", "IncompleteIdentityN", "RecoveredIdentityN",
   "TerminalProofN", "ReceiptProofN"}
AutonomousPredecessorEvidence ==
  {"None", "HashOnlyOwnership", "ExactWsvFrontier",
   "MergeReceiptCarrierRevalidated"}
StartupPhases == {"Cold", "EnvelopesReconstructed", "Repairing", "Published"}
DebugPhases == {"Idle", "CarrierReserved", "Appended", "RestartPending",
                "RestartAccounted"}

VARIABLES
  \* @type: Int;
  routeSnapshotHeight,
  \* @type: Int;
  routeLatestHeight,
  \* @type: Str;
  carrierNStatus,
  \* @type: Str;
  carrierNSource,
  \* @type: Str;
  autonomousPredecessorEvidence,
  \* @type: Bool;
  autonomousPredecessorAdmitted,
  \* @type: Str;
  startupPhase,
  \* @type: Bool;
  carrierEnvelopesReconstructed,
  \* @type: Bool;
  startupCapacityMutation,
  \* @type: Bool;
  certifiedFrontier,
  \* @type: Bool;
  frontierPairCapacityObligation,
  \* @type: Bool;
  frontierBundleCapacityObligation,
  \* @type: Bool;
  frontierPairEnvelope,
  \* @type: Bool;
  frontierBundleEnvelope,
  \* @type: Bool;
  frontierStartupClosed,
  \* @type: Bool;
  claimSetPeakAdmitted,
  \* @type: Bool;
  claimSetFirstMutation,
  \* @type: Bool;
  associationStagePeakAdmitted,
  \* @type: Bool;
  associationStageFirstMutation,
  \* @type: Bool;
  pruneCapacityPeakAdmitted,
  \* @type: Bool;
  pruneReservationEnvelopeCovered,
  \* @type: Bool;
  pruneFirstDurableMutation,
  \* @type: Str;
  debugPhase,
  \* @type: Bool;
  debugCarrierReservationDurable,
  \* @type: Bool;
  debugAppendDurable,
  \* @type: Bool;
  debugRuntimeAccounted,
  \* @type: Bool;
  debugRestartAccounted

CarrierVars ==
  <<routeSnapshotHeight, routeLatestHeight, carrierNStatus, carrierNSource>>

PredecessorVars ==
  <<autonomousPredecessorEvidence, autonomousPredecessorAdmitted>>

StartupVars ==
  <<startupPhase, carrierEnvelopesReconstructed, startupCapacityMutation>>

FrontierVars ==
  <<certifiedFrontier, frontierPairCapacityObligation,
    frontierBundleCapacityObligation, frontierPairEnvelope,
    frontierBundleEnvelope, frontierStartupClosed>>

PeakVars ==
  <<claimSetPeakAdmitted, claimSetFirstMutation,
    associationStagePeakAdmitted, associationStageFirstMutation,
    pruneCapacityPeakAdmitted, pruneReservationEnvelopeCovered,
    pruneFirstDurableMutation>>

DebugVars ==
  <<debugPhase, debugCarrierReservationDurable, debugAppendDurable,
    debugRuntimeAccounted, debugRestartAccounted>>

vars ==
  <<CarrierVars, PredecessorVars, StartupVars, FrontierVars, PeakVars,
    DebugVars>>

Init ==
  /\ Mode \in RecoveryCapacityModes
  /\ routeSnapshotHeight = 1
  /\ routeLatestHeight = 1
  /\ carrierNStatus = "Incomplete"
  /\ carrierNSource = "IncompleteIdentityN"
  /\ autonomousPredecessorEvidence = "None"
  /\ autonomousPredecessorAdmitted = FALSE
  /\ startupPhase = "Cold"
  /\ carrierEnvelopesReconstructed = FALSE
  /\ startupCapacityMutation = FALSE
  /\ certifiedFrontier = FALSE
  /\ frontierPairCapacityObligation = FALSE
  /\ frontierBundleCapacityObligation = FALSE
  /\ frontierPairEnvelope = FALSE
  /\ frontierBundleEnvelope = FALSE
  /\ frontierStartupClosed = FALSE
  /\ claimSetPeakAdmitted = FALSE
  /\ claimSetFirstMutation = FALSE
  /\ associationStagePeakAdmitted = FALSE
  /\ associationStageFirstMutation = FALSE
  /\ pruneCapacityPeakAdmitted = FALSE
  /\ pruneReservationEnvelopeCovered = FALSE
  /\ pruneFirstDurableMutation = FALSE
  /\ debugPhase = "Idle"
  /\ debugCarrierReservationDurable = FALSE
  /\ debugAppendDurable = FALSE
  /\ debugRuntimeAccounted = FALSE
  /\ debugRestartAccounted = FALSE

AdvanceRouteSnapshotToNPlusOne ==
  /\ routeSnapshotHeight = 1
  /\ carrierNStatus = "Incomplete"
  /\ routeSnapshotHeight' = 2
  /\ routeLatestHeight' = 2
  /\ carrierNStatus' = carrierNStatus
  /\ carrierNSource' =
       IF Mode = "RouteLatestOnlySkip"
       THEN "None"
       ELSE carrierNSource
  /\ UNCHANGED <<PredecessorVars, StartupVars, FrontierVars, PeakVars,
                  DebugVars>>

RecoverCarrierNFromExactIncompleteIdentity ==
  /\ carrierNStatus = "Incomplete"
  /\ carrierNSource = "IncompleteIdentityN"
  /\ carrierNStatus' = "Recovered"
  /\ carrierNSource' = "RecoveredIdentityN"
  /\ UNCHANGED <<routeSnapshotHeight, routeLatestHeight,
                  PredecessorVars, StartupVars, FrontierVars, PeakVars,
                  DebugVars>>

DischargeCarrierNWithTerminalProof ==
  /\ carrierNStatus = "Incomplete"
  /\ carrierNSource = "IncompleteIdentityN"
  /\ carrierNStatus' = "Terminal"
  /\ carrierNSource' = "TerminalProofN"
  /\ UNCHANGED <<routeSnapshotHeight, routeLatestHeight,
                  PredecessorVars, StartupVars, FrontierVars, PeakVars,
                  DebugVars>>

DischargeCarrierNWithReceiptProof ==
  /\ carrierNStatus = "Incomplete"
  /\ carrierNSource = "IncompleteIdentityN"
  /\ carrierNStatus' = "Receipted"
  /\ carrierNSource' = "ReceiptProofN"
  /\ UNCHANGED <<routeSnapshotHeight, routeLatestHeight,
                  PredecessorVars, StartupVars, FrontierVars, PeakVars,
                  DebugVars>>

ObserveHashOnlyAutonomousPredecessor ==
  /\ autonomousPredecessorEvidence = "None"
  /\ autonomousPredecessorEvidence' = "HashOnlyOwnership"
  /\ autonomousPredecessorAdmitted' =
       (Mode = "HashOnlyAutonomousPredecessor")
  /\ UNCHANGED <<CarrierVars, StartupVars, FrontierVars, PeakVars, DebugVars>>

AdmitAutonomousPredecessorFromExactWsvFrontier ==
  /\ autonomousPredecessorEvidence = "None"
  /\ autonomousPredecessorEvidence' = "ExactWsvFrontier"
  /\ autonomousPredecessorAdmitted' = TRUE
  /\ UNCHANGED <<CarrierVars, StartupVars, FrontierVars, PeakVars, DebugVars>>

AdmitAutonomousPredecessorFromRevalidatedMergeReceipt ==
  /\ autonomousPredecessorEvidence = "None"
  /\ autonomousPredecessorEvidence' = "MergeReceiptCarrierRevalidated"
  /\ autonomousPredecessorAdmitted' = TRUE
  /\ UNCHANGED <<CarrierVars, StartupVars, FrontierVars, PeakVars, DebugVars>>

ReconstructCarrierEnvelopesOnStartup ==
  /\ startupPhase = "Cold"
  /\ startupPhase' = "EnvelopesReconstructed"
  /\ carrierEnvelopesReconstructed' = TRUE
  /\ startupCapacityMutation' = startupCapacityMutation
  /\ UNCHANGED <<CarrierVars, PredecessorVars, FrontierVars, PeakVars,
                  DebugVars>>

BeginStartupCapacityRepair ==
  /\ startupPhase \in {"Cold", "EnvelopesReconstructed"}
  /\ (carrierEnvelopesReconstructed \/
      Mode = "StartupRepairBeforeEnvelope")
  /\ startupPhase' = "Repairing"
  /\ carrierEnvelopesReconstructed' = carrierEnvelopesReconstructed
  /\ startupCapacityMutation' = TRUE
  /\ UNCHANGED <<CarrierVars, PredecessorVars, FrontierVars, PeakVars,
                  DebugVars>>

PublishStartupRepair ==
  /\ startupPhase = "Repairing"
  /\ startupPhase' = "Published"
  /\ UNCHANGED <<carrierEnvelopesReconstructed, startupCapacityMutation,
                  CarrierVars, PredecessorVars, FrontierVars, PeakVars,
                  DebugVars>>

CertifyFrontier ==
  /\ ~certifiedFrontier
  /\ certifiedFrontier' = TRUE
  /\ frontierPairCapacityObligation' = TRUE
  /\ frontierBundleCapacityObligation' =
       (Mode # "FrontierMissingBundleEnvelope")
  /\ frontierPairEnvelope' = TRUE
  /\ frontierBundleEnvelope' =
       (Mode # "FrontierMissingBundleEnvelope")
  /\ frontierStartupClosed' = FALSE
  /\ UNCHANGED <<CarrierVars, PredecessorVars, StartupVars, PeakVars,
                  DebugVars>>

CrashAfterCertifiedFrontier ==
  /\ certifiedFrontier
  /\ ~frontierStartupClosed
  /\ frontierPairCapacityObligation
  /\ frontierBundleCapacityObligation
  /\ frontierPairEnvelope' = FALSE
  /\ frontierBundleEnvelope' = FALSE
  /\ frontierStartupClosed' = TRUE
  /\ UNCHANGED <<certifiedFrontier, frontierPairCapacityObligation,
                  frontierBundleCapacityObligation,
                  CarrierVars, PredecessorVars, StartupVars, PeakVars,
                  DebugVars>>

ReconstructCertifiedFrontierEnvelopes ==
  /\ certifiedFrontier
  /\ frontierStartupClosed
  /\ frontierPairCapacityObligation
  /\ frontierBundleCapacityObligation
  /\ frontierPairEnvelope' = TRUE
  /\ frontierBundleEnvelope' = TRUE
  /\ UNCHANGED <<certifiedFrontier, frontierPairCapacityObligation,
                  frontierBundleCapacityObligation, frontierStartupClosed,
                  CarrierVars, PredecessorVars, StartupVars, PeakVars,
                  DebugVars>>

OpenAfterCertifiedFrontierReconstruction ==
  /\ certifiedFrontier
  /\ frontierStartupClosed
  /\ frontierPairEnvelope
  /\ frontierBundleEnvelope
  /\ frontierStartupClosed' = FALSE
  /\ UNCHANGED <<certifiedFrontier, frontierPairCapacityObligation,
                  frontierBundleCapacityObligation, frontierPairEnvelope,
                  frontierBundleEnvelope,
                  CarrierVars, PredecessorVars, StartupVars, PeakVars,
                  DebugVars>>

AdmitEntrypointClaimSetPeak ==
  /\ ~claimSetPeakAdmitted
  /\ ~claimSetFirstMutation
  /\ claimSetPeakAdmitted' = TRUE
  /\ UNCHANGED <<claimSetFirstMutation, associationStagePeakAdmitted,
                  associationStageFirstMutation, pruneCapacityPeakAdmitted,
                  pruneReservationEnvelopeCovered,
                  pruneFirstDurableMutation,
                  CarrierVars, PredecessorVars, StartupVars, FrontierVars,
                  DebugVars>>

BeginEntrypointClaimSetMutation ==
  /\ ~claimSetFirstMutation
  /\ (claimSetPeakAdmitted \/ Mode = "ClaimPeakAfterMutation")
  /\ claimSetFirstMutation' = TRUE
  /\ UNCHANGED <<claimSetPeakAdmitted, associationStagePeakAdmitted,
                  associationStageFirstMutation, pruneCapacityPeakAdmitted,
                  pruneReservationEnvelopeCovered,
                  pruneFirstDurableMutation,
                  CarrierVars, PredecessorVars, StartupVars, FrontierVars,
                  DebugVars>>

AdmitCanonicalAssociationStagePeak ==
  /\ ~associationStagePeakAdmitted
  /\ ~associationStageFirstMutation
  /\ associationStagePeakAdmitted' = TRUE
  /\ UNCHANGED <<associationStageFirstMutation, claimSetPeakAdmitted,
                  claimSetFirstMutation, pruneCapacityPeakAdmitted,
                  pruneReservationEnvelopeCovered,
                  pruneFirstDurableMutation,
                  CarrierVars, PredecessorVars, StartupVars, FrontierVars,
                  DebugVars>>

BeginCanonicalAssociationStageMutation ==
  /\ ~associationStageFirstMutation
  /\ (associationStagePeakAdmitted \/
      Mode = "AssociationPeakAfterMutation")
  /\ associationStageFirstMutation' = TRUE
  /\ UNCHANGED <<associationStagePeakAdmitted, claimSetPeakAdmitted,
                  claimSetFirstMutation, pruneCapacityPeakAdmitted,
                  pruneReservationEnvelopeCovered,
                  pruneFirstDurableMutation,
                  CarrierVars, PredecessorVars, StartupVars, FrontierVars,
                  DebugVars>>

AdmitPruneCapacityPeak ==
  /\ ~pruneCapacityPeakAdmitted
  /\ ~pruneFirstDurableMutation
  /\ pruneCapacityPeakAdmitted' = TRUE
  /\ pruneReservationEnvelopeCovered' =
       (Mode # "PrunePeakDropsReservationEnvelope")
  /\ UNCHANGED <<pruneFirstDurableMutation, claimSetPeakAdmitted,
                  claimSetFirstMutation, associationStagePeakAdmitted,
                  associationStageFirstMutation,
                  CarrierVars, PredecessorVars, StartupVars, FrontierVars,
                  DebugVars>>

BeginPruneDurableMutation ==
  /\ ~pruneFirstDurableMutation
  /\ (pruneCapacityPeakAdmitted \/ Mode = "PrunePeakAfterMutation")
  /\ pruneFirstDurableMutation' = TRUE
  /\ UNCHANGED <<pruneCapacityPeakAdmitted,
                  pruneReservationEnvelopeCovered, claimSetPeakAdmitted,
                  claimSetFirstMutation, associationStagePeakAdmitted,
                  associationStageFirstMutation,
                  CarrierVars, PredecessorVars, StartupVars, FrontierVars,
                  DebugVars>>

ReserveDebugCarrierCapacity ==
  /\ debugPhase = "Idle"
  /\ debugPhase' = "CarrierReserved"
  /\ debugCarrierReservationDurable' = TRUE
  /\ UNCHANGED <<debugAppendDurable, debugRuntimeAccounted,
                  debugRestartAccounted,
                  CarrierVars, PredecessorVars, StartupVars, FrontierVars,
                  PeakVars>>

AppendDebugAfterCarrierReservation ==
  /\ debugPhase \in {"Idle", "CarrierReserved"}
  /\ (debugCarrierReservationDurable \/
      Mode = "DebugAppendBeforeCarrierReservation")
  /\ debugPhase' = "Appended"
  /\ debugAppendDurable' = TRUE
  /\ debugRuntimeAccounted' = TRUE
  /\ UNCHANGED <<debugCarrierReservationDurable, debugRestartAccounted,
                  CarrierVars, PredecessorVars, StartupVars, FrontierVars,
                  PeakVars>>

CrashAfterDebugAppend ==
  /\ debugPhase = "Appended"
  /\ debugAppendDurable
  /\ debugPhase' = "RestartPending"
  /\ debugRuntimeAccounted' = FALSE
  /\ debugRestartAccounted' = FALSE
  /\ UNCHANGED <<debugCarrierReservationDurable, debugAppendDurable,
                  CarrierVars, PredecessorVars, StartupVars, FrontierVars,
                  PeakVars>>

AccountDebugAppendOnRestart ==
  /\ debugPhase = "RestartPending"
  /\ debugAppendDurable
  /\ debugPhase' = "RestartAccounted"
  /\ debugRuntimeAccounted' = FALSE
  /\ debugRestartAccounted' =
       (Mode # "DebugRestartDropsAccounting")
  /\ UNCHANGED <<debugCarrierReservationDurable, debugAppendDurable,
                  CarrierVars, PredecessorVars, StartupVars, FrontierVars,
                  PeakVars>>

Next ==
  \/ AdvanceRouteSnapshotToNPlusOne
  \/ RecoverCarrierNFromExactIncompleteIdentity
  \/ DischargeCarrierNWithTerminalProof
  \/ DischargeCarrierNWithReceiptProof
  \/ ObserveHashOnlyAutonomousPredecessor
  \/ AdmitAutonomousPredecessorFromExactWsvFrontier
  \/ AdmitAutonomousPredecessorFromRevalidatedMergeReceipt
  \/ ReconstructCarrierEnvelopesOnStartup
  \/ BeginStartupCapacityRepair
  \/ PublishStartupRepair
  \/ CertifyFrontier
  \/ CrashAfterCertifiedFrontier
  \/ ReconstructCertifiedFrontierEnvelopes
  \/ OpenAfterCertifiedFrontierReconstruction
  \/ AdmitEntrypointClaimSetPeak
  \/ BeginEntrypointClaimSetMutation
  \/ AdmitCanonicalAssociationStagePeak
  \/ BeginCanonicalAssociationStageMutation
  \/ AdmitPruneCapacityPeak
  \/ BeginPruneDurableMutation
  \/ ReserveDebugCarrierCapacity
  \/ AppendDebugAfterCarrierReservation
  \/ CrashAfterDebugAppend
  \/ AccountDebugAppendOnRestart

AutonomousRecoveryCapacitySpec == Init /\ [][Next]_vars

AutonomousRecoveryCapacityTypeInvariant ==
  /\ Mode \in RecoveryCapacityModes
  /\ routeSnapshotHeight \in 1..2
  /\ routeLatestHeight \in 1..2
  /\ carrierNStatus \in CarrierNStatuses
  /\ carrierNSource \in CarrierNSources
  /\ autonomousPredecessorEvidence \in AutonomousPredecessorEvidence
  /\ autonomousPredecessorAdmitted \in BOOLEAN
  /\ startupPhase \in StartupPhases
  /\ carrierEnvelopesReconstructed \in BOOLEAN
  /\ startupCapacityMutation \in BOOLEAN
  /\ certifiedFrontier \in BOOLEAN
  /\ frontierPairCapacityObligation \in BOOLEAN
  /\ frontierBundleCapacityObligation \in BOOLEAN
  /\ frontierPairEnvelope \in BOOLEAN
  /\ frontierBundleEnvelope \in BOOLEAN
  /\ frontierStartupClosed \in BOOLEAN
  /\ claimSetPeakAdmitted \in BOOLEAN
  /\ claimSetFirstMutation \in BOOLEAN
  /\ associationStagePeakAdmitted \in BOOLEAN
  /\ associationStageFirstMutation \in BOOLEAN
  /\ pruneCapacityPeakAdmitted \in BOOLEAN
  /\ pruneReservationEnvelopeCovered \in BOOLEAN
  /\ pruneFirstDurableMutation \in BOOLEAN
  /\ debugPhase \in DebugPhases
  /\ debugCarrierReservationDurable \in BOOLEAN
  /\ debugAppendDurable \in BOOLEAN
  /\ debugRuntimeAccounted \in BOOLEAN
  /\ debugRestartAccounted \in BOOLEAN

MLIncompleteCarrierNRecoverable ==
  /\ (carrierNStatus = "Incomplete") =>
       (carrierNSource = "IncompleteIdentityN")
  /\ (carrierNStatus = "Recovered") =>
       (carrierNSource = "RecoveredIdentityN")
  /\ (carrierNStatus = "Terminal") =>
       (carrierNSource = "TerminalProofN")
  /\ (carrierNStatus = "Receipted") =>
       (carrierNSource = "ReceiptProofN")

MLStartupRepairAfterCarrierEnvelopes ==
  startupCapacityMutation => carrierEnvelopesReconstructed

MLAutonomousPredecessorGloballyApplied ==
  /\ autonomousPredecessorAdmitted =>
       autonomousPredecessorEvidence
         \in {"ExactWsvFrontier", "MergeReceiptCarrierRevalidated"}
  /\ (autonomousPredecessorEvidence = "HashOnlyOwnership") =>
       ~autonomousPredecessorAdmitted

MLCertifiedFrontierCapacityReconstructable ==
  /\ certifiedFrontier =>
       (frontierPairCapacityObligation /\
        frontierBundleCapacityObligation)
  /\ (certifiedFrontier /\ ~frontierStartupClosed) =>
       (frontierPairEnvelope /\ frontierBundleEnvelope)
  /\ (certifiedFrontier /\ frontierStartupClosed) =>
       (frontierPairCapacityObligation /\
        frontierBundleCapacityObligation)

MLMutationPeaksAdmittedBeforeFirstWrite ==
  /\ claimSetFirstMutation => claimSetPeakAdmitted
  /\ associationStageFirstMutation => associationStagePeakAdmitted
  /\ pruneFirstDurableMutation =>
       (pruneCapacityPeakAdmitted /\
        pruneReservationEnvelopeCovered)

MLDebugAppendReservationAndRestartAccounting ==
  /\ debugAppendDurable => debugCarrierReservationDurable
  /\ (debugPhase = "Appended") => debugRuntimeAccounted
  /\ (debugPhase = "RestartAccounted" /\ debugAppendDurable) =>
       debugRestartAccounted

AutonomousRecoveryCapacitySafetyInvariant ==
  /\ AutonomousRecoveryCapacityTypeInvariant
  /\ MLIncompleteCarrierNRecoverable
  /\ MLAutonomousPredecessorGloballyApplied
  /\ MLStartupRepairAfterCarrierEnvelopes
  /\ MLCertifiedFrontierCapacityReconstructable
  /\ MLMutationPeaksAdmittedBeforeFirstWrite
  /\ MLDebugAppendReservationAndRestartAccounting

AutonomousRecoveryCapacityProductionRefinementObligation ==
  AutonomousRecoveryCapacitySafetyInvariant

=============================================================================
