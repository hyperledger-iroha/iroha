---- MODULE SumeragiPrecommitSignerHistoryGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi precommit signer history recovery.

This slice captures `record_precommit_signers(...)`,
`precommit_signer_history()`, `precommit_signers_for_round(...)`,
`selection_from_precommit_signer_history_record(...)`, and the cached
precommit-signer branch of block-sync QC reconstruction. It abstracts concrete
peers, hashes, BLS signatures, and stake values into representative cases
while preserving the observable contracts: signer history is kept in newest
height/view order, exact lookups bind block/height/view/epoch, block-sync
history filters mode tag and expected epoch before choosing the highest view,
empty or malformed rosters fail closed, permissioned fallback requires commit
quorum but no stake snapshot, NPoS fallback requires an aligned stake snapshot
and strict stake quorum, returned selections are sourced from precommit signer
history without synthetic QC/checkpoint artifacts, and cached QC reconstruction
requires a non-empty aggregate signature.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

HistoryReplacesOlderView == "history_replaces_older_view"
HistoryKeepsDistinctBlock == "history_keeps_distinct_block"
HistoryNewestFirst == "history_newest_first"
RoundLookupExact == "round_lookup_exact"
ExactRecordFiltersIdentity == "exact_record_filters_identity"
ExactRecordPicksHighestView == "exact_record_picks_highest_view"
EmptyValidatorSetRejected == "empty_validator_set_rejected"
RosterLenMismatchRejected == "roster_len_mismatch_rejected"
InvalidSignerIndexRejected == "invalid_signer_index_rejected"
PermissionedBelowQuorumRejected == "permissioned_below_quorum_rejected"
PermissionedAtQuorumAccepted == "permissioned_at_quorum_accepted"
NposMissingSnapshotRejected == "npos_missing_snapshot_rejected"
NposMismatchedSnapshotRejected == "npos_mismatched_snapshot_rejected"
NposMissingStakeRejected == "npos_missing_stake_rejected"
NposZeroTotalRejected == "npos_zero_total_rejected"
NposBoundaryRejected == "npos_boundary_rejected"
NposStakeQuorumAccepted == "npos_stake_quorum_accepted"
SelectionClearsQcCheckpoint == "selection_clears_qc_checkpoint"
FailedQcDerivationFallback == "failed_qc_derivation_fallback"
CachedQcRequiresAggregate == "cached_qc_requires_aggregate"
CachedQcBuildsCommitQc == "cached_qc_builds_commit_qc"

Cases == {
  HistoryReplacesOlderView,
  HistoryKeepsDistinctBlock,
  HistoryNewestFirst,
  RoundLookupExact,
  ExactRecordFiltersIdentity,
  ExactRecordPicksHighestView,
  EmptyValidatorSetRejected,
  RosterLenMismatchRejected,
  InvalidSignerIndexRejected,
  PermissionedBelowQuorumRejected,
  PermissionedAtQuorumAccepted,
  NposMissingSnapshotRejected,
  NposMismatchedSnapshotRejected,
  NposMissingStakeRejected,
  NposZeroTotalRejected,
  NposBoundaryRejected,
  NposStakeQuorumAccepted,
  SelectionClearsQcCheckpoint,
  FailedQcDerivationFallback,
  CachedQcRequiresAggregate,
  CachedQcBuildsCommitQc
}

RejectFallback == 1
AcceptSelection == 2
PreserveNewerRecord == 3
PreserveDistinctRecord == 4
SortHeightViewDesc == 5
ExactRoundMatch == 6
FilterBlockHeight == 7
FilterOptionalView == 8
FilterModeTag == 9
FilterExpectedEpoch == 10
PickHighestView == 11
RejectEmptyRoster == 12
RejectRosterLenMismatch == 13
RejectInvalidSignerIndex == 14
CountQuorumCheck == 15
RejectBelowCountQuorum == 16
PermissionedNoStakeSnapshot == 17
RequireAlignedStakeSnapshot == 18
RejectMissingSnapshot == 19
RejectMismatchedSnapshot == 20
StakeQuorumCheck == 21
RejectMissingStake == 22
RejectZeroTotal == 23
StrictStakeGreaterThan == 24
RejectBelowStakeQuorum == 25
AcceptStakeQuorum == 26
SourcePrecommitSignerHistory == 27
NoCommitQcArtifact == 28
NoCheckpointArtifact == 29
ReturnExactRoster == 30
ReturnStakeSnapshot == 31
FailedQcFallsBack == 32
RequireAggregateSignature == 33
BuildCommitQc == 34

Actions == 1..34

SpecActions(c) ==
  CASE c = HistoryReplacesOlderView ->
      {PreserveNewerRecord}
    [] c = HistoryKeepsDistinctBlock ->
      {PreserveDistinctRecord}
    [] c = HistoryNewestFirst ->
      {SortHeightViewDesc}
    [] c = RoundLookupExact ->
      {ExactRoundMatch}
    [] c = ExactRecordFiltersIdentity ->
      {FilterBlockHeight, FilterOptionalView, FilterModeTag, FilterExpectedEpoch}
    [] c = ExactRecordPicksHighestView ->
      {PickHighestView}
    [] c = EmptyValidatorSetRejected ->
      {RejectFallback, RejectEmptyRoster}
    [] c = RosterLenMismatchRejected ->
      {RejectFallback, RejectRosterLenMismatch}
    [] c = InvalidSignerIndexRejected ->
      {RejectFallback, RejectInvalidSignerIndex}
    [] c = PermissionedBelowQuorumRejected ->
      {CountQuorumCheck, RejectBelowCountQuorum, RejectFallback}
    [] c = PermissionedAtQuorumAccepted ->
      {CountQuorumCheck, PermissionedNoStakeSnapshot, AcceptSelection,
       SourcePrecommitSignerHistory, ReturnExactRoster}
    [] c = NposMissingSnapshotRejected ->
      {RequireAlignedStakeSnapshot, RejectMissingSnapshot, RejectFallback}
    [] c = NposMismatchedSnapshotRejected ->
      {RequireAlignedStakeSnapshot, RejectMismatchedSnapshot, RejectFallback}
    [] c = NposMissingStakeRejected ->
      {RequireAlignedStakeSnapshot, StakeQuorumCheck, RejectMissingStake,
       RejectFallback}
    [] c = NposZeroTotalRejected ->
      {RequireAlignedStakeSnapshot, StakeQuorumCheck, RejectZeroTotal,
       RejectFallback}
    [] c = NposBoundaryRejected ->
      {RequireAlignedStakeSnapshot, StakeQuorumCheck, StrictStakeGreaterThan,
       RejectBelowStakeQuorum, RejectFallback}
    [] c = NposStakeQuorumAccepted ->
      {RequireAlignedStakeSnapshot, StakeQuorumCheck, StrictStakeGreaterThan,
       AcceptStakeQuorum, AcceptSelection, ReturnStakeSnapshot,
       SourcePrecommitSignerHistory, ReturnExactRoster}
    [] c = SelectionClearsQcCheckpoint ->
      {AcceptSelection, SourcePrecommitSignerHistory, NoCommitQcArtifact,
       NoCheckpointArtifact}
    [] c = FailedQcDerivationFallback ->
      {FailedQcFallsBack, AcceptSelection, SourcePrecommitSignerHistory}
    [] c = CachedQcRequiresAggregate ->
      {RequireAggregateSignature, RejectFallback}
    [] c = CachedQcBuildsCommitQc ->
      {RequireAggregateSignature, BuildCommitQc}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "history_drops_newer_view"
       /\ c = HistoryReplacesOlderView ->
      spec \ {PreserveNewerRecord}
    [] Bug = "history_drops_distinct_block"
       /\ c = HistoryKeepsDistinctBlock ->
      spec \ {PreserveDistinctRecord}
    [] Bug = "history_not_newest_first"
       /\ c = HistoryNewestFirst ->
      spec \ {SortHeightViewDesc}
    [] Bug = "round_lookup_ignores_epoch"
       /\ c = RoundLookupExact ->
      spec \ {ExactRoundMatch}
    [] Bug = "exact_record_ignores_mode"
       /\ c = ExactRecordFiltersIdentity ->
      spec \ {FilterModeTag}
    [] Bug = "exact_record_ignores_view"
       /\ c = ExactRecordFiltersIdentity ->
      spec \ {FilterOptionalView}
    [] Bug = "exact_record_uses_lowest_view"
       /\ c = ExactRecordPicksHighestView ->
      spec \ {PickHighestView}
    [] Bug = "empty_roster_accepted"
       /\ c = EmptyValidatorSetRejected ->
      (spec \ {RejectFallback, RejectEmptyRoster}) \cup {AcceptSelection}
    [] Bug = "roster_len_mismatch_accepted"
       /\ c = RosterLenMismatchRejected ->
      (spec \ {RejectFallback, RejectRosterLenMismatch}) \cup {AcceptSelection}
    [] Bug = "invalid_signer_index_accepted"
       /\ c = InvalidSignerIndexRejected ->
      (spec \ {RejectFallback, RejectInvalidSignerIndex}) \cup {AcceptSelection}
    [] Bug = "permissioned_below_quorum_accepted"
       /\ c = PermissionedBelowQuorumRejected ->
      (spec \ {RejectBelowCountQuorum, RejectFallback}) \cup {AcceptSelection}
    [] Bug = "permissioned_requires_snapshot"
       /\ c = PermissionedAtQuorumAccepted ->
      (spec \ {PermissionedNoStakeSnapshot, AcceptSelection})
        \cup {RequireAlignedStakeSnapshot, RejectMissingSnapshot, RejectFallback}
    [] Bug = "npos_missing_snapshot_accepted"
       /\ c = NposMissingSnapshotRejected ->
      (spec \ {RejectMissingSnapshot, RejectFallback}) \cup {AcceptSelection}
    [] Bug = "npos_mismatched_snapshot_accepted"
       /\ c = NposMismatchedSnapshotRejected ->
      (spec \ {RejectMismatchedSnapshot, RejectFallback}) \cup {AcceptSelection}
    [] Bug = "npos_missing_stake_accepted"
       /\ c = NposMissingStakeRejected ->
      (spec \ {RejectMissingStake, RejectFallback}) \cup {AcceptSelection}
    [] Bug = "npos_zero_total_false"
       /\ c = NposZeroTotalRejected ->
      (spec \ {RejectZeroTotal}) \cup {RejectBelowStakeQuorum}
    [] Bug = "npos_boundary_accepts"
       /\ c = NposBoundaryRejected ->
      (spec \ {RejectBelowStakeQuorum, RejectFallback}) \cup {AcceptSelection}
    [] Bug = "npos_valid_rejected"
       /\ c = NposStakeQuorumAccepted ->
      (spec \ {AcceptStakeQuorum, AcceptSelection}) \cup {RejectFallback}
    [] Bug = "selection_keeps_qc_artifact"
       /\ c = SelectionClearsQcCheckpoint ->
      spec \ {NoCommitQcArtifact}
    [] Bug = "selection_drops_stake_snapshot"
       /\ c = NposStakeQuorumAccepted ->
      spec \ {ReturnStakeSnapshot}
    [] Bug = "failed_qc_no_fallback"
       /\ c = FailedQcDerivationFallback ->
      (spec \ {FailedQcFallsBack, AcceptSelection}) \cup {RejectFallback}
    [] Bug = "cached_qc_allows_empty_aggregate"
       /\ c = CachedQcRequiresAggregate ->
      (spec \ {RequireAggregateSignature, RejectFallback}) \cup {BuildCommitQc}
    [] Bug = "cached_qc_rejects_valid"
       /\ c = CachedQcBuildsCommitQc ->
      (spec \ {BuildCommitQc}) \cup {RejectFallback}
    [] OTHER -> spec

Bugs == {
  "none",
  "history_drops_newer_view",
  "history_drops_distinct_block",
  "history_not_newest_first",
  "round_lookup_ignores_epoch",
  "exact_record_ignores_mode",
  "exact_record_ignores_view",
  "exact_record_uses_lowest_view",
  "empty_roster_accepted",
  "roster_len_mismatch_accepted",
  "invalid_signer_index_accepted",
  "permissioned_below_quorum_accepted",
  "permissioned_requires_snapshot",
  "npos_missing_snapshot_accepted",
  "npos_mismatched_snapshot_accepted",
  "npos_missing_stake_accepted",
  "npos_zero_total_false",
  "npos_boundary_accepts",
  "npos_valid_rejected",
  "selection_keeps_qc_artifact",
  "selection_drops_stake_snapshot",
  "failed_qc_no_fallback",
  "cached_qc_allows_empty_aggregate",
  "cached_qc_rejects_valid"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1

PrecommitSignerHistoryCoreSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

NoBugInvariant == PrecommitSignerHistoryCoreSafety

SafetyFast == PrecommitSignerHistoryCoreSafety

BugHistoryDropsNewerView ==
  {} = {PreserveNewerRecord}

BugHistoryDropsDistinctBlock ==
  {} = {PreserveDistinctRecord}

BugHistoryNotNewestFirst ==
  {} = {SortHeightViewDesc}

BugRoundLookupIgnoresEpoch ==
  {} = {ExactRoundMatch}

BugExactRecordIgnoresMode ==
  {FilterBlockHeight, FilterOptionalView, FilterExpectedEpoch}
    = {FilterBlockHeight, FilterOptionalView, FilterModeTag, FilterExpectedEpoch}

BugExactRecordIgnoresView ==
  {FilterBlockHeight, FilterModeTag, FilterExpectedEpoch}
    = {FilterBlockHeight, FilterOptionalView, FilterModeTag, FilterExpectedEpoch}

BugExactRecordUsesLowestView ==
  {} = {PickHighestView}

BugEmptyRosterAccepted ==
  {AcceptSelection} = {RejectFallback, RejectEmptyRoster}

BugRosterLenMismatchAccepted ==
  {AcceptSelection} = {RejectFallback, RejectRosterLenMismatch}

BugInvalidSignerIndexAccepted ==
  {AcceptSelection} = {RejectFallback, RejectInvalidSignerIndex}

BugPermissionedBelowQuorumAccepted ==
  {CountQuorumCheck, AcceptSelection}
    = {CountQuorumCheck, RejectBelowCountQuorum, RejectFallback}

BugPermissionedRequiresSnapshot ==
  {CountQuorumCheck, SourcePrecommitSignerHistory, ReturnExactRoster,
   RequireAlignedStakeSnapshot, RejectMissingSnapshot, RejectFallback}
    = {CountQuorumCheck, PermissionedNoStakeSnapshot, AcceptSelection,
       SourcePrecommitSignerHistory, ReturnExactRoster}

BugNposMissingSnapshotAccepted ==
  {RequireAlignedStakeSnapshot, AcceptSelection}
    = {RequireAlignedStakeSnapshot, RejectMissingSnapshot, RejectFallback}

BugNposMismatchedSnapshotAccepted ==
  {RequireAlignedStakeSnapshot, AcceptSelection}
    = {RequireAlignedStakeSnapshot, RejectMismatchedSnapshot, RejectFallback}

BugNposMissingStakeAccepted ==
  {RequireAlignedStakeSnapshot, StakeQuorumCheck, AcceptSelection}
    = {RequireAlignedStakeSnapshot, StakeQuorumCheck, RejectMissingStake,
       RejectFallback}

BugNposZeroTotalFalse ==
  {RequireAlignedStakeSnapshot, StakeQuorumCheck, RejectBelowStakeQuorum,
   RejectFallback}
    = {RequireAlignedStakeSnapshot, StakeQuorumCheck, RejectZeroTotal,
       RejectFallback}

BugNposBoundaryAccepts ==
  {RequireAlignedStakeSnapshot, StakeQuorumCheck, StrictStakeGreaterThan,
   AcceptSelection}
    = {RequireAlignedStakeSnapshot, StakeQuorumCheck, StrictStakeGreaterThan,
       RejectBelowStakeQuorum, RejectFallback}

BugNposValidRejected ==
  {RequireAlignedStakeSnapshot, StakeQuorumCheck, StrictStakeGreaterThan,
   RejectFallback, ReturnStakeSnapshot, SourcePrecommitSignerHistory,
   ReturnExactRoster}
    = {RequireAlignedStakeSnapshot, StakeQuorumCheck, StrictStakeGreaterThan,
       AcceptStakeQuorum, AcceptSelection, ReturnStakeSnapshot,
       SourcePrecommitSignerHistory, ReturnExactRoster}

BugSelectionKeepsQcArtifact ==
  {AcceptSelection, SourcePrecommitSignerHistory, NoCheckpointArtifact}
    = {AcceptSelection, SourcePrecommitSignerHistory, NoCommitQcArtifact,
       NoCheckpointArtifact}

BugSelectionDropsStakeSnapshot ==
  {RequireAlignedStakeSnapshot, StakeQuorumCheck, StrictStakeGreaterThan,
   AcceptStakeQuorum, AcceptSelection, SourcePrecommitSignerHistory,
   ReturnExactRoster}
    = {RequireAlignedStakeSnapshot, StakeQuorumCheck, StrictStakeGreaterThan,
       AcceptStakeQuorum, AcceptSelection, ReturnStakeSnapshot,
       SourcePrecommitSignerHistory, ReturnExactRoster}

BugFailedQcNoFallback ==
  {SourcePrecommitSignerHistory, RejectFallback}
    = {FailedQcFallsBack, AcceptSelection, SourcePrecommitSignerHistory}

BugCachedQcAllowsEmptyAggregate ==
  {BuildCommitQc} = {RequireAggregateSignature, RejectFallback}

BugCachedQcRejectsValid ==
  {RequireAggregateSignature, RejectFallback}
    = {RequireAggregateSignature, BuildCommitQc}

====
