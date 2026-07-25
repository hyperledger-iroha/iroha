---- MODULE SumeragiV2LockedBodyReproposalMutation ----
EXTENDS Integers

(***************************************************************************
Bounded regression kernel for later-view locked-body proposal admission.

`FixedHigh` projects the exact installed TC high and admits the unchanged
locked subject.  `NoHighOnly` reproduces the old justification bug and rejects
that proposal.  `FixedConflict` keeps the exact TC projection but rejects an
equal-rank different subject.  `EqualRankConflict` weakens the safe-value
comparison from strict to non-strict and therefore accepts the conflicting
retarget.  These are structural mutation witnesses, not a temporal liveness
proof for the full asynchronous model.
***************************************************************************)

CONSTANT Mode

VARIABLES phase, accepted

vars == <<phase, accepted>>

NoRank == -1
NoSubject == "None"
LockedRank == 1
LockedSubject == "A"
ConflictingSubject == "B"
LaterView == 2

FixedProposalJustified(proposalView, proposalJustifyRank,
                       proposalJustifySubject,
                       installedHighRank, installedHighSubject) ==
  /\ proposalView = LaterView
  /\ proposalJustifyRank = installedHighRank
  /\ proposalJustifySubject = installedHighSubject

NoHighOnlyProposalJustified(proposalView, proposalJustifyRank,
                            proposalJustifySubject,
                            installedHighRank, installedHighSubject) ==
  /\ proposalView = LaterView
  /\ installedHighRank = NoRank
  /\ installedHighSubject = NoSubject
  /\ proposalJustifyRank = NoRank
  /\ proposalJustifySubject = NoSubject

FixedSafeToPrepare(proposalSubject, proposalJustifyRank,
                   proposalJustifySubject) ==
  \/ proposalSubject = LockedSubject
  \/ /\ proposalJustifyRank > LockedRank
     /\ proposalJustifySubject = proposalSubject

EqualRankSafeToPrepare(proposalSubject, proposalJustifyRank,
                       proposalJustifySubject) ==
  \/ proposalSubject = LockedSubject
  \/ /\ proposalJustifyRank >= LockedRank
     /\ proposalJustifySubject = proposalSubject

HighReproposalAccepted ==
  LET justified ==
        IF Mode = "NoHighOnly"
        THEN NoHighOnlyProposalJustified(
               LaterView, LockedRank, LockedSubject,
               LockedRank, LockedSubject)
        ELSE FixedProposalJustified(
               LaterView, LockedRank, LockedSubject,
               LockedRank, LockedSubject)
  IN /\ justified
     /\ FixedSafeToPrepare(LockedSubject, LockedRank, LockedSubject)

EqualRankConflictAccepted ==
  LET safe ==
        IF Mode = "EqualRankConflict"
        THEN EqualRankSafeToPrepare(
               ConflictingSubject, LockedRank, ConflictingSubject)
        ELSE FixedSafeToPrepare(
               ConflictingSubject, LockedRank, ConflictingSubject)
  IN /\ FixedProposalJustified(
           LaterView, LockedRank, ConflictingSubject,
           LockedRank, ConflictingSubject)
     /\ safe

Init ==
  /\ phase = 0
  /\ accepted = FALSE

Attempt ==
  /\ phase = 0
  /\ phase' = 1
  /\ accepted' =
       IF Mode \in {"FixedHigh", "NoHighOnly"}
       THEN HighReproposalAccepted
       ELSE EqualRankConflictAccepted

Next == Attempt \/ UNCHANGED vars

Spec == Init /\ [][Next]_vars

TypeInvariant ==
  /\ phase \in 0..1
  /\ accepted \in BOOLEAN

LaterHighReproposalAccepted ==
  phase = 0 \/ accepted

EqualRankConflictRejected ==
  phase = 0 \/ ~accepted

=============================================================================
