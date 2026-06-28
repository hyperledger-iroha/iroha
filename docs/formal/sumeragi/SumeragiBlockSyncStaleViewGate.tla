---- MODULE SumeragiBlockSyncStaleViewGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the stale-view admission gate in
`handle_block_sync_update(...)`.

When a BlockSyncUpdate targets a stale view, the live path drops it only if the
update was not requested, the block is not already known locally, and there is
no commit evidence attached. Commit evidence includes an incoming commit QC,
a validator checkpoint, or embedded commit votes. Dropped updates are recorded
as `BlockSyncUpdate` / `Dropped` / `StaleView`, return `Ok(())`, and do not
clear missing-block requests; requested, locally known, or evidence-bearing
stale updates continue to the later block-sync gates.
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
  "fresh_view",
  "stale_unrequested_unknown_no_evidence",
  "stale_requested",
  "stale_known_block",
  "stale_with_qc",
  "stale_with_checkpoint",
  "stale_with_votes"
}

StaleView(c) ==
  c # "fresh_view"

RequestedMissing(c) ==
  c = "stale_requested"

BlockKnownLocally(c) ==
  c = "stale_known_block"

IncomingQc(c) ==
  c = "stale_with_qc"

ValidatorCheckpoint(c) ==
  c = "stale_with_checkpoint"

HasCommitVotes(c) ==
  c = "stale_with_votes"

SpecHasCommitEvidence(c) ==
  IncomingQc(c) \/ ValidatorCheckpoint(c) \/ HasCommitVotes(c)

SpecDrop(c) ==
  /\ StaleView(c)
  /\ ~RequestedMissing(c)
  /\ ~BlockKnownLocally(c)
  /\ ~SpecHasCommitEvidence(c)

SpecRecordKind(c) ==
  IF SpecDrop(c) THEN "BlockSyncUpdate" ELSE "none"

SpecRecordOutcome(c) ==
  IF SpecDrop(c) THEN "Dropped" ELSE "none"

SpecRecordReason(c) ==
  IF SpecDrop(c) THEN "StaleView" ELSE "none"

SpecClearMissing(c) ==
  FALSE

SpecContinue(c) ==
  ~SpecDrop(c)

SpecReturnOk(c) ==
  TRUE

ActualHasCommitEvidence(c) ==
  CASE Bug = "stale_qc_ignored_as_evidence"
       /\ c = "stale_with_qc" -> FALSE
    [] Bug = "stale_checkpoint_ignored_as_evidence"
       /\ c = "stale_with_checkpoint" -> FALSE
    [] Bug = "stale_votes_ignored_as_evidence"
       /\ c = "stale_with_votes" -> FALSE
    [] OTHER -> SpecHasCommitEvidence(c)

ActualDrop(c) ==
  CASE Bug = "fresh_dropped"
       /\ c = "fresh_view" -> TRUE
    [] Bug = "stale_unrequested_unknown_no_evidence_allowed"
       /\ c = "stale_unrequested_unknown_no_evidence" -> FALSE
    [] Bug = "stale_requested_dropped"
       /\ c = "stale_requested" -> TRUE
    [] Bug = "stale_known_dropped"
       /\ c = "stale_known_block" -> TRUE
    [] Bug = "stale_qc_dropped"
       /\ c = "stale_with_qc" -> TRUE
    [] Bug = "stale_checkpoint_dropped"
       /\ c = "stale_with_checkpoint" -> TRUE
    [] Bug = "stale_votes_dropped"
       /\ c = "stale_with_votes" -> TRUE
    [] OTHER ->
       /\ StaleView(c)
       /\ ~RequestedMissing(c)
       /\ ~BlockKnownLocally(c)
       /\ ~ActualHasCommitEvidence(c)

ActualRecordKind(c) ==
  IF ~ActualDrop(c) THEN "none"
  ELSE CASE Bug = "stale_drop_wrong_kind"
            /\ c = "stale_unrequested_unknown_no_evidence" -> "Qc"
         [] OTHER -> "BlockSyncUpdate"

ActualRecordOutcome(c) ==
  IF ~ActualDrop(c) THEN "none"
  ELSE CASE Bug = "stale_drop_wrong_outcome"
            /\ c = "stale_unrequested_unknown_no_evidence" -> "Accepted"
         [] OTHER -> "Dropped"

ActualRecordReason(c) ==
  IF ~ActualDrop(c) THEN "none"
  ELSE CASE Bug = "stale_drop_wrong_reason"
            /\ c = "stale_unrequested_unknown_no_evidence" -> "FutureWindow"
         [] OTHER -> "StaleView"

ActualClearMissing(c) ==
  IF Bug = "stale_drop_clears_missing"
     /\ c = "stale_unrequested_unknown_no_evidence"
  THEN TRUE
  ELSE FALSE

ActualContinue(c) ==
  ~ActualDrop(c)

ActualReturnOk(c) ==
  CASE Bug = "stale_drop_returns_error"
       /\ c = "stale_unrequested_unknown_no_evidence" -> FALSE
    [] OTHER -> TRUE

Matches(c) ==
  /\ ActualHasCommitEvidence(c) = SpecHasCommitEvidence(c)
  /\ ActualDrop(c) = SpecDrop(c)
  /\ ActualRecordKind(c) = SpecRecordKind(c)
  /\ ActualRecordOutcome(c) = SpecRecordOutcome(c)
  /\ ActualRecordReason(c) = SpecRecordReason(c)
  /\ ActualClearMissing(c) = SpecClearMissing(c)
  /\ ActualContinue(c) = SpecContinue(c)
  /\ ActualReturnOk(c) = SpecReturnOk(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "fresh_dropped",
       "stale_unrequested_unknown_no_evidence_allowed",
       "stale_drop_wrong_kind",
       "stale_drop_wrong_outcome",
       "stale_drop_wrong_reason",
       "stale_drop_returns_error",
       "stale_drop_clears_missing",
       "stale_requested_dropped",
       "stale_known_dropped",
       "stale_qc_dropped",
       "stale_checkpoint_dropped",
       "stale_votes_dropped",
       "stale_qc_ignored_as_evidence",
       "stale_checkpoint_ignored_as_evidence",
       "stale_votes_ignored_as_evidence"
     }
  /\ checked = 0

StaleViewMatchesSpec ==
  \A c \in Cases: Matches(c)

BlockSyncStaleViewExactness ==
  /\ StaleViewMatchesSpec

BlockSyncStaleViewCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncStaleViewExactness

SafetyFast == BlockSyncStaleViewExactness

FreshViewContinues ==
  Matches("fresh_view")

StaleNoEvidenceDrops ==
  Matches("stale_unrequested_unknown_no_evidence")

StaleDropRecordKind ==
  Matches("stale_unrequested_unknown_no_evidence")

StaleDropRecordOutcome ==
  Matches("stale_unrequested_unknown_no_evidence")

StaleDropRecordReason ==
  Matches("stale_unrequested_unknown_no_evidence")

StaleDropReturnsOk ==
  Matches("stale_unrequested_unknown_no_evidence")

StaleDropDoesNotClearMissing ==
  Matches("stale_unrequested_unknown_no_evidence")

StaleRequestedContinues ==
  Matches("stale_requested")

StaleKnownContinues ==
  Matches("stale_known_block")

StaleQcContinues ==
  Matches("stale_with_qc")

StaleCheckpointContinues ==
  Matches("stale_with_checkpoint")

StaleVotesContinues ==
  Matches("stale_with_votes")

QcCountsAsEvidence ==
  Matches("stale_with_qc")

CheckpointCountsAsEvidence ==
  Matches("stale_with_checkpoint")

VotesCountAsEvidence ==
  Matches("stale_with_votes")

=============================================================================
====
