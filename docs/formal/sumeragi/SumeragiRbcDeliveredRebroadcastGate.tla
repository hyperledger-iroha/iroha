---- MODULE SumeragiRbcDeliveredRebroadcastGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for delivered-session RBC rebroadcast handling.

This slice captures:
- the delivered-session deadline branch in `rbc_next_due(...)`; and
- the delivered-session branch of `rebroadcast_stalled_rbc_payloads(...)`.

The model pins the observable contract: empty/invalid/exact-frontier-owner,
inactive, and hot-repair-suppressed sessions do not schedule delivered DELIVER
rebroadcast work; delivered sessions without a previous DELIVER rebroadcast are
due immediately; previous-send deadlines are `last + cooldown` clamped to at
least `now`; observers and DA-disabled nodes do not rebroadcast; an empty
session set clears the rebroadcast cursor; delivered sessions use only targeted
READY rescue and DELIVER rebroadcast, never payload repair or READY-set broad
rebroadcast; DELIVER rebroadcast requires hot repair, cooldown due, and a
buildable DELIVER; and successful DELIVER rebroadcast records the timestamp and
reports progress.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NextEmpty == "next_empty"
NextInvalid == "next_invalid"
NextExactOwner == "next_exact_owner"
NextInactive == "next_inactive"
NextHotSuppressed == "next_hot_suppressed"
NextNoLast == "next_no_last"
NextBeforeCooldown == "next_before_cooldown"
NextBoundary == "next_boundary"
NextFutureLast == "next_future_last"

NextCases == {
  NextEmpty,
  NextInvalid,
  NextExactOwner,
  NextInactive,
  NextHotSuppressed,
  NextNoLast,
  NextBeforeCooldown,
  NextBoundary,
  NextFutureLast
}

TickObserver == "tick_observer"
TickDaDisabled == "tick_da_disabled"
TickNoSessions == "tick_no_sessions"
TickHotSuppressed == "tick_hot_suppressed"
TickRescueOnly == "tick_rescue_only"
TickDeliverNotDue == "tick_deliver_not_due"
TickDeliverBuilderMissing == "tick_deliver_builder_missing"
TickDeliverBroadcast == "tick_deliver_broadcast"
TickRescueAndBroadcast == "tick_rescue_and_broadcast"

TickCases == {
  TickObserver,
  TickDaDisabled,
  TickNoSessions,
  TickHotSuppressed,
  TickRescueOnly,
  TickDeliverNotDue,
  TickDeliverBuilderMissing,
  TickDeliverBroadcast,
  TickRescueAndBroadcast
}

Now == 20
Cooldown == 10

NoDue ==
  [due |-> FALSE, time |-> 0]

DueAt(t) ==
  [due |-> TRUE, time |-> t]

Deadline(last) ==
  LET raw == last + Cooldown IN
  IF raw >= Now THEN raw ELSE Now

SpecNext(c) ==
  CASE c \in {
      NextEmpty,
      NextInvalid,
      NextExactOwner,
      NextInactive,
      NextHotSuppressed
    } -> NoDue
    [] c = NextNoLast -> DueAt(Now)
    [] c = NextBeforeCooldown -> DueAt(Deadline(15))
    [] c = NextBoundary -> DueAt(Deadline(10))
    [] c = NextFutureLast -> DueAt(Deadline(30))
    [] OTHER -> NoDue

ActualNext(c) ==
  CASE Bug = "next_empty_schedules"
       /\ c = NextEmpty -> DueAt(Now)
    [] Bug = "next_invalid_schedules"
       /\ c = NextInvalid -> DueAt(Now)
    [] Bug = "next_exact_owner_schedules"
       /\ c = NextExactOwner -> DueAt(Now)
    [] Bug = "next_inactive_schedules"
       /\ c = NextInactive -> DueAt(Now)
    [] Bug = "next_hot_suppressed_schedules"
       /\ c = NextHotSuppressed -> DueAt(Now)
    [] Bug = "next_no_last_waits"
       /\ c = NextNoLast -> NoDue
    [] Bug = "next_before_cooldown_returns_now"
       /\ c = NextBeforeCooldown -> DueAt(Now)
    [] Bug = "next_boundary_waits"
       /\ c = NextBoundary -> DueAt(Now + 1)
    [] Bug = "next_future_last_clamps_now"
       /\ c = NextFutureLast -> DueAt(Now)
    [] OTHER -> SpecNext(c)

TickResult(
  progress,
  cursor_cleared,
  ready_rescue,
  deliver_broadcast,
  deliver_last_recorded,
  payload_repair,
  payload_broadcast,
  ready_broadcast
) ==
  [
    progress |-> progress,
    cursor_cleared |-> cursor_cleared,
    ready_rescue |-> ready_rescue,
    deliver_broadcast |-> deliver_broadcast,
    deliver_last_recorded |-> deliver_last_recorded,
    payload_repair |-> payload_repair,
    payload_broadcast |-> payload_broadcast,
    ready_broadcast |-> ready_broadcast
  ]

NoTickProgress ==
  TickResult(FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)

SpecTick(c) ==
  CASE c \in {TickObserver, TickDaDisabled, TickHotSuppressed, TickDeliverNotDue,
      TickDeliverBuilderMissing} -> NoTickProgress
    [] c = TickNoSessions ->
      TickResult(FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] c = TickRescueOnly ->
      TickResult(TRUE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] c = TickDeliverBroadcast ->
      TickResult(TRUE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE)
    [] c = TickRescueAndBroadcast ->
      TickResult(TRUE, FALSE, TRUE, TRUE, TRUE, FALSE, FALSE, FALSE)
    [] OTHER -> NoTickProgress

ActualTick(c) ==
  CASE Bug = "observer_rebroadcasts"
       /\ c = TickObserver ->
         TickResult(TRUE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE)
    [] Bug = "da_disabled_rebroadcasts"
       /\ c = TickDaDisabled ->
         TickResult(TRUE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE)
    [] Bug = "no_sessions_keeps_cursor"
       /\ c = TickNoSessions -> NoTickProgress
    [] Bug = "hot_suppressed_rescues"
       /\ c = TickHotSuppressed ->
         TickResult(TRUE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "hot_suppressed_broadcasts"
       /\ c = TickHotSuppressed ->
         TickResult(TRUE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE)
    [] Bug = "rescue_no_progress"
       /\ c = TickRescueOnly ->
         TickResult(FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "deliver_not_due_broadcasts"
       /\ c = TickDeliverNotDue ->
         TickResult(TRUE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE)
    [] Bug = "builder_missing_broadcasts"
       /\ c = TickDeliverBuilderMissing ->
         TickResult(TRUE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE)
    [] Bug = "deliver_broadcast_not_recorded"
       /\ c = TickDeliverBroadcast ->
         TickResult(TRUE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "deliver_broadcast_no_progress"
       /\ c = TickDeliverBroadcast ->
         TickResult(FALSE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE)
    [] Bug = "delivered_runs_payload_repair"
       /\ c = TickDeliverBroadcast ->
         TickResult(TRUE, FALSE, FALSE, TRUE, TRUE, TRUE, FALSE, FALSE)
    [] Bug = "delivered_runs_payload_broadcast"
       /\ c = TickDeliverBroadcast ->
         TickResult(TRUE, FALSE, FALSE, TRUE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "delivered_runs_ready_broadcast"
       /\ c = TickDeliverBroadcast ->
         TickResult(TRUE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, TRUE)
    [] OTHER -> SpecTick(c)

BugSet == {
  "none",
  "next_empty_schedules",
  "next_invalid_schedules",
  "next_exact_owner_schedules",
  "next_inactive_schedules",
  "next_hot_suppressed_schedules",
  "next_no_last_waits",
  "next_before_cooldown_returns_now",
  "next_boundary_waits",
  "next_future_last_clamps_now",
  "observer_rebroadcasts",
  "da_disabled_rebroadcasts",
  "no_sessions_keeps_cursor",
  "hot_suppressed_rescues",
  "hot_suppressed_broadcasts",
  "rescue_no_progress",
  "deliver_not_due_broadcasts",
  "builder_missing_broadcasts",
  "deliver_broadcast_not_recorded",
  "deliver_broadcast_no_progress",
  "delivered_runs_payload_repair",
  "delivered_runs_payload_broadcast",
  "delivered_runs_ready_broadcast"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0
  /\ \A c \in NextCases:
       LET r == ActualNext(c) IN
       /\ r.due \in BOOLEAN
       /\ r.time \in Nat
  /\ \A c \in TickCases:
       LET r == ActualTick(c) IN
       /\ r.progress \in BOOLEAN
       /\ r.cursor_cleared \in BOOLEAN
       /\ r.ready_rescue \in BOOLEAN
       /\ r.deliver_broadcast \in BOOLEAN
       /\ r.deliver_last_recorded \in BOOLEAN
       /\ r.payload_repair \in BOOLEAN
       /\ r.payload_broadcast \in BOOLEAN
       /\ r.ready_broadcast \in BOOLEAN

NextDueExact ==
  \A c \in NextCases:
    ActualNext(c) = SpecNext(c)

TickExact ==
  \A c \in TickCases:
    ActualTick(c) = SpecTick(c)

NextStable ==
  /\ ~ActualNext(NextEmpty).due
  /\ ~ActualNext(NextInvalid).due
  /\ ~ActualNext(NextExactOwner).due
  /\ ~ActualNext(NextInactive).due
  /\ ~ActualNext(NextHotSuppressed).due
  /\ ActualNext(NextNoLast) = DueAt(Now)
  /\ ActualNext(NextBeforeCooldown) = DueAt(25)
  /\ ActualNext(NextBoundary) = DueAt(Now)
  /\ ActualNext(NextFutureLast) = DueAt(40)

TickStable ==
  /\ ~ActualTick(TickObserver).progress
  /\ ~ActualTick(TickDaDisabled).progress
  /\ ActualTick(TickNoSessions).cursor_cleared
  /\ ~ActualTick(TickHotSuppressed).ready_rescue
  /\ ~ActualTick(TickHotSuppressed).deliver_broadcast
  /\ ActualTick(TickRescueOnly).progress
  /\ ActualTick(TickRescueOnly).ready_rescue
  /\ ~ActualTick(TickDeliverNotDue).deliver_broadcast
  /\ ~ActualTick(TickDeliverBuilderMissing).deliver_broadcast
  /\ ActualTick(TickDeliverBroadcast).deliver_broadcast
  /\ ActualTick(TickDeliverBroadcast).deliver_last_recorded
  /\ ActualTick(TickDeliverBroadcast).progress
  /\ ActualTick(TickRescueAndBroadcast).ready_rescue
  /\ ActualTick(TickRescueAndBroadcast).deliver_broadcast
  /\ ~ActualTick(TickDeliverBroadcast).payload_repair
  /\ ~ActualTick(TickDeliverBroadcast).payload_broadcast
  /\ ~ActualTick(TickDeliverBroadcast).ready_broadcast

RbcDeliveredRebroadcastCoreSafety ==
  /\ NextDueExact
  /\ TickExact
  /\ NextStable
  /\ TickStable

RbcDeliveredRebroadcastExactness == RbcDeliveredRebroadcastCoreSafety

RbcDeliveredRebroadcastCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcDeliveredRebroadcastExactness

SafetyFast ==
  RbcDeliveredRebroadcastExactness

====
