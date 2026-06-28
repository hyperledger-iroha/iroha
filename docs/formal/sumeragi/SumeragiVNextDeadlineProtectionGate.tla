---- MODULE SumeragiVNextDeadlineProtectionGate ----
EXTENDS Naturals, Integers

(***************************************************************************
A bounded abstract model for vNext validation deadline/protection helpers in
`main_loop.rs`.

This slice pins:
`vnext_due_instant(...)`,
`vnext_validation_worker_fresh_deadline(...)`,
`vnext_validation_timeout_protected_blocks(...)`,
`vnext_validation_backpressure_protected_blocks(...)`, and
`vnext_round_next_due(...)`.

Instants and millisecond timestamps are finite integers. `NoDeadline` models
`Option::None`, and protected-block outputs are finite hash-id sets. The model
preserves the helper contract: due times at or before `now_ms` wake immediately,
future due times add the saturating delta or fall back to `now` on overflow,
fresh validation deadlines require matching general and vNext ownership and no
inline-fallback reason, timeout protection includes only fresh running slots,
backpressure protection includes only backpressured slots with retained pending
blocks, and vNext round wakeups ignore terminal/non-running states while taking
the earliest applicable deadline.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoDeadline == -1
Now == 10
NowMs == 100
MaxInstant == 20
HashA == 1
HashB == 2
HashUniverse == {HashA, HashB}

Min(a, b) == IF a <= b THEN a ELSE b
Max(a, b) == IF a >= b THEN a ELSE b

MergeDeadline(left, right) ==
  CASE left = NoDeadline -> right
    [] right = NoDeadline -> left
    [] OTHER -> Min(left, right)

DueCases == {
  "due_past",
  "due_now",
  "due_future",
  "due_overflow"
}

DueMs(c) ==
  CASE c = "due_past" -> 90
    [] c = "due_now" -> NowMs
    [] c = "due_future" -> 104
    [] c = "due_overflow" -> 130
    [] OTHER -> NowMs

SpecDueInstant(c) ==
  IF DueMs(c) <= NowMs
  THEN Now
  ELSE IF Now + (DueMs(c) - NowMs) > MaxInstant
  THEN Now
  ELSE Now + (DueMs(c) - NowMs)

ActualDueInstant(c) ==
  CASE Bug = "due_past_not_now" /\ c = "due_past" ->
      0
    [] Bug = "due_future_drops_delta" /\ c = "due_future" ->
      Now
    [] Bug = "due_overflow_saturates" /\ c = "due_overflow" ->
      MaxInstant
    [] OTHER -> SpecDueInstant(c)

FreshCases == {
  "fresh_missing_inflight",
  "fresh_missing_vnext",
  "fresh_wrong_slot",
  "fresh_inline_reason",
  "fresh_future_deadline",
  "fresh_past_clamped",
  "fresh_overflow_clamped"
}

FreshHasInflight(c) ==
  c # "fresh_missing_inflight"

FreshHasVnextInflight(c) ==
  c # "fresh_missing_vnext"

FreshSlotMatches(c) ==
  c # "fresh_wrong_slot"

FreshInlineReason(c) ==
  c = "fresh_inline_reason"

FreshStarted(c) ==
  CASE c = "fresh_future_deadline" -> 8
    [] c = "fresh_past_clamped" -> 1
    [] c = "fresh_overflow_clamped" -> 18
    [] OTHER -> 8

FreshStallTimeout(c) ==
  CASE c = "fresh_future_deadline" -> 5
    [] c = "fresh_past_clamped" -> 3
    [] c = "fresh_overflow_clamped" -> 5
    [] OTHER -> 5

FreshRawDeadline(c) ==
  FreshStarted(c) + FreshStallTimeout(c)

SpecFreshDeadline(c) ==
  IF ~FreshHasInflight(c)
  THEN NoDeadline
  ELSE IF ~FreshHasVnextInflight(c)
  THEN NoDeadline
  ELSE IF ~FreshSlotMatches(c)
  THEN NoDeadline
  ELSE IF FreshInlineReason(c)
  THEN NoDeadline
  ELSE IF FreshRawDeadline(c) > MaxInstant
  THEN Now
  ELSE Max(FreshRawDeadline(c), Now)

ActualFreshDeadline(c) ==
  CASE Bug = "fresh_missing_inflight_some" /\ c = "fresh_missing_inflight" ->
      Now
    [] Bug = "fresh_missing_vnext_some" /\ c = "fresh_missing_vnext" ->
      Now
    [] Bug = "fresh_wrong_slot_some" /\ c = "fresh_wrong_slot" ->
      Now
    [] Bug = "fresh_inline_reason_ignored" /\ c = "fresh_inline_reason" ->
      Now + 1
    [] Bug = "fresh_past_not_clamped" /\ c = "fresh_past_clamped" ->
      FreshRawDeadline(c)
    [] Bug = "fresh_overflow_saturates" /\ c = "fresh_overflow_clamped" ->
      MaxInstant
    [] OTHER -> SpecFreshDeadline(c)

TimeoutCases == {
  "timeout_missing_round",
  "timeout_running_fresh",
  "timeout_running_stale",
  "timeout_backpressured",
  "timeout_terminal",
  "timeout_mixed"
}

SpecTimeoutProtected(c) ==
  CASE c = "timeout_running_fresh" -> {HashA}
    [] c = "timeout_mixed" -> {HashA}
    [] OTHER -> {}

ActualTimeoutProtected(c) ==
  CASE Bug = "timeout_drops_fresh" /\ c = "timeout_running_fresh" ->
      {}
    [] Bug = "timeout_protects_stale" /\ c = "timeout_running_stale" ->
      {HashA}
    [] Bug = "timeout_protects_backpressure" /\ c = "timeout_backpressured" ->
      {HashA}
    [] Bug = "timeout_protects_terminal" /\ c = "timeout_terminal" ->
      {HashA}
    [] Bug = "timeout_mixed_drops_second" /\ c = "timeout_mixed" ->
      {}
    [] OTHER -> SpecTimeoutProtected(c)

BackpressureCases == {
  "backpressure_missing_round",
  "backpressure_pending",
  "backpressure_missing_pending",
  "backpressure_running_pending",
  "backpressure_terminal_pending",
  "backpressure_mixed"
}

SpecBackpressureProtected(c) ==
  CASE c = "backpressure_pending" -> {HashA}
    [] c = "backpressure_mixed" -> {HashA}
    [] OTHER -> {}

ActualBackpressureProtected(c) ==
  CASE Bug = "backpressure_drops_pending" /\ c = "backpressure_pending" ->
      {}
    [] Bug = "backpressure_protects_missing_pending"
       /\ c = "backpressure_missing_pending" ->
      {HashA}
    [] Bug = "backpressure_protects_running"
       /\ c = "backpressure_running_pending" ->
      {HashA}
    [] Bug = "backpressure_protects_terminal"
       /\ c = "backpressure_terminal_pending" ->
      {HashA}
    [] Bug = "backpressure_mixed_drops_pending" /\ c = "backpressure_mixed" ->
      {}
    [] OTHER -> SpecBackpressureProtected(c)

RoundCases == {
  "round_empty",
  "round_running_fresh",
  "round_running_stale_future",
  "round_running_stale_due",
  "round_backpressure_future",
  "round_backpressure_due",
  "round_terminal",
  "round_multiple_min"
}

RoundFreshDeadline(c) ==
  CASE c = "round_running_fresh" -> 13
    [] c = "round_multiple_min" -> 12
    [] OTHER -> NoDeadline

RoundDueMs(c) ==
  CASE c = "round_running_stale_future" -> 104
    [] c = "round_backpressure_future" -> 105
    [] c = "round_running_stale_due" -> 100
    [] c = "round_backpressure_due" -> 99
    [] c = "round_multiple_min" -> 107
    [] OTHER -> NowMs

RoundSuspicionDeadline(c) ==
  IF c \in {
       "round_running_stale_future",
       "round_running_stale_due",
       "round_backpressure_future",
       "round_backpressure_due",
       "round_multiple_min"
     }
  THEN IF RoundDueMs(c) <= NowMs
       THEN Now
       ELSE IF Now + (RoundDueMs(c) - NowMs) > MaxInstant
       THEN Now
       ELSE Now + (RoundDueMs(c) - NowMs)
  ELSE NoDeadline

SpecRoundNextDue(c) ==
  CASE c = "round_empty" -> NoDeadline
    [] c = "round_terminal" -> NoDeadline
    [] c = "round_running_fresh" -> RoundFreshDeadline(c)
    [] c = "round_multiple_min" ->
      MergeDeadline(RoundFreshDeadline(c), RoundSuspicionDeadline(c))
    [] OTHER -> RoundSuspicionDeadline(c)

ActualRoundNextDue(c) ==
  CASE Bug = "round_running_fresh_uses_suspicion"
       /\ c = "round_running_fresh" ->
      RoundSuspicionDeadline("round_running_stale_future")
    [] Bug = "round_running_stale_none"
       /\ c = "round_running_stale_future" ->
      NoDeadline
    [] Bug = "round_backpressure_none"
       /\ c = "round_backpressure_future" ->
      NoDeadline
    [] Bug = "round_terminal_due"
       /\ c = "round_terminal" ->
      Now
    [] Bug = "round_uses_latest_due"
       /\ c = "round_multiple_min" ->
      Max(RoundFreshDeadline(c), RoundSuspicionDeadline(c))
    [] OTHER -> SpecRoundNextDue(c)

Bugs == {
  "none",
  "due_past_not_now",
  "due_future_drops_delta",
  "due_overflow_saturates",
  "fresh_missing_inflight_some",
  "fresh_missing_vnext_some",
  "fresh_wrong_slot_some",
  "fresh_inline_reason_ignored",
  "fresh_past_not_clamped",
  "fresh_overflow_saturates",
  "timeout_drops_fresh",
  "timeout_protects_stale",
  "timeout_protects_backpressure",
  "timeout_protects_terminal",
  "timeout_mixed_drops_second",
  "backpressure_drops_pending",
  "backpressure_protects_missing_pending",
  "backpressure_protects_running",
  "backpressure_protects_terminal",
  "backpressure_mixed_drops_pending",
  "round_running_fresh_uses_suspicion",
  "round_running_stale_none",
  "round_backpressure_none",
  "round_terminal_due",
  "round_uses_latest_due"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

DeadlineOutput == NoDeadline..MaxInstant

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in DueCases:
       /\ SpecDueInstant(c) \in 0..MaxInstant
       /\ ActualDueInstant(c) \in 0..MaxInstant
  /\ \A c \in FreshCases:
       /\ SpecFreshDeadline(c) \in DeadlineOutput
       /\ ActualFreshDeadline(c) \in DeadlineOutput
  /\ \A c \in TimeoutCases:
       /\ SpecTimeoutProtected(c) \subseteq HashUniverse
       /\ ActualTimeoutProtected(c) \subseteq HashUniverse
  /\ \A c \in BackpressureCases:
       /\ SpecBackpressureProtected(c) \subseteq HashUniverse
       /\ ActualBackpressureProtected(c) \subseteq HashUniverse
  /\ \A c \in RoundCases:
       /\ SpecRoundNextDue(c) \in DeadlineOutput
       /\ ActualRoundNextDue(c) \in DeadlineOutput

DueMatchesSpec ==
  \A c \in DueCases:
    ActualDueInstant(c) = SpecDueInstant(c)

FreshMatchesSpec ==
  \A c \in FreshCases:
    ActualFreshDeadline(c) = SpecFreshDeadline(c)

TimeoutProtectionMatchesSpec ==
  \A c \in TimeoutCases:
    ActualTimeoutProtected(c) = SpecTimeoutProtected(c)

BackpressureProtectionMatchesSpec ==
  \A c \in BackpressureCases:
    ActualBackpressureProtected(c) = SpecBackpressureProtected(c)

RoundNextDueMatchesSpec ==
  \A c \in RoundCases:
    ActualRoundNextDue(c) = SpecRoundNextDue(c)

VNextDeadlineProtectionExactness ==
  /\ DueMatchesSpec
  /\ FreshMatchesSpec
  /\ TimeoutProtectionMatchesSpec
  /\ BackpressureProtectionMatchesSpec
  /\ RoundNextDueMatchesSpec

Safety ==
  VNextDeadlineProtectionExactness

VNextDeadlineProtectionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ VNextDeadlineProtectionExactness

BugDuePastNotNow ==
  ActualDueInstant("due_past") = SpecDueInstant("due_past")

BugDueFutureDropsDelta ==
  ActualDueInstant("due_future") = SpecDueInstant("due_future")

BugDueOverflowSaturates ==
  ActualDueInstant("due_overflow") = SpecDueInstant("due_overflow")

BugFreshMissingInflightSome ==
  ActualFreshDeadline("fresh_missing_inflight") =
    SpecFreshDeadline("fresh_missing_inflight")

BugFreshMissingVnextSome ==
  ActualFreshDeadline("fresh_missing_vnext") =
    SpecFreshDeadline("fresh_missing_vnext")

BugFreshWrongSlotSome ==
  ActualFreshDeadline("fresh_wrong_slot") =
    SpecFreshDeadline("fresh_wrong_slot")

BugFreshInlineReasonIgnored ==
  ActualFreshDeadline("fresh_inline_reason") =
    SpecFreshDeadline("fresh_inline_reason")

BugFreshPastNotClamped ==
  ActualFreshDeadline("fresh_past_clamped") =
    SpecFreshDeadline("fresh_past_clamped")

BugFreshOverflowSaturates ==
  ActualFreshDeadline("fresh_overflow_clamped") =
    SpecFreshDeadline("fresh_overflow_clamped")

BugTimeoutDropsFresh ==
  ActualTimeoutProtected("timeout_running_fresh") =
    SpecTimeoutProtected("timeout_running_fresh")

BugTimeoutProtectsStale ==
  ActualTimeoutProtected("timeout_running_stale") =
    SpecTimeoutProtected("timeout_running_stale")

BugTimeoutProtectsBackpressure ==
  ActualTimeoutProtected("timeout_backpressured") =
    SpecTimeoutProtected("timeout_backpressured")

BugTimeoutProtectsTerminal ==
  ActualTimeoutProtected("timeout_terminal") =
    SpecTimeoutProtected("timeout_terminal")

BugTimeoutMixedDropsSecond ==
  ActualTimeoutProtected("timeout_mixed") =
    SpecTimeoutProtected("timeout_mixed")

BugBackpressureDropsPending ==
  ActualBackpressureProtected("backpressure_pending") =
    SpecBackpressureProtected("backpressure_pending")

BugBackpressureProtectsMissingPending ==
  ActualBackpressureProtected("backpressure_missing_pending") =
    SpecBackpressureProtected("backpressure_missing_pending")

BugBackpressureProtectsRunning ==
  ActualBackpressureProtected("backpressure_running_pending") =
    SpecBackpressureProtected("backpressure_running_pending")

BugBackpressureProtectsTerminal ==
  ActualBackpressureProtected("backpressure_terminal_pending") =
    SpecBackpressureProtected("backpressure_terminal_pending")

BugBackpressureMixedDropsPending ==
  ActualBackpressureProtected("backpressure_mixed") =
    SpecBackpressureProtected("backpressure_mixed")

BugRoundRunningFreshUsesSuspicion ==
  ActualRoundNextDue("round_running_fresh") =
    SpecRoundNextDue("round_running_fresh")

BugRoundRunningStaleNone ==
  ActualRoundNextDue("round_running_stale_future") =
    SpecRoundNextDue("round_running_stale_future")

BugRoundBackpressureNone ==
  ActualRoundNextDue("round_backpressure_future") =
    SpecRoundNextDue("round_backpressure_future")

BugRoundTerminalDue ==
  ActualRoundNextDue("round_terminal") = SpecRoundNextDue("round_terminal")

BugRoundUsesLatestDue ==
  ActualRoundNextDue("round_multiple_min") =
    SpecRoundNextDue("round_multiple_min")

====
