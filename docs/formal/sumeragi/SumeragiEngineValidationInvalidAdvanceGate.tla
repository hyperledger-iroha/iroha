---- MODULE SumeragiEngineValidationInvalidAdvanceGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact invalid-validation round advancement.

This slice models the field-level round/output side effects in
`ConsensusEngine::on_validation_result(...)` after an invalid current
validation result:

  next = RoundId { view: round.view.saturating_add(1), ..round }

The engine must store exactly that next round, preserving height, epoch, and
validator set while advancing only the view with saturating arithmetic. The
same exact next round must be used in both the NewView vote and the
AdvanceView output. Valid current results and ignored callbacks preserve the
current round and emit no view-advance outputs.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipStateAdvance,
  \* @type: Bool;
  BugAdvanceWrongHeight,
  \* @type: Bool;
  BugAdvanceWrongEpoch,
  \* @type: Bool;
  BugAdvanceWrongValidatorSet,
  \* @type: Bool;
  BugWrapMaxView,
  \* @type: Bool;
  BugSkipSignVote,
  \* @type: Bool;
  BugSkipAdvanceOutput,
  \* @type: Bool;
  BugSignOldRound,
  \* @type: Bool;
  BugAdvanceOutputOldRound,
  \* @type: Bool;
  BugOutputRoundsMismatch,
  \* @type: Bool;
  BugAdvanceOnValid,
  \* @type: Bool;
  BugAdvanceOnIgnored

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "invalid_normal_no_highest",
  "invalid_normal_with_highest",
  "invalid_max_view",
  "valid_current",
  "wrong_round",
  "wrong_block_hash",
  "no_inflight",
  "replay_after_valid",
  "superseded_by_commit"
}

RoundValues == {
  "none",
  "round_current",
  "round_next",
  "round_next_max",
  "round_wrong_height",
  "round_wrong_epoch",
  "round_wrong_validator_set",
  "round_wrapped"
}

CurrentInvalid(candidate) ==
  candidate \in {
    "invalid_normal_no_highest",
    "invalid_normal_with_highest",
    "invalid_max_view"
  }

CurrentValid(candidate) ==
  candidate = "valid_current"

Ignored(candidate) ==
  ~(CurrentInvalid(candidate) \/ CurrentValid(candidate))

MaxView(candidate) ==
  candidate = "invalid_max_view"

InitialRound(candidate) ==
  "round_current"

ExpectedNextRound(candidate) ==
  IF MaxView(candidate)
  THEN "round_next_max"
  ELSE "round_next"

SpecFinalRound(candidate) ==
  IF CurrentInvalid(candidate)
  THEN ExpectedNextRound(candidate)
  ELSE InitialRound(candidate)

SpecSignVoteRound(candidate) ==
  IF CurrentInvalid(candidate)
  THEN ExpectedNextRound(candidate)
  ELSE "none"

SpecAdvanceOutputRound(candidate) ==
  IF CurrentInvalid(candidate)
  THEN ExpectedNextRound(candidate)
  ELSE "none"

WrongStateRound(candidate) ==
  CASE BugAdvanceWrongHeight -> "round_wrong_height"
    [] BugAdvanceWrongEpoch -> "round_wrong_epoch"
    [] BugAdvanceWrongValidatorSet -> "round_wrong_validator_set"
    [] BugWrapMaxView /\ MaxView(candidate) -> "round_wrapped"
    [] OTHER -> ExpectedNextRound(candidate)

ImplementationFinalRound(candidate) ==
  IF CurrentInvalid(candidate)
  THEN
    IF BugSkipStateAdvance
    THEN InitialRound(candidate)
    ELSE WrongStateRound(candidate)
  ELSE IF CurrentValid(candidate)
       THEN
         IF BugAdvanceOnValid
         THEN "round_next"
         ELSE InitialRound(candidate)
       ELSE
         IF BugAdvanceOnIgnored
         THEN "round_next"
         ELSE InitialRound(candidate)

ImplementationSignVoteRound(candidate) ==
  IF CurrentInvalid(candidate)
  THEN
    IF BugSkipSignVote
    THEN "none"
    ELSE IF BugSignOldRound
         THEN InitialRound(candidate)
         ELSE IF BugOutputRoundsMismatch
              THEN "round_wrong_epoch"
              ELSE ExpectedNextRound(candidate)
  ELSE IF CurrentValid(candidate) /\ BugAdvanceOnValid
       THEN "round_next"
       ELSE IF Ignored(candidate) /\ BugAdvanceOnIgnored
            THEN "round_next"
            ELSE "none"

ImplementationAdvanceOutputRound(candidate) ==
  IF CurrentInvalid(candidate)
  THEN
    IF BugSkipAdvanceOutput
    THEN "none"
    ELSE IF BugAdvanceOutputOldRound
         THEN InitialRound(candidate)
         ELSE ExpectedNextRound(candidate)
  ELSE IF CurrentValid(candidate) /\ BugAdvanceOnValid
       THEN "round_next"
       ELSE IF Ignored(candidate) /\ BugAdvanceOnIgnored
            THEN "round_next"
            ELSE "none"

TypeInvariant ==
  /\ BugSkipStateAdvance \in BOOLEAN
  /\ BugAdvanceWrongHeight \in BOOLEAN
  /\ BugAdvanceWrongEpoch \in BOOLEAN
  /\ BugAdvanceWrongValidatorSet \in BOOLEAN
  /\ BugWrapMaxView \in BOOLEAN
  /\ BugSkipSignVote \in BOOLEAN
  /\ BugSkipAdvanceOutput \in BOOLEAN
  /\ BugSignOldRound \in BOOLEAN
  /\ BugAdvanceOutputOldRound \in BOOLEAN
  /\ BugOutputRoundsMismatch \in BOOLEAN
  /\ BugAdvanceOnValid \in BOOLEAN
  /\ BugAdvanceOnIgnored \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialRound(candidate) \in RoundValues
    /\ ExpectedNextRound(candidate) \in RoundValues
    /\ SpecFinalRound(candidate) \in RoundValues
    /\ SpecSignVoteRound(candidate) \in RoundValues
    /\ SpecAdvanceOutputRound(candidate) \in RoundValues
    /\ ImplementationFinalRound(candidate) \in RoundValues
    /\ ImplementationSignVoteRound(candidate) \in RoundValues
    /\ ImplementationAdvanceOutputRound(candidate) \in RoundValues

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

FinalRoundMatchesSpec ==
  \A candidate \in tried:
    ImplementationFinalRound(candidate) = SpecFinalRound(candidate)

SignVoteRoundMatchesSpec ==
  \A candidate \in tried:
    ImplementationSignVoteRound(candidate) = SpecSignVoteRound(candidate)

AdvanceOutputRoundMatchesSpec ==
  \A candidate \in tried:
    ImplementationAdvanceOutputRound(candidate) = SpecAdvanceOutputRound(candidate)

InvalidStateUsesExactNextRound ==
  \A candidate \in tried:
    CurrentInvalid(candidate) =>
      ImplementationFinalRound(candidate) = ExpectedNextRound(candidate)

InvalidOutputsUseExactNextRound ==
  \A candidate \in tried:
    CurrentInvalid(candidate) =>
      /\ ImplementationSignVoteRound(candidate) = ExpectedNextRound(candidate)
      /\ ImplementationAdvanceOutputRound(candidate) = ExpectedNextRound(candidate)

InvalidOutputRoundsAgreeWithState ==
  \A candidate \in tried:
    CurrentInvalid(candidate) =>
      /\ ImplementationSignVoteRound(candidate) = ImplementationFinalRound(candidate)
      /\ ImplementationAdvanceOutputRound(candidate) = ImplementationFinalRound(candidate)

ValidCurrentPreservesRoundAndHasNoOutputs ==
  \A candidate \in tried:
    CurrentValid(candidate) =>
      /\ ImplementationFinalRound(candidate) = InitialRound(candidate)
      /\ ImplementationSignVoteRound(candidate) = "none"
      /\ ImplementationAdvanceOutputRound(candidate) = "none"

IgnoredCallbacksPreserveRoundAndHaveNoOutputs ==
  \A candidate \in tried:
    Ignored(candidate) =>
      /\ ImplementationFinalRound(candidate) = InitialRound(candidate)
      /\ ImplementationSignVoteRound(candidate) = "none"
      /\ ImplementationAdvanceOutputRound(candidate) = "none"

SaturatingMaxViewDoesNotWrap ==
  "invalid_max_view" \in tried =>
    /\ ImplementationFinalRound("invalid_max_view") = "round_next_max"
    /\ ImplementationSignVoteRound("invalid_max_view") = "round_next_max"
    /\ ImplementationAdvanceOutputRound("invalid_max_view") = "round_next_max"

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialRound(candidate) \in RoundValues
    /\ ExpectedNextRound(candidate) \in RoundValues
    /\ SpecFinalRound(candidate) \in RoundValues
    /\ SpecSignVoteRound(candidate) \in RoundValues
    /\ SpecAdvanceOutputRound(candidate) \in RoundValues
    /\ ImplementationFinalRound(candidate) \in RoundValues
    /\ ImplementationSignVoteRound(candidate) \in RoundValues
    /\ ImplementationAdvanceOutputRound(candidate) \in RoundValues

EngineValidationInvalidAdvanceExactness ==
  /\ FinalRoundMatchesSpec
  /\ SignVoteRoundMatchesSpec
  /\ AdvanceOutputRoundMatchesSpec
  /\ InvalidStateUsesExactNextRound
  /\ InvalidOutputsUseExactNextRound
  /\ InvalidOutputRoundsAgreeWithState
  /\ ValidCurrentPreservesRoundAndHasNoOutputs
  /\ IgnoredCallbacksPreserveRoundAndHaveNoOutputs
  /\ SaturatingMaxViewDoesNotWrap
  /\ ValuesStayInDomain

Safety ==
  EngineValidationInvalidAdvanceExactness

EngineValidationInvalidAdvanceCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineValidationInvalidAdvanceExactness

SafetyFast == EngineValidationInvalidAdvanceExactness

====
