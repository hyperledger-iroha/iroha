---- MODULE SumeragiVNextChainOrderGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for vNext chain-order and helper construction.

`ChainOrder::new(...)` is the validation gate for deterministic successor
ordering. It must reject empty orders, zero or overlong critical prefixes, and
quarantine tails that start before the critical prefix or beyond the roster.
Accepted orders expose exactly the critical prefix and `successor_of(...)`
must never return a successor outside that prefix.

`QuorumPolicy::smallest_satisfying_prefix_len(...)` must return the first
prefix that satisfies count or strict stake quorum, and no prefix when quorum
is impossible. `build_signer_bitmap(...)` must use the canonical
ceil(roster_len / 8) byte length, reject duplicate signer indices, and reject
out-of-range indices before any certificate can carry the bitmap.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  orderOk,
  \* @type: Set(Int);
  critical,
  \* @type: Int;
  successor,
  \* @type: Int;
  prefixLen,
  \* @type: Bool;
  bitmapOk,
  \* @type: Int;
  bitmapLen

\* @type: <<Str, Bool, Set(Int), Int, Int, Bool, Int>>;
vars == <<candidate, orderOk, critical, successor, prefixLen, bitmapOk, bitmapLen>>

Validators == 1..9

OrderCases == {
  "valid_order",
  "empty_order",
  "zero_critical",
  "critical_after_end",
  "quarantine_before_critical",
  "quarantine_after_end"
}

InvalidOrderCases == OrderCases \ {"valid_order"}

SuccessorCases == {
  "successor_first",
  "successor_tail",
  "successor_quarantine",
  "successor_unknown"
}

PrefixCases == {
  "count_prefix_minimal",
  "count_prefix_none",
  "stake_prefix_minimal",
  "stake_exact_boundary",
  "stake_missing_weight",
  "stake_zero_total"
}

BitmapCases == {
  "bitmap_empty_roster",
  "bitmap_one_signer",
  "bitmap_eight_signers",
  "bitmap_nine_signers",
  "bitmap_duplicate",
  "bitmap_out_of_range"
}

Cases == OrderCases \union SuccessorCases \union PrefixCases \union BitmapCases

OrderLen(c) ==
  CASE c = "empty_order" -> 0
    [] c = "critical_after_end" -> 3
    [] c = "quarantine_after_end" -> 4
    [] OTHER -> 5

CriticalLen(c) ==
  CASE c = "zero_critical" -> 0
    [] c = "critical_after_end" -> 4
    [] OTHER -> 3

QuarantineStart(c) ==
  CASE c = "quarantine_before_critical" -> 2
    [] c = "quarantine_after_end" -> 5
    [] OTHER -> 3

SpecOrderOk(c) ==
  c \notin OrderCases \/
    /\ OrderLen(c) > 0
    /\ CriticalLen(c) > 0
    /\ CriticalLen(c) <= OrderLen(c)
    /\ QuarantineStart(c) >= CriticalLen(c)
    /\ QuarantineStart(c) <= OrderLen(c)

ActualOrderOk(c) ==
  CASE c = "empty_order" /\ Bug = "accept_empty_order" -> TRUE
    [] c = "zero_critical" /\ Bug = "accept_zero_critical" -> TRUE
    [] c = "critical_after_end" /\ Bug = "accept_critical_after_end" -> TRUE
    [] c = "quarantine_before_critical" /\
          Bug = "accept_quarantine_before_critical" -> TRUE
    [] c = "quarantine_after_end" /\
          Bug = "accept_quarantine_after_end" -> TRUE
    [] OTHER -> SpecOrderOk(c)

SpecCritical(c) ==
  IF c = "valid_order"
  THEN {idx \in Validators : idx <= CriticalLen(c)}
  ELSE {}

ActualCritical(c) ==
  CASE c = "valid_order" /\ Bug = "critical_path_includes_tail" ->
      {idx \in Validators : idx <= QuarantineStart(c) + 1}
    [] c = "valid_order" /\ Bug = "critical_path_drops_last" ->
      {idx \in Validators : idx < CriticalLen(c)}
    [] OTHER -> SpecCritical(c)

SpecSuccessor(c) ==
  CASE c = "successor_first" -> 2
    [] OTHER -> 0

ActualSuccessor(c) ==
  CASE c = "successor_first" /\ Bug = "successor_off_by_one" -> 3
    [] c = "successor_tail" /\ Bug = "tail_has_successor" -> 4
    [] c = "successor_unknown" /\ Bug = "unknown_has_successor" -> 1
    [] c = "successor_quarantine" /\ Bug = "quarantine_has_successor" -> 5
    [] OTHER -> SpecSuccessor(c)

SpecPrefixLen(c) ==
  CASE c = "count_prefix_minimal" -> 3
    [] c = "count_prefix_none" -> 0
    [] c = "stake_prefix_minimal" -> 2
    [] c = "stake_exact_boundary" -> 2
    [] c = "stake_missing_weight" -> 0
    [] c = "stake_zero_total" -> 0
    [] OTHER -> 0

ActualPrefixLen(c) ==
  CASE c = "count_prefix_minimal" /\ Bug = "count_prefix_off_by_one" -> 2
    [] c = "count_prefix_none" /\ Bug = "count_prefix_accepts_impossible" -> 5
    [] c = "stake_exact_boundary" /\ Bug = "stake_uses_non_strict" -> 1
    [] c = "stake_missing_weight" /\ Bug = "stake_missing_weight_accepted" -> 1
    [] c = "stake_zero_total" /\ Bug = "stake_zero_total_accepted" -> 1
    [] OTHER -> SpecPrefixLen(c)

SpecBitmapOk(c) ==
  c \notin BitmapCases \/ c \notin {"bitmap_duplicate", "bitmap_out_of_range"}

SpecBitmapLen(c) ==
  CASE c = "bitmap_empty_roster" -> 0
    [] c = "bitmap_one_signer" -> 1
    [] c = "bitmap_eight_signers" -> 1
    [] c = "bitmap_nine_signers" -> 2
    [] OTHER -> 0

ActualBitmapOk(c) ==
  CASE c = "bitmap_duplicate" /\ Bug = "bitmap_allows_duplicate" -> TRUE
    [] c = "bitmap_out_of_range" /\ Bug = "bitmap_allows_out_of_range" -> TRUE
    [] OTHER -> SpecBitmapOk(c)

ActualBitmapLen(c) ==
  CASE c = "bitmap_nine_signers" /\ Bug = "bitmap_wrong_length_for_nine" -> 1
    [] c = "bitmap_duplicate" /\ Bug = "bitmap_allows_duplicate" -> 1
    [] c = "bitmap_out_of_range" /\ Bug = "bitmap_allows_out_of_range" -> 1
    [] OTHER -> SpecBitmapLen(c)

BugModes == {
  "none",
  "accept_empty_order",
  "accept_zero_critical",
  "accept_critical_after_end",
  "accept_quarantine_before_critical",
  "accept_quarantine_after_end",
  "critical_path_includes_tail",
  "critical_path_drops_last",
  "successor_off_by_one",
  "tail_has_successor",
  "unknown_has_successor",
  "quarantine_has_successor",
  "count_prefix_off_by_one",
  "count_prefix_accepts_impossible",
  "stake_uses_non_strict",
  "stake_missing_weight_accepted",
  "stake_zero_total_accepted",
  "bitmap_wrong_length_for_nine",
  "bitmap_allows_duplicate",
  "bitmap_allows_out_of_range"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases \union {"none"}
  /\ orderOk \in BOOLEAN
  /\ critical \subseteq Validators
  /\ successor \in 0..9
  /\ prefixLen \in 0..9
  /\ bitmapOk \in BOOLEAN
  /\ bitmapLen \in 0..2

Init ==
  /\ candidate = "none"
  /\ orderOk = FALSE
  /\ critical = {}
  /\ successor = 0
  /\ prefixLen = 0
  /\ bitmapOk = FALSE
  /\ bitmapLen = 0

Apply(c) ==
  /\ candidate' = c
  /\ orderOk' = ActualOrderOk(c)
  /\ critical' = ActualCritical(c)
  /\ successor' = ActualSuccessor(c)
  /\ prefixLen' = ActualPrefixLen(c)
  /\ bitmapOk' = ActualBitmapOk(c)
  /\ bitmapLen' = ActualBitmapLen(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

OrderOkMatchesSpec ==
  candidate = "none" \/ candidate \notin OrderCases
    \/ orderOk = SpecOrderOk(candidate)

InvalidOrdersFailClosed ==
  candidate \in InvalidOrderCases => ~orderOk

ValidOrderCriticalPrefixMatches ==
  candidate = "valid_order" =>
    /\ orderOk
    /\ critical = SpecCritical(candidate)

CriticalPathExcludesTail ==
  candidate = "valid_order" =>
    \A idx \in critical: idx <= CriticalLen(candidate)

RejectedOrdersExposeNoCriticalPath ==
  candidate \in InvalidOrderCases => critical = {}

SuccessorMatchesSpec ==
  candidate \in SuccessorCases => successor = SpecSuccessor(candidate)

TailPeerHasNoCriticalSuccessor ==
  candidate \in {
    "successor_tail",
    "successor_quarantine",
    "successor_unknown"
  } => successor = 0

PrefixMatchesSpec ==
  candidate \in PrefixCases => prefixLen = SpecPrefixLen(candidate)

CountPrefixMinimal ==
  candidate = "count_prefix_minimal" => prefixLen = 3

ImpossibleCountPrefixReturnsNone ==
  candidate = "count_prefix_none" => prefixLen = 0

StrictStakeBoundaryNeedsMoreThanExact ==
  candidate = "stake_exact_boundary" => prefixLen = 2

StakeFailuresFailClosed ==
  candidate \in {"stake_missing_weight", "stake_zero_total"} => prefixLen = 0

BitmapOkMatchesSpec ==
  candidate \in BitmapCases => bitmapOk = SpecBitmapOk(candidate)

BitmapLengthMatchesSpec ==
  candidate \in BitmapCases => bitmapLen = SpecBitmapLen(candidate)

BitmapFailuresFailClosed ==
  candidate \in {"bitmap_duplicate", "bitmap_out_of_range"} =>
    /\ ~bitmapOk
    /\ bitmapLen = 0

Safety ==
  /\ OrderOkMatchesSpec
  /\ InvalidOrdersFailClosed
  /\ ValidOrderCriticalPrefixMatches
  /\ CriticalPathExcludesTail
  /\ RejectedOrdersExposeNoCriticalPath
  /\ SuccessorMatchesSpec
  /\ TailPeerHasNoCriticalSuccessor
  /\ PrefixMatchesSpec
  /\ CountPrefixMinimal
  /\ ImpossibleCountPrefixReturnsNone
  /\ StrictStakeBoundaryNeedsMoreThanExact
  /\ StakeFailuresFailClosed
  /\ BitmapOkMatchesSpec
  /\ BitmapLengthMatchesSpec
  /\ BitmapFailuresFailClosed

====
