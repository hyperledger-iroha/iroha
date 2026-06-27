---- MODULE SumeragiBlockSyncRosterGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `allow_uncertified_block_sync_roster(...)`.

The helper decides when block-sync roster selection may proceed without
certified roster artifacts. It should allow any height when the local node
explicitly requested a missing block, and otherwise only the exact next height
using Rust's saturating `u64::saturating_add(1)` edge behavior.
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
  "requested_stale",
  "requested_same_height",
  "requested_next_height",
  "requested_future",
  "unrequested_zero_next",
  "unrequested_next_height",
  "unrequested_saturated_next",
  "unrequested_same_height",
  "unrequested_stale",
  "unrequested_future"
}

RequestedMissing(c) ==
  c \in {
    "requested_stale",
    "requested_same_height",
    "requested_next_height",
    "requested_future"
  }

LocalHeight(c) ==
  CASE c = "requested_stale" -> 3
    [] c = "requested_same_height" -> 3
    [] c = "requested_next_height" -> 3
    [] c = "requested_future" -> 3
    [] c = "unrequested_zero_next" -> 0
    [] c = "unrequested_next_height" -> 3
    [] c = "unrequested_saturated_next" -> MaxHeight
    [] c = "unrequested_same_height" -> 3
    [] c = "unrequested_stale" -> 3
    [] OTHER -> 3

BlockHeight(c) ==
  CASE c = "requested_stale" -> 2
    [] c = "requested_same_height" -> 3
    [] c = "requested_next_height" -> 4
    [] c = "requested_future" -> 5
    [] c = "unrequested_zero_next" -> 1
    [] c = "unrequested_next_height" -> 4
    [] c = "unrequested_saturated_next" -> MaxHeight
    [] c = "unrequested_same_height" -> 3
    [] c = "unrequested_stale" -> 2
    [] OTHER -> 5

SaturatingAddOne(h) ==
  IF h = MaxHeight THEN MaxHeight ELSE h + 1

SpecAllow(c) ==
  RequestedMissing(c) \/ BlockHeight(c) = SaturatingAddOne(LocalHeight(c))

ActualAllow(c) ==
  CASE Bug = "requested_stale_rejected"
       /\ c = "requested_stale" -> FALSE
    [] Bug = "requested_same_rejected"
       /\ c = "requested_same_height" -> FALSE
    [] Bug = "requested_future_rejected"
       /\ c = "requested_future" -> FALSE
    [] Bug = "unrequested_zero_next_rejected"
       /\ c = "unrequested_zero_next" -> FALSE
    [] Bug = "unrequested_next_rejected"
       /\ c = "unrequested_next_height" -> FALSE
    [] Bug = "saturated_next_rejected"
       /\ c = "unrequested_saturated_next" -> FALSE
    [] Bug = "unrequested_same_accepted"
       /\ c = "unrequested_same_height" -> TRUE
    [] Bug = "unrequested_stale_accepted"
       /\ c = "unrequested_stale" -> TRUE
    [] Bug = "unrequested_future_accepted"
       /\ c = "unrequested_future" -> TRUE
    [] OTHER -> SpecAllow(c)

Matches(c) ==
  ActualAllow(c) = SpecAllow(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "requested_stale_rejected",
       "requested_same_rejected",
       "requested_future_rejected",
       "unrequested_zero_next_rejected",
       "unrequested_next_rejected",
       "saturated_next_rejected",
       "unrequested_same_accepted",
       "unrequested_stale_accepted",
       "unrequested_future_accepted"
     }
  /\ checked = 0
  /\ MaxHeight = 5

RosterAdmissionMatchesSpec ==
  \A c \in Cases: Matches(c)

BlockSyncRosterExactness ==
  RosterAdmissionMatchesSpec

BlockSyncRosterCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncRosterExactness

SafetyFast ==
  BlockSyncRosterExactness

RequestedStaleAllowed ==
  Matches("requested_stale")

RequestedSameAllowed ==
  Matches("requested_same_height")

RequestedFutureAllowed ==
  Matches("requested_future")

UnrequestedZeroNextAllowed ==
  Matches("unrequested_zero_next")

UnrequestedNextAllowed ==
  Matches("unrequested_next_height")

SaturatedNextAllowed ==
  Matches("unrequested_saturated_next")

UnrequestedSameRejected ==
  Matches("unrequested_same_height")

UnrequestedStaleRejected ==
  Matches("unrequested_stale")

UnrequestedFutureRejected ==
  Matches("unrequested_future")

====
