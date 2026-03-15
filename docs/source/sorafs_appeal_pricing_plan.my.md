---
lang: my
direction: ltr
source: docs/source/sorafs_appeal_pricing_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f73be34bae18420b53954733c1b5e732bac698962e3413911c6573a13297e78
source_last_modified: "2025-12-29T18:16:36.133963+00:00"
translation_last_reviewed: 2026-02-07
title: Moderation Appeal Pricing Engine
summary: Final specification for SFM-4b2 covering deposit formulas, settlement logic, APIs, treasury reporting, and rollout.
---

# Moderation Appeal Pricing Engine

## Goals & Scope
- Calculate congestion-aware appeal deposits that reflect case complexity, backlog, and panel size.
- Manage refunds/slashing logic tied to moderation outcomes and ensure treasury reporting integrity.
- Provide APIs/CLI tooling so moderation services and operators can query pricing, post deposits, and reconcile settlements.

This specification completes **SFM-4b2 — Appeal pricing engine**.

## Architecture Overview
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Pricing service (`appeal_pricingd`) | Computes deposit requirements, updates rate tables, exposes REST/gRPC endpoints. | Stateless; configuration via governance-approved file. |
| Escrow contract (`AppealEscrowContractV1`) | Holds deposits on-chain, releases or slashes funds based on decisions. | Multi-sig controlled (Moderation + Treasury). |
| Settlement processor (`appeal_settlementd`) | Listens to moderation decisions, triggers escrow release/slash, generates statements. | Integrates with orderbook/treasury ledgers. |
| Reporting pipeline | Aggregates deposits/refunds/slashes, feeds treasury dashboards, exports CSV/Norito for audits. | Weekly + monthly cadence. |
| CLI/SDK | `sorafs appeal` commands; SDK helpers for pricing queries and settlement events. | Works with moderation portal and treasury tools. |

## Pricing Model
- Inputs:
  - `class` ∈ { `content`, `access`, `fraud`, `other` }
  - `backlog` (open appeals in class)
  - `evidence_size_mb`
  - `urgency` ∈ { `normal`, `high` } (requires moderator approval)
  - `panel_size` (juror count)
- Formula:
  ```
  base = class_base_rate[class]
  backlog_factor = min(backlog / backlog_target[class], backlog_cap[class])
  size_multiplier = 1 + min(evidence_size_mb / size_divisor[class], size_cap[class])
  urgency_multiplier = { normal: 1.0, high: 1.2 }
  panel_multiplier = panel_size / default_panel_size (default 7)

  deposit = base * (1 + backlog_factor) * size_multiplier * urgency_multiplier * panel_multiplier
  deposit = clamp(deposit, min_deposit[class], max_deposit[class])
  ```
- Default parameters (governance adjustable):
  - `class_base_rate`: content 150 XOR, access 200, fraud 500, other 120.
  - `backlog_target`: 50 for content, 30 access, 20 fraud; `backlog_cap` = 1.0.
  - `size_divisor`: 100 MB (content), 50 (access/fraud); `size_cap` = 2.0.
  - Min deposit: 100 XOR; max deposit: 2,500 XOR (fraud 5,000).
- Emergency override: governance may set `surge_multiplier` (e.g., 1.25) for specific classes.

## APIs
- `POST /v2/appeals/pricing/quote`:
  ```json
  { "class": "content", "backlog": 28, "evidence_size_mb": 45, "urgency": "normal", "panel_size": 7 }
  ```
  → response includes `deposit_xor`, `breakdown`, `valid_until`, `config_version`.
- `GET /v2/appeals/pricing/config` – returns active configuration (for transparency).
- `POST /v2/appeals/deposit` – creates deposit record, forwards funds to escrow contract (requires signature + token).
- `GET /v2/appeals/deposit/{case_id}` – status (posted, released, slashed).
- `GET /v2/appeals/report?period=2026-W12` – aggregated totals for treasury.
- All endpoints require mTLS + OAuth scopes (`appeals.pricing.read`, `appeals.deposit.write`, etc.).

## Escrow & Settlement
- Escrow contract operations:
  - `post_deposit(case_id, account, amount)` – called after pricing quote, locks XOR.
  - `release(case_id, amount)` – returns funds (refund).
  - `slash(case_id, amount)` – transfers to treasury account.
  - `partial_release` for partial refunds.
- Settlement triggers:
  - Moderation decision event consumed by `appeal_settlementd`.
  - Determination:
    - `uphold` (appeal successful): refund 100% + accrued interest (if configured).
    - `overturn` (appeal fails): apply class-specific refund/slash ratio.
    - `withdrawn`: refund 90% if before juror assignment; 0% after deliberation start.
    - Non-compliance (no-show, missing evidence) may apply penalties from config.
- Treasury ledger entries recorded as `AppealDepositPostedV1`, `AppealDepositSettledV1`.

Governance maintains the canonical settlement manifest under
`docs/examples/ministry/appeal_settlement_config_baseline.json`. Each entry
specifies the refund/treasury ratios for standard verdicts (`uphold`, `overturn`,
`modify`), special cases (withdrawn-before-panel, withdrawn-after-panel,
frivolous, escalated), and the baseline panel reward formula (25 XOR per juror
plus a 10 XOR per-case bonus). CLI helpers and treasury dashboards load this
manifest to guarantee that refunds (e.g., 90 % when a case is withdrawn before
sortition, 50 % refunds for frivolous cases) and slashed amounts are computed
consistently.

`sorafs_cli appeal disburse` combines the settlement manifest with the juror
roster to emit a deterministic payout plan: refund transfer target, deposit
slash for treasury, escrow holdback, per-juror stipend/bonus, and no-show
forfeitures (forfeited panel rewards are redirected to treasury for easy
reconciliation). Use `--format=json` to archive the exact plan alongside the
appeal evidence bundle.

## Reporting & Dashboards
- Metrics tracked:
  - Deposits per class, refunds, slashes.
  - Average deposit vs case duration.
  - Surge multiplier events.
- Weekly report `AppealPricingReportV1` includes:
  - Deposits/refunds/slashes by class.
  - Backlog snapshot.
  - Utilization of appeal budget.
  - Config changes (base rates, multipliers).
- Reports published to Governance DAG, pinned to IPFS, and visible on transparency dashboard.
- CLI `sorafs appeal report --week 2026-W12 --format csv`.

## Observability & Alerts
- Metrics:
  - `sorafs_appeal_pricing_requests_total{class}`
  - `sorafs_appeal_deposit_total{class,status}`
  - `sorafs_appeal_backlog{class}`
  - `sorafs_appeal_pricing_duration_seconds_bucket`
  - `sorafs_appeal_refund_slash_ratio{class}`
- Alerts:
  - Backlog factor > 0.6 for >24h.
  - Deposits failing (escrow call failures).
  - Report generation failure.
  - Pricing config mismatch between services.
- Logs include `case_id`, `quoted_deposit`, `parameters`, `config_version`.

## Security & Compliance
- Pricing responses signed (`PricingQuoteSignatureV1`); deposit requests include signature over quote + case ID to prevent tampering.
- Escrow contract requires multi-sig transactions; operations audited.
- Treasury integration ensures funds tracked in accounting system (XOR ledger + USD equivalent).
- Data retention: deposit/settlement records retained 7 years.
- Rate changes require governance vote; config hashed and stored in Governance DAG.

## Testing & Rollout
- Deterministic unit tests for pricing formula under various inputs.
- Integration tests with escrow contract on testnet.
- Load tests for API (target 200 req/s).
- Simulation tests verifying backlog impact on deposits.
- Rollout steps:
  1. Implement pricing service + escrow contract (staging).
  2. Calibrate base rates with historical data; governance approves config.
  3. Enable deposit posting in staging; reconcile with settlement events.
  4. Production rollout with monitoring; start with content class, expand to others after 2 cycles.

## Documentation & Tooling
- Update moderation operator manual with pricing policies & CLI usage.
- Treasury runbook for reconciliations.
- Transparency doc `docs/source/sorafs_transparency_plan.md` references new reports.
- `sorafs_cli appeal quote` exposes the baseline deposit calculator with JSON
  and table output so moderation services, QA, and treasury can exercise the
  formula without rebuilding the pricing engine. Provide
  `--config=docs/examples/ministry/appeal_pricing_config_baseline.json` (or `-`
  for stdin) to hydrate the CLI with governance-managed manifests the moment
  they land on the DAG so quotes and automation evidence always use the
  ratified parameters.

## Implementation Checklist
- [x] Define pricing formula, parameters, and configs.
- [x] Specify APIs, escrow interactions, and settlement rules.
- [x] Document reporting, observability, and alerts.
- [x] Capture security, governance, and auditing requirements.
- [x] Outline testing strategy and rollout plan.

With this specification, treasury, moderation, and governance teams can implement the appeal pricing engine consistently across services and audits.
