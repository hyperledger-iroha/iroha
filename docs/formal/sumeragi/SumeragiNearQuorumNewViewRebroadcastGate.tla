---- MODULE SumeragiNearQuorumNewViewRebroadcastGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for near-quorum NEW_VIEW rebroadcast.

This slice models `maybe_rebroadcast_near_quorum_new_view_votes(...)`. The
helper may rebroadcast partial NEW_VIEW votes only from validators, for the
committed frontier height, with nonzero support that is still below the
required quorum, and after the rebroadcast log admits the slot under the
cooldown floor `max(rebroadcast_cooldown, PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)`.
When admitted it must dispatch NEW_VIEW votes with backpressure and the exact
near-quorum label. A zero backpressure result returns zero and must not nudge
the pacemaker. A positive rebroadcast nudges the pacemaker only when
`now + PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL` is representable and earlier than
the current pacemaker deadline.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "valid_rebroadcast_nudges",
  "valid_rebroadcast_deadline_earlier",
  "valid_rebroadcast_time_overflow",
  "observer_rejected",
  "non_frontier_height_rejected",
  "zero_support_rejected",
  "exact_quorum_rejected",
  "over_quorum_rejected",
  "required_zero_rejected",
  "cooldown_blocked",
  "cooldown_floor_blocks",
  "backpressure_zero"
}

Observer(c) ==
  c = "observer_rejected"

FrontierHeight(c) ==
  c # "non_frontier_height_rejected"

Support(c) ==
  CASE c = "zero_support_rejected" -> 0
    [] c = "exact_quorum_rejected" -> 3
    [] c = "over_quorum_rejected" -> 4
    [] c = "required_zero_rejected" -> 1
    [] OTHER -> 2

Required(c) ==
  IF c = "required_zero_rejected" THEN 0 ELSE 3

SupportAdmissible(c) ==
  Support(c) > 0 /\ Support(c) < Required(c)

CooldownAllowed(c) ==
  c # "cooldown_blocked" /\ c # "cooldown_floor_blocks"

RawCooldownWouldAllow(c) ==
  c = "cooldown_floor_blocks"

RebroadcastedByBackpressure(c) ==
  IF c = "backpressure_zero" THEN 0 ELSE 2

CheckedAddOk(c) ==
  c # "valid_rebroadcast_time_overflow"

PacemakerDeadlineLater(c) ==
  c # "valid_rebroadcast_deadline_earlier"

SpecDispatch(c) ==
  /\ ~Observer(c)
  /\ FrontierHeight(c)
  /\ SupportAdmissible(c)
  /\ CooldownAllowed(c)

SpecReturn(c) ==
  IF SpecDispatch(c) THEN RebroadcastedByBackpressure(c) ELSE 0

SpecPacemakerNudge(c) ==
  /\ SpecReturn(c) > 0
  /\ CheckedAddOk(c)
  /\ PacemakerDeadlineLater(c)

SpecPhase(c) ==
  IF SpecDispatch(c) THEN "NewView" ELSE "none"

SpecBackpressure(c) ==
  SpecDispatch(c)

SpecLabel(c) ==
  IF SpecDispatch(c) THEN "near_quorum_new_view_rebroadcast" ELSE "none"

ActualDispatch(c) ==
  CASE Bug = "reject_valid_rebroadcast"
       /\ c = "valid_rebroadcast_nudges" -> FALSE
    [] Bug = "accept_observer"
       /\ c = "observer_rejected" -> TRUE
    [] Bug = "accept_non_frontier_height"
       /\ c = "non_frontier_height_rejected" -> TRUE
    [] Bug = "accept_zero_support"
       /\ c = "zero_support_rejected" -> TRUE
    [] Bug = "accept_exact_quorum"
       /\ c = "exact_quorum_rejected" -> TRUE
    [] Bug = "accept_over_quorum"
       /\ c = "over_quorum_rejected" -> TRUE
    [] Bug = "accept_required_zero"
       /\ c = "required_zero_rejected" -> TRUE
    [] Bug = "ignore_cooldown"
       /\ c = "cooldown_blocked" -> TRUE
    [] Bug = "use_raw_cooldown"
       /\ RawCooldownWouldAllow(c) -> TRUE
    [] OTHER -> SpecDispatch(c)

ActualReturn(c) ==
  CASE Bug = "return_nonzero_when_backpressure_zero"
       /\ c = "backpressure_zero" -> 2
    [] OTHER ->
       IF ActualDispatch(c) THEN RebroadcastedByBackpressure(c) ELSE 0

ActualPacemakerNudge(c) ==
  CASE Bug = "skip_pacemaker_nudge"
       /\ c = "valid_rebroadcast_nudges" -> FALSE
    [] Bug = "nudge_without_rebroadcast"
       /\ c = "backpressure_zero" -> TRUE
    [] Bug = "overwrite_earlier_deadline"
       /\ c = "valid_rebroadcast_deadline_earlier" -> TRUE
    [] Bug = "nudge_on_time_overflow"
       /\ c = "valid_rebroadcast_time_overflow" -> TRUE
    [] OTHER ->
       /\ ActualReturn(c) > 0
       /\ CheckedAddOk(c)
       /\ PacemakerDeadlineLater(c)

ActualPhase(c) ==
  IF Bug = "dispatch_commit_phase" /\ ActualDispatch(c)
  THEN "Commit"
  ELSE IF ActualDispatch(c) THEN "NewView" ELSE "none"

ActualBackpressure(c) ==
  IF Bug = "disable_backpressure" /\ ActualDispatch(c)
  THEN FALSE
  ELSE ActualDispatch(c)

ActualLabel(c) ==
  IF Bug = "wrong_rebroadcast_label" /\ ActualDispatch(c)
  THEN "generic_vote_rebroadcast"
  ELSE IF ActualDispatch(c) THEN "near_quorum_new_view_rebroadcast" ELSE "none"

Matches(c) ==
  /\ ActualDispatch(c) = SpecDispatch(c)
  /\ ActualReturn(c) = SpecReturn(c)
  /\ ActualPacemakerNudge(c) = SpecPacemakerNudge(c)
  /\ ActualPhase(c) = SpecPhase(c)
  /\ ActualBackpressure(c) = SpecBackpressure(c)
  /\ ActualLabel(c) = SpecLabel(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "reject_valid_rebroadcast",
       "accept_observer",
       "accept_non_frontier_height",
       "accept_zero_support",
       "accept_exact_quorum",
       "accept_over_quorum",
       "accept_required_zero",
       "ignore_cooldown",
       "use_raw_cooldown",
       "return_nonzero_when_backpressure_zero",
       "skip_pacemaker_nudge",
       "nudge_without_rebroadcast",
       "overwrite_earlier_deadline",
       "nudge_on_time_overflow",
       "dispatch_commit_phase",
       "disable_backpressure",
       "wrong_rebroadcast_label"
     }
  /\ checked = 0

Safety ==
  \A c \in Cases: Matches(c)

AdmissionGates ==
  /\ Matches("observer_rejected")
  /\ Matches("non_frontier_height_rejected")
  /\ Matches("zero_support_rejected")
  /\ Matches("exact_quorum_rejected")
  /\ Matches("over_quorum_rejected")
  /\ Matches("required_zero_rejected")

CooldownGates ==
  /\ Matches("cooldown_blocked")
  /\ Matches("cooldown_floor_blocks")

DispatchAndReturn ==
  /\ Matches("valid_rebroadcast_nudges")
  /\ Matches("backpressure_zero")

PacemakerDeadlineNudge ==
  /\ Matches("valid_rebroadcast_nudges")
  /\ Matches("valid_rebroadcast_deadline_earlier")
  /\ Matches("valid_rebroadcast_time_overflow")

DispatchMetadata ==
  Matches("valid_rebroadcast_nudges")

=============================================================================
====
