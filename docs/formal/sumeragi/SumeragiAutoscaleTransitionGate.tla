---- MODULE SumeragiAutoscaleTransitionGate ----
EXTENDS FiniteSets, Integers

(***************************************************************************
A bounded model for the autoscale commit gate and elastic-lane lifecycle.

The original slice checked the exact `autoscale_transition_committed_at(...)`
call-site contract. That direct truth-table remains below. The state machine
also models the safety boundary around an automatically created or retired
lane: optimistic catalog binding, a recoverable physical-geometry journal,
fresh incarnations, activation heights, drain-only retirement, restart
reconciliation, and fail-closed stale-artifact admission.

`Bug` is used by the expected-failure configurations. `"none"` is the
production specification.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

Lanes == {0, 1, 2}
BaselineLanes == {0}
ElasticLanes == {1, 2}
MinLanes == 1
MaxLanes == 3
MaxHeight == 4

VARIABLES
  \* Original direct helper gate sentinel.
  \* @type: Int;
  checked,
  \* Last committed global height.
  \* @type: Int;
  height,
  \* Authoritative consensus catalog and last consensus-published catalog.
  \* @type: Set(Int);
  catalog,
  \* @type: Set(Int);
  committedCatalog,
  \* Physically provisioned Kura/WSV lane geometry.
  \* @type: Set(Int);
  physical,
  \* Per-lane monotonic generation and active incarnation (0 = inactive).
  \* @type: Int -> Int;
  generation,
  \* @type: Int -> Int;
  retiredGeneration,
  \* @type: Int -> Int;
  incarnation,
  \* First global proposal height allowed for the active incarnation.
  \* @type: Int -> Int;
  activation,
  \* @type: Int -> Int;
  transitionHeight,
  \* 0 = none, l + 1 = create l, -(l + 1) = retire l.
  \* @type: Int;
  pending,
  \* @type: Set(Int);
  pendingBase,
  \* 0 = clean, 1 = physical prepare, 2 = catalog published.
  \* @type: Int;
  journalPhase,
  \* @type: Bool;
  pressure,
  \* @type: Bool;
  idle,
  \* @type: Set(Int);
  drained,
  \* @type: Bool;
  staleArtifactAccepted,
  \* @type: Bool;
  unsafeRetirement,
  \* @type: Bool;
  nonConsensusCatalogWrite,
  \* @type: Bool;
  baselineDigestIntact

vars ==
  <<checked, height, catalog, committedCatalog, physical, generation,
    retiredGeneration, incarnation, activation, transitionHeight, pending,
    pendingBase, journalPhase, pressure, idle, drained, staleArtifactAccepted, unsafeRetirement,
    nonConsensusCatalogWrite, baselineDigestIntact>>

(***************************************************************************
Direct transition-helper truth table retained for source-level mutation gates.
***************************************************************************)

Cases == {
  "enabled_matching_success",
  "enabled_matching_failure",
  "disabled_matching_success",
  "enabled_previous_success",
  "enabled_next_success",
  "disabled_previous_failure"
}

Enabled(c) ==
  c \in {
    "enabled_matching_success",
    "enabled_matching_failure",
    "enabled_previous_success",
    "enabled_next_success"
  }

CommitSuccess(c) ==
  c \in {
    "enabled_matching_success",
    "disabled_matching_success",
    "enabled_previous_success",
    "enabled_next_success"
  }

CommittedHeight(c) == 10

LastTransitionHeight(c) ==
  CASE c \in {
         "enabled_matching_success",
         "enabled_matching_failure",
         "disabled_matching_success"
       } -> 10
    [] c \in {"enabled_previous_success", "disabled_previous_failure"} -> 9
    [] c = "enabled_next_success" -> 11
    [] OTHER -> 0

SpecHelperResult(c) ==
  Enabled(c) /\ LastTransitionHeight(c) = CommittedHeight(c)

ActualHelperResult(c) ==
  CASE Bug = "skip_matching_transition"
       /\ Enabled(c)
       /\ LastTransitionHeight(c) = CommittedHeight(c) -> FALSE
    [] Bug = "ignore_enabled"
       /\ ~Enabled(c)
       /\ LastTransitionHeight(c) = CommittedHeight(c) -> TRUE
    [] Bug = "ignore_height"
       /\ Enabled(c) -> TRUE
    [] Bug = "off_by_one_previous"
       /\ Enabled(c)
       /\ LastTransitionHeight(c) + 1 = CommittedHeight(c) -> TRUE
    [] Bug = "off_by_one_next"
       /\ Enabled(c)
       /\ LastTransitionHeight(c) = CommittedHeight(c) + 1 -> TRUE
    [] OTHER -> SpecHelperResult(c)

SpecQueueReconfigured(c) ==
  CommitSuccess(c) /\ SpecHelperResult(c)

ActualQueueReconfigured(c) ==
  CASE Bug = "skip_success_reconfigure"
       /\ CommitSuccess(c)
       /\ ActualHelperResult(c) -> FALSE
    [] Bug = "reconfigure_failed_commit"
       /\ ~CommitSuccess(c)
       /\ ActualHelperResult(c) -> TRUE
    [] Bug = "reconfigure_without_transition"
       /\ CommitSuccess(c)
       /\ ~ActualHelperResult(c) -> TRUE
    [] OTHER -> CommitSuccess(c) /\ ActualHelperResult(c)

SpecReportedHeight(c) ==
  IF SpecQueueReconfigured(c) THEN CommittedHeight(c) ELSE -1

ActualReportedHeight(c) ==
  IF ActualQueueReconfigured(c) THEN
    IF Bug = "wrong_reported_height" THEN CommittedHeight(c) + 1 ELSE CommittedHeight(c)
  ELSE -1

SpecCase(c) ==
  <<SpecHelperResult(c), SpecQueueReconfigured(c), SpecReportedHeight(c)>>

ActualCase(c) ==
  <<ActualHelperResult(c), ActualQueueReconfigured(c), ActualReportedHeight(c)>>

(***************************************************************************
Elastic lifecycle state machine.
***************************************************************************)

Init ==
  /\ checked = 0
  /\ height = 0
  /\ catalog = BaselineLanes
  /\ committedCatalog = BaselineLanes
  /\ physical = BaselineLanes
  /\ generation = [l \in Lanes |-> IF l \in BaselineLanes THEN 1 ELSE 0]
  /\ retiredGeneration = [l \in Lanes |-> 0]
  /\ incarnation = [l \in Lanes |-> IF l \in BaselineLanes THEN 1 ELSE 0]
  /\ activation = [l \in Lanes |-> IF l \in BaselineLanes THEN 0 ELSE -1]
  /\ transitionHeight = [l \in Lanes |-> IF l \in BaselineLanes THEN 0 ELSE -1]
  /\ pending = 0
  /\ pendingBase = BaselineLanes
  /\ journalPhase = 0
  /\ pressure = FALSE
  /\ idle = FALSE
  /\ drained = BaselineLanes
  /\ staleArtifactAccepted = FALSE
  /\ unsafeRetirement = FALSE
  /\ nonConsensusCatalogWrite = FALSE
  /\ baselineDigestIntact = TRUE

ObservePressure ==
  /\ ~pressure
  /\ pressure' = TRUE
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase,
                 journalPhase, idle, drained, staleArtifactAccepted,
                 unsafeRetirement, nonConsensusCatalogWrite,
                 baselineDigestIntact>>

ObserveIdle ==
  /\ ~idle
  /\ idle' = TRUE
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase,
                 journalPhase, pressure, drained, staleArtifactAccepted,
                 unsafeRetirement, nonConsensusCatalogWrite,
                 baselineDigestIntact>>

DrainLane(l) ==
  /\ l \in catalog
  /\ l \notin drained
  /\ drained' = drained \cup {l}
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase,
                 journalPhase, pressure, idle, staleArtifactAccepted,
                 unsafeRetirement, nonConsensusCatalogWrite,
                 baselineDigestIntact>>

StageCreate(l) ==
  /\ pending = 0
  /\ journalPhase = 0
  /\ pressure
  /\ l \in ElasticLanes \ catalog
  /\ Cardinality(catalog) < MaxLanes
  /\ pending' = l + 1
  /\ pendingBase' = catalog
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, journalPhase, pressure,
                 idle, drained, staleArtifactAccepted, unsafeRetirement,
                 nonConsensusCatalogWrite, baselineDigestIntact>>

StageRetire(l) ==
  /\ pending = 0
  /\ journalPhase = 0
  /\ idle
  /\ l \in (catalog \cap ElasticLanes)
  /\ Cardinality(catalog) > MinLanes
  /\ (l \in drained \/ Bug = "retire_undrained")
  /\ pending' = -(l + 1)
  /\ pendingBase' = catalog
  /\ unsafeRetirement' = (unsafeRetirement \/ l \notin drained)
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, journalPhase, pressure,
                 idle, drained, staleArtifactAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact>>

PrepareCreate(l) ==
  /\ pending = l + 1
  /\ journalPhase = 0
  /\ physical = catalog
  /\ physical' = physical \cup {l}
  /\ journalPhase' = 1
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, generation,
                 retiredGeneration, incarnation, activation, transitionHeight,
                 pending, pendingBase, pressure, idle,
                 drained, staleArtifactAccepted, unsafeRetirement,
                 nonConsensusCatalogWrite, baselineDigestIntact>>

PrepareRetire(l) ==
  /\ pending = -(l + 1)
  /\ journalPhase = 0
  /\ physical = catalog
  /\ physical' = physical \ {l}
  /\ journalPhase' = 1
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, generation,
                 retiredGeneration, incarnation, activation, transitionHeight,
                 pending, pendingBase, pressure, idle,
                 drained, staleArtifactAccepted, unsafeRetirement,
                 nonConsensusCatalogWrite, baselineDigestIntact>>

CommitCreate(l) ==
  /\ pending = l + 1
  /\ journalPhase = 1
  /\ height < MaxHeight
  /\ (pendingBase = catalog \/ Bug = "stale_catalog_commit")
  /\ catalog' = catalog \cup {l}
  /\ committedCatalog' = catalog'
  /\ height' = height + 1
  /\ generation' =
       [generation EXCEPT ![l] =
          IF Bug = "reuse_incarnation" /\ retiredGeneration[l] > 0
          THEN @
          ELSE @ + 1]
  /\ incarnation' = [incarnation EXCEPT ![l] = generation'[l]]
  /\ activation' =
       [activation EXCEPT ![l] = IF Bug = "activate_early" THEN height ELSE height']
  /\ transitionHeight' = [transitionHeight EXCEPT ![l] = height']
  /\ drained' = drained \ {l}
  /\ pending' = 0
  /\ pendingBase' = catalog'
  /\ journalPhase' = 2
  /\ pressure' = FALSE
  /\ UNCHANGED <<checked, physical, retiredGeneration, idle, staleArtifactAccepted,
                 unsafeRetirement, nonConsensusCatalogWrite,
                 baselineDigestIntact>>

CommitRetire(l) ==
  /\ pending = -(l + 1)
  /\ journalPhase = 1
  /\ height < MaxHeight
  /\ (pendingBase = catalog \/ Bug = "stale_catalog_commit")
  /\ catalog' = catalog \ {l}
  /\ committedCatalog' = catalog'
  /\ height' = height + 1
  /\ retiredGeneration' = [retiredGeneration EXCEPT ![l] = generation[l]]
  /\ incarnation' = [incarnation EXCEPT ![l] = 0]
  /\ activation' = [activation EXCEPT ![l] = -1]
  /\ transitionHeight' = [transitionHeight EXCEPT ![l] = height']
  /\ pending' = 0
  /\ pendingBase' = catalog'
  /\ journalPhase' = 2
  /\ idle' = FALSE
  /\ UNCHANGED <<checked, physical, generation, pressure, drained,
                 staleArtifactAccepted, unsafeRetirement,
                 nonConsensusCatalogWrite, baselineDigestIntact>>

FinalizeJournal ==
  /\ journalPhase = 2
  /\ physical = catalog
  /\ journalPhase' = 0
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase,
                 pressure, idle, drained, staleArtifactAccepted,
                 unsafeRetirement, nonConsensusCatalogWrite,
                 baselineDigestIntact>>

CrashRecover ==
  /\ journalPhase \in {1, 2}
  /\ physical' = catalog
  /\ pending' = 0
  /\ pendingBase' = catalog
  /\ journalPhase' = 0
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, generation,
                 retiredGeneration, incarnation, activation, transitionHeight,
                 pressure, idle, drained,
                 staleArtifactAccepted, unsafeRetirement,
                 nonConsensusCatalogWrite, baselineDigestIntact>>

AttemptArtifact(l, claimedIncarnation, proposalHeight) ==
  /\ l \in Lanes
  /\ claimedIncarnation \in 0..(MaxHeight + 1)
  /\ proposalHeight \in 0..(MaxHeight + 1)
  /\ LET valid ==
           /\ l \in catalog
           /\ incarnation[l] = claimedIncarnation
           /\ claimedIncarnation > 0
           /\ proposalHeight >= activation[l]
     IN staleArtifactAccepted' =
          (staleArtifactAccepted \/ (~valid /\ Bug = "accept_stale_artifact"))
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase,
                 journalPhase, pressure, idle, drained, unsafeRetirement,
                 nonConsensusCatalogWrite, baselineDigestIntact>>

InjectedRestartDrift ==
  /\ Bug = "restart_geometry_drift"
  /\ journalPhase = 0
  /\ physical' = physical \cup (ElasticLanes \ catalog)
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, generation,
                 retiredGeneration, incarnation, activation, transitionHeight,
                 pending, pendingBase, journalPhase,
                 pressure, idle, drained, staleArtifactAccepted,
                 unsafeRetirement, nonConsensusCatalogWrite,
                 baselineDigestIntact>>

InjectedNonConsensusCatalogWrite(l) ==
  /\ Bug = "non_consensus_catalog_write"
  /\ l \in ElasticLanes \ catalog
  /\ catalog' = catalog \cup {l}
  /\ nonConsensusCatalogWrite' = TRUE
  /\ UNCHANGED <<checked, height, committedCatalog, physical, generation,
                 retiredGeneration, incarnation, activation, transitionHeight,
                 pending, pendingBase, journalPhase,
                 pressure, idle, drained, staleArtifactAccepted,
                 unsafeRetirement, baselineDigestIntact>>

InjectedBaselineMutation ==
  /\ Bug = "mutate_baseline"
  /\ baselineDigestIntact' = FALSE
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase,
                 journalPhase, pressure, idle, drained, staleArtifactAccepted,
                 unsafeRetirement, nonConsensusCatalogWrite>>

Next ==
  \/ ObservePressure
  \/ ObserveIdle
  \/ \E l \in Lanes: DrainLane(l)
  \/ \E l \in ElasticLanes: StageCreate(l)
  \/ \E l \in ElasticLanes: StageRetire(l)
  \/ \E l \in ElasticLanes: PrepareCreate(l)
  \/ \E l \in ElasticLanes: PrepareRetire(l)
  \/ \E l \in ElasticLanes: CommitCreate(l)
  \/ \E l \in ElasticLanes: CommitRetire(l)
  \/ FinalizeJournal
  \/ CrashRecover
  \/ \E l \in Lanes, i \in 0..(MaxHeight + 1), h \in 0..(MaxHeight + 1):
       AttemptArtifact(l, i, h)
  \/ InjectedRestartDrift
  \/ \E l \in ElasticLanes: InjectedNonConsensusCatalogWrite(l)
  \/ InjectedBaselineMutation

(***************************************************************************
Safety properties.
***************************************************************************)

TypeInvariant ==
  /\ checked = 0
  /\ height \in 0..MaxHeight
  /\ catalog \subseteq Lanes
  /\ committedCatalog \subseteq Lanes
  /\ physical \subseteq Lanes
  /\ generation \in [Lanes -> 0..(MaxHeight + 1)]
  /\ retiredGeneration \in [Lanes -> 0..MaxHeight]
  /\ incarnation \in [Lanes -> 0..(MaxHeight + 1)]
  /\ activation \in [Lanes -> -1..MaxHeight]
  /\ transitionHeight \in [Lanes -> -1..MaxHeight]
  /\ pending \in {-3, -2, 0, 2, 3}
  /\ pendingBase \subseteq Lanes
  /\ journalPhase \in {0, 1, 2}
  /\ pressure \in BOOLEAN
  /\ idle \in BOOLEAN
  /\ drained \subseteq Lanes
  /\ staleArtifactAccepted \in BOOLEAN
  /\ unsafeRetirement \in BOOLEAN
  /\ nonConsensusCatalogWrite \in BOOLEAN
  /\ baselineDigestIntact \in BOOLEAN

TransitionMatchesSpec ==
  \A c \in Cases: ActualCase(c) = SpecCase(c)

BaselinePreserved ==
  /\ BaselineLanes \subseteq catalog
  /\ baselineDigestIntact

CapacityBounds ==
  /\ Cardinality(catalog) >= MinLanes
  /\ Cardinality(catalog) <= MaxLanes

ActiveIncarnationDiscipline ==
  \A l \in catalog:
    /\ incarnation[l] = generation[l]
    /\ incarnation[l] > 0
    /\ generation[l] > retiredGeneration[l]
    /\ activation[l] >= 0
    /\ activation[l] <= height
    /\ activation[l] = transitionHeight[l]

InactiveIncarnationDiscipline ==
  \A l \in (Lanes \ catalog):
    /\ incarnation[l] = 0
    /\ activation[l] = -1
    /\ transitionHeight[l] <= height

CleanGeometryMatchesCatalog ==
  journalPhase = 0 => physical = catalog

PublishedGeometryMatchesCatalog ==
  journalPhase = 2 => physical = catalog

OnlyConsensusPublishesCatalog ==
  /\ ~nonConsensusCatalogWrite
  /\ catalog = committedCatalog

StaleArtifactsFailClosed == ~staleArtifactAccepted

RetirementRequiresDrain == ~unsafeRetirement

AutoscaleTransitionExactness ==
  /\ TransitionMatchesSpec
  /\ BaselinePreserved
  /\ CapacityBounds
  /\ ActiveIncarnationDiscipline
  /\ InactiveIncarnationDiscipline
  /\ CleanGeometryMatchesCatalog
  /\ PublishedGeometryMatchesCatalog
  /\ OnlyConsensusPublishesCatalog
  /\ StaleArtifactsFailClosed
  /\ RetirementRequiresDrain

AutoscaleTransitionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ AutoscaleTransitionExactness

SafetyFast == AutoscaleTransitionCorrectnessEnvelope

(***************************************************************************
Expected-failure invariants for the original source-level mutants.
***************************************************************************)

BugSkipMatchingTransition ==
  ActualCase("enabled_matching_success") = SpecCase("enabled_matching_success")

BugIgnoreEnabled ==
  ActualCase("disabled_matching_success") = SpecCase("disabled_matching_success")

BugIgnoreHeight ==
  ActualCase("enabled_previous_success") = SpecCase("enabled_previous_success")

BugOffByOnePrevious ==
  ActualCase("enabled_previous_success") = SpecCase("enabled_previous_success")

BugOffByOneNext ==
  ActualCase("enabled_next_success") = SpecCase("enabled_next_success")

BugSkipSuccessReconfigure ==
  ActualCase("enabled_matching_success") = SpecCase("enabled_matching_success")

BugReconfigureFailedCommit ==
  ActualCase("enabled_matching_failure") = SpecCase("enabled_matching_failure")

BugReconfigureWithoutTransition ==
  ActualCase("disabled_matching_success") = SpecCase("disabled_matching_success")

BugWrongReportedHeight ==
  ActualCase("enabled_matching_success") = SpecCase("enabled_matching_success")

====
