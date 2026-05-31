---- MODULE SumeragiSlotAuthoritativePayloadGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for slot-level authoritative payload knowledge.

This slice captures `slot_has_authoritative_payload(height, view)`. Unlike the
hash-keyed authoritative payload helper, this predicate scans the actor's local
state by slot. A valid pending or commit-inflight owner for the exact slot
short-circuits. Invalid, aborted, retired, or wrong-slot local owners are
ignored and may fall through to later inflight, Kura, or RBC evidence. Kura
counts only when the block loaded at the requested height has the requested view
and is the committed block for that height. RBC counts only when a session key
matches the slot, the branch is not retained, and the RBC progress predicate has
authoritative payload material.
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
PendingRetired == "pending_retired"
PendingWrongHeight == "pending_wrong_height"
PendingWrongView == "pending_wrong_view"
PendingInvalidWithInflight == "pending_invalid_with_inflight"
PendingAbortedWithKura == "pending_aborted_with_kura"
PendingRetiredWithRbc == "pending_retired_with_rbc"
InflightValid == "inflight_valid"
InflightInvalid == "inflight_invalid"
InflightAborted == "inflight_aborted"
InflightRetired == "inflight_retired"
InflightWrongHeight == "inflight_wrong_height"
InflightWrongView == "inflight_wrong_view"
InflightInvalidWithKura == "inflight_invalid_with_kura"
InflightAbortedWithRbc == "inflight_aborted_with_rbc"
KuraCommitted == "kura_committed"
KuraWrongView == "kura_wrong_view"
KuraUncommitted == "kura_uncommitted"
KuraMissing == "kura_missing"
KuraWrongViewWithRbc == "kura_wrong_view_with_rbc"
KuraUncommittedWithRbc == "kura_uncommitted_with_rbc"
RbcAuthoritative == "rbc_authoritative"
RbcRetainedBranch == "rbc_retained_branch"
RbcWrongHeight == "rbc_wrong_height"
RbcWrongView == "rbc_wrong_view"
RbcNoAuthoritativePayload == "rbc_no_authoritative_payload"
AbsentSlot == "absent_slot"

Cases == {
  PendingValid,
  PendingInvalid,
  PendingAborted,
  PendingRetired,
  PendingWrongHeight,
  PendingWrongView,
  PendingInvalidWithInflight,
  PendingAbortedWithKura,
  PendingRetiredWithRbc,
  InflightValid,
  InflightInvalid,
  InflightAborted,
  InflightRetired,
  InflightWrongHeight,
  InflightWrongView,
  InflightInvalidWithKura,
  InflightAbortedWithRbc,
  KuraCommitted,
  KuraWrongView,
  KuraUncommitted,
  KuraMissing,
  KuraWrongViewWithRbc,
  KuraUncommittedWithRbc,
  RbcAuthoritative,
  RbcRetainedBranch,
  RbcWrongHeight,
  RbcWrongView,
  RbcNoAuthoritativePayload,
  AbsentSlot
}

PendingAcceptedCases == {PendingValid}
PendingInvalidCases == {PendingInvalid, PendingInvalidWithInflight}
PendingAbortedCases == {PendingAborted, PendingAbortedWithKura}
PendingRetiredCases == {PendingRetired, PendingRetiredWithRbc}
PendingWrongSlotCases == {PendingWrongHeight, PendingWrongView}
PendingPresentCases ==
  PendingAcceptedCases \cup PendingInvalidCases \cup PendingAbortedCases
    \cup PendingRetiredCases \cup PendingWrongSlotCases

InflightAcceptedCases == {InflightValid, PendingInvalidWithInflight}
InflightInvalidCases == {InflightInvalid, InflightInvalidWithKura}
InflightAbortedCases == {InflightAborted, InflightAbortedWithRbc}
InflightRetiredCases == {InflightRetired}
InflightWrongSlotCases == {InflightWrongHeight, InflightWrongView}
InflightPresentCases ==
  InflightAcceptedCases \cup InflightInvalidCases \cup InflightAbortedCases
    \cup InflightRetiredCases \cup InflightWrongSlotCases

KuraAcceptedCases == {
  KuraCommitted,
  PendingAbortedWithKura,
  InflightInvalidWithKura
}
KuraWrongViewCases == {KuraWrongView, KuraWrongViewWithRbc}
KuraUncommittedCases == {KuraUncommitted, KuraUncommittedWithRbc}
KuraMissingCases == {KuraMissing}
KuraPresentCases ==
  KuraAcceptedCases \cup KuraWrongViewCases \cup KuraUncommittedCases
    \cup KuraMissingCases

RbcAcceptedCases == {
  RbcAuthoritative,
  PendingRetiredWithRbc,
  InflightAbortedWithRbc,
  KuraWrongViewWithRbc,
  KuraUncommittedWithRbc
}
RbcRetainedCases == {RbcRetainedBranch}
RbcWrongSlotCases == {RbcWrongHeight, RbcWrongView}
RbcNoPayloadCases == {RbcNoAuthoritativePayload}
RbcPresentCases ==
  RbcAcceptedCases \cup RbcRetainedCases \cup RbcWrongSlotCases
    \cup RbcNoPayloadCases

PendingAccepted(c) == c \in PendingAcceptedCases
InflightAccepted(c) == c \in InflightAcceptedCases
KuraAccepted(c) == c \in KuraAcceptedCases
RbcAccepted(c) == c \in RbcAcceptedCases

PendingPasses(c) == ~PendingAccepted(c)
InflightPasses(c) == PendingPasses(c) /\ ~InflightAccepted(c)
KuraPasses(c) == InflightPasses(c) /\ ~KuraAccepted(c)

SpecResult(c) ==
  PendingAccepted(c) \/ InflightAccepted(c) \/ KuraAccepted(c)
    \/ RbcAccepted(c)

ReturnTrue == 1
ReturnFalse == 2
CheckPending == 3
CheckInflight == 4
CheckKura == 5
CheckRbc == 6
PendingAcceptedAction == 7
PendingInvalidIgnored == 8
PendingAbortedIgnored == 9
PendingRetiredIgnored == 10
PendingSlotMismatchIgnored == 11
InflightAcceptedAction == 12
InflightInvalidIgnored == 13
InflightAbortedIgnored == 14
InflightRetiredIgnored == 15
InflightSlotMismatchIgnored == 16
KuraAcceptedAction == 17
KuraViewMismatchIgnored == 18
KuraUncommittedIgnored == 19
KuraMissingIgnored == 20
RbcAcceptedAction == 21
RbcRetainedRejected == 22
RbcSlotMismatchRejected == 23
RbcNoAuthoritativePayloadRejected == 24

ActionUniverse == 1..24

PendingAction(c) ==
  CASE PendingAccepted(c) -> {PendingAcceptedAction}
    [] c \in PendingInvalidCases -> {PendingInvalidIgnored}
    [] c \in PendingAbortedCases -> {PendingAbortedIgnored}
    [] c \in PendingRetiredCases -> {PendingRetiredIgnored}
    [] c \in PendingWrongSlotCases -> {PendingSlotMismatchIgnored}
    [] OTHER -> {}

InflightAction(c) ==
  CASE InflightAccepted(c) -> {InflightAcceptedAction}
    [] c \in InflightInvalidCases -> {InflightInvalidIgnored}
    [] c \in InflightAbortedCases -> {InflightAbortedIgnored}
    [] c \in InflightRetiredCases -> {InflightRetiredIgnored}
    [] c \in InflightWrongSlotCases -> {InflightSlotMismatchIgnored}
    [] OTHER -> {}

KuraAction(c) ==
  CASE KuraAccepted(c) -> {KuraAcceptedAction}
    [] c \in KuraWrongViewCases -> {KuraViewMismatchIgnored}
    [] c \in KuraUncommittedCases -> {KuraUncommittedIgnored}
    [] c \in KuraMissingCases -> {KuraMissingIgnored}
    [] OTHER -> {}

RbcAction(c) ==
  CASE RbcAccepted(c) -> {RbcAcceptedAction}
    [] c \in RbcRetainedCases -> {RbcRetainedRejected}
    [] c \in RbcWrongSlotCases -> {RbcSlotMismatchRejected}
    [] c \in RbcNoPayloadCases -> {RbcNoAuthoritativePayloadRejected}
    [] OTHER -> {}

SpecActions(c) ==
  {CheckPending}
    \cup (IF SpecResult(c) THEN {ReturnTrue} ELSE {ReturnFalse})
    \cup PendingAction(c)
    \cup (IF PendingPasses(c) THEN {CheckInflight} ELSE {})
    \cup (IF PendingPasses(c) THEN InflightAction(c) ELSE {})
    \cup (IF InflightPasses(c) THEN {CheckKura} ELSE {})
    \cup (IF InflightPasses(c) THEN KuraAction(c) ELSE {})
    \cup (IF KuraPasses(c) THEN {CheckRbc} ELSE {})
    \cup (IF KuraPasses(c) THEN RbcAction(c) ELSE {})

ImplementationResult(c) ==
  CASE Bug = "reject_valid_pending"
       /\ c = PendingValid ->
      FALSE
    [] Bug = "accept_invalid_pending"
       /\ c = PendingInvalid ->
      TRUE
    [] Bug = "accept_aborted_pending"
       /\ c = PendingAborted ->
      TRUE
    [] Bug = "accept_retired_pending"
       /\ c = PendingRetired ->
      TRUE
    [] Bug = "pending_rejected_blocks_fallback"
       /\ c \in {PendingInvalidWithInflight, PendingAbortedWithKura,
                 PendingRetiredWithRbc} ->
      FALSE
    [] Bug = "accept_wrong_pending_slot"
       /\ c \in PendingWrongSlotCases ->
      TRUE
    [] Bug = "reject_valid_inflight"
       /\ c = InflightValid ->
      FALSE
    [] Bug = "accept_invalid_inflight"
       /\ c = InflightInvalid ->
      TRUE
    [] Bug = "accept_aborted_inflight"
       /\ c = InflightAborted ->
      TRUE
    [] Bug = "accept_retired_inflight"
       /\ c = InflightRetired ->
      TRUE
    [] Bug = "inflight_rejected_blocks_fallback"
       /\ c \in {InflightInvalidWithKura, InflightAbortedWithRbc} ->
      FALSE
    [] Bug = "accept_wrong_inflight_slot"
       /\ c \in InflightWrongSlotCases ->
      TRUE
    [] Bug = "reject_committed_kura"
       /\ c = KuraCommitted ->
      FALSE
    [] Bug = "accept_wrong_view_kura"
       /\ c = KuraWrongView ->
      TRUE
    [] Bug = "accept_uncommitted_kura"
       /\ c = KuraUncommitted ->
      TRUE
    [] Bug = "kura_rejected_blocks_rbc"
       /\ c \in {KuraWrongViewWithRbc, KuraUncommittedWithRbc} ->
      FALSE
    [] Bug = "reject_authoritative_rbc"
       /\ c = RbcAuthoritative ->
      FALSE
    [] Bug = "accept_retained_rbc"
       /\ c = RbcRetainedBranch ->
      TRUE
    [] Bug = "accept_wrong_rbc_slot"
       /\ c \in RbcWrongSlotCases ->
      TRUE
    [] Bug = "accept_non_authoritative_rbc"
       /\ c = RbcNoAuthoritativePayload ->
      TRUE
    [] Bug = "accept_absent_slot"
       /\ c = AbsentSlot ->
      TRUE
    [] OTHER -> SpecResult(c)

WithReturn(actions, result) ==
  (actions \ {ReturnTrue, ReturnFalse})
    \cup (IF result THEN {ReturnTrue} ELSE {ReturnFalse})

ImplementationActions(c) ==
  WithReturn(SpecActions(c), ImplementationResult(c))

Bugs == {
  "none",
  "reject_valid_pending",
  "accept_invalid_pending",
  "accept_aborted_pending",
  "accept_retired_pending",
  "pending_rejected_blocks_fallback",
  "accept_wrong_pending_slot",
  "reject_valid_inflight",
  "accept_invalid_inflight",
  "accept_aborted_inflight",
  "accept_retired_inflight",
  "inflight_rejected_blocks_fallback",
  "accept_wrong_inflight_slot",
  "reject_committed_kura",
  "accept_wrong_view_kura",
  "accept_uncommitted_kura",
  "kura_rejected_blocks_rbc",
  "reject_authoritative_rbc",
  "accept_retained_rbc",
  "accept_wrong_rbc_slot",
  "accept_non_authoritative_rbc",
  "accept_absent_slot"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecResult(c) \in BOOLEAN
       /\ ImplementationResult(c) \in BOOLEAN
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ResultMatchesSpec ==
  \A c \in Cases:
    ImplementationResult(c) = SpecResult(c)

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

PendingSlotPolicy ==
  /\ ImplementationResult(PendingValid) = TRUE
  /\ PendingAcceptedAction \in ImplementationActions(PendingValid)
  /\ \A c \in PendingInvalidCases \cup PendingAbortedCases
             \cup PendingRetiredCases \cup PendingWrongSlotCases:
       c \notin {PendingInvalidWithInflight, PendingAbortedWithKura,
                 PendingRetiredWithRbc}
         => ~PendingAccepted(c)
  /\ ImplementationResult(PendingInvalid) = FALSE
  /\ PendingInvalidIgnored \in ImplementationActions(PendingInvalid)
  /\ ImplementationResult(PendingAborted) = FALSE
  /\ PendingAbortedIgnored \in ImplementationActions(PendingAborted)
  /\ ImplementationResult(PendingRetired) = FALSE
  /\ PendingRetiredIgnored \in ImplementationActions(PendingRetired)
  /\ ImplementationResult(PendingInvalidWithInflight) = TRUE
  /\ ImplementationResult(PendingAbortedWithKura) = TRUE
  /\ ImplementationResult(PendingRetiredWithRbc) = TRUE

InflightSlotPolicy ==
  /\ ImplementationResult(InflightValid) = TRUE
  /\ InflightAcceptedAction \in ImplementationActions(InflightValid)
  /\ ImplementationResult(InflightInvalid) = FALSE
  /\ InflightInvalidIgnored \in ImplementationActions(InflightInvalid)
  /\ ImplementationResult(InflightAborted) = FALSE
  /\ InflightAbortedIgnored \in ImplementationActions(InflightAborted)
  /\ ImplementationResult(InflightRetired) = FALSE
  /\ InflightRetiredIgnored \in ImplementationActions(InflightRetired)
  /\ ImplementationResult(InflightInvalidWithKura) = TRUE
  /\ ImplementationResult(InflightAbortedWithRbc) = TRUE

KuraSlotPolicy ==
  /\ ImplementationResult(KuraCommitted) = TRUE
  /\ KuraAcceptedAction \in ImplementationActions(KuraCommitted)
  /\ ImplementationResult(KuraWrongView) = FALSE
  /\ KuraViewMismatchIgnored \in ImplementationActions(KuraWrongView)
  /\ ImplementationResult(KuraUncommitted) = FALSE
  /\ KuraUncommittedIgnored \in ImplementationActions(KuraUncommitted)
  /\ ImplementationResult(KuraMissing) = FALSE
  /\ KuraMissingIgnored \in ImplementationActions(KuraMissing)
  /\ ImplementationResult(KuraWrongViewWithRbc) = TRUE
  /\ ImplementationResult(KuraUncommittedWithRbc) = TRUE

RbcSlotPolicy ==
  /\ ImplementationResult(RbcAuthoritative) = TRUE
  /\ RbcAcceptedAction \in ImplementationActions(RbcAuthoritative)
  /\ ImplementationResult(RbcRetainedBranch) = FALSE
  /\ RbcRetainedRejected \in ImplementationActions(RbcRetainedBranch)
  /\ ImplementationResult(RbcWrongHeight) = FALSE
  /\ RbcSlotMismatchRejected \in ImplementationActions(RbcWrongHeight)
  /\ ImplementationResult(RbcWrongView) = FALSE
  /\ RbcSlotMismatchRejected \in ImplementationActions(RbcWrongView)
  /\ ImplementationResult(RbcNoAuthoritativePayload) = FALSE
  /\ RbcNoAuthoritativePayloadRejected
       \in ImplementationActions(RbcNoAuthoritativePayload)
  /\ ImplementationResult(AbsentSlot) = FALSE

LookupShapeMatchesShortCircuit ==
  /\ \A c \in Cases:
       CheckPending \in ImplementationActions(c)
  /\ \A c \in PendingAcceptedCases:
       /\ ~(CheckInflight \in ImplementationActions(c))
       /\ ~(CheckKura \in ImplementationActions(c))
       /\ ~(CheckRbc \in ImplementationActions(c))
  /\ \A c \in Cases \ PendingAcceptedCases:
       CheckInflight \in ImplementationActions(c)
  /\ \A c \in InflightAcceptedCases:
       /\ ~(CheckKura \in ImplementationActions(c))
       /\ ~(CheckRbc \in ImplementationActions(c))
  /\ \A c \in Cases \ (PendingAcceptedCases \cup InflightAcceptedCases):
       CheckKura \in ImplementationActions(c)
  /\ \A c \in KuraAcceptedCases:
       ~(CheckRbc \in ImplementationActions(c))
  /\ \A c \in Cases \ (PendingAcceptedCases \cup InflightAcceptedCases
                       \cup KuraAcceptedCases):
       CheckRbc \in ImplementationActions(c)

NoBugInvariant ==
  /\ ResultMatchesSpec
  /\ ActionsMatchSpec
  /\ PendingSlotPolicy
  /\ InflightSlotPolicy
  /\ KuraSlotPolicy
  /\ RbcSlotPolicy
  /\ LookupShapeMatchesShortCircuit

SafetyFast == NoBugInvariant

====
