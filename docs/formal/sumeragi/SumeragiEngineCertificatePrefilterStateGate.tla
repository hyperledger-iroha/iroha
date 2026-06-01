---- MODULE SumeragiEngineCertificatePrefilterStateGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for certificate prefilter state handoff.

This slice models the shared `ConsensusEngine::on_certificate(...)` prefilter
before phase-specific handlers run. Rejected certificates must return before
any state mutation or output. Accepted certificates must dispatch to the
correct phase handler with the prefilter-visible engine state unchanged; all
state mutation is owned by `on_prepare_qc(...)`, `on_commit_qc(...)`, or
`on_new_view_qc(...)`, not by the shared prefilter.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptedDropsDispatch,
  \* @type: Bool;
  BugAcceptedMutatesPhase,
  \* @type: Bool;
  BugAcceptedUpdatesRound,
  \* @type: Bool;
  BugAcceptedClearsLock,
  \* @type: Bool;
  BugAcceptedRecordsHighest,
  \* @type: Bool;
  BugAcceptedClearsPending,
  \* @type: Bool;
  BugAcceptedClearsValidation,
  \* @type: Bool;
  BugRejectedMutatesPhase,
  \* @type: Bool;
  BugRejectedUpdatesRound,
  \* @type: Bool;
  BugRejectedClearsLock,
  \* @type: Bool;
  BugRejectedRecordsHighest,
  \* @type: Bool;
  BugRejectedClearsPending,
  \* @type: Bool;
  BugRejectedClearsValidation,
  \* @type: Bool;
  BugRejectedEmitsOutput

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

TlcSingletonOrEmpty == Cardinality(tried) \in {0, 1}

Cases == {
  "current_prepare",
  "current_commit",
  "new_view_lower_view",
  "new_view_same_view",
  "new_view_future_view",
  "stale_prepare",
  "stale_commit",
  "committed_prepare",
  "committed_commit",
  "committed_new_view",
  "wrong_height_prepare",
  "wrong_height_commit",
  "wrong_height_new_view",
  "wrong_epoch_prepare",
  "wrong_epoch_commit",
  "wrong_epoch_new_view",
  "wrong_validator_set_prepare",
  "wrong_validator_set_commit",
  "wrong_validator_set_new_view",
  "wrong_quorum_prepare",
  "wrong_quorum_commit",
  "wrong_quorum_new_view"
}

Handlers == {"none", "prepare", "commit", "newView"}
Phases == {"Proposal", "Prepare", "Commit", "PendingFinality"}
Rounds == {"round_current", "round_wrong"}
Locks == {"none", "lock_subject_a"}
HighestQcs == {"none", "qc_low", "qc_wrong"}
PendingFinality == {"none", "pending_subject_a"}
ValidationOwners == {"none", "subject_a"}

PhaseHandler(candidate) ==
  CASE candidate \in {
      "current_prepare",
      "stale_prepare",
      "committed_prepare",
      "wrong_height_prepare",
      "wrong_epoch_prepare",
      "wrong_validator_set_prepare",
      "wrong_quorum_prepare"
    } -> "prepare"
    [] candidate \in {
      "current_commit",
      "stale_commit",
      "committed_commit",
      "wrong_height_commit",
      "wrong_epoch_commit",
      "wrong_validator_set_commit",
      "wrong_quorum_commit"
    } -> "commit"
    [] OTHER -> "newView"

CommittedHeight(candidate) ==
  candidate \in {
    "committed_prepare",
    "committed_commit",
    "committed_new_view"
  }

WrongContext(candidate) ==
  candidate \in {
    "wrong_height_prepare",
    "wrong_height_commit",
    "wrong_height_new_view",
    "wrong_epoch_prepare",
    "wrong_epoch_commit",
    "wrong_epoch_new_view",
    "wrong_validator_set_prepare",
    "wrong_validator_set_commit",
    "wrong_validator_set_new_view"
  }

WrongQuorumPolicy(candidate) ==
  candidate \in {
    "wrong_quorum_prepare",
    "wrong_quorum_commit",
    "wrong_quorum_new_view"
  }

StalePrepareCommit(candidate) ==
  candidate \in {"stale_prepare", "stale_commit"}

SpecHandler(candidate) ==
  IF \/ CommittedHeight(candidate)
     \/ WrongContext(candidate)
     \/ WrongQuorumPolicy(candidate)
     \/ StalePrepareCommit(candidate)
  THEN "none"
  ELSE PhaseHandler(candidate)

Accepted(candidate) ==
  SpecHandler(candidate) # "none"

Rejected(candidate) ==
  SpecHandler(candidate) = "none"

InitialPhase(candidate) ==
  CASE candidate = "current_prepare" -> "Prepare"
    [] candidate = "current_commit" -> "Commit"
    [] candidate = "new_view_future_view" -> "Proposal"
    [] OTHER -> "PendingFinality"

InitialRound(candidate) == "round_current"

InitialLockedQc(candidate) == "lock_subject_a"

InitialHighestQc(candidate) == "qc_low"

InitialPendingFinality(candidate) == "pending_subject_a"

InitialValidating(candidate) == "subject_a"

ImplementationHandler(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedDropsDispatch
    THEN "none"
    ELSE PhaseHandler(candidate)
  ELSE "none"

ImplementationOutputEmitted(candidate) ==
  IF Rejected(candidate)
  THEN BugRejectedEmitsOutput
  ELSE FALSE

ImplementationPhase(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedMutatesPhase
    THEN "PendingFinality"
    ELSE InitialPhase(candidate)
  ELSE IF BugRejectedMutatesPhase
       THEN "Prepare"
       ELSE InitialPhase(candidate)

ImplementationRound(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedUpdatesRound
    THEN "round_wrong"
    ELSE InitialRound(candidate)
  ELSE IF BugRejectedUpdatesRound
       THEN "round_wrong"
       ELSE InitialRound(candidate)

ImplementationLockedQc(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedClearsLock
    THEN "none"
    ELSE InitialLockedQc(candidate)
  ELSE IF BugRejectedClearsLock
       THEN "none"
       ELSE InitialLockedQc(candidate)

ImplementationHighestQc(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedRecordsHighest
    THEN "qc_wrong"
    ELSE InitialHighestQc(candidate)
  ELSE IF BugRejectedRecordsHighest
       THEN "qc_wrong"
       ELSE InitialHighestQc(candidate)

ImplementationPendingFinality(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedClearsPending
    THEN "none"
    ELSE InitialPendingFinality(candidate)
  ELSE IF BugRejectedClearsPending
       THEN "none"
       ELSE InitialPendingFinality(candidate)

ImplementationValidating(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedClearsValidation
    THEN "none"
    ELSE InitialValidating(candidate)
  ELSE IF BugRejectedClearsValidation
       THEN "none"
       ELSE InitialValidating(candidate)

TypeInvariant ==
  /\ BugAcceptedDropsDispatch \in BOOLEAN
  /\ BugAcceptedMutatesPhase \in BOOLEAN
  /\ BugAcceptedUpdatesRound \in BOOLEAN
  /\ BugAcceptedClearsLock \in BOOLEAN
  /\ BugAcceptedRecordsHighest \in BOOLEAN
  /\ BugAcceptedClearsPending \in BOOLEAN
  /\ BugAcceptedClearsValidation \in BOOLEAN
  /\ BugRejectedMutatesPhase \in BOOLEAN
  /\ BugRejectedUpdatesRound \in BOOLEAN
  /\ BugRejectedClearsLock \in BOOLEAN
  /\ BugRejectedRecordsHighest \in BOOLEAN
  /\ BugRejectedClearsPending \in BOOLEAN
  /\ BugRejectedClearsValidation \in BOOLEAN
  /\ BugRejectedEmitsOutput \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ PhaseHandler(candidate) \in Handlers
    /\ SpecHandler(candidate) \in Handlers
    /\ ImplementationHandler(candidate) \in Handlers
    /\ InitialPhase(candidate) \in Phases
    /\ ImplementationPhase(candidate) \in Phases
    /\ InitialRound(candidate) \in Rounds
    /\ ImplementationRound(candidate) \in Rounds
    /\ InitialLockedQc(candidate) \in Locks
    /\ ImplementationLockedQc(candidate) \in Locks
    /\ InitialHighestQc(candidate) \in HighestQcs
    /\ ImplementationHighestQc(candidate) \in HighestQcs
    /\ InitialPendingFinality(candidate) \in PendingFinality
    /\ ImplementationPendingFinality(candidate) \in PendingFinality
    /\ InitialValidating(candidate) \in ValidationOwners
    /\ ImplementationValidating(candidate) \in ValidationOwners
    /\ ImplementationOutputEmitted(candidate) \in BOOLEAN

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

AcceptedDispatchesToCorrectHandler ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationHandler(candidate) = PhaseHandler(candidate)

AcceptedHandlerReceivesOriginalState ==
  \A candidate \in tried:
    Accepted(candidate) =>
      /\ ImplementationPhase(candidate) = InitialPhase(candidate)
      /\ ImplementationRound(candidate) = InitialRound(candidate)
      /\ ImplementationLockedQc(candidate) = InitialLockedQc(candidate)
      /\ ImplementationHighestQc(candidate) = InitialHighestQc(candidate)
      /\ ImplementationPendingFinality(candidate) =
        InitialPendingFinality(candidate)
      /\ ImplementationValidating(candidate) = InitialValidating(candidate)

RejectedDoesNotDispatch ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationHandler(candidate) = "none"

RejectedStatePreserved ==
  \A candidate \in tried:
    Rejected(candidate) =>
      /\ ImplementationPhase(candidate) = InitialPhase(candidate)
      /\ ImplementationRound(candidate) = InitialRound(candidate)
      /\ ImplementationLockedQc(candidate) = InitialLockedQc(candidate)
      /\ ImplementationHighestQc(candidate) = InitialHighestQc(candidate)
      /\ ImplementationPendingFinality(candidate) =
        InitialPendingFinality(candidate)
      /\ ImplementationValidating(candidate) = InitialValidating(candidate)

RejectedEmitsNothing ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ~ImplementationOutputEmitted(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ PhaseHandler(candidate) \in Handlers
    /\ SpecHandler(candidate) \in Handlers
    /\ ImplementationHandler(candidate) \in Handlers
    /\ InitialPhase(candidate) \in Phases
    /\ ImplementationPhase(candidate) \in Phases
    /\ InitialRound(candidate) \in Rounds
    /\ ImplementationRound(candidate) \in Rounds
    /\ InitialLockedQc(candidate) \in Locks
    /\ ImplementationLockedQc(candidate) \in Locks
    /\ InitialHighestQc(candidate) \in HighestQcs
    /\ ImplementationHighestQc(candidate) \in HighestQcs
    /\ InitialPendingFinality(candidate) \in PendingFinality
    /\ ImplementationPendingFinality(candidate) \in PendingFinality
    /\ InitialValidating(candidate) \in ValidationOwners
    /\ ImplementationValidating(candidate) \in ValidationOwners
    /\ ImplementationOutputEmitted(candidate) \in BOOLEAN

Safety ==
  /\ AcceptedDispatchesToCorrectHandler
  /\ AcceptedHandlerReceivesOriginalState
  /\ RejectedDoesNotDispatch
  /\ RejectedStatePreserved
  /\ RejectedEmitsNothing
  /\ ValuesStayInDomain

=============================================================================
