---- MODULE SumeragiActiveLockRejectRecoveryGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for active-height lock-reject recovery routing.

This slice captures `trigger_active_height_lock_reject_recovery(...)` in
`main_loop/proposal_handlers.rs`. It abstracts hashes, clocks, and frontier
state internals to finite cases while preserving the helper contract: only the
active height routes through recovery, highest-QC evidence is fetched before
falling back to the latest committed QC, missing evidence does not invent a
fetch, active recovery advances with the `missing_qc` cause, requested height,
requested view, and exact-repair flags, pristine no-progress recovery is
cleared before re-entering the view-change path, non-pristine recovery is
preserved, final view-change reporting follows the recovery advance result, and
the rejected branch is not admitted into pending proposal state.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NonActiveHeightNoop == "non_active_height_noop"
ActiveHighestQcFetch == "active_highest_qc_fetch"
ActiveCommittedQcFallbackFetch == "active_committed_qc_fallback_fetch"
ActiveNoQcNoFetch == "active_no_qc_no_fetch"
AdvanceProgressOnly == "advance_progress_only"
AdvanceRotateTriggersViewChange == "advance_rotate_triggers_view_change"
AdvanceWaitNoViewChange == "advance_wait_no_view_change"
PristineNoneEventRotate == "pristine_none_event_rotate"
PristineNoneEventDeferred == "pristine_none_event_deferred"
NonPristineWrongHeightPreserved == "non_pristine_wrong_height_preserved"
NonPristineWrongCausePreserved == "non_pristine_wrong_cause_preserved"
NonPristineProgressPreserved == "non_pristine_progress_preserved"
NonPristineRotationViewPreserved == "non_pristine_rotation_view_preserved"
NonPristineActionAtPreserved == "non_pristine_action_at_preserved"
RequestedViewPreserved == "requested_view_preserved"
MissingQcCausePreserved == "missing_qc_cause_preserved"
RejectedBranchNotAcceptedCase == "rejected_branch_not_accepted"

Cases == {
  NonActiveHeightNoop,
  ActiveHighestQcFetch,
  ActiveCommittedQcFallbackFetch,
  ActiveNoQcNoFetch,
  AdvanceProgressOnly,
  AdvanceRotateTriggersViewChange,
  AdvanceWaitNoViewChange,
  PristineNoneEventRotate,
  PristineNoneEventDeferred,
  NonPristineWrongHeightPreserved,
  NonPristineWrongCausePreserved,
  NonPristineProgressPreserved,
  NonPristineRotationViewPreserved,
  NonPristineActionAtPreserved,
  RequestedViewPreserved,
  MissingQcCausePreserved,
  RejectedBranchNotAcceptedCase
}

ReturnNoop == 1
FetchHighestQc == 2
FetchCommittedQc == 3
NoFetchWithoutQc == 4
AdvanceMissingQcCause == 5
AdvanceActiveHeight == 6
AdvanceRequestedView == 7
AdvanceExactRepairFlags == 8
AdvanceProgressRecorded == 9
ClearPristineRecovery == 10
PreserveRecoveryState == 11
HandleViewAdvanceEvent == 12
ViewChangeCauseMissingQc == 13
ViewChangeRequestedView == 14
FinalRotateViewChange == 15
NoViewChange == 16
RejectedBranchNotAccepted == 17
NoPendingRejectedBranch == 18

ActionUniverse == 1..18

ActiveRecoveryBase ==
  {AdvanceMissingQcCause, AdvanceActiveHeight, AdvanceRequestedView,
   AdvanceExactRepairFlags, RejectedBranchNotAccepted,
   NoPendingRejectedBranch}

NonPristineCases == {
  NonPristineWrongHeightPreserved,
  NonPristineWrongCausePreserved,
  NonPristineProgressPreserved,
  NonPristineRotationViewPreserved,
  NonPristineActionAtPreserved
}

SpecActions(c) ==
  CASE c = NonActiveHeightNoop ->
      {ReturnNoop, RejectedBranchNotAccepted, NoPendingRejectedBranch}
    [] c = ActiveHighestQcFetch ->
      ActiveRecoveryBase \cup {FetchHighestQc, NoViewChange}
    [] c = ActiveCommittedQcFallbackFetch ->
      ActiveRecoveryBase \cup {FetchCommittedQc, NoViewChange}
    [] c = ActiveNoQcNoFetch ->
      ActiveRecoveryBase \cup {NoFetchWithoutQc, NoViewChange}
    [] c = AdvanceProgressOnly ->
      ActiveRecoveryBase \cup {AdvanceProgressRecorded, NoViewChange}
    [] c = AdvanceRotateTriggersViewChange ->
      ActiveRecoveryBase \cup {FinalRotateViewChange}
    [] c = AdvanceWaitNoViewChange ->
      ActiveRecoveryBase \cup {NoViewChange}
    [] c = PristineNoneEventRotate ->
      ActiveRecoveryBase \cup {ClearPristineRecovery, HandleViewAdvanceEvent,
                               ViewChangeCauseMissingQc,
                               ViewChangeRequestedView,
                               FinalRotateViewChange}
    [] c = PristineNoneEventDeferred ->
      ActiveRecoveryBase \cup {ClearPristineRecovery, HandleViewAdvanceEvent,
                               ViewChangeCauseMissingQc,
                               ViewChangeRequestedView, NoViewChange}
    [] c \in NonPristineCases ->
      ActiveRecoveryBase \cup {PreserveRecoveryState, NoViewChange}
    [] c = RequestedViewPreserved ->
      ActiveRecoveryBase \cup {HandleViewAdvanceEvent,
                               ViewChangeRequestedView, NoViewChange}
    [] c = MissingQcCausePreserved ->
      ActiveRecoveryBase \cup {HandleViewAdvanceEvent,
                               ViewChangeCauseMissingQc, NoViewChange}
    [] c = RejectedBranchNotAcceptedCase ->
      ActiveRecoveryBase \cup {NoViewChange}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "non_active_routes"
       /\ c = NonActiveHeightNoop ->
      ActiveRecoveryBase \cup {NoViewChange}
    [] Bug = "skip_highest_fetch"
       /\ c = ActiveHighestQcFetch ->
      spec \ {FetchHighestQc}
    [] Bug = "skip_committed_fallback"
       /\ c = ActiveCommittedQcFallbackFetch ->
      spec \ {FetchCommittedQc}
    [] Bug = "fetch_without_qc"
       /\ c = ActiveNoQcNoFetch ->
      (spec \ {NoFetchWithoutQc}) \cup {FetchHighestQc}
    [] Bug = "advance_wrong_cause"
       /\ c = ActiveHighestQcFetch ->
      spec \ {AdvanceMissingQcCause}
    [] Bug = "advance_wrong_height"
       /\ c = ActiveHighestQcFetch ->
      spec \ {AdvanceActiveHeight}
    [] Bug = "advance_wrong_view"
       /\ c = ActiveHighestQcFetch ->
      spec \ {AdvanceRequestedView}
    [] Bug = "advance_wrong_flags"
       /\ c = ActiveHighestQcFetch ->
      spec \ {AdvanceExactRepairFlags}
    [] Bug = "skip_pristine_clear"
       /\ c = PristineNoneEventRotate ->
      spec \ {ClearPristineRecovery}
    [] Bug = "clear_nonpristine_height"
       /\ c = NonPristineWrongHeightPreserved ->
      (spec \ {PreserveRecoveryState}) \cup {ClearPristineRecovery}
    [] Bug = "clear_nonpristine_cause"
       /\ c = NonPristineWrongCausePreserved ->
      (spec \ {PreserveRecoveryState}) \cup {ClearPristineRecovery}
    [] Bug = "clear_nonpristine_progress"
       /\ c = NonPristineProgressPreserved ->
      (spec \ {PreserveRecoveryState}) \cup {ClearPristineRecovery}
    [] Bug = "clear_nonpristine_rotation"
       /\ c = NonPristineRotationViewPreserved ->
      (spec \ {PreserveRecoveryState}) \cup {ClearPristineRecovery}
    [] Bug = "clear_nonpristine_action"
       /\ c = NonPristineActionAtPreserved ->
      (spec \ {PreserveRecoveryState}) \cup {ClearPristineRecovery}
    [] Bug = "skip_view_change_event"
       /\ c = PristineNoneEventRotate ->
      spec \ {HandleViewAdvanceEvent}
    [] Bug = "wrong_view_change_cause"
       /\ c = MissingQcCausePreserved ->
      spec \ {ViewChangeCauseMissingQc}
    [] Bug = "wrong_requested_view"
       /\ c = RequestedViewPreserved ->
      spec \ {ViewChangeRequestedView}
    [] Bug = "view_change_on_wait"
       /\ c = AdvanceWaitNoViewChange ->
      (spec \ {NoViewChange}) \cup {FinalRotateViewChange}
    [] Bug = "skip_rotate_view_change"
       /\ c = AdvanceRotateTriggersViewChange ->
      spec \ {FinalRotateViewChange}
    [] Bug = "accepts_rejected_branch"
       /\ c = RejectedBranchNotAcceptedCase ->
      spec \ {RejectedBranchNotAccepted}
    [] Bug = "queues_rejected_branch"
       /\ c = RejectedBranchNotAcceptedCase ->
      spec \ {NoPendingRejectedBranch}
    [] OTHER -> spec

Bugs == {
  "none",
  "non_active_routes",
  "skip_highest_fetch",
  "skip_committed_fallback",
  "fetch_without_qc",
  "advance_wrong_cause",
  "advance_wrong_height",
  "advance_wrong_view",
  "advance_wrong_flags",
  "skip_pristine_clear",
  "clear_nonpristine_height",
  "clear_nonpristine_cause",
  "clear_nonpristine_progress",
  "clear_nonpristine_rotation",
  "clear_nonpristine_action",
  "skip_view_change_event",
  "wrong_view_change_cause",
  "wrong_requested_view",
  "view_change_on_wait",
  "skip_rotate_view_change",
  "accepts_rejected_branch",
  "queues_rejected_branch"
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

NonActiveHeightReturnsEarly ==
  ImplementationActions(NonActiveHeightNoop) =
    {ReturnNoop, RejectedBranchNotAccepted, NoPendingRejectedBranch}

QcFetchFollowsEvidence ==
  /\ FetchHighestQc \in ImplementationActions(ActiveHighestQcFetch)
  /\ FetchCommittedQc \in ImplementationActions(ActiveCommittedQcFallbackFetch)
  /\ NoFetchWithoutQc \in ImplementationActions(ActiveNoQcNoFetch)
  /\ ~(FetchHighestQc \in ImplementationActions(ActiveNoQcNoFetch))
  /\ ~(FetchCommittedQc \in ImplementationActions(ActiveNoQcNoFetch))

ActiveRecoveryUsesMissingQcContext ==
  \A c \in (Cases \ {NonActiveHeightNoop}):
    {AdvanceMissingQcCause, AdvanceActiveHeight, AdvanceRequestedView,
     AdvanceExactRepairFlags} \subseteq ImplementationActions(c)

PristineNoneFallbackReentersViewChange ==
  /\ ClearPristineRecovery \in ImplementationActions(PristineNoneEventRotate)
  /\ HandleViewAdvanceEvent \in ImplementationActions(PristineNoneEventRotate)
  /\ ViewChangeCauseMissingQc \in ImplementationActions(PristineNoneEventRotate)
  /\ ViewChangeRequestedView \in ImplementationActions(PristineNoneEventRotate)
  /\ ClearPristineRecovery \in ImplementationActions(PristineNoneEventDeferred)
  /\ HandleViewAdvanceEvent \in ImplementationActions(PristineNoneEventDeferred)
  /\ ViewChangeCauseMissingQc \in ImplementationActions(PristineNoneEventDeferred)
  /\ ViewChangeRequestedView \in ImplementationActions(PristineNoneEventDeferred)

NonPristineRecoveryIsPreserved ==
  \A c \in NonPristineCases:
    /\ PreserveRecoveryState \in ImplementationActions(c)
    /\ ~(ClearPristineRecovery \in ImplementationActions(c))

ViewChangeReportedOnlyForRotate ==
  /\ FinalRotateViewChange \in
       ImplementationActions(AdvanceRotateTriggersViewChange)
  /\ NoViewChange \in ImplementationActions(AdvanceWaitNoViewChange)
  /\ ~(FinalRotateViewChange \in ImplementationActions(AdvanceWaitNoViewChange))

RejectedBranchNeverAdmitted ==
  /\ RejectedBranchNotAccepted \in
       ImplementationActions(RejectedBranchNotAcceptedCase)
  /\ NoPendingRejectedBranch \in
       ImplementationActions(RejectedBranchNotAcceptedCase)

NoBugInvariant ==
  /\ ActionsMatchSpec
  /\ NonActiveHeightReturnsEarly
  /\ QcFetchFollowsEvidence
  /\ ActiveRecoveryUsesMissingQcContext
  /\ PristineNoneFallbackReentersViewChange
  /\ NonPristineRecoveryIsPreserved
  /\ ViewChangeReportedOnlyForRotate
  /\ RejectedBranchNeverAdmitted

ActiveLockRejectRecoveryExactness ==
  /\ ActionsMatchSpec
  /\ NonActiveHeightReturnsEarly
  /\ QcFetchFollowsEvidence
  /\ ActiveRecoveryUsesMissingQcContext
  /\ PristineNoneFallbackReentersViewChange
  /\ NonPristineRecoveryIsPreserved
  /\ ViewChangeReportedOnlyForRotate
  /\ RejectedBranchNeverAdmitted

ActiveLockRejectRecoveryCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ActiveLockRejectRecoveryExactness

SafetyFast ==
  ActiveLockRejectRecoveryExactness

====
