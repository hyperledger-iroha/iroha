---- MODULE SumeragiMissingQcTimingGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for missing-QC timing helper decisions.

This slice pins `idle_round_timed_out(...)`, `idle_view_timeout(...)`,
`next_missing_qc_timeout_streak(...)`,
`forced_proposal_attempt_allowed(...)`,
`should_defer_missing_qc_rotation(...)`,
`missing_qc_rotation_hard_cap(...)`, and
`saturating_mul_duration(...)`.

The multiplication cap is represented by `MaxMillis` to keep the model finite;
it stands in for Rust's `u64::MAX` millisecond cap.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

MaxMillis == 20

Min(a, b) == IF a <= b THEN a ELSE b

Max(a, b) == IF a >= b THEN a ELSE b

Max3(a, b, c) == Max(Max(a, b), c)

SaturatingIncU8(value) ==
  IF value >= 255 THEN 255 ELSE value + 1

HardCapMs(timeout, window) ==
  Max3(timeout + window, window * 2, 1)

IdleRoundCases == {
  "idle_before_timeout",
  "idle_at_timeout",
  "idle_after_timeout",
  "idle_nonempty_after_timeout",
  "idle_zero_timeout_after_age"
}

IdleRoundPendingEmpty(c) ==
  c # "idle_nonempty_after_timeout"

IdleRoundAge(c) ==
  CASE c = "idle_before_timeout" -> 9
    [] c = "idle_at_timeout" -> 10
    [] c = "idle_after_timeout" -> 11
    [] c = "idle_nonempty_after_timeout" -> 11
    [] c = "idle_zero_timeout_after_age" -> 11
    [] OTHER -> 0

IdleRoundTimeout(c) ==
  CASE c = "idle_zero_timeout_after_age" -> 0
    [] OTHER -> 10

SpecIdleRound(c) ==
  /\ IdleRoundPendingEmpty(c)
  /\ IdleRoundTimeout(c) # 0
  /\ IdleRoundAge(c) >= IdleRoundTimeout(c)

ActualIdleRound(c) ==
  CASE Bug = "idle_round_nonempty_times_out"
       /\ c = "idle_nonempty_after_timeout" -> TRUE
    [] Bug = "idle_round_zero_timeout_times_out"
       /\ c = "idle_zero_timeout_after_age" -> TRUE
    [] Bug = "idle_round_strict_age"
       /\ c = "idle_at_timeout" -> FALSE
    [] OTHER -> SpecIdleRound(c)

IdleViewCases == {
  "view_seen_zero_commit",
  "view_seen_commit",
  "view_da_zero_inputs",
  "view_da_sum",
  "view_da_propose_only",
  "view_no_da_zero_inputs",
  "view_no_da_grace_below_commit",
  "view_no_da_commit_cap",
  "view_no_da_commit_zero"
}

IdleViewProposalSeen(c) ==
  c \in {"view_seen_zero_commit", "view_seen_commit"}

IdleViewDaEnabled(c) ==
  c \in {"view_da_zero_inputs", "view_da_sum", "view_da_propose_only"}

IdleViewCommit(c) ==
  CASE c = "view_seen_commit" -> 7
    [] c = "view_da_sum" -> 5
    [] c = "view_no_da_grace_below_commit" -> 10
    [] c = "view_no_da_commit_cap" -> 8
    [] OTHER -> 0

IdleViewPropose(c) ==
  CASE c = "view_da_sum" -> 3
    [] c = "view_da_propose_only" -> 4
    [] c = "view_no_da_grace_below_commit" -> 2
    [] c = "view_no_da_commit_cap" -> 3
    [] c = "view_no_da_commit_zero" -> 2
    [] OTHER -> 0

SpecIdleView(c) ==
  IF IdleViewProposalSeen(c) THEN
    IdleViewCommit(c)
  ELSE IF IdleViewDaEnabled(c) THEN
    Max(IdleViewCommit(c) + IdleViewPropose(c), 1)
  ELSE
    Max(Min(IdleViewPropose(c) * 4, IdleViewCommit(c)), 1)

ActualIdleView(c) ==
  CASE Bug = "idle_view_seen_uses_grace"
       /\ c = "view_seen_zero_commit" -> 1
    [] Bug = "idle_view_da_omits_floor"
       /\ c = "view_da_zero_inputs" -> 0
    [] Bug = "idle_view_da_uses_min"
       /\ c = "view_da_sum" ->
       Min(IdleViewCommit(c) + IdleViewPropose(c), IdleViewCommit(c))
    [] Bug = "idle_view_no_da_ignores_cap"
       /\ c = "view_no_da_commit_cap" -> IdleViewPropose(c) * 4
    [] Bug = "idle_view_no_da_omits_floor"
       /\ c = "view_no_da_zero_inputs" -> 0
    [] OTHER -> SpecIdleView(c)

StreakCases == {
  "streak_no_previous",
  "streak_height_mismatch",
  "streak_same_view",
  "streak_lower_view",
  "streak_advances_zero",
  "streak_advances_mid",
  "streak_saturates"
}

StreakHasPrevious(c) ==
  c # "streak_no_previous"

StreakHeightMatches(c) ==
  c # "streak_height_mismatch"

StreakViewAdvances(c) ==
  c \in {"streak_advances_zero", "streak_advances_mid", "streak_saturates"}

StreakLast(c) ==
  CASE c = "streak_height_mismatch" -> 3
    [] c = "streak_same_view" -> 3
    [] c = "streak_lower_view" -> 3
    [] c = "streak_advances_mid" -> 7
    [] c = "streak_saturates" -> 255
    [] OTHER -> 0

SpecStreak(c) ==
  IF /\ StreakHasPrevious(c)
     /\ StreakHeightMatches(c)
     /\ StreakViewAdvances(c)
  THEN
    SaturatingIncU8(StreakLast(c))
  ELSE
    0

ActualStreak(c) ==
  CASE Bug = "streak_wrong_height_increments"
       /\ c = "streak_height_mismatch" -> SaturatingIncU8(StreakLast(c))
    [] Bug = "streak_same_view_increments"
       /\ c = "streak_same_view" -> SaturatingIncU8(StreakLast(c))
    [] Bug = "streak_missing_previous_one"
       /\ c = "streak_no_previous" -> 1
    [] Bug = "streak_saturation_wraps"
       /\ c = "streak_saturates" -> 0
    [] OTHER -> SpecStreak(c)

ForcedCases == {
  "forced_zero_max",
  "forced_below_max",
  "forced_equal_max",
  "forced_above_max"
}

ForcedAttempts(c) ==
  CASE c = "forced_equal_max" -> 2
    [] c = "forced_above_max" -> 3
    [] OTHER -> 1

ForcedMax(c) ==
  CASE c = "forced_zero_max" -> 0
    [] OTHER -> 2

SpecForced(c) ==
  /\ ForcedMax(c) # 0
  /\ ForcedAttempts(c) < ForcedMax(c)

ActualForced(c) ==
  CASE Bug = "forced_zero_max_allows"
       /\ c = "forced_zero_max" -> TRUE
    [] Bug = "forced_equal_allows"
       /\ c = "forced_equal_max" -> TRUE
    [] Bug = "forced_below_denies"
       /\ c = "forced_below_max" -> FALSE
    [] OTHER -> SpecForced(c)

DeferCases == {
  "defer_before_reacquire",
  "defer_at_reacquire_rotate",
  "defer_after_reacquire_rotate",
  "defer_after_reacquire_no_rotate_before_cap",
  "defer_before_hard_cap_no_rotate",
  "defer_at_hard_cap_no_rotate",
  "defer_zero_floor_before_cap",
  "defer_zero_floor_at_cap"
}

DeferTimeout(c) ==
  CASE c \in {"defer_zero_floor_before_cap", "defer_zero_floor_at_cap"} -> 0
    [] OTHER -> 10

DeferWindow(c) ==
  CASE c \in {"defer_zero_floor_before_cap", "defer_zero_floor_at_cap"} -> 0
    [] OTHER -> 20

DeferAge(c) ==
  CASE c = "defer_before_reacquire" -> 25
    [] c = "defer_at_reacquire_rotate" -> 30
    [] c = "defer_after_reacquire_rotate" -> 31
    [] c = "defer_after_reacquire_no_rotate_before_cap" -> 35
    [] c = "defer_before_hard_cap_no_rotate" -> 39
    [] c = "defer_at_hard_cap_no_rotate" -> 40
    [] c = "defer_zero_floor_at_cap" -> 1
    [] OTHER -> 0

DeferRotateAfter(c) ==
  c \in {
    "defer_before_reacquire",
    "defer_at_reacquire_rotate",
    "defer_after_reacquire_rotate"
  }

SpecDefer(c) ==
  LET deadline == DeferTimeout(c) + DeferWindow(c) IN
    IF DeferAge(c) < deadline THEN
      TRUE
    ELSE IF DeferRotateAfter(c) THEN
      FALSE
    ELSE
      DeferAge(c) < HardCapMs(DeferTimeout(c), DeferWindow(c))

ActualDefer(c) ==
  CASE Bug = "defer_skips_reacquire_window"
       /\ c = "defer_before_reacquire" -> FALSE
    [] Bug = "defer_rotates_at_reacquire_deadline_even_when_disabled"
       /\ c = "defer_after_reacquire_no_rotate_before_cap" -> FALSE
    [] Bug = "defer_ignores_rotate_after_flag"
       /\ c = "defer_at_reacquire_rotate" -> TRUE
    [] Bug = "defer_hard_cap_inclusive"
       /\ c = "defer_at_hard_cap_no_rotate" -> TRUE
    [] OTHER -> SpecDefer(c)

HardCapCases == {
  "hardcap_timeout_plus_window",
  "hardcap_double_window",
  "hardcap_floor",
  "hardcap_equal"
}

HardCapTimeout(c) ==
  CASE c = "hardcap_timeout_plus_window" -> 10
    [] c = "hardcap_double_window" -> 1
    [] c = "hardcap_equal" -> 5
    [] OTHER -> 0

HardCapWindow(c) ==
  CASE c = "hardcap_timeout_plus_window" -> 3
    [] c = "hardcap_double_window" -> 10
    [] c = "hardcap_equal" -> 5
    [] OTHER -> 0

SpecHardCap(c) ==
  HardCapMs(HardCapTimeout(c), HardCapWindow(c))

ActualHardCap(c) ==
  CASE Bug = "hard_cap_uses_min"
       /\ c = "hardcap_timeout_plus_window" ->
       Min(HardCapTimeout(c) + HardCapWindow(c), HardCapWindow(c) * 2)
    [] Bug = "hard_cap_omits_floor"
       /\ c = "hardcap_floor" ->
       Max(HardCapTimeout(c) + HardCapWindow(c), HardCapWindow(c) * 2)
    [] Bug = "hard_cap_skips_double_window"
       /\ c = "hardcap_double_window" ->
       Max(HardCapTimeout(c) + HardCapWindow(c), 1)
    [] OTHER -> SpecHardCap(c)

MulCases == {
  "mul_zero_duration",
  "mul_zero_multiplier",
  "mul_scaled",
  "mul_exact_cap",
  "mul_overflow"
}

MulDuration(c) ==
  CASE c = "mul_zero_duration" -> 0
    [] c = "mul_zero_multiplier" -> 5
    [] c = "mul_scaled" -> 4
    [] c = "mul_exact_cap" -> 10
    [] c = "mul_overflow" -> 8
    [] OTHER -> 0

MulFactor(c) ==
  CASE c = "mul_zero_duration" -> 10
    [] c = "mul_zero_multiplier" -> 0
    [] c = "mul_scaled" -> 3
    [] c = "mul_exact_cap" -> 2
    [] c = "mul_overflow" -> 3
    [] OTHER -> 0

SpecMul(c) ==
  Min(MulDuration(c) * MulFactor(c), MaxMillis)

ActualMul(c) ==
  CASE Bug = "saturating_mul_overflows"
       /\ c = "mul_overflow" -> MulDuration(c) * MulFactor(c)
    [] Bug = "saturating_mul_zero_adds_one"
       /\ c = "mul_zero_duration" -> 1
    [] Bug = "saturating_mul_uses_add"
       /\ c = "mul_scaled" -> MulDuration(c) + MulFactor(c)
    [] OTHER -> SpecMul(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "idle_round_nonempty_times_out",
       "idle_round_zero_timeout_times_out",
       "idle_round_strict_age",
       "idle_view_seen_uses_grace",
       "idle_view_da_omits_floor",
       "idle_view_da_uses_min",
       "idle_view_no_da_ignores_cap",
       "idle_view_no_da_omits_floor",
       "streak_wrong_height_increments",
       "streak_same_view_increments",
       "streak_missing_previous_one",
       "streak_saturation_wraps",
       "forced_zero_max_allows",
       "forced_equal_allows",
       "forced_below_denies",
       "defer_skips_reacquire_window",
       "defer_rotates_at_reacquire_deadline_even_when_disabled",
       "defer_ignores_rotate_after_flag",
       "defer_hard_cap_inclusive",
       "hard_cap_uses_min",
       "hard_cap_omits_floor",
       "hard_cap_skips_double_window",
       "saturating_mul_overflows",
       "saturating_mul_zero_adds_one",
       "saturating_mul_uses_add"
     }
  /\ checked = 0

IdleRoundMatchesSpec ==
  \A c \in IdleRoundCases:
    ActualIdleRound(c) = SpecIdleRound(c)

IdleViewMatchesSpec ==
  \A c \in IdleViewCases:
    ActualIdleView(c) = SpecIdleView(c)

StreakMatchesSpec ==
  \A c \in StreakCases:
    ActualStreak(c) = SpecStreak(c)

ForcedProposalMatchesSpec ==
  \A c \in ForcedCases:
    ActualForced(c) = SpecForced(c)

DeferRotationMatchesSpec ==
  \A c \in DeferCases:
    ActualDefer(c) = SpecDefer(c)

HardCapMatchesSpec ==
  \A c \in HardCapCases:
    ActualHardCap(c) = SpecHardCap(c)

SaturatingMulMatchesSpec ==
  \A c \in MulCases:
    ActualMul(c) = SpecMul(c)

IdleRoundAnchors ==
  /\ SpecIdleRound("idle_before_timeout") = FALSE
  /\ SpecIdleRound("idle_at_timeout") = TRUE
  /\ SpecIdleRound("idle_after_timeout") = TRUE
  /\ SpecIdleRound("idle_nonempty_after_timeout") = FALSE
  /\ SpecIdleRound("idle_zero_timeout_after_age") = FALSE

IdleViewAnchors ==
  /\ SpecIdleView("view_seen_zero_commit") = 0
  /\ SpecIdleView("view_seen_commit") = 7
  /\ SpecIdleView("view_da_zero_inputs") = 1
  /\ SpecIdleView("view_da_sum") = 8
  /\ SpecIdleView("view_da_propose_only") = 4
  /\ SpecIdleView("view_no_da_zero_inputs") = 1
  /\ SpecIdleView("view_no_da_grace_below_commit") = 8
  /\ SpecIdleView("view_no_da_commit_cap") = 8
  /\ SpecIdleView("view_no_da_commit_zero") = 1

StreakAnchors ==
  /\ SpecStreak("streak_no_previous") = 0
  /\ SpecStreak("streak_height_mismatch") = 0
  /\ SpecStreak("streak_same_view") = 0
  /\ SpecStreak("streak_lower_view") = 0
  /\ SpecStreak("streak_advances_zero") = 1
  /\ SpecStreak("streak_advances_mid") = 8
  /\ SpecStreak("streak_saturates") = 255

ForcedProposalAnchors ==
  /\ SpecForced("forced_zero_max") = FALSE
  /\ SpecForced("forced_below_max") = TRUE
  /\ SpecForced("forced_equal_max") = FALSE
  /\ SpecForced("forced_above_max") = FALSE

DeferRotationAnchors ==
  /\ SpecDefer("defer_before_reacquire") = TRUE
  /\ SpecDefer("defer_at_reacquire_rotate") = FALSE
  /\ SpecDefer("defer_after_reacquire_rotate") = FALSE
  /\ SpecDefer("defer_after_reacquire_no_rotate_before_cap") = TRUE
  /\ SpecDefer("defer_before_hard_cap_no_rotate") = TRUE
  /\ SpecDefer("defer_at_hard_cap_no_rotate") = FALSE
  /\ SpecDefer("defer_zero_floor_before_cap") = TRUE
  /\ SpecDefer("defer_zero_floor_at_cap") = FALSE

HardCapAnchors ==
  /\ SpecHardCap("hardcap_timeout_plus_window") = 13
  /\ SpecHardCap("hardcap_double_window") = 20
  /\ SpecHardCap("hardcap_floor") = 1
  /\ SpecHardCap("hardcap_equal") = 10

SaturatingMulAnchors ==
  /\ SpecMul("mul_zero_duration") = 0
  /\ SpecMul("mul_zero_multiplier") = 0
  /\ SpecMul("mul_scaled") = 12
  /\ SpecMul("mul_exact_cap") = MaxMillis
  /\ SpecMul("mul_overflow") = MaxMillis

SafetyFast ==
  /\ IdleRoundMatchesSpec
  /\ IdleViewMatchesSpec
  /\ StreakMatchesSpec
  /\ ForcedProposalMatchesSpec
  /\ DeferRotationMatchesSpec
  /\ HardCapMatchesSpec
  /\ SaturatingMulMatchesSpec
  /\ IdleRoundAnchors
  /\ IdleViewAnchors
  /\ StreakAnchors
  /\ ForcedProposalAnchors
  /\ DeferRotationAnchors
  /\ HardCapAnchors
  /\ SaturatingMulAnchors

BugIdleRoundNonemptyTimesOut ==
  ActualIdleRound("idle_nonempty_after_timeout") =
    SpecIdleRound("idle_nonempty_after_timeout")

BugIdleRoundZeroTimeoutTimesOut ==
  ActualIdleRound("idle_zero_timeout_after_age") =
    SpecIdleRound("idle_zero_timeout_after_age")

BugIdleRoundStrictAge ==
  ActualIdleRound("idle_at_timeout") = SpecIdleRound("idle_at_timeout")

BugIdleViewSeenUsesGrace ==
  ActualIdleView("view_seen_zero_commit") = SpecIdleView("view_seen_zero_commit")

BugIdleViewDaOmitsFloor ==
  ActualIdleView("view_da_zero_inputs") = SpecIdleView("view_da_zero_inputs")

BugIdleViewDaUsesMin ==
  ActualIdleView("view_da_sum") = SpecIdleView("view_da_sum")

BugIdleViewNoDaIgnoresCap ==
  ActualIdleView("view_no_da_commit_cap") = SpecIdleView("view_no_da_commit_cap")

BugIdleViewNoDaOmitsFloor ==
  ActualIdleView("view_no_da_zero_inputs") =
    SpecIdleView("view_no_da_zero_inputs")

BugStreakWrongHeightIncrements ==
  ActualStreak("streak_height_mismatch") = SpecStreak("streak_height_mismatch")

BugStreakSameViewIncrements ==
  ActualStreak("streak_same_view") = SpecStreak("streak_same_view")

BugStreakMissingPreviousOne ==
  ActualStreak("streak_no_previous") = SpecStreak("streak_no_previous")

BugStreakSaturationWraps ==
  ActualStreak("streak_saturates") = SpecStreak("streak_saturates")

BugForcedZeroMaxAllows ==
  ActualForced("forced_zero_max") = SpecForced("forced_zero_max")

BugForcedEqualAllows ==
  ActualForced("forced_equal_max") = SpecForced("forced_equal_max")

BugForcedBelowDenies ==
  ActualForced("forced_below_max") = SpecForced("forced_below_max")

BugDeferSkipsReacquireWindow ==
  ActualDefer("defer_before_reacquire") = SpecDefer("defer_before_reacquire")

BugDeferRotatesAtReacquireDeadlineEvenWhenDisabled ==
  ActualDefer("defer_after_reacquire_no_rotate_before_cap") =
    SpecDefer("defer_after_reacquire_no_rotate_before_cap")

BugDeferIgnoresRotateAfterFlag ==
  ActualDefer("defer_at_reacquire_rotate") =
    SpecDefer("defer_at_reacquire_rotate")

BugDeferHardCapInclusive ==
  ActualDefer("defer_at_hard_cap_no_rotate") =
    SpecDefer("defer_at_hard_cap_no_rotate")

BugHardCapUsesMin ==
  ActualHardCap("hardcap_timeout_plus_window") =
    SpecHardCap("hardcap_timeout_plus_window")

BugHardCapOmitsFloor ==
  ActualHardCap("hardcap_floor") = SpecHardCap("hardcap_floor")

BugHardCapSkipsDoubleWindow ==
  ActualHardCap("hardcap_double_window") = SpecHardCap("hardcap_double_window")

BugSaturatingMulOverflows ==
  ActualMul("mul_overflow") = SpecMul("mul_overflow")

BugSaturatingMulZeroAddsOne ==
  ActualMul("mul_zero_duration") = SpecMul("mul_zero_duration")

BugSaturatingMulUsesAdd ==
  ActualMul("mul_scaled") = SpecMul("mul_scaled")

====
