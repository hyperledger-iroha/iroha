---- MODULE SumeragiRbcAvailabilityRescheduleGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `rbc_availability_unresolved_for_reschedule(...)`.

The helper gates quorum rescheduling on DA/RBC availability. It is intentionally
fail-open outside DA mode, after the availability timeout, when the block
payload is already local, when no usable session exists, and for invalid or
delivered sessions. Before the timeout it blocks rescheduling when the session
is still pending, still missing chunks, or lacks enough READY signatures.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

DaDisabled == "DaDisabled"
TimeoutBoundary == "TimeoutBoundary"
TimeoutBelowPending == "TimeoutBelowPending"
TimeoutZeroPending == "TimeoutZeroPending"
LocalPayloadAvailable == "LocalPayloadAvailable"
PendingEntry == "PendingEntry"
NoSession == "NoSession"
InvalidSession == "InvalidSession"
DeliveredSession == "DeliveredSession"
CompleteReady == "CompleteReady"
MissingChunks == "MissingChunks"
ZeroTotalReady == "ZeroTotalReady"
NotReady == "NotReady"
CompleteButNotReady == "CompleteButNotReady"

Cases == {
  DaDisabled,
  TimeoutBoundary,
  TimeoutBelowPending,
  TimeoutZeroPending,
  LocalPayloadAvailable,
  PendingEntry,
  NoSession,
  InvalidSession,
  DeliveredSession,
  CompleteReady,
  MissingChunks,
  ZeroTotalReady,
  NotReady,
  CompleteButNotReady
}

FailOpenGateCases == {
  DaDisabled,
  TimeoutBoundary,
  LocalPayloadAvailable
}

TerminalSessionCases == {
  NoSession,
  InvalidSession,
  DeliveredSession,
  CompleteReady,
  ZeroTotalReady
}

PendingSessionCases == {
  TimeoutBelowPending,
  TimeoutZeroPending,
  PendingEntry
}

AvailabilityDeficitCases == {
  MissingChunks,
  NotReady,
  CompleteButNotReady
}

DaEnabled(c) ==
  c /= DaDisabled

AvailabilityTimeout(c) ==
  IF c = TimeoutZeroPending THEN 0 ELSE 100

StallAge(c) ==
  CASE c = TimeoutBoundary -> 100
    [] c = TimeoutBelowPending -> 99
    [] c = TimeoutZeroPending -> 1000
    [] OTHER -> 0

LocalPayload(c) ==
  c = LocalPayloadAvailable

PendingContains(c) ==
  c \in {TimeoutBelowPending, TimeoutZeroPending, PendingEntry}

SessionPresent(c) ==
  c \notin {NoSession}

SessionInvalid(c) ==
  c = InvalidSession

SessionDelivered(c) ==
  c = DeliveredSession

TotalChunks(c) ==
  CASE c = ZeroTotalReady -> 0
    [] OTHER -> 4

ReceivedChunks(c) ==
  CASE c = MissingChunks -> 2
    [] OTHER -> TotalChunks(c)

RequiredReady(c) ==
  3

ReadySignatures(c) ==
  CASE c \in {NotReady, CompleteButNotReady} -> 2
    [] OTHER -> 3

TimedOut(c) ==
  AvailabilityTimeout(c) # 0 /\ StallAge(c) >= AvailabilityTimeout(c)

SessionMissingChunks(c) ==
  TotalChunks(c) # 0 /\ ReceivedChunks(c) < TotalChunks(c)

ReadyQuorum(c) ==
  ReadySignatures(c) >= RequiredReady(c)

SpecUnresolved(c) ==
  IF ~DaEnabled(c) THEN FALSE
  ELSE IF TimedOut(c) THEN FALSE
  ELSE IF LocalPayload(c) THEN FALSE
  ELSE IF PendingContains(c) THEN TRUE
  ELSE IF ~SessionPresent(c) THEN FALSE
  ELSE IF SessionInvalid(c) THEN FALSE
  ELSE IF SessionDelivered(c) THEN FALSE
  ELSE SessionMissingChunks(c) \/ ~ReadyQuorum(c)

ActualUnresolved(c) ==
  CASE Bug = "da_disabled_blocks"
       /\ c = DaDisabled -> TRUE
    [] Bug = "timeout_boundary_blocks"
       /\ c = TimeoutBoundary -> TRUE
    [] Bug = "timeout_zero_lifts_gate"
       /\ c = TimeoutZeroPending -> FALSE
    [] Bug = "local_payload_blocks"
       /\ c = LocalPayloadAvailable -> TRUE
    [] Bug = "pending_entry_ignored"
       /\ c = PendingEntry -> FALSE
    [] Bug = "absent_session_blocks"
       /\ c = NoSession -> TRUE
    [] Bug = "invalid_session_blocks"
       /\ c = InvalidSession -> TRUE
    [] Bug = "delivered_session_blocks"
       /\ c = DeliveredSession -> TRUE
    [] Bug = "complete_ready_blocks"
       /\ c = CompleteReady -> TRUE
    [] Bug = "missing_chunks_ignored"
       /\ c = MissingChunks -> FALSE
    [] Bug = "zero_total_counts_missing"
       /\ c = ZeroTotalReady -> TRUE
    [] Bug = "not_ready_ignored"
       /\ c = NotReady -> FALSE
    [] Bug = "complete_not_ready_allowed"
       /\ c = CompleteButNotReady -> FALSE
    [] OTHER -> SpecUnresolved(c)

BugSet == {
  "none",
  "da_disabled_blocks",
  "timeout_boundary_blocks",
  "timeout_zero_lifts_gate",
  "local_payload_blocks",
  "pending_entry_ignored",
  "absent_session_blocks",
  "invalid_session_blocks",
  "delivered_session_blocks",
  "complete_ready_blocks",
  "missing_chunks_ignored",
  "zero_total_counts_missing",
  "not_ready_ignored",
  "complete_not_ready_allowed"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0
  /\ \A c \in Cases: ActualUnresolved(c) \in BOOLEAN

SelectionExact ==
  \A c \in Cases:
    ActualUnresolved(c) = SpecUnresolved(c)

FailOpenStable ==
  /\ ~ActualUnresolved(DaDisabled)
  /\ ~ActualUnresolved(TimeoutBoundary)
  /\ ~ActualUnresolved(LocalPayloadAvailable)
  /\ ~ActualUnresolved(NoSession)
  /\ ~ActualUnresolved(InvalidSession)
  /\ ~ActualUnresolved(DeliveredSession)
  /\ ~ActualUnresolved(CompleteReady)
  /\ ~ActualUnresolved(ZeroTotalReady)

UnresolvedBlocksStable ==
  /\ ActualUnresolved(TimeoutBelowPending)
  /\ ActualUnresolved(TimeoutZeroPending)
  /\ ActualUnresolved(PendingEntry)
  /\ ActualUnresolved(MissingChunks)
  /\ ActualUnresolved(NotReady)
  /\ ActualUnresolved(CompleteButNotReady)

RbcAvailabilityRescheduleCoreSafety ==
  /\ SelectionExact
  /\ FailOpenStable
  /\ UnresolvedBlocksStable

SafetyFast ==
  RbcAvailabilityRescheduleCoreSafety

RbcAvailabilityFailOpenGateExact ==
  \A c \in FailOpenGateCases:
    /\ ActualUnresolved(c) = SpecUnresolved(c)
    /\ ActualUnresolved(c) = FALSE
    /\ IF c = DaDisabled THEN ~DaEnabled(c) ELSE TRUE
    /\ IF c = TimeoutBoundary THEN TimedOut(c) ELSE TRUE
    /\ IF c = LocalPayloadAvailable THEN LocalPayload(c) ELSE TRUE

RbcAvailabilityTerminalSessionExact ==
  \A c \in TerminalSessionCases:
    /\ ActualUnresolved(c) = SpecUnresolved(c)
    /\ ActualUnresolved(c) = FALSE
    /\ ~PendingContains(c)
    /\ IF c = NoSession THEN ~SessionPresent(c) ELSE TRUE
    /\ IF c = InvalidSession THEN SessionInvalid(c) ELSE TRUE
    /\ IF c = DeliveredSession THEN SessionDelivered(c) ELSE TRUE
    /\ IF c = CompleteReady THEN ~SessionMissingChunks(c) /\ ReadyQuorum(c)
       ELSE TRUE
    /\ IF c = ZeroTotalReady THEN ~SessionMissingChunks(c) /\ ReadyQuorum(c)
       ELSE TRUE

RbcAvailabilityPendingBlocksExact ==
  \A c \in PendingSessionCases:
    /\ ActualUnresolved(c) = SpecUnresolved(c)
    /\ ActualUnresolved(c) = TRUE
    /\ PendingContains(c)
    /\ ~TimedOut(c)

RbcAvailabilityDeficitBlocksExact ==
  \A c \in AvailabilityDeficitCases:
    /\ ActualUnresolved(c) = SpecUnresolved(c)
    /\ ActualUnresolved(c) = TRUE
    /\ ~PendingContains(c)
    /\ SessionPresent(c)
    /\ ~SessionInvalid(c)
    /\ ~SessionDelivered(c)
    /\ IF c = MissingChunks THEN SessionMissingChunks(c) ELSE TRUE
    /\ IF c \in {NotReady, CompleteButNotReady} THEN ~ReadyQuorum(c)
       ELSE TRUE

RbcAvailabilityRescheduleExactness ==
  /\ RbcAvailabilityRescheduleCoreSafety
  /\ RbcAvailabilityFailOpenGateExact
  /\ RbcAvailabilityTerminalSessionExact
  /\ RbcAvailabilityPendingBlocksExact
  /\ RbcAvailabilityDeficitBlocksExact

====
