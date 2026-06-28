---- MODULE SumeragiBlockKnownLocallyGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for actor-local block knowledge.

This slice captures `block_known_locally(...)`. The helper is stricter than
`block_payload_available_locally(...)`: aborted pending/inflight owners and
deferred-only block-sync payloads do not count. It is also looser than
`block_known_for_lock(...)`: invalid-but-present pending/inflight entries and a
hash-only `pending_processing` marker are still considered locally known for
consensus routing and recovery decisions. Kura height knowledge also counts.
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
KuraBlock == "kura_block"
AbsentBlock == "absent_block"

Cases == {
  PendingValid,
  PendingInvalid,
  PendingAborted,
  InflightValid,
  InflightInvalid,
  InflightAborted,
  HashOnlyProcessing,
  DeferredPayload,
  KuraBlock,
  AbsentBlock
}

ReturnKnown == 1
ReturnUnknown == 2
CheckPending == 3
CheckInflight == 4
CheckProcessing == 5
CheckKura == 6
PendingValidAccepted == 7
PendingInvalidAccepted == 8
PendingAbortedRejected == 9
InflightValidAccepted == 10
InflightInvalidAccepted == 11
InflightAbortedRejected == 12
HashOnlyProcessingAccepted == 13
DeferredPayloadIgnored == 14
KuraBlockAccepted == 15

ActionUniverse == 1..15

SpecActions(c) ==
  CASE c = PendingValid ->
      {ReturnKnown, CheckPending, PendingValidAccepted}
    [] c = PendingInvalid ->
      {ReturnKnown, CheckPending, PendingInvalidAccepted}
    [] c = PendingAborted ->
      {ReturnUnknown, CheckPending, CheckInflight, CheckProcessing,
       CheckKura, PendingAbortedRejected}
    [] c = InflightValid ->
      {ReturnKnown, CheckPending, CheckInflight, InflightValidAccepted}
    [] c = InflightInvalid ->
      {ReturnKnown, CheckPending, CheckInflight, InflightInvalidAccepted}
    [] c = InflightAborted ->
      {ReturnUnknown, CheckPending, CheckInflight, CheckProcessing,
       CheckKura, InflightAbortedRejected}
    [] c = HashOnlyProcessing ->
      {ReturnKnown, CheckPending, CheckInflight, CheckProcessing,
       HashOnlyProcessingAccepted}
    [] c = DeferredPayload ->
      {ReturnUnknown, CheckPending, CheckInflight, CheckProcessing,
       CheckKura, DeferredPayloadIgnored}
    [] c = KuraBlock ->
      {ReturnKnown, CheckPending, CheckInflight, CheckProcessing,
       CheckKura, KuraBlockAccepted}
    [] c = AbsentBlock ->
      {ReturnUnknown, CheckPending, CheckInflight, CheckProcessing, CheckKura}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "skip_pending_lookup"
       /\ c \in {PendingValid, PendingInvalid, PendingAborted} ->
      (spec \ {CheckPending, ReturnKnown, PendingValidAccepted,
               PendingInvalidAccepted, PendingAbortedRejected}) \cup
        {ReturnUnknown}
    [] Bug = "reject_valid_pending"
       /\ c = PendingValid ->
      (spec \ {ReturnKnown, PendingValidAccepted}) \cup {ReturnUnknown}
    [] Bug = "reject_invalid_pending"
       /\ c = PendingInvalid ->
      (spec \ {ReturnKnown, PendingInvalidAccepted}) \cup {ReturnUnknown}
    [] Bug = "accept_aborted_pending"
       /\ c = PendingAborted ->
      (spec \ {ReturnUnknown, PendingAbortedRejected}) \cup {ReturnKnown}
    [] Bug = "skip_inflight_lookup"
       /\ c \in {InflightValid, InflightInvalid, InflightAborted} ->
      (spec \ {CheckInflight, ReturnKnown, InflightValidAccepted,
               InflightInvalidAccepted, InflightAbortedRejected}) \cup
        {ReturnUnknown}
    [] Bug = "reject_valid_inflight"
       /\ c = InflightValid ->
      (spec \ {ReturnKnown, InflightValidAccepted}) \cup {ReturnUnknown}
    [] Bug = "reject_invalid_inflight"
       /\ c = InflightInvalid ->
      (spec \ {ReturnKnown, InflightInvalidAccepted}) \cup {ReturnUnknown}
    [] Bug = "accept_aborted_inflight"
       /\ c = InflightAborted ->
      (spec \ {ReturnUnknown, InflightAbortedRejected}) \cup {ReturnKnown}
    [] Bug = "ignore_hash_only_processing"
       /\ c = HashOnlyProcessing ->
      (spec \ {ReturnKnown, HashOnlyProcessingAccepted}) \cup {ReturnUnknown}
    [] Bug = "accept_deferred_payload"
       /\ c = DeferredPayload ->
      (spec \ {ReturnUnknown, DeferredPayloadIgnored}) \cup {ReturnKnown}
    [] Bug = "ignore_kura_block"
       /\ c = KuraBlock ->
      (spec \ {ReturnKnown, KuraBlockAccepted}) \cup {ReturnUnknown}
    [] Bug = "accept_absent_block"
       /\ c = AbsentBlock ->
      (spec \ {ReturnUnknown}) \cup {ReturnKnown}
    [] OTHER -> spec

Bugs == {
  "none",
  "skip_pending_lookup",
  "reject_valid_pending",
  "reject_invalid_pending",
  "accept_aborted_pending",
  "skip_inflight_lookup",
  "reject_valid_inflight",
  "reject_invalid_inflight",
  "accept_aborted_inflight",
  "ignore_hash_only_processing",
  "accept_deferred_payload",
  "ignore_kura_block",
  "accept_absent_block"
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

PendingEntriesAreKnownUnlessAborted ==
  /\ ReturnKnown \in ImplementationActions(PendingValid)
  /\ PendingValidAccepted \in ImplementationActions(PendingValid)
  /\ ReturnKnown \in ImplementationActions(PendingInvalid)
  /\ PendingInvalidAccepted \in ImplementationActions(PendingInvalid)
  /\ ReturnUnknown \in ImplementationActions(PendingAborted)
  /\ PendingAbortedRejected \in ImplementationActions(PendingAborted)
  /\ ~(ReturnKnown \in ImplementationActions(PendingAborted))

InflightEntriesAreKnownUnlessAborted ==
  /\ ReturnKnown \in ImplementationActions(InflightValid)
  /\ InflightValidAccepted \in ImplementationActions(InflightValid)
  /\ ReturnKnown \in ImplementationActions(InflightInvalid)
  /\ InflightInvalidAccepted \in ImplementationActions(InflightInvalid)
  /\ ReturnUnknown \in ImplementationActions(InflightAborted)
  /\ InflightAbortedRejected \in ImplementationActions(InflightAborted)
  /\ ~(ReturnKnown \in ImplementationActions(InflightAborted))

HashOnlyAndKuraCountAsKnown ==
  /\ ReturnKnown \in ImplementationActions(HashOnlyProcessing)
  /\ HashOnlyProcessingAccepted \in ImplementationActions(HashOnlyProcessing)
  /\ ReturnKnown \in ImplementationActions(KuraBlock)
  /\ KuraBlockAccepted \in ImplementationActions(KuraBlock)

DeferredOnlyAndAbsentDoNotCountAsKnown ==
  /\ ReturnUnknown \in ImplementationActions(DeferredPayload)
  /\ DeferredPayloadIgnored \in ImplementationActions(DeferredPayload)
  /\ ~(ReturnKnown \in ImplementationActions(DeferredPayload))
  /\ ReturnUnknown \in ImplementationActions(AbsentBlock)
  /\ ~(ReturnKnown \in ImplementationActions(AbsentBlock))

LookupShapeMatchesShortCircuit ==
  /\ \A c \in Cases:
       CheckPending \in ImplementationActions(c)
  /\ \A c \in Cases \ {PendingValid, PendingInvalid}:
       CheckInflight \in ImplementationActions(c)
  /\ \A c \in Cases \ {PendingValid, PendingInvalid,
                       InflightValid, InflightInvalid}:
       CheckProcessing \in ImplementationActions(c)
  /\ \A c \in Cases \ {PendingValid, PendingInvalid,
                       InflightValid, InflightInvalid,
                       HashOnlyProcessing}:
       CheckKura \in ImplementationActions(c)

BlockKnownLocallyCoreSafety ==
  /\ ActionsMatchSpec
  /\ PendingEntriesAreKnownUnlessAborted
  /\ InflightEntriesAreKnownUnlessAborted
  /\ HashOnlyAndKuraCountAsKnown
  /\ DeferredOnlyAndAbsentDoNotCountAsKnown
  /\ LookupShapeMatchesShortCircuit

BlockKnownLocallyExactness ==
  /\ ActionsMatchSpec
  /\ PendingEntriesAreKnownUnlessAborted
  /\ InflightEntriesAreKnownUnlessAborted
  /\ HashOnlyAndKuraCountAsKnown
  /\ DeferredOnlyAndAbsentDoNotCountAsKnown
  /\ LookupShapeMatchesShortCircuit
BlockKnownLocallyCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockKnownLocallyExactness

NoBugInvariant == BlockKnownLocallyExactness

SafetyFast == BlockKnownLocallyExactness

====
