---- MODULE SumeragiNewViewTrackerGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `NewViewTracker` aggregation semantics.

This slice covers the mutable tracker in `main_loop.rs`: highest-QC promotion,
per-slot sender deduplication, roster/local quorum counting, pruning/removal
boundaries, and highest eligible selection. Concrete peers and QC hashes are
collapsed into representative cases while preserving the ordering and quorum
contracts that drive view-change re-anchoring.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

HighestKeepsLower == 1
HighestPromotesHigherHeight == 2
HighestPromotesHigherView == 3
HighestPromotesCommitTie == 4
HighestKeepsCommitTieOverPrepare == 5
RecordInsertsNewEntry == 6
RecordDuplicateSenderStable == 7
RecordDistinctSenderIncrements == 8
RecordSeparatesKeys == 9
CountFiltersRoster == 10
CountIncludesRosterSender == 11
CountAddsLocalInRosterWhenAbsent == 12
CountDoesNotDoubleCountLocal == 13
CountIgnoresLocalOutsideRoster == 14
SelectEmptyRosterNone == 15
SelectRequiresQuorum == 16
SelectChoosesHighestEligibleKey == 17
SelectForHeightFiltersExactHeight == 18
SelectAtOrAboveFiltersMinHeight == 19
HighestQuorumViewChoosesHighestView == 20
HighestQuorumViewFiltersHeight == 21
PruneCommittedDropsBelowAndEqual == 22
PruneCommittedKeepsAbove == 23
DropBelowHeightDropsBelow == 24
DropBelowHeightKeepsEqualAndAbove == 25
RemoveDropsExactKey == 26
RemoveKeepsOtherKeys == 27
DropBelowViewDropsLowerSameHeight == 28
DropBelowViewKeepsEqualAndHigherSameHeight == 29
DropBelowViewKeepsOtherHeight == 30

Candidates == 1..30

KeepExistingHighest == 1
PromoteHigherHeight == 2
PromoteHigherView == 3
PromoteCommitTie == 4
RejectLowerHighest == 5
RejectPrepareTieDemotion == 6
InsertEntry == 7
InsertSender == 8
KeepDuplicateCount == 9
IncrementDistinctCount == 10
PreserveOtherKeyCount == 11
CountRosterSender == 12
IgnoreNonRosterSender == 13
CountLocalAbsentInRoster == 14
DoNotDoubleCountLocal == 15
IgnoreLocalOutsideRoster == 16
ReturnNone == 17
ReturnSelection == 18
RequireRosterNonEmpty == 19
RequireQuorum == 20
SelectHighestKey == 21
FilterExactHeight == 22
FilterMinHeight == 23
ReturnHighestView == 24
RemoveCommittedAndBelow == 25
KeepAboveCommitted == 26
RemoveBelowHeight == 27
KeepEqualHeight == 28
KeepAboveHeight == 29
RemoveExactKey == 30
KeepOtherKey == 31
DropLowerViewSameHeight == 32
KeepEqualViewSameHeight == 33
KeepHigherViewSameHeight == 34
KeepOtherHeight == 35

Actions == 1..35

SpecActions(candidate) ==
  CASE candidate = HighestKeepsLower ->
      {KeepExistingHighest, RejectLowerHighest}
    [] candidate = HighestPromotesHigherHeight ->
      {PromoteHigherHeight}
    [] candidate = HighestPromotesHigherView ->
      {PromoteHigherView}
    [] candidate = HighestPromotesCommitTie ->
      {PromoteCommitTie}
    [] candidate = HighestKeepsCommitTieOverPrepare ->
      {KeepExistingHighest, RejectPrepareTieDemotion}
    [] candidate = RecordInsertsNewEntry ->
      {InsertEntry, InsertSender}
    [] candidate = RecordDuplicateSenderStable ->
      {InsertSender, KeepDuplicateCount}
    [] candidate = RecordDistinctSenderIncrements ->
      {InsertSender, IncrementDistinctCount}
    [] candidate = RecordSeparatesKeys ->
      {InsertEntry, InsertSender, PreserveOtherKeyCount}
    [] candidate = CountFiltersRoster ->
      {CountRosterSender, IgnoreNonRosterSender}
    [] candidate = CountIncludesRosterSender ->
      {CountRosterSender}
    [] candidate = CountAddsLocalInRosterWhenAbsent ->
      {CountRosterSender, CountLocalAbsentInRoster}
    [] candidate = CountDoesNotDoubleCountLocal ->
      {CountRosterSender, DoNotDoubleCountLocal}
    [] candidate = CountIgnoresLocalOutsideRoster ->
      {CountRosterSender, IgnoreLocalOutsideRoster}
    [] candidate = SelectEmptyRosterNone ->
      {ReturnNone}
    [] candidate = SelectRequiresQuorum ->
      {RequireRosterNonEmpty, RequireQuorum, ReturnNone}
    [] candidate = SelectChoosesHighestEligibleKey ->
      {RequireRosterNonEmpty, RequireQuorum, ReturnSelection, SelectHighestKey}
    [] candidate = SelectForHeightFiltersExactHeight ->
      {RequireRosterNonEmpty, RequireQuorum, ReturnSelection, FilterExactHeight}
    [] candidate = SelectAtOrAboveFiltersMinHeight ->
      {RequireRosterNonEmpty, RequireQuorum, ReturnSelection, FilterMinHeight}
    [] candidate = HighestQuorumViewChoosesHighestView ->
      {RequireRosterNonEmpty, RequireQuorum, ReturnHighestView}
    [] candidate = HighestQuorumViewFiltersHeight ->
      {RequireRosterNonEmpty, RequireQuorum, ReturnHighestView, FilterExactHeight}
    [] candidate = PruneCommittedDropsBelowAndEqual ->
      {RemoveCommittedAndBelow}
    [] candidate = PruneCommittedKeepsAbove ->
      {KeepAboveCommitted}
    [] candidate = DropBelowHeightDropsBelow ->
      {RemoveBelowHeight}
    [] candidate = DropBelowHeightKeepsEqualAndAbove ->
      {KeepEqualHeight, KeepAboveHeight}
    [] candidate = RemoveDropsExactKey ->
      {RemoveExactKey}
    [] candidate = RemoveKeepsOtherKeys ->
      {KeepOtherKey}
    [] candidate = DropBelowViewDropsLowerSameHeight ->
      {DropLowerViewSameHeight, KeepOtherHeight}
    [] candidate = DropBelowViewKeepsEqualAndHigherSameHeight ->
      {KeepEqualViewSameHeight, KeepHigherViewSameHeight}
    [] candidate = DropBelowViewKeepsOtherHeight ->
      {KeepOtherHeight}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = HighestKeepsLower /\ Bug = "highest_lower_not_ignored" ->
      (spec \ {KeepExistingHighest, RejectLowerHighest}) \cup {PromoteHigherHeight}
    [] candidate = HighestPromotesHigherHeight /\
          Bug = "highest_higher_height_not_promoted" ->
      spec \ {PromoteHigherHeight}
    [] candidate = HighestPromotesHigherView /\
          Bug = "highest_higher_view_not_promoted" ->
      spec \ {PromoteHigherView}
    [] candidate = HighestPromotesCommitTie /\
          Bug = "highest_commit_tie_not_promoted" ->
      spec \ {PromoteCommitTie}
    [] candidate = HighestKeepsCommitTieOverPrepare /\
          Bug = "highest_prepare_demotes_commit" ->
      (spec \ {KeepExistingHighest, RejectPrepareTieDemotion}) \cup {PromoteHigherView}
    [] candidate = RecordInsertsNewEntry /\ Bug = "record_missing_insert" ->
      spec \ {InsertEntry}
    [] candidate = RecordDuplicateSenderStable /\
          Bug = "record_duplicate_increments" ->
      (spec \ {KeepDuplicateCount}) \cup {IncrementDistinctCount}
    [] candidate = RecordDistinctSenderIncrements /\
          Bug = "record_distinct_ignored" ->
      (spec \ {IncrementDistinctCount}) \cup {KeepDuplicateCount}
    [] candidate = RecordSeparatesKeys /\ Bug = "record_cross_key_dedup" ->
      spec \ {PreserveOtherKeyCount}
    [] candidate = CountFiltersRoster /\ Bug = "count_includes_non_roster" ->
      spec \ {IgnoreNonRosterSender}
    [] candidate = CountIncludesRosterSender /\ Bug = "count_drops_roster_sender" ->
      spec \ {CountRosterSender}
    [] candidate = CountAddsLocalInRosterWhenAbsent /\
          Bug = "count_local_not_counted" ->
      spec \ {CountLocalAbsentInRoster}
    [] candidate = CountDoesNotDoubleCountLocal /\
          Bug = "count_local_double_counted" ->
      spec \ {DoNotDoubleCountLocal}
    [] candidate = CountIgnoresLocalOutsideRoster /\
          Bug = "count_local_outside_roster_counted" ->
      spec \ {IgnoreLocalOutsideRoster}
    [] candidate = SelectEmptyRosterNone /\ Bug = "select_empty_roster_some" ->
      (spec \ {ReturnNone}) \cup {ReturnSelection}
    [] candidate = SelectRequiresQuorum /\ Bug = "select_below_quorum" ->
      (spec \ {RequireQuorum, ReturnNone}) \cup {ReturnSelection}
    [] candidate = SelectChoosesHighestEligibleKey /\
          Bug = "select_lowest_eligible" ->
      spec \ {SelectHighestKey}
    [] candidate = SelectForHeightFiltersExactHeight /\
          Bug = "select_ignores_height_filter" ->
      spec \ {FilterExactHeight}
    [] candidate = SelectAtOrAboveFiltersMinHeight /\
          Bug = "select_ignores_min_height" ->
      spec \ {FilterMinHeight}
    [] candidate = HighestQuorumViewChoosesHighestView /\
          Bug = "highest_view_returns_lowest" ->
      spec \ {ReturnHighestView}
    [] candidate = HighestQuorumViewFiltersHeight /\
          Bug = "highest_view_wrong_height" ->
      spec \ {FilterExactHeight}
    [] candidate = PruneCommittedDropsBelowAndEqual /\
          Bug = "prune_keeps_committed" ->
      spec \ {RemoveCommittedAndBelow}
    [] candidate = PruneCommittedKeepsAbove /\ Bug = "prune_drops_above" ->
      (spec \ {KeepAboveCommitted}) \cup {RemoveCommittedAndBelow}
    [] candidate = DropBelowHeightDropsBelow /\
          Bug = "drop_below_height_keeps_below" ->
      spec \ {RemoveBelowHeight}
    [] candidate = DropBelowHeightKeepsEqualAndAbove /\
          Bug = "drop_below_height_drops_equal" ->
      spec \ {KeepEqualHeight}
    [] candidate = RemoveDropsExactKey /\ Bug = "remove_keeps_exact" ->
      spec \ {RemoveExactKey}
    [] candidate = RemoveKeepsOtherKeys /\ Bug = "remove_drops_other" ->
      spec \ {KeepOtherKey}
    [] candidate = DropBelowViewDropsLowerSameHeight /\
          Bug = "drop_below_view_keeps_lower_same_height" ->
      spec \ {DropLowerViewSameHeight}
    [] candidate = DropBelowViewKeepsEqualAndHigherSameHeight /\
          Bug = "drop_below_view_drops_equal" ->
      spec \ {KeepEqualViewSameHeight}
    [] candidate = DropBelowViewKeepsOtherHeight /\
          Bug = "drop_below_view_drops_other_height" ->
      spec \ {KeepOtherHeight}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

Bugs == {
  "none",
  "highest_lower_not_ignored",
  "highest_higher_height_not_promoted",
  "highest_higher_view_not_promoted",
  "highest_commit_tie_not_promoted",
  "highest_prepare_demotes_commit",
  "record_missing_insert",
  "record_duplicate_increments",
  "record_distinct_ignored",
  "record_cross_key_dedup",
  "count_includes_non_roster",
  "count_drops_roster_sender",
  "count_local_not_counted",
  "count_local_double_counted",
  "count_local_outside_roster_counted",
  "select_empty_roster_some",
  "select_below_quorum",
  "select_lowest_eligible",
  "select_ignores_height_filter",
  "select_ignores_min_height",
  "highest_view_returns_lowest",
  "highest_view_wrong_height",
  "prune_keeps_committed",
  "prune_drops_above",
  "drop_below_height_drops_equal",
  "drop_below_height_keeps_below",
  "remove_keeps_exact",
  "remove_drops_other",
  "drop_below_view_drops_other_height",
  "drop_below_view_keeps_lower_same_height",
  "drop_below_view_drops_equal"
}

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked = 0
  /\ \A candidate \in Candidates:
       ImplementationActions(candidate) \subseteq Actions

NewViewTrackerMatchesSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

SelectionRequiresRosterQuorum ==
  \A candidate \in Candidates:
    ReturnSelection \in ImplementationActions(candidate) =>
      /\ RequireRosterNonEmpty \in ImplementationActions(candidate)
      /\ RequireQuorum \in ImplementationActions(candidate)

LocalVoteCountIsRosterBounded ==
  \A candidate \in Candidates:
    CountLocalAbsentInRoster \in ImplementationActions(candidate) =>
      /\ CountRosterSender \in ImplementationActions(candidate)
      /\ IgnoreLocalOutsideRoster \notin ImplementationActions(candidate)

HeightPrunesKeepUpperBoundary ==
  \A candidate \in Candidates:
    RemoveBelowHeight \in ImplementationActions(candidate) =>
      KeepEqualHeight \notin ImplementationActions(candidate)

Safety ==
  /\ NewViewTrackerMatchesSpec
  /\ SelectionRequiresRosterQuorum
  /\ LocalVoteCountIsRosterBounded
  /\ HeightPrunesKeepUpperBoundary

=============================================================================
