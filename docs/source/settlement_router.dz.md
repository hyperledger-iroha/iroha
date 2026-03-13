---
lang: dz
direction: ltr
source: docs/source/settlement_router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: de86613366ad63e3bd17bed5652fa3a728708512fdfc0678c8b0418001a9603a
source_last_modified: "2025-12-29T18:16:36.086394+00:00"
translation_last_reviewed: 2026-02-07
---

# Settlement Router

The settlement router enforces a single, deterministic path for translating
dataspace-specific gas tokens into XOR liabilities. Every lane follows the same
calculation pipeline so operators can reconcile buffers, swaps, and receipts
without per-lane heuristics. This document explains the architecture that backs
roadmap item **NX-3 Unified lane settlement & XOR conversion** and captures the
runbook expected from operators and integrators.

NX-3 is shipped; the live operator runbook and telemetry guide live in
`docs/settlement-router.md`. Keep this note aligned with that reference when
router behaviour changes.

## Goals

- Convert local gas fees into XOR using a canonical 60 s TWAP, safety margin,
  and liquidity haircut so block producers never diverge on the XOR due.
- Enforce buffer policies (alert below 75 %, throttle below 25 %, XOR-only at
  10 %, halt at 2 %) before a dataspace is allowed to consume subsidised
  capacity, with explicit soft vs hard breach signals.
- Provide deterministic swap-line plumbing so repo/reverse-repo facilities can
  top up XOR buffers when the public AMM/CLMM depth is insufficient.
- Emit receipts, telemetry, and nightly reconciliation artefacts for every lane
  so auditors have a tamper-evident record of each conversion.

## Components

### Configuration and inputs

`SettlementConfig` (`crates/settlement_router/src/config.rs`) carries the
roadmap defaults:

- `twap_window`: 60 s TWAP window exported by the canonical oracle.
- `epsilon`: 25 bps safety margin that ensures inclusion always over-collects.
- `buffer_horizon_hours`: 72 h of spend that the XOR buffer must cover.

Per-dataspace gas assets specify `twap_local_per_xor` and a
`liquidity_profile`, feeding the router via the lane gas metadata
(`crates/iroha_config/src/parameters/user.rs`). Liquidity profiles (`tier1`
deep, `tier2` medium, `tier3` thin) map to default haircuts of 0/25/75 bps with
optional governance overrides (`crates/settlement_router/src/haircut.rs`).

### Shadow-price calculation

`ShadowPriceCalculator` consumes the config + liquidity tier and produces two
numbers per transaction (`crates/settlement_router/src/price.rs`):

1. `xor_due`: micro-XOR debited immediately from the dataspace buffer.
2. `xor_with_haircut`: expected XOR after applying the liquidity haircut.

All arithmetic uses `rust_decimal::Decimal` and rounds up (`ceil`) so the
results are deterministic. The `SettlementEngine` façade in
`crates/iroha_core/src/settlement/mod.rs` injects the caller-provided `source_id`
(normally the transaction hash), timestamps the quote, and returns a
`SettlementReceipt` (`crates/settlement_router/src/receipt.rs`).

### Buffer policy

The router tracks buffer capacity and enforcement thresholds through
`BufferPolicy` (`crates/settlement_router/src/policy.rs`). Runtime code queries
`BufferPolicy::evaluate` (Normal/Alert/Throttle/XorOnly/Halt) or the convenience
`is_soft_breached`/`is_hard_breached` helpers before admitting a transaction:

- Soft breach (Alert/Throttle) ⇒ emit alerts + telemetry and request treasury
  attention.
- Hard breach (XorOnly/Halt) ⇒ lane is forced into XOR-only mode until the
  buffer recovers.

Default thresholds mirror the roadmap (alert below 75 %, throttle below 25 %,
XOR-only below 10 %, halt below 2 % of configured capacity).

### Swap lines and repo facilities

`swapline.rs` models repo/reverse-repo style facilities that supplement AMM
liquidity. Each `SwapLineConfig` declares:

- XOR limit (`limit_xor`) and utilisation ratio.
- Collateral type + haircut (CBDC, XOR, stablecoin).
- Fee or profit-share schedule (`fee_rate_bps`) with a flag for
  Sharia-compliant fee-only models.

`SwapLineExposure` tracks outstanding balances and posted collateral so runtime
code can call `is_healthy` before drawing additional liquidity. These helpers
will back the treasury automation that tops up XOR buffers whenever AMM depth
is insufficient, as described in the NX-3 roadmap entry.

### Receipts, commitments, and reporting

Block builders record pending receipts through `SettlementAccumulator`
(`crates/iroha_core/src/settlement/mod.rs`) keyed by transaction hash. During
`BlockBuilder::finalize` the accumulator drains into per-lane builders
(`crates/iroha_core/src/block.rs:3124-3401`), producing a list of
`LaneSettlementReceipt`s that are embedded in `LaneBlockCommitment`. The latest
commitments are exposed through `/v2/sumeragi/status` as
`lane_settlement_commitments` (`crates/iroha_core/src/sumeragi/status.rs`),
mirroring the summary described in `docs/source/nexus_fee_model.md`.

Every receipt contains:

- `source_id`, `local_amount_micro`, `xor_due`, `xor_after_haircut`, and `xor_variance_micro` (the safety margin consumed per transaction).
- UTC millisecond timestamp (rounded for deterministic serialisation).
- Liquidity profile, epsilon, and TWAP parameters recorded in the builder so
  nightly reconciliation can reconstruct the quote inputs.

### Telemetry and metrics

The current telemetry surface emits the settlement lifecycle counters documented
in `docs/source/telemetry.md`:

- `iroha_settlement_events_total{kind,outcome,reason}`
- `iroha_settlement_finality_events_total{kind,outcome,final_state}`
- `iroha_settlement_fx_window_ms{order,atomicity}` for PvP FX windows.

Per-lane settlement snapshots now publish buffer and swap-line gauges via
`Metrics::record_lane_settlement_snapshot`
(`crates/iroha_telemetry/src/metrics.rs`):

- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`,
  `iroha_settlement_buffer_status`
- `iroha_settlement_pnl_xor`, `iroha_haircut_bp`,
  `iroha_swapline_utilisation`

These gauges carry the per-dataspace buffer headroom, cumulative safety-margin
spend, active haircut tier, and swap-line utilisation so SRE can alert before
gating occurs. Dashboards and Alertmanager rules should source these metrics
alongside the lifecycle counters.

### Swap execution path (AMM / RFQ / swapline top-ups)

The router records deterministic swap metadata in `LaneSwapMetadata` via
`SwapEvidence` (`crates/iroha_core/src/fees/mod.rs`), attaching it to every
`LaneBlockCommitment`:

- Epsilon (bps), TWAP window (seconds), liquidity profile, volatility class
- Canonical TWAP (`twap_local_per_xor`) as a normalised decimal string

Runtime swap execution may select the canonical AMM/CLMM path, a governed RFQ,
or a treasury swapline top-up when buffers fall. When adding those hooks:

- Populate the chosen path in the operator evidence bundle and store the lane
  commitment excerpt with `LaneSwapMetadata`.
- Keep swapline bring-up/unwind evidence under
  `artifacts/settlement/swaplines/` as described in
  `docs/source/settlement_swapline_runbook.md`.
- Alert on utilisation/buffer gauges before engaging swaplines; record the
  volatility class used to justify any epsilon increase.

## Operator workflow

1. **Quote validation** - Confirm each dataspace publishes a 60s TWAP and a
   `liquidity_profile`. The settlement router rejects a zero TWAP to avoid
   undefined conversion ratios.
2. **Receipt reconciliation** - Pull `lane_settlement_commitments` from
   `/v2/sumeragi/status` or the nightly export, verify that the sums of
   `xor_due_micro` and `xor_after_haircut_micro` match ledger expectations, and
   archive the attached parameters.
3. **Buffer monitoring** - Track the buffer soft/hard thresholds via telemetry.
   When the soft alert triggers, notify treasury and consider drawing on an
   approved swap line; when the hard threshold triggers, enforce XOR-only
   inclusion and log the incident.
4. **Swap-line health** - Ensure outstanding swaps respect the configured limit
   and collateral ratio by inspecting `SwapLineExposure::is_healthy` results
   (exposed via future telemetry). Automated liquidation hooks must top up or
   wind down unhealthy facilities before accrual breaches governance policy.
   Track utilisation via `iroha_swapline_utilisation` and archive activation or
   unwind bundles per `docs/source/settlement_swapline_runbook.md`.
5. **Audit artefacts** - Attach the signed `LaneBlockCommitment` receipts,
   operator dashboards, and swap-line logs to the reconciliation package so
   regulators can replay the deterministic calculation.

## Status

The NX-3 settlement router is live: swap metadata and per-transaction receipts
are embedded in `LaneBlockCommitment`, exposed through `/v2/sumeragi/status`,
and mirrored into the settlement telemetry surface (buffer headroom, haircut
tiers, swap utilisation) for dashboards and alerts. Operators should use this
document as the reconciler’s guide; no roadmap items remain open for NX-3.
