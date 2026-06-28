---- MODULE SumeragiPendingResponseFlushGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the pending response readiness wrappers:

- `flush_pending_fetch_requests_if_ready(...)`
- `flush_pending_block_body_requests_if_ready(...)`

Both helpers first require a pending map entry for the block hash, then build
the canonical recovery payload and honor canonical committed-response deferral.
When ready, the fetch path removes the pending fetch entry and calls the batch
response helper with all bypass allowances disabled. The exact body path removes
the pending body-request entry, constructs a BlockBodyResponse from the block
hash, height, view, and payload, and dispatches it through the plain-fallback
helper to exactly the recorded requesters.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PeerIds == {"a", "b", "c"}

Cases == {
  "fetch_absent",
  "fetch_deferred",
  "fetch_ready_one",
  "fetch_ready_empty_entry",
  "body_absent",
  "body_deferred",
  "body_ready_two",
  "body_ready_empty_entry"
}

IsFetch(c) ==
  c \in {
    "fetch_absent",
    "fetch_deferred",
    "fetch_ready_one",
    "fetch_ready_empty_entry"
  }

IsBody(c) ==
  ~IsFetch(c)

HasPendingKey(c) ==
  c \notin {"fetch_absent", "body_absent"}

Deferred(c) ==
  c \in {"fetch_deferred", "body_deferred"}

Ready(c) ==
  HasPendingKey(c) /\ ~Deferred(c)

FetchRequester(c, p) ==
  /\ IsFetch(c)
  /\ c \in {"fetch_deferred", "fetch_ready_one"}
  /\ p = "a"

BodyRequester(c, p) ==
  /\ IsBody(c)
  /\ c \in {"body_deferred", "body_ready_two"}
  /\ p \in {"a", "b"}

SpecReturn(c) ==
  Ready(c)

SpecBuildPayload(c) ==
  HasPendingKey(c)

SpecFetchRemoved(c) ==
  IsFetch(c) /\ Ready(c)

SpecBodyRemoved(c) ==
  IsBody(c) /\ Ready(c)

SpecFetchBatchCalled(c) ==
  IsFetch(c) /\ Ready(c)

SpecFetchBatchPeer(c, p) ==
  SpecFetchBatchCalled(c) /\ FetchRequester(c, p)

SpecFetchBatchForceArg(c) ==
  FALSE

SpecFetchBatchAllowHighestArg(c) ==
  FALSE

SpecFetchBatchAllowHintlessArg(c) ==
  FALSE

SpecBodyResponseConstructed(c) ==
  IsBody(c) /\ Ready(c)

SpecBodyResponseHashOk(c) ==
  SpecBodyResponseConstructed(c)

SpecBodyResponseHeightOk(c) ==
  SpecBodyResponseConstructed(c)

SpecBodyResponseViewOk(c) ==
  SpecBodyResponseConstructed(c)

SpecBodyResponsePayloadOk(c) ==
  SpecBodyResponseConstructed(c)

SpecBodyDispatch(c, p) ==
  SpecBodyResponseConstructed(c) /\ BodyRequester(c, p)

SpecBodyAllDispatchesUseFallback(c) ==
  TRUE

ActualReturn(c) ==
  CASE Bug = "fetch_absent_returns_true"
       /\ c = "fetch_absent" -> TRUE
    [] Bug = "fetch_deferred_returns_true"
       /\ c = "fetch_deferred" -> TRUE
    [] Bug = "fetch_ready_returns_false"
       /\ c = "fetch_ready_one" -> FALSE
    [] Bug = "fetch_empty_entry_treated_absent"
       /\ c = "fetch_ready_empty_entry" -> FALSE
    [] Bug = "body_absent_returns_true"
       /\ c = "body_absent" -> TRUE
    [] Bug = "body_deferred_returns_true"
       /\ c = "body_deferred" -> TRUE
    [] Bug = "body_ready_returns_false"
       /\ c = "body_ready_two" -> FALSE
    [] Bug = "body_empty_entry_treated_absent"
       /\ c = "body_ready_empty_entry" -> FALSE
    [] OTHER -> SpecReturn(c)

ActualBuildPayload(c) ==
  CASE Bug = "fetch_absent_builds_payload"
       /\ c = "fetch_absent" -> TRUE
    [] Bug = "fetch_ready_skips_build"
       /\ c = "fetch_ready_one" -> FALSE
    [] Bug = "fetch_empty_entry_treated_absent"
       /\ c = "fetch_ready_empty_entry" -> FALSE
    [] Bug = "body_absent_builds_payload"
       /\ c = "body_absent" -> TRUE
    [] Bug = "body_ready_skips_build"
       /\ c = "body_ready_two" -> FALSE
    [] Bug = "body_empty_entry_treated_absent"
       /\ c = "body_ready_empty_entry" -> FALSE
    [] OTHER -> SpecBuildPayload(c)

ActualFetchRemoved(c) ==
  CASE Bug = "fetch_deferred_removes_pending"
       /\ c = "fetch_deferred" -> TRUE
    [] Bug = "fetch_ready_keeps_pending"
       /\ c = "fetch_ready_one" -> FALSE
    [] Bug = "fetch_empty_entry_treated_absent"
       /\ c = "fetch_ready_empty_entry" -> FALSE
    [] OTHER -> SpecFetchRemoved(c)

ActualBodyRemoved(c) ==
  CASE Bug = "body_deferred_removes_pending"
       /\ c = "body_deferred" -> TRUE
    [] Bug = "body_ready_keeps_pending"
       /\ c = "body_ready_two" -> FALSE
    [] Bug = "body_empty_entry_treated_absent"
       /\ c = "body_ready_empty_entry" -> FALSE
    [] OTHER -> SpecBodyRemoved(c)

ActualFetchBatchCalled(c) ==
  CASE Bug = "fetch_ready_skips_batch"
       /\ c = "fetch_ready_one" -> FALSE
    [] Bug = "fetch_empty_entry_treated_absent"
       /\ c = "fetch_ready_empty_entry" -> FALSE
    [] OTHER -> SpecFetchBatchCalled(c)

ActualFetchBatchPeer(c, p) ==
  CASE Bug = "fetch_batch_drops_requester"
       /\ c = "fetch_ready_one"
       /\ p = "a" -> FALSE
    [] Bug = "fetch_batch_adds_nonrequester"
       /\ c = "fetch_ready_one"
       /\ p = "b" -> TRUE
    [] OTHER -> ActualFetchBatchCalled(c) /\ FetchRequester(c, p)

ActualFetchBatchForceArg(c) ==
  CASE Bug = "fetch_ready_force_bypass_true"
       /\ c = "fetch_ready_one" -> TRUE
    [] OTHER -> FALSE

ActualFetchBatchAllowHighestArg(c) ==
  CASE Bug = "fetch_ready_allow_highest_true"
       /\ c = "fetch_ready_one" -> TRUE
    [] OTHER -> FALSE

ActualFetchBatchAllowHintlessArg(c) ==
  CASE Bug = "fetch_ready_allow_hintless_true"
       /\ c = "fetch_ready_one" -> TRUE
    [] OTHER -> FALSE

ActualBodyResponseConstructed(c) ==
  CASE Bug = "body_ready_skips_response"
       /\ c = "body_ready_two" -> FALSE
    [] Bug = "body_empty_entry_treated_absent"
       /\ c = "body_ready_empty_entry" -> FALSE
    [] OTHER -> SpecBodyResponseConstructed(c)

ActualBodyResponseHashOk(c) ==
  IF ~ActualBodyResponseConstructed(c) THEN FALSE
  ELSE CASE Bug = "body_response_wrong_hash"
            /\ c = "body_ready_two" -> FALSE
         [] OTHER -> TRUE

ActualBodyResponseHeightOk(c) ==
  IF ~ActualBodyResponseConstructed(c) THEN FALSE
  ELSE CASE Bug = "body_response_wrong_height"
            /\ c = "body_ready_two" -> FALSE
         [] OTHER -> TRUE

ActualBodyResponseViewOk(c) ==
  IF ~ActualBodyResponseConstructed(c) THEN FALSE
  ELSE CASE Bug = "body_response_wrong_view"
            /\ c = "body_ready_two" -> FALSE
         [] OTHER -> TRUE

ActualBodyResponsePayloadOk(c) ==
  IF ~ActualBodyResponseConstructed(c) THEN FALSE
  ELSE CASE Bug = "body_response_wrong_payload"
            /\ c = "body_ready_two" -> FALSE
         [] OTHER -> TRUE

ActualBodyDispatch(c, p) ==
  CASE Bug = "body_ready_skips_dispatch_peer"
       /\ c = "body_ready_two"
       /\ p = "a" -> FALSE
    [] Bug = "body_ready_dispatches_nonrequester"
       /\ c = "body_ready_two"
       /\ p = "c" -> TRUE
    [] OTHER -> ActualBodyResponseConstructed(c) /\ BodyRequester(c, p)

ActualBodyAllDispatchesUseFallback(c) ==
  CASE Bug = "body_dispatch_without_fallback"
       /\ c = "body_ready_two" -> FALSE
    [] OTHER -> TRUE

Matches(c) ==
  /\ ActualReturn(c) = SpecReturn(c)
  /\ ActualBuildPayload(c) = SpecBuildPayload(c)
  /\ ActualFetchRemoved(c) = SpecFetchRemoved(c)
  /\ ActualBodyRemoved(c) = SpecBodyRemoved(c)
  /\ ActualFetchBatchCalled(c) = SpecFetchBatchCalled(c)
  /\ ActualFetchBatchForceArg(c) = SpecFetchBatchForceArg(c)
  /\ ActualFetchBatchAllowHighestArg(c) = SpecFetchBatchAllowHighestArg(c)
  /\ ActualFetchBatchAllowHintlessArg(c) = SpecFetchBatchAllowHintlessArg(c)
  /\ ActualBodyResponseConstructed(c) = SpecBodyResponseConstructed(c)
  /\ ActualBodyResponseHashOk(c) = SpecBodyResponseHashOk(c)
  /\ ActualBodyResponseHeightOk(c) = SpecBodyResponseHeightOk(c)
  /\ ActualBodyResponseViewOk(c) = SpecBodyResponseViewOk(c)
  /\ ActualBodyResponsePayloadOk(c) = SpecBodyResponsePayloadOk(c)
  /\ ActualBodyAllDispatchesUseFallback(c) = SpecBodyAllDispatchesUseFallback(c)
  /\ \A p \in PeerIds:
       /\ ActualFetchBatchPeer(c, p) = SpecFetchBatchPeer(c, p)
       /\ ActualBodyDispatch(c, p) = SpecBodyDispatch(c, p)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "fetch_absent_returns_true",
       "fetch_absent_builds_payload",
       "fetch_deferred_returns_true",
       "fetch_deferred_removes_pending",
       "fetch_ready_returns_false",
       "fetch_ready_skips_build",
       "fetch_ready_keeps_pending",
       "fetch_ready_skips_batch",
       "fetch_batch_drops_requester",
       "fetch_batch_adds_nonrequester",
       "fetch_ready_force_bypass_true",
       "fetch_ready_allow_highest_true",
       "fetch_ready_allow_hintless_true",
       "fetch_empty_entry_treated_absent",
       "body_absent_returns_true",
       "body_absent_builds_payload",
       "body_deferred_returns_true",
       "body_deferred_removes_pending",
       "body_ready_returns_false",
       "body_ready_skips_build",
       "body_ready_skips_response",
       "body_ready_keeps_pending",
       "body_ready_skips_dispatch_peer",
       "body_ready_dispatches_nonrequester",
       "body_response_wrong_hash",
       "body_response_wrong_height",
       "body_response_wrong_view",
       "body_response_wrong_payload",
       "body_dispatch_without_fallback",
       "body_empty_entry_treated_absent"
     }
  /\ checked = 0

PendingResponseFlushMatchesSpec ==
  \A c \in Cases: Matches(c)

PendingResponseFlushExactness ==
  /\ PendingResponseFlushMatchesSpec

PendingResponseFlushCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ PendingResponseFlushExactness

SafetyFast ==
  PendingResponseFlushExactness

FetchAbsentReturnsFalse ==
  Matches("fetch_absent")

FetchAbsentDoesNotBuild ==
  Matches("fetch_absent")

FetchDeferredReturnsFalse ==
  Matches("fetch_deferred")

FetchDeferredKeepsPending ==
  Matches("fetch_deferred")

FetchReadyReturnsTrue ==
  Matches("fetch_ready_one")

FetchReadyBuildsPayload ==
  Matches("fetch_ready_one")

FetchReadyRemovesPending ==
  Matches("fetch_ready_one")

FetchReadyCallsBatch ==
  Matches("fetch_ready_one")

FetchBatchKeepsRequesters ==
  Matches("fetch_ready_one")

FetchBatchRejectsNonrequesters ==
  Matches("fetch_ready_one")

FetchReadyForceBypassFalse ==
  Matches("fetch_ready_one")

FetchReadyAllowHighestFalse ==
  Matches("fetch_ready_one")

FetchReadyAllowHintlessFalse ==
  Matches("fetch_ready_one")

FetchEmptyEntryStillFlushes ==
  Matches("fetch_ready_empty_entry")

BodyAbsentReturnsFalse ==
  Matches("body_absent")

BodyAbsentDoesNotBuild ==
  Matches("body_absent")

BodyDeferredReturnsFalse ==
  Matches("body_deferred")

BodyDeferredKeepsPending ==
  Matches("body_deferred")

BodyReadyReturnsTrue ==
  Matches("body_ready_two")

BodyReadyBuildsPayload ==
  Matches("body_ready_two")

BodyReadyConstructsResponse ==
  Matches("body_ready_two")

BodyReadyRemovesPending ==
  Matches("body_ready_two")

BodyDispatchesRequesters ==
  Matches("body_ready_two")

BodyRejectsNonrequesters ==
  Matches("body_ready_two")

BodyResponseBindsHash ==
  Matches("body_ready_two")

BodyResponseBindsHeight ==
  Matches("body_ready_two")

BodyResponseBindsView ==
  Matches("body_ready_two")

BodyResponseBindsPayload ==
  Matches("body_ready_two")

BodyDispatchUsesFallbackHelper ==
  Matches("body_ready_two")

BodyEmptyEntryStillFlushes ==
  Matches("body_ready_empty_entry")

====
