---- MODULE SumeragiLocalSignedBlockLookupGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for local signed-block materialization.

This slice captures `local_signed_block_for_hash_with_repair_options(...)` and
its two public wrappers. Normal lookup excludes aborted pending/inflight
owners; body repair includes them. Both modes reject invalid pending/inflight
owners, and rejected owners do not stop the lookup: later local inflight,
deferred block-sync, or Kura material can still provide the block. Deferred
payloads are lower priority than accepted pending/inflight owners and higher
priority than Kura.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NormalLookup == "normal_lookup"
BodyRepairLookup == "body_repair_lookup"
Modes == {NormalLookup, BodyRepairLookup}

PendingValid == "pending_valid"
PendingInvalid == "pending_invalid"
PendingAborted == "pending_aborted"
PendingInvalidWithInflight == "pending_invalid_with_inflight"
PendingInvalidWithDeferred == "pending_invalid_with_deferred"
PendingInvalidWithKura == "pending_invalid_with_kura"
PendingAbortedWithInflight == "pending_aborted_with_inflight"
PendingAbortedWithDeferred == "pending_aborted_with_deferred"
PendingAbortedWithKura == "pending_aborted_with_kura"
InflightValid == "inflight_valid"
InflightInvalid == "inflight_invalid"
InflightAborted == "inflight_aborted"
InflightInvalidWithDeferred == "inflight_invalid_with_deferred"
InflightInvalidWithKura == "inflight_invalid_with_kura"
InflightAbortedWithDeferred == "inflight_aborted_with_deferred"
InflightAbortedWithKura == "inflight_aborted_with_kura"
DeferredPayload == "deferred_payload"
KuraBlock == "kura_block"
AbsentBlock == "absent_block"

Cases == {
  PendingValid,
  PendingInvalid,
  PendingAborted,
  PendingInvalidWithInflight,
  PendingInvalidWithDeferred,
  PendingInvalidWithKura,
  PendingAbortedWithInflight,
  PendingAbortedWithDeferred,
  PendingAbortedWithKura,
  InflightValid,
  InflightInvalid,
  InflightAborted,
  InflightInvalidWithDeferred,
  InflightInvalidWithKura,
  InflightAbortedWithDeferred,
  InflightAbortedWithKura,
  DeferredPayload,
  KuraBlock,
  AbsentBlock
}

PendingInvalidCases == {
  PendingInvalid,
  PendingInvalidWithInflight,
  PendingInvalidWithDeferred,
  PendingInvalidWithKura
}

PendingAbortedCases == {
  PendingAborted,
  PendingAbortedWithInflight,
  PendingAbortedWithDeferred,
  PendingAbortedWithKura
}

PendingPresentCases ==
  {PendingValid} \cup PendingInvalidCases \cup PendingAbortedCases

InflightValidCases == {
  InflightValid,
  PendingInvalidWithInflight,
  PendingAbortedWithInflight
}

InflightInvalidCases == {
  InflightInvalid,
  InflightInvalidWithDeferred,
  InflightInvalidWithKura
}

InflightAbortedCases == {
  InflightAborted,
  InflightAbortedWithDeferred,
  InflightAbortedWithKura
}

InflightPresentCases ==
  InflightValidCases \cup InflightInvalidCases \cup InflightAbortedCases

DeferredCases == {
  DeferredPayload,
  PendingInvalidWithDeferred,
  PendingAbortedWithDeferred,
  InflightInvalidWithDeferred,
  InflightAbortedWithDeferred
}

KuraCases == {
  KuraBlock,
  PendingInvalidWithKura,
  PendingAbortedWithKura,
  InflightInvalidWithKura,
  InflightAbortedWithKura
}

PendingSource == "pending"
InflightSource == "inflight"
DeferredSource == "deferred"
KuraSource == "kura"
NoneSource == "none"
Sources == {PendingSource, InflightSource, DeferredSource, KuraSource, NoneSource}

PendingAllowed(c, m) ==
  c = PendingValid \/ (m = BodyRepairLookup /\ c \in PendingAbortedCases)

InflightAllowed(c, m) ==
  c \in InflightValidCases \/ (m = BodyRepairLookup /\ c \in InflightAbortedCases)

DeferredPresent(c) == c \in DeferredCases
KuraPresent(c) == c \in KuraCases

SourceWithoutInflight(c) ==
  IF DeferredPresent(c) THEN DeferredSource
  ELSE IF KuraPresent(c) THEN KuraSource
  ELSE NoneSource

SourceWithoutPending(c, m) ==
  IF InflightAllowed(c, m) THEN InflightSource
  ELSE SourceWithoutInflight(c)

SpecSource(c, m) ==
  IF PendingAllowed(c, m) THEN PendingSource
  ELSE SourceWithoutPending(c, m)

CheckPending == 1
CheckInflight == 2
CheckDeferred == 3
CheckKura == 4
ReturnSome == 5
ReturnNone == 6
PendingReturned == 7
PendingInvalidRejected == 8
PendingAbortedRejected == 9
InflightReturned == 10
InflightInvalidRejected == 11
InflightAbortedRejected == 12
DeferredReturned == 13
KuraReturned == 14

ActionUniverse == 1..14

SpecActions(c, m) ==
  {CheckPending}
    \cup (IF SpecSource(c, m) = NoneSource THEN {ReturnNone} ELSE {ReturnSome})
    \cup (IF PendingAllowed(c, m) THEN {PendingReturned} ELSE {})
    \cup (IF c \in PendingInvalidCases THEN {PendingInvalidRejected} ELSE {})
    \cup (IF c \in PendingAbortedCases /\ m = NormalLookup
          THEN {PendingAbortedRejected}
          ELSE {})
    \cup (IF ~PendingAllowed(c, m) THEN {CheckInflight} ELSE {})
    \cup (IF ~PendingAllowed(c, m) /\ InflightAllowed(c, m)
          THEN {InflightReturned}
          ELSE {})
    \cup (IF ~PendingAllowed(c, m) /\ c \in InflightInvalidCases
          THEN {InflightInvalidRejected}
          ELSE {})
    \cup (IF ~PendingAllowed(c, m)
              /\ c \in InflightAbortedCases
              /\ m = NormalLookup
          THEN {InflightAbortedRejected}
          ELSE {})
    \cup (IF ~PendingAllowed(c, m) /\ ~InflightAllowed(c, m)
          THEN {CheckDeferred}
          ELSE {})
    \cup (IF SpecSource(c, m) = DeferredSource THEN {DeferredReturned} ELSE {})
    \cup (IF ~PendingAllowed(c, m)
              /\ ~InflightAllowed(c, m)
              /\ ~DeferredPresent(c)
          THEN {CheckKura}
          ELSE {})
    \cup (IF SpecSource(c, m) = KuraSource THEN {KuraReturned} ELSE {})

WithReturn(actions, source) ==
  (actions \ {ReturnSome, ReturnNone, PendingReturned, InflightReturned,
              DeferredReturned, KuraReturned})
    \cup (IF source = NoneSource THEN {ReturnNone} ELSE {ReturnSome})
    \cup (IF source = PendingSource THEN {PendingReturned} ELSE {})
    \cup (IF source = InflightSource THEN {InflightReturned} ELSE {})
    \cup (IF source = DeferredSource THEN {DeferredReturned} ELSE {})
    \cup (IF source = KuraSource THEN {KuraReturned} ELSE {})

RejectedPendingHasFallback(c, m) ==
  c \in PendingPresentCases
  /\ ~PendingAllowed(c, m)
  /\ SourceWithoutPending(c, m) # NoneSource

RejectedInflightHasFallback(c, m) ==
  ~PendingAllowed(c, m)
  /\ c \in InflightPresentCases
  /\ ~InflightAllowed(c, m)
  /\ SourceWithoutInflight(c) # NoneSource

ImplementationSource(c, m) ==
  CASE Bug = "skip_pending_lookup"
       /\ c \in PendingPresentCases ->
      SourceWithoutPending(c, m)
    [] Bug = "reject_valid_pending"
       /\ c = PendingValid ->
      NoneSource
    [] Bug = "accept_invalid_pending"
       /\ c \in PendingInvalidCases ->
      PendingSource
    [] Bug = "accept_aborted_pending_normal"
       /\ m = NormalLookup
       /\ c \in PendingAbortedCases ->
      PendingSource
    [] Bug = "reject_aborted_pending_repair"
       /\ m = BodyRepairLookup
       /\ c \in PendingAbortedCases ->
      SourceWithoutPending(c, m)
    [] Bug = "reject_pending_fallback"
       /\ RejectedPendingHasFallback(c, m) ->
      NoneSource
    [] Bug = "skip_inflight_lookup"
       /\ ~PendingAllowed(c, m)
       /\ c \in InflightPresentCases ->
      SourceWithoutInflight(c)
    [] Bug = "reject_valid_inflight"
       /\ ~PendingAllowed(c, m)
       /\ c \in InflightValidCases ->
      NoneSource
    [] Bug = "accept_invalid_inflight"
       /\ ~PendingAllowed(c, m)
       /\ c \in InflightInvalidCases ->
      InflightSource
    [] Bug = "accept_aborted_inflight_normal"
       /\ ~PendingAllowed(c, m)
       /\ m = NormalLookup
       /\ c \in InflightAbortedCases ->
      InflightSource
    [] Bug = "reject_aborted_inflight_repair"
       /\ ~PendingAllowed(c, m)
       /\ m = BodyRepairLookup
       /\ c \in InflightAbortedCases ->
      SourceWithoutInflight(c)
    [] Bug = "reject_inflight_fallback"
       /\ RejectedInflightHasFallback(c, m) ->
      NoneSource
    [] Bug = "ignore_deferred_payload"
       /\ SpecSource(c, m) = DeferredSource ->
      IF KuraPresent(c) THEN KuraSource ELSE NoneSource
    [] Bug = "deferred_overrides_repair_owner"
       /\ m = BodyRepairLookup
       /\ c \in {PendingAbortedWithDeferred, InflightAbortedWithDeferred} ->
      DeferredSource
    [] Bug = "ignore_kura_block"
       /\ SpecSource(c, m) = KuraSource ->
      NoneSource
    [] Bug = "accept_absent_block"
       /\ c = AbsentBlock ->
      KuraSource
    [] OTHER -> SpecSource(c, m)

ImplementationActions(c, m) ==
  LET spec == SpecActions(c, m) IN
  CASE Bug = "skip_pending_lookup"
       /\ c \in PendingPresentCases ->
      WithReturn(
        spec \ {CheckPending, PendingReturned, PendingInvalidRejected,
                PendingAbortedRejected},
        ImplementationSource(c, m)
      )
    [] Bug = "skip_inflight_lookup"
       /\ ~PendingAllowed(c, m)
       /\ c \in InflightPresentCases ->
      WithReturn(
        spec \ {CheckInflight, InflightReturned, InflightInvalidRejected,
                InflightAbortedRejected},
        ImplementationSource(c, m)
      )
    [] OTHER ->
      WithReturn(spec, ImplementationSource(c, m))

Bugs == {
  "none",
  "skip_pending_lookup",
  "reject_valid_pending",
  "accept_invalid_pending",
  "accept_aborted_pending_normal",
  "reject_aborted_pending_repair",
  "reject_pending_fallback",
  "skip_inflight_lookup",
  "reject_valid_inflight",
  "accept_invalid_inflight",
  "accept_aborted_inflight_normal",
  "reject_aborted_inflight_repair",
  "reject_inflight_fallback",
  "ignore_deferred_payload",
  "deferred_overrides_repair_owner",
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
       \A m \in Modes:
         /\ SpecSource(c, m) \in Sources
         /\ ImplementationSource(c, m) \in Sources
         /\ SpecActions(c, m) \subseteq ActionUniverse
         /\ ImplementationActions(c, m) \subseteq ActionUniverse

SourceMatchesSpec ==
  \A c \in Cases:
    \A m \in Modes:
      ImplementationSource(c, m) = SpecSource(c, m)

ActionsMatchSpec ==
  \A c \in Cases:
    \A m \in Modes:
      ImplementationActions(c, m) = SpecActions(c, m)

NormalLookupRejectsAbortedOwners ==
  /\ ImplementationSource(PendingAborted, NormalLookup) = NoneSource
  /\ ImplementationSource(InflightAborted, NormalLookup) = NoneSource
  /\ PendingAbortedRejected \in ImplementationActions(PendingAborted, NormalLookup)
  /\ InflightAbortedRejected \in ImplementationActions(InflightAborted, NormalLookup)

BodyRepairIncludesAbortedOwners ==
  /\ ImplementationSource(PendingAborted, BodyRepairLookup) = PendingSource
  /\ PendingReturned \in ImplementationActions(PendingAborted, BodyRepairLookup)
  /\ ImplementationSource(InflightAborted, BodyRepairLookup) = InflightSource
  /\ InflightReturned \in ImplementationActions(InflightAborted, BodyRepairLookup)

InvalidOwnersAreNeverReturned ==
  /\ ImplementationSource(PendingInvalid, NormalLookup) = NoneSource
  /\ ImplementationSource(PendingInvalid, BodyRepairLookup) = NoneSource
  /\ ImplementationSource(InflightInvalid, NormalLookup) = NoneSource
  /\ ImplementationSource(InflightInvalid, BodyRepairLookup) = NoneSource
  /\ PendingInvalidRejected \in ImplementationActions(PendingInvalid, NormalLookup)
  /\ PendingInvalidRejected \in ImplementationActions(PendingInvalid, BodyRepairLookup)
  /\ InflightInvalidRejected \in ImplementationActions(InflightInvalid, NormalLookup)
  /\ InflightInvalidRejected \in ImplementationActions(InflightInvalid, BodyRepairLookup)

RejectedOwnersFallThrough ==
  /\ ImplementationSource(PendingInvalidWithInflight, NormalLookup) = InflightSource
  /\ ImplementationSource(PendingInvalidWithDeferred, NormalLookup) = DeferredSource
  /\ ImplementationSource(PendingInvalidWithKura, NormalLookup) = KuraSource
  /\ ImplementationSource(PendingAbortedWithInflight, NormalLookup) = InflightSource
  /\ ImplementationSource(PendingAbortedWithDeferred, NormalLookup) = DeferredSource
  /\ ImplementationSource(PendingAbortedWithKura, NormalLookup) = KuraSource
  /\ ImplementationSource(InflightInvalidWithDeferred, NormalLookup) = DeferredSource
  /\ ImplementationSource(InflightInvalidWithKura, NormalLookup) = KuraSource
  /\ ImplementationSource(InflightAbortedWithDeferred, NormalLookup) = DeferredSource
  /\ ImplementationSource(InflightAbortedWithKura, NormalLookup) = KuraSource

RepairOwnerPriorityBeatsFallback ==
  /\ ImplementationSource(PendingAbortedWithInflight, BodyRepairLookup) = PendingSource
  /\ ImplementationSource(PendingAbortedWithDeferred, BodyRepairLookup) = PendingSource
  /\ ImplementationSource(PendingAbortedWithKura, BodyRepairLookup) = PendingSource
  /\ ImplementationSource(InflightAbortedWithDeferred, BodyRepairLookup) = InflightSource
  /\ ImplementationSource(InflightAbortedWithKura, BodyRepairLookup) = InflightSource

DeferredAndKuraFallbacks ==
  /\ ImplementationSource(DeferredPayload, NormalLookup) = DeferredSource
  /\ ImplementationSource(DeferredPayload, BodyRepairLookup) = DeferredSource
  /\ DeferredReturned \in ImplementationActions(DeferredPayload, NormalLookup)
  /\ DeferredReturned \in ImplementationActions(DeferredPayload, BodyRepairLookup)
  /\ ImplementationSource(KuraBlock, NormalLookup) = KuraSource
  /\ ImplementationSource(KuraBlock, BodyRepairLookup) = KuraSource
  /\ KuraReturned \in ImplementationActions(KuraBlock, NormalLookup)
  /\ KuraReturned \in ImplementationActions(KuraBlock, BodyRepairLookup)

AbsentBlocksRemainMissing ==
  /\ ImplementationSource(AbsentBlock, NormalLookup) = NoneSource
  /\ ImplementationSource(AbsentBlock, BodyRepairLookup) = NoneSource
  /\ ReturnNone \in ImplementationActions(AbsentBlock, NormalLookup)
  /\ ReturnNone \in ImplementationActions(AbsentBlock, BodyRepairLookup)

LookupShapeMatchesPriority ==
  /\ \A c \in Cases:
       \A m \in Modes:
         CheckPending \in ImplementationActions(c, m)
  /\ \A c \in Cases:
       \A m \in Modes:
         PendingAllowed(c, m) =>
           ~(CheckInflight \in ImplementationActions(c, m))
  /\ \A c \in Cases:
       \A m \in Modes:
         ~PendingAllowed(c, m) =>
           CheckInflight \in ImplementationActions(c, m)
  /\ \A c \in Cases:
       \A m \in Modes:
         (PendingAllowed(c, m) \/ InflightAllowed(c, m)) =>
           ~(CheckDeferred \in ImplementationActions(c, m))
  /\ \A c \in Cases:
       \A m \in Modes:
         ~(PendingAllowed(c, m) \/ InflightAllowed(c, m)) =>
           CheckDeferred \in ImplementationActions(c, m)
  /\ \A c \in Cases:
       \A m \in Modes:
         (PendingAllowed(c, m) \/ InflightAllowed(c, m) \/ DeferredPresent(c)) =>
           ~(CheckKura \in ImplementationActions(c, m))
  /\ \A c \in Cases:
       \A m \in Modes:
         ~(PendingAllowed(c, m) \/ InflightAllowed(c, m) \/ DeferredPresent(c)) =>
           CheckKura \in ImplementationActions(c, m)

NoBugInvariant ==
  /\ SourceMatchesSpec
  /\ ActionsMatchSpec
  /\ NormalLookupRejectsAbortedOwners
  /\ BodyRepairIncludesAbortedOwners
  /\ InvalidOwnersAreNeverReturned
  /\ RejectedOwnersFallThrough
  /\ RepairOwnerPriorityBeatsFallback
  /\ DeferredAndKuraFallbacks
  /\ AbsentBlocksRemainMissing
  /\ LookupShapeMatchesPriority

SafetyFast == NoBugInvariant

====
