---- MODULE SumeragiLocalPayloadAvailabilityGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for broad actor-local payload availability.

This slice captures `block_payload_available_locally(...)`. The helper is a
coarse local-material predicate: any pending block entry, commit-inflight block,
hash-only `pending_processing` marker, deferred block-sync payload, or Kura
payload counts as locally available. It intentionally does not filter invalid
or aborted local owners; narrower predicates decide whether the material is fit
for progress, fetch suppression, or lock extension.
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
PendingInvalid == "pending_invalid"
PendingAborted == "pending_aborted"
InflightValid == "inflight_valid"
InflightInvalid == "inflight_invalid"
InflightAborted == "inflight_aborted"
HashOnlyProcessing == "hash_only_processing"
DeferredPayload == "deferred_payload"
KuraPayload == "kura_payload"
AbsentPayload == "absent_payload"

Cases == {
  PendingValid,
  PendingInvalid,
  PendingAborted,
  InflightValid,
  InflightInvalid,
  InflightAborted,
  HashOnlyProcessing,
  DeferredPayload,
  KuraPayload,
  AbsentPayload
}

ReturnAvailable == 1
ReturnMissing == 2
CheckPending == 3
CheckInflight == 4
CheckProcessing == 5
CheckDeferred == 6
CheckKura == 7
PendingValidAccepted == 8
PendingInvalidAccepted == 9
PendingAbortedAccepted == 10
InflightValidAccepted == 11
InflightInvalidAccepted == 12
InflightAbortedAccepted == 13
HashOnlyProcessingAccepted == 14
DeferredPayloadAccepted == 15
KuraPayloadAccepted == 16

ActionUniverse == 1..16

SpecActions(c) ==
  CASE c = PendingValid ->
      {ReturnAvailable, CheckPending, PendingValidAccepted}
    [] c = PendingInvalid ->
      {ReturnAvailable, CheckPending, PendingInvalidAccepted}
    [] c = PendingAborted ->
      {ReturnAvailable, CheckPending, PendingAbortedAccepted}
    [] c = InflightValid ->
      {ReturnAvailable, CheckPending, CheckInflight, InflightValidAccepted}
    [] c = InflightInvalid ->
      {ReturnAvailable, CheckPending, CheckInflight,
       InflightInvalidAccepted}
    [] c = InflightAborted ->
      {ReturnAvailable, CheckPending, CheckInflight,
       InflightAbortedAccepted}
    [] c = HashOnlyProcessing ->
      {ReturnAvailable, CheckPending, CheckInflight, CheckProcessing,
       HashOnlyProcessingAccepted}
    [] c = DeferredPayload ->
      {ReturnAvailable, CheckPending, CheckInflight, CheckProcessing,
       CheckDeferred, DeferredPayloadAccepted}
    [] c = KuraPayload ->
      {ReturnAvailable, CheckPending, CheckInflight, CheckProcessing,
       CheckDeferred, CheckKura, KuraPayloadAccepted}
    [] c = AbsentPayload ->
      {ReturnMissing, CheckPending, CheckInflight, CheckProcessing,
       CheckDeferred, CheckKura}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "skip_pending_lookup"
       /\ c \in {PendingValid, PendingInvalid, PendingAborted} ->
      (spec \ {CheckPending, ReturnAvailable, PendingValidAccepted,
               PendingInvalidAccepted, PendingAbortedAccepted}) \cup
        {ReturnMissing}
    [] Bug = "reject_valid_pending"
       /\ c = PendingValid ->
      (spec \ {ReturnAvailable, PendingValidAccepted}) \cup {ReturnMissing}
    [] Bug = "reject_invalid_pending"
       /\ c = PendingInvalid ->
      (spec \ {ReturnAvailable, PendingInvalidAccepted}) \cup {ReturnMissing}
    [] Bug = "reject_aborted_pending"
       /\ c = PendingAborted ->
      (spec \ {ReturnAvailable, PendingAbortedAccepted}) \cup {ReturnMissing}
    [] Bug = "skip_inflight_lookup"
       /\ c \in {InflightValid, InflightInvalid, InflightAborted} ->
      (spec \ {CheckInflight, ReturnAvailable, InflightValidAccepted,
               InflightInvalidAccepted, InflightAbortedAccepted}) \cup
        {ReturnMissing}
    [] Bug = "reject_valid_inflight"
       /\ c = InflightValid ->
      (spec \ {ReturnAvailable, InflightValidAccepted}) \cup {ReturnMissing}
    [] Bug = "reject_invalid_inflight"
       /\ c = InflightInvalid ->
      (spec \ {ReturnAvailable, InflightInvalidAccepted}) \cup
        {ReturnMissing}
    [] Bug = "reject_aborted_inflight"
       /\ c = InflightAborted ->
      (spec \ {ReturnAvailable, InflightAbortedAccepted}) \cup
        {ReturnMissing}
    [] Bug = "ignore_hash_only_processing"
       /\ c = HashOnlyProcessing ->
      (spec \ {ReturnAvailable, HashOnlyProcessingAccepted}) \cup
        {ReturnMissing}
    [] Bug = "ignore_deferred_payload"
       /\ c = DeferredPayload ->
      (spec \ {ReturnAvailable, DeferredPayloadAccepted}) \cup {ReturnMissing}
    [] Bug = "ignore_kura_payload"
       /\ c = KuraPayload ->
      (spec \ {ReturnAvailable, KuraPayloadAccepted}) \cup {ReturnMissing}
    [] Bug = "accept_absent_payload"
       /\ c = AbsentPayload ->
      (spec \ {ReturnMissing}) \cup {ReturnAvailable}
    [] OTHER -> spec

Bugs == {
  "none",
  "skip_pending_lookup",
  "reject_valid_pending",
  "reject_invalid_pending",
  "reject_aborted_pending",
  "skip_inflight_lookup",
  "reject_valid_inflight",
  "reject_invalid_inflight",
  "reject_aborted_inflight",
  "ignore_hash_only_processing",
  "ignore_deferred_payload",
  "ignore_kura_payload",
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

PendingEntriesAlwaysCountAsLocalPayload ==
  /\ ReturnAvailable \in ImplementationActions(PendingValid)
  /\ PendingValidAccepted \in ImplementationActions(PendingValid)
  /\ ReturnAvailable \in ImplementationActions(PendingInvalid)
  /\ PendingInvalidAccepted \in ImplementationActions(PendingInvalid)
  /\ ReturnAvailable \in ImplementationActions(PendingAborted)
  /\ PendingAbortedAccepted \in ImplementationActions(PendingAborted)

InflightEntriesAlwaysCountAsLocalPayload ==
  /\ ReturnAvailable \in ImplementationActions(InflightValid)
  /\ InflightValidAccepted \in ImplementationActions(InflightValid)
  /\ ReturnAvailable \in ImplementationActions(InflightInvalid)
  /\ InflightInvalidAccepted \in ImplementationActions(InflightInvalid)
  /\ ReturnAvailable \in ImplementationActions(InflightAborted)
  /\ InflightAbortedAccepted \in ImplementationActions(InflightAborted)

HashDeferredAndKuraMaterialCountsAsLocalPayload ==
  /\ ReturnAvailable \in ImplementationActions(HashOnlyProcessing)
  /\ HashOnlyProcessingAccepted \in ImplementationActions(HashOnlyProcessing)
  /\ ReturnAvailable \in ImplementationActions(DeferredPayload)
  /\ DeferredPayloadAccepted \in ImplementationActions(DeferredPayload)
  /\ ReturnAvailable \in ImplementationActions(KuraPayload)
  /\ KuraPayloadAccepted \in ImplementationActions(KuraPayload)

AbsentPayloadDoesNotCount ==
  /\ ReturnMissing \in ImplementationActions(AbsentPayload)
  /\ ~(ReturnAvailable \in ImplementationActions(AbsentPayload))

LookupShapeMatchesShortCircuit ==
  /\ \A c \in Cases:
       CheckPending \in ImplementationActions(c)
  /\ \A c \in Cases \ {PendingValid, PendingInvalid, PendingAborted}:
       CheckInflight \in ImplementationActions(c)
  /\ \A c \in {HashOnlyProcessing, DeferredPayload, KuraPayload,
               AbsentPayload}:
       CheckProcessing \in ImplementationActions(c)
  /\ \A c \in {DeferredPayload, KuraPayload, AbsentPayload}:
       CheckDeferred \in ImplementationActions(c)
  /\ \A c \in {KuraPayload, AbsentPayload}:
       CheckKura \in ImplementationActions(c)

LocalPayloadAvailabilityCoreSafety ==
  /\ ActionsMatchSpec
  /\ PendingEntriesAlwaysCountAsLocalPayload
  /\ InflightEntriesAlwaysCountAsLocalPayload
  /\ HashDeferredAndKuraMaterialCountsAsLocalPayload
  /\ AbsentPayloadDoesNotCount
  /\ LookupShapeMatchesShortCircuit

LocalPayloadAvailabilityExactness ==
  LocalPayloadAvailabilityCoreSafety

LocalPayloadAvailabilityCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ LocalPayloadAvailabilityExactness

NoBugInvariant == LocalPayloadAvailabilityExactness

SafetyFast == LocalPayloadAvailabilityExactness

====
