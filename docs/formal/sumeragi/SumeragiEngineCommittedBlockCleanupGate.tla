---- MODULE SumeragiEngineCommittedBlockCleanupGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for pure-engine committed-block cleanup side effects.

This slice models the current-height cleanup branch in
`ConsensusEngine::on_committed_block(...)`. A fresh committed-block
notification for the current height records the height, clears in-flight
validation, returns the engine to proposal phase, and clears both the
pending-finality state and pending certificate map. A fresh notification for a
different height records that height but leaves current consensus ownership
unchanged. Duplicate or conflicting notifications for an already committed
height return before recording, cleanup, or output side effects.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipFreshRecord,
  \* @type: Bool;
  BugSkipCurrentValidationClear,
  \* @type: Bool;
  BugSkipCurrentPendingClear,
  \* @type: Bool;
  BugSkipCurrentPendingMapRemove,
  \* @type: Bool;
  BugWrongPhaseAfterCurrentCommit,
  \* @type: Bool;
  BugCleanupOtherHeight,
  \* @type: Bool;
  BugDuplicateCleansValidation,
  \* @type: Bool;
  BugDuplicateClearsPending,
  \* @type: Bool;
  BugConflictCleansValidation,
  \* @type: Bool;
  BugConflictClearsPending,
  \* @type: Bool;
  BugEmitCommitBlock

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "fresh_current_clean",
  "fresh_current_validating",
  "fresh_current_pending_matching",
  "fresh_current_pending_conflicting",
  "fresh_other_validating",
  "fresh_other_pending",
  "duplicate_current_validating",
  "duplicate_current_pending",
  "conflict_current_validating",
  "conflict_current_pending"
}

Phases == {"Proposal", "Prepare", "PendingFinality"}

Fresh(candidate) ==
  candidate \in {
    "fresh_current_clean",
    "fresh_current_validating",
    "fresh_current_pending_matching",
    "fresh_current_pending_conflicting",
    "fresh_other_validating",
    "fresh_other_pending"
  }

CurrentHeight(candidate) ==
  candidate \in {
    "fresh_current_clean",
    "fresh_current_validating",
    "fresh_current_pending_matching",
    "fresh_current_pending_conflicting",
    "duplicate_current_validating",
    "duplicate_current_pending",
    "conflict_current_validating",
    "conflict_current_pending"
  }

Duplicate(candidate) ==
  candidate \in {"duplicate_current_validating", "duplicate_current_pending"}

Conflict(candidate) ==
  candidate \in {"conflict_current_validating", "conflict_current_pending"}

InitialValidating(candidate) ==
  candidate \in {
    "fresh_current_validating",
    "fresh_other_validating",
    "duplicate_current_validating",
    "conflict_current_validating"
  }

InitialPending(candidate) ==
  candidate \in {
    "fresh_current_pending_matching",
    "fresh_current_pending_conflicting",
    "fresh_other_pending",
    "duplicate_current_pending",
    "conflict_current_pending"
  }

InitialPhase(candidate) ==
  CASE InitialPending(candidate) -> "PendingFinality"
    [] InitialValidating(candidate) -> "Prepare"
    [] OTHER -> "Proposal"

SpecRecords(candidate) ==
  Fresh(candidate)

SpecValidationAfter(candidate) ==
  IF Fresh(candidate) /\ CurrentHeight(candidate)
  THEN FALSE
  ELSE InitialValidating(candidate)

SpecPendingAfter(candidate) ==
  IF Fresh(candidate) /\ CurrentHeight(candidate)
  THEN FALSE
  ELSE InitialPending(candidate)

SpecPendingMapAfter(candidate) ==
  IF Fresh(candidate) /\ CurrentHeight(candidate)
  THEN FALSE
  ELSE InitialPending(candidate)

SpecPhaseAfter(candidate) ==
  IF Fresh(candidate) /\ CurrentHeight(candidate)
  THEN "Proposal"
  ELSE InitialPhase(candidate)

ImplementationRecords(candidate) ==
  Fresh(candidate) /\ ~BugSkipFreshRecord

ImplementationValidationAfter(candidate) ==
  IF Fresh(candidate) /\ CurrentHeight(candidate) /\ InitialValidating(candidate)
  THEN BugSkipCurrentValidationClear
  ELSE IF Fresh(candidate) /\ ~CurrentHeight(candidate) /\ InitialValidating(candidate)
       THEN ~BugCleanupOtherHeight
       ELSE IF Duplicate(candidate) /\ InitialValidating(candidate)
            THEN ~BugDuplicateCleansValidation
            ELSE IF Conflict(candidate) /\ InitialValidating(candidate)
                 THEN ~BugConflictCleansValidation
                 ELSE InitialValidating(candidate)

ImplementationPendingAfter(candidate) ==
  IF Fresh(candidate) /\ CurrentHeight(candidate) /\ InitialPending(candidate)
  THEN BugSkipCurrentPendingClear
  ELSE IF Fresh(candidate) /\ ~CurrentHeight(candidate) /\ InitialPending(candidate)
       THEN ~BugCleanupOtherHeight
       ELSE IF Duplicate(candidate) /\ InitialPending(candidate)
            THEN ~BugDuplicateClearsPending
            ELSE IF Conflict(candidate) /\ InitialPending(candidate)
                 THEN ~BugConflictClearsPending
                 ELSE InitialPending(candidate)

ImplementationPendingMapAfter(candidate) ==
  IF Fresh(candidate) /\ CurrentHeight(candidate) /\ InitialPending(candidate)
  THEN BugSkipCurrentPendingMapRemove
  ELSE IF Fresh(candidate) /\ ~CurrentHeight(candidate) /\ InitialPending(candidate)
       THEN ~BugCleanupOtherHeight
       ELSE IF Duplicate(candidate) /\ InitialPending(candidate)
            THEN ~BugDuplicateClearsPending
            ELSE IF Conflict(candidate) /\ InitialPending(candidate)
                 THEN ~BugConflictClearsPending
                 ELSE InitialPending(candidate)

ImplementationPhaseAfter(candidate) ==
  IF Fresh(candidate) /\ CurrentHeight(candidate)
  THEN
    IF BugWrongPhaseAfterCurrentCommit
    THEN InitialPhase(candidate)
    ELSE "Proposal"
  ELSE IF Fresh(candidate) /\ ~CurrentHeight(candidate) /\ BugCleanupOtherHeight
       THEN "Proposal"
       ELSE InitialPhase(candidate)

ImplementationEmitsCommitBlock(candidate) ==
  BugEmitCommitBlock

TypeInvariant ==
  /\ BugSkipFreshRecord \in BOOLEAN
  /\ BugSkipCurrentValidationClear \in BOOLEAN
  /\ BugSkipCurrentPendingClear \in BOOLEAN
  /\ BugSkipCurrentPendingMapRemove \in BOOLEAN
  /\ BugWrongPhaseAfterCurrentCommit \in BOOLEAN
  /\ BugCleanupOtherHeight \in BOOLEAN
  /\ BugDuplicateCleansValidation \in BOOLEAN
  /\ BugDuplicateClearsPending \in BOOLEAN
  /\ BugConflictCleansValidation \in BOOLEAN
  /\ BugConflictClearsPending \in BOOLEAN
  /\ BugEmitCommitBlock \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialPhase(candidate) \in Phases
    /\ SpecPhaseAfter(candidate) \in Phases
    /\ ImplementationPhaseAfter(candidate) \in Phases

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

RecordsMatchSpec ==
  \A candidate \in tried:
    ImplementationRecords(candidate) = SpecRecords(candidate)

CleanupMatchesSpec ==
  \A candidate \in tried:
    /\ ImplementationValidationAfter(candidate) = SpecValidationAfter(candidate)
    /\ ImplementationPendingAfter(candidate) = SpecPendingAfter(candidate)
    /\ ImplementationPendingMapAfter(candidate) = SpecPendingMapAfter(candidate)
    /\ ImplementationPhaseAfter(candidate) = SpecPhaseAfter(candidate)

FreshCurrentNotificationsCleanup ==
  \A candidate \in tried:
    (Fresh(candidate) /\ CurrentHeight(candidate)) =>
      /\ ImplementationRecords(candidate)
      /\ ImplementationValidationAfter(candidate) = FALSE
      /\ ImplementationPendingAfter(candidate) = FALSE
      /\ ImplementationPendingMapAfter(candidate) = FALSE
      /\ ImplementationPhaseAfter(candidate) = "Proposal"

OtherHeightNotificationsPreserveOwnership ==
  \A candidate \in tried:
    (Fresh(candidate) /\ ~CurrentHeight(candidate)) =>
      /\ ImplementationValidationAfter(candidate) = InitialValidating(candidate)
      /\ ImplementationPendingAfter(candidate) = InitialPending(candidate)
      /\ ImplementationPendingMapAfter(candidate) = InitialPending(candidate)
      /\ ImplementationPhaseAfter(candidate) = InitialPhase(candidate)

DuplicateNotificationsAreNoops ==
  \A candidate \in tried:
    Duplicate(candidate) =>
      /\ ~ImplementationRecords(candidate)
      /\ ImplementationValidationAfter(candidate) = InitialValidating(candidate)
      /\ ImplementationPendingAfter(candidate) = InitialPending(candidate)
      /\ ImplementationPendingMapAfter(candidate) = InitialPending(candidate)
      /\ ImplementationPhaseAfter(candidate) = InitialPhase(candidate)

ConflictingNotificationsAreNoops ==
  \A candidate \in tried:
    Conflict(candidate) =>
      /\ ~ImplementationRecords(candidate)
      /\ ImplementationValidationAfter(candidate) = InitialValidating(candidate)
      /\ ImplementationPendingAfter(candidate) = InitialPending(candidate)
      /\ ImplementationPendingMapAfter(candidate) = InitialPending(candidate)
      /\ ImplementationPhaseAfter(candidate) = InitialPhase(candidate)

PendingStateAndMapStayAligned ==
  \A candidate \in tried:
    ImplementationPendingAfter(candidate) = ImplementationPendingMapAfter(candidate)

NoCommitBlockOutput ==
  \A candidate \in tried:
    ~ImplementationEmitsCommitBlock(candidate)

Safety ==
  /\ RecordsMatchSpec
  /\ CleanupMatchesSpec
  /\ FreshCurrentNotificationsCleanup
  /\ OtherHeightNotificationsPreserveOwnership
  /\ DuplicateNotificationsAreNoops
  /\ ConflictingNotificationsAreNoops
  /\ PendingStateAndMapStayAligned
  /\ NoCommitBlockOutput

=============================================================================
