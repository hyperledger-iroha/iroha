---- MODULE SumeragiHighestQcFetchBodyKnownGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for highest-QC body fetch suppression.

This slice captures `block_body_known_for_highest_qc_fetch(...)`. The helper
answers whether a highest-QC block body fetch is still necessary. Kura bodies
and non-aborted pending or commit-inflight bodies suppress the fetch even when
their validation status is invalid, because this gate is about local body
presence rather than payload validity for progress. Aborted pending/inflight
bodies, deferred-only block-sync payloads, hash-only `pending_processing`
markers, and absence keep the body missing for this fetch path.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

KuraBody == "kura_body"
PendingValid == "pending_valid"
PendingInvalid == "pending_invalid"
PendingAborted == "pending_aborted"
InflightValid == "inflight_valid"
InflightInvalid == "inflight_invalid"
InflightAborted == "inflight_aborted"
DeferredPayload == "deferred_payload"
HashOnlyProcessing == "hash_only_processing"
AbsentBody == "absent_body"

Cases == {
  KuraBody,
  PendingValid,
  PendingInvalid,
  PendingAborted,
  InflightValid,
  InflightInvalid,
  InflightAborted,
  DeferredPayload,
  HashOnlyProcessing,
  AbsentBody
}

ReturnKnown == 1
ReturnMissing == 2
CheckKura == 3
CheckPending == 4
CheckInflight == 5
KuraAccepted == 6
PendingValidAccepted == 7
PendingInvalidAccepted == 8
PendingAbortedRejected == 9
InflightValidAccepted == 10
InflightInvalidAccepted == 11
InflightAbortedRejected == 12
DeferredPayloadIgnored == 13
HashOnlyProcessingIgnored == 14

ActionUniverse == 1..14

SpecActions(c) ==
  CASE c = KuraBody ->
      {ReturnKnown, CheckKura, KuraAccepted}
    [] c = PendingValid ->
      {ReturnKnown, CheckKura, CheckPending, PendingValidAccepted}
    [] c = PendingInvalid ->
      {ReturnKnown, CheckKura, CheckPending, PendingInvalidAccepted}
    [] c = PendingAborted ->
      {ReturnMissing, CheckKura, CheckPending, CheckInflight,
       PendingAbortedRejected}
    [] c = InflightValid ->
      {ReturnKnown, CheckKura, CheckPending, CheckInflight,
       InflightValidAccepted}
    [] c = InflightInvalid ->
      {ReturnKnown, CheckKura, CheckPending, CheckInflight,
       InflightInvalidAccepted}
    [] c = InflightAborted ->
      {ReturnMissing, CheckKura, CheckPending, CheckInflight,
       InflightAbortedRejected}
    [] c = DeferredPayload ->
      {ReturnMissing, CheckKura, CheckPending, CheckInflight,
       DeferredPayloadIgnored}
    [] c = HashOnlyProcessing ->
      {ReturnMissing, CheckKura, CheckPending, CheckInflight,
       HashOnlyProcessingIgnored}
    [] c = AbsentBody ->
      {ReturnMissing, CheckKura, CheckPending, CheckInflight}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "ignore_kura_body"
       /\ c = KuraBody ->
      (spec \ {ReturnKnown, KuraAccepted}) \cup {ReturnMissing}
    [] Bug = "skip_pending_lookup"
       /\ c \in {PendingValid, PendingInvalid, PendingAborted} ->
      (spec \ {CheckPending, ReturnKnown, PendingValidAccepted,
               PendingInvalidAccepted, PendingAbortedRejected}) \cup
        {ReturnMissing}
    [] Bug = "reject_valid_pending"
       /\ c = PendingValid ->
      (spec \ {ReturnKnown, PendingValidAccepted}) \cup {ReturnMissing}
    [] Bug = "reject_invalid_pending"
       /\ c = PendingInvalid ->
      (spec \ {ReturnKnown, PendingInvalidAccepted}) \cup {ReturnMissing}
    [] Bug = "accept_aborted_pending"
       /\ c = PendingAborted ->
      (spec \ {ReturnMissing, PendingAbortedRejected}) \cup {ReturnKnown}
    [] Bug = "skip_inflight_lookup"
       /\ c \in {InflightValid, InflightInvalid, InflightAborted} ->
      (spec \ {CheckInflight, ReturnKnown, InflightValidAccepted,
               InflightInvalidAccepted, InflightAbortedRejected}) \cup
        {ReturnMissing}
    [] Bug = "reject_valid_inflight"
       /\ c = InflightValid ->
      (spec \ {ReturnKnown, InflightValidAccepted}) \cup {ReturnMissing}
    [] Bug = "reject_invalid_inflight"
       /\ c = InflightInvalid ->
      (spec \ {ReturnKnown, InflightInvalidAccepted}) \cup {ReturnMissing}
    [] Bug = "accept_aborted_inflight"
       /\ c = InflightAborted ->
      (spec \ {ReturnMissing, InflightAbortedRejected}) \cup {ReturnKnown}
    [] Bug = "accept_deferred_payload"
       /\ c = DeferredPayload ->
      (spec \ {ReturnMissing, DeferredPayloadIgnored}) \cup {ReturnKnown}
    [] Bug = "accept_hash_only_processing"
       /\ c = HashOnlyProcessing ->
      (spec \ {ReturnMissing, HashOnlyProcessingIgnored}) \cup {ReturnKnown}
    [] Bug = "accept_absent_body"
       /\ c = AbsentBody ->
      (spec \ {ReturnMissing}) \cup {ReturnKnown}
    [] OTHER -> spec

Bugs == {
  "none",
  "ignore_kura_body",
  "skip_pending_lookup",
  "reject_valid_pending",
  "reject_invalid_pending",
  "accept_aborted_pending",
  "skip_inflight_lookup",
  "reject_valid_inflight",
  "reject_invalid_inflight",
  "accept_aborted_inflight",
  "accept_deferred_payload",
  "accept_hash_only_processing",
  "accept_absent_body"
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

KuraBodiesSuppressFetch ==
  /\ ReturnKnown \in ImplementationActions(KuraBody)
  /\ KuraAccepted \in ImplementationActions(KuraBody)

PendingBodiesSuppressFetchUnlessAborted ==
  /\ ReturnKnown \in ImplementationActions(PendingValid)
  /\ PendingValidAccepted \in ImplementationActions(PendingValid)
  /\ ReturnKnown \in ImplementationActions(PendingInvalid)
  /\ PendingInvalidAccepted \in ImplementationActions(PendingInvalid)
  /\ ReturnMissing \in ImplementationActions(PendingAborted)
  /\ PendingAbortedRejected \in ImplementationActions(PendingAborted)
  /\ ~(ReturnKnown \in ImplementationActions(PendingAborted))

InflightBodiesSuppressFetchUnlessAborted ==
  /\ ReturnKnown \in ImplementationActions(InflightValid)
  /\ InflightValidAccepted \in ImplementationActions(InflightValid)
  /\ ReturnKnown \in ImplementationActions(InflightInvalid)
  /\ InflightInvalidAccepted \in ImplementationActions(InflightInvalid)
  /\ ReturnMissing \in ImplementationActions(InflightAborted)
  /\ InflightAbortedRejected \in ImplementationActions(InflightAborted)
  /\ ~(ReturnKnown \in ImplementationActions(InflightAborted))

DeferredHashOnlyAndAbsentBodiesStillFetch ==
  /\ ReturnMissing \in ImplementationActions(DeferredPayload)
  /\ DeferredPayloadIgnored \in ImplementationActions(DeferredPayload)
  /\ ~(ReturnKnown \in ImplementationActions(DeferredPayload))
  /\ ReturnMissing \in ImplementationActions(HashOnlyProcessing)
  /\ HashOnlyProcessingIgnored \in ImplementationActions(HashOnlyProcessing)
  /\ ~(ReturnKnown \in ImplementationActions(HashOnlyProcessing))
  /\ ReturnMissing \in ImplementationActions(AbsentBody)
  /\ ~(ReturnKnown \in ImplementationActions(AbsentBody))

LookupShapeMatchesShortCircuit ==
  /\ \A c \in Cases:
       CheckKura \in ImplementationActions(c)
  /\ \A c \in Cases \ {KuraBody}:
       CheckPending \in ImplementationActions(c)
  /\ \A c \in Cases \ {KuraBody, PendingValid, PendingInvalid}:
       CheckInflight \in ImplementationActions(c)

HighestQcFetchBodyKnownCoreSafety ==
  /\ ActionsMatchSpec
  /\ KuraBodiesSuppressFetch
  /\ PendingBodiesSuppressFetchUnlessAborted
  /\ InflightBodiesSuppressFetchUnlessAborted
  /\ DeferredHashOnlyAndAbsentBodiesStillFetch
  /\ LookupShapeMatchesShortCircuit

HighestQcFetchBodyKnownExactness ==
  HighestQcFetchBodyKnownCoreSafety

HighestQcFetchBodyKnownCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ HighestQcFetchBodyKnownExactness

NoBugInvariant == HighestQcFetchBodyKnownExactness

SafetyFast == HighestQcFetchBodyKnownExactness

====
