---- MODULE SumeragiKeepFrontierPendingActiveGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for
`keep_frontier_pending_active_across_view_change(...)`.

The helper is a preservation gate used while pruning pending same-height work
after a view change. It first requires the requested hash to be the matching
live frontier owner for the height. Only then may three exact-hash sources keep
the pending entry active: a stale prior-view pending wrapper with observed
commit-QC or a local commit vote bridged by active/locked local ownership,
validation inflight, or commit inflight. Pending misses must fall through to
inflight sources, while any missing frontier slot, mismatched slot/hash, or
dead owner suppresses every source.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PendingCommitQcLive == "pending_commit_qc_live"
PendingLocalVoteActiveBridge == "pending_local_vote_active_bridge"
PendingLocalVoteLockBridge == "pending_local_vote_lock_bridge"

ValidationInflightLive == "validation_inflight_live"
CommitInflightLive == "commit_inflight_live"
PendingAbortedWithValidation == "pending_aborted_with_validation"
PendingRetiredWithCommit == "pending_retired_with_commit"

NoFrontierSlot == "no_frontier_slot"
SlotWrongHeight == "slot_wrong_height"
SlotWrongHash == "slot_wrong_hash"
DeadOwnerWithPendingQc == "dead_owner_with_pending_qc"
DeadOwnerWithValidation == "dead_owner_with_validation"
DeadOwnerWithCommit == "dead_owner_with_commit"

PendingAborted == "pending_aborted"
PendingRetired == "pending_retired"
PendingWrongHeight == "pending_wrong_height"
PendingSameView == "pending_same_view"
PendingFutureView == "pending_future_view"
PendingWrongHash == "pending_wrong_hash"
PendingLocalVoteNoBridge == "pending_local_vote_no_bridge"
PendingNoEvidence == "pending_no_evidence"

ValidationWrongHash == "validation_wrong_hash"
CommitWrongHash == "commit_wrong_hash"
NoPreservationSource == "no_preservation_source"

Cases == {
  PendingCommitQcLive,
  PendingLocalVoteActiveBridge,
  PendingLocalVoteLockBridge,
  ValidationInflightLive,
  CommitInflightLive,
  PendingAbortedWithValidation,
  PendingRetiredWithCommit,
  NoFrontierSlot,
  SlotWrongHeight,
  SlotWrongHash,
  DeadOwnerWithPendingQc,
  DeadOwnerWithValidation,
  DeadOwnerWithCommit,
  PendingAborted,
  PendingRetired,
  PendingWrongHeight,
  PendingSameView,
  PendingFutureView,
  PendingWrongHash,
  PendingLocalVoteNoBridge,
  PendingNoEvidence,
  ValidationWrongHash,
  CommitWrongHash,
  NoPreservationSource
}

IneligibleCases == {
  NoFrontierSlot,
  SlotWrongHeight,
  SlotWrongHash,
  DeadOwnerWithPendingQc,
  DeadOwnerWithValidation,
  DeadOwnerWithCommit
}

PendingPreserveCases == {
  PendingCommitQcLive,
  PendingLocalVoteActiveBridge,
  PendingLocalVoteLockBridge
}

ValidationPreserveCases == {
  ValidationInflightLive,
  PendingAbortedWithValidation
}

CommitPreserveCases == {
  CommitInflightLive,
  PendingRetiredWithCommit
}

PendingRejectedCases == {
  PendingAborted,
  PendingRetired,
  PendingWrongHeight,
  PendingSameView,
  PendingFutureView,
  PendingWrongHash,
  PendingLocalVoteNoBridge,
  PendingNoEvidence
}

InflightRejectedCases == {
  ValidationWrongHash,
  CommitWrongHash
}

Eligible(c) == ~(c \in IneligibleCases)

SpecResult(c) ==
  Eligible(c)
    /\ (c \in PendingPreserveCases
      \/ c \in ValidationPreserveCases
      \/ c \in CommitPreserveCases)

ImplementationResult(c) ==
  CASE Bug = "reject_pending_commit_qc"
       /\ c = PendingCommitQcLive ->
      FALSE
    [] Bug = "reject_pending_local_vote_active_bridge"
       /\ c = PendingLocalVoteActiveBridge ->
      FALSE
    [] Bug = "reject_pending_local_vote_lock_bridge"
       /\ c = PendingLocalVoteLockBridge ->
      FALSE
    [] Bug = "reject_validation_inflight"
       /\ c = ValidationInflightLive ->
      FALSE
    [] Bug = "reject_commit_inflight"
       /\ c = CommitInflightLive ->
      FALSE
    [] Bug = "pending_aborted_blocks_validation"
       /\ c = PendingAbortedWithValidation ->
      FALSE
    [] Bug = "pending_retired_blocks_commit"
       /\ c = PendingRetiredWithCommit ->
      FALSE
    [] Bug = "accept_no_frontier_slot"
       /\ c = NoFrontierSlot ->
      TRUE
    [] Bug = "accept_slot_wrong_height"
       /\ c = SlotWrongHeight ->
      TRUE
    [] Bug = "accept_slot_wrong_hash"
       /\ c = SlotWrongHash ->
      TRUE
    [] Bug = "accept_dead_owner_pending_qc"
       /\ c = DeadOwnerWithPendingQc ->
      TRUE
    [] Bug = "accept_dead_owner_validation"
       /\ c = DeadOwnerWithValidation ->
      TRUE
    [] Bug = "accept_dead_owner_commit"
       /\ c = DeadOwnerWithCommit ->
      TRUE
    [] Bug = "accept_pending_aborted"
       /\ c = PendingAborted ->
      TRUE
    [] Bug = "accept_pending_retired"
       /\ c = PendingRetired ->
      TRUE
    [] Bug = "accept_pending_wrong_height"
       /\ c = PendingWrongHeight ->
      TRUE
    [] Bug = "accept_pending_same_view"
       /\ c = PendingSameView ->
      TRUE
    [] Bug = "accept_pending_future_view"
       /\ c = PendingFutureView ->
      TRUE
    [] Bug = "accept_pending_wrong_hash"
       /\ c = PendingWrongHash ->
      TRUE
    [] Bug = "accept_pending_local_vote_no_bridge"
       /\ c = PendingLocalVoteNoBridge ->
      TRUE
    [] Bug = "accept_pending_no_evidence"
       /\ c = PendingNoEvidence ->
      TRUE
    [] Bug = "accept_validation_wrong_hash"
       /\ c = ValidationWrongHash ->
      TRUE
    [] Bug = "accept_commit_wrong_hash"
       /\ c = CommitWrongHash ->
      TRUE
    [] Bug = "accept_no_source"
       /\ c = NoPreservationSource ->
      TRUE
    [] OTHER -> SpecResult(c)

Bugs == {
  "none",
  "reject_pending_commit_qc",
  "reject_pending_local_vote_active_bridge",
  "reject_pending_local_vote_lock_bridge",
  "reject_validation_inflight",
  "reject_commit_inflight",
  "pending_aborted_blocks_validation",
  "pending_retired_blocks_commit",
  "accept_no_frontier_slot",
  "accept_slot_wrong_height",
  "accept_slot_wrong_hash",
  "accept_dead_owner_pending_qc",
  "accept_dead_owner_validation",
  "accept_dead_owner_commit",
  "accept_pending_aborted",
  "accept_pending_retired",
  "accept_pending_wrong_height",
  "accept_pending_same_view",
  "accept_pending_future_view",
  "accept_pending_wrong_hash",
  "accept_pending_local_vote_no_bridge",
  "accept_pending_no_evidence",
  "accept_validation_wrong_hash",
  "accept_commit_wrong_hash",
  "accept_no_source"
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

LiveFrontierOwnerRequired ==
  /\ ~ImplementationResult(NoFrontierSlot)
  /\ ~ImplementationResult(SlotWrongHeight)
  /\ ~ImplementationResult(SlotWrongHash)
  /\ ~ImplementationResult(DeadOwnerWithPendingQc)
  /\ ~ImplementationResult(DeadOwnerWithValidation)
  /\ ~ImplementationResult(DeadOwnerWithCommit)

PendingPreservationRequiresPriorValidEvidence ==
  /\ ImplementationResult(PendingCommitQcLive)
  /\ ImplementationResult(PendingLocalVoteActiveBridge)
  /\ ImplementationResult(PendingLocalVoteLockBridge)
  /\ ~ImplementationResult(PendingAborted)
  /\ ~ImplementationResult(PendingRetired)
  /\ ~ImplementationResult(PendingWrongHeight)
  /\ ~ImplementationResult(PendingSameView)
  /\ ~ImplementationResult(PendingFutureView)
  /\ ~ImplementationResult(PendingWrongHash)
  /\ ~ImplementationResult(PendingNoEvidence)

LocalVoteRequiresViewBridge ==
  ~ImplementationResult(PendingLocalVoteNoBridge)

InflightSourcesRequireMatchingHash ==
  /\ ImplementationResult(ValidationInflightLive)
  /\ ImplementationResult(CommitInflightLive)
  /\ ~ImplementationResult(ValidationWrongHash)
  /\ ~ImplementationResult(CommitWrongHash)

PendingMissesDoNotBlockInflightFallbacks ==
  /\ ImplementationResult(PendingAbortedWithValidation)
  /\ ImplementationResult(PendingRetiredWithCommit)

NoSourceRejected ==
  ~ImplementationResult(NoPreservationSource)

AcceptedSourceAnchors ==
  /\ ImplementationResult(PendingCommitQcLive)
  /\ ImplementationResult(PendingLocalVoteActiveBridge)
  /\ ImplementationResult(PendingLocalVoteLockBridge)
  /\ ImplementationResult(ValidationInflightLive)
  /\ ImplementationResult(CommitInflightLive)
  /\ ImplementationResult(PendingAbortedWithValidation)
  /\ ImplementationResult(PendingRetiredWithCommit)

FrontierEligibilityRejectionAnchors ==
  /\ ~ImplementationResult(NoFrontierSlot)
  /\ ~ImplementationResult(SlotWrongHeight)
  /\ ~ImplementationResult(SlotWrongHash)
  /\ ~ImplementationResult(DeadOwnerWithPendingQc)
  /\ ~ImplementationResult(DeadOwnerWithValidation)
  /\ ~ImplementationResult(DeadOwnerWithCommit)

PendingShapeRejectionAnchors ==
  /\ ~ImplementationResult(PendingAborted)
  /\ ~ImplementationResult(PendingRetired)
  /\ ~ImplementationResult(PendingWrongHeight)
  /\ ~ImplementationResult(PendingSameView)
  /\ ~ImplementationResult(PendingFutureView)
  /\ ~ImplementationResult(PendingWrongHash)
  /\ ~ImplementationResult(PendingNoEvidence)

LocalVoteBridgeRejectionAnchors ==
  ~ImplementationResult(PendingLocalVoteNoBridge)

InflightHashRejectionAnchors ==
  /\ ~ImplementationResult(ValidationWrongHash)
  /\ ~ImplementationResult(CommitWrongHash)

NoSourceRejectionAnchors ==
  ~ImplementationResult(NoPreservationSource)

NoBugInvariant ==
  /\ ResultsMatchSpec
  /\ LiveFrontierOwnerRequired
  /\ PendingPreservationRequiresPriorValidEvidence
  /\ LocalVoteRequiresViewBridge
  /\ InflightSourcesRequireMatchingHash
  /\ PendingMissesDoNotBlockInflightFallbacks
  /\ NoSourceRejected
  /\ AcceptedSourceAnchors
  /\ FrontierEligibilityRejectionAnchors
  /\ PendingShapeRejectionAnchors
  /\ LocalVoteBridgeRejectionAnchors
  /\ InflightHashRejectionAnchors
  /\ NoSourceRejectionAnchors

SafetyFast == NoBugInvariant

KeepFrontierPendingActiveExactness ==
  /\ ResultsMatchSpec
  /\ LiveFrontierOwnerRequired
  /\ PendingPreservationRequiresPriorValidEvidence
  /\ LocalVoteRequiresViewBridge
  /\ InflightSourcesRequireMatchingHash
  /\ PendingMissesDoNotBlockInflightFallbacks
  /\ NoSourceRejected
  /\ AcceptedSourceAnchors
  /\ FrontierEligibilityRejectionAnchors
  /\ PendingShapeRejectionAnchors
  /\ LocalVoteBridgeRejectionAnchors
  /\ InflightHashRejectionAnchors
  /\ NoSourceRejectionAnchors

KeepFrontierPendingActiveCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ KeepFrontierPendingActiveExactness

====
