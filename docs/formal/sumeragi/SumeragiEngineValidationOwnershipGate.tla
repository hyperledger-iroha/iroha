---- MODULE SumeragiEngineValidationOwnershipGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact validation-owner cleanup.

This slice models the `self.validating = None` side effect in
`ConsensusEngine::on_validation_result(...)`. A callback may clear the
validation owner only after its round matches the current engine round, an
in-flight validation owner exists, and the callback block hash matches that
owner's block hash. Both valid and invalid current callbacks consume the
owner. Wrong-round, wrong-block, no-in-flight, replayed, and superseded
callbacks preserve the existing validation-owner state exactly.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugKeepValidOwner,
  \* @type: Bool;
  BugKeepInvalidOwner,
  \* @type: Bool;
  BugClearWrongRoundOwner,
  \* @type: Bool;
  BugClearWrongBlockOwner,
  \* @type: Bool;
  BugReplaceWrongRoundOwner,
  \* @type: Bool;
  BugReplaceWrongBlockOwner,
  \* @type: Bool;
  BugSetOwnerOnNoInflight,
  \* @type: Bool;
  BugSetOwnerOnReplay,
  \* @type: Bool;
  BugSetOwnerOnSuperseded

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "valid_current",
  "invalid_current",
  "wrong_round",
  "wrong_block_hash",
  "no_inflight",
  "replay_after_valid",
  "superseded_by_commit",
  "superseded_by_committed_block"
}

OwnerValues == {
  "none",
  "subject_a",
  "subject_b",
  "subject_wrong"
}

CurrentValid(candidate) ==
  candidate = "valid_current"

CurrentInvalid(candidate) ==
  candidate = "invalid_current"

CurrentMatch(candidate) ==
  CurrentValid(candidate) \/ CurrentInvalid(candidate)

WrongRound(candidate) ==
  candidate = "wrong_round"

WrongBlock(candidate) ==
  candidate = "wrong_block_hash"

NoInflight(candidate) ==
  candidate \in {"no_inflight", "replay_after_valid"}

Superseded(candidate) ==
  candidate \in {"superseded_by_commit", "superseded_by_committed_block"}

InitialOwner(candidate) ==
  IF candidate \in {
    "no_inflight",
    "replay_after_valid",
    "superseded_by_commit",
    "superseded_by_committed_block"
  }
  THEN "none"
  ELSE "subject_a"

SpecFinalOwner(candidate) ==
  IF CurrentMatch(candidate)
  THEN "none"
  ELSE InitialOwner(candidate)

ImplementationCurrentOwner(candidate) ==
  IF CurrentValid(candidate)
  THEN
    IF BugKeepValidOwner
    THEN InitialOwner(candidate)
    ELSE "none"
  ELSE IF BugKeepInvalidOwner
       THEN InitialOwner(candidate)
       ELSE "none"

ImplementationIgnoredOwner(candidate) ==
  IF WrongRound(candidate)
  THEN
    IF BugClearWrongRoundOwner
    THEN "none"
    ELSE IF BugReplaceWrongRoundOwner
         THEN "subject_wrong"
         ELSE InitialOwner(candidate)
  ELSE IF WrongBlock(candidate)
       THEN
         IF BugClearWrongBlockOwner
         THEN "none"
         ELSE IF BugReplaceWrongBlockOwner
              THEN "subject_wrong"
              ELSE InitialOwner(candidate)
       ELSE IF candidate = "no_inflight"
            THEN
              IF BugSetOwnerOnNoInflight
              THEN "subject_b"
              ELSE InitialOwner(candidate)
            ELSE IF candidate = "replay_after_valid"
                 THEN
                   IF BugSetOwnerOnReplay
                   THEN "subject_b"
                   ELSE InitialOwner(candidate)
                 ELSE IF Superseded(candidate)
                      THEN
                        IF BugSetOwnerOnSuperseded
                        THEN "subject_b"
                        ELSE InitialOwner(candidate)
                      ELSE InitialOwner(candidate)

ImplementationFinalOwner(candidate) ==
  IF CurrentMatch(candidate)
  THEN ImplementationCurrentOwner(candidate)
  ELSE ImplementationIgnoredOwner(candidate)

TypeInvariant ==
  /\ BugKeepValidOwner \in BOOLEAN
  /\ BugKeepInvalidOwner \in BOOLEAN
  /\ BugClearWrongRoundOwner \in BOOLEAN
  /\ BugClearWrongBlockOwner \in BOOLEAN
  /\ BugReplaceWrongRoundOwner \in BOOLEAN
  /\ BugReplaceWrongBlockOwner \in BOOLEAN
  /\ BugSetOwnerOnNoInflight \in BOOLEAN
  /\ BugSetOwnerOnReplay \in BOOLEAN
  /\ BugSetOwnerOnSuperseded \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialOwner(candidate) \in OwnerValues
    /\ SpecFinalOwner(candidate) \in OwnerValues
    /\ ImplementationFinalOwner(candidate) \in OwnerValues

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

FinalOwnerMatchesSpec ==
  \A candidate \in tried:
    ImplementationFinalOwner(candidate) = SpecFinalOwner(candidate)

CurrentValidClearsOwner ==
  "valid_current" \in tried =>
    ImplementationFinalOwner("valid_current") = "none"

CurrentInvalidClearsOwner ==
  "invalid_current" \in tried =>
    ImplementationFinalOwner("invalid_current") = "none"

WrongRoundPreservesOwner ==
  "wrong_round" \in tried =>
    ImplementationFinalOwner("wrong_round") = InitialOwner("wrong_round")

WrongBlockPreservesOwner ==
  "wrong_block_hash" \in tried =>
    ImplementationFinalOwner("wrong_block_hash") = InitialOwner("wrong_block_hash")

NoInflightPreservesNone ==
  "no_inflight" \in tried =>
    ImplementationFinalOwner("no_inflight") = "none"

ReplayPreservesNone ==
  "replay_after_valid" \in tried =>
    ImplementationFinalOwner("replay_after_valid") = "none"

SupersededCallbacksPreserveNone ==
  \A candidate \in tried:
    Superseded(candidate) =>
      ImplementationFinalOwner(candidate) = "none"

IgnoredCallbacksPreserveOwnerExactly ==
  \A candidate \in tried:
    ~CurrentMatch(candidate) =>
      ImplementationFinalOwner(candidate) = InitialOwner(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialOwner(candidate) \in OwnerValues
    /\ SpecFinalOwner(candidate) \in OwnerValues
    /\ ImplementationFinalOwner(candidate) \in OwnerValues

EngineValidationOwnershipExactness ==
  /\ FinalOwnerMatchesSpec
  /\ CurrentValidClearsOwner
  /\ CurrentInvalidClearsOwner
  /\ WrongRoundPreservesOwner
  /\ WrongBlockPreservesOwner
  /\ NoInflightPreservesNone
  /\ ReplayPreservesNone
  /\ SupersededCallbacksPreserveNone
  /\ IgnoredCallbacksPreserveOwnerExactly
  /\ ValuesStayInDomain

Safety ==
  EngineValidationOwnershipExactness

EngineValidationOwnershipCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineValidationOwnershipExactness

SafetyFast == EngineValidationOwnershipExactness

====
