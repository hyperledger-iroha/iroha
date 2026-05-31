---- MODULE SumeragiEngineCommitQcPhaseGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact Commit-QC phase transitions.

This slice models the phase side effects in
`ConsensusEngine::on_commit_qc(...)`, including the shared
`on_certificate(...)` prefilter and pending-finality replay/conflict guard. A
payload-available Commit QC commits through `commit_subject(...)` and must
return the engine to `Proposal` phase. A missing-payload Commit QC must move
the engine to `PendingFinality`. Rejected Commit QCs and pending-finality
replay/conflict returns preserve the phase that existed before the certificate
was handled.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAvailableSkipsProposalPhase,
  \* @type: Bool;
  BugAvailableWrongPhase,
  \* @type: Bool;
  BugMissingSkipsPendingPhase,
  \* @type: Bool;
  BugMissingWrongPhase,
  \* @type: Bool;
  BugRejectedMovesProposal,
  \* @type: Bool;
  BugRejectedWrongPhase,
  \* @type: Bool;
  BugPendingReplayMovesProposal,
  \* @type: Bool;
  BugPendingConflictMovesProposal,
  \* @type: Bool;
  BugPendingWrongPhase

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "available_from_proposal",
  "available_from_prepare",
  "available_from_commit",
  "missing_from_proposal",
  "missing_from_prepare",
  "missing_from_commit",
  "wrong_height",
  "wrong_epoch",
  "wrong_validator_set",
  "wrong_quorum_policy",
  "stale_view",
  "committed_height",
  "pending_replay",
  "pending_conflict"
}

PhaseValues == {
  "phase_proposal",
  "phase_prepare",
  "phase_commit",
  "phase_pending_finality",
  "phase_wrong"
}

PayloadAvailable(candidate) ==
  candidate \in {
    "available_from_proposal",
    "available_from_prepare",
    "available_from_commit"
  }

PayloadMissing(candidate) ==
  candidate \in {
    "missing_from_proposal",
    "missing_from_prepare",
    "missing_from_commit"
  }

Accepted(candidate) ==
  PayloadAvailable(candidate) \/ PayloadMissing(candidate)

RejectedPrefilter(candidate) ==
  candidate \in {
    "wrong_height",
    "wrong_epoch",
    "wrong_validator_set",
    "wrong_quorum_policy",
    "stale_view",
    "committed_height"
  }

PendingReplay(candidate) ==
  candidate = "pending_replay"

PendingConflict(candidate) ==
  candidate = "pending_conflict"

InitialPhase(candidate) ==
  CASE candidate \in {"available_from_proposal", "missing_from_proposal", "wrong_height"} ->
      "phase_proposal"
    [] candidate \in {"available_from_prepare", "missing_from_prepare", "wrong_epoch",
                      "wrong_quorum_policy", "stale_view"} ->
      "phase_prepare"
    [] candidate \in {"available_from_commit", "missing_from_commit", "wrong_validator_set",
                      "committed_height"} ->
      "phase_commit"
    [] candidate \in {"pending_replay", "pending_conflict"} ->
      "phase_pending_finality"
    [] OTHER -> "phase_wrong"

SpecFinalPhase(candidate) ==
  IF PayloadAvailable(candidate)
  THEN "phase_proposal"
  ELSE IF PayloadMissing(candidate)
       THEN "phase_pending_finality"
       ELSE InitialPhase(candidate)

ImplementationAvailablePhase(candidate) ==
  IF BugAvailableSkipsProposalPhase
  THEN InitialPhase(candidate)
  ELSE IF BugAvailableWrongPhase
       THEN "phase_wrong"
       ELSE "phase_proposal"

ImplementationMissingPhase(candidate) ==
  IF BugMissingSkipsPendingPhase
  THEN InitialPhase(candidate)
  ELSE IF BugMissingWrongPhase
       THEN "phase_wrong"
       ELSE "phase_pending_finality"

ImplementationRejectedPhase(candidate) ==
  IF BugRejectedWrongPhase
  THEN "phase_wrong"
  ELSE IF BugRejectedMovesProposal
       THEN "phase_proposal"
       ELSE InitialPhase(candidate)

ImplementationPendingReplayPhase(candidate) ==
  IF BugPendingWrongPhase
  THEN "phase_wrong"
  ELSE IF BugPendingReplayMovesProposal
       THEN "phase_proposal"
       ELSE InitialPhase(candidate)

ImplementationPendingConflictPhase(candidate) ==
  IF BugPendingWrongPhase
  THEN "phase_wrong"
  ELSE IF BugPendingConflictMovesProposal
       THEN "phase_proposal"
       ELSE InitialPhase(candidate)

ImplementationFinalPhase(candidate) ==
  IF PayloadAvailable(candidate)
  THEN ImplementationAvailablePhase(candidate)
  ELSE IF PayloadMissing(candidate)
       THEN ImplementationMissingPhase(candidate)
       ELSE IF RejectedPrefilter(candidate)
            THEN ImplementationRejectedPhase(candidate)
            ELSE IF PendingReplay(candidate)
                 THEN ImplementationPendingReplayPhase(candidate)
                 ELSE IF PendingConflict(candidate)
                      THEN ImplementationPendingConflictPhase(candidate)
                      ELSE InitialPhase(candidate)

TypeInvariant ==
  /\ BugAvailableSkipsProposalPhase \in BOOLEAN
  /\ BugAvailableWrongPhase \in BOOLEAN
  /\ BugMissingSkipsPendingPhase \in BOOLEAN
  /\ BugMissingWrongPhase \in BOOLEAN
  /\ BugRejectedMovesProposal \in BOOLEAN
  /\ BugRejectedWrongPhase \in BOOLEAN
  /\ BugPendingReplayMovesProposal \in BOOLEAN
  /\ BugPendingConflictMovesProposal \in BOOLEAN
  /\ BugPendingWrongPhase \in BOOLEAN
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

AvailableCommitQcsReturnToProposalPhase ==
  \A candidate \in tried:
    PayloadAvailable(candidate) =>
      ImplementationFinalPhase(candidate) = "phase_proposal"

MissingPayloadCommitQcsEnterPendingFinalityPhase ==
  \A candidate \in tried:
    PayloadMissing(candidate) =>
      ImplementationFinalPhase(candidate) = "phase_pending_finality"

RejectedPrefilterPreservesPhase ==
  \A candidate \in tried:
    RejectedPrefilter(candidate) =>
      ImplementationFinalPhase(candidate) = InitialPhase(candidate)

PendingReplayPreservesPhase ==
  \A candidate \in tried:
    PendingReplay(candidate) =>
      ImplementationFinalPhase(candidate) = InitialPhase(candidate)

PendingConflictPreservesPhase ==
  \A candidate \in tried:
    PendingConflict(candidate) =>
      ImplementationFinalPhase(candidate) = InitialPhase(candidate)

IgnoredCommitQcsNeverChangePhase ==
  \A candidate \in tried:
    ~Accepted(candidate) =>
      ImplementationFinalPhase(candidate) = InitialPhase(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialPhase(candidate) \in PhaseValues
    /\ SpecFinalPhase(candidate) \in PhaseValues
    /\ ImplementationFinalPhase(candidate) \in PhaseValues

Safety ==
  /\ FinalPhaseMatchesSpec
  /\ AvailableCommitQcsReturnToProposalPhase
  /\ MissingPayloadCommitQcsEnterPendingFinalityPhase
  /\ RejectedPrefilterPreservesPhase
  /\ PendingReplayPreservesPhase
  /\ PendingConflictPreservesPhase
  /\ IgnoredCommitQcsNeverChangePhase
  /\ ValuesStayInDomain

====
