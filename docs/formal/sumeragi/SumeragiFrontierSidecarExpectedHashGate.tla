---- MODULE SumeragiFrontierSidecarExpectedHashGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for contiguous-frontier sidecar expected-hash hints.

This slice pins `contiguous_frontier_sidecar_expected_hash_hint(...)` and
`sidecar_commit_qc_view_for_hash(...)`. The expected-hash hint must select the
first valid source in order: unresolved tracked missing-block requests, deferred
frontier QC payload hints, exact unresolved observed heads, then exact
unresolved Prepare/Commit QCs from the cache. Within tracked requests and QC
cache entries, selection is deterministic by phase rank, view, and hash. The
sidecar commit-QC view helper accepts only Commit QCs for the sidecar height and
the sidecar block hash.
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
  "tracked_reject_exact",
  "tracked_accept_wrong_height",
  "tracked_accept_authoritative",
  "tracked_select_lower_phase",
  "tracked_select_lower_view",
  "tracked_select_lower_hash",
  "tracked_precedence_lost_to_deferred",
  "deferred_reject_exact",
  "deferred_precedence_lost_to_observed",
  "observed_reject_exact",
  "observed_accept_wrong_height",
  "observed_accept_authoritative",
  "cache_reject_exact_prepare",
  "cache_reject_exact_commit",
  "cache_accept_new_view",
  "cache_accept_wrong_height",
  "cache_accept_authoritative",
  "cache_select_lower_phase",
  "cache_select_lower_view",
  "cache_select_lower_hash",
  "commit_qc_reject_exact",
  "commit_qc_accept_prepare",
  "commit_qc_accept_wrong_height",
  "commit_qc_accept_wrong_hash",
  "commit_qc_accept_absent"
}

TrackedExactSelected ==
  IF Bug = "tracked_reject_exact" THEN FALSE ELSE TRUE

TrackedWrongHeightRejected ==
  IF Bug = "tracked_accept_wrong_height" THEN FALSE ELSE TRUE

TrackedAuthoritativeRejected ==
  IF Bug = "tracked_accept_authoritative" THEN FALSE ELSE TRUE

TrackedHigherPhaseSelected ==
  IF Bug = "tracked_select_lower_phase" THEN FALSE ELSE TRUE

TrackedHigherViewSelected ==
  IF Bug = "tracked_select_lower_view" THEN FALSE ELSE TRUE

TrackedHigherHashSelected ==
  IF Bug = "tracked_select_lower_hash" THEN FALSE ELSE TRUE

TrackedPrecedesDeferred ==
  IF Bug = "tracked_precedence_lost_to_deferred" THEN FALSE ELSE TRUE

DeferredExactSelected ==
  IF Bug = "deferred_reject_exact" THEN FALSE ELSE TRUE

DeferredPrecedesObserved ==
  IF Bug = "deferred_precedence_lost_to_observed" THEN FALSE ELSE TRUE

ObservedExactSelected ==
  IF Bug = "observed_reject_exact" THEN FALSE ELSE TRUE

ObservedWrongHeightRejected ==
  IF Bug = "observed_accept_wrong_height" THEN FALSE ELSE TRUE

ObservedAuthoritativeRejected ==
  IF Bug = "observed_accept_authoritative" THEN FALSE ELSE TRUE

CachePrepareExactSelected ==
  IF Bug = "cache_reject_exact_prepare" THEN FALSE ELSE TRUE

CacheCommitExactSelected ==
  IF Bug = "cache_reject_exact_commit" THEN FALSE ELSE TRUE

CacheNewViewRejected ==
  IF Bug = "cache_accept_new_view" THEN FALSE ELSE TRUE

CacheWrongHeightRejected ==
  IF Bug = "cache_accept_wrong_height" THEN FALSE ELSE TRUE

CacheAuthoritativeRejected ==
  IF Bug = "cache_accept_authoritative" THEN FALSE ELSE TRUE

CacheHigherPhaseSelected ==
  IF Bug = "cache_select_lower_phase" THEN FALSE ELSE TRUE

CacheHigherViewSelected ==
  IF Bug = "cache_select_lower_view" THEN FALSE ELSE TRUE

CacheHigherHashSelected ==
  IF Bug = "cache_select_lower_hash" THEN FALSE ELSE TRUE

SidecarCommitQcExactAccepted ==
  IF Bug = "commit_qc_reject_exact" THEN FALSE ELSE TRUE

SidecarPrepareQcRejected ==
  IF Bug = "commit_qc_accept_prepare" THEN FALSE ELSE TRUE

SidecarWrongHeightQcRejected ==
  IF Bug = "commit_qc_accept_wrong_height" THEN FALSE ELSE TRUE

SidecarWrongHashQcRejected ==
  IF Bug = "commit_qc_accept_wrong_hash" THEN FALSE ELSE TRUE

SidecarAbsentQcRejected ==
  IF Bug = "commit_qc_accept_absent" THEN FALSE ELSE TRUE

Init ==
  checked = 0

Next ==
  \/ /\ checked < 25
     /\ checked' = checked + 1
  \/ /\ checked = 25
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..25

TrackedHintSafety ==
  /\ TrackedExactSelected
  /\ TrackedWrongHeightRejected
  /\ TrackedAuthoritativeRejected
  /\ TrackedHigherPhaseSelected
  /\ TrackedHigherViewSelected
  /\ TrackedHigherHashSelected
  /\ TrackedPrecedesDeferred

DeferredHintSafety ==
  /\ DeferredExactSelected
  /\ DeferredPrecedesObserved

ObservedHeadSafety ==
  /\ ObservedExactSelected
  /\ ObservedWrongHeightRejected
  /\ ObservedAuthoritativeRejected

CacheHintSafety ==
  /\ CachePrepareExactSelected
  /\ CacheCommitExactSelected
  /\ CacheNewViewRejected
  /\ CacheWrongHeightRejected
  /\ CacheAuthoritativeRejected
  /\ CacheHigherPhaseSelected
  /\ CacheHigherViewSelected
  /\ CacheHigherHashSelected

SidecarCommitQcSafety ==
  /\ SidecarCommitQcExactAccepted
  /\ SidecarPrepareQcRejected
  /\ SidecarWrongHeightQcRejected
  /\ SidecarWrongHashQcRejected
  /\ SidecarAbsentQcRejected

SafetyFast ==
  /\ TrackedHintSafety
  /\ DeferredHintSafety
  /\ ObservedHeadSafety
  /\ CacheHintSafety
  /\ SidecarCommitQcSafety

TrackedHintAnchors ==
  /\ TrackedHintSafety
  /\ TrackedExactSelected
  /\ TrackedWrongHeightRejected
  /\ TrackedAuthoritativeRejected
  /\ TrackedHigherPhaseSelected
  /\ TrackedHigherViewSelected
  /\ TrackedHigherHashSelected
  /\ TrackedPrecedesDeferred

DeferredHintAnchors ==
  /\ DeferredHintSafety
  /\ DeferredExactSelected
  /\ DeferredPrecedesObserved

ObservedHeadAnchors ==
  /\ ObservedHeadSafety
  /\ ObservedExactSelected
  /\ ObservedWrongHeightRejected
  /\ ObservedAuthoritativeRejected

CacheHintAnchors ==
  /\ CacheHintSafety
  /\ CachePrepareExactSelected
  /\ CacheCommitExactSelected
  /\ CacheNewViewRejected
  /\ CacheWrongHeightRejected
  /\ CacheAuthoritativeRejected
  /\ CacheHigherPhaseSelected
  /\ CacheHigherViewSelected
  /\ CacheHigherHashSelected

SidecarCommitQcAnchors ==
  /\ SidecarCommitQcSafety
  /\ SidecarCommitQcExactAccepted
  /\ SidecarPrepareQcRejected
  /\ SidecarWrongHeightQcRejected
  /\ SidecarWrongHashQcRejected
  /\ SidecarAbsentQcRejected

FrontierSidecarExpectedHashSafetyAnchors ==
  /\ TrackedHintAnchors
  /\ DeferredHintAnchors
  /\ ObservedHeadAnchors
  /\ CacheHintAnchors
  /\ SidecarCommitQcAnchors

Safety == FrontierSidecarExpectedHashSafetyAnchors

====
