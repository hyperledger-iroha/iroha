---- MODULE SumeragiBlockSyncRosterStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi block-sync roster status accounting.

This slice captures `inc_block_sync_roster_source(...)`,
`inc_block_sync_roster_drop_missing()`,
`inc_block_sync_roster_drop_unsolicited_share_blocks()`,
`snapshot().block_sync_roster`, and the roster subset of the test-only
`reset_block_sync_counters_for_tests()` helper from `status.rs`.
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
PairHintSource == 2
CommitQcHintSource == 3
CheckpointHintSource == 4
CommitQcHistorySource == 5
CheckpointHistorySource == 6
RosterSidecarSource == 7
CommitRosterJournalSource == 8
UnknownSourceNoop == 9
DropMissingRecord == 10
DropUnsolicitedRecord == 11
RepeatedCommitHintAccumulates == 12
RepeatedDropMissingAccumulates == 13
SnapshotProjectsCommitHint == 14
SnapshotProjectsCheckpointHint == 15
SnapshotProjectsHistory == 16
SnapshotProjectsSidecar == 17
SnapshotProjectsJournal == 18
SnapshotProjectsDrops == 19
TopLevelSnapshotIncludesRoster == 20
ResetAfterRecordsClears == 21

Candidates == 1..21

ResetCounters == 1
IncrementCommitQcHint == 2
IncrementCheckpointHint == 3
IncrementCommitQcHistory == 4
IncrementCheckpointHistory == 5
IncrementRosterSidecar == 6
IncrementCommitRosterJournal == 7
UnknownSourceNoChange == 8
IncrementDropMissing == 9
IncrementDropUnsolicited == 10
SameCounterAccumulates == 11
SnapshotCommitHintMatches == 12
SnapshotCheckpointHintMatches == 13
SnapshotHistoryMatches == 14
SnapshotSidecarMatches == 15
SnapshotJournalMatches == 16
SnapshotDropMatches == 17
TopLevelRosterMatches == 18

Actions == 1..18

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      {ResetCounters}
    [] candidate = PairHintSource ->
      {IncrementCommitQcHint, IncrementCheckpointHint}
    [] candidate = CommitQcHintSource ->
      {IncrementCommitQcHint}
    [] candidate = CheckpointHintSource ->
      {IncrementCheckpointHint}
    [] candidate = CommitQcHistorySource ->
      {IncrementCommitQcHistory}
    [] candidate = CheckpointHistorySource ->
      {IncrementCheckpointHistory}
    [] candidate = RosterSidecarSource ->
      {IncrementRosterSidecar}
    [] candidate = CommitRosterJournalSource ->
      {IncrementCommitRosterJournal}
    [] candidate = UnknownSourceNoop ->
      {UnknownSourceNoChange}
    [] candidate = DropMissingRecord ->
      {IncrementDropMissing}
    [] candidate = DropUnsolicitedRecord ->
      {IncrementDropUnsolicited}
    [] candidate = RepeatedCommitHintAccumulates ->
      {IncrementCommitQcHint, SameCounterAccumulates,
       SnapshotCommitHintMatches}
    [] candidate = RepeatedDropMissingAccumulates ->
      {IncrementDropMissing, SameCounterAccumulates, SnapshotDropMatches}
    [] candidate = SnapshotProjectsCommitHint ->
      {SnapshotCommitHintMatches}
    [] candidate = SnapshotProjectsCheckpointHint ->
      {SnapshotCheckpointHintMatches}
    [] candidate = SnapshotProjectsHistory ->
      {SnapshotHistoryMatches}
    [] candidate = SnapshotProjectsSidecar ->
      {SnapshotSidecarMatches}
    [] candidate = SnapshotProjectsJournal ->
      {SnapshotJournalMatches}
    [] candidate = SnapshotProjectsDrops ->
      {SnapshotDropMatches}
    [] candidate = TopLevelSnapshotIncludesRoster ->
      {TopLevelRosterMatches}
    [] candidate = ResetAfterRecordsClears ->
      {ResetCounters}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = PairHintSource /\ Bug = "pair_hint_missing_commit" ->
      spec \ {IncrementCommitQcHint}
    [] candidate = PairHintSource /\
          Bug = "pair_hint_missing_checkpoint" ->
      spec \ {IncrementCheckpointHint}
    [] candidate = CommitQcHintSource /\
          Bug = "commit_qc_hint_not_counted" ->
      spec \ {IncrementCommitQcHint}
    [] candidate = CommitQcHintSource /\
          Bug = "commit_qc_hint_counts_checkpoint" ->
      spec \cup {IncrementCheckpointHint}
    [] candidate = CheckpointHintSource /\
          Bug = "checkpoint_hint_not_counted" ->
      spec \ {IncrementCheckpointHint}
    [] candidate = CheckpointHintSource /\
          Bug = "checkpoint_hint_counts_commit" ->
      spec \cup {IncrementCommitQcHint}
    [] candidate = CommitQcHistorySource /\
          Bug = "commit_qc_history_not_counted" ->
      spec \ {IncrementCommitQcHistory}
    [] candidate = CheckpointHistorySource /\
          Bug = "checkpoint_history_not_counted" ->
      spec \ {IncrementCheckpointHistory}
    [] candidate = RosterSidecarSource /\
          Bug = "roster_sidecar_not_counted" ->
      spec \ {IncrementRosterSidecar}
    [] candidate = CommitRosterJournalSource /\
          Bug = "commit_roster_journal_not_counted" ->
      spec \ {IncrementCommitRosterJournal}
    [] candidate = UnknownSourceNoop /\
          Bug = "unknown_source_increments_commit" ->
      spec \cup {IncrementCommitQcHint}
    [] candidate = DropMissingRecord /\ Bug = "drop_missing_not_counted" ->
      spec \ {IncrementDropMissing}
    [] candidate = DropUnsolicitedRecord /\
          Bug = "drop_unsolicited_not_counted" ->
      spec \ {IncrementDropUnsolicited}
    [] candidate = RepeatedCommitHintAccumulates /\
          Bug = "repeated_commit_hint_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotCommitHintMatches}
    [] candidate = RepeatedDropMissingAccumulates /\
          Bug = "repeated_drop_missing_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotDropMatches}
    [] candidate = SnapshotProjectsCommitHint /\
          Bug = "snapshot_commit_hint_mismatch" ->
      spec \ {SnapshotCommitHintMatches}
    [] candidate = SnapshotProjectsCheckpointHint /\
          Bug = "snapshot_checkpoint_hint_mismatch" ->
      spec \ {SnapshotCheckpointHintMatches}
    [] candidate = SnapshotProjectsHistory /\
          Bug = "snapshot_history_mismatch" ->
      spec \ {SnapshotHistoryMatches}
    [] candidate = SnapshotProjectsSidecar /\
          Bug = "snapshot_sidecar_mismatch" ->
      spec \ {SnapshotSidecarMatches}
    [] candidate = SnapshotProjectsJournal /\
          Bug = "snapshot_journal_mismatch" ->
      spec \ {SnapshotJournalMatches}
    [] candidate = SnapshotProjectsDrops /\
          Bug = "snapshot_drop_mismatch" ->
      spec \ {SnapshotDropMatches}
    [] candidate = TopLevelSnapshotIncludesRoster /\
          Bug = "top_level_snapshot_drops_roster" ->
      spec \ {TopLevelRosterMatches}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counters" ->
      spec \ {ResetCounters}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 21
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..21

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugResetEmptyKeepsCounters ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugPairHintMissingCommit ==
  ImplementationActions(PairHintSource) = SpecActions(PairHintSource)

BugPairHintMissingCheckpoint ==
  ImplementationActions(PairHintSource) = SpecActions(PairHintSource)

BugCommitQcHintNotCounted ==
  ImplementationActions(CommitQcHintSource) =
    SpecActions(CommitQcHintSource)

BugCommitQcHintCountsCheckpoint ==
  ImplementationActions(CommitQcHintSource) =
    SpecActions(CommitQcHintSource)

BugCheckpointHintNotCounted ==
  ImplementationActions(CheckpointHintSource) =
    SpecActions(CheckpointHintSource)

BugCheckpointHintCountsCommit ==
  ImplementationActions(CheckpointHintSource) =
    SpecActions(CheckpointHintSource)

BugCommitQcHistoryNotCounted ==
  ImplementationActions(CommitQcHistorySource) =
    SpecActions(CommitQcHistorySource)

BugCheckpointHistoryNotCounted ==
  ImplementationActions(CheckpointHistorySource) =
    SpecActions(CheckpointHistorySource)

BugRosterSidecarNotCounted ==
  ImplementationActions(RosterSidecarSource) =
    SpecActions(RosterSidecarSource)

BugCommitRosterJournalNotCounted ==
  ImplementationActions(CommitRosterJournalSource) =
    SpecActions(CommitRosterJournalSource)

BugUnknownSourceIncrementsCommit ==
  ImplementationActions(UnknownSourceNoop) = SpecActions(UnknownSourceNoop)

BugDropMissingNotCounted ==
  ImplementationActions(DropMissingRecord) = SpecActions(DropMissingRecord)

BugDropUnsolicitedNotCounted ==
  ImplementationActions(DropUnsolicitedRecord) =
    SpecActions(DropUnsolicitedRecord)

BugRepeatedCommitHintOverwritesCount ==
  ImplementationActions(RepeatedCommitHintAccumulates) =
    SpecActions(RepeatedCommitHintAccumulates)

BugRepeatedDropMissingOverwritesCount ==
  ImplementationActions(RepeatedDropMissingAccumulates) =
    SpecActions(RepeatedDropMissingAccumulates)

BugSnapshotCommitHintMismatch ==
  ImplementationActions(SnapshotProjectsCommitHint) =
    SpecActions(SnapshotProjectsCommitHint)

BugSnapshotCheckpointHintMismatch ==
  ImplementationActions(SnapshotProjectsCheckpointHint) =
    SpecActions(SnapshotProjectsCheckpointHint)

BugSnapshotHistoryMismatch ==
  ImplementationActions(SnapshotProjectsHistory) =
    SpecActions(SnapshotProjectsHistory)

BugSnapshotSidecarMismatch ==
  ImplementationActions(SnapshotProjectsSidecar) =
    SpecActions(SnapshotProjectsSidecar)

BugSnapshotJournalMismatch ==
  ImplementationActions(SnapshotProjectsJournal) =
    SpecActions(SnapshotProjectsJournal)

BugSnapshotDropMismatch ==
  ImplementationActions(SnapshotProjectsDrops) =
    SpecActions(SnapshotProjectsDrops)

BugTopLevelSnapshotDropsRoster ==
  ImplementationActions(TopLevelSnapshotIncludesRoster) =
    SpecActions(TopLevelSnapshotIncludesRoster)

BugResetAfterRecordsKeepsCounters ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
