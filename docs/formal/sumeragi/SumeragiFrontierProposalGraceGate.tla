---- MODULE SumeragiFrontierProposalGraceGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for frontier proposal and missing-QC grace helpers.

This slice pins `frontier_proposal_grace_tx_count(...)`,
`proposal_assembly_stale_window(...)`, `frontier_ingress_drain_grace(...)`,
`frontier_full_proposal_grace(...)`,
`initial_frontier_proposal_grace(...)`, and
`frontier_missing_qc_reacquire_window(...)`.

The model keeps duration arithmetic finite with `MaxMillis`; it stands in for
Rust's saturating duration operations while preserving branch behavior.
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
TxQuantum == 128
MaxAssemblyMultiplier == 4
IngressIdleFloor == 750
IngressBacklogCeiling == 250
QueueNudgeMin == 100
SkewGraceFloor == 500
ActiveGapLimit == 5000

TxCountCases == {
  "tx_no_config_world_limit",
  "tx_config_caps_world_limit",
  "tx_world_caps_config_limit",
  "tx_zero_floor",
  "full_config_cap_reduces_assembly",
  "full_world_cap_reduces_assembly"
}

AssemblyCases == {
  "assembly_one_batch",
  "assembly_full_batch_grace",
  "assembly_two_batches",
  "assembly_capped_large",
  "full_config_cap_reduces_assembly",
  "full_world_cap_reduces_assembly"
}

FullBaseCases == {
  "assembly_one_batch",
  "assembly_full_batch_grace",
  "assembly_two_batches",
  "assembly_capped_large",
  "full_zero_quorum_floor",
  "full_uses_ingress",
  "full_active_uses_cooldown",
  "full_config_cap_reduces_assembly",
  "full_world_cap_reduces_assembly",
  "saturating_add_full"
}

IngressCases == {
  "ingress_idle_floor",
  "ingress_active_ceiling",
  "full_uses_ingress",
  "full_active_uses_cooldown",
  "saturating_add_full",
  "saturating_mul_skew"
}

FullProposalGraceCases == {
  "full_zero_quorum_floor",
  "full_uses_ingress",
  "full_active_uses_cooldown",
  "full_config_cap_reduces_assembly",
  "full_world_cap_reduces_assembly",
  "missing_qc_uses_full",
  "saturating_add_full"
}

InitialFrontierGraceCases == {
  "initial_da_sla_uses_proposal",
  "initial_no_da_uses_active_gap",
  "initial_da_no_sla_uses_skew",
  "initial_active_gap_dominates"
}

SkewGraceCases == {
  "initial_da_sla_uses_proposal",
  "initial_no_da_uses_active_gap",
  "initial_da_no_sla_uses_skew",
  "initial_active_gap_dominates",
  "saturating_mul_skew"
}

MissingQcCases == {
  "missing_qc_uses_recovery",
  "missing_qc_uses_full"
}

Min(a, b) == IF a <= b THEN a ELSE b
Max(a, b) == IF a >= b THEN a ELSE b

SaturatingAdd(a, b) ==
  IF a + b >= MaxMillis THEN MaxMillis ELSE a + b

SaturatingMul(value, multiplier) ==
  IF value * multiplier >= MaxMillis THEN MaxMillis ELSE value * multiplier

\* @type: Str => Int;
WorldTxLimit(c) ==
  CASE c = "tx_config_caps_world_limit" -> 200
    [] c = "tx_world_caps_config_limit" -> 50
    [] c = "tx_zero_floor" -> 0
    [] c = "assembly_one_batch" -> 127
    [] c = "assembly_full_batch_grace" -> 128
    [] c = "assembly_two_batches" -> 256
    [] c = "assembly_capped_large" -> 512
    [] c = "full_config_cap_reduces_assembly" -> 512
    [] c = "full_world_cap_reduces_assembly" -> 128
    [] OTHER -> 64

\* @type: Str => Bool;
ConfigTxCapPresent(c) ==
  c \in {
    "tx_config_caps_world_limit",
    "tx_world_caps_config_limit",
    "full_config_cap_reduces_assembly",
    "full_world_cap_reduces_assembly"
  }

\* @type: Str => Int;
ConfigTxCap(c) ==
  CASE c = "tx_config_caps_world_limit" -> 50
    [] c = "tx_world_caps_config_limit" -> 200
    [] c = "full_config_cap_reduces_assembly" -> 128
    [] c = "full_world_cap_reduces_assembly" -> 512
    [] OTHER -> 0

\* @type: Str => Int;
SpecTxCount(c) ==
  Max(
    IF ConfigTxCapPresent(c)
    THEN Min(ConfigTxCap(c), WorldTxLimit(c))
    ELSE WorldTxLimit(c),
    1
  )

\* @type: Str => Int;
QuorumTimeout(c) ==
  CASE c = "full_zero_quorum_floor" -> 0
    [] c = "saturating_add_full" -> 9500
    [] OTHER -> 400

\* @type: Str => Int;
SpecFullBase(c) == Max(QuorumTimeout(c), 1)

\* @type: Str => Int;
SpecAssemblyBatches(c) ==
  CASE c = "assembly_full_batch_grace" -> 1
    [] c = "assembly_two_batches" -> 2
    [] c = "assembly_capped_large" -> 4
    [] c \in {
         "full_config_cap_reduces_assembly",
         "full_world_cap_reduces_assembly"
       } -> 1
    [] OTHER -> 1

\* @type: Str => Int;
SpecAssemblyFullBatchGrace(c) ==
  IF c \in {
    "assembly_full_batch_grace",
    "assembly_two_batches",
    "assembly_capped_large",
    "full_config_cap_reduces_assembly",
    "full_world_cap_reduces_assembly"
  }
  THEN 1
  ELSE 0

\* @type: Str => Int;
SpecAssemblyMultiplier(c) ==
  Min(
    Max(SpecAssemblyBatches(c) + SpecAssemblyFullBatchGrace(c), 1),
    MaxAssemblyMultiplier
  )

\* @type: Str => Int;
SpecAssemblyWindow(c) ==
  SaturatingMul(SpecFullBase(c), SpecAssemblyMultiplier(c))

\* @type: Str => Int;
RebroadcastCooldown(c) ==
  CASE c = "full_active_uses_cooldown" -> 900
    [] c = "saturating_mul_skew" -> 3000
    [] OTHER -> 100

\* @type: Str => Bool;
QueueActiveBacklog(c) ==
  c \in {"ingress_active_ceiling", "full_active_uses_cooldown"}

\* @type: Str => Int;
SpecIngressIdleGrace(c) ==
  Max(SaturatingMul(RebroadcastCooldown(c), 5), IngressIdleFloor)

\* @type: Str => Int;
SpecIngressDrainGrace(c) ==
  IF QueueActiveBacklog(c)
  THEN Max(Min(SpecIngressIdleGrace(c), IngressBacklogCeiling), QueueNudgeMin)
  ELSE SpecIngressIdleGrace(c)

\* @type: Str => Int;
SpecFullProposalGrace(c) ==
  Max(
    SaturatingAdd(
      SpecAssemblyWindow(c),
      Max(SpecIngressDrainGrace(c), RebroadcastCooldown(c))
    ),
    1
  )

\* @type: Str => Int;
SpecBoundedSkewGrace(c) ==
  Max(SaturatingMul(RebroadcastCooldown(c), 4), SkewGraceFloor)

\* @type: Str => Bool;
DaEnabled(c) ==
  c # "initial_no_da_uses_active_gap"

\* @type: Str => Bool;
ActiveBlockProductionSla(c) ==
  c \in {
    "initial_da_sla_uses_proposal",
    "initial_no_da_uses_active_gap",
    "initial_active_gap_dominates"
  }

\* @type: Str => Int;
ConfigCommitInflightTimeout(c) ==
  CASE c = "initial_da_sla_uses_proposal" -> 1000
    [] c = "initial_no_da_uses_active_gap" -> 1000
    [] c = "initial_active_gap_dominates" -> 2000
    [] OTHER -> ActiveGapLimit

\* @type: Str => Int;
SpecActiveGapCeiling(c) ==
  Max(Min(ConfigCommitInflightTimeout(c), ActiveGapLimit), 1)

\* @type: Str => Int;
SpecInitialProposalGraceTerm(c) ==
  IF DaEnabled(c) /\ ActiveBlockProductionSla(c)
  THEN SpecFullProposalGrace(c)
  ELSE 0

\* @type: Str => Int;
SpecInitialFrontierGraceUncapped(c) ==
  Max(SpecBoundedSkewGrace(c), SpecInitialProposalGraceTerm(c))

\* @type: Str => Int;
SpecInitialFrontierProposalGrace(c) ==
  IF ActiveBlockProductionSla(c)
  THEN Max(SpecInitialFrontierGraceUncapped(c), SpecActiveGapCeiling(c))
  ELSE SpecInitialFrontierGraceUncapped(c)

\* @type: Str => Int;
RecoveryMissingQcReacquireWindow(c) ==
  CASE c = "missing_qc_uses_recovery" -> 3000
    [] c = "missing_qc_uses_full" -> 500
    [] OTHER -> 500

\* @type: Str => Int;
SpecMissingQcReacquireWindow(c) ==
  Max(RecoveryMissingQcReacquireWindow(c), SpecFullProposalGrace(c))

\* @type: Str => Int;
ActualTxCount(c) ==
  CASE Bug = "tx_count_ignores_config_cap"
       /\ c = "tx_config_caps_world_limit" -> Max(WorldTxLimit(c), 1)
    [] Bug = "tx_count_ignores_world_limit"
       /\ c = "tx_world_caps_config_limit" -> Max(ConfigTxCap(c), 1)
    [] Bug = "tx_count_no_floor"
       /\ c = "tx_zero_floor" ->
          IF ConfigTxCapPresent(c)
          THEN Min(ConfigTxCap(c), WorldTxLimit(c))
          ELSE WorldTxLimit(c)
    [] OTHER -> SpecTxCount(c)

\* @type: Str => Int;
ActualFullBase(c) ==
  CASE Bug = "full_omits_base_floor"
       /\ c = "full_zero_quorum_floor" -> QuorumTimeout(c)
    [] OTHER -> SpecFullBase(c)

\* @type: Str => Int;
ActualAssemblyBatches(c) ==
  CASE Bug = "full_ignores_config_tx_cap"
       /\ c = "full_config_cap_reduces_assembly" -> 4
    [] Bug = "full_ignores_world_tx_limit"
       /\ c = "full_world_cap_reduces_assembly" -> 4
    [] OTHER -> SpecAssemblyBatches(c)

\* @type: Str => Int;
ActualAssemblyFullBatchGrace(c) ==
  SpecAssemblyFullBatchGrace(c)

\* @type: Str => Int;
ActualAssemblyMultiplier(c) ==
  CASE Bug = "assembly_missing_full_batch_grace"
       /\ c = "assembly_full_batch_grace" ->
          Min(Max(ActualAssemblyBatches(c), 1), MaxAssemblyMultiplier)
    [] Bug = "assembly_no_multiplier_cap"
       /\ c = "assembly_capped_large" ->
          Max(ActualAssemblyBatches(c) + ActualAssemblyFullBatchGrace(c), 1)
    [] OTHER ->
          Min(
            Max(ActualAssemblyBatches(c) + ActualAssemblyFullBatchGrace(c), 1),
            MaxAssemblyMultiplier
          )

\* @type: Str => Int;
ActualAssemblyWindow(c) ==
  SaturatingMul(ActualFullBase(c), ActualAssemblyMultiplier(c))

\* @type: Str => Int;
ActualIngressIdleGrace(c) ==
  CASE Bug = "ingress_omits_idle_floor"
       /\ c = "ingress_idle_floor" -> SaturatingMul(RebroadcastCooldown(c), 5)
    [] OTHER -> SpecIngressIdleGrace(c)

\* @type: Str => Int;
ActualIngressDrainGrace(c) ==
  IF QueueActiveBacklog(c)
  THEN
    CASE Bug = "ingress_active_omits_ceiling"
         /\ c = "ingress_active_ceiling" ->
            Max(ActualIngressIdleGrace(c), QueueNudgeMin)
      [] OTHER ->
            Max(Min(ActualIngressIdleGrace(c), IngressBacklogCeiling), QueueNudgeMin)
  ELSE ActualIngressIdleGrace(c)

\* @type: Str => Int;
ActualFullProposalGrace(c) ==
  CASE Bug = "full_omits_ingress_grace"
       /\ c = "full_uses_ingress" -> Max(ActualAssemblyWindow(c), 1)
    [] Bug = "full_uses_min_ingress_cooldown"
       /\ c = "full_active_uses_cooldown" ->
          Max(
            SaturatingAdd(
              ActualAssemblyWindow(c),
              Min(ActualIngressDrainGrace(c), RebroadcastCooldown(c))
            ),
            1
          )
    [] Bug = "saturating_add_full_overflows"
       /\ c = "saturating_add_full" ->
          Max(
            ActualAssemblyWindow(c)
              + Max(ActualIngressDrainGrace(c), RebroadcastCooldown(c)),
            1
          )
    [] OTHER ->
          Max(
            SaturatingAdd(
              ActualAssemblyWindow(c),
              Max(ActualIngressDrainGrace(c), RebroadcastCooldown(c))
            ),
            1
          )

\* @type: Str => Int;
ActualBoundedSkewGrace(c) ==
  CASE Bug = "skew_omits_floor"
       /\ c = "initial_da_no_sla_uses_skew" ->
          SaturatingMul(RebroadcastCooldown(c), 4)
    [] Bug = "saturating_mul_skew_overflows"
       /\ c = "saturating_mul_skew" ->
          RebroadcastCooldown(c) * 4
    [] OTHER -> SpecBoundedSkewGrace(c)

\* @type: Str => Int;
ActualInitialProposalGraceTerm(c) ==
  CASE Bug = "initial_ignores_da_gate"
       /\ c = "initial_no_da_uses_active_gap" ->
          IF ActiveBlockProductionSla(c) THEN ActualFullProposalGrace(c) ELSE 0
    [] Bug = "initial_ignores_sla_gate_for_proposal"
       /\ c = "initial_da_no_sla_uses_skew" ->
          IF DaEnabled(c) THEN ActualFullProposalGrace(c) ELSE 0
    [] OTHER ->
          IF DaEnabled(c) /\ ActiveBlockProductionSla(c)
          THEN ActualFullProposalGrace(c)
          ELSE 0

\* @type: Str => Int;
ActualInitialFrontierGraceUncapped(c) ==
  CASE Bug = "initial_uses_min_skew_proposal"
       /\ c = "initial_da_sla_uses_proposal" ->
          Min(ActualBoundedSkewGrace(c), ActualInitialProposalGraceTerm(c))
    [] OTHER ->
          Max(ActualBoundedSkewGrace(c), ActualInitialProposalGraceTerm(c))

\* @type: Str => Int;
ActualInitialFrontierProposalGrace(c) ==
  CASE Bug = "initial_omits_active_gap_cap"
       /\ c = "initial_active_gap_dominates" ->
          ActualInitialFrontierGraceUncapped(c)
    [] ActiveBlockProductionSla(c) ->
          Max(ActualInitialFrontierGraceUncapped(c), SpecActiveGapCeiling(c))
    [] OTHER -> ActualInitialFrontierGraceUncapped(c)

\* @type: Str => Int;
ActualMissingQcReacquireWindow(c) ==
  CASE Bug = "missing_qc_uses_min"
       /\ c = "missing_qc_uses_recovery" ->
          Min(RecoveryMissingQcReacquireWindow(c), ActualFullProposalGrace(c))
    [] OTHER ->
          Max(RecoveryMissingQcReacquireWindow(c), ActualFullProposalGrace(c))

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "tx_count_ignores_config_cap",
       "tx_count_ignores_world_limit",
       "tx_count_no_floor",
       "assembly_missing_full_batch_grace",
       "assembly_no_multiplier_cap",
       "ingress_omits_idle_floor",
       "ingress_active_omits_ceiling",
       "full_omits_base_floor",
       "full_omits_ingress_grace",
       "full_uses_min_ingress_cooldown",
       "full_ignores_config_tx_cap",
       "full_ignores_world_tx_limit",
       "initial_ignores_da_gate",
       "initial_ignores_sla_gate_for_proposal",
       "initial_omits_active_gap_cap",
       "initial_uses_min_skew_proposal",
       "skew_omits_floor",
       "missing_qc_uses_min",
       "saturating_add_full_overflows",
       "saturating_mul_skew_overflows"
     }
  /\ checked = 0

TxCountMatchesSpec ==
  \A c \in TxCountCases:
    ActualTxCount(c) = SpecTxCount(c)

FullBaseMatchesSpec ==
  \A c \in FullBaseCases:
    ActualFullBase(c) = SpecFullBase(c)

AssemblyMultiplierMatchesSpec ==
  \A c \in AssemblyCases:
    ActualAssemblyMultiplier(c) = SpecAssemblyMultiplier(c)

AssemblyWindowMatchesSpec ==
  \A c \in AssemblyCases:
    ActualAssemblyWindow(c) = SpecAssemblyWindow(c)

IngressIdleGraceMatchesSpec ==
  \A c \in IngressCases:
    ActualIngressIdleGrace(c) = SpecIngressIdleGrace(c)

IngressDrainGraceMatchesSpec ==
  \A c \in IngressCases:
    ActualIngressDrainGrace(c) = SpecIngressDrainGrace(c)

FullProposalGraceMatchesSpec ==
  \A c \in FullProposalGraceCases:
    ActualFullProposalGrace(c) = SpecFullProposalGrace(c)

BoundedSkewGraceMatchesSpec ==
  \A c \in SkewGraceCases:
    ActualBoundedSkewGrace(c) = SpecBoundedSkewGrace(c)

InitialProposalTermMatchesSpec ==
  \A c \in InitialFrontierGraceCases:
    ActualInitialProposalGraceTerm(c) = SpecInitialProposalGraceTerm(c)

InitialFrontierUncappedMatchesSpec ==
  \A c \in InitialFrontierGraceCases:
    ActualInitialFrontierGraceUncapped(c) =
      SpecInitialFrontierGraceUncapped(c)

InitialFrontierGraceMatchesSpec ==
  \A c \in InitialFrontierGraceCases:
    ActualInitialFrontierProposalGrace(c) =
      SpecInitialFrontierProposalGrace(c)

MissingQcReacquireMatchesSpec ==
  \A c \in MissingQcCases:
    ActualMissingQcReacquireWindow(c) = SpecMissingQcReacquireWindow(c)

TxCountAnchors ==
  /\ SpecTxCount("tx_config_caps_world_limit") = 50
  /\ SpecTxCount("tx_world_caps_config_limit") = 50
  /\ SpecTxCount("tx_zero_floor") = 1
  /\ SpecTxCount("tx_no_config_world_limit") = 64
  /\ SpecTxCount("full_config_cap_reduces_assembly") = 128
  /\ SpecTxCount("full_world_cap_reduces_assembly") = 128

AssemblyAnchors ==
  /\ SpecAssemblyMultiplier("assembly_one_batch") = 1
  /\ SpecAssemblyMultiplier("assembly_full_batch_grace") = 2
  /\ SpecAssemblyMultiplier("assembly_two_batches") = 3
  /\ SpecAssemblyMultiplier("assembly_capped_large") = MaxAssemblyMultiplier
  /\ SpecAssemblyWindow("assembly_full_batch_grace") = 800
  /\ SpecAssemblyWindow("assembly_capped_large") = 1600
  /\ SpecAssemblyMultiplier("full_config_cap_reduces_assembly") = 2
  /\ SpecAssemblyMultiplier("full_world_cap_reduces_assembly") = 2
  /\ SpecAssemblyWindow("full_config_cap_reduces_assembly") = 800
  /\ SpecAssemblyWindow("full_world_cap_reduces_assembly") = 800

IngressAnchors ==
  /\ SpecIngressDrainGrace("ingress_idle_floor") = IngressIdleFloor
  /\ SpecIngressDrainGrace("ingress_active_ceiling") = IngressBacklogCeiling

FullProposalGraceAnchors ==
  /\ SpecFullProposalGrace("full_zero_quorum_floor") = 751
  /\ SpecFullProposalGrace("full_uses_ingress") = 1150
  /\ SpecFullProposalGrace("full_active_uses_cooldown") = 1300
  /\ SpecFullProposalGrace("full_config_cap_reduces_assembly") = 1550
  /\ SpecFullProposalGrace("full_world_cap_reduces_assembly") = 1550

FullProposalTxBudgetMatchesSpec ==
  /\ ActualFullProposalGrace("full_config_cap_reduces_assembly") =
       SpecFullProposalGrace("full_config_cap_reduces_assembly")
  /\ ActualFullProposalGrace("full_world_cap_reduces_assembly") =
       SpecFullProposalGrace("full_world_cap_reduces_assembly")

InitialFrontierGraceAnchors ==
  /\ SpecInitialFrontierProposalGrace("initial_da_sla_uses_proposal") = 1150
  /\ SpecInitialFrontierProposalGrace("initial_no_da_uses_active_gap") = 1000
  /\ SpecInitialFrontierProposalGrace("initial_da_no_sla_uses_skew") = SkewGraceFloor
  /\ SpecInitialFrontierProposalGrace("initial_active_gap_dominates") = 2000

MissingQcAnchors ==
  /\ SpecMissingQcReacquireWindow("missing_qc_uses_recovery") = 3000
  /\ SpecMissingQcReacquireWindow("missing_qc_uses_full") = 1150

SaturatingArithmeticAnchors ==
  /\ SpecFullProposalGrace("saturating_add_full") = MaxMillis
  /\ SpecBoundedSkewGrace("saturating_mul_skew") = MaxMillis

FrontierProposalTxBudgetExact ==
  /\ TxCountMatchesSpec
  /\ TxCountAnchors

FrontierProposalAssemblyExact ==
  /\ FullBaseMatchesSpec
  /\ AssemblyMultiplierMatchesSpec
  /\ AssemblyWindowMatchesSpec
  /\ AssemblyAnchors

FrontierProposalIngressExact ==
  /\ IngressIdleGraceMatchesSpec
  /\ IngressDrainGraceMatchesSpec
  /\ IngressAnchors

FrontierFullProposalGraceExact ==
  /\ FullProposalGraceMatchesSpec
  /\ FullProposalGraceAnchors
  /\ FullProposalTxBudgetMatchesSpec

FrontierInitialProposalGraceExact ==
  /\ BoundedSkewGraceMatchesSpec
  /\ InitialProposalTermMatchesSpec
  /\ InitialFrontierUncappedMatchesSpec
  /\ InitialFrontierGraceMatchesSpec
  /\ InitialFrontierGraceAnchors

FrontierMissingQcReacquireExact ==
  /\ MissingQcReacquireMatchesSpec
  /\ MissingQcAnchors

FrontierProposalSaturatingArithmeticExact ==
  /\ SaturatingArithmeticAnchors

FrontierProposalGraceExactness ==
  /\ TxCountMatchesSpec
  /\ FullBaseMatchesSpec
  /\ AssemblyMultiplierMatchesSpec
  /\ AssemblyWindowMatchesSpec
  /\ IngressIdleGraceMatchesSpec
  /\ IngressDrainGraceMatchesSpec
  /\ FullProposalGraceMatchesSpec
  /\ BoundedSkewGraceMatchesSpec
  /\ InitialProposalTermMatchesSpec
  /\ InitialFrontierUncappedMatchesSpec
  /\ InitialFrontierGraceMatchesSpec
  /\ MissingQcReacquireMatchesSpec
  /\ TxCountAnchors
  /\ AssemblyAnchors
  /\ IngressAnchors
  /\ FullProposalGraceAnchors
  /\ FullProposalTxBudgetMatchesSpec
  /\ InitialFrontierGraceAnchors
  /\ MissingQcAnchors
  /\ SaturatingArithmeticAnchors

SafetyFast ==
  FrontierProposalGraceExactness

BugTxCountIgnoresConfigCap ==
  ActualTxCount("tx_config_caps_world_limit") =
    SpecTxCount("tx_config_caps_world_limit")

BugTxCountIgnoresWorldLimit ==
  ActualTxCount("tx_world_caps_config_limit") =
    SpecTxCount("tx_world_caps_config_limit")

BugTxCountNoFloor ==
  ActualTxCount("tx_zero_floor") = SpecTxCount("tx_zero_floor")

BugAssemblyMissingFullBatchGrace ==
  ActualAssemblyMultiplier("assembly_full_batch_grace") =
    SpecAssemblyMultiplier("assembly_full_batch_grace")

BugAssemblyNoMultiplierCap ==
  ActualAssemblyMultiplier("assembly_capped_large") =
    SpecAssemblyMultiplier("assembly_capped_large")

BugIngressOmitsIdleFloor ==
  ActualIngressDrainGrace("ingress_idle_floor") =
    SpecIngressDrainGrace("ingress_idle_floor")

BugIngressActiveOmitsCeiling ==
  ActualIngressDrainGrace("ingress_active_ceiling") =
    SpecIngressDrainGrace("ingress_active_ceiling")

BugFullOmitsBaseFloor ==
  ActualFullProposalGrace("full_zero_quorum_floor") =
    SpecFullProposalGrace("full_zero_quorum_floor")

BugFullOmitsIngressGrace ==
  ActualFullProposalGrace("full_uses_ingress") =
    SpecFullProposalGrace("full_uses_ingress")

BugFullUsesMinIngressCooldown ==
  ActualFullProposalGrace("full_active_uses_cooldown") =
    SpecFullProposalGrace("full_active_uses_cooldown")

BugFullIgnoresConfigTxCap ==
  ActualFullProposalGrace("full_config_cap_reduces_assembly") =
    SpecFullProposalGrace("full_config_cap_reduces_assembly")

BugFullIgnoresWorldTxLimit ==
  ActualFullProposalGrace("full_world_cap_reduces_assembly") =
    SpecFullProposalGrace("full_world_cap_reduces_assembly")

BugInitialIgnoresDaGate ==
  ActualInitialFrontierProposalGrace("initial_no_da_uses_active_gap") =
    SpecInitialFrontierProposalGrace("initial_no_da_uses_active_gap")

BugInitialIgnoresSlaGateForProposal ==
  ActualInitialFrontierProposalGrace("initial_da_no_sla_uses_skew") =
    SpecInitialFrontierProposalGrace("initial_da_no_sla_uses_skew")

BugInitialOmitsActiveGapCap ==
  ActualInitialFrontierProposalGrace("initial_active_gap_dominates") =
    SpecInitialFrontierProposalGrace("initial_active_gap_dominates")

BugInitialUsesMinSkewProposal ==
  ActualInitialFrontierProposalGrace("initial_da_sla_uses_proposal") =
    SpecInitialFrontierProposalGrace("initial_da_sla_uses_proposal")

BugSkewOmitsFloor ==
  ActualBoundedSkewGrace("initial_da_no_sla_uses_skew") =
    SpecBoundedSkewGrace("initial_da_no_sla_uses_skew")

BugMissingQcUsesMin ==
  ActualMissingQcReacquireWindow("missing_qc_uses_recovery") =
    SpecMissingQcReacquireWindow("missing_qc_uses_recovery")

BugSaturatingAddFullOverflows ==
  ActualFullProposalGrace("saturating_add_full") =
    SpecFullProposalGrace("saturating_add_full")

BugSaturatingMulSkewOverflows ==
  ActualBoundedSkewGrace("saturating_mul_skew") =
    SpecBoundedSkewGrace("saturating_mul_skew")

====
