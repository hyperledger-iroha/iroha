---- MODULE SumeragiActionableVoteBackedProposalGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for proposal-side vote-backed evidence helpers:

* `precommit_vote_blocks_proposal_assembly(...)`
* `slot_has_actionable_vote_backed_proposal_evidence(...)`

Precommit votes block same-slot proposal assembly only when they are Commit
votes with exact height/view/epoch and actionable payload material. Slot-level
proposal evidence accepts Prepare/Commit votes and QCs with exact height,
view, epoch, and actionable payload material.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PrecommitCommitActionable == "precommit_commit_actionable"
PrecommitPrepareActionable == "precommit_prepare_actionable"
PrecommitWrongHeight == "precommit_wrong_height"
PrecommitWrongView == "precommit_wrong_view"
PrecommitWrongEpoch == "precommit_wrong_epoch"
PrecommitCommitNonActionable == "precommit_commit_non_actionable"

SlotPrepareVoteActionable == "slot_prepare_vote_actionable"
SlotCommitVoteActionable == "slot_commit_vote_actionable"
SlotPrepareQcActionable == "slot_prepare_qc_actionable"
SlotCommitQcActionable == "slot_commit_qc_actionable"
SlotWrongHeightVote == "slot_wrong_height_vote"
SlotWrongViewVote == "slot_wrong_view_vote"
SlotWrongEpochVote == "slot_wrong_epoch_vote"
SlotPrevoteActionable == "slot_prevote_actionable"
SlotNewViewQcActionable == "slot_new_view_qc_actionable"
SlotPrepareVoteNonActionable == "slot_prepare_vote_non_actionable"
SlotPrepareQcNonActionable == "slot_prepare_qc_non_actionable"
NoSlotEvidence == "no_slot_evidence"

Cases == {
  PrecommitCommitActionable,
  PrecommitPrepareActionable,
  PrecommitWrongHeight,
  PrecommitWrongView,
  PrecommitWrongEpoch,
  PrecommitCommitNonActionable,
  SlotPrepareVoteActionable,
  SlotCommitVoteActionable,
  SlotPrepareQcActionable,
  SlotCommitQcActionable,
  SlotWrongHeightVote,
  SlotWrongViewVote,
  SlotWrongEpochVote,
  SlotPrevoteActionable,
  SlotNewViewQcActionable,
  SlotPrepareVoteNonActionable,
  SlotPrepareQcNonActionable,
  NoSlotEvidence
}

VoteCases == {
  PrecommitCommitActionable,
  PrecommitPrepareActionable,
  PrecommitWrongHeight,
  PrecommitWrongView,
  PrecommitWrongEpoch,
  PrecommitCommitNonActionable,
  SlotPrepareVoteActionable,
  SlotCommitVoteActionable,
  SlotWrongHeightVote,
  SlotWrongViewVote,
  SlotWrongEpochVote,
  SlotPrevoteActionable,
  SlotPrepareVoteNonActionable
}

QcCases == {
  SlotPrepareQcActionable,
  SlotCommitQcActionable,
  SlotNewViewQcActionable,
  SlotPrepareQcNonActionable
}

CommitVoteCases == {
  PrecommitCommitActionable,
  PrecommitWrongHeight,
  PrecommitWrongView,
  PrecommitWrongEpoch,
  PrecommitCommitNonActionable,
  SlotCommitVoteActionable
}

PrepareOrCommitCases == {
  PrecommitCommitActionable,
  PrecommitPrepareActionable,
  PrecommitWrongHeight,
  PrecommitWrongView,
  PrecommitWrongEpoch,
  PrecommitCommitNonActionable,
  SlotPrepareVoteActionable,
  SlotCommitVoteActionable,
  SlotPrepareQcActionable,
  SlotCommitQcActionable,
  SlotWrongHeightVote,
  SlotWrongViewVote,
  SlotWrongEpochVote,
  SlotPrepareVoteNonActionable,
  SlotPrepareQcNonActionable
}

CorrectHeightCases == Cases \ {PrecommitWrongHeight, SlotWrongHeightVote, NoSlotEvidence}
CorrectViewCases == Cases \ {PrecommitWrongView, SlotWrongViewVote, NoSlotEvidence}
CorrectEpochCases == Cases \ {PrecommitWrongEpoch, SlotWrongEpochVote, NoSlotEvidence}

ActionablePayloadCases == {
  PrecommitCommitActionable,
  PrecommitPrepareActionable,
  PrecommitWrongHeight,
  PrecommitWrongView,
  PrecommitWrongEpoch,
  SlotPrepareVoteActionable,
  SlotCommitVoteActionable,
  SlotPrepareQcActionable,
  SlotCommitQcActionable,
  SlotWrongHeightVote,
  SlotWrongViewVote,
  SlotWrongEpochVote,
  SlotPrevoteActionable,
  SlotNewViewQcActionable
}

SpecPrecommitBlocks(c) ==
  c \in VoteCases
    /\ c \in CommitVoteCases
    /\ c \in CorrectHeightCases
    /\ c \in CorrectViewCases
    /\ c \in CorrectEpochCases
    /\ c \in ActionablePayloadCases

SpecSlotEvidence(c) ==
  (c \in VoteCases \/ c \in QcCases)
    /\ c \in PrepareOrCommitCases
    /\ c \in CorrectHeightCases
    /\ c \in CorrectViewCases
    /\ c \in CorrectEpochCases
    /\ c \in ActionablePayloadCases

ImplementationPrecommitBlocks(c) ==
  CASE Bug = "reject_precommit_commit_actionable"
       /\ c = PrecommitCommitActionable ->
      FALSE
    [] Bug = "accept_precommit_prepare"
       /\ c = PrecommitPrepareActionable ->
      TRUE
    [] Bug = "accept_precommit_wrong_height"
       /\ c = PrecommitWrongHeight ->
      TRUE
    [] Bug = "accept_precommit_wrong_view"
       /\ c = PrecommitWrongView ->
      TRUE
    [] Bug = "accept_precommit_wrong_epoch"
       /\ c = PrecommitWrongEpoch ->
      TRUE
    [] Bug = "accept_precommit_non_actionable"
       /\ c = PrecommitCommitNonActionable ->
      TRUE
    [] OTHER -> SpecPrecommitBlocks(c)

ImplementationSlotEvidence(c) ==
  CASE Bug = "reject_slot_prepare_vote"
       /\ c = SlotPrepareVoteActionable ->
      FALSE
    [] Bug = "reject_slot_commit_vote"
       /\ c = SlotCommitVoteActionable ->
      FALSE
    [] Bug = "reject_slot_prepare_qc"
       /\ c = SlotPrepareQcActionable ->
      FALSE
    [] Bug = "reject_slot_commit_qc"
       /\ c = SlotCommitQcActionable ->
      FALSE
    [] Bug = "accept_slot_wrong_height"
       /\ c = SlotWrongHeightVote ->
      TRUE
    [] Bug = "accept_slot_wrong_view"
       /\ c = SlotWrongViewVote ->
      TRUE
    [] Bug = "accept_slot_wrong_epoch"
       /\ c = SlotWrongEpochVote ->
      TRUE
    [] Bug = "accept_slot_prevote"
       /\ c = SlotPrevoteActionable ->
      TRUE
    [] Bug = "accept_slot_new_view_qc"
       /\ c = SlotNewViewQcActionable ->
      TRUE
    [] Bug = "accept_slot_non_actionable_vote"
       /\ c = SlotPrepareVoteNonActionable ->
      TRUE
    [] Bug = "accept_slot_non_actionable_qc"
       /\ c = SlotPrepareQcNonActionable ->
      TRUE
    [] OTHER -> SpecSlotEvidence(c)

Bugs == {
  "none",
  "reject_precommit_commit_actionable",
  "accept_precommit_prepare",
  "accept_precommit_wrong_height",
  "accept_precommit_wrong_view",
  "accept_precommit_wrong_epoch",
  "accept_precommit_non_actionable",
  "reject_slot_prepare_vote",
  "reject_slot_commit_vote",
  "reject_slot_prepare_qc",
  "reject_slot_commit_qc",
  "accept_slot_wrong_height",
  "accept_slot_wrong_view",
  "accept_slot_wrong_epoch",
  "accept_slot_prevote",
  "accept_slot_new_view_qc",
  "accept_slot_non_actionable_vote",
  "accept_slot_non_actionable_qc"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecPrecommitBlocks(c) \in BOOLEAN
       /\ SpecSlotEvidence(c) \in BOOLEAN
       /\ ImplementationPrecommitBlocks(c) \in BOOLEAN
       /\ ImplementationSlotEvidence(c) \in BOOLEAN

PrecommitMatchesSpec ==
  \A c \in Cases:
    ImplementationPrecommitBlocks(c) = SpecPrecommitBlocks(c)

SlotEvidenceMatchesSpec ==
  \A c \in Cases:
    ImplementationSlotEvidence(c) = SpecSlotEvidence(c)

PrecommitStrictCommitGate ==
  /\ ImplementationPrecommitBlocks(PrecommitCommitActionable)
  /\ ~ImplementationPrecommitBlocks(PrecommitPrepareActionable)
  /\ ~ImplementationPrecommitBlocks(PrecommitWrongHeight)
  /\ ~ImplementationPrecommitBlocks(PrecommitWrongView)
  /\ ~ImplementationPrecommitBlocks(PrecommitWrongEpoch)
  /\ ~ImplementationPrecommitBlocks(PrecommitCommitNonActionable)

PrecommitOnlyExactCommitVotes ==
  /\ ImplementationPrecommitBlocks(PrecommitCommitActionable)
  /\ ImplementationPrecommitBlocks(SlotCommitVoteActionable)
  /\ \A c \in Cases \ {PrecommitCommitActionable, SlotCommitVoteActionable}:
       ~ImplementationPrecommitBlocks(c)

SlotEvidenceAcceptsVoteAndQcSources ==
  /\ ImplementationSlotEvidence(SlotPrepareVoteActionable)
  /\ ImplementationSlotEvidence(SlotCommitVoteActionable)
  /\ ImplementationSlotEvidence(SlotPrepareQcActionable)
  /\ ImplementationSlotEvidence(SlotCommitQcActionable)

SlotEvidenceAcceptsExactPrecommitVotes ==
  /\ ImplementationSlotEvidence(PrecommitCommitActionable)
  /\ ImplementationSlotEvidence(PrecommitPrepareActionable)

SlotEvidenceRejectsWrongShapeAndMissingPayload ==
  /\ ~ImplementationSlotEvidence(SlotWrongHeightVote)
  /\ ~ImplementationSlotEvidence(SlotWrongViewVote)
  /\ ~ImplementationSlotEvidence(SlotWrongEpochVote)
  /\ ~ImplementationSlotEvidence(SlotPrevoteActionable)
  /\ ~ImplementationSlotEvidence(SlotNewViewQcActionable)
  /\ ~ImplementationSlotEvidence(SlotPrepareVoteNonActionable)
  /\ ~ImplementationSlotEvidence(SlotPrepareQcNonActionable)
  /\ ~ImplementationSlotEvidence(NoSlotEvidence)

SlotEvidenceRejectsPrecommitMismatches ==
  /\ ~ImplementationSlotEvidence(PrecommitWrongHeight)
  /\ ~ImplementationSlotEvidence(PrecommitWrongView)
  /\ ~ImplementationSlotEvidence(PrecommitWrongEpoch)
  /\ ~ImplementationSlotEvidence(PrecommitCommitNonActionable)

NoBugInvariant ==
  /\ PrecommitMatchesSpec
  /\ SlotEvidenceMatchesSpec
  /\ PrecommitStrictCommitGate
  /\ PrecommitOnlyExactCommitVotes
  /\ SlotEvidenceAcceptsVoteAndQcSources
  /\ SlotEvidenceAcceptsExactPrecommitVotes
  /\ SlotEvidenceRejectsWrongShapeAndMissingPayload
  /\ SlotEvidenceRejectsPrecommitMismatches

ActionableVoteBackedProposalExactness ==
  NoBugInvariant

ActionableVoteBackedProposalCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ActionableVoteBackedProposalExactness

SafetyFast ==
  ActionableVoteBackedProposalExactness

====
