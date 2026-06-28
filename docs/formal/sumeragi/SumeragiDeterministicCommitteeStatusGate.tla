---- MODULE SumeragiDeterministicCommitteeStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi deterministic-committee status state.

This slice captures `set_consensus_deterministic_committee_size(...)`, its
`snapshot()` projection, and the relevant subset of the test-only
`reset_missing_block_fetch_counters_for_tests()` helper from `status.rs`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetEmpty == 1
CommitteeSizeRecord == 2
ZeroCommitteeSizeRecord == 3
OverwriteCommitteeSize == 4
LowerCommitteeSizeOverwrites == 5
SnapshotProjectsCommitteeSize == 6
ResetAfterRecordClears == 7

Candidates == 1..7

ResetCommitteeSize == 1
StoreCommitteeSize == 2
StoreZeroCommitteeSize == 3
IncrementCommitteeSize == 4
StoreReplacesPrevious == 5
StoreAcceptsLower == 6
SnapshotCommitteeSizeMatches == 7

Actions == 1..7

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      {ResetCommitteeSize}
    [] candidate = CommitteeSizeRecord ->
      {StoreCommitteeSize}
    [] candidate = ZeroCommitteeSizeRecord ->
      {StoreZeroCommitteeSize, SnapshotCommitteeSizeMatches}
    [] candidate = OverwriteCommitteeSize ->
      {StoreCommitteeSize, StoreReplacesPrevious,
       SnapshotCommitteeSizeMatches}
    [] candidate = LowerCommitteeSizeOverwrites ->
      {StoreCommitteeSize, StoreAcceptsLower, StoreReplacesPrevious,
       SnapshotCommitteeSizeMatches}
    [] candidate = SnapshotProjectsCommitteeSize ->
      {SnapshotCommitteeSizeMatches}
    [] candidate = ResetAfterRecordClears ->
      {ResetCommitteeSize}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_size" ->
      spec \ {ResetCommitteeSize}
    [] candidate = CommitteeSizeRecord /\
          Bug = "committee_size_not_set" ->
      spec \ {StoreCommitteeSize}
    [] candidate = ZeroCommitteeSizeRecord /\
          Bug = "zero_size_ignored" ->
      spec \ {StoreZeroCommitteeSize, SnapshotCommitteeSizeMatches}
    [] candidate = CommitteeSizeRecord /\
          Bug = "committee_size_counts_as_counter" ->
      (spec \ {StoreCommitteeSize}) \cup {IncrementCommitteeSize}
    [] candidate = OverwriteCommitteeSize /\
          Bug = "committee_size_not_overwritten" ->
      spec \ {StoreReplacesPrevious, SnapshotCommitteeSizeMatches}
    [] candidate = LowerCommitteeSizeOverwrites /\
          Bug = "lower_size_not_overwritten" ->
      spec \ {StoreAcceptsLower, StoreReplacesPrevious,
              SnapshotCommitteeSizeMatches}
    [] candidate = SnapshotProjectsCommitteeSize /\
          Bug = "snapshot_committee_size_mismatch" ->
      spec \ {SnapshotCommitteeSizeMatches}
    [] candidate = ResetAfterRecordClears /\
          Bug = "reset_after_record_keeps_size" ->
      spec \ {ResetCommitteeSize}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 7
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..7

DeterministicCommitteeStatusActionsMatchSpec ==
  /\ ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)
  /\ ImplementationActions(CommitteeSizeRecord) =
       SpecActions(CommitteeSizeRecord)
  /\ ImplementationActions(ZeroCommitteeSizeRecord) =
       SpecActions(ZeroCommitteeSizeRecord)
  /\ ImplementationActions(OverwriteCommitteeSize) =
       SpecActions(OverwriteCommitteeSize)
  /\ ImplementationActions(LowerCommitteeSizeOverwrites) =
       SpecActions(LowerCommitteeSizeOverwrites)
  /\ ImplementationActions(SnapshotProjectsCommitteeSize) =
       SpecActions(SnapshotProjectsCommitteeSize)
  /\ ImplementationActions(ResetAfterRecordClears) =
       SpecActions(ResetAfterRecordClears)

DeterministicCommitteeStatusExactness ==
  /\ DeterministicCommitteeStatusActionsMatchSpec

Safety ==
  DeterministicCommitteeStatusExactness

DeterministicCommitteeStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ DeterministicCommitteeStatusExactness

BugResetEmptyKeepsSize ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugCommitteeSizeNotSet ==
  ImplementationActions(CommitteeSizeRecord) =
    SpecActions(CommitteeSizeRecord)

BugZeroSizeIgnored ==
  ImplementationActions(ZeroCommitteeSizeRecord) =
    SpecActions(ZeroCommitteeSizeRecord)

BugCommitteeSizeCountsAsCounter ==
  ImplementationActions(CommitteeSizeRecord) =
    SpecActions(CommitteeSizeRecord)

BugCommitteeSizeNotOverwritten ==
  ImplementationActions(OverwriteCommitteeSize) =
    SpecActions(OverwriteCommitteeSize)

BugLowerSizeNotOverwritten ==
  ImplementationActions(LowerCommitteeSizeOverwrites) =
    SpecActions(LowerCommitteeSizeOverwrites)

BugSnapshotCommitteeSizeMismatch ==
  ImplementationActions(SnapshotProjectsCommitteeSize) =
    SpecActions(SnapshotProjectsCommitteeSize)

BugResetAfterRecordKeepsSize ==
  ImplementationActions(ResetAfterRecordClears) =
    SpecActions(ResetAfterRecordClears)

=============================================================================
====
