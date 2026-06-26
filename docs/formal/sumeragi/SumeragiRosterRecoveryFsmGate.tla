---- MODULE SumeragiRosterRecoveryFsmGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the roster-unavailability recovery FSM.

This slice pins `step_roster_recovery_state(...)` and the bookkeeping contract
in `transition_roster_recovery_state(...)`:
- Steady enters ReelectRoster only on RosterUnavailable.
- ReelectRoster rotates on CandidatesAvailable, waits on CandidatesEmpty, and
  returns to Steady on RoundAdvanced.
- WaitCandidates re-enters ReelectRoster on CandidatesAvailable and returns to
  Steady on RoundAdvanced.
- RotateView is terminal for one tick: every event returns it to Steady.
- Only actual state changes report `true`, record dwell time for the previous
  state, reset `state_entered_at`, and count a transition to the next state.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Steady == 1
ReelectRoster == 2
WaitCandidates == 3
RotateView == 4

States == {Steady, ReelectRoster, WaitCandidates, RotateView}

SteadyRosterUnavailable == "steady_roster_unavailable"
SteadyCandidatesAvailable == "steady_candidates_available"
ReelectCandidatesAvailable == "reelect_candidates_available"
ReelectCandidatesEmpty == "reelect_candidates_empty"
ReelectRoundAdvanced == "reelect_round_advanced"
ReelectRosterUnavailable == "reelect_roster_unavailable"
WaitCandidatesAvailable == "wait_candidates_available"
WaitRoundAdvanced == "wait_round_advanced"
WaitCandidatesEmpty == "wait_candidates_empty"
WaitRosterUnavailable == "wait_roster_unavailable"
RotateRosterUnavailable == "rotate_roster_unavailable"
RotateCandidatesAvailable == "rotate_candidates_available"
RotateCandidatesEmpty == "rotate_candidates_empty"
RotateRoundAdvanced == "rotate_round_advanced"

Cases == {
  SteadyRosterUnavailable,
  SteadyCandidatesAvailable,
  ReelectCandidatesAvailable,
  ReelectCandidatesEmpty,
  ReelectRoundAdvanced,
  ReelectRosterUnavailable,
  WaitCandidatesAvailable,
  WaitRoundAdvanced,
  WaitCandidatesEmpty,
  WaitRosterUnavailable,
  RotateRosterUnavailable,
  RotateCandidatesAvailable,
  RotateCandidatesEmpty,
  RotateRoundAdvanced
}

CaseState(c) ==
  CASE c \in {SteadyRosterUnavailable, SteadyCandidatesAvailable} -> Steady
    [] c \in {ReelectCandidatesAvailable, ReelectCandidatesEmpty,
              ReelectRoundAdvanced, ReelectRosterUnavailable} ->
      ReelectRoster
    [] c \in {WaitCandidatesAvailable, WaitRoundAdvanced,
              WaitCandidatesEmpty, WaitRosterUnavailable} ->
      WaitCandidates
    [] OTHER -> RotateView

SpecNextState(c) ==
  CASE c = SteadyRosterUnavailable -> ReelectRoster
    [] c = ReelectCandidatesAvailable -> RotateView
    [] c = ReelectCandidatesEmpty -> WaitCandidates
    [] c = ReelectRoundAdvanced -> Steady
    [] c = WaitCandidatesAvailable -> ReelectRoster
    [] c = WaitRoundAdvanced -> Steady
    [] c \in {RotateRosterUnavailable, RotateCandidatesAvailable,
              RotateCandidatesEmpty, RotateRoundAdvanced} ->
      Steady
    [] OTHER -> CaseState(c)

SpecChanged(c) ==
  SpecNextState(c) # CaseState(c)

NextSteadyAction == 1
NextReelectAction == 2
NextWaitAction == 3
NextRotateAction == 4
ReturnsChanged == 5
ReturnsUnchanged == 6
DwellRecorded == 7
EnteredAtReset == 8
TransitionCountNext == 9
NoDwellRecorded == 10
NoEnteredAtReset == 11
NoTransitionCount == 12
TransitionCountCurrent == 13

ActionUniverse == 1..13

NextAction(state) ==
  CASE state = Steady -> NextSteadyAction
    [] state = ReelectRoster -> NextReelectAction
    [] state = WaitCandidates -> NextWaitAction
    [] state = RotateView -> NextRotateAction

BuildActions(next, changed, count_current) ==
  {NextAction(next)}
    \cup
      IF changed THEN
        {ReturnsChanged, DwellRecorded, EnteredAtReset}
          \cup {IF count_current THEN TransitionCountCurrent
                ELSE TransitionCountNext}
      ELSE
        {ReturnsUnchanged, NoDwellRecorded, NoEnteredAtReset,
         NoTransitionCount}

SpecActions(c) ==
  BuildActions(SpecNextState(c), SpecChanged(c), FALSE)

ImplementationActions(c) ==
  CASE Bug = "steady_roster_unavailable_stays_steady"
       /\ c = SteadyRosterUnavailable ->
      BuildActions(Steady, FALSE, FALSE)
    [] Bug = "steady_candidates_available_rotates"
       /\ c = SteadyCandidatesAvailable ->
      BuildActions(RotateView, TRUE, FALSE)
    [] Bug = "reelect_candidates_available_stays"
       /\ c = ReelectCandidatesAvailable ->
      BuildActions(ReelectRoster, FALSE, FALSE)
    [] Bug = "reelect_candidates_empty_stays"
       /\ c = ReelectCandidatesEmpty ->
      BuildActions(ReelectRoster, FALSE, FALSE)
    [] Bug = "reelect_round_advanced_stays"
       /\ c = ReelectRoundAdvanced ->
      BuildActions(ReelectRoster, FALSE, FALSE)
    [] Bug = "reelect_roster_unavailable_resets"
       /\ c = ReelectRosterUnavailable ->
      BuildActions(Steady, TRUE, FALSE)
    [] Bug = "wait_candidates_available_stays"
       /\ c = WaitCandidatesAvailable ->
      BuildActions(WaitCandidates, FALSE, FALSE)
    [] Bug = "wait_round_advanced_stays"
       /\ c = WaitRoundAdvanced ->
      BuildActions(WaitCandidates, FALSE, FALSE)
    [] Bug = "wait_candidates_empty_reelects"
       /\ c = WaitCandidatesEmpty ->
      BuildActions(ReelectRoster, TRUE, FALSE)
    [] Bug = "wait_roster_unavailable_reelects"
       /\ c = WaitRosterUnavailable ->
      BuildActions(ReelectRoster, TRUE, FALSE)
    [] Bug = "rotate_roster_unavailable_reelects"
       /\ c = RotateRosterUnavailable ->
      BuildActions(ReelectRoster, TRUE, FALSE)
    [] Bug = "rotate_candidates_empty_waits"
       /\ c = RotateCandidatesEmpty ->
      BuildActions(WaitCandidates, TRUE, FALSE)
    [] Bug = "no_change_reports_change"
       /\ c = SteadyCandidatesAvailable ->
      BuildActions(Steady, TRUE, FALSE)
    [] Bug = "change_reports_unchanged"
       /\ c = SteadyRosterUnavailable ->
      BuildActions(ReelectRoster, FALSE, FALSE)
    [] Bug = "skip_dwell_record"
       /\ c = ReelectCandidatesAvailable ->
      SpecActions(c) \ {DwellRecorded}
    [] Bug = "skip_entered_at_reset"
       /\ c = ReelectCandidatesAvailable ->
      SpecActions(c) \ {EnteredAtReset}
    [] Bug = "transition_counts_current"
       /\ c = ReelectCandidatesAvailable ->
      BuildActions(RotateView, TRUE, TRUE)
    [] OTHER -> SpecActions(c)

Bugs == {
  "none",
  "steady_roster_unavailable_stays_steady",
  "steady_candidates_available_rotates",
  "reelect_candidates_available_stays",
  "reelect_candidates_empty_stays",
  "reelect_round_advanced_stays",
  "reelect_roster_unavailable_resets",
  "wait_candidates_available_stays",
  "wait_round_advanced_stays",
  "wait_candidates_empty_reelects",
  "wait_roster_unavailable_reelects",
  "rotate_roster_unavailable_reelects",
  "rotate_candidates_empty_waits",
  "no_change_reports_change",
  "change_reports_unchanged",
  "skip_dwell_record",
  "skip_entered_at_reset",
  "transition_counts_current"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ CaseState(c) \in States
       /\ SpecNextState(c) \in States
       /\ SpecChanged(c) \in BOOLEAN
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

SteadyTransitionsOnlyOnRosterUnavailable ==
  /\ NextReelectAction \in ImplementationActions(SteadyRosterUnavailable)
  /\ NextSteadyAction \in ImplementationActions(SteadyCandidatesAvailable)

ReelectTransitionsMatchCandidateAndRoundEvents ==
  /\ NextRotateAction \in ImplementationActions(ReelectCandidatesAvailable)
  /\ NextWaitAction \in ImplementationActions(ReelectCandidatesEmpty)
  /\ NextSteadyAction \in ImplementationActions(ReelectRoundAdvanced)
  /\ NextReelectAction \in ImplementationActions(ReelectRosterUnavailable)

WaitCandidatesTransitionsMatchCandidateAndRoundEvents ==
  /\ NextReelectAction \in ImplementationActions(WaitCandidatesAvailable)
  /\ NextSteadyAction \in ImplementationActions(WaitRoundAdvanced)
  /\ NextWaitAction \in ImplementationActions(WaitCandidatesEmpty)
  /\ NextWaitAction \in ImplementationActions(WaitRosterUnavailable)

RotateViewAlwaysReturnsToSteady ==
  \A c \in {RotateRosterUnavailable, RotateCandidatesAvailable,
            RotateCandidatesEmpty, RotateRoundAdvanced}:
    NextSteadyAction \in ImplementationActions(c)

ChangedTransitionsRecordBookkeepingForNextState ==
  \A c \in Cases:
    SpecChanged(c) =>
      /\ ReturnsChanged \in ImplementationActions(c)
      /\ DwellRecorded \in ImplementationActions(c)
      /\ EnteredAtReset \in ImplementationActions(c)
      /\ TransitionCountNext \in ImplementationActions(c)
      /\ ~(TransitionCountCurrent \in ImplementationActions(c))

UnchangedTransitionsDoNotRecordBookkeeping ==
  \A c \in Cases:
    ~SpecChanged(c) =>
      /\ ReturnsUnchanged \in ImplementationActions(c)
      /\ NoDwellRecorded \in ImplementationActions(c)
      /\ NoEnteredAtReset \in ImplementationActions(c)
      /\ NoTransitionCount \in ImplementationActions(c)
      /\ ~(ReturnsChanged \in ImplementationActions(c))

OneNextStateAction ==
  \A c \in Cases:
    LET nexts ==
      {NextSteadyAction, NextReelectAction, NextWaitAction, NextRotateAction}
        \cap ImplementationActions(c)
    IN nexts \in {
      {NextSteadyAction},
      {NextReelectAction},
      {NextWaitAction},
      {NextRotateAction}
    }

NextStateActionMatchesSpec ==
  \A c \in Cases:
    NextAction(SpecNextState(c)) \in ImplementationActions(c)

ChangeReturnMatchesSpec ==
  \A c \in Cases:
    /\ (ReturnsChanged \in ImplementationActions(c)) = SpecChanged(c)
    /\ (ReturnsUnchanged \in ImplementationActions(c)) = ~SpecChanged(c)
    /\ ~(
         ReturnsChanged \in ImplementationActions(c)
           /\ ReturnsUnchanged \in ImplementationActions(c)
       )

ChangedBookkeepingExclusive ==
  \A c \in Cases:
    SpecChanged(c) =>
      /\ NoDwellRecorded \notin ImplementationActions(c)
      /\ NoEnteredAtReset \notin ImplementationActions(c)
      /\ NoTransitionCount \notin ImplementationActions(c)

UnchangedBookkeepingExclusive ==
  \A c \in Cases:
    ~SpecChanged(c) =>
      /\ DwellRecorded \notin ImplementationActions(c)
      /\ EnteredAtReset \notin ImplementationActions(c)
      /\ TransitionCountNext \notin ImplementationActions(c)
      /\ TransitionCountCurrent \notin ImplementationActions(c)

TerminalRotateAnchors ==
  /\ NextSteadyAction \in ImplementationActions(RotateRosterUnavailable)
  /\ NextSteadyAction \in ImplementationActions(RotateCandidatesAvailable)
  /\ NextSteadyAction \in ImplementationActions(RotateCandidatesEmpty)
  /\ NextSteadyAction \in ImplementationActions(RotateRoundAdvanced)
  /\ ReturnsChanged \in ImplementationActions(RotateCandidatesAvailable)
  /\ DwellRecorded \in ImplementationActions(RotateCandidatesEmpty)

RosterRecoveryFsmCoreSafety ==
  /\ ActionsMatchSpec
  /\ SteadyTransitionsOnlyOnRosterUnavailable
  /\ ReelectTransitionsMatchCandidateAndRoundEvents
  /\ WaitCandidatesTransitionsMatchCandidateAndRoundEvents
  /\ RotateViewAlwaysReturnsToSteady
  /\ ChangedTransitionsRecordBookkeepingForNextState
  /\ UnchangedTransitionsDoNotRecordBookkeeping
  /\ OneNextStateAction
  /\ NextStateActionMatchesSpec
  /\ ChangeReturnMatchesSpec
  /\ ChangedBookkeepingExclusive
  /\ UnchangedBookkeepingExclusive
  /\ TerminalRotateAnchors

NoBugInvariant == RosterRecoveryFsmCoreSafety

SafetyFast == RosterRecoveryFsmCoreSafety

RosterRecoveryFsmCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RosterRecoveryFsmCoreSafety

====
