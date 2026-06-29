---- MODULE SumeragiBlockSyncKnownSelectedRosterGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the selected-roster known-block branch in
`handle_block_sync_update(...)`.

After a block-sync update yields a verified roster selection, the live path
records the selected source, caches the vote roster, records any validator
checkpoint, and prepares a commit-roster journal record when the selection
carries a commit QC.  That commit-roster record is persisted only once the block
is already known locally.  Known blocks then process commit votes, replay a
candidate commit QC unless it is already present in both local caches, clear any
satisfied missing-QC request, clear the missing-block request with
`PayloadAvailable`, and return `Ok(())`.

Unknown blocks continue into the normal signed-block validation path after the
same selected-roster bookkeeping; they do not persist the prepared commit-roster
record and they do not take the known-block clear/return shortcut.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "unknown_selected",
  "known_no_qc",
  "known_checkpoint_only",
  "known_selection_qc",
  "known_incoming_qc",
  "known_incoming_preempts_selection",
  "known_selection_preempts_checkpoint",
  "known_checkpoint_conversion_fails",
  "known_redundant_qc",
  "known_cached_only_replays",
  "known_snapshot_only_replays",
  "known_prepare_none",
  "known_cached_commit_qc",
  "known_synth_checkpoint_record",
  "known_checkpoint_record",
  "known_stake_record"
}

BlockKnown(c) ==
  c # "unknown_selected"

IncomingQc(c) ==
  c \in {"known_incoming_qc", "known_incoming_preempts_selection"}

SelectionCommitQc(c) ==
  c \in {
    "unknown_selected",
    "known_selection_qc",
    "known_incoming_preempts_selection",
    "known_selection_preempts_checkpoint",
    "known_redundant_qc",
    "known_cached_only_replays",
    "known_snapshot_only_replays",
    "known_prepare_none",
    "known_synth_checkpoint_record",
    "known_checkpoint_record",
    "known_stake_record"
  }

SelectionCheckpoint(c) ==
  c \in {
    "unknown_selected",
    "known_checkpoint_only",
    "known_selection_preempts_checkpoint",
    "known_checkpoint_conversion_fails",
    "known_checkpoint_record"
  }

SelectionStake(c) ==
  c \in {"unknown_selected", "known_stake_record"}

CheckpointConverts(c) ==
  SelectionCheckpoint(c) /\ c # "known_checkpoint_conversion_fails"

CachedQcMatch(c) ==
  c \in {"known_redundant_qc", "known_cached_only_replays"}

LocalSnapshotQcMatch(c) ==
  c \in {"known_redundant_qc", "known_snapshot_only_replays"}

PrepareWorkSome(c) ==
  c # "known_prepare_none"

CachedCommitQcAvailable(c) ==
  CachedQcMatch(c)
    \/ LocalSnapshotQcMatch(c)
    \/ c = "known_cached_commit_qc"

SpecSourceMetricRecorded(c) ==
  TRUE

SpecVoteRosterCached(c) ==
  TRUE

SpecCheckpointRecorded(c) ==
  SelectionCheckpoint(c)

SpecCommitRosterRecordPrepared(c) ==
  SelectionCommitQc(c)

SpecCommitRosterPersisted(c) ==
  BlockKnown(c) /\ SelectionCommitQc(c)

SpecCommitRosterCheckpointKind(c) ==
  IF ~SpecCommitRosterPersisted(c) THEN "none"
  ELSE IF SelectionCheckpoint(c) THEN "selection_checkpoint"
  ELSE "synth_from_commit_qc"

SpecCommitRosterStakeIncluded(c) ==
  SpecCommitRosterPersisted(c) /\ SelectionStake(c)

SpecProcessVotes(c) ==
  BlockKnown(c)

SpecQcSource(c) ==
  IF ~BlockKnown(c) THEN "later"
  ELSE IF IncomingQc(c) THEN "incoming"
  ELSE IF SelectionCommitQc(c) THEN "selection"
  ELSE IF CheckpointConverts(c) THEN "checkpoint"
  ELSE "none"

SpecHasKnownQcCandidate(c) ==
  SpecQcSource(c) \in {"incoming", "selection", "checkpoint"}

SpecRedundantReplay(c) ==
  /\ BlockKnown(c)
  /\ SpecHasKnownQcCandidate(c)
  /\ CachedQcMatch(c)
  /\ LocalSnapshotQcMatch(c)

SpecPrepareCalled(c) ==
  /\ BlockKnown(c)
  /\ SpecHasKnownQcCandidate(c)
  /\ ~SpecRedundantReplay(c)

SpecPrepareCommitQcMatchArg(c) ==
  SpecPrepareCalled(c) /\ SpecQcSource(c) = "selection"

SpecEnqueueWork(c) ==
  SpecPrepareCalled(c) /\ PrepareWorkSome(c)

SpecClearMissingCommitQc(c) ==
  BlockKnown(c) /\ CachedCommitQcAvailable(c)

SpecClearMissingBlock(c) ==
  BlockKnown(c)

SpecClearMissingBlockReason(c) ==
  IF SpecClearMissingBlock(c) THEN "PayloadAvailable" ELSE "none"

SpecReturnKind(c) ==
  IF BlockKnown(c) THEN "Ok" ELSE "continue"

SpecContinues(c) ==
  ~BlockKnown(c)

ActualSourceMetricRecorded(c) ==
  IF Bug = "source_metric_skipped"
     /\ c = "known_selection_qc"
  THEN FALSE
  ELSE TRUE

ActualVoteRosterCached(c) ==
  IF Bug = "vote_roster_not_cached"
     /\ c = "known_selection_qc"
  THEN FALSE
  ELSE TRUE

ActualCheckpointRecorded(c) ==
  IF Bug = "checkpoint_not_recorded"
     /\ c = "known_checkpoint_only"
  THEN FALSE
  ELSE IF Bug = "absent_checkpoint_recorded"
          /\ c = "known_selection_qc" THEN TRUE
  ELSE SelectionCheckpoint(c)

ActualCommitRosterRecordPrepared(c) ==
  IF Bug = "commit_roster_record_not_prepared"
     /\ c = "known_selection_qc"
  THEN FALSE
  ELSE SelectionCommitQc(c)

ActualCommitRosterPersisted(c) ==
  IF Bug = "commit_roster_not_persisted"
     /\ c = "known_selection_qc"
  THEN FALSE
  ELSE IF Bug = "commit_roster_persisted_for_unknown"
          /\ c = "unknown_selected" THEN TRUE
  ELSE BlockKnown(c) /\ SelectionCommitQc(c)

ActualCommitRosterCheckpointKind(c) ==
  IF ~ActualCommitRosterPersisted(c) THEN "none"
  ELSE IF Bug = "commit_roster_uses_synth_over_selection_checkpoint"
          /\ c = "known_checkpoint_record" THEN "synth_from_commit_qc"
  ELSE IF SelectionCheckpoint(c) THEN "selection_checkpoint"
  ELSE "synth_from_commit_qc"

ActualCommitRosterStakeIncluded(c) ==
  IF Bug = "commit_roster_drops_stake"
     /\ c = "known_stake_record"
  THEN FALSE
  ELSE ActualCommitRosterPersisted(c) /\ SelectionStake(c)

ActualProcessVotes(c) ==
  IF Bug = "known_votes_not_processed"
     /\ c = "known_selection_qc"
  THEN FALSE
  ELSE BlockKnown(c)

ActualQcSource(c) ==
  IF ~BlockKnown(c) THEN "later"
  ELSE IF Bug = "incoming_qc_not_preferred"
          /\ c = "known_incoming_preempts_selection" THEN "selection"
  ELSE IF IncomingQc(c) THEN "incoming"
  ELSE IF Bug = "selection_qc_not_preferred"
          /\ c = "known_selection_preempts_checkpoint" THEN "checkpoint"
  ELSE IF SelectionCommitQc(c) THEN "selection"
  ELSE IF Bug = "checkpoint_qc_not_used"
          /\ c = "known_checkpoint_only" THEN "none"
  ELSE IF Bug = "invalid_checkpoint_used"
          /\ c = "known_checkpoint_conversion_fails" THEN "checkpoint"
  ELSE IF CheckpointConverts(c) THEN "checkpoint"
  ELSE "none"

ActualHasKnownQcCandidate(c) ==
  ActualQcSource(c) \in {"incoming", "selection", "checkpoint"}

ActualRedundantReplay(c) ==
  IF Bug = "redundant_qc_replayed"
     /\ c = "known_redundant_qc"
  THEN FALSE
  ELSE BlockKnown(c)
    /\ ActualHasKnownQcCandidate(c)
    /\ CachedQcMatch(c)
    /\ LocalSnapshotQcMatch(c)

ActualPrepareCalled(c) ==
  IF Bug = "cached_only_skips_replay"
     /\ c = "known_cached_only_replays"
  THEN FALSE
  ELSE IF Bug = "snapshot_only_skips_replay"
          /\ c = "known_snapshot_only_replays" THEN FALSE
  ELSE IF Bug = "prepare_not_called_for_selection"
          /\ c = "known_selection_qc" THEN FALSE
  ELSE BlockKnown(c)
    /\ ActualHasKnownQcCandidate(c)
    /\ ~ActualRedundantReplay(c)

ActualPrepareCommitQcMatchArg(c) ==
  IF Bug = "commit_qc_match_arg_wrong_for_incoming"
     /\ c = "known_incoming_preempts_selection"
  THEN TRUE
  ELSE IF Bug = "commit_qc_match_arg_wrong_for_selection"
          /\ c = "known_selection_qc" THEN FALSE
  ELSE ActualPrepareCalled(c) /\ ActualQcSource(c) = "selection"

ActualEnqueueWork(c) ==
  IF Bug = "prepare_none_enqueued"
     /\ c = "known_prepare_none"
  THEN TRUE
  ELSE ActualPrepareCalled(c) /\ PrepareWorkSome(c)

ActualClearMissingCommitQc(c) ==
  IF Bug = "cached_commit_qc_no_clear"
     /\ c = "known_cached_commit_qc"
  THEN FALSE
  ELSE IF Bug = "no_cached_commit_qc_cleared"
          /\ c = "known_no_qc" THEN TRUE
  ELSE BlockKnown(c) /\ CachedCommitQcAvailable(c)

ActualClearMissingBlock(c) ==
  IF Bug = "missing_block_not_cleared"
     /\ c = "known_selection_qc"
  THEN FALSE
  ELSE IF Bug = "unknown_path_clears_missing"
          /\ c = "unknown_selected" THEN TRUE
  ELSE BlockKnown(c)

ActualClearMissingBlockReason(c) ==
  IF ~ActualClearMissingBlock(c) THEN "none"
  ELSE IF Bug = "missing_block_wrong_reason"
          /\ c = "known_selection_qc" THEN "Obsolete"
  ELSE "PayloadAvailable"

ActualReturnKind(c) ==
  IF Bug = "known_path_returns_error"
     /\ c = "known_selection_qc"
  THEN "Err"
  ELSE IF Bug = "unknown_path_returns_ok"
          /\ c = "unknown_selected" THEN "Ok"
  ELSE IF BlockKnown(c) THEN "Ok"
  ELSE "continue"

ActualContinues(c) ==
  IF Bug = "known_path_continues"
     /\ c = "known_selection_qc"
  THEN TRUE
  ELSE ~BlockKnown(c)

Matches(c) ==
  /\ ActualSourceMetricRecorded(c) = SpecSourceMetricRecorded(c)
  /\ ActualVoteRosterCached(c) = SpecVoteRosterCached(c)
  /\ ActualCheckpointRecorded(c) = SpecCheckpointRecorded(c)
  /\ ActualCommitRosterRecordPrepared(c) = SpecCommitRosterRecordPrepared(c)
  /\ ActualCommitRosterPersisted(c) = SpecCommitRosterPersisted(c)
  /\ ActualCommitRosterCheckpointKind(c) = SpecCommitRosterCheckpointKind(c)
  /\ ActualCommitRosterStakeIncluded(c) = SpecCommitRosterStakeIncluded(c)
  /\ ActualProcessVotes(c) = SpecProcessVotes(c)
  /\ ActualQcSource(c) = SpecQcSource(c)
  /\ ActualRedundantReplay(c) = SpecRedundantReplay(c)
  /\ ActualPrepareCalled(c) = SpecPrepareCalled(c)
  /\ ActualPrepareCommitQcMatchArg(c) = SpecPrepareCommitQcMatchArg(c)
  /\ ActualEnqueueWork(c) = SpecEnqueueWork(c)
  /\ ActualClearMissingCommitQc(c) = SpecClearMissingCommitQc(c)
  /\ ActualClearMissingBlock(c) = SpecClearMissingBlock(c)
  /\ ActualClearMissingBlockReason(c) = SpecClearMissingBlockReason(c)
  /\ ActualReturnKind(c) = SpecReturnKind(c)
  /\ ActualContinues(c) = SpecContinues(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "source_metric_skipped",
       "vote_roster_not_cached",
       "checkpoint_not_recorded",
       "absent_checkpoint_recorded",
       "commit_roster_record_not_prepared",
       "commit_roster_not_persisted",
       "commit_roster_persisted_for_unknown",
       "commit_roster_uses_synth_over_selection_checkpoint",
       "commit_roster_drops_stake",
       "known_votes_not_processed",
       "incoming_qc_not_preferred",
       "selection_qc_not_preferred",
       "checkpoint_qc_not_used",
       "invalid_checkpoint_used",
       "redundant_qc_replayed",
       "cached_only_skips_replay",
       "snapshot_only_skips_replay",
       "prepare_none_enqueued",
       "prepare_not_called_for_selection",
       "commit_qc_match_arg_wrong_for_incoming",
       "commit_qc_match_arg_wrong_for_selection",
       "cached_commit_qc_no_clear",
       "no_cached_commit_qc_cleared",
       "missing_block_not_cleared",
       "missing_block_wrong_reason",
       "known_path_continues",
       "known_path_returns_error",
       "unknown_path_returns_ok",
       "unknown_path_clears_missing"
     }
  /\ checked = 0

KnownSelectedRosterMatchesSpec ==
  \A c \in Cases: Matches(c)

BlockSyncKnownSelectedRosterExactness ==
  /\ KnownSelectedRosterMatchesSpec

BlockSyncKnownSelectedRosterCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncKnownSelectedRosterExactness

SafetyFast ==
  BlockSyncKnownSelectedRosterExactness

SelectedRosterBookkeeping ==
  Matches("unknown_selected")
    /\ Matches("known_selection_qc")
    /\ Matches("known_checkpoint_only")

CommitRosterPersistence ==
  Matches("known_selection_qc")
    /\ Matches("known_synth_checkpoint_record")
    /\ Matches("known_checkpoint_record")
    /\ Matches("known_stake_record")
    /\ Matches("unknown_selected")

KnownQcSourcePrecedence ==
  Matches("known_incoming_qc")
    /\ Matches("known_incoming_preempts_selection")
    /\ Matches("known_selection_preempts_checkpoint")
    /\ Matches("known_checkpoint_only")
    /\ Matches("known_checkpoint_conversion_fails")

KnownQcReplaySuppression ==
  Matches("known_redundant_qc")
    /\ Matches("known_cached_only_replays")
    /\ Matches("known_snapshot_only_replays")
    /\ Matches("known_prepare_none")

KnownClearAndReturn ==
  Matches("known_no_qc")
    /\ Matches("known_cached_commit_qc")
    /\ Matches("known_selection_qc")
    /\ Matches("unknown_selected")

=============================================================================
====
