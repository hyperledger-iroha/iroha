---- MODULE SumeragiStaleMissingCommitQcPruneGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the missing commit-QC request branch of
`prune_stale_view_state(height, min_view)`.

The helper removes stale same-height known-block commit-QC recovery requests
unless one of two preservation paths applies: the request is for the exact
active frontier repair slot and still has an actionable dependency, or the
request is a Commit-phase local-payload repair for the current slot owner
without a cached commit QC. Wrong-height and fresh-view requests are not
eligible for pruning.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

WrongHeight == "wrong_height"
FreshEqualView == "fresh_equal_view"
FreshFutureView == "fresh_future_view"
StaleNoPreserve == "stale_no_preserve"
StaleExactFrontierRepair == "stale_exact_frontier_repair"
StaleExactFrontierInactive == "stale_exact_frontier_inactive"
StaleExactFrontierWrongHash == "stale_exact_frontier_wrong_hash"
StaleExactFrontierNoDependency == "stale_exact_frontier_no_dependency"
StaleLocalPayload == "stale_local_payload"
StaleLocalPayloadNonCommitPhase == "stale_local_payload_non_commit_phase"
StaleLocalPayloadCachedQc == "stale_local_payload_cached_qc"
StaleLocalPayloadWrongOwner == "stale_local_payload_wrong_owner"
StaleLocalPayloadWrongSlot == "stale_local_payload_wrong_slot"
StaleBothPreserve == "stale_both_preserve"

Cases == {
  WrongHeight,
  FreshEqualView,
  FreshFutureView,
  StaleNoPreserve,
  StaleExactFrontierRepair,
  StaleExactFrontierInactive,
  StaleExactFrontierWrongHash,
  StaleExactFrontierNoDependency,
  StaleLocalPayload,
  StaleLocalPayloadNonCommitPhase,
  StaleLocalPayloadCachedQc,
  StaleLocalPayloadWrongOwner,
  StaleLocalPayloadWrongSlot,
  StaleBothPreserve
}

StaleCases == Cases \ {WrongHeight, FreshEqualView, FreshFutureView}

ExactFrontierGateCases == {
  StaleExactFrontierRepair,
  StaleExactFrontierNoDependency,
  StaleBothPreserve
}

PreserveCases == {
  StaleExactFrontierRepair,
  StaleLocalPayload,
  StaleBothPreserve
}

SpecRemove(c) ==
  c \in StaleCases /\ ~(c \in PreserveCases)

RequestPresent == 1
RequestAbsent == 2
RemovalCountIncremented == 3
RemovalCountUnchanged == 4
FrontierGateChecked == 5
ActionableDependencyChecked == 6
LocalPayloadChecked == 7

ActionUniverse == 1..7

KeepActions ==
  {RequestPresent, RemovalCountUnchanged}

RemoveActions ==
  {RequestAbsent, RemovalCountIncremented}

SpecActions(c) ==
  (IF SpecRemove(c) THEN RemoveActions ELSE KeepActions)
    \cup (IF c \in StaleCases THEN {FrontierGateChecked, LocalPayloadChecked} ELSE {})
    \cup (IF c \in ExactFrontierGateCases THEN {ActionableDependencyChecked} ELSE {})

RemoveWithoutCountActions ==
  {RequestAbsent, RemovalCountUnchanged, FrontierGateChecked,
   LocalPayloadChecked}

RemoveExactWithoutCountActions ==
  {RequestAbsent, RemovalCountUnchanged, FrontierGateChecked,
   LocalPayloadChecked, ActionableDependencyChecked}

RemoveStaleActions ==
  {RequestAbsent, RemovalCountIncremented, FrontierGateChecked,
   LocalPayloadChecked}

RemoveExactActions ==
  {RequestAbsent, RemovalCountIncremented, FrontierGateChecked,
   LocalPayloadChecked, ActionableDependencyChecked}

KeepStaleActions ==
  {RequestPresent, RemovalCountUnchanged, FrontierGateChecked,
   LocalPayloadChecked}

KeepExactActions ==
  {RequestPresent, RemovalCountUnchanged, FrontierGateChecked,
   LocalPayloadChecked, ActionableDependencyChecked}

ImplementationActions(c) ==
  CASE Bug = "prune_wrong_height"
       /\ c = WrongHeight ->
      {RequestAbsent, RemovalCountIncremented}
    [] Bug = "prune_equal_view"
       /\ c = FreshEqualView ->
      {RequestAbsent, RemovalCountIncremented, FrontierGateChecked}
    [] Bug = "prune_future_view"
       /\ c = FreshFutureView ->
      {RequestAbsent, RemovalCountIncremented, FrontierGateChecked}
    [] Bug = "keep_stale_no_preserve"
       /\ c = StaleNoPreserve ->
      KeepStaleActions
    [] Bug = "drop_exact_frontier_repair"
       /\ c = StaleExactFrontierRepair ->
      RemoveExactActions
    [] Bug = "preserve_exact_frontier_inactive"
       /\ c = StaleExactFrontierInactive ->
      KeepStaleActions
    [] Bug = "preserve_exact_frontier_wrong_hash"
       /\ c = StaleExactFrontierWrongHash ->
      KeepStaleActions
    [] Bug = "preserve_exact_frontier_no_dependency"
       /\ c = StaleExactFrontierNoDependency ->
      KeepExactActions
    [] Bug = "drop_local_payload"
       /\ c = StaleLocalPayload ->
      RemoveStaleActions
    [] Bug = "accept_local_payload_non_commit_phase"
       /\ c = StaleLocalPayloadNonCommitPhase ->
      KeepStaleActions
    [] Bug = "accept_local_payload_cached_qc"
       /\ c = StaleLocalPayloadCachedQc ->
      KeepStaleActions
    [] Bug = "accept_local_payload_wrong_owner"
       /\ c = StaleLocalPayloadWrongOwner ->
      KeepStaleActions
    [] Bug = "accept_local_payload_wrong_slot"
       /\ c = StaleLocalPayloadWrongSlot ->
      KeepStaleActions
    [] Bug = "require_both_preserve_sources"
       /\ c \in {StaleExactFrontierRepair, StaleLocalPayload} ->
      IF c = StaleExactFrontierRepair
      THEN RemoveExactActions
      ELSE RemoveStaleActions
    [] Bug = "skip_removal_count"
       /\ c = StaleNoPreserve ->
      RemoveWithoutCountActions
    [] Bug = "skip_removal_count_exact"
       /\ c = StaleExactFrontierNoDependency ->
      RemoveExactWithoutCountActions
    [] OTHER -> SpecActions(c)

Bugs == {
  "none",
  "prune_wrong_height",
  "prune_equal_view",
  "prune_future_view",
  "keep_stale_no_preserve",
  "drop_exact_frontier_repair",
  "preserve_exact_frontier_inactive",
  "preserve_exact_frontier_wrong_hash",
  "preserve_exact_frontier_no_dependency",
  "drop_local_payload",
  "accept_local_payload_non_commit_phase",
  "accept_local_payload_cached_qc",
  "accept_local_payload_wrong_owner",
  "accept_local_payload_wrong_slot",
  "require_both_preserve_sources",
  "skip_removal_count",
  "skip_removal_count_exact"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecRemove(c) \in BOOLEAN
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

OnlyStaleSameHeightRequestsRemoved ==
  /\ RequestPresent \in ImplementationActions(WrongHeight)
  /\ RequestPresent \in ImplementationActions(FreshEqualView)
  /\ RequestPresent \in ImplementationActions(FreshFutureView)
  /\ RequestAbsent \in ImplementationActions(StaleNoPreserve)

ExactFrontierRepairRequiresActiveActionableExactSlot ==
  /\ RequestPresent \in ImplementationActions(StaleExactFrontierRepair)
  /\ RequestAbsent \in ImplementationActions(StaleExactFrontierInactive)
  /\ RequestAbsent \in ImplementationActions(StaleExactFrontierWrongHash)
  /\ RequestAbsent \in ImplementationActions(StaleExactFrontierNoDependency)
  /\ ActionableDependencyChecked \in
       ImplementationActions(StaleExactFrontierRepair)

LocalPayloadRepairRequiresCommitPhaseFreshQcOwnerAndExactSlot ==
  /\ RequestPresent \in ImplementationActions(StaleLocalPayload)
  /\ RequestAbsent \in ImplementationActions(StaleLocalPayloadNonCommitPhase)
  /\ RequestAbsent \in ImplementationActions(StaleLocalPayloadCachedQc)
  /\ RequestAbsent \in ImplementationActions(StaleLocalPayloadWrongOwner)
  /\ RequestAbsent \in ImplementationActions(StaleLocalPayloadWrongSlot)
  /\ LocalPayloadChecked \in ImplementationActions(StaleLocalPayload)

EitherPreserveSourceIsSufficient ==
  /\ RequestPresent \in ImplementationActions(StaleExactFrontierRepair)
  /\ RequestPresent \in ImplementationActions(StaleLocalPayload)
  /\ RequestPresent \in ImplementationActions(StaleBothPreserve)

RemovalCounterMatchesRemoval ==
  \A c \in Cases:
    (RequestAbsent \in ImplementationActions(c))
      <=> (RemovalCountIncremented \in ImplementationActions(c))

NoRequestKeptWithIncrement ==
  \A c \in Cases:
    (RequestPresent \in ImplementationActions(c))
      => ~(RemovalCountIncremented \in ImplementationActions(c))

NonStaleRetentionAnchors ==
  /\ ImplementationActions(WrongHeight) = KeepActions
  /\ ImplementationActions(FreshEqualView) = KeepActions
  /\ ImplementationActions(FreshFutureView) = KeepActions

StaleRemovalAnchors ==
  /\ RequestAbsent \in ImplementationActions(StaleNoPreserve)
  /\ RequestAbsent \in ImplementationActions(StaleExactFrontierInactive)
  /\ RequestAbsent \in ImplementationActions(StaleExactFrontierWrongHash)
  /\ RequestAbsent \in ImplementationActions(StaleExactFrontierNoDependency)
  /\ RequestAbsent \in ImplementationActions(StaleLocalPayloadNonCommitPhase)
  /\ RequestAbsent \in ImplementationActions(StaleLocalPayloadCachedQc)
  /\ RequestAbsent \in ImplementationActions(StaleLocalPayloadWrongOwner)
  /\ RequestAbsent \in ImplementationActions(StaleLocalPayloadWrongSlot)

PreserveSourceAnchors ==
  /\ RequestPresent \in ImplementationActions(StaleExactFrontierRepair)
  /\ RequestPresent \in ImplementationActions(StaleLocalPayload)
  /\ RequestPresent \in ImplementationActions(StaleBothPreserve)
  /\ ~(RequestAbsent \in ImplementationActions(StaleExactFrontierRepair))
  /\ ~(RequestAbsent \in ImplementationActions(StaleLocalPayload))
  /\ ~(RequestAbsent \in ImplementationActions(StaleBothPreserve))

GateCheckAnchors ==
  /\ \A c \in StaleCases:
       /\ FrontierGateChecked \in ImplementationActions(c)
       /\ LocalPayloadChecked \in ImplementationActions(c)
  /\ ActionableDependencyChecked \in
       ImplementationActions(StaleExactFrontierRepair)
  /\ ActionableDependencyChecked \in
       ImplementationActions(StaleExactFrontierNoDependency)
  /\ ActionableDependencyChecked \in ImplementationActions(StaleBothPreserve)
  /\ ~(ActionableDependencyChecked \in
       ImplementationActions(StaleExactFrontierInactive))
  /\ ~(ActionableDependencyChecked \in
       ImplementationActions(StaleExactFrontierWrongHash))

RemovalCounterAnchors ==
  /\ RemovalCountUnchanged \in ImplementationActions(WrongHeight)
  /\ RemovalCountUnchanged \in ImplementationActions(FreshEqualView)
  /\ RemovalCountUnchanged \in ImplementationActions(FreshFutureView)
  /\ RemovalCountUnchanged \in ImplementationActions(StaleExactFrontierRepair)
  /\ RemovalCountUnchanged \in ImplementationActions(StaleLocalPayload)
  /\ RemovalCountUnchanged \in ImplementationActions(StaleBothPreserve)
  /\ RemovalCountIncremented \in ImplementationActions(StaleNoPreserve)
  /\ RemovalCountIncremented \in
       ImplementationActions(StaleExactFrontierNoDependency)
  /\ RemovalCountIncremented \in
       ImplementationActions(StaleLocalPayloadCachedQc)

StaleMissingCommitQcPruneCoreSafety ==
  /\ ActionsMatchSpec
  /\ OnlyStaleSameHeightRequestsRemoved
  /\ ExactFrontierRepairRequiresActiveActionableExactSlot
  /\ LocalPayloadRepairRequiresCommitPhaseFreshQcOwnerAndExactSlot
  /\ EitherPreserveSourceIsSufficient
  /\ RemovalCounterMatchesRemoval
  /\ NoRequestKeptWithIncrement
  /\ NonStaleRetentionAnchors
  /\ StaleRemovalAnchors
  /\ PreserveSourceAnchors
  /\ GateCheckAnchors
  /\ RemovalCounterAnchors

NoBugInvariant == StaleMissingCommitQcPruneCoreSafety

SafetyFast == StaleMissingCommitQcPruneCoreSafety

====
