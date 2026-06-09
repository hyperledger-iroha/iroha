---- MODULE SumeragiQuorumRescheduleBackoffGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for quorum-reschedule backoff helpers in
`main_loop/reschedule.rs`.

This slice pins the deterministic arithmetic in
`adaptive_quorum_reschedule_backoff(...)` and the enable/disable contract for
`contiguous_frontier_vote_backed_fast_resend_window(...)`.
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
  "base_zero",
  "no_votes_no_stall",
  "one_missing_no_stall",
  "at_quorum_no_stall",
  "over_quorum_no_stall",
  "timeout_zero_huge_stall",
  "below_moderate_stall",
  "moderate_boundary_no_votes",
  "moderate_boundary_at_quorum",
  "severe_boundary_one_missing",
  "min_zero_boundary",
  "resend_zero_cooldown",
  "resend_nonzero_cooldown",
  "fast_enabled",
  "fast_enabled_zero_cooldown",
  "fast_not_contiguous",
  "fast_zero_votes",
  "fast_at_quorum",
  "fast_over_quorum",
  "fast_relay_backpressure",
  "fast_vote_queue_backlog",
  "fast_rbc_unresolved"
}

BaseZeroCases == {
  "base_zero"
}

DeficitMultiplierCases == {
  "no_votes_no_stall",
  "one_missing_no_stall",
  "at_quorum_no_stall",
  "over_quorum_no_stall",
  "min_zero_boundary"
}

StallEscalationCases == {
  "timeout_zero_huge_stall",
  "below_moderate_stall",
  "moderate_boundary_no_votes",
  "moderate_boundary_at_quorum",
  "severe_boundary_one_missing"
}

ResendWindowCases == {
  "resend_zero_cooldown",
  "resend_nonzero_cooldown",
  "fast_enabled",
  "fast_enabled_zero_cooldown"
}

FastPresentCases == {
  "fast_enabled",
  "fast_enabled_zero_cooldown"
}

FastRejectCases == {
  "fast_not_contiguous",
  "fast_zero_votes",
  "fast_at_quorum",
  "fast_over_quorum",
  "fast_relay_backpressure",
  "fast_vote_queue_backlog",
  "fast_rbc_unresolved"
}

Min(a, b) == IF a <= b THEN a ELSE b
Max(a, b) == IF a >= b THEN a ELSE b
BoolToInt(b) == IF b THEN 1 ELSE 0
SatSub(a, b) == IF a >= b THEN a - b ELSE 0

BaseBackoff(c) ==
  IF c = "base_zero" THEN 0 ELSE 100

QuorumTimeout(c) ==
  IF c = "timeout_zero_huge_stall" THEN 0 ELSE 50

StallAge(c) ==
  CASE c = "timeout_zero_huge_stall" -> 1000
    [] c = "below_moderate_stall" -> 99
    [] c \in {"moderate_boundary_no_votes", "moderate_boundary_at_quorum"} -> 100
    [] c = "severe_boundary_one_missing" -> 200
    [] OTHER -> 0

MinVotes(c) ==
  CASE c = "min_zero_boundary" -> 0
    [] c \in {"fast_at_quorum", "fast_over_quorum"} -> 3
    [] OTHER -> 3

VoteCount(c) ==
  CASE c \in {"no_votes_no_stall", "moderate_boundary_no_votes",
       "base_zero", "fast_zero_votes", "resend_zero_cooldown",
       "resend_nonzero_cooldown"} -> 0
    [] c \in {"one_missing_no_stall", "severe_boundary_one_missing",
       "fast_enabled", "fast_enabled_zero_cooldown",
       "fast_not_contiguous", "fast_relay_backpressure",
       "fast_vote_queue_backlog", "fast_rbc_unresolved"} -> 2
    [] c \in {"at_quorum_no_stall", "moderate_boundary_at_quorum",
       "fast_at_quorum"} -> 3
    [] c = "over_quorum_no_stall" -> 5
    [] c = "fast_over_quorum" -> 4
    [] OTHER -> 0

RebroadcastCooldown(c) ==
  IF c \in {"resend_zero_cooldown", "fast_enabled_zero_cooldown"} THEN 0
  ELSE 25

ContiguousFrontier(c) ==
  c # "fast_not_contiguous"

RelayBackpressure(c) ==
  c = "fast_relay_backpressure"

VoteQueueBacklog(c) ==
  c = "fast_vote_queue_backlog"

RbcUnresolved(c) ==
  c = "fast_rbc_unresolved"

SpecInitialMultiplier(c) ==
  LET deficit == SatSub(MinVotes(c), VoteCount(c)) IN
  IF deficit >= SatSub(MinVotes(c), 1) THEN 3
  ELSE IF deficit > 0 THEN 2
  ELSE 1

SpecEscalated(c) ==
  QuorumTimeout(c) # 0 /\ StallAge(c) >= QuorumTimeout(c) * 2

SpecMultiplier(c) ==
  IF BaseBackoff(c) = 0 THEN 0
  ELSE IF QuorumTimeout(c) # 0 /\ StallAge(c) >= QuorumTimeout(c) * 4 THEN
    Max(SpecInitialMultiplier(c), 5)
  ELSE IF QuorumTimeout(c) # 0 /\ StallAge(c) >= QuorumTimeout(c) * 2 THEN
    Max(SpecInitialMultiplier(c), 4)
  ELSE SpecInitialMultiplier(c)

SpecBackoff(c) ==
  IF BaseBackoff(c) = 0 THEN 0 ELSE BaseBackoff(c) * SpecMultiplier(c)

SpecResendWindow(c) ==
  Max(RebroadcastCooldown(c), 1)

SpecFastPresent(c) ==
  ContiguousFrontier(c)
    /\ VoteCount(c) # 0
    /\ VoteCount(c) < MinVotes(c)
    /\ ~RelayBackpressure(c)
    /\ ~VoteQueueBacklog(c)
    /\ ~RbcUnresolved(c)

SpecFastWindow(c) ==
  IF SpecFastPresent(c) THEN SpecResendWindow(c) ELSE 0

\* @type: (Str) => <<Int, Int, Int, Int>>;
SpecOutput(c) ==
  <<SpecBackoff(c), BoolToInt(SpecEscalated(c)),
    BoolToInt(SpecFastPresent(c)), SpecFastWindow(c)>>

ActualInitialMultiplier(c) ==
  CASE Bug = "deficit_no_votes_uses_one" /\ c = "no_votes_no_stall" -> 1
    [] Bug = "deficit_one_missing_uses_three" /\
       c = "one_missing_no_stall" -> 3
    [] Bug = "at_quorum_uses_two" /\ c = "at_quorum_no_stall" -> 2
    [] Bug = "min_zero_uses_one" /\ c = "min_zero_boundary" -> 1
    [] OTHER -> SpecInitialMultiplier(c)

ActualEscalated(c) ==
  CASE Bug = "base_zero_escalates" /\ c = "base_zero" -> TRUE
    [] Bug = "timeout_zero_escalates" /\ c = "timeout_zero_huge_stall" -> TRUE
    [] Bug = "moderate_boundary_not_escalated" /\
       c = "moderate_boundary_no_votes" -> FALSE
    [] Bug = "severe_not_escalated" /\ c = "severe_boundary_one_missing" ->
       FALSE
    [] OTHER -> SpecEscalated(c)

ActualMultiplier(c) ==
  CASE Bug = "base_zero_multiplied" /\ c = "base_zero" -> 3
    [] Bug = "timeout_zero_escalates" /\ c = "timeout_zero_huge_stall" -> 5
    [] Bug = "moderate_boundary_not_escalated" /\
       c = "moderate_boundary_no_votes" -> ActualInitialMultiplier(c)
    [] Bug = "moderate_uses_three" /\
       c = "moderate_boundary_at_quorum" -> 3
    [] Bug = "severe_boundary_uses_moderate" /\
       c = "severe_boundary_one_missing" -> 4
    [] Bug = "severe_not_escalated" /\ c = "severe_boundary_one_missing" ->
       ActualInitialMultiplier(c)
    [] OTHER ->
       IF BaseBackoff(c) = 0 THEN 0
       ELSE IF QuorumTimeout(c) # 0 /\ StallAge(c) >= QuorumTimeout(c) * 4 THEN
         Max(ActualInitialMultiplier(c), 5)
       ELSE IF QuorumTimeout(c) # 0 /\ StallAge(c) >= QuorumTimeout(c) * 2 THEN
         Max(ActualInitialMultiplier(c), 4)
       ELSE ActualInitialMultiplier(c)

ActualBackoff(c) ==
  IF Bug = "base_zero_multiplied" /\ c = "base_zero" THEN 300
  ELSE IF BaseBackoff(c) = 0 THEN 0
  ELSE BaseBackoff(c) * ActualMultiplier(c)

ActualResendWindow(c) ==
  CASE Bug = "resend_zero_returns_zero" /\ c = "fast_enabled_zero_cooldown" ->
       0
    [] Bug = "fast_window_uses_double" /\ c = "fast_enabled" ->
       RebroadcastCooldown(c) * 2
    [] OTHER -> SpecResendWindow(c)

ActualFastPresent(c) ==
  CASE Bug = "fast_ignores_contiguous" /\ c = "fast_not_contiguous" -> TRUE
    [] Bug = "fast_allows_zero_votes" /\ c = "fast_zero_votes" -> TRUE
    [] Bug = "fast_allows_at_quorum" /\ c = "fast_at_quorum" -> TRUE
    [] Bug = "fast_allows_over_quorum" /\ c = "fast_over_quorum" -> TRUE
    [] Bug = "fast_ignores_relay" /\ c = "fast_relay_backpressure" -> TRUE
    [] Bug = "fast_ignores_queue" /\ c = "fast_vote_queue_backlog" -> TRUE
    [] Bug = "fast_ignores_rbc" /\ c = "fast_rbc_unresolved" -> TRUE
    [] OTHER -> SpecFastPresent(c)

ActualFastWindow(c) ==
  IF ActualFastPresent(c) THEN ActualResendWindow(c) ELSE 0

\* @type: (Str) => <<Int, Int, Int, Int>>;
ActualOutput(c) ==
  <<ActualBackoff(c), BoolToInt(ActualEscalated(c)),
    BoolToInt(ActualFastPresent(c)), ActualFastWindow(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "base_zero_escalates",
       "base_zero_multiplied",
       "deficit_no_votes_uses_one",
       "deficit_one_missing_uses_three",
       "at_quorum_uses_two",
       "min_zero_uses_one",
       "timeout_zero_escalates",
       "moderate_boundary_not_escalated",
       "moderate_uses_three",
       "severe_boundary_uses_moderate",
       "severe_not_escalated",
       "resend_zero_returns_zero",
       "fast_window_uses_double",
       "fast_ignores_contiguous",
       "fast_allows_zero_votes",
       "fast_allows_at_quorum",
       "fast_allows_over_quorum",
       "fast_ignores_relay",
       "fast_ignores_queue",
       "fast_ignores_rbc"
     }
  /\ checked = 0

QuorumRescheduleBackoffCoreSafety ==
  /\ ActualOutput("base_zero") = SpecOutput("base_zero")
  /\ ActualOutput("no_votes_no_stall") = SpecOutput("no_votes_no_stall")
  /\ ActualOutput("one_missing_no_stall") = SpecOutput("one_missing_no_stall")
  /\ ActualOutput("at_quorum_no_stall") = SpecOutput("at_quorum_no_stall")
  /\ ActualOutput("over_quorum_no_stall") = SpecOutput("over_quorum_no_stall")
  /\ ActualOutput("timeout_zero_huge_stall") =
       SpecOutput("timeout_zero_huge_stall")
  /\ ActualOutput("below_moderate_stall") =
       SpecOutput("below_moderate_stall")
  /\ ActualOutput("moderate_boundary_no_votes") =
       SpecOutput("moderate_boundary_no_votes")
  /\ ActualOutput("moderate_boundary_at_quorum") =
       SpecOutput("moderate_boundary_at_quorum")
  /\ ActualOutput("severe_boundary_one_missing") =
       SpecOutput("severe_boundary_one_missing")
  /\ ActualOutput("min_zero_boundary") = SpecOutput("min_zero_boundary")
  /\ ActualOutput("resend_zero_cooldown") =
       SpecOutput("resend_zero_cooldown")
  /\ ActualOutput("resend_nonzero_cooldown") =
       SpecOutput("resend_nonzero_cooldown")
  /\ ActualOutput("fast_enabled") = SpecOutput("fast_enabled")
  /\ ActualOutput("fast_enabled_zero_cooldown") =
       SpecOutput("fast_enabled_zero_cooldown")
  /\ ActualOutput("fast_not_contiguous") = SpecOutput("fast_not_contiguous")
  /\ ActualOutput("fast_zero_votes") = SpecOutput("fast_zero_votes")
  /\ ActualOutput("fast_at_quorum") = SpecOutput("fast_at_quorum")
  /\ ActualOutput("fast_over_quorum") = SpecOutput("fast_over_quorum")
  /\ ActualOutput("fast_relay_backpressure") =
       SpecOutput("fast_relay_backpressure")
  /\ ActualOutput("fast_vote_queue_backlog") =
       SpecOutput("fast_vote_queue_backlog")
  /\ ActualOutput("fast_rbc_unresolved") = SpecOutput("fast_rbc_unresolved")

SafetyFast ==
  QuorumRescheduleBackoffCoreSafety

QuorumBackoffBaseZeroExact ==
  \A c \in BaseZeroCases:
    /\ ActualBackoff(c) = 0
    /\ ActualMultiplier(c) = 0
    /\ ActualEscalated(c) = SpecEscalated(c)
    /\ ActualOutput(c) = SpecOutput(c)

QuorumBackoffDeficitMultiplierExact ==
  \A c \in DeficitMultiplierCases:
    /\ ActualInitialMultiplier(c) = SpecInitialMultiplier(c)
    /\ ActualMultiplier(c) = SpecMultiplier(c)
    /\ ActualBackoff(c) = SpecBackoff(c)
    /\ ActualOutput(c) = SpecOutput(c)

QuorumBackoffStallEscalationExact ==
  \A c \in StallEscalationCases:
    /\ ActualEscalated(c) = SpecEscalated(c)
    /\ ActualMultiplier(c) = SpecMultiplier(c)
    /\ ActualBackoff(c) = SpecBackoff(c)
    /\ IF QuorumTimeout(c) = 0 THEN ~ActualEscalated(c) ELSE TRUE
    /\ IF QuorumTimeout(c) # 0 /\ StallAge(c) >= QuorumTimeout(c) * 4
       THEN ActualMultiplier(c) >= 5
       ELSE TRUE
    /\ IF QuorumTimeout(c) # 0 /\ StallAge(c) >= QuorumTimeout(c) * 2
       THEN ActualMultiplier(c) >= 4
       ELSE TRUE
    /\ ActualOutput(c) = SpecOutput(c)

QuorumResendWindowExact ==
  \A c \in ResendWindowCases:
    /\ ActualResendWindow(c) = SpecResendWindow(c)
    /\ ActualResendWindow(c) >= 1
    /\ ActualOutput(c) = SpecOutput(c)

QuorumFastResendPresentExact ==
  \A c \in FastPresentCases:
    /\ ActualFastPresent(c) = TRUE
    /\ ActualFastWindow(c) = SpecResendWindow(c)
    /\ ActualFastWindow(c) >= 1
    /\ ActualOutput(c) = SpecOutput(c)

QuorumFastResendRejectExact ==
  \A c \in FastRejectCases:
    /\ ActualFastPresent(c) = FALSE
    /\ ActualFastWindow(c) = 0
    /\ ActualOutput(c) = SpecOutput(c)

QuorumRescheduleBackoffExactness ==
  /\ QuorumRescheduleBackoffCoreSafety
  /\ QuorumBackoffBaseZeroExact
  /\ QuorumBackoffDeficitMultiplierExact
  /\ QuorumBackoffStallEscalationExact
  /\ QuorumResendWindowExact
  /\ QuorumFastResendPresentExact
  /\ QuorumFastResendRejectExact

BugBaseZeroEscalates ==
  ActualOutput("base_zero") = SpecOutput("base_zero")

BugBaseZeroMultiplied ==
  ActualOutput("base_zero") = SpecOutput("base_zero")

BugDeficitNoVotesUsesOne ==
  ActualOutput("no_votes_no_stall") = SpecOutput("no_votes_no_stall")

BugDeficitOneMissingUsesThree ==
  ActualOutput("one_missing_no_stall") = SpecOutput("one_missing_no_stall")

BugAtQuorumUsesTwo ==
  ActualOutput("at_quorum_no_stall") = SpecOutput("at_quorum_no_stall")

BugMinZeroUsesOne ==
  ActualOutput("min_zero_boundary") = SpecOutput("min_zero_boundary")

BugTimeoutZeroEscalates ==
  ActualOutput("timeout_zero_huge_stall") =
    SpecOutput("timeout_zero_huge_stall")

BugModerateBoundaryNotEscalated ==
  ActualOutput("moderate_boundary_no_votes") =
    SpecOutput("moderate_boundary_no_votes")

BugModerateUsesThree ==
  ActualOutput("moderate_boundary_at_quorum") =
    SpecOutput("moderate_boundary_at_quorum")

BugSevereBoundaryUsesModerate ==
  ActualOutput("severe_boundary_one_missing") =
    SpecOutput("severe_boundary_one_missing")

BugSevereNotEscalated ==
  ActualOutput("severe_boundary_one_missing") =
    SpecOutput("severe_boundary_one_missing")

BugResendZeroReturnsZero ==
  ActualOutput("fast_enabled_zero_cooldown") =
    SpecOutput("fast_enabled_zero_cooldown")

BugFastWindowUsesDouble ==
  ActualOutput("fast_enabled") = SpecOutput("fast_enabled")

BugFastIgnoresContiguous ==
  ActualOutput("fast_not_contiguous") = SpecOutput("fast_not_contiguous")

BugFastAllowsZeroVotes ==
  ActualOutput("fast_zero_votes") = SpecOutput("fast_zero_votes")

BugFastAllowsAtQuorum ==
  ActualOutput("fast_at_quorum") = SpecOutput("fast_at_quorum")

BugFastAllowsOverQuorum ==
  ActualOutput("fast_over_quorum") = SpecOutput("fast_over_quorum")

BugFastIgnoresRelay ==
  ActualOutput("fast_relay_backpressure") =
    SpecOutput("fast_relay_backpressure")

BugFastIgnoresQueue ==
  ActualOutput("fast_vote_queue_backlog") =
    SpecOutput("fast_vote_queue_backlog")

BugFastIgnoresRbc ==
  ActualOutput("fast_rbc_unresolved") = SpecOutput("fast_rbc_unresolved")

Safety ==
  QuorumRescheduleBackoffCoreSafety

=============================================================================
====
