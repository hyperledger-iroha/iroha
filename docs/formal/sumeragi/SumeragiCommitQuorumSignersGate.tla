---- MODULE SumeragiCommitQuorumSignersGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for commit-QC signer quorum gating.

This slice models `has_commit_quorum_signers(...)` and the failed-commit caller
branch in `Actor::apply_commit_outcome(...)`. Missing QC signer metadata never
counts as a quorum; a present signer set counts exactly when its cardinality is
at least `min_votes_for_commit`. The failed-commit quorum branch is reachable
only on a commit failure after the helper has accepted the signer set.
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
  "missing_min_one_failure",
  "missing_min_zero_failure",
  "present_empty_min_zero_failure",
  "present_empty_min_one_failure",
  "below_min_failure",
  "boundary_failure",
  "above_min_failure",
  "boundary_success"
}

SignersPresent(c) ==
  ~(c \in {"missing_min_one_failure", "missing_min_zero_failure"})

CommitFailed(c) ==
  c # "boundary_success"

MinVotes(c) ==
  CASE c \in {"missing_min_zero_failure", "present_empty_min_zero_failure"} -> 0
    [] c \in {"missing_min_one_failure", "present_empty_min_one_failure"} -> 1
    [] OTHER -> 3

SignerCount(c) ==
  CASE ~SignersPresent(c) -> 0
    [] c \in {"present_empty_min_zero_failure", "present_empty_min_one_failure"} -> 0
    [] c = "below_min_failure" -> 2
    [] c \in {"boundary_failure", "boundary_success"} -> 3
    [] c = "above_min_failure" -> 4
    [] OTHER -> 0

SpecHasQuorumSigners(c) ==
  SignersPresent(c) /\ SignerCount(c) >= MinVotes(c)

ActualHasQuorumSigners(c) ==
  CASE Bug = "missing_signers_accepted"
       /\ ~SignersPresent(c)
       /\ MinVotes(c) > 0 -> TRUE
    [] Bug = "missing_zero_min_accepted"
       /\ ~SignersPresent(c)
       /\ MinVotes(c) = 0 -> TRUE
    [] Bug = "zero_min_present_rejected"
       /\ SignersPresent(c)
       /\ SignerCount(c) = 0
       /\ MinVotes(c) = 0 -> FALSE
    [] Bug = "empty_present_accepted"
       /\ SignersPresent(c)
       /\ SignerCount(c) = 0
       /\ MinVotes(c) > 0 -> TRUE
    [] Bug = "below_quorum_accepted"
       /\ SignersPresent(c)
       /\ SignerCount(c) > 0
       /\ SignerCount(c) < MinVotes(c) -> TRUE
    [] Bug = "boundary_rejected"
       /\ SignersPresent(c)
       /\ SignerCount(c) = MinVotes(c) -> FALSE
    [] Bug = "above_quorum_rejected"
       /\ SignersPresent(c)
       /\ SignerCount(c) > MinVotes(c) -> FALSE
    [] OTHER -> SpecHasQuorumSigners(c)

SpecFailedCommitQuorumBranch(c) ==
  CommitFailed(c) /\ SpecHasQuorumSigners(c)

ActualFailedCommitQuorumBranch(c) ==
  CASE Bug = "branch_without_failed_commit"
       /\ ActualHasQuorumSigners(c) -> TRUE
    [] OTHER -> CommitFailed(c) /\ ActualHasQuorumSigners(c)

\* @type: Str => <<Bool, Bool>>;
SpecCase(c) ==
  <<SpecHasQuorumSigners(c), SpecFailedCommitQuorumBranch(c)>>

\* @type: Str => <<Bool, Bool>>;
ActualCase(c) ==
  <<ActualHasQuorumSigners(c), ActualFailedCommitQuorumBranch(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in {
       "none",
       "missing_signers_accepted",
       "missing_zero_min_accepted",
       "zero_min_present_rejected",
       "empty_present_accepted",
       "below_quorum_accepted",
       "boundary_rejected",
       "above_quorum_rejected",
       "branch_without_failed_commit"
     }

SpecQuorumDecisionAnchors ==
  /\ ~SpecHasQuorumSigners("missing_min_one_failure")
  /\ ~SpecHasQuorumSigners("missing_min_zero_failure")
  /\ SpecHasQuorumSigners("present_empty_min_zero_failure")
  /\ ~SpecHasQuorumSigners("present_empty_min_one_failure")
  /\ ~SpecHasQuorumSigners("below_min_failure")
  /\ SpecHasQuorumSigners("boundary_failure")
  /\ SpecHasQuorumSigners("above_min_failure")
  /\ SpecHasQuorumSigners("boundary_success")

SpecFailedCommitBranchShape ==
  \A c \in Cases:
    SpecFailedCommitQuorumBranch(c) =
      (CommitFailed(c) /\ SpecHasQuorumSigners(c))

SpecFailedCommitBranchAnchors ==
  /\ ~SpecFailedCommitQuorumBranch("missing_min_one_failure")
  /\ ~SpecFailedCommitQuorumBranch("missing_min_zero_failure")
  /\ SpecFailedCommitQuorumBranch("present_empty_min_zero_failure")
  /\ ~SpecFailedCommitQuorumBranch("present_empty_min_one_failure")
  /\ ~SpecFailedCommitQuorumBranch("below_min_failure")
  /\ SpecFailedCommitQuorumBranch("boundary_failure")
  /\ SpecFailedCommitQuorumBranch("above_min_failure")
  /\ ~SpecFailedCommitQuorumBranch("boundary_success")

CommitQuorumSignersMatchesSpec ==
  \A c \in Cases: ActualCase(c) = SpecCase(c)

SafetyFast == CommitQuorumSignersMatchesSpec

BugMissingSignersAccepted ==
  ActualCase("missing_min_one_failure") = SpecCase("missing_min_one_failure")

BugMissingZeroMinAccepted ==
  ActualCase("missing_min_zero_failure") = SpecCase("missing_min_zero_failure")

BugZeroMinPresentRejected ==
  ActualCase("present_empty_min_zero_failure") =
    SpecCase("present_empty_min_zero_failure")

BugEmptyPresentAccepted ==
  ActualCase("present_empty_min_one_failure") =
    SpecCase("present_empty_min_one_failure")

BugBelowQuorumAccepted ==
  ActualCase("below_min_failure") = SpecCase("below_min_failure")

BugBoundaryRejected ==
  ActualCase("boundary_failure") = SpecCase("boundary_failure")

BugAboveQuorumRejected ==
  ActualCase("above_min_failure") = SpecCase("above_min_failure")

BugBranchWithoutFailedCommit ==
  ActualCase("boundary_success") = SpecCase("boundary_success")

====
