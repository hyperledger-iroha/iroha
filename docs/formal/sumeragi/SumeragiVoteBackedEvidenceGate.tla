---- MODULE SumeragiVoteBackedEvidenceGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the vote-backed consensus evidence helpers:

* `slot_has_vote_backed_consensus_evidence(height, view)`
* `slot_has_locally_known_vote_backed_consensus_evidence(height, view)`
* `height_has_vote_backed_consensus_evidence(height)`

Only Prepare/Commit votes or QCs at the expected height/epoch count as
vote-backed evidence. Slot-scoped helpers also require the queried view. The
locally-known slot helper additionally requires the referenced block to be
known locally, while the height-scoped helper deliberately ignores view.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoEvidence == "no_evidence"
SlotPrepareVoteKnown == "slot_prepare_vote_known"
SlotCommitVoteUnknown == "slot_commit_vote_unknown"
SlotPrepareQcKnown == "slot_prepare_qc_known"
SlotCommitQcUnknown == "slot_commit_qc_unknown"
WrongHeightVote == "wrong_height_vote"
WrongViewVote == "wrong_view_vote"
WrongEpochVote == "wrong_epoch_vote"
PrevoteVote == "prevote_vote"
NewViewQc == "new_view_qc"
HeightDifferentViewVote == "height_different_view_vote"
HeightDifferentViewQc == "height_different_view_qc"

Cases == {
  NoEvidence,
  SlotPrepareVoteKnown,
  SlotCommitVoteUnknown,
  SlotPrepareQcKnown,
  SlotCommitQcUnknown,
  WrongHeightVote,
  WrongViewVote,
  WrongEpochVote,
  PrevoteVote,
  NewViewQc,
  HeightDifferentViewVote,
  HeightDifferentViewQc
}

VoteCases == {
  SlotPrepareVoteKnown,
  SlotCommitVoteUnknown,
  WrongHeightVote,
  WrongViewVote,
  WrongEpochVote,
  PrevoteVote,
  HeightDifferentViewVote
}

QcCases == {
  SlotPrepareQcKnown,
  SlotCommitQcUnknown,
  NewViewQc,
  HeightDifferentViewQc
}

PrepareOrCommitPhaseCases == {
  SlotPrepareVoteKnown,
  SlotCommitVoteUnknown,
  SlotPrepareQcKnown,
  SlotCommitQcUnknown,
  WrongHeightVote,
  WrongViewVote,
  WrongEpochVote,
  HeightDifferentViewVote,
  HeightDifferentViewQc
}

CorrectHeightCases == Cases \ {NoEvidence, WrongHeightVote}
CorrectViewCases == Cases \ {NoEvidence, WrongViewVote,
  HeightDifferentViewVote, HeightDifferentViewQc}
CorrectEpochCases == Cases \ {NoEvidence, WrongEpochVote}

LocallyKnownBlockCases == {
  SlotPrepareVoteKnown,
  SlotPrepareQcKnown,
  HeightDifferentViewVote
}

HasEvidenceSource(c) ==
  c \in VoteCases \/ c \in QcCases

SpecSlotEvidence(c) ==
  HasEvidenceSource(c)
    /\ c \in PrepareOrCommitPhaseCases
    /\ c \in CorrectHeightCases
    /\ c \in CorrectViewCases
    /\ c \in CorrectEpochCases

SpecLocalSlotEvidence(c) ==
  SpecSlotEvidence(c) /\ c \in LocallyKnownBlockCases

SpecHeightEvidence(c) ==
  HasEvidenceSource(c)
    /\ c \in PrepareOrCommitPhaseCases
    /\ c \in CorrectHeightCases
    /\ c \in CorrectEpochCases

ImplementationSlotEvidence(c) ==
  CASE Bug = "reject_slot_prepare_vote"
       /\ c = SlotPrepareVoteKnown ->
      FALSE
    [] Bug = "reject_slot_commit_vote"
       /\ c = SlotCommitVoteUnknown ->
      FALSE
    [] Bug = "reject_slot_prepare_qc"
       /\ c = SlotPrepareQcKnown ->
      FALSE
    [] Bug = "reject_slot_commit_qc"
       /\ c = SlotCommitQcUnknown ->
      FALSE
    [] Bug = "accept_slot_wrong_height"
       /\ c = WrongHeightVote ->
      TRUE
    [] Bug = "accept_slot_wrong_view"
       /\ c = WrongViewVote ->
      TRUE
    [] Bug = "accept_slot_wrong_epoch"
       /\ c = WrongEpochVote ->
      TRUE
    [] Bug = "accept_slot_prevote"
       /\ c = PrevoteVote ->
      TRUE
    [] Bug = "accept_slot_new_view_qc"
       /\ c = NewViewQc ->
      TRUE
    [] OTHER -> SpecSlotEvidence(c)

ImplementationLocalSlotEvidence(c) ==
  CASE Bug = "local_accept_unknown_vote"
       /\ c = SlotCommitVoteUnknown ->
      TRUE
    [] Bug = "local_accept_unknown_qc"
       /\ c = SlotCommitQcUnknown ->
      TRUE
    [] Bug = "local_reject_known_vote"
       /\ c = SlotPrepareVoteKnown ->
      FALSE
    [] Bug = "local_reject_known_qc"
       /\ c = SlotPrepareQcKnown ->
      FALSE
    [] OTHER -> SpecLocalSlotEvidence(c)

ImplementationHeightEvidence(c) ==
  CASE Bug = "height_reject_different_view_vote"
       /\ c = HeightDifferentViewVote ->
      FALSE
    [] Bug = "height_reject_different_view_qc"
       /\ c = HeightDifferentViewQc ->
      FALSE
    [] Bug = "height_accept_wrong_height"
       /\ c = WrongHeightVote ->
      TRUE
    [] Bug = "height_accept_wrong_epoch"
       /\ c = WrongEpochVote ->
      TRUE
    [] Bug = "height_accept_prevote"
       /\ c = PrevoteVote ->
      TRUE
    [] Bug = "height_accept_new_view_qc"
       /\ c = NewViewQc ->
      TRUE
    [] OTHER -> SpecHeightEvidence(c)

Bugs == {
  "none",
  "reject_slot_prepare_vote",
  "reject_slot_commit_vote",
  "reject_slot_prepare_qc",
  "reject_slot_commit_qc",
  "accept_slot_wrong_height",
  "accept_slot_wrong_view",
  "accept_slot_wrong_epoch",
  "accept_slot_prevote",
  "accept_slot_new_view_qc",
  "local_accept_unknown_vote",
  "local_accept_unknown_qc",
  "local_reject_known_vote",
  "local_reject_known_qc",
  "height_reject_different_view_vote",
  "height_reject_different_view_qc",
  "height_accept_wrong_height",
  "height_accept_wrong_epoch",
  "height_accept_prevote",
  "height_accept_new_view_qc"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecSlotEvidence(c) \in BOOLEAN
       /\ SpecLocalSlotEvidence(c) \in BOOLEAN
       /\ SpecHeightEvidence(c) \in BOOLEAN
       /\ ImplementationSlotEvidence(c) \in BOOLEAN
       /\ ImplementationLocalSlotEvidence(c) \in BOOLEAN
       /\ ImplementationHeightEvidence(c) \in BOOLEAN

SlotEvidenceMatchesSpec ==
  \A c \in Cases:
    ImplementationSlotEvidence(c) = SpecSlotEvidence(c)

LocalSlotEvidenceMatchesSpec ==
  \A c \in Cases:
    ImplementationLocalSlotEvidence(c) = SpecLocalSlotEvidence(c)

HeightEvidenceMatchesSpec ==
  \A c \in Cases:
    ImplementationHeightEvidence(c) = SpecHeightEvidence(c)

SlotSourcesAccepted ==
  /\ ImplementationSlotEvidence(SlotPrepareVoteKnown)
  /\ ImplementationSlotEvidence(SlotCommitVoteUnknown)
  /\ ImplementationSlotEvidence(SlotPrepareQcKnown)
  /\ ImplementationSlotEvidence(SlotCommitQcUnknown)

SlotRejectsWrongShape ==
  /\ ~ImplementationSlotEvidence(WrongHeightVote)
  /\ ~ImplementationSlotEvidence(WrongViewVote)
  /\ ~ImplementationSlotEvidence(WrongEpochVote)
  /\ ~ImplementationSlotEvidence(PrevoteVote)
  /\ ~ImplementationSlotEvidence(NewViewQc)
  /\ ~ImplementationSlotEvidence(NoEvidence)

LocalSlotRequiresKnownBlock ==
  /\ ImplementationLocalSlotEvidence(SlotPrepareVoteKnown)
  /\ ImplementationLocalSlotEvidence(SlotPrepareQcKnown)
  /\ ~ImplementationLocalSlotEvidence(SlotCommitVoteUnknown)
  /\ ~ImplementationLocalSlotEvidence(SlotCommitQcUnknown)

HeightIgnoresViewButNotHeightEpochOrPhase ==
  /\ ImplementationHeightEvidence(SlotPrepareVoteKnown)
  /\ ImplementationHeightEvidence(SlotCommitVoteUnknown)
  /\ ImplementationHeightEvidence(SlotPrepareQcKnown)
  /\ ImplementationHeightEvidence(SlotCommitQcUnknown)
  /\ ImplementationHeightEvidence(HeightDifferentViewVote)
  /\ ImplementationHeightEvidence(HeightDifferentViewQc)
  /\ ~ImplementationHeightEvidence(WrongHeightVote)
  /\ ~ImplementationHeightEvidence(WrongEpochVote)
  /\ ~ImplementationHeightEvidence(PrevoteVote)
  /\ ~ImplementationHeightEvidence(NewViewQc)
  /\ ~ImplementationHeightEvidence(NoEvidence)

VoteBackedEvidenceCoreSafety ==
  /\ SlotEvidenceMatchesSpec
  /\ LocalSlotEvidenceMatchesSpec
  /\ HeightEvidenceMatchesSpec
  /\ SlotSourcesAccepted
  /\ SlotRejectsWrongShape
  /\ LocalSlotRequiresKnownBlock
  /\ HeightIgnoresViewButNotHeightEpochOrPhase

NoBugInvariant == VoteBackedEvidenceCoreSafety

SafetyFast == VoteBackedEvidenceCoreSafety

====
