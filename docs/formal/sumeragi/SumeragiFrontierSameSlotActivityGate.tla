---- MODULE SumeragiFrontierSameSlotActivityGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for exact-slot frontier recovery activity helpers.

This slice pins the `frontier_recovery_same_slot_*` helper family. Same-slot
activity may suppress rotation only when it belongs to the exact
height/view, is still fresh within the recovery window, and has the required
payload, vote, request, or repair evidence. Old-view or wrong-height activity
must not leak into later views, terminal/passive slot modes must not look live,
and bookkeeping-only timestamp refreshes must not keep stale vote-backed
recovery active.
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
  "payload_reject_cached_evidence",
  "payload_reject_pending_progress",
  "payload_reject_commit_inflight",
  "payload_accept_old_view",
  "payload_accept_wrong_height",
  "payload_accept_finalized_slot",
  "payload_accept_stale_slot",
  "payload_accept_body_present",
  "ingress_accept_no_backlog",
  "ingress_accept_no_payload",
  "vote_reject_slot_evidence",
  "vote_reject_work",
  "vote_accept_old_view",
  "vote_accept_passive_slot",
  "vote_accept_stale_slot",
  "vote_accept_no_evidence",
  "vote_accept_wrong_phase_work",
  "vote_accept_wrong_epoch_work",
  "vote_uses_bookkeeping_refresh",
  "missing_block_reject_valid",
  "missing_block_accept_old_view",
  "missing_block_accept_wrong_phase",
  "missing_block_accept_stale",
  "missing_block_accept_no_actionable",
  "missing_commit_reject_valid",
  "missing_commit_accept_old_view",
  "missing_commit_accept_prepare_phase",
  "missing_commit_accept_stale",
  "missing_commit_accept_no_actionable",
  "missing_payload_reject_slot",
  "missing_payload_reject_deferred",
  "missing_payload_accept_old_view",
  "missing_payload_accept_body_present",
  "missing_payload_accept_wrong_phase",
  "missing_payload_accept_stale",
  "missing_payload_accept_no_actionable"
}

PayloadCachedExact ==
  IF Bug = "payload_reject_cached_evidence" THEN FALSE ELSE TRUE

PayloadPendingExact ==
  IF Bug = "payload_reject_pending_progress" THEN FALSE ELSE TRUE

PayloadCommitInflightExact ==
  IF Bug = "payload_reject_commit_inflight" THEN FALSE ELSE TRUE

PayloadFrontierSlotExact ==
  TRUE

PayloadOldView ==
  IF Bug = "payload_accept_old_view" THEN TRUE ELSE FALSE

PayloadWrongHeight ==
  IF Bug = "payload_accept_wrong_height" THEN TRUE ELSE FALSE

PayloadFinalizedSlot ==
  IF Bug = "payload_accept_finalized_slot" THEN TRUE ELSE FALSE

PayloadStaleSlot ==
  IF Bug = "payload_accept_stale_slot" THEN TRUE ELSE FALSE

PayloadBodyPresent ==
  IF Bug = "payload_accept_body_present" THEN TRUE ELSE FALSE

IngressExact ==
  TRUE

IngressNoBacklog ==
  IF Bug = "ingress_accept_no_backlog" THEN TRUE ELSE FALSE

IngressNoPayloadProgress ==
  IF Bug = "ingress_accept_no_payload" THEN TRUE ELSE FALSE

VoteBackedSlotExact ==
  IF Bug = "vote_reject_slot_evidence" THEN FALSE ELSE TRUE

VoteBackedWorkExact ==
  IF Bug = "vote_reject_work" THEN FALSE ELSE TRUE

VoteBackedOldView ==
  IF Bug = "vote_accept_old_view" THEN TRUE ELSE FALSE

VoteBackedPassiveSlot ==
  IF Bug = "vote_accept_passive_slot" THEN TRUE ELSE FALSE

VoteBackedStaleSlot ==
  IF Bug = "vote_accept_stale_slot" THEN TRUE ELSE FALSE

VoteBackedNoEvidence ==
  IF Bug = "vote_accept_no_evidence" THEN TRUE ELSE FALSE

VoteBackedWrongPhaseWork ==
  IF Bug = "vote_accept_wrong_phase_work" THEN TRUE ELSE FALSE

VoteBackedWrongEpochWork ==
  IF Bug = "vote_accept_wrong_epoch_work" THEN TRUE ELSE FALSE

VoteBackedBookkeepingOnly ==
  IF Bug = "vote_uses_bookkeeping_refresh" THEN TRUE ELSE FALSE

MissingBlockExact ==
  IF Bug = "missing_block_reject_valid" THEN FALSE ELSE TRUE

MissingBlockOldView ==
  IF Bug = "missing_block_accept_old_view" THEN TRUE ELSE FALSE

MissingBlockWrongPhase ==
  IF Bug = "missing_block_accept_wrong_phase" THEN TRUE ELSE FALSE

MissingBlockStale ==
  IF Bug = "missing_block_accept_stale" THEN TRUE ELSE FALSE

MissingBlockNoActionableDependency ==
  IF Bug = "missing_block_accept_no_actionable" THEN TRUE ELSE FALSE

MissingCommitExact ==
  IF Bug = "missing_commit_reject_valid" THEN FALSE ELSE TRUE

MissingCommitOldView ==
  IF Bug = "missing_commit_accept_old_view" THEN TRUE ELSE FALSE

MissingCommitPreparePhase ==
  IF Bug = "missing_commit_accept_prepare_phase" THEN TRUE ELSE FALSE

MissingCommitStale ==
  IF Bug = "missing_commit_accept_stale" THEN TRUE ELSE FALSE

MissingCommitNoActionableDependency ==
  IF Bug = "missing_commit_accept_no_actionable" THEN TRUE ELSE FALSE

MissingPayloadSlotExact ==
  IF Bug = "missing_payload_reject_slot" THEN FALSE ELSE TRUE

MissingPayloadDeferredExact ==
  IF Bug = "missing_payload_reject_deferred" THEN FALSE ELSE TRUE

MissingPayloadOldView ==
  IF Bug = "missing_payload_accept_old_view" THEN TRUE ELSE FALSE

MissingPayloadBodyPresent ==
  IF Bug = "missing_payload_accept_body_present" THEN TRUE ELSE FALSE

MissingPayloadWrongPhase ==
  IF Bug = "missing_payload_accept_wrong_phase" THEN TRUE ELSE FALSE

MissingPayloadStale ==
  IF Bug = "missing_payload_accept_stale" THEN TRUE ELSE FALSE

MissingPayloadNoActionableDependency ==
  IF Bug = "missing_payload_accept_no_actionable" THEN TRUE ELSE FALSE

Init ==
  checked = 0

Next ==
  \/ /\ checked < 36
     /\ checked' = checked + 1
  \/ /\ checked = 36
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..36

PayloadProgressSafety ==
  /\ PayloadCachedExact
  /\ PayloadPendingExact
  /\ PayloadCommitInflightExact
  /\ PayloadFrontierSlotExact
  /\ ~PayloadOldView
  /\ ~PayloadWrongHeight
  /\ ~PayloadFinalizedSlot
  /\ ~PayloadStaleSlot
  /\ ~PayloadBodyPresent

IngressSafety ==
  /\ IngressExact
  /\ ~IngressNoBacklog
  /\ ~IngressNoPayloadProgress

VoteBackedSafety ==
  /\ VoteBackedSlotExact
  /\ VoteBackedWorkExact
  /\ ~VoteBackedOldView
  /\ ~VoteBackedPassiveSlot
  /\ ~VoteBackedStaleSlot
  /\ ~VoteBackedNoEvidence
  /\ ~VoteBackedWrongPhaseWork
  /\ ~VoteBackedWrongEpochWork
  /\ ~VoteBackedBookkeepingOnly

MissingBlockRequestSafety ==
  /\ MissingBlockExact
  /\ ~MissingBlockOldView
  /\ ~MissingBlockWrongPhase
  /\ ~MissingBlockStale
  /\ ~MissingBlockNoActionableDependency

MissingCommitQcRepairSafety ==
  /\ MissingCommitExact
  /\ ~MissingCommitOldView
  /\ ~MissingCommitPreparePhase
  /\ ~MissingCommitStale
  /\ ~MissingCommitNoActionableDependency

MissingPayloadRecoverySafety ==
  /\ MissingPayloadSlotExact
  /\ MissingPayloadDeferredExact
  /\ ~MissingPayloadOldView
  /\ ~MissingPayloadBodyPresent
  /\ ~MissingPayloadWrongPhase
  /\ ~MissingPayloadStale
  /\ ~MissingPayloadNoActionableDependency

SameSlotActivityRejectsNonExactInputs ==
  /\ ~PayloadOldView
  /\ ~PayloadWrongHeight
  /\ ~PayloadFinalizedSlot
  /\ ~PayloadStaleSlot
  /\ ~PayloadBodyPresent
  /\ ~IngressNoBacklog
  /\ ~IngressNoPayloadProgress
  /\ ~VoteBackedOldView
  /\ ~VoteBackedPassiveSlot
  /\ ~VoteBackedStaleSlot
  /\ ~VoteBackedNoEvidence
  /\ ~VoteBackedWrongPhaseWork
  /\ ~VoteBackedWrongEpochWork
  /\ ~VoteBackedBookkeepingOnly
  /\ ~MissingBlockOldView
  /\ ~MissingBlockWrongPhase
  /\ ~MissingBlockStale
  /\ ~MissingBlockNoActionableDependency
  /\ ~MissingCommitOldView
  /\ ~MissingCommitPreparePhase
  /\ ~MissingCommitStale
  /\ ~MissingCommitNoActionableDependency
  /\ ~MissingPayloadOldView
  /\ ~MissingPayloadBodyPresent
  /\ ~MissingPayloadWrongPhase
  /\ ~MissingPayloadStale
  /\ ~MissingPayloadNoActionableDependency

SameSlotActivityHasExactPositiveEvidence ==
  /\ (\/ PayloadCachedExact
      \/ PayloadPendingExact
      \/ PayloadCommitInflightExact
      \/ PayloadFrontierSlotExact)
  /\ IngressExact
  /\ (\/ VoteBackedSlotExact
      \/ VoteBackedWorkExact)
  /\ MissingBlockExact
  /\ MissingCommitExact
  /\ (\/ MissingPayloadSlotExact
      \/ MissingPayloadDeferredExact)

FrontierSameSlotActivityExactness ==
  /\ SameSlotActivityRejectsNonExactInputs
  /\ SameSlotActivityHasExactPositiveEvidence

SafetyFast ==
  /\ PayloadProgressSafety
  /\ IngressSafety
  /\ VoteBackedSafety
  /\ MissingBlockRequestSafety
  /\ MissingCommitQcRepairSafety
  /\ MissingPayloadRecoverySafety
  /\ FrontierSameSlotActivityExactness

PayloadProgressAnchors ==
  /\ PayloadProgressSafety
  /\ PayloadCachedExact
  /\ PayloadPendingExact
  /\ PayloadCommitInflightExact
  /\ PayloadFrontierSlotExact
  /\ ~PayloadOldView
  /\ ~PayloadWrongHeight
  /\ ~PayloadFinalizedSlot
  /\ ~PayloadStaleSlot
  /\ ~PayloadBodyPresent

IngressAnchors ==
  /\ IngressSafety
  /\ IngressExact
  /\ ~IngressNoBacklog
  /\ ~IngressNoPayloadProgress

VoteBackedAnchors ==
  /\ VoteBackedSafety
  /\ VoteBackedSlotExact
  /\ VoteBackedWorkExact
  /\ ~VoteBackedOldView
  /\ ~VoteBackedPassiveSlot
  /\ ~VoteBackedStaleSlot
  /\ ~VoteBackedNoEvidence
  /\ ~VoteBackedWrongPhaseWork
  /\ ~VoteBackedWrongEpochWork
  /\ ~VoteBackedBookkeepingOnly

MissingBlockRequestAnchors ==
  /\ MissingBlockRequestSafety
  /\ MissingBlockExact
  /\ ~MissingBlockOldView
  /\ ~MissingBlockWrongPhase
  /\ ~MissingBlockStale
  /\ ~MissingBlockNoActionableDependency

MissingCommitQcRepairAnchors ==
  /\ MissingCommitQcRepairSafety
  /\ MissingCommitExact
  /\ ~MissingCommitOldView
  /\ ~MissingCommitPreparePhase
  /\ ~MissingCommitStale
  /\ ~MissingCommitNoActionableDependency

MissingPayloadRecoveryAnchors ==
  /\ MissingPayloadRecoverySafety
  /\ MissingPayloadSlotExact
  /\ MissingPayloadDeferredExact
  /\ ~MissingPayloadOldView
  /\ ~MissingPayloadBodyPresent
  /\ ~MissingPayloadWrongPhase
  /\ ~MissingPayloadStale
  /\ ~MissingPayloadNoActionableDependency

FrontierSameSlotActivitySafetyAnchors ==
  /\ PayloadProgressAnchors
  /\ IngressAnchors
  /\ VoteBackedAnchors
  /\ MissingBlockRequestAnchors
  /\ MissingCommitQcRepairAnchors
  /\ MissingPayloadRecoveryAnchors

FrontierSameSlotActivityCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SafetyFast
  /\ FrontierSameSlotActivitySafetyAnchors

Safety == FrontierSameSlotActivitySafetyAnchors

====
