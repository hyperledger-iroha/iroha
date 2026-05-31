---- MODULE SumeragiFrontierRecoveryAdvanceGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `advance_frontier_recovery(...)`.

The helper advances the contiguous-frontier recovery state machine. This model
abstracts signatures, payload bytes, and peer transport, and pins the
deterministic decisions that must remain stable across nodes: reason-to-cause
mapping, committed+1 gating, committed-edge/passive catch-up preemption,
same-height evidence seeding, exact-frontier event routing, live-work and
cooldown suppression, actionable dependency admission, state max/merge updates,
catch-up range-pull/cleanup transitions, and rotate-armed view changes.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Str;
  checked

\* @type: <<Str>>;
vars == <<checked>>

ReasonMissingPayload == "reason_missing_payload"
ReasonHardCap == "reason_hard_cap"
ReasonRangePull == "reason_range_pull"
ReasonQuorumTimeout == "reason_quorum_timeout"
ReasonMissingQcFallback == "reason_missing_qc_fallback"

NonFrontierHeight == "non_frontier_height"
CommittedEdgeBlocks == "committed_edge_blocks"
PassiveCatchupBlocks == "passive_catchup_blocks"
SeedNonExactNonHardCap == "seed_nonexact_non_hard_cap"
NoSeedForHardCap == "no_seed_for_hard_cap"
ExactLagExpiredRoutesEvent == "exact_lag_expired_routes_event"
ExactLiveWorkSuppress == "exact_live_work_suppress"
ExactHardCapBypassesLiveWork == "exact_hard_cap_bypasses_live_work"
ExactQuorumNoRotationNoSlotSeeds == "exact_quorum_no_rotation_no_slot_seeds"
ExactQuorumNoRotationRebroadcastedSuppress ==
  "exact_quorum_no_rotation_rebroadcasted_suppress"
ExactQuorumAllowsEvent == "exact_quorum_allows_event"
ExactFetchRetryReturns == "exact_fetch_retry_returns"
ExactFetchRetryFallsThroughViewAdvance ==
  "exact_fetch_retry_falls_through_view_advance"
ExactNoAllowRotationSuppress == "exact_no_allow_rotation_suppress"
NoActionableClearsState == "no_actionable_clears_state"
ReservedRecoveryWindowActionable == "reserved_recovery_window_actionable"
ActionableCreatesState == "actionable_creates_state"
LastViewMax == "last_view_max"
DependencyProgressMax == "dependency_progress_max"
WindowZeroStoresOnly == "window_zero_stores_only"
SameSlotIngressRecentSuppress == "same_slot_ingress_recent_suppress"
RotateArmedIngressGraceSuppress == "rotate_armed_ingress_grace_suppress"
SameHeightBacklogSuppress == "same_height_backlog_suppress"
ActionCooldownSuppress == "action_cooldown_suppress"
CatchUpWindowOneRangePull == "catchup_window_one_range_pull"
CatchUpWindowTwoCleanupArms == "catchup_window_two_cleanup_arms"
CatchUpWindowTwoNoEmitStillCleanup ==
  "catchup_window_two_no_emit_still_cleanup"
CatchUpNoEmitNoCleanupNone == "catchup_no_emit_no_cleanup_none"
RotateArmedNoAllowSuppress == "rotate_armed_no_allow_suppress"
RotateArmedSameViewSuppress == "rotate_armed_same_view_suppress"
RotateArmedGraceSuppress == "rotate_armed_grace_suppress"
RotateArmedWindowElapsedRotates == "rotate_armed_window_elapsed_rotates"
RotateArmedLowWindowsDirectTrigger ==
  "rotate_armed_low_windows_direct_trigger"
RotateAfterRotateResetsCatchup == "rotate_after_rotate_resets_catchup"

CauseCases == {
  ReasonMissingPayload,
  ReasonHardCap,
  ReasonRangePull,
  ReasonQuorumTimeout,
  ReasonMissingQcFallback
}

BranchCases == {
  NonFrontierHeight,
  CommittedEdgeBlocks,
  PassiveCatchupBlocks,
  SeedNonExactNonHardCap,
  NoSeedForHardCap,
  ExactLagExpiredRoutesEvent,
  ExactLiveWorkSuppress,
  ExactHardCapBypassesLiveWork,
  ExactQuorumNoRotationNoSlotSeeds,
  ExactQuorumNoRotationRebroadcastedSuppress,
  ExactQuorumAllowsEvent,
  ExactFetchRetryReturns,
  ExactFetchRetryFallsThroughViewAdvance,
  ExactNoAllowRotationSuppress,
  NoActionableClearsState,
  ReservedRecoveryWindowActionable,
  ActionableCreatesState,
  LastViewMax,
  DependencyProgressMax,
  WindowZeroStoresOnly,
  SameSlotIngressRecentSuppress,
  RotateArmedIngressGraceSuppress,
  SameHeightBacklogSuppress,
  ActionCooldownSuppress,
  CatchUpWindowOneRangePull,
  CatchUpWindowTwoCleanupArms,
  CatchUpWindowTwoNoEmitStillCleanup,
  CatchUpNoEmitNoCleanupNone,
  RotateArmedNoAllowSuppress,
  RotateArmedSameViewSuppress,
  RotateArmedGraceSuppress,
  RotateArmedWindowElapsedRotates,
  RotateArmedLowWindowsDirectTrigger,
  RotateAfterRotateResetsCatchup
}

Cases == CauseCases \cup BranchCases

CauseMissingPayload == 1
CauseQuorumTimeout == 2
CauseMissingQc == 3
AdvanceNone == 4
AdvanceCatchUp == 5
AdvanceRotate == 6
HeightMatched == 7
HeightMismatched == 8
CommitEdgeChecked == 9
CommitEdgeSuppresses == 10
PassiveChecked == 11
PassiveSuppresses == 12
SeedSameHeightEvidence == 13
NoSeedSameHeightEvidence == 14
ExactPathChecked == 15
ExactSlotEvent == 16
LagWindowEvent == 17
LiveWorkSuppress == 18
HardCapBypassesSuppression == 19
QuorumTimeoutEvent == 20
FetchRetryEvent == 21
ViewAdvanceEvent == 22
RotationAllowed == 23
RotationNotAllowed == 24
ActionableChecked == 25
Actionable == 26
NoActionable == 27
StateCleared == 28
StateStored == 29
StateCreated == 30
LastViewMaxed == 31
DependencyProgressMaxed == 32
WindowIndexZero == 33
WindowIndexNonZero == 34
CooldownSuppress == 35
RangePullEmit == 36
RangePullAllPeers == 37
RangePullScoped == 38
NoRangePull == 39
CleanupApplied == 40
CleanupSkipped == 41
PhaseCatchUp == 42
PhaseRotateArmed == 43
LastRotationViewMarked == 44
ExhaustedViewChangeApplied == 45
DirectViewChangeTriggered == 46
StateResetAfterRotate == 47

ActionUniverse == 1..47

FrontierChecks ==
  {HeightMatched, CommitEdgeChecked, PassiveChecked}

NoSeedFrontierChecks ==
  FrontierChecks \cup {NoSeedSameHeightEvidence}

ExactBase ==
  NoSeedFrontierChecks \cup {ExactPathChecked, ExactSlotEvent}

GenericBase ==
  NoSeedFrontierChecks \cup {ActionableChecked}

CatchUpBase ==
  GenericBase \cup {Actionable, PhaseCatchUp, WindowIndexNonZero}

RotateArmedBase ==
  GenericBase \cup {Actionable, PhaseRotateArmed, WindowIndexNonZero}

RotateResult ==
  {LastRotationViewMarked, StateResetAfterRotate, PhaseCatchUp, AdvanceRotate}

SpecActions(c) ==
  CASE c = ReasonMissingPayload ->
      {CauseMissingPayload}
    [] c = ReasonHardCap ->
      {CauseMissingPayload}
    [] c = ReasonRangePull ->
      {CauseMissingPayload}
    [] c = ReasonQuorumTimeout ->
      {CauseQuorumTimeout}
    [] c = ReasonMissingQcFallback ->
      {CauseMissingQc}
    [] c = NonFrontierHeight ->
      {HeightMismatched, NoSeedSameHeightEvidence, AdvanceNone}
    [] c = CommittedEdgeBlocks ->
      {HeightMatched, CommitEdgeChecked, CommitEdgeSuppresses,
       NoSeedSameHeightEvidence, AdvanceNone}
    [] c = PassiveCatchupBlocks ->
      {HeightMatched, CommitEdgeChecked, PassiveChecked, PassiveSuppresses,
       NoSeedSameHeightEvidence, AdvanceNone}
    [] c = SeedNonExactNonHardCap ->
      FrontierChecks \cup {SeedSameHeightEvidence, ActionableChecked,
       NoActionable, StateCleared, AdvanceNone}
    [] c = NoSeedForHardCap ->
      GenericBase \cup {NoActionable, StateCleared, AdvanceNone}
    [] c = ExactLagExpiredRoutesEvent ->
      ExactBase \cup {LagWindowEvent}
    [] c = ExactLiveWorkSuppress ->
      ExactBase \cup {LiveWorkSuppress, AdvanceNone}
    [] c = ExactHardCapBypassesLiveWork ->
      ExactBase \cup {HardCapBypassesSuppression, QuorumTimeoutEvent,
       CauseQuorumTimeout}
    [] c = ExactQuorumNoRotationNoSlotSeeds ->
      ExactBase \cup {RotationNotAllowed, SeedSameHeightEvidence, AdvanceNone}
    [] c = ExactQuorumNoRotationRebroadcastedSuppress ->
      ExactBase \cup {RotationNotAllowed, AdvanceNone}
    [] c = ExactQuorumAllowsEvent ->
      ExactBase \cup {RotationAllowed, QuorumTimeoutEvent, CauseQuorumTimeout}
    [] c = ExactFetchRetryReturns ->
      ExactBase \cup {FetchRetryEvent, AdvanceCatchUp}
    [] c = ExactFetchRetryFallsThroughViewAdvance ->
      ExactBase \cup {FetchRetryEvent, RotationAllowed, ViewAdvanceEvent,
       CauseMissingQc}
    [] c = ExactNoAllowRotationSuppress ->
      ExactBase \cup {RotationNotAllowed, AdvanceNone}
    [] c = NoActionableClearsState ->
      GenericBase \cup {NoActionable, StateCleared, AdvanceNone}
    [] c = ReservedRecoveryWindowActionable ->
      GenericBase \cup {Actionable, StateStored, PhaseCatchUp,
       WindowIndexZero, AdvanceNone}
    [] c = ActionableCreatesState ->
      GenericBase \cup {Actionable, StateCreated, PhaseCatchUp,
       LastViewMaxed, DependencyProgressMaxed, WindowIndexZero, StateStored,
       AdvanceNone}
    [] c = LastViewMax ->
      GenericBase \cup {Actionable, LastViewMaxed, StateStored,
       WindowIndexZero, AdvanceNone}
    [] c = DependencyProgressMax ->
      GenericBase \cup {Actionable, DependencyProgressMaxed, StateStored,
       WindowIndexZero, AdvanceNone}
    [] c = WindowZeroStoresOnly ->
      GenericBase \cup {Actionable, WindowIndexZero, StateStored,
       NoRangePull, CleanupSkipped, AdvanceNone}
    [] c = SameSlotIngressRecentSuppress ->
      CatchUpBase \cup {StateStored, AdvanceNone}
    [] c = RotateArmedIngressGraceSuppress ->
      RotateArmedBase \cup {StateStored, AdvanceNone}
    [] c = SameHeightBacklogSuppress ->
      CatchUpBase \cup {LiveWorkSuppress, StateStored, AdvanceNone}
    [] c = ActionCooldownSuppress ->
      CatchUpBase \cup {CooldownSuppress, StateStored, AdvanceNone}
    [] c = CatchUpWindowOneRangePull ->
      CatchUpBase \cup {RangePullEmit, RangePullScoped, CleanupSkipped,
       StateStored, AdvanceCatchUp}
    [] c = CatchUpWindowTwoCleanupArms ->
      CatchUpBase \cup {RangePullEmit, RangePullAllPeers, CleanupApplied,
       PhaseRotateArmed, StateStored, AdvanceCatchUp}
    [] c = CatchUpWindowTwoNoEmitStillCleanup ->
      CatchUpBase \cup {NoRangePull, RangePullAllPeers, CleanupApplied,
       PhaseRotateArmed, StateStored, AdvanceCatchUp}
    [] c = CatchUpNoEmitNoCleanupNone ->
      CatchUpBase \cup {NoRangePull, CleanupSkipped, StateStored,
       AdvanceNone}
    [] c = RotateArmedNoAllowSuppress ->
      RotateArmedBase \cup {RotationNotAllowed, StateStored, AdvanceNone}
    [] c = RotateArmedSameViewSuppress ->
      RotateArmedBase \cup {RotationAllowed, LastRotationViewMarked,
       StateStored, AdvanceNone}
    [] c = RotateArmedGraceSuppress ->
      RotateArmedBase \cup {RotationAllowed, StateStored, AdvanceNone}
    [] c = RotateArmedWindowElapsedRotates ->
      RotateArmedBase \cup {RotationAllowed, ExhaustedViewChangeApplied}
        \cup RotateResult
    [] c = RotateArmedLowWindowsDirectTrigger ->
      RotateArmedBase \cup {RotationAllowed, DirectViewChangeTriggered}
        \cup RotateResult
    [] c = RotateAfterRotateResetsCatchup ->
      RotateArmedBase \cup {RotationAllowed, ExhaustedViewChangeApplied}
        \cup RotateResult
    [] OTHER -> {}

ImplementationActions(c) ==
  CASE Bug = "cause_missing_payload_maps_missing_qc"
       /\ c = ReasonMissingPayload ->
      {CauseMissingQc}
    [] Bug = "cause_quorum_maps_missing_qc"
       /\ c = ReasonQuorumTimeout ->
      {CauseMissingQc}
    [] Bug = "nonfrontier_advances"
       /\ c = NonFrontierHeight ->
      (SpecActions(c) \ {AdvanceNone}) \cup {AdvanceRotate}
    [] Bug = "skip_committed_edge_block"
       /\ c = CommittedEdgeBlocks ->
      (SpecActions(c) \ {CommitEdgeSuppresses, AdvanceNone})
        \cup {ActionableChecked, AdvanceCatchUp}
    [] Bug = "skip_passive_catchup_block"
       /\ c = PassiveCatchupBlocks ->
      (SpecActions(c) \ {PassiveSuppresses, AdvanceNone})
        \cup {ActionableChecked, AdvanceCatchUp}
    [] Bug = "seed_hard_cap_nonexact"
       /\ c = NoSeedForHardCap ->
      (SpecActions(c) \ {NoSeedSameHeightEvidence}) \cup {SeedSameHeightEvidence}
    [] Bug = "skip_seed_nonexact"
       /\ c = SeedNonExactNonHardCap ->
      (SpecActions(c) \ {SeedSameHeightEvidence}) \cup {NoSeedSameHeightEvidence}
    [] Bug = "exact_lag_event_skipped"
       /\ c = ExactLagExpiredRoutesEvent ->
      (SpecActions(c) \ {LagWindowEvent, ExactSlotEvent}) \cup {AdvanceNone}
    [] Bug = "exact_live_work_not_suppressed"
       /\ c = ExactLiveWorkSuppress ->
      (SpecActions(c) \ {LiveWorkSuppress, AdvanceNone})
        \cup {QuorumTimeoutEvent, AdvanceRotate}
    [] Bug = "exact_hard_cap_still_suppresses"
       /\ c = ExactHardCapBypassesLiveWork ->
      (SpecActions(c) \ {HardCapBypassesSuppression, QuorumTimeoutEvent})
        \cup {LiveWorkSuppress, AdvanceNone}
    [] Bug = "exact_quorum_no_rotation_rotates"
       /\ c = ExactQuorumNoRotationNoSlotSeeds ->
      (SpecActions(c) \ {SeedSameHeightEvidence, AdvanceNone})
        \cup {QuorumTimeoutEvent, AdvanceRotate}
    [] Bug = "exact_rebroadcasted_rotates"
       /\ c = ExactQuorumNoRotationRebroadcastedSuppress ->
      (SpecActions(c) \ {AdvanceNone}) \cup {QuorumTimeoutEvent, AdvanceRotate}
    [] Bug = "exact_fetch_retry_dropped"
       /\ c = ExactFetchRetryReturns ->
      (SpecActions(c) \ {FetchRetryEvent, AdvanceCatchUp}) \cup {AdvanceNone}
    [] Bug = "exact_no_allow_rotates"
       /\ c = ExactNoAllowRotationSuppress ->
      (SpecActions(c) \ {RotationNotAllowed, AdvanceNone})
        \cup {RotationAllowed, ViewAdvanceEvent, AdvanceRotate}
    [] Bug = "no_actionable_keeps_state"
       /\ c = NoActionableClearsState ->
      (SpecActions(c) \ {StateCleared}) \cup {StateStored}
    [] Bug = "no_actionable_advances"
       /\ c = NoActionableClearsState ->
      (SpecActions(c) \ {AdvanceNone}) \cup {AdvanceCatchUp}
    [] Bug = "actionability_ignores_reserved"
       /\ c = ReservedRecoveryWindowActionable ->
      (SpecActions(c) \ {Actionable, StateStored, PhaseCatchUp})
        \cup {NoActionable, StateCleared}
    [] Bug = "last_view_overwritten"
       /\ c = LastViewMax ->
      SpecActions(c) \ {LastViewMaxed}
    [] Bug = "dependency_progress_not_maxed"
       /\ c = DependencyProgressMax ->
      SpecActions(c) \ {DependencyProgressMaxed}
    [] Bug = "window_zero_emits"
       /\ c = WindowZeroStoresOnly ->
      (SpecActions(c) \ {NoRangePull, AdvanceNone})
        \cup {RangePullEmit, AdvanceCatchUp}
    [] Bug = "same_slot_recent_not_suppressed"
       /\ c = SameSlotIngressRecentSuppress ->
      (SpecActions(c) \ {AdvanceNone}) \cup {RangePullEmit, AdvanceCatchUp}
    [] Bug = "rotate_armed_grace_not_suppressed"
       /\ c = RotateArmedIngressGraceSuppress ->
      (SpecActions(c) \ {AdvanceNone, StateStored})
        \cup {LastRotationViewMarked, AdvanceRotate}
    [] Bug = "backlog_not_suppressed"
       /\ c = SameHeightBacklogSuppress ->
      (SpecActions(c) \ {LiveWorkSuppress, AdvanceNone})
        \cup {RangePullEmit, AdvanceCatchUp}
    [] Bug = "cooldown_not_suppressed"
       /\ c = ActionCooldownSuppress ->
      (SpecActions(c) \ {CooldownSuppress, AdvanceNone})
        \cup {RangePullEmit, AdvanceCatchUp}
    [] Bug = "catchup_window1_all_peers"
       /\ c = CatchUpWindowOneRangePull ->
      (SpecActions(c) \ {RangePullScoped}) \cup {RangePullAllPeers}
    [] Bug = "catchup_window1_no_emit"
       /\ c = CatchUpWindowOneRangePull ->
      (SpecActions(c) \ {RangePullEmit, AdvanceCatchUp})
        \cup {NoRangePull, AdvanceNone}
    [] Bug = "catchup_window2_skips_cleanup"
       /\ c = CatchUpWindowTwoCleanupArms ->
      (SpecActions(c) \ {CleanupApplied, PhaseRotateArmed})
        \cup {CleanupSkipped}
    [] Bug = "cleanup_not_rotate_armed"
       /\ c = CatchUpWindowTwoNoEmitStillCleanup ->
      (SpecActions(c) \ {PhaseRotateArmed}) \cup {PhaseCatchUp}
    [] Bug = "no_emit_no_cleanup_advances"
       /\ c = CatchUpNoEmitNoCleanupNone ->
      (SpecActions(c) \ {AdvanceNone}) \cup {AdvanceCatchUp}
    [] Bug = "rotate_no_allow_rotates"
       /\ c = RotateArmedNoAllowSuppress ->
      (SpecActions(c) \ {RotationNotAllowed, StateStored, AdvanceNone})
        \cup {RotationAllowed, LastRotationViewMarked, AdvanceRotate}
    [] Bug = "rotate_same_view_rotates"
       /\ c = RotateArmedSameViewSuppress ->
      (SpecActions(c) \ {StateStored, AdvanceNone}) \cup {AdvanceRotate}
    [] Bug = "rotate_grace_rotates"
       /\ c = RotateArmedGraceSuppress ->
      (SpecActions(c) \ {StateStored, AdvanceNone})
        \cup {LastRotationViewMarked, AdvanceRotate}
    [] Bug = "rotate_elapsed_suppresses"
       /\ c = RotateArmedWindowElapsedRotates ->
      (SpecActions(c) \ {LastRotationViewMarked, StateResetAfterRotate,
       ExhaustedViewChangeApplied, AdvanceRotate}) \cup {StateStored, AdvanceNone}
    [] Bug = "rotate_low_windows_uses_exhausted"
       /\ c = RotateArmedLowWindowsDirectTrigger ->
      (SpecActions(c) \ {DirectViewChangeTriggered})
        \cup {ExhaustedViewChangeApplied}
    [] Bug = "rotate_does_not_reset_state"
       /\ c = RotateAfterRotateResetsCatchup ->
      SpecActions(c) \ {StateResetAfterRotate, PhaseCatchUp}
    [] Bug = "rotate_skips_last_rotation"
       /\ c = RotateAfterRotateResetsCatchup ->
      SpecActions(c) \ {LastRotationViewMarked}
    [] OTHER -> SpecActions(c)

Init ==
  checked \in Cases

Next ==
  UNCHANGED checked

TypeInvariant ==
  /\ checked \in Cases
  /\ \A c \in Cases : SpecActions(c) \subseteq ActionUniverse
  /\ \A c \in Cases : ImplementationActions(c) \subseteq ActionUniverse

CauseSafety ==
  \A c \in CauseCases : ImplementationActions(c) = SpecActions(c)

EarlyGateSafety ==
  /\ ImplementationActions(NonFrontierHeight) = SpecActions(NonFrontierHeight)
  /\ ImplementationActions(CommittedEdgeBlocks) = SpecActions(CommittedEdgeBlocks)
  /\ ImplementationActions(PassiveCatchupBlocks) = SpecActions(PassiveCatchupBlocks)
  /\ ImplementationActions(SeedNonExactNonHardCap) = SpecActions(SeedNonExactNonHardCap)
  /\ ImplementationActions(NoSeedForHardCap) = SpecActions(NoSeedForHardCap)

ExactFrontierSafety ==
  /\ ImplementationActions(ExactLagExpiredRoutesEvent) = SpecActions(ExactLagExpiredRoutesEvent)
  /\ ImplementationActions(ExactLiveWorkSuppress) = SpecActions(ExactLiveWorkSuppress)
  /\ ImplementationActions(ExactHardCapBypassesLiveWork) = SpecActions(ExactHardCapBypassesLiveWork)
  /\ ImplementationActions(ExactQuorumNoRotationNoSlotSeeds) = SpecActions(ExactQuorumNoRotationNoSlotSeeds)
  /\ ImplementationActions(ExactQuorumNoRotationRebroadcastedSuppress) =
       SpecActions(ExactQuorumNoRotationRebroadcastedSuppress)
  /\ ImplementationActions(ExactQuorumAllowsEvent) = SpecActions(ExactQuorumAllowsEvent)
  /\ ImplementationActions(ExactFetchRetryReturns) = SpecActions(ExactFetchRetryReturns)
  /\ ImplementationActions(ExactFetchRetryFallsThroughViewAdvance) =
       SpecActions(ExactFetchRetryFallsThroughViewAdvance)
  /\ ImplementationActions(ExactNoAllowRotationSuppress) = SpecActions(ExactNoAllowRotationSuppress)

ActionableStateSafety ==
  /\ ImplementationActions(NoActionableClearsState) = SpecActions(NoActionableClearsState)
  /\ ImplementationActions(ReservedRecoveryWindowActionable) =
       SpecActions(ReservedRecoveryWindowActionable)
  /\ ImplementationActions(ActionableCreatesState) = SpecActions(ActionableCreatesState)
  /\ ImplementationActions(LastViewMax) = SpecActions(LastViewMax)
  /\ ImplementationActions(DependencyProgressMax) = SpecActions(DependencyProgressMax)
  /\ ImplementationActions(WindowZeroStoresOnly) = SpecActions(WindowZeroStoresOnly)

SuppressionSafety ==
  /\ ImplementationActions(SameSlotIngressRecentSuppress) =
       SpecActions(SameSlotIngressRecentSuppress)
  /\ ImplementationActions(RotateArmedIngressGraceSuppress) =
       SpecActions(RotateArmedIngressGraceSuppress)
  /\ ImplementationActions(SameHeightBacklogSuppress) = SpecActions(SameHeightBacklogSuppress)
  /\ ImplementationActions(ActionCooldownSuppress) = SpecActions(ActionCooldownSuppress)

CatchUpSafety ==
  /\ ImplementationActions(CatchUpWindowOneRangePull) = SpecActions(CatchUpWindowOneRangePull)
  /\ ImplementationActions(CatchUpWindowTwoCleanupArms) = SpecActions(CatchUpWindowTwoCleanupArms)
  /\ ImplementationActions(CatchUpWindowTwoNoEmitStillCleanup) =
       SpecActions(CatchUpWindowTwoNoEmitStillCleanup)
  /\ ImplementationActions(CatchUpNoEmitNoCleanupNone) = SpecActions(CatchUpNoEmitNoCleanupNone)

RotateSafety ==
  /\ ImplementationActions(RotateArmedNoAllowSuppress) = SpecActions(RotateArmedNoAllowSuppress)
  /\ ImplementationActions(RotateArmedSameViewSuppress) = SpecActions(RotateArmedSameViewSuppress)
  /\ ImplementationActions(RotateArmedGraceSuppress) = SpecActions(RotateArmedGraceSuppress)
  /\ ImplementationActions(RotateArmedWindowElapsedRotates) =
       SpecActions(RotateArmedWindowElapsedRotates)
  /\ ImplementationActions(RotateArmedLowWindowsDirectTrigger) =
       SpecActions(RotateArmedLowWindowsDirectTrigger)
  /\ ImplementationActions(RotateAfterRotateResetsCatchup) =
       SpecActions(RotateAfterRotateResetsCatchup)

SafetyFast ==
  /\ CauseSafety
  /\ EarlyGateSafety
  /\ ExactFrontierSafety
  /\ ActionableStateSafety
  /\ SuppressionSafety
  /\ CatchUpSafety
  /\ RotateSafety

====
