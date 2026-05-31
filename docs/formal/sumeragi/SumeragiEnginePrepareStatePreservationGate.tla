---- MODULE SumeragiEnginePrepareStatePreservationGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for Prepare-QC unrelated-state preservation.

This slice models the state fields that `ConsensusEngine::on_prepare_qc(...)`
must not touch. Accepted fresh Prepare QCs update the lock/highest-QC record,
phase, commit-vote cache, and output path covered by companion models, but
they preserve the current round, pending-finality marker, validation owner, and
committed-height record exactly. Prepare QCs rejected by the shared
certificate prefilter, same-round replay/conflict guard, or pending-finality
guard preserve those same fields exactly and emit no unrelated state change.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptedUpdatesRound,
  \* @type: Bool;
  BugAcceptedClearsPending,
  \* @type: Bool;
  BugAcceptedClearsValidation,
  \* @type: Bool;
  BugAcceptedRecordsCommit,
  \* @type: Bool;
  BugAcceptedClearsCommitRecord,
  \* @type: Bool;
  BugRejectedUpdatesRound,
  \* @type: Bool;
  BugRejectedClearsPending,
  \* @type: Bool;
  BugRejectedClearsValidation,
  \* @type: Bool;
  BugRejectedRecordsCommit,
  \* @type: Bool;
  BugRejectedClearsCommitRecord

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "safe_clean",
  "safe_with_pending_marker",
  "safe_with_validation_owner",
  "wrong_height",
  "wrong_epoch",
  "wrong_validator_set",
  "wrong_quorum_policy",
  "stale_view",
  "committed_height",
  "replay_same_prepare",
  "conflicting_prepare",
  "pending_finality"
}

RoundValues == {"round_current", "round_next"}
PendingValues == {"none", "pending_subject_a"}
ValidationOwners == {"none", "subject_a"}
CommittedValues == {
  "none",
  "committed_other_height",
  "committed_current_height",
  "committed_wrong_height"
}

Accepted(candidate) ==
  candidate \in {
    "safe_clean",
    "safe_with_pending_marker",
    "safe_with_validation_owner"
  }

RejectedPrefilter(candidate) ==
  candidate \in {
    "wrong_height",
    "wrong_epoch",
    "wrong_validator_set",
    "wrong_quorum_policy",
    "stale_view",
    "committed_height"
  }

ReplayConflictOrPending(candidate) ==
  candidate \in {
    "replay_same_prepare",
    "conflicting_prepare",
    "pending_finality"
  }

Rejected(candidate) ==
  RejectedPrefilter(candidate) \/ ReplayConflictOrPending(candidate)

InitialRound(candidate) == "round_current"

InitialPendingFinality(candidate) ==
  IF candidate \in {"safe_with_pending_marker", "pending_finality"}
  THEN "pending_subject_a"
  ELSE "none"

InitialValidationOwner(candidate) ==
  IF candidate \in {
    "safe_with_validation_owner",
    "stale_view",
    "replay_same_prepare",
    "conflicting_prepare"
  }
  THEN "subject_a"
  ELSE "none"

InitialCommitted(candidate) ==
  IF candidate = "committed_height"
  THEN "committed_current_height"
  ELSE "committed_other_height"

ImplementationRound(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedUpdatesRound
    THEN "round_next"
    ELSE InitialRound(candidate)
  ELSE IF BugRejectedUpdatesRound
       THEN "round_next"
       ELSE InitialRound(candidate)

ImplementationPendingFinality(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedClearsPending
    THEN "none"
    ELSE InitialPendingFinality(candidate)
  ELSE IF BugRejectedClearsPending
       THEN "none"
       ELSE InitialPendingFinality(candidate)

ImplementationValidationOwner(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedClearsValidation
    THEN "none"
    ELSE InitialValidationOwner(candidate)
  ELSE IF BugRejectedClearsValidation
       THEN "none"
       ELSE InitialValidationOwner(candidate)

ImplementationCommitted(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedRecordsCommit
    THEN "committed_wrong_height"
    ELSE IF BugAcceptedClearsCommitRecord
         THEN "none"
         ELSE InitialCommitted(candidate)
  ELSE IF BugRejectedRecordsCommit
       THEN "committed_wrong_height"
       ELSE IF BugRejectedClearsCommitRecord
            THEN "none"
            ELSE InitialCommitted(candidate)

TypeInvariant ==
  /\ BugAcceptedUpdatesRound \in BOOLEAN
  /\ BugAcceptedClearsPending \in BOOLEAN
  /\ BugAcceptedClearsValidation \in BOOLEAN
  /\ BugAcceptedRecordsCommit \in BOOLEAN
  /\ BugAcceptedClearsCommitRecord \in BOOLEAN
  /\ BugRejectedUpdatesRound \in BOOLEAN
  /\ BugRejectedClearsPending \in BOOLEAN
  /\ BugRejectedClearsValidation \in BOOLEAN
  /\ BugRejectedRecordsCommit \in BOOLEAN
  /\ BugRejectedClearsCommitRecord \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialRound(candidate) \in RoundValues
    /\ ImplementationRound(candidate) \in RoundValues
    /\ InitialPendingFinality(candidate) \in PendingValues
    /\ ImplementationPendingFinality(candidate) \in PendingValues
    /\ InitialValidationOwner(candidate) \in ValidationOwners
    /\ ImplementationValidationOwner(candidate) \in ValidationOwners
    /\ InitialCommitted(candidate) \in CommittedValues
    /\ ImplementationCommitted(candidate) \in CommittedValues

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

AcceptedPreservesRound ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationRound(candidate) = InitialRound(candidate)

AcceptedPreservesPendingFinality ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationPendingFinality(candidate) =
        InitialPendingFinality(candidate)

AcceptedPreservesValidationOwner ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationValidationOwner(candidate) =
        InitialValidationOwner(candidate)

AcceptedPreservesCommittedRecord ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationCommitted(candidate) = InitialCommitted(candidate)

RejectedPreservesRound ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationRound(candidate) = InitialRound(candidate)

RejectedPreservesPendingFinality ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationPendingFinality(candidate) =
        InitialPendingFinality(candidate)

RejectedPreservesValidationOwner ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationValidationOwner(candidate) =
        InitialValidationOwner(candidate)

RejectedPreservesCommittedRecord ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationCommitted(candidate) = InitialCommitted(candidate)

AllModeledStatePreserved ==
  \A candidate \in tried:
    /\ ImplementationRound(candidate) = InitialRound(candidate)
    /\ ImplementationPendingFinality(candidate) =
      InitialPendingFinality(candidate)
    /\ ImplementationValidationOwner(candidate) =
      InitialValidationOwner(candidate)
    /\ ImplementationCommitted(candidate) = InitialCommitted(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialRound(candidate) \in RoundValues
    /\ ImplementationRound(candidate) \in RoundValues
    /\ InitialPendingFinality(candidate) \in PendingValues
    /\ ImplementationPendingFinality(candidate) \in PendingValues
    /\ InitialValidationOwner(candidate) \in ValidationOwners
    /\ ImplementationValidationOwner(candidate) \in ValidationOwners
    /\ InitialCommitted(candidate) \in CommittedValues
    /\ ImplementationCommitted(candidate) \in CommittedValues

Safety ==
  /\ AcceptedPreservesRound
  /\ AcceptedPreservesPendingFinality
  /\ AcceptedPreservesValidationOwner
  /\ AcceptedPreservesCommittedRecord
  /\ RejectedPreservesRound
  /\ RejectedPreservesPendingFinality
  /\ RejectedPreservesValidationOwner
  /\ RejectedPreservesCommittedRecord
  /\ AllModeledStatePreserved
  /\ ValuesStayInDomain

=============================================================================
