---- MODULE SumeragiBlockSyncKnownRosterGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the selected-roster bookkeeping and known-block
terminal path in `handle_block_sync_update(...)`.

Once a block-sync roster selection exists, the live path records the selected
source, caches the vote roster, records any validator checkpoint sidecar, and
builds a commit-roster record from the selected commit QC. That record is
persisted only for already-known blocks. Known blocks then process embedded
commit votes, select at most one commit-QC replay source in strict order
(incoming QC, selected commit QC, reconstructed checkpoint QC), suppress
redundant QC replay only when both the QC cache and local roster snapshot
already match, clear any obsolete missing-commit-QC request when the QC is now
cached, clear the missing-block request as `PayloadAvailable`, and return
`Ok(())`.

Unknown blocks share the initial selected-roster bookkeeping but continue into
signature validation instead of taking the known-block terminal path.
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
  "known_no_qc",
  "known_incoming_qc",
  "known_selection_qc",
  "known_checkpoint_only",
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
  "known_stake_record",
  "unknown_selected"
}

BlockKnown(c) ==
  c # "unknown_selected"

IncomingQc(c) ==
  c \in {"known_incoming_qc", "known_incoming_preempts_selection"}

SelectionCommitQc(c) ==
  c \in {
    "known_selection_qc",
    "known_incoming_preempts_selection",
    "known_selection_preempts_checkpoint",
    "known_redundant_qc",
    "known_cached_only_replays",
    "known_snapshot_only_replays",
    "known_prepare_none",
    "known_cached_commit_qc",
    "known_synth_checkpoint_record",
    "known_checkpoint_record",
    "known_stake_record",
    "unknown_selected"
  }

SelectionCheckpoint(c) ==
  c \in {
    "known_checkpoint_only",
    "known_selection_preempts_checkpoint",
    "known_checkpoint_conversion_fails",
    "known_checkpoint_record",
    "unknown_selected"
  }

SelectionStake(c) ==
  c \in {"known_stake_record", "unknown_selected"}

CheckpointConverts(c) ==
  SelectionCheckpoint(c) /\ c # "known_checkpoint_conversion_fails"

CachedQcMatch(c) ==
  c \in {"known_redundant_qc", "known_cached_only_replays"}

LocalSnapshotQcMatch(c) ==
  c \in {"known_redundant_qc", "known_snapshot_only_replays"}

PrepareWorkSome(c) ==
  c # "known_prepare_none"

CachedCommitQcAvailable(c) ==
  c = "known_cached_commit_qc"

SpecSourceMetricRecorded(c) ==
  TRUE

SpecVoteRosterCached(c) ==
  TRUE

SpecCheckpointRecorded(c) ==
  SelectionCheckpoint(c)

SpecCommitRosterPrepared(c) ==
  SelectionCommitQc(c)

SpecCommitRosterPersisted(c) ==
  BlockKnown(c) /\ SpecCommitRosterPrepared(c)

SpecCommitRosterCheckpointKind(c) ==
  IF ~SpecCommitRosterPersisted(c) THEN "none"
  ELSE IF SelectionCheckpoint(c) THEN "selection_checkpoint"
  ELSE "synth_from_commit_qc"

SpecCommitRosterStakeIncluded(c) ==
  SpecCommitRosterPersisted(c) /\ SelectionStake(c)

SpecProcessVotes(c) ==
  BlockKnown(c)

SpecCandidateQcSource(c) ==
  IF ~BlockKnown(c) THEN "later"
  ELSE IF IncomingQc(c) THEN "incoming"
  ELSE IF SelectionCommitQc(c) THEN "selection"
  ELSE IF CheckpointConverts(c) THEN "checkpoint"
  ELSE "none"

SpecRedundantReplay(c) ==
  /\ BlockKnown(c)
  /\ SpecCandidateQcSource(c) # "none"
  /\ CachedQcMatch(c)
  /\ LocalSnapshotQcMatch(c)

SpecPrepareKnownQcWork(c) ==
  /\ BlockKnown(c)
  /\ SpecCandidateQcSource(c) # "none"
  /\ ~SpecRedundantReplay(c)

SpecPrepareCommitQcMatchArg(c) ==
  IF ~SpecPrepareKnownQcWork(c) THEN "not_called"
  ELSE IF SpecCandidateQcSource(c) = "selection" THEN "true"
  ELSE "false"

SpecEnqueueKnownQcWork(c) ==
  SpecPrepareKnownQcWork(c) /\ PrepareWorkSome(c)

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
  ~(Bug = "source_metric_not_recorded" /\ c = "known_selection_qc")

ActualVoteRosterCached(c) ==
  ~(Bug = "vote_roster_not_cached" /\ c = "known_selection_qc")

ActualCheckpointRecorded(c) ==
  IF Bug = "checkpoint_not_recorded"
     /\ c = "known_checkpoint_only"
  THEN FALSE
  ELSE IF Bug = "absent_checkpoint_recorded"
          /\ c = "known_selection_qc" THEN TRUE
  ELSE SelectionCheckpoint(c)

ActualCommitRosterPrepared(c) ==
  SelectionCommitQc(c)

ActualCommitRosterPersisted(c) ==
  IF Bug = "commit_roster_not_persisted"
     /\ c = "known_selection_qc"
  THEN FALSE
  ELSE IF Bug = "commit_roster_persisted_for_unknown"
          /\ c = "unknown_selected" THEN TRUE
  ELSE BlockKnown(c) /\ ActualCommitRosterPrepared(c)

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
  ELSE IF Bug = "unknown_votes_processed"
          /\ c = "unknown_selected" THEN TRUE
  ELSE BlockKnown(c)

ActualCandidateQcSource(c) ==
  IF ~BlockKnown(c) THEN "later"
  ELSE IF Bug = "incoming_qc_not_preferred"
          /\ c = "known_incoming_preempts_selection" THEN "selection"
  ELSE IF Bug = "selection_qc_not_preferred"
          /\ c = "known_selection_preempts_checkpoint" THEN "checkpoint"
  ELSE IF Bug = "checkpoint_qc_not_used"
          /\ c = "known_checkpoint_only" THEN "none"
  ELSE IF Bug = "invalid_checkpoint_used"
          /\ c = "known_checkpoint_conversion_fails" THEN "checkpoint"
  ELSE IF IncomingQc(c) THEN "incoming"
  ELSE IF SelectionCommitQc(c) THEN "selection"
  ELSE IF CheckpointConverts(c) THEN "checkpoint"
  ELSE "none"

ActualRedundantReplay(c) ==
  IF Bug = "redundant_qc_replayed"
     /\ c = "known_redundant_qc"
  THEN FALSE
  ELSE IF Bug = "cached_only_skips_replay"
          /\ c = "known_cached_only_replays" THEN TRUE
  ELSE IF Bug = "snapshot_only_skips_replay"
          /\ c = "known_snapshot_only_replays" THEN TRUE
  ELSE
    /\ BlockKnown(c)
    /\ ActualCandidateQcSource(c) # "none"
    /\ CachedQcMatch(c)
    /\ LocalSnapshotQcMatch(c)

ActualPrepareKnownQcWork(c) ==
  /\ BlockKnown(c)
  /\ ActualCandidateQcSource(c) # "none"
  /\ ~ActualRedundantReplay(c)

ActualPrepareCommitQcMatchArg(c) ==
  IF ~ActualPrepareKnownQcWork(c) THEN "not_called"
  ELSE IF Bug = "commit_qc_match_arg_wrong_for_incoming"
          /\ c = "known_incoming_preempts_selection" THEN "true"
  ELSE IF Bug = "commit_qc_match_arg_wrong_for_selection"
          /\ c = "known_selection_qc" THEN "false"
  ELSE IF ActualCandidateQcSource(c) = "selection" THEN "true"
  ELSE "false"

ActualEnqueueKnownQcWork(c) ==
  IF Bug = "prepare_none_enqueued"
     /\ c = "known_prepare_none"
  THEN TRUE
  ELSE ActualPrepareKnownQcWork(c) /\ PrepareWorkSome(c)

ActualClearMissingCommitQc(c) ==
  IF Bug = "cached_commit_qc_no_clear"
     /\ c = "known_cached_commit_qc"
  THEN FALSE
  ELSE IF Bug = "no_cached_commit_qc_cleared"
          /\ c = "known_selection_qc" THEN TRUE
  ELSE BlockKnown(c) /\ CachedCommitQcAvailable(c)

ActualClearMissingBlock(c) ==
  IF Bug = "missing_block_not_cleared"
     /\ c = "known_selection_qc"
  THEN FALSE
  ELSE IF Bug = "unknown_block_cleared"
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
  ELSE IF Bug = "unknown_path_returns_ok"
          /\ c = "unknown_selected" THEN FALSE
  ELSE ~BlockKnown(c)

Matches(c) ==
  /\ ActualSourceMetricRecorded(c) = SpecSourceMetricRecorded(c)
  /\ ActualVoteRosterCached(c) = SpecVoteRosterCached(c)
  /\ ActualCheckpointRecorded(c) = SpecCheckpointRecorded(c)
  /\ ActualCommitRosterPrepared(c) = SpecCommitRosterPrepared(c)
  /\ ActualCommitRosterPersisted(c) = SpecCommitRosterPersisted(c)
  /\ ActualCommitRosterCheckpointKind(c) = SpecCommitRosterCheckpointKind(c)
  /\ ActualCommitRosterStakeIncluded(c) = SpecCommitRosterStakeIncluded(c)
  /\ ActualProcessVotes(c) = SpecProcessVotes(c)
  /\ ActualCandidateQcSource(c) = SpecCandidateQcSource(c)
  /\ ActualRedundantReplay(c) = SpecRedundantReplay(c)
  /\ ActualPrepareKnownQcWork(c) = SpecPrepareKnownQcWork(c)
  /\ ActualPrepareCommitQcMatchArg(c) = SpecPrepareCommitQcMatchArg(c)
  /\ ActualEnqueueKnownQcWork(c) = SpecEnqueueKnownQcWork(c)
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
       "source_metric_not_recorded",
       "vote_roster_not_cached",
       "checkpoint_not_recorded",
       "absent_checkpoint_recorded",
       "commit_roster_not_persisted",
       "commit_roster_persisted_for_unknown",
       "commit_roster_uses_synth_over_selection_checkpoint",
       "commit_roster_drops_stake",
       "known_votes_not_processed",
       "unknown_votes_processed",
       "incoming_qc_not_preferred",
       "selection_qc_not_preferred",
       "checkpoint_qc_not_used",
       "invalid_checkpoint_used",
       "redundant_qc_replayed",
       "cached_only_skips_replay",
       "snapshot_only_skips_replay",
       "prepare_none_enqueued",
       "commit_qc_match_arg_wrong_for_incoming",
       "commit_qc_match_arg_wrong_for_selection",
       "cached_commit_qc_no_clear",
       "no_cached_commit_qc_cleared",
       "missing_block_not_cleared",
       "missing_block_wrong_reason",
       "unknown_block_cleared",
       "known_path_continues",
       "known_path_returns_error",
       "unknown_path_returns_ok"
     }
  /\ checked = 0

KnownRosterMatchesSpec ==
  \A c \in Cases: Matches(c)

BlockSyncKnownRosterExactness ==
  KnownRosterMatchesSpec

BlockSyncKnownRosterCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncKnownRosterExactness

SafetyFast ==
  BlockSyncKnownRosterExactness

KnownSelectedRosterBookkeeping ==
  Matches("known_selection_qc")

KnownCheckpointBookkeeping ==
  Matches("known_checkpoint_only")
    /\ Matches("known_checkpoint_record")
    /\ Matches("known_synth_checkpoint_record")
    /\ Matches("known_stake_record")

KnownQcSourcePriority ==
  Matches("known_incoming_qc")
    /\ Matches("known_selection_qc")
    /\ Matches("known_checkpoint_only")
    /\ Matches("known_incoming_preempts_selection")
    /\ Matches("known_selection_preempts_checkpoint")
    /\ Matches("known_checkpoint_conversion_fails")

KnownQcReplaySuppression ==
  Matches("known_redundant_qc")
    /\ Matches("known_cached_only_replays")
    /\ Matches("known_snapshot_only_replays")
    /\ Matches("known_prepare_none")

KnownCleanupAndReturn ==
  Matches("known_no_qc")
    /\ Matches("known_cached_commit_qc")

UnknownSelectedRosterContinues ==
  Matches("unknown_selected")

=============================================================================
====
