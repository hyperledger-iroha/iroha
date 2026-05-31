---- MODULE SumeragiRosterIndexProjectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi roster index projection helpers.

This slice captures `compute_roster_indices_from_topology(...)` and
`apply_roster_indices_to_manager(...)` from `main_loop/roster.rs`. It abstracts
peers and provider rosters into a small finite set while preserving the
observable contracts:

- empty topology projects to an empty index list,
- absent providers fall back to contiguous topology-local indices,
- complete providers project provider positions in topology order,
- incomplete providers fall back to contiguous local indices,
- provider index overflow fails closed with an empty projection,
- epoch managers receive contiguous topology-local indices using the projected
  index count, not the sparse/global provider values, and
- effective-length overflow leaves the previous manager state unchanged.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ProjectionEmptyTopology == "projection_empty_topology"
ProjectionNoProviderLen3 == "projection_no_provider_len3"
ProjectionCompleteProviderSparse == "projection_complete_provider_sparse"
ProjectionProviderMissingFallback == "projection_provider_missing_fallback"
ProjectionProviderEmptyFallback == "projection_provider_empty_fallback"
ProjectionProviderOverflowEmpty == "projection_provider_overflow_empty"
ApplyNonemptyNormalizes == "apply_nonempty_normalizes"
ApplyEmptyUsesRosterLen == "apply_empty_uses_roster_len"
ApplyEmptyZeroLen == "apply_empty_zero_len"
ApplyOverflowLeavesPrevious == "apply_overflow_leaves_previous"
ProjectionThenManager == "projection_then_manager"
MissingThenManager == "missing_then_manager"

Cases == {
  ProjectionEmptyTopology,
  ProjectionNoProviderLen3,
  ProjectionCompleteProviderSparse,
  ProjectionProviderMissingFallback,
  ProjectionProviderEmptyFallback,
  ProjectionProviderOverflowEmpty,
  ApplyNonemptyNormalizes,
  ApplyEmptyUsesRosterLen,
  ApplyEmptyZeroLen,
  ApplyOverflowLeavesPrevious,
  ProjectionThenManager,
  MissingThenManager
}

ProjEmpty == 1
ProjLocal01 == 2
ProjLocal012 == 3
ProjSparse21 == 4
ProjPartial0 == 5
ProjLocal12 == 6
ProjSparse12 == 7
ProjLocal123 == 8
ManagerEmpty == 9
Manager01 == 10
Manager012 == 11
ManagerPrev45 == 12
ManagerSparse79 == 13
Manager0 == 14
Manager0123 == 15

Actions == 1..15

SpecActions(c) ==
  CASE c = ProjectionEmptyTopology ->
      {ProjEmpty}
    [] c = ProjectionNoProviderLen3 ->
      {ProjLocal012}
    [] c = ProjectionCompleteProviderSparse ->
      {ProjSparse21}
    [] c = ProjectionProviderMissingFallback ->
      {ProjLocal01}
    [] c = ProjectionProviderEmptyFallback ->
      {ProjLocal01}
    [] c = ProjectionProviderOverflowEmpty ->
      {ProjEmpty}
    [] c = ApplyNonemptyNormalizes ->
      {Manager01}
    [] c = ApplyEmptyUsesRosterLen ->
      {Manager012}
    [] c = ApplyEmptyZeroLen ->
      {ManagerEmpty}
    [] c = ApplyOverflowLeavesPrevious ->
      {ManagerPrev45}
    [] c = ProjectionThenManager ->
      {ProjSparse21, Manager01}
    [] c = MissingThenManager ->
      {ProjLocal01, Manager01}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "project_empty_nonempty"
       /\ c = ProjectionNoProviderLen3 ->
      (spec \ {ProjLocal012}) \cup {ProjEmpty}
    [] Bug = "no_provider_one_based"
       /\ c = ProjectionNoProviderLen3 ->
      (spec \ {ProjLocal012}) \cup {ProjLocal123}
    [] Bug = "provider_sparse_uses_topology_indices"
       /\ c = ProjectionCompleteProviderSparse ->
      (spec \ {ProjSparse21}) \cup {ProjLocal01}
    [] Bug = "provider_sparse_sorts_indices"
       /\ c = ProjectionCompleteProviderSparse ->
      (spec \ {ProjSparse21}) \cup {ProjSparse12}
    [] Bug = "missing_provider_keeps_partial"
       /\ c = ProjectionProviderMissingFallback ->
      (spec \ {ProjLocal01}) \cup {ProjPartial0}
    [] Bug = "missing_provider_empty"
       /\ c = ProjectionProviderEmptyFallback ->
      (spec \ {ProjLocal01}) \cup {ProjEmpty}
    [] Bug = "provider_overflow_falls_back"
       /\ c = ProjectionProviderOverflowEmpty ->
      (spec \ {ProjEmpty}) \cup {ProjLocal01}
    [] Bug = "apply_nonempty_preserves_sparse"
       /\ c = ApplyNonemptyNormalizes ->
      (spec \ {Manager01}) \cup {ManagerSparse79}
    [] Bug = "apply_empty_uses_zero"
       /\ c = ApplyEmptyUsesRosterLen ->
      (spec \ {Manager012}) \cup {ManagerEmpty}
    [] Bug = "apply_empty_off_by_one"
       /\ c = ApplyEmptyUsesRosterLen ->
      (spec \ {Manager012}) \cup {Manager01}
    [] Bug = "apply_zero_empty_inserts_zero"
       /\ c = ApplyEmptyZeroLen ->
      (spec \ {ManagerEmpty}) \cup {Manager0}
    [] Bug = "apply_overflow_clears"
       /\ c = ApplyOverflowLeavesPrevious ->
      (spec \ {ManagerPrev45}) \cup {ManagerEmpty}
    [] Bug = "apply_overflow_sets_effective"
       /\ c = ApplyOverflowLeavesPrevious ->
      (spec \ {ManagerPrev45}) \cup {Manager0123}
    [] Bug = "projected_manager_uses_provider_max"
       /\ c = ProjectionThenManager ->
      (spec \ {Manager01}) \cup {Manager012}
    [] Bug = "missing_then_manager_uses_partial"
       /\ c = MissingThenManager ->
      (spec \ {ProjLocal01, Manager01}) \cup {ProjPartial0, Manager0}
    [] OTHER -> spec

Bugs == {
  "none",
  "project_empty_nonempty",
  "no_provider_one_based",
  "provider_sparse_uses_topology_indices",
  "provider_sparse_sorts_indices",
  "missing_provider_keeps_partial",
  "missing_provider_empty",
  "provider_overflow_falls_back",
  "apply_nonempty_preserves_sparse",
  "apply_empty_uses_zero",
  "apply_empty_off_by_one",
  "apply_zero_empty_inserts_zero",
  "apply_overflow_clears",
  "apply_overflow_sets_effective",
  "projected_manager_uses_provider_max",
  "missing_then_manager_uses_partial"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

NoBugInvariant ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

SafetyFast == NoBugInvariant

BugProjectEmptyNonempty == NoBugInvariant
BugNoProviderOneBased == NoBugInvariant
BugProviderSparseUsesTopologyIndices == NoBugInvariant
BugProviderSparseSortsIndices == NoBugInvariant
BugMissingProviderKeepsPartial == NoBugInvariant
BugMissingProviderEmpty == NoBugInvariant
BugProviderOverflowFallsBack == NoBugInvariant
BugApplyNonemptyPreservesSparse == NoBugInvariant
BugApplyEmptyUsesZero == NoBugInvariant
BugApplyEmptyOffByOne == NoBugInvariant
BugApplyZeroEmptyInsertsZero == NoBugInvariant
BugApplyOverflowClears == NoBugInvariant
BugApplyOverflowSetsEffective == NoBugInvariant
BugProjectedManagerUsesProviderMax == NoBugInvariant
BugMissingThenManagerUsesPartial == NoBugInvariant

====
