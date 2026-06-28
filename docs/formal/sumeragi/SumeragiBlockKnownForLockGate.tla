---- MODULE SumeragiBlockKnownForLockGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for lock-safety block knowledge.

This slice captures `block_known_for_lock(...)`. The helper is stricter than
`block_known_locally(...)` for pending blocks: pending entries count only after
validation succeeds and retry abortion is absent. Unlike the payload-progress
helper, invalid or aborted pending entries do not stop the lookup; later
non-aborted inflight ownership, hash-only processing, or Kura height knowledge
can still prove the block known for lock/highest-QC handling. Deferred-only
block-sync payloads do not count.
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
PendingInvalidWithInflight == "pending_invalid_with_inflight"
PendingInvalidWithProcessing == "pending_invalid_with_processing"
PendingInvalidWithKura == "pending_invalid_with_kura"
PendingAborted == "pending_aborted"
PendingAbortedWithInflight == "pending_aborted_with_inflight"
PendingAbortedWithProcessing == "pending_aborted_with_processing"
PendingAbortedWithKura == "pending_aborted_with_kura"
InflightValid == "inflight_valid"
InflightInvalid == "inflight_invalid"
InflightAborted == "inflight_aborted"
InflightAbortedWithProcessing == "inflight_aborted_with_processing"
InflightAbortedWithKura == "inflight_aborted_with_kura"
HashOnlyProcessing == "hash_only_processing"
DeferredPayload == "deferred_payload"
KuraBlock == "kura_block"
AbsentBlock == "absent_block"

Cases == {
  PendingValid,
  PendingInvalid,
  PendingInvalidWithInflight,
  PendingInvalidWithProcessing,
  PendingInvalidWithKura,
  PendingAborted,
  PendingAbortedWithInflight,
  PendingAbortedWithProcessing,
  PendingAbortedWithKura,
  InflightValid,
  InflightInvalid,
  InflightAborted,
  InflightAbortedWithProcessing,
  InflightAbortedWithKura,
  HashOnlyProcessing,
  DeferredPayload,
  KuraBlock,
  AbsentBlock
}

PendingInvalidCases == {
  PendingInvalid,
  PendingInvalidWithInflight,
  PendingInvalidWithProcessing,
  PendingInvalidWithKura
}

PendingAbortedCases == {
  PendingAborted,
  PendingAbortedWithInflight,
  PendingAbortedWithProcessing,
  PendingAbortedWithKura
}

PendingPresentCases ==
  {PendingValid} \cup PendingInvalidCases \cup PendingAbortedCases

InflightValidCases == {
  InflightValid,
  PendingInvalidWithInflight,
  PendingAbortedWithInflight
}

InflightInvalidCases == {InflightInvalid}

InflightAbortedCases == {
  InflightAborted,
  InflightAbortedWithProcessing,
  InflightAbortedWithKura
}

InflightPresentCases ==
  InflightValidCases \cup InflightInvalidCases \cup InflightAbortedCases

ProcessingCases == {
  HashOnlyProcessing,
  PendingInvalidWithProcessing,
  PendingAbortedWithProcessing,
  InflightAbortedWithProcessing
}

KuraCases == {
  KuraBlock,
  PendingInvalidWithKura,
  PendingAbortedWithKura,
  InflightAbortedWithKura
}

DeferredCases == {DeferredPayload}

PendingPass(c) == c = PendingValid
InflightPass(c) == c \in (InflightValidCases \cup InflightInvalidCases)
ProcessingPresent(c) == c \in ProcessingCases
KuraPresent(c) == c \in KuraCases

SpecKnown(c) ==
  PendingPass(c)
  \/ InflightPass(c)
  \/ ProcessingPresent(c)
  \/ KuraPresent(c)

ReturnKnown == 1
ReturnUnknown == 2
CheckPending == 3
CheckInflight == 4
CheckProcessing == 5
CheckKura == 6
PendingValidAccepted == 7
PendingInvalidRejected == 8
PendingAbortedRejected == 9
InflightValidAccepted == 10
InflightInvalidAccepted == 11
InflightAbortedRejected == 12
HashOnlyProcessingAccepted == 13
DeferredPayloadIgnored == 14
KuraBlockAccepted == 15

ActionUniverse == 1..15

SpecActions(c) ==
  {CheckPending}
    \cup (IF SpecKnown(c) THEN {ReturnKnown} ELSE {ReturnUnknown})
    \cup (IF ~PendingPass(c) THEN {CheckInflight} ELSE {})
    \cup (IF ~PendingPass(c) /\ ~InflightPass(c)
          THEN {CheckProcessing}
          ELSE {})
    \cup (IF ~PendingPass(c) /\ ~InflightPass(c) /\ ~ProcessingPresent(c)
          THEN {CheckKura}
          ELSE {})
    \cup (IF PendingPass(c) THEN {PendingValidAccepted} ELSE {})
    \cup (IF c \in PendingInvalidCases THEN {PendingInvalidRejected} ELSE {})
    \cup (IF c \in PendingAbortedCases THEN {PendingAbortedRejected} ELSE {})
    \cup (IF c \in InflightValidCases THEN {InflightValidAccepted} ELSE {})
    \cup (IF c \in InflightInvalidCases THEN {InflightInvalidAccepted} ELSE {})
    \cup (IF c \in InflightAbortedCases THEN {InflightAbortedRejected} ELSE {})
    \cup (IF ProcessingPresent(c) /\ ~PendingPass(c) /\ ~InflightPass(c)
          THEN {HashOnlyProcessingAccepted}
          ELSE {})
    \cup (IF c \in DeferredCases THEN {DeferredPayloadIgnored} ELSE {})
    \cup (IF KuraPresent(c)
              /\ ~PendingPass(c)
              /\ ~InflightPass(c)
              /\ ~ProcessingPresent(c)
          THEN {KuraBlockAccepted}
          ELSE {})

WithReturn(actions, known) ==
  (actions \ {ReturnKnown, ReturnUnknown})
    \cup (IF known THEN {ReturnKnown} ELSE {ReturnUnknown})

KnownWithoutPending(c) ==
  InflightPass(c) \/ ProcessingPresent(c) \/ KuraPresent(c)

KnownWithoutInflight(c) ==
  PendingPass(c) \/ ProcessingPresent(c) \/ KuraPresent(c)

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "skip_pending_lookup"
       /\ c \in PendingPresentCases ->
      WithReturn(
        spec \ {CheckPending, PendingValidAccepted, PendingInvalidRejected,
                PendingAbortedRejected},
        KnownWithoutPending(c)
      )
    [] Bug = "reject_valid_pending"
       /\ c = PendingValid ->
      WithReturn(spec \ {PendingValidAccepted}, FALSE)
    [] Bug = "accept_invalid_pending"
       /\ c \in PendingInvalidCases ->
      WithReturn(
        spec \ {PendingInvalidRejected, CheckInflight, CheckProcessing,
                CheckKura},
        TRUE
      )
    [] Bug = "reject_invalid_pending_fallback"
       /\ c \in {PendingInvalidWithInflight, PendingInvalidWithProcessing,
                 PendingInvalidWithKura} ->
      WithReturn(spec, FALSE)
    [] Bug = "accept_aborted_pending"
       /\ c \in PendingAbortedCases ->
      WithReturn(
        spec \ {PendingAbortedRejected, CheckInflight, CheckProcessing,
                CheckKura},
        TRUE
      )
    [] Bug = "reject_aborted_pending_fallback"
       /\ c \in {PendingAbortedWithInflight, PendingAbortedWithProcessing,
                 PendingAbortedWithKura} ->
      WithReturn(spec, FALSE)
    [] Bug = "skip_inflight_lookup"
       /\ c \in InflightPresentCases ->
      WithReturn(
        spec \ {CheckInflight, InflightValidAccepted,
                InflightInvalidAccepted, InflightAbortedRejected},
        KnownWithoutInflight(c)
      )
    [] Bug = "reject_valid_inflight"
       /\ c \in InflightValidCases ->
      WithReturn(spec \ {InflightValidAccepted}, FALSE)
    [] Bug = "reject_invalid_inflight"
       /\ c \in InflightInvalidCases ->
      WithReturn(spec \ {InflightInvalidAccepted}, FALSE)
    [] Bug = "accept_aborted_inflight"
       /\ c \in InflightAbortedCases ->
      WithReturn(spec \ {InflightAbortedRejected}, TRUE)
    [] Bug = "reject_aborted_inflight_fallback"
       /\ c \in {InflightAbortedWithProcessing, InflightAbortedWithKura} ->
      WithReturn(spec, FALSE)
    [] Bug = "ignore_hash_only_processing"
       /\ ProcessingPresent(c) ->
      WithReturn(spec \ {HashOnlyProcessingAccepted}, KuraPresent(c))
    [] Bug = "accept_deferred_payload"
       /\ c = DeferredPayload ->
      WithReturn(spec \ {DeferredPayloadIgnored}, TRUE)
    [] Bug = "ignore_kura_block"
       /\ KuraPresent(c) ->
      WithReturn(spec \ {KuraBlockAccepted}, FALSE)
    [] Bug = "accept_absent_block"
       /\ c = AbsentBlock ->
      WithReturn(spec, TRUE)
    [] OTHER -> spec

Bugs == {
  "none",
  "skip_pending_lookup",
  "reject_valid_pending",
  "accept_invalid_pending",
  "reject_invalid_pending_fallback",
  "accept_aborted_pending",
  "reject_aborted_pending_fallback",
  "skip_inflight_lookup",
  "reject_valid_inflight",
  "reject_invalid_inflight",
  "accept_aborted_inflight",
  "reject_aborted_inflight_fallback",
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

KnownMatchesSpec ==
  \A c \in Cases:
    (ReturnKnown \in ImplementationActions(c)) <=> SpecKnown(c)

PendingValidityIsRequired ==
  /\ ReturnKnown \in ImplementationActions(PendingValid)
  /\ PendingValidAccepted \in ImplementationActions(PendingValid)
  /\ ReturnUnknown \in ImplementationActions(PendingInvalid)
  /\ PendingInvalidRejected \in ImplementationActions(PendingInvalid)
  /\ ~(ReturnKnown \in ImplementationActions(PendingInvalid))
  /\ ReturnUnknown \in ImplementationActions(PendingAborted)
  /\ PendingAbortedRejected \in ImplementationActions(PendingAborted)
  /\ ~(ReturnKnown \in ImplementationActions(PendingAborted))

RejectedPendingFallsThroughToLaterSources ==
  /\ ReturnKnown \in ImplementationActions(PendingInvalidWithInflight)
  /\ ReturnKnown \in ImplementationActions(PendingInvalidWithProcessing)
  /\ ReturnKnown \in ImplementationActions(PendingInvalidWithKura)
  /\ ReturnKnown \in ImplementationActions(PendingAbortedWithInflight)
  /\ ReturnKnown \in ImplementationActions(PendingAbortedWithProcessing)
  /\ ReturnKnown \in ImplementationActions(PendingAbortedWithKura)

InflightEntriesAreKnownUnlessAborted ==
  /\ ReturnKnown \in ImplementationActions(InflightValid)
  /\ InflightValidAccepted \in ImplementationActions(InflightValid)
  /\ ReturnKnown \in ImplementationActions(InflightInvalid)
  /\ InflightInvalidAccepted \in ImplementationActions(InflightInvalid)
  /\ ReturnUnknown \in ImplementationActions(InflightAborted)
  /\ InflightAbortedRejected \in ImplementationActions(InflightAborted)
  /\ ~(ReturnKnown \in ImplementationActions(InflightAborted))

RejectedInflightFallsThroughToLaterSources ==
  /\ ReturnKnown \in ImplementationActions(InflightAbortedWithProcessing)
  /\ ReturnKnown \in ImplementationActions(InflightAbortedWithKura)

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
  /\ ~(CheckInflight \in ImplementationActions(PendingValid))
  /\ \A c \in Cases:
       ~PendingPass(c) => CheckInflight \in ImplementationActions(c)
  /\ \A c \in Cases:
       (PendingPass(c) \/ InflightPass(c)) =>
         ~(CheckProcessing \in ImplementationActions(c))
  /\ \A c \in Cases:
       ~(PendingPass(c) \/ InflightPass(c)) =>
         CheckProcessing \in ImplementationActions(c)
  /\ \A c \in Cases:
       (PendingPass(c) \/ InflightPass(c) \/ ProcessingPresent(c)) =>
         ~(CheckKura \in ImplementationActions(c))
  /\ \A c \in Cases:
       ~(PendingPass(c) \/ InflightPass(c) \/ ProcessingPresent(c)) =>
         CheckKura \in ImplementationActions(c)

BlockKnownForLockCoreSafety ==
  /\ ActionsMatchSpec
  /\ KnownMatchesSpec
  /\ PendingValidityIsRequired
  /\ RejectedPendingFallsThroughToLaterSources
  /\ InflightEntriesAreKnownUnlessAborted
  /\ RejectedInflightFallsThroughToLaterSources
  /\ HashOnlyAndKuraCountAsKnown
  /\ DeferredOnlyAndAbsentDoNotCountAsKnown
  /\ LookupShapeMatchesShortCircuit

BlockKnownForLockExactness ==
  BlockKnownForLockCoreSafety

BlockKnownForLockCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockKnownForLockExactness

NoBugInvariant == BlockKnownForLockExactness

SafetyFast == BlockKnownForLockExactness

====
