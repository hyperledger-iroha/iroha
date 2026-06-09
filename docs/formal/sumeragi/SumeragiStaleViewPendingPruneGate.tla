---- MODULE SumeragiStaleViewPendingPruneGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the pending-block branch of
`prune_stale_view_state(height, min_view)`.

The helper prunes pending same-height entries whose view is lower than the
new round. Matching live frontier-owner work stays active. Other stale pending
entries are removed from active consensus ownership and local execution state
is detached. In DA mode, uncommitted valid payloads are reinserted only as
retired same-height data so block sync can serve payloads without reviving the
old branch. Removed entries clean RBC state when DA is disabled, validation
failed, or a committed payload no longer needs post-commit RBC retention.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

WrongHeight == "wrong_height"
FreshView == "fresh_view"
LiveOwnerKept == "live_owner_kept"
DaRetainValid == "da_retain_valid"
DaRetainAlreadyRetired == "da_retain_already_retired"
DaInvalid == "da_invalid"
NoDaValid == "no_da_valid"
NoDaInvalid == "no_da_invalid"
DaCommittedKeepRbc == "da_committed_keep_rbc"
DaCommittedCleanRbc == "da_committed_clean_rbc"
DaCommittedInvalid == "da_committed_invalid"

Cases == {
  WrongHeight,
  FreshView,
  LiveOwnerKept,
  DaRetainValid,
  DaRetainAlreadyRetired,
  DaInvalid,
  NoDaValid,
  NoDaInvalid,
  DaCommittedKeepRbc,
  DaCommittedCleanRbc,
  DaCommittedInvalid
}

StaleCandidateCases == Cases \ {WrongHeight, FreshView}
RetainedCases == {DaRetainValid, DaRetainAlreadyRetired}
RemovedCleanRbcCases == {
  DaInvalid,
  NoDaValid,
  NoDaInvalid,
  DaCommittedCleanRbc,
  DaCommittedInvalid
}
RemovedKeepRbcCases == {DaCommittedKeepRbc}

PendingPresent == 1
PendingAbsent == 2
ActivePending == 3
RetiredPending == 4
ValidationPreserved == 5
ValidationCleared == 6
VnextCleared == 7
SupersededCleared == 8
RetainedBranchNoted == 9
RbcPreserved == 10
RbcCleaned == 11
Untouched == 12

ActionUniverse == 1..12

UntouchedActions ==
  {Untouched, PendingPresent, ActivePending, ValidationPreserved, RbcPreserved}

KeepLiveOwnerActions ==
  {PendingPresent, ActivePending, ValidationPreserved, RbcPreserved}

RetainRetiredActions ==
  {PendingPresent, RetiredPending, ValidationCleared, VnextCleared,
   SupersededCleared, RetainedBranchNoted, RbcPreserved}

RetainActiveActions ==
  {PendingPresent, ActivePending, ValidationCleared, VnextCleared,
   SupersededCleared, RetainedBranchNoted, RbcPreserved}

RetainRetiredNoNoteActions ==
  {PendingPresent, RetiredPending, ValidationCleared, VnextCleared,
   SupersededCleared, RbcPreserved}

RetainRetiredCleanRbcActions ==
  {PendingPresent, RetiredPending, ValidationCleared, VnextCleared,
   SupersededCleared, RetainedBranchNoted, RbcCleaned}

RetainRetiredNoValidationCleanupActions ==
  {PendingPresent, RetiredPending, VnextCleared, SupersededCleared,
   RetainedBranchNoted, RbcPreserved}

RemovedCleanRbcActions ==
  {PendingAbsent, ValidationCleared, VnextCleared, SupersededCleared,
   RbcCleaned}

RemovedKeepRbcActions ==
  {PendingAbsent, ValidationCleared, VnextCleared, SupersededCleared,
   RbcPreserved}

RemovedCleanRbcNoValidationCleanupActions ==
  {PendingAbsent, VnextCleared, SupersededCleared, RbcCleaned}

RemovedCleanRbcNoVnextCleanupActions ==
  {PendingAbsent, ValidationCleared, SupersededCleared, RbcCleaned}

RemovedCleanRbcNoSupersededCleanupActions ==
  {PendingAbsent, ValidationCleared, VnextCleared, RbcCleaned}

SpecActions(c) ==
  CASE c \in {WrongHeight, FreshView} -> UntouchedActions
    [] c = LiveOwnerKept -> KeepLiveOwnerActions
    [] c \in RetainedCases -> RetainRetiredActions
    [] c \in RemovedCleanRbcCases -> RemovedCleanRbcActions
    [] c \in RemovedKeepRbcCases -> RemovedKeepRbcActions

ImplementationActions(c) ==
  CASE Bug = "prune_wrong_height"
       /\ c = WrongHeight ->
      RemovedCleanRbcActions
    [] Bug = "prune_fresh_view"
       /\ c = FreshView ->
      RemovedCleanRbcActions
    [] Bug = "drop_live_owner"
       /\ c = LiveOwnerKept ->
      RemovedKeepRbcActions
    [] Bug = "retire_live_owner"
       /\ c = LiveOwnerKept ->
      RetainRetiredActions
    [] Bug = "clear_validation_live_owner"
       /\ c = LiveOwnerKept ->
      {PendingPresent, ActivePending, ValidationCleared, RbcPreserved}
    [] Bug = "remove_da_valid"
       /\ c = DaRetainValid ->
      RemovedKeepRbcActions
    [] Bug = "skip_retire_da_valid"
       /\ c = DaRetainValid ->
      RetainActiveActions
    [] Bug = "skip_retained_note"
       /\ c = DaRetainValid ->
      RetainRetiredNoNoteActions
    [] Bug = "clean_rbc_da_retained"
       /\ c = DaRetainValid ->
      RetainRetiredCleanRbcActions
    [] Bug = "unretire_already_retired"
       /\ c = DaRetainAlreadyRetired ->
      RetainActiveActions
    [] Bug = "retain_da_invalid"
       /\ c = DaInvalid ->
      RetainRetiredActions
    [] Bug = "retain_no_da"
       /\ c = NoDaValid ->
      RetainRetiredActions
    [] Bug = "retain_committed"
       /\ c = DaCommittedCleanRbc ->
      RetainRetiredActions
    [] Bug = "skip_validation_cleanup_removed"
       /\ c = DaInvalid ->
      RemovedCleanRbcNoValidationCleanupActions
    [] Bug = "skip_vnext_cleanup_removed"
       /\ c = DaInvalid ->
      RemovedCleanRbcNoVnextCleanupActions
    [] Bug = "skip_superseded_cleanup_removed"
       /\ c = DaInvalid ->
      RemovedCleanRbcNoSupersededCleanupActions
    [] Bug = "skip_validation_cleanup_retained"
       /\ c = DaRetainValid ->
      RetainRetiredNoValidationCleanupActions
    [] Bug = "skip_rbc_cleanup_no_da"
       /\ c = NoDaValid ->
      RemovedKeepRbcActions
    [] Bug = "skip_rbc_cleanup_invalid"
       /\ c = DaInvalid ->
      RemovedKeepRbcActions
    [] Bug = "skip_rbc_cleanup_committed"
       /\ c = DaCommittedCleanRbc ->
      RemovedKeepRbcActions
    [] Bug = "clean_rbc_committed_retain"
       /\ c = DaCommittedKeepRbc ->
      RemovedCleanRbcActions
    [] OTHER -> SpecActions(c)

Bugs == {
  "none",
  "prune_wrong_height",
  "prune_fresh_view",
  "drop_live_owner",
  "retire_live_owner",
  "clear_validation_live_owner",
  "remove_da_valid",
  "skip_retire_da_valid",
  "skip_retained_note",
  "clean_rbc_da_retained",
  "unretire_already_retired",
  "retain_da_invalid",
  "retain_no_da",
  "retain_committed",
  "skip_validation_cleanup_removed",
  "skip_vnext_cleanup_removed",
  "skip_superseded_cleanup_removed",
  "skip_validation_cleanup_retained",
  "skip_rbc_cleanup_no_da",
  "skip_rbc_cleanup_invalid",
  "skip_rbc_cleanup_committed",
  "clean_rbc_committed_retain"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

OnlyStaleCandidatesArePruned ==
  /\ Untouched \in ImplementationActions(WrongHeight)
  /\ Untouched \in ImplementationActions(FreshView)
  /\ PendingPresent \in ImplementationActions(WrongHeight)
  /\ PendingPresent \in ImplementationActions(FreshView)
  /\ ValidationPreserved \in ImplementationActions(WrongHeight)
  /\ ValidationPreserved \in ImplementationActions(FreshView)

LiveOwnerRemainsActiveAndAttached ==
  /\ PendingPresent \in ImplementationActions(LiveOwnerKept)
  /\ ActivePending \in ImplementationActions(LiveOwnerKept)
  /\ ValidationPreserved \in ImplementationActions(LiveOwnerKept)
  /\ RbcPreserved \in ImplementationActions(LiveOwnerKept)
  /\ ~(RetiredPending \in ImplementationActions(LiveOwnerKept))
  /\ ~(PendingAbsent \in ImplementationActions(LiveOwnerKept))

DaRetainsOnlyUncommittedValidAsRetired ==
  /\ PendingPresent \in ImplementationActions(DaRetainValid)
  /\ RetiredPending \in ImplementationActions(DaRetainValid)
  /\ RetainedBranchNoted \in ImplementationActions(DaRetainValid)
  /\ RbcPreserved \in ImplementationActions(DaRetainValid)
  /\ PendingPresent \in ImplementationActions(DaRetainAlreadyRetired)
  /\ RetiredPending \in ImplementationActions(DaRetainAlreadyRetired)
  /\ ~(ActivePending \in ImplementationActions(DaRetainValid))
  /\ ~(ActivePending \in ImplementationActions(DaRetainAlreadyRetired))
  /\ PendingAbsent \in ImplementationActions(DaInvalid)
  /\ PendingAbsent \in ImplementationActions(NoDaValid)
  /\ PendingAbsent \in ImplementationActions(DaCommittedCleanRbc)

LocalExecutionStateClearedForRemovedOrRetainedStaleWork ==
  \A c \in StaleCandidateCases \ {LiveOwnerKept}:
    /\ ValidationCleared \in ImplementationActions(c)
    /\ VnextCleared \in ImplementationActions(c)
    /\ SupersededCleared \in ImplementationActions(c)

RbcCleanupPolicy ==
  /\ RbcPreserved \in ImplementationActions(DaRetainValid)
  /\ RbcPreserved \in ImplementationActions(DaCommittedKeepRbc)
  /\ RbcCleaned \in ImplementationActions(DaInvalid)
  /\ RbcCleaned \in ImplementationActions(NoDaValid)
  /\ RbcCleaned \in ImplementationActions(NoDaInvalid)
  /\ RbcCleaned \in ImplementationActions(DaCommittedCleanRbc)
  /\ RbcCleaned \in ImplementationActions(DaCommittedInvalid)

NoRetainedBranchWithoutRetiredPending ==
  \A c \in Cases:
    (RetainedBranchNoted \in ImplementationActions(c))
      => (RetiredPending \in ImplementationActions(c))

UntouchedSelectionAnchors ==
  /\ ImplementationActions(WrongHeight) = UntouchedActions
  /\ ImplementationActions(FreshView) = UntouchedActions

LiveOwnerPreservationAnchors ==
  ImplementationActions(LiveOwnerKept) = KeepLiveOwnerActions

RetiredRetentionAnchors ==
  /\ ImplementationActions(DaRetainValid) = RetainRetiredActions
  /\ ImplementationActions(DaRetainAlreadyRetired) = RetainRetiredActions

RemovalCleanupAnchors ==
  /\ ImplementationActions(DaInvalid) = RemovedCleanRbcActions
  /\ ImplementationActions(NoDaValid) = RemovedCleanRbcActions
  /\ ImplementationActions(NoDaInvalid) = RemovedCleanRbcActions
  /\ ImplementationActions(DaCommittedCleanRbc) = RemovedCleanRbcActions
  /\ ImplementationActions(DaCommittedInvalid) = RemovedCleanRbcActions
  /\ ImplementationActions(DaCommittedKeepRbc) = RemovedKeepRbcActions

RbcPolicyAnchors ==
  /\ RbcPreserved \in ImplementationActions(DaRetainValid)
  /\ RbcPreserved \in ImplementationActions(DaRetainAlreadyRetired)
  /\ RbcPreserved \in ImplementationActions(DaCommittedKeepRbc)
  /\ RbcCleaned \in ImplementationActions(DaInvalid)
  /\ RbcCleaned \in ImplementationActions(NoDaValid)
  /\ RbcCleaned \in ImplementationActions(NoDaInvalid)
  /\ RbcCleaned \in ImplementationActions(DaCommittedCleanRbc)
  /\ RbcCleaned \in ImplementationActions(DaCommittedInvalid)

RetainedBranchShapeAnchors ==
  /\ RetainedBranchNoted \in ImplementationActions(DaRetainValid)
  /\ RetainedBranchNoted \in ImplementationActions(DaRetainAlreadyRetired)
  /\ ~(RetainedBranchNoted \in ImplementationActions(LiveOwnerKept))
  /\ ~(RetainedBranchNoted \in ImplementationActions(DaInvalid))
  /\ ~(RetainedBranchNoted \in ImplementationActions(NoDaValid))
  /\ ~(RetainedBranchNoted \in ImplementationActions(DaCommittedCleanRbc))

StaleViewPendingPruneCoreSafety ==
  /\ ActionsMatchSpec
  /\ OnlyStaleCandidatesArePruned
  /\ LiveOwnerRemainsActiveAndAttached
  /\ DaRetainsOnlyUncommittedValidAsRetired
  /\ LocalExecutionStateClearedForRemovedOrRetainedStaleWork
  /\ RbcCleanupPolicy
  /\ NoRetainedBranchWithoutRetiredPending
  /\ UntouchedSelectionAnchors
  /\ LiveOwnerPreservationAnchors
  /\ RetiredRetentionAnchors
  /\ RemovalCleanupAnchors
  /\ RbcPolicyAnchors
  /\ RetainedBranchShapeAnchors

NoBugInvariant == StaleViewPendingPruneCoreSafety

SafetyFast == StaleViewPendingPruneCoreSafety

====
