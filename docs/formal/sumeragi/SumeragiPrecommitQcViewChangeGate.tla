---- MODULE SumeragiPrecommitQcViewChangeGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for selecting the precommit QC used by pacemaker
view-change handling.

This slice models `precommit_qc_for_view_change(...)`. The helper filters the
local highest QC to Commit phase before comparing it with the latest committed
QC. When both candidates exist, the Commit-phase highest QC wins exactly when
its `(height, view)` pair is lexicographically greater than or equal to the
committed QC. Non-Commit highest QCs are ignored and fall back to committed QC
when one exists.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Str;
  selected,
  \* @type: Bool;
  highest_filtered_to_commit,
  \* @type: Bool;
  committed_fallback_used,
  \* @type: Bool;
  compared_height_view,
  \* @type: Bool;
  tie_prefers_highest

\* @type: <<Str, Str, Bool, Bool, Bool, Bool>>;
vars == <<candidate, selected, highest_filtered_to_commit,
  committed_fallback_used, compared_height_view, tie_prefers_highest>>

Cases == {
  "none_none",
  "highest_prepare_no_committed",
  "highest_commit_no_committed",
  "no_highest_committed",
  "highest_prepare_committed",
  "highest_commit_newer_height",
  "highest_commit_higher_height_lower_view",
  "highest_commit_same_height_newer_view",
  "highest_commit_equal_slot",
  "highest_commit_same_height_older_view",
  "highest_commit_older_height",
  "highest_commit_lower_height_higher_view"
}

HighestCommitCases == {
  "highest_commit_no_committed",
  "highest_commit_newer_height",
  "highest_commit_higher_height_lower_view",
  "highest_commit_same_height_newer_view",
  "highest_commit_equal_slot",
  "highest_commit_same_height_older_view",
  "highest_commit_older_height",
  "highest_commit_lower_height_higher_view"
}

HighestNonCommitCases == {
  "highest_prepare_no_committed",
  "highest_prepare_committed"
}

CommittedPresentCases == {
  "no_highest_committed",
  "highest_prepare_committed",
  "highest_commit_newer_height",
  "highest_commit_higher_height_lower_view",
  "highest_commit_same_height_newer_view",
  "highest_commit_equal_slot",
  "highest_commit_same_height_older_view",
  "highest_commit_older_height",
  "highest_commit_lower_height_higher_view"
}

BothCommitCandidatesCases ==
  HighestCommitCases \intersect CommittedPresentCases

HighestWinsCases == {
  "highest_commit_no_committed",
  "highest_commit_newer_height",
  "highest_commit_higher_height_lower_view",
  "highest_commit_same_height_newer_view",
  "highest_commit_equal_slot"
}

CommittedWinsCases == {
  "no_highest_committed",
  "highest_prepare_committed",
  "highest_commit_same_height_older_view",
  "highest_commit_older_height",
  "highest_commit_lower_height_higher_view"
}

NoSelectionCases == {
  "none_none",
  "highest_prepare_no_committed"
}

TieCases == {"highest_commit_equal_slot"}
HigherHeightLowerViewCases == {"highest_commit_higher_height_lower_view"}
LowerHeightHigherViewCases == {"highest_commit_lower_height_higher_view"}

SpecSelected(c) ==
  IF c \in HighestWinsCases THEN "highest"
  ELSE IF c \in CommittedWinsCases THEN "committed"
  ELSE "none"

SpecHighestFiltered(c) == c \in HighestCommitCases

SpecCommittedFallback(c) == c \in CommittedWinsCases

SpecComparedHeightView(c) == c \in BothCommitCandidatesCases

SpecTiePrefersHighest(c) == c \in TieCases

ActualSelected(c) ==
  IF Bug = "select_committed_when_none" /\ c = "none_none" THEN "committed"
  ELSE IF Bug = "select_non_commit_highest_without_committed"
      /\ c = "highest_prepare_no_committed" THEN "highest"
  ELSE IF Bug = "select_non_commit_highest_over_committed"
      /\ c = "highest_prepare_committed" THEN "highest"
  ELSE IF Bug = "skip_highest_without_committed"
      /\ c = "highest_commit_no_committed" THEN "none"
  ELSE IF Bug = "skip_committed_without_highest"
      /\ c = "no_highest_committed" THEN "none"
  ELSE IF Bug = "committed_over_newer_height"
      /\ c = "highest_commit_newer_height" THEN "committed"
  ELSE IF Bug = "committed_over_higher_height_lower_view"
      /\ c = "highest_commit_higher_height_lower_view" THEN "committed"
  ELSE IF Bug = "committed_over_same_height_newer_view"
      /\ c = "highest_commit_same_height_newer_view" THEN "committed"
  ELSE IF Bug = "committed_over_equal_slot_highest"
      /\ c = "highest_commit_equal_slot" THEN "committed"
  ELSE IF Bug = "highest_over_same_height_older_view"
      /\ c = "highest_commit_same_height_older_view" THEN "highest"
  ELSE IF Bug = "highest_over_older_height"
      /\ c = "highest_commit_older_height" THEN "highest"
  ELSE IF Bug = "highest_over_lower_height_higher_view"
      /\ c = "highest_commit_lower_height_higher_view" THEN "highest"
  ELSE IF Bug = "comparison_uses_view_before_height"
      /\ c = "highest_commit_higher_height_lower_view" THEN "committed"
  ELSE IF Bug = "comparison_uses_view_before_height"
      /\ c = "highest_commit_lower_height_higher_view" THEN "highest"
  ELSE IF Bug = "select_none_when_both_commit"
      /\ c \in BothCommitCandidatesCases THEN "none"
  ELSE SpecSelected(c)

ActualHighestFiltered(c) ==
  \/ /\ SpecHighestFiltered(c)
     /\ Bug # "drop_commit_highest_filter"
  \/ /\ c \in HighestNonCommitCases
     /\ Bug = "accept_non_commit_filter"

ActualCommittedFallback(c) ==
  /\ SpecCommittedFallback(c)
  /\ Bug # "skip_committed_fallback"

ActualComparedHeightView(c) ==
  /\ SpecComparedHeightView(c)
  /\ Bug # "skip_height_view_comparison"

ActualTiePrefersHighest(c) ==
  /\ SpecTiePrefersHighest(c)
  /\ Bug # "tie_does_not_prefer_highest"

Init ==
  /\ candidate = "none"
  /\ selected = "none"
  /\ highest_filtered_to_commit = FALSE
  /\ committed_fallback_used = FALSE
  /\ compared_height_view = FALSE
  /\ tie_prefers_highest = FALSE

CheckCase(c) ==
  /\ candidate = "none"
  /\ candidate' = c
  /\ selected' = ActualSelected(c)
  /\ highest_filtered_to_commit' = ActualHighestFiltered(c)
  /\ committed_fallback_used' = ActualCommittedFallback(c)
  /\ compared_height_view' = ActualComparedHeightView(c)
  /\ tie_prefers_highest' = ActualTiePrefersHighest(c)

Next ==
  \/ \E c \in Cases : CheckCase(c)
  \/ /\ candidate # "none"
     /\ UNCHANGED vars

TypeInvariant ==
  /\ candidate \in Cases \union {"none"}
  /\ selected \in {"none", "highest", "committed"}
  /\ highest_filtered_to_commit \in BOOLEAN
  /\ committed_fallback_used \in BOOLEAN
  /\ compared_height_view \in BOOLEAN
  /\ tie_prefers_highest \in BOOLEAN

SelectionMatchesSpec ==
  candidate = "none" \/ selected = SpecSelected(candidate)

HighestFilterMatchesSpec ==
  candidate = "none" \/
    highest_filtered_to_commit = SpecHighestFiltered(candidate)

CommittedFallbackMatchesSpec ==
  candidate = "none" \/
    committed_fallback_used = SpecCommittedFallback(candidate)

HeightViewComparisonMatchesSpec ==
  candidate = "none" \/
    compared_height_view = SpecComparedHeightView(candidate)

TiePreferenceMatchesSpec ==
  candidate = "none" \/ tie_prefers_highest = SpecTiePrefersHighest(candidate)

NonCommitHighestNeverSelected ==
  candidate \in HighestNonCommitCases => selected # "highest"

HighestSelectedOnlyFromCommitHighest ==
  selected = "highest" => candidate \in HighestWinsCases

CommittedSelectedOnlyWhenPresent ==
  selected = "committed" => candidate \in CommittedPresentCases

BothCommitCandidatesUseHeightViewComparison ==
  candidate \in BothCommitCandidatesCases => compared_height_view

HigherHeightBeatsLowerView ==
  candidate \in HigherHeightLowerViewCases => selected = "highest"

LowerHeightLosesDespiteHigherView ==
  candidate \in LowerHeightHigherViewCases => selected = "committed"

EqualHeightViewPrefersHighest ==
  candidate \in TieCases =>
    /\ selected = "highest"
    /\ tie_prefers_highest

CommittedFallbackOnlyAfterNoCommitHighestWins ==
  committed_fallback_used => selected = "committed"

NoCandidateSelectsNone ==
  candidate \in NoSelectionCases => selected = "none"

PrecommitQcSelectionExact ==
  /\ SelectionMatchesSpec
  /\ NonCommitHighestNeverSelected
  /\ HighestSelectedOnlyFromCommitHighest
  /\ CommittedSelectedOnlyWhenPresent
  /\ NoCandidateSelectsNone

PrecommitQcFilterFallbackExact ==
  /\ HighestFilterMatchesSpec
  /\ CommittedFallbackMatchesSpec
  /\ CommittedFallbackOnlyAfterNoCommitHighestWins

PrecommitQcOrderingExact ==
  /\ HeightViewComparisonMatchesSpec
  /\ TiePreferenceMatchesSpec
  /\ BothCommitCandidatesUseHeightViewComparison
  /\ HigherHeightBeatsLowerView
  /\ LowerHeightLosesDespiteHigherView
  /\ EqualHeightViewPrefersHighest

PrecommitQcViewChangeExactness ==
  /\ PrecommitQcSelectionExact
  /\ PrecommitQcFilterFallbackExact
  /\ PrecommitQcOrderingExact

PrecommitQcViewChangeCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ PrecommitQcViewChangeExactness

Safety ==
  PrecommitQcViewChangeExactness

=============================================================================
====
