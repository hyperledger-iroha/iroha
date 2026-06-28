---- MODULE SumeragiProposalHintAdmissionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi proposal-hint admission.

`handle_proposal_hint(...)` accepts only fresh, locally consistent proposal
hints whose highest-QC reference has the expected height/epoch, does not
conflict with committed history, matches local metadata when known, and extends
the locked chain. Missing future highest-QC dependencies are deliberately
dropped as proposal metadata while arming exact repair, and cross-view hints may
be cached only as dependency context. Accepted hints update PRF context, cache
the hint, mark the slot observed, replay deferred votes, and prune old observed
slots; they update the local highest QC only when the incoming reference is
strictly newer or promotes the same height/view to COMMIT.
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
  phase_sampled,
  \* @type: Bool;
  replayed,
  \* @type: Bool;
  pruned,
  \* @type: Bool;
  committed_pruned,
  \* @type: Bool;
  conflict_suppressed

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, accepted, deferred, dropped, cached, observed,
  highest_updated, dependency_requested, defer_marker, prf_updated,
  phase_sampled, replayed, pruned, committed_pruned, conflict_suppressed>>

Cases == {
  "valid_new_highest",
  "valid_same_current",
  "valid_stale_highest",
  "valid_phase_promotion",
  "cached_conflict_committed_replacement",
  "valid_lock_lag_update_defer",
  "stale_height",
  "stale_view",
  "highest_height_mismatch",
  "highest_epoch_mismatch",
  "cached_conflict",
  "stored_height_mismatch",
  "committed_conflict",
  "missing_committed_highest",
  "missing_future_highest_same_view",
  "missing_future_highest_cross_view",
  "local_height_mismatch",
  "local_view_mismatch",
  "locked_qc_reject"
}

AcceptedCases == {
  "valid_new_highest",
  "valid_same_current",
  "valid_stale_highest",
  "valid_phase_promotion",
  "cached_conflict_committed_replacement",
  "valid_lock_lag_update_defer"
}

DeferredCases == {
  "missing_future_highest_same_view",
  "missing_future_highest_cross_view"
}

DroppedOnlyCases == (Cases \ AcceptedCases) \ DeferredCases

HighestUpdateCases == {
  "valid_new_highest",
  "valid_phase_promotion",
  "cached_conflict_committed_replacement"
}

DependencyCases == {
  "missing_future_highest_same_view",
  "missing_future_highest_cross_view",
  "valid_lock_lag_update_defer"
}

CommittedConflictCases == {
  "committed_conflict",
  "missing_committed_highest"
}

SpecAccept(c) == c \in AcceptedCases

ActualAccept(c) ==
  \/ /\ c \in AcceptedCases
     /\ Bug # "drop_accepted_hint"
  \/ /\ c = "stale_height"
     /\ Bug = "accept_stale_height"
  \/ /\ c = "stale_view"
     /\ Bug = "accept_stale_view"
  \/ /\ c = "highest_height_mismatch"
     /\ Bug = "accept_highest_height_mismatch"
  \/ /\ c = "highest_epoch_mismatch"
     /\ Bug = "accept_highest_epoch_mismatch"
  \/ /\ c = "cached_conflict"
     /\ Bug = "accept_cached_conflict"
  \/ /\ c = "stored_height_mismatch"
     /\ Bug = "accept_stored_height_mismatch"
  \/ /\ c = "committed_conflict"
     /\ Bug = "accept_committed_conflict"
  \/ /\ c = "missing_committed_highest"
     /\ Bug = "accept_missing_committed_highest"
  \/ /\ c \in DeferredCases
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

SpecCached(c) ==
  \/ c \in AcceptedCases
  \/ c = "missing_future_highest_cross_view"

ActualCached(c) ==
  IF ActualAccept(c)
  THEN Bug # "skip_cache_on_accept"
  ELSE
    CASE c = "missing_future_highest_cross_view" ->
         /\ ActualDeferred(c)
         /\ Bug # "skip_deferred_cross_view_cache"
      [] c = "missing_future_highest_same_view" ->
         Bug = "cache_same_view_deferred"
      [] OTHER ->
         Bug = "cache_rejected_hint"

SpecObserved(c) == c \in AcceptedCases

ActualObserved(c) ==
  IF ActualAccept(c)
  THEN Bug # "skip_observed_on_accept"
  ELSE Bug = "observe_rejected_hint"

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
  ELSE Bug = "request_dependency_for_clean_hint"

SpecDeferMarker(c) == c \in DependencyCases

ActualDeferMarker(c) ==
  IF c \in DependencyCases /\ (ActualDeferred(c) \/ ActualAccept(c))
  THEN Bug # "skip_defer_marker"
  ELSE Bug = "marker_for_clean_hint"

SpecPrfUpdated(c) ==
  \/ c \in AcceptedCases
  \/ c = "locked_qc_reject"

ActualPrfUpdated(c) ==
  IF c \in AcceptedCases \/ c = "locked_qc_reject"
  THEN Bug # "skip_prf_update"
  ELSE Bug = "prf_update_before_admission"

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
  ELSE Bug = "suppress_clean_hint"

BugModes == {
  "none",
  "drop_accepted_hint",
  "accept_stale_height",
  "accept_stale_view",
  "accept_highest_height_mismatch",
  "accept_highest_epoch_mismatch",
  "accept_cached_conflict",
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
  "skip_deferred_cross_view_cache",
  "cache_same_view_deferred",
  "cache_rejected_hint",
  "skip_observed_on_accept",
  "observe_rejected_hint",
  "skip_highest_update",
  "spurious_highest_update",
  "highest_update_on_reject",
  "skip_dependency_request",
  "request_dependency_for_clean_hint",
  "skip_defer_marker",
  "marker_for_clean_hint",
  "skip_prf_update",
  "prf_update_before_admission",
  "skip_phase_sample",
  "phase_sample_on_reject",
  "skip_replay",
  "replay_on_reject",
  "skip_prune",
  "prune_on_reject",
  "skip_stale_height_prune",
  "prune_committed_on_non_stale",
  "skip_committed_conflict_suppression",
  "suppress_clean_hint"
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
  /\ phase_sampled \in BOOLEAN
  /\ replayed \in BOOLEAN
  /\ pruned \in BOOLEAN
  /\ committed_pruned \in BOOLEAN
  /\ conflict_suppressed \in BOOLEAN

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
  /\ phase_sampled = FALSE
  /\ replayed = FALSE
  /\ pruned = FALSE
  /\ committed_pruned = FALSE
  /\ conflict_suppressed = FALSE

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
  /\ phase_sampled' = ActualPhaseSampled(c)
  /\ replayed' = ActualReplayed(c)
  /\ pruned' = ActualPruned(c)
  /\ committed_pruned' = ActualCommittedPruned(c)
  /\ conflict_suppressed' = ActualConflictSuppressed(c)

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
    /\ ~observed
    /\ ~highest_updated
    /\ ~phase_sampled
    /\ ~replayed
    /\ ~pruned

SameViewDeferredDoesNotCache ==
  candidate = "missing_future_highest_same_view" => ~cached

CrossViewDeferredCachesOnlyDependencyContext ==
  candidate = "missing_future_highest_cross_view" =>
    /\ cached
    /\ ~observed

AcceptedHintsCacheAndObserve ==
  candidate \in AcceptedCases =>
    /\ cached
    /\ observed
    /\ phase_sampled
    /\ replayed
    /\ pruned

AcceptedHintsUpdatePrf ==
  candidate \in AcceptedCases => prf_updated

LockedRejectUpdatesPrfButDoesNotCache ==
  candidate = "locked_qc_reject" =>
    /\ prf_updated
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

CleanHintsDoNotSuppressCommittedConflicts ==
  candidate \in AcceptedCases => ~conflict_suppressed

ProposalHintAdmissionExactness ==
  /\ AcceptMatchesSpec
  /\ DeferredMatchesSpec
  /\ DroppedMatchesSpec
  /\ CacheMatchesSpec
  /\ ObservedMatchesSpec
  /\ HighestUpdateMatchesSpec
  /\ DependencyRequestMatchesSpec
  /\ DeferMarkerMatchesSpec
  /\ PrfUpdateMatchesSpec
  /\ PhaseSampleMatchesSpec
  /\ ReplayMatchesSpec
  /\ PruneMatchesSpec
  /\ CommittedPruneMatchesSpec
  /\ ConflictSuppressionMatchesSpec
  /\ AcceptedCasesAccepted
  /\ DroppedOnlyCasesRejected
  /\ DeferredCasesRemainDropped
  /\ SameViewDeferredDoesNotCache
  /\ CrossViewDeferredCachesOnlyDependencyContext
  /\ AcceptedHintsCacheAndObserve
  /\ AcceptedHintsUpdatePrf
  /\ LockedRejectUpdatesPrfButDoesNotCache
  /\ HighestUpdateOnlyForNewerOrPromotion
  /\ LockLagDefersHighestButKeepsMetadata
  /\ CommittedConflictsAreSuppressedAndDropped
  /\ StaleHeightPrunesCommittedCacheOnly
  /\ CleanHintsDoNotSuppressCommittedConflicts

Safety ==
  ProposalHintAdmissionExactness

ProposalHintAdmissionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ProposalHintAdmissionExactness

====
