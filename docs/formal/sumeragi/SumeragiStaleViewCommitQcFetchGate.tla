---- MODULE SumeragiStaleViewCommitQcFetchGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for stale-view commit-QC fetch admission.

This slice captures `pending_allows_stale_view_commit_qc_fetch(...)` from
`main_loop/commit.rs` and the local `pending_extends_tip(...)` predicate it
delegates to. It abstracts concrete block hashes as finite integers while
preserving the deterministic contract: stale-view commit-QC fetches are allowed
only for the exact pending block hash, height, and view; the pending block must
not be invalid or consensus-inactive; a local commit vote must already have
been emitted; and the pending block must extend the current tip by exactly one
height with matching parent/tip hash, including the all-absent parent/tip case.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoneHash == 0
TipHash == 1
BlockHash == 2
OtherHash == 3

ValidExactTip == 1
ValidAbsentTip == 2
BlockHashMismatch == 3
HeightMismatch == 4
ViewMismatch == 5
InvalidPending == 6
ConsensusInactive == 7
NoLocalCommitVote == 8
TipHeightMismatch == 9
ParentHashMismatch == 10
TipHashMissingParentPresent == 11
ParentMissingTipPresent == 12

Candidates == 1..12

PendingBlockHash(c) == BlockHash
RequestBlockHash(c) == IF c = BlockHashMismatch THEN OtherHash ELSE BlockHash

PendingHeight(c) ==
  CASE c = ValidAbsentTip -> 1
    [] OTHER -> 6

RequestHeight(c) == IF c = HeightMismatch THEN PendingHeight(c) + 1 ELSE PendingHeight(c)

PendingView(c) == 4
RequestView(c) == IF c = ViewMismatch THEN 5 ELSE 4

PendingInvalid(c) == c = InvalidPending
PendingConsensusInactive(c) == c = ConsensusInactive
PendingLocalCommitVoteEmitted(c) == c # NoLocalCommitVote

TipHeight(c) ==
  CASE c = ValidAbsentTip -> 0
    [] c = TipHeightMismatch -> 6
    [] OTHER -> 5

PendingParent(c) ==
  CASE c = ValidAbsentTip -> NoneHash
    [] c = ParentHashMismatch -> OtherHash
    [] c = TipHashMissingParentPresent -> TipHash
    [] c = ParentMissingTipPresent -> NoneHash
    [] OTHER -> TipHash

CurrentTipHash(c) ==
  CASE c = ValidAbsentTip -> NoneHash
    [] c = ParentHashMismatch -> TipHash
    [] c = TipHashMissingParentPresent -> NoneHash
    [] c = ParentMissingTipPresent -> TipHash
    [] OTHER -> TipHash

SpecPendingExtendsTip(c) ==
  /\ PendingHeight(c) = TipHeight(c) + 1
  /\ PendingParent(c) = CurrentTipHash(c)

ImplPendingExtendsTip(c) ==
  CASE Bug = "tip_allows_same_height" ->
      /\ PendingHeight(c) = TipHeight(c)
      /\ PendingParent(c) = CurrentTipHash(c)
    [] Bug = "tip_ignores_parent" ->
      PendingHeight(c) = TipHeight(c) + 1
    [] Bug = "tip_requires_present_hash" ->
      /\ PendingHeight(c) = TipHeight(c) + 1
      /\ PendingParent(c) = CurrentTipHash(c)
      /\ CurrentTipHash(c) # NoneHash
    [] Bug = "tip_allows_missing_parent" ->
      /\ PendingHeight(c) = TipHeight(c) + 1
      /\ (PendingParent(c) = CurrentTipHash(c) \/ PendingParent(c) = NoneHash)
    [] OTHER -> SpecPendingExtendsTip(c)

SpecAllowed(c) ==
  /\ PendingBlockHash(c) = RequestBlockHash(c)
  /\ PendingHeight(c) = RequestHeight(c)
  /\ PendingView(c) = RequestView(c)
  /\ ~PendingInvalid(c)
  /\ ~PendingConsensusInactive(c)
  /\ PendingLocalCommitVoteEmitted(c)
  /\ SpecPendingExtendsTip(c)

ImplAllowed(c) ==
  CASE Bug = "skip_block_hash" ->
      /\ PendingHeight(c) = RequestHeight(c)
      /\ PendingView(c) = RequestView(c)
      /\ ~PendingInvalid(c)
      /\ ~PendingConsensusInactive(c)
      /\ PendingLocalCommitVoteEmitted(c)
      /\ ImplPendingExtendsTip(c)
    [] Bug = "skip_height" ->
      /\ PendingBlockHash(c) = RequestBlockHash(c)
      /\ PendingView(c) = RequestView(c)
      /\ ~PendingInvalid(c)
      /\ ~PendingConsensusInactive(c)
      /\ PendingLocalCommitVoteEmitted(c)
      /\ ImplPendingExtendsTip(c)
    [] Bug = "skip_view" ->
      /\ PendingBlockHash(c) = RequestBlockHash(c)
      /\ PendingHeight(c) = RequestHeight(c)
      /\ ~PendingInvalid(c)
      /\ ~PendingConsensusInactive(c)
      /\ PendingLocalCommitVoteEmitted(c)
      /\ ImplPendingExtendsTip(c)
    [] Bug = "skip_validation_status" ->
      /\ PendingBlockHash(c) = RequestBlockHash(c)
      /\ PendingHeight(c) = RequestHeight(c)
      /\ PendingView(c) = RequestView(c)
      /\ ~PendingConsensusInactive(c)
      /\ PendingLocalCommitVoteEmitted(c)
      /\ ImplPendingExtendsTip(c)
    [] Bug = "skip_consensus_active" ->
      /\ PendingBlockHash(c) = RequestBlockHash(c)
      /\ PendingHeight(c) = RequestHeight(c)
      /\ PendingView(c) = RequestView(c)
      /\ ~PendingInvalid(c)
      /\ PendingLocalCommitVoteEmitted(c)
      /\ ImplPendingExtendsTip(c)
    [] Bug = "skip_local_commit_vote" ->
      /\ PendingBlockHash(c) = RequestBlockHash(c)
      /\ PendingHeight(c) = RequestHeight(c)
      /\ PendingView(c) = RequestView(c)
      /\ ~PendingInvalid(c)
      /\ ~PendingConsensusInactive(c)
      /\ ImplPendingExtendsTip(c)
    [] Bug = "reject_valid" /\ c = ValidExactTip ->
      FALSE
    [] OTHER ->
      /\ PendingBlockHash(c) = RequestBlockHash(c)
      /\ PendingHeight(c) = RequestHeight(c)
      /\ PendingView(c) = RequestView(c)
      /\ ~PendingInvalid(c)
      /\ ~PendingConsensusInactive(c)
      /\ PendingLocalCommitVoteEmitted(c)
      /\ ImplPendingExtendsTip(c)

Init == checked = 0

Next == UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "skip_block_hash",
       "skip_height",
       "skip_view",
       "skip_validation_status",
       "skip_consensus_active",
       "skip_local_commit_vote",
       "tip_allows_same_height",
       "tip_ignores_parent",
       "tip_requires_present_hash",
       "tip_allows_missing_parent",
       "reject_valid"
     }
  /\ checked = 0
  /\ \A c \in Candidates: SpecAllowed(c) \in BOOLEAN
  /\ \A c \in Candidates: ImplAllowed(c) \in BOOLEAN

Safety ==
  \A c \in Candidates:
    ImplAllowed(c) = SpecAllowed(c)

StaleViewFetchIdentityExact ==
  \A c \in {BlockHashMismatch, HeightMismatch, ViewMismatch}:
    ImplAllowed(c) = SpecAllowed(c)

StaleViewPendingStateGateExact ==
  \A c \in {InvalidPending, ConsensusInactive, NoLocalCommitVote}:
    ImplAllowed(c) = SpecAllowed(c)

StaleViewTipExtensionExact ==
  \A c \in {
    TipHeightMismatch,
    ParentHashMismatch,
    TipHashMissingParentPresent,
    ParentMissingTipPresent,
    ValidAbsentTip
  }:
    ImplPendingExtendsTip(c) = SpecPendingExtendsTip(c)

StaleViewPositiveAdmissionExact ==
  /\ ImplAllowed(ValidExactTip) = SpecAllowed(ValidExactTip)
  /\ ImplAllowed(ValidAbsentTip) = SpecAllowed(ValidAbsentTip)

StaleViewCommitQcFetchExactness ==
  /\ StaleViewFetchIdentityExact
  /\ StaleViewPendingStateGateExact
  /\ StaleViewTipExtensionExact
  /\ StaleViewPositiveAdmissionExact

BugSkipBlockHash ==
  ImplAllowed(BlockHashMismatch) = SpecAllowed(BlockHashMismatch)

BugSkipHeight ==
  ImplAllowed(HeightMismatch) = SpecAllowed(HeightMismatch)

BugSkipView ==
  ImplAllowed(ViewMismatch) = SpecAllowed(ViewMismatch)

BugSkipValidationStatus ==
  ImplAllowed(InvalidPending) = SpecAllowed(InvalidPending)

BugSkipConsensusActive ==
  ImplAllowed(ConsensusInactive) = SpecAllowed(ConsensusInactive)

BugSkipLocalCommitVote ==
  ImplAllowed(NoLocalCommitVote) = SpecAllowed(NoLocalCommitVote)

BugTipAllowsSameHeight ==
  ImplAllowed(TipHeightMismatch) = SpecAllowed(TipHeightMismatch)

BugTipIgnoresParent ==
  ImplAllowed(ParentHashMismatch) = SpecAllowed(ParentHashMismatch)

BugTipRequiresPresentHash ==
  ImplAllowed(ValidAbsentTip) = SpecAllowed(ValidAbsentTip)

BugTipAllowsMissingParent ==
  ImplAllowed(ParentMissingTipPresent) = SpecAllowed(ParentMissingTipPresent)

BugRejectValid ==
  ImplAllowed(ValidExactTip) = SpecAllowed(ValidExactTip)

====
