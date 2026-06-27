---- MODULE SumeragiRbcNextDueGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the non-delivered scheduling surface of
`rbc_next_due(...)`.

This slice pins the observable wakeup contract: empty RBC state has no
deadline; pending persistence refresh and outbound chunk queues wake
immediately; invalid, exact-frontier-owner, inactive, missing-roster, and
unverified-roster sessions are skipped; a live unsent READY session with an
authoritative roster wakes immediately; complete sessions at READY quorum do
not schedule repair; missing chunks or under-quorum READY schedule payload
rebroadcast from the payload cooldown; missing chunks also schedule chunk
repair from the chunk-repair cooldown; READY evidence schedules targeted
payload rescue, using the smaller READY cooldown for exact-frontier body
repair; under-quorum READY evidence schedules READY rebroadcast; deadlines
from missing last-send timestamps are due now, future/overflowing additions
clamp to now, and multiple candidates merge to the earliest deadline.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoDue == -1
Now == 20
PayloadCooldown == 10
ReadyCooldown == 4
DeliverCooldown == 30
MaxTime == 60

Empty == "empty"
PersistRefresh == "persist_refresh"
OutboundChunks == "outbound_chunks"
InvalidSession == "invalid_session"
ExactOwner == "exact_owner"
InactiveSession == "inactive_session"
UnsentReadyAuthoritative == "unsent_ready_authoritative"
UnsentReadyMissingRoster == "unsent_ready_missing_roster"
UnsentReadyUnverifiedRoster == "unsent_ready_unverified_roster"
CompleteReadyQuorum == "complete_ready_quorum"
PayloadNoLast == "payload_no_last"
PayloadCooldownFuture == "payload_cooldown_future"
PayloadCooldownPast == "payload_cooldown_past"
PayloadCooldownOverflow == "payload_cooldown_overflow"
ChunkRepairEarlier == "chunk_repair_earlier"
ChunkRepairNoLast == "chunk_repair_no_last"
ReadyEvidenceTargetedNoLast == "ready_evidence_targeted_no_last"
ReadyEvidenceTargetedFuture == "ready_evidence_targeted_future"
ExactFrontierTargetedUsesReady == "exact_frontier_targeted_uses_ready"
ReadyRebroadcastNoLast == "ready_rebroadcast_no_last"
ReadyRebroadcastFuture == "ready_rebroadcast_future"
ReadyQuorumSkipsReadyRebroadcast == "ready_quorum_skips_ready_rebroadcast"
MergeEarliestPayloadReady == "merge_earliest_payload_ready"
StatusTtlZero == "status_ttl_zero"
StatusStaleFuture == "status_stale_future"
StatusStaleNow == "status_stale_now"

Cases == {
  Empty,
  PersistRefresh,
  OutboundChunks,
  InvalidSession,
  ExactOwner,
  InactiveSession,
  UnsentReadyAuthoritative,
  UnsentReadyMissingRoster,
  UnsentReadyUnverifiedRoster,
  CompleteReadyQuorum,
  PayloadNoLast,
  PayloadCooldownFuture,
  PayloadCooldownPast,
  PayloadCooldownOverflow,
  ChunkRepairEarlier,
  ChunkRepairNoLast,
  ReadyEvidenceTargetedNoLast,
  ReadyEvidenceTargetedFuture,
  ExactFrontierTargetedUsesReady,
  ReadyRebroadcastNoLast,
  ReadyRebroadcastFuture,
  ReadyQuorumSkipsReadyRebroadcast,
  MergeEarliestPayloadReady,
  StatusTtlZero,
  StatusStaleFuture,
  StatusStaleNow
}

MinDue(a, b) ==
  CASE a = NoDue -> b
    [] b = NoDue -> a
    [] a <= b -> a
    [] OTHER -> b

AddDeadline(last, cooldown) ==
  IF last + cooldown > MaxTime THEN Now
  ELSE IF last + cooldown >= Now THEN last + cooldown
  ELSE Now

SpecDue(c) ==
  CASE c = Empty -> NoDue
    [] c = PersistRefresh -> Now
    [] c = OutboundChunks -> Now
    [] c \in {InvalidSession, ExactOwner, InactiveSession} -> NoDue
    [] c = UnsentReadyAuthoritative -> Now
    [] c \in {UnsentReadyMissingRoster, UnsentReadyUnverifiedRoster} -> NoDue
    [] c = CompleteReadyQuorum -> NoDue
    [] c = PayloadNoLast -> Now
    [] c = PayloadCooldownFuture -> AddDeadline(15, PayloadCooldown)
    [] c = PayloadCooldownPast -> AddDeadline(5, PayloadCooldown)
    [] c = PayloadCooldownOverflow -> AddDeadline(55, PayloadCooldown)
    [] c = ChunkRepairEarlier ->
      MinDue(AddDeadline(18, PayloadCooldown), AddDeadline(12, PayloadCooldown))
    [] c = ChunkRepairNoLast -> Now
    [] c = ReadyEvidenceTargetedNoLast -> Now
    [] c = ReadyEvidenceTargetedFuture -> AddDeadline(15, PayloadCooldown)
    [] c = ExactFrontierTargetedUsesReady -> AddDeadline(18, ReadyCooldown)
    [] c = ReadyRebroadcastNoLast -> Now
    [] c = ReadyRebroadcastFuture -> AddDeadline(18, ReadyCooldown)
    [] c = ReadyQuorumSkipsReadyRebroadcast -> AddDeadline(15, PayloadCooldown)
    [] c = MergeEarliestPayloadReady ->
      MinDue(AddDeadline(18, PayloadCooldown), AddDeadline(18, ReadyCooldown))
    [] c = StatusTtlZero -> NoDue
    [] c = StatusStaleFuture -> 27
    [] c = StatusStaleNow -> Now
    [] OTHER -> NoDue

ActualDue(c) ==
  CASE Bug = "empty_wakes_now"
       /\ c = Empty -> Now
    [] Bug = "persist_refresh_ignored"
       /\ c = PersistRefresh -> NoDue
    [] Bug = "outbound_chunks_ignored"
       /\ c = OutboundChunks -> NoDue
    [] Bug = "invalid_session_schedules"
       /\ c = InvalidSession -> Now
    [] Bug = "exact_owner_schedules"
       /\ c = ExactOwner -> Now
    [] Bug = "inactive_session_schedules"
       /\ c = InactiveSession -> Now
    [] Bug = "unsent_ready_waits"
       /\ c = UnsentReadyAuthoritative -> AddDeadline(18, ReadyCooldown)
    [] Bug = "missing_roster_retries_ready"
       /\ c = UnsentReadyMissingRoster -> Now
    [] Bug = "unverified_roster_retries_ready"
       /\ c = UnsentReadyUnverifiedRoster -> Now
    [] Bug = "complete_quorum_repairs"
       /\ c = CompleteReadyQuorum -> Now
    [] Bug = "payload_no_last_waits"
       /\ c = PayloadNoLast -> AddDeadline(15, PayloadCooldown)
    [] Bug = "payload_future_returns_now"
       /\ c = PayloadCooldownFuture -> Now
    [] Bug = "payload_past_returns_raw"
       /\ c = PayloadCooldownPast -> 15
    [] Bug = "payload_overflow_saturates_max"
       /\ c = PayloadCooldownOverflow -> MaxTime
    [] Bug = "chunk_repair_ignored"
       /\ c = ChunkRepairEarlier -> AddDeadline(18, PayloadCooldown)
    [] Bug = "chunk_repair_no_last_waits"
       /\ c = ChunkRepairNoLast -> AddDeadline(12, PayloadCooldown)
    [] Bug = "targeted_no_last_waits"
       /\ c = ReadyEvidenceTargetedNoLast -> AddDeadline(15, PayloadCooldown)
    [] Bug = "targeted_future_returns_now"
       /\ c = ReadyEvidenceTargetedFuture -> Now
    [] Bug = "exact_frontier_uses_payload_cooldown"
       /\ c = ExactFrontierTargetedUsesReady -> AddDeadline(18, PayloadCooldown)
    [] Bug = "ready_rebroadcast_no_last_waits"
       /\ c = ReadyRebroadcastNoLast -> AddDeadline(18, ReadyCooldown)
    [] Bug = "ready_rebroadcast_future_returns_now"
       /\ c = ReadyRebroadcastFuture -> Now
    [] Bug = "ready_quorum_still_ready_rebroadcasts"
       /\ c = ReadyQuorumSkipsReadyRebroadcast -> AddDeadline(18, ReadyCooldown)
    [] Bug = "merge_uses_latest_deadline"
       /\ c = MergeEarliestPayloadReady -> AddDeadline(18, PayloadCooldown)
    [] Bug = "status_ttl_zero_schedules"
       /\ c = StatusTtlZero -> Now
    [] Bug = "status_stale_ignored"
       /\ c = StatusStaleFuture -> NoDue
    [] Bug = "status_stale_now_waits"
       /\ c = StatusStaleNow -> 27
    [] OTHER -> SpecDue(c)

BugSet == {
  "none",
  "empty_wakes_now",
  "persist_refresh_ignored",
  "outbound_chunks_ignored",
  "invalid_session_schedules",
  "exact_owner_schedules",
  "inactive_session_schedules",
  "unsent_ready_waits",
  "missing_roster_retries_ready",
  "unverified_roster_retries_ready",
  "complete_quorum_repairs",
  "payload_no_last_waits",
  "payload_future_returns_now",
  "payload_past_returns_raw",
  "payload_overflow_saturates_max",
  "chunk_repair_ignored",
  "chunk_repair_no_last_waits",
  "targeted_no_last_waits",
  "targeted_future_returns_now",
  "exact_frontier_uses_payload_cooldown",
  "ready_rebroadcast_no_last_waits",
  "ready_rebroadcast_future_returns_now",
  "ready_quorum_still_ready_rebroadcasts",
  "merge_uses_latest_deadline",
  "status_ttl_zero_schedules",
  "status_stale_ignored",
  "status_stale_now_waits"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0
  /\ \A c \in Cases:
       ActualDue(c) \in {NoDue} \cup Now..MaxTime

DueExact ==
  \A c \in Cases:
    ActualDue(c) = SpecDue(c)

EntryGatesStable ==
  /\ ActualDue(Empty) = NoDue
  /\ ActualDue(PersistRefresh) = Now
  /\ ActualDue(OutboundChunks) = Now
  /\ ActualDue(InvalidSession) = NoDue
  /\ ActualDue(ExactOwner) = NoDue
  /\ ActualDue(InactiveSession) = NoDue
  /\ ActualDue(UnsentReadyAuthoritative) = Now
  /\ ActualDue(UnsentReadyMissingRoster) = NoDue
  /\ ActualDue(UnsentReadyUnverifiedRoster) = NoDue
  /\ ActualDue(CompleteReadyQuorum) = NoDue

CooldownStable ==
  /\ ActualDue(PayloadNoLast) = Now
  /\ ActualDue(PayloadCooldownFuture) = 25
  /\ ActualDue(PayloadCooldownPast) = Now
  /\ ActualDue(PayloadCooldownOverflow) = Now
  /\ ActualDue(ChunkRepairEarlier) = 22
  /\ ActualDue(ChunkRepairNoLast) = Now
  /\ ActualDue(ReadyEvidenceTargetedNoLast) = Now
  /\ ActualDue(ReadyEvidenceTargetedFuture) = 25
  /\ ActualDue(ExactFrontierTargetedUsesReady) = 22
  /\ ActualDue(ReadyRebroadcastNoLast) = Now
  /\ ActualDue(ReadyRebroadcastFuture) = 22
  /\ ActualDue(ReadyQuorumSkipsReadyRebroadcast) = 25
  /\ ActualDue(MergeEarliestPayloadReady) = 22

StatusStable ==
  /\ ActualDue(StatusTtlZero) = NoDue
  /\ ActualDue(StatusStaleFuture) = 27
  /\ ActualDue(StatusStaleNow) = Now

RbcNextDueCoreSafety ==
  /\ DueExact
  /\ EntryGatesStable
  /\ CooldownStable
  /\ StatusStable

RbcNextDueExactness == RbcNextDueCoreSafety

RbcNextDueCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcNextDueExactness

SafetyFast ==
  RbcNextDueExactness

====
