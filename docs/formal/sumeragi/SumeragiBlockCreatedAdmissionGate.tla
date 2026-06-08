---- MODULE SumeragiBlockCreatedAdmissionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for direct BlockCreated payload admission.

`handle_block_created_with_preserve_policy(...)` has several admission exits
before full payload/RBC/QC replay work completes. This model captures those
observable decisions: hard drops, duplicate handling, deferred replay while a
payload is already being processed, missing-highest repair, passive retained
same-height branches, authoritative owner updates, proposal-context seeding,
and commit-pipeline wakeup only after payload admission.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  payload_accepted,
  \* @type: Bool;
  deferred,
  \* @type: Bool;
  dropped,
  \* @type: Bool;
  pending_updated,
  \* @type: Bool;
  passive_retained,
  \* @type: Bool;
  authoritative_owned,
  \* @type: Bool;
  duplicate_handled,
  \* @type: Bool;
  replay_preserved,
  \* @type: Bool;
  dependency_requested,
  \* @type: Bool;
  defer_marker,
  \* @type: Bool;
  parent_requested,
  \* @type: Bool;
  gap_requested,
  \* @type: Bool;
  stale_cleanup,
  \* @type: Bool;
  invalid_evidence,
  \* @type: Bool;
  lock_reject_recorded,
  \* @type: Bool;
  proposal_cached,
  \* @type: Bool;
  proposal_observed,
  \* @type: Bool;
  phase_sampled,
  \* @type: Bool;
  commit_pipeline_requested,
  \* @type: Bool;
  missing_request_cleared,
  \* @type: Bool;
  payload_mismatch_recovery

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, payload_accepted, deferred, dropped, pending_updated,
  passive_retained, authoritative_owned, duplicate_handled, replay_preserved,
  dependency_requested, defer_marker, parent_requested, gap_requested,
  stale_cleanup, invalid_evidence, lock_reject_recorded, proposal_cached,
  proposal_observed, phase_sampled, commit_pipeline_requested,
  missing_request_cleared, payload_mismatch_recovery>>

Cases == {
  "valid_authoritative",
  "valid_authoritative_inline_proposal",
  "valid_stale_view_recovery",
  "valid_stale_payload_only",
  "valid_duplicate",
  "valid_revive_aborted",
  "valid_pending_processing_defer",
  "valid_commit_inflight_defer",
  "valid_future_height_request",
  "valid_future_height_gap_request",
  "valid_empty_due_triggers",
  "valid_proposal_mismatch_continue",
  "local_removed",
  "stale_height",
  "stale_view_without_request",
  "lock_rejected_sink",
  "authoritative_owner_conflict",
  "empty_payload_without_triggers",
  "hint_mismatch_fatal",
  "missing_highest_hint",
  "locked_qc_reject_with_hint",
  "locked_qc_reject_no_hint",
  "proposal_mismatch_preserve",
  "rbc_payload_mismatch"
}

PayloadAcceptedCases == {
  "valid_authoritative",
  "valid_authoritative_inline_proposal",
  "valid_stale_view_recovery",
  "valid_stale_payload_only",
  "valid_revive_aborted",
  "valid_future_height_request",
  "valid_future_height_gap_request",
  "valid_empty_due_triggers",
  "valid_proposal_mismatch_continue"
}

ReplayDeferredCases == {
  "valid_pending_processing_defer",
  "valid_commit_inflight_defer",
  "missing_highest_hint"
}

DroppedCases == {
  "valid_duplicate",
  "local_removed",
  "stale_height",
  "stale_view_without_request",
  "lock_rejected_sink",
  "authoritative_owner_conflict",
  "empty_payload_without_triggers",
  "hint_mismatch_fatal",
  "missing_highest_hint",
  "locked_qc_reject_with_hint",
  "locked_qc_reject_no_hint",
  "proposal_mismatch_preserve",
  "rbc_payload_mismatch"
}

PassiveRetainedCases == {"valid_stale_payload_only"}

AuthoritativeOwnerCases == PayloadAcceptedCases \ PassiveRetainedCases

ProposalContextCases == {"valid_authoritative_inline_proposal"}

DependencyCases == {"missing_highest_hint"}

ParentRequestCases == {
  "valid_future_height_request",
  "valid_future_height_gap_request"
}

GapRequestCases == {"valid_future_height_gap_request"}

InvalidEvidenceCases == {
  "empty_payload_without_triggers",
  "valid_proposal_mismatch_continue",
  "proposal_mismatch_preserve"
}

LockRejectCases == {
  "locked_qc_reject_with_hint",
  "locked_qc_reject_no_hint"
}

MissingRequestClearCases ==
  PayloadAcceptedCases \union {
    "valid_duplicate",
    "valid_pending_processing_defer",
    "valid_commit_inflight_defer",
    "authoritative_owner_conflict",
    "empty_payload_without_triggers",
    "proposal_mismatch_preserve",
    "rbc_payload_mismatch"
  }

PayloadMismatchRecoveryCases == {"valid_proposal_mismatch_continue"}

SpecPayloadAccepted(c) == c \in PayloadAcceptedCases

ActualPayloadAccepted(c) ==
  \/ /\ c \in PayloadAcceptedCases
     /\ Bug # "drop_valid_block"
  \/ /\ c = "local_removed"
     /\ Bug = "accept_local_removed"
  \/ /\ c = "stale_height"
     /\ Bug = "accept_stale_height"
  \/ /\ c = "stale_view_without_request"
     /\ Bug = "accept_stale_view_without_request"
  \/ /\ c = "lock_rejected_sink"
     /\ Bug = "accept_lock_rejected_sink"
  \/ /\ c = "authoritative_owner_conflict"
     /\ Bug = "accept_authoritative_owner_conflict"
  \/ /\ c = "empty_payload_without_triggers"
     /\ Bug = "accept_empty_payload_without_triggers"
  \/ /\ c = "hint_mismatch_fatal"
     /\ Bug = "accept_hint_mismatch"
  \/ /\ c = "missing_highest_hint"
     /\ Bug = "accept_missing_highest_hint"
  \/ /\ c = "locked_qc_reject_with_hint"
     /\ Bug = "accept_locked_qc_with_hint"
  \/ /\ c = "locked_qc_reject_no_hint"
     /\ Bug = "accept_locked_qc_no_hint"
  \/ /\ c = "proposal_mismatch_preserve"
     /\ Bug = "accept_proposal_mismatch_preserve"
  \/ /\ c = "rbc_payload_mismatch"
     /\ Bug = "accept_rbc_payload_mismatch"

SpecDeferred(c) == c \in ReplayDeferredCases

ActualDeferred(c) ==
  IF c \in ReplayDeferredCases /\ ~ActualPayloadAccepted(c)
  THEN Bug # "skip_replay_preserve"
  ELSE Bug = "defer_clean_block"

SpecDropped(c) == c \in DroppedCases

ActualDropped(c) ==
  IF ActualPayloadAccepted(c)
  THEN FALSE
  ELSE IF c = "valid_duplicate"
  THEN Bug # "skip_duplicate_drop"
  ELSE IF c \in DroppedCases
  THEN Bug # "skip_drop_on_reject"
  ELSE Bug = "drop_clean_block"

SpecPendingUpdated(c) == c \in PayloadAcceptedCases

ActualPendingUpdated(c) ==
  IF ActualPayloadAccepted(c)
  THEN Bug # "skip_pending_update"
  ELSE Bug = "pending_update_on_reject"

SpecPassiveRetained(c) == c \in PassiveRetainedCases

ActualPassiveRetained(c) ==
  IF ActualPayloadAccepted(c)
  THEN
    IF c \in PassiveRetainedCases
    THEN Bug # "skip_passive_retained"
    ELSE Bug = "retain_clean_as_passive"
  ELSE Bug = "retain_rejected_as_passive"

SpecAuthoritativeOwned(c) == c \in AuthoritativeOwnerCases

ActualAuthoritativeOwned(c) ==
  IF ActualPayloadAccepted(c)
  THEN
    IF c \in PassiveRetainedCases
    THEN Bug = "authority_for_passive_or_reject"
    ELSE Bug # "skip_authoritative_owner"
  ELSE Bug = "authority_for_passive_or_reject"

SpecDuplicateHandled(c) == c = "valid_duplicate"

ActualDuplicateHandled(c) ==
  IF c = "valid_duplicate" /\ ~ActualPayloadAccepted(c)
  THEN Bug # "skip_duplicate_handling"
  ELSE Bug = "duplicate_handling_on_clean_block"

SpecReplayPreserved(c) == c \in ReplayDeferredCases

ActualReplayPreserved(c) ==
  IF c \in ReplayDeferredCases /\ ~ActualPayloadAccepted(c)
  THEN Bug # "skip_replay_preserve"
  ELSE Bug = "preserve_clean_block"

SpecDependencyRequested(c) == c \in DependencyCases

ActualDependencyRequested(c) ==
  IF c \in DependencyCases /\ ~ActualPayloadAccepted(c)
  THEN Bug # "skip_dependency_request"
  ELSE Bug = "request_dependency_for_clean_block"

SpecDeferMarker(c) == c \in DependencyCases

ActualDeferMarker(c) ==
  IF c \in DependencyCases /\ ~ActualPayloadAccepted(c)
  THEN Bug # "skip_defer_marker"
  ELSE Bug = "marker_for_clean_block"

SpecParentRequested(c) == c \in ParentRequestCases

ActualParentRequested(c) ==
  IF ActualPayloadAccepted(c) /\ c \in ParentRequestCases
  THEN Bug # "skip_parent_request"
  ELSE Bug = "request_parent_for_current_height"

SpecGapRequested(c) == c \in GapRequestCases

ActualGapRequested(c) ==
  IF ActualPayloadAccepted(c) /\ c \in GapRequestCases
  THEN Bug # "skip_gap_request"
  ELSE Bug = "request_gap_for_current_height"

SpecStaleCleanup(c) == c = "stale_height"

ActualStaleCleanup(c) ==
  IF c = "stale_height" /\ ~ActualPayloadAccepted(c)
  THEN Bug # "skip_stale_cleanup"
  ELSE Bug = "stale_cleanup_on_fresh"

SpecInvalidEvidence(c) == c \in InvalidEvidenceCases

ActualInvalidEvidence(c) ==
  IF c \in InvalidEvidenceCases
  THEN Bug # "skip_invalid_evidence"
  ELSE Bug = "evidence_on_clean_block"

SpecLockRejectRecorded(c) == c \in LockRejectCases

ActualLockRejectRecorded(c) ==
  IF c \in LockRejectCases /\ ~ActualPayloadAccepted(c)
  THEN Bug # "skip_lock_reject_record"
  ELSE Bug = "lock_reject_on_clean_block"

SpecProposalCached(c) == c \in ProposalContextCases

ActualProposalCached(c) ==
  IF ActualPayloadAccepted(c) /\ c \in ProposalContextCases
  THEN Bug # "skip_proposal_cache"
  ELSE Bug = "cache_proposal_on_reject"

SpecProposalObserved(c) == c \in ProposalContextCases

ActualProposalObserved(c) ==
  IF ActualPayloadAccepted(c) /\ c \in ProposalContextCases
  THEN Bug # "skip_proposal_observed"
  ELSE Bug = "observe_proposal_on_reject"

SpecPhaseSampled(c) == c \in PayloadAcceptedCases

ActualPhaseSampled(c) ==
  IF ActualPayloadAccepted(c)
  THEN Bug # "skip_phase_sample"
  ELSE Bug = "phase_sample_on_reject"

SpecCommitPipelineRequested(c) == c \in PayloadAcceptedCases

ActualCommitPipelineRequested(c) ==
  IF ActualPayloadAccepted(c)
  THEN Bug # "skip_commit_pipeline_request"
  ELSE Bug = "commit_pipeline_on_reject"

SpecMissingRequestCleared(c) == c \in MissingRequestClearCases

ActualMissingRequestCleared(c) ==
  IF c \in MissingRequestClearCases
  THEN Bug # "skip_missing_request_clear"
  ELSE Bug = "clear_missing_dependency_request"

SpecPayloadMismatchRecovery(c) == c \in PayloadMismatchRecoveryCases

ActualPayloadMismatchRecovery(c) ==
  IF c \in PayloadMismatchRecoveryCases
  THEN Bug # "skip_payload_mismatch_recovery"
  ELSE Bug = "payload_mismatch_recovery_on_clean"

BugModes == {
  "none",
  "drop_valid_block",
  "accept_local_removed",
  "accept_stale_height",
  "accept_stale_view_without_request",
  "accept_lock_rejected_sink",
  "accept_authoritative_owner_conflict",
  "accept_empty_payload_without_triggers",
  "accept_hint_mismatch",
  "accept_missing_highest_hint",
  "accept_locked_qc_with_hint",
  "accept_locked_qc_no_hint",
  "accept_proposal_mismatch_preserve",
  "accept_rbc_payload_mismatch",
  "skip_duplicate_drop",
  "skip_drop_on_reject",
  "drop_clean_block",
  "skip_pending_update",
  "pending_update_on_reject",
  "skip_passive_retained",
  "retain_clean_as_passive",
  "retain_rejected_as_passive",
  "skip_authoritative_owner",
  "authority_for_passive_or_reject",
  "skip_duplicate_handling",
  "duplicate_handling_on_clean_block",
  "skip_replay_preserve",
  "defer_clean_block",
  "preserve_clean_block",
  "skip_dependency_request",
  "request_dependency_for_clean_block",
  "skip_defer_marker",
  "marker_for_clean_block",
  "skip_parent_request",
  "request_parent_for_current_height",
  "skip_gap_request",
  "request_gap_for_current_height",
  "skip_stale_cleanup",
  "stale_cleanup_on_fresh",
  "skip_invalid_evidence",
  "evidence_on_clean_block",
  "skip_lock_reject_record",
  "lock_reject_on_clean_block",
  "skip_proposal_cache",
  "cache_proposal_on_reject",
  "skip_proposal_observed",
  "observe_proposal_on_reject",
  "skip_phase_sample",
  "phase_sample_on_reject",
  "skip_commit_pipeline_request",
  "commit_pipeline_on_reject",
  "skip_missing_request_clear",
  "clear_missing_dependency_request",
  "skip_payload_mismatch_recovery",
  "payload_mismatch_recovery_on_clean"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases \union {"none"}
  /\ payload_accepted \in BOOLEAN
  /\ deferred \in BOOLEAN
  /\ dropped \in BOOLEAN
  /\ pending_updated \in BOOLEAN
  /\ passive_retained \in BOOLEAN
  /\ authoritative_owned \in BOOLEAN
  /\ duplicate_handled \in BOOLEAN
  /\ replay_preserved \in BOOLEAN
  /\ dependency_requested \in BOOLEAN
  /\ defer_marker \in BOOLEAN
  /\ parent_requested \in BOOLEAN
  /\ gap_requested \in BOOLEAN
  /\ stale_cleanup \in BOOLEAN
  /\ invalid_evidence \in BOOLEAN
  /\ lock_reject_recorded \in BOOLEAN
  /\ proposal_cached \in BOOLEAN
  /\ proposal_observed \in BOOLEAN
  /\ phase_sampled \in BOOLEAN
  /\ commit_pipeline_requested \in BOOLEAN
  /\ missing_request_cleared \in BOOLEAN
  /\ payload_mismatch_recovery \in BOOLEAN

Init ==
  /\ candidate = "none"
  /\ payload_accepted = FALSE
  /\ deferred = FALSE
  /\ dropped = FALSE
  /\ pending_updated = FALSE
  /\ passive_retained = FALSE
  /\ authoritative_owned = FALSE
  /\ duplicate_handled = FALSE
  /\ replay_preserved = FALSE
  /\ dependency_requested = FALSE
  /\ defer_marker = FALSE
  /\ parent_requested = FALSE
  /\ gap_requested = FALSE
  /\ stale_cleanup = FALSE
  /\ invalid_evidence = FALSE
  /\ lock_reject_recorded = FALSE
  /\ proposal_cached = FALSE
  /\ proposal_observed = FALSE
  /\ phase_sampled = FALSE
  /\ commit_pipeline_requested = FALSE
  /\ missing_request_cleared = FALSE
  /\ payload_mismatch_recovery = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ payload_accepted' = ActualPayloadAccepted(c)
  /\ deferred' = ActualDeferred(c)
  /\ dropped' = ActualDropped(c)
  /\ pending_updated' = ActualPendingUpdated(c)
  /\ passive_retained' = ActualPassiveRetained(c)
  /\ authoritative_owned' = ActualAuthoritativeOwned(c)
  /\ duplicate_handled' = ActualDuplicateHandled(c)
  /\ replay_preserved' = ActualReplayPreserved(c)
  /\ dependency_requested' = ActualDependencyRequested(c)
  /\ defer_marker' = ActualDeferMarker(c)
  /\ parent_requested' = ActualParentRequested(c)
  /\ gap_requested' = ActualGapRequested(c)
  /\ stale_cleanup' = ActualStaleCleanup(c)
  /\ invalid_evidence' = ActualInvalidEvidence(c)
  /\ lock_reject_recorded' = ActualLockRejectRecorded(c)
  /\ proposal_cached' = ActualProposalCached(c)
  /\ proposal_observed' = ActualProposalObserved(c)
  /\ phase_sampled' = ActualPhaseSampled(c)
  /\ commit_pipeline_requested' = ActualCommitPipelineRequested(c)
  /\ missing_request_cleared' = ActualMissingRequestCleared(c)
  /\ payload_mismatch_recovery' = ActualPayloadMismatchRecovery(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

PayloadAcceptanceMatchesSpec ==
  candidate = "none" \/ payload_accepted = SpecPayloadAccepted(candidate)

DeferredMatchesSpec ==
  candidate = "none" \/ deferred = SpecDeferred(candidate)

DroppedMatchesSpec ==
  candidate = "none" \/ dropped = SpecDropped(candidate)

PendingUpdateMatchesSpec ==
  candidate = "none" \/ pending_updated = SpecPendingUpdated(candidate)

PassiveRetainedMatchesSpec ==
  candidate = "none" \/ passive_retained = SpecPassiveRetained(candidate)

AuthoritativeOwnerMatchesSpec ==
  candidate = "none" \/ authoritative_owned = SpecAuthoritativeOwned(candidate)

DuplicateHandlingMatchesSpec ==
  candidate = "none" \/ duplicate_handled = SpecDuplicateHandled(candidate)

ReplayPreserveMatchesSpec ==
  candidate = "none" \/ replay_preserved = SpecReplayPreserved(candidate)

DependencyRequestMatchesSpec ==
  candidate = "none" \/ dependency_requested = SpecDependencyRequested(candidate)

DeferMarkerMatchesSpec ==
  candidate = "none" \/ defer_marker = SpecDeferMarker(candidate)

ParentRequestMatchesSpec ==
  candidate = "none" \/ parent_requested = SpecParentRequested(candidate)

GapRequestMatchesSpec ==
  candidate = "none" \/ gap_requested = SpecGapRequested(candidate)

StaleCleanupMatchesSpec ==
  candidate = "none" \/ stale_cleanup = SpecStaleCleanup(candidate)

InvalidEvidenceMatchesSpec ==
  candidate = "none" \/ invalid_evidence = SpecInvalidEvidence(candidate)

LockRejectMatchesSpec ==
  candidate = "none" \/ lock_reject_recorded = SpecLockRejectRecorded(candidate)

ProposalCacheMatchesSpec ==
  candidate = "none" \/ proposal_cached = SpecProposalCached(candidate)

ProposalObservedMatchesSpec ==
  candidate = "none" \/ proposal_observed = SpecProposalObserved(candidate)

PhaseSampleMatchesSpec ==
  candidate = "none" \/ phase_sampled = SpecPhaseSampled(candidate)

CommitPipelineMatchesSpec ==
  candidate = "none" \/ commit_pipeline_requested = SpecCommitPipelineRequested(candidate)

MissingRequestClearMatchesSpec ==
  candidate = "none" \/ missing_request_cleared = SpecMissingRequestCleared(candidate)

PayloadMismatchRecoveryMatchesSpec ==
  candidate = "none" \/ payload_mismatch_recovery = SpecPayloadMismatchRecovery(candidate)

AcceptedPayloadsUpdatePendingAndWakeCommit ==
  candidate \in PayloadAcceptedCases =>
    /\ payload_accepted
    /\ pending_updated
    /\ phase_sampled
    /\ commit_pipeline_requested

PassiveBranchesDoNotBecomeAuthoritative ==
  candidate \in PassiveRetainedCases =>
    /\ passive_retained
    /\ ~authoritative_owned

AuthoritativePayloadsOwnSlot ==
  candidate \in AuthoritativeOwnerCases => authoritative_owned

DuplicateDropsButRefreshesPayloadState ==
  candidate = "valid_duplicate" =>
    /\ dropped
    /\ duplicate_handled
    /\ ~pending_updated
    /\ missing_request_cleared

DeferredReplayDoesNotMutatePending ==
  candidate \in ReplayDeferredCases =>
    /\ deferred
    /\ replay_preserved
    /\ ~pending_updated
    /\ ~phase_sampled
    /\ ~commit_pipeline_requested

MissingHighestArmsDependencyRepair ==
  candidate = "missing_highest_hint" =>
    /\ dependency_requested
    /\ defer_marker
    /\ dropped

FutureHeightRequestsParentsBeforeAdmission ==
  candidate \in ParentRequestCases => parent_requested

FutureHeightGapRequestsRangeRepair ==
  candidate \in GapRequestCases => gap_requested

RejectedBlocksDoNotWakeCommitPipeline ==
  candidate \in DroppedCases => ~commit_pipeline_requested

LockRejectedBlocksRecordLockRejection ==
  candidate \in LockRejectCases => lock_reject_recorded

InvalidPayloadsEmitEvidenceWhenProposalContextExists ==
  candidate \in InvalidEvidenceCases => invalid_evidence

InlineProposalContextIsCachedAndObserved ==
  candidate \in ProposalContextCases =>
    /\ proposal_cached
    /\ proposal_observed

BlockCreatedAdmissionOutcomeExactness ==
  /\ PayloadAcceptanceMatchesSpec
  /\ DeferredMatchesSpec
  /\ DroppedMatchesSpec

BlockCreatedAdmissionStateExactness ==
  /\ PendingUpdateMatchesSpec
  /\ PassiveRetainedMatchesSpec
  /\ AuthoritativeOwnerMatchesSpec
  /\ DuplicateHandlingMatchesSpec

BlockCreatedAdmissionReplayRepairExactness ==
  /\ ReplayPreserveMatchesSpec
  /\ DependencyRequestMatchesSpec
  /\ DeferMarkerMatchesSpec
  /\ ParentRequestMatchesSpec
  /\ GapRequestMatchesSpec
  /\ StaleCleanupMatchesSpec

BlockCreatedAdmissionEvidenceExactness ==
  /\ InvalidEvidenceMatchesSpec
  /\ LockRejectMatchesSpec
  /\ ProposalCacheMatchesSpec
  /\ ProposalObservedMatchesSpec
  /\ PayloadMismatchRecoveryMatchesSpec

BlockCreatedAdmissionCommitExactness ==
  /\ PhaseSampleMatchesSpec
  /\ CommitPipelineMatchesSpec
  /\ MissingRequestClearMatchesSpec
  /\ AcceptedPayloadsUpdatePendingAndWakeCommit
  /\ PassiveBranchesDoNotBecomeAuthoritative
  /\ AuthoritativePayloadsOwnSlot
  /\ DuplicateDropsButRefreshesPayloadState
  /\ DeferredReplayDoesNotMutatePending
  /\ MissingHighestArmsDependencyRepair
  /\ FutureHeightRequestsParentsBeforeAdmission
  /\ FutureHeightGapRequestsRangeRepair
  /\ RejectedBlocksDoNotWakeCommitPipeline
  /\ LockRejectedBlocksRecordLockRejection
  /\ InvalidPayloadsEmitEvidenceWhenProposalContextExists
  /\ InlineProposalContextIsCachedAndObserved

BlockCreatedAdmissionExactness ==
  /\ BlockCreatedAdmissionOutcomeExactness
  /\ BlockCreatedAdmissionStateExactness
  /\ BlockCreatedAdmissionReplayRepairExactness
  /\ BlockCreatedAdmissionEvidenceExactness
  /\ BlockCreatedAdmissionCommitExactness

Safety == BlockCreatedAdmissionExactness

====
