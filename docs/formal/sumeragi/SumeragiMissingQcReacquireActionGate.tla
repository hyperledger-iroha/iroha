---- MODULE SumeragiMissingQcReacquireActionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `reacquire_missing_qc_dependencies(...)`.

Admission, live-frontier suppression, and sidecar retargeting are covered by
neighboring models. This slice pins the orchestration that composes them:
attempt recording, no-signal throttle marking, suppression side effects,
highest-QC fetch gating, lock-lag range-pull anchoring, repeated same-height
broad-tier promotion, cooldown clearing, success accounting, and final return
value derivation.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Str;
  checked

\* @type: <<Str>>;
vars == <<checked>>

PriorDifferentView == "prior_different_view"
PriorSameView == "prior_same_view"
PriorOtherHeight == "prior_other_height"
AttemptRecordExact == "attempt_record_exact"
NoSignalThrottle == "no_signal_throttle"
DependencySignalsNoThrottle == "dependency_signals_no_throttle"
SuppressedNoWork == "suppressed_no_work"
SuppressedSidecar == "suppressed_sidecar"
ObservedHeadFetch == "observed_head_fetch"
FarAheadFrontierUnresolved == "far_ahead_frontier_unresolved"
FarAheadNoFrontierUnresolved == "far_ahead_no_frontier_unresolved"
NoObservedHead == "no_observed_head"
LockLagRetarget == "lock_lag_retarget"
LockLagNotLower == "lock_lag_not_lower"
BroadTierSameHeight == "broad_tier_same_height"
BroadTierLockLagDifferentHeight == "broad_tier_lock_lag_different_height"
NarrowTierDefault == "narrow_tier_default"
AnchorPullSuccess == "anchor_pull_success"
AnchorPullNoEmit == "anchor_pull_no_emit"
SidecarOnlySuccess == "sidecar_only_success"
HighestOnlySuccess == "highest_only_success"
NoWorkNoSuppress == "no_work_no_suppress"
RequestedSuccessCounter == "requested_success_counter"
TriggeredSuccessCounter == "triggered_success_counter"

Cases == {
  PriorDifferentView,
  PriorSameView,
  PriorOtherHeight,
  AttemptRecordExact,
  NoSignalThrottle,
  DependencySignalsNoThrottle,
  SuppressedNoWork,
  SuppressedSidecar,
  ObservedHeadFetch,
  FarAheadFrontierUnresolved,
  FarAheadNoFrontierUnresolved,
  NoObservedHead,
  LockLagRetarget,
  LockLagNotLower,
  BroadTierSameHeight,
  BroadTierLockLagDifferentHeight,
  NarrowTierDefault,
  AnchorPullSuccess,
  AnchorPullNoEmit,
  SidecarOnlySuccess,
  HighestOnlySuccess,
  NoWorkNoSuppress,
  RequestedSuccessCounter,
  TriggeredSuccessCounter
}

PriorSameHeightReacquire == 1
NoPriorSameHeightReacquire == 2
AttemptRecordedExact == 3
AttemptRecordedWrongView == 4
NoSignalThrottleRecorded == 5
NoSignalThrottleSkipped == 6
AttemptCounterIncremented == 7
SuppressionChecked == 8
Suppressed == 9
NotSuppressed == 10
SidecarHintAttempted == 11
SidecarRequested == 12
NoSidecarRequest == 13
ObservedHeadChecked == 14
HighestFetchRequested == 15
HighestFetchSuppressedFarAhead == 16
NoHighestFetch == 17
RangePullHeightLockLag == 18
RangePullHeightRequested == 19
BroadTierTrusted == 20
NarrowTier == 21
CooldownClearedRangePullHeight == 22
CooldownClearedOriginalHeight == 23
CooldownNotCleared == 24
AnchorPullRequested == 25
AnchorPullNoEmitAction == 26
AnchorPullSuppressed == 27
SuccessCounterIncremented == 28
SuccessCounterSkipped == 29
ReturnsTrue == 30
ReturnsFalse == 31

ActionUniverse == 1..31

CommonStart ==
  {AttemptRecordedExact, AttemptCounterIncremented, SuppressionChecked,
   SidecarHintAttempted}

SuppressedBase ==
  CommonStart \cup {Suppressed, NoHighestFetch, AnchorPullSuppressed}

NotSuppressedBase ==
  CommonStart \cup {NotSuppressed, ObservedHeadChecked}

SpecActions(c) ==
  CASE c = PriorDifferentView ->
      {PriorSameHeightReacquire}
    [] c = PriorSameView ->
      {NoPriorSameHeightReacquire}
    [] c = PriorOtherHeight ->
      {NoPriorSameHeightReacquire}
    [] c = AttemptRecordExact ->
      {AttemptRecordedExact, AttemptCounterIncremented}
    [] c = NoSignalThrottle ->
      {AttemptRecordedExact, NoSignalThrottleRecorded, AttemptCounterIncremented}
    [] c = DependencySignalsNoThrottle ->
      {AttemptRecordedExact, NoSignalThrottleSkipped, AttemptCounterIncremented}
    [] c = SuppressedNoWork ->
      SuppressedBase \cup {NoSidecarRequest, SuccessCounterSkipped, ReturnsTrue}
    [] c = SuppressedSidecar ->
      SuppressedBase \cup {SidecarRequested, SuccessCounterIncremented, ReturnsTrue}
    [] c = ObservedHeadFetch ->
      NotSuppressedBase \cup {NoSidecarRequest, HighestFetchRequested,
       RangePullHeightRequested, NarrowTier, CooldownNotCleared,
       AnchorPullNoEmitAction, SuccessCounterIncremented, ReturnsTrue}
    [] c = FarAheadFrontierUnresolved ->
      NotSuppressedBase \cup {NoSidecarRequest, HighestFetchSuppressedFarAhead,
       RangePullHeightRequested, NarrowTier, CooldownNotCleared,
       AnchorPullRequested, SuccessCounterIncremented, ReturnsTrue}
    [] c = FarAheadNoFrontierUnresolved ->
      NotSuppressedBase \cup {NoSidecarRequest, HighestFetchRequested,
       RangePullHeightRequested, NarrowTier, CooldownNotCleared,
       AnchorPullNoEmitAction, SuccessCounterIncremented, ReturnsTrue}
    [] c = NoObservedHead ->
      NotSuppressedBase \cup {NoSidecarRequest, NoHighestFetch,
       RangePullHeightRequested, NarrowTier, CooldownNotCleared,
       AnchorPullNoEmitAction, SuccessCounterSkipped, ReturnsFalse}
    [] c = LockLagRetarget ->
      NotSuppressedBase \cup {NoSidecarRequest, NoHighestFetch,
       RangePullHeightLockLag, NarrowTier, CooldownNotCleared,
       AnchorPullRequested, SuccessCounterIncremented, ReturnsTrue}
    [] c = LockLagNotLower ->
      NotSuppressedBase \cup {NoSidecarRequest, NoHighestFetch,
       RangePullHeightRequested, NarrowTier, CooldownNotCleared,
       AnchorPullRequested, SuccessCounterIncremented, ReturnsTrue}
    [] c = BroadTierSameHeight ->
      NotSuppressedBase \cup {PriorSameHeightReacquire, NoSidecarRequest,
       NoHighestFetch, RangePullHeightRequested, BroadTierTrusted,
       CooldownClearedRangePullHeight, AnchorPullRequested,
       SuccessCounterIncremented, ReturnsTrue}
    [] c = BroadTierLockLagDifferentHeight ->
      NotSuppressedBase \cup {PriorSameHeightReacquire, NoSidecarRequest,
       NoHighestFetch, RangePullHeightLockLag, BroadTierTrusted,
       CooldownClearedRangePullHeight, CooldownClearedOriginalHeight,
       AnchorPullRequested, SuccessCounterIncremented, ReturnsTrue}
    [] c = NarrowTierDefault ->
      NotSuppressedBase \cup {NoPriorSameHeightReacquire, NoSidecarRequest,
       NoHighestFetch, RangePullHeightRequested, NarrowTier,
       CooldownNotCleared, AnchorPullRequested, SuccessCounterIncremented,
       ReturnsTrue}
    [] c = AnchorPullSuccess ->
      NotSuppressedBase \cup {NoSidecarRequest, NoHighestFetch,
       RangePullHeightRequested, NarrowTier, CooldownNotCleared,
       AnchorPullRequested, SuccessCounterIncremented, ReturnsTrue}
    [] c = AnchorPullNoEmit ->
      NotSuppressedBase \cup {NoSidecarRequest, NoHighestFetch,
       RangePullHeightRequested, NarrowTier, CooldownNotCleared,
       AnchorPullNoEmitAction, SuccessCounterSkipped, ReturnsFalse}
    [] c = SidecarOnlySuccess ->
      NotSuppressedBase \cup {SidecarRequested, NoHighestFetch,
       RangePullHeightRequested, NarrowTier, CooldownNotCleared,
       AnchorPullNoEmitAction, SuccessCounterIncremented, ReturnsTrue}
    [] c = HighestOnlySuccess ->
      NotSuppressedBase \cup {NoSidecarRequest, HighestFetchRequested,
       RangePullHeightRequested, NarrowTier, CooldownNotCleared,
       AnchorPullNoEmitAction, SuccessCounterIncremented, ReturnsTrue}
    [] c = NoWorkNoSuppress ->
      NotSuppressedBase \cup {NoSidecarRequest, NoHighestFetch,
       RangePullHeightRequested, NarrowTier, CooldownNotCleared,
       AnchorPullNoEmitAction, SuccessCounterSkipped, ReturnsFalse}
    [] c = RequestedSuccessCounter ->
      NotSuppressedBase \cup {SidecarRequested, NoHighestFetch,
       RangePullHeightRequested, NarrowTier, CooldownNotCleared,
       AnchorPullNoEmitAction, SuccessCounterIncremented, ReturnsTrue}
    [] c = TriggeredSuccessCounter ->
      NotSuppressedBase \cup {NoSidecarRequest, HighestFetchRequested,
       RangePullHeightRequested, NarrowTier, CooldownNotCleared,
       AnchorPullNoEmitAction, SuccessCounterIncremented, ReturnsTrue}
    [] OTHER -> {}

ImplementationActions(c) ==
  CASE Bug = "prior_same_view_marked_prior"
       /\ c = PriorSameView ->
      {PriorSameHeightReacquire}
    [] Bug = "prior_other_height_marked_prior"
       /\ c = PriorOtherHeight ->
      {PriorSameHeightReacquire}
    [] Bug = "skip_attempt_record"
       /\ c = AttemptRecordExact ->
      SpecActions(c) \ {AttemptRecordedExact}
    [] Bug = "record_previous_view"
       /\ c = AttemptRecordExact ->
      (SpecActions(c) \ {AttemptRecordedExact}) \cup {AttemptRecordedWrongView}
    [] Bug = "skip_no_signal_throttle"
       /\ c = NoSignalThrottle ->
      (SpecActions(c) \ {NoSignalThrottleRecorded}) \cup {NoSignalThrottleSkipped}
    [] Bug = "throttle_with_dependency_signals"
       /\ c = DependencySignalsNoThrottle ->
      (SpecActions(c) \ {NoSignalThrottleSkipped}) \cup {NoSignalThrottleRecorded}
    [] Bug = "skip_attempt_counter"
       /\ c = AttemptRecordExact ->
      SpecActions(c) \ {AttemptCounterIncremented}
    [] Bug = "skip_suppression_check"
       /\ c = SuppressedNoWork ->
      SpecActions(c) \ {SuppressionChecked}
    [] Bug = "suppress_emits_highest_fetch"
       /\ c = SuppressedNoWork ->
      (SpecActions(c) \ {NoHighestFetch}) \cup {HighestFetchRequested}
    [] Bug = "suppress_emits_anchor_pull"
       /\ c = SuppressedNoWork ->
      (SpecActions(c) \ {AnchorPullSuppressed}) \cup {AnchorPullRequested}
    [] Bug = "suppress_skips_sidecar_hint"
       /\ c = SuppressedSidecar ->
      SpecActions(c) \ {SidecarHintAttempted}
    [] Bug = "suppress_counts_success_without_work"
       /\ c = SuppressedNoWork ->
      (SpecActions(c) \ {SuccessCounterSkipped}) \cup {SuccessCounterIncremented}
    [] Bug = "sidecar_success_not_requested"
       /\ c = SidecarOnlySuccess ->
      (SpecActions(c) \ {SidecarRequested, ReturnsTrue})
        \cup {NoSidecarRequest, ReturnsFalse}
    [] Bug = "observed_head_skip_fetch"
       /\ c = ObservedHeadFetch ->
      (SpecActions(c) \ {HighestFetchRequested}) \cup {NoHighestFetch}
    [] Bug = "far_ahead_unresolved_fetches"
       /\ c = FarAheadFrontierUnresolved ->
      (SpecActions(c) \ {HighestFetchSuppressedFarAhead}) \cup {HighestFetchRequested}
    [] Bug = "far_ahead_without_unresolved_suppressed"
       /\ c = FarAheadNoFrontierUnresolved ->
      (SpecActions(c) \ {HighestFetchRequested}) \cup {HighestFetchSuppressedFarAhead}
    [] Bug = "no_observed_head_fetches"
       /\ c = NoObservedHead ->
      (SpecActions(c) \ {NoHighestFetch}) \cup {HighestFetchRequested}
    [] Bug = "lock_lag_ignored"
       /\ c = LockLagRetarget ->
      (SpecActions(c) \ {RangePullHeightLockLag}) \cup {RangePullHeightRequested}
    [] Bug = "non_lower_lock_lag_used"
       /\ c = LockLagNotLower ->
      (SpecActions(c) \ {RangePullHeightRequested}) \cup {RangePullHeightLockLag}
    [] Bug = "broad_tier_not_enabled_for_prior"
       /\ c = BroadTierSameHeight ->
      (SpecActions(c) \ {BroadTierTrusted}) \cup {NarrowTier}
    [] Bug = "broad_tier_enabled_without_prior"
       /\ c = NarrowTierDefault ->
      (SpecActions(c) \ {NarrowTier, CooldownNotCleared})
        \cup {BroadTierTrusted, CooldownClearedRangePullHeight}
    [] Bug = "broad_same_height_skip_cooldown_clear"
       /\ c = BroadTierSameHeight ->
      (SpecActions(c) \ {CooldownClearedRangePullHeight}) \cup {CooldownNotCleared}
    [] Bug = "broad_lock_lag_skip_requested_clear"
       /\ c = BroadTierLockLagDifferentHeight ->
      SpecActions(c) \ {CooldownClearedOriginalHeight}
    [] Bug = "narrow_clears_cooldown"
       /\ c = NarrowTierDefault ->
      (SpecActions(c) \ {CooldownNotCleared}) \cup {CooldownClearedRangePullHeight}
    [] Bug = "anchor_success_not_requested"
       /\ c = AnchorPullSuccess ->
      (SpecActions(c) \ {AnchorPullRequested, ReturnsTrue})
        \cup {AnchorPullNoEmitAction, ReturnsFalse}
    [] Bug = "anchor_suppressed_when_not_suppress"
       /\ c = AnchorPullSuccess ->
      (SpecActions(c) \ {AnchorPullRequested}) \cup {AnchorPullSuppressed}
    [] Bug = "no_work_returns_true"
       /\ c = NoWorkNoSuppress ->
      (SpecActions(c) \ {ReturnsFalse}) \cup {ReturnsTrue}
    [] Bug = "requested_no_success_counter"
       /\ c = RequestedSuccessCounter ->
      (SpecActions(c) \ {SuccessCounterIncremented}) \cup {SuccessCounterSkipped}
    [] Bug = "trigger_no_success_counter"
       /\ c = TriggeredSuccessCounter ->
      (SpecActions(c) \ {SuccessCounterIncremented}) \cup {SuccessCounterSkipped}
    [] Bug = "result_false_on_sidecar"
       /\ c = SidecarOnlySuccess ->
      (SpecActions(c) \ {ReturnsTrue}) \cup {ReturnsFalse}
    [] Bug = "result_false_on_highest_fetch"
       /\ c = HighestOnlySuccess ->
      (SpecActions(c) \ {ReturnsTrue}) \cup {ReturnsFalse}
    [] OTHER -> SpecActions(c)

Init ==
  checked \in Cases

Next ==
  UNCHANGED checked

TypeInvariant ==
  /\ checked \in Cases
  /\ \A c \in Cases : SpecActions(c) \subseteq ActionUniverse
  /\ \A c \in Cases : ImplementationActions(c) \subseteq ActionUniverse

PriorSafety ==
  /\ ImplementationActions(PriorDifferentView) = SpecActions(PriorDifferentView)
  /\ ImplementationActions(PriorSameView) = SpecActions(PriorSameView)
  /\ ImplementationActions(PriorOtherHeight) = SpecActions(PriorOtherHeight)

AttemptSafety ==
  /\ ImplementationActions(AttemptRecordExact) = SpecActions(AttemptRecordExact)
  /\ ImplementationActions(NoSignalThrottle) = SpecActions(NoSignalThrottle)
  /\ ImplementationActions(DependencySignalsNoThrottle) =
       SpecActions(DependencySignalsNoThrottle)

SuppressionSafety ==
  /\ ImplementationActions(SuppressedNoWork) = SpecActions(SuppressedNoWork)
  /\ ImplementationActions(SuppressedSidecar) = SpecActions(SuppressedSidecar)

HighestFetchSafety ==
  /\ ImplementationActions(ObservedHeadFetch) = SpecActions(ObservedHeadFetch)
  /\ ImplementationActions(FarAheadFrontierUnresolved) =
       SpecActions(FarAheadFrontierUnresolved)
  /\ ImplementationActions(FarAheadNoFrontierUnresolved) =
       SpecActions(FarAheadNoFrontierUnresolved)
  /\ ImplementationActions(NoObservedHead) = SpecActions(NoObservedHead)

RangePullSafety ==
  /\ ImplementationActions(LockLagRetarget) = SpecActions(LockLagRetarget)
  /\ ImplementationActions(LockLagNotLower) = SpecActions(LockLagNotLower)
  /\ ImplementationActions(BroadTierSameHeight) = SpecActions(BroadTierSameHeight)
  /\ ImplementationActions(BroadTierLockLagDifferentHeight) =
       SpecActions(BroadTierLockLagDifferentHeight)
  /\ ImplementationActions(NarrowTierDefault) = SpecActions(NarrowTierDefault)
  /\ ImplementationActions(AnchorPullSuccess) = SpecActions(AnchorPullSuccess)
  /\ ImplementationActions(AnchorPullNoEmit) = SpecActions(AnchorPullNoEmit)

ResultSafety ==
  /\ ImplementationActions(SidecarOnlySuccess) = SpecActions(SidecarOnlySuccess)
  /\ ImplementationActions(HighestOnlySuccess) = SpecActions(HighestOnlySuccess)
  /\ ImplementationActions(NoWorkNoSuppress) = SpecActions(NoWorkNoSuppress)
  /\ ImplementationActions(RequestedSuccessCounter) = SpecActions(RequestedSuccessCounter)
  /\ ImplementationActions(TriggeredSuccessCounter) = SpecActions(TriggeredSuccessCounter)

SafetyFast ==
  /\ PriorSafety
  /\ AttemptSafety
  /\ SuppressionSafety
  /\ HighestFetchSafety
  /\ RangePullSafety
  /\ ResultSafety

PriorAnchors ==
  /\ PriorSafety
  /\ ImplementationActions(PriorDifferentView) = SpecActions(PriorDifferentView)
  /\ ImplementationActions(PriorSameView) = SpecActions(PriorSameView)
  /\ ImplementationActions(PriorOtherHeight) = SpecActions(PriorOtherHeight)

AttemptAnchors ==
  /\ AttemptSafety
  /\ ImplementationActions(AttemptRecordExact) = SpecActions(AttemptRecordExact)
  /\ ImplementationActions(NoSignalThrottle) = SpecActions(NoSignalThrottle)
  /\ ImplementationActions(DependencySignalsNoThrottle) =
       SpecActions(DependencySignalsNoThrottle)

SuppressionAnchors ==
  /\ SuppressionSafety
  /\ ImplementationActions(SuppressedNoWork) = SpecActions(SuppressedNoWork)
  /\ ImplementationActions(SuppressedSidecar) = SpecActions(SuppressedSidecar)

HighestFetchAnchors ==
  /\ HighestFetchSafety
  /\ ImplementationActions(ObservedHeadFetch) = SpecActions(ObservedHeadFetch)
  /\ ImplementationActions(FarAheadFrontierUnresolved) =
       SpecActions(FarAheadFrontierUnresolved)
  /\ ImplementationActions(FarAheadNoFrontierUnresolved) =
       SpecActions(FarAheadNoFrontierUnresolved)
  /\ ImplementationActions(NoObservedHead) = SpecActions(NoObservedHead)

RangePullAnchors ==
  /\ RangePullSafety
  /\ ImplementationActions(LockLagRetarget) = SpecActions(LockLagRetarget)
  /\ ImplementationActions(LockLagNotLower) = SpecActions(LockLagNotLower)
  /\ ImplementationActions(BroadTierSameHeight) = SpecActions(BroadTierSameHeight)
  /\ ImplementationActions(BroadTierLockLagDifferentHeight) =
       SpecActions(BroadTierLockLagDifferentHeight)
  /\ ImplementationActions(NarrowTierDefault) = SpecActions(NarrowTierDefault)
  /\ ImplementationActions(AnchorPullSuccess) = SpecActions(AnchorPullSuccess)
  /\ ImplementationActions(AnchorPullNoEmit) = SpecActions(AnchorPullNoEmit)

ResultAnchors ==
  /\ ResultSafety
  /\ ImplementationActions(SidecarOnlySuccess) = SpecActions(SidecarOnlySuccess)
  /\ ImplementationActions(HighestOnlySuccess) = SpecActions(HighestOnlySuccess)
  /\ ImplementationActions(NoWorkNoSuppress) = SpecActions(NoWorkNoSuppress)
  /\ ImplementationActions(RequestedSuccessCounter) = SpecActions(RequestedSuccessCounter)
  /\ ImplementationActions(TriggeredSuccessCounter) = SpecActions(TriggeredSuccessCounter)

MissingQcReacquireActionSafetyAnchors ==
  /\ PriorAnchors
  /\ AttemptAnchors
  /\ SuppressionAnchors
  /\ HighestFetchAnchors
  /\ RangePullAnchors
  /\ ResultAnchors

Safety ==
  MissingQcReacquireActionSafetyAnchors

====
