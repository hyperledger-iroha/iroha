---- MODULE SumeragiStalledPendingFrontierTimeoutGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for stalled pending-frontier timeout derivation.

This slice pins `backlog_extended_view_change_timeout(...)`,
`active_block_production_gap_ceiling(...)`,
`cap_active_block_production_gap(...)`, and
`stalled_pending_frontier_pending_timeout(...)`. Recovery backlog signals extend
the base view-change timeout with bounded saturating arithmetic. Consensus
ingress backlog only raises the deferred-QC TTL multiplier when resilience,
consensus-queue backlog, and active transaction backlog are all present. Active
transaction backlog then caps the resulting timeout by the active block
production gap ceiling.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

MaxMillis == 10000
ActiveGapLimit == 5000
BacklogGraceFloor == 400

Cases == {
  "no_backlog_base",
  "worker_recovery_extends",
  "residual_recovery_extends",
  "unresolved_rbc_extends",
  "consensus_ingress_uses_4x",
  "consensus_disabled_uses_2x",
  "consensus_queue_without_active",
  "queue_active_no_consensus",
  "queue_active_caps",
  "queue_inactive_no_cap",
  "grace_floor_400",
  "grace_uses_cooldown_times_8",
  "cap_by_double_base",
  "cap_floor_ttl_plus_cooldown",
  "deferred_qc_dominates",
  "backlog_timeout_dominates",
  "saturating_mul",
  "saturating_add",
  "active_ceiling_floor",
  "active_ceiling_global_cap"
}

Min(a, b) == IF a <= b THEN a ELSE b
Max(a, b) == IF a >= b THEN a ELSE b

SaturatingAdd(a, b) ==
  IF a + b >= MaxMillis THEN MaxMillis ELSE a + b

SaturatingMul(value, multiplier) ==
  IF value * multiplier >= MaxMillis THEN MaxMillis ELSE value * multiplier

\* @type: Str => Int;
BaseTimeout(c) ==
  CASE c = "deferred_qc_dominates" -> 500
    [] c = "saturating_add" -> 9500
    [] OTHER -> 1000

\* @type: Str => Int;
RebroadcastCooldown(c) ==
  CASE c = "grace_floor_400" -> 20
    [] c \in {"cap_by_double_base", "cap_floor_ttl_plus_cooldown", "saturating_add"} -> 200
    [] OTHER -> 100

\* @type: Str => Int;
RecoveryDeferredQcTtl(c) ==
  CASE c \in {
         "consensus_ingress_uses_4x",
         "consensus_disabled_uses_2x",
         "consensus_queue_without_active",
         "queue_active_no_consensus"
       } -> 600
    [] c \in {"queue_active_caps", "queue_inactive_no_cap", "active_ceiling_floor"} -> 2000
    [] c = "active_ceiling_global_cap" -> 4000
    [] c = "cap_floor_ttl_plus_cooldown" -> 2500
    [] c = "deferred_qc_dominates" -> 1000
    [] c = "saturating_mul" -> 3000
    [] OTHER -> 100

\* @type: Str => Int;
ConfigCommitInflightTimeout(c) ==
  CASE c \in {"queue_active_caps", "queue_inactive_no_cap"} -> 1500
    [] c = "active_ceiling_floor" -> 0
    [] c = "active_ceiling_global_cap" -> 8000
    [] OTHER -> ActiveGapLimit

\* @type: Str => Bool;
ResilienceEnabled(c) ==
  c # "consensus_disabled_uses_2x"

\* @type: Str => Bool;
WorkerRecoveryBacklog(c) ==
  c \in {
    "worker_recovery_extends",
    "grace_floor_400",
    "grace_uses_cooldown_times_8",
    "cap_by_double_base",
    "cap_floor_ttl_plus_cooldown",
    "backlog_timeout_dominates",
    "saturating_add"
  }

\* @type: Str => Bool;
ResidualRoundBacklog(c) == c = "residual_recovery_extends"

\* @type: Str => Bool;
UnresolvedRbcBacklog(c) == c = "unresolved_rbc_extends"

\* @type: Str => Bool;
ConsensusQueueBacklog(c) ==
  c \in {
    "consensus_ingress_uses_4x",
    "consensus_disabled_uses_2x",
    "consensus_queue_without_active",
    "saturating_mul"
  }

\* @type: Str => Bool;
QueueActiveBacklog(c) ==
  c \in {
    "consensus_ingress_uses_4x",
    "consensus_disabled_uses_2x",
    "queue_active_no_consensus",
    "queue_active_caps",
    "saturating_mul",
    "active_ceiling_floor",
    "active_ceiling_global_cap"
  }

\* @type: Str => Bool;
SpecRecoveryBacklogSignalsActive(c) ==
  \/ WorkerRecoveryBacklog(c)
  \/ ResidualRoundBacklog(c)
  \/ UnresolvedRbcBacklog(c)

\* @type: Str => Int;
SpecBacklogGrace(c) ==
  Max(SaturatingMul(RebroadcastCooldown(c), 8), BacklogGraceFloor)

\* @type: Str => Int;
SpecExtendedBacklog(c) ==
  SaturatingAdd(BaseTimeout(c), SpecBacklogGrace(c))

\* @type: Str => Int;
SpecBacklogCapFloor(c) ==
  Max(
    SaturatingAdd(RecoveryDeferredQcTtl(c), RebroadcastCooldown(c)),
    BaseTimeout(c)
  )

\* @type: Str => Int;
SpecBacklogCap(c) ==
  Max(SaturatingMul(BaseTimeout(c), 2), SpecBacklogCapFloor(c))

\* @type: Str => Int;
SpecBacklogTimeout(c) ==
  IF SpecRecoveryBacklogSignalsActive(c)
  THEN Min(SpecExtendedBacklog(c), SpecBacklogCap(c))
  ELSE BaseTimeout(c)

\* @type: Str => Bool;
SpecConsensusIngressBacklogActive(c) ==
  /\ ResilienceEnabled(c)
  /\ ConsensusQueueBacklog(c)
  /\ QueueActiveBacklog(c)

\* @type: Str => Int;
SpecDeferredMultiplier(c) ==
  IF SpecConsensusIngressBacklogActive(c) THEN 4 ELSE 2

\* @type: Str => Int;
SpecDeferredTimeout(c) ==
  SaturatingMul(RecoveryDeferredQcTtl(c), SpecDeferredMultiplier(c))

\* @type: Str => Int;
SpecUncappedFrontierTimeout(c) ==
  Max(SpecDeferredTimeout(c), SpecBacklogTimeout(c))

\* @type: Str => Int;
SpecActiveGapCeiling(c) ==
  Max(Min(ConfigCommitInflightTimeout(c), ActiveGapLimit), 1)

\* @type: Str => Int;
SpecFrontierTimeout(c) ==
  IF QueueActiveBacklog(c)
  THEN Min(SpecUncappedFrontierTimeout(c), SpecActiveGapCeiling(c))
  ELSE SpecUncappedFrontierTimeout(c)

\* @type: Str => Bool;
ActualRecoveryBacklogSignalsActive(c) ==
  CASE Bug = "recovery_uses_consensus_queue"
       /\ c = "consensus_queue_without_active" -> ConsensusQueueBacklog(c)
    [] Bug = "skip_worker_recovery_extension"
       /\ c = "worker_recovery_extends" -> FALSE
    [] Bug = "skip_residual_recovery_extension"
       /\ c = "residual_recovery_extends" -> FALSE
    [] Bug = "skip_unresolved_rbc_extension"
       /\ c = "unresolved_rbc_extends" -> FALSE
    [] OTHER -> SpecRecoveryBacklogSignalsActive(c)

\* @type: Str => Int;
ActualBacklogGrace(c) ==
  CASE Bug = "backlog_grace_no_floor"
       /\ c = "grace_floor_400" -> SaturatingMul(RebroadcastCooldown(c), 8)
    [] Bug = "backlog_grace_uses_multiplier_4"
       /\ c = "grace_uses_cooldown_times_8" ->
          Max(SaturatingMul(RebroadcastCooldown(c), 4), BacklogGraceFloor)
    [] OTHER -> SpecBacklogGrace(c)

\* @type: Str => Int;
ActualExtendedBacklog(c) ==
  CASE Bug = "saturating_add_overflows"
       /\ c = "saturating_add" -> BaseTimeout(c) + ActualBacklogGrace(c)
    [] OTHER -> SaturatingAdd(BaseTimeout(c), ActualBacklogGrace(c))

\* @type: Str => Int;
ActualBacklogCapFloor(c) ==
  CASE Bug = "cap_floor_omits_ttl"
       /\ c = "cap_floor_ttl_plus_cooldown" ->
          Max(RebroadcastCooldown(c), BaseTimeout(c))
    [] Bug = "cap_floor_omits_cooldown"
       /\ c = "cap_floor_ttl_plus_cooldown" ->
          Max(RecoveryDeferredQcTtl(c), BaseTimeout(c))
    [] OTHER ->
          Max(
            SaturatingAdd(RecoveryDeferredQcTtl(c), RebroadcastCooldown(c)),
            BaseTimeout(c)
          )

\* @type: Str => Int;
ActualBacklogCap(c) ==
  CASE Bug = "cap_uses_min_double_base"
       /\ c = "cap_by_double_base" ->
          Min(SaturatingMul(BaseTimeout(c), 2), ActualBacklogCapFloor(c))
    [] OTHER -> Max(SaturatingMul(BaseTimeout(c), 2), ActualBacklogCapFloor(c))

\* @type: Str => Int;
ActualBacklogTimeout(c) ==
  IF ActualRecoveryBacklogSignalsActive(c)
  THEN
    CASE Bug = "backlog_extended_not_capped"
         /\ c = "cap_by_double_base" -> ActualExtendedBacklog(c)
      [] OTHER -> Min(ActualExtendedBacklog(c), ActualBacklogCap(c))
  ELSE BaseTimeout(c)

\* @type: Str => Bool;
ActualConsensusIngressBacklogActive(c) ==
  CASE Bug = "consensus_ingress_ignores_resilience"
       /\ c = "consensus_disabled_uses_2x" ->
          ConsensusQueueBacklog(c) /\ QueueActiveBacklog(c)
    [] Bug = "consensus_ingress_ignores_queue_active"
       /\ c = "consensus_queue_without_active" ->
          ResilienceEnabled(c) /\ ConsensusQueueBacklog(c)
    [] Bug = "consensus_ingress_ignores_consensus_queue"
       /\ c = "queue_active_no_consensus" ->
          ResilienceEnabled(c) /\ QueueActiveBacklog(c)
    [] OTHER -> SpecConsensusIngressBacklogActive(c)

\* @type: Str => Int;
ActualDeferredMultiplier(c) ==
  CASE Bug = "deferred_multiplier_always_2"
       /\ c = "consensus_ingress_uses_4x" -> 2
    [] Bug = "deferred_multiplier_always_4"
       /\ c = "deferred_qc_dominates" -> 4
    [] OTHER -> IF ActualConsensusIngressBacklogActive(c) THEN 4 ELSE 2

\* @type: Str => Int;
ActualDeferredTimeout(c) ==
  CASE Bug = "saturating_mul_overflows"
       /\ c = "saturating_mul" ->
          RecoveryDeferredQcTtl(c) * ActualDeferredMultiplier(c)
    [] OTHER -> SaturatingMul(RecoveryDeferredQcTtl(c), ActualDeferredMultiplier(c))

\* @type: Str => Int;
ActualUncappedFrontierTimeout(c) ==
  CASE Bug = "timeout_uses_min_not_max"
       /\ c = "backlog_timeout_dominates" ->
          Min(ActualDeferredTimeout(c), ActualBacklogTimeout(c))
    [] OTHER -> Max(ActualDeferredTimeout(c), ActualBacklogTimeout(c))

\* @type: Str => Int;
ActualActiveGapCeiling(c) ==
  CASE Bug = "active_ceiling_no_floor"
       /\ c = "active_ceiling_floor" ->
          Min(ConfigCommitInflightTimeout(c), ActiveGapLimit)
    [] Bug = "active_ceiling_no_global_cap"
       /\ c = "active_ceiling_global_cap" ->
          Max(ConfigCommitInflightTimeout(c), 1)
    [] OTHER -> SpecActiveGapCeiling(c)

\* @type: Str => Int;
ActualFrontierTimeout(c) ==
  CASE Bug = "queue_active_no_cap"
       /\ c = "queue_active_caps" -> ActualUncappedFrontierTimeout(c)
    [] Bug = "queue_inactive_caps"
       /\ c = "queue_inactive_no_cap" ->
          Min(ActualUncappedFrontierTimeout(c), ActualActiveGapCeiling(c))
    [] Bug = "cap_uses_base_timeout"
       /\ c = "queue_active_caps" ->
          Min(ActualUncappedFrontierTimeout(c), BaseTimeout(c))
    [] QueueActiveBacklog(c) ->
          Min(ActualUncappedFrontierTimeout(c), ActualActiveGapCeiling(c))
    [] OTHER -> ActualUncappedFrontierTimeout(c)

\* @type: Str => <<Bool, Int, Int, Int, Int, Bool, Int, Int, Int, Int, Int>>;
SpecDecision(c) ==
  <<SpecRecoveryBacklogSignalsActive(c),
    SpecBacklogGrace(c),
    SpecExtendedBacklog(c),
    SpecBacklogCap(c),
    SpecBacklogTimeout(c),
    SpecConsensusIngressBacklogActive(c),
    SpecDeferredMultiplier(c),
    SpecDeferredTimeout(c),
    SpecUncappedFrontierTimeout(c),
    SpecActiveGapCeiling(c),
    SpecFrontierTimeout(c)>>

\* @type: Str => <<Bool, Int, Int, Int, Int, Bool, Int, Int, Int, Int, Int>>;
ActualDecision(c) ==
  <<ActualRecoveryBacklogSignalsActive(c),
    ActualBacklogGrace(c),
    ActualExtendedBacklog(c),
    ActualBacklogCap(c),
    ActualBacklogTimeout(c),
    ActualConsensusIngressBacklogActive(c),
    ActualDeferredMultiplier(c),
    ActualDeferredTimeout(c),
    ActualUncappedFrontierTimeout(c),
    ActualActiveGapCeiling(c),
    ActualFrontierTimeout(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "recovery_uses_consensus_queue",
       "skip_worker_recovery_extension",
       "skip_residual_recovery_extension",
       "skip_unresolved_rbc_extension",
       "backlog_grace_no_floor",
       "backlog_grace_uses_multiplier_4",
       "backlog_extended_not_capped",
       "cap_floor_omits_ttl",
       "cap_floor_omits_cooldown",
       "cap_uses_min_double_base",
       "consensus_ingress_ignores_resilience",
       "consensus_ingress_ignores_queue_active",
       "consensus_ingress_ignores_consensus_queue",
       "deferred_multiplier_always_2",
       "deferred_multiplier_always_4",
       "timeout_uses_min_not_max",
       "queue_active_no_cap",
       "queue_inactive_caps",
       "cap_uses_base_timeout",
       "active_ceiling_no_floor",
       "active_ceiling_no_global_cap",
       "saturating_mul_overflows",
       "saturating_add_overflows"
     }
  /\ checked = 0

DecisionMatchesSpec ==
  \A c \in Cases:
    ActualDecision(c) = SpecDecision(c)

RecoverySignalsMatchesSpec ==
  \A c \in Cases:
    ActualRecoveryBacklogSignalsActive(c) =
      SpecRecoveryBacklogSignalsActive(c)

BacklogGraceMatchesSpec ==
  \A c \in Cases:
    ActualBacklogGrace(c) = SpecBacklogGrace(c)

ExtendedBacklogMatchesSpec ==
  \A c \in Cases:
    ActualExtendedBacklog(c) = SpecExtendedBacklog(c)

BacklogCapFloorMatchesSpec ==
  \A c \in Cases:
    ActualBacklogCapFloor(c) = SpecBacklogCapFloor(c)

BacklogCapMatchesSpec ==
  \A c \in Cases:
    ActualBacklogCap(c) = SpecBacklogCap(c)

BacklogTimeoutMatchesSpec ==
  \A c \in Cases:
    ActualBacklogTimeout(c) = SpecBacklogTimeout(c)

ConsensusIngressMatchesSpec ==
  \A c \in Cases:
    ActualConsensusIngressBacklogActive(c) =
      SpecConsensusIngressBacklogActive(c)

DeferredMultiplierMatchesSpec ==
  \A c \in Cases:
    ActualDeferredMultiplier(c) = SpecDeferredMultiplier(c)

DeferredTimeoutMatchesSpec ==
  \A c \in Cases:
    ActualDeferredTimeout(c) = SpecDeferredTimeout(c)

UncappedTimeoutMatchesSpec ==
  \A c \in Cases:
    ActualUncappedFrontierTimeout(c) = SpecUncappedFrontierTimeout(c)

ActiveGapCeilingMatchesSpec ==
  \A c \in Cases:
    ActualActiveGapCeiling(c) = SpecActiveGapCeiling(c)

FrontierTimeoutMatchesSpec ==
  \A c \in Cases:
    ActualFrontierTimeout(c) = SpecFrontierTimeout(c)

RecoverySignalAnchors ==
  /\ SpecRecoveryBacklogSignalsActive("worker_recovery_extends")
  /\ SpecRecoveryBacklogSignalsActive("residual_recovery_extends")
  /\ SpecRecoveryBacklogSignalsActive("unresolved_rbc_extends")
  /\ ~SpecRecoveryBacklogSignalsActive("consensus_queue_without_active")

BacklogGraceAnchors ==
  /\ SpecBacklogGrace("grace_floor_400") = BacklogGraceFloor
  /\ SpecBacklogGrace("grace_uses_cooldown_times_8") = 800

BacklogTimeoutAnchors ==
  /\ SpecBacklogTimeout("cap_by_double_base") = 2000
  /\ SpecBacklogTimeout("cap_floor_ttl_plus_cooldown") = 2600
  /\ SpecBacklogTimeout("worker_recovery_extends") = 1800
  /\ SpecBacklogTimeout("no_backlog_base") = BaseTimeout("no_backlog_base")

ConsensusIngressAnchors ==
  /\ SpecConsensusIngressBacklogActive("consensus_ingress_uses_4x")
  /\ ~SpecConsensusIngressBacklogActive("consensus_disabled_uses_2x")
  /\ ~SpecConsensusIngressBacklogActive("consensus_queue_without_active")
  /\ ~SpecConsensusIngressBacklogActive("queue_active_no_consensus")

DeferredMultiplierAnchors ==
  /\ SpecDeferredMultiplier("consensus_ingress_uses_4x") = 4
  /\ SpecDeferredMultiplier("consensus_disabled_uses_2x") = 2
  /\ SpecDeferredMultiplier("consensus_queue_without_active") = 2
  /\ SpecDeferredMultiplier("queue_active_no_consensus") = 2

FrontierTimeoutAnchors ==
  /\ SpecFrontierTimeout("queue_active_caps") = 1500
  /\ SpecFrontierTimeout("queue_inactive_no_cap") = 4000
  /\ SpecFrontierTimeout("consensus_ingress_uses_4x") = 2400
  /\ SpecFrontierTimeout("deferred_qc_dominates") = 2000

ActiveGapCeilingAnchors ==
  /\ SpecActiveGapCeiling("active_ceiling_floor") = 1
  /\ SpecActiveGapCeiling("active_ceiling_global_cap") = ActiveGapLimit

SaturatingArithmeticAnchors ==
  /\ SpecDeferredTimeout("saturating_mul") = MaxMillis
  /\ SpecExtendedBacklog("saturating_add") = MaxMillis

DecisionProjectionAnchors ==
  /\ SpecDecision("no_backlog_base") =
       <<FALSE, 800, 1800, 2000, 1000, FALSE, 2, 200, 1000,
         ActiveGapLimit, 1000>>
  /\ SpecDecision("worker_recovery_extends") =
       <<TRUE, 800, 1800, 2000, 1800, FALSE, 2, 200, 1800,
         ActiveGapLimit, 1800>>
  /\ SpecDecision("consensus_ingress_uses_4x") =
       <<FALSE, 800, 1800, 2000, 1000, TRUE, 4, 2400, 2400,
         ActiveGapLimit, 2400>>
  /\ SpecDecision("queue_active_caps") =
       <<FALSE, 800, 1800, 2100, 1000, FALSE, 2, 4000, 4000,
         1500, 1500>>

SafetyFast ==
  /\ DecisionMatchesSpec
  /\ RecoverySignalsMatchesSpec
  /\ BacklogGraceMatchesSpec
  /\ ExtendedBacklogMatchesSpec
  /\ BacklogCapFloorMatchesSpec
  /\ BacklogCapMatchesSpec
  /\ BacklogTimeoutMatchesSpec
  /\ ConsensusIngressMatchesSpec
  /\ DeferredMultiplierMatchesSpec
  /\ DeferredTimeoutMatchesSpec
  /\ UncappedTimeoutMatchesSpec
  /\ ActiveGapCeilingMatchesSpec
  /\ FrontierTimeoutMatchesSpec
  /\ RecoverySignalAnchors
  /\ BacklogGraceAnchors
  /\ BacklogTimeoutAnchors
  /\ ConsensusIngressAnchors
  /\ DeferredMultiplierAnchors
  /\ FrontierTimeoutAnchors
  /\ ActiveGapCeilingAnchors
  /\ SaturatingArithmeticAnchors
  /\ DecisionProjectionAnchors

BugRecoveryUsesConsensusQueue ==
  ActualDecision("consensus_queue_without_active") =
    SpecDecision("consensus_queue_without_active")

BugSkipWorkerRecoveryExtension ==
  ActualDecision("worker_recovery_extends") =
    SpecDecision("worker_recovery_extends")

BugSkipResidualRecoveryExtension ==
  ActualDecision("residual_recovery_extends") =
    SpecDecision("residual_recovery_extends")

BugSkipUnresolvedRbcExtension ==
  ActualDecision("unresolved_rbc_extends") =
    SpecDecision("unresolved_rbc_extends")

BugBacklogGraceNoFloor ==
  ActualDecision("grace_floor_400") = SpecDecision("grace_floor_400")

BugBacklogGraceUsesMultiplier4 ==
  ActualDecision("grace_uses_cooldown_times_8") =
    SpecDecision("grace_uses_cooldown_times_8")

BugBacklogExtendedNotCapped ==
  ActualDecision("cap_by_double_base") = SpecDecision("cap_by_double_base")

BugCapFloorOmitsTtl ==
  ActualDecision("cap_floor_ttl_plus_cooldown") =
    SpecDecision("cap_floor_ttl_plus_cooldown")

BugCapFloorOmitsCooldown ==
  ActualDecision("cap_floor_ttl_plus_cooldown") =
    SpecDecision("cap_floor_ttl_plus_cooldown")

BugCapUsesMinDoubleBase ==
  ActualDecision("cap_by_double_base") = SpecDecision("cap_by_double_base")

BugConsensusIngressIgnoresResilience ==
  ActualDecision("consensus_disabled_uses_2x") =
    SpecDecision("consensus_disabled_uses_2x")

BugConsensusIngressIgnoresQueueActive ==
  ActualDecision("consensus_queue_without_active") =
    SpecDecision("consensus_queue_without_active")

BugConsensusIngressIgnoresConsensusQueue ==
  ActualDecision("queue_active_no_consensus") =
    SpecDecision("queue_active_no_consensus")

BugDeferredMultiplierAlways2 ==
  ActualDecision("consensus_ingress_uses_4x") =
    SpecDecision("consensus_ingress_uses_4x")

BugDeferredMultiplierAlways4 ==
  ActualDecision("deferred_qc_dominates") =
    SpecDecision("deferred_qc_dominates")

BugTimeoutUsesMinNotMax ==
  ActualDecision("backlog_timeout_dominates") =
    SpecDecision("backlog_timeout_dominates")

BugQueueActiveNoCap ==
  ActualDecision("queue_active_caps") = SpecDecision("queue_active_caps")

BugQueueInactiveCaps ==
  ActualDecision("queue_inactive_no_cap") = SpecDecision("queue_inactive_no_cap")

BugCapUsesBaseTimeout ==
  ActualDecision("queue_active_caps") = SpecDecision("queue_active_caps")

BugActiveCeilingNoFloor ==
  ActualDecision("active_ceiling_floor") = SpecDecision("active_ceiling_floor")

BugActiveCeilingNoGlobalCap ==
  ActualDecision("active_ceiling_global_cap") =
    SpecDecision("active_ceiling_global_cap")

BugSaturatingMulOverflows ==
  ActualDecision("saturating_mul") = SpecDecision("saturating_mul")

BugSaturatingAddOverflows ==
  ActualDecision("saturating_add") = SpecDecision("saturating_add")

====
