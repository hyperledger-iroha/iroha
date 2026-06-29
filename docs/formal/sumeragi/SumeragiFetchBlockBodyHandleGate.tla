---- MODULE SumeragiFetchBlockBodyHandleGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `handle_fetch_block_body(...)`.

The handler serves exact block-body repair requests from a local certified
block only when the local block hash, height, and view all match the request
and the canonical committed deferral helper says proof material is already
available. Otherwise it records a deferred request:

- exact local matches that are not deferred dispatch through the exact
  BlockBodyResponse/plain-fallback helper and remove the requester from the
  pending-body requester set,
- exact local matches that are deferred stash the requester in the pending-body
  requester set and do not dispatch or remove it,
- local hash hits whose header does not match the request fall through to the
  ordinary stash policy instead of serving mismatched body material,
- a matching active frontier slot captures the requester before the broader
  pending-body window is considered,
- non-frontier requesters are stashed only inside the pending-body height
  window,
- every path releases the ingress dedup key exactly once, and every non-dispatch
  path records a deferred/not-found handling outcome.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "exact_created_dispatch",
  "exact_proof_dispatch",
  "exact_canonical_defer",
  "local_height_mismatch_frontier",
  "local_view_mismatch_window",
  "local_identity_mismatch_outside",
  "no_local_frontier_stash",
  "no_local_frontier_over_window",
  "no_local_window_stash",
  "no_local_outside_window"
}

LocalBlockFound(c) ==
  c \in {
    "exact_created_dispatch",
    "exact_proof_dispatch",
    "exact_canonical_defer",
    "local_height_mismatch_frontier",
    "local_view_mismatch_window",
    "local_identity_mismatch_outside"
  }

IdentityMatches(c) ==
  c \in {
    "exact_created_dispatch",
    "exact_proof_dispatch",
    "exact_canonical_defer"
  }

SpecExactLocal(c) ==
  LocalBlockFound(c) /\ IdentityMatches(c)

SpecShouldDefer(c) ==
  c = "exact_canonical_defer"

FrontierMatches(c) ==
  c \in {
    "local_height_mismatch_frontier",
    "no_local_frontier_stash",
    "no_local_frontier_over_window"
  }

WindowAllows(c) ==
  c \in {
    "local_view_mismatch_window",
    "no_local_frontier_over_window",
    "no_local_window_stash"
  }

SpecDispatch(c) ==
  SpecExactLocal(c) /\ ~SpecShouldDefer(c)

SpecPendingStash(c) ==
  IF SpecExactLocal(c) /\ SpecShouldDefer(c)
  THEN TRUE
  ELSE ~SpecExactLocal(c) /\ ~FrontierMatches(c) /\ WindowAllows(c)

SpecFrontierStash(c) ==
  ~SpecExactLocal(c) /\ FrontierMatches(c)

SpecRemoveRequester(c) ==
  SpecDispatch(c)

SpecDeferredRecord(c) ==
  ~SpecDispatch(c)

SpecDedupReleaseCount(c) ==
  1

SpecDispatchUsesPlainFallbackHelper(c) ==
  SpecDispatch(c)

ActualDispatch(c) ==
  CASE Bug = "dispatch_deferred_canonical"
       /\ c = "exact_canonical_defer" -> TRUE
    [] Bug = "skip_exact_dispatch"
       /\ c = "exact_created_dispatch" -> FALSE
    [] Bug = "serve_identity_mismatch"
       /\ c = "local_height_mismatch_frontier" -> TRUE
    [] Bug = "serve_no_local"
       /\ c = "no_local_outside_window" -> TRUE
    [] OTHER -> SpecDispatch(c)

ActualPendingStash(c) ==
  CASE Bug = "skip_pending_stash_on_deferral"
       /\ c = "exact_canonical_defer" -> FALSE
    [] Bug = "pending_stash_on_dispatch"
       /\ c = "exact_created_dispatch" -> TRUE
    [] Bug = "skip_window_stash"
       /\ c = "no_local_window_stash" -> FALSE
    [] Bug = "stash_outside_window"
       /\ c = "no_local_outside_window" -> TRUE
    [] Bug = "pending_stash_when_frontier_matched"
       /\ c = "no_local_frontier_over_window" -> TRUE
    [] OTHER -> SpecPendingStash(c)

ActualFrontierStash(c) ==
  CASE Bug = "skip_frontier_stash"
       /\ c = "no_local_frontier_stash" -> FALSE
    [] Bug = "frontier_stash_on_exact_dispatch"
       /\ c = "exact_proof_dispatch" -> TRUE
    [] Bug = "frontier_stash_when_exact_deferred"
       /\ c = "exact_canonical_defer" -> TRUE
    [] OTHER -> SpecFrontierStash(c)

ActualRemoveRequester(c) ==
  CASE Bug = "skip_remove_on_dispatch"
       /\ c = "exact_proof_dispatch" -> FALSE
    [] Bug = "remove_requester_on_deferral"
       /\ c = "exact_canonical_defer" -> TRUE
    [] OTHER -> SpecRemoveRequester(c)

ActualDeferredRecord(c) ==
  CASE Bug = "record_deferred_on_dispatch"
       /\ c = "exact_created_dispatch" -> TRUE
    [] Bug = "skip_deferred_record_no_local"
       /\ c = "no_local_outside_window" -> FALSE
    [] Bug = "skip_deferred_record_frontier"
       /\ c = "no_local_frontier_stash" -> FALSE
    [] OTHER -> SpecDeferredRecord(c)

ActualDedupReleaseCount(c) ==
  CASE Bug = "skip_dedup_release_dispatch"
       /\ c = "exact_created_dispatch" -> 0
    [] Bug = "skip_dedup_release_deferred"
       /\ c = "exact_canonical_defer" -> 0
    [] Bug = "double_dedup_release_window"
       /\ c = "no_local_window_stash" -> 2
    [] OTHER -> SpecDedupReleaseCount(c)

ActualDispatchUsesPlainFallbackHelper(c) ==
  CASE Bug = "dispatch_without_plain_fallback_helper"
       /\ c = "exact_proof_dispatch" -> FALSE
    [] Bug = "plain_fallback_helper_without_dispatch"
       /\ c = "no_local_window_stash" -> TRUE
    [] OTHER -> SpecDispatchUsesPlainFallbackHelper(c)

Matches(c) ==
  /\ ActualDispatch(c) = SpecDispatch(c)
  /\ ActualPendingStash(c) = SpecPendingStash(c)
  /\ ActualFrontierStash(c) = SpecFrontierStash(c)
  /\ ActualRemoveRequester(c) = SpecRemoveRequester(c)
  /\ ActualDeferredRecord(c) = SpecDeferredRecord(c)
  /\ ActualDedupReleaseCount(c) = SpecDedupReleaseCount(c)
  /\ ActualDispatchUsesPlainFallbackHelper(c) = SpecDispatchUsesPlainFallbackHelper(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "dispatch_deferred_canonical",
       "skip_exact_dispatch",
       "serve_identity_mismatch",
       "serve_no_local",
       "skip_pending_stash_on_deferral",
       "pending_stash_on_dispatch",
       "skip_window_stash",
       "stash_outside_window",
       "pending_stash_when_frontier_matched",
       "skip_frontier_stash",
       "frontier_stash_on_exact_dispatch",
       "frontier_stash_when_exact_deferred",
       "skip_remove_on_dispatch",
       "remove_requester_on_deferral",
       "record_deferred_on_dispatch",
       "skip_deferred_record_no_local",
       "skip_deferred_record_frontier",
       "skip_dedup_release_dispatch",
       "skip_dedup_release_deferred",
       "double_dedup_release_window",
       "dispatch_without_plain_fallback_helper",
       "plain_fallback_helper_without_dispatch"
     }
  /\ checked = 0

FetchBlockBodyHandleMatchesSpec ==
  \A c \in Cases: Matches(c)

FetchBlockBodyHandleExactness ==
  /\ FetchBlockBodyHandleMatchesSpec

FetchBlockBodyHandleCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ FetchBlockBodyHandleExactness

SafetyFast ==
  FetchBlockBodyHandleExactness

DeferredCanonicalNotDispatched ==
  Matches("exact_canonical_defer")

ExactResponseDispatched ==
  Matches("exact_created_dispatch")

ProofResponseDispatched ==
  Matches("exact_proof_dispatch")

IdentityMismatchNotServed ==
  Matches("local_height_mismatch_frontier")

NoLocalNotServed ==
  Matches("no_local_outside_window")

DeferralStashesPending ==
  Matches("exact_canonical_defer")

DispatchDoesNotStashPending ==
  Matches("exact_created_dispatch")

WindowRequesterStashed ==
  Matches("no_local_window_stash")

OutsideWindowNotStashed ==
  Matches("no_local_outside_window")

FrontierBeatsWindowStash ==
  Matches("no_local_frontier_over_window")

FrontierRequesterStashed ==
  Matches("no_local_frontier_stash")

ExactDispatchDoesNotStashFrontier ==
  Matches("exact_proof_dispatch")

DeferralUsesPendingNotFrontier ==
  Matches("exact_canonical_defer")

DispatchRemovesRequester ==
  Matches("exact_proof_dispatch")

DeferralKeepsRequester ==
  Matches("exact_canonical_defer")

DispatchNotDeferred ==
  Matches("exact_created_dispatch")

NoLocalDeferredRecorded ==
  Matches("no_local_outside_window")

DedupReleasedOnDispatch ==
  Matches("exact_created_dispatch")

DedupReleasedOnDeferral ==
  Matches("exact_canonical_defer")

DedupReleasedOnceOnWindowStash ==
  Matches("no_local_window_stash")

DispatchUsesPlainFallbackHelper ==
  Matches("exact_proof_dispatch")

NoHelperWithoutDispatch ==
  Matches("no_local_window_stash")

====
