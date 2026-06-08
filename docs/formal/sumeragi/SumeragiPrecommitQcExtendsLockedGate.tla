---- MODULE SumeragiPrecommitQcExtendsLockedGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the actor-level precommit-QC locked-chain gate.

This slice models `precommit_qc_extends_locked(...)`. The wrapper allows
non-Commit phases, absent locks, and locks whose block payload is not locally
known before constructing the candidate Commit QC and delegating to
`qc_satisfies_locked_with_lookup(...)`. Once the checked path is reached, the
candidate is accepted only by newer-view bypass, exact locked hash, or explicit
parent-chain extension; same-view missing, divergent, regressed, or conflicting
parent evidence is rejected and emits the skip-aggregation warning.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NonCommitPhaseAllows == "non_commit_phase_allows"
CommitNoLockAllows == "commit_no_lock_allows"
CommitMissingLockedBlockAllows == "commit_missing_locked_block_allows"
CommitSameHashAllows == "commit_same_hash_allows"
CommitExtendsParentAllows == "commit_extends_parent_allows"
CommitExtendsGrandparentAllows == "commit_extends_grandparent_allows"
CommitSameViewMissingParentRejects ==
  "commit_same_view_missing_parent_rejects"
CommitSameViewDivergentParentRejects ==
  "commit_same_view_divergent_parent_rejects"
CommitSameViewHeightRegressionRejects ==
  "commit_same_view_height_regression_rejects"
CommitSameHeightWrongHashRejects ==
  "commit_same_height_wrong_hash_rejects"
CommitNewerViewDivergentAllows == "commit_newer_view_divergent_allows"
CommitNewerViewHeightRegressionAllows ==
  "commit_newer_view_height_regression_allows"

Cases == {
  NonCommitPhaseAllows,
  CommitNoLockAllows,
  CommitMissingLockedBlockAllows,
  CommitSameHashAllows,
  CommitExtendsParentAllows,
  CommitExtendsGrandparentAllows,
  CommitSameViewMissingParentRejects,
  CommitSameViewDivergentParentRejects,
  CommitSameViewHeightRegressionRejects,
  CommitSameHeightWrongHashRejects,
  CommitNewerViewDivergentAllows,
  CommitNewerViewHeightRegressionAllows
}

CommitPhase == 1
NonCommitPhase == 2
NoLockedQc == 3
LockedQcPresent == 4
LockedBlockKnown == 5
LockedBlockMissing == 6
CandidateCommitQcBuilt == 7
StructuralLockCheck == 8
ExactLockedHash == 9
ParentLookup == 10
GrandparentLookup == 11
ExtendsLockedChain == 12
MissingParentRejected == 13
DivergentParentRejected == 14
HeightRegressionRejected == 15
SameHeightWrongHashRejected == 16
NewerViewBypass == 17
AllowAggregation == 18
RejectAggregation == 19
SkipAggregationWarning == 20

Actions == 1..20

SpecActions(c) ==
  CASE c = NonCommitPhaseAllows ->
      {NonCommitPhase, AllowAggregation}
    [] c = CommitNoLockAllows ->
      {CommitPhase, NoLockedQc, AllowAggregation}
    [] c = CommitMissingLockedBlockAllows ->
      {CommitPhase, LockedQcPresent, LockedBlockMissing, AllowAggregation}
    [] c = CommitSameHashAllows ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateCommitQcBuilt, StructuralLockCheck, ExactLockedHash,
       AllowAggregation}
    [] c = CommitExtendsParentAllows ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateCommitQcBuilt, StructuralLockCheck, ParentLookup,
       ExtendsLockedChain, AllowAggregation}
    [] c = CommitExtendsGrandparentAllows ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateCommitQcBuilt, StructuralLockCheck, ParentLookup,
       GrandparentLookup, ExtendsLockedChain, AllowAggregation}
    [] c = CommitSameViewMissingParentRejects ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateCommitQcBuilt, StructuralLockCheck, ParentLookup,
       MissingParentRejected, RejectAggregation, SkipAggregationWarning}
    [] c = CommitSameViewDivergentParentRejects ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateCommitQcBuilt, StructuralLockCheck, ParentLookup,
       DivergentParentRejected, RejectAggregation, SkipAggregationWarning}
    [] c = CommitSameViewHeightRegressionRejects ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateCommitQcBuilt, StructuralLockCheck, HeightRegressionRejected,
       RejectAggregation, SkipAggregationWarning}
    [] c = CommitSameHeightWrongHashRejects ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateCommitQcBuilt, StructuralLockCheck,
       SameHeightWrongHashRejected, RejectAggregation, SkipAggregationWarning}
    [] c = CommitNewerViewDivergentAllows ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateCommitQcBuilt, NewerViewBypass, AllowAggregation}
    [] c = CommitNewerViewHeightRegressionAllows ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateCommitQcBuilt, NewerViewBypass, AllowAggregation}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "reject_non_commit_phase"
       /\ c = NonCommitPhaseAllows ->
      (spec \ {AllowAggregation}) \cup {RejectAggregation}
    [] Bug = "reject_no_lock"
       /\ c = CommitNoLockAllows ->
      (spec \ {AllowAggregation}) \cup {RejectAggregation}
    [] Bug = "reject_missing_locked_block"
       /\ c = CommitMissingLockedBlockAllows ->
      (spec \ {AllowAggregation}) \cup {RejectAggregation}
    [] Bug = "reject_same_hash"
       /\ c = CommitSameHashAllows ->
      (spec \ {AllowAggregation}) \cup {RejectAggregation}
    [] Bug = "reject_parent_extension"
       /\ c = CommitExtendsParentAllows ->
      (spec \ {AllowAggregation}) \cup {RejectAggregation}
    [] Bug = "reject_grandparent_extension"
       /\ c = CommitExtendsGrandparentAllows ->
      (spec \ {AllowAggregation}) \cup {RejectAggregation}
    [] Bug = "accept_missing_parent"
       /\ c = CommitSameViewMissingParentRejects ->
      (spec \ {MissingParentRejected, RejectAggregation,
        SkipAggregationWarning}) \cup {AllowAggregation}
    [] Bug = "accept_divergent_parent"
       /\ c = CommitSameViewDivergentParentRejects ->
      (spec \ {DivergentParentRejected, RejectAggregation,
        SkipAggregationWarning}) \cup {AllowAggregation}
    [] Bug = "accept_height_regression"
       /\ c = CommitSameViewHeightRegressionRejects ->
      (spec \ {HeightRegressionRejected, RejectAggregation,
        SkipAggregationWarning}) \cup {AllowAggregation}
    [] Bug = "accept_same_height_wrong_hash"
       /\ c = CommitSameHeightWrongHashRejects ->
      (spec \ {SameHeightWrongHashRejected, RejectAggregation,
        SkipAggregationWarning}) \cup {AllowAggregation}
    [] Bug = "reject_newer_view_divergent"
       /\ c = CommitNewerViewDivergentAllows ->
      (spec \ {NewerViewBypass, AllowAggregation}) \cup
        {StructuralLockCheck, ParentLookup, DivergentParentRejected,
         RejectAggregation}
    [] Bug = "reject_newer_view_height_regression"
       /\ c = CommitNewerViewHeightRegressionAllows ->
      (spec \ {NewerViewBypass, AllowAggregation}) \cup
        {StructuralLockCheck, HeightRegressionRejected, RejectAggregation}
    [] Bug = "skip_warning_on_reject"
       /\ c = CommitSameViewDivergentParentRejects ->
      spec \ {SkipAggregationWarning}
    [] Bug = "warn_on_non_commit"
       /\ c = NonCommitPhaseAllows ->
      spec \cup {SkipAggregationWarning}
    [] Bug = "warn_on_allowed_extension"
       /\ c = CommitExtendsParentAllows ->
      spec \cup {SkipAggregationWarning}
    [] OTHER -> spec

Bugs == {
  "none",
  "reject_non_commit_phase",
  "reject_no_lock",
  "reject_missing_locked_block",
  "reject_same_hash",
  "reject_parent_extension",
  "reject_grandparent_extension",
  "accept_missing_parent",
  "accept_divergent_parent",
  "accept_height_regression",
  "accept_same_height_wrong_hash",
  "reject_newer_view_divergent",
  "reject_newer_view_height_regression",
  "skip_warning_on_reject",
  "warn_on_non_commit",
  "warn_on_allowed_extension"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

PrecommitQcExtendsLockedCoreSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

NoBugInvariant == PrecommitQcExtendsLockedCoreSafety

SafetyFast == PrecommitQcExtendsLockedCoreSafety

BugRejectNonCommitPhase == NoBugInvariant
BugRejectNoLock == NoBugInvariant
BugRejectMissingLockedBlock == NoBugInvariant
BugRejectSameHash == NoBugInvariant
BugRejectParentExtension == NoBugInvariant
BugRejectGrandparentExtension == NoBugInvariant
BugAcceptMissingParent == NoBugInvariant
BugAcceptDivergentParent == NoBugInvariant
BugAcceptHeightRegression == NoBugInvariant
BugAcceptSameHeightWrongHash == NoBugInvariant
BugRejectNewerViewDivergent == NoBugInvariant
BugRejectNewerViewHeightRegression == NoBugInvariant
BugSkipWarningOnReject == NoBugInvariant
BugWarnOnNonCommit == NoBugInvariant
BugWarnOnAllowedExtension == NoBugInvariant

====
