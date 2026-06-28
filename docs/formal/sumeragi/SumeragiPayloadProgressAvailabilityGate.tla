---- MODULE SumeragiPayloadProgressAvailabilityGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for actor-local payload availability for progress.

This slice captures `block_payload_available_for_progress(...)`. The helper is
weaker than authoritative proposal ownership but stronger than a hash-only
processing marker: valid or aborted local payload owners can unblock progress,
invalid local owners fail closed, deferred block-sync payloads and Kura payloads
count when no local owner rejects the hash, and a hash-only `pending_processing`
entry is not enough.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PendingValid == "pending_valid"
PendingAborted == "pending_aborted"
PendingInvalidWithKura == "pending_invalid_with_kura"
InflightValid == "inflight_valid"
InflightAborted == "inflight_aborted"
InflightInvalidWithKura == "inflight_invalid_with_kura"
DeferredPayload == "deferred_payload"
KuraPayload == "kura_payload"
HashOnlyProcessing == "hash_only_processing"
AbsentPayload == "absent_payload"

Cases == {
  PendingValid,
  PendingAborted,
  PendingInvalidWithKura,
  InflightValid,
  InflightAborted,
  InflightInvalidWithKura,
  DeferredPayload,
  KuraPayload,
  HashOnlyProcessing,
  AbsentPayload
}

ReturnAvailable == 1
ReturnMissing == 2
CheckPending == 3
CheckInflight == 4
CheckDeferred == 5
CheckKura == 6
PendingValidAccepted == 7
PendingAbortedAccepted == 8
PendingInvalidRejected == 9
InflightValidAccepted == 10
InflightAbortedAccepted == 11
InflightInvalidRejected == 12
DeferredPayloadAccepted == 13
KuraPayloadAccepted == 14
HashOnlyProcessingRejected == 15
NoFallbackAfterInvalidOwner == 16

ActionUniverse == 1..16

SpecActions(c) ==
  CASE c = PendingValid ->
      {ReturnAvailable, CheckPending, PendingValidAccepted}
    [] c = PendingAborted ->
      {ReturnAvailable, CheckPending, PendingAbortedAccepted}
    [] c = PendingInvalidWithKura ->
      {ReturnMissing, CheckPending, PendingInvalidRejected,
       NoFallbackAfterInvalidOwner}
    [] c = InflightValid ->
      {ReturnAvailable, CheckPending, CheckInflight, InflightValidAccepted}
    [] c = InflightAborted ->
      {ReturnAvailable, CheckPending, CheckInflight, InflightAbortedAccepted}
    [] c = InflightInvalidWithKura ->
      {ReturnMissing, CheckPending, CheckInflight, InflightInvalidRejected,
       NoFallbackAfterInvalidOwner}
    [] c = DeferredPayload ->
      {ReturnAvailable, CheckPending, CheckInflight, CheckDeferred,
       DeferredPayloadAccepted}
    [] c = KuraPayload ->
      {ReturnAvailable, CheckPending, CheckInflight, CheckDeferred, CheckKura,
       KuraPayloadAccepted}
    [] c = HashOnlyProcessing ->
      {ReturnMissing, CheckPending, CheckInflight, CheckDeferred, CheckKura,
       HashOnlyProcessingRejected}
    [] c = AbsentPayload ->
      {ReturnMissing, CheckPending, CheckInflight, CheckDeferred, CheckKura}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "skip_pending_lookup"
       /\ c \in {PendingValid, PendingAborted, PendingInvalidWithKura} ->
      (spec \ {CheckPending, ReturnAvailable, PendingValidAccepted,
               PendingAbortedAccepted, PendingInvalidRejected,
               NoFallbackAfterInvalidOwner}) \cup {ReturnMissing}
    [] Bug = "reject_aborted_pending"
       /\ c = PendingAborted ->
      (spec \ {ReturnAvailable, PendingAbortedAccepted}) \cup {ReturnMissing}
    [] Bug = "accept_invalid_pending"
       /\ c = PendingInvalidWithKura ->
      (spec \ {ReturnMissing, PendingInvalidRejected,
               NoFallbackAfterInvalidOwner}) \cup {ReturnAvailable}
    [] Bug = "fallback_after_invalid_pending"
       /\ c = PendingInvalidWithKura ->
      (spec \ {ReturnMissing, NoFallbackAfterInvalidOwner}) \cup
        {ReturnAvailable, CheckKura, KuraPayloadAccepted}
    [] Bug = "skip_inflight_lookup"
       /\ c \in {InflightValid, InflightAborted, InflightInvalidWithKura} ->
      (spec \ {CheckInflight, ReturnAvailable, InflightValidAccepted,
               InflightAbortedAccepted, InflightInvalidRejected,
               NoFallbackAfterInvalidOwner}) \cup {ReturnMissing}
    [] Bug = "reject_aborted_inflight"
       /\ c = InflightAborted ->
      (spec \ {ReturnAvailable, InflightAbortedAccepted}) \cup {ReturnMissing}
    [] Bug = "accept_invalid_inflight"
       /\ c = InflightInvalidWithKura ->
      (spec \ {ReturnMissing, InflightInvalidRejected,
               NoFallbackAfterInvalidOwner}) \cup {ReturnAvailable}
    [] Bug = "fallback_after_invalid_inflight"
       /\ c = InflightInvalidWithKura ->
      (spec \ {ReturnMissing, NoFallbackAfterInvalidOwner}) \cup
        {ReturnAvailable, CheckKura, KuraPayloadAccepted}
    [] Bug = "ignore_deferred_payload"
       /\ c = DeferredPayload ->
      (spec \ {ReturnAvailable, DeferredPayloadAccepted}) \cup {ReturnMissing}
    [] Bug = "ignore_kura_payload"
       /\ c = KuraPayload ->
      (spec \ {ReturnAvailable, KuraPayloadAccepted}) \cup {ReturnMissing}
    [] Bug = "accept_hash_only_processing"
       /\ c = HashOnlyProcessing ->
      (spec \ {ReturnMissing, HashOnlyProcessingRejected}) \cup
        {ReturnAvailable}
    [] Bug = "accept_absent_payload"
       /\ c = AbsentPayload ->
      (spec \ {ReturnMissing}) \cup {ReturnAvailable}
    [] OTHER -> spec

Bugs == {
  "none",
  "skip_pending_lookup",
  "reject_aborted_pending",
  "accept_invalid_pending",
  "fallback_after_invalid_pending",
  "skip_inflight_lookup",
  "reject_aborted_inflight",
  "accept_invalid_inflight",
  "fallback_after_invalid_inflight",
  "ignore_deferred_payload",
  "ignore_kura_payload",
  "accept_hash_only_processing",
  "accept_absent_payload"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

PendingPayloadOwnersGateProgress ==
  /\ ReturnAvailable \in ImplementationActions(PendingValid)
  /\ ReturnAvailable \in ImplementationActions(PendingAborted)
  /\ PendingValidAccepted \in ImplementationActions(PendingValid)
  /\ PendingAbortedAccepted \in ImplementationActions(PendingAborted)
  /\ ReturnMissing \in ImplementationActions(PendingInvalidWithKura)
  /\ PendingInvalidRejected \in ImplementationActions(PendingInvalidWithKura)
  /\ NoFallbackAfterInvalidOwner \in ImplementationActions(PendingInvalidWithKura)

InflightPayloadOwnersGateProgress ==
  /\ ReturnAvailable \in ImplementationActions(InflightValid)
  /\ ReturnAvailable \in ImplementationActions(InflightAborted)
  /\ InflightValidAccepted \in ImplementationActions(InflightValid)
  /\ InflightAbortedAccepted \in ImplementationActions(InflightAborted)
  /\ ReturnMissing \in ImplementationActions(InflightInvalidWithKura)
  /\ InflightInvalidRejected \in ImplementationActions(InflightInvalidWithKura)
  /\ NoFallbackAfterInvalidOwner \in ImplementationActions(InflightInvalidWithKura)

FallbackPayloadStoresGateProgress ==
  /\ ReturnAvailable \in ImplementationActions(DeferredPayload)
  /\ DeferredPayloadAccepted \in ImplementationActions(DeferredPayload)
  /\ ReturnAvailable \in ImplementationActions(KuraPayload)
  /\ KuraPayloadAccepted \in ImplementationActions(KuraPayload)

HashOnlyAndAbsentPayloadsDoNotGateProgress ==
  /\ ReturnMissing \in ImplementationActions(HashOnlyProcessing)
  /\ HashOnlyProcessingRejected \in ImplementationActions(HashOnlyProcessing)
  /\ ~(ReturnAvailable \in ImplementationActions(HashOnlyProcessing))
  /\ ReturnMissing \in ImplementationActions(AbsentPayload)
  /\ ~(ReturnAvailable \in ImplementationActions(AbsentPayload))

InvalidOwnersDoNotFallThroughToFallbackStores ==
  \A c \in {PendingInvalidWithKura, InflightInvalidWithKura}:
    /\ ReturnMissing \in ImplementationActions(c)
    /\ NoFallbackAfterInvalidOwner \in ImplementationActions(c)
    /\ ~(ReturnAvailable \in ImplementationActions(c))
    /\ ~(KuraPayloadAccepted \in ImplementationActions(c))

PayloadProgressAvailabilityCoreSafety ==
  /\ ActionsMatchSpec
  /\ PendingPayloadOwnersGateProgress
  /\ InflightPayloadOwnersGateProgress
  /\ FallbackPayloadStoresGateProgress
  /\ HashOnlyAndAbsentPayloadsDoNotGateProgress
  /\ InvalidOwnersDoNotFallThroughToFallbackStores

PayloadProgressAvailabilityExactness ==
  /\ ActionsMatchSpec
  /\ PendingPayloadOwnersGateProgress
  /\ InflightPayloadOwnersGateProgress
  /\ FallbackPayloadStoresGateProgress
  /\ HashOnlyAndAbsentPayloadsDoNotGateProgress
  /\ InvalidOwnersDoNotFallThroughToFallbackStores
PayloadProgressAvailabilityCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ PayloadProgressAvailabilityExactness

NoBugInvariant == PayloadProgressAvailabilityExactness

SafetyFast == PayloadProgressAvailabilityExactness

====
