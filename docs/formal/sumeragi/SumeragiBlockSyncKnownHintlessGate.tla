---- MODULE SumeragiBlockSyncKnownHintlessGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the already-known, hintless BlockSyncUpdate fast
path in `handle_block_sync_update(...)`.

After committed-height conflict checks, the live path asks Kura whether the
incoming block hash is already known. If the block is known and the update
carries no roster hint, the update is skipped, the missing-block request for
that hash is cleared as `PayloadAvailable`, and the handler returns `Ok(())`.
Any roster hint -- commit QC, validator checkpoint, stake snapshot, or embedded
commit vote -- keeps the update on the later roster/vote path, and unknown
blocks never take this known-block fast path.
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
  "unknown_no_hint",
  "known_no_hint",
  "known_qc_hint",
  "known_checkpoint_hint",
  "known_stake_hint",
  "known_vote_hint",
  "unknown_qc_hint"
}

BlockKnown(c) ==
  c \in {
    "known_no_hint",
    "known_qc_hint",
    "known_checkpoint_hint",
    "known_stake_hint",
    "known_vote_hint"
  }

IncomingQc(c) ==
  c \in {"known_qc_hint", "unknown_qc_hint"}

ValidatorCheckpoint(c) ==
  c = "known_checkpoint_hint"

StakeSnapshot(c) ==
  c = "known_stake_hint"

CommitVotes(c) ==
  c = "known_vote_hint"

SpecHasRosterHint(c) ==
  IncomingQc(c) \/ ValidatorCheckpoint(c) \/ StakeSnapshot(c) \/ CommitVotes(c)

SpecFastPath(c) ==
  BlockKnown(c) /\ ~SpecHasRosterHint(c)

SpecClearMissing(c) ==
  SpecFastPath(c)

SpecClearReason(c) ==
  IF SpecClearMissing(c) THEN "PayloadAvailable" ELSE "none"

SpecRecordsStatus(c) ==
  FALSE

SpecReturnKind(c) ==
  IF SpecFastPath(c) THEN "Ok" ELSE "continue"

SpecContinues(c) ==
  ~SpecFastPath(c)

ActualHasRosterHint(c) ==
  CASE Bug = "qc_hint_ignored"
       /\ c = "known_qc_hint" -> FALSE
    [] Bug = "checkpoint_hint_ignored"
       /\ c = "known_checkpoint_hint" -> FALSE
    [] Bug = "stake_hint_ignored"
       /\ c = "known_stake_hint" -> FALSE
    [] Bug = "vote_hint_ignored"
       /\ c = "known_vote_hint" -> FALSE
    [] OTHER -> SpecHasRosterHint(c)

ActualFastPath(c) ==
  CASE Bug = "known_no_hint_not_skipped"
       /\ c = "known_no_hint" -> FALSE
    [] Bug = "unknown_no_hint_skipped"
       /\ c = "unknown_no_hint" -> TRUE
    [] Bug = "unknown_qc_hint_skipped"
       /\ c = "unknown_qc_hint" -> TRUE
    [] OTHER -> BlockKnown(c) /\ ~ActualHasRosterHint(c)

ActualClearMissing(c) ==
  IF ~ActualFastPath(c) THEN FALSE
  ELSE CASE Bug = "known_no_hint_no_clear"
            /\ c = "known_no_hint" -> FALSE
         [] OTHER -> TRUE

ActualClearReason(c) ==
  IF ~ActualClearMissing(c) THEN "none"
  ELSE CASE Bug = "known_no_hint_wrong_clear_reason"
            /\ c = "known_no_hint" -> "Obsolete"
         [] OTHER -> "PayloadAvailable"

ActualRecordsStatus(c) ==
  Bug = "known_no_hint_records_status" /\ c = "known_no_hint"

ActualReturnKind(c) ==
  IF ActualFastPath(c) THEN
    CASE Bug = "known_no_hint_returns_error"
         /\ c = "known_no_hint" -> "Err"
      [] OTHER -> "Ok"
  ELSE "continue"

ActualContinues(c) ==
  IF Bug = "known_no_hint_continues"
     /\ c = "known_no_hint"
  THEN TRUE
  ELSE ~ActualFastPath(c)

Matches(c) ==
  /\ ActualHasRosterHint(c) = SpecHasRosterHint(c)
  /\ ActualFastPath(c) = SpecFastPath(c)
  /\ ActualClearMissing(c) = SpecClearMissing(c)
  /\ ActualClearReason(c) = SpecClearReason(c)
  /\ ActualRecordsStatus(c) = SpecRecordsStatus(c)
  /\ ActualReturnKind(c) = SpecReturnKind(c)
  /\ ActualContinues(c) = SpecContinues(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "known_no_hint_not_skipped",
       "known_no_hint_no_clear",
       "known_no_hint_wrong_clear_reason",
       "known_no_hint_records_status",
       "known_no_hint_returns_error",
       "known_no_hint_continues",
       "unknown_no_hint_skipped",
       "unknown_qc_hint_skipped",
       "qc_hint_ignored",
       "checkpoint_hint_ignored",
       "stake_hint_ignored",
       "vote_hint_ignored"
     }
  /\ checked = 0

KnownHintlessMatchesSpec ==
  \A c \in Cases: Matches(c)

BlockSyncKnownHintlessExactness ==
  /\ KnownHintlessMatchesSpec

BlockSyncKnownHintlessCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncKnownHintlessExactness

SafetyFast ==
  BlockSyncKnownHintlessExactness

KnownHintlessFastPath ==
  Matches("known_no_hint")

KnownHintlessClearsMissing ==
  Matches("known_no_hint")

KnownHintlessClearReason ==
  Matches("known_no_hint")

KnownHintlessNoStatusRecord ==
  Matches("known_no_hint")

KnownHintlessReturnsOk ==
  Matches("known_no_hint")

KnownHintlessDoesNotContinue ==
  Matches("known_no_hint")

UnknownHintlessContinues ==
  Matches("unknown_no_hint")

UnknownWithQcContinues ==
  Matches("unknown_qc_hint")

KnownQcHintContinues ==
  Matches("known_qc_hint")

KnownCheckpointHintContinues ==
  Matches("known_checkpoint_hint")

KnownStakeHintContinues ==
  Matches("known_stake_hint")

KnownVoteHintContinues ==
  Matches("known_vote_hint")

=============================================================================
====
