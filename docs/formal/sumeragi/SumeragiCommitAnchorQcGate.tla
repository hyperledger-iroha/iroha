---- MODULE SumeragiCommitAnchorQcGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi commit-anchor QC promotion.

This slice captures `Actor::promote_commit_anchor_qc(...)`. It abstracts QC
headers to symbolic identities while preserving the helper contract:
`highest_qc` keeps an existing strictly newer `(height, view)` pair, otherwise
uses the incoming commit anchor; `locked_qc` keeps an existing equal-or-newer
pair, otherwise uses the incoming anchor; precommit votes are pruned exactly
when the lock changes; and a retained highest QC that does not satisfy the
locked chain is realigned back to the lock with status reflecting the final
highest/locked choices.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoExisting == "no_existing"
CurrentHighestNewerCompatible == "current_highest_newer_compatible"
CurrentHighestNewerIncompatible == "current_highest_newer_incompatible"
IncomingHigherHighest == "incoming_higher_highest"
EqualHighestReplaced == "equal_highest_replaced"
CurrentLockNewer == "current_lock_newer"
EqualLockKept == "equal_lock_kept"
IncomingLockNewer == "incoming_lock_newer"
CurrentLockHighestCompatible == "current_lock_highest_compatible"
CurrentLockHighestIncompatible == "current_lock_highest_incompatible"

Cases == {
  NoExisting,
  CurrentHighestNewerCompatible,
  CurrentHighestNewerIncompatible,
  IncomingHigherHighest,
  EqualHighestReplaced,
  CurrentLockNewer,
  EqualLockKept,
  IncomingLockNewer,
  CurrentLockHighestCompatible,
  CurrentLockHighestIncompatible
}

NoneQc == "none"
IncomingQc == "incoming"
CurrentHighestQc == "current_highest"
CurrentLockQc == "current_lock"

QcValues == {NoneQc, IncomingQc, CurrentHighestQc, CurrentLockQc}

HasCurrentLock(c) ==
  c \in {
    CurrentLockNewer,
    EqualLockKept,
    IncomingLockNewer,
    CurrentLockHighestCompatible,
    CurrentLockHighestIncompatible
  }

SpecFinalLock(c) ==
  CASE c \in {CurrentLockNewer, EqualLockKept,
              CurrentLockHighestCompatible,
              CurrentLockHighestIncompatible} ->
      CurrentLockQc
    [] OTHER -> IncomingQc

SpecFinalHighest(c) ==
  CASE c \in {CurrentHighestNewerCompatible,
              CurrentLockHighestCompatible} ->
      CurrentHighestQc
    [] c \in {CurrentHighestNewerIncompatible,
              CurrentLockHighestIncompatible} ->
      SpecFinalLock(c)
    [] OTHER -> IncomingQc

SpecPrunesPrecommitVotes(c) ==
  IF HasCurrentLock(c) THEN SpecFinalLock(c) # CurrentLockQc ELSE TRUE

SpecHighestStatus(c) == SpecFinalHighest(c)
SpecLockedStatus(c) == SpecFinalLock(c)

ActualFinalLock(c) ==
  CASE Bug = "lock_regresses_to_incoming"
       /\ c = CurrentLockNewer ->
      IncomingQc
    [] Bug = "equal_lock_replaced"
       /\ c = EqualLockKept ->
      IncomingQc
    [] Bug = "lock_ignores_newer_incoming"
       /\ c = IncomingLockNewer ->
      CurrentLockQc
    [] OTHER -> SpecFinalLock(c)

ActualFinalHighest(c) ==
  CASE Bug = "highest_regresses_to_incoming"
       /\ c = CurrentHighestNewerCompatible ->
      IncomingQc
    [] Bug = "highest_ignores_incoming"
       /\ c = IncomingHigherHighest ->
      CurrentHighestQc
    [] Bug = "equal_highest_keeps_current"
       /\ c = EqualHighestReplaced ->
      CurrentHighestQc
    [] Bug = "skip_realign_incompatible"
       /\ c \in {CurrentHighestNewerIncompatible,
                 CurrentLockHighestIncompatible} ->
      CurrentHighestQc
    [] Bug = "realign_compatible_highest"
       /\ c \in {CurrentHighestNewerCompatible,
                 CurrentLockHighestCompatible} ->
      ActualFinalLock(c)
    [] OTHER -> SpecFinalHighest(c)

ActualPrunesPrecommitVotes(c) ==
  CASE Bug = "skip_prune_on_lock_change"
       /\ SpecPrunesPrecommitVotes(c) ->
      FALSE
    [] Bug = "prune_on_unchanged_lock"
       /\ ~SpecPrunesPrecommitVotes(c) ->
      TRUE
    [] OTHER -> SpecPrunesPrecommitVotes(c)

ActualHighestStatus(c) ==
  CASE Bug = "highest_status_not_realigned"
       /\ c \in {CurrentHighestNewerIncompatible,
                 CurrentLockHighestIncompatible} ->
      CurrentHighestQc
    [] OTHER -> ActualFinalHighest(c)

ActualLockedStatus(c) ==
  CASE Bug = "locked_status_not_updated"
       /\ SpecPrunesPrecommitVotes(c) ->
      IF HasCurrentLock(c) THEN CurrentLockQc ELSE NoneQc
    [] OTHER -> ActualFinalLock(c)

Bugs == {
  "none",
  "highest_regresses_to_incoming",
  "highest_ignores_incoming",
  "equal_highest_keeps_current",
  "lock_regresses_to_incoming",
  "equal_lock_replaced",
  "lock_ignores_newer_incoming",
  "skip_prune_on_lock_change",
  "prune_on_unchanged_lock",
  "skip_realign_incompatible",
  "realign_compatible_highest",
  "highest_status_not_realigned",
  "locked_status_not_updated"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecFinalHighest(c) \in QcValues
       /\ SpecFinalLock(c) \in QcValues
       /\ ActualFinalHighest(c) \in QcValues
       /\ ActualFinalLock(c) \in QcValues
       /\ ActualHighestStatus(c) \in QcValues
       /\ ActualLockedStatus(c) \in QcValues
       /\ SpecPrunesPrecommitVotes(c) \in BOOLEAN
       /\ ActualPrunesPrecommitVotes(c) \in BOOLEAN

HighestSelectionMatchesSpec ==
  \A c \in Cases:
    ActualFinalHighest(c) = SpecFinalHighest(c)

LockedSelectionMatchesSpec ==
  \A c \in Cases:
    ActualFinalLock(c) = SpecFinalLock(c)

PruneMatchesLockChange ==
  \A c \in Cases:
    ActualPrunesPrecommitVotes(c) = SpecPrunesPrecommitVotes(c)

StatusMatchesFinalSelections ==
  \A c \in Cases:
    /\ ActualHighestStatus(c) = SpecHighestStatus(c)
    /\ ActualLockedStatus(c) = SpecLockedStatus(c)

CommitAnchorQcCoreSafety ==
  /\ HighestSelectionMatchesSpec
  /\ LockedSelectionMatchesSpec
  /\ PruneMatchesLockChange
  /\ StatusMatchesFinalSelections

CommitAnchorSelectionExact ==
  /\ HighestSelectionMatchesSpec
  /\ LockedSelectionMatchesSpec

CommitAnchorPruneExact ==
  PruneMatchesLockChange

CommitAnchorStatusPublicationExact ==
  StatusMatchesFinalSelections

CommitAnchorQcPromotionExactness ==
  /\ CommitAnchorSelectionExact
  /\ CommitAnchorPruneExact
  /\ CommitAnchorStatusPublicationExact

NoBugInvariant == CommitAnchorQcCoreSafety

SafetyFast == CommitAnchorQcCoreSafety

BugHighestRegressesToIncoming == NoBugInvariant
BugHighestIgnoresIncoming == NoBugInvariant
BugEqualHighestKeepsCurrent == NoBugInvariant
BugLockRegressesToIncoming == NoBugInvariant
BugEqualLockReplaced == NoBugInvariant
BugLockIgnoresNewerIncoming == NoBugInvariant
BugSkipPruneOnLockChange == NoBugInvariant
BugPruneOnUnchangedLock == NoBugInvariant
BugSkipRealignIncompatible == NoBugInvariant
BugRealignCompatibleHighest == NoBugInvariant
BugHighestStatusNotRealigned == NoBugInvariant
BugLockedStatusNotUpdated == NoBugInvariant

====
