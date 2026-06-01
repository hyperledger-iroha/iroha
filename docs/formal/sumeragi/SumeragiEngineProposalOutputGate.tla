---- MODULE SumeragiEngineProposalOutputGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact proposal output fields.

This slice models the `ConsensusOutput` values emitted by
`ConsensusEngine::on_proposal(...)`. Accepted proposals must emit exactly a
`ValidateBlock` for the proposal subject followed by a prepare `SignVote` for
the proposal round and proposal subject with no highest-QC reference. Rejected
proposals must emit nothing, including guard failures for phase, round,
highest-QC compatibility, and proposal-lock checks.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipValidateOutput,
  \* @type: Bool;
  BugSkipPrepareVoteOutput,
  \* @type: Bool;
  BugSwapOutputOrder,
  \* @type: Bool;
  BugValidateWrongSubject,
  \* @type: Bool;
  BugVoteWrongPhase,
  \* @type: Bool;
  BugVoteWrongRound,
  \* @type: Bool;
  BugVoteWrongSubject,
  \* @type: Bool;
  BugVoteCarriesHighestQc,
  \* @type: Bool;
  BugEmitOnRejected

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Candidates == {
  "safe_unlocked_no_qc",
  "safe_locked_subject_no_qc",
  "safe_conflict_higher_qc",
  "wrong_phase",
  "wrong_round",
  "incompatible_highest",
  "locked_conflict_no_qc",
  "locked_conflict_equal_qc",
  "locked_conflict_lower_qc"
}

Subjects == {"subject_a", "subject_b", "subject_wrong"}
Rounds == {"round_current", "round_wrong", "round_highest"}
Phases == {"Prepare", "Commit", "NewView"}
HighestQcs == {"none", "qc_higher", "qc_wrong"}
OutputShapes == {
  "empty",
  "validate_only",
  "vote_only",
  "validate_then_vote",
  "vote_then_validate"
}

Accepted(candidate) ==
  candidate \in {
    "safe_unlocked_no_qc",
    "safe_locked_subject_no_qc",
    "safe_conflict_higher_qc"
  }

ProposalSubject(candidate) ==
  IF candidate \in {
    "safe_conflict_higher_qc",
    "locked_conflict_no_qc",
    "locked_conflict_equal_qc",
    "locked_conflict_lower_qc"
  }
  THEN "subject_b"
  ELSE "subject_a"

ProposalRound(candidate) ==
  IF candidate = "wrong_round"
  THEN "round_wrong"
  ELSE "round_current"

ProposalHighestQc(candidate) ==
  IF candidate \in {"safe_conflict_higher_qc", "locked_conflict_equal_qc"}
  THEN "qc_higher"
  ELSE IF candidate = "incompatible_highest"
       THEN "qc_wrong"
       ELSE "none"

ImplementationHasValidation(candidate) ==
  IF Accepted(candidate)
  THEN ~BugSkipValidateOutput
  ELSE BugEmitOnRejected

ImplementationHasVote(candidate) ==
  IF Accepted(candidate)
  THEN ~BugSkipPrepareVoteOutput
  ELSE BugEmitOnRejected

ImplementationOutputShape(candidate) ==
  IF ImplementationHasValidation(candidate) /\ ImplementationHasVote(candidate)
  THEN
    IF Accepted(candidate) /\ BugSwapOutputOrder
    THEN "vote_then_validate"
    ELSE "validate_then_vote"
  ELSE IF ImplementationHasValidation(candidate)
       THEN "validate_only"
       ELSE IF ImplementationHasVote(candidate)
            THEN "vote_only"
            ELSE "empty"

ImplementationValidationSubject(candidate) ==
  IF ImplementationHasValidation(candidate)
  THEN
    IF Accepted(candidate) /\ BugValidateWrongSubject
    THEN "subject_wrong"
    ELSE ProposalSubject(candidate)
  ELSE "subject_wrong"

ImplementationVotePhase(candidate) ==
  IF ImplementationHasVote(candidate)
  THEN
    IF Accepted(candidate) /\ BugVoteWrongPhase
    THEN "Commit"
    ELSE "Prepare"
  ELSE "Prepare"

ImplementationVoteRound(candidate) ==
  IF ImplementationHasVote(candidate)
  THEN
    IF Accepted(candidate) /\ BugVoteWrongRound
    THEN "round_wrong"
    ELSE "round_current"
  ELSE "round_current"

ImplementationVoteSubject(candidate) ==
  IF ImplementationHasVote(candidate)
  THEN
    IF Accepted(candidate) /\ BugVoteWrongSubject
    THEN "subject_wrong"
    ELSE ProposalSubject(candidate)
  ELSE "subject_wrong"

ImplementationVoteHighestQc(candidate) ==
  IF ImplementationHasVote(candidate)
  THEN
    IF Accepted(candidate) /\ BugVoteCarriesHighestQc
    THEN ProposalHighestQc(candidate)
    ELSE "none"
  ELSE "none"

TypeInvariant ==
  /\ BugSkipValidateOutput \in BOOLEAN
  /\ BugSkipPrepareVoteOutput \in BOOLEAN
  /\ BugSwapOutputOrder \in BOOLEAN
  /\ BugValidateWrongSubject \in BOOLEAN
  /\ BugVoteWrongPhase \in BOOLEAN
  /\ BugVoteWrongRound \in BOOLEAN
  /\ BugVoteWrongSubject \in BOOLEAN
  /\ BugVoteCarriesHighestQc \in BOOLEAN
  /\ BugEmitOnRejected \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ \A candidate \in tried:
    /\ ProposalSubject(candidate) \in Subjects
    /\ ProposalRound(candidate) \in Rounds
    /\ ProposalHighestQc(candidate) \in HighestQcs
    /\ ImplementationOutputShape(candidate) \in OutputShapes
    /\ ImplementationValidationSubject(candidate) \in Subjects
    /\ ImplementationVotePhase(candidate) \in Phases
    /\ ImplementationVoteRound(candidate) \in Rounds
    /\ ImplementationVoteSubject(candidate) \in Subjects
    /\ ImplementationVoteHighestQc(candidate) \in HighestQcs

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

AcceptedOutputShapeExact ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationOutputShape(candidate) = "validate_then_vote"

AcceptedOutputsStayPaired ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationHasValidation(candidate) = ImplementationHasVote(candidate)

AcceptedValidateSubjectExact ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationValidationSubject(candidate) = ProposalSubject(candidate)

AcceptedPrepareVotePhaseExact ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationVotePhase(candidate) = "Prepare"

AcceptedPrepareVoteRoundExact ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationVoteRound(candidate) = ProposalRound(candidate)

AcceptedPrepareVoteSubjectExact ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationVoteSubject(candidate) = ProposalSubject(candidate)

AcceptedPrepareVoteHighestQcNone ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationVoteHighestQc(candidate) = "none"

RejectedProposalsEmitNothing ==
  \A candidate \in tried:
    ~Accepted(candidate) =>
      /\ ~ImplementationHasValidation(candidate)
      /\ ~ImplementationHasVote(candidate)
      /\ ImplementationOutputShape(candidate) = "empty"

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ ProposalSubject(candidate) \in Subjects
    /\ ProposalRound(candidate) \in Rounds
    /\ ProposalHighestQc(candidate) \in HighestQcs
    /\ ImplementationOutputShape(candidate) \in OutputShapes
    /\ ImplementationValidationSubject(candidate) \in Subjects
    /\ ImplementationVotePhase(candidate) \in Phases
    /\ ImplementationVoteRound(candidate) \in Rounds
    /\ ImplementationVoteSubject(candidate) \in Subjects
    /\ ImplementationVoteHighestQc(candidate) \in HighestQcs

Safety ==
  /\ AcceptedOutputShapeExact
  /\ AcceptedOutputsStayPaired
  /\ AcceptedValidateSubjectExact
  /\ AcceptedPrepareVotePhaseExact
  /\ AcceptedPrepareVoteRoundExact
  /\ AcceptedPrepareVoteSubjectExact
  /\ AcceptedPrepareVoteHighestQcNone
  /\ RejectedProposalsEmitNothing
  /\ ValuesStayInDomain

=============================================================================
====
