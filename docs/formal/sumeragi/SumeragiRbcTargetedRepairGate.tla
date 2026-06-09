---- MODULE SumeragiRbcTargetedRepairGate ----
EXTENDS Naturals, Sequences

(***************************************************************************
A bounded abstract model for targeted RBC READY/DELIVER repair helpers.

This slice captures:
- `send_targeted_rbc_ready_set_to_peers(...)`;
- `send_targeted_rbc_deliver_to_peers(...)`; and
- the targeted payload/READY portions of `rescue_rbc_missing_ready_peers(...)`.

The model pins the observable recovery contract: targeted sends reject empty
work, local-only targets, missing rosters, and unavailable bundles; remote
targets are deduplicated before send; READY repair records its cooldown only
after a READY set is actually sent; DELIVER repair emits one DELIVER per remote
target; rescue is disabled for observers, DA-disabled nodes, committed
delivered sessions, invalid sessions, suppressed passive sessions, and
local-only missing READY sets; authoritative local READY evidence may bypass
suppression for READY repair; targeted payload rescue is allowed only outside
authoritative-below-quorum repair or after delivery/quorum; payload timestamps
are recorded only after a body or non-empty chunk bundle is actually sent; and
delivered missing-commit-QC stalls use the max(base, payload) READY cooldown.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Local == "local"
P1 == "p1"
P2 == "p2"
P3 == "p3"

Peers == {Local, P1, P2, P3}

ReadyNoSignatures == "ready_no_signatures"
ReadyNoTargets == "ready_no_targets"
ReadyLocalOnly == "ready_local_only"
ReadyRosterMissing == "ready_roster_missing"
ReadyBundleMissing == "ready_bundle_missing"
ReadySendsDedup == "ready_sends_dedup"

ReadySendCases == {
  ReadyNoSignatures,
  ReadyNoTargets,
  ReadyLocalOnly,
  ReadyRosterMissing,
  ReadyBundleMissing,
  ReadySendsDedup
}

\* @type: Str => Seq(Str);
SpecReadyTargets(c) ==
  CASE c = ReadySendsDedup -> <<P1, P2>>
    [] OTHER -> <<>>

ReadySigCount(c) ==
  IF c = ReadyNoSignatures THEN 0 ELSE 2

ReadyRosterAvailable(c) ==
  c /= ReadyRosterMissing

ReadyBundleAvailable(c) ==
  c /= ReadyBundleMissing

ReadyInputHasRemoteTarget(c) ==
  c \in {ReadyRosterMissing, ReadyBundleMissing, ReadySendsDedup}

ReadyResult(sent, targets, messages, recorded) ==
  [
    sent |-> sent,
    targets |-> targets,
    messages |-> messages,
    recorded |-> recorded
  ]

SpecReadySend(c) ==
  IF ReadySigCount(c) = 0 \/ ~ReadyInputHasRemoteTarget(c) THEN
    ReadyResult(FALSE, <<>>, 0, FALSE)
  ELSE IF ~ReadyRosterAvailable(c) \/ ~ReadyBundleAvailable(c) THEN
    ReadyResult(FALSE, <<>>, 0, FALSE)
  ELSE
    ReadyResult(
      TRUE,
      SpecReadyTargets(c),
      ReadySigCount(c) * Len(SpecReadyTargets(c)),
      TRUE
    )

ActualReadySend(c) ==
  CASE Bug = "ready_accept_no_signatures"
       /\ c = ReadyNoSignatures -> ReadyResult(TRUE, <<P1>>, 1, TRUE)
    [] Bug = "ready_accept_local_only"
       /\ c = ReadyLocalOnly -> ReadyResult(TRUE, <<Local>>, ReadySigCount(c), TRUE)
    [] Bug = "ready_accept_missing_roster"
       /\ c = ReadyRosterMissing -> ReadyResult(TRUE, <<P1>>, ReadySigCount(c), TRUE)
    [] Bug = "ready_accept_missing_bundle"
       /\ c = ReadyBundleMissing -> ReadyResult(TRUE, <<P1>>, ReadySigCount(c), TRUE)
    [] Bug = "ready_keeps_duplicate_targets"
       /\ c = ReadySendsDedup ->
         ReadyResult(TRUE, <<P1, P1, P2>>, ReadySigCount(c) * 3, TRUE)
    [] Bug = "ready_send_not_recorded"
       /\ c = ReadySendsDedup ->
         ReadyResult(TRUE, SpecReadyTargets(c), ReadySigCount(c) * 2, FALSE)
    [] Bug = "ready_wrong_message_count"
       /\ c = ReadySendsDedup ->
         ReadyResult(TRUE, SpecReadyTargets(c), ReadySigCount(c), TRUE)
    [] OTHER -> SpecReadySend(c)

DeliverNoTargets == "deliver_no_targets"
DeliverLocalOnly == "deliver_local_only"
DeliverSendsDedup == "deliver_sends_dedup"

DeliverSendCases == {
  DeliverNoTargets,
  DeliverLocalOnly,
  DeliverSendsDedup
}

\* @type: Str => Seq(Str);
SpecDeliverTargets(c) ==
  IF c = DeliverSendsDedup THEN <<P1, P2>> ELSE <<>>

DeliverResult(sent, targets, messages) ==
  [
    sent |-> sent,
    targets |-> targets,
    messages |-> messages
  ]

SpecDeliverSend(c) ==
  IF c = DeliverSendsDedup THEN
    DeliverResult(TRUE, SpecDeliverTargets(c), Len(SpecDeliverTargets(c)))
  ELSE
    DeliverResult(FALSE, <<>>, 0)

ActualDeliverSend(c) ==
  CASE Bug = "deliver_accept_local_only"
       /\ c = DeliverLocalOnly -> DeliverResult(TRUE, <<Local>>, 1)
    [] Bug = "deliver_keeps_duplicate_targets"
       /\ c = DeliverSendsDedup -> DeliverResult(TRUE, <<P1, P1, P2>>, 3)
    [] Bug = "deliver_wrong_message_count"
       /\ c = DeliverSendsDedup -> DeliverResult(TRUE, SpecDeliverTargets(c), 1)
    [] OTHER -> SpecDeliverSend(c)

RescueObserver == "rescue_observer"
RescueDaDisabled == "rescue_da_disabled"
RescueDeliveredCommitted == "rescue_delivered_committed"
RescueSuppressedPassive == "rescue_suppressed_passive"
RescueInvalid == "rescue_invalid"
RescueLocalOnly == "rescue_local_only"
RescueReadyDue == "rescue_ready_due"
RescuePayloadBodyDue == "rescue_payload_body_due"
RescuePayloadChunksDue == "rescue_payload_chunks_due"
RescuePayloadEmptyChunks == "rescue_payload_empty_chunks"
RescuePayloadNotDueReadyDue == "rescue_payload_not_due_ready_due"
RescueAuthReadyBelowQuorum == "rescue_auth_ready_below_quorum"
RescueAuthReadyDelivered == "rescue_auth_ready_delivered"
RescueAuthReadyQuorum == "rescue_auth_ready_quorum"
RescueDeliveredMissingQcWait == "rescue_delivered_missing_qc_wait"
RescueDeliveredMissingQcAfterMax == "rescue_delivered_missing_qc_after_max"

RescueCases == {
  RescueObserver,
  RescueDaDisabled,
  RescueDeliveredCommitted,
  RescueSuppressedPassive,
  RescueInvalid,
  RescueLocalOnly,
  RescueReadyDue,
  RescuePayloadBodyDue,
  RescuePayloadChunksDue,
  RescuePayloadEmptyChunks,
  RescuePayloadNotDueReadyDue,
  RescueAuthReadyBelowQuorum,
  RescueAuthReadyDelivered,
  RescueAuthReadyQuorum,
  RescueDeliveredMissingQcWait,
  RescueDeliveredMissingQcAfterMax
}

Observer(c) == c = RescueObserver
DaEnabled(c) == c /= RescueDaDisabled
DeliveredCommitted(c) == c = RescueDeliveredCommitted
Suppressed(c) == c = RescueSuppressedPassive
Invalid(c) == c = RescueInvalid
RemoteTargets(c) == c /= RescueLocalOnly

SessionDelivered(c) ==
  c \in {
    RescueAuthReadyDelivered,
    RescueDeliveredMissingQcWait,
    RescueDeliveredMissingQcAfterMax
  }

AuthoritativeReadyRepair(c) ==
  c \in {
    RescueAuthReadyBelowQuorum,
    RescueAuthReadyDelivered,
    RescueAuthReadyQuorum,
    RescueDeliveredMissingQcWait,
    RescueDeliveredMissingQcAfterMax
  }

ReadyCount(c) ==
  CASE c \in {
      RescueAuthReadyBelowQuorum,
      RescueDeliveredMissingQcWait,
      RescueDeliveredMissingQcAfterMax
    } -> 2
    [] OTHER -> 3

RequiredReady(c) ==
  3

ReadyQuorum(c) ==
  RequiredReady(c) /= 0 /\ ReadyCount(c) >= RequiredReady(c)

PayloadAllowed(c) ==
  ~AuthoritativeReadyRepair(c) \/ SessionDelivered(c) \/ ReadyQuorum(c)

PayloadDue(c) ==
  c \in {
    RescuePayloadBodyDue,
    RescuePayloadChunksDue,
    RescuePayloadEmptyChunks,
    RescueAuthReadyBelowQuorum,
    RescueAuthReadyDelivered,
    RescueAuthReadyQuorum
  }

PayloadBody(c) ==
  c = RescuePayloadBodyDue

PayloadChunks(c) ==
  c \in {
    RescuePayloadChunksDue,
    RescueAuthReadyDelivered,
    RescueAuthReadyQuorum
  }

PayloadEmptyChunks(c) ==
  c = RescuePayloadEmptyChunks

PayloadSent(c) ==
  PayloadAllowed(c) /\ PayloadDue(c) /\ (PayloadBody(c) \/ PayloadChunks(c))

ReadyBaseDue(c) ==
  c \in {
    RescueReadyDue,
    RescuePayloadNotDueReadyDue,
    RescueAuthReadyBelowQuorum,
    RescueAuthReadyDelivered,
    RescueAuthReadyQuorum
  }

MissingCommitQcPending(c) ==
  c \in {RescueDeliveredMissingQcWait, RescueDeliveredMissingQcAfterMax}

ReadyDue(c) ==
  IF SessionDelivered(c)
     /\ RequiredReady(c) /= 0
     /\ ReadyCount(c) < RequiredReady(c)
     /\ MissingCommitQcPending(c)
  THEN c = RescueDeliveredMissingQcAfterMax
  ELSE ReadyBaseDue(c)

EarlyReject(c) ==
  Observer(c)
  \/ ~DaEnabled(c)
  \/ DeliveredCommitted(c)
  \/ (Suppressed(c) /\ ~SessionDelivered(c) /\ ~AuthoritativeReadyRepair(c))
  \/ Invalid(c)
  \/ ~RemoteTargets(c)

RescueResult(sent, ready_sent, payload_sent, ready_recorded, payload_recorded) ==
  [
    sent |-> sent,
    ready_sent |-> ready_sent,
    payload_sent |-> payload_sent,
    ready_recorded |-> ready_recorded,
    payload_recorded |-> payload_recorded
  ]

SpecRescue(c) ==
  IF EarlyReject(c) THEN
    RescueResult(FALSE, FALSE, FALSE, FALSE, FALSE)
  ELSE
    LET payload_sent == PayloadSent(c)
        ready_sent == ReadyDue(c)
    IN RescueResult(
      payload_sent \/ ready_sent,
      ready_sent,
      payload_sent,
      ready_sent,
      payload_sent
    )

ActualRescue(c) ==
  CASE Bug = "rescue_observer_sends"
       /\ c = RescueObserver -> RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "rescue_da_disabled_sends"
       /\ c = RescueDaDisabled -> RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "rescue_committed_sends"
       /\ c = RescueDeliveredCommitted -> RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "rescue_suppressed_passive_sends"
       /\ c = RescueSuppressedPassive -> RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "rescue_invalid_sends"
       /\ c = RescueInvalid -> RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "rescue_local_only_sends"
       /\ c = RescueLocalOnly -> RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "rescue_ready_due_ignored"
       /\ c = RescueReadyDue -> RescueResult(FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "rescue_payload_body_not_recorded"
       /\ c = RescuePayloadBodyDue -> RescueResult(TRUE, FALSE, TRUE, FALSE, FALSE)
    [] Bug = "rescue_payload_chunks_ignored"
       /\ c = RescuePayloadChunksDue -> RescueResult(FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "rescue_empty_chunks_recorded"
       /\ c = RescuePayloadEmptyChunks -> RescueResult(TRUE, FALSE, TRUE, FALSE, TRUE)
    [] Bug = "rescue_payload_not_due_sends"
       /\ c = RescuePayloadNotDueReadyDue -> RescueResult(TRUE, TRUE, TRUE, TRUE, TRUE)
    [] Bug = "rescue_auth_below_quorum_payload_sends"
       /\ c = RescueAuthReadyBelowQuorum -> RescueResult(TRUE, TRUE, TRUE, TRUE, TRUE)
    [] Bug = "rescue_auth_delivered_payload_blocked"
       /\ c = RescueAuthReadyDelivered -> RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "rescue_auth_quorum_payload_blocked"
       /\ c = RescueAuthReadyQuorum -> RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "rescue_missing_qc_uses_base_cooldown"
       /\ c = RescueDeliveredMissingQcWait -> RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "rescue_missing_qc_after_max_waits"
       /\ c = RescueDeliveredMissingQcAfterMax -> RescueResult(FALSE, FALSE, FALSE, FALSE, FALSE)
    [] OTHER -> SpecRescue(c)

BugSet == {
  "none",
  "ready_accept_no_signatures",
  "ready_accept_local_only",
  "ready_accept_missing_roster",
  "ready_accept_missing_bundle",
  "ready_keeps_duplicate_targets",
  "ready_send_not_recorded",
  "ready_wrong_message_count",
  "deliver_accept_local_only",
  "deliver_keeps_duplicate_targets",
  "deliver_wrong_message_count",
  "rescue_observer_sends",
  "rescue_da_disabled_sends",
  "rescue_committed_sends",
  "rescue_suppressed_passive_sends",
  "rescue_invalid_sends",
  "rescue_local_only_sends",
  "rescue_ready_due_ignored",
  "rescue_payload_body_not_recorded",
  "rescue_payload_chunks_ignored",
  "rescue_empty_chunks_recorded",
  "rescue_payload_not_due_sends",
  "rescue_auth_below_quorum_payload_sends",
  "rescue_auth_delivered_payload_blocked",
  "rescue_auth_quorum_payload_blocked",
  "rescue_missing_qc_uses_base_cooldown",
  "rescue_missing_qc_after_max_waits"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 26
     /\ checked' = checked + 1
  \/ /\ checked = 26
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked \in 0..26
  /\ \A c \in ReadySendCases:
       /\ ActualReadySend(c).sent \in BOOLEAN
       /\ Len(ActualReadySend(c).targets) <= 3
       /\ \A i \in 1..Len(ActualReadySend(c).targets):
            ActualReadySend(c).targets[i] \in Peers
       /\ ActualReadySend(c).messages \in Nat
       /\ ActualReadySend(c).recorded \in BOOLEAN
  /\ \A c \in DeliverSendCases:
       /\ ActualDeliverSend(c).sent \in BOOLEAN
       /\ Len(ActualDeliverSend(c).targets) <= 3
       /\ \A i \in 1..Len(ActualDeliverSend(c).targets):
            ActualDeliverSend(c).targets[i] \in Peers
       /\ ActualDeliverSend(c).messages \in Nat
  /\ \A c \in RescueCases:
       /\ ActualRescue(c).sent \in BOOLEAN
       /\ ActualRescue(c).ready_sent \in BOOLEAN
       /\ ActualRescue(c).payload_sent \in BOOLEAN
       /\ ActualRescue(c).ready_recorded \in BOOLEAN
       /\ ActualRescue(c).payload_recorded \in BOOLEAN

ReadySendExact ==
  \A c \in ReadySendCases:
    ActualReadySend(c) = SpecReadySend(c)

DeliverSendExact ==
  \A c \in DeliverSendCases:
    ActualDeliverSend(c) = SpecDeliverSend(c)

RescueExact ==
  \A c \in RescueCases:
    ActualRescue(c) = SpecRescue(c)

TargetedSendStable ==
  /\ ~ActualReadySend(ReadyNoSignatures).sent
  /\ ~ActualReadySend(ReadyLocalOnly).sent
  /\ ~ActualReadySend(ReadyRosterMissing).sent
  /\ ~ActualReadySend(ReadyBundleMissing).sent
  /\ ActualReadySend(ReadySendsDedup).targets = <<P1, P2>>
  /\ ActualReadySend(ReadySendsDedup).messages = 4
  /\ ActualReadySend(ReadySendsDedup).recorded
  /\ ~ActualDeliverSend(DeliverLocalOnly).sent
  /\ ActualDeliverSend(DeliverSendsDedup).targets = <<P1, P2>>
  /\ ActualDeliverSend(DeliverSendsDedup).messages = 2

RescueGatesStable ==
  /\ ~ActualRescue(RescueObserver).sent
  /\ ~ActualRescue(RescueDaDisabled).sent
  /\ ~ActualRescue(RescueDeliveredCommitted).sent
  /\ ~ActualRescue(RescueSuppressedPassive).sent
  /\ ~ActualRescue(RescueInvalid).sent
  /\ ~ActualRescue(RescueLocalOnly).sent

RescueProgressStable ==
  /\ ActualRescue(RescueReadyDue).ready_sent
  /\ ActualRescue(RescuePayloadBodyDue).payload_recorded
  /\ ActualRescue(RescuePayloadChunksDue).payload_recorded
  /\ ~ActualRescue(RescuePayloadEmptyChunks).payload_recorded
  /\ ActualRescue(RescuePayloadNotDueReadyDue).ready_sent
  /\ ~ActualRescue(RescuePayloadNotDueReadyDue).payload_sent
  /\ ActualRescue(RescueAuthReadyBelowQuorum).ready_sent
  /\ ~ActualRescue(RescueAuthReadyBelowQuorum).payload_sent
  /\ ActualRescue(RescueAuthReadyDelivered).payload_recorded
  /\ ActualRescue(RescueAuthReadyQuorum).payload_recorded
  /\ ~ActualRescue(RescueDeliveredMissingQcWait).ready_sent
  /\ ActualRescue(RescueDeliveredMissingQcAfterMax).ready_sent

RbcTargetedRepairCoreSafety ==
  /\ ReadySendExact
  /\ DeliverSendExact
  /\ RescueExact
  /\ TargetedSendStable
  /\ RescueGatesStable
  /\ RescueProgressStable

SafetyFast ==
  RbcTargetedRepairCoreSafety

AllReadySendCasesMatchSpec ==
  \A c \in ReadySendCases:
    ActualReadySend(c) = SpecReadySend(c)

AllDeliverSendCasesMatchSpec ==
  \A c \in DeliverSendCases:
    ActualDeliverSend(c) = SpecDeliverSend(c)

AllRescueCasesMatchSpec ==
  \A c \in RescueCases:
    ActualRescue(c) = SpecRescue(c)

ReadyRejectAnchors ==
  /\ ActualReadySend(ReadyNoSignatures) =
       ReadyResult(FALSE, <<>>, 0, FALSE)
  /\ ActualReadySend(ReadyNoTargets) =
       ReadyResult(FALSE, <<>>, 0, FALSE)
  /\ ActualReadySend(ReadyLocalOnly) =
       ReadyResult(FALSE, <<>>, 0, FALSE)
  /\ ActualReadySend(ReadyRosterMissing) =
       ReadyResult(FALSE, <<>>, 0, FALSE)
  /\ ActualReadySend(ReadyBundleMissing) =
       ReadyResult(FALSE, <<>>, 0, FALSE)

ReadySendAnchors ==
  /\ ActualReadySend(ReadySendsDedup).sent
  /\ ActualReadySend(ReadySendsDedup).targets = <<P1, P2>>
  /\ ActualReadySend(ReadySendsDedup).messages = 4
  /\ ActualReadySend(ReadySendsDedup).recorded

DeliverSendAnchors ==
  /\ ActualDeliverSend(DeliverNoTargets) = DeliverResult(FALSE, <<>>, 0)
  /\ ActualDeliverSend(DeliverLocalOnly) = DeliverResult(FALSE, <<>>, 0)
  /\ ActualDeliverSend(DeliverSendsDedup) =
       DeliverResult(TRUE, <<P1, P2>>, 2)

RescueEarlyRejectAnchors ==
  /\ ActualRescue(RescueObserver) =
       RescueResult(FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualRescue(RescueDaDisabled) =
       RescueResult(FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualRescue(RescueDeliveredCommitted) =
       RescueResult(FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualRescue(RescueSuppressedPassive) =
       RescueResult(FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualRescue(RescueInvalid) =
       RescueResult(FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualRescue(RescueLocalOnly) =
       RescueResult(FALSE, FALSE, FALSE, FALSE, FALSE)

RescueReadyAnchors ==
  /\ ActualRescue(RescueReadyDue) =
       RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)
  /\ ActualRescue(RescuePayloadNotDueReadyDue) =
       RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)

RescuePayloadAnchors ==
  /\ ActualRescue(RescuePayloadBodyDue) =
       RescueResult(TRUE, FALSE, TRUE, FALSE, TRUE)
  /\ ActualRescue(RescuePayloadChunksDue) =
       RescueResult(TRUE, FALSE, TRUE, FALSE, TRUE)
  /\ ActualRescue(RescuePayloadEmptyChunks) =
       RescueResult(FALSE, FALSE, FALSE, FALSE, FALSE)

RescueAuthoritativeReadyAnchors ==
  /\ ActualRescue(RescueAuthReadyBelowQuorum) =
       RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)
  /\ ActualRescue(RescueAuthReadyDelivered) =
       RescueResult(TRUE, TRUE, TRUE, TRUE, TRUE)
  /\ ActualRescue(RescueAuthReadyQuorum) =
       RescueResult(TRUE, TRUE, TRUE, TRUE, TRUE)

RescueMissingQcAnchors ==
  /\ ActualRescue(RescueDeliveredMissingQcWait) =
       RescueResult(FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualRescue(RescueDeliveredMissingQcAfterMax) =
       RescueResult(TRUE, TRUE, FALSE, TRUE, FALSE)

SafetyAnchors ==
  /\ AllReadySendCasesMatchSpec
  /\ AllDeliverSendCasesMatchSpec
  /\ AllRescueCasesMatchSpec
  /\ ReadyRejectAnchors
  /\ ReadySendAnchors
  /\ DeliverSendAnchors
  /\ RescueEarlyRejectAnchors
  /\ RescueReadyAnchors
  /\ RescuePayloadAnchors
  /\ RescueAuthoritativeReadyAnchors
  /\ RescueMissingQcAnchors

====
