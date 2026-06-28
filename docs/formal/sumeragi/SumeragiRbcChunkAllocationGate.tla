---- MODULE SumeragiRbcChunkAllocationGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `distribute_chunks(...)` and
`distribute_allocation_weights(...)`.

The helper pair assigns a bounded number of RBC chunks to weighted lanes and
dataspaces. The base helper uses floor division plus largest-remainder
tie-breaking by lower index. The allocation wrapper then gives zero-allocation
positive weights one chunk and trims excess from the end, matching the concrete
reverse-order adjustment.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Int;
  base_len,
  \* @type: Int;
  min_len,
  \* @type: Int;
  base0,
  \* @type: Int;
  base1,
  \* @type: Int;
  base2,
  \* @type: Int;
  min0,
  \* @type: Int;
  min1,
  \* @type: Int;
  min2

\* @type: <<Str, Int, Int, Int, Int, Int, Int, Int, Int>>;
vars == <<candidate, base_len, min_len, base0, base1, base2, min0, min1, min2>>

Cases == {
  "empty_weights",
  "zero_total_positive",
  "all_zero_under_total",
  "all_zero_over_total",
  "equal_exact",
  "tie_remainder",
  "weighted_priority",
  "zero_weight_excluded",
  "min_one_trim_reverse",
  "two_slots_tie",
  "positive_count_exceeds_total"
}

CountValues == 0..32

Len(c) ==
  CASE c = "empty_weights" -> 0
    [] c = "two_slots_tie" -> 2
    [] OTHER -> 3

Total(c) ==
  CASE c = "empty_weights" -> 4
    [] c = "zero_total_positive" -> 0
    [] c = "all_zero_under_total" -> 2
    [] c = "all_zero_over_total" -> 5
    [] c = "equal_exact" -> 6
    [] c = "tie_remainder" -> 2
    [] c = "weighted_priority" -> 5
    [] c = "zero_weight_excluded" -> 3
    [] c = "min_one_trim_reverse" -> 2
    [] c = "two_slots_tie" -> 1
    [] c = "positive_count_exceeds_total" -> 1

Weight0(c) ==
  CASE c \in {"all_zero_under_total", "all_zero_over_total", "empty_weights"} -> 0
    [] c = "zero_weight_excluded" -> 0
    [] OTHER -> 1

Weight1(c) ==
  CASE c \in {"all_zero_under_total", "all_zero_over_total", "empty_weights"} -> 0
    [] c \in {"weighted_priority", "min_one_trim_reverse"} -> 100
    [] c = "zero_weight_excluded" -> 5
    [] OTHER -> 1

Weight2(c) ==
  CASE c \in {
         "all_zero_under_total",
         "all_zero_over_total",
         "empty_weights",
         "two_slots_tie",
         "zero_weight_excluded"
       } -> 0
    [] OTHER -> 1

TotalWeight(c) ==
  Weight0(c) + Weight1(c) + Weight2(c)

Min(a, b) ==
  IF a <= b THEN a ELSE b

SumBase ==
  base0 + base1 + base2

SumMin ==
  min0 + min1 + min2

SpecBase0(c) ==
  CASE c = "all_zero_under_total" -> 1
    [] c = "all_zero_over_total" -> 1
    [] c = "equal_exact" -> 2
    [] c = "tie_remainder" -> 1
    [] c = "two_slots_tie" -> 1
    [] c = "positive_count_exceeds_total" -> 1
    [] OTHER -> 0

SpecBase1(c) ==
  CASE c = "all_zero_under_total" -> 1
    [] c = "all_zero_over_total" -> 1
    [] c = "equal_exact" -> 2
    [] c = "tie_remainder" -> 1
    [] c = "weighted_priority" -> 5
    [] c = "zero_weight_excluded" -> 3
    [] c = "min_one_trim_reverse" -> 2
    [] OTHER -> 0

SpecBase2(c) ==
  CASE c = "all_zero_over_total" -> 1
    [] c = "equal_exact" -> 2
    [] OTHER -> 0

SpecMin0(c) ==
  CASE c = "weighted_priority" -> 1
    [] c = "min_one_trim_reverse" -> 1
    [] OTHER -> SpecBase0(c)

SpecMin1(c) ==
  CASE c = "weighted_priority" -> 4
    [] c = "min_one_trim_reverse" -> 1
    [] OTHER -> SpecBase1(c)

SpecMin2(c) ==
  SpecBase2(c)

ActualBaseLen(c) ==
  CASE Bug = "empty_returns_three" /\ c = "empty_weights" -> 3
    [] OTHER -> Len(c)

ActualMinLen(c) ==
  CASE Bug = "empty_returns_three" /\ c = "empty_weights" -> 3
    [] OTHER -> Len(c)

ActualBase0(c) ==
  CASE Bug = "zero_total_allocates_one" /\ c = "zero_total_positive" -> 1
    [] Bug = "all_zero_spreads_all_total" /\ c = "all_zero_over_total" -> 3
    [] Bug = "all_zero_skips_fallback" /\ c \in {"all_zero_under_total", "all_zero_over_total"} -> 0
    [] Bug = "zero_weight_gets_leftover" /\ c = "zero_weight_excluded" -> 1
    [] Bug = "remainder_tie_prefers_high_index" /\ c \in {"tie_remainder", "two_slots_tie"} -> 0
    [] Bug = "weighted_rounds_up_floor" /\ c = "weighted_priority" -> 1
    [] OTHER -> SpecBase0(c)

ActualBase1(c) ==
  CASE Bug = "all_zero_spreads_all_total" /\ c = "all_zero_over_total" -> 1
    [] Bug = "all_zero_skips_fallback" /\ c \in {"all_zero_under_total", "all_zero_over_total"} -> 0
    [] Bug = "weighted_ignores_remainder" /\ c = "weighted_priority" -> 4
    [] Bug = "weighted_rounds_up_floor" /\ c = "weighted_priority" -> 5
    [] OTHER -> SpecBase1(c)

ActualBase2(c) ==
  CASE Bug = "all_zero_spreads_all_total" /\ c = "all_zero_over_total" -> 1
    [] Bug = "all_zero_skips_fallback" /\ c \in {"all_zero_under_total", "all_zero_over_total"} -> 0
    [] Bug = "zero_weight_gets_leftover" /\ c = "zero_weight_excluded" -> 1
    [] Bug = "remainder_tie_prefers_high_index" /\ c = "tie_remainder" -> 1
    [] Bug = "weighted_rounds_up_floor" /\ c = "weighted_priority" -> 1
    [] OTHER -> SpecBase2(c)

ActualMin0(c) ==
  CASE Bug = "zero_total_allocates_one" /\ c = "zero_total_positive" -> 1
    [] Bug = "min_wrapper_skips_minimum" /\ c = "weighted_priority" -> SpecBase0(c)
    [] Bug = "min_wrapper_keeps_excess" /\ c = "weighted_priority" -> 1
    [] Bug = "min_trim_forward" /\ c = "min_one_trim_reverse" -> 0
    [] OTHER -> SpecMin0(c)

ActualMin1(c) ==
  CASE Bug = "min_wrapper_skips_minimum" /\ c = "weighted_priority" -> SpecBase1(c)
    [] Bug = "min_wrapper_keeps_excess" /\ c = "weighted_priority" -> 5
    [] Bug = "min_trim_forward" /\ c = "min_one_trim_reverse" -> 1
    [] OTHER -> SpecMin1(c)

ActualMin2(c) ==
  CASE Bug = "min_wrapper_skips_minimum" /\ c = "weighted_priority" -> SpecBase2(c)
    [] Bug = "min_wrapper_keeps_excess" /\ c = "weighted_priority" -> 1
    [] Bug = "min_trim_forward" /\ c = "min_one_trim_reverse" -> 1
    [] OTHER -> SpecMin2(c)

TypeInvariant ==
  /\ Bug \in {
       "none",
       "empty_returns_three",
       "zero_total_allocates_one",
       "all_zero_spreads_all_total",
       "all_zero_skips_fallback",
       "zero_weight_gets_leftover",
       "remainder_tie_prefers_high_index",
       "weighted_ignores_remainder",
       "weighted_rounds_up_floor",
       "min_wrapper_skips_minimum",
       "min_wrapper_keeps_excess",
       "min_trim_forward"
     }
  /\ candidate \in Cases
  /\ base_len \in 0..3
  /\ min_len \in 0..3
  /\ base0 \in CountValues
  /\ base1 \in CountValues
  /\ base2 \in CountValues
  /\ min0 \in CountValues
  /\ min1 \in CountValues
  /\ min2 \in CountValues

Init ==
  /\ candidate \in Cases
  /\ base_len = ActualBaseLen(candidate)
  /\ min_len = ActualMinLen(candidate)
  /\ base0 = ActualBase0(candidate)
  /\ base1 = ActualBase1(candidate)
  /\ base2 = ActualBase2(candidate)
  /\ min0 = ActualMin0(candidate)
  /\ min1 = ActualMin1(candidate)
  /\ min2 = ActualMin2(candidate)

Next ==
  UNCHANGED vars

BaseMatchesSpec ==
  /\ base_len = Len(candidate)
  /\ base0 = SpecBase0(candidate)
  /\ base1 = SpecBase1(candidate)
  /\ base2 = SpecBase2(candidate)

MinMatchesSpec ==
  /\ min_len = Len(candidate)
  /\ min0 = SpecMin0(candidate)
  /\ min1 = SpecMin1(candidate)
  /\ min2 = SpecMin2(candidate)

EmptyWeightsReturnEmpty ==
  candidate = "empty_weights" =>
    /\ base_len = 0
    /\ min_len = 0

ZeroTotalYieldsZeros ==
  Total(candidate) = 0 =>
    /\ SumBase = 0
    /\ SumMin = 0

AllZeroFallbackOnePass ==
  TotalWeight(candidate) = 0 /\ Len(candidate) # 0 /\ Total(candidate) # 0 =>
    /\ SumBase = Min(Total(candidate), Len(candidate))
    /\ base0 <= 1
    /\ base1 <= 1
    /\ base2 <= 1

PositiveWeightBasePreservesTotal ==
  TotalWeight(candidate) # 0 => SumBase = Total(candidate)

ZeroWeightGetsNoBaseAllocation ==
  TotalWeight(candidate) # 0 =>
    /\ (Weight0(candidate) = 0 => base0 = 0)
    /\ (Weight1(candidate) = 0 => base1 = 0)
    /\ (Weight2(candidate) = 0 => base2 = 0)

EqualWeightsSplitEvenly ==
  candidate = "equal_exact" =>
    /\ base0 = 2
    /\ base1 = 2
    /\ base2 = 2

RemainderTiesPreferLowerIndex ==
  /\ candidate = "tie_remainder" =>
       /\ base0 = 1
       /\ base1 = 1
       /\ base2 = 0
  /\ candidate = "two_slots_tie" =>
       /\ base0 = 1
       /\ base1 = 0
       /\ base2 = 0

LargestRemainderReceivesLeftover ==
  candidate = "weighted_priority" =>
    /\ base0 = 0
    /\ base1 = 5
    /\ base2 = 0

MinWrapperPreservesZeroTotal ==
  Total(candidate) = 0 => SumMin = 0

MinWrapperPreservesTotalForPositiveWeights ==
  TotalWeight(candidate) # 0 => SumMin = Total(candidate)

MinWrapperNeverAllocatesZeroWeight ==
  TotalWeight(candidate) # 0 =>
    /\ (Weight0(candidate) = 0 => min0 = 0)
    /\ (Weight1(candidate) = 0 => min1 = 0)
    /\ (Weight2(candidate) = 0 => min2 = 0)

MinWrapperReverseTrimMatchesSpec ==
  /\ candidate = "weighted_priority" =>
       /\ min0 = 1
       /\ min1 = 4
       /\ min2 = 0
  /\ candidate = "min_one_trim_reverse" =>
       /\ min0 = 1
       /\ min1 = 1
       /\ min2 = 0
  /\ candidate = "positive_count_exceeds_total" =>
       /\ min0 = 1
       /\ min1 = 0
       /\ min2 = 0

AllocationEntriesBoundedByTotalWhenPositive ==
  Total(candidate) # 0 =>
    /\ base0 <= Total(candidate)
    /\ base1 <= Total(candidate)
    /\ base2 <= Total(candidate)
    /\ min0 <= Total(candidate)
    /\ min1 <= Total(candidate)
    /\ min2 <= Total(candidate)

RbcChunkAllocationCoreSafety ==
  /\ BaseMatchesSpec
  /\ MinMatchesSpec
  /\ EmptyWeightsReturnEmpty
  /\ ZeroTotalYieldsZeros
  /\ AllZeroFallbackOnePass
  /\ PositiveWeightBasePreservesTotal
  /\ ZeroWeightGetsNoBaseAllocation
  /\ EqualWeightsSplitEvenly
  /\ RemainderTiesPreferLowerIndex
  /\ LargestRemainderReceivesLeftover
  /\ MinWrapperPreservesZeroTotal
  /\ MinWrapperPreservesTotalForPositiveWeights
  /\ MinWrapperNeverAllocatesZeroWeight
  /\ MinWrapperReverseTrimMatchesSpec
  /\ AllocationEntriesBoundedByTotalWhenPositive

RbcChunkAllocationExactness ==
  /\ BaseMatchesSpec
  /\ MinMatchesSpec
  /\ EmptyWeightsReturnEmpty
  /\ ZeroTotalYieldsZeros
  /\ AllZeroFallbackOnePass
  /\ PositiveWeightBasePreservesTotal
  /\ ZeroWeightGetsNoBaseAllocation
  /\ EqualWeightsSplitEvenly
  /\ RemainderTiesPreferLowerIndex
  /\ LargestRemainderReceivesLeftover
  /\ MinWrapperPreservesZeroTotal
  /\ MinWrapperPreservesTotalForPositiveWeights
  /\ MinWrapperNeverAllocatesZeroWeight
  /\ MinWrapperReverseTrimMatchesSpec
  /\ AllocationEntriesBoundedByTotalWhenPositive

RbcChunkAllocationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcChunkAllocationExactness

Safety == RbcChunkAllocationExactness

====
