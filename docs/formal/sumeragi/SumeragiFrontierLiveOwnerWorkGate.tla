---- MODULE SumeragiFrontierLiveOwnerWorkGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for
`frontier_slot_has_live_local_owner_work_for_view(...)`.

The helper protects a same-height frontier owner while local work or quorum
evidence still makes that owner live. Finalized and passive-catchup slots are
terminal and suppress every source. Otherwise, exact active pending work,
commit inflight work, validation inflight work, observed slot commit QC,
later-view competing quorum lockout, explicit locally-voted lock state, or
exact local vote history can keep the owner live. A terminal pending wrapper
without observed commit QC blocks only the local-lock and local-history paths,
not independent validation or slot-QC evidence.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PendingLive == "pending_live"
FinalizedWithPending == "finalized_with_pending"
PassiveWithLocalVote == "passive_with_local_vote"

PendingAborted == "pending_aborted"
PendingRetired == "pending_retired"
PendingInvalid == "pending_invalid"
PendingWrongHeight == "pending_wrong_height"
PendingWrongView == "pending_wrong_view"
PendingNonExtending == "pending_non_extending"
PendingInvalidWithValidation == "pending_invalid_with_validation"
TerminalNoCommitWithValidation == "terminal_no_commit_with_validation"

CommitInflightLive == "commit_inflight_live"
CommitInflightAborted == "commit_inflight_aborted"
CommitInflightInvalid == "commit_inflight_invalid"
CommitInflightWrongHeight == "commit_inflight_wrong_height"
CommitInflightWrongView == "commit_inflight_wrong_view"
CommitInflightNonExtending == "commit_inflight_non_extending"
CommitInflightWrongHeightWithQc == "commit_inflight_wrong_height_with_qc"

ValidationInflightLive == "validation_inflight_live"
SlotCommitQcObserved == "slot_commit_qc_observed"

CompetingDirectQuorumLater == "competing_direct_quorum_later"
CompetingAggregateLockLater == "competing_aggregate_lock_later"
CompetingDirectExactView == "competing_direct_exact_view"
CompetingNoVotesLater == "competing_no_votes_later"
CompetingFreshQuorumPossible == "competing_fresh_quorum_possible"

LocallyVotedNoPending == "locally_voted_no_pending"
LocallyVotedTerminalWithCommitQc == "locally_voted_terminal_with_commit_qc"
LocallyVotedTerminalNoCommitQc == "locally_voted_terminal_no_commit_qc"

LocalVoteHistoryNoPending == "local_vote_history_no_pending"
LocalVoteHistoryTerminalWithCommitQc == "local_vote_history_terminal_with_commit_qc"
LocalVoteHistoryTerminalNoCommitQc == "local_vote_history_terminal_no_commit_qc"
LocalVoteHistoryWrongEpoch == "local_vote_history_wrong_epoch"
LocalVoteHistoryWrongBlock == "local_vote_history_wrong_block"
LocalVoteHistoryPrevote == "local_vote_history_prevote"
LocalVoteHistoryRemote == "local_vote_history_remote"

NoOwnerWork == "no_owner_work"

Cases == {
  PendingLive,
  FinalizedWithPending,
  PassiveWithLocalVote,
  PendingAborted,
  PendingRetired,
  PendingInvalid,
  PendingWrongHeight,
  PendingWrongView,
  PendingNonExtending,
  PendingInvalidWithValidation,
  TerminalNoCommitWithValidation,
  CommitInflightLive,
  CommitInflightAborted,
  CommitInflightInvalid,
  CommitInflightWrongHeight,
  CommitInflightWrongView,
  CommitInflightNonExtending,
  CommitInflightWrongHeightWithQc,
  ValidationInflightLive,
  SlotCommitQcObserved,
  CompetingDirectQuorumLater,
  CompetingAggregateLockLater,
  CompetingDirectExactView,
  CompetingNoVotesLater,
  CompetingFreshQuorumPossible,
  LocallyVotedNoPending,
  LocallyVotedTerminalWithCommitQc,
  LocallyVotedTerminalNoCommitQc,
  LocalVoteHistoryNoPending,
  LocalVoteHistoryTerminalWithCommitQc,
  LocalVoteHistoryTerminalNoCommitQc,
  LocalVoteHistoryWrongEpoch,
  LocalVoteHistoryWrongBlock,
  LocalVoteHistoryPrevote,
  LocalVoteHistoryRemote,
  NoOwnerWork
}

ModeRejectedCases == {FinalizedWithPending, PassiveWithLocalVote}

PendingLiveCases == {PendingLive}
CommitInflightLiveCases == {CommitInflightLive}
ValidationInflightCases == {
  ValidationInflightLive,
  PendingInvalidWithValidation,
  TerminalNoCommitWithValidation
}
SlotCommitQcCases == {
  SlotCommitQcObserved,
  CommitInflightWrongHeightWithQc
}
CompetingLockedCases == {
  CompetingDirectQuorumLater,
  CompetingAggregateLockLater
}
LocalLockCases == {
  LocallyVotedNoPending,
  LocallyVotedTerminalWithCommitQc
}
LocalVoteHistoryCases == {
  LocalVoteHistoryNoPending,
  LocalVoteHistoryTerminalWithCommitQc
}

SpecResult(c) ==
  ~(c \in ModeRejectedCases)
    /\ (c \in PendingLiveCases
      \/ c \in CommitInflightLiveCases
      \/ c \in ValidationInflightCases
      \/ c \in SlotCommitQcCases
      \/ c \in CompetingLockedCases
      \/ c \in LocalLockCases
      \/ c \in LocalVoteHistoryCases)

ImplementationResult(c) ==
  CASE Bug = "reject_pending_live"
       /\ c = PendingLive ->
      FALSE
    [] Bug = "accept_finalized"
       /\ c = FinalizedWithPending ->
      TRUE
    [] Bug = "accept_passive"
       /\ c = PassiveWithLocalVote ->
      TRUE
    [] Bug = "accept_pending_aborted"
       /\ c = PendingAborted ->
      TRUE
    [] Bug = "accept_pending_retired"
       /\ c = PendingRetired ->
      TRUE
    [] Bug = "accept_pending_invalid"
       /\ c = PendingInvalid ->
      TRUE
    [] Bug = "accept_pending_wrong_height"
       /\ c = PendingWrongHeight ->
      TRUE
    [] Bug = "accept_pending_wrong_view"
       /\ c = PendingWrongView ->
      TRUE
    [] Bug = "accept_pending_non_extending"
       /\ c = PendingNonExtending ->
      TRUE
    [] Bug = "pending_invalid_blocks_validation"
       /\ c = PendingInvalidWithValidation ->
      FALSE
    [] Bug = "terminal_blocks_validation"
       /\ c = TerminalNoCommitWithValidation ->
      FALSE
    [] Bug = "reject_commit_inflight"
       /\ c = CommitInflightLive ->
      FALSE
    [] Bug = "accept_commit_inflight_aborted"
       /\ c = CommitInflightAborted ->
      TRUE
    [] Bug = "accept_commit_inflight_invalid"
       /\ c = CommitInflightInvalid ->
      TRUE
    [] Bug = "accept_commit_inflight_wrong_height"
       /\ c = CommitInflightWrongHeight ->
      TRUE
    [] Bug = "accept_commit_inflight_wrong_view"
       /\ c = CommitInflightWrongView ->
      TRUE
    [] Bug = "accept_commit_inflight_non_extending"
       /\ c = CommitInflightNonExtending ->
      TRUE
    [] Bug = "commit_wrong_height_blocks_slot_qc"
       /\ c = CommitInflightWrongHeightWithQc ->
      FALSE
    [] Bug = "reject_validation_inflight"
       /\ c = ValidationInflightLive ->
      FALSE
    [] Bug = "reject_slot_commit_qc"
       /\ c = SlotCommitQcObserved ->
      FALSE
    [] Bug = "reject_competing_direct"
       /\ c = CompetingDirectQuorumLater ->
      FALSE
    [] Bug = "reject_competing_aggregate"
       /\ c = CompetingAggregateLockLater ->
      FALSE
    [] Bug = "accept_competing_exact_view"
       /\ c = CompetingDirectExactView ->
      TRUE
    [] Bug = "accept_competing_no_votes"
       /\ c = CompetingNoVotesLater ->
      TRUE
    [] Bug = "accept_competing_fresh_quorum"
       /\ c = CompetingFreshQuorumPossible ->
      TRUE
    [] Bug = "reject_local_lock"
       /\ c = LocallyVotedNoPending ->
      FALSE
    [] Bug = "accept_local_lock_terminal_no_commit"
       /\ c = LocallyVotedTerminalNoCommitQc ->
      TRUE
    [] Bug = "reject_local_history"
       /\ c = LocalVoteHistoryNoPending ->
      FALSE
    [] Bug = "accept_local_history_terminal_no_commit"
       /\ c = LocalVoteHistoryTerminalNoCommitQc ->
      TRUE
    [] Bug = "accept_local_history_wrong_epoch"
       /\ c = LocalVoteHistoryWrongEpoch ->
      TRUE
    [] Bug = "accept_local_history_wrong_block"
       /\ c = LocalVoteHistoryWrongBlock ->
      TRUE
    [] Bug = "accept_local_history_prevote"
       /\ c = LocalVoteHistoryPrevote ->
      TRUE
    [] Bug = "accept_local_history_remote"
       /\ c = LocalVoteHistoryRemote ->
      TRUE
    [] Bug = "accept_no_work"
       /\ c = NoOwnerWork ->
      TRUE
    [] OTHER -> SpecResult(c)

Bugs == {
  "none",
  "reject_pending_live",
  "accept_finalized",
  "accept_passive",
  "accept_pending_aborted",
  "accept_pending_retired",
  "accept_pending_invalid",
  "accept_pending_wrong_height",
  "accept_pending_wrong_view",
  "accept_pending_non_extending",
  "pending_invalid_blocks_validation",
  "terminal_blocks_validation",
  "reject_commit_inflight",
  "accept_commit_inflight_aborted",
  "accept_commit_inflight_invalid",
  "accept_commit_inflight_wrong_height",
  "accept_commit_inflight_wrong_view",
  "accept_commit_inflight_non_extending",
  "commit_wrong_height_blocks_slot_qc",
  "reject_validation_inflight",
  "reject_slot_commit_qc",
  "reject_competing_direct",
  "reject_competing_aggregate",
  "accept_competing_exact_view",
  "accept_competing_no_votes",
  "accept_competing_fresh_quorum",
  "reject_local_lock",
  "accept_local_lock_terminal_no_commit",
  "reject_local_history",
  "accept_local_history_terminal_no_commit",
  "accept_local_history_wrong_epoch",
  "accept_local_history_wrong_block",
  "accept_local_history_prevote",
  "accept_local_history_remote",
  "accept_no_work"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecResult(c) \in BOOLEAN
       /\ ImplementationResult(c) \in BOOLEAN

ResultsMatchSpec ==
  \A c \in Cases:
    ImplementationResult(c) = SpecResult(c)

TerminalModesSuppressOwnerWork ==
  /\ ~ImplementationResult(FinalizedWithPending)
  /\ ~ImplementationResult(PassiveWithLocalVote)

PrimaryLiveSourcesAccepted ==
  /\ ImplementationResult(PendingLive)
  /\ ImplementationResult(CommitInflightLive)
  /\ ImplementationResult(ValidationInflightLive)
  /\ ImplementationResult(SlotCommitQcObserved)

InvalidLocalSourcesRejected ==
  /\ ~ImplementationResult(PendingAborted)
  /\ ~ImplementationResult(PendingRetired)
  /\ ~ImplementationResult(PendingInvalid)
  /\ ~ImplementationResult(PendingWrongHeight)
  /\ ~ImplementationResult(PendingWrongView)
  /\ ~ImplementationResult(PendingNonExtending)
  /\ ~ImplementationResult(CommitInflightAborted)
  /\ ~ImplementationResult(CommitInflightInvalid)
  /\ ~ImplementationResult(CommitInflightWrongHeight)
  /\ ~ImplementationResult(CommitInflightWrongView)
  /\ ~ImplementationResult(CommitInflightNonExtending)

CompetingQuorumRequiresLaterLockedView ==
  /\ ImplementationResult(CompetingDirectQuorumLater)
  /\ ImplementationResult(CompetingAggregateLockLater)
  /\ ~ImplementationResult(CompetingDirectExactView)
  /\ ~ImplementationResult(CompetingNoVotesLater)
  /\ ~ImplementationResult(CompetingFreshQuorumPossible)

TerminalPendingBlocksOnlyLocalVotePaths ==
  /\ ImplementationResult(LocallyVotedNoPending)
  /\ ImplementationResult(LocallyVotedTerminalWithCommitQc)
  /\ ~ImplementationResult(LocallyVotedTerminalNoCommitQc)
  /\ ImplementationResult(LocalVoteHistoryNoPending)
  /\ ImplementationResult(LocalVoteHistoryTerminalWithCommitQc)
  /\ ~ImplementationResult(LocalVoteHistoryTerminalNoCommitQc)
  /\ ImplementationResult(TerminalNoCommitWithValidation)

LocalVoteHistoryMustMatchExactLocalSlot ==
  /\ ~ImplementationResult(LocalVoteHistoryWrongEpoch)
  /\ ~ImplementationResult(LocalVoteHistoryWrongBlock)
  /\ ~ImplementationResult(LocalVoteHistoryPrevote)
  /\ ~ImplementationResult(LocalVoteHistoryRemote)

FallthroughSourcesSurviveEarlierMisses ==
  /\ ImplementationResult(PendingInvalidWithValidation)
  /\ ImplementationResult(TerminalNoCommitWithValidation)
  /\ ImplementationResult(CommitInflightWrongHeightWithQc)

NoWorkRejected ==
  ~ImplementationResult(NoOwnerWork)

AcceptedEvidenceClassAnchors ==
  /\ ImplementationResult(PendingLive)
  /\ ImplementationResult(CommitInflightLive)
  /\ ImplementationResult(ValidationInflightLive)
  /\ ImplementationResult(SlotCommitQcObserved)
  /\ ImplementationResult(CompetingDirectQuorumLater)
  /\ ImplementationResult(CompetingAggregateLockLater)
  /\ ImplementationResult(LocallyVotedNoPending)
  /\ ImplementationResult(LocallyVotedTerminalWithCommitQc)
  /\ ImplementationResult(LocalVoteHistoryNoPending)
  /\ ImplementationResult(LocalVoteHistoryTerminalWithCommitQc)

TerminalModeRejectionAnchors ==
  /\ ~ImplementationResult(FinalizedWithPending)
  /\ ~ImplementationResult(PassiveWithLocalVote)
  /\ ~ImplementationResult(LocallyVotedTerminalNoCommitQc)
  /\ ~ImplementationResult(LocalVoteHistoryTerminalNoCommitQc)

PendingAndCommitShapeRejectionAnchors ==
  /\ ~ImplementationResult(PendingAborted)
  /\ ~ImplementationResult(PendingRetired)
  /\ ~ImplementationResult(PendingInvalid)
  /\ ~ImplementationResult(PendingWrongHeight)
  /\ ~ImplementationResult(PendingWrongView)
  /\ ~ImplementationResult(PendingNonExtending)
  /\ ~ImplementationResult(CommitInflightAborted)
  /\ ~ImplementationResult(CommitInflightInvalid)
  /\ ~ImplementationResult(CommitInflightWrongHeight)
  /\ ~ImplementationResult(CommitInflightWrongView)
  /\ ~ImplementationResult(CommitInflightNonExtending)

CompetingQuorumRejectionAnchors ==
  /\ ~ImplementationResult(CompetingDirectExactView)
  /\ ~ImplementationResult(CompetingNoVotesLater)
  /\ ~ImplementationResult(CompetingFreshQuorumPossible)

FallthroughPreservationAnchors ==
  /\ ImplementationResult(PendingInvalidWithValidation)
  /\ ImplementationResult(TerminalNoCommitWithValidation)
  /\ ImplementationResult(CommitInflightWrongHeightWithQc)

LocalVoteHistoryRejectionAnchors ==
  /\ ~ImplementationResult(LocalVoteHistoryWrongEpoch)
  /\ ~ImplementationResult(LocalVoteHistoryWrongBlock)
  /\ ~ImplementationResult(LocalVoteHistoryPrevote)
  /\ ~ImplementationResult(LocalVoteHistoryRemote)
  /\ ~ImplementationResult(NoOwnerWork)

NoBugInvariant ==
  /\ ResultsMatchSpec
  /\ TerminalModesSuppressOwnerWork
  /\ PrimaryLiveSourcesAccepted
  /\ InvalidLocalSourcesRejected
  /\ CompetingQuorumRequiresLaterLockedView
  /\ TerminalPendingBlocksOnlyLocalVotePaths
  /\ LocalVoteHistoryMustMatchExactLocalSlot
  /\ FallthroughSourcesSurviveEarlierMisses
  /\ NoWorkRejected
  /\ AcceptedEvidenceClassAnchors
  /\ TerminalModeRejectionAnchors
  /\ PendingAndCommitShapeRejectionAnchors
  /\ CompetingQuorumRejectionAnchors
  /\ FallthroughPreservationAnchors
  /\ LocalVoteHistoryRejectionAnchors

SafetyFast == NoBugInvariant

====
