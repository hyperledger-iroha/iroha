---- MODULE SumeragiMissingBlockHardCapGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for missing-block hard-cap recovery escalation.

This slice models the decisive branch of
`maybe_escalate_missing_block_height_recovery(...)`: stalled missing-block
recovery may rotate the active contiguous height only after the hard cap is due
and all convergence, view, priority, stall-window, and duplicate-escalation
guards allow it. It also models the special lock-lag override, duplicate
same-view budget sealing, request latch side effects, no-actionable cleanup,
and range-pull-only progress that must not become a view change.
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

NoHardCapNoTrigger == 1
HardCapTriggers == 2
RecentDependencyProgressDefers == 3
RecentRbcProgressDefers == 4
InflightRangePullProgressDefers == 5
AlreadyTriggeredSuppresses == 6
AlreadyTriggeredSealsBudget == 7
NonActiveHeightSuppresses == 8
NonContiguousHeightSuppresses == 9
BackgroundPrioritySuppresses == 10
AdvancedCurrentViewSuppresses == 11
EscalatedViewSuppresses == 12
TierAdvanceDefers == 13
ViewChangeDeferralSuppresses == 14
StallWindowUnavailableSuppresses == 15
StallWindowAvailableTriggers == 16
LockLagOverrideTriggers == 17
LockLagBackgroundSuppresses == 18
LockLagAdvancedViewSuppresses == 19
LockLagAlreadyTriggeredSuppresses == 20
LockLagEscalatedSuppresses == 21
TriggerRecordsBudget == 22
TriggerLatchesRequest == 23
TriggerUsesMissingPayloadCause == 24
SuppressedDoesNotLatch == 25
NoActionableClearsRecovery == 26
NoActionableNoProgress == 27
RangePullBeforeHardCapNoViewChange == 28

Candidates == 1..28

NoBug == 0
TriggerBeforeHardCapBug == 1
DropHardCapTriggerBug == 2
IgnoreDependencyProgressBug == 3
IgnoreRbcProgressBug == 4
IgnoreInflightRangePullBug == 5
RetriggerCurrentViewBug == 6
SkipAlreadyTriggeredBudgetSealBug == 7
TriggerNonActiveHeightBug == 8
TriggerNonContiguousHeightBug == 9
TriggerBackgroundPriorityBug == 10
IgnoreAdvancedCurrentViewBug == 11
IgnoreEscalatedViewBug == 12
IgnoreTierDeferralBug == 13
IgnoreViewChangeDeferralBug == 14
IgnoreStallWindowBug == 15
DropStallWindowTriggerBug == 16
RequireStallWindowForLockLagBug == 17
LockLagTriggersBackgroundBug == 18
LockLagIgnoresAdvancedViewBug == 19
LockLagIgnoresAlreadyTriggeredBug == 20
LockLagIgnoresEscalatedViewBug == 21
SkipBudgetEscalatedRecordBug == 22
SkipRequestLatchBug == 23
WrongViewChangeCauseBug == 24
LatchOnSuppressedBug == 25
KeepRecoveryWithoutActionableBug == 26
ProgressWithoutActionableBug == 27
RangePullTriggersViewChangeBug == 28

Bugs == 0..28

BugTriggerBeforeHardCap == Bug = TriggerBeforeHardCapBug
BugDropHardCapTrigger == Bug = DropHardCapTriggerBug
BugIgnoreDependencyProgress == Bug = IgnoreDependencyProgressBug
BugIgnoreRbcProgress == Bug = IgnoreRbcProgressBug
BugIgnoreInflightRangePull == Bug = IgnoreInflightRangePullBug
BugRetriggerCurrentView == Bug = RetriggerCurrentViewBug
BugSkipAlreadyTriggeredBudgetSeal == Bug = SkipAlreadyTriggeredBudgetSealBug
BugTriggerNonActiveHeight == Bug = TriggerNonActiveHeightBug
BugTriggerNonContiguousHeight == Bug = TriggerNonContiguousHeightBug
BugTriggerBackgroundPriority == Bug = TriggerBackgroundPriorityBug
BugIgnoreAdvancedCurrentView == Bug = IgnoreAdvancedCurrentViewBug
BugIgnoreEscalatedView == Bug = IgnoreEscalatedViewBug
BugIgnoreTierDeferral == Bug = IgnoreTierDeferralBug
BugIgnoreViewChangeDeferral == Bug = IgnoreViewChangeDeferralBug
BugIgnoreStallWindow == Bug = IgnoreStallWindowBug
BugDropStallWindowTrigger == Bug = DropStallWindowTriggerBug
BugRequireStallWindowForLockLag == Bug = RequireStallWindowForLockLagBug
BugLockLagTriggersBackground == Bug = LockLagTriggersBackgroundBug
BugLockLagIgnoresAdvancedView == Bug = LockLagIgnoresAdvancedViewBug
BugLockLagIgnoresAlreadyTriggered == Bug = LockLagIgnoresAlreadyTriggeredBug
BugLockLagIgnoresEscalatedView == Bug = LockLagIgnoresEscalatedViewBug
BugSkipBudgetEscalatedRecord == Bug = SkipBudgetEscalatedRecordBug
BugSkipRequestLatch == Bug = SkipRequestLatchBug
BugWrongViewChangeCause == Bug = WrongViewChangeCauseBug
BugLatchOnSuppressed == Bug = LatchOnSuppressedBug
BugKeepRecoveryWithoutActionable == Bug = KeepRecoveryWithoutActionableBug
BugProgressWithoutActionable == Bug = ProgressWithoutActionableBug
BugRangePullTriggersViewChange == Bug = RangePullTriggersViewChangeBug

DecisionCandidates == {
  NoHardCapNoTrigger,
  HardCapTriggers,
  RecentDependencyProgressDefers,
  RecentRbcProgressDefers,
  InflightRangePullProgressDefers,
  AlreadyTriggeredSuppresses,
  NonActiveHeightSuppresses,
  NonContiguousHeightSuppresses,
  BackgroundPrioritySuppresses,
  AdvancedCurrentViewSuppresses,
  EscalatedViewSuppresses,
  TierAdvanceDefers,
  ViewChangeDeferralSuppresses,
  StallWindowUnavailableSuppresses,
  StallWindowAvailableTriggers,
  LockLagOverrideTriggers,
  LockLagBackgroundSuppresses,
  LockLagAdvancedViewSuppresses,
  LockLagAlreadyTriggeredSuppresses,
  LockLagEscalatedSuppresses,
  RangePullBeforeHardCapNoViewChange
}

SpecTriggers(candidate) ==
  candidate \in {
    HardCapTriggers,
    StallWindowAvailableTriggers,
    LockLagOverrideTriggers
  }

ImplementationTriggers(candidate) ==
  CASE candidate = NoHardCapNoTrigger -> BugTriggerBeforeHardCap
    [] candidate = HardCapTriggers -> ~BugDropHardCapTrigger
    [] candidate = RecentDependencyProgressDefers -> BugIgnoreDependencyProgress
    [] candidate = RecentRbcProgressDefers -> BugIgnoreRbcProgress
    [] candidate = InflightRangePullProgressDefers -> BugIgnoreInflightRangePull
    [] candidate = AlreadyTriggeredSuppresses -> BugRetriggerCurrentView
    [] candidate = NonActiveHeightSuppresses -> BugTriggerNonActiveHeight
    [] candidate = NonContiguousHeightSuppresses -> BugTriggerNonContiguousHeight
    [] candidate = BackgroundPrioritySuppresses -> BugTriggerBackgroundPriority
    [] candidate = AdvancedCurrentViewSuppresses -> BugIgnoreAdvancedCurrentView
    [] candidate = EscalatedViewSuppresses -> BugIgnoreEscalatedView
    [] candidate = TierAdvanceDefers -> BugIgnoreTierDeferral
    [] candidate = ViewChangeDeferralSuppresses -> BugIgnoreViewChangeDeferral
    [] candidate = StallWindowUnavailableSuppresses -> BugIgnoreStallWindow
    [] candidate = StallWindowAvailableTriggers -> ~BugDropStallWindowTrigger
    [] candidate = LockLagOverrideTriggers -> ~BugRequireStallWindowForLockLag
    [] candidate = LockLagBackgroundSuppresses -> BugLockLagTriggersBackground
    [] candidate = LockLagAdvancedViewSuppresses -> BugLockLagIgnoresAdvancedView
    [] candidate = LockLagAlreadyTriggeredSuppresses -> BugLockLagIgnoresAlreadyTriggered
    [] candidate = LockLagEscalatedSuppresses -> BugLockLagIgnoresEscalatedView
    [] candidate = RangePullBeforeHardCapNoViewChange -> BugRangePullTriggersViewChange
    [] OTHER -> FALSE

ImplementationBudgetSealed(candidate) ==
  CASE candidate = AlreadyTriggeredSealsBudget -> ~BugSkipAlreadyTriggeredBudgetSeal
    [] candidate = TriggerRecordsBudget -> ~BugSkipBudgetEscalatedRecord
    [] OTHER -> FALSE

ImplementationRequestLatched(candidate) ==
  CASE candidate = TriggerLatchesRequest -> ~BugSkipRequestLatch
    [] candidate = SuppressedDoesNotLatch -> BugLatchOnSuppressed
    [] OTHER -> FALSE

ImplementationMissingPayloadCause(candidate) ==
  /\ candidate = TriggerUsesMissingPayloadCause
  /\ ~BugWrongViewChangeCause

ImplementationRecoveryCleared(candidate) ==
  /\ candidate = NoActionableClearsRecovery
  /\ ~BugKeepRecoveryWithoutActionable

ImplementationProgressed(candidate) ==
  CASE candidate = NoActionableNoProgress -> BugProgressWithoutActionable
    [] candidate = RangePullBeforeHardCapNoViewChange -> TRUE
    [] candidate \in {HardCapTriggers, StallWindowAvailableTriggers, LockLagOverrideTriggers}
        -> ImplementationTriggers(candidate)
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

HardCapDecisionMatchesSpec ==
  \A candidate \in tried:
    candidate \in DecisionCandidates =>
      ImplementationTriggers(candidate) <=> SpecTriggers(candidate)

DuplicateAndTriggerSideEffects ==
  /\ AlreadyTriggeredSealsBudget \in tried =>
       ImplementationBudgetSealed(AlreadyTriggeredSealsBudget)
  /\ TriggerRecordsBudget \in tried =>
       ImplementationBudgetSealed(TriggerRecordsBudget)
  /\ TriggerLatchesRequest \in tried =>
       ImplementationRequestLatched(TriggerLatchesRequest)
  /\ TriggerUsesMissingPayloadCause \in tried =>
       ImplementationMissingPayloadCause(TriggerUsesMissingPayloadCause)
  /\ SuppressedDoesNotLatch \in tried =>
       ~ImplementationRequestLatched(SuppressedDoesNotLatch)

NoActionableCleanupIsTerminal ==
  /\ NoActionableClearsRecovery \in tried =>
       ImplementationRecoveryCleared(NoActionableClearsRecovery)
  /\ NoActionableNoProgress \in tried =>
       ~ImplementationProgressed(NoActionableNoProgress)

RangePullProgressDoesNotRotate ==
  RangePullBeforeHardCapNoViewChange \in tried =>
    /\ ImplementationProgressed(RangePullBeforeHardCapNoViewChange)
    /\ ~ImplementationTriggers(RangePullBeforeHardCapNoViewChange)

Safety ==
  /\ HardCapDecisionMatchesSpec
  /\ DuplicateAndTriggerSideEffects
  /\ NoActionableCleanupIsTerminal
  /\ RangePullProgressDoesNotRotate

====
