---
lang: uz
direction: ltr
source: docs/source/sorafs_reserve_rent_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22eb4a0ba14cb8e962c7fd5fe50393f2683b066afab4a04778c2596b97296902
source_last_modified: "2026-01-22T16:26:46.592997+00:00"
translation_last_reviewed: 2026-02-07
title: Reserve+Rent & Lifecycle Policy
summary: Final specification for SFM-6 covering reserve underwriting, lifecycle states, credit lines, APIs, dashboards, and rollout.
---

# Reserve+Rent & Lifecycle Policy

## Goals & Scope
- Define the financial policy for provider reserves and recurring rent, including lifecycle stages and credit line management.
- Provide APIs, dashboards, and alerts to monitor provider health, enforce underwriting ratios, and trigger governance actions.
- Integrate with hedging/billing, reputation, and moderation systems.

This specification completes **SFM-6 — Reserve-plus-Rent & lifecycle policies**.

## Policy Model
- Key variables:
  - `monthly_rent = base_rate_tier * capacity_gib * duration_factor`
  - `reserve_requirement = underwriting_ratio * monthly_rent`
  - `effective_rent = monthly_rent - min(reserve_balance / underwriting_ratio, monthly_rent)`
  - Reserve top-up threshold = 0.8 × reserve requirement.
- Tier base rates (governance adjustable): hot 12 XOR/GiB-month, warm 6, archive 2.
- Duration factors: monthly 1.0, quarterly 0.9, annual 0.75.
- Underwriting ratios default: Tier A 2.0, Tier B 3.0, Tier C 4.5.
- Credit line caps: Tier A 2× monthly rent, Tier B 1×, Tier C manual approval.
- Interest accrues daily (simple) with APR defined per tier (3%, 6%, none).
- Lifecycle stages (same table as draft) with automated transitions.
- Penalties:
  - Stage `Warning`: restrict new manifests.
  - `Grace`: auto-draw credit line.
  - `Delinquent`: penalty APR + governance notification.
  - `Default`: disable adverts, initiate slashing (pull from reserve then credit line).
- Manual appeal: `ReserveAppealV1` allows provider to request extension; governance decision required within 72h.

## APIs & Services
- Reserve service (`reserve_rentd`) computes rent/reserve, manages credit lines, emits events.
- REST endpoints:
  - `GET /v1/reserve/summary?provider=` – returns `ReserveSummaryV1` (balance, requirement, stage, credit line status).
  - `POST /v1/reserve/top-up` – apply reserve top-up.
  - `POST /v1/reserve/withdraw` – withdraw excess reserve (must maintain requirement).
  - `POST /v1/reserve/appeal` – file appeal, optionally attach documentation.
  - `GET /v1/reserve/lifecycle` – list stage thresholds and overrides.
  - `GET /v1/reserve/events?provider=` – pagination of `ReserveLifecycleEventV1`.
- CLI commands:
  - `sorafs reserve status --provider <id>`
  - `sorafs reserve top-up --provider <id> --amount 500`
  - `sorafs reserve appeal --provider <id> --reason <text>`
  - `sorafs reserve config` – display policy parameters.
  - `sorafs reserve quote --storage-class <hot|warm|cold> --tier <tier-a|tier-b|tier-c> --duration <monthly|quarterly|annual> --gib <capacity>` — compute deterministic rent/reserve breakdowns (monthly rent, reserve requirement, top-up threshold, credit line cap) using the embedded policy or JSON/Norito overrides. Quotes are emitted as JSON and can be persisted via `--quote-out`. The CLI reuses the shared `ReservePolicyV1` schema so economics dashboards and SDKs can reference the same Norito payloads without reimplementing the formulas. The JSON payload now includes a `ledger_projection` object with:
    - `rent_due` — XOR due for the billing period after applying reserve offsets.
    - `reserve_shortfall` — reserve delta required to satisfy underwriting.
    - `top_up_shortfall` — amount needed to clear the top-up alert threshold.
    - `meets_underwriting` / `needs_top_up_alert` — booleans used by dashboards and admission ISIs to trigger policy transitions.
  - `sorafs reserve ledger --quote <path> --provider-account <id> --treasury-account <id> --reserve-account <id> --asset-definition xor#sora` — convert a saved quote into the concrete XOR transfers required for rent settlement and reserve top-ups. The helper reads the `ledger_projection` block, echoes the micro-XOR totals, and emits an `instructions` array containing Norito-encoded `Transfer` ISIs that can be piped straight into existing automation (or stored alongside governance evidence).

## Integration Points
- **Hedging/Billing**: rent calculations feed billing aggregator; hedging service uses reserve balance to estimate exposure.
- **Reputation**: stage transitions adjust reputation degradation flags.
- **Orderbook**: providers in `Grace` or worse receive limits/penalties.
- **Governance**: events recorded in DAG; council receives weekly summary.
- **Compliance**: default stage triggers compliance notifications (disconnect provider).

## Observability
- Metrics:
  - `sorafs_reserve_balance_xor{provider}`
  - `sorafs_reserve_requirement_xor{provider}`
  - `sorafs_reserve_stage{stage}`
  - `sorafs_reserve_credit_usage{provider}`
  - `sorafs_reserve_alerts_total{type}`
  - `sorafs_reserve_default_total`
- Alerts:
  - Reserve coverage < 0.8.
  - Credit line usage > 95%.
  - Stage transitions to `Grace` or `Delinquent`.
  - Appeal backlog > SLA.
- Dashboards display provider reserve health, aggregate exposure, stage distribution.

## Security & Governance
- Reserve and credit line adjustments require authentication (mTLS + RBAC).
- Data stored encrypted; audit logs hashed daily and committed to DAG.
- Governance approves policy changes via `ReservePolicyUpdateV1`; changes propagate to config service.
- Stage overrides (manual) require multi-sig sign-off; recorded as `ReserveOverrideEventV1`.

## Testing & Rollout
- Unit tests for rent/reserve calculations, stage transitions, credit line logic.
- Integration tests with billing and hedging to ensure consistency.
- Chaos tests: simulate reserve depletion, credit line max out, ensure alerts triggered.
- Rollout:
  1. Implement reserve service with staging dataset.
  2. Calibrate parameters with Economics WG.
  3. Deploy dashboards and alert rules.
  4. Staging bake with select providers; adjust thresholds.
  5. Production rollout; monitor initial month, report to governance.

## Automation & Dashboards

### Quote Matrix Generator

Run `cargo xtask sorafs-reserve-matrix` to emit a deterministic JSON matrix of
rent/reserve quotes covering the requested storage classes, tiers, durations,
and capacity bands. The task loads `ReservePolicyV1` (either from the embedded
defaults or the supplied `--policy-json`/`--policy-norito` override), applies
the underwriting ratios documented above, and records both the raw micro-XOR
amounts and the policy metadata so dashboards can assert provenance.

```bash
cargo xtask sorafs-reserve-matrix \
  --capacity 10 --capacity 100 --capacity 1000 \
  --storage-class hot --storage-class warm \
  --tier tier-a --tier tier-b \
  --duration monthly --duration annual \
  --reserve-balance 250.5 \
  --out artifacts/sorafs_reserve/matrix.json
```

Use `--label <text>` to tag the generated artefact (useful when comparing
dashboards or governance submissions) and `--reserve-balance <XOR>` to model
effective rent when an operator already maintains a reserve. The JSON output
includes `policy_sha256`, `policy_version`, and `reserve_balance_micro_xor`
fields alongside per-combination quotes so automation and analytics tooling can
trace every data point back to the exact policy used. Each quote entry also
contains a `ledger_projection` block (matching the CLI output) so dashboards,
reserve auditors, and ledger ISIs can render rent/reserve deltas without
recomputing underwriting math.

### Reserve Ledger Digest & Dashboard Wiring

Field teams asked for a deterministic way to embed `sorafs reserve ledger`
output inside dashboards and governance packets. The workflow below turns the
CLI JSON into a reusable digest and keeps the telemetry panels in sync with the
ledger projection that triggered the payment.

1. **Generate the ledger projection JSON.**
   ```bash
  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account i105... \
    --treasury-account i105... \
    --reserve-account i105... \
    --asset-definition xor#sora \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
2. **Normalise the values with the new helper.**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   `scripts/telemetry/reserve_ledger_digest.py` converts the micro‑XOR values
   into XOR, records whether underwriting thresholds were satisfied, and hashes
   the execution timestamp. The helper now also captures the **transfer feed**
   (`transfers` block) so rent and reserve top-ups appear alongside the projected
   ledger deltas, and `instruction_count` proves the CLI emitted both transfers.
   The script accepts multiple `--ledger` paths (plus per-ledger `--label`
   overrides) and can emit NDJSON batches via `--ndjson-out`, letting economics
   automation ingest an entire rent cycle without bespoke glue. The Markdown
   and JSON digests slot directly into governance packets while the JSON
   artefact can be ingested by automation or replayed in dashboards. The
   `--out-prom` flag writes a Prometheus textfile snapshot (`sorafs_reserve_ledger_*`
   metrics, including `sorafs_reserve_ledger_transfer_xor` +
   `sorafs_reserve_ledger_instruction_total`) so any node exporter with the
   textfile collector enabled can surface the latest ledger requirements to
   Grafana and Alertmanager without bespoke exporters; batched runs append every
   ledger to the same textfile so Alertmanager rewires as soon as treasury
   stages a new reserve transfer.
3. **Attach the digest to telemetry.** The digest JSON is scraped by the
   rent/reserve exporter and enriches the `torii_da_*` counters with the same
   labels (`provider`, `storage_class`, `tier`) that the CLI used for the quote.
   Store the digest under `artifacts/sorafs_reserve/ledger/<provider>/` so the
   observability jobs that refresh `dashboards/grafana/sorafs_capacity_health.json`
   and the reserve-focused board in
   `dashboards/grafana/sorafs_reserve_economics.json` can locate the latest
   projection before each rent cycle.
4. **Update the runbook evidence block.** Drop the Markdown digest next to the
   weekly economics report (`docs/source/sorafs/reports/`) and link it from the
   rent burn-down so reviewers see the exact ledger inputs that produced the
   transfers.

### Metrics, Dashboards, and Alerts

Reserve telemetry now hinges on the DA counters emitted by Torii
(`crates/iroha_telemetry/src/metrics.rs`). The table below calls out the panels
and alert packs that consume those metrics so operators know which evidence to
collect after running the ledger helper.

| Metric | Grafana panel / dashboard | Alert / Runbook hook | Notes |
|--------|--------------------------|----------------------|-------|
| `torii_da_rent_base_micro_total` | “DA Rent Distribution (XOR/hour)” in `dashboards/grafana/sorafs_capacity_health.json` | Include in the weekly rent digest; panel traces how much rent was invoiced as XOR. |
| `torii_da_protocol_reserve_micro_total` | Same dashboard/panel (`refId=B`) | Feed into `dashboards/alerts/sorafs_capacity_rules.yml` via the `SoraFSCapacityPressure` context; rising reserve flows drive early warnings when underwriting falls behind. |
| `torii_da_provider_reward_micro_total` | “DA Rent Distribution” (`refId=C`) | Record spurts inside the economics status note so treasury can correlate payouts with ledger digests. |
| `torii_da_pdp_bonus_micro_total` / `torii_da_potr_bonus_micro_total` | “DA Bonus Accrual (XOR/hour)” panel in `dashboards/grafana/sorafs_capacity_health.json` | Reference in the PDP/PoTR compliance runbook; attach Alertmanager output when bonuses exceed policy. |
| `torii_da_rent_gib_months_total` | Capacity Usage widgets (same dashboard) | Pair with the ledger digest to show how many GiB·months were invoiced alongside the XOR amounts. |
| `sorafs_reserve_ledger_*` (rent/top-up/underwriting gauges) | “Reserve Snapshot (XOR)” + “Top-up Required” in `dashboards/grafana/sorafs_reserve_economics.json` (mirrored cards remain on the capacity board for historical context) | `SoraFSReserveLedgerTopUpRequired` and `SoraFSReserveLedgerUnderwritingBreach` inside `dashboards/alerts/sorafs_capacity_rules.yml` fire when the CLI projects a top-up or an underwriting failure. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | “Transfers by Kind”, “Latest Transfer Breakdown”, the coverage cards in `dashboards/grafana/sorafs_reserve_economics.json`, and the mirrored transfer coverage stats on the capacity board (`dashboards/grafana/sorafs_capacity_health.json`) | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing`, `SoraFSReserveLedgerTopUpTransferMissing`, `SoraFSReserveTransferRentMismatch`, and `SoraFSReserveTransferTopUpMismatch` in `dashboards/alerts/sorafs_capacity_rules.yml` cover missing/zeroed or mismatched transfer feeds whenever rent/top-up is required. |

Whenever the counters or dashboards change, re-run
`python3 scripts/telemetry/reserve_ledger_digest.py --ledger <...> --print` (or
point `--ndjson-out` / `--out-prom` at the automation directories) and attach
the refreshed digest to the rent burn-in evidence bundle. This keeps the
dashboards, alert packs, and governance packets aligned with the latest ledger
projection without re-deriving the math by hand. The transfer feed plus coverage
cards make it obvious when rent/reserve instructions drift from the ledger
projection, and the new alerts fire as soon as a digest omits the required rent
or reserve top-up transfers.

## Implementation Checklist
- [x] Specify policy formulas, stages, and parameters.
- [x] Document APIs, CLI, and integrations.
- [x] Capture observability metrics, alerts, and governance controls.
- [x] Outline testing, calibration, and rollout.

With this specification, economics and platform teams can implement Reserve+Rent management that keeps providers solvent while maintaining governance oversight.
