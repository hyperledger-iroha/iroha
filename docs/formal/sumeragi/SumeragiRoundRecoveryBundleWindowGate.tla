---- MODULE SumeragiRoundRecoveryBundleWindowGate ----
EXTENDS Integers, Sequences

(***************************************************************************
A bounded abstract model for the same-height round-recovery bundle window gate.

This slice captures:
- `RoundRecoveryBundleSource::{as_str,gate_class}`;
- `RoundRecoveryBundleGateClass::as_str`;
- `round_recovery_bundle_window_snapshot_with_window(...)`; and
- `try_reserve_round_recovery_bundle_window_with_window(...)`.

The concrete implementation stores one gate record per height and partitions
reservations into commit and non-commit classes. Commit reschedules can use a
short explicit resend window without rearming the non-commit recovery bundle
window, while non-commit sources remain single-use inside their shared default
window. Time is collapsed into representative same-window, boundary, and
next-window cases.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CommitClass == "Commit"
NonCommitClass == "NonCommit"
Classes == {CommitClass, NonCommitClass}

CommitQuorumReschedule == "CommitQuorumReschedule"
RosterProofFallback == "RosterProofFallback"
RangePullExpiry == "RangePullExpiry"
PayloadMismatchRecovery == "PayloadMismatchRecovery"
NoneSource == "none"

Sources == {
  CommitQuorumReschedule,
  RosterProofFallback,
  RangePullExpiry,
  PayloadMismatchRecovery
}

SpecClassLabel(cls) ==
  CASE cls = CommitClass -> "commit"
    [] cls = NonCommitClass -> "non_commit"
    [] OTHER -> "unknown"

ActualClassLabel(cls) ==
  CASE Bug = "commit_class_label_wrong" /\ cls = CommitClass -> "non_commit"
    [] Bug = "noncommit_class_label_wrong" /\ cls = NonCommitClass -> "commit"
    [] OTHER -> SpecClassLabel(cls)

SpecSourceLabel(source) ==
  CASE source = CommitQuorumReschedule -> "commit_quorum_reschedule"
    [] source = RosterProofFallback -> "roster_proof_fallback"
    [] source = RangePullExpiry -> "range_pull_expiry"
    [] source = PayloadMismatchRecovery -> "payload_mismatch_recovery"
    [] OTHER -> "unknown"

ActualSourceLabel(source) ==
  CASE Bug = "commit_source_label_wrong"
       /\ source = CommitQuorumReschedule -> "commit"
    [] Bug = "roster_source_label_wrong"
       /\ source = RosterProofFallback -> "roster"
    [] Bug = "range_source_label_wrong"
       /\ source = RangePullExpiry -> "range_pull"
    [] Bug = "payload_source_label_wrong"
       /\ source = PayloadMismatchRecovery -> "payload_mismatch"
    [] OTHER -> SpecSourceLabel(source)

SpecSourceClass(source) ==
  CASE source = CommitQuorumReschedule -> CommitClass
    [] source = RosterProofFallback -> NonCommitClass
    [] source = RangePullExpiry -> NonCommitClass
    [] source = PayloadMismatchRecovery -> NonCommitClass
    [] OTHER -> NonCommitClass

ActualSourceClass(source) ==
  CASE Bug = "commit_source_noncommit_class"
       /\ source = CommitQuorumReschedule -> NonCommitClass
    [] Bug = "payload_source_commit_class"
       /\ source = PayloadMismatchRecovery -> CommitClass
    [] Bug = "range_source_commit_class"
       /\ source = RangePullExpiry -> CommitClass
    [] Bug = "roster_source_commit_class"
       /\ source = RosterProofFallback -> CommitClass
    [] OTHER -> SpecSourceClass(source)

CommitThenNonCommitSameWindow == "commit_then_noncommit_same_window"
NonCommitThenOtherNonCommitSameWindow ==
  "noncommit_then_other_noncommit_same_window"
NonCommitNextDefaultWindow == "noncommit_next_default_window"
NonCommitBoundaryDefaultWindow == "noncommit_boundary_default_window"
NonCommitBeforeBoundary == "noncommit_before_boundary"
NonCommitThenCommitShortWindow == "noncommit_then_commit_short_window"
NonCommitThenCommitShortThenNonCommitBeforeDefault ==
  "noncommit_then_commit_short_then_noncommit_before_default"
CommitShortSameWindow == "commit_short_same_window"
CommitShortBoundary == "commit_short_boundary"
OtherHeightSameClass == "other_height_same_class"
ZeroWindowFloored == "zero_window_floored"

Scenarios == {
  CommitThenNonCommitSameWindow,
  NonCommitThenOtherNonCommitSameWindow,
  NonCommitNextDefaultWindow,
  NonCommitBoundaryDefaultWindow,
  NonCommitBeforeBoundary,
  NonCommitThenCommitShortWindow,
  NonCommitThenCommitShortThenNonCommitBeforeDefault,
  CommitShortSameWindow,
  CommitShortBoundary,
  OtherHeightSameClass,
  ZeroWindowFloored
}

\* Tuple fields:
\* <<first_allowed, second_allowed, third_allowed,
\*   commit_window_index, noncommit_window_index,
\*   last_commit_source, last_noncommit_source>>
SpecScenario(c) ==
  CASE c = CommitThenNonCommitSameWindow ->
      <<TRUE, TRUE, FALSE, 0, 0,
        CommitQuorumReschedule, PayloadMismatchRecovery>>
    [] c = NonCommitThenOtherNonCommitSameWindow ->
      <<TRUE, FALSE, FALSE, 0, 0, NoneSource, PayloadMismatchRecovery>>
    [] c = NonCommitNextDefaultWindow ->
      <<TRUE, TRUE, FALSE, 0, 1, NoneSource, RosterProofFallback>>
    [] c = NonCommitBoundaryDefaultWindow ->
      <<TRUE, TRUE, FALSE, 0, 1, NoneSource, RangePullExpiry>>
    [] c = NonCommitBeforeBoundary ->
      <<TRUE, FALSE, FALSE, 0, 0, NoneSource, PayloadMismatchRecovery>>
    [] c = NonCommitThenCommitShortWindow ->
      <<TRUE, TRUE, FALSE, 1, 0,
        CommitQuorumReschedule, PayloadMismatchRecovery>>
    [] c = NonCommitThenCommitShortThenNonCommitBeforeDefault ->
      <<TRUE, TRUE, FALSE, 1, 0,
        CommitQuorumReschedule, PayloadMismatchRecovery>>
    [] c = CommitShortSameWindow ->
      <<TRUE, FALSE, FALSE, 0, 0, CommitQuorumReschedule, NoneSource>>
    [] c = CommitShortBoundary ->
      <<TRUE, TRUE, FALSE, 1, 0, CommitQuorumReschedule, NoneSource>>
    [] c = OtherHeightSameClass ->
      <<TRUE, TRUE, FALSE, 0, 0, NoneSource, RangePullExpiry>>
    [] c = ZeroWindowFloored ->
      <<TRUE, TRUE, FALSE, 1, 0, CommitQuorumReschedule, NoneSource>>
    [] OTHER -> <<FALSE, FALSE, FALSE, 0, 0, NoneSource, NoneSource>>

ActualScenario(c) ==
  CASE Bug = "commit_noncommit_collide"
       /\ c = CommitThenNonCommitSameWindow ->
      <<TRUE, FALSE, FALSE, 0, 0, CommitQuorumReschedule, NoneSource>>
    [] Bug = "noncommit_sources_independent"
       /\ c = NonCommitThenOtherNonCommitSameWindow ->
      <<TRUE, TRUE, FALSE, 0, 0, NoneSource, RangePullExpiry>>
    [] Bug = "noncommit_boundary_blocked"
       /\ c = NonCommitBoundaryDefaultWindow ->
      <<TRUE, FALSE, FALSE, 0, 0, NoneSource, PayloadMismatchRecovery>>
    [] Bug = "noncommit_before_boundary_allowed"
       /\ c = NonCommitBeforeBoundary ->
      <<TRUE, TRUE, FALSE, 0, 0, NoneSource, RangePullExpiry>>
    [] Bug = "short_commit_rearms_noncommit"
       /\ c = NonCommitThenCommitShortThenNonCommitBeforeDefault ->
      <<TRUE, TRUE, TRUE, 1, 1, CommitQuorumReschedule, RangePullExpiry>>
    [] Bug = "commit_explicit_window_ignored"
       /\ c = CommitShortBoundary ->
      <<TRUE, FALSE, FALSE, 0, 0, CommitQuorumReschedule, NoneSource>>
    [] Bug = "commit_before_boundary_allowed"
       /\ c = CommitShortSameWindow ->
      <<TRUE, TRUE, FALSE, 0, 0, CommitQuorumReschedule, NoneSource>>
    [] Bug = "height_not_keyed"
       /\ c = OtherHeightSameClass ->
      <<TRUE, FALSE, FALSE, 0, 0, NoneSource, PayloadMismatchRecovery>>
    [] Bug = "zero_window_not_floored"
       /\ c = ZeroWindowFloored ->
      <<TRUE, FALSE, FALSE, 0, 0, CommitQuorumReschedule, NoneSource>>
    [] OTHER -> SpecScenario(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

ClassLabelsMatch ==
  \A cls \in Classes: ActualClassLabel(cls) = SpecClassLabel(cls)

SourceLabelsMatch ==
  \A source \in Sources: ActualSourceLabel(source) = SpecSourceLabel(source)

SourceClassesMatch ==
  \A source \in Sources: ActualSourceClass(source) = SpecSourceClass(source)

ScenarioOutcomesMatch ==
  \A c \in Scenarios: ActualScenario(c) = SpecScenario(c)

RoundRecoveryBundleWindowExactness ==
  /\ ClassLabelsMatch
  /\ SourceLabelsMatch
  /\ SourceClassesMatch
  /\ ScenarioOutcomesMatch

RoundRecoveryBundleWindowCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RoundRecoveryBundleWindowExactness

BugCommitClassLabelWrong ==
  ActualClassLabel(CommitClass) = SpecClassLabel(CommitClass)

BugNonCommitClassLabelWrong ==
  ActualClassLabel(NonCommitClass) = SpecClassLabel(NonCommitClass)

BugCommitSourceLabelWrong ==
  ActualSourceLabel(CommitQuorumReschedule) =
    SpecSourceLabel(CommitQuorumReschedule)

BugRosterSourceLabelWrong ==
  ActualSourceLabel(RosterProofFallback) = SpecSourceLabel(RosterProofFallback)

BugRangeSourceLabelWrong ==
  ActualSourceLabel(RangePullExpiry) = SpecSourceLabel(RangePullExpiry)

BugPayloadSourceLabelWrong ==
  ActualSourceLabel(PayloadMismatchRecovery) =
    SpecSourceLabel(PayloadMismatchRecovery)

BugCommitSourceNonCommitClass ==
  ActualSourceClass(CommitQuorumReschedule) =
    SpecSourceClass(CommitQuorumReschedule)

BugPayloadSourceCommitClass ==
  ActualSourceClass(PayloadMismatchRecovery) =
    SpecSourceClass(PayloadMismatchRecovery)

BugRangeSourceCommitClass ==
  ActualSourceClass(RangePullExpiry) = SpecSourceClass(RangePullExpiry)

BugRosterSourceCommitClass ==
  ActualSourceClass(RosterProofFallback) = SpecSourceClass(RosterProofFallback)

BugCommitNonCommitCollide ==
  ActualScenario(CommitThenNonCommitSameWindow) =
    SpecScenario(CommitThenNonCommitSameWindow)

BugNonCommitSourcesIndependent ==
  ActualScenario(NonCommitThenOtherNonCommitSameWindow) =
    SpecScenario(NonCommitThenOtherNonCommitSameWindow)

BugNonCommitBoundaryBlocked ==
  ActualScenario(NonCommitBoundaryDefaultWindow) =
    SpecScenario(NonCommitBoundaryDefaultWindow)

BugNonCommitBeforeBoundaryAllowed ==
  ActualScenario(NonCommitBeforeBoundary) = SpecScenario(NonCommitBeforeBoundary)

BugShortCommitRearmsNonCommit ==
  ActualScenario(NonCommitThenCommitShortThenNonCommitBeforeDefault) =
    SpecScenario(NonCommitThenCommitShortThenNonCommitBeforeDefault)

BugCommitExplicitWindowIgnored ==
  ActualScenario(CommitShortBoundary) = SpecScenario(CommitShortBoundary)

BugCommitBeforeBoundaryAllowed ==
  ActualScenario(CommitShortSameWindow) = SpecScenario(CommitShortSameWindow)

BugHeightNotKeyed ==
  ActualScenario(OtherHeightSameClass) = SpecScenario(OtherHeightSameClass)

BugZeroWindowNotFloored ==
  ActualScenario(ZeroWindowFloored) = SpecScenario(ZeroWindowFloored)

====
