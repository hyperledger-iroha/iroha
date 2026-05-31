---- MODULE SumeragiHighestQcDependencyDeferralGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for highest-QC dependency deferral helpers.

This slice captures `should_force_missing_highest_fetch(...)`,
`defer_highest_qc_update_for_lock_catchup(...)`, and
`defer_round_until_highest_qc_dependency_resolves(...)` in
`main_loop/proposal_handlers.rs`. It abstracts block hashes, local storage,
pending state, and range-pull cooldowns to finite cases while preserving the
helper contract: retry-aborted non-invalid pending blocks and the active
processing hash force exact repair, invalid/clean/absent pending state uses the
ordinary exact repair path, lock-lag catch-up always defers by forcing exact
highest-QC repair and reanchoring range pull at locked+1, known local highest
blocks prune missing-defer markers instead of leaving generic missing-block
state, suppressed lock-rejected hashes do not regain markers or fetches, and
deferral never observes the proposal slot or advances the local highest QC.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ForceRetryAbortedPending == "force_retry_aborted_pending"
ForcePendingProcessing == "force_pending_processing"
NoForceInvalidRetryAborted == "no_force_invalid_retry_aborted"
NoForceCleanPending == "no_force_clean_pending"
NoForceAbsent == "no_force_absent"
NoLockLagCleanExact == "no_lock_lag_clean_exact"
NoLockLagForceExact == "no_lock_lag_force_exact"
LockLagUnknownBlock == "lock_lag_unknown_block"
LockLagKnownBlock == "lock_lag_known_block"
LockLagKnownExistingMarkerPruned == "lock_lag_known_existing_marker_pruned"
LockLagRangePullAnchor == "lock_lag_range_pull_anchor"
LockLagSourcePreserved == "lock_lag_source_preserved"
LockRejectedSuppressed == "lock_rejected_suppressed"
DeferDoesNotObserveSlot == "defer_does_not_observe_slot"
DeferDoesNotUpdateHighest == "defer_does_not_update_highest"

Cases == {
  ForceRetryAbortedPending,
  ForcePendingProcessing,
  NoForceInvalidRetryAborted,
  NoForceCleanPending,
  NoForceAbsent,
  NoLockLagCleanExact,
  NoLockLagForceExact,
  LockLagUnknownBlock,
  LockLagKnownBlock,
  LockLagKnownExistingMarkerPruned,
  LockLagRangePullAnchor,
  LockLagSourcePreserved,
  LockRejectedSuppressed,
  DeferDoesNotObserveSlot,
  DeferDoesNotUpdateHighest
}

ReturnDeferred == 1
ForceDecision == 2
NoForceDecision == 3
ForceExactFetch == 4
ExactFetch == 5
MarkDeferMarker == 6
NoDeferMarker == 7
PruneDeferMarker == 8
RangePullCatchup == 9
CatchupAnchorLockedNext == 10
LockLagReason == 11
NoRangePull == 12
NoSlotObserved == 13
NoHighestUpdate == 14
NoGenericMissingRequest == 15
NoFetchSuppressedHash == 16
SourcePreserved == 17

ActionUniverse == 1..17

DeferBase ==
  {ReturnDeferred, NoSlotObserved, NoHighestUpdate, SourcePreserved}

LockLagBase ==
  DeferBase \cup {ForceExactFetch, RangePullCatchup,
                  CatchupAnchorLockedNext, LockLagReason}

SpecActions(c) ==
  CASE c \in {ForceRetryAbortedPending, ForcePendingProcessing} ->
      {ForceDecision}
    [] c \in {NoForceInvalidRetryAborted, NoForceCleanPending,
              NoForceAbsent} ->
      {NoForceDecision}
    [] c = NoLockLagCleanExact ->
      DeferBase \cup {MarkDeferMarker, ExactFetch, NoRangePull}
    [] c = NoLockLagForceExact ->
      DeferBase \cup {MarkDeferMarker, ForceExactFetch, NoRangePull}
    [] c = LockLagUnknownBlock ->
      LockLagBase \cup {MarkDeferMarker}
    [] c = LockLagKnownBlock ->
      LockLagBase \cup {NoDeferMarker, PruneDeferMarker,
                        NoGenericMissingRequest}
    [] c = LockLagKnownExistingMarkerPruned ->
      LockLagBase \cup {NoDeferMarker, PruneDeferMarker,
                        NoGenericMissingRequest}
    [] c = LockLagRangePullAnchor ->
      LockLagBase \cup {MarkDeferMarker}
    [] c = LockLagSourcePreserved ->
      LockLagBase \cup {MarkDeferMarker}
    [] c = LockRejectedSuppressed ->
      DeferBase \cup {NoDeferMarker, NoFetchSuppressedHash, NoRangePull}
    [] c = DeferDoesNotObserveSlot ->
      DeferBase \cup {MarkDeferMarker, ExactFetch, NoRangePull}
    [] c = DeferDoesNotUpdateHighest ->
      DeferBase \cup {MarkDeferMarker, ExactFetch, NoRangePull}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "skip_force_retry_aborted"
       /\ c = ForceRetryAbortedPending ->
      {NoForceDecision}
    [] Bug = "skip_force_processing"
       /\ c = ForcePendingProcessing ->
      {NoForceDecision}
    [] Bug = "force_invalid_retry"
       /\ c = NoForceInvalidRetryAborted ->
      {ForceDecision}
    [] Bug = "force_clean_pending"
       /\ c = NoForceCleanPending ->
      {ForceDecision}
    [] Bug = "force_absent"
       /\ c = NoForceAbsent ->
      {ForceDecision}
    [] Bug = "clean_uses_force"
       /\ c = NoLockLagCleanExact ->
      (spec \ {ExactFetch}) \cup {ForceExactFetch}
    [] Bug = "force_uses_exact"
       /\ c = NoLockLagForceExact ->
      (spec \ {ForceExactFetch}) \cup {ExactFetch}
    [] Bug = "no_lock_lag_skip_marker"
       /\ c = NoLockLagCleanExact ->
      spec \ {MarkDeferMarker}
    [] Bug = "no_lock_lag_range_pull"
       /\ c = NoLockLagCleanExact ->
      (spec \ {NoRangePull}) \cup {RangePullCatchup}
    [] Bug = "lock_lag_skip_marker_unknown"
       /\ c = LockLagUnknownBlock ->
      spec \ {MarkDeferMarker}
    [] Bug = "lock_lag_marks_known"
       /\ c = LockLagKnownBlock ->
      (spec \ {NoDeferMarker, PruneDeferMarker}) \cup {MarkDeferMarker}
    [] Bug = "lock_lag_skip_marker_prune"
       /\ c = LockLagKnownExistingMarkerPruned ->
      spec \ {PruneDeferMarker}
    [] Bug = "lock_lag_skip_force_fetch"
       /\ c = LockLagUnknownBlock ->
      spec \ {ForceExactFetch}
    [] Bug = "lock_lag_skip_range_pull"
       /\ c = LockLagUnknownBlock ->
      spec \ {RangePullCatchup}
    [] Bug = "lock_lag_wrong_anchor"
       /\ c = LockLagRangePullAnchor ->
      spec \ {CatchupAnchorLockedNext}
    [] Bug = "lock_lag_wrong_reason"
       /\ c = LockLagRangePullAnchor ->
      spec \ {LockLagReason}
    [] Bug = "lock_lag_leaves_generic_missing"
       /\ c = LockLagKnownBlock ->
      spec \ {NoGenericMissingRequest}
    [] Bug = "source_not_preserved"
       /\ c = LockLagSourcePreserved ->
      spec \ {SourcePreserved}
    [] Bug = "lock_rejected_marker_reintroduced"
       /\ c = LockRejectedSuppressed ->
      (spec \ {NoDeferMarker}) \cup {MarkDeferMarker}
    [] Bug = "lock_rejected_fetch_requested"
       /\ c = LockRejectedSuppressed ->
      (spec \ {NoFetchSuppressedHash}) \cup {ForceExactFetch}
    [] Bug = "defer_observes_slot"
       /\ c = DeferDoesNotObserveSlot ->
      spec \ {NoSlotObserved}
    [] Bug = "defer_updates_highest"
       /\ c = DeferDoesNotUpdateHighest ->
      spec \ {NoHighestUpdate}
    [] OTHER -> spec

Bugs == {
  "none",
  "skip_force_retry_aborted",
  "skip_force_processing",
  "force_invalid_retry",
  "force_clean_pending",
  "force_absent",
  "clean_uses_force",
  "force_uses_exact",
  "no_lock_lag_skip_marker",
  "no_lock_lag_range_pull",
  "lock_lag_skip_marker_unknown",
  "lock_lag_marks_known",
  "lock_lag_skip_marker_prune",
  "lock_lag_skip_force_fetch",
  "lock_lag_skip_range_pull",
  "lock_lag_wrong_anchor",
  "lock_lag_wrong_reason",
  "lock_lag_leaves_generic_missing",
  "source_not_preserved",
  "lock_rejected_marker_reintroduced",
  "lock_rejected_fetch_requested",
  "defer_observes_slot",
  "defer_updates_highest"
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

ForceFetchDecisionMatchesPendingState ==
  /\ ForceDecision \in ImplementationActions(ForceRetryAbortedPending)
  /\ ForceDecision \in ImplementationActions(ForcePendingProcessing)
  /\ NoForceDecision \in ImplementationActions(NoForceInvalidRetryAborted)
  /\ NoForceDecision \in ImplementationActions(NoForceCleanPending)
  /\ NoForceDecision \in ImplementationActions(NoForceAbsent)

NonLockLagUsesExactOrForcedFetchWithoutRangePull ==
  /\ {ReturnDeferred, MarkDeferMarker, ExactFetch, NoRangePull}
       \subseteq ImplementationActions(NoLockLagCleanExact)
  /\ {ReturnDeferred, MarkDeferMarker, ForceExactFetch, NoRangePull}
       \subseteq ImplementationActions(NoLockLagForceExact)
  /\ ~(RangePullCatchup \in ImplementationActions(NoLockLagCleanExact))

LockLagReanchorsRecovery ==
  \A c \in {LockLagUnknownBlock, LockLagKnownBlock,
            LockLagKnownExistingMarkerPruned, LockLagRangePullAnchor,
            LockLagSourcePreserved}:
    {ReturnDeferred, ForceExactFetch, RangePullCatchup,
     CatchupAnchorLockedNext, LockLagReason, SourcePreserved}
       \subseteq ImplementationActions(c)

KnownLockLagDoesNotLeaveMissingDependency ==
  \A c \in {LockLagKnownBlock, LockLagKnownExistingMarkerPruned}:
    /\ NoDeferMarker \in ImplementationActions(c)
    /\ PruneDeferMarker \in ImplementationActions(c)
    /\ NoGenericMissingRequest \in ImplementationActions(c)
    /\ ~(MarkDeferMarker \in ImplementationActions(c))

LockRejectedDependencyStaysSuppressed ==
  /\ NoDeferMarker \in ImplementationActions(LockRejectedSuppressed)
  /\ NoFetchSuppressedHash \in ImplementationActions(LockRejectedSuppressed)
  /\ ~(MarkDeferMarker \in ImplementationActions(LockRejectedSuppressed))
  /\ ~(ForceExactFetch \in ImplementationActions(LockRejectedSuppressed))
  /\ ~(ExactFetch \in ImplementationActions(LockRejectedSuppressed))

DeferralDoesNotAcceptSlot ==
  \A c \in Cases \ {ForceRetryAbortedPending, ForcePendingProcessing,
                    NoForceInvalidRetryAborted, NoForceCleanPending,
                    NoForceAbsent}:
    /\ NoSlotObserved \in ImplementationActions(c)
    /\ NoHighestUpdate \in ImplementationActions(c)

NoBugInvariant ==
  /\ ActionsMatchSpec
  /\ ForceFetchDecisionMatchesPendingState
  /\ NonLockLagUsesExactOrForcedFetchWithoutRangePull
  /\ LockLagReanchorsRecovery
  /\ KnownLockLagDoesNotLeaveMissingDependency
  /\ LockRejectedDependencyStaysSuppressed
  /\ DeferralDoesNotAcceptSlot

SafetyFast == NoBugInvariant

====
