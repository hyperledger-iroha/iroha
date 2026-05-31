---- MODULE SumeragiActorGatePriorityGate ----

(***************************************************************************
A bounded abstract model for Sumeragi's `ActorGate` priority and fairness gate.

This slice models the gate policy in `ActorGate::can_enter`, plus the state
effects in `ActorGate::enter` and `ActorGuard::drop`. It deliberately abstracts
away the protected actor payload and condvar scheduling. The checked surface is
the consensus-facing ordering contract: only one actor may enter while the gate
is in flight; availability body work gets a bounded burst before availability
critical work; availability work yields after a bounded burst to urgent and
DA-critical waiters; urgent work yields to DA-critical and regular waiters after
their configured caps; and entry/drop side effects update waiting counters,
streaks, and wakeups consistently.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Str;
  priority,
  \* @type: Str;
  decision,
  \* @type: Bool;
  setInFlight,
  \* @type: Bool;
  clearInFlight,
  \* @type: Bool;
  decrementOwnWaiter,
  \* @type: Bool;
  notifyWaiters,
  \* @type: Str;
  availabilityStreakEffect,
  \* @type: Str;
  bodyStreakEffect,
  \* @type: Str;
  urgentStreakEffect

\* @type: <<Str, Str, Str, Bool, Bool, Bool, Bool, Str, Str, Str>>;
vars ==
  <<candidate,
    priority,
    decision,
    setInFlight,
    clearInFlight,
    decrementOwnWaiter,
    notifyWaiters,
    availabilityStreakEffect,
    bodyStreakEffect,
    urgentStreakEffect>>

Priorities == {
  "AvailabilityBody",
  "AvailabilityCritical",
  "Urgent",
  "DaCritical",
  "Regular",
  "None"
}

Decisions == {
  "Enter",
  "Block",
  "Drop"
}

Effects == {
  "IncCap",
  "Inc",
  "Reset",
  "Keep",
  "None"
}

Cases == {
  "idle_regular_enters",
  "inflight_blocks_all",
  "availability_body_first",
  "availability_body_defers_to_critical_after_body_cap",
  "availability_critical_waits_for_body_burst",
  "availability_critical_after_body_cap",
  "availability_burst_defers_to_urgent",
  "availability_burst_defers_to_da_critical",
  "urgent_waits_for_availability_burst",
  "urgent_after_availability_cap",
  "urgent_before_da_critical_cap",
  "urgent_defers_to_da_critical_after_cap",
  "da_critical_waits_for_availability_burst",
  "da_critical_waits_for_urgent_cap",
  "da_critical_after_urgent_cap",
  "regular_waits_for_availability",
  "regular_waits_for_da_critical",
  "regular_waits_for_urgent_until_cap",
  "regular_after_urgent_cap",
  "availability_body_entry_effects",
  "availability_critical_entry_effects",
  "urgent_entry_effects",
  "da_critical_entry_effects",
  "regular_entry_effects",
  "drop_urgent_keeps_urgent_streak",
  "drop_non_urgent_resets_urgent_streak"
}

EntryCases == {
  "idle_regular_enters",
  "availability_body_first",
  "availability_critical_after_body_cap",
  "urgent_after_availability_cap",
  "urgent_before_da_critical_cap",
  "da_critical_after_urgent_cap",
  "regular_after_urgent_cap",
  "availability_body_entry_effects",
  "availability_critical_entry_effects",
  "urgent_entry_effects",
  "da_critical_entry_effects",
  "regular_entry_effects"
}

BlockCases == {
  "inflight_blocks_all",
  "availability_body_defers_to_critical_after_body_cap",
  "availability_critical_waits_for_body_burst",
  "availability_burst_defers_to_urgent",
  "availability_burst_defers_to_da_critical",
  "urgent_waits_for_availability_burst",
  "urgent_defers_to_da_critical_after_cap",
  "da_critical_waits_for_availability_burst",
  "da_critical_waits_for_urgent_cap",
  "regular_waits_for_availability",
  "regular_waits_for_da_critical",
  "regular_waits_for_urgent_until_cap"
}

DropCases == {
  "drop_urgent_keeps_urgent_streak",
  "drop_non_urgent_resets_urgent_streak"
}

AvailabilityBodyEntryCases == {
  "availability_body_first",
  "availability_body_entry_effects"
}

AvailabilityCriticalEntryCases == {
  "availability_critical_after_body_cap",
  "availability_critical_entry_effects"
}

UrgentEntryCases == {
  "urgent_after_availability_cap",
  "urgent_before_da_critical_cap",
  "urgent_entry_effects"
}

DaCriticalEntryCases == {
  "da_critical_after_urgent_cap",
  "da_critical_entry_effects"
}

RegularEntryCases == {
  "idle_regular_enters",
  "regular_after_urgent_cap",
  "regular_entry_effects"
}

SpecPriority(c) ==
  CASE c \in AvailabilityBodyEntryCases -> "AvailabilityBody"
    [] c = "availability_body_defers_to_critical_after_body_cap" -> "AvailabilityBody"
    [] c \in AvailabilityCriticalEntryCases -> "AvailabilityCritical"
    [] c = "availability_critical_waits_for_body_burst" -> "AvailabilityCritical"
    [] c \in {"availability_burst_defers_to_urgent", "availability_burst_defers_to_da_critical"} -> "AvailabilityCritical"
    [] c \in UrgentEntryCases -> "Urgent"
    [] c \in {"urgent_waits_for_availability_burst", "urgent_defers_to_da_critical_after_cap"} -> "Urgent"
    [] c \in DaCriticalEntryCases -> "DaCritical"
    [] c \in {"da_critical_waits_for_availability_burst", "da_critical_waits_for_urgent_cap"} -> "DaCritical"
    [] c \in RegularEntryCases -> "Regular"
    [] c \in {"regular_waits_for_availability", "regular_waits_for_da_critical", "regular_waits_for_urgent_until_cap"} -> "Regular"
    [] c = "drop_urgent_keeps_urgent_streak" -> "Urgent"
    [] c = "drop_non_urgent_resets_urgent_streak" -> "Regular"
    [] OTHER -> "None"

SpecDecision(c) ==
  CASE c \in EntryCases -> "Enter"
    [] c \in BlockCases -> "Block"
    [] c \in DropCases -> "Drop"
    [] OTHER -> "Block"

SpecSetInFlight(c) ==
  c \in EntryCases

SpecClearInFlight(c) ==
  c \in DropCases

SpecDecrementOwnWaiter(c) ==
  c \in EntryCases

SpecNotifyWaiters(c) ==
  c \in DropCases

SpecAvailabilityStreakEffect(c) ==
  CASE c \in AvailabilityBodyEntryCases -> "IncCap"
    [] c \in AvailabilityCriticalEntryCases -> "IncCap"
    [] c \in UrgentEntryCases -> "Reset"
    [] c \in DaCriticalEntryCases -> "Reset"
    [] c \in RegularEntryCases -> "Reset"
    [] OTHER -> "Keep"

SpecBodyStreakEffect(c) ==
  CASE c \in AvailabilityBodyEntryCases -> "IncCap"
    [] c \in AvailabilityCriticalEntryCases -> "Reset"
    [] OTHER -> "Keep"

SpecUrgentStreakEffect(c) ==
  CASE c \in AvailabilityBodyEntryCases -> "Reset"
    [] c \in AvailabilityCriticalEntryCases -> "Reset"
    [] c \in UrgentEntryCases -> "Inc"
    [] c \in DaCriticalEntryCases -> "Reset"
    [] c \in RegularEntryCases -> "Reset"
    [] c = "drop_urgent_keeps_urgent_streak" -> "Keep"
    [] c = "drop_non_urgent_resets_urgent_streak" -> "Reset"
    [] OTHER -> "Keep"

ActualDecision(c) ==
  CASE c = "inflight_blocks_all" /\ Bug = "inflight_allows_entry" -> "Enter"
    [] c = "availability_body_defers_to_critical_after_body_cap" /\ Bug = "body_ignores_critical_cap" -> "Enter"
    [] c = "availability_critical_waits_for_body_burst" /\ Bug = "critical_skips_body_burst" -> "Enter"
    [] c = "availability_burst_defers_to_urgent" /\ Bug = "availability_ignores_urgent_cap" -> "Enter"
    [] c = "availability_burst_defers_to_da_critical" /\ Bug = "availability_ignores_da_cap" -> "Enter"
    [] c = "urgent_waits_for_availability_burst" /\ Bug = "urgent_skips_availability_burst" -> "Enter"
    [] c = "urgent_defers_to_da_critical_after_cap" /\ Bug = "urgent_starves_da_critical" -> "Enter"
    [] c = "da_critical_waits_for_availability_burst" /\ Bug = "da_critical_skips_availability_burst" -> "Enter"
    [] c = "da_critical_waits_for_urgent_cap" /\ Bug = "da_critical_skips_urgent_cap" -> "Enter"
    [] c = "regular_waits_for_availability" /\ Bug = "regular_skips_availability" -> "Enter"
    [] c = "regular_waits_for_da_critical" /\ Bug = "regular_skips_da_critical" -> "Enter"
    [] c = "regular_waits_for_urgent_until_cap" /\ Bug = "regular_skips_urgent_cap" -> "Enter"
    [] OTHER -> SpecDecision(c)

ActualSetInFlight(c) ==
  CASE c \in EntryCases /\ Bug = "entry_does_not_set_inflight" -> FALSE
    [] OTHER -> SpecSetInFlight(c)

ActualClearInFlight(c) ==
  CASE c \in DropCases /\ Bug = "drop_keeps_inflight" -> FALSE
    [] OTHER -> SpecClearInFlight(c)

ActualDecrementOwnWaiter(c) ==
  CASE c \in EntryCases /\ Bug = "entry_does_not_decrement_waiter" -> FALSE
    [] OTHER -> SpecDecrementOwnWaiter(c)

ActualNotifyWaiters(c) ==
  CASE c \in DropCases /\ Bug = "drop_skips_notify" -> FALSE
    [] OTHER -> SpecNotifyWaiters(c)

ActualAvailabilityStreakEffect(c) ==
  CASE c \in AvailabilityBodyEntryCases /\ Bug = "body_does_not_increment_availability_streak" -> "Keep"
    [] c \in AvailabilityCriticalEntryCases /\ Bug = "critical_does_not_increment_availability_streak" -> "Keep"
    [] c \in UrgentEntryCases /\ Bug = "urgent_does_not_reset_availability_streak" -> "Keep"
    [] OTHER -> SpecAvailabilityStreakEffect(c)

ActualBodyStreakEffect(c) ==
  CASE c \in AvailabilityBodyEntryCases /\ Bug = "body_does_not_increment_body_streak" -> "Keep"
    [] c \in AvailabilityCriticalEntryCases /\ Bug = "critical_does_not_reset_body_streak" -> "Keep"
    [] OTHER -> SpecBodyStreakEffect(c)

ActualUrgentStreakEffect(c) ==
  CASE c \in UrgentEntryCases /\ Bug = "urgent_does_not_increment_urgent_streak" -> "Keep"
    [] c \in DaCriticalEntryCases /\ Bug = "da_critical_keeps_urgent_streak" -> "Keep"
    [] c \in RegularEntryCases /\ Bug = "regular_keeps_urgent_streak" -> "Keep"
    [] c = "drop_urgent_keeps_urgent_streak" /\ Bug = "drop_urgent_resets_urgent_streak" -> "Reset"
    [] c = "drop_non_urgent_resets_urgent_streak" /\ Bug = "drop_non_urgent_keeps_urgent_streak" -> "Keep"
    [] OTHER -> SpecUrgentStreakEffect(c)

BugModes == {
  "none",
  "inflight_allows_entry",
  "body_ignores_critical_cap",
  "critical_skips_body_burst",
  "availability_ignores_urgent_cap",
  "availability_ignores_da_cap",
  "urgent_skips_availability_burst",
  "urgent_starves_da_critical",
  "da_critical_skips_availability_burst",
  "da_critical_skips_urgent_cap",
  "regular_skips_availability",
  "regular_skips_da_critical",
  "regular_skips_urgent_cap",
  "entry_does_not_set_inflight",
  "entry_does_not_decrement_waiter",
  "body_does_not_increment_body_streak",
  "body_does_not_increment_availability_streak",
  "critical_does_not_reset_body_streak",
  "critical_does_not_increment_availability_streak",
  "urgent_does_not_increment_urgent_streak",
  "urgent_does_not_reset_availability_streak",
  "da_critical_keeps_urgent_streak",
  "regular_keeps_urgent_streak",
  "drop_keeps_inflight",
  "drop_urgent_resets_urgent_streak",
  "drop_non_urgent_keeps_urgent_streak",
  "drop_skips_notify"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases
  /\ priority \in Priorities
  /\ decision \in Decisions
  /\ setInFlight \in BOOLEAN
  /\ clearInFlight \in BOOLEAN
  /\ decrementOwnWaiter \in BOOLEAN
  /\ notifyWaiters \in BOOLEAN
  /\ availabilityStreakEffect \in Effects
  /\ bodyStreakEffect \in Effects
  /\ urgentStreakEffect \in Effects

Init ==
  /\ candidate = "idle_regular_enters"
  /\ priority = "Regular"
  /\ decision = "Enter"
  /\ setInFlight = TRUE
  /\ clearInFlight = FALSE
  /\ decrementOwnWaiter = TRUE
  /\ notifyWaiters = FALSE
  /\ availabilityStreakEffect = "Reset"
  /\ bodyStreakEffect = "Keep"
  /\ urgentStreakEffect = "Reset"

Apply(c) ==
  /\ candidate' = c
  /\ priority' = SpecPriority(c)
  /\ decision' = ActualDecision(c)
  /\ setInFlight' = ActualSetInFlight(c)
  /\ clearInFlight' = ActualClearInFlight(c)
  /\ decrementOwnWaiter' = ActualDecrementOwnWaiter(c)
  /\ notifyWaiters' = ActualNotifyWaiters(c)
  /\ availabilityStreakEffect' = ActualAvailabilityStreakEffect(c)
  /\ bodyStreakEffect' = ActualBodyStreakEffect(c)
  /\ urgentStreakEffect' = ActualUrgentStreakEffect(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

MatchesSpec ==
  /\ priority = SpecPriority(candidate)
  /\ decision = SpecDecision(candidate)
  /\ setInFlight = SpecSetInFlight(candidate)
  /\ clearInFlight = SpecClearInFlight(candidate)
  /\ decrementOwnWaiter = SpecDecrementOwnWaiter(candidate)
  /\ notifyWaiters = SpecNotifyWaiters(candidate)
  /\ availabilityStreakEffect = SpecAvailabilityStreakEffect(candidate)
  /\ bodyStreakEffect = SpecBodyStreakEffect(candidate)
  /\ urgentStreakEffect = SpecUrgentStreakEffect(candidate)

InFlightBlocksAll ==
  candidate = "inflight_blocks_all" =>
    /\ decision = "Block"
    /\ ~setInFlight
    /\ ~decrementOwnWaiter

AvailabilityBodyYieldsToCriticalAfterBodyCap ==
  candidate = "availability_body_defers_to_critical_after_body_cap" =>
    decision = "Block"

AvailabilityCriticalHonorsBodyBurst ==
  /\ candidate = "availability_critical_waits_for_body_burst" => decision = "Block"
  /\ candidate = "availability_critical_after_body_cap" => decision = "Enter"

AvailabilityBurstYieldsToUrgentAndDa ==
  candidate \in {"availability_burst_defers_to_urgent", "availability_burst_defers_to_da_critical"} =>
    decision = "Block"

UrgentHonorsAvailabilityAndDaCaps ==
  /\ candidate = "urgent_waits_for_availability_burst" => decision = "Block"
  /\ candidate = "urgent_after_availability_cap" => decision = "Enter"
  /\ candidate = "urgent_before_da_critical_cap" => decision = "Enter"
  /\ candidate = "urgent_defers_to_da_critical_after_cap" => decision = "Block"

DaCriticalHonorsAvailabilityAndUrgentCaps ==
  /\ candidate = "da_critical_waits_for_availability_burst" => decision = "Block"
  /\ candidate = "da_critical_waits_for_urgent_cap" => decision = "Block"
  /\ candidate = "da_critical_after_urgent_cap" => decision = "Enter"

RegularHonorsAvailabilityDaAndUrgentCaps ==
  /\ candidate = "regular_waits_for_availability" => decision = "Block"
  /\ candidate = "regular_waits_for_da_critical" => decision = "Block"
  /\ candidate = "regular_waits_for_urgent_until_cap" => decision = "Block"
  /\ candidate = "regular_after_urgent_cap" => decision = "Enter"
  /\ candidate = "idle_regular_enters" => decision = "Enter"

EntrySetsOwnershipAndConsumesWaiter ==
  decision = "Enter" =>
    /\ setInFlight
    /\ decrementOwnWaiter
    /\ ~clearInFlight
    /\ ~notifyWaiters

BlockedEntriesHaveNoSideEffects ==
  decision = "Block" =>
    /\ ~setInFlight
    /\ ~clearInFlight
    /\ ~decrementOwnWaiter
    /\ ~notifyWaiters

AvailabilityEntryStreakEffects ==
  /\ candidate \in AvailabilityBodyEntryCases =>
       /\ availabilityStreakEffect = "IncCap"
       /\ bodyStreakEffect = "IncCap"
       /\ urgentStreakEffect = "Reset"
  /\ candidate \in AvailabilityCriticalEntryCases =>
       /\ availabilityStreakEffect = "IncCap"
       /\ bodyStreakEffect = "Reset"
       /\ urgentStreakEffect = "Reset"

UrgentEntryStreakEffects ==
  candidate \in UrgentEntryCases =>
    /\ urgentStreakEffect = "Inc"
    /\ availabilityStreakEffect = "Reset"

DaCriticalAndRegularEntryResetStreaks ==
  candidate \in (DaCriticalEntryCases \cup RegularEntryCases) =>
    /\ urgentStreakEffect = "Reset"
    /\ availabilityStreakEffect = "Reset"

DropClearsAndNotifies ==
  decision = "Drop" =>
    /\ clearInFlight
    /\ notifyWaiters
    /\ ~setInFlight
    /\ ~decrementOwnWaiter

UrgentDropPreservesUrgentStreak ==
  candidate = "drop_urgent_keeps_urgent_streak" =>
    urgentStreakEffect = "Keep"

NonUrgentDropResetsUrgentStreak ==
  candidate = "drop_non_urgent_resets_urgent_streak" =>
    urgentStreakEffect = "Reset"

Safety ==
  /\ MatchesSpec
  /\ InFlightBlocksAll
  /\ AvailabilityBodyYieldsToCriticalAfterBodyCap
  /\ AvailabilityCriticalHonorsBodyBurst
  /\ AvailabilityBurstYieldsToUrgentAndDa
  /\ UrgentHonorsAvailabilityAndDaCaps
  /\ DaCriticalHonorsAvailabilityAndUrgentCaps
  /\ RegularHonorsAvailabilityDaAndUrgentCaps
  /\ EntrySetsOwnershipAndConsumesWaiter
  /\ BlockedEntriesHaveNoSideEffects
  /\ AvailabilityEntryStreakEffects
  /\ UrgentEntryStreakEffects
  /\ DaCriticalAndRegularEntryResetStreaks
  /\ DropClearsAndNotifies
  /\ UrgentDropPreservesUrgentStreak
  /\ NonUrgentDropResetsUrgentStreak

=============================================================================
