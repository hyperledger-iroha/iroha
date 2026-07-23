---- MODULE SumeragiV2AutoscaleLifecycle ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
Bounded executable model for one automatically managed lane across creation,
activation, drain, archival, destruction, and same-ID recreation.

The production refinement is source-bound separately to
`StateBlock::maybe_apply_nexus_autoscale`,
`State::preflight_committed_autoscale_lane_geometry`,
`State::apply_committed_autoscale_lane_lifecycle`, and
`State::validate_committed_autoscale_drain_metadata_update` in
`crates/iroha_core/src/state.rs`.  This finite model is counterexample-search
evidence only; it does not prove that those Rust transitions refine it.
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
   "ReuseRetiredIncarnation"}

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
  unsafeDestroy

vars ==
  <<phase, incarnation, storagePrepared, drainBlocked, drainEvidence,
    archiveDurable, retiredIncarnations, everCreated, retirementCount,
    unsafeDestroy>>

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
  /\ storagePrepared' = TRUE
  /\ drainBlocked' = FALSE
  /\ drainEvidence' = FALSE
  /\ archiveDurable' = FALSE
  /\ UNCHANGED <<retiredIncarnations, everCreated, retirementCount,
                 unsafeDestroy>>

ActivateLane ==
  /\ phase = "Initializing"
  /\ storagePrepared
  /\ phase' = "Active"
  /\ drainBlocked' = TRUE
  /\ UNCHANGED <<incarnation, storagePrepared, drainEvidence,
                 archiveDurable, retiredIncarnations, everCreated,
                 retirementCount, unsafeDestroy>>

BeginDrain ==
  /\ phase = "Active"
  /\ phase' = "Draining"
  /\ UNCHANGED <<incarnation, storagePrepared, drainBlocked,
                 drainEvidence, archiveDurable, retiredIncarnations,
                 everCreated, retirementCount, unsafeDestroy>>

ClearDrainBlockers ==
  /\ phase = "Draining"
  /\ drainBlocked
  /\ drainBlocked' = FALSE
  /\ UNCHANGED <<phase, incarnation, storagePrepared, drainEvidence,
                 archiveDurable, retiredIncarnations, everCreated,
                 retirementCount, unsafeDestroy>>

CertifyDrain ==
  /\ phase = "Draining"
  /\ (Mode = "EarlyDrainCertificate" \/ ~drainBlocked)
  /\ phase' = "Certified"
  /\ drainEvidence' = TRUE
  /\ UNCHANGED <<incarnation, storagePrepared, drainBlocked,
                 archiveDurable, retiredIncarnations, everCreated,
                 retirementCount, unsafeDestroy>>

ArchiveLane ==
  /\ phase = "Certified"
  /\ drainEvidence
  /\ ~drainBlocked
  /\ phase' = "Archived"
  /\ archiveDurable' = TRUE
  /\ UNCHANGED <<incarnation, storagePrepared, drainBlocked,
                 drainEvidence, retiredIncarnations, everCreated,
                 retirementCount, unsafeDestroy>>

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
  /\ incarnation' = 0
  /\ storagePrepared' = FALSE
  /\ drainBlocked' = FALSE
  /\ drainEvidence' = FALSE
  /\ archiveDurable' = FALSE

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

AutoscaleLifecycleSafetyInvariant ==
  /\ LifecycleTypeInvariant
  /\ StorageBeforeActivationInvariant
  /\ DrainEvidenceInvariant
  /\ ArchiveBeforeDestroyInvariant
  /\ NoIncarnationReuseInvariant

LifecycleSpec == Init /\ [][Next]_vars

(***************************************************************************
This is deliberately ledgered as specified_unproved.  TLC checks the finite
kernel and its mutations; a future cross-tool proof must establish the trace
mapping from the source-bound Rust entry points.
***************************************************************************)
AutoscaleLifecycleProductionRefinementObligation ==
  LifecycleSpec => []AutoscaleLifecycleSafetyInvariant

====
