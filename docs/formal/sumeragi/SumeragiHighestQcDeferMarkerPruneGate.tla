---- MODULE SumeragiHighestQcDeferMarkerPruneGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for cleanup of
`highest_qc_missing_defer_markers`.

This slice pins three marker-pruning paths:
- `prune_stale_view_state(height, min_view)` removes only same-height markers
  whose view is below the new minimum view;
- `clear_consensus_recovery_for_round(height, view)` removes same-height
  markers through the cleared view and keeps later-view markers;
- `prune_highest_qc_missing_defer_markers(committed_height)` removes markers
  at or below the committed height, markers whose block is now known locally,
  and markers whose hash is no longer an actionable dependency.

Wrong-height markers, equal-min-view markers, later-view recovery markers, and
future unresolved dependencies must remain available for the missing-QC
recovery/stall machinery.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ViewWrongHeight == "view_wrong_height"
ViewStaleLower == "view_stale_lower"
ViewEqualMin == "view_equal_min"
ViewFuture == "view_future"

RecoveryWrongHeight == "recovery_wrong_height"
RecoveryLower == "recovery_lower"
RecoveryEqual == "recovery_equal"
RecoveryFuture == "recovery_future"

DependencyCommittedBelow == "dependency_committed_below"
DependencyCommittedEqual == "dependency_committed_equal"
DependencyKnownLocalFuture == "dependency_known_local_future"
DependencyNonActionableFuture == "dependency_non_actionable_future"
DependencyUnresolvedFuture == "dependency_unresolved_future"

ViewCases == {
  ViewWrongHeight,
  ViewStaleLower,
  ViewEqualMin,
  ViewFuture
}

RecoveryCases == {
  RecoveryWrongHeight,
  RecoveryLower,
  RecoveryEqual,
  RecoveryFuture
}

DependencyCases == {
  DependencyCommittedBelow,
  DependencyCommittedEqual,
  DependencyKnownLocalFuture,
  DependencyNonActionableFuture,
  DependencyUnresolvedFuture
}

Cases == ViewCases \cup RecoveryCases \cup DependencyCases

ViewSameHeightCases == ViewCases \ {ViewWrongHeight}
RecoverySameHeightCases == RecoveryCases \ {RecoveryWrongHeight}
DependencyFutureCases ==
  DependencyCases \ {DependencyCommittedBelow, DependencyCommittedEqual}

SpecRemove(c) ==
  c \in {
    ViewStaleLower,
    RecoveryLower,
    RecoveryEqual,
    DependencyCommittedBelow,
    DependencyCommittedEqual,
    DependencyKnownLocalFuture,
    DependencyNonActionableFuture
  }

MarkerPresent == 1
MarkerAbsent == 2
ViewHeightChecked == 3
ViewMinViewChecked == 4
RecoveryHeightChecked == 5
RecoveryViewChecked == 6
CommittedHeightChecked == 7
LocalKnownChecked == 8
NonActionableChecked == 9

ActionUniverse == 1..9

KeepActions == {MarkerPresent}
RemoveActions == {MarkerAbsent}

SpecActions(c) ==
  (IF SpecRemove(c) THEN RemoveActions ELSE KeepActions)
    \cup (IF c \in ViewCases THEN {ViewHeightChecked} ELSE {})
    \cup (IF c \in ViewSameHeightCases THEN {ViewMinViewChecked} ELSE {})
    \cup (IF c \in RecoveryCases THEN {RecoveryHeightChecked} ELSE {})
    \cup (IF c \in RecoverySameHeightCases THEN {RecoveryViewChecked} ELSE {})
    \cup (IF c \in DependencyCases THEN {CommittedHeightChecked} ELSE {})
    \cup (IF c \in DependencyFutureCases THEN {LocalKnownChecked} ELSE {})
    \cup (IF c \in {DependencyNonActionableFuture, DependencyUnresolvedFuture}
        THEN {NonActionableChecked}
        ELSE {})

ViewWrongHeightRemovedActions ==
  {MarkerAbsent, ViewHeightChecked}

ViewSameHeightKeptActions ==
  {MarkerPresent, ViewHeightChecked, ViewMinViewChecked}

ViewSameHeightRemovedActions ==
  {MarkerAbsent, ViewHeightChecked, ViewMinViewChecked}

RecoveryWrongHeightRemovedActions ==
  {MarkerAbsent, RecoveryHeightChecked}

RecoverySameHeightKeptActions ==
  {MarkerPresent, RecoveryHeightChecked, RecoveryViewChecked}

RecoverySameHeightRemovedActions ==
  {MarkerAbsent, RecoveryHeightChecked, RecoveryViewChecked}

DependencyCommittedKeptActions ==
  {MarkerPresent, CommittedHeightChecked}

DependencyKnownLocalKeptActions ==
  {MarkerPresent, CommittedHeightChecked, LocalKnownChecked}

DependencyKnownLocalRemovedWithoutCheckActions ==
  {MarkerAbsent, CommittedHeightChecked}

DependencyNonActionableKeptActions ==
  {MarkerPresent, CommittedHeightChecked, LocalKnownChecked,
   NonActionableChecked}

DependencyNonActionableRemovedWithoutCheckActions ==
  {MarkerAbsent, CommittedHeightChecked, LocalKnownChecked}

DependencyUnresolvedRemovedActions ==
  {MarkerAbsent, CommittedHeightChecked, LocalKnownChecked,
   NonActionableChecked}

ImplementationActions(c) ==
  CASE Bug = "view_prune_wrong_height"
       /\ c = ViewWrongHeight ->
      ViewWrongHeightRemovedActions
    [] Bug = "view_keep_stale"
       /\ c = ViewStaleLower ->
      ViewSameHeightKeptActions
    [] Bug = "view_prune_equal_min"
       /\ c = ViewEqualMin ->
      ViewSameHeightRemovedActions
    [] Bug = "view_prune_future"
       /\ c = ViewFuture ->
      ViewSameHeightRemovedActions
    [] Bug = "recovery_prune_wrong_height"
       /\ c = RecoveryWrongHeight ->
      RecoveryWrongHeightRemovedActions
    [] Bug = "recovery_keep_lower"
       /\ c = RecoveryLower ->
      RecoverySameHeightKeptActions
    [] Bug = "recovery_keep_equal"
       /\ c = RecoveryEqual ->
      RecoverySameHeightKeptActions
    [] Bug = "recovery_prune_future"
       /\ c = RecoveryFuture ->
      RecoverySameHeightRemovedActions
    [] Bug = "dependency_keep_below_committed"
       /\ c = DependencyCommittedBelow ->
      DependencyCommittedKeptActions
    [] Bug = "dependency_keep_equal_committed"
       /\ c = DependencyCommittedEqual ->
      DependencyCommittedKeptActions
    [] Bug = "dependency_keep_known_local"
       /\ c = DependencyKnownLocalFuture ->
      DependencyKnownLocalKeptActions
    [] Bug = "dependency_keep_non_actionable"
       /\ c = DependencyNonActionableFuture ->
      DependencyNonActionableKeptActions
    [] Bug = "dependency_drop_unresolved"
       /\ c = DependencyUnresolvedFuture ->
      DependencyUnresolvedRemovedActions
    [] Bug = "dependency_skip_local_known_check"
       /\ c = DependencyKnownLocalFuture ->
      DependencyKnownLocalRemovedWithoutCheckActions
    [] Bug = "dependency_skip_non_actionable_check"
       /\ c = DependencyNonActionableFuture ->
      DependencyNonActionableRemovedWithoutCheckActions
    [] OTHER -> SpecActions(c)

Bugs == {
  "none",
  "view_prune_wrong_height",
  "view_keep_stale",
  "view_prune_equal_min",
  "view_prune_future",
  "recovery_prune_wrong_height",
  "recovery_keep_lower",
  "recovery_keep_equal",
  "recovery_prune_future",
  "dependency_keep_below_committed",
  "dependency_keep_equal_committed",
  "dependency_keep_known_local",
  "dependency_keep_non_actionable",
  "dependency_drop_unresolved",
  "dependency_skip_local_known_check",
  "dependency_skip_non_actionable_check"
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

ViewChangePrunesOnlyStaleSameHeightMarkers ==
  /\ MarkerPresent \in ImplementationActions(ViewWrongHeight)
  /\ MarkerAbsent \in ImplementationActions(ViewStaleLower)
  /\ MarkerPresent \in ImplementationActions(ViewEqualMin)
  /\ MarkerPresent \in ImplementationActions(ViewFuture)

RecoveryClearRemovesSameHeightThroughCurrentView ==
  /\ MarkerPresent \in ImplementationActions(RecoveryWrongHeight)
  /\ MarkerAbsent \in ImplementationActions(RecoveryLower)
  /\ MarkerAbsent \in ImplementationActions(RecoveryEqual)
  /\ MarkerPresent \in ImplementationActions(RecoveryFuture)

DependencyCleanupRemovesCommittedKnownOrNonActionableMarkers ==
  /\ MarkerAbsent \in ImplementationActions(DependencyCommittedBelow)
  /\ MarkerAbsent \in ImplementationActions(DependencyCommittedEqual)
  /\ MarkerAbsent \in ImplementationActions(DependencyKnownLocalFuture)
  /\ MarkerAbsent \in ImplementationActions(DependencyNonActionableFuture)
  /\ MarkerPresent \in ImplementationActions(DependencyUnresolvedFuture)

DependencyCleanupShortCircuitsAfterCommittedOrKnown ==
  /\ ~(LocalKnownChecked \in ImplementationActions(DependencyCommittedBelow))
  /\ ~(LocalKnownChecked \in ImplementationActions(DependencyCommittedEqual))
  /\ LocalKnownChecked \in ImplementationActions(DependencyKnownLocalFuture)
  /\ ~(NonActionableChecked \in
       ImplementationActions(DependencyKnownLocalFuture))
  /\ NonActionableChecked \in
       ImplementationActions(DependencyNonActionableFuture)
  /\ NonActionableChecked \in
       ImplementationActions(DependencyUnresolvedFuture)

NoMarkerIsBothPresentAndAbsent ==
  \A c \in Cases:
    ~(/\ MarkerPresent \in ImplementationActions(c)
      /\ MarkerAbsent \in ImplementationActions(c))

ViewWrongHeightRetentionAnchors ==
  /\ ImplementationActions(ViewWrongHeight) =
       {MarkerPresent, ViewHeightChecked}
  /\ ~(ViewMinViewChecked \in ImplementationActions(ViewWrongHeight))

ViewStaleBoundaryAnchors ==
  /\ MarkerAbsent \in ImplementationActions(ViewStaleLower)
  /\ MarkerPresent \in ImplementationActions(ViewEqualMin)
  /\ MarkerPresent \in ImplementationActions(ViewFuture)
  /\ ViewMinViewChecked \in ImplementationActions(ViewStaleLower)
  /\ ViewMinViewChecked \in ImplementationActions(ViewEqualMin)
  /\ ViewMinViewChecked \in ImplementationActions(ViewFuture)

RecoveryWrongHeightRetentionAnchors ==
  /\ ImplementationActions(RecoveryWrongHeight) =
       {MarkerPresent, RecoveryHeightChecked}
  /\ ~(RecoveryViewChecked \in ImplementationActions(RecoveryWrongHeight))

RecoveryViewBoundaryAnchors ==
  /\ MarkerAbsent \in ImplementationActions(RecoveryLower)
  /\ MarkerAbsent \in ImplementationActions(RecoveryEqual)
  /\ MarkerPresent \in ImplementationActions(RecoveryFuture)
  /\ RecoveryViewChecked \in ImplementationActions(RecoveryLower)
  /\ RecoveryViewChecked \in ImplementationActions(RecoveryEqual)
  /\ RecoveryViewChecked \in ImplementationActions(RecoveryFuture)

DependencyCommittedRemovalAnchors ==
  /\ ImplementationActions(DependencyCommittedBelow) =
       {MarkerAbsent, CommittedHeightChecked}
  /\ ImplementationActions(DependencyCommittedEqual) =
       {MarkerAbsent, CommittedHeightChecked}
  /\ ~(LocalKnownChecked \in ImplementationActions(DependencyCommittedBelow))
  /\ ~(LocalKnownChecked \in ImplementationActions(DependencyCommittedEqual))
  /\ ~(NonActionableChecked \in
       ImplementationActions(DependencyCommittedBelow))
  /\ ~(NonActionableChecked \in
       ImplementationActions(DependencyCommittedEqual))

DependencyFuturePredicateAnchors ==
  /\ MarkerAbsent \in ImplementationActions(DependencyKnownLocalFuture)
  /\ LocalKnownChecked \in
       ImplementationActions(DependencyKnownLocalFuture)
  /\ ~(NonActionableChecked \in
       ImplementationActions(DependencyKnownLocalFuture))
  /\ MarkerAbsent \in ImplementationActions(DependencyNonActionableFuture)
  /\ LocalKnownChecked \in
       ImplementationActions(DependencyNonActionableFuture)
  /\ NonActionableChecked \in
       ImplementationActions(DependencyNonActionableFuture)
  /\ MarkerPresent \in ImplementationActions(DependencyUnresolvedFuture)
  /\ LocalKnownChecked \in
       ImplementationActions(DependencyUnresolvedFuture)
  /\ NonActionableChecked \in
       ImplementationActions(DependencyUnresolvedFuture)

NoBugInvariant ==
  /\ ActionsMatchSpec
  /\ ViewChangePrunesOnlyStaleSameHeightMarkers
  /\ RecoveryClearRemovesSameHeightThroughCurrentView
  /\ DependencyCleanupRemovesCommittedKnownOrNonActionableMarkers
  /\ DependencyCleanupShortCircuitsAfterCommittedOrKnown
  /\ NoMarkerIsBothPresentAndAbsent
  /\ ViewWrongHeightRetentionAnchors
  /\ ViewStaleBoundaryAnchors
  /\ RecoveryWrongHeightRetentionAnchors
  /\ RecoveryViewBoundaryAnchors
  /\ DependencyCommittedRemovalAnchors
  /\ DependencyFuturePredicateAnchors

SafetyFast == NoBugInvariant

HighestQcDeferMarkerPruneExactness ==
  /\ ActionsMatchSpec
  /\ ViewChangePrunesOnlyStaleSameHeightMarkers
  /\ RecoveryClearRemovesSameHeightThroughCurrentView
  /\ DependencyCleanupRemovesCommittedKnownOrNonActionableMarkers
  /\ DependencyCleanupShortCircuitsAfterCommittedOrKnown
  /\ NoMarkerIsBothPresentAndAbsent
  /\ ViewWrongHeightRetentionAnchors
  /\ ViewStaleBoundaryAnchors
  /\ RecoveryWrongHeightRetentionAnchors
  /\ RecoveryViewBoundaryAnchors
  /\ DependencyCommittedRemovalAnchors
  /\ DependencyFuturePredicateAnchors

HighestQcDeferMarkerPruneCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ HighestQcDeferMarkerPruneExactness

====
