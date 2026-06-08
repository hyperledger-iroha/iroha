---- MODULE SumeragiProposalBudgetGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for proposal-side budget and cap helpers in
`main_loop/propose.rs`.

This slice pins deterministic arithmetic for consensus queue backpressure,
DA payload budget selection, transaction budget caps, fast-finality transaction
and gas caps, and proposal stale-window scaling. The Rust implementation uses
larger platform types and saturating operations; the model keeps a small,
representative domain while preserving the observable branch contracts.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

FastThreshold == 100
RbcMaxTotalChunks == 4
PayloadCapAbsent == 9999
Quantum == 100
MaxMultiplier == 8

QueueCases == {
  "queue_block_cap_floor",
  "queue_rbc_cap_floor",
  "queue_below_caps",
  "queue_at_block_cap",
  "queue_at_rbc_cap"
}

DaCases == {
  "da_chunk_zero_floor",
  "da_payload_cap_wins",
  "da_pending_bytes_wins",
  "da_pending_chunk_floor",
  "da_rbc_budget_wins"
}

TxCases == {
  "tx_no_config_empty_queue",
  "tx_config_caps_param",
  "tx_param_caps_config",
  "tx_queue_caps_target"
}

FastTxCases == {
  "fast_tx_cap_commit_time",
  "fast_tx_cap_effective_time",
  "fast_tx_cap_not_applicable",
  "fast_tx_no_cap"
}

GasCases == {
  "gas_no_base",
  "gas_no_fast_cap",
  "gas_fast_cap_applies",
  "gas_fast_cap_not_applicable"
}

StaleCases == {
  "stale_zero_tx",
  "stale_one_batch",
  "stale_full_batch_grace",
  "stale_capped_large"
}

Min(a, b) == IF a <= b THEN a ELSE b
Max(a, b) == IF a >= b THEN a ELSE b
BoolToInt(b) == IF b THEN 1 ELSE 0

QueueBlockDepth(c) ==
  CASE c = "queue_block_cap_floor" -> 1
    [] c = "queue_below_caps" -> 1
    [] c = "queue_at_block_cap" -> 2
    [] OTHER -> 0

QueueRbcDepth(c) ==
  CASE c = "queue_rbc_cap_floor" -> 1
    [] c = "queue_below_caps" -> 1
    [] c = "queue_at_rbc_cap" -> 2
    [] OTHER -> 0

QueueBlockCap(c) ==
  CASE c = "queue_block_cap_floor" -> 0
    [] c \in {"queue_below_caps", "queue_at_block_cap"} -> 2
    [] OTHER -> 5

QueueRbcCap(c) ==
  CASE c = "queue_rbc_cap_floor" -> 0
    [] c \in {"queue_below_caps", "queue_at_rbc_cap"} -> 2
    [] OTHER -> 5

SpecQueueBackpressure(c) ==
  QueueBlockDepth(c) >= Max(QueueBlockCap(c), 1)
    \/ QueueRbcDepth(c) >= Max(QueueRbcCap(c), 1)

ActualQueueBackpressure(c) ==
  CASE Bug = "queue_zero_block_cap_allows"
       /\ c = "queue_block_cap_floor" -> FALSE
    [] Bug = "queue_zero_rbc_cap_allows"
       /\ c = "queue_rbc_cap_floor" -> FALSE
    [] Bug = "queue_at_block_cap_allows"
       /\ c = "queue_at_block_cap" -> FALSE
    [] Bug = "queue_at_rbc_cap_allows"
       /\ c = "queue_at_rbc_cap" -> FALSE
    [] OTHER -> SpecQueueBackpressure(c)

\* @type: (Str) => <<Int, Int, Int>>;
SpecQueueOutput(c) ==
  <<BoolToInt(SpecQueueBackpressure(c)), Max(QueueBlockCap(c), 1),
    Max(QueueRbcCap(c), 1)>>

\* @type: (Str) => <<Int, Int, Int>>;
ActualQueueOutput(c) ==
  <<BoolToInt(ActualQueueBackpressure(c)), Max(QueueBlockCap(c), 1),
    Max(QueueRbcCap(c), 1)>>

DaChunkMax(c) ==
  CASE c = "da_chunk_zero_floor" -> 0
    [] c = "da_rbc_budget_wins" -> 3
    [] OTHER -> 5

DaPendingMaxBytes(c) ==
  CASE c = "da_chunk_zero_floor" -> 10
    [] c = "da_pending_bytes_wins" -> 7
    [] c \in {"da_pending_chunk_floor", "da_rbc_budget_wins"} -> 50
    [] OTHER -> 50

DaPendingMaxChunks(c) ==
  CASE c = "da_pending_chunk_floor" -> 0
    [] c = "da_rbc_budget_wins" -> 50
    [] OTHER -> 10

DaPayloadCap(c) ==
  CASE c = "da_payload_cap_wins" -> 12
    [] OTHER -> PayloadCapAbsent

DaEffectiveChunk(c) == Max(DaChunkMax(c), 1)

SpecDaRbcBudget(c) ==
  DaEffectiveChunk(c) * RbcMaxTotalChunks

SpecDaPendingBudget(c) ==
  Min(DaPendingMaxBytes(c), DaEffectiveChunk(c) * Max(DaPendingMaxChunks(c), 1))

SpecDaBudget(c) ==
  Min(DaPayloadCap(c), Min(SpecDaRbcBudget(c), SpecDaPendingBudget(c)))

ActualDaRbcBudget(c) ==
  CASE Bug = "da_chunk_zero_not_floored"
       /\ c = "da_chunk_zero_floor" -> 0
    [] OTHER -> SpecDaRbcBudget(c)

ActualDaPendingBudget(c) ==
  CASE Bug = "da_chunk_zero_not_floored"
       /\ c = "da_chunk_zero_floor" ->
         Min(DaPendingMaxBytes(c), DaChunkMax(c) * Max(DaPendingMaxChunks(c), 1))
    [] Bug = "da_ignore_pending_bytes"
       /\ c = "da_pending_bytes_wins" ->
         DaEffectiveChunk(c) * Max(DaPendingMaxChunks(c), 1)
    [] Bug = "da_pending_chunk_zero"
       /\ c = "da_pending_chunk_floor" ->
         Min(DaPendingMaxBytes(c), DaEffectiveChunk(c) * DaPendingMaxChunks(c))
    [] OTHER -> SpecDaPendingBudget(c)

ActualDaBudget(c) ==
  CASE Bug = "da_ignore_payload_cap"
       /\ c = "da_payload_cap_wins" ->
         Min(ActualDaRbcBudget(c), ActualDaPendingBudget(c))
    [] Bug = "da_ignore_rbc_budget"
       /\ c = "da_rbc_budget_wins" ->
         Min(DaPayloadCap(c), ActualDaPendingBudget(c))
    [] OTHER ->
         Min(DaPayloadCap(c), Min(ActualDaRbcBudget(c), ActualDaPendingBudget(c)))

\* @type: (Str) => <<Int, Int, Int, Int>>;
SpecDaOutput(c) ==
  <<SpecDaRbcBudget(c), SpecDaPendingBudget(c), DaPayloadCap(c),
    SpecDaBudget(c)>>

\* @type: (Str) => <<Int, Int, Int, Int>>;
ActualDaOutput(c) ==
  <<ActualDaRbcBudget(c), ActualDaPendingBudget(c), DaPayloadCap(c),
    ActualDaBudget(c)>>

TxQueueLen(c) ==
  CASE c = "tx_no_config_empty_queue" -> 0
    [] c = "tx_queue_caps_target" -> 2
    [] OTHER -> 10

TxParamLimit(c) ==
  CASE c = "tx_param_caps_config" -> 4
    [] OTHER -> 9

TxConfigCap(c) ==
  CASE c = "tx_config_caps_param" -> 3
    [] c = "tx_param_caps_config" -> 8
    [] OTHER -> 0

SpecConfiguredTarget(c) ==
  IF TxConfigCap(c) = 0 THEN TxParamLimit(c)
  ELSE Min(TxConfigCap(c), TxParamLimit(c))

SpecTxMaxInBlock(c) ==
  Max(Min(TxQueueLen(c), SpecConfiguredTarget(c)), 1)

ActualConfiguredTarget(c) ==
  CASE Bug = "tx_ignore_config_cap"
       /\ c = "tx_config_caps_param" -> TxParamLimit(c)
    [] Bug = "tx_ignore_param_cap"
       /\ c = "tx_param_caps_config" -> TxConfigCap(c)
    [] OTHER -> SpecConfiguredTarget(c)

ActualTxMaxInBlock(c) ==
  CASE Bug = "tx_empty_queue_zero"
       /\ c = "tx_no_config_empty_queue" -> 0
    [] OTHER -> Max(Min(TxQueueLen(c), ActualConfiguredTarget(c)), 1)

\* @type: (Str) => <<Int, Int>>;
SpecTxOutput(c) ==
  <<SpecConfiguredTarget(c), SpecTxMaxInBlock(c)>>

\* @type: (Str) => <<Int, Int>>;
ActualTxOutput(c) ==
  <<ActualConfiguredTarget(c), ActualTxMaxInBlock(c)>>

FastTxQueueLen(c) == 20
FastTxParamLimit(c) == 20
FastTxConfigCap(c) == 15

FastTxCommitTime(c) ==
  CASE c \in {"fast_tx_cap_commit_time", "fast_tx_no_cap"} -> 100
    [] OTHER -> 200

FastTxEffectiveTime(c) ==
  CASE c = "fast_tx_cap_effective_time" -> 100
    [] c = "fast_tx_no_cap" -> 100
    [] OTHER -> 200

FastTxCap(c) ==
  IF c = "fast_tx_no_cap" THEN 0 ELSE 6

SpecFastTxConfiguredTarget(c) ==
  Min(FastTxConfigCap(c), FastTxParamLimit(c))

SpecFastCapApplies(c) ==
  FastTxCommitTime(c) <= FastThreshold
    \/ FastTxEffectiveTime(c) <= FastThreshold

SpecFastTxTarget(c) ==
  IF FastTxCap(c) # 0 /\ SpecFastCapApplies(c) THEN
    Min(SpecFastTxConfiguredTarget(c), FastTxCap(c))
  ELSE SpecFastTxConfiguredTarget(c)

SpecFastTxMaxInBlock(c) ==
  Max(Min(FastTxQueueLen(c), SpecFastTxTarget(c)), 1)

SpecFastTxCapped(c) ==
  SpecFastTxTarget(c) < SpecFastTxConfiguredTarget(c)

ActualFastCapApplies(c) ==
  CASE Bug = "fast_tx_ignore_commit_time"
       /\ c = "fast_tx_cap_commit_time" ->
         FastTxEffectiveTime(c) <= FastThreshold
    [] Bug = "fast_tx_ignore_effective_time"
       /\ c = "fast_tx_cap_effective_time" ->
         FastTxCommitTime(c) <= FastThreshold
    [] Bug = "fast_tx_apply_when_slow"
       /\ c = "fast_tx_cap_not_applicable" -> TRUE
    [] OTHER -> SpecFastCapApplies(c)

ActualFastTxTarget(c) ==
  IF FastTxCap(c) # 0 /\ ActualFastCapApplies(c) THEN
    Min(SpecFastTxConfiguredTarget(c), FastTxCap(c))
  ELSE SpecFastTxConfiguredTarget(c)

ActualFastTxMaxInBlock(c) ==
  Max(Min(FastTxQueueLen(c), ActualFastTxTarget(c)), 1)

ActualFastTxCapped(c) ==
  ActualFastTxTarget(c) < SpecFastTxConfiguredTarget(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int>>;
SpecFastTxOutput(c) ==
  <<SpecFastTxConfiguredTarget(c), SpecFastTxTarget(c),
    SpecFastTxMaxInBlock(c), BoolToInt(SpecFastTxCapped(c)),
    BoolToInt(SpecFastCapApplies(c))>>

\* @type: (Str) => <<Int, Int, Int, Int, Int>>;
ActualFastTxOutput(c) ==
  <<SpecFastTxConfiguredTarget(c), ActualFastTxTarget(c),
    ActualFastTxMaxInBlock(c), BoolToInt(ActualFastTxCapped(c)),
    BoolToInt(ActualFastCapApplies(c))>>

GasBase(c) ==
  IF c = "gas_no_base" THEN 0 ELSE 10

GasFastCap(c) ==
  IF c = "gas_no_fast_cap" THEN 0 ELSE 4

GasCommitTime(c) ==
  IF c = "gas_fast_cap_not_applicable" THEN 200 ELSE 100

GasEffectiveTime(c) ==
  IF c = "gas_fast_cap_not_applicable" THEN 200 ELSE 100

SpecGasCapApplies(c) ==
  GasCommitTime(c) <= FastThreshold \/ GasEffectiveTime(c) <= FastThreshold

SpecGasPresent(c) ==
  GasBase(c) # 0

SpecGasLimit(c) ==
  IF GasBase(c) = 0 THEN 0
  ELSE IF GasFastCap(c) = 0 \/ ~SpecGasCapApplies(c) THEN GasBase(c)
  ELSE Min(GasBase(c), GasFastCap(c))

ActualGasCapApplies(c) ==
  CASE Bug = "gas_apply_when_slow"
       /\ c = "gas_fast_cap_not_applicable" -> TRUE
    [] OTHER -> SpecGasCapApplies(c)

ActualGasPresent(c) ==
  CASE Bug = "gas_no_base_returns_fast"
       /\ c = "gas_no_base" -> TRUE
    [] OTHER -> SpecGasPresent(c)

ActualGasLimit(c) ==
  CASE Bug = "gas_no_base_returns_fast"
       /\ c = "gas_no_base" -> GasFastCap(c)
    [] Bug = "gas_ignore_fast_cap"
       /\ c = "gas_fast_cap_applies" -> GasBase(c)
    [] OTHER ->
       IF GasBase(c) = 0 THEN 0
       ELSE IF GasFastCap(c) = 0 \/ ~ActualGasCapApplies(c) THEN GasBase(c)
       ELSE Min(GasBase(c), GasFastCap(c))

\* @type: (Str) => <<Int, Int, Int>>;
SpecGasOutput(c) ==
  <<BoolToInt(SpecGasPresent(c)), SpecGasLimit(c),
    BoolToInt(SpecGasCapApplies(c))>>

\* @type: (Str) => <<Int, Int, Int>>;
ActualGasOutput(c) ==
  <<BoolToInt(ActualGasPresent(c)), ActualGasLimit(c),
    BoolToInt(ActualGasCapApplies(c))>>

StaleBase(c) == 10

StaleTxCount(c) ==
  CASE c = "stale_zero_tx" -> 0
    [] c = "stale_one_batch" -> 50
    [] c = "stale_full_batch_grace" -> 100
    [] c = "stale_capped_large" -> 1000
    [] OTHER -> 0

SpecStaleBatches(c) ==
  CASE c = "stale_zero_tx" -> 0
    [] c \in {"stale_one_batch", "stale_full_batch_grace"} -> 1
    [] c = "stale_capped_large" -> 10
    [] OTHER -> (StaleTxCount(c) + Quantum - 1) \div Quantum

SpecStaleGrace(c) ==
  IF StaleTxCount(c) >= Quantum THEN 1 ELSE 0

SpecStaleMultiplier(c) ==
  Min(Max(SpecStaleBatches(c) + SpecStaleGrace(c), 1), MaxMultiplier)

SpecStaleWindow(c) ==
  StaleBase(c) * SpecStaleMultiplier(c)

ActualStaleGrace(c) ==
  CASE Bug = "stale_missing_full_batch_grace"
       /\ c = "stale_full_batch_grace" -> 0
    [] OTHER -> SpecStaleGrace(c)

ActualStaleMultiplier(c) ==
  CASE Bug = "stale_zero_tx_zero"
       /\ c = "stale_zero_tx" -> 0
    [] Bug = "stale_no_max_cap"
       /\ c = "stale_capped_large" ->
         Max(SpecStaleBatches(c) + ActualStaleGrace(c), 1)
    [] OTHER ->
         Min(Max(SpecStaleBatches(c) + ActualStaleGrace(c), 1), MaxMultiplier)

ActualStaleWindow(c) ==
  StaleBase(c) * ActualStaleMultiplier(c)

\* @type: (Str) => <<Int, Int, Int, Int>>;
SpecStaleOutput(c) ==
  <<SpecStaleBatches(c), SpecStaleGrace(c), SpecStaleMultiplier(c),
    SpecStaleWindow(c)>>

\* @type: (Str) => <<Int, Int, Int, Int>>;
ActualStaleOutput(c) ==
  <<SpecStaleBatches(c), ActualStaleGrace(c), ActualStaleMultiplier(c),
    ActualStaleWindow(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "queue_zero_block_cap_allows",
       "queue_zero_rbc_cap_allows",
       "queue_at_block_cap_allows",
       "queue_at_rbc_cap_allows",
       "da_chunk_zero_not_floored",
       "da_ignore_payload_cap",
       "da_ignore_pending_bytes",
       "da_pending_chunk_zero",
       "da_ignore_rbc_budget",
       "tx_empty_queue_zero",
       "tx_ignore_config_cap",
       "tx_ignore_param_cap",
       "fast_tx_ignore_commit_time",
       "fast_tx_ignore_effective_time",
       "fast_tx_apply_when_slow",
       "gas_no_base_returns_fast",
       "gas_ignore_fast_cap",
       "gas_apply_when_slow",
       "stale_zero_tx_zero",
       "stale_missing_full_batch_grace",
       "stale_no_max_cap"
     }
  /\ checked = 0

ProposalBudgetMatchesSpec ==
  /\ \A c \in QueueCases: ActualQueueOutput(c) = SpecQueueOutput(c)
  /\ \A c \in DaCases: ActualDaOutput(c) = SpecDaOutput(c)
  /\ \A c \in TxCases: ActualTxOutput(c) = SpecTxOutput(c)
  /\ \A c \in FastTxCases: ActualFastTxOutput(c) = SpecFastTxOutput(c)
  /\ \A c \in GasCases: ActualGasOutput(c) = SpecGasOutput(c)
  /\ \A c \in StaleCases: ActualStaleOutput(c) = SpecStaleOutput(c)

SafetyFast ==
  ProposalBudgetMatchesSpec

BugQueueZeroBlockCapAllows ==
  ActualQueueOutput("queue_block_cap_floor") =
    SpecQueueOutput("queue_block_cap_floor")

BugQueueZeroRbcCapAllows ==
  ActualQueueOutput("queue_rbc_cap_floor") =
    SpecQueueOutput("queue_rbc_cap_floor")

BugQueueAtBlockCapAllows ==
  ActualQueueOutput("queue_at_block_cap") =
    SpecQueueOutput("queue_at_block_cap")

BugQueueAtRbcCapAllows ==
  ActualQueueOutput("queue_at_rbc_cap") =
    SpecQueueOutput("queue_at_rbc_cap")

BugDaChunkZeroNotFloored ==
  ActualDaOutput("da_chunk_zero_floor") = SpecDaOutput("da_chunk_zero_floor")

BugDaIgnorePayloadCap ==
  ActualDaOutput("da_payload_cap_wins") = SpecDaOutput("da_payload_cap_wins")

BugDaIgnorePendingBytes ==
  ActualDaOutput("da_pending_bytes_wins") =
    SpecDaOutput("da_pending_bytes_wins")

BugDaPendingChunkZero ==
  ActualDaOutput("da_pending_chunk_floor") =
    SpecDaOutput("da_pending_chunk_floor")

BugDaIgnoreRbcBudget ==
  ActualDaOutput("da_rbc_budget_wins") = SpecDaOutput("da_rbc_budget_wins")

BugTxEmptyQueueZero ==
  ActualTxOutput("tx_no_config_empty_queue") =
    SpecTxOutput("tx_no_config_empty_queue")

BugTxIgnoreConfigCap ==
  ActualTxOutput("tx_config_caps_param") = SpecTxOutput("tx_config_caps_param")

BugTxIgnoreParamCap ==
  ActualTxOutput("tx_param_caps_config") = SpecTxOutput("tx_param_caps_config")

BugFastTxIgnoreCommitTime ==
  ActualFastTxOutput("fast_tx_cap_commit_time") =
    SpecFastTxOutput("fast_tx_cap_commit_time")

BugFastTxIgnoreEffectiveTime ==
  ActualFastTxOutput("fast_tx_cap_effective_time") =
    SpecFastTxOutput("fast_tx_cap_effective_time")

BugFastTxApplyWhenSlow ==
  ActualFastTxOutput("fast_tx_cap_not_applicable") =
    SpecFastTxOutput("fast_tx_cap_not_applicable")

BugGasNoBaseReturnsFast ==
  ActualGasOutput("gas_no_base") = SpecGasOutput("gas_no_base")

BugGasIgnoreFastCap ==
  ActualGasOutput("gas_fast_cap_applies") =
    SpecGasOutput("gas_fast_cap_applies")

BugGasApplyWhenSlow ==
  ActualGasOutput("gas_fast_cap_not_applicable") =
    SpecGasOutput("gas_fast_cap_not_applicable")

BugStaleZeroTxZero ==
  ActualStaleOutput("stale_zero_tx") = SpecStaleOutput("stale_zero_tx")

BugStaleMissingFullBatchGrace ==
  ActualStaleOutput("stale_full_batch_grace") =
    SpecStaleOutput("stale_full_batch_grace")

BugStaleNoMaxCap ==
  ActualStaleOutput("stale_capped_large") =
    SpecStaleOutput("stale_capped_large")

====
