---- MODULE SumeragiRbcRepairRequestGate ----
EXTENDS Naturals, Sequences

(***************************************************************************
A bounded abstract model for RBC repair request helpers.

This slice captures:
- `rbc_payload_rebroadcast_due(...)` and the sibling due helpers;
- `rbc_missing_ready_peers(...)`;
- `ordered_rbc_repair_targets(...)` and the deterministic wrapper's leader
  preference / commit-quorum target limit;
- `maybe_request_missing_rbc_init(...)`; and
- `maybe_request_missing_rbc_chunks(...)`.

The model pins the observable recovery contract: cooldowns are first-send open,
exact-boundary due, zero-cooldown due, and future-clock safe; repair targets
skip the local peer, deduplicate, honor the preferred leader first, and clamp to
the commit-quorum limit; missing READY peers are projected by validator index;
missing INIT repair requests are once-per-cooldown before fallback; and chunk
repair clears completed sessions, preserves state when no target exists, resets
after chunk progress, and otherwise falls back only after the cooldown.
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
P4 == "p4"

Peers == {Local, P1, P2, P3, P4}

NotNeeded == "not_needed"
Requested == "requested"
Waiting == "waiting"
Fallback == "fallback"

Outcomes == {NotNeeded, Requested, Waiting, Fallback}

DueAbsent == "due_absent"
DueBefore == "due_before"
DueBoundary == "due_boundary"
DueAfter == "due_after"
DueZeroCooldown == "due_zero_cooldown"
DueFutureClock == "due_future_clock"

DueCases == {
  DueAbsent,
  DueBefore,
  DueBoundary,
  DueAfter,
  DueZeroCooldown,
  DueFutureClock
}

HasLast(c) ==
  c /= DueAbsent

DueNow(c) ==
  CASE c = DueBefore -> 19
    [] c = DueFutureClock -> 9
    [] OTHER -> 20

DueLast(c) ==
  CASE c = DueBefore -> 10
    [] c = DueBoundary -> 10
    [] c = DueAfter -> 9
    [] c = DueZeroCooldown -> 20
    [] c = DueFutureClock -> 30
    [] OTHER -> 0

DueCooldown(c) ==
  IF c = DueZeroCooldown THEN 0 ELSE 10

Age(now, last) ==
  IF now >= last THEN now - last ELSE 0

SpecDue(c) ==
  ~HasLast(c) \/ Age(DueNow(c), DueLast(c)) >= DueCooldown(c)

ActualDue(c) ==
  CASE Bug = "due_absent_waits"
       /\ c = DueAbsent -> FALSE
    [] Bug = "due_before_allowed"
       /\ c = DueBefore -> TRUE
    [] Bug = "due_boundary_waits"
       /\ c = DueBoundary -> FALSE
    [] Bug = "due_zero_cooldown_waits"
       /\ c = DueZeroCooldown -> FALSE
    [] Bug = "due_future_allowed"
       /\ c = DueFutureClock -> TRUE
    [] OTHER -> SpecDue(c)

TargetLimitZero == "target_limit_zero"
TargetPreferredFirst == "target_preferred_first"
TargetPreferredLocal == "target_preferred_local"
TargetSkipLocalAndDup == "target_skip_local_and_dup"
TargetTruncate == "target_truncate"
TargetLimitCovers == "target_limit_covers"

TargetCases == {
  TargetLimitZero,
  TargetPreferredFirst,
  TargetPreferredLocal,
  TargetSkipLocalAndDup,
  TargetTruncate,
  TargetLimitCovers
}

\* @type: Str => Seq(Str);
SpecTargets(c) ==
  CASE c = TargetLimitZero -> <<>>
    [] c = TargetPreferredFirst -> <<P2, P1, P3>>
    [] c = TargetPreferredLocal -> <<P1, P2>>
    [] c = TargetSkipLocalAndDup -> <<P1, P2>>
    [] c = TargetTruncate -> <<P1, P2>>
    [] c = TargetLimitCovers -> <<P1, P2>>
    [] OTHER -> <<>>

\* @type: Str => Seq(Str);
ActualTargets(c) ==
  CASE Bug = "target_limit_zero_selects"
       /\ c = TargetLimitZero -> <<P1>>
    [] Bug = "target_includes_local"
       /\ c = TargetPreferredLocal -> <<Local, P1>>
    [] Bug = "target_omits_preferred"
       /\ c = TargetPreferredFirst -> <<P1, P2, P3>>
    [] Bug = "target_keeps_duplicate"
       /\ c = TargetSkipLocalAndDup -> <<P1, P1, P2>>
    [] Bug = "target_ignores_limit"
       /\ c = TargetTruncate -> <<P1, P2, P3>>
    [] OTHER -> SpecTargets(c)

DetEmptyRoster == "det_empty_roster"
DetLocalOnly == "det_local_only"
DetLeaderPreferred == "det_leader_preferred"
DetLeaderLocal == "det_leader_local"
DetFiveValidators == "det_five_validators"

DetCases == {
  DetEmptyRoster,
  DetLocalOnly,
  DetLeaderPreferred,
  DetLeaderLocal,
  DetFiveValidators
}

\* @type: Str => Seq(Str);
SpecDetTargets(c) ==
  CASE c = DetEmptyRoster -> <<>>
    [] c = DetLocalOnly -> <<>>
    [] c = DetLeaderPreferred -> <<P2, P1, P3>>
    [] c = DetLeaderLocal -> <<P1, P2, P3>>
    [] c = DetFiveValidators -> <<P4, P1, P2, P3>>
    [] OTHER -> <<>>

\* @type: Str => Seq(Str);
ActualDetTargets(c) ==
  CASE Bug = "det_empty_returns_target"
       /\ c = DetEmptyRoster -> <<P1>>
    [] Bug = "det_leader_not_first"
       /\ c = DetLeaderPreferred -> <<P1, P2, P3>>
    [] OTHER -> SpecDetTargets(c)

MissingAllReady == "missing_all_ready"
MissingSome == "missing_some"
MissingOutOfRangeReady == "missing_out_of_range_ready"
MissingNoneReady == "missing_none_ready"

MissingReadyCases == {
  MissingAllReady,
  MissingSome,
  MissingOutOfRangeReady,
  MissingNoneReady
}

\* @type: Str => Seq(Str);
SpecMissingReady(c) ==
  CASE c = MissingAllReady -> <<>>
    [] c = MissingSome -> <<P1, P3>>
    [] c = MissingOutOfRangeReady -> <<P2, P3>>
    [] c = MissingNoneReady -> <<P1, P2, P3>>
    [] OTHER -> <<>>

\* @type: Str => Seq(Str);
ActualMissingReady(c) ==
  CASE Bug = "missing_ready_includes_ready"
       /\ c = MissingSome -> <<P1, P2, P3>>
    [] Bug = "missing_ready_oob_blocks_peer"
       /\ c = MissingOutOfRangeReady -> <<P2>>
    [] OTHER -> SpecMissingReady(c)

InitNoTargets == "init_no_targets"
InitFirstRequest == "init_first_request"
InitWaiting == "init_waiting"
InitFallbackBoundary == "init_fallback_boundary"
InitZeroCooldownFallback == "init_zero_cooldown_fallback"
InitFutureClockWaits == "init_future_clock_waits"

InitCases == {
  InitNoTargets,
  InitFirstRequest,
  InitWaiting,
  InitFallbackBoundary,
  InitZeroCooldownFallback,
  InitFutureClockWaits
}

InitTargets(c) ==
  c /= InitNoTargets

InitHasState(c) ==
  c \in {
    InitWaiting,
    InitFallbackBoundary,
    InitZeroCooldownFallback,
    InitFutureClockWaits
  }

InitNow(c) ==
  IF c = InitFutureClockWaits THEN 9 ELSE 20

InitLast(c) ==
  CASE c = InitWaiting -> 12
    [] c = InitFallbackBoundary -> 10
    [] c = InitZeroCooldownFallback -> 20
    [] c = InitFutureClockWaits -> 30
    [] OTHER -> 0

InitCooldown(c) ==
  IF c = InitZeroCooldownFallback THEN 0 ELSE 10

InitResult(outcome, present, last_time, sent, fallback) ==
  [
    outcome |-> outcome,
    state_present |-> present,
    last_sent |-> last_time,
    sent |-> sent,
    fallback |-> fallback
  ]

SpecInit(c) ==
  IF ~InitTargets(c) THEN
    InitResult(NotNeeded, FALSE, 0, FALSE, FALSE)
  ELSE IF ~InitHasState(c) THEN
    InitResult(Requested, TRUE, InitNow(c), TRUE, FALSE)
  ELSE IF Age(InitNow(c), InitLast(c)) < InitCooldown(c) THEN
    InitResult(Waiting, TRUE, InitLast(c), FALSE, FALSE)
  ELSE
    InitResult(Fallback, FALSE, 0, FALSE, TRUE)

ActualInit(c) ==
  CASE Bug = "init_no_targets_requested"
       /\ c = InitNoTargets -> InitResult(Requested, TRUE, InitNow(c), TRUE, FALSE)
    [] Bug = "init_first_not_recorded"
       /\ c = InitFirstRequest -> InitResult(Requested, FALSE, 0, TRUE, FALSE)
    [] Bug = "init_waiting_fallbacks"
       /\ c = InitWaiting -> InitResult(Fallback, FALSE, 0, FALSE, TRUE)
    [] Bug = "init_boundary_waits"
       /\ c = InitFallbackBoundary -> InitResult(Waiting, TRUE, InitLast(c), FALSE, FALSE)
    [] Bug = "init_fallback_keeps_state"
       /\ c = InitFallbackBoundary -> InitResult(Fallback, TRUE, InitLast(c), FALSE, TRUE)
    [] OTHER -> SpecInit(c)

ChunkNoMissingClears == "chunk_no_missing_clears"
ChunkNoTargetsPreserves == "chunk_no_targets_preserves"
ChunkFirstRequest == "chunk_first_request"
ChunkProgressRequests == "chunk_progress_requests"
ChunkProgressAtTimeoutRequests == "chunk_progress_at_timeout_requests"
ChunkWaitingNoProgress == "chunk_waiting_no_progress"
ChunkFallbackBoundary == "chunk_fallback_boundary"
ChunkFutureClockWaits == "chunk_future_clock_waits"

ChunkCases == {
  ChunkNoMissingClears,
  ChunkNoTargetsPreserves,
  ChunkFirstRequest,
  ChunkProgressRequests,
  ChunkProgressAtTimeoutRequests,
  ChunkWaitingNoProgress,
  ChunkFallbackBoundary,
  ChunkFutureClockWaits
}

ChunkMissing(c) ==
  c /= ChunkNoMissingClears

ChunkTargets(c) ==
  c /= ChunkNoTargetsPreserves

ChunkHasState(c) ==
  c \in {
    ChunkNoMissingClears,
    ChunkNoTargetsPreserves,
    ChunkProgressRequests,
    ChunkProgressAtTimeoutRequests,
    ChunkWaitingNoProgress,
    ChunkFallbackBoundary,
    ChunkFutureClockWaits
  }

ChunkNow(c) ==
  IF c = ChunkFutureClockWaits THEN 9 ELSE 20

ChunkLast(c) ==
  CASE c = ChunkProgressRequests -> 15
    [] c = ChunkProgressAtTimeoutRequests -> 10
    [] c = ChunkWaitingNoProgress -> 15
    [] c = ChunkFallbackBoundary -> 10
    [] c = ChunkFutureClockWaits -> 30
    [] OTHER -> 12

ChunkCooldown(c) ==
  10

ChunkReceived(c) ==
  CASE c \in {ChunkProgressRequests, ChunkProgressAtTimeoutRequests} -> 3
    [] c = ChunkFirstRequest -> 1
    [] OTHER -> 2

ChunkSnapshot(c) ==
  CASE c \in {ChunkProgressRequests, ChunkProgressAtTimeoutRequests} -> 2
    [] c = ChunkFirstRequest -> 0
    [] OTHER -> 2

ChunkResult(outcome, present, last_time, snapshot, sent, removed, fallback) ==
  [
    outcome |-> outcome,
    state_present |-> present,
    last_sent |-> last_time,
    received_snapshot |-> snapshot,
    sent |-> sent,
    removed |-> removed,
    fallback |-> fallback
  ]

SpecChunk(c) ==
  IF ~ChunkMissing(c) THEN
    ChunkResult(NotNeeded, FALSE, 0, 0, FALSE, TRUE, FALSE)
  ELSE IF ~ChunkTargets(c) THEN
    ChunkResult(NotNeeded, TRUE, ChunkLast(c), ChunkSnapshot(c), FALSE, FALSE, FALSE)
  ELSE IF ChunkHasState(c) /\ ChunkReceived(c) > ChunkSnapshot(c) THEN
    ChunkResult(Requested, TRUE, ChunkNow(c), ChunkReceived(c), TRUE, TRUE, FALSE)
  ELSE IF ChunkHasState(c)
          /\ Age(ChunkNow(c), ChunkLast(c)) < ChunkCooldown(c) THEN
    ChunkResult(Waiting, TRUE, ChunkLast(c), ChunkSnapshot(c), FALSE, FALSE, FALSE)
  ELSE IF ChunkHasState(c) THEN
    ChunkResult(Fallback, FALSE, 0, 0, FALSE, TRUE, TRUE)
  ELSE
    ChunkResult(Requested, TRUE, ChunkNow(c), ChunkReceived(c), TRUE, FALSE, FALSE)

ActualChunk(c) ==
  CASE Bug = "chunk_no_missing_keeps_state"
       /\ c = ChunkNoMissingClears ->
         ChunkResult(NotNeeded, TRUE, ChunkLast(c), ChunkSnapshot(c), FALSE, FALSE, FALSE)
    [] Bug = "chunk_no_targets_clears_state"
       /\ c = ChunkNoTargetsPreserves ->
         ChunkResult(NotNeeded, FALSE, 0, 0, FALSE, TRUE, FALSE)
    [] Bug = "chunk_first_not_recorded"
       /\ c = ChunkFirstRequest ->
         ChunkResult(Requested, FALSE, 0, 0, TRUE, FALSE, FALSE)
    [] Bug = "chunk_progress_waits"
       /\ c = ChunkProgressRequests ->
         ChunkResult(Waiting, TRUE, ChunkLast(c), ChunkSnapshot(c), FALSE, FALSE, FALSE)
    [] Bug = "chunk_progress_at_timeout_fallbacks"
       /\ c = ChunkProgressAtTimeoutRequests ->
         ChunkResult(Fallback, FALSE, 0, 0, FALSE, TRUE, TRUE)
    [] Bug = "chunk_boundary_waits"
       /\ c = ChunkFallbackBoundary ->
         ChunkResult(Waiting, TRUE, ChunkLast(c), ChunkSnapshot(c), FALSE, FALSE, FALSE)
    [] Bug = "chunk_future_fallbacks"
       /\ c = ChunkFutureClockWaits ->
         ChunkResult(Fallback, FALSE, 0, 0, FALSE, TRUE, TRUE)
    [] OTHER -> SpecChunk(c)

BugSet == {
  "none",
  "due_absent_waits",
  "due_before_allowed",
  "due_boundary_waits",
  "due_zero_cooldown_waits",
  "due_future_allowed",
  "target_limit_zero_selects",
  "target_includes_local",
  "target_omits_preferred",
  "target_keeps_duplicate",
  "target_ignores_limit",
  "det_empty_returns_target",
  "det_leader_not_first",
  "missing_ready_includes_ready",
  "missing_ready_oob_blocks_peer",
  "init_no_targets_requested",
  "init_first_not_recorded",
  "init_waiting_fallbacks",
  "init_boundary_waits",
  "init_fallback_keeps_state",
  "chunk_no_missing_keeps_state",
  "chunk_no_targets_clears_state",
  "chunk_first_not_recorded",
  "chunk_progress_waits",
  "chunk_progress_at_timeout_fallbacks",
  "chunk_boundary_waits",
  "chunk_future_fallbacks"
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
  /\ \A c \in DueCases: ActualDue(c) \in BOOLEAN
  /\ \A c \in TargetCases:
       /\ Len(ActualTargets(c)) <= 4
       /\ \A i \in 1..Len(ActualTargets(c)): ActualTargets(c)[i] \in Peers
  /\ \A c \in DetCases:
       /\ Len(ActualDetTargets(c)) <= 4
       /\ \A i \in 1..Len(ActualDetTargets(c)): ActualDetTargets(c)[i] \in Peers
  /\ \A c \in MissingReadyCases:
       /\ Len(ActualMissingReady(c)) <= 4
       /\ \A i \in 1..Len(ActualMissingReady(c)):
            ActualMissingReady(c)[i] \in Peers
  /\ \A c \in InitCases:
       /\ ActualInit(c).outcome \in Outcomes
       /\ ActualInit(c).state_present \in BOOLEAN
       /\ ActualInit(c).last_sent \in Nat
       /\ ActualInit(c).sent \in BOOLEAN
       /\ ActualInit(c).fallback \in BOOLEAN
  /\ \A c \in ChunkCases:
       /\ ActualChunk(c).outcome \in Outcomes
       /\ ActualChunk(c).state_present \in BOOLEAN
       /\ ActualChunk(c).last_sent \in Nat
       /\ ActualChunk(c).received_snapshot \in Nat
       /\ ActualChunk(c).sent \in BOOLEAN
       /\ ActualChunk(c).removed \in BOOLEAN
       /\ ActualChunk(c).fallback \in BOOLEAN

CooldownExact ==
  \A c \in DueCases:
    ActualDue(c) = SpecDue(c)

TargetsExact ==
  /\ \A c \in TargetCases:
       ActualTargets(c) = SpecTargets(c)
  /\ \A c \in DetCases:
       ActualDetTargets(c) = SpecDetTargets(c)
  /\ \A c \in MissingReadyCases:
       ActualMissingReady(c) = SpecMissingReady(c)

InitRepairStateMachineExact ==
  \A c \in InitCases:
    ActualInit(c) = SpecInit(c)

ChunkRepairStateMachineExact ==
  \A c \in ChunkCases:
    ActualChunk(c) = SpecChunk(c)

CooldownBoundaryStable ==
  /\ ActualDue(DueAbsent)
  /\ ~ActualDue(DueBefore)
  /\ ActualDue(DueBoundary)
  /\ ActualDue(DueAfter)
  /\ ActualDue(DueZeroCooldown)
  /\ ~ActualDue(DueFutureClock)

TargetSelectionStable ==
  /\ ActualTargets(TargetLimitZero) = <<>>
  /\ ActualTargets(TargetPreferredFirst)[1] = P2
  /\ ActualTargets(TargetPreferredLocal) = <<P1, P2>>
  /\ ActualTargets(TargetSkipLocalAndDup) = <<P1, P2>>
  /\ ActualTargets(TargetTruncate) = <<P1, P2>>
  /\ ActualDetTargets(DetEmptyRoster) = <<>>
  /\ ActualDetTargets(DetLeaderPreferred)[1] = P2
  /\ ActualMissingReady(MissingSome) = <<P1, P3>>

RepairStateStable ==
  /\ ActualInit(InitFirstRequest).outcome = Requested
  /\ ActualInit(InitFirstRequest).state_present
  /\ ActualInit(InitWaiting).outcome = Waiting
  /\ ActualInit(InitFallbackBoundary).outcome = Fallback
  /\ ~ActualInit(InitFallbackBoundary).state_present
  /\ ActualChunk(ChunkNoMissingClears).outcome = NotNeeded
  /\ ~ActualChunk(ChunkNoMissingClears).state_present
  /\ ActualChunk(ChunkNoTargetsPreserves).state_present
  /\ ActualChunk(ChunkProgressRequests).outcome = Requested
  /\ ActualChunk(ChunkProgressAtTimeoutRequests).outcome = Requested
  /\ ActualChunk(ChunkFallbackBoundary).outcome = Fallback
  /\ ActualChunk(ChunkFutureClockWaits).outcome = Waiting

SafetyFast ==
  /\ CooldownExact
  /\ TargetsExact
  /\ InitRepairStateMachineExact
  /\ ChunkRepairStateMachineExact
  /\ CooldownBoundaryStable
  /\ TargetSelectionStable
  /\ RepairStateStable

AllDueCasesMatchSpec ==
  \A c \in DueCases:
    ActualDue(c) = SpecDue(c)

AllTargetCasesMatchSpec ==
  \A c \in TargetCases:
    ActualTargets(c) = SpecTargets(c)

AllDeterministicTargetCasesMatchSpec ==
  \A c \in DetCases:
    ActualDetTargets(c) = SpecDetTargets(c)

AllMissingReadyCasesMatchSpec ==
  \A c \in MissingReadyCases:
    ActualMissingReady(c) = SpecMissingReady(c)

AllInitRepairCasesMatchSpec ==
  \A c \in InitCases:
    ActualInit(c) = SpecInit(c)

AllChunkRepairCasesMatchSpec ==
  \A c \in ChunkCases:
    ActualChunk(c) = SpecChunk(c)

DueCooldownAnchors ==
  /\ ActualDue(DueAbsent)
  /\ ~ActualDue(DueBefore)
  /\ ActualDue(DueBoundary)
  /\ ActualDue(DueAfter)
  /\ ActualDue(DueZeroCooldown)
  /\ ~ActualDue(DueFutureClock)

TargetOrderingAnchors ==
  /\ ActualTargets(TargetLimitZero) = <<>>
  /\ ActualTargets(TargetPreferredFirst) = <<P2, P1, P3>>
  /\ ActualTargets(TargetPreferredLocal) = <<P1, P2>>
  /\ ActualTargets(TargetSkipLocalAndDup) = <<P1, P2>>
  /\ ActualTargets(TargetTruncate) = <<P1, P2>>
  /\ ActualTargets(TargetLimitCovers) = <<P1, P2>>

DeterministicTargetAnchors ==
  /\ ActualDetTargets(DetEmptyRoster) = <<>>
  /\ ActualDetTargets(DetLocalOnly) = <<>>
  /\ ActualDetTargets(DetLeaderPreferred) = <<P2, P1, P3>>
  /\ ActualDetTargets(DetLeaderLocal) = <<P1, P2, P3>>
  /\ ActualDetTargets(DetFiveValidators) = <<P4, P1, P2, P3>>

MissingReadyAnchors ==
  /\ ActualMissingReady(MissingAllReady) = <<>>
  /\ ActualMissingReady(MissingSome) = <<P1, P3>>
  /\ ActualMissingReady(MissingOutOfRangeReady) = <<P2, P3>>
  /\ ActualMissingReady(MissingNoneReady) = <<P1, P2, P3>>

InitRepairAnchors ==
  /\ ActualInit(InitNoTargets) =
       InitResult(NotNeeded, FALSE, 0, FALSE, FALSE)
  /\ ActualInit(InitFirstRequest) =
       InitResult(Requested, TRUE, InitNow(InitFirstRequest), TRUE, FALSE)
  /\ ActualInit(InitWaiting) =
       InitResult(Waiting, TRUE, InitLast(InitWaiting), FALSE, FALSE)
  /\ ActualInit(InitFallbackBoundary) =
       InitResult(Fallback, FALSE, 0, FALSE, TRUE)
  /\ ActualInit(InitZeroCooldownFallback) =
       InitResult(Fallback, FALSE, 0, FALSE, TRUE)
  /\ ActualInit(InitFutureClockWaits) =
       InitResult(Waiting, TRUE, InitLast(InitFutureClockWaits), FALSE, FALSE)

ChunkRepairAnchors ==
  /\ ActualChunk(ChunkNoMissingClears) =
       ChunkResult(NotNeeded, FALSE, 0, 0, FALSE, TRUE, FALSE)
  /\ ActualChunk(ChunkNoTargetsPreserves) =
       ChunkResult(NotNeeded, TRUE, ChunkLast(ChunkNoTargetsPreserves),
         ChunkSnapshot(ChunkNoTargetsPreserves), FALSE, FALSE, FALSE)
  /\ ActualChunk(ChunkFirstRequest) =
       ChunkResult(Requested, TRUE, ChunkNow(ChunkFirstRequest),
         ChunkReceived(ChunkFirstRequest), TRUE, FALSE, FALSE)
  /\ ActualChunk(ChunkProgressRequests) =
       ChunkResult(Requested, TRUE, ChunkNow(ChunkProgressRequests),
         ChunkReceived(ChunkProgressRequests), TRUE, TRUE, FALSE)
  /\ ActualChunk(ChunkProgressAtTimeoutRequests) =
       ChunkResult(Requested, TRUE, ChunkNow(ChunkProgressAtTimeoutRequests),
         ChunkReceived(ChunkProgressAtTimeoutRequests), TRUE, TRUE, FALSE)
  /\ ActualChunk(ChunkWaitingNoProgress) =
       ChunkResult(Waiting, TRUE, ChunkLast(ChunkWaitingNoProgress),
         ChunkSnapshot(ChunkWaitingNoProgress), FALSE, FALSE, FALSE)
  /\ ActualChunk(ChunkFallbackBoundary) =
       ChunkResult(Fallback, FALSE, 0, 0, FALSE, TRUE, TRUE)
  /\ ActualChunk(ChunkFutureClockWaits) =
       ChunkResult(Waiting, TRUE, ChunkLast(ChunkFutureClockWaits),
         ChunkSnapshot(ChunkFutureClockWaits), FALSE, FALSE, FALSE)

SafetyAnchors ==
  /\ AllDueCasesMatchSpec
  /\ AllTargetCasesMatchSpec
  /\ AllDeterministicTargetCasesMatchSpec
  /\ AllMissingReadyCasesMatchSpec
  /\ AllInitRepairCasesMatchSpec
  /\ AllChunkRepairCasesMatchSpec
  /\ DueCooldownAnchors
  /\ TargetOrderingAnchors
  /\ DeterministicTargetAnchors
  /\ MissingReadyAnchors
  /\ InitRepairAnchors
  /\ ChunkRepairAnchors

====
