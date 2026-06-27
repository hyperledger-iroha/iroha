---- MODULE SumeragiBlockSyncVoteDeferralGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the `handle_block_sync_update(...)` boundary that
processes embedded commit votes before the deferred BlockSyncUpdate path.

The code first filters embedded commit votes by phase, block hash, height, view,
and epoch. Those votes may arm missing-block recovery, but the local
`requested_missing_block` flag is refreshed only when implicit frontier recovery
is allowed or the update was explicitly requested. A known-block vote-only
update returns through a fast path before deferral. Otherwise, an entry
deferral reason sends a vote-stripped BlockSyncUpdate to the deferred cache
while preserving QC, checkpoint, and stake sidecars.
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
  "no_votes_continue",
  "valid_vote_continue",
  "invalid_phase_drop",
  "invalid_hash_drop",
  "invalid_height_drop",
  "invalid_view_drop",
  "invalid_epoch_drop",
  "mixed_votes",
  "vote_request_implicit_allowed",
  "vote_request_implicit_blocked",
  "vote_request_explicit",
  "known_vote_only_fast_path",
  "known_with_qc_defers",
  "defer_after_votes",
  "defer_no_votes",
  "defer_preserves_qc",
  "defer_preserves_checkpoint",
  "defer_preserves_stake"
}

ValidVoteCount(c) ==
  CASE c \in {
       "valid_vote_continue",
       "vote_request_implicit_allowed",
       "vote_request_implicit_blocked",
       "vote_request_explicit",
       "known_vote_only_fast_path",
       "defer_after_votes"
     } -> 1
    [] c = "mixed_votes" -> 1
    [] OTHER -> 0

InvalidVoteCount(c) ==
  IF c \in {
       "invalid_phase_drop",
       "invalid_hash_drop",
       "invalid_height_drop",
       "invalid_view_drop",
       "invalid_epoch_drop",
       "mixed_votes"
     }
  THEN 1
  ELSE 0

IncomingVoteCount(c) == ValidVoteCount(c) + InvalidVoteCount(c)

HasCommitVotes(c) ==
  IncomingVoteCount(c) > 0

IncomingQc(c) ==
  c \in {"known_with_qc_defers", "defer_preserves_qc"}

IncomingCheckpoint(c) ==
  c = "defer_preserves_checkpoint"

IncomingStake(c) ==
  c = "defer_preserves_stake"

HasSidecar(c) ==
  IncomingQc(c) \/ IncomingCheckpoint(c) \/ IncomingStake(c)

BlockKnown(c) ==
  c \in {"known_vote_only_fast_path", "known_with_qc_defers"}

EntryDeferralReason(c) ==
  c \in {
    "known_vote_only_fast_path",
    "known_with_qc_defers",
    "defer_after_votes",
    "defer_no_votes",
    "defer_preserves_qc",
    "defer_preserves_checkpoint",
    "defer_preserves_stake"
  }

VoteArmedMissingRequest(c) ==
  c \in {
    "vote_request_implicit_allowed",
    "vote_request_implicit_blocked",
    "vote_request_explicit"
  }

ExplicitRequested(c) ==
  c = "vote_request_explicit"

InitialRequested(c) ==
  ExplicitRequested(c)

ImplicitAllowed(c) ==
  c = "vote_request_implicit_allowed"

SpecProcessedVotes(c) ==
  ValidVoteCount(c)

SpecDroppedVotes(c) ==
  InvalidVoteCount(c)

SpecRequestedMissing(c) ==
  IF VoteArmedMissingRequest(c) /\ (ImplicitAllowed(c) \/ ExplicitRequested(c))
  THEN TRUE
  ELSE InitialRequested(c)

SpecFastPath(c) ==
  /\ BlockKnown(c)
  /\ HasCommitVotes(c)
  /\ ~HasSidecar(c)

SpecClearMissingRequest(c) ==
  SpecFastPath(c)

SpecDefer(c) ==
  /\ ~SpecFastPath(c)
  /\ EntryDeferralReason(c)

SpecOutcome(c) ==
  IF SpecFastPath(c) THEN "known_vote_only_fast"
  ELSE IF SpecDefer(c) THEN "deferred"
  ELSE "continue"

SpecVotesHandledBeforeOutcome(c) ==
  TRUE

SpecDeferredVoteCount(c) ==
  IF SpecDefer(c) THEN 0 ELSE -1

SpecDeferredQc(c) ==
  SpecDefer(c) /\ IncomingQc(c)

SpecDeferredCheckpoint(c) ==
  SpecDefer(c) /\ IncomingCheckpoint(c)

SpecDeferredStake(c) ==
  SpecDefer(c) /\ IncomingStake(c)

SpecDeferredReason(c) ==
  IF SpecDefer(c) THEN "entry_deferral_reason" ELSE "none"

ActualProcessedVotes(c) ==
  CASE Bug = "valid_vote_dropped"
       /\ c = "valid_vote_continue" -> 0
    [] Bug = "invalid_phase_processed"
       /\ c = "invalid_phase_drop" -> 1
    [] Bug = "invalid_hash_processed"
       /\ c = "invalid_hash_drop" -> 1
    [] Bug = "invalid_height_processed"
       /\ c = "invalid_height_drop" -> 1
    [] Bug = "invalid_view_processed"
       /\ c = "invalid_view_drop" -> 1
    [] Bug = "invalid_epoch_processed"
       /\ c = "invalid_epoch_drop" -> 1
    [] Bug = "mixed_drops_valid"
       /\ c = "mixed_votes" -> 0
    [] Bug = "mixed_processes_invalid"
       /\ c = "mixed_votes" -> 2
    [] OTHER -> SpecProcessedVotes(c)

ActualDroppedVotes(c) ==
  CASE Bug = "valid_vote_dropped"
       /\ c = "valid_vote_continue" -> 1
    [] Bug = "invalid_phase_processed"
       /\ c = "invalid_phase_drop" -> 0
    [] Bug = "invalid_hash_processed"
       /\ c = "invalid_hash_drop" -> 0
    [] Bug = "invalid_height_processed"
       /\ c = "invalid_height_drop" -> 0
    [] Bug = "invalid_view_processed"
       /\ c = "invalid_view_drop" -> 0
    [] Bug = "invalid_epoch_processed"
       /\ c = "invalid_epoch_drop" -> 0
    [] Bug = "mixed_drops_valid"
       /\ c = "mixed_votes" -> 2
    [] Bug = "mixed_processes_invalid"
       /\ c = "mixed_votes" -> 0
    [] OTHER -> SpecDroppedVotes(c)

ActualRequestedMissing(c) ==
  CASE Bug = "vote_request_not_refreshed"
       /\ c = "vote_request_implicit_allowed" -> FALSE
    [] Bug = "vote_request_refreshed_without_allowed"
       /\ c = "vote_request_implicit_blocked" -> TRUE
    [] Bug = "explicit_request_lost"
       /\ c = "vote_request_explicit" -> FALSE
    [] OTHER -> SpecRequestedMissing(c)

ActualFastPath(c) ==
  CASE Bug = "known_vote_only_not_fast"
       /\ c = "known_vote_only_fast_path" -> FALSE
    [] Bug = "qc_known_treated_vote_only"
       /\ c = "known_with_qc_defers" -> TRUE
    [] OTHER -> SpecFastPath(c)

ActualClearMissingRequest(c) ==
  IF ActualFastPath(c)
  THEN CASE Bug = "known_vote_only_no_clear"
            /\ c = "known_vote_only_fast_path" -> FALSE
         [] OTHER -> TRUE
  ELSE FALSE

ActualDefer(c) ==
  IF ActualFastPath(c) THEN
    /\ Bug = "known_vote_only_defers"
    /\ c = "known_vote_only_fast_path"
  ELSE CASE Bug = "deferral_skipped"
            /\ c = "defer_after_votes" -> FALSE
         [] OTHER -> EntryDeferralReason(c)

ActualOutcome(c) ==
  IF ActualFastPath(c) THEN
    IF ActualDefer(c) THEN "fast_and_deferred" ELSE "known_vote_only_fast"
  ELSE IF ActualDefer(c) THEN "deferred"
  ELSE "continue"

ActualVotesHandledBeforeOutcome(c) ==
  CASE Bug = "defer_before_vote_processing"
       /\ c = "defer_after_votes" -> FALSE
    [] OTHER -> TRUE

ActualDeferredVoteCount(c) ==
  IF ~ActualDefer(c) THEN -1
  ELSE CASE Bug = "defer_keeps_votes"
            /\ c = "defer_after_votes" -> IncomingVoteCount(c)
         [] OTHER -> 0

ActualDeferredQc(c) ==
  IF ~ActualDefer(c) THEN FALSE
  ELSE CASE Bug = "defer_drops_qc"
            /\ c = "defer_preserves_qc" -> FALSE
         [] OTHER -> IncomingQc(c)

ActualDeferredCheckpoint(c) ==
  IF ~ActualDefer(c) THEN FALSE
  ELSE CASE Bug = "defer_drops_checkpoint"
            /\ c = "defer_preserves_checkpoint" -> FALSE
         [] OTHER -> IncomingCheckpoint(c)

ActualDeferredStake(c) ==
  IF ~ActualDefer(c) THEN FALSE
  ELSE CASE Bug = "defer_drops_stake"
            /\ c = "defer_preserves_stake" -> FALSE
         [] OTHER -> IncomingStake(c)

ActualDeferredReason(c) ==
  IF ~ActualDefer(c) THEN "none"
  ELSE CASE Bug = "defer_wrong_reason"
            /\ c = "defer_no_votes" -> "wrong_reason"
         [] OTHER -> "entry_deferral_reason"

Matches(c) ==
  /\ ActualProcessedVotes(c) = SpecProcessedVotes(c)
  /\ ActualDroppedVotes(c) = SpecDroppedVotes(c)
  /\ ActualRequestedMissing(c) = SpecRequestedMissing(c)
  /\ ActualFastPath(c) = SpecFastPath(c)
  /\ ActualClearMissingRequest(c) = SpecClearMissingRequest(c)
  /\ ActualDefer(c) = SpecDefer(c)
  /\ ActualOutcome(c) = SpecOutcome(c)
  /\ ActualVotesHandledBeforeOutcome(c) = SpecVotesHandledBeforeOutcome(c)
  /\ ActualDeferredVoteCount(c) = SpecDeferredVoteCount(c)
  /\ ActualDeferredQc(c) = SpecDeferredQc(c)
  /\ ActualDeferredCheckpoint(c) = SpecDeferredCheckpoint(c)
  /\ ActualDeferredStake(c) = SpecDeferredStake(c)
  /\ ActualDeferredReason(c) = SpecDeferredReason(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "valid_vote_dropped",
       "invalid_phase_processed",
       "invalid_hash_processed",
       "invalid_height_processed",
       "invalid_view_processed",
       "invalid_epoch_processed",
       "mixed_drops_valid",
       "mixed_processes_invalid",
       "vote_request_not_refreshed",
       "vote_request_refreshed_without_allowed",
       "explicit_request_lost",
       "known_vote_only_not_fast",
       "known_vote_only_defers",
       "known_vote_only_no_clear",
       "qc_known_treated_vote_only",
       "deferral_skipped",
       "defer_keeps_votes",
       "defer_drops_qc",
       "defer_drops_checkpoint",
       "defer_drops_stake",
       "defer_before_vote_processing",
       "defer_wrong_reason"
     }
  /\ checked = 0

VoteDeferralMatchesSpec ==
  \A c \in Cases: Matches(c)

BlockSyncVoteDeferralExactness ==
  VoteDeferralMatchesSpec

BlockSyncVoteDeferralCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncVoteDeferralExactness

SafetyFast ==
  BlockSyncVoteDeferralExactness

NoVotesContinue ==
  Matches("no_votes_continue")

ValidVoteProcessed ==
  Matches("valid_vote_continue")

InvalidPhaseDropped ==
  Matches("invalid_phase_drop")

InvalidHashDropped ==
  Matches("invalid_hash_drop")

InvalidHeightDropped ==
  Matches("invalid_height_drop")

InvalidViewDropped ==
  Matches("invalid_view_drop")

InvalidEpochDropped ==
  Matches("invalid_epoch_drop")

MixedVotesFiltered ==
  Matches("mixed_votes")

VoteRequestRefreshes ==
  Matches("vote_request_implicit_allowed")

VoteRequestRequiresAllowance ==
  Matches("vote_request_implicit_blocked")

ExplicitRequestPreserved ==
  Matches("vote_request_explicit")

KnownVoteOnlyFastPath ==
  Matches("known_vote_only_fast_path")

KnownVoteOnlyDoesNotDefer ==
  Matches("known_vote_only_fast_path")

KnownVoteOnlyClearsMissing ==
  Matches("known_vote_only_fast_path")

KnownWithQcDefers ==
  Matches("known_with_qc_defers")

DeferralRuns ==
  Matches("defer_after_votes")

DeferredVotesStripped ==
  Matches("defer_after_votes")

DeferredQcPreserved ==
  Matches("defer_preserves_qc")

DeferredCheckpointPreserved ==
  Matches("defer_preserves_checkpoint")

DeferredStakePreserved ==
  Matches("defer_preserves_stake")

VotesBeforeDeferral ==
  Matches("defer_after_votes")

DeferralReasonForwarded ==
  Matches("defer_no_votes")

=============================================================================
====
