---- MODULE SumeragiEmptyBlockQcDropGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the empty-block QC filter.

This slice models `block_is_empty(...)`, `should_drop_qc_on_empty_block(...)`,
and the cleanup side effects in `drop_empty_block_state(...)`.  Non-NewView
QCs for locally known empty blocks without due time triggers must be dropped
before downstream QC processing can cache the QC or update lock/highest-QC
state.  The drop path records an invalid-payload message outcome and clears
all block-scoped pending, request, RBC, QC, vote, proposal, roster, and signer
state for the rejected block.  Unknown blocks, NewView QCs, non-empty blocks,
and empty blocks with due time triggers must continue without that cleanup.
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
  "new_view_known_empty",
  "commit_unknown_block",
  "commit_nonempty_pending",
  "commit_empty_due_triggers",
  "prepare_empty_pending",
  "commit_empty_pending",
  "commit_empty_kura",
  "commit_empty_vote_history",
  "commit_empty_proposal_context",
  "commit_empty_rbc_session",
  "commit_empty_missing_request",
  "commit_empty_caches",
  "commit_empty_roster_signer_cache"
}

Phase(c) ==
  CASE c = "new_view_known_empty" -> "NewView"
    [] c = "prepare_empty_pending" -> "Prepare"
    [] OTHER -> "Commit"

BlockKnown(c) ==
  c # "commit_unknown_block"

BlockEmpty(c) ==
  c # "commit_nonempty_pending"

TimeTriggersDue(c) ==
  c = "commit_empty_due_triggers"

BlockIsEmptyResult(c) ==
  IF ~BlockKnown(c)
  THEN "unknown"
  ELSE IF BlockEmpty(c) /\ ~TimeTriggersDue(c)
  THEN "true"
  ELSE "false"

SpecDropped(c) ==
  /\ Phase(c) # "NewView"
  /\ BlockIsEmptyResult(c) = "true"

SpecContinues(c) ==
  ~SpecDropped(c)

SpecInvalidPayloadRecorded(c) ==
  SpecDropped(c)

InitialPending(c) ==
  c \in {
    "commit_nonempty_pending",
    "commit_empty_due_triggers",
    "prepare_empty_pending",
    "commit_empty_pending",
    "commit_empty_vote_history",
    "commit_empty_proposal_context",
    "commit_empty_rbc_session",
    "commit_empty_missing_request",
    "commit_empty_caches",
    "commit_empty_roster_signer_cache"
  }

InitialMissingRequest(c) ==
  c = "commit_empty_missing_request"

InitialRbcSession(c) ==
  c = "commit_empty_rbc_session"

InitialQcCache(c) ==
  c = "commit_empty_caches"

InitialQcTally(c) ==
  c = "commit_empty_caches"

InitialProposal(c) ==
  c = "commit_empty_proposal_context"

InitialHint(c) ==
  c = "commit_empty_proposal_context"

InitialVoteLog(c) ==
  c = "commit_empty_vote_history"

InitialVoteValidation(c) ==
  c = "commit_empty_vote_history"

InitialRosterCache(c) ==
  c = "commit_empty_roster_signer_cache"

InitialSignerCache(c) ==
  c = "commit_empty_roster_signer_cache"

SpecRetained(initial, c) ==
  initial /\ ~SpecDropped(c)

SpecPendingRetained(c) ==
  SpecRetained(InitialPending(c), c)

SpecMissingRequestRetained(c) ==
  SpecRetained(InitialMissingRequest(c), c)

SpecRbcSessionRetained(c) ==
  SpecRetained(InitialRbcSession(c), c)

SpecQcCacheRetained(c) ==
  SpecRetained(InitialQcCache(c), c)

SpecQcTallyRetained(c) ==
  SpecRetained(InitialQcTally(c), c)

SpecProposalRetained(c) ==
  SpecRetained(InitialProposal(c), c)

SpecHintRetained(c) ==
  SpecRetained(InitialHint(c), c)

SpecVoteLogRetained(c) ==
  SpecRetained(InitialVoteLog(c), c)

SpecVoteValidationRetained(c) ==
  SpecRetained(InitialVoteValidation(c), c)

SpecRosterCacheRetained(c) ==
  SpecRetained(InitialRosterCache(c), c)

SpecSignerCacheRetained(c) ==
  SpecRetained(InitialSignerCache(c), c)

ActualDropped(c) ==
  CASE Bug = "drop_new_view_empty"
       /\ c = "new_view_known_empty" -> TRUE
    [] Bug = "drop_unknown_block"
       /\ c = "commit_unknown_block" -> TRUE
    [] Bug = "drop_nonempty_block"
       /\ c = "commit_nonempty_pending" -> TRUE
    [] Bug = "drop_trigger_due_empty"
       /\ c = "commit_empty_due_triggers" -> TRUE
    [] Bug = "keep_prepare_empty"
       /\ c = "prepare_empty_pending" -> FALSE
    [] Bug = "keep_commit_empty"
       /\ c = "commit_empty_pending" -> FALSE
    [] OTHER -> SpecDropped(c)

ActualContinues(c) ==
  CASE Bug = "continue_after_drop"
       /\ SpecDropped(c) -> TRUE
    [] Bug = "stop_after_clean"
       /\ c = "commit_nonempty_pending" -> FALSE
    [] OTHER -> ~ActualDropped(c)

ActualInvalidPayloadRecorded(c) ==
  CASE Bug = "skip_invalid_record_on_drop"
       /\ ActualDropped(c) -> FALSE
    [] Bug = "record_invalid_on_clean"
       /\ c = "commit_nonempty_pending" -> TRUE
    [] OTHER -> ActualDropped(c)

ActualPendingRetained(c) ==
  CASE Bug = "skip_pending_cleanup"
       /\ ActualDropped(c)
       /\ InitialPending(c) -> TRUE
    [] Bug = "cleanup_pending_on_clean"
       /\ c = "commit_nonempty_pending" -> FALSE
    [] OTHER -> InitialPending(c) /\ ~ActualDropped(c)

ActualMissingRequestRetained(c) ==
  CASE Bug = "skip_missing_request_clear"
       /\ ActualDropped(c)
       /\ InitialMissingRequest(c) -> TRUE
    [] OTHER -> InitialMissingRequest(c) /\ ~ActualDropped(c)

ActualRbcSessionRetained(c) ==
  CASE Bug = "skip_rbc_cleanup"
       /\ ActualDropped(c)
       /\ InitialRbcSession(c) -> TRUE
    [] OTHER -> InitialRbcSession(c) /\ ~ActualDropped(c)

ActualQcCacheRetained(c) ==
  CASE Bug = "skip_qc_cache_cleanup"
       /\ ActualDropped(c)
       /\ InitialQcCache(c) -> TRUE
    [] OTHER -> InitialQcCache(c) /\ ~ActualDropped(c)

ActualQcTallyRetained(c) ==
  CASE Bug = "skip_qc_tally_cleanup"
       /\ ActualDropped(c)
       /\ InitialQcTally(c) -> TRUE
    [] OTHER -> InitialQcTally(c) /\ ~ActualDropped(c)

ActualProposalRetained(c) ==
  CASE Bug = "skip_proposal_pop"
       /\ ActualDropped(c)
       /\ InitialProposal(c) -> TRUE
    [] OTHER -> InitialProposal(c) /\ ~ActualDropped(c)

ActualHintRetained(c) ==
  CASE Bug = "skip_hint_pop"
       /\ ActualDropped(c)
       /\ InitialHint(c) -> TRUE
    [] OTHER -> InitialHint(c) /\ ~ActualDropped(c)

ActualVoteLogRetained(c) ==
  CASE Bug = "skip_vote_log_cleanup"
       /\ ActualDropped(c)
       /\ InitialVoteLog(c) -> TRUE
    [] OTHER -> InitialVoteLog(c) /\ ~ActualDropped(c)

ActualVoteValidationRetained(c) ==
  CASE Bug = "skip_vote_validation_cleanup"
       /\ ActualDropped(c)
       /\ InitialVoteValidation(c) -> TRUE
    [] OTHER -> InitialVoteValidation(c) /\ ~ActualDropped(c)

ActualRosterCacheRetained(c) ==
  CASE Bug = "skip_roster_cache_cleanup"
       /\ ActualDropped(c)
       /\ InitialRosterCache(c) -> TRUE
    [] OTHER -> InitialRosterCache(c) /\ ~ActualDropped(c)

ActualSignerCacheRetained(c) ==
  CASE Bug = "skip_signer_cache_cleanup"
       /\ ActualDropped(c)
       /\ InitialSignerCache(c) -> TRUE
    [] OTHER -> InitialSignerCache(c) /\ ~ActualDropped(c)

Matches(c) ==
  /\ ActualDropped(c) = SpecDropped(c)
  /\ ActualContinues(c) = SpecContinues(c)
  /\ ActualInvalidPayloadRecorded(c) = SpecInvalidPayloadRecorded(c)
  /\ ActualPendingRetained(c) = SpecPendingRetained(c)
  /\ ActualMissingRequestRetained(c) = SpecMissingRequestRetained(c)
  /\ ActualRbcSessionRetained(c) = SpecRbcSessionRetained(c)
  /\ ActualQcCacheRetained(c) = SpecQcCacheRetained(c)
  /\ ActualQcTallyRetained(c) = SpecQcTallyRetained(c)
  /\ ActualProposalRetained(c) = SpecProposalRetained(c)
  /\ ActualHintRetained(c) = SpecHintRetained(c)
  /\ ActualVoteLogRetained(c) = SpecVoteLogRetained(c)
  /\ ActualVoteValidationRetained(c) = SpecVoteValidationRetained(c)
  /\ ActualRosterCacheRetained(c) = SpecRosterCacheRetained(c)
  /\ ActualSignerCacheRetained(c) = SpecSignerCacheRetained(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "drop_new_view_empty",
       "drop_unknown_block",
       "drop_nonempty_block",
       "drop_trigger_due_empty",
       "keep_prepare_empty",
       "keep_commit_empty",
       "skip_invalid_record_on_drop",
       "record_invalid_on_clean",
       "continue_after_drop",
       "stop_after_clean",
       "skip_pending_cleanup",
       "cleanup_pending_on_clean",
       "skip_missing_request_clear",
       "skip_rbc_cleanup",
       "skip_qc_cache_cleanup",
       "skip_qc_tally_cleanup",
       "skip_proposal_pop",
       "skip_hint_pop",
       "skip_vote_log_cleanup",
       "skip_vote_validation_cleanup",
       "skip_roster_cache_cleanup",
       "skip_signer_cache_cleanup"
     }
  /\ checked = 0

EmptyBlockMatchesSpec ==
  \A c \in Cases: Matches(c)

Safety == EmptyBlockMatchesSpec

SafetyFast == EmptyBlockMatchesSpec

EmptyBlockDropDecisionExact ==
  \A c \in Cases:
    /\ ActualDropped(c) = SpecDropped(c)
    /\ ActualContinues(c) = SpecContinues(c)

EmptyBlockTelemetryExact ==
  \A c \in Cases:
    ActualInvalidPayloadRecorded(c) = SpecInvalidPayloadRecorded(c)

EmptyBlockPendingCleanupExact ==
  \A c \in Cases:
    /\ ActualPendingRetained(c) = SpecPendingRetained(c)
    /\ ActualMissingRequestRetained(c) = SpecMissingRequestRetained(c)
    /\ ActualRbcSessionRetained(c) = SpecRbcSessionRetained(c)

EmptyBlockQcCleanupExact ==
  \A c \in Cases:
    /\ ActualQcCacheRetained(c) = SpecQcCacheRetained(c)
    /\ ActualQcTallyRetained(c) = SpecQcTallyRetained(c)

EmptyBlockProposalContextCleanupExact ==
  \A c \in Cases:
    /\ ActualProposalRetained(c) = SpecProposalRetained(c)
    /\ ActualHintRetained(c) = SpecHintRetained(c)

EmptyBlockVoteCleanupExact ==
  \A c \in Cases:
    /\ ActualVoteLogRetained(c) = SpecVoteLogRetained(c)
    /\ ActualVoteValidationRetained(c) = SpecVoteValidationRetained(c)

EmptyBlockRosterSignerCleanupExact ==
  \A c \in Cases:
    /\ ActualRosterCacheRetained(c) = SpecRosterCacheRetained(c)
    /\ ActualSignerCacheRetained(c) = SpecSignerCacheRetained(c)

EmptyBlockQcDropExactness ==
  /\ EmptyBlockDropDecisionExact
  /\ EmptyBlockTelemetryExact
  /\ EmptyBlockPendingCleanupExact
  /\ EmptyBlockQcCleanupExact
  /\ EmptyBlockProposalContextCleanupExact
  /\ EmptyBlockVoteCleanupExact
  /\ EmptyBlockRosterSignerCleanupExact

NewViewNeverDropped ==
  Matches("new_view_known_empty")

UnknownBlockNeverDropped ==
  Matches("commit_unknown_block")

EmptyCommitDrops ==
  Matches("commit_empty_pending")

TriggeredEmptyContinues ==
  Matches("commit_empty_due_triggers")

DropCleansBlockState ==
  /\ Matches("commit_empty_caches")
  /\ Matches("commit_empty_vote_history")
  /\ Matches("commit_empty_roster_signer_cache")

=============================================================================
====
