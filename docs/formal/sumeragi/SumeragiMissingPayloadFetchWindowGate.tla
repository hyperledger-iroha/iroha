---- MODULE SumeragiMissingPayloadFetchWindowGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for same-height missing-payload fetch window helpers.

This slice covers `missing_payload_fetch_window_snapshot(...)`,
`missing_payload_fetch_window_already_emitted(...)`,
`mark_missing_payload_fetch_window_emitted(...)`,
`clear_missing_payload_fetch_window_gate_for_height(...)`,
`clear_missing_payload_fetch_window_gate_for_block(...)`, and
`effective_hash_miss_escalation_cap(...)` from
`crates/iroha_core/src/sumeragi/main_loop.rs`.

It preserves the deterministic recovery contract:
- payload-fetch windows exist only for the active round height, active
  same-height missing-QC stall mode, and unresolved payload dependencies,
- fresh gates capture the current stall window and have no targeted-fetch mark,
- repeated same-window snapshots preserve the gate, older stall windows reset
  stale marks, and newer stall windows advance without claiming the new window
  was already emitted,
- emission marks are exact-height/block/window updates and clear helpers remove
  only their exact scope, and
- lock-lag hash-miss escalation caps widen deterministically near/far from the
  catch-up frontier while staying clamped by the base/attempt cap.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

WindowCases == {
  "wrong_height_clears",
  "stall_absent_clears",
  "stall_wrong_height_clears",
  "stall_inactive_clears",
  "dependency_absent_clears",
  "fresh_gate_initializes",
  "same_window_preserves",
  "older_stall_resets",
  "newer_stall_advances"
}

SnapshotNone == 1
SnapshotSome == 2
RemoveGate == 3
CreateGate == 4
SnapshotHeight == 5
SnapshotHash == 6
EnteredNow == 7
LastWindowNow == 8
WindowFromStall == 9
LastFetchNone == 10
PreserveEntered == 11
PreserveLastWindow == 12
PreserveWindow == 13
PreserveLastFetch == 14
ClearTargetWindow == 15

SpecWindowActions(c) ==
  CASE c \in {
       "wrong_height_clears",
       "stall_absent_clears",
       "stall_wrong_height_clears",
       "stall_inactive_clears",
       "dependency_absent_clears"
     } ->
      {SnapshotNone, RemoveGate}
    [] c = "fresh_gate_initializes" ->
      {SnapshotSome, CreateGate, SnapshotHeight, SnapshotHash, EnteredNow,
       LastWindowNow, WindowFromStall, LastFetchNone}
    [] c = "same_window_preserves" ->
      {SnapshotSome, SnapshotHeight, SnapshotHash, PreserveEntered,
       PreserveLastWindow, PreserveWindow, PreserveLastFetch}
    [] c = "older_stall_resets" ->
      {SnapshotSome, SnapshotHeight, SnapshotHash, EnteredNow, LastWindowNow,
       WindowFromStall, ClearTargetWindow, LastFetchNone}
    [] c = "newer_stall_advances" ->
      {SnapshotSome, SnapshotHeight, SnapshotHash, PreserveEntered,
       LastWindowNow, WindowFromStall, PreserveLastFetch}
    [] OTHER -> {}

ActualWindowActions(c) ==
  LET spec == SpecWindowActions(c) IN
  CASE Bug = "snapshot_wrong_height_retained"
       /\ c = "wrong_height_clears" ->
      {SnapshotSome, PreserveEntered, PreserveWindow}
    [] Bug = "snapshot_inactive_stall_keeps_gate"
       /\ c = "stall_inactive_clears" ->
      {SnapshotSome, PreserveEntered, PreserveWindow}
    [] Bug = "snapshot_no_dependency_keeps_gate"
       /\ c = "dependency_absent_clears" ->
      {SnapshotSome, PreserveEntered, PreserveWindow}
    [] Bug = "fresh_missing_create"
       /\ c = "fresh_gate_initializes" ->
      spec \ {CreateGate}
    [] Bug = "fresh_wrong_window"
       /\ c = "fresh_gate_initializes" ->
      spec \ {WindowFromStall}
    [] Bug = "fresh_missing_entered"
       /\ c = "fresh_gate_initializes" ->
      spec \ {EnteredNow}
    [] Bug = "same_window_resets_entered"
       /\ c = "same_window_preserves" ->
      (spec \ {PreserveEntered}) \cup {EnteredNow}
    [] Bug = "older_window_preserves_fetch"
       /\ c = "older_stall_resets" ->
      (spec \ {ClearTargetWindow, LastFetchNone}) \cup {PreserveLastFetch}
    [] Bug = "newer_window_clears_fetch"
       /\ c = "newer_stall_advances" ->
      (spec \ {PreserveLastFetch}) \cup {ClearTargetWindow, LastFetchNone}
    [] OTHER -> spec

EmitCases == {
  "already_absent_false",
  "already_exact_true",
  "already_previous_false",
  "mark_absent_noop",
  "mark_existing_sets_window",
  "clear_height_exact_removes",
  "clear_height_other_retains",
  "clear_block_exact_removes",
  "clear_block_other_retains"
}

AlreadyFalse == 21
AlreadyTrue == 22
NoCreate == 23
MarkWindow == 24
MarkTime == 25
RetainGate == 26

SpecEmitActions(c) ==
  CASE c \in {"already_absent_false", "already_previous_false"} ->
      {AlreadyFalse}
    [] c = "already_exact_true" ->
      {AlreadyTrue}
    [] c = "mark_absent_noop" ->
      {NoCreate, AlreadyFalse}
    [] c = "mark_existing_sets_window" ->
      {MarkWindow, MarkTime, AlreadyTrue}
    [] c \in {"clear_height_exact_removes", "clear_block_exact_removes"} ->
      {RemoveGate, AlreadyFalse}
    [] c \in {"clear_height_other_retains", "clear_block_other_retains"} ->
      {RetainGate, AlreadyTrue}
    [] OTHER -> {}

ActualEmitActions(c) ==
  LET spec == SpecEmitActions(c) IN
  CASE Bug = "already_absent_true"
       /\ c = "already_absent_false" ->
      {AlreadyTrue}
    [] Bug = "already_previous_true"
       /\ c = "already_previous_false" ->
      {AlreadyTrue}
    [] Bug = "mark_absent_creates"
       /\ c = "mark_absent_noop" ->
      {CreateGate, MarkWindow, MarkTime, AlreadyTrue}
    [] Bug = "mark_existing_wrong_window"
       /\ c = "mark_existing_sets_window" ->
      spec \ {MarkWindow, AlreadyTrue}
    [] Bug = "clear_height_drops_other"
       /\ c = "clear_height_other_retains" ->
      {RemoveGate, AlreadyFalse}
    [] Bug = "clear_height_keeps_exact"
       /\ c = "clear_height_exact_removes" ->
      {RetainGate, AlreadyTrue}
    [] Bug = "clear_block_drops_other"
       /\ c = "clear_block_other_retains" ->
      {RemoveGate, AlreadyFalse}
    [] Bug = "clear_block_keeps_exact"
       /\ c = "clear_block_exact_removes" ->
      {RetainGate, AlreadyTrue}
    [] OTHER -> spec

CapCases == {
  "cap_no_lock_lag",
  "cap_no_frontier",
  "cap_near",
  "cap_near_ceil_dominates",
  "cap_far",
  "cap_far_flag_near_height",
  "cap_clamped",
  "cap_attempt_below_base"
}

CeilDiv(a, b) == (a + b - 1) \div b

Max(a, b) == IF a >= b THEN a ELSE b

Min(a, b) == IF a <= b THEN a ELSE b

BaseCap(c) ==
  CASE c = "cap_near_ceil_dominates" -> 1
    [] c = "cap_far" -> 1
    [] c = "cap_far_flag_near_height" -> 1
    [] c = "cap_clamped" -> 5
    [] c = "cap_attempt_below_base" -> 5
    [] OTHER -> 2

AttemptCap(c) ==
  CASE c = "cap_near" -> 3
    [] c = "cap_near_ceil_dominates" -> 10
    [] c = "cap_far" -> 12
    [] c = "cap_far_flag_near_height" -> 12
    [] c = "cap_clamped" -> 6
    [] c = "cap_attempt_below_base" -> 2
    [] OTHER -> 10

LockLagActive(c) ==
  c # "cap_no_lock_lag"

FrontierPresent(c) ==
  c # "cap_no_frontier"

FarFuture(c) ==
  c \in {"cap_far", "cap_far_flag_near_height"}

Height(c) ==
  CASE c = "cap_far" -> 14
    [] c = "cap_far_flag_near_height" -> 11
    [] OTHER -> 11

Frontier(c) == 10

NearCap(c) ==
  Max(BaseCap(c) + 2, CeilDiv(AttemptCap(c), 3))

MaxCap(c) ==
  Max(AttemptCap(c), BaseCap(c))

SpecCap(c) ==
  IF ~LockLagActive(c) \/ ~FrontierPresent(c) THEN
    BaseCap(c)
  ELSE
    LET selected ==
      IF FarFuture(c) /\ Height(c) > Frontier(c) + 1 THEN
        Max(NearCap(c), CeilDiv(AttemptCap(c), 2))
      ELSE
        NearCap(c)
    IN Min(selected, MaxCap(c))

ActualCap(c) ==
  CASE Bug = "cap_lock_lag_ignored"
       /\ c = "cap_near" ->
      BaseCap(c)
    [] Bug = "cap_no_frontier_widens"
       /\ c = "cap_no_frontier" ->
      Min(NearCap(c), MaxCap(c))
    [] Bug = "cap_near_omits_plus_two"
       /\ c = "cap_near" ->
      Min(Max(BaseCap(c), CeilDiv(AttemptCap(c), 3)), MaxCap(c))
    [] Bug = "cap_near_floor_div"
       /\ c = "cap_near_ceil_dominates" ->
      Min(Max(BaseCap(c) + 2, AttemptCap(c) \div 3), MaxCap(c))
    [] Bug = "cap_far_omits_half"
       /\ c = "cap_far" ->
      Min(NearCap(c), MaxCap(c))
    [] Bug = "cap_far_ignores_height"
       /\ c = "cap_far_flag_near_height" ->
      Min(Max(NearCap(c), CeilDiv(AttemptCap(c), 2)), MaxCap(c))
    [] Bug = "cap_skips_max_clamp"
       /\ c = "cap_clamped" ->
      NearCap(c)
    [] OTHER -> SpecCap(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

BugModes == {
  "none",
  "snapshot_wrong_height_retained",
  "snapshot_inactive_stall_keeps_gate",
  "snapshot_no_dependency_keeps_gate",
  "fresh_missing_create",
  "fresh_wrong_window",
  "fresh_missing_entered",
  "same_window_resets_entered",
  "older_window_preserves_fetch",
  "newer_window_clears_fetch",
  "already_absent_true",
  "already_previous_true",
  "mark_absent_creates",
  "mark_existing_wrong_window",
  "clear_height_drops_other",
  "clear_height_keeps_exact",
  "clear_block_drops_other",
  "clear_block_keeps_exact",
  "cap_lock_lag_ignored",
  "cap_no_frontier_widens",
  "cap_near_omits_plus_two",
  "cap_near_floor_div",
  "cap_far_omits_half",
  "cap_far_ignores_height",
  "cap_skips_max_clamp"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ checked = 0

SafetyFast ==
  /\ \A c \in WindowCases:
       ActualWindowActions(c) = SpecWindowActions(c)
  /\ \A c \in EmitCases:
       ActualEmitActions(c) = SpecEmitActions(c)
  /\ \A c \in CapCases:
       ActualCap(c) = SpecCap(c)

====
