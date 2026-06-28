---- MODULE SumeragiCanonicalFrontierReanchorGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for canonical contiguous-frontier reanchor gating.

The modeled helpers are:
- `reason_is_canonical_frontier_reanchor(...)`;
- `canonical_frontier_reanchor_gate_heights(...)`;
- `canonical_frontier_reanchor_window_snapshot(...)`;
- `canonical_frontier_reanchor_dependency_progress_unchanged(...)`;
- `canonical_frontier_reanchor_stride_blocks_missing_qc_rotation(...)`;
- the canonical-frontier branch in
  `request_range_pull_from_anchor_with_tier(...)`; and
- `suppress_quorum_view_change_while_frontier_reanchor_unresolved(...)`.

The critical safety shape is that all canonical-frontier reanchor sources share
one deterministic `(frontier, frontier)` window key, emit at the stride
prescribed by the dwell window, suppress duplicate emissions in the same
window, keep dependency-progress watermarks monotonic, and suppress quorum
view-change only while the previously emitted unresolved reanchor still has no
new dependency progress and no missing-QC stall reservation.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Str;
  checked

\* @type: <<Str>>;
vars == <<checked>>

FutureReason == "future_reason"
FrontierGapReason == "frontier_gap_reason"
IdleReacquireReason == "idle_reacquire_reason"
CommitConflictReason == "commit_conflict_reason"
UnrelatedReason == "unrelated_reason"

NoCatchupTarget == "no_catchup_target"
CanonicalBehindTarget == "canonical_behind_target"
CollapseCanonicalHeight == "collapse_canonical_height"
GateActiveSameHeight == "gate_active_same_height"
GateActiveAheadHeight == "gate_active_ahead_height"

SnapshotInitial == "snapshot_initial"
SnapshotAdvanceTwoWindows == "snapshot_advance_two_windows"
SnapshotNoElapsed == "snapshot_no_elapsed"

ProgressOlder == "progress_older"
ProgressEqual == "progress_equal"
ProgressNewer == "progress_newer"
ProgressCleared == "progress_cleared"
ProgressCreated == "progress_created"
ProgressAbsent == "progress_absent"

StrideWindow8Eligible == "stride_window_8_eligible"
StrideWindow9Blocked == "stride_window_9_blocked"
StrideWindow10Eligible == "stride_window_10_eligible"
StrideWindow27Blocked == "stride_window_27_blocked"
StrideWindow28Eligible == "stride_window_28_eligible"
StrideWindow81Blocked == "stride_window_81_blocked"
NoDependencyStrideNoBlock == "no_dependency_stride_no_block"
NoPreviousEmitStrideNoBlock == "no_previous_emit_stride_no_block"

CanonicalWindowAlreadyEmitted == "canonical_window_already_emitted"
CanonicalStrideSuppressed == "canonical_stride_suppressed"
CanonicalWindow0Cohort == "canonical_window_0_cohort"
CanonicalWindow1Cohort == "canonical_window_1_cohort"
CanonicalWindow2AllPeers == "canonical_window_2_all_peers"
SmallPeerAllPeers == "small_peer_all_peers"
DuplicateTargetsDeduped == "duplicate_targets_deduped"
EmptyTargetsNoSend == "empty_targets_no_send"
CooldownDuplicate == "cooldown_duplicate"
CooldownBoundary == "cooldown_boundary"
MarkOnlyAfterSend == "mark_only_after_send"

SuppressQuorumUnchanged == "suppress_quorum_unchanged"
NoSuppressProgressAdvanced == "no_suppress_progress_advanced"
NoSuppressMissingQcCause == "no_suppress_missing_qc_cause"
NoSuppressReservationAvailable == "no_suppress_reservation_available"
NoSuppressNoUnresolved == "no_suppress_no_unresolved"
NoSuppressNoEmit == "no_suppress_no_emit"

ReasonCases == {
  FutureReason,
  FrontierGapReason,
  IdleReacquireReason,
  CommitConflictReason,
  UnrelatedReason
}

GateCases == {
  NoCatchupTarget,
  CanonicalBehindTarget,
  CollapseCanonicalHeight,
  GateActiveSameHeight,
  GateActiveAheadHeight
}

SnapshotCases == {
  SnapshotInitial,
  SnapshotAdvanceTwoWindows,
  SnapshotNoElapsed
}

ProgressCases == {
  ProgressOlder,
  ProgressEqual,
  ProgressNewer,
  ProgressCleared,
  ProgressCreated,
  ProgressAbsent
}

StrideCases == {
  StrideWindow8Eligible,
  StrideWindow9Blocked,
  StrideWindow10Eligible,
  StrideWindow27Blocked,
  StrideWindow28Eligible,
  StrideWindow81Blocked,
  NoDependencyStrideNoBlock,
  NoPreviousEmitStrideNoBlock
}

RangePullCases == {
  CanonicalWindowAlreadyEmitted,
  CanonicalStrideSuppressed,
  CanonicalWindow0Cohort,
  CanonicalWindow1Cohort,
  CanonicalWindow2AllPeers,
  SmallPeerAllPeers,
  DuplicateTargetsDeduped,
  EmptyTargetsNoSend,
  CooldownDuplicate,
  CooldownBoundary,
  MarkOnlyAfterSend
}

ViewSuppressionCases == {
  SuppressQuorumUnchanged,
  NoSuppressProgressAdvanced,
  NoSuppressMissingQcCause,
  NoSuppressReservationAvailable,
  NoSuppressNoUnresolved,
  NoSuppressNoEmit
}

Cases ==
  ReasonCases \cup GateCases \cup SnapshotCases \cup ProgressCases
  \cup StrideCases \cup RangePullCases \cup ViewSuppressionCases

ReasonAccepted == 1
ReasonIgnored == 2
GateReason == 3
NoGateReason == 4
GateActive == 5
GateInactive == 6
GateFrontierTarget == 7
GateCanonicalCollapsed == 8
GateCanonicalOriginal == 9
SnapshotCreates == 10
SnapshotAdvances == 11
SnapshotPreservesEntered == 12
SnapshotPreservesIndex == 13
WindowIndex0 == 14
WindowIndex2 == 15
ProgressChecked == 16
ProgressUnchanged == 17
ProgressChanged == 18
StrideChecked == 19
StrideBlocks == 20
StrideAllows == 21
Stride1 == 22
Stride2 == 23
Stride4 == 24
Stride8 == 25
UnresolvedChecked == 26
NoUnresolvedDependency == 27
EmittedWindowChecked == 28
NoPreviousEmit == 29
WindowSuppressed == 30
Send == 31
NoSend == 32
MarkWindow == 33
NoMark == 34
Cohort12 == 35
Cohort23 == 36
AllPeers == 37
SortedDeduped == 38
DedupChecked == 39
DedupBlocked == 40
DedupAllows == 41
TargetsEmpty == 42
TargetsNonEmpty == 43
CooldownBoundaryReached == 44
ViewSuppress == 45
ViewAllows == 46
CauseChecked == 47
ReservationChecked == 48
ReservationAvailable == 49
DependencyProgressChecked == 50

ActionUniverse == 1..50

CanonicalReasons == {
  FutureReason,
  FrontierGapReason,
  IdleReacquireReason,
  CommitConflictReason
}

BaseCanonicalSend ==
  {GateActive, Send, SortedDeduped, DedupChecked, DedupAllows,
   TargetsNonEmpty, MarkWindow, GateFrontierTarget, GateCanonicalCollapsed}

ViewSuppressionChecks ==
  {CauseChecked, UnresolvedChecked, EmittedWindowChecked,
   DependencyProgressChecked, ReservationChecked}

SpecActions(c) ==
  CASE c \in CanonicalReasons ->
      {ReasonAccepted, GateReason}
    [] c = UnrelatedReason ->
      {ReasonIgnored, NoGateReason}
    [] c = NoCatchupTarget ->
      {GateInactive, NoSend, NoMark}
    [] c = CanonicalBehindTarget ->
      {GateInactive, NoSend, NoMark}
    [] c = CollapseCanonicalHeight ->
      {GateActive, GateFrontierTarget, GateCanonicalCollapsed}
    [] c = GateActiveSameHeight ->
      {GateActive, GateFrontierTarget, GateCanonicalCollapsed}
    [] c = GateActiveAheadHeight ->
      {GateActive, GateFrontierTarget, GateCanonicalCollapsed}
    [] c = SnapshotInitial ->
      {SnapshotCreates, SnapshotPreservesEntered,
       SnapshotPreservesIndex, WindowIndex0}
    [] c = SnapshotAdvanceTwoWindows ->
      {SnapshotAdvances, SnapshotPreservesEntered, WindowIndex2}
    [] c = SnapshotNoElapsed ->
      {SnapshotPreservesEntered, SnapshotPreservesIndex}
    [] c \in {ProgressOlder, ProgressEqual, ProgressCleared, ProgressAbsent} ->
      {ProgressChecked, ProgressUnchanged}
    [] c \in {ProgressNewer, ProgressCreated} ->
      {ProgressChecked, ProgressChanged}
    [] c = StrideWindow8Eligible ->
      {StrideChecked, Stride1, StrideAllows,
       UnresolvedChecked, EmittedWindowChecked}
    [] c = StrideWindow9Blocked ->
      {StrideChecked, Stride2, StrideBlocks,
       UnresolvedChecked, EmittedWindowChecked}
    [] c = StrideWindow10Eligible ->
      {StrideChecked, Stride2, StrideAllows,
       UnresolvedChecked, EmittedWindowChecked}
    [] c = StrideWindow27Blocked ->
      {StrideChecked, Stride4, StrideBlocks,
       UnresolvedChecked, EmittedWindowChecked}
    [] c = StrideWindow28Eligible ->
      {StrideChecked, Stride4, StrideAllows,
       UnresolvedChecked, EmittedWindowChecked}
    [] c = StrideWindow81Blocked ->
      {StrideChecked, Stride8, StrideBlocks,
       UnresolvedChecked, EmittedWindowChecked}
    [] c = NoDependencyStrideNoBlock ->
      {StrideChecked, StrideAllows, NoUnresolvedDependency,
       EmittedWindowChecked}
    [] c = NoPreviousEmitStrideNoBlock ->
      {StrideChecked, StrideAllows, UnresolvedChecked, NoPreviousEmit}
    [] c = CanonicalWindowAlreadyEmitted ->
      {GateActive, EmittedWindowChecked, WindowSuppressed, NoSend, NoMark}
    [] c = CanonicalStrideSuppressed ->
      {GateActive, StrideChecked, StrideBlocks, WindowSuppressed,
       NoSend, NoMark}
    [] c = CanonicalWindow0Cohort ->
      BaseCanonicalSend \cup {Cohort12}
    [] c = CanonicalWindow1Cohort ->
      BaseCanonicalSend \cup {Cohort23}
    [] c = CanonicalWindow2AllPeers ->
      BaseCanonicalSend \cup {AllPeers}
    [] c = SmallPeerAllPeers ->
      BaseCanonicalSend \cup {AllPeers}
    [] c = DuplicateTargetsDeduped ->
      BaseCanonicalSend \cup {Cohort12}
    [] c = EmptyTargetsNoSend ->
      {GateActive, TargetsEmpty, DedupChecked, DedupAllows, NoSend, NoMark}
    [] c = CooldownDuplicate ->
      {GateActive, TargetsNonEmpty, DedupChecked, DedupBlocked, NoSend, NoMark}
    [] c = CooldownBoundary ->
      BaseCanonicalSend \cup {Cohort12, CooldownBoundaryReached}
    [] c = MarkOnlyAfterSend ->
      BaseCanonicalSend \cup {Cohort12}
    [] c = SuppressQuorumUnchanged ->
      ViewSuppressionChecks \cup {ViewSuppress, ProgressUnchanged}
    [] c = NoSuppressProgressAdvanced ->
      ViewSuppressionChecks \cup {ViewAllows, ProgressChanged}
    [] c = NoSuppressMissingQcCause ->
      {CauseChecked, ViewAllows}
    [] c = NoSuppressReservationAvailable ->
      ViewSuppressionChecks \cup {ViewAllows, ProgressUnchanged,
        ReservationAvailable}
    [] c = NoSuppressNoUnresolved ->
      {CauseChecked, NoUnresolvedDependency, ViewAllows}
    [] c = NoSuppressNoEmit ->
      {CauseChecked, UnresolvedChecked, NoPreviousEmit, ViewAllows}
    [] OTHER -> {}

ImplementationActions(c) ==
  CASE Bug = "reject_future_reason"
       /\ c = FutureReason ->
      {ReasonIgnored, NoGateReason}
    [] Bug = "reject_frontier_gap_reason"
       /\ c = FrontierGapReason ->
      {ReasonIgnored, NoGateReason}
    [] Bug = "reject_idle_reacquire_reason"
       /\ c = IdleReacquireReason ->
      {ReasonIgnored, NoGateReason}
    [] Bug = "reject_commit_conflict_reason"
       /\ c = CommitConflictReason ->
      {ReasonIgnored, NoGateReason}
    [] Bug = "accept_unrelated_reason"
       /\ c = UnrelatedReason ->
      {ReasonAccepted, GateReason}
    [] Bug = "gate_without_target"
       /\ c = NoCatchupTarget ->
      {GateActive, GateFrontierTarget, GateCanonicalCollapsed}
    [] Bug = "gate_allows_canonical_behind"
       /\ c = CanonicalBehindTarget ->
      {GateActive, GateFrontierTarget, GateCanonicalCollapsed}
    [] Bug = "gate_uses_canonical_height_key"
       /\ c = CollapseCanonicalHeight ->
      {GateActive, GateFrontierTarget, GateCanonicalOriginal}
    [] Bug = "snapshot_skips_advance"
       /\ c = SnapshotAdvanceTwoWindows ->
      {SnapshotPreservesEntered, SnapshotPreservesIndex}
    [] Bug = "snapshot_resets_entered"
       /\ c = SnapshotInitial ->
      SpecActions(c) \ {SnapshotPreservesEntered}
    [] Bug = "progress_newer_unchanged"
       /\ c = ProgressNewer ->
      {ProgressChecked, ProgressUnchanged}
    [] Bug = "progress_created_unchanged"
       /\ c = ProgressCreated ->
      {ProgressChecked, ProgressUnchanged}
    [] Bug = "progress_cleared_changed"
       /\ c = ProgressCleared ->
      {ProgressChecked, ProgressChanged}
    [] Bug = "stride_window9_allows"
       /\ c = StrideWindow9Blocked ->
      (SpecActions(c) \ {StrideBlocks}) \cup {StrideAllows}
    [] Bug = "stride_window10_blocks"
       /\ c = StrideWindow10Eligible ->
      (SpecActions(c) \ {StrideAllows}) \cup {StrideBlocks}
    [] Bug = "stride_window27_allows"
       /\ c = StrideWindow27Blocked ->
      (SpecActions(c) \ {StrideBlocks}) \cup {StrideAllows}
    [] Bug = "stride_window81_allows"
       /\ c = StrideWindow81Blocked ->
      (SpecActions(c) \ {StrideBlocks}) \cup {StrideAllows}
    [] Bug = "stride_ignores_dependency"
       /\ c = NoDependencyStrideNoBlock ->
      (SpecActions(c) \ {StrideAllows}) \cup {StrideBlocks}
    [] Bug = "stride_ignores_previous_emit"
       /\ c = NoPreviousEmitStrideNoBlock ->
      (SpecActions(c) \ {StrideAllows}) \cup {StrideBlocks}
    [] Bug = "already_emitted_sends"
       /\ c = CanonicalWindowAlreadyEmitted ->
      (SpecActions(c) \ {NoSend, NoMark}) \cup {Send, MarkWindow}
    [] Bug = "stride_suppressed_sends"
       /\ c = CanonicalStrideSuppressed ->
      (SpecActions(c) \ {NoSend, NoMark}) \cup {Send, MarkWindow}
    [] Bug = "window0_uses_all_peers"
       /\ c = CanonicalWindow0Cohort ->
      (SpecActions(c) \ {Cohort12}) \cup {AllPeers}
    [] Bug = "window1_wrong_cohort"
       /\ c = CanonicalWindow1Cohort ->
      (SpecActions(c) \ {Cohort23}) \cup {Cohort12}
    [] Bug = "window2_not_all_peers"
       /\ c = CanonicalWindow2AllPeers ->
      (SpecActions(c) \ {AllPeers}) \cup {Cohort12}
    [] Bug = "small_peer_uses_cohort"
       /\ c = SmallPeerAllPeers ->
      (SpecActions(c) \ {AllPeers}) \cup {Cohort12}
    [] Bug = "skip_sort_dedup"
       /\ c = DuplicateTargetsDeduped ->
      SpecActions(c) \ {SortedDeduped}
    [] Bug = "empty_targets_sends"
       /\ c = EmptyTargetsNoSend ->
      (SpecActions(c) \ {NoSend, NoMark}) \cup {Send, MarkWindow}
    [] Bug = "cooldown_duplicate_sends"
       /\ c = CooldownDuplicate ->
      (SpecActions(c) \ {NoSend, NoMark, DedupBlocked}) \cup
        {Send, MarkWindow, DedupAllows}
    [] Bug = "mark_without_send"
       /\ c = EmptyTargetsNoSend ->
      (SpecActions(c) \ {NoMark}) \cup {MarkWindow}
    [] Bug = "skip_mark_on_send"
       /\ c = MarkOnlyAfterSend ->
      (SpecActions(c) \ {MarkWindow}) \cup {NoMark}
    [] Bug = "suppress_view_on_progress"
       /\ c = NoSuppressProgressAdvanced ->
      (SpecActions(c) \ {ViewAllows}) \cup {ViewSuppress}
    [] Bug = "suppress_wrong_cause"
       /\ c = NoSuppressMissingQcCause ->
      (SpecActions(c) \ {ViewAllows}) \cup {ViewSuppress}
    [] Bug = "suppress_when_reservation_available"
       /\ c = NoSuppressReservationAvailable ->
      (SpecActions(c) \ {ViewAllows}) \cup {ViewSuppress}
    [] Bug = "suppress_without_unresolved"
       /\ c = NoSuppressNoUnresolved ->
      (SpecActions(c) \ {ViewAllows}) \cup {ViewSuppress}
    [] Bug = "suppress_without_emit"
       /\ c = NoSuppressNoEmit ->
      (SpecActions(c) \ {ViewAllows}) \cup {ViewSuppress}
    [] OTHER -> SpecActions(c)

Init ==
  checked \in Cases

Next ==
  UNCHANGED checked

TypeInvariant ==
  /\ checked \in Cases
  /\ \A c \in Cases : SpecActions(c) \subseteq ActionUniverse
  /\ \A c \in Cases : ImplementationActions(c) \subseteq ActionUniverse

ReasonClassifierSafety ==
  /\ \A c \in CanonicalReasons : ImplementationActions(c) = SpecActions(c)
  /\ ImplementationActions(UnrelatedReason) = SpecActions(UnrelatedReason)

GateHeightSafety ==
  /\ ImplementationActions(NoCatchupTarget) = SpecActions(NoCatchupTarget)
  /\ ImplementationActions(CanonicalBehindTarget) = SpecActions(CanonicalBehindTarget)
  /\ ImplementationActions(CollapseCanonicalHeight) = SpecActions(CollapseCanonicalHeight)
  /\ ImplementationActions(GateActiveSameHeight) = SpecActions(GateActiveSameHeight)
  /\ ImplementationActions(GateActiveAheadHeight) = SpecActions(GateActiveAheadHeight)

SnapshotAndProgressSafety ==
  /\ \A c \in SnapshotCases : ImplementationActions(c) = SpecActions(c)
  /\ \A c \in ProgressCases : ImplementationActions(c) = SpecActions(c)

StrideSafety ==
  \A c \in StrideCases : ImplementationActions(c) = SpecActions(c)

RangePullSafety ==
  \A c \in RangePullCases : ImplementationActions(c) = SpecActions(c)

ViewSuppressionSafety ==
  \A c \in ViewSuppressionCases : ImplementationActions(c) = SpecActions(c)

SafetyFast ==
  /\ ReasonClassifierSafety
  /\ GateHeightSafety
  /\ SnapshotAndProgressSafety
  /\ StrideSafety
  /\ RangePullSafety
  /\ ViewSuppressionSafety

ReasonClassifierAnchors ==
  /\ ReasonClassifierSafety
  /\ \A c \in CanonicalReasons : ImplementationActions(c) = SpecActions(c)
  /\ ImplementationActions(UnrelatedReason) = SpecActions(UnrelatedReason)

GateHeightAnchors ==
  /\ GateHeightSafety
  /\ ImplementationActions(NoCatchupTarget) = SpecActions(NoCatchupTarget)
  /\ ImplementationActions(CanonicalBehindTarget) = SpecActions(CanonicalBehindTarget)
  /\ ImplementationActions(CollapseCanonicalHeight) = SpecActions(CollapseCanonicalHeight)
  /\ ImplementationActions(GateActiveSameHeight) = SpecActions(GateActiveSameHeight)
  /\ ImplementationActions(GateActiveAheadHeight) = SpecActions(GateActiveAheadHeight)

SnapshotAndProgressAnchors ==
  /\ SnapshotAndProgressSafety
  /\ \A c \in SnapshotCases : ImplementationActions(c) = SpecActions(c)
  /\ \A c \in ProgressCases : ImplementationActions(c) = SpecActions(c)

StrideAnchors ==
  /\ StrideSafety
  /\ \A c \in StrideCases : ImplementationActions(c) = SpecActions(c)

RangePullAnchors ==
  /\ RangePullSafety
  /\ \A c \in RangePullCases : ImplementationActions(c) = SpecActions(c)

ViewSuppressionAnchors ==
  /\ ViewSuppressionSafety
  /\ \A c \in ViewSuppressionCases : ImplementationActions(c) = SpecActions(c)

CanonicalFrontierReanchorSafetyAnchors ==
  /\ ReasonClassifierAnchors
  /\ GateHeightAnchors
  /\ SnapshotAndProgressAnchors
  /\ StrideAnchors
  /\ RangePullAnchors
  /\ ViewSuppressionAnchors

CanonicalFrontierReanchorExactness ==
  /\ ReasonClassifierAnchors
  /\ GateHeightAnchors
  /\ SnapshotAndProgressAnchors
  /\ StrideAnchors
  /\ RangePullAnchors
  /\ ViewSuppressionAnchors
CanonicalFrontierReanchorCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ CanonicalFrontierReanchorExactness

Safety ==
  CanonicalFrontierReanchorCorrectnessEnvelope

====
