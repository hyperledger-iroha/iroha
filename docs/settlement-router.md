# Deterministic Settlement Router (NX-3)

**Status:** Completed (NX-3)  
**Owners:** Economics WG / Core Ledger WG / Treasury / SRE  
**Scope:** Canonical XOR settlement path used by all lanes/dataspaces. Shipped router crate, lane-level receipts, buffer guard rails, telemetry, and operator evidence surfaces.

## Goals
- Unify XOR conversion and receipt generation across single-lane and Nexus builds.
- Apply deterministic haircuts + volatility margins with guard-railed buffers so operators can pace settlement safely.
- Expose receipts, telemetry, and dashboards that auditors can replay without bespoke tooling.

## Architecture
| Component | Location | Responsibility |
|-----------|----------|----------------|
| Router primitives | `crates/settlement_router/` | Shadow-price calculator, haircut tiers, buffer policy helpers, settlement receipt type.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】【crates/settlement_router/src/policy.rs:1】 |
| Runtime façade | `crates/iroha_core/src/settlement/mod.rs:1` | Wraps router config into `SettlementEngine`, exposes `quote` + accumulator used during block execution. |
| Block integration | `crates/iroha_core/src/block.rs:120` | Drains `PendingSettlement` records, aggregates `LaneSettlementCommitment` per lane/dataspace, parses lane buffer metadata, and emits telemetry. |
| Telemetry & dashboards | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` | Prometheus/OTLP metrics for buffers, variance, haircuts, conversion counts; Grafana board for SRE. |
| Reference schema | `docs/source/nexus_fee_model.md:1` | Documents settlement receipt fields persisted in `LaneBlockCommitment`. |

## Configuration
Router knobs live under `[settlement.router]` (validated by `iroha_config`):

```toml
[settlement.router]
twap_window_seconds = 60      # TWAP window used to derive local→XOR conversions
epsilon_bps = 25              # Base margin added to every quote (basis points)
buffer_alert_pct = 75         # Remaining-buffer % that opens an alert
buffer_throttle_pct = 25      # Remaining-buffer % where throttling begins
buffer_xor_only_pct = 10      # Remaining-buffer % where XOR-only mode is enforced
buffer_halt_pct = 2           # Remaining-buffer % where settlement halts
buffer_horizon_hours = 72     # Horizon (hours) represented by the XOR buffer
```

Lane metadata wires in the per-dataspace buffer account:
- `settlement.buffer_account` — account that holds the reserve (e.g., `buffer::cbdc_treasury`).
- `settlement.buffer_asset` — asset definition debited for headroom (typically `xor#sora`).
- `settlement.buffer_capacity_micro` — configured capacity in micro-XOR (decimal string).

Absent metadata disables buffer snapshotting for that lane (telemetry falls back to zeroed capacity/status).

## Conversion Pipeline
1. **Quote:** `SettlementEngine::quote` applies the configured epsilon + volatility margin and haircut tier to TWAP quotes, returning a `SettlementReceipt` with `xor_due` and `xor_after_haircut` plus the timestamp and caller-supplied `source_id`.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **Accumulate:** During block execution the executor records `PendingSettlement` entries (local amount, TWAP, epsilon, volatility bucket, liquidity profile, oracle timestamp). `LaneSettlementBuilder` aggregates totals and swap metadata per `(lane, dataspace)` before sealing the block.【crates/iroha_core/src/settlement/mod.rs:34】【crates/iroha_core/src/block.rs:3460】
3. **Buffer snapshot:** If lane metadata declares a buffer, the builder captures a `SettlementBufferSnapshot` (remaining headroom, capacity, status) using the `BufferPolicy` thresholds from config.【crates/iroha_core/src/block.rs:203】
4. **Commit + telemetry:** Receipts and swap evidence land inside `LaneBlockCommitment` and are mirrored into status snapshots. Telemetry records buffer gauges, variance (`iroha_settlement_pnl_xor`), applied margin (`iroha_settlement_haircut_bp`), optional swapline utilisation, and per-asset conversion/haircut counters so dashboards and alerts stay in sync with the block contents.【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **Evidence surfaces:** `status::set_lane_settlement_commitments` publishes commitments for relays/DA consumers, Grafana dashboards read the Prometheus metrics, and operators use `ops/runbooks/settlement-buffers.md` alongside `dashboards/grafana/settlement_router_overview.json` to track refill/throttle events.

## Telemetry & Evidence
- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`, `iroha_settlement_buffer_status` — buffer snapshot per lane/dataspace (micro-XOR + encoded state).【crates/iroha_telemetry/src/metrics.rs:6212】
- `iroha_settlement_pnl_xor` — realised variance between due vs post-haircut XOR for the block batch.【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` — effective epsilon/haircut basis points applied to the batch.【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` — optional utilisation bucketed by liquidity profile when swap evidence is present.【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — per-lane/dataspace counters for settlement conversions and cumulative haircuts (XOR units).【crates/iroha_telemetry/src/metrics.rs:6260】【crates/iroha_core/src/block.rs:304】
- Grafana board: `dashboards/grafana/settlement_router_overview.json` (buffer headroom, variance, haircuts) plus Alertmanager rules embedded in the Nexus lane alert pack.
- Operator runbook: `ops/runbooks/settlement-buffers.md` (refill/alert workflow) and the FAQ in `docs/source/nexus_settlement_faq.md`.

## Developer & SRE Checklist
- Set `[settlement.router]` values in `config/config.json5` (or TOML) and validate via `irohad --version` logs; ensure thresholds satisfy `alert > throttle > xor_only > halt`.
- Populate lane metadata with the buffer account/asset/capacity so buffer gauges reflect live reserves; omit the fields for lanes that should not track buffers.
- Monitor `settlement_router_*` and `iroha_settlement_*` metrics via `dashboards/grafana/settlement_router_overview.json`; alert on throttle/XOR-only/halt states.
- Run `cargo test -p settlement_router` for pricing/policy coverage and the existing block-level aggregation tests in `crates/iroha_core/src/block.rs`.
- Record governance approvals for config changes in `docs/source/nexus_fee_model.md` and keep `status.md` updated when thresholds or telemetry surfaces change.

## Rollout Plan Snapshot
- Router + telemetry ship in every build; no feature gates. Lane metadata controls whether buffer snapshots publish.
- Default config matches the roadmap values (60 s TWAP, 25 bp base epsilon, 72 h buffer horizon); tune via config and restart `irohad` to apply.
- Evidence bundle = lane settlement commitments + Prometheus scrape for the `settlement_router_*`/`iroha_settlement_*` series + Grafana screenshot/JSON export for the affected window.

## Evidence & References
- NX-3 settlement router acceptance notes: `status.md` (NX-3 section).
- Operator surfaces: `dashboards/grafana/settlement_router_overview.json`, `ops/runbooks/settlement-buffers.md`.
- Receipt schema and API surfaces: `docs/source/nexus_fee_model.md`, `/v1/sumeragi/status` -> `lane_settlement_commitments`.
