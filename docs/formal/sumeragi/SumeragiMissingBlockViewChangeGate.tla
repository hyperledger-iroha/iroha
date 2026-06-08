---- MODULE SumeragiMissingBlockViewChangeGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for missing-block view-change escalation.

This slice models the recovery boundary formed by
`MissingBlockRequest::can_trigger_view_change`,
`MissingBlockRequest::view_change_due`,
`MissingBlockRequest::mark_view_change_if_due`,
`missing_block_next_due`,
`should_defer_missing_block_view_change`, and
`clear_missing_block_view_change`. Consensus-priority requests may arm a
view-change only after their configured dwell window, current-view latches and
last-trigger timestamps throttle repeated rotations, backlog/progress signals
defer escalation, scheduler deadlines include only armed untriggered windows,
and clearing a recovered block drops the view-change window/latch without
removing the request itself.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Set(Int);
  tried

\* @type: <<Set(Int)>>;
vars == <<tried>>

TlcSingletonOrEmpty == Cardinality(tried) \in {0, 1}

ConsensusCanTrigger == 1
BackgroundCannotTrigger == 2
MissingWindowNotDue == 3
ZeroWindowNotDue == 4
EarlyDwellNotDue == 5
DwellBoundaryDue == 6
CurrentViewLatchSuppresses == 7
PriorViewLatchAllows == 8
MissingLastTriggerAllows == 9
RecentLastTriggerThrottles == 10
LastTriggerBoundaryDue == 11
MarkDueReturnsTrue == 12
MarkDueLatchesCurrentView == 13
MarkDueRecordsNow == 14
MarkNotDueReturnsFalse == 15
MarkNotDueLeavesState == 16
ClearDropsWindow == 17
ClearDropsTriggeredView == 18
ClearKeepsRequest == 19
SchedulerIncludesViewDeadline == 20
SchedulerSkipsCurrentViewLatch == 21
SchedulerSkipsMissingWindow == 22
SchedulerSkipsZeroWindow == 23
DeferRecentDependencyProgress == 24
DeferRecentRbcProgress == 25
DeferRecentDependencyAcrossView == 26
DeferInflightRangePullProgress == 27
StaleProgressNoDefer == 28
BacklogBeforeExtensionDefers == 29
BacklogAfterExtensionNoDefer == 30

Candidates == 1..30

NoBug == 0
BackgroundCanTriggerBug == 1
MissingWindowTriggersBug == 2
ZeroWindowTriggersBug == 3
EarlyDwellTriggersBug == 4
DwellBoundaryDroppedBug == 5
CurrentViewRetriggersBug == 6
OldViewLatchBlocksNewViewBug == 7
MissingLastTriggerRejectedBug == 8
RecentLastTriggerIgnoredBug == 9
LastTriggerBoundaryDroppedBug == 10
MarkDueReturnsFalseBug == 11
SkipTriggerViewRecordBug == 12
WrongTriggeredViewBug == 13
SkipLastTriggerRecordBug == 14
MarkWithoutDueBug == 15
MarkNotDueMutatesBug == 16
ClearKeepsWindowBug == 17
ClearKeepsTriggerBug == 18
ClearRemovesRequestBug == 19
SchedulerIgnoresViewWindowBug == 20
SchedulerReschedulesCurrentViewBug == 21
SchedulerArmsMissingWindowBug == 22
SchedulerArmsZeroWindowBug == 23
IgnoreDependencyProgressBug == 24
IgnoreRbcProgressBug == 25
RequireViewMatchForProgressBug == 26
IgnoreRangePullProgressBug == 27
DeferStaleProgressBug == 28
IgnoreBacklogBug == 29
BacklogSticksForeverBug == 30

Bugs == 0..30

BugBackgroundCanTrigger == Bug = BackgroundCanTriggerBug
BugMissingWindowTriggers == Bug = MissingWindowTriggersBug
BugZeroWindowTriggers == Bug = ZeroWindowTriggersBug
BugEarlyDwellTriggers == Bug = EarlyDwellTriggersBug
BugDwellBoundaryDropped == Bug = DwellBoundaryDroppedBug
BugCurrentViewRetriggers == Bug = CurrentViewRetriggersBug
BugOldViewLatchBlocksNewView == Bug = OldViewLatchBlocksNewViewBug
BugMissingLastTriggerRejected == Bug = MissingLastTriggerRejectedBug
BugRecentLastTriggerIgnored == Bug = RecentLastTriggerIgnoredBug
BugLastTriggerBoundaryDropped == Bug = LastTriggerBoundaryDroppedBug
BugMarkDueReturnsFalse == Bug = MarkDueReturnsFalseBug
BugSkipTriggerViewRecord == Bug = SkipTriggerViewRecordBug
BugWrongTriggeredView == Bug = WrongTriggeredViewBug
BugSkipLastTriggerRecord == Bug = SkipLastTriggerRecordBug
BugMarkWithoutDue == Bug = MarkWithoutDueBug
BugMarkNotDueMutates == Bug = MarkNotDueMutatesBug
BugClearKeepsWindow == Bug = ClearKeepsWindowBug
BugClearKeepsTrigger == Bug = ClearKeepsTriggerBug
BugClearRemovesRequest == Bug = ClearRemovesRequestBug
BugSchedulerIgnoresViewWindow == Bug = SchedulerIgnoresViewWindowBug
BugSchedulerReschedulesCurrentView == Bug = SchedulerReschedulesCurrentViewBug
BugSchedulerArmsMissingWindow == Bug = SchedulerArmsMissingWindowBug
BugSchedulerArmsZeroWindow == Bug = SchedulerArmsZeroWindowBug
BugIgnoreDependencyProgress == Bug = IgnoreDependencyProgressBug
BugIgnoreRbcProgress == Bug = IgnoreRbcProgressBug
BugRequireViewMatchForProgress == Bug = RequireViewMatchForProgressBug
BugIgnoreRangePullProgress == Bug = IgnoreRangePullProgressBug
BugDeferStaleProgress == Bug = DeferStaleProgressBug
BugIgnoreBacklog == Bug = IgnoreBacklogBug
BugBacklogSticksForever == Bug = BacklogSticksForeverBug

NoView == 0
CurrentView == 1
OtherView == 2

DueCandidates == {
  BackgroundCannotTrigger,
  MissingWindowNotDue,
  ZeroWindowNotDue,
  EarlyDwellNotDue,
  DwellBoundaryDue,
  CurrentViewLatchSuppresses,
  PriorViewLatchAllows,
  MissingLastTriggerAllows,
  RecentLastTriggerThrottles,
  LastTriggerBoundaryDue
}

SpecViewChangeDue(candidate) ==
  candidate \in {
    DwellBoundaryDue,
    PriorViewLatchAllows,
    MissingLastTriggerAllows,
    LastTriggerBoundaryDue
  }

ImplementationCanTrigger(candidate) ==
  CASE candidate = ConsensusCanTrigger -> TRUE
    [] candidate = BackgroundCannotTrigger -> BugBackgroundCanTrigger
    [] OTHER -> FALSE

ImplementationViewChangeDue(candidate) ==
  CASE candidate = BackgroundCannotTrigger -> BugBackgroundCanTrigger
    [] candidate = MissingWindowNotDue -> BugMissingWindowTriggers
    [] candidate = ZeroWindowNotDue -> BugZeroWindowTriggers
    [] candidate = EarlyDwellNotDue -> BugEarlyDwellTriggers
    [] candidate = DwellBoundaryDue -> ~BugDwellBoundaryDropped
    [] candidate = CurrentViewLatchSuppresses -> BugCurrentViewRetriggers
    [] candidate = PriorViewLatchAllows -> ~BugOldViewLatchBlocksNewView
    [] candidate = MissingLastTriggerAllows -> ~BugMissingLastTriggerRejected
    [] candidate = RecentLastTriggerThrottles -> BugRecentLastTriggerIgnored
    [] candidate = LastTriggerBoundaryDue -> ~BugLastTriggerBoundaryDropped
    [] OTHER -> FALSE

ImplementationMarkReturnsTrue(candidate) ==
  CASE candidate \in {MarkDueReturnsTrue, MarkDueLatchesCurrentView, MarkDueRecordsNow}
        -> ~BugMarkDueReturnsFalse
    [] candidate \in {MarkNotDueReturnsFalse, MarkNotDueLeavesState}
        -> BugMarkWithoutDue
    [] OTHER -> FALSE

ImplementationTriggeredView(candidate) ==
  CASE candidate = MarkDueLatchesCurrentView ->
        IF BugSkipTriggerViewRecord THEN NoView
        ELSE IF BugWrongTriggeredView THEN OtherView
        ELSE CurrentView
    [] candidate = MarkNotDueLeavesState ->
        IF BugMarkNotDueMutates THEN OtherView ELSE NoView
    [] OTHER -> NoView

ImplementationLastTriggerNow(candidate) ==
  /\ candidate = MarkDueRecordsNow
  /\ ~BugSkipLastTriggerRecord

ImplementationWindowCleared(candidate) ==
  /\ candidate = ClearDropsWindow
  /\ ~BugClearKeepsWindow

ImplementationTriggeredViewCleared(candidate) ==
  /\ candidate = ClearDropsTriggeredView
  /\ ~BugClearKeepsTrigger

ImplementationRequestStillTracked(candidate) ==
  /\ candidate = ClearKeepsRequest
  /\ ~BugClearRemovesRequest

ImplementationSchedulesViewDeadline(candidate) ==
  CASE candidate = SchedulerIncludesViewDeadline -> ~BugSchedulerIgnoresViewWindow
    [] candidate = SchedulerSkipsCurrentViewLatch -> BugSchedulerReschedulesCurrentView
    [] candidate = SchedulerSkipsMissingWindow -> BugSchedulerArmsMissingWindow
    [] candidate = SchedulerSkipsZeroWindow -> BugSchedulerArmsZeroWindow
    [] OTHER -> FALSE

ImplementationShouldDefer(candidate) ==
  CASE candidate = DeferRecentDependencyProgress -> ~BugIgnoreDependencyProgress
    [] candidate = DeferRecentRbcProgress -> ~BugIgnoreRbcProgress
    [] candidate = DeferRecentDependencyAcrossView -> ~BugRequireViewMatchForProgress
    [] candidate = DeferInflightRangePullProgress -> ~BugIgnoreRangePullProgress
    [] candidate = StaleProgressNoDefer -> BugDeferStaleProgress
    [] candidate = BacklogBeforeExtensionDefers -> ~BugIgnoreBacklog
    [] candidate = BacklogAfterExtensionNoDefer -> BugBacklogSticksForever
    [] OTHER -> FALSE

TypeInvariant ==
  /\ Bug \in Bugs
  /\ tried \subseteq Candidates

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

ViewChangeAuthority ==
  /\ ConsensusCanTrigger \in tried =>
       ImplementationCanTrigger(ConsensusCanTrigger)
  /\ BackgroundCannotTrigger \in tried =>
       ~ImplementationCanTrigger(BackgroundCannotTrigger)

ViewChangeDueMatchesSpec ==
  \A candidate \in tried:
    candidate \in DueCandidates =>
      ImplementationViewChangeDue(candidate) <=> SpecViewChangeDue(candidate)

MarkingSemantics ==
  /\ MarkDueReturnsTrue \in tried =>
       ImplementationMarkReturnsTrue(MarkDueReturnsTrue)
  /\ MarkDueLatchesCurrentView \in tried =>
       /\ ImplementationMarkReturnsTrue(MarkDueLatchesCurrentView)
       /\ ImplementationTriggeredView(MarkDueLatchesCurrentView) = CurrentView
  /\ MarkDueRecordsNow \in tried =>
       /\ ImplementationMarkReturnsTrue(MarkDueRecordsNow)
       /\ ImplementationLastTriggerNow(MarkDueRecordsNow)
  /\ MarkNotDueReturnsFalse \in tried =>
       ~ImplementationMarkReturnsTrue(MarkNotDueReturnsFalse)
  /\ MarkNotDueLeavesState \in tried =>
       /\ ~ImplementationMarkReturnsTrue(MarkNotDueLeavesState)
       /\ ImplementationTriggeredView(MarkNotDueLeavesState) = NoView

ClearDropsOnlyViewChangeState ==
  /\ ClearDropsWindow \in tried =>
       ImplementationWindowCleared(ClearDropsWindow)
  /\ ClearDropsTriggeredView \in tried =>
       ImplementationTriggeredViewCleared(ClearDropsTriggeredView)
  /\ ClearKeepsRequest \in tried =>
       ImplementationRequestStillTracked(ClearKeepsRequest)

SchedulerMatchesArmedWindows ==
  /\ SchedulerIncludesViewDeadline \in tried =>
       ImplementationSchedulesViewDeadline(SchedulerIncludesViewDeadline)
  /\ SchedulerSkipsCurrentViewLatch \in tried =>
       ~ImplementationSchedulesViewDeadline(SchedulerSkipsCurrentViewLatch)
  /\ SchedulerSkipsMissingWindow \in tried =>
       ~ImplementationSchedulesViewDeadline(SchedulerSkipsMissingWindow)
  /\ SchedulerSkipsZeroWindow \in tried =>
       ~ImplementationSchedulesViewDeadline(SchedulerSkipsZeroWindow)

DeferralMatchesProgressAndBacklog ==
  /\ DeferRecentDependencyProgress \in tried =>
       ImplementationShouldDefer(DeferRecentDependencyProgress)
  /\ DeferRecentRbcProgress \in tried =>
       ImplementationShouldDefer(DeferRecentRbcProgress)
  /\ DeferRecentDependencyAcrossView \in tried =>
       ImplementationShouldDefer(DeferRecentDependencyAcrossView)
  /\ DeferInflightRangePullProgress \in tried =>
       ImplementationShouldDefer(DeferInflightRangePullProgress)
  /\ StaleProgressNoDefer \in tried =>
       ~ImplementationShouldDefer(StaleProgressNoDefer)
  /\ BacklogBeforeExtensionDefers \in tried =>
       ImplementationShouldDefer(BacklogBeforeExtensionDefers)
  /\ BacklogAfterExtensionNoDefer \in tried =>
       ~ImplementationShouldDefer(BacklogAfterExtensionNoDefer)

ViewChangeAuthorityCases == {
  ConsensusCanTrigger, BackgroundCannotTrigger
}

ViewChangeDueWindowCases == {
  MissingWindowNotDue, ZeroWindowNotDue, EarlyDwellNotDue, DwellBoundaryDue,
  CurrentViewLatchSuppresses, PriorViewLatchAllows, MissingLastTriggerAllows,
  RecentLastTriggerThrottles, LastTriggerBoundaryDue
}

ViewChangeMarkCases == {
  MarkDueReturnsTrue, MarkDueLatchesCurrentView, MarkDueRecordsNow,
  MarkNotDueReturnsFalse, MarkNotDueLeavesState
}

ViewChangeClearCases == {
  ClearDropsWindow, ClearDropsTriggeredView, ClearKeepsRequest
}

ViewChangeSchedulerCases == {
  SchedulerIncludesViewDeadline, SchedulerSkipsCurrentViewLatch,
  SchedulerSkipsMissingWindow, SchedulerSkipsZeroWindow
}

ViewChangeDeferralCases == {
  DeferRecentDependencyProgress, DeferRecentRbcProgress,
  DeferRecentDependencyAcrossView, DeferInflightRangePullProgress,
  StaleProgressNoDefer, BacklogBeforeExtensionDefers,
  BacklogAfterExtensionNoDefer
}

MissingBlockViewChangeGroupedCases ==
  ViewChangeAuthorityCases \cup ViewChangeDueWindowCases \cup
  ViewChangeMarkCases \cup ViewChangeClearCases \cup
  ViewChangeSchedulerCases \cup ViewChangeDeferralCases

MissingBlockViewChangeCaseGroupsComplete ==
  MissingBlockViewChangeGroupedCases = Candidates

MissingBlockViewChangeAuthorityExact ==
  ViewChangeAuthority

MissingBlockViewChangeDueExact ==
  ViewChangeDueMatchesSpec

MissingBlockViewChangeMarkExact ==
  MarkingSemantics

MissingBlockViewChangeClearExact ==
  ClearDropsOnlyViewChangeState

MissingBlockViewChangeSchedulerExact ==
  SchedulerMatchesArmedWindows

MissingBlockViewChangeDeferralExact ==
  DeferralMatchesProgressAndBacklog

MissingBlockViewChangeExactness ==
  /\ MissingBlockViewChangeCaseGroupsComplete
  /\ MissingBlockViewChangeAuthorityExact
  /\ MissingBlockViewChangeDueExact
  /\ MissingBlockViewChangeMarkExact
  /\ MissingBlockViewChangeClearExact
  /\ MissingBlockViewChangeSchedulerExact
  /\ MissingBlockViewChangeDeferralExact

Safety ==
  MissingBlockViewChangeExactness

====
