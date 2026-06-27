---- MODULE SumeragiDropPrecommitVoteForLockGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for precommit-vote lock filtering.

This slice models `drop_precommit_vote_for_lock(...)`. The helper ignores
non-Commit votes and unlocked actors, drops Commit votes below the locked
height, drops same-height conflicts before any payload lookup, permits votes
while the locked payload or candidate payload is unavailable, and checks the
locked chain only when both payloads are locally known. Checked same-view
non-extension drops record both consensus handling and vote-validation drop
status, while below-height drops record only consensus handling.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NonCommitAllows == "non_commit_allows"
CommitNoLockAllows == "commit_no_lock_allows"
CommitBelowLockDrops == "commit_below_lock_drops"
CommitSameHashLockMissingAllows == "commit_same_hash_lock_missing_allows"
CommitSameHeightSameHashAllows == "commit_same_height_same_hash_allows"
CommitSameHeightConflictDrops == "commit_same_height_conflict_drops"
CommitSameHeightNewerViewConflictDrops ==
  "commit_same_height_newer_view_conflict_drops"
CommitLockPayloadMissingHigherAllows ==
  "commit_lock_payload_missing_higher_allows"
CommitCandidatePayloadMissingHigherAllows ==
  "commit_candidate_payload_missing_higher_allows"
CommitHigherSameViewParentAllows == "commit_higher_same_view_parent_allows"
CommitHigherSameViewGrandparentAllows ==
  "commit_higher_same_view_grandparent_allows"
CommitHigherSameViewMissingParentDrops ==
  "commit_higher_same_view_missing_parent_drops"
CommitHigherSameViewDivergentParentDrops ==
  "commit_higher_same_view_divergent_parent_drops"
CommitHigherNewerViewDivergentAllows ==
  "commit_higher_newer_view_divergent_allows"

Cases == {
  NonCommitAllows,
  CommitNoLockAllows,
  CommitBelowLockDrops,
  CommitSameHashLockMissingAllows,
  CommitSameHeightSameHashAllows,
  CommitSameHeightConflictDrops,
  CommitSameHeightNewerViewConflictDrops,
  CommitLockPayloadMissingHigherAllows,
  CommitCandidatePayloadMissingHigherAllows,
  CommitHigherSameViewParentAllows,
  CommitHigherSameViewGrandparentAllows,
  CommitHigherSameViewMissingParentDrops,
  CommitHigherSameViewDivergentParentDrops,
  CommitHigherNewerViewDivergentAllows
}

CommitPhase == 1
NonCommitPhase == 2
NoLockedQc == 3
LockedQcPresent == 4
BelowLockedHeight == 5
SameHeightSameHash == 6
SameHeightConflict == 7
LockedBlockKnown == 8
LockedBlockMissing == 9
CandidateBlockKnown == 10
CandidateBlockMissing == 11
CandidateCommitRefBuilt == 12
ParentLookup == 13
GrandparentLookup == 14
ExtendsLockedChain == 15
MissingParentRejected == 16
DivergentParentRejected == 17
NewerViewBypass == 18
AllowVote == 19
DropVote == 20
RecordConsensusDrop == 21
RecordValidationDrop == 22

Actions == 1..22

SpecActions(c) ==
  CASE c = NonCommitAllows ->
      {NonCommitPhase, AllowVote}
    [] c = CommitNoLockAllows ->
      {CommitPhase, NoLockedQc, AllowVote}
    [] c = CommitBelowLockDrops ->
      {CommitPhase, LockedQcPresent, BelowLockedHeight, DropVote,
       RecordConsensusDrop}
    [] c = CommitSameHashLockMissingAllows ->
      {CommitPhase, LockedQcPresent, SameHeightSameHash,
       LockedBlockMissing, AllowVote}
    [] c = CommitSameHeightSameHashAllows ->
      {CommitPhase, LockedQcPresent, SameHeightSameHash,
       LockedBlockKnown, CandidateBlockKnown, CandidateCommitRefBuilt,
       ExtendsLockedChain, AllowVote}
    [] c = CommitSameHeightConflictDrops ->
      {CommitPhase, LockedQcPresent, SameHeightConflict, DropVote,
       RecordConsensusDrop, RecordValidationDrop}
    [] c = CommitSameHeightNewerViewConflictDrops ->
      {CommitPhase, LockedQcPresent, SameHeightConflict, NewerViewBypass,
       DropVote, RecordConsensusDrop, RecordValidationDrop}
    [] c = CommitLockPayloadMissingHigherAllows ->
      {CommitPhase, LockedQcPresent, LockedBlockMissing, AllowVote}
    [] c = CommitCandidatePayloadMissingHigherAllows ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateBlockMissing, AllowVote}
    [] c = CommitHigherSameViewParentAllows ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateBlockKnown, CandidateCommitRefBuilt, ParentLookup,
       ExtendsLockedChain, AllowVote}
    [] c = CommitHigherSameViewGrandparentAllows ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateBlockKnown, CandidateCommitRefBuilt, ParentLookup,
       GrandparentLookup, ExtendsLockedChain, AllowVote}
    [] c = CommitHigherSameViewMissingParentDrops ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateBlockKnown, CandidateCommitRefBuilt, ParentLookup,
       MissingParentRejected, DropVote, RecordConsensusDrop,
       RecordValidationDrop}
    [] c = CommitHigherSameViewDivergentParentDrops ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateBlockKnown, CandidateCommitRefBuilt, ParentLookup,
       DivergentParentRejected, DropVote, RecordConsensusDrop,
       RecordValidationDrop}
    [] c = CommitHigherNewerViewDivergentAllows ->
      {CommitPhase, LockedQcPresent, LockedBlockKnown,
       CandidateBlockKnown, CandidateCommitRefBuilt, NewerViewBypass,
       AllowVote}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "drop_non_commit"
       /\ c = NonCommitAllows ->
      (spec \ {AllowVote}) \cup {DropVote, RecordConsensusDrop}
    [] Bug = "drop_no_lock"
       /\ c = CommitNoLockAllows ->
      (spec \ {AllowVote}) \cup {DropVote, RecordConsensusDrop}
    [] Bug = "allow_below_locked_height"
       /\ c = CommitBelowLockDrops ->
      (spec \ {BelowLockedHeight, DropVote, RecordConsensusDrop}) \cup
        {AllowVote}
    [] Bug = "record_validation_drop_below_height"
       /\ c = CommitBelowLockDrops ->
      spec \cup {RecordValidationDrop}
    [] Bug = "allow_same_height_conflict"
       /\ c = CommitSameHeightConflictDrops ->
      (spec \ {SameHeightConflict, DropVote, RecordConsensusDrop,
        RecordValidationDrop}) \cup {AllowVote}
    [] Bug = "skip_validation_drop_same_height_conflict"
       /\ c = CommitSameHeightConflictDrops ->
      spec \ {RecordValidationDrop}
    [] Bug = "allow_same_height_newer_conflict"
       /\ c = CommitSameHeightNewerViewConflictDrops ->
      (spec \ {SameHeightConflict, DropVote, RecordConsensusDrop,
        RecordValidationDrop}) \cup {AllowVote}
    [] Bug = "require_lock_payload_for_same_hash"
       /\ c = CommitSameHashLockMissingAllows ->
      (spec \ {AllowVote}) \cup {DropVote, RecordConsensusDrop}
    [] Bug = "drop_lock_payload_missing_higher"
       /\ c = CommitLockPayloadMissingHigherAllows ->
      (spec \ {AllowVote}) \cup {DropVote, RecordConsensusDrop}
    [] Bug = "drop_candidate_payload_missing"
       /\ c = CommitCandidatePayloadMissingHigherAllows ->
      (spec \ {AllowVote}) \cup {DropVote, RecordConsensusDrop}
    [] Bug = "reject_parent_extension"
       /\ c = CommitHigherSameViewParentAllows ->
      (spec \ {AllowVote}) \cup {DropVote, RecordConsensusDrop,
        RecordValidationDrop}
    [] Bug = "reject_grandparent_extension"
       /\ c = CommitHigherSameViewGrandparentAllows ->
      (spec \ {AllowVote}) \cup {DropVote, RecordConsensusDrop,
        RecordValidationDrop}
    [] Bug = "allow_missing_parent"
       /\ c = CommitHigherSameViewMissingParentDrops ->
      (spec \ {MissingParentRejected, DropVote, RecordConsensusDrop,
        RecordValidationDrop}) \cup {AllowVote}
    [] Bug = "allow_divergent_parent"
       /\ c = CommitHigherSameViewDivergentParentDrops ->
      (spec \ {DivergentParentRejected, DropVote, RecordConsensusDrop,
        RecordValidationDrop}) \cup {AllowVote}
    [] Bug = "reject_newer_view_bypass"
       /\ c = CommitHigherNewerViewDivergentAllows ->
      (spec \ {NewerViewBypass, AllowVote}) \cup
        {ParentLookup, DivergentParentRejected, DropVote,
         RecordConsensusDrop, RecordValidationDrop}
    [] Bug = "skip_consensus_drop_on_reject"
       /\ c = CommitHigherSameViewDivergentParentDrops ->
      spec \ {RecordConsensusDrop}
    [] Bug = "record_validation_drop_on_allowed"
       /\ c = CommitHigherSameViewParentAllows ->
      spec \cup {RecordValidationDrop}
    [] OTHER -> spec

Bugs == {
  "none",
  "drop_non_commit",
  "drop_no_lock",
  "allow_below_locked_height",
  "record_validation_drop_below_height",
  "allow_same_height_conflict",
  "skip_validation_drop_same_height_conflict",
  "allow_same_height_newer_conflict",
  "require_lock_payload_for_same_hash",
  "drop_lock_payload_missing_higher",
  "drop_candidate_payload_missing",
  "reject_parent_extension",
  "reject_grandparent_extension",
  "allow_missing_parent",
  "allow_divergent_parent",
  "reject_newer_view_bypass",
  "skip_consensus_drop_on_reject",
  "record_validation_drop_on_allowed"
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

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

DropPrecommitVoteForLockExactness ==
  ActionsMatchSpec

DropPrecommitVoteForLockCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ DropPrecommitVoteForLockExactness

NoBugInvariant ==
  DropPrecommitVoteForLockExactness

SafetyFast ==
  DropPrecommitVoteForLockExactness

BugDropNonCommit == NoBugInvariant
BugDropNoLock == NoBugInvariant
BugAllowBelowLockedHeight == NoBugInvariant
BugRecordValidationDropBelowHeight == NoBugInvariant
BugAllowSameHeightConflict == NoBugInvariant
BugSkipValidationDropSameHeightConflict == NoBugInvariant
BugAllowSameHeightNewerConflict == NoBugInvariant
BugRequireLockPayloadForSameHash == NoBugInvariant
BugDropLockPayloadMissingHigher == NoBugInvariant
BugDropCandidatePayloadMissing == NoBugInvariant
BugRejectParentExtension == NoBugInvariant
BugRejectGrandparentExtension == NoBugInvariant
BugAllowMissingParent == NoBugInvariant
BugAllowDivergentParent == NoBugInvariant
BugRejectNewerViewBypass == NoBugInvariant
BugSkipConsensusDropOnReject == NoBugInvariant
BugRecordValidationDropOnAllowed == NoBugInvariant

====
