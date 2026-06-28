---- MODULE SumeragiEnginePreparePhaseGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact Prepare-QC phase transitions.

This slice models the `state.phase = EnginePhase::Commit` side effect in
`ConsensusEngine::on_prepare_qc(...)`, including the shared
`on_certificate(...)` prefilter, the prepare-vote replay/conflict guard, and
the pending-finality guard. Every accepted fresh Prepare QC must leave the
engine in `Commit` phase. Rejected Prepare QCs, replayed/conflicting
same-round Prepare QCs, and pending-finality returns preserve the phase that
was present before the certificate was handled.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipCommitPhase,
  \* @type: Bool;
  BugWrongAcceptedPhase,
  \* @type: Bool;
  BugCommitOnRejected,
  \* @type: Bool;
  BugWrongPhaseOnRejected,
  \* @type: Bool;
  BugCommitOnReplayConflict,
  \* @type: Bool;
  BugWrongPhaseOnReplayConflict,
  \* @type: Bool;
  BugCommitOnPendingFinality,
  \* @type: Bool;
  BugWrongPhaseOnPendingFinality

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "safe_from_proposal",
  "safe_from_prepare",
  "safe_from_commit",
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

PhaseValues == {
  "phase_proposal",
  "phase_prepare",
  "phase_commit",
  "phase_pending_finality",
  "phase_wrong"
}

Accepted(candidate) ==
  candidate \in {
    "safe_from_proposal",
    "safe_from_prepare",
    "safe_from_commit"
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

ReplayConflict(candidate) ==
  candidate \in {"replay_same_prepare", "conflicting_prepare"}

PendingFinality(candidate) ==
  candidate = "pending_finality"

InitialPhase(candidate) ==
  CASE candidate = "safe_from_proposal" -> "phase_proposal"
    [] candidate = "safe_from_prepare" -> "phase_prepare"
    [] candidate = "safe_from_commit" -> "phase_commit"
    [] candidate \in {"wrong_height", "wrong_quorum_policy"} -> "phase_proposal"
    [] candidate \in {"wrong_epoch", "stale_view", "conflicting_prepare"} -> "phase_prepare"
    [] candidate \in {"wrong_validator_set", "committed_height", "replay_same_prepare"} -> "phase_commit"
    [] candidate = "pending_finality" -> "phase_pending_finality"
    [] OTHER -> "phase_wrong"

SpecFinalPhase(candidate) ==
  IF Accepted(candidate)
  THEN "phase_commit"
  ELSE InitialPhase(candidate)

ImplementationAcceptedPhase(candidate) ==
  IF BugSkipCommitPhase
  THEN InitialPhase(candidate)
  ELSE IF BugWrongAcceptedPhase
       THEN "phase_wrong"
       ELSE "phase_commit"

ImplementationRejectedPhase(candidate) ==
  IF RejectedPrefilter(candidate)
  THEN
    IF BugWrongPhaseOnRejected
    THEN "phase_wrong"
    ELSE IF BugCommitOnRejected
         THEN "phase_commit"
         ELSE InitialPhase(candidate)
  ELSE IF ReplayConflict(candidate)
       THEN
         IF BugWrongPhaseOnReplayConflict
         THEN "phase_wrong"
         ELSE IF BugCommitOnReplayConflict
              THEN "phase_commit"
              ELSE InitialPhase(candidate)
       ELSE IF PendingFinality(candidate)
            THEN
              IF BugWrongPhaseOnPendingFinality
              THEN "phase_wrong"
              ELSE IF BugCommitOnPendingFinality
                   THEN "phase_commit"
                   ELSE InitialPhase(candidate)
            ELSE InitialPhase(candidate)

ImplementationFinalPhase(candidate) ==
  IF Accepted(candidate)
  THEN ImplementationAcceptedPhase(candidate)
  ELSE ImplementationRejectedPhase(candidate)

TypeInvariant ==
  /\ BugSkipCommitPhase \in BOOLEAN
  /\ BugWrongAcceptedPhase \in BOOLEAN
  /\ BugCommitOnRejected \in BOOLEAN
  /\ BugWrongPhaseOnRejected \in BOOLEAN
  /\ BugCommitOnReplayConflict \in BOOLEAN
  /\ BugWrongPhaseOnReplayConflict \in BOOLEAN
  /\ BugCommitOnPendingFinality \in BOOLEAN
  /\ BugWrongPhaseOnPendingFinality \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialPhase(candidate) \in PhaseValues
    /\ SpecFinalPhase(candidate) \in PhaseValues
    /\ ImplementationFinalPhase(candidate) \in PhaseValues

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

FinalPhaseMatchesSpec ==
  \A candidate \in tried:
    ImplementationFinalPhase(candidate) = SpecFinalPhase(candidate)

AcceptedPrepareMovesToCommitPhase ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationFinalPhase(candidate) = "phase_commit"

RejectedPrefilterPreservesPhase ==
  \A candidate \in tried:
    RejectedPrefilter(candidate) =>
      ImplementationFinalPhase(candidate) = InitialPhase(candidate)

ReplayConflictPreservesPhase ==
  \A candidate \in tried:
    ReplayConflict(candidate) =>
      ImplementationFinalPhase(candidate) = InitialPhase(candidate)

PendingFinalityPreservesPhase ==
  \A candidate \in tried:
    PendingFinality(candidate) =>
      ImplementationFinalPhase(candidate) = InitialPhase(candidate)

IgnoredPrepareQcsNeverChangePhase ==
  \A candidate \in tried:
    ~Accepted(candidate) =>
      ImplementationFinalPhase(candidate) = InitialPhase(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialPhase(candidate) \in PhaseValues
    /\ SpecFinalPhase(candidate) \in PhaseValues
    /\ ImplementationFinalPhase(candidate) \in PhaseValues

EnginePreparePhaseExactness ==
  /\ FinalPhaseMatchesSpec
  /\ AcceptedPrepareMovesToCommitPhase
  /\ RejectedPrefilterPreservesPhase
  /\ ReplayConflictPreservesPhase
  /\ PendingFinalityPreservesPhase
  /\ IgnoredPrepareQcsNeverChangePhase
  /\ ValuesStayInDomain

Safety ==
  EnginePreparePhaseExactness

EnginePreparePhaseCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EnginePreparePhaseExactness

SafetyFast == EnginePreparePhaseExactness

====
