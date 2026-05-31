---- MODULE SumeragiBlockSyncSnapshotHintGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for known-block commit-roster snapshot hint filtering
in `handle_block_sync_update(...)`.

After the known-block hintless fast path and the frontier vote-placeholder
side effect, the live path asks for a locally recorded commit-roster snapshot
only when the incoming block is already known. If that snapshot exists, incoming
roster hints are filtered against it before later roster selection:

* a matching commit QC is preserved;
* a different commit QC is preserved only when it names the same validator set,
  so the later validation path can revalidate the different signer aggregate;
* mismatching validator checkpoints are dropped;
* stake snapshots are dropped when no local stake snapshot exists or hashes
  differ.

Unknown blocks and known blocks without a local snapshot preserve all incoming
hints. The branch does not record status, clear missing-block requests, defer
the update, or return early.
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
  "unknown_snapshot_hints",
  "known_no_snapshot_hints",
  "known_no_hints",
  "known_matching_qc",
  "known_same_roster_diff_qc",
  "known_diff_roster_qc",
  "known_same_hash_diff_roster_qc",
  "known_matching_checkpoint",
  "known_mismatch_checkpoint",
  "known_matching_stake",
  "known_no_local_stake",
  "known_mismatch_stake",
  "known_all_matching",
  "known_all_mismatch"
}

BlockKnown(c) ==
  c # "unknown_snapshot_hints"

SnapshotExists(c) ==
  c # "known_no_snapshot_hints"

SpecSnapshotPresent(c) ==
  BlockKnown(c) /\ SnapshotExists(c)

IncomingQc(c) ==
  c \in {
    "unknown_snapshot_hints",
    "known_no_snapshot_hints",
    "known_matching_qc",
    "known_same_roster_diff_qc",
    "known_diff_roster_qc",
    "known_same_hash_diff_roster_qc",
    "known_all_matching",
    "known_all_mismatch"
  }

IncomingCheckpoint(c) ==
  c \in {
    "unknown_snapshot_hints",
    "known_no_snapshot_hints",
    "known_matching_checkpoint",
    "known_mismatch_checkpoint",
    "known_all_matching",
    "known_all_mismatch"
  }

IncomingStake(c) ==
  c \in {
    "unknown_snapshot_hints",
    "known_no_snapshot_hints",
    "known_matching_stake",
    "known_no_local_stake",
    "known_mismatch_stake",
    "known_all_matching",
    "known_all_mismatch"
  }

QcHashMatches(c) ==
  c \in {
    "known_matching_qc",
    "known_same_hash_diff_roster_qc",
    "known_all_matching"
  }

QcSameValidatorSet(c) ==
  c \in {
    "known_matching_qc",
    "known_same_roster_diff_qc",
    "known_all_matching"
  }

CheckpointHashMatches(c) ==
  c \in {"known_matching_checkpoint", "known_all_matching"}

LocalStakePresent(c) ==
  c # "known_no_local_stake"

StakeHashMatches(c) ==
  c \in {"known_matching_stake", "known_all_matching"}

SpecQcAfter(c) ==
  IF ~IncomingQc(c) THEN FALSE
  ELSE IF ~SpecSnapshotPresent(c) THEN TRUE
  ELSE IF QcHashMatches(c) THEN TRUE
  ELSE QcSameValidatorSet(c)

SpecQcRevalidated(c) ==
  /\ IncomingQc(c)
  /\ SpecSnapshotPresent(c)
  /\ ~QcHashMatches(c)
  /\ QcSameValidatorSet(c)

SpecCheckpointAfter(c) ==
  IF ~IncomingCheckpoint(c) THEN FALSE
  ELSE IF ~SpecSnapshotPresent(c) THEN TRUE
  ELSE CheckpointHashMatches(c)

SpecStakeAfter(c) ==
  IF ~IncomingStake(c) THEN FALSE
  ELSE IF ~SpecSnapshotPresent(c) THEN TRUE
  ELSE LocalStakePresent(c) /\ StakeHashMatches(c)

SpecRecordsStatus(c) ==
  FALSE

SpecClearsMissing(c) ==
  FALSE

SpecDefers(c) ==
  FALSE

SpecReturnKind(c) ==
  "continue"

SpecContinues(c) ==
  TRUE

ActualSnapshotPresent(c) ==
  CASE Bug = "unknown_applies_snapshot"
       /\ c = "unknown_snapshot_hints" -> TRUE
    [] Bug = "known_no_snapshot_applies_snapshot"
       /\ c = "known_no_snapshot_hints" -> TRUE
    [] OTHER -> SpecSnapshotPresent(c)

ActualQcAfter(c) ==
  IF ~IncomingQc(c) THEN FALSE
  ELSE IF Bug = "matching_qc_dropped"
          /\ c = "known_matching_qc" THEN FALSE
  ELSE IF Bug = "same_roster_diff_qc_dropped"
          /\ c = "known_same_roster_diff_qc" THEN FALSE
  ELSE IF Bug = "diff_roster_qc_kept"
          /\ c = "known_diff_roster_qc" THEN TRUE
  ELSE IF Bug = "same_hash_diff_roster_qc_dropped"
          /\ c = "known_same_hash_diff_roster_qc" THEN FALSE
  ELSE IF Bug = "all_mismatch_keeps_qc"
          /\ c = "known_all_mismatch" THEN TRUE
  ELSE IF ~ActualSnapshotPresent(c) THEN TRUE
  ELSE IF QcHashMatches(c) THEN TRUE
  ELSE QcSameValidatorSet(c)

ActualQcRevalidated(c) ==
  CASE Bug = "same_roster_diff_qc_not_revalidated"
       /\ c = "known_same_roster_diff_qc" -> FALSE
    [] Bug = "same_hash_diff_roster_qc_revalidated"
       /\ c = "known_same_hash_diff_roster_qc" -> TRUE
    [] Bug = "diff_roster_qc_revalidated"
       /\ c = "known_diff_roster_qc" -> TRUE
    [] OTHER ->
         /\ IncomingQc(c)
         /\ ActualSnapshotPresent(c)
         /\ ActualQcAfter(c)
         /\ ~QcHashMatches(c)
         /\ QcSameValidatorSet(c)

ActualCheckpointAfter(c) ==
  IF ~IncomingCheckpoint(c) THEN FALSE
  ELSE CASE Bug = "matching_checkpoint_dropped"
            /\ c = "known_matching_checkpoint" -> FALSE
         [] Bug = "mismatch_checkpoint_kept"
            /\ c = "known_mismatch_checkpoint" -> TRUE
         [] Bug = "all_mismatch_keeps_checkpoint"
            /\ c = "known_all_mismatch" -> TRUE
         [] ~ActualSnapshotPresent(c) -> TRUE
         [] OTHER -> CheckpointHashMatches(c)

ActualStakeAfter(c) ==
  IF ~IncomingStake(c) THEN FALSE
  ELSE CASE Bug = "matching_stake_dropped"
            /\ c = "known_matching_stake" -> FALSE
         [] Bug = "no_local_stake_kept"
            /\ c = "known_no_local_stake" -> TRUE
         [] Bug = "mismatch_stake_kept"
            /\ c = "known_mismatch_stake" -> TRUE
         [] Bug = "all_mismatch_keeps_stake"
            /\ c = "known_all_mismatch" -> TRUE
         [] ~ActualSnapshotPresent(c) -> TRUE
         [] OTHER -> LocalStakePresent(c) /\ StakeHashMatches(c)

ActualRecordsStatus(c) ==
  Bug = "records_status" /\ c = "known_all_matching"

ActualClearsMissing(c) ==
  Bug = "clears_missing" /\ c = "known_all_matching"

ActualDefers(c) ==
  Bug = "defers_update" /\ c = "known_all_matching"

ActualReturnKind(c) ==
  IF Bug = "returns_early" /\ c = "known_all_matching" THEN "Ok" ELSE "continue"

ActualContinues(c) ==
  ~(Bug = "returns_early" /\ c = "known_all_matching")

Matches(c) ==
  /\ ActualSnapshotPresent(c) = SpecSnapshotPresent(c)
  /\ ActualQcAfter(c) = SpecQcAfter(c)
  /\ ActualQcRevalidated(c) = SpecQcRevalidated(c)
  /\ ActualCheckpointAfter(c) = SpecCheckpointAfter(c)
  /\ ActualStakeAfter(c) = SpecStakeAfter(c)
  /\ ActualRecordsStatus(c) = SpecRecordsStatus(c)
  /\ ActualClearsMissing(c) = SpecClearsMissing(c)
  /\ ActualDefers(c) = SpecDefers(c)
  /\ ActualReturnKind(c) = SpecReturnKind(c)
  /\ ActualContinues(c) = SpecContinues(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "unknown_applies_snapshot",
       "known_no_snapshot_applies_snapshot",
       "matching_qc_dropped",
       "same_roster_diff_qc_dropped",
       "diff_roster_qc_kept",
       "same_hash_diff_roster_qc_dropped",
       "same_hash_diff_roster_qc_revalidated",
       "same_roster_diff_qc_not_revalidated",
       "diff_roster_qc_revalidated",
       "matching_checkpoint_dropped",
       "mismatch_checkpoint_kept",
       "matching_stake_dropped",
       "no_local_stake_kept",
       "mismatch_stake_kept",
       "all_mismatch_keeps_qc",
       "all_mismatch_keeps_checkpoint",
       "all_mismatch_keeps_stake",
       "records_status",
       "clears_missing",
       "defers_update",
       "returns_early"
     }
  /\ checked = 0

SafetyFast ==
  \A c \in Cases: Matches(c)

UnknownDoesNotUseSnapshot ==
  Matches("unknown_snapshot_hints")

KnownWithoutSnapshotPreservesHints ==
  Matches("known_no_snapshot_hints")

MatchingQcPreserved ==
  Matches("known_matching_qc")

SameRosterDifferentQcRevalidated ==
  Matches("known_same_roster_diff_qc")

DifferentRosterQcDropped ==
  Matches("known_diff_roster_qc")

SameHashQcPreservedWithoutRosterCheck ==
  Matches("known_same_hash_diff_roster_qc")

MatchingCheckpointPreserved ==
  Matches("known_matching_checkpoint")

MismatchingCheckpointDropped ==
  Matches("known_mismatch_checkpoint")

MatchingStakePreserved ==
  Matches("known_matching_stake")

MissingLocalStakeDropsIncoming ==
  Matches("known_no_local_stake")

MismatchingStakeDropped ==
  Matches("known_mismatch_stake")

MatchingHintsPreserved ==
  Matches("known_all_matching")

MismatchingHintsDropped ==
  Matches("known_all_mismatch")

=============================================================================
