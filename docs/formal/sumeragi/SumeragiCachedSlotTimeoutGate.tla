---- MODULE SumeragiCachedSlotTimeoutGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for cached proposal-slot timeout selection.

This slice models `cached_slot_effective_quorum_timeout(...)`,
`next_cached_slot_timeout_streak(...)`, and
`cached_slot_timeout_hysteresis_remaining(...)`. Near-commit-quorum payload
repair can shorten the cached-slot timeout only when one more precommit vote
would satisfy quorum, local payload data is missing, and neither consensus
queue nor RBC backpressure is active. Repeated NPoS cached-slot timeouts record
a saturating u8 streak and are damped by a bounded hysteresis factor derived
from that previous timeout streak, while permissioned mode, zero timeout,
missing history, wrong height, and non-advancing views never wait.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  uses_fast_timeout,
  \* @type: Bool;
  returns_base_timeout,
  \* @type: Bool;
  returns_shorter_near_timeout,
  \* @type: Bool;
  hysteresis_wait,
  \* @type: Int;
  next_streak,
  \* @type: Int;
  hysteresis_factor

\* @type: <<Str, Bool, Bool, Bool, Bool, Int, Int>>;
vars == <<candidate, uses_fast_timeout, returns_base_timeout,
  returns_shorter_near_timeout, hysteresis_wait, next_streak,
  hysteresis_factor>>

EffectiveCases == {
  "effective_zero_votes",
  "effective_far_from_quorum",
  "effective_at_quorum",
  "effective_near_fast_shorter",
  "effective_near_fast_not_shorter",
  "effective_near_no_missing_data",
  "effective_near_consensus_backlog",
  "effective_near_rbc_incomplete",
  "effective_near_both_backlogs"
}

HysteresisCases == {
  "hysteresis_permissioned_mode",
  "hysteresis_zero_quorum_timeout",
  "hysteresis_no_previous",
  "hysteresis_height_mismatch",
  "hysteresis_same_view",
  "hysteresis_lower_view",
  "hysteresis_streak0_before",
  "hysteresis_streak1_before",
  "hysteresis_streak2_before",
  "hysteresis_streak3_before",
  "hysteresis_streak_max_before",
  "hysteresis_boundary",
  "hysteresis_after"
}

Cases == EffectiveCases \union HysteresisCases

NearFastEligibleCases == {
  "effective_near_fast_shorter",
  "effective_near_fast_not_shorter"
}

NearFastShorterCases == {"effective_near_fast_shorter"}
NearFastNotShorterCases == {"effective_near_fast_not_shorter"}
NonFastEffectiveCases == EffectiveCases \ NearFastEligibleCases

HysteresisInvalidCases == {
  "hysteresis_permissioned_mode",
  "hysteresis_zero_quorum_timeout",
  "hysteresis_no_previous",
  "hysteresis_height_mismatch",
  "hysteresis_same_view",
  "hysteresis_lower_view"
}

HysteresisBeforeCases == {
  "hysteresis_streak0_before",
  "hysteresis_streak1_before",
  "hysteresis_streak2_before",
  "hysteresis_streak3_before",
  "hysteresis_streak_max_before"
}

HysteresisBoundaryCases == {"hysteresis_boundary"}
HysteresisAfterCases == {"hysteresis_after"}
HysteresisValidCases ==
  HysteresisBeforeCases \union HysteresisBoundaryCases \union HysteresisAfterCases

Streak0Cases == {"hysteresis_streak0_before", "hysteresis_boundary",
  "hysteresis_after"}
Streak1Cases == {"hysteresis_streak1_before"}
Streak2Cases == {"hysteresis_streak2_before"}
Streak3Cases == {"hysteresis_streak3_before"}
StreakMaxCases == {"hysteresis_streak_max_before"}

SpecUsesFastTimeout(c) == c \in NearFastEligibleCases

SpecReturnsBaseTimeout(c) ==
  c \in EffectiveCases /\ c \notin NearFastShorterCases

SpecReturnsShorterNearTimeout(c) == c \in NearFastShorterCases

SpecHysteresisWait(c) == c \in HysteresisBeforeCases

SpecNextStreak(c) ==
  IF c \in Streak0Cases THEN 1
  ELSE IF c \in Streak1Cases THEN 2
  ELSE IF c \in Streak2Cases THEN 3
  ELSE IF c \in Streak3Cases THEN 4
  ELSE IF c \in StreakMaxCases THEN 255
  ELSE 0

SpecHysteresisFactor(c) ==
  IF c \in Streak0Cases THEN 2
  ELSE IF c \in Streak1Cases THEN 3
  ELSE IF c \in (Streak2Cases \union Streak3Cases \union StreakMaxCases) THEN 4
  ELSE 0

ActualUsesFastTimeout(c) ==
  \/ /\ SpecUsesFastTimeout(c)
     /\ Bug # "skip_near_min_path"
  \/ /\ c = "effective_zero_votes"
     /\ Bug = "fast_without_votes"
  \/ /\ c = "effective_far_from_quorum"
     /\ Bug = "fast_far_from_quorum"
  \/ /\ c = "effective_at_quorum"
     /\ Bug = "fast_at_quorum"
  \/ /\ c = "effective_near_no_missing_data"
     /\ Bug = "fast_without_missing_data"
  \/ /\ c = "effective_near_consensus_backlog"
     /\ Bug = "fast_with_consensus_backlog"
  \/ /\ c = "effective_near_rbc_incomplete"
     /\ Bug = "fast_with_rbc_incomplete"
  \/ /\ c = "effective_near_both_backlogs"
     /\ Bug = "fast_with_both_backlogs"

ActualReturnsBaseTimeout(c) ==
  \/ /\ SpecReturnsBaseTimeout(c)
     /\ ~(Bug = "return_near_when_min_is_base"
          /\ c \in NearFastNotShorterCases)
  \/ /\ c \in NearFastShorterCases
     /\ Bug = "skip_near_fast_shorter"

ActualReturnsShorterNearTimeout(c) ==
  \/ /\ SpecReturnsShorterNearTimeout(c)
     /\ Bug # "skip_near_fast_shorter"
  \/ /\ c \in NonFastEffectiveCases
     /\ Bug = "return_fast_for_non_near"
  \/ /\ c \in NearFastNotShorterCases
     /\ Bug = "return_near_when_min_is_base"

ActualHysteresisWait(c) ==
  \/ /\ SpecHysteresisWait(c)
     /\ Bug # "no_wait_before_boundary"
  \/ /\ c = "hysteresis_permissioned_mode"
     /\ Bug = "hysteresis_in_permissioned"
  \/ /\ c = "hysteresis_zero_quorum_timeout"
     /\ Bug = "hysteresis_zero_timeout"
  \/ /\ c = "hysteresis_no_previous"
     /\ Bug = "hysteresis_without_previous"
  \/ /\ c = "hysteresis_height_mismatch"
     /\ Bug = "hysteresis_height_mismatch"
  \/ /\ c = "hysteresis_same_view"
     /\ Bug = "hysteresis_same_view"
  \/ /\ c = "hysteresis_lower_view"
     /\ Bug = "hysteresis_lower_view"
  \/ /\ c \in HysteresisBoundaryCases
     /\ Bug = "boundary_still_waits"
  \/ /\ c \in HysteresisAfterCases
     /\ Bug = "after_still_waits"

ActualNextStreak(c) ==
  IF Bug = "streak_overflow_wraps" /\ c \in StreakMaxCases THEN 0
  ELSE IF Bug = "skip_streak_increment" /\ c \in HysteresisValidCases THEN 0
  ELSE SpecNextStreak(c)

ActualHysteresisFactor(c) ==
  IF Bug = "wrong_factor_streak0" /\ c \in Streak0Cases THEN 1
  ELSE IF Bug = "wrong_factor_streak1" /\ c \in Streak1Cases THEN 2
  ELSE IF Bug = "wrong_factor_streak2" /\ c \in Streak2Cases THEN 3
  ELSE IF Bug = "streak_not_capped_for_factor"
    /\ c \in (Streak3Cases \union StreakMaxCases) THEN 5
  ELSE SpecHysteresisFactor(c)

Init ==
  /\ candidate = "none"
  /\ uses_fast_timeout = FALSE
  /\ returns_base_timeout = FALSE
  /\ returns_shorter_near_timeout = FALSE
  /\ hysteresis_wait = FALSE
  /\ next_streak = 0
  /\ hysteresis_factor = 0

CheckCase(c) ==
  /\ candidate = "none"
  /\ candidate' = c
  /\ uses_fast_timeout' = ActualUsesFastTimeout(c)
  /\ returns_base_timeout' = ActualReturnsBaseTimeout(c)
  /\ returns_shorter_near_timeout' = ActualReturnsShorterNearTimeout(c)
  /\ hysteresis_wait' = ActualHysteresisWait(c)
  /\ next_streak' = ActualNextStreak(c)
  /\ hysteresis_factor' = ActualHysteresisFactor(c)

Next ==
  \/ \E c \in Cases : CheckCase(c)
  \/ /\ candidate # "none"
     /\ UNCHANGED vars

TypeInvariant ==
  /\ candidate \in Cases \union {"none"}
  /\ uses_fast_timeout \in BOOLEAN
  /\ returns_base_timeout \in BOOLEAN
  /\ returns_shorter_near_timeout \in BOOLEAN
  /\ hysteresis_wait \in BOOLEAN
  /\ next_streak \in 0..255
  /\ hysteresis_factor \in 0..5

CasePartitionExact ==
  /\ Cases = EffectiveCases \union HysteresisCases
  /\ EffectiveCases \intersect HysteresisCases = {}
  /\ EffectiveCases = NearFastEligibleCases \union NonFastEffectiveCases
  /\ NearFastEligibleCases =
       NearFastShorterCases \union NearFastNotShorterCases
  /\ NearFastShorterCases \intersect NearFastNotShorterCases = {}
  /\ HysteresisCases = HysteresisInvalidCases \union HysteresisValidCases
  /\ HysteresisInvalidCases \intersect HysteresisValidCases = {}
  /\ HysteresisValidCases =
       HysteresisBeforeCases \union HysteresisBoundaryCases
       \union HysteresisAfterCases
  /\ HysteresisValidCases =
       Streak0Cases \union Streak1Cases \union Streak2Cases
       \union Streak3Cases \union StreakMaxCases

FastTimeoutMatchesSpec ==
  candidate = "none" \/ uses_fast_timeout = SpecUsesFastTimeout(candidate)

BaseTimeoutMatchesSpec ==
  candidate = "none" \/ returns_base_timeout = SpecReturnsBaseTimeout(candidate)

ShorterNearTimeoutMatchesSpec ==
  candidate = "none" \/
    returns_shorter_near_timeout = SpecReturnsShorterNearTimeout(candidate)

HysteresisWaitMatchesSpec ==
  candidate = "none" \/ hysteresis_wait = SpecHysteresisWait(candidate)

NextStreakMatchesSpec ==
  candidate = "none" \/ next_streak = SpecNextStreak(candidate)

HysteresisFactorMatchesSpec ==
  candidate = "none" \/ hysteresis_factor = SpecHysteresisFactor(candidate)

FastTimeoutRequiresNearQuorumMissingDataAndNoBacklog ==
  uses_fast_timeout => candidate \in NearFastEligibleCases

ShorterNearTimeoutRequiresFastMinPath ==
  returns_shorter_near_timeout => uses_fast_timeout

NearFastShorterUsesMinAndReturnsShorter ==
  candidate \in NearFastShorterCases =>
    /\ uses_fast_timeout
    /\ returns_shorter_near_timeout
    /\ ~returns_base_timeout

NearFastNotShorterUsesMinButReturnsBase ==
  candidate \in NearFastNotShorterCases =>
    /\ uses_fast_timeout
    /\ returns_base_timeout
    /\ ~returns_shorter_near_timeout

NonFastEffectiveCasesReturnBase ==
  candidate \in NonFastEffectiveCases =>
    /\ ~uses_fast_timeout
    /\ returns_base_timeout
    /\ ~returns_shorter_near_timeout

HysteresisWaitOnlyBeforeBoundary ==
  hysteresis_wait => candidate \in HysteresisBeforeCases

HysteresisWaitRequiresPositiveFactor ==
  hysteresis_wait => hysteresis_factor > 0

BoundaryAndAfterDoNotWait ==
  candidate \in (HysteresisBoundaryCases \union HysteresisAfterCases) =>
    ~hysteresis_wait

InvalidHysteresisInputsDoNotWaitOrAdvanceStreak ==
  candidate \in HysteresisInvalidCases =>
    /\ ~hysteresis_wait
    /\ next_streak = 0
    /\ hysteresis_factor = 0

StreakZeroUsesTwoTimeoutFactor ==
  candidate \in Streak0Cases => hysteresis_factor = 2

StreakOneUsesThreeTimeoutFactor ==
  candidate \in Streak1Cases => hysteresis_factor = 3

StreakTwoAndAboveUseCappedFourTimeoutFactor ==
  candidate \in (Streak2Cases \union Streak3Cases \union StreakMaxCases) =>
    hysteresis_factor = 4

MaxStreakSaturatesAndCapsFactor ==
  candidate \in StreakMaxCases =>
    /\ next_streak = 255
    /\ hysteresis_factor = 4

Safety ==
  /\ CasePartitionExact
  /\ FastTimeoutMatchesSpec
  /\ BaseTimeoutMatchesSpec
  /\ ShorterNearTimeoutMatchesSpec
  /\ HysteresisWaitMatchesSpec
  /\ NextStreakMatchesSpec
  /\ HysteresisFactorMatchesSpec
  /\ FastTimeoutRequiresNearQuorumMissingDataAndNoBacklog
  /\ ShorterNearTimeoutRequiresFastMinPath
  /\ NearFastShorterUsesMinAndReturnsShorter
  /\ NearFastNotShorterUsesMinButReturnsBase
  /\ NonFastEffectiveCasesReturnBase
  /\ HysteresisWaitOnlyBeforeBoundary
  /\ HysteresisWaitRequiresPositiveFactor
  /\ BoundaryAndAfterDoNotWait
  /\ InvalidHysteresisInputsDoNotWaitOrAdvanceStreak
  /\ StreakZeroUsesTwoTimeoutFactor
  /\ StreakOneUsesThreeTimeoutFactor
  /\ StreakTwoAndAboveUseCappedFourTimeoutFactor
  /\ MaxStreakSaturatesAndCapsFactor

=============================================================================
====
