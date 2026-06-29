---- MODULE SumeragiLateNewViewEmissionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for late NEW_VIEW emission.

This slice models `maybe_emit_late_new_view_vote_to_complete_near_quorum(...)`.
The helper may emit a local NEW_VIEW vote only for the committed frontier,
within the configured future-view window, with a local validator index present
in the view-aligned signature topology. Candidate groups must target the same
highest-QC subject, exclude the local signer, and either complete the quorum
after adding the local vote or satisfy the active-frontier catch-up fallback.

Permissioned completion must count only voting signers, so a non-voting local
index cannot complete quorum. NPoS completion requires stake quorum, while a
missing stake roster or stake-quorum error fails closed and an unmappable group
does not prevent later viable groups from being selected. When multiple groups
are viable, completing candidates outrank catch-up candidates; ties use
highest-QC rank and then support size in the Rust helper. The final return
value must still respect the inner `emit_new_view_vote_to_complete_near_quorum`
result.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoSelection == "none"
CompleteLow == "complete_low"
CompleteHigh == "complete_high"
CatchUpLow == "catchup_low"
CatchUpHigh == "catchup_high"
UnderPermissioned == "under_permissioned"
PaddingPermissioned == "padding_permissioned"
NposComplete == "npos_complete"
NposMissingRoster == "npos_missing_roster"
NposMapError == "npos_map_error"
NposStakeError == "npos_stake_error"
InvalidSubject == "invalid_subject"
LocalSigned == "local_signed"
NonSuperseding == "nonsuperseding"

Candidates == {
  NoSelection,
  CompleteLow,
  CompleteHigh,
  CatchUpLow,
  CatchUpHigh,
  UnderPermissioned,
  PaddingPermissioned,
  NposComplete,
  NposMissingRoster,
  NposMapError,
  NposStakeError,
  InvalidSubject,
  LocalSigned,
  NonSuperseding
}

Cases == {
  "permissioned_completes",
  "permissioned_exact_group_quorum",
  "permissioned_under_quorum",
  "permissioned_local_padding",
  "npos_stake_completes",
  "npos_missing_stake_roster",
  "npos_signer_map_error_then_valid",
  "npos_stake_error",
  "frontier_catch_up",
  "non_frontier_height",
  "far_future_view",
  "future_window_disabled",
  "local_missing",
  "subject_mismatch",
  "local_already_signed",
  "existing_vote_not_superseded",
  "existing_vote_superseded",
  "select_completion_over_catchup",
  "select_higher_rank_completion",
  "emit_failure"
}

FrontierHeight(c) ==
  c # "non_frontier_height"

ViewAllowed(c) ==
  c # "far_future_view"

LocalIndexKnown(c) ==
  c # "local_missing"

TopLevelOk(c) ==
  /\ FrontierHeight(c)
  /\ ViewAllowed(c)
  /\ LocalIndexKnown(c)

InnerEmitOk(c) ==
  c # "emit_failure"

CandidateSet(c) ==
  CASE c = "permissioned_completes" -> {CompleteLow}
    [] c = "permissioned_exact_group_quorum" -> {CompleteLow}
    [] c = "permissioned_under_quorum" -> {UnderPermissioned}
    [] c = "permissioned_local_padding" -> {PaddingPermissioned}
    [] c = "npos_stake_completes" -> {NposComplete}
    [] c = "npos_missing_stake_roster" -> {NposMissingRoster}
    [] c = "npos_signer_map_error_then_valid" -> {NposMapError, NposComplete}
    [] c = "npos_stake_error" -> {NposStakeError}
    [] c = "frontier_catch_up" -> {CatchUpLow}
    [] c = "subject_mismatch" -> {InvalidSubject}
    [] c = "local_already_signed" -> {LocalSigned}
    [] c = "existing_vote_not_superseded" -> {NonSuperseding}
    [] c = "existing_vote_superseded" -> {CompleteLow}
    [] c = "select_completion_over_catchup" -> {CompleteLow, CatchUpHigh}
    [] c = "select_higher_rank_completion" -> {CompleteLow, CompleteHigh}
    [] OTHER -> {CompleteLow}

SubjectMatches(candidate) ==
  candidate # InvalidSubject

LocalAbsent(candidate) ==
  candidate # LocalSigned

ExistingVoteAllows(candidate) ==
  candidate # NonSuperseding

CandidateCompletes(candidate) ==
  candidate \in {CompleteLow, CompleteHigh, NposComplete}

CandidateCatchUp(candidate) ==
  candidate \in {CatchUpLow, CatchUpHigh}

SpecHas(c, candidate) ==
  /\ TopLevelOk(c)
  /\ candidate \in CandidateSet(c)
  /\ SubjectMatches(candidate)
  /\ LocalAbsent(candidate)
  /\ ExistingVoteAllows(candidate)
  /\ (CandidateCompletes(candidate) \/ CandidateCatchUp(candidate))

SpecSelected(c) ==
  IF SpecHas(c, CompleteHigh) THEN CompleteHigh
  ELSE IF SpecHas(c, CompleteLow) THEN CompleteLow
  ELSE IF SpecHas(c, NposComplete) THEN NposComplete
  ELSE IF SpecHas(c, CatchUpHigh) THEN CatchUpHigh
  ELSE IF SpecHas(c, CatchUpLow) THEN CatchUpLow
  ELSE NoSelection

ActualSelected(c) ==
  CASE Bug = "reject_permissioned_completion"
       /\ c = "permissioned_completes" -> NoSelection
    [] Bug = "reject_permissioned_exact_group_quorum"
       /\ c = "permissioned_exact_group_quorum" -> NoSelection
    [] Bug = "accept_permissioned_under_quorum"
       /\ c = "permissioned_under_quorum" -> UnderPermissioned
    [] Bug = "accept_permissioned_padding_completion"
       /\ c = "permissioned_local_padding" -> PaddingPermissioned
    [] Bug = "reject_npos_stake_completion"
       /\ c = "npos_stake_completes" -> NoSelection
    [] Bug = "accept_npos_missing_stake_roster"
       /\ c = "npos_missing_stake_roster" -> NposMissingRoster
    [] Bug = "stop_on_npos_signer_map_error"
       /\ c = "npos_signer_map_error_then_valid" -> NoSelection
    [] Bug = "accept_npos_stake_error"
       /\ c = "npos_stake_error" -> NposStakeError
    [] Bug = "reject_frontier_catch_up"
       /\ c = "frontier_catch_up" -> NoSelection
    [] Bug = "accept_non_frontier_height"
       /\ c = "non_frontier_height" -> CompleteLow
    [] Bug = "accept_far_future_view"
       /\ c = "far_future_view" -> CompleteLow
    [] Bug = "reject_future_window_disabled"
       /\ c = "future_window_disabled" -> NoSelection
    [] Bug = "accept_missing_local_index"
       /\ c = "local_missing" -> CompleteLow
    [] Bug = "accept_subject_mismatch"
       /\ c = "subject_mismatch" -> InvalidSubject
    [] Bug = "accept_local_already_signed"
       /\ c = "local_already_signed" -> LocalSigned
    [] Bug = "accept_unsuperseded_existing_vote"
       /\ c = "existing_vote_not_superseded" -> NonSuperseding
    [] Bug = "reject_superseding_existing_vote"
       /\ c = "existing_vote_superseded" -> NoSelection
    [] Bug = "prefer_catchup_over_completion"
       /\ c = "select_completion_over_catchup" -> CatchUpHigh
    [] Bug = "prefer_lower_rank_completion"
       /\ c = "select_higher_rank_completion" -> CompleteLow
    [] OTHER -> SpecSelected(c)

SpecEmitted(c) ==
  SpecSelected(c) # NoSelection /\ InnerEmitOk(c)

ActualEmitted(c) ==
  IF Bug = "ignore_inner_emit_failure" /\ c = "emit_failure"
  THEN ActualSelected(c) # NoSelection
  ELSE ActualSelected(c) # NoSelection /\ InnerEmitOk(c)

SelectedMatches(c) ==
  ActualSelected(c) = SpecSelected(c)

OutputMatches(c) ==
  ActualEmitted(c) = SpecEmitted(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "reject_permissioned_completion",
       "reject_permissioned_exact_group_quorum",
       "accept_permissioned_under_quorum",
       "accept_permissioned_padding_completion",
       "reject_npos_stake_completion",
       "accept_npos_missing_stake_roster",
       "stop_on_npos_signer_map_error",
       "accept_npos_stake_error",
       "reject_frontier_catch_up",
       "accept_non_frontier_height",
       "accept_far_future_view",
       "reject_future_window_disabled",
       "accept_missing_local_index",
       "accept_subject_mismatch",
       "accept_local_already_signed",
       "accept_unsuperseded_existing_vote",
       "reject_superseding_existing_vote",
       "prefer_catchup_over_completion",
       "prefer_lower_rank_completion",
       "ignore_inner_emit_failure"
     }
  /\ checked = 0

SelectionMatchesSpec ==
  \A c \in Cases:
    SelectedMatches(c)

OutputMatchesSpec ==
  \A c \in Cases:
    OutputMatches(c)

LateNewViewEmissionExactness ==
  /\ SelectionMatchesSpec
  /\ OutputMatchesSpec

FrontierViewAndLocalGates ==
  /\ SelectedMatches("non_frontier_height")
  /\ SelectedMatches("far_future_view")
  /\ SelectedMatches("future_window_disabled")
  /\ SelectedMatches("local_missing")

PermissionedCompletion ==
  /\ SelectedMatches("permissioned_completes")
  /\ SelectedMatches("permissioned_exact_group_quorum")
  /\ SelectedMatches("permissioned_under_quorum")
  /\ SelectedMatches("permissioned_local_padding")

NposCompletion ==
  /\ SelectedMatches("npos_stake_completes")
  /\ SelectedMatches("npos_missing_stake_roster")
  /\ SelectedMatches("npos_signer_map_error_then_valid")
  /\ SelectedMatches("npos_stake_error")

SameSlotAndSubjectGuards ==
  /\ SelectedMatches("subject_mismatch")
  /\ SelectedMatches("local_already_signed")
  /\ SelectedMatches("existing_vote_not_superseded")
  /\ SelectedMatches("existing_vote_superseded")

SelectionOrder ==
  /\ SelectedMatches("frontier_catch_up")
  /\ SelectedMatches("select_completion_over_catchup")
  /\ SelectedMatches("select_higher_rank_completion")

InnerEmissionResult ==
  OutputMatches("emit_failure")

Safety ==
  LateNewViewEmissionExactness

LateNewViewEmissionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ LateNewViewEmissionExactness

=============================================================================
====
