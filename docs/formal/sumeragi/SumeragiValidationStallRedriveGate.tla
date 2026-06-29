---- MODULE SumeragiValidationStallRedriveGate ----
EXTENDS Naturals, Integers

(***************************************************************************
A bounded abstract model for validation stall/freshness and vNext redrive
helpers in `main_loop/validation.rs`.

The Rust helpers decide whether asynchronous block validation is still fresh,
whether inline fallback may proceed, and whether a vNext validation slot should
be redriven. This model keeps the data finite while preserving the correctness
surface: stall timeouts are the maximum of inline, EMA, and DA-entrypoint
floors clamped by the effective cap; non-vNext validation requires connected
worker queues and matching frontier ownership; elapsed-time boundaries use
strict freshness and inclusive stall/redrive thresholds; terminal or missing
vNext slots do not redrive.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoneResult == "none"
SomeResult == "some"
WorkerDisconnected == "worker_disconnected"
StaleFrontier == "stale_frontier"
Stalled == "stalled"
OrphanedQueued == "orphaned_queued"
OrphanedRunning == "orphaned_running"
StalledRunning == "stalled_running"
Backpressured == "backpressured"

BaseInline == "base_inline"
EmaWins == "ema_wins"
ExtraWins == "extra_wins"
CapClamps == "cap_clamps"
NonDaCap == "non_da_cap"
DaNoPendingExtraZero == "da_no_pending_extra_zero"
DaPendingExtra == "da_pending_extra"

StallCases == {
  BaseInline,
  EmaWins,
  ExtraWins,
  CapClamps,
  NonDaCap,
  DaNoPendingExtraZero,
  DaPendingExtra
}

FreshMissingInflight == "fresh_missing_inflight"
FreshDisconnected == "fresh_disconnected"
FreshStaleFrontier == "fresh_stale_frontier"
FreshBeforeTimeout == "fresh_before_timeout"
FreshAtTimeout == "fresh_at_timeout"
FreshAfterTimeout == "fresh_after_timeout"
FreshVnextDisconnectedOk == "fresh_vnext_disconnected_ok"

FreshCases == {
  FreshMissingInflight,
  FreshDisconnected,
  FreshStaleFrontier,
  FreshBeforeTimeout,
  FreshAtTimeout,
  FreshAfterTimeout,
  FreshVnextDisconnectedOk
}

InlineMissingInflight == "inline_missing_inflight"
InlineDisconnected == "inline_disconnected"
InlineStaleFrontier == "inline_stale_frontier"
InlineBeforeTimeout == "inline_before_timeout"
InlineAtTimeout == "inline_at_timeout"
InlineAfterTimeout == "inline_after_timeout"
InlineVnextDisconnectedStillFresh == "inline_vnext_disconnected_still_fresh"

InlineCases == {
  InlineMissingInflight,
  InlineDisconnected,
  InlineStaleFrontier,
  InlineBeforeTimeout,
  InlineAtTimeout,
  InlineAfterTimeout,
  InlineVnextDisconnectedStillFresh
}

RedriveMissingSlot == "redrive_missing_slot"
RedriveTerminalCommitted == "redrive_terminal_committed"
RedriveQueuedBeforeTimeout == "redrive_queued_before_timeout"
RedriveQueuedAtTimeout == "redrive_queued_at_timeout"
RedriveQueuedAfterTimeout == "redrive_queued_after_timeout"
RedriveRunningOrphanNoInflight == "redrive_running_orphan_no_inflight"
RedriveRunningOrphanNoVnext == "redrive_running_orphan_no_vnext"
RedriveRunningFresh == "redrive_running_fresh"
RedriveRunningStalled == "redrive_running_stalled"
RedriveBackpressuredBeforeTimeout == "redrive_backpressured_before_timeout"
RedriveBackpressuredAtTimeout == "redrive_backpressured_at_timeout"
RedriveBackpressuredAfterTimeout == "redrive_backpressured_after_timeout"
RedriveUnqueued == "redrive_unqueued"
RedriveValid == "redrive_valid"
RedriveInvalid == "redrive_invalid"

RedriveCases == {
  RedriveMissingSlot,
  RedriveTerminalCommitted,
  RedriveQueuedBeforeTimeout,
  RedriveQueuedAtTimeout,
  RedriveQueuedAfterTimeout,
  RedriveRunningOrphanNoInflight,
  RedriveRunningOrphanNoVnext,
  RedriveRunningFresh,
  RedriveRunningStalled,
  RedriveBackpressuredBeforeTimeout,
  RedriveBackpressuredAtTimeout,
  RedriveBackpressuredAfterTimeout,
  RedriveUnqueued,
  RedriveValid,
  RedriveInvalid
}

FreshOutputs == {NoneResult, SomeResult}
InlineOutputs == {NoneResult, WorkerDisconnected, StaleFrontier, Stalled}
RedriveOutputs ==
  {NoneResult, OrphanedQueued, OrphanedRunning, StalledRunning, Backpressured}

Max(a, b) == IF a >= b THEN a ELSE b

Min(a, b) == IF a <= b THEN a ELSE b

Max3(a, b, c) == Max(Max(a, b), c)

InlineFloor(c) ==
  CASE c = BaseInline -> 10
    [] c = EmaWins -> 10
    [] c = ExtraWins -> 10
    [] c = CapClamps -> 50
    [] c = NonDaCap -> 20
    [] c = DaNoPendingExtraZero -> 10
    [] c = DaPendingExtra -> 10
    [] OTHER -> 0

EmaFloor(c) ==
  CASE c = BaseInline -> 0
    [] c = EmaWins -> 30
    [] c = ExtraWins -> 20
    [] c = CapClamps -> 70
    [] c = NonDaCap -> 40
    [] c = DaNoPendingExtraZero -> 0
    [] c = DaPendingExtra -> 15
    [] OTHER -> 0

DaEnabled(c) ==
  c \in {ExtraWins, CapClamps, DaNoPendingExtraZero, DaPendingExtra}

PendingBlock(c) ==
  c \in {ExtraWins, CapClamps, DaPendingExtra}

PendingEntryExtra(c) ==
  CASE c = ExtraWins -> 40
    [] c = CapClamps -> 90
    [] c = DaNoPendingExtraZero -> 40
    [] c = DaPendingExtra -> 32
    [] OTHER -> 0

ExtraFloor(c) ==
  IF DaEnabled(c) /\ PendingBlock(c) THEN PendingEntryExtra(c) ELSE 0

DaCap(c) ==
  CASE c = CapClamps -> 60
    [] OTHER -> 100

NonDaCapValue(c) ==
  CASE c = CapClamps -> 25
    [] c = NonDaCap -> 25
    [] OTHER -> 100

Cap(c) ==
  IF DaEnabled(c) THEN DaCap(c) ELSE NonDaCapValue(c)

SpecStallTimeout(c) ==
  Min(Max3(InlineFloor(c), EmaFloor(c), ExtraFloor(c)), Cap(c))

ActualStallTimeout(c) ==
  CASE Bug = "stall_drops_inline_floor" /\ c = BaseInline ->
      Min(Max(EmaFloor(c), ExtraFloor(c)), Cap(c))
    [] Bug = "stall_drops_ema_floor" /\ c = EmaWins ->
      Min(Max(InlineFloor(c), ExtraFloor(c)), Cap(c))
    [] Bug = "stall_drops_extra_floor" /\ c = ExtraWins ->
      Min(Max(InlineFloor(c), EmaFloor(c)), Cap(c))
    [] Bug = "stall_ignores_cap" /\ c = CapClamps ->
      Max3(InlineFloor(c), EmaFloor(c), ExtraFloor(c))
    [] Bug = "stall_uses_non_da_cap" /\ c = CapClamps ->
      Min(Max3(InlineFloor(c), EmaFloor(c), ExtraFloor(c)), NonDaCapValue(c))
    [] Bug = "stall_da_extra_without_pending" /\ c = DaNoPendingExtraZero ->
      Min(Max3(InlineFloor(c), EmaFloor(c), PendingEntryExtra(c)), Cap(c))
    [] OTHER -> SpecStallTimeout(c)

FreshHasInflight(c) ==
  c # FreshMissingInflight

FreshWorkerConnected(c) ==
  c \notin {FreshDisconnected, FreshVnextDisconnectedOk}

FreshVnextOwned(c) ==
  c = FreshVnextDisconnectedOk

FreshFrontierMatches(c) ==
  c # FreshStaleFrontier

FreshTimeout(c) == 10

FreshElapsed(c) ==
  CASE c = FreshBeforeTimeout -> 9
    [] c = FreshAtTimeout -> 10
    [] c = FreshAfterTimeout -> 11
    [] c = FreshVnextDisconnectedOk -> 9
    [] OTHER -> 0

SpecFresh(c) ==
  IF ~FreshHasInflight(c)
  THEN NoneResult
  ELSE IF ~FreshVnextOwned(c) /\ ~FreshWorkerConnected(c)
  THEN NoneResult
  ELSE IF ~FreshFrontierMatches(c)
  THEN NoneResult
  ELSE IF FreshElapsed(c) < FreshTimeout(c)
  THEN SomeResult
  ELSE NoneResult

ActualFresh(c) ==
  CASE Bug = "fresh_missing_inflight_returns_timeout"
       /\ c = FreshMissingInflight ->
      SomeResult
    [] Bug = "fresh_disconnected_worker_returns_timeout"
       /\ c = FreshDisconnected ->
      SomeResult
    [] Bug = "fresh_stale_frontier_returns_timeout"
       /\ c = FreshStaleFrontier ->
      SomeResult
    [] Bug = "fresh_boundary_stays_fresh"
       /\ c = FreshAtTimeout ->
      SomeResult
    [] Bug = "fresh_before_timeout_none"
       /\ c = FreshBeforeTimeout ->
      NoneResult
    [] OTHER -> SpecFresh(c)

InlineHasInflight(c) ==
  c # InlineMissingInflight

InlineWorkerConnected(c) ==
  c \notin {InlineDisconnected, InlineVnextDisconnectedStillFresh}

InlineVnextOwned(c) ==
  c = InlineVnextDisconnectedStillFresh

InlineFrontierMatches(c) ==
  c # InlineStaleFrontier

InlineTimeout(c) == 10

InlineElapsed(c) ==
  CASE c = InlineBeforeTimeout -> 9
    [] c = InlineAtTimeout -> 10
    [] c = InlineAfterTimeout -> 11
    [] c = InlineVnextDisconnectedStillFresh -> 9
    [] OTHER -> 0

SpecInline(c) ==
  IF ~InlineHasInflight(c)
  THEN NoneResult
  ELSE IF ~InlineVnextOwned(c) /\ ~InlineWorkerConnected(c)
  THEN WorkerDisconnected
  ELSE IF ~InlineFrontierMatches(c)
  THEN StaleFrontier
  ELSE IF InlineElapsed(c) >= InlineTimeout(c)
  THEN Stalled
  ELSE NoneResult

ActualInline(c) ==
  CASE Bug = "inline_missing_inflight_stalled"
       /\ c = InlineMissingInflight ->
      Stalled
    [] Bug = "inline_disconnected_none"
       /\ c = InlineDisconnected ->
      NoneResult
    [] Bug = "inline_stale_frontier_none"
       /\ c = InlineStaleFrontier ->
      NoneResult
    [] Bug = "inline_before_timeout_stalled"
       /\ c = InlineBeforeTimeout ->
      Stalled
    [] Bug = "inline_at_timeout_none"
       /\ c = InlineAtTimeout ->
      NoneResult
    [] OTHER -> SpecInline(c)

RedriveHasSlot(c) ==
  c # RedriveMissingSlot

RedriveTerminal(c) ==
  c = RedriveTerminalCommitted

RedriveQueued(c) ==
  c \in {RedriveQueuedBeforeTimeout, RedriveQueuedAtTimeout,
         RedriveQueuedAfterTimeout}

RedriveRunning(c) ==
  c \in {RedriveRunningOrphanNoInflight, RedriveRunningOrphanNoVnext,
         RedriveRunningFresh, RedriveRunningStalled}

RedriveBackpressured(c) ==
  c \in {RedriveBackpressuredBeforeTimeout, RedriveBackpressuredAtTimeout,
         RedriveBackpressuredAfterTimeout}

RedriveRetryMs(c) == 10

RedriveElapsed(c) ==
  CASE c \in {RedriveQueuedBeforeTimeout, RedriveBackpressuredBeforeTimeout} ->
      9
    [] c \in {RedriveQueuedAtTimeout, RedriveBackpressuredAtTimeout} ->
      10
    [] c \in {RedriveQueuedAfterTimeout, RedriveBackpressuredAfterTimeout} ->
      11
    [] OTHER -> 0

RedriveHasGeneralInflight(c) ==
  c # RedriveRunningOrphanNoInflight

RedriveHasVnextInflight(c) ==
  c # RedriveRunningOrphanNoVnext

RunningInlineReason(c) ==
  IF c = RedriveRunningStalled THEN Stalled ELSE NoneResult

SpecRedrive(c) ==
  IF ~RedriveHasSlot(c) \/ RedriveTerminal(c)
  THEN NoneResult
  ELSE IF RedriveQueued(c)
  THEN IF RedriveElapsed(c) >= RedriveRetryMs(c)
       THEN OrphanedQueued
       ELSE NoneResult
  ELSE IF RedriveRunning(c)
  THEN IF ~RedriveHasGeneralInflight(c) \/ ~RedriveHasVnextInflight(c)
       THEN OrphanedRunning
       ELSE IF RunningInlineReason(c) = Stalled
       THEN StalledRunning
       ELSE NoneResult
  ELSE IF RedriveBackpressured(c)
  THEN IF RedriveElapsed(c) >= RedriveRetryMs(c)
       THEN Backpressured
       ELSE NoneResult
  ELSE NoneResult

ActualRedrive(c) ==
  CASE Bug = "redrive_terminal_running"
       /\ c = RedriveTerminalCommitted ->
      StalledRunning
    [] Bug = "redrive_queued_before_timeout"
       /\ c = RedriveQueuedBeforeTimeout ->
      OrphanedQueued
    [] Bug = "redrive_queued_at_timeout_none"
       /\ c = RedriveQueuedAtTimeout ->
      NoneResult
    [] Bug = "redrive_running_orphan_none"
       /\ c = RedriveRunningOrphanNoInflight ->
      NoneResult
    [] Bug = "redrive_running_fresh_stalled"
       /\ c = RedriveRunningFresh ->
      StalledRunning
    [] Bug = "redrive_backpressured_before_timeout"
       /\ c = RedriveBackpressuredBeforeTimeout ->
      Backpressured
    [] Bug = "redrive_backpressured_at_timeout_none"
       /\ c = RedriveBackpressuredAtTimeout ->
      NoneResult
    [] Bug = "redrive_unqueued_or_valid"
       /\ c \in {RedriveUnqueued, RedriveValid, RedriveInvalid} ->
      OrphanedQueued
    [] OTHER -> SpecRedrive(c)

Bugs == {
  "none",
  "stall_drops_inline_floor",
  "stall_drops_ema_floor",
  "stall_drops_extra_floor",
  "stall_ignores_cap",
  "stall_uses_non_da_cap",
  "stall_da_extra_without_pending",
  "fresh_missing_inflight_returns_timeout",
  "fresh_disconnected_worker_returns_timeout",
  "fresh_stale_frontier_returns_timeout",
  "fresh_boundary_stays_fresh",
  "fresh_before_timeout_none",
  "inline_missing_inflight_stalled",
  "inline_disconnected_none",
  "inline_stale_frontier_none",
  "inline_before_timeout_stalled",
  "inline_at_timeout_none",
  "redrive_terminal_running",
  "redrive_queued_before_timeout",
  "redrive_queued_at_timeout_none",
  "redrive_running_orphan_none",
  "redrive_running_fresh_stalled",
  "redrive_backpressured_before_timeout",
  "redrive_backpressured_at_timeout_none",
  "redrive_unqueued_or_valid"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in StallCases:
       /\ SpecStallTimeout(c) \in 0..100
       /\ ActualStallTimeout(c) \in 0..100
  /\ \A c \in FreshCases:
       /\ SpecFresh(c) \in FreshOutputs
       /\ ActualFresh(c) \in FreshOutputs
  /\ \A c \in InlineCases:
       /\ SpecInline(c) \in InlineOutputs
       /\ ActualInline(c) \in InlineOutputs
  /\ \A c \in RedriveCases:
       /\ SpecRedrive(c) \in RedriveOutputs
       /\ ActualRedrive(c) \in RedriveOutputs

StallMatchesSpec ==
  \A c \in StallCases:
    ActualStallTimeout(c) = SpecStallTimeout(c)

FreshMatchesSpec ==
  \A c \in FreshCases:
    ActualFresh(c) = SpecFresh(c)

InlineMatchesSpec ==
  \A c \in InlineCases:
    ActualInline(c) = SpecInline(c)

RedriveMatchesSpec ==
  \A c \in RedriveCases:
    ActualRedrive(c) = SpecRedrive(c)

ValidationStallRedriveExactness ==
  /\ StallMatchesSpec
  /\ FreshMatchesSpec
  /\ InlineMatchesSpec
  /\ RedriveMatchesSpec

Safety ==
  ValidationStallRedriveExactness

ValidationStallRedriveCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ValidationStallRedriveExactness

BugStallDropsInlineFloor ==
  ActualStallTimeout(BaseInline) = SpecStallTimeout(BaseInline)

BugStallDropsEmaFloor ==
  ActualStallTimeout(EmaWins) = SpecStallTimeout(EmaWins)

BugStallDropsExtraFloor ==
  ActualStallTimeout(ExtraWins) = SpecStallTimeout(ExtraWins)

BugStallIgnoresCap ==
  ActualStallTimeout(CapClamps) = SpecStallTimeout(CapClamps)

BugStallUsesNonDaCap ==
  ActualStallTimeout(CapClamps) = SpecStallTimeout(CapClamps)

BugStallDaExtraWithoutPending ==
  ActualStallTimeout(DaNoPendingExtraZero) =
    SpecStallTimeout(DaNoPendingExtraZero)

BugFreshMissingInflightReturnsTimeout ==
  ActualFresh(FreshMissingInflight) = SpecFresh(FreshMissingInflight)

BugFreshDisconnectedWorkerReturnsTimeout ==
  ActualFresh(FreshDisconnected) = SpecFresh(FreshDisconnected)

BugFreshStaleFrontierReturnsTimeout ==
  ActualFresh(FreshStaleFrontier) = SpecFresh(FreshStaleFrontier)

BugFreshBoundaryStaysFresh ==
  ActualFresh(FreshAtTimeout) = SpecFresh(FreshAtTimeout)

BugFreshBeforeTimeoutNone ==
  ActualFresh(FreshBeforeTimeout) = SpecFresh(FreshBeforeTimeout)

BugInlineMissingInflightStalled ==
  ActualInline(InlineMissingInflight) = SpecInline(InlineMissingInflight)

BugInlineDisconnectedNone ==
  ActualInline(InlineDisconnected) = SpecInline(InlineDisconnected)

BugInlineStaleFrontierNone ==
  ActualInline(InlineStaleFrontier) = SpecInline(InlineStaleFrontier)

BugInlineBeforeTimeoutStalled ==
  ActualInline(InlineBeforeTimeout) = SpecInline(InlineBeforeTimeout)

BugInlineAtTimeoutNone ==
  ActualInline(InlineAtTimeout) = SpecInline(InlineAtTimeout)

BugRedriveTerminalRunning ==
  ActualRedrive(RedriveTerminalCommitted) = SpecRedrive(RedriveTerminalCommitted)

BugRedriveQueuedBeforeTimeout ==
  ActualRedrive(RedriveQueuedBeforeTimeout) =
    SpecRedrive(RedriveQueuedBeforeTimeout)

BugRedriveQueuedAtTimeoutNone ==
  ActualRedrive(RedriveQueuedAtTimeout) = SpecRedrive(RedriveQueuedAtTimeout)

BugRedriveRunningOrphanNone ==
  /\ ActualRedrive(RedriveRunningOrphanNoInflight) =
       SpecRedrive(RedriveRunningOrphanNoInflight)
  /\ ActualRedrive(RedriveRunningOrphanNoVnext) =
       SpecRedrive(RedriveRunningOrphanNoVnext)

BugRedriveRunningFreshStalled ==
  ActualRedrive(RedriveRunningFresh) = SpecRedrive(RedriveRunningFresh)

BugRedriveBackpressuredBeforeTimeout ==
  ActualRedrive(RedriveBackpressuredBeforeTimeout) =
    SpecRedrive(RedriveBackpressuredBeforeTimeout)

BugRedriveBackpressuredAtTimeoutNone ==
  ActualRedrive(RedriveBackpressuredAtTimeout) =
    SpecRedrive(RedriveBackpressuredAtTimeout)

BugRedriveUnqueuedOrValid ==
  /\ ActualRedrive(RedriveUnqueued) = SpecRedrive(RedriveUnqueued)
  /\ ActualRedrive(RedriveValid) = SpecRedrive(RedriveValid)
  /\ ActualRedrive(RedriveInvalid) = SpecRedrive(RedriveInvalid)

====
