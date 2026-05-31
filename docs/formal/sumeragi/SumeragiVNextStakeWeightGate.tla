---- MODULE SumeragiVNextStakeWeightGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for vNext stake-weight helper semantics in
`sumeragi/vnext.rs`.

This slice pins `stake_weight(...)` and the arithmetic contract used by
`stake_quorum_satisfied(...)`: lookup returns the first matching validator
weight, absent validators return `None`, zero weights are still present
weights, zero total stake fails closed, missing weights fail closed, checked
addition/multiplication failures fail closed, and stake quorum is strict
greater-than two thirds rather than inclusive.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoWeight == -1
MaxNumeric == 20

LookupCases == {
  "lookup_empty",
  "lookup_first",
  "lookup_second",
  "lookup_absent",
  "lookup_duplicate_first",
  "lookup_zero_weight"
}

SpecWeight(c) ==
  CASE c = "lookup_first" -> 3
    [] c = "lookup_second" -> 5
    [] c = "lookup_duplicate_first" -> 7
    [] c = "lookup_zero_weight" -> 0
    [] OTHER -> NoWeight

ActualWeight(c) ==
  CASE Bug = "lookup_empty_returns_zero"
       /\ c = "lookup_empty" -> 0
    [] Bug = "lookup_absent_returns_zero"
       /\ c = "lookup_absent" -> 0
    [] Bug = "lookup_skips_first"
       /\ c = "lookup_first" -> NoWeight
    [] Bug = "lookup_uses_last_duplicate"
       /\ c = "lookup_duplicate_first" -> 1
    [] Bug = "lookup_rejects_zero_weight"
       /\ c = "lookup_zero_weight" -> NoWeight
    [] OTHER -> SpecWeight(c)

QuorumCases == {
  "quorum_zero_total",
  "quorum_missing_weight",
  "quorum_boundary",
  "quorum_above",
  "quorum_add_overflow",
  "quorum_stake_mul_overflow",
  "quorum_total_mul_overflow",
  "quorum_duplicate_weight_first"
}

QuorumTotal(c) ==
  CASE c = "quorum_zero_total" -> 0
    [] c = "quorum_boundary" -> 9
    [] c = "quorum_above" -> 8
    [] c = "quorum_add_overflow" -> 9
    [] c = "quorum_stake_mul_overflow" -> 10
    [] c = "quorum_total_mul_overflow" -> 11
    [] c = "quorum_duplicate_weight_first" -> 8
    [] OTHER -> 9

QuorumStake(c) ==
  CASE c = "quorum_boundary" -> 6
    [] c = "quorum_above" -> 6
    [] c = "quorum_add_overflow" -> MaxNumeric + 1
    [] c = "quorum_stake_mul_overflow" -> 8
    [] c = "quorum_total_mul_overflow" -> 5
    [] c = "quorum_duplicate_weight_first" -> 6
    [] OTHER -> 4

QuorumWeightsKnown(c) ==
  c # "quorum_missing_weight"

QuorumAddOk(c) ==
  c # "quorum_add_overflow"

SpecQuorum(c) ==
  IF QuorumTotal(c) = 0 THEN
    FALSE
  ELSE IF ~QuorumWeightsKnown(c) THEN
    FALSE
  ELSE IF ~QuorumAddOk(c) THEN
    FALSE
  ELSE IF QuorumStake(c) * 3 > MaxNumeric THEN
    FALSE
  ELSE IF QuorumTotal(c) * 2 > MaxNumeric THEN
    FALSE
  ELSE
    QuorumStake(c) * 3 > QuorumTotal(c) * 2

ActualQuorum(c) ==
  CASE Bug = "quorum_zero_total_accepts"
       /\ c = "quorum_zero_total" -> TRUE
    [] Bug = "quorum_missing_weight_accepts"
       /\ c = "quorum_missing_weight" -> TRUE
    [] Bug = "quorum_boundary_inclusive"
       /\ c = "quorum_boundary" ->
      QuorumStake(c) * 3 >= QuorumTotal(c) * 2
    [] Bug = "quorum_add_overflow_saturates"
       /\ c = "quorum_add_overflow" -> TRUE
    [] Bug = "quorum_stake_mul_overflow_saturates"
       /\ c = "quorum_stake_mul_overflow" -> TRUE
    [] Bug = "quorum_total_mul_overflow_zero"
       /\ c = "quorum_total_mul_overflow" -> TRUE
    [] Bug = "quorum_uses_last_duplicate_weight"
       /\ c = "quorum_duplicate_weight_first" ->
      1 * 3 > QuorumTotal(c) * 2
    [] OTHER -> SpecQuorum(c)

Init == checked = 0
Next == checked = 0 /\ checked' = 1

TypeInvariant == checked \in 0..1

SafetyFast ==
  /\ \A c \in LookupCases: ActualWeight(c) = SpecWeight(c)
  /\ \A c \in QuorumCases: ActualQuorum(c) = SpecQuorum(c)
  /\ ActualWeight("lookup_empty") = NoWeight
  /\ ActualWeight("lookup_absent") = NoWeight
  /\ ActualWeight("lookup_duplicate_first") = 7
  /\ ActualWeight("lookup_zero_weight") = 0
  /\ ActualQuorum("quorum_zero_total") = FALSE
  /\ ActualQuorum("quorum_missing_weight") = FALSE
  /\ ActualQuorum("quorum_boundary") = FALSE
  /\ ActualQuorum("quorum_above") = TRUE
  /\ ActualQuorum("quorum_add_overflow") = FALSE
  /\ ActualQuorum("quorum_stake_mul_overflow") = FALSE
  /\ ActualQuorum("quorum_total_mul_overflow") = FALSE

BugLookupEmptyReturnsZero ==
  ActualWeight("lookup_empty") = SpecWeight("lookup_empty")

BugLookupAbsentReturnsZero ==
  ActualWeight("lookup_absent") = SpecWeight("lookup_absent")

BugLookupSkipsFirst ==
  ActualWeight("lookup_first") = SpecWeight("lookup_first")

BugLookupUsesLastDuplicate ==
  ActualWeight("lookup_duplicate_first") = SpecWeight("lookup_duplicate_first")

BugLookupRejectsZeroWeight ==
  ActualWeight("lookup_zero_weight") = SpecWeight("lookup_zero_weight")

BugQuorumZeroTotalAccepts ==
  ActualQuorum("quorum_zero_total") = SpecQuorum("quorum_zero_total")

BugQuorumMissingWeightAccepts ==
  ActualQuorum("quorum_missing_weight") = SpecQuorum("quorum_missing_weight")

BugQuorumBoundaryInclusive ==
  ActualQuorum("quorum_boundary") = SpecQuorum("quorum_boundary")

BugQuorumAddOverflowSaturates ==
  ActualQuorum("quorum_add_overflow") = SpecQuorum("quorum_add_overflow")

BugQuorumStakeMulOverflowSaturates ==
  ActualQuorum("quorum_stake_mul_overflow") =
    SpecQuorum("quorum_stake_mul_overflow")

BugQuorumTotalMulOverflowZero ==
  ActualQuorum("quorum_total_mul_overflow") =
    SpecQuorum("quorum_total_mul_overflow")

BugQuorumUsesLastDuplicateWeight ==
  ActualQuorum("quorum_duplicate_weight_first") =
    SpecQuorum("quorum_duplicate_weight_first")
====
