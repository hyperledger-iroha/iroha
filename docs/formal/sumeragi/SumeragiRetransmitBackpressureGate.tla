---- MODULE SumeragiRetransmitBackpressureGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for retransmit backpressure pacing helpers in
`main_loop/reschedule.rs`.

The implementation computes a deterministic pressure score from transaction
queue pressure and RBC store pressure, uses that score to throttle retransmit
fanout without fully disabling known targets, scales the rebroadcast cooldown,
expands consensus-ingress reschedule backoff under backlog, and clamps the
near-quorum payload-repair timeout.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

SoftBytes == 128
HardBytes == 512

Cases == {
  "capacity_zero",
  "tx_low",
  "tx_sixty",
  "tx_eighty",
  "tx_ninety_five",
  "tx_saturated",
  "rbc_level_one",
  "rbc_level_two",
  "rbc_soft",
  "rbc_hard",
  "combined_moderate",
  "combined_heavy",
  "zero_targets_heavy",
  "half_round_up",
  "quarter_round_up",
  "base_zero_backoff",
  "consensus_backlog",
  "near_backlog",
  "timeout_low_clamp",
  "timeout_mid",
  "timeout_high_clamp"
}

QueuePressureCases == {
  "capacity_zero",
  "tx_low",
  "tx_sixty",
  "tx_eighty",
  "tx_ninety_five",
  "tx_saturated"
}

RbcPressureCases == {
  "rbc_level_one",
  "rbc_level_two",
  "rbc_soft",
  "rbc_hard"
}

CombinedPressureCases == {
  "combined_moderate",
  "combined_heavy"
}

TargetLimitCases == {
  "combined_heavy",
  "zero_targets_heavy",
  "half_round_up",
  "quarter_round_up"
}

CooldownCases == {
  "tx_low",
  "half_round_up",
  "combined_moderate",
  "combined_heavy"
}

BackoffCases == {
  "base_zero_backoff",
  "consensus_backlog",
  "near_backlog"
}

TimeoutCases == {
  "timeout_low_clamp",
  "timeout_mid",
  "timeout_high_clamp"
}

Min(a, b) == IF a <= b THEN a ELSE b
Max(a, b) == IF a >= b THEN a ELSE b
CeilDiv(n, d) == IF n = 0 THEN 0 ELSE ((n - 1) \div d) + 1

TxDepth(c) ==
  CASE c = "capacity_zero" -> 100
    [] c = "tx_low" -> 4
    [] c = "tx_sixty" -> 60
    [] c = "tx_eighty" -> 80
    [] c = "tx_ninety_five" -> 95
    [] c = "tx_saturated" -> 1
    [] c \in {"combined_moderate"} -> 80
    [] c \in {"combined_heavy", "zero_targets_heavy"} -> 100
    [] OTHER -> 0

TxCapacity(c) ==
  IF c = "capacity_zero" THEN 0 ELSE 100

TxSaturated(c) ==
  c \in {"tx_saturated", "combined_heavy", "zero_targets_heavy"}

RbcPressureLevel(c) ==
  CASE c = "rbc_level_one" -> 1
    [] c = "rbc_level_two" -> 2
    [] c = "combined_moderate" -> 1
    [] c \in {"combined_heavy", "zero_targets_heavy"} -> 2
    [] c \in {"half_round_up", "quarter_round_up"} -> 1
    [] OTHER -> 0

RbcBytes(c) ==
  CASE c = "rbc_soft" -> SoftBytes
    [] c = "rbc_hard" -> HardBytes
    [] c = "combined_moderate" -> SoftBytes
    [] c \in {"combined_heavy", "zero_targets_heavy"} -> HardBytes
    [] c = "quarter_round_up" -> HardBytes
    [] OTHER -> 0

TargetCount(c) ==
  CASE c = "zero_targets_heavy" -> 0
    [] c = "half_round_up" -> 5
    [] c = "quarter_round_up" -> 5
    [] OTHER -> 12

BaseBackoff(c) ==
  IF c = "base_zero_backoff" THEN 0 ELSE 100

ConsensusBacklog(c) ==
  c \in {"consensus_backlog", "near_backlog", "base_zero_backoff"}

NearQuorumBacklog(c) ==
  c \in {"near_backlog", "base_zero_backoff"}

RebroadcastCooldown(c) ==
  CASE c = "timeout_low_clamp" -> 50
    [] c = "timeout_mid" -> 300
    [] c = "timeout_high_clamp" -> 2000
    [] OTHER -> 300

TxUtilPct(c) ==
  IF TxCapacity(c) = 0 THEN 0
  ELSE (TxDepth(c) * 100) \div Max(TxCapacity(c), 1)

SpecTxScore(c) ==
  IF TxSaturated(c) \/ TxUtilPct(c) >= 95 THEN 3
  ELSE IF TxUtilPct(c) >= 80 THEN 2
  ELSE IF TxUtilPct(c) >= 60 THEN 1
  ELSE 0

SpecRbcLevelScore(c) ==
  IF RbcPressureLevel(c) >= 2 THEN 3
  ELSE IF RbcPressureLevel(c) = 1 THEN 2
  ELSE 0

SpecRbcBytesScore(c) ==
  IF RbcBytes(c) >= HardBytes THEN 2
  ELSE IF RbcBytes(c) >= SoftBytes THEN 1
  ELSE 0

SpecPressureScore(c) ==
  SpecTxScore(c) + SpecRbcLevelScore(c) + SpecRbcBytesScore(c)

LimitFor(score, target_count) ==
  IF target_count = 0 THEN 0
  ELSE IF score >= 6 THEN 1
  ELSE IF score >= 4 THEN Max(CeilDiv(target_count, 4), 1)
  ELSE IF score >= 2 THEN Max(CeilDiv(target_count, 2), 1)
  ELSE target_count

SpecTargetLimit(c) ==
  LimitFor(SpecPressureScore(c), TargetCount(c))

CooldownFor(score) ==
  IF score >= 6 THEN 4
  ELSE IF score >= 4 THEN 3
  ELSE IF score >= 2 THEN 2
  ELSE 1

SpecCooldownMultiplier(c) ==
  CooldownFor(SpecPressureScore(c))

SpecIngressBackoff(c) ==
  IF BaseBackoff(c) = 0 THEN 0
  ELSE IF NearQuorumBacklog(c) THEN BaseBackoff(c) * 8
  ELSE IF ConsensusBacklog(c) THEN BaseBackoff(c) * 4
  ELSE BaseBackoff(c)

SpecNearTimeout(c) ==
  Min(Max(RebroadcastCooldown(c) * 2, 200), 2000)

\* @type: (Str) => <<Int, Int, Int, Int, Int>>;
SpecOutput(c) ==
  <<SpecPressureScore(c), SpecTargetLimit(c), SpecCooldownMultiplier(c),
    SpecIngressBackoff(c), SpecNearTimeout(c)>>

ActualTxScore(c) ==
  CASE Bug = "capacity_zero_uses_depth" /\ c = "capacity_zero" -> 3
    [] Bug = "tx_60_zero" /\ c = "tx_sixty" -> 0
    [] Bug = "tx_80_as_one" /\ c = "tx_eighty" -> 1
    [] Bug = "tx_95_as_two" /\ c = "tx_ninety_five" -> 2
    [] Bug = "ignore_saturation" /\ c = "tx_saturated" -> 0
    [] OTHER -> SpecTxScore(c)

ActualRbcLevelScore(c) ==
  CASE Bug = "rbc_level_one_low" /\ c = "rbc_level_one" -> 1
    [] Bug = "rbc_level_two_low" /\ c = "rbc_level_two" -> 2
    [] OTHER -> SpecRbcLevelScore(c)

ActualRbcBytesScore(c) ==
  CASE Bug = "rbc_soft_ignored" /\ c = "rbc_soft" -> 0
    [] Bug = "rbc_hard_as_soft" /\ c = "rbc_hard" -> 1
    [] OTHER -> SpecRbcBytesScore(c)

ActualPressureScore(c) ==
  CASE Bug = "combined_not_additive" /\ c = "combined_moderate" -> 2
    [] OTHER -> ActualTxScore(c) + ActualRbcLevelScore(c) +
         ActualRbcBytesScore(c)

ActualTargetLimit(c) ==
  CASE Bug = "heavy_limit_zero" /\ c = "combined_heavy" -> 0
    [] Bug = "zero_targets_limit_one" /\ c = "zero_targets_heavy" -> 1
    [] Bug = "half_floor_division" /\ c = "half_round_up" ->
         TargetCount(c) \div 2
    [] Bug = "quarter_floor_division" /\ c = "quarter_round_up" ->
         TargetCount(c) \div 4
    [] OTHER -> LimitFor(ActualPressureScore(c), TargetCount(c))

ActualCooldownMultiplier(c) ==
  CASE Bug = "cooldown_medium_one" /\ c = "half_round_up" -> 1
    [] Bug = "cooldown_heavy_three" /\ c = "combined_heavy" -> 3
    [] OTHER -> CooldownFor(ActualPressureScore(c))

ActualIngressBackoff(c) ==
  CASE Bug = "base_zero_multiplied" /\ c = "base_zero_backoff" -> 800
    [] Bug = "consensus_backlog_ignored" /\ c = "consensus_backlog" ->
         BaseBackoff(c)
    [] Bug = "near_backlog_uses_consensus" /\ c = "near_backlog" ->
         BaseBackoff(c) * 4
    [] OTHER -> SpecIngressBackoff(c)

ActualNearTimeout(c) ==
  CASE Bug = "timeout_no_lower_clamp" /\ c = "timeout_low_clamp" ->
         RebroadcastCooldown(c) * 2
    [] Bug = "timeout_not_doubled" /\ c = "timeout_mid" ->
         RebroadcastCooldown(c)
    [] Bug = "timeout_no_upper_clamp" /\ c = "timeout_high_clamp" ->
         RebroadcastCooldown(c) * 2
    [] OTHER -> SpecNearTimeout(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int>>;
ActualOutput(c) ==
  <<ActualPressureScore(c), ActualTargetLimit(c), ActualCooldownMultiplier(c),
    ActualIngressBackoff(c), ActualNearTimeout(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "capacity_zero_uses_depth",
       "tx_60_zero",
       "tx_80_as_one",
       "tx_95_as_two",
       "ignore_saturation",
       "rbc_level_one_low",
       "rbc_level_two_low",
       "rbc_soft_ignored",
       "rbc_hard_as_soft",
       "combined_not_additive",
       "heavy_limit_zero",
       "zero_targets_limit_one",
       "half_floor_division",
       "quarter_floor_division",
       "cooldown_medium_one",
       "cooldown_heavy_three",
       "base_zero_multiplied",
       "consensus_backlog_ignored",
       "near_backlog_uses_consensus",
       "timeout_no_lower_clamp",
       "timeout_not_doubled",
       "timeout_no_upper_clamp"
     }
  /\ checked = 0

SafetyFast ==
  /\ ActualOutput("capacity_zero") = SpecOutput("capacity_zero")
  /\ ActualOutput("tx_low") = SpecOutput("tx_low")
  /\ ActualOutput("tx_sixty") = SpecOutput("tx_sixty")
  /\ ActualOutput("tx_eighty") = SpecOutput("tx_eighty")
  /\ ActualOutput("tx_ninety_five") = SpecOutput("tx_ninety_five")
  /\ ActualOutput("tx_saturated") = SpecOutput("tx_saturated")
  /\ ActualOutput("rbc_level_one") = SpecOutput("rbc_level_one")
  /\ ActualOutput("rbc_level_two") = SpecOutput("rbc_level_two")
  /\ ActualOutput("rbc_soft") = SpecOutput("rbc_soft")
  /\ ActualOutput("rbc_hard") = SpecOutput("rbc_hard")
  /\ ActualOutput("combined_moderate") = SpecOutput("combined_moderate")
  /\ ActualOutput("combined_heavy") = SpecOutput("combined_heavy")
  /\ ActualOutput("zero_targets_heavy") = SpecOutput("zero_targets_heavy")
  /\ ActualOutput("half_round_up") = SpecOutput("half_round_up")
  /\ ActualOutput("quarter_round_up") = SpecOutput("quarter_round_up")
  /\ ActualOutput("base_zero_backoff") = SpecOutput("base_zero_backoff")
  /\ ActualOutput("consensus_backlog") = SpecOutput("consensus_backlog")
  /\ ActualOutput("near_backlog") = SpecOutput("near_backlog")
  /\ ActualOutput("timeout_low_clamp") = SpecOutput("timeout_low_clamp")
  /\ ActualOutput("timeout_mid") = SpecOutput("timeout_mid")
  /\ ActualOutput("timeout_high_clamp") = SpecOutput("timeout_high_clamp")

RetransmitQueuePressureExact ==
  \A c \in QueuePressureCases:
    /\ ActualTxScore(c) = SpecTxScore(c)
    /\ ActualPressureScore(c) = SpecPressureScore(c)
    /\ ActualOutput(c) = SpecOutput(c)

RetransmitRbcPressureExact ==
  \A c \in RbcPressureCases:
    /\ ActualRbcLevelScore(c) = SpecRbcLevelScore(c)
    /\ ActualRbcBytesScore(c) = SpecRbcBytesScore(c)
    /\ ActualPressureScore(c) = SpecPressureScore(c)
    /\ ActualOutput(c) = SpecOutput(c)

RetransmitCombinedPressureExact ==
  \A c \in CombinedPressureCases:
    /\ ActualPressureScore(c) =
         ActualTxScore(c) + ActualRbcLevelScore(c) + ActualRbcBytesScore(c)
    /\ ActualPressureScore(c) = SpecPressureScore(c)
    /\ ActualOutput(c) = SpecOutput(c)

RetransmitTargetLimitExact ==
  \A c \in TargetLimitCases:
    /\ ActualTargetLimit(c) = SpecTargetLimit(c)
    /\ ActualTargetLimit(c) = LimitFor(ActualPressureScore(c), TargetCount(c))
    /\ IF TargetCount(c) = 0 THEN ActualTargetLimit(c) = 0
       ELSE ActualTargetLimit(c) >= 1
    /\ ActualTargetLimit(c) <= TargetCount(c)
    /\ ActualOutput(c) = SpecOutput(c)

RetransmitCooldownExact ==
  \A c \in CooldownCases:
    /\ ActualCooldownMultiplier(c) = SpecCooldownMultiplier(c)
    /\ ActualCooldownMultiplier(c) = CooldownFor(ActualPressureScore(c))
    /\ ActualOutput(c) = SpecOutput(c)

RetransmitBackoffExact ==
  \A c \in BackoffCases:
    /\ ActualIngressBackoff(c) = SpecIngressBackoff(c)
    /\ IF BaseBackoff(c) = 0 THEN ActualIngressBackoff(c) = 0 ELSE TRUE
    /\ IF NearQuorumBacklog(c) /\ BaseBackoff(c) # 0 THEN
         ActualIngressBackoff(c) = BaseBackoff(c) * 8
       ELSE TRUE
    /\ IF ConsensusBacklog(c) /\ ~NearQuorumBacklog(c) /\ BaseBackoff(c) # 0
       THEN ActualIngressBackoff(c) = BaseBackoff(c) * 4
       ELSE TRUE
    /\ ActualOutput(c) = SpecOutput(c)

RetransmitTimeoutClampExact ==
  \A c \in TimeoutCases:
    /\ ActualNearTimeout(c) = SpecNearTimeout(c)
    /\ ActualNearTimeout(c) >= 200
    /\ ActualNearTimeout(c) <= 2000
    /\ ActualOutput(c) = SpecOutput(c)

RetransmitBackpressurePacingExactness ==
  /\ SafetyFast
  /\ RetransmitQueuePressureExact
  /\ RetransmitRbcPressureExact
  /\ RetransmitCombinedPressureExact
  /\ RetransmitTargetLimitExact
  /\ RetransmitCooldownExact
  /\ RetransmitBackoffExact
  /\ RetransmitTimeoutClampExact

BugCapacityZeroUsesDepth ==
  ActualOutput("capacity_zero") = SpecOutput("capacity_zero")

BugTx60Zero ==
  ActualOutput("tx_sixty") = SpecOutput("tx_sixty")

BugTx80AsOne ==
  ActualOutput("tx_eighty") = SpecOutput("tx_eighty")

BugTx95AsTwo ==
  ActualOutput("tx_ninety_five") = SpecOutput("tx_ninety_five")

BugIgnoreSaturation ==
  ActualOutput("tx_saturated") = SpecOutput("tx_saturated")

BugRbcLevelOneLow ==
  ActualOutput("rbc_level_one") = SpecOutput("rbc_level_one")

BugRbcLevelTwoLow ==
  ActualOutput("rbc_level_two") = SpecOutput("rbc_level_two")

BugRbcSoftIgnored ==
  ActualOutput("rbc_soft") = SpecOutput("rbc_soft")

BugRbcHardAsSoft ==
  ActualOutput("rbc_hard") = SpecOutput("rbc_hard")

BugCombinedNotAdditive ==
  ActualOutput("combined_moderate") = SpecOutput("combined_moderate")

BugHeavyLimitZero ==
  ActualOutput("combined_heavy") = SpecOutput("combined_heavy")

BugZeroTargetsLimitOne ==
  ActualOutput("zero_targets_heavy") = SpecOutput("zero_targets_heavy")

BugHalfFloorDivision ==
  ActualOutput("half_round_up") = SpecOutput("half_round_up")

BugQuarterFloorDivision ==
  ActualOutput("quarter_round_up") = SpecOutput("quarter_round_up")

BugCooldownMediumOne ==
  ActualOutput("half_round_up") = SpecOutput("half_round_up")

BugCooldownHeavyThree ==
  ActualOutput("combined_heavy") = SpecOutput("combined_heavy")

BugBaseZeroMultiplied ==
  ActualOutput("base_zero_backoff") = SpecOutput("base_zero_backoff")

BugConsensusBacklogIgnored ==
  ActualOutput("consensus_backlog") = SpecOutput("consensus_backlog")

BugNearBacklogUsesConsensus ==
  ActualOutput("near_backlog") = SpecOutput("near_backlog")

BugTimeoutNoLowerClamp ==
  ActualOutput("timeout_low_clamp") = SpecOutput("timeout_low_clamp")

BugTimeoutNotDoubled ==
  ActualOutput("timeout_mid") = SpecOutput("timeout_mid")

BugTimeoutNoUpperClamp ==
  ActualOutput("timeout_high_clamp") = SpecOutput("timeout_high_clamp")

Safety ==
  SafetyFast

=============================================================================
====
