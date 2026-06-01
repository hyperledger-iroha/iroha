---- MODULE SumeragiEngineProposalStateGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact proposal state mutation.

This slice models the state fields changed by
`ConsensusEngine::on_proposal(...)`. Accepted proposals may only move the
engine phase from Proposal to Prepare; they must preserve the current round,
locked QC, highest QC, and pending-finality marker exactly. Rejected proposals
must preserve the whole modeled state exactly, including a non-Proposal phase
when the phase guard rejects the input.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptedStaysProposal,
  \* @type: Bool;
  BugAcceptedUsesCommitPhase,
  \* @type: Bool;
  BugAcceptedUpdatesRound,
  \* @type: Bool;
  BugAcceptedClearsLock,
  \* @type: Bool;
  BugAcceptedRecordsProposalHighest,
  \* @type: Bool;
  BugAcceptedClearsPending,
  \* @type: Bool;
  BugRejectedEntersPrepare,
  \* @type: Bool;
  BugRejectedUpdatesRound,
  \* @type: Bool;
  BugRejectedClearsLock,
  \* @type: Bool;
  BugRejectedRecordsProposalHighest,
  \* @type: Bool;
  BugRejectedClearsPending

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Candidates == {
  "safe_unlocked_pending",
  "safe_locked_subject",
  "safe_conflict_higher_qc",
  "wrong_phase_commit",
  "wrong_round",
  "incompatible_highest",
  "locked_conflict_no_qc",
  "locked_conflict_equal_qc",
  "locked_conflict_lower_qc"
}

Phases == {"Proposal", "Prepare", "Commit", "PendingFinality"}
Rounds == {"round_current", "round_wrong"}
Locks == {"none", "lock_subject_a"}
HighestQcs == {"none", "qc_low", "qc_equal", "qc_high", "qc_wrong"}
PendingFinality == {"none", "pending_subject_a"}

Accepted(candidate) ==
  candidate \in {
    "safe_unlocked_pending",
    "safe_locked_subject",
    "safe_conflict_higher_qc"
  }

InitialPhase(candidate) ==
  IF candidate = "wrong_phase_commit"
  THEN "Commit"
  ELSE "Proposal"

InitialRound(candidate) == "round_current"

ProposalRound(candidate) ==
  IF candidate = "wrong_round"
  THEN "round_wrong"
  ELSE "round_current"

InitialLockedQc(candidate) ==
  IF candidate \in {
    "safe_unlocked_pending",
    "wrong_phase_commit",
    "wrong_round",
    "incompatible_highest"
  }
  THEN "none"
  ELSE "lock_subject_a"

InitialHighestQc(candidate) ==
  IF candidate \in {"safe_conflict_higher_qc", "incompatible_highest"}
  THEN "qc_low"
  ELSE IF candidate = "locked_conflict_equal_qc"
       THEN "qc_equal"
       ELSE "none"

ProposalHighestQc(candidate) ==
  IF candidate = "safe_conflict_higher_qc"
  THEN "qc_high"
  ELSE IF candidate = "incompatible_highest"
       THEN "qc_wrong"
       ELSE IF candidate = "locked_conflict_equal_qc"
            THEN "qc_equal"
            ELSE IF candidate = "locked_conflict_lower_qc"
                 THEN "qc_low"
                 ELSE "none"

InitialPendingFinality(candidate) ==
  IF candidate \in {
    "safe_unlocked_pending",
    "wrong_phase_commit",
    "locked_conflict_no_qc"
  }
  THEN "pending_subject_a"
  ELSE "none"

SpecFinalPhase(candidate) ==
  IF Accepted(candidate)
  THEN "Prepare"
  ELSE InitialPhase(candidate)

ImplementationFinalPhase(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedStaysProposal
    THEN "Proposal"
    ELSE IF BugAcceptedUsesCommitPhase
         THEN "Commit"
         ELSE "Prepare"
  ELSE IF BugRejectedEntersPrepare
       THEN "Prepare"
       ELSE InitialPhase(candidate)

ImplementationFinalRound(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedUpdatesRound
    THEN "round_wrong"
    ELSE InitialRound(candidate)
  ELSE IF BugRejectedUpdatesRound
       THEN ProposalRound(candidate)
       ELSE InitialRound(candidate)

ImplementationFinalLockedQc(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedClearsLock
  THEN "none"
  ELSE IF ~Accepted(candidate) /\ BugRejectedClearsLock
       THEN "none"
       ELSE InitialLockedQc(candidate)

ImplementationFinalHighestQc(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedRecordsProposalHighest
  THEN ProposalHighestQc(candidate)
  ELSE IF ~Accepted(candidate) /\ BugRejectedRecordsProposalHighest
       THEN ProposalHighestQc(candidate)
       ELSE InitialHighestQc(candidate)

ImplementationFinalPendingFinality(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedClearsPending
  THEN "none"
  ELSE IF ~Accepted(candidate) /\ BugRejectedClearsPending
       THEN "none"
       ELSE InitialPendingFinality(candidate)

TypeInvariant ==
  /\ BugAcceptedStaysProposal \in BOOLEAN
  /\ BugAcceptedUsesCommitPhase \in BOOLEAN
  /\ BugAcceptedUpdatesRound \in BOOLEAN
  /\ BugAcceptedClearsLock \in BOOLEAN
  /\ BugAcceptedRecordsProposalHighest \in BOOLEAN
  /\ BugAcceptedClearsPending \in BOOLEAN
  /\ BugRejectedEntersPrepare \in BOOLEAN
  /\ BugRejectedUpdatesRound \in BOOLEAN
  /\ BugRejectedClearsLock \in BOOLEAN
  /\ BugRejectedRecordsProposalHighest \in BOOLEAN
  /\ BugRejectedClearsPending \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ \A candidate \in tried:
    /\ InitialPhase(candidate) \in Phases
    /\ SpecFinalPhase(candidate) \in Phases
    /\ ImplementationFinalPhase(candidate) \in Phases
    /\ InitialRound(candidate) \in Rounds
    /\ ProposalRound(candidate) \in Rounds
    /\ ImplementationFinalRound(candidate) \in Rounds
    /\ InitialLockedQc(candidate) \in Locks
    /\ ImplementationFinalLockedQc(candidate) \in Locks
    /\ InitialHighestQc(candidate) \in HighestQcs
    /\ ProposalHighestQc(candidate) \in HighestQcs
    /\ ImplementationFinalHighestQc(candidate) \in HighestQcs
    /\ InitialPendingFinality(candidate) \in PendingFinality
    /\ ImplementationFinalPendingFinality(candidate) \in PendingFinality

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

AcceptedPhaseIsPrepare ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationFinalPhase(candidate) = "Prepare"

AcceptedRoundPreserved ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationFinalRound(candidate) = InitialRound(candidate)

AcceptedLockPreserved ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationFinalLockedQc(candidate) = InitialLockedQc(candidate)

AcceptedHighestPreserved ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationFinalHighestQc(candidate) = InitialHighestQc(candidate)

AcceptedPendingFinalityPreserved ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationFinalPendingFinality(candidate) =
        InitialPendingFinality(candidate)

RejectedStatePreserved ==
  \A candidate \in tried:
    ~Accepted(candidate) =>
      /\ ImplementationFinalPhase(candidate) = InitialPhase(candidate)
      /\ ImplementationFinalRound(candidate) = InitialRound(candidate)
      /\ ImplementationFinalLockedQc(candidate) = InitialLockedQc(candidate)
      /\ ImplementationFinalHighestQc(candidate) = InitialHighestQc(candidate)
      /\ ImplementationFinalPendingFinality(candidate) =
        InitialPendingFinality(candidate)

RejectedPhasePreservedExactly ==
  \A candidate \in tried:
    ~Accepted(candidate) =>
      ImplementationFinalPhase(candidate) = InitialPhase(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialPhase(candidate) \in Phases
    /\ SpecFinalPhase(candidate) \in Phases
    /\ ImplementationFinalPhase(candidate) \in Phases
    /\ InitialRound(candidate) \in Rounds
    /\ ProposalRound(candidate) \in Rounds
    /\ ImplementationFinalRound(candidate) \in Rounds
    /\ InitialLockedQc(candidate) \in Locks
    /\ ImplementationFinalLockedQc(candidate) \in Locks
    /\ InitialHighestQc(candidate) \in HighestQcs
    /\ ProposalHighestQc(candidate) \in HighestQcs
    /\ ImplementationFinalHighestQc(candidate) \in HighestQcs
    /\ InitialPendingFinality(candidate) \in PendingFinality
    /\ ImplementationFinalPendingFinality(candidate) \in PendingFinality

Safety ==
  /\ AcceptedPhaseIsPrepare
  /\ AcceptedRoundPreserved
  /\ AcceptedLockPreserved
  /\ AcceptedHighestPreserved
  /\ AcceptedPendingFinalityPreserved
  /\ RejectedStatePreserved
  /\ RejectedPhasePreservedExactly
  /\ ValuesStayInDomain

=============================================================================
====
