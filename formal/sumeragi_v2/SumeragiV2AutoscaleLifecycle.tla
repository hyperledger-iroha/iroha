---- MODULE SumeragiV2AutoscaleLifecycle ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
Bounded executable model for one automatically managed lane across creation,
activation, drain, archival, destruction, and same-ID recreation.

The production refinement is source-bound separately to
`StateBlock::apply_staged_nexus_autoscale`,
`State::preflight_committed_autoscale_lane_geometry`,
`State::apply_committed_autoscale_lane_geometry`,
`State::apply_committed_autoscale_lane_lifecycle`, and
`State::validate_committed_autoscale_drain_metadata_update` in
`crates/iroha_core/src/state.rs`, plus quorum validation in
`crates/iroha_core/src/lane_consensus.rs`. This finite model is
counterexample-search evidence only; it does not prove that those Rust
transitions refine it.
***************************************************************************)

CONSTANTS
  \* @type: Str;
  Mode,
  \* @type: Int;
  InitialIncarnation,
  \* @type: Int;
  RecreatedIncarnation

LifecycleModes ==
  {"Fixed", "EarlyDrainCertificate", "DestroyBeforeArchive",
   "ReuseRetiredIncarnation", "ActivateBeforeStorage",
   "WeakDrainCertificate", "CleanupByLaneId"}

LifecyclePhases ==
  {"Absent", "Initializing", "Active", "Draining", "Certified", "Archived"}

LifecycleConfiguration ==
  /\ Mode \in LifecycleModes
  /\ InitialIncarnation \in Nat \ {0}
  /\ RecreatedIncarnation \in Nat \ {0}
  /\ InitialIncarnation # RecreatedIncarnation

VARIABLES
  \* @type: Str;
  phase,
  \* @type: Int;
  incarnation,
  \* @type: Bool;
  storagePrepared,
  \* @type: Bool;
  drainBlocked,
  \* @type: Bool;
  drainEvidence,
  \* @type: Bool;
  archiveDurable,
  \* @type: Set(Int);
  retiredIncarnations,
  \* @type: Bool;
  everCreated,
  \* @type: Int;
  retirementCount,
  \* @type: Bool;
  unsafeDestroy,
  \* @type: Bool;
  routeVisible,
  \* @type: Bool;
  drainCertificateQuorum,
  \* @type: Bool;
  drainCertificateAtSignedCommitFloor,
  \* @type: Bool;
  drainCloseFenced,
  \* @type: Bool;
  retirementCarrierLater,
  \* @type: Int;
  retiredExactIncarnation,
  \* @type: Bool;
  staleIncarnationAccepted

vars ==
  <<phase, incarnation, storagePrepared, drainBlocked, drainEvidence,
    archiveDurable, retiredIncarnations, everCreated, retirementCount,
    unsafeDestroy, routeVisible, drainCertificateQuorum,
    drainCertificateAtSignedCommitFloor, drainCloseFenced,
    retirementCarrierLater, retiredExactIncarnation,
    staleIncarnationAccepted>>

Init ==
  /\ LifecycleConfiguration
  /\ phase = "Absent"
  /\ incarnation = 0
  /\ storagePrepared = FALSE
  /\ drainBlocked = FALSE
  /\ drainEvidence = FALSE
  /\ archiveDurable = FALSE
  /\ retiredIncarnations = {}
  /\ everCreated = FALSE
  /\ retirementCount = 0
  /\ unsafeDestroy = FALSE
  /\ routeVisible = FALSE
  /\ drainCertificateQuorum = FALSE
  /\ drainCertificateAtSignedCommitFloor = FALSE
  /\ drainCloseFenced = FALSE
  /\ retirementCarrierLater = FALSE
  /\ retiredExactIncarnation = 0
  /\ staleIncarnationAccepted = FALSE

InitializeLane ==
  /\ phase = "Absent"
  /\ retirementCount \in 0..1
  /\ phase' = "Initializing"
  /\ incarnation' =
       IF ~everCreated
       THEN InitialIncarnation
       ELSE IF Mode = "ReuseRetiredIncarnation"
            THEN InitialIncarnation
            ELSE RecreatedIncarnation
  /\ storagePrepared' = (Mode # "ActivateBeforeStorage")
  /\ drainBlocked' = FALSE
  /\ drainEvidence' = FALSE
  /\ archiveDurable' = FALSE
  /\ routeVisible' = FALSE
  /\ drainCertificateQuorum' = FALSE
  /\ drainCertificateAtSignedCommitFloor' = FALSE
  /\ drainCloseFenced' = FALSE
  /\ retirementCarrierLater' = FALSE
  /\ UNCHANGED <<retiredIncarnations, everCreated, retirementCount,
                 unsafeDestroy, retiredExactIncarnation,
                 staleIncarnationAccepted>>

ActivateLane ==
  /\ phase = "Initializing"
  /\ (storagePrepared \/ Mode = "ActivateBeforeStorage")
  /\ phase' = "Active"
  /\ drainBlocked' = TRUE
  /\ routeVisible' = TRUE
  /\ UNCHANGED <<incarnation, storagePrepared, drainEvidence,
                 archiveDurable, retiredIncarnations, everCreated,
                 retirementCount, unsafeDestroy, drainCertificateQuorum,
                 drainCertificateAtSignedCommitFloor, drainCloseFenced,
                 retirementCarrierLater, retiredExactIncarnation,
                 staleIncarnationAccepted>>

BeginDrain ==
  /\ phase = "Active"
  /\ phase' = "Draining"
  /\ UNCHANGED <<incarnation, storagePrepared, drainBlocked,
                 drainEvidence, archiveDurable, retiredIncarnations,
                 everCreated, retirementCount, unsafeDestroy, routeVisible,
                 drainCertificateQuorum,
                 drainCertificateAtSignedCommitFloor, drainCloseFenced,
                 retirementCarrierLater, retiredExactIncarnation,
                 staleIncarnationAccepted>>

ClearDrainBlockers ==
  /\ phase = "Draining"
  /\ drainBlocked
  /\ drainBlocked' = FALSE
  /\ UNCHANGED <<phase, incarnation, storagePrepared, drainEvidence,
                 archiveDurable, retiredIncarnations, everCreated,
                 retirementCount, unsafeDestroy, routeVisible,
                 drainCertificateQuorum,
                 drainCertificateAtSignedCommitFloor, drainCloseFenced,
                 retirementCarrierLater, retiredExactIncarnation,
                 staleIncarnationAccepted>>

CertifyDrain ==
  /\ phase = "Draining"
  /\ (Mode = "EarlyDrainCertificate" \/ ~drainBlocked)
  /\ phase' = "Certified"
  /\ drainEvidence' = TRUE
  /\ drainCertificateQuorum' = (Mode # "WeakDrainCertificate")
  /\ drainCertificateAtSignedCommitFloor' = TRUE
  /\ drainCloseFenced' = TRUE
  /\ UNCHANGED <<incarnation, storagePrepared, drainBlocked,
                 archiveDurable, retiredIncarnations, everCreated,
                 retirementCount, unsafeDestroy, routeVisible,
                 retirementCarrierLater, retiredExactIncarnation,
                 staleIncarnationAccepted>>

ArchiveLane ==
  /\ phase = "Certified"
  /\ drainEvidence
  /\ ~drainBlocked
  /\ phase' = "Archived"
  /\ archiveDurable' = TRUE
  /\ retirementCarrierLater' = TRUE
  /\ UNCHANGED <<incarnation, storagePrepared, drainBlocked,
                 drainEvidence, retiredIncarnations, everCreated,
                 retirementCount, unsafeDestroy, routeVisible,
                 drainCertificateQuorum,
                 drainCertificateAtSignedCommitFloor, drainCloseFenced,
                 retiredExactIncarnation, staleIncarnationAccepted>>

DestroyLane ==
  /\ retirementCount = 0
  /\ phase \in
       (IF Mode = "DestroyBeforeArchive"
        THEN {"Certified", "Archived"}
        ELSE {"Archived"})
  /\ phase' = "Absent"
  /\ retiredIncarnations' = retiredIncarnations \cup {incarnation}
  /\ everCreated' = TRUE
  /\ retirementCount' = 1
  /\ unsafeDestroy' = (unsafeDestroy \/ ~archiveDurable)
  /\ retiredExactIncarnation' =
       IF Mode = "CleanupByLaneId"
       THEN IF incarnation = InitialIncarnation
            THEN RecreatedIncarnation
            ELSE InitialIncarnation
       ELSE incarnation
  /\ staleIncarnationAccepted' = (Mode = "CleanupByLaneId")
  /\ incarnation' = 0
  /\ storagePrepared' = FALSE
  /\ drainBlocked' = FALSE
  /\ drainEvidence' = FALSE
  /\ archiveDurable' = FALSE
  /\ routeVisible' = FALSE
  /\ UNCHANGED <<drainCertificateQuorum,
                 drainCertificateAtSignedCommitFloor, drainCloseFenced,
                 retirementCarrierLater>>

Next ==
  \/ InitializeLane
  \/ ActivateLane
  \/ BeginDrain
  \/ ClearDrainBlockers
  \/ CertifyDrain
  \/ ArchiveLane
  \/ DestroyLane

LifecycleTypeInvariant ==
  /\ LifecycleConfiguration
  /\ phase \in LifecyclePhases
  /\ incarnation \in {0, InitialIncarnation, RecreatedIncarnation}
  /\ storagePrepared \in BOOLEAN
  /\ drainBlocked \in BOOLEAN
  /\ drainEvidence \in BOOLEAN
  /\ archiveDurable \in BOOLEAN
  /\ retiredIncarnations \subseteq
       {InitialIncarnation, RecreatedIncarnation}
  /\ IsFiniteSet(retiredIncarnations)
  /\ everCreated \in BOOLEAN
  /\ retirementCount \in 0..1
  /\ unsafeDestroy \in BOOLEAN
  /\ routeVisible \in BOOLEAN
  /\ drainCertificateQuorum \in BOOLEAN
  /\ drainCertificateAtSignedCommitFloor \in BOOLEAN
  /\ drainCloseFenced \in BOOLEAN
  /\ retirementCarrierLater \in BOOLEAN
  /\ retiredExactIncarnation \in
       {0, InitialIncarnation, RecreatedIncarnation}
  /\ staleIncarnationAccepted \in BOOLEAN

StorageBeforeActivationInvariant ==
  phase \in {"Active", "Draining", "Certified", "Archived"} =>
    storagePrepared

DrainEvidenceInvariant ==
  phase \in {"Certified", "Archived"} =>
    /\ ~drainBlocked
    /\ drainEvidence

ArchiveBeforeDestroyInvariant == ~unsafeDestroy

NoIncarnationReuseInvariant ==
  phase \in {"Initializing", "Active", "Draining", "Certified", "Archived"} =>
    incarnation \notin retiredIncarnations

MLActivationAfterAtomicCreate ==
  routeVisible =>
    /\ phase \in {"Active", "Draining", "Certified", "Archived"}
    /\ storagePrepared
    /\ incarnation # 0

MLDrainImpliesNoOwnedWork == DrainEvidenceInvariant

MLDrainCertificateMonotonic ==
  /\ (phase \in {"Certified", "Archived"} =>
       /\ drainCertificateQuorum
       /\ drainCertificateAtSignedCommitFloor
       /\ drainCloseFenced)
  /\ (phase = "Archived" => retirementCarrierLater)

MLRetirementConsumesExactIncarnation ==
  /\ ArchiveBeforeDestroyInvariant
  /\ NoIncarnationReuseInvariant
  /\ (retirementCount = 1 =>
       /\ retiredExactIncarnation \in retiredIncarnations
       /\ ~staleIncarnationAccepted)

AutoscaleLifecycleSafetyInvariant ==
  /\ LifecycleTypeInvariant
  /\ StorageBeforeActivationInvariant
  /\ DrainEvidenceInvariant
  /\ ArchiveBeforeDestroyInvariant
  /\ NoIncarnationReuseInvariant
  /\ MLActivationAfterAtomicCreate
  /\ MLDrainImpliesNoOwnedWork
  /\ MLDrainCertificateMonotonic
  /\ MLRetirementConsumesExactIncarnation

LifecycleSpec == Init /\ [][Next]_vars

(***************************************************************************
This support operator is not an independent proof-ledger row.  TLC checks the
finite kernel and its mutations; release still requires source-bound evidence
for every production trace mapping that consumes this operator.
***************************************************************************)
AutoscaleLifecycleProductionRefinementObligation ==
  LifecycleSpec => []AutoscaleLifecycleSafetyInvariant

====
