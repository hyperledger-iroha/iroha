---- MODULE SumeragiAuthoritativePayloadProgressGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for authoritative payload progress lookup.

This slice captures `with_authoritative_payload_for_progress(...)`. The helper
may return payload bytes from a valid, non-aborted pending owner, a valid,
non-aborted commit-inflight owner, or Kura when the requested block hash is the
committed hash for the loaded block's header height. Invalid or aborted local
owners fail closed immediately, deferred block-sync payloads are ignored, and
Kura misses or uncommitted blocks remain unavailable.
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
PendingValidWithInflight == "pending_valid_with_inflight"
PendingValidWithKura == "pending_valid_with_kura"
PendingInvalid == "pending_invalid"
PendingInvalidWithInflight == "pending_invalid_with_inflight"
PendingInvalidWithDeferred == "pending_invalid_with_deferred"
PendingInvalidWithKura == "pending_invalid_with_kura"
PendingAborted == "pending_aborted"
PendingAbortedWithInflight == "pending_aborted_with_inflight"
PendingAbortedWithDeferred == "pending_aborted_with_deferred"
PendingAbortedWithKura == "pending_aborted_with_kura"
InflightValid == "inflight_valid"
InflightValidWithKura == "inflight_valid_with_kura"
InflightInvalid == "inflight_invalid"
InflightInvalidWithDeferred == "inflight_invalid_with_deferred"
InflightInvalidWithKura == "inflight_invalid_with_kura"
InflightAborted == "inflight_aborted"
InflightAbortedWithDeferred == "inflight_aborted_with_deferred"
InflightAbortedWithKura == "inflight_aborted_with_kura"
DeferredPayload == "deferred_payload"
DeferredWithKura == "deferred_with_kura"
KuraCommitted == "kura_committed"
KuraHeightUnknown == "kura_height_unknown"
KuraBlockMissing == "kura_block_missing"
KuraUncommitted == "kura_uncommitted"
AbsentPayload == "absent_payload"

Cases == {
  PendingValid,
  PendingValidWithInflight,
  PendingValidWithKura,
  PendingInvalid,
  PendingInvalidWithInflight,
  PendingInvalidWithDeferred,
  PendingInvalidWithKura,
  PendingAborted,
  PendingAbortedWithInflight,
  PendingAbortedWithDeferred,
  PendingAbortedWithKura,
  InflightValid,
  InflightValidWithKura,
  InflightInvalid,
  InflightInvalidWithDeferred,
  InflightInvalidWithKura,
  InflightAborted,
  InflightAbortedWithDeferred,
  InflightAbortedWithKura,
  DeferredPayload,
  DeferredWithKura,
  KuraCommitted,
  KuraHeightUnknown,
  KuraBlockMissing,
  KuraUncommitted,
  AbsentPayload
}

PendingValidCases == {
  PendingValid,
  PendingValidWithInflight,
  PendingValidWithKura
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
  PendingValidCases \cup PendingInvalidCases \cup PendingAbortedCases

InflightValidCases == {
  InflightValid,
  InflightValidWithKura
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

DeferredOnlyCases == {
  PendingInvalidWithDeferred,
  PendingAbortedWithDeferred,
  InflightInvalidWithDeferred,
  InflightAbortedWithDeferred,
  DeferredPayload
}

KuraCommittedCases == {
  PendingValidWithKura,
  PendingInvalidWithKura,
  PendingAbortedWithKura,
  InflightValidWithKura,
  InflightInvalidWithKura,
  InflightAbortedWithKura,
  DeferredWithKura,
  KuraCommitted
}

KuraHeightKnownCases == {
  DeferredWithKura,
  KuraCommitted,
  KuraBlockMissing,
  KuraUncommitted
}

KuraBlockLoadedCases == {
  DeferredWithKura,
  KuraCommitted,
  KuraUncommitted
}

PendingSource == "pending"
InflightSource == "inflight"
DeferredSource == "deferred"
KuraSource == "kura"
NoneSource == "none"
Sources == {
  PendingSource,
  InflightSource,
  DeferredSource,
  KuraSource,
  NoneSource
}

SourceAfterRejectedPending(c) ==
  IF c = PendingInvalidWithInflight \/ c = PendingAbortedWithInflight THEN
    InflightSource
  ELSE IF c = PendingInvalidWithDeferred \/ c = PendingAbortedWithDeferred THEN
    DeferredSource
  ELSE IF c = PendingInvalidWithKura \/ c = PendingAbortedWithKura THEN
    KuraSource
  ELSE
    NoneSource

SourceAfterRejectedInflight(c) ==
  IF c = InflightInvalidWithDeferred \/ c = InflightAbortedWithDeferred THEN
    DeferredSource
  ELSE IF c = InflightInvalidWithKura \/ c = InflightAbortedWithKura THEN
    KuraSource
  ELSE
    NoneSource

SourceAfterLocalOwners(c) ==
  IF c = DeferredWithKura \/ c = KuraCommitted THEN KuraSource ELSE NoneSource

SpecSource(c) ==
  IF c \in PendingValidCases THEN PendingSource
  ELSE IF c \in PendingInvalidCases \cup PendingAbortedCases THEN NoneSource
  ELSE IF c \in InflightValidCases THEN InflightSource
  ELSE IF c \in InflightInvalidCases \cup InflightAbortedCases THEN NoneSource
  ELSE SourceAfterLocalOwners(c)

ReturnSome == 1
ReturnNone == 2
CheckPending == 3
CheckInflight == 4
CheckKuraHeight == 5
CheckKuraBlock == 6
CheckCommittedHash == 7
PendingReturned == 8
PendingInvalidRejected == 9
PendingAbortedRejected == 10
InflightReturned == 11
InflightInvalidRejected == 12
InflightAbortedRejected == 13
DeferredIgnored == 14
KuraReturned == 15
KuraHeightMissing == 16
KuraBlockMissingAction == 17
KuraUncommittedRejected == 18

ActionUniverse == 1..18

SpecActions(c) ==
  {CheckPending}
    \cup (IF SpecSource(c) = NoneSource THEN {ReturnNone} ELSE {ReturnSome})
    \cup (IF c \in PendingValidCases THEN {PendingReturned} ELSE {})
    \cup (IF c \in PendingInvalidCases THEN {PendingInvalidRejected} ELSE {})
    \cup (IF c \in PendingAbortedCases THEN {PendingAbortedRejected} ELSE {})
    \cup (IF c \notin PendingPresentCases THEN {CheckInflight} ELSE {})
    \cup (IF c \in InflightValidCases THEN {InflightReturned} ELSE {})
    \cup (IF c \in InflightInvalidCases THEN {InflightInvalidRejected} ELSE {})
    \cup (IF c \in InflightAbortedCases THEN {InflightAbortedRejected} ELSE {})
    \cup (IF c \notin PendingPresentCases \cup InflightPresentCases
          THEN {CheckKuraHeight}
          ELSE {})
    \cup (IF c \notin PendingPresentCases \cup InflightPresentCases
              /\ c \notin KuraHeightKnownCases
          THEN {KuraHeightMissing}
          ELSE {})
    \cup (IF c \notin PendingPresentCases \cup InflightPresentCases
              /\ c \in KuraHeightKnownCases
          THEN {CheckKuraBlock}
          ELSE {})
    \cup (IF c = KuraBlockMissing THEN {KuraBlockMissingAction} ELSE {})
    \cup (IF c \notin PendingPresentCases \cup InflightPresentCases
              /\ c \in KuraBlockLoadedCases
          THEN {CheckCommittedHash}
          ELSE {})
    \cup (IF SpecSource(c) = KuraSource THEN {KuraReturned} ELSE {})
    \cup (IF c \in DeferredOnlyCases \cup {DeferredWithKura}
              /\ c \notin PendingPresentCases \cup InflightPresentCases
          THEN {DeferredIgnored}
          ELSE {})
    \cup (IF c = KuraUncommitted THEN {KuraUncommittedRejected} ELSE {})

WithReturn(actions, source) ==
  (actions \ {ReturnSome, ReturnNone, PendingReturned, InflightReturned,
              KuraReturned})
    \cup (IF source = NoneSource THEN {ReturnNone} ELSE {ReturnSome})
    \cup (IF source = PendingSource THEN {PendingReturned} ELSE {})
    \cup (IF source = InflightSource THEN {InflightReturned} ELSE {})
    \cup (IF source = KuraSource THEN {KuraReturned} ELSE {})

ImplementationSource(c) ==
  CASE Bug = "reject_valid_pending"
       /\ c \in PendingValidCases ->
      NoneSource
    [] Bug = "accept_invalid_pending"
       /\ c \in PendingInvalidCases ->
      PendingSource
    [] Bug = "accept_aborted_pending"
       /\ c \in PendingAbortedCases ->
      PendingSource
    [] Bug = "pending_rejected_falls_through"
       /\ c \in PendingInvalidCases \cup PendingAbortedCases ->
      SourceAfterRejectedPending(c)
    [] Bug = "reject_valid_inflight"
       /\ c \in InflightValidCases ->
      NoneSource
    [] Bug = "accept_invalid_inflight"
       /\ c \in InflightInvalidCases ->
      InflightSource
    [] Bug = "accept_aborted_inflight"
       /\ c \in InflightAbortedCases ->
      InflightSource
    [] Bug = "inflight_rejected_falls_through"
       /\ c \in InflightInvalidCases \cup InflightAbortedCases ->
      SourceAfterRejectedInflight(c)
    [] Bug = "accept_deferred_payload"
       /\ SpecSource(c) \notin {PendingSource, InflightSource}
       /\ c \in DeferredOnlyCases \cup {DeferredWithKura} ->
      DeferredSource
    [] Bug = "ignore_committed_kura"
       /\ SpecSource(c) = KuraSource ->
      NoneSource
    [] Bug = "accept_uncommitted_kura"
       /\ c = KuraUncommitted ->
      KuraSource
    [] Bug = "accept_absent_payload"
       /\ c \in {KuraHeightUnknown, KuraBlockMissing, AbsentPayload} ->
      KuraSource
    [] OTHER -> SpecSource(c)

ImplementationActions(c) ==
  WithReturn(SpecActions(c), ImplementationSource(c))

Bugs == {
  "none",
  "reject_valid_pending",
  "accept_invalid_pending",
  "accept_aborted_pending",
  "pending_rejected_falls_through",
  "reject_valid_inflight",
  "accept_invalid_inflight",
  "accept_aborted_inflight",
  "inflight_rejected_falls_through",
  "accept_deferred_payload",
  "ignore_committed_kura",
  "accept_uncommitted_kura",
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
       /\ SpecSource(c) \in Sources
       /\ ImplementationSource(c) \in Sources
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

SourceMatchesSpec ==
  \A c \in Cases:
    ImplementationSource(c) = SpecSource(c)

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

ValidPendingOwnersWin ==
  /\ \A c \in PendingValidCases:
       /\ ImplementationSource(c) = PendingSource
       /\ PendingReturned \in ImplementationActions(c)
       /\ ~(CheckInflight \in ImplementationActions(c))
       /\ ~(CheckKuraHeight \in ImplementationActions(c))

RejectedPendingOwnersFailClosed ==
  /\ \A c \in PendingInvalidCases:
       /\ ImplementationSource(c) = NoneSource
       /\ PendingInvalidRejected \in ImplementationActions(c)
       /\ ~(CheckInflight \in ImplementationActions(c))
       /\ ~(CheckKuraHeight \in ImplementationActions(c))
  /\ \A c \in PendingAbortedCases:
       /\ ImplementationSource(c) = NoneSource
       /\ PendingAbortedRejected \in ImplementationActions(c)
       /\ ~(CheckInflight \in ImplementationActions(c))
       /\ ~(CheckKuraHeight \in ImplementationActions(c))

ValidInflightOwnersWin ==
  /\ \A c \in InflightValidCases:
       /\ ImplementationSource(c) = InflightSource
       /\ InflightReturned \in ImplementationActions(c)
       /\ CheckInflight \in ImplementationActions(c)
       /\ ~(CheckKuraHeight \in ImplementationActions(c))

RejectedInflightOwnersFailClosed ==
  /\ \A c \in InflightInvalidCases:
       /\ ImplementationSource(c) = NoneSource
       /\ InflightInvalidRejected \in ImplementationActions(c)
       /\ CheckInflight \in ImplementationActions(c)
       /\ ~(CheckKuraHeight \in ImplementationActions(c))
  /\ \A c \in InflightAbortedCases:
       /\ ImplementationSource(c) = NoneSource
       /\ InflightAbortedRejected \in ImplementationActions(c)
       /\ CheckInflight \in ImplementationActions(c)
       /\ ~(CheckKuraHeight \in ImplementationActions(c))

DeferredPayloadsAreNeverReturned ==
  \A c \in Cases:
    ImplementationSource(c) # DeferredSource

CommittedKuraBlocksAreAuthoritative ==
  /\ ImplementationSource(KuraCommitted) = KuraSource
  /\ ImplementationSource(DeferredWithKura) = KuraSource
  /\ KuraReturned \in ImplementationActions(KuraCommitted)
  /\ KuraReturned \in ImplementationActions(DeferredWithKura)
  /\ CheckCommittedHash \in ImplementationActions(KuraCommitted)
  /\ CheckCommittedHash \in ImplementationActions(DeferredWithKura)

KuraMissesAndUncommittedBlocksStayMissing ==
  /\ ImplementationSource(KuraHeightUnknown) = NoneSource
  /\ KuraHeightMissing \in ImplementationActions(KuraHeightUnknown)
  /\ ImplementationSource(KuraBlockMissing) = NoneSource
  /\ KuraBlockMissingAction \in ImplementationActions(KuraBlockMissing)
  /\ ImplementationSource(KuraUncommitted) = NoneSource
  /\ KuraUncommittedRejected \in ImplementationActions(KuraUncommitted)
  /\ ImplementationSource(AbsentPayload) = NoneSource

LookupShapeMatchesShortCircuit ==
  /\ \A c \in Cases:
       CheckPending \in ImplementationActions(c)
  /\ \A c \in PendingPresentCases:
       /\ ~(CheckInflight \in ImplementationActions(c))
       /\ ~(CheckKuraHeight \in ImplementationActions(c))
  /\ \A c \in Cases \ PendingPresentCases:
       CheckInflight \in ImplementationActions(c)
  /\ \A c \in InflightPresentCases:
       ~(CheckKuraHeight \in ImplementationActions(c))
  /\ \A c \in Cases \ (PendingPresentCases \cup InflightPresentCases):
       CheckKuraHeight \in ImplementationActions(c)
  /\ \A c \in {KuraHeightUnknown, DeferredPayload, AbsentPayload}:
       ~(CheckKuraBlock \in ImplementationActions(c))
  /\ \A c \in KuraHeightKnownCases:
       CheckKuraBlock \in ImplementationActions(c)
  /\ \A c \in KuraBlockLoadedCases:
       CheckCommittedHash \in ImplementationActions(c)

AuthoritativePayloadProgressCoreSafety ==
  /\ SourceMatchesSpec
  /\ ActionsMatchSpec
  /\ ValidPendingOwnersWin
  /\ RejectedPendingOwnersFailClosed
  /\ ValidInflightOwnersWin
  /\ RejectedInflightOwnersFailClosed
  /\ DeferredPayloadsAreNeverReturned
  /\ CommittedKuraBlocksAreAuthoritative
  /\ KuraMissesAndUncommittedBlocksStayMissing
  /\ LookupShapeMatchesShortCircuit

AuthoritativePayloadProgressExactness ==
  /\ SourceMatchesSpec
  /\ ActionsMatchSpec
  /\ ValidPendingOwnersWin
  /\ RejectedPendingOwnersFailClosed
  /\ ValidInflightOwnersWin
  /\ RejectedInflightOwnersFailClosed
  /\ DeferredPayloadsAreNeverReturned
  /\ CommittedKuraBlocksAreAuthoritative
  /\ KuraMissesAndUncommittedBlocksStayMissing
  /\ LookupShapeMatchesShortCircuit
AuthoritativePayloadProgressCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ AuthoritativePayloadProgressExactness

NoBugInvariant == AuthoritativePayloadProgressExactness

SafetyFast == AuthoritativePayloadProgressExactness

====
