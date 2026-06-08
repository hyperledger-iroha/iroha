---- MODULE SumeragiFrontierQuorumOwnerActionableGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for live contiguous-frontier cleanup preservation.

This slice pins the composition between
`frontier_quorum_timeout_owner_still_actionable(...)` and
`should_preserve_live_contiguous_frontier_cleanup(...)`. Quorum-timeout cleanup
may preserve same-height state only for the live committed+1 height, only for
the phase tracker's current view, and only while the owner has actionable
frontier-slot, vote-backed, dependency-backlog, RBC sender, missing-block,
missing-commit-QC, or vote-backed recovery work. Stale views, wrong heights,
passive/invalid work, and no-source cleanup must not preserve the frontier.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoBug == "none"

Bugs == {
  NoBug,
  "owner_reject_active",
  "vote_reject_evidence",
  "dependency_reject_backlog",
  "sender_reject_activity",
  "missing_block_reject_activity",
  "missing_commit_reject_activity",
  "vote_backed_reject_activity",
  "action_accept_no_source",
  "action_accept_owner_wrong_view",
  "action_accept_vote_wrong_view",
  "action_accept_dependency_stale",
  "action_accept_sender_stale",
  "action_accept_missing_block_wrong_view",
  "action_accept_missing_commit_wrong_phase",
  "action_accept_vote_backed_passive",
  "preserve_reject_owner_source",
  "preserve_reject_recovery_source",
  "preserve_accept_non_frontier_height",
  "preserve_accept_stale_current_view",
  "preserve_accept_no_actionable"
}

OwnerActiveExact ==
  IF Bug = "owner_reject_active" THEN FALSE ELSE TRUE

VoteEvidenceExact ==
  IF Bug = "vote_reject_evidence" THEN FALSE ELSE TRUE

DependencyBacklogExact ==
  IF Bug = "dependency_reject_backlog" THEN FALSE ELSE TRUE

RbcSenderActivityExact ==
  IF Bug = "sender_reject_activity" THEN FALSE ELSE TRUE

MissingBlockActivityExact ==
  IF Bug = "missing_block_reject_activity" THEN FALSE ELSE TRUE

MissingCommitActivityExact ==
  IF Bug = "missing_commit_reject_activity" THEN FALSE ELSE TRUE

VoteBackedRecoveryExact ==
  IF Bug = "vote_backed_reject_activity" THEN FALSE ELSE TRUE

ActionableWithoutSourceAccepted ==
  IF Bug = "action_accept_no_source" THEN TRUE ELSE FALSE

OwnerWrongViewAccepted ==
  IF Bug = "action_accept_owner_wrong_view" THEN TRUE ELSE FALSE

VoteWrongViewAccepted ==
  IF Bug = "action_accept_vote_wrong_view" THEN TRUE ELSE FALSE

DependencyStaleAccepted ==
  IF Bug = "action_accept_dependency_stale" THEN TRUE ELSE FALSE

SenderStaleAccepted ==
  IF Bug = "action_accept_sender_stale" THEN TRUE ELSE FALSE

MissingBlockWrongViewAccepted ==
  IF Bug = "action_accept_missing_block_wrong_view" THEN TRUE ELSE FALSE

MissingCommitWrongPhaseAccepted ==
  IF Bug = "action_accept_missing_commit_wrong_phase" THEN TRUE ELSE FALSE

VoteBackedPassiveAccepted ==
  IF Bug = "action_accept_vote_backed_passive" THEN TRUE ELSE FALSE

PreserveOwnerSource ==
  IF Bug = "preserve_reject_owner_source" THEN FALSE ELSE TRUE

PreserveRecoverySource ==
  IF Bug = "preserve_reject_recovery_source" THEN FALSE ELSE TRUE

PreserveNonFrontierHeightAccepted ==
  IF Bug = "preserve_accept_non_frontier_height" THEN TRUE ELSE FALSE

PreserveStaleCurrentViewAccepted ==
  IF Bug = "preserve_accept_stale_current_view" THEN TRUE ELSE FALSE

PreserveNoActionableAccepted ==
  IF Bug = "preserve_accept_no_actionable" THEN TRUE ELSE FALSE

Init ==
  checked = 0

Next ==
  \/ /\ checked < 20
     /\ checked' = checked + 1
  \/ /\ checked = 20
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..20

ActionableSourceSafety ==
  /\ OwnerActiveExact
  /\ VoteEvidenceExact
  /\ DependencyBacklogExact
  /\ RbcSenderActivityExact
  /\ MissingBlockActivityExact
  /\ MissingCommitActivityExact
  /\ VoteBackedRecoveryExact
  /\ ~ActionableWithoutSourceAccepted
  /\ ~OwnerWrongViewAccepted
  /\ ~VoteWrongViewAccepted
  /\ ~DependencyStaleAccepted
  /\ ~SenderStaleAccepted
  /\ ~MissingBlockWrongViewAccepted
  /\ ~MissingCommitWrongPhaseAccepted
  /\ ~VoteBackedPassiveAccepted

LiveCleanupPreserveSafety ==
  /\ PreserveOwnerSource
  /\ PreserveRecoverySource
  /\ ~PreserveNonFrontierHeightAccepted
  /\ ~PreserveStaleCurrentViewAccepted
  /\ ~PreserveNoActionableAccepted

QuorumOwnerActionableRejectsNonExactInputs ==
  /\ ~ActionableWithoutSourceAccepted
  /\ ~OwnerWrongViewAccepted
  /\ ~VoteWrongViewAccepted
  /\ ~DependencyStaleAccepted
  /\ ~SenderStaleAccepted
  /\ ~MissingBlockWrongViewAccepted
  /\ ~MissingCommitWrongPhaseAccepted
  /\ ~VoteBackedPassiveAccepted
  /\ ~PreserveNonFrontierHeightAccepted
  /\ ~PreserveStaleCurrentViewAccepted
  /\ ~PreserveNoActionableAccepted

QuorumOwnerActionableHasExactPositiveEvidence ==
  /\ OwnerActiveExact
  /\ VoteEvidenceExact
  /\ DependencyBacklogExact
  /\ RbcSenderActivityExact
  /\ MissingBlockActivityExact
  /\ MissingCommitActivityExact
  /\ VoteBackedRecoveryExact
  /\ PreserveOwnerSource
  /\ PreserveRecoverySource

FrontierQuorumOwnerCleanupExactness ==
  /\ QuorumOwnerActionableRejectsNonExactInputs
  /\ QuorumOwnerActionableHasExactPositiveEvidence

SafetyFast ==
  /\ ActionableSourceSafety
  /\ LiveCleanupPreserveSafety
  /\ FrontierQuorumOwnerCleanupExactness

ActionableSourceAnchors ==
  /\ ActionableSourceSafety
  /\ OwnerActiveExact
  /\ VoteEvidenceExact
  /\ DependencyBacklogExact
  /\ RbcSenderActivityExact
  /\ MissingBlockActivityExact
  /\ MissingCommitActivityExact
  /\ VoteBackedRecoveryExact
  /\ ~ActionableWithoutSourceAccepted
  /\ ~OwnerWrongViewAccepted
  /\ ~VoteWrongViewAccepted
  /\ ~DependencyStaleAccepted
  /\ ~SenderStaleAccepted
  /\ ~MissingBlockWrongViewAccepted
  /\ ~MissingCommitWrongPhaseAccepted
  /\ ~VoteBackedPassiveAccepted

LiveCleanupPreserveAnchors ==
  /\ LiveCleanupPreserveSafety
  /\ PreserveOwnerSource
  /\ PreserveRecoverySource
  /\ ~PreserveNonFrontierHeightAccepted
  /\ ~PreserveStaleCurrentViewAccepted
  /\ ~PreserveNoActionableAccepted

FrontierQuorumOwnerActionableSafetyAnchors ==
  /\ ActionableSourceAnchors
  /\ LiveCleanupPreserveAnchors

Safety == FrontierQuorumOwnerActionableSafetyAnchors

====
