---- MODULE SumeragiMissingRequestClearGate ----

(***************************************************************************
A bounded abstract model for missing-block request clearing helpers.

This slice captures the preservation-vs-obsolete decisions made by
`should_clear_missing_request_on_locked_reject(...)` and
`should_clear_missing_request_on_stale_block_drop(...)` in
`main_loop/proposal_handlers.rs`.

Locked-QC rejections may clear missing requests only when local evidence
disproves the branch: committed history conflicts with the requested hash, a
known ancestry edge conflicts with an already committed lock, a missing parent
competes with a durable locked block, or local ancestry proves non-extension of
the committed locked chain. Stale BlockCreated drops clear requests below the
committed tip, when the payload is available locally, or when committed history
disproves the hash; otherwise committed-height payload-repair requests remain
alive.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

SpecLockedSameHash == FALSE
ActualLockedSameHash ==
  IF Bug = 1 THEN TRUE ELSE SpecLockedSameHash

SpecKnownConflictBelowCommitted == TRUE
ActualKnownConflictBelowCommitted ==
  IF Bug = 2
  THEN FALSE
  ELSE SpecKnownConflictBelowCommitted

SpecKnownConflictBelowLocked == TRUE
ActualKnownConflictBelowLocked ==
  IF Bug = 3
  THEN FALSE
  ELSE SpecKnownConflictBelowLocked

SpecKnownConflictFuturePreserved == FALSE
ActualKnownConflictFuturePreserved ==
  IF Bug = 4
  THEN TRUE
  ELSE SpecKnownConflictFuturePreserved

SpecKnownParentLockedCommittedConflict == TRUE
ActualKnownParentLockedCommittedConflict ==
  IF Bug = 5
  THEN FALSE
  ELSE SpecKnownParentLockedCommittedConflict

SpecParentlessLockedCommittedConflict == TRUE
ActualParentlessLockedCommittedConflict ==
  IF Bug = 6
  THEN FALSE
  ELSE SpecParentlessLockedCommittedConflict

SpecMissingParentDurableLockSameHeight == TRUE
ActualMissingParentDurableLockSameHeight ==
  IF Bug = 7
  THEN FALSE
  ELSE SpecMissingParentDurableLockSameHeight

SpecUnresolvedParentlessFuture == FALSE
ActualUnresolvedParentlessFuture ==
  IF Bug = 8
  THEN TRUE
  ELSE SpecUnresolvedParentlessFuture

SpecUncommittedLockedConflictPreserved == FALSE
ActualUncommittedLockedConflictPreserved ==
  IF Bug = 9
  THEN TRUE
  ELSE SpecUncommittedLockedConflictPreserved

SpecCleanFutureRequest == FALSE
ActualCleanFutureRequest == SpecCleanFutureRequest

SpecStaleBelowCommitted == TRUE
ActualStaleBelowCommitted ==
  IF Bug = 10
  THEN FALSE
  ELSE SpecStaleBelowCommitted

SpecStalePayloadAvailable == TRUE
ActualStalePayloadAvailable ==
  IF Bug = 11
  THEN FALSE
  ELSE SpecStalePayloadAvailable

SpecStaleCommittedConflict == TRUE
ActualStaleCommittedConflict ==
  IF Bug = 12
  THEN FALSE
  ELSE SpecStaleCommittedConflict

SpecStaleTipMissingPayloadPreserved == FALSE
ActualStaleTipMissingPayloadPreserved ==
  IF Bug = 13
  THEN TRUE
  ELSE SpecStaleTipMissingPayloadPreserved

SpecStaleTipMatchingCommittedPreserved == FALSE
ActualStaleTipMatchingCommittedPreserved ==
  IF Bug = 14
  THEN TRUE
  ELSE SpecStaleTipMatchingCommittedPreserved

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

\* @type: <<Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
SpecOutput ==
  <<SpecLockedSameHash, SpecKnownConflictBelowCommitted,
    SpecKnownConflictBelowLocked, SpecKnownConflictFuturePreserved,
    SpecKnownParentLockedCommittedConflict,
    SpecParentlessLockedCommittedConflict,
    SpecMissingParentDurableLockSameHeight, SpecUnresolvedParentlessFuture,
    SpecUncommittedLockedConflictPreserved, SpecCleanFutureRequest,
    SpecStaleBelowCommitted, SpecStalePayloadAvailable,
    SpecStaleCommittedConflict, SpecStaleTipMissingPayloadPreserved,
    SpecStaleTipMatchingCommittedPreserved>>

\* @type: <<Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
ActualOutput ==
  <<ActualLockedSameHash, ActualKnownConflictBelowCommitted,
    ActualKnownConflictBelowLocked, ActualKnownConflictFuturePreserved,
    ActualKnownParentLockedCommittedConflict,
    ActualParentlessLockedCommittedConflict,
    ActualMissingParentDurableLockSameHeight, ActualUnresolvedParentlessFuture,
    ActualUncommittedLockedConflictPreserved, ActualCleanFutureRequest,
    ActualStaleBelowCommitted, ActualStalePayloadAvailable,
    ActualStaleCommittedConflict, ActualStaleTipMissingPayloadPreserved,
    ActualStaleTipMatchingCommittedPreserved>>

MissingRequestClearOutputMatchesSpec ==
  ActualOutput = SpecOutput

MissingRequestClearExactness ==
  /\ MissingRequestClearOutputMatchesSpec

MissingRequestClearCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ MissingRequestClearExactness

SafetyFast ==
  MissingRequestClearExactness

BugClearLockedHash ==
  ActualLockedSameHash = SpecLockedSameHash

BugPreserveKnownConflictBelowCommitted ==
  ActualKnownConflictBelowCommitted = SpecKnownConflictBelowCommitted

BugPreserveKnownConflictBelowLocked ==
  ActualKnownConflictBelowLocked = SpecKnownConflictBelowLocked

BugClearFutureKnownConflict ==
  ActualKnownConflictFuturePreserved = SpecKnownConflictFuturePreserved

BugIgnoreKnownParentConflict ==
  ActualKnownParentLockedCommittedConflict =
    SpecKnownParentLockedCommittedConflict

BugIgnoreParentlessLockedConflict ==
  ActualParentlessLockedCommittedConflict =
    SpecParentlessLockedCommittedConflict

BugPreserveDurableLockCompetitor ==
  ActualMissingParentDurableLockSameHeight =
    SpecMissingParentDurableLockSameHeight

BugClearUnresolvedParentless ==
  ActualUnresolvedParentlessFuture = SpecUnresolvedParentlessFuture

BugClearUncommittedLockedConflict ==
  ActualUncommittedLockedConflictPreserved =
    SpecUncommittedLockedConflictPreserved

BugPreserveStaleBelowCommitted ==
  ActualStaleBelowCommitted = SpecStaleBelowCommitted

BugPreserveStalePayloadAvailable ==
  ActualStalePayloadAvailable = SpecStalePayloadAvailable

BugPreserveStaleCommittedConflict ==
  ActualStaleCommittedConflict = SpecStaleCommittedConflict

BugClearStaleTipMissingPayload ==
  ActualStaleTipMissingPayloadPreserved = SpecStaleTipMissingPayloadPreserved

BugClearStaleTipMatchingCommitted ==
  ActualStaleTipMatchingCommittedPreserved =
    SpecStaleTipMatchingCommittedPreserved

====
