---- MODULE SumeragiBlockSyncLockedQcGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for block-sync locked-QC helper gates.

This slice captures `block_sync_qc_extends_locked_chain(...)`,
`block_sync_qc_same_height_conflict(...)`,
`block_sync_qc_same_height_recoverable(...)`,
`defer_block_sync_qc_while_locked_payload_missing(...)`, and
`block_sync_qc_is_stale_against_lock(...)`. It keeps the helper distinctions
that matter for safety and liveness: no lock allows extension checks, missing
locked payloads allow only newer-view QCs at the extension helper, same-height
conflicts are view-gated, recoverability is an explicit Commit/flag gate,
deferral quarantines only non-newer non-matching QCs while the locked payload
is absent, and stale checks require a lock plus below-height and non-newer view.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoLockExtendsAllows == "no_lock_extends_allows"
MissingLockPayloadNewerViewExtendsAllows ==
  "missing_lock_payload_newer_view_extends_allows"
MissingLockPayloadSameViewExtendsRejects ==
  "missing_lock_payload_same_view_extends_rejects"
KnownLockSameHashExtendsAllows == "known_lock_same_hash_extends_allows"
KnownLockParentExtendsAllows == "known_lock_parent_extends_allows"
KnownLockMissingParentExtendsRejects ==
  "known_lock_missing_parent_extends_rejects"
KnownLockDivergentParentExtendsRejects ==
  "known_lock_divergent_parent_extends_rejects"
SameHeightConflictSameViewTrue == "same_height_conflict_same_view_true"
SameHeightConflictNewerViewFalse == "same_height_conflict_newer_view_false"
SameHeightConflictDifferentHeightFalse ==
  "same_height_conflict_different_height_false"
SameHeightRecoverableAllowedCommitTrue ==
  "same_height_recoverable_allowed_commit_true"
SameHeightRecoverableDeniedByFlagFalse ==
  "same_height_recoverable_denied_by_flag_false"
SameHeightRecoverablePrepareFalse ==
  "same_height_recoverable_prepare_false"
SameHeightRecoverableSameHashFalse ==
  "same_height_recoverable_same_hash_false"
DeferNoLockFalse == "defer_no_lock_false"
DeferLockPayloadKnownFalse == "defer_lock_payload_known_false"
DeferNewerViewFalse == "defer_newer_view_false"
DeferSameHashFalse == "defer_same_hash_false"
DeferMissingPayloadSameViewTrue == "defer_missing_payload_same_view_true"
StaleNoLockFalse == "stale_no_lock_false"
StaleBelowSameViewTrue == "stale_below_same_view_true"
StaleBelowNewerViewFalse == "stale_below_newer_view_false"
StaleSameHeightFalse == "stale_same_height_false"

Cases == {
  NoLockExtendsAllows,
  MissingLockPayloadNewerViewExtendsAllows,
  MissingLockPayloadSameViewExtendsRejects,
  KnownLockSameHashExtendsAllows,
  KnownLockParentExtendsAllows,
  KnownLockMissingParentExtendsRejects,
  KnownLockDivergentParentExtendsRejects,
  SameHeightConflictSameViewTrue,
  SameHeightConflictNewerViewFalse,
  SameHeightConflictDifferentHeightFalse,
  SameHeightRecoverableAllowedCommitTrue,
  SameHeightRecoverableDeniedByFlagFalse,
  SameHeightRecoverablePrepareFalse,
  SameHeightRecoverableSameHashFalse,
  DeferNoLockFalse,
  DeferLockPayloadKnownFalse,
  DeferNewerViewFalse,
  DeferSameHashFalse,
  DeferMissingPayloadSameViewTrue,
  StaleNoLockFalse,
  StaleBelowSameViewTrue,
  StaleBelowNewerViewFalse,
  StaleSameHeightFalse
}

ExtendsCheck == 1
ConflictCheck == 2
RecoverableCheck == 3
DeferCheck == 4
StaleCheck == 5
NoLockedQc == 6
LockedQcPresent == 7
LockedPayloadKnown == 8
LockedPayloadMissing == 9
CommitPhase == 10
NonCommitPhase == 11
AllowNonextendingQc == 12
SameHeight == 13
BelowLockedHeight == 14
HashConflict == 15
SameHash == 16
ViewNotNewer == 17
NewerViewBypass == 18
ParentLookup == 19
MissingParentRejected == 20
DivergentParentRejected == 21
ExtendsAllow == 22
ExtendsReject == 23
ConflictTrue == 24
ConflictFalse == 25
RecoverableTrue == 26
RecoverableFalse == 27
DeferTrue == 28
DeferFalse == 29
DropMissingLockIfUnknown == 30
QuarantineLockedPayload == 31
RecordConsensusDrop == 32
StaleTrue == 33
StaleFalse == 34

Actions == 1..34

SpecActions(c) ==
  CASE c = NoLockExtendsAllows ->
      {ExtendsCheck, NoLockedQc, ExtendsAllow}
    [] c = MissingLockPayloadNewerViewExtendsAllows ->
      {ExtendsCheck, LockedQcPresent, LockedPayloadMissing,
       NewerViewBypass, ExtendsAllow}
    [] c = MissingLockPayloadSameViewExtendsRejects ->
      {ExtendsCheck, LockedQcPresent, LockedPayloadMissing,
       ViewNotNewer, ExtendsReject}
    [] c = KnownLockSameHashExtendsAllows ->
      {ExtendsCheck, LockedQcPresent, LockedPayloadKnown, SameHash,
       ExtendsAllow}
    [] c = KnownLockParentExtendsAllows ->
      {ExtendsCheck, LockedQcPresent, LockedPayloadKnown, ParentLookup,
       ExtendsAllow}
    [] c = KnownLockMissingParentExtendsRejects ->
      {ExtendsCheck, LockedQcPresent, LockedPayloadKnown, ParentLookup,
       MissingParentRejected, ExtendsReject}
    [] c = KnownLockDivergentParentExtendsRejects ->
      {ExtendsCheck, LockedQcPresent, LockedPayloadKnown, ParentLookup,
       DivergentParentRejected, ExtendsReject}
    [] c = SameHeightConflictSameViewTrue ->
      {ConflictCheck, LockedQcPresent, SameHeight, ViewNotNewer,
       HashConflict, ConflictTrue}
    [] c = SameHeightConflictNewerViewFalse ->
      {ConflictCheck, LockedQcPresent, SameHeight, NewerViewBypass,
       HashConflict, ConflictFalse}
    [] c = SameHeightConflictDifferentHeightFalse ->
      {ConflictCheck, LockedQcPresent, ViewNotNewer, HashConflict,
       ConflictFalse}
    [] c = SameHeightRecoverableAllowedCommitTrue ->
      {RecoverableCheck, AllowNonextendingQc, CommitPhase, SameHeight,
       HashConflict, RecoverableTrue}
    [] c = SameHeightRecoverableDeniedByFlagFalse ->
      {RecoverableCheck, CommitPhase, SameHeight, HashConflict,
       RecoverableFalse}
    [] c = SameHeightRecoverablePrepareFalse ->
      {RecoverableCheck, AllowNonextendingQc, NonCommitPhase, SameHeight,
       HashConflict, RecoverableFalse}
    [] c = SameHeightRecoverableSameHashFalse ->
      {RecoverableCheck, AllowNonextendingQc, CommitPhase, SameHeight,
       SameHash, RecoverableFalse}
    [] c = DeferNoLockFalse ->
      {DeferCheck, NoLockedQc, DeferFalse}
    [] c = DeferLockPayloadKnownFalse ->
      {DeferCheck, LockedQcPresent, LockedPayloadKnown, DeferFalse}
    [] c = DeferNewerViewFalse ->
      {DeferCheck, LockedQcPresent, LockedPayloadMissing, NewerViewBypass,
       DeferFalse}
    [] c = DeferSameHashFalse ->
      {DeferCheck, LockedQcPresent, LockedPayloadMissing, SameHeight,
       SameHash, DeferFalse}
    [] c = DeferMissingPayloadSameViewTrue ->
      {DeferCheck, LockedQcPresent, LockedPayloadMissing, ViewNotNewer,
       HashConflict, DeferTrue, DropMissingLockIfUnknown,
       QuarantineLockedPayload, RecordConsensusDrop}
    [] c = StaleNoLockFalse ->
      {StaleCheck, NoLockedQc, StaleFalse}
    [] c = StaleBelowSameViewTrue ->
      {StaleCheck, LockedQcPresent, BelowLockedHeight, ViewNotNewer,
       StaleTrue}
    [] c = StaleBelowNewerViewFalse ->
      {StaleCheck, LockedQcPresent, BelowLockedHeight, NewerViewBypass,
       StaleFalse}
    [] c = StaleSameHeightFalse ->
      {StaleCheck, LockedQcPresent, SameHeight, ViewNotNewer, StaleFalse}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "extends_rejects_no_lock"
       /\ c = NoLockExtendsAllows ->
      (spec \ {ExtendsAllow}) \cup {ExtendsReject}
    [] Bug = "extends_allows_same_view_missing_lock_payload"
       /\ c = MissingLockPayloadSameViewExtendsRejects ->
      (spec \ {ExtendsReject}) \cup {ExtendsAllow}
    [] Bug = "extends_rejects_newer_view_missing_lock_payload"
       /\ c = MissingLockPayloadNewerViewExtendsAllows ->
      (spec \ {NewerViewBypass, ExtendsAllow}) \cup {ExtendsReject}
    [] Bug = "extends_rejects_same_hash"
       /\ c = KnownLockSameHashExtendsAllows ->
      (spec \ {ExtendsAllow}) \cup {ExtendsReject}
    [] Bug = "extends_rejects_parent_extension"
       /\ c = KnownLockParentExtendsAllows ->
      (spec \ {ExtendsAllow}) \cup {ExtendsReject}
    [] Bug = "extends_allows_missing_parent"
       /\ c = KnownLockMissingParentExtendsRejects ->
      (spec \ {MissingParentRejected, ExtendsReject}) \cup {ExtendsAllow}
    [] Bug = "extends_allows_divergent_parent"
       /\ c = KnownLockDivergentParentExtendsRejects ->
      (spec \ {DivergentParentRejected, ExtendsReject}) \cup {ExtendsAllow}
    [] Bug = "conflict_ignores_view_gate"
       /\ c = SameHeightConflictNewerViewFalse ->
      (spec \ {ConflictFalse}) \cup {ConflictTrue}
    [] Bug = "conflict_ignores_height"
       /\ c = SameHeightConflictDifferentHeightFalse ->
      (spec \ {ConflictFalse}) \cup {SameHeight, ConflictTrue}
    [] Bug = "recoverable_ignores_flag"
       /\ c = SameHeightRecoverableDeniedByFlagFalse ->
      (spec \ {RecoverableFalse}) \cup {AllowNonextendingQc,
        RecoverableTrue}
    [] Bug = "recoverable_allows_prepare"
       /\ c = SameHeightRecoverablePrepareFalse ->
      (spec \ {RecoverableFalse}) \cup {RecoverableTrue}
    [] Bug = "recoverable_rejects_allowed_commit"
       /\ c = SameHeightRecoverableAllowedCommitTrue ->
      (spec \ {RecoverableTrue}) \cup {RecoverableFalse}
    [] Bug = "recoverable_allows_same_hash"
       /\ c = SameHeightRecoverableSameHashFalse ->
      (spec \ {RecoverableFalse}) \cup {HashConflict, RecoverableTrue}
    [] Bug = "defer_without_lock"
       /\ c = DeferNoLockFalse ->
      (spec \ {DeferFalse}) \cup {DeferTrue, QuarantineLockedPayload,
        RecordConsensusDrop}
    [] Bug = "defer_when_lock_payload_known"
       /\ c = DeferLockPayloadKnownFalse ->
      (spec \ {DeferFalse}) \cup {DeferTrue, QuarantineLockedPayload,
        RecordConsensusDrop}
    [] Bug = "defer_newer_view"
       /\ c = DeferNewerViewFalse ->
      (spec \ {DeferFalse}) \cup {DeferTrue, QuarantineLockedPayload,
        RecordConsensusDrop}
    [] Bug = "defer_same_hash"
       /\ c = DeferSameHashFalse ->
      (spec \ {DeferFalse}) \cup {DeferTrue, QuarantineLockedPayload,
        RecordConsensusDrop}
    [] Bug = "defer_skips_quarantine"
       /\ c = DeferMissingPayloadSameViewTrue ->
      spec \ {QuarantineLockedPayload}
    [] Bug = "defer_skips_consensus_drop"
       /\ c = DeferMissingPayloadSameViewTrue ->
      spec \ {RecordConsensusDrop}
    [] Bug = "stale_ignores_view_gate"
       /\ c = StaleBelowNewerViewFalse ->
      (spec \ {StaleFalse}) \cup {StaleTrue}
    [] Bug = "stale_allows_same_height"
       /\ c = StaleSameHeightFalse ->
      (spec \ {StaleFalse}) \cup {StaleTrue}
    [] Bug = "stale_without_lock"
       /\ c = StaleNoLockFalse ->
      (spec \ {StaleFalse}) \cup {StaleTrue}
    [] OTHER -> spec

Bugs == {
  "none",
  "extends_rejects_no_lock",
  "extends_allows_same_view_missing_lock_payload",
  "extends_rejects_newer_view_missing_lock_payload",
  "extends_rejects_same_hash",
  "extends_rejects_parent_extension",
  "extends_allows_missing_parent",
  "extends_allows_divergent_parent",
  "conflict_ignores_view_gate",
  "conflict_ignores_height",
  "recoverable_ignores_flag",
  "recoverable_allows_prepare",
  "recoverable_rejects_allowed_commit",
  "recoverable_allows_same_hash",
  "defer_without_lock",
  "defer_when_lock_payload_known",
  "defer_newer_view",
  "defer_same_hash",
  "defer_skips_quarantine",
  "defer_skips_consensus_drop",
  "stale_ignores_view_gate",
  "stale_allows_same_height",
  "stale_without_lock"
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

BlockSyncLockedQcCoreSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

NoBugInvariant == BlockSyncLockedQcCoreSafety

SafetyFast == BlockSyncLockedQcCoreSafety

BugExtendsRejectsNoLock == NoBugInvariant
BugExtendsAllowsSameViewMissingLockPayload == NoBugInvariant
BugExtendsRejectsNewerViewMissingLockPayload == NoBugInvariant
BugExtendsRejectsSameHash == NoBugInvariant
BugExtendsRejectsParentExtension == NoBugInvariant
BugExtendsAllowsMissingParent == NoBugInvariant
BugExtendsAllowsDivergentParent == NoBugInvariant
BugConflictIgnoresViewGate == NoBugInvariant
BugConflictIgnoresHeight == NoBugInvariant
BugRecoverableIgnoresFlag == NoBugInvariant
BugRecoverableAllowsPrepare == NoBugInvariant
BugRecoverableRejectsAllowedCommit == NoBugInvariant
BugRecoverableAllowsSameHash == NoBugInvariant
BugDeferWithoutLock == NoBugInvariant
BugDeferWhenLockPayloadKnown == NoBugInvariant
BugDeferNewerView == NoBugInvariant
BugDeferSameHash == NoBugInvariant
BugDeferSkipsQuarantine == NoBugInvariant
BugDeferSkipsConsensusDrop == NoBugInvariant
BugStaleIgnoresViewGate == NoBugInvariant
BugStaleAllowsSameHeight == NoBugInvariant
BugStaleWithoutLock == NoBugInvariant

====
