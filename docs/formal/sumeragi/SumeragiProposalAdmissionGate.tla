---- MODULE SumeragiProposalAdmissionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi proposal metadata admission.

`handle_proposal(...)` accepts only fresh proposal metadata whose proposal epoch,
highest-QC reference, parent hash, committed-edge relationship, and local parent
metadata are consistent. Missing future highest-QC parents are dropped as
accepted metadata while exact repair is armed. Accepted proposals update PRF
context, mark the proposal slot observed, cache the proposal, replay deferred
votes, and prune old observed slots. Proposal metadata alone must not wake the
commit pipeline or record payload-phase progress.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  accepted,
  \* @type: Bool;
  deferred,
  \* @type: Bool;
  dropped,
  \* @type: Bool;
  cached,
  \* @type: Bool;
  observed,
  \* @type: Bool;
  highest_updated,
  \* @type: Bool;
  dependency_requested,
  \* @type: Bool;
  defer_marker,
  \* @type: Bool;
  prf_updated,
  \* @type: Bool;
  leader_context_sampled,
  \* @type: Bool;
  phase_sampled,
  \* @type: Bool;
  replayed,
  \* @type: Bool;
  pruned,
  \* @type: Bool;
  committed_pruned,
  \* @type: Bool;
  conflict_suppressed,
  \* @type: Bool;
  commit_pipeline_woken,
  \* @type: Bool;
  payload_phase_recorded

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, accepted, deferred, dropped, cached, observed,
  highest_updated, dependency_requested, defer_marker, prf_updated,
  leader_context_sampled, phase_sampled, replayed, pruned, committed_pruned,
  conflict_suppressed, commit_pipeline_woken, payload_phase_recorded>>

Cases == {
  "valid_new_highest",
  "valid_same_current",
  "valid_stale_highest",
  "valid_phase_promotion",
  "valid_lock_lag_update_defer",
  "stale_height",
  "stale_view",
  "proposal_epoch_mismatch",
  "highest_height_mismatch",
  "highest_epoch_mismatch",
  "parent_hash_mismatch",
  "stored_height_mismatch",
  "committed_conflict",
  "missing_committed_highest",
  "missing_future_highest",
  "local_height_mismatch",
  "local_view_mismatch",
  "locked_qc_reject"
}

AcceptedCases == {
  "valid_new_highest",
  "valid_same_current",
  "valid_stale_highest",
  "valid_phase_promotion",
  "valid_lock_lag_update_defer"
}

DeferredCases == {"missing_future_highest"}

DroppedOnlyCases == (Cases \ AcceptedCases) \ DeferredCases

HighestUpdateCases == {
  "valid_new_highest",
  "valid_phase_promotion"
}

DependencyCases == {
  "missing_future_highest",
  "valid_lock_lag_update_defer"
}

CommittedConflictCases == {
  "committed_conflict",
  "missing_committed_highest"
}

LeaderContextCases == AcceptedCases \union {"locked_qc_reject"}

SpecAccept(c) == c \in AcceptedCases

ActualAccept(c) ==
  \/ /\ c \in AcceptedCases
     /\ Bug # "drop_accepted_proposal"
  \/ /\ c = "stale_height"
     /\ Bug = "accept_stale_height"
  \/ /\ c = "stale_view"
     /\ Bug = "accept_stale_view"
  \/ /\ c = "proposal_epoch_mismatch"
     /\ Bug = "accept_proposal_epoch_mismatch"
  \/ /\ c = "highest_height_mismatch"
     /\ Bug = "accept_highest_height_mismatch"
  \/ /\ c = "highest_epoch_mismatch"
     /\ Bug = "accept_highest_epoch_mismatch"
  \/ /\ c = "parent_hash_mismatch"
     /\ Bug = "accept_parent_hash_mismatch"
  \/ /\ c = "stored_height_mismatch"
     /\ Bug = "accept_stored_height_mismatch"
  \/ /\ c = "committed_conflict"
     /\ Bug = "accept_committed_conflict"
  \/ /\ c = "missing_committed_highest"
     /\ Bug = "accept_missing_committed_highest"
  \/ /\ c = "missing_future_highest"
     /\ Bug = "accept_missing_future_highest"
  \/ /\ c = "local_height_mismatch"
     /\ Bug = "accept_local_height_mismatch"
  \/ /\ c = "local_view_mismatch"
     /\ Bug = "accept_local_view_mismatch"
  \/ /\ c = "locked_qc_reject"
     /\ Bug = "accept_locked_qc_reject"

SpecDeferred(c) == c \in DeferredCases

ActualDeferred(c) ==
  /\ c \in DeferredCases
  /\ ~ActualAccept(c)
  /\ Bug # "skip_defer_dependency"

SpecDropped(c) == ~SpecAccept(c)

ActualDropped(c) ==
  IF ActualAccept(c)
  THEN FALSE
  ELSE Bug # "skip_drop_on_reject"

SpecCached(c) == c \in AcceptedCases

ActualCached(c) ==
  IF ActualAccept(c)
  THEN Bug # "skip_cache_on_accept"
  ELSE Bug = "cache_rejected_proposal"

SpecObserved(c) == c \in AcceptedCases

ActualObserved(c) ==
  IF ActualAccept(c)
  THEN Bug # "skip_observed_on_accept"
  ELSE Bug = "observe_rejected_proposal"

SpecHighestUpdated(c) == c \in HighestUpdateCases

ActualHighestUpdated(c) ==
  IF ActualAccept(c)
  THEN
    IF c \in HighestUpdateCases
    THEN Bug # "skip_highest_update"
    ELSE Bug = "spurious_highest_update"
  ELSE Bug = "highest_update_on_reject"

SpecDependencyRequested(c) == c \in DependencyCases

ActualDependencyRequested(c) ==
  IF c \in DependencyCases /\ ~ActualAccept(c)
  THEN Bug # "skip_dependency_request"
  ELSE IF c = "valid_lock_lag_update_defer" /\ ActualAccept(c)
  THEN Bug # "skip_dependency_request"
  ELSE Bug = "request_dependency_for_clean_proposal"

SpecDeferMarker(c) == c \in DependencyCases

ActualDeferMarker(c) ==
  IF c \in DependencyCases /\ (ActualDeferred(c) \/ ActualAccept(c))
  THEN Bug # "skip_defer_marker"
  ELSE Bug = "marker_for_clean_proposal"

SpecPrfUpdated(c) ==
  \/ c \in AcceptedCases
  \/ c = "locked_qc_reject"

ActualPrfUpdated(c) ==
  IF c \in AcceptedCases \/ c = "locked_qc_reject"
  THEN Bug # "skip_prf_update"
  ELSE Bug = "prf_update_before_admission"

SpecLeaderContextSampled(c) == c \in LeaderContextCases

ActualLeaderContextSampled(c) ==
  IF c \in LeaderContextCases
  THEN Bug # "skip_leader_context"
  ELSE Bug = "leader_context_on_reject"

SpecPhaseSampled(c) == c \in AcceptedCases

ActualPhaseSampled(c) ==
  IF ActualAccept(c) THEN Bug # "skip_phase_sample" ELSE Bug = "phase_sample_on_reject"

SpecReplayed(c) == c \in AcceptedCases

ActualReplayed(c) ==
  IF ActualAccept(c) THEN Bug # "skip_replay" ELSE Bug = "replay_on_reject"

SpecPruned(c) == c \in AcceptedCases

ActualPruned(c) ==
  IF ActualAccept(c) THEN Bug # "skip_prune" ELSE Bug = "prune_on_reject"

SpecCommittedPruned(c) == c = "stale_height"

ActualCommittedPruned(c) ==
  IF c = "stale_height" /\ ~ActualAccept(c)
  THEN Bug # "skip_stale_height_prune"
  ELSE Bug = "prune_committed_on_non_stale"

SpecConflictSuppressed(c) == c \in CommittedConflictCases

ActualConflictSuppressed(c) ==
  IF c \in CommittedConflictCases /\ ~ActualAccept(c)
  THEN Bug # "skip_committed_conflict_suppression"
  ELSE Bug = "suppress_clean_proposal"

SpecCommitPipelineWoken(c) == FALSE

ActualCommitPipelineWoken(c) == Bug = "wake_commit_pipeline"

SpecPayloadPhaseRecorded(c) == FALSE

ActualPayloadPhaseRecorded(c) == Bug = "record_payload_phase"

BugModes == {
  "none",
  "drop_accepted_proposal",
  "accept_stale_height",
  "accept_stale_view",
  "accept_proposal_epoch_mismatch",
  "accept_highest_height_mismatch",
  "accept_highest_epoch_mismatch",
  "accept_parent_hash_mismatch",
  "accept_stored_height_mismatch",
  "accept_committed_conflict",
  "accept_missing_committed_highest",
  "accept_missing_future_highest",
  "accept_local_height_mismatch",
  "accept_local_view_mismatch",
  "accept_locked_qc_reject",
  "skip_defer_dependency",
  "skip_drop_on_reject",
  "skip_cache_on_accept",
  "cache_rejected_proposal",
  "skip_observed_on_accept",
  "observe_rejected_proposal",
  "skip_highest_update",
  "spurious_highest_update",
  "highest_update_on_reject",
  "skip_dependency_request",
  "request_dependency_for_clean_proposal",
  "skip_defer_marker",
  "marker_for_clean_proposal",
  "skip_prf_update",
  "prf_update_before_admission",
  "skip_leader_context",
  "leader_context_on_reject",
  "skip_phase_sample",
  "phase_sample_on_reject",
  "skip_replay",
  "replay_on_reject",
  "skip_prune",
  "prune_on_reject",
  "skip_stale_height_prune",
  "prune_committed_on_non_stale",
  "skip_committed_conflict_suppression",
  "suppress_clean_proposal",
  "wake_commit_pipeline",
  "record_payload_phase"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases \union {"none"}
  /\ accepted \in BOOLEAN
  /\ deferred \in BOOLEAN
  /\ dropped \in BOOLEAN
  /\ cached \in BOOLEAN
  /\ observed \in BOOLEAN
  /\ highest_updated \in BOOLEAN
  /\ dependency_requested \in BOOLEAN
  /\ defer_marker \in BOOLEAN
  /\ prf_updated \in BOOLEAN
  /\ leader_context_sampled \in BOOLEAN
  /\ phase_sampled \in BOOLEAN
  /\ replayed \in BOOLEAN
  /\ pruned \in BOOLEAN
  /\ committed_pruned \in BOOLEAN
  /\ conflict_suppressed \in BOOLEAN
  /\ commit_pipeline_woken \in BOOLEAN
  /\ payload_phase_recorded \in BOOLEAN

Init ==
  /\ candidate = "none"
  /\ accepted = FALSE
  /\ deferred = FALSE
  /\ dropped = FALSE
  /\ cached = FALSE
  /\ observed = FALSE
  /\ highest_updated = FALSE
  /\ dependency_requested = FALSE
  /\ defer_marker = FALSE
  /\ prf_updated = FALSE
  /\ leader_context_sampled = FALSE
  /\ phase_sampled = FALSE
  /\ replayed = FALSE
  /\ pruned = FALSE
  /\ committed_pruned = FALSE
  /\ conflict_suppressed = FALSE
  /\ commit_pipeline_woken = FALSE
  /\ payload_phase_recorded = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ accepted' = ActualAccept(c)
  /\ deferred' = ActualDeferred(c)
  /\ dropped' = ActualDropped(c)
  /\ cached' = ActualCached(c)
  /\ observed' = ActualObserved(c)
  /\ highest_updated' = ActualHighestUpdated(c)
  /\ dependency_requested' = ActualDependencyRequested(c)
  /\ defer_marker' = ActualDeferMarker(c)
  /\ prf_updated' = ActualPrfUpdated(c)
  /\ leader_context_sampled' = ActualLeaderContextSampled(c)
  /\ phase_sampled' = ActualPhaseSampled(c)
  /\ replayed' = ActualReplayed(c)
  /\ pruned' = ActualPruned(c)
  /\ committed_pruned' = ActualCommittedPruned(c)
  /\ conflict_suppressed' = ActualConflictSuppressed(c)
  /\ commit_pipeline_woken' = ActualCommitPipelineWoken(c)
  /\ payload_phase_recorded' = ActualPayloadPhaseRecorded(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

AcceptMatchesSpec ==
  candidate = "none" \/ accepted = SpecAccept(candidate)

DeferredMatchesSpec ==
  candidate = "none" \/ deferred = SpecDeferred(candidate)

DroppedMatchesSpec ==
  candidate = "none" \/ dropped = SpecDropped(candidate)

CacheMatchesSpec ==
  candidate = "none" \/ cached = SpecCached(candidate)

ObservedMatchesSpec ==
  candidate = "none" \/ observed = SpecObserved(candidate)

HighestUpdateMatchesSpec ==
  candidate = "none" \/ highest_updated = SpecHighestUpdated(candidate)

DependencyRequestMatchesSpec ==
  candidate = "none" \/ dependency_requested = SpecDependencyRequested(candidate)

DeferMarkerMatchesSpec ==
  candidate = "none" \/ defer_marker = SpecDeferMarker(candidate)

PrfUpdateMatchesSpec ==
  candidate = "none" \/ prf_updated = SpecPrfUpdated(candidate)

LeaderContextMatchesSpec ==
  candidate = "none" \/ leader_context_sampled = SpecLeaderContextSampled(candidate)

PhaseSampleMatchesSpec ==
  candidate = "none" \/ phase_sampled = SpecPhaseSampled(candidate)

ReplayMatchesSpec ==
  candidate = "none" \/ replayed = SpecReplayed(candidate)

PruneMatchesSpec ==
  candidate = "none" \/ pruned = SpecPruned(candidate)

CommittedPruneMatchesSpec ==
  candidate = "none" \/ committed_pruned = SpecCommittedPruned(candidate)

ConflictSuppressionMatchesSpec ==
  candidate = "none" \/ conflict_suppressed = SpecConflictSuppressed(candidate)

CommitPipelineWakeMatchesSpec ==
  candidate = "none" \/ commit_pipeline_woken = SpecCommitPipelineWoken(candidate)

PayloadPhaseMatchesSpec ==
  candidate = "none" \/ payload_phase_recorded = SpecPayloadPhaseRecorded(candidate)

AcceptedCasesAccepted ==
  candidate \in AcceptedCases => accepted

DroppedOnlyCasesRejected ==
  candidate \in DroppedOnlyCases => ~accepted

DeferredCasesRemainDropped ==
  candidate \in DeferredCases =>
    /\ deferred
    /\ dropped
    /\ ~accepted
    /\ dependency_requested
    /\ defer_marker
    /\ ~cached
    /\ ~observed
    /\ ~highest_updated
    /\ ~phase_sampled
    /\ ~replayed
    /\ ~pruned

AcceptedProposalsCacheAndObserve ==
  candidate \in AcceptedCases =>
    /\ cached
    /\ observed
    /\ phase_sampled
    /\ replayed
    /\ pruned

AcceptedProposalsUpdatePrfAndLeaderContext ==
  candidate \in AcceptedCases =>
    /\ prf_updated
    /\ leader_context_sampled

LockedRejectUpdatesPrfAndLeaderOnly ==
  candidate = "locked_qc_reject" =>
    /\ prf_updated
    /\ leader_context_sampled
    /\ dropped
    /\ ~cached
    /\ ~observed

HighestUpdateOnlyForNewerOrPromotion ==
  candidate \in AcceptedCases =>
    (highest_updated <=> candidate \in HighestUpdateCases)

LockLagDefersHighestButKeepsMetadata ==
  candidate = "valid_lock_lag_update_defer" =>
    /\ accepted
    /\ cached
    /\ observed
    /\ dependency_requested
    /\ defer_marker
    /\ ~highest_updated

CommittedConflictsAreSuppressedAndDropped ==
  candidate \in CommittedConflictCases =>
    /\ dropped
    /\ conflict_suppressed
    /\ ~cached
    /\ ~observed

StaleHeightPrunesCommittedCacheOnly ==
  candidate = "stale_height" =>
    /\ dropped
    /\ committed_pruned
    /\ ~cached
    /\ ~observed

CleanProposalsDoNotSuppressCommittedConflicts ==
  candidate \in AcceptedCases => ~conflict_suppressed

ProposalMetadataDoesNotWakeCommitPipeline ==
  ~commit_pipeline_woken

ProposalMetadataDoesNotRecordPayloadPhase ==
  ~payload_phase_recorded

ProposalAdmissionExactness ==
  /\ AcceptMatchesSpec
  /\ DeferredMatchesSpec
  /\ DroppedMatchesSpec
  /\ CacheMatchesSpec
  /\ ObservedMatchesSpec
  /\ HighestUpdateMatchesSpec
  /\ DependencyRequestMatchesSpec
  /\ DeferMarkerMatchesSpec
  /\ PrfUpdateMatchesSpec
  /\ LeaderContextMatchesSpec
  /\ PhaseSampleMatchesSpec
  /\ ReplayMatchesSpec
  /\ PruneMatchesSpec
  /\ CommittedPruneMatchesSpec
  /\ ConflictSuppressionMatchesSpec
  /\ CommitPipelineWakeMatchesSpec
  /\ PayloadPhaseMatchesSpec
  /\ AcceptedCasesAccepted
  /\ DroppedOnlyCasesRejected
  /\ DeferredCasesRemainDropped
  /\ AcceptedProposalsCacheAndObserve
  /\ AcceptedProposalsUpdatePrfAndLeaderContext
  /\ LockedRejectUpdatesPrfAndLeaderOnly
  /\ HighestUpdateOnlyForNewerOrPromotion
  /\ LockLagDefersHighestButKeepsMetadata
  /\ CommittedConflictsAreSuppressedAndDropped
  /\ StaleHeightPrunesCommittedCacheOnly
  /\ CleanProposalsDoNotSuppressCommittedConflicts
  /\ ProposalMetadataDoesNotWakeCommitPipeline
  /\ ProposalMetadataDoesNotRecordPayloadPhase

Safety ==
  ProposalAdmissionExactness

ProposalAdmissionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ProposalAdmissionExactness

====
