---- MODULE SumeragiBlockBodyRequestStashGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `should_stash_pending_block_body_request(...)`.

The helper keeps exact block-body requesters only for the next committed slot
through the bounded missing-request stale-height margin. The configured margin
is floored at one, and both lower and upper window edges use Rust-style
saturating addition.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

MaxHeight == 5

Cases == {
  "zero_margin_next",
  "one_margin_next",
  "within_margin",
  "upper_boundary",
  "beyond_margin",
  "same_height",
  "stale_height",
  "zero_committed_next",
  "saturated_committed_boundary",
  "saturated_upper_boundary",
  "saturated_lower_below"
}

CommittedHeight(c) ==
  CASE c = "zero_committed_next" -> 0
    [] c \in {"saturated_committed_boundary", "saturated_lower_below"} -> MaxHeight
    [] c = "saturated_upper_boundary" -> MaxHeight - 1
    [] OTHER -> 3

RawMargin(c) ==
  CASE c \in {"zero_margin_next", "zero_committed_next", "saturated_committed_boundary"} -> 0
    [] c \in {"one_margin_next", "beyond_margin"} -> 1
    [] c = "upper_boundary" -> 2
    [] c = "saturated_upper_boundary" -> 3
    [] OTHER -> 3

Height(c) ==
  CASE c \in {"zero_margin_next", "one_margin_next"} -> 4
    [] c = "within_margin" -> 5
    [] c = "upper_boundary" -> 5
    [] c = "beyond_margin" -> 5
    [] c = "same_height" -> 3
    [] c = "stale_height" -> 2
    [] c = "zero_committed_next" -> 1
    [] c = "saturated_committed_boundary" -> MaxHeight
    [] c = "saturated_upper_boundary" -> MaxHeight
    [] OTHER -> MaxHeight - 1

Max(a, b) ==
  IF a >= b THEN a ELSE b

SatAdd(a, b) ==
  IF a + b > MaxHeight THEN MaxHeight ELSE a + b

EffectiveMargin(c) ==
  Max(RawMargin(c), 1)

LowerBound(c) ==
  SatAdd(CommittedHeight(c), 1)

UpperBound(c) ==
  SatAdd(CommittedHeight(c), EffectiveMargin(c))

SpecStash(c) ==
  /\ Height(c) >= LowerBound(c)
  /\ Height(c) <= UpperBound(c)

ActualStash(c) ==
  CASE Bug = "zero_margin_not_floored"
       /\ c = "zero_margin_next" -> FALSE
    [] Bug = "next_height_rejected"
       /\ c = "one_margin_next" -> FALSE
    [] Bug = "within_margin_rejected"
       /\ c = "within_margin" -> FALSE
    [] Bug = "upper_boundary_exclusive"
       /\ c = "upper_boundary" -> FALSE
    [] Bug = "beyond_margin_allowed"
       /\ c = "beyond_margin" -> TRUE
    [] Bug = "same_height_allowed"
       /\ c = "same_height" -> TRUE
    [] Bug = "stale_height_allowed"
       /\ c = "stale_height" -> TRUE
    [] Bug = "zero_committed_next_rejected"
       /\ c = "zero_committed_next" -> FALSE
    [] Bug = "saturated_committed_boundary_rejected"
       /\ c = "saturated_committed_boundary" -> FALSE
    [] Bug = "saturated_upper_boundary_rejected"
       /\ c = "saturated_upper_boundary" -> FALSE
    [] Bug = "saturated_lower_below_allowed"
       /\ c = "saturated_lower_below" -> TRUE
    [] OTHER -> SpecStash(c)

Matches(c) ==
  ActualStash(c) = SpecStash(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "zero_margin_not_floored",
       "next_height_rejected",
       "within_margin_rejected",
       "upper_boundary_exclusive",
       "beyond_margin_allowed",
       "same_height_allowed",
       "stale_height_allowed",
       "zero_committed_next_rejected",
       "saturated_committed_boundary_rejected",
       "saturated_upper_boundary_rejected",
       "saturated_lower_below_allowed"
     }
  /\ checked = 0
  /\ MaxHeight = 5

StashWindowMatchesSpec ==
  \A c \in Cases: Matches(c)

SafetyFast == StashWindowMatchesSpec

ZeroMarginNextAllowed ==
  Matches("zero_margin_next")

NextHeightAllowed ==
  Matches("one_margin_next")

WithinMarginAllowed ==
  Matches("within_margin")

UpperBoundaryAllowed ==
  Matches("upper_boundary")

BeyondMarginRejected ==
  Matches("beyond_margin")

SameHeightRejected ==
  Matches("same_height")

StaleHeightRejected ==
  Matches("stale_height")

ZeroCommittedNextAllowed ==
  Matches("zero_committed_next")

SaturatedCommittedBoundaryAllowed ==
  Matches("saturated_committed_boundary")

SaturatedUpperBoundaryAllowed ==
  Matches("saturated_upper_boundary")

SaturatedLowerBelowRejected ==
  Matches("saturated_lower_below")

====
