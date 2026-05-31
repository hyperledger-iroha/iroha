---- MODULE SumeragiAutoscaleTransitionGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for autoscale-transition commit gating.

This slice models `autoscale_transition_committed_at(...)` and the success-path
caller in `Actor::apply_commit_outcome(...)`. Queue reconfiguration is allowed
only after a successful commit when Nexus autoscale is enabled and the
configured last transition height exactly equals the committed pending height.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "enabled_matching_success",
  "enabled_matching_failure",
  "disabled_matching_success",
  "enabled_previous_success",
  "enabled_next_success",
  "disabled_previous_failure"
}

Enabled(c) ==
  c \in {
    "enabled_matching_success",
    "enabled_matching_failure",
    "enabled_previous_success",
    "enabled_next_success"
  }

CommitSuccess(c) ==
  c \in {
    "enabled_matching_success",
    "disabled_matching_success",
    "enabled_previous_success",
    "enabled_next_success"
  }

CommittedHeight(c) == 10

LastTransitionHeight(c) ==
  CASE c \in {
         "enabled_matching_success",
         "enabled_matching_failure",
         "disabled_matching_success"
       } -> 10
    [] c \in {"enabled_previous_success", "disabled_previous_failure"} -> 9
    [] c = "enabled_next_success" -> 11
    [] OTHER -> 0

SpecHelperResult(c) ==
  Enabled(c) /\ LastTransitionHeight(c) = CommittedHeight(c)

ActualHelperResult(c) ==
  CASE Bug = "skip_matching_transition"
       /\ Enabled(c)
       /\ LastTransitionHeight(c) = CommittedHeight(c) -> FALSE
    [] Bug = "ignore_enabled"
       /\ ~Enabled(c)
       /\ LastTransitionHeight(c) = CommittedHeight(c) -> TRUE
    [] Bug = "ignore_height"
       /\ Enabled(c) -> TRUE
    [] Bug = "off_by_one_previous"
       /\ Enabled(c)
       /\ LastTransitionHeight(c) + 1 = CommittedHeight(c) -> TRUE
    [] Bug = "off_by_one_next"
       /\ Enabled(c)
       /\ LastTransitionHeight(c) = CommittedHeight(c) + 1 -> TRUE
    [] OTHER -> SpecHelperResult(c)

SpecQueueReconfigured(c) ==
  CommitSuccess(c) /\ SpecHelperResult(c)

ActualQueueReconfigured(c) ==
  CASE Bug = "skip_success_reconfigure"
       /\ CommitSuccess(c)
       /\ ActualHelperResult(c) -> FALSE
    [] Bug = "reconfigure_failed_commit"
       /\ ~CommitSuccess(c)
       /\ ActualHelperResult(c) -> TRUE
    [] Bug = "reconfigure_without_transition"
       /\ CommitSuccess(c)
       /\ ~ActualHelperResult(c) -> TRUE
    [] OTHER -> CommitSuccess(c) /\ ActualHelperResult(c)

SpecReportedHeight(c) ==
  IF SpecQueueReconfigured(c) THEN CommittedHeight(c) ELSE -1

ActualReportedHeight(c) ==
  IF ActualQueueReconfigured(c) THEN
    IF Bug = "wrong_reported_height" THEN CommittedHeight(c) + 1 ELSE CommittedHeight(c)
  ELSE -1

\* @type: Str => <<Bool, Bool, Int>>;
SpecCase(c) ==
  <<SpecHelperResult(c), SpecQueueReconfigured(c), SpecReportedHeight(c)>>

\* @type: Str => <<Bool, Bool, Int>>;
ActualCase(c) ==
  <<ActualHelperResult(c), ActualQueueReconfigured(c), ActualReportedHeight(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

SafetyFast ==
  \A c \in Cases: ActualCase(c) = SpecCase(c)

BugSkipMatchingTransition ==
  ActualCase("enabled_matching_success") = SpecCase("enabled_matching_success")

BugIgnoreEnabled ==
  ActualCase("disabled_matching_success") = SpecCase("disabled_matching_success")

BugIgnoreHeight ==
  ActualCase("enabled_previous_success") = SpecCase("enabled_previous_success")

BugOffByOnePrevious ==
  ActualCase("enabled_previous_success") = SpecCase("enabled_previous_success")

BugOffByOneNext ==
  ActualCase("enabled_next_success") = SpecCase("enabled_next_success")

BugSkipSuccessReconfigure ==
  ActualCase("enabled_matching_success") = SpecCase("enabled_matching_success")

BugReconfigureFailedCommit ==
  ActualCase("enabled_matching_failure") = SpecCase("enabled_matching_failure")

BugReconfigureWithoutTransition ==
  ActualCase("disabled_matching_success") = SpecCase("disabled_matching_success")

BugWrongReportedHeight ==
  ActualCase("enabled_matching_success") = SpecCase("enabled_matching_success")

====
