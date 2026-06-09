---- MODULE SumeragiValidationEvidenceQcGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `qc_for_validation_evidence(...)`.

Invalid-proposal evidence may borrow a QC only when the rejected block has a
parent hash, the candidate QC subject equals that parent, and the candidate QC
height is strictly below the rejected block height. Candidates are considered in
implementation order: highest QC, then locked QC, then latest committed QC.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "no_parent",
  "highest_match",
  "locked_match",
  "committed_match",
  "highest_and_locked_match",
  "locked_and_committed_match",
  "all_match",
  "highest_subject_mismatch_locked_match",
  "highest_height_equal_locked_match",
  "highest_height_above_committed_match",
  "committed_height_equal",
  "no_candidate_match"
}

ParentPresent(c) ==
  c # "no_parent"

HighestPresent(c) ==
  c \in {
    "no_parent",
    "highest_match",
    "highest_and_locked_match",
    "all_match",
    "highest_subject_mismatch_locked_match",
    "highest_height_equal_locked_match",
    "highest_height_above_committed_match",
    "no_candidate_match"
  }

LockedPresent(c) ==
  c \in {
    "locked_match",
    "highest_and_locked_match",
    "locked_and_committed_match",
    "all_match",
    "highest_subject_mismatch_locked_match",
    "highest_height_equal_locked_match"
  }

CommittedPresent(c) ==
  c \in {
    "committed_match",
    "locked_and_committed_match",
    "all_match",
    "highest_height_above_committed_match",
    "committed_height_equal"
  }

HighestSubjectMatches(c) ==
  c \notin {
    "highest_subject_mismatch_locked_match",
    "no_candidate_match"
  }

LockedSubjectMatches(c) ==
  TRUE

CommittedSubjectMatches(c) ==
  c # "no_candidate_match"

HighestHeightBelowBlock(c) ==
  c \notin {
    "highest_height_equal_locked_match",
    "highest_height_above_committed_match"
  }

LockedHeightBelowBlock(c) ==
  TRUE

CommittedHeightBelowBlock(c) ==
  c # "committed_height_equal"

HighestOk(c) ==
  ParentPresent(c)
    /\ HighestPresent(c)
    /\ HighestSubjectMatches(c)
    /\ HighestHeightBelowBlock(c)

LockedOk(c) ==
  ParentPresent(c)
    /\ LockedPresent(c)
    /\ LockedSubjectMatches(c)
    /\ LockedHeightBelowBlock(c)

CommittedOk(c) ==
  ParentPresent(c)
    /\ CommittedPresent(c)
    /\ CommittedSubjectMatches(c)
    /\ CommittedHeightBelowBlock(c)

SpecSelected(c) ==
  IF HighestOk(c) THEN
    "highest"
  ELSE IF LockedOk(c) THEN
    "locked"
  ELSE IF CommittedOk(c) THEN
    "committed"
  ELSE
    "none"

ActualSelected(c) ==
  CASE Bug = "accept_without_parent"
       /\ c = "no_parent" -> "highest"
    [] Bug = "ignore_subject_gate"
       /\ c = "highest_subject_mismatch_locked_match" -> "highest"
    [] Bug = "allow_equal_height"
       /\ c = "highest_height_equal_locked_match" -> "highest"
    [] Bug = "allow_future_height"
       /\ c = "highest_height_above_committed_match" -> "highest"
    [] Bug = "prefer_locked_over_highest"
       /\ c = "highest_and_locked_match" -> "locked"
    [] Bug = "prefer_committed_over_locked"
       /\ c = "locked_and_committed_match" -> "committed"
    [] Bug = "prefer_committed_over_all"
       /\ c = "all_match" -> "committed"
    [] Bug = "skip_highest"
       /\ c = "highest_match" -> "none"
    [] Bug = "skip_locked"
       /\ c = "locked_match" -> "none"
    [] Bug = "skip_committed"
       /\ c = "committed_match" -> "none"
    [] Bug = "synthesize_absent"
       /\ c = "no_candidate_match" -> "committed"
    [] Bug = "drop_after_highest_mismatch"
       /\ c = "highest_subject_mismatch_locked_match" -> "none"
    [] Bug = "drop_after_highest_stale"
       /\ c = "highest_height_equal_locked_match" -> "none"
    [] OTHER -> SpecSelected(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

ValidationEvidenceQcMatchesSpec ==
  \A c \in Cases: ActualSelected(c) = SpecSelected(c)

SafetyFast ==
  ValidationEvidenceQcMatchesSpec

BugAcceptWithoutParent ==
  ActualSelected("no_parent") = SpecSelected("no_parent")

BugIgnoreSubjectGate ==
  ActualSelected("highest_subject_mismatch_locked_match") =
    SpecSelected("highest_subject_mismatch_locked_match")

BugAllowEqualHeight ==
  ActualSelected("highest_height_equal_locked_match") =
    SpecSelected("highest_height_equal_locked_match")

BugAllowFutureHeight ==
  ActualSelected("highest_height_above_committed_match") =
    SpecSelected("highest_height_above_committed_match")

BugPreferLockedOverHighest ==
  ActualSelected("highest_and_locked_match") =
    SpecSelected("highest_and_locked_match")

BugPreferCommittedOverLocked ==
  ActualSelected("locked_and_committed_match") =
    SpecSelected("locked_and_committed_match")

BugPreferCommittedOverAll ==
  ActualSelected("all_match") = SpecSelected("all_match")

BugSkipHighest ==
  ActualSelected("highest_match") = SpecSelected("highest_match")

BugSkipLocked ==
  ActualSelected("locked_match") = SpecSelected("locked_match")

BugSkipCommitted ==
  ActualSelected("committed_match") = SpecSelected("committed_match")

BugSynthesizeAbsent ==
  ActualSelected("no_candidate_match") = SpecSelected("no_candidate_match")

BugDropAfterHighestMismatch ==
  ActualSelected("highest_subject_mismatch_locked_match") =
    SpecSelected("highest_subject_mismatch_locked_match")

BugDropAfterHighestStale ==
  ActualSelected("highest_height_equal_locked_match") =
    SpecSelected("highest_height_equal_locked_match")

====
