---- MODULE SumeragiPacemakerCoreGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the Sumeragi pacemaker state machine.

This slice captures `Pacemaker::{new,with_interval,set_interval,should_fire}`
and the test-only `deadline()` accessor. It abstracts `Instant`/`Duration`
arithmetic into representative boundary cases while preserving the observable
contract: construction stores the proposal interval and schedules the first
deadline at `now + interval`, `set_interval(...)` resets both the interval and
deadline from the update time, `should_fire(...)` is strict before the deadline
and inclusive at the deadline, successful fires advance the next deadline from
the current `now` using the current interval, calls before the next deadline do
not mutate it, zero intervals fire at `now` without inventing time movement, and
the deadline accessor is read-only.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NewPositiveInterval == "new_positive_interval"
NewZeroInterval == "new_zero_interval"
SetIntervalResetsDeadline == "set_interval_resets_deadline"
SetZeroIntervalResetsToNow == "set_zero_interval_resets_to_now"
BeforeDeadlineNoFire == "before_deadline_no_fire"
AtDeadlineFires == "at_deadline_fires"
AfterDeadlineFires == "after_deadline_fires"
AfterSetBeforeNewDeadlineNoFire == "after_set_before_new_deadline_no_fire"
AfterSetAtNewDeadlineFires == "after_set_at_new_deadline_fires"
FireAdvancesFromLateNow == "fire_advances_from_late_now"
RepeatedBeforeNextDeadlineNoFire == "repeated_before_next_deadline_no_fire"
ZeroIntervalFiresAtNow == "zero_interval_fires_at_now"
DeadlineAccessorReadOnly == "deadline_accessor_read_only"

Cases == {
  NewPositiveInterval,
  NewZeroInterval,
  SetIntervalResetsDeadline,
  SetZeroIntervalResetsToNow,
  BeforeDeadlineNoFire,
  AtDeadlineFires,
  AfterDeadlineFires,
  AfterSetBeforeNewDeadlineNoFire,
  AfterSetAtNewDeadlineFires,
  FireAdvancesFromLateNow,
  RepeatedBeforeNextDeadlineNoFire,
  ZeroIntervalFiresAtNow,
  DeadlineAccessorReadOnly
}

IntervalStored == 1
IntervalUpdated == 2
DeadlineNowPlusInterval == 3
DeadlineNow == 4
DeadlineResetFromSetNow == 5
DeadlineResetToSetNow == 6
FireFalse == 7
FireTrue == 8
DeadlineUnchanged == 9
DeadlineAdvancedFromNow == 10
DeadlineAdvancedFromOldDeadline == 11
UsesCurrentInterval == 12
UsesUpdatedInterval == 13
UsesOldInterval == 14
UsesZeroInterval == 15
StrictBeforeDeadline == 16
BoundaryInclusive == 17
DeadlineAccessorReturnsDeadline == 18
DeadlineNoMutation == 19
FirstFireTrue == 20
SecondFireFalse == 21
DeadlineAdvancedFromFirstNow == 22
DeadlineUnchangedAfterSecond == 23
DeadlineSameAsNow == 24
DeadlineSameAsSetNow == 25

Actions == 1..25

SpecActions(c) ==
  CASE c = NewPositiveInterval ->
      {IntervalStored, DeadlineNowPlusInterval,
       DeadlineAccessorReturnsDeadline}
    [] c = NewZeroInterval ->
      {IntervalStored, DeadlineNow, DeadlineAccessorReturnsDeadline}
    [] c = SetIntervalResetsDeadline ->
      {IntervalUpdated, DeadlineResetFromSetNow,
       DeadlineAccessorReturnsDeadline}
    [] c = SetZeroIntervalResetsToNow ->
      {IntervalUpdated, DeadlineResetToSetNow, DeadlineSameAsSetNow,
       DeadlineAccessorReturnsDeadline}
    [] c = BeforeDeadlineNoFire ->
      {FireFalse, DeadlineUnchanged, StrictBeforeDeadline}
    [] c = AtDeadlineFires ->
      {FireTrue, BoundaryInclusive, DeadlineAdvancedFromNow,
       UsesCurrentInterval}
    [] c = AfterDeadlineFires ->
      {FireTrue, DeadlineAdvancedFromNow, UsesCurrentInterval}
    [] c = AfterSetBeforeNewDeadlineNoFire ->
      {IntervalUpdated, DeadlineResetFromSetNow, FireFalse,
       DeadlineUnchanged, StrictBeforeDeadline}
    [] c = AfterSetAtNewDeadlineFires ->
      {IntervalUpdated, DeadlineResetFromSetNow, FireTrue,
       BoundaryInclusive, DeadlineAdvancedFromNow, UsesUpdatedInterval}
    [] c = FireAdvancesFromLateNow ->
      {FireTrue, DeadlineAdvancedFromNow, UsesCurrentInterval}
    [] c = RepeatedBeforeNextDeadlineNoFire ->
      {FirstFireTrue, DeadlineAdvancedFromFirstNow, SecondFireFalse,
       DeadlineUnchangedAfterSecond, StrictBeforeDeadline}
    [] c = ZeroIntervalFiresAtNow ->
      {FireTrue, UsesZeroInterval, DeadlineAdvancedFromNow, DeadlineSameAsNow,
       BoundaryInclusive}
    [] c = DeadlineAccessorReadOnly ->
      {DeadlineAccessorReturnsDeadline, DeadlineNoMutation}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "new_deadline_uses_now"
       /\ c = NewPositiveInterval ->
      (spec \ {DeadlineNowPlusInterval}) \cup {DeadlineNow}
    [] Bug = "new_drops_interval"
       /\ c = NewPositiveInterval ->
      spec \ {IntervalStored}
    [] Bug = "set_interval_keeps_old_interval"
       /\ c = SetIntervalResetsDeadline ->
      (spec \ {IntervalUpdated}) \cup {UsesOldInterval}
    [] Bug = "set_interval_keeps_old_deadline"
       /\ c = SetIntervalResetsDeadline ->
      (spec \ {DeadlineResetFromSetNow}) \cup {DeadlineUnchanged}
    [] Bug = "before_deadline_fires"
       /\ c = BeforeDeadlineNoFire ->
      (spec \ {FireFalse, StrictBeforeDeadline}) \cup {FireTrue}
    [] Bug = "before_deadline_advances"
       /\ c = BeforeDeadlineNoFire ->
      (spec \ {DeadlineUnchanged}) \cup {DeadlineAdvancedFromNow}
    [] Bug = "deadline_boundary_strict"
       /\ c = AtDeadlineFires ->
      (spec \ {FireTrue, BoundaryInclusive, DeadlineAdvancedFromNow})
        \cup {FireFalse, DeadlineUnchanged}
    [] Bug = "fire_keeps_deadline"
       /\ c \in {AtDeadlineFires, AfterDeadlineFires} ->
      (spec \ {DeadlineAdvancedFromNow}) \cup {DeadlineUnchanged}
    [] Bug = "fire_advances_from_old_deadline"
       /\ c = FireAdvancesFromLateNow ->
      (spec \ {DeadlineAdvancedFromNow}) \cup {DeadlineAdvancedFromOldDeadline}
    [] Bug = "fire_uses_old_interval"
       /\ c = AfterSetAtNewDeadlineFires ->
      (spec \ {UsesUpdatedInterval}) \cup {UsesOldInterval}
    [] Bug = "deadline_accessor_mutates"
       /\ c = DeadlineAccessorReadOnly ->
      (spec \ {DeadlineNoMutation}) \cup {DeadlineAdvancedFromNow}
    [] Bug = "zero_interval_suppressed"
       /\ c = ZeroIntervalFiresAtNow ->
      (spec \ {FireTrue, BoundaryInclusive}) \cup {FireFalse}
    [] Bug = "repeated_before_next_fires"
       /\ c = RepeatedBeforeNextDeadlineNoFire ->
      (spec \ {SecondFireFalse, DeadlineUnchangedAfterSecond})
        \cup {FireTrue, DeadlineAdvancedFromNow}
    [] OTHER -> spec

Bugs == {
  "none",
  "new_deadline_uses_now",
  "new_drops_interval",
  "set_interval_keeps_old_interval",
  "set_interval_keeps_old_deadline",
  "before_deadline_fires",
  "before_deadline_advances",
  "deadline_boundary_strict",
  "fire_keeps_deadline",
  "fire_advances_from_old_deadline",
  "fire_uses_old_interval",
  "deadline_accessor_mutates",
  "zero_interval_suppressed",
  "repeated_before_next_fires"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

PacemakerCoreDirectSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

NoBugInvariant == PacemakerCoreDirectSafety

SafetyFast == PacemakerCoreDirectSafety

BugNewDeadlineUsesNow == NoBugInvariant
BugNewDropsInterval == NoBugInvariant
BugSetIntervalKeepsOldInterval == NoBugInvariant
BugSetIntervalKeepsOldDeadline == NoBugInvariant
BugBeforeDeadlineFires == NoBugInvariant
BugBeforeDeadlineAdvances == NoBugInvariant
BugDeadlineBoundaryStrict == NoBugInvariant
BugFireKeepsDeadline == NoBugInvariant
BugFireAdvancesFromOldDeadline == NoBugInvariant
BugFireUsesOldInterval == NoBugInvariant
BugDeadlineAccessorMutates == NoBugInvariant
BugZeroIntervalSuppressed == NoBugInvariant
BugRepeatedBeforeNextFires == NoBugInvariant

====
