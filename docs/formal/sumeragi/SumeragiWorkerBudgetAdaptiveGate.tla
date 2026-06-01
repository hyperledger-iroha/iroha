---- MODULE SumeragiWorkerBudgetAdaptiveGate ----

(***************************************************************************
A bounded abstract model for Sumeragi worker-loop budget and adaptive caps.

This slice models the deterministic budget/cap helpers used before the worker
drain scheduler makes a tier choice: `worker_time_budget`,
`vote_rx_drain_budget`, `cap_*_drain_budget`, `idle_tick_gap`,
`busy_tick_gap`, `block_backlog_drain_cap`, and `apply_adaptive_drain_caps`.
It abstracts concrete durations and queue depths into representative boundary
cases. The checked contract is that drain budgets remain anchored to the
consensus cadence, vote draining may use DA quorum windows but stays capped,
tick gaps stay within floor/max bounds, block backlog depth maps to stable
tier caps, and adaptive queue caps throttle payload/RBC/block work without
accidentally starving repair traffic or changing idle configurations.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Str;
  timeBudget,
  \* @type: Str;
  voteBudget,
  \* @type: Str;
  drainBudget,
  \* @type: Str;
  idleGap,
  \* @type: Str;
  busyGap,
  \* @type: Str;
  blockBacklogCap,
  \* @type: Str;
  blockCap,
  \* @type: Str;
  payloadCap,
  \* @type: Str;
  rbcCap

\* @type: <<Str, Str, Str, Str, Str, Str, Str, Str, Str, Str>>;
vars ==
  <<candidate,
    timeBudget,
    voteBudget,
    drainBudget,
    idleGap,
    busyGap,
    blockBacklogCap,
    blockCap,
    payloadCap,
    rbcCap>>

TimeBudgetResults == {
  "None",
  "Floor",
  "QuarterWindow",
  "GlobalCap",
  "ConfigCap",
  "UnaffectedByDaMultiplier",
  "DaMultiplierScaled"
}

VoteBudgetResults == {
  "None",
  "DaQuorumWindow",
  "DaMultiplierWindow",
  "MaxBudgetCap",
  "ConfigCap",
  "Floor",
  "CommitOnly"
}

DrainBudgetResults == {
  "None",
  "Floor",
  "Raw",
  "GlobalCap",
  "ConfigCap"
}

GapResults == {
  "None",
  "Floor",
  "QuarterWindow",
  "MaxGap",
  "BusyFloor",
  "IdleCap",
  "OverIdle"
}

BacklogCaps == {
  "None",
  "Zero",
  "Small",
  "Medium",
  "Large",
  "Huge"
}

AdaptiveCaps == {
  "None",
  "Preserve",
  "VoteReduced",
  "BlockMin",
  "BlockScaled",
  "Changed",
  "RbcReduced"
}

Cases == {
  "worker_zero_window_floor",
  "worker_small_window_floor",
  "worker_mid_window_quarter",
  "worker_large_window_cap",
  "worker_config_cap",
  "worker_da_multiplier_ignored",
  "vote_da_quorum_window",
  "vote_da_multiplier_window",
  "vote_max_budget_cap",
  "vote_config_cap",
  "vote_zero_floor",
  "drain_floor",
  "drain_global_cap",
  "vote_drain_floor",
  "rbc_config_cap",
  "idle_gap_floor",
  "idle_gap_max",
  "busy_gap_floor",
  "busy_gap_idle_cap",
  "block_depth_zero",
  "block_depth_small",
  "block_depth_medium",
  "block_depth_large",
  "block_depth_huge",
  "vote_backlog_payload_reduced",
  "vote_backlog_rbc_preserved",
  "no_backlog_preserves_caps",
  "block_backlog_block_cap",
  "block_backlog_payload_min",
  "block_backlog_payload_scaled",
  "block_backlog_rbc_scaled"
}

SpecTimeBudget(c) ==
  CASE c = "worker_zero_window_floor" -> "Floor"
    [] c = "worker_small_window_floor" -> "Floor"
    [] c = "worker_mid_window_quarter" -> "QuarterWindow"
    [] c = "worker_large_window_cap" -> "GlobalCap"
    [] c = "worker_config_cap" -> "ConfigCap"
    [] c = "worker_da_multiplier_ignored" -> "UnaffectedByDaMultiplier"
    [] OTHER -> "None"

SpecVoteBudget(c) ==
  CASE c = "vote_da_quorum_window" -> "DaQuorumWindow"
    [] c = "vote_da_multiplier_window" -> "DaMultiplierWindow"
    [] c = "vote_max_budget_cap" -> "MaxBudgetCap"
    [] c = "vote_config_cap" -> "ConfigCap"
    [] c = "vote_zero_floor" -> "Floor"
    [] OTHER -> "None"

SpecDrainBudget(c) ==
  CASE c = "drain_floor" -> "Floor"
    [] c = "drain_global_cap" -> "GlobalCap"
    [] c = "vote_drain_floor" -> "Floor"
    [] c = "rbc_config_cap" -> "ConfigCap"
    [] OTHER -> "None"

SpecIdleGap(c) ==
  CASE c = "idle_gap_floor" -> "Floor"
    [] c = "idle_gap_max" -> "MaxGap"
    [] OTHER -> "None"

SpecBusyGap(c) ==
  CASE c = "busy_gap_floor" -> "BusyFloor"
    [] c = "busy_gap_idle_cap" -> "IdleCap"
    [] OTHER -> "None"

SpecBlockBacklogCap(c) ==
  CASE c = "block_depth_zero" -> "Zero"
    [] c = "block_depth_small" -> "Small"
    [] c = "block_depth_medium" -> "Medium"
    [] c = "block_depth_large" -> "Large"
    [] c = "block_depth_huge" -> "Huge"
    [] OTHER -> "None"

SpecBlockCap(c) ==
  CASE c = "block_backlog_block_cap" -> "Large"
    [] c = "no_backlog_preserves_caps" -> "Preserve"
    [] OTHER -> "None"

SpecPayloadCap(c) ==
  CASE c = "vote_backlog_payload_reduced" -> "VoteReduced"
    [] c = "no_backlog_preserves_caps" -> "Preserve"
    [] c = "block_backlog_payload_min" -> "BlockMin"
    [] c = "block_backlog_payload_scaled" -> "BlockScaled"
    [] OTHER -> "None"

SpecRbcCap(c) ==
  CASE c = "vote_backlog_rbc_preserved" -> "Preserve"
    [] c = "no_backlog_preserves_caps" -> "Preserve"
    [] c = "block_backlog_payload_min" -> "BlockMin"
    [] c = "block_backlog_rbc_scaled" -> "BlockScaled"
    [] OTHER -> "None"

ActualTimeBudget(c) ==
  CASE c = "worker_zero_window_floor" /\ Bug = "worker_zero_window_not_floored" -> "QuarterWindow"
    [] c = "worker_small_window_floor" /\ Bug = "worker_small_window_not_floored" -> "QuarterWindow"
    [] c = "worker_mid_window_quarter" /\ Bug = "worker_mid_window_not_quartered" -> "Floor"
    [] c = "worker_large_window_cap" /\ Bug = "worker_large_window_ignores_global_cap" -> "QuarterWindow"
    [] c = "worker_config_cap" /\ Bug = "worker_config_cap_ignored" -> "GlobalCap"
    [] c = "worker_da_multiplier_ignored" /\ Bug = "worker_uses_da_multiplier" -> "DaMultiplierScaled"
    [] OTHER -> SpecTimeBudget(c)

ActualVoteBudget(c) ==
  CASE c = "vote_da_quorum_window" /\ Bug = "vote_da_window_uses_commit_only" -> "CommitOnly"
    [] c = "vote_da_multiplier_window" /\ Bug = "vote_da_multiplier_ignored" -> "DaQuorumWindow"
    [] c = "vote_max_budget_cap" /\ Bug = "vote_max_budget_ignored" -> "DaQuorumWindow"
    [] c = "vote_config_cap" /\ Bug = "vote_config_cap_ignored" -> "DaQuorumWindow"
    [] c = "vote_zero_floor" /\ Bug = "vote_zero_not_floored" -> "None"
    [] OTHER -> SpecVoteBudget(c)

ActualDrainBudget(c) ==
  CASE c = "drain_floor" /\ Bug = "drain_floor_ignored" -> "Raw"
    [] c = "drain_global_cap" /\ Bug = "drain_global_cap_ignored" -> "Raw"
    [] c = "vote_drain_floor" /\ Bug = "vote_drain_floor_ignored" -> "Raw"
    [] c = "rbc_config_cap" /\ Bug = "rbc_cap_ignored" -> "GlobalCap"
    [] OTHER -> SpecDrainBudget(c)

ActualIdleGap(c) ==
  CASE c = "idle_gap_floor" /\ Bug = "idle_gap_floor_ignored" -> "QuarterWindow"
    [] c = "idle_gap_max" /\ Bug = "idle_gap_max_ignored" -> "QuarterWindow"
    [] OTHER -> SpecIdleGap(c)

ActualBusyGap(c) ==
  CASE c = "busy_gap_floor" /\ Bug = "busy_gap_floor_ignored" -> "QuarterWindow"
    [] c = "busy_gap_idle_cap" /\ Bug = "busy_gap_exceeds_idle" -> "OverIdle"
    [] OTHER -> SpecBusyGap(c)

ActualBlockBacklogCap(c) ==
  CASE c = "block_depth_zero" /\ Bug = "block_zero_cap_nonzero" -> "Small"
    [] c = "block_depth_small" /\ Bug = "block_small_uses_medium" -> "Medium"
    [] c = "block_depth_medium" /\ Bug = "block_medium_uses_large" -> "Large"
    [] c = "block_depth_large" /\ Bug = "block_large_uses_huge" -> "Huge"
    [] c = "block_depth_huge" /\ Bug = "block_huge_clamped_large" -> "Large"
    [] OTHER -> SpecBlockBacklogCap(c)

ActualBlockCap(c) ==
  CASE c = "block_backlog_block_cap" /\ Bug = "block_backlog_does_not_cap_blocks" -> "Huge"
    [] c = "no_backlog_preserves_caps" /\ Bug = "no_backlog_changes_caps" -> "Changed"
    [] OTHER -> SpecBlockCap(c)

ActualPayloadCap(c) ==
  CASE c = "vote_backlog_payload_reduced" /\ Bug = "vote_backlog_does_not_reduce_payload" -> "Preserve"
    [] c = "no_backlog_preserves_caps" /\ Bug = "no_backlog_changes_caps" -> "Changed"
    [] c = "block_backlog_payload_min" /\ Bug = "block_backlog_payload_below_min" -> "VoteReduced"
    [] c = "block_backlog_payload_scaled" /\ Bug = "block_backlog_payload_not_scaled" -> "Preserve"
    [] OTHER -> SpecPayloadCap(c)

ActualRbcCap(c) ==
  CASE c = "vote_backlog_rbc_preserved" /\ Bug = "vote_backlog_reduces_rbc" -> "RbcReduced"
    [] c = "no_backlog_preserves_caps" /\ Bug = "no_backlog_changes_caps" -> "Changed"
    [] c = "block_backlog_payload_min" /\ Bug = "block_backlog_rbc_below_min" -> "RbcReduced"
    [] c = "block_backlog_rbc_scaled" /\ Bug = "block_backlog_rbc_not_scaled" -> "Preserve"
    [] OTHER -> SpecRbcCap(c)

BugModes == {
  "none",
  "worker_zero_window_not_floored",
  "worker_small_window_not_floored",
  "worker_mid_window_not_quartered",
  "worker_large_window_ignores_global_cap",
  "worker_config_cap_ignored",
  "worker_uses_da_multiplier",
  "vote_da_window_uses_commit_only",
  "vote_da_multiplier_ignored",
  "vote_max_budget_ignored",
  "vote_config_cap_ignored",
  "vote_zero_not_floored",
  "drain_floor_ignored",
  "drain_global_cap_ignored",
  "vote_drain_floor_ignored",
  "rbc_cap_ignored",
  "idle_gap_floor_ignored",
  "idle_gap_max_ignored",
  "busy_gap_floor_ignored",
  "busy_gap_exceeds_idle",
  "block_zero_cap_nonzero",
  "block_small_uses_medium",
  "block_medium_uses_large",
  "block_large_uses_huge",
  "block_huge_clamped_large",
  "vote_backlog_does_not_reduce_payload",
  "vote_backlog_reduces_rbc",
  "no_backlog_changes_caps",
  "block_backlog_does_not_cap_blocks",
  "block_backlog_payload_below_min",
  "block_backlog_payload_not_scaled",
  "block_backlog_rbc_below_min",
  "block_backlog_rbc_not_scaled"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases
  /\ timeBudget \in TimeBudgetResults
  /\ voteBudget \in VoteBudgetResults
  /\ drainBudget \in DrainBudgetResults
  /\ idleGap \in GapResults
  /\ busyGap \in GapResults
  /\ blockBacklogCap \in BacklogCaps
  /\ blockCap \in (BacklogCaps \cup AdaptiveCaps)
  /\ payloadCap \in AdaptiveCaps
  /\ rbcCap \in AdaptiveCaps

Init ==
  /\ candidate = "worker_mid_window_quarter"
  /\ timeBudget = "QuarterWindow"
  /\ voteBudget = "None"
  /\ drainBudget = "None"
  /\ idleGap = "None"
  /\ busyGap = "None"
  /\ blockBacklogCap = "None"
  /\ blockCap = "None"
  /\ payloadCap = "None"
  /\ rbcCap = "None"

Apply(c) ==
  /\ candidate' = c
  /\ timeBudget' = ActualTimeBudget(c)
  /\ voteBudget' = ActualVoteBudget(c)
  /\ drainBudget' = ActualDrainBudget(c)
  /\ idleGap' = ActualIdleGap(c)
  /\ busyGap' = ActualBusyGap(c)
  /\ blockBacklogCap' = ActualBlockBacklogCap(c)
  /\ blockCap' = ActualBlockCap(c)
  /\ payloadCap' = ActualPayloadCap(c)
  /\ rbcCap' = ActualRbcCap(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

MatchesSpec ==
  /\ timeBudget = SpecTimeBudget(candidate)
  /\ voteBudget = SpecVoteBudget(candidate)
  /\ drainBudget = SpecDrainBudget(candidate)
  /\ idleGap = SpecIdleGap(candidate)
  /\ busyGap = SpecBusyGap(candidate)
  /\ blockBacklogCap = SpecBlockBacklogCap(candidate)
  /\ blockCap = SpecBlockCap(candidate)
  /\ payloadCap = SpecPayloadCap(candidate)
  /\ rbcCap = SpecRbcCap(candidate)

WorkerTimeBudgetClamps ==
  /\ candidate \in {"worker_zero_window_floor", "worker_small_window_floor"} =>
       timeBudget = "Floor"
  /\ candidate = "worker_mid_window_quarter" =>
       timeBudget = "QuarterWindow"
  /\ candidate = "worker_large_window_cap" =>
       timeBudget = "GlobalCap"
  /\ candidate = "worker_config_cap" =>
       timeBudget = "ConfigCap"
  /\ candidate = "worker_da_multiplier_ignored" =>
       timeBudget = "UnaffectedByDaMultiplier"

VoteDrainBudgetClamps ==
  /\ candidate = "vote_da_quorum_window" =>
       voteBudget = "DaQuorumWindow"
  /\ candidate = "vote_da_multiplier_window" =>
       voteBudget = "DaMultiplierWindow"
  /\ candidate = "vote_max_budget_cap" =>
       voteBudget = "MaxBudgetCap"
  /\ candidate = "vote_config_cap" =>
       voteBudget = "ConfigCap"
  /\ candidate = "vote_zero_floor" =>
       voteBudget = "Floor"

GenericDrainBudgetClamps ==
  /\ candidate = "drain_floor" =>
       drainBudget = "Floor"
  /\ candidate = "drain_global_cap" =>
       drainBudget = "GlobalCap"
  /\ candidate = "vote_drain_floor" =>
       drainBudget = "Floor"
  /\ candidate = "rbc_config_cap" =>
       drainBudget = "ConfigCap"

TickGapClamps ==
  /\ candidate = "idle_gap_floor" =>
       idleGap = "Floor"
  /\ candidate = "idle_gap_max" =>
       idleGap = "MaxGap"
  /\ candidate = "busy_gap_floor" =>
       busyGap = "BusyFloor"
  /\ candidate = "busy_gap_idle_cap" =>
       busyGap = "IdleCap"

BlockBacklogDepthTiers ==
  /\ candidate = "block_depth_zero" => blockBacklogCap = "Zero"
  /\ candidate = "block_depth_small" => blockBacklogCap = "Small"
  /\ candidate = "block_depth_medium" => blockBacklogCap = "Medium"
  /\ candidate = "block_depth_large" => blockBacklogCap = "Large"
  /\ candidate = "block_depth_huge" => blockBacklogCap = "Huge"

AdaptiveVoteBacklogCaps ==
  /\ candidate = "vote_backlog_payload_reduced" =>
       payloadCap = "VoteReduced"
  /\ candidate = "vote_backlog_rbc_preserved" =>
       rbcCap = "Preserve"

AdaptiveBlockBacklogCaps ==
  /\ candidate = "block_backlog_block_cap" =>
       blockCap = "Large"
  /\ candidate = "block_backlog_payload_min" =>
       /\ payloadCap = "BlockMin"
       /\ rbcCap = "BlockMin"
  /\ candidate = "block_backlog_payload_scaled" =>
       payloadCap = "BlockScaled"
  /\ candidate = "block_backlog_rbc_scaled" =>
       rbcCap = "BlockScaled"

NoBacklogPreservesCaps ==
  candidate = "no_backlog_preserves_caps" =>
    /\ blockCap = "Preserve"
    /\ payloadCap = "Preserve"
    /\ rbcCap = "Preserve"

Safety ==
  /\ MatchesSpec
  /\ WorkerTimeBudgetClamps
  /\ VoteDrainBudgetClamps
  /\ GenericDrainBudgetClamps
  /\ TickGapClamps
  /\ BlockBacklogDepthTiers
  /\ AdaptiveVoteBacklogCaps
  /\ AdaptiveBlockBacklogCaps
  /\ NoBacklogPreservesCaps

=============================================================================
====
