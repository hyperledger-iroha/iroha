---- MODULE SumeragiEngineViewAdvanceSaturationGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for pure-engine view advancement saturation.

`ConsensusEngine::on_tick(...)` and the invalid branch of
`ConsensusEngine::on_validation_result(...)` advance the view with
`saturating_add(1)`. This slice proves the boundary property that ordinary
views advance by exactly one, the maximum view never wraps to zero, and emitted
`NewView`/`AdvanceView` outputs bind the same saturated round that is stored in
engine state. Valid, stale, wrong-block, and no-in-flight validation callbacks
must not advance the view.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugTickWrapAtMax,
  \* @type: Bool;
  BugTickStaysBeforeMax,
  \* @type: Bool;
  BugInvalidWrapAtMax,
  \* @type: Bool;
  BugInvalidStaysBeforeMax,
  \* @type: Bool;
  BugValidAdvances,
  \* @type: Bool;
  BugStaleValidationAdvances,
  \* @type: Bool;
  BugNoInflightAdvances,
  \* @type: Bool;
  BugOutputUsesOldView

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

MaxView == 3

Cases == {
  "tick_mid",
  "tick_max",
  "invalid_mid",
  "invalid_max",
  "valid_current",
  "wrong_round_invalid",
  "wrong_block_invalid",
  "no_inflight_invalid"
}

CurrentView(candidate) ==
  CASE candidate \in {"tick_max", "invalid_max"} -> MaxView
    [] OTHER -> 2

IsTick(candidate) ==
  candidate \in {"tick_mid", "tick_max"}

IsCurrentInvalidValidation(candidate) ==
  candidate \in {"invalid_mid", "invalid_max"}

IsValidValidation(candidate) ==
  candidate = "valid_current"

IsStaleOrWrongValidation(candidate) ==
  candidate \in {"wrong_round_invalid", "wrong_block_invalid"}

IsNoInflightValidation(candidate) ==
  candidate = "no_inflight_invalid"

SaturatingNext(view) ==
  IF view = MaxView THEN MaxView ELSE view + 1

SpecAdvances(candidate) ==
  IsTick(candidate) \/ IsCurrentInvalidValidation(candidate)

SpecNextView(candidate) ==
  IF SpecAdvances(candidate)
  THEN SaturatingNext(CurrentView(candidate))
  ELSE CurrentView(candidate)

ImplementationAdvances(candidate) ==
  IF SpecAdvances(candidate)
  THEN TRUE
  ELSE
    \/ /\ IsValidValidation(candidate)
       /\ BugValidAdvances
    \/ /\ IsStaleOrWrongValidation(candidate)
       /\ BugStaleValidationAdvances
    \/ /\ IsNoInflightValidation(candidate)
       /\ BugNoInflightAdvances

BuggySaturatingNext(candidate) ==
  IF IsTick(candidate)
  THEN
    IF CurrentView(candidate) = MaxView /\ BugTickWrapAtMax
    THEN 0
    ELSE IF CurrentView(candidate) < MaxView /\ BugTickStaysBeforeMax
    THEN CurrentView(candidate)
    ELSE SaturatingNext(CurrentView(candidate))
  ELSE IF IsCurrentInvalidValidation(candidate)
  THEN
    IF CurrentView(candidate) = MaxView /\ BugInvalidWrapAtMax
    THEN 0
    ELSE IF CurrentView(candidate) < MaxView /\ BugInvalidStaysBeforeMax
    THEN CurrentView(candidate)
    ELSE SaturatingNext(CurrentView(candidate))
  ELSE
    SaturatingNext(CurrentView(candidate))

ImplementationNextView(candidate) ==
  IF ImplementationAdvances(candidate)
  THEN BuggySaturatingNext(candidate)
  ELSE CurrentView(candidate)

EmitsNewViewVote(candidate) ==
  ImplementationAdvances(candidate)

EmitsAdvanceView(candidate) ==
  ImplementationAdvances(candidate)

OutputView(candidate) ==
  IF ImplementationAdvances(candidate)
  THEN
    IF BugOutputUsesOldView
    THEN CurrentView(candidate)
    ELSE ImplementationNextView(candidate)
  ELSE CurrentView(candidate)

TypeInvariant ==
  /\ BugTickWrapAtMax \in BOOLEAN
  /\ BugTickStaysBeforeMax \in BOOLEAN
  /\ BugInvalidWrapAtMax \in BOOLEAN
  /\ BugInvalidStaysBeforeMax \in BOOLEAN
  /\ BugValidAdvances \in BOOLEAN
  /\ BugStaleValidationAdvances \in BOOLEAN
  /\ BugNoInflightAdvances \in BOOLEAN
  /\ BugOutputUsesOldView \in BOOLEAN
  /\ tried \subseteq Cases

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Cases \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Cases: TryCandidate(candidate)
  \/ Stable

ViewMatchesSpec ==
  \A candidate \in tried:
    ImplementationNextView(candidate) = SpecNextView(candidate)

AdvancementMatchesSpec ==
  \A candidate \in tried:
    ImplementationAdvances(candidate) <=> SpecAdvances(candidate)

ViewNeverWraps ==
  \A candidate \in tried:
    ImplementationNextView(candidate) >= CurrentView(candidate)

MidViewsAdvanceByOne ==
  \A candidate \in tried:
    /\ SpecAdvances(candidate)
    /\ CurrentView(candidate) < MaxView
    => ImplementationNextView(candidate) = CurrentView(candidate) + 1

MaxViewsSaturate ==
  \A candidate \in tried:
    /\ SpecAdvances(candidate)
    /\ CurrentView(candidate) = MaxView
    => ImplementationNextView(candidate) = MaxView

NonAdvancingValidationCallbacksStayPut ==
  \A candidate \in tried:
    ~SpecAdvances(candidate) =>
      /\ ImplementationNextView(candidate) = CurrentView(candidate)
      /\ ~EmitsNewViewVote(candidate)
      /\ ~EmitsAdvanceView(candidate)

AdvancingInputsEmitBothOutputs ==
  \A candidate \in tried:
    SpecAdvances(candidate) =>
      /\ EmitsNewViewVote(candidate)
      /\ EmitsAdvanceView(candidate)

OutputsBindStoredView ==
  \A candidate \in tried:
    ImplementationAdvances(candidate) =>
      OutputView(candidate) = ImplementationNextView(candidate)

EngineViewAdvanceSaturationExactness ==
  /\ ViewMatchesSpec
  /\ AdvancementMatchesSpec
  /\ ViewNeverWraps
  /\ MidViewsAdvanceByOne
  /\ MaxViewsSaturate
  /\ NonAdvancingValidationCallbacksStayPut
  /\ AdvancingInputsEmitBothOutputs
  /\ OutputsBindStoredView

EngineViewAdvanceSaturationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineViewAdvanceSaturationExactness

Safety == EngineViewAdvanceSaturationExactness

====
