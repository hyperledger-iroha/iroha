---- MODULE SumeragiSameHeightVoteConflictGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for same-height local vote conflict helpers.

This slice pins `local_same_height_vote(...)`,
`local_conflicting_slot_vote(...)`, `local_conflicting_frontier_vote(...)`,
`certified_commit_hash_supersedes_same_height_vote_conflict(...)`,
`new_view_qc_supersedes_same_height_vote_conflict(...)`, and
`same_height_vote_verification_pending_at_or_before_view(...)`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

SelectionCases == {
  "single_prepare_conflicts",
  "single_prepare_same_hash",
  "single_commit_conflicts",
  "commit_beats_prepare",
  "highest_commit_view",
  "wrong_signer_ignored",
  "wrong_height_ignored",
  "wrong_epoch_ignored",
  "non_vote_phase_ignored",
  "no_vote"
}

FrontierCases == {
  "frontier_conflict",
  "frontier_no_conflict",
  "same_height_not_frontier",
  "future_height_not_frontier"
}

CertifiedCases == {
  "certified_commit_match",
  "certified_prepare_match",
  "certified_missing",
  "certified_hash_mismatch"
}

NewViewCases == {
  "new_view_supersedes",
  "proposal_not_newer",
  "locked_same_height",
  "recoverable_conflict_qc",
  "recoverable_any_qc",
  "highest_not_commit",
  "highest_not_parent_height",
  "highest_not_latest",
  "missing_new_view_qc",
  "new_view_wrong_subject",
  "new_view_wrong_height",
  "new_view_wrong_epoch",
  "new_view_below_min",
  "new_view_above_proposal",
  "new_view_wrong_highest"
}

PendingVerificationCases == {
  "pending_prepare_at_view",
  "pending_commit_below_view",
  "pending_future_view",
  "pending_wrong_height",
  "pending_wrong_epoch",
  "pending_new_view_phase",
  "pending_none"
}

BoolToInt(b) == IF b THEN 1 ELSE 0

\* Selection output fields: present, is_commit, view, hash_differs.
\* @type: (Str) => <<Int, Int, Int, Int>>;
SpecSelectionOutput(c) ==
  CASE c = "single_prepare_conflicts" -> <<1, 0, 2, 1>>
    [] c = "single_prepare_same_hash" -> <<1, 0, 2, 0>>
    [] c = "single_commit_conflicts" -> <<1, 1, 3, 1>>
    [] c = "commit_beats_prepare" -> <<1, 1, 1, 1>>
    [] c = "highest_commit_view" -> <<1, 1, 6, 1>>
    [] OTHER -> <<0, 0, 0, 0>>

\* @type: (Str) => <<Int, Int, Int, Int>>;
ActualSelectionOutput(c) ==
  CASE Bug = "selects_nonlocal"
       /\ c = "wrong_signer_ignored" -> <<1, 0, 2, 1>>
    [] Bug = "selects_wrong_height"
       /\ c = "wrong_height_ignored" -> <<1, 0, 2, 1>>
    [] Bug = "selects_wrong_epoch"
       /\ c = "wrong_epoch_ignored" -> <<1, 0, 2, 1>>
    [] Bug = "selects_non_vote_phase"
       /\ c = "non_vote_phase_ignored" -> <<1, 0, 2, 1>>
    [] Bug = "prepare_beats_commit"
       /\ c = "commit_beats_prepare" -> <<1, 0, 9, 1>>
    [] Bug = "lower_commit_view_wins"
       /\ c = "highest_commit_view" -> <<1, 1, 2, 1>>
    [] OTHER -> SpecSelectionOutput(c)

SpecConflict(c) ==
  SpecSelectionOutput(c)[1] = 1 /\ SpecSelectionOutput(c)[4] = 1

ActualConflict(c) ==
  CASE Bug = "same_hash_conflicts"
       /\ c = "single_prepare_same_hash" -> TRUE
    [] Bug = "different_hash_ignored"
       /\ c = "single_prepare_conflicts" -> FALSE
    [] OTHER ->
       ActualSelectionOutput(c)[1] = 1 /\ ActualSelectionOutput(c)[4] = 1

SpecConflictOutput(c) ==
  BoolToInt(SpecConflict(c))

ActualConflictOutput(c) ==
  BoolToInt(ActualConflict(c))

SpecFrontierConflict(c) ==
  c = "frontier_conflict"

ActualFrontierConflict(c) ==
  CASE Bug = "frontier_skips_conflict"
       /\ c = "frontier_conflict" -> FALSE
    [] Bug = "frontier_accepts_same_height"
       /\ c = "same_height_not_frontier" -> TRUE
    [] Bug = "frontier_accepts_future_height"
       /\ c = "future_height_not_frontier" -> TRUE
    [] OTHER -> SpecFrontierConflict(c)

SpecFrontierOutput(c) ==
  BoolToInt(SpecFrontierConflict(c))

ActualFrontierOutput(c) ==
  BoolToInt(ActualFrontierConflict(c))

SpecCertifiedSupersedes(c) ==
  c = "certified_commit_match"

ActualCertifiedSupersedes(c) ==
  CASE Bug = "certified_rejects_match"
       /\ c = "certified_commit_match" -> FALSE
    [] Bug = "certified_prepare_supersedes"
       /\ c = "certified_prepare_match" -> TRUE
    [] Bug = "certified_missing_supersedes"
       /\ c = "certified_missing" -> TRUE
    [] Bug = "certified_wrong_hash_supersedes"
       /\ c = "certified_hash_mismatch" -> TRUE
    [] OTHER -> SpecCertifiedSupersedes(c)

SpecCertifiedOutput(c) ==
  BoolToInt(SpecCertifiedSupersedes(c))

ActualCertifiedOutput(c) ==
  BoolToInt(ActualCertifiedSupersedes(c))

SpecNewViewSupersedes(c) ==
  c = "new_view_supersedes"

ActualNewViewSupersedes(c) ==
  CASE Bug = "new_view_rejects_valid"
       /\ c = "new_view_supersedes" -> FALSE
    [] Bug = "new_view_allows_equal_view"
       /\ c = "proposal_not_newer" -> TRUE
    [] Bug = "new_view_ignores_lock"
       /\ c = "locked_same_height" -> TRUE
    [] Bug = "new_view_ignores_conflict_recovery"
       /\ c = "recoverable_conflict_qc" -> TRUE
    [] Bug = "new_view_ignores_any_recovery"
       /\ c = "recoverable_any_qc" -> TRUE
    [] Bug = "new_view_allows_non_commit_highest"
       /\ c = "highest_not_commit" -> TRUE
    [] Bug = "new_view_allows_wrong_parent_height"
       /\ c = "highest_not_parent_height" -> TRUE
    [] Bug = "new_view_allows_nonlatest_highest"
       /\ c = "highest_not_latest" -> TRUE
    [] Bug = "new_view_allows_missing_qc"
       /\ c = "missing_new_view_qc" -> TRUE
    [] Bug = "new_view_ignores_qc_subject"
       /\ c = "new_view_wrong_subject" -> TRUE
    [] Bug = "new_view_ignores_qc_height"
       /\ c = "new_view_wrong_height" -> TRUE
    [] Bug = "new_view_ignores_qc_epoch"
       /\ c = "new_view_wrong_epoch" -> TRUE
    [] Bug = "new_view_allows_below_min"
       /\ c = "new_view_below_min" -> TRUE
    [] Bug = "new_view_allows_above_proposal"
       /\ c = "new_view_above_proposal" -> TRUE
    [] Bug = "new_view_ignores_highest_link"
       /\ c = "new_view_wrong_highest" -> TRUE
    [] OTHER -> SpecNewViewSupersedes(c)

SpecNewViewOutput(c) ==
  BoolToInt(SpecNewViewSupersedes(c))

ActualNewViewOutput(c) ==
  BoolToInt(ActualNewViewSupersedes(c))

SpecPendingVerification(c) ==
  c \in {"pending_prepare_at_view", "pending_commit_below_view"}

ActualPendingVerification(c) ==
  CASE Bug = "pending_skips_prepare"
       /\ c = "pending_prepare_at_view" -> FALSE
    [] Bug = "pending_skips_commit"
       /\ c = "pending_commit_below_view" -> FALSE
    [] Bug = "pending_allows_future_view"
       /\ c = "pending_future_view" -> TRUE
    [] Bug = "pending_ignores_height"
       /\ c = "pending_wrong_height" -> TRUE
    [] Bug = "pending_ignores_epoch"
       /\ c = "pending_wrong_epoch" -> TRUE
    [] Bug = "pending_accepts_non_vote_phase"
       /\ c = "pending_new_view_phase" -> TRUE
    [] OTHER -> SpecPendingVerification(c)

SpecPendingOutput(c) ==
  BoolToInt(SpecPendingVerification(c))

ActualPendingOutput(c) ==
  BoolToInt(ActualPendingVerification(c))

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "selects_nonlocal",
       "selects_wrong_height",
       "selects_wrong_epoch",
       "selects_non_vote_phase",
       "prepare_beats_commit",
       "lower_commit_view_wins",
       "same_hash_conflicts",
       "different_hash_ignored",
       "frontier_skips_conflict",
       "frontier_accepts_same_height",
       "frontier_accepts_future_height",
       "certified_rejects_match",
       "certified_prepare_supersedes",
       "certified_missing_supersedes",
       "certified_wrong_hash_supersedes",
       "new_view_rejects_valid",
       "new_view_allows_equal_view",
       "new_view_ignores_lock",
       "new_view_ignores_conflict_recovery",
       "new_view_ignores_any_recovery",
       "new_view_allows_non_commit_highest",
       "new_view_allows_wrong_parent_height",
       "new_view_allows_nonlatest_highest",
       "new_view_allows_missing_qc",
       "new_view_ignores_qc_subject",
       "new_view_ignores_qc_height",
       "new_view_ignores_qc_epoch",
       "new_view_allows_below_min",
       "new_view_allows_above_proposal",
       "new_view_ignores_highest_link",
       "pending_skips_prepare",
       "pending_skips_commit",
       "pending_allows_future_view",
       "pending_ignores_height",
       "pending_ignores_epoch",
       "pending_accepts_non_vote_phase"
     }
  /\ checked = 0

SameHeightVoteConflictMatchesSpec ==
  /\ \A c \in SelectionCases:
       ActualSelectionOutput(c) = SpecSelectionOutput(c)
  /\ \A c \in SelectionCases:
       ActualConflictOutput(c) = SpecConflictOutput(c)
  /\ \A c \in FrontierCases:
       ActualFrontierOutput(c) = SpecFrontierOutput(c)
  /\ \A c \in CertifiedCases:
       ActualCertifiedOutput(c) = SpecCertifiedOutput(c)
  /\ \A c \in NewViewCases:
       ActualNewViewOutput(c) = SpecNewViewOutput(c)
  /\ \A c \in PendingVerificationCases:
       ActualPendingOutput(c) = SpecPendingOutput(c)

SafetyFast ==
  SameHeightVoteConflictMatchesSpec

BugSelectsNonlocal ==
  ActualSelectionOutput("wrong_signer_ignored") =
    SpecSelectionOutput("wrong_signer_ignored")

BugSelectsWrongHeight ==
  ActualSelectionOutput("wrong_height_ignored") =
    SpecSelectionOutput("wrong_height_ignored")

BugSelectsWrongEpoch ==
  ActualSelectionOutput("wrong_epoch_ignored") =
    SpecSelectionOutput("wrong_epoch_ignored")

BugSelectsNonVotePhase ==
  ActualSelectionOutput("non_vote_phase_ignored") =
    SpecSelectionOutput("non_vote_phase_ignored")

BugPrepareBeatsCommit ==
  ActualSelectionOutput("commit_beats_prepare") =
    SpecSelectionOutput("commit_beats_prepare")

BugLowerCommitViewWins ==
  ActualSelectionOutput("highest_commit_view") =
    SpecSelectionOutput("highest_commit_view")

BugSameHashConflicts ==
  ActualConflictOutput("single_prepare_same_hash") =
    SpecConflictOutput("single_prepare_same_hash")

BugDifferentHashIgnored ==
  ActualConflictOutput("single_prepare_conflicts") =
    SpecConflictOutput("single_prepare_conflicts")

BugFrontierSkipsConflict ==
  ActualFrontierOutput("frontier_conflict") =
    SpecFrontierOutput("frontier_conflict")

BugFrontierAcceptsSameHeight ==
  ActualFrontierOutput("same_height_not_frontier") =
    SpecFrontierOutput("same_height_not_frontier")

BugFrontierAcceptsFutureHeight ==
  ActualFrontierOutput("future_height_not_frontier") =
    SpecFrontierOutput("future_height_not_frontier")

BugCertifiedRejectsMatch ==
  ActualCertifiedOutput("certified_commit_match") =
    SpecCertifiedOutput("certified_commit_match")

BugCertifiedPrepareSupersedes ==
  ActualCertifiedOutput("certified_prepare_match") =
    SpecCertifiedOutput("certified_prepare_match")

BugCertifiedMissingSupersedes ==
  ActualCertifiedOutput("certified_missing") =
    SpecCertifiedOutput("certified_missing")

BugCertifiedWrongHashSupersedes ==
  ActualCertifiedOutput("certified_hash_mismatch") =
    SpecCertifiedOutput("certified_hash_mismatch")

BugNewViewRejectsValid ==
  ActualNewViewOutput("new_view_supersedes") =
    SpecNewViewOutput("new_view_supersedes")

BugNewViewAllowsEqualView ==
  ActualNewViewOutput("proposal_not_newer") =
    SpecNewViewOutput("proposal_not_newer")

BugNewViewIgnoresLock ==
  ActualNewViewOutput("locked_same_height") =
    SpecNewViewOutput("locked_same_height")

BugNewViewIgnoresConflictRecovery ==
  ActualNewViewOutput("recoverable_conflict_qc") =
    SpecNewViewOutput("recoverable_conflict_qc")

BugNewViewIgnoresAnyRecovery ==
  ActualNewViewOutput("recoverable_any_qc") =
    SpecNewViewOutput("recoverable_any_qc")

BugNewViewAllowsNonCommitHighest ==
  ActualNewViewOutput("highest_not_commit") =
    SpecNewViewOutput("highest_not_commit")

BugNewViewAllowsWrongParentHeight ==
  ActualNewViewOutput("highest_not_parent_height") =
    SpecNewViewOutput("highest_not_parent_height")

BugNewViewAllowsNonlatestHighest ==
  ActualNewViewOutput("highest_not_latest") =
    SpecNewViewOutput("highest_not_latest")

BugNewViewAllowsMissingQc ==
  ActualNewViewOutput("missing_new_view_qc") =
    SpecNewViewOutput("missing_new_view_qc")

BugNewViewIgnoresQcSubject ==
  ActualNewViewOutput("new_view_wrong_subject") =
    SpecNewViewOutput("new_view_wrong_subject")

BugNewViewIgnoresQcHeight ==
  ActualNewViewOutput("new_view_wrong_height") =
    SpecNewViewOutput("new_view_wrong_height")

BugNewViewIgnoresQcEpoch ==
  ActualNewViewOutput("new_view_wrong_epoch") =
    SpecNewViewOutput("new_view_wrong_epoch")

BugNewViewAllowsBelowMin ==
  ActualNewViewOutput("new_view_below_min") =
    SpecNewViewOutput("new_view_below_min")

BugNewViewAllowsAboveProposal ==
  ActualNewViewOutput("new_view_above_proposal") =
    SpecNewViewOutput("new_view_above_proposal")

BugNewViewIgnoresHighestLink ==
  ActualNewViewOutput("new_view_wrong_highest") =
    SpecNewViewOutput("new_view_wrong_highest")

BugPendingSkipsPrepare ==
  ActualPendingOutput("pending_prepare_at_view") =
    SpecPendingOutput("pending_prepare_at_view")

BugPendingSkipsCommit ==
  ActualPendingOutput("pending_commit_below_view") =
    SpecPendingOutput("pending_commit_below_view")

BugPendingAllowsFutureView ==
  ActualPendingOutput("pending_future_view") =
    SpecPendingOutput("pending_future_view")

BugPendingIgnoresHeight ==
  ActualPendingOutput("pending_wrong_height") =
    SpecPendingOutput("pending_wrong_height")

BugPendingIgnoresEpoch ==
  ActualPendingOutput("pending_wrong_epoch") =
    SpecPendingOutput("pending_wrong_epoch")

BugPendingAcceptsNonVotePhase ==
  ActualPendingOutput("pending_new_view_phase") =
    SpecPendingOutput("pending_new_view_phase")

====
