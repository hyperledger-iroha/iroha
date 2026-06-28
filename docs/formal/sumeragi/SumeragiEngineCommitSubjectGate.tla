---- MODULE SumeragiEngineCommitSubjectGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for pure-engine finality side effects.

This slice models `ConsensusEngine::commit_subject(...)`. The helper must
refuse to mutate or emit finality when the current round height is already
committed to a conflicting block hash. Otherwise it records the subject hash at
the current height, clears pending-finality and validation ownership, returns
the engine to proposal phase, and emits exactly one `CommitBlock` output for
the subject.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipFreshRecord,
  \* @type: Bool;
  BugRejectMatchingCommitted,
  \* @type: Bool;
  BugKeepPendingFinality,
  \* @type: Bool;
  BugKeepValidation,
  \* @type: Bool;
  BugWrongPhaseAfterCommit,
  \* @type: Bool;
  BugSkipCommitOutput,
  \* @type: Bool;
  BugOverwriteConflict,
  \* @type: Bool;
  BugEmitOnConflict,
  \* @type: Bool;
  BugClearPendingOnConflict,
  \* @type: Bool;
  BugClearValidationOnConflict

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "fresh_clean",
  "fresh_pending_finality",
  "fresh_validating",
  "matching_committed",
  "conflict_clean",
  "conflict_pending_finality",
  "conflict_validating"
}

CommittedValues == {"none", "subject", "other"}
Phases == {"Prepare", "Commit", "PendingFinality", "Proposal"}

InitialCommitted(candidate) ==
  CASE candidate = "matching_committed" -> "subject"
    [] candidate \in {"conflict_clean", "conflict_pending_finality", "conflict_validating"} -> "other"
    [] OTHER -> "none"

InitialPendingFinality(candidate) ==
  candidate \in {"fresh_pending_finality", "conflict_pending_finality"}

InitialValidating(candidate) ==
  candidate \in {"fresh_validating", "conflict_validating"}

InitialPhase(candidate) ==
  CASE InitialPendingFinality(candidate) -> "PendingFinality"
    [] InitialValidating(candidate) -> "Prepare"
    [] OTHER -> "Commit"

SpecCommits(candidate) ==
  InitialCommitted(candidate) # "other"

ImplementationCommits(candidate) ==
  IF SpecCommits(candidate)
  THEN
    /\ ~(InitialCommitted(candidate) = "none" /\ BugSkipFreshRecord)
    /\ ~(InitialCommitted(candidate) = "subject" /\ BugRejectMatchingCommitted)
  ELSE
    BugOverwriteConflict

ImplementationCommitted(candidate) ==
  IF ImplementationCommits(candidate)
  THEN "subject"
  ELSE InitialCommitted(candidate)

ImplementationPendingFinality(candidate) ==
  IF ImplementationCommits(candidate)
  THEN
    IF BugKeepPendingFinality
    THEN InitialPendingFinality(candidate)
    ELSE FALSE
  ELSE
    IF BugClearPendingOnConflict
    THEN FALSE
    ELSE InitialPendingFinality(candidate)

ImplementationValidating(candidate) ==
  IF ImplementationCommits(candidate)
  THEN
    IF BugKeepValidation
    THEN InitialValidating(candidate)
    ELSE FALSE
  ELSE
    IF BugClearValidationOnConflict
    THEN FALSE
    ELSE InitialValidating(candidate)

ImplementationPhase(candidate) ==
  IF ImplementationCommits(candidate)
  THEN
    IF BugWrongPhaseAfterCommit
    THEN "Commit"
    ELSE "Proposal"
  ELSE
    InitialPhase(candidate)

ImplementationEmitsCommitBlock(candidate) ==
  IF ImplementationCommits(candidate)
  THEN ~BugSkipCommitOutput
  ELSE BugEmitOnConflict

SpecCommitted(candidate) ==
  IF SpecCommits(candidate)
  THEN "subject"
  ELSE InitialCommitted(candidate)

SpecPendingFinality(candidate) ==
  IF SpecCommits(candidate)
  THEN FALSE
  ELSE InitialPendingFinality(candidate)

SpecValidating(candidate) ==
  IF SpecCommits(candidate)
  THEN FALSE
  ELSE InitialValidating(candidate)

SpecPhase(candidate) ==
  IF SpecCommits(candidate)
  THEN "Proposal"
  ELSE InitialPhase(candidate)

TypeInvariant ==
  /\ BugSkipFreshRecord \in BOOLEAN
  /\ BugRejectMatchingCommitted \in BOOLEAN
  /\ BugKeepPendingFinality \in BOOLEAN
  /\ BugKeepValidation \in BOOLEAN
  /\ BugWrongPhaseAfterCommit \in BOOLEAN
  /\ BugSkipCommitOutput \in BOOLEAN
  /\ BugOverwriteConflict \in BOOLEAN
  /\ BugEmitOnConflict \in BOOLEAN
  /\ BugClearPendingOnConflict \in BOOLEAN
  /\ BugClearValidationOnConflict \in BOOLEAN
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

CommitSubjectMatchesSpec ==
  \A candidate \in tried:
    /\ ImplementationCommitted(candidate) = SpecCommitted(candidate)
    /\ ImplementationPendingFinality(candidate) = SpecPendingFinality(candidate)
    /\ ImplementationValidating(candidate) = SpecValidating(candidate)
    /\ ImplementationPhase(candidate) = SpecPhase(candidate)
    /\ ImplementationEmitsCommitBlock(candidate) = SpecCommits(candidate)

FreshCommitsRecordSubject ==
  \A candidate \in tried:
    InitialCommitted(candidate) = "none" =>
      ImplementationCommitted(candidate) = "subject"

MatchingCommittedStillCommits ==
  "matching_committed" \in tried =>
    /\ ImplementationCommitted("matching_committed") = "subject"
    /\ ImplementationEmitsCommitBlock("matching_committed")

ConflictsNeverMutate ==
  \A candidate \in tried:
    InitialCommitted(candidate) = "other" =>
      /\ ImplementationCommitted(candidate) = "other"
      /\ ImplementationPendingFinality(candidate) = InitialPendingFinality(candidate)
      /\ ImplementationValidating(candidate) = InitialValidating(candidate)
      /\ ImplementationPhase(candidate) = InitialPhase(candidate)

ConflictsNeverEmit ==
  \A candidate \in tried:
    InitialCommitted(candidate) = "other" =>
      ~ImplementationEmitsCommitBlock(candidate)

SuccessfulCommitsClearPendingFinality ==
  \A candidate \in tried:
    SpecCommits(candidate) =>
      ~ImplementationPendingFinality(candidate)

SuccessfulCommitsClearValidation ==
  \A candidate \in tried:
    SpecCommits(candidate) =>
      ~ImplementationValidating(candidate)

SuccessfulCommitsReturnProposalPhase ==
  \A candidate \in tried:
    SpecCommits(candidate) =>
      ImplementationPhase(candidate) = "Proposal"

SuccessfulCommitsEmitExactlyOnce ==
  \A candidate \in tried:
    SpecCommits(candidate) =>
      ImplementationEmitsCommitBlock(candidate)

EngineCommitSubjectExactness ==
  /\ CommitSubjectMatchesSpec
  /\ FreshCommitsRecordSubject
  /\ MatchingCommittedStillCommits
  /\ ConflictsNeverMutate
  /\ ConflictsNeverEmit
  /\ SuccessfulCommitsClearPendingFinality
  /\ SuccessfulCommitsClearValidation
  /\ SuccessfulCommitsReturnProposalPhase
  /\ SuccessfulCommitsEmitExactlyOnce

Safety ==
  EngineCommitSubjectExactness

EngineCommitSubjectCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineCommitSubjectExactness

SafetyFast == EngineCommitSubjectExactness

====
