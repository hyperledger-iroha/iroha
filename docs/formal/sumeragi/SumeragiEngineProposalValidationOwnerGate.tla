---- MODULE SumeragiEngineProposalValidationOwnerGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact proposal validation-owner recording.

This slice models the `self.validating = Some(proposal.subject)` side effect in
`ConsensusEngine::on_proposal(...)`. Accepted proposals must set the validation
owner to exactly the accepted proposal subject, overwriting any stale owner
left from earlier work. Rejected proposals, including wrong phase/round,
incompatible highest-QC, and lock-conflict cases, must preserve the preexisting
owner state exactly.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipOwnerRecord,
  \* @type: Bool;
  BugKeepExistingOwner,
  \* @type: Bool;
  BugRecordWrongSubject,
  \* @type: Bool;
  BugRecordLockedSubject,
  \* @type: Bool;
  BugClearOnRejected,
  \* @type: Bool;
  BugReplaceOnRejected,
  \* @type: Bool;
  BugSetOwnerOnRejectedNone

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Candidates == {
  "safe_unlocked_none",
  "safe_locked_subject_existing",
  "safe_conflict_higher_qc_existing",
  "wrong_phase_existing",
  "wrong_round_existing",
  "incompatible_highest_existing",
  "locked_conflict_no_qc_existing",
  "locked_conflict_equal_qc_none",
  "locked_conflict_lower_qc_existing"
}

OwnerValues == {
  "none",
  "subject_a",
  "subject_b",
  "subject_existing",
  "subject_wrong"
}

Accepted(candidate) ==
  candidate \in {
    "safe_unlocked_none",
    "safe_locked_subject_existing",
    "safe_conflict_higher_qc_existing"
  }

InitialOwner(candidate) ==
  IF candidate \in {
    "safe_unlocked_none",
    "locked_conflict_equal_qc_none"
  }
  THEN "none"
  ELSE "subject_existing"

ProposalSubject(candidate) ==
  IF candidate \in {
    "safe_conflict_higher_qc_existing",
    "locked_conflict_no_qc_existing",
    "locked_conflict_equal_qc_none",
    "locked_conflict_lower_qc_existing"
  }
  THEN "subject_b"
  ELSE "subject_a"

SpecFinalOwner(candidate) ==
  IF Accepted(candidate)
  THEN ProposalSubject(candidate)
  ELSE InitialOwner(candidate)

ImplementationAcceptedOwner(candidate) ==
  IF BugSkipOwnerRecord
  THEN InitialOwner(candidate)
  ELSE IF BugKeepExistingOwner /\ InitialOwner(candidate) # "none"
       THEN InitialOwner(candidate)
       ELSE IF BugRecordWrongSubject
            THEN "subject_wrong"
            ELSE IF BugRecordLockedSubject
                 /\ candidate = "safe_conflict_higher_qc_existing"
                 THEN "subject_a"
                 ELSE ProposalSubject(candidate)

ImplementationRejectedOwner(candidate) ==
  IF BugClearOnRejected /\ InitialOwner(candidate) # "none"
  THEN "none"
  ELSE IF BugReplaceOnRejected /\ InitialOwner(candidate) # "none"
       THEN "subject_wrong"
       ELSE IF BugSetOwnerOnRejectedNone /\ InitialOwner(candidate) = "none"
            THEN "subject_wrong"
            ELSE InitialOwner(candidate)

ImplementationFinalOwner(candidate) ==
  IF Accepted(candidate)
  THEN ImplementationAcceptedOwner(candidate)
  ELSE ImplementationRejectedOwner(candidate)

TypeInvariant ==
  /\ BugSkipOwnerRecord \in BOOLEAN
  /\ BugKeepExistingOwner \in BOOLEAN
  /\ BugRecordWrongSubject \in BOOLEAN
  /\ BugRecordLockedSubject \in BOOLEAN
  /\ BugClearOnRejected \in BOOLEAN
  /\ BugReplaceOnRejected \in BOOLEAN
  /\ BugSetOwnerOnRejectedNone \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ \A candidate \in tried:
    /\ InitialOwner(candidate) \in OwnerValues
    /\ ProposalSubject(candidate) \in OwnerValues
    /\ SpecFinalOwner(candidate) \in OwnerValues
    /\ ImplementationFinalOwner(candidate) \in OwnerValues

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

FinalOwnerMatchesSpec ==
  \A candidate \in tried:
    ImplementationFinalOwner(candidate) = SpecFinalOwner(candidate)

AcceptedProposalsRecordExactSubject ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationFinalOwner(candidate) = ProposalSubject(candidate)

AcceptedProposalsOverwriteExistingOwner ==
  \A candidate \in tried:
    /\ Accepted(candidate)
    /\ InitialOwner(candidate) # "none"
    =>
      /\ ImplementationFinalOwner(candidate) # InitialOwner(candidate)
      /\ ImplementationFinalOwner(candidate) = ProposalSubject(candidate)

RejectedProposalsPreserveOwner ==
  \A candidate \in tried:
    ~Accepted(candidate) =>
      ImplementationFinalOwner(candidate) = InitialOwner(candidate)

RejectedExistingOwnerPreserved ==
  \A candidate \in tried:
    /\ ~Accepted(candidate)
    /\ InitialOwner(candidate) # "none"
    =>
      ImplementationFinalOwner(candidate) = "subject_existing"

RejectedNoneOwnerPreserved ==
  \A candidate \in tried:
    /\ ~Accepted(candidate)
    /\ InitialOwner(candidate) = "none"
    =>
      ImplementationFinalOwner(candidate) = "none"

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialOwner(candidate) \in OwnerValues
    /\ ProposalSubject(candidate) \in OwnerValues
    /\ SpecFinalOwner(candidate) \in OwnerValues
    /\ ImplementationFinalOwner(candidate) \in OwnerValues

Safety ==
  /\ FinalOwnerMatchesSpec
  /\ AcceptedProposalsRecordExactSubject
  /\ AcceptedProposalsOverwriteExistingOwner
  /\ RejectedProposalsPreserveOwner
  /\ RejectedExistingOwnerPreserved
  /\ RejectedNoneOwnerPreserved
  /\ ValuesStayInDomain

=============================================================================
====
