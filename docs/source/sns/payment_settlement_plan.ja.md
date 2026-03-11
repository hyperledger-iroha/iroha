---
lang: ja
direction: ltr
source: docs/source/sns/payment_settlement_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 769067521950bb9eb539a09cdecf3a043b4db3d2c0409e010284d60f0d155a38
source_last_modified: "2026-01-22T15:55:55.226686+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
title: Payment & Settlement Service (SN-5)
summary: Financial workflow specification for SNS settlement flows, treasury routing, reconciliation evidence, and automation.
---

# Payment & Settlement Service (SN-5)

Roadmap item **SN-5 — Payment & Settlement Service** requires a deterministic
way to route registrar revenue, steward splits, referral rebates, and refunds
without inventing bespoke spreadsheets per release. This document captures the
canonical policy, automation surfaces, observability hooks, and evidence
trail the payment service must provide before Nexus operators can open public
registrations.

## 1. Goals & Scope

- Bind every registration/renewal/refund to a deterministic Norito payload that
  encodes the fee calculation and ledger movements.
- Keep the 70/30 revenue split (treasury/steward) plus ≤10 % referral carve‑outs
  auditable through CLI helpers, dashboards, and monthly statements.
- Deliver a reconciliation service (`sns_settlementd`) with REST/Norito RPC
  surfaces, CLI wrappers, and runbooks so treasury teams can ingest statements
  without manual exports.
- Provide transparent evidence bundles: daily diffs, refund approvals, invoices,
  and ledger snapshots stored under `docs/source/sns/reports/`.

## 2. Revenue Book & Formulas

| Component | Symbol | Description | Notes |
|-----------|--------|-------------|-------|
| Gross fee | `gross_fee` | Amount paid by registrant (XOR) | Derived from registrar pricing (`pricing_class`, term years). |
| Treasury share | `treasury_share = gross_fee × 0.70` | Routed to `xor#sora::treasury`. | Governance may adjust via `PaymentPolicyV1`. |
| Steward share | `steward_share = gross_fee × 0.30 - referral_bonus` | Paid to suffix steward escrow. | Negative balance blocked. |
| Referral bonus | `referral_bonus = min(gross_fee × referral_percent, gross_fee × 0.10)` | Optional per registration. | Referral policy stored per suffix. |
| Refund holdback | `holdback` | Optional governance override (e.g., disputes). | Deducted from treasury share until released. |

Settlement bundles MUST publish the following Norito structure:

```yaml
PaymentBundleV1:
  settlement_id: <uuid>
  selector: <label.suffix>
  gross_fee: "100 xor#sora"
  treasury_share: "70 xor#sora"
  steward_share: "25 xor#sora"
  referral_bonus: "5 xor#sora"
  holdback: "0 xor#sora"
  invoices: [InvoiceLineV1, …]
  ledger_projection:
    debit_account: "i105..."
    credit_instructions: [Transfer, Transfer, ...]
```

`ledger_projection` mirrors the pattern established by the reserve + rent plan
and is consumed by automation to emit concrete `Transfer` ISIs.

## 3. Service Architecture

| Component | Responsibilities | Artefacts |
|-----------|-----------------|-----------|
| `sns_settlementd` | Applies policy, validates payment proofs, emits settlement bundles, exposes `/v1/sns/settlements`. | Schema: `PaymentBundleV1`, `InvoiceLineV1`, `RefundRecordV1`. |
| Settlement queue (`sns_settlement_queue`) | Idempotent pipeline (Kafka/SQS/Norito queue) that stores pending bundles and retries ledger commits. | Each record carries hash + `X-Iroha-Dedup-Key`. |
| Ledger writer (`sns_settlement_writer`) | Converts bundle projections into `Transfer` ISIs and submits them via Torii. | Reuses `iroha_cli app sns settlement ledger`. |
| Reconciliation job | Generates daily diff JSON + Markdown statements (`docs/source/sns/reports/settlement_<YYYYMMDD>.md`). | Links bundle hash → ledger tx hash + alert status. |
| Refund desk | Wraps governance approval, generates `RefundRecordV1`, calls `/v1/sns/settlements/{id}/refund`. | CLI helper + template. |

All components must emit structured logs (`PaymentEventV1`) so downstream
systems can reproduce the audit trail without scraping stdout.

## 4. APIs & CLI Surface

### REST / Norito RPC

| Route | Method | Description |
|-------|--------|-------------|
| `/v1/sns/settlements` | POST | Submit a signed `PaymentBundleV1` for execution (registrar or automation). |
| `/v1/sns/settlements/{id}` | GET | Fetch bundle status (`Pending`, `Posted`, `Failed`, `Refunded`). |
| `/v1/sns/settlements/{id}/refund` | POST | Attach `RefundRecordV1` once governance approves a partial or full refund. |
| `/v1/sns/settlements/statements?period=YYYY-MM` | GET | Return reconciliation digest (`StatementSummaryV1`). |

### CLI helpers

| Command | Purpose |
|---------|---------|
| `iroha_cli app sns settlement quote --selector makoto.sora --term-years 2 --pricing hot-tier-a --referral 0.05` | Computes the fee matrix, referral deduction, and ledger projection. |
| `iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05-01/makoto.sora.json --treasury-account i105... --steward-account i105...` | Emits Norito `Transfer` ISIs and persists them beside the bundle. |
| `iroha_cli app sns settlement reconcile --period 2026-05 --out reports/settlement_202605.md` | Compares Torii transactions against expected bundles, flags drift, and writes Markdown + JSON digests. |
| `iroha_cli app sns settlement refund --bundle <path> --amount 30 --reason "duplicate charge" --approval ticket.json` | Produces `RefundRecordV1` with governance metadata. |

All helpers accept `--json-out` to capture machine-readable artefacts for CI
and governance review.

## 5. Observability & Dashboards

Metrics exported via Prometheus/OTLP:

- `sns_settlement_bundles_total{status}` — bundle lifecycle transitions.
- `sns_settlement_amount_xor{bucket}` — labeled by suffix, steward, referral.
- `sns_settlement_reconciliation_failures_total` — mismatched ledger posts.
- `sns_refund_processing_seconds{result}` — refund latency.
- `sns_settlement_pending_age_seconds_max` — alert on stuck queues.

Dashboards: `dashboards/grafana/sns_payment_settlement.json` (treasury/steward
panel, referral chart, reconciliation heatmap) and Alertmanager rules under
`dashboards/alerts/sns_payment_settlement_rules.yml`. Operators must capture
PNG snapshots + `promtool` output and store them alongside monthly statements.

## 6. Evidence & Reporting

- **Daily diff:** JSON + Markdown in
  `docs/source/sns/reports/settlement_<YYYYMMDD>.{json,md}` summarising bundles,
  ledger tx hashes, alerts, and pending refunds.
- **Monthly statement:** `docs/source/sns/reports/settlement_<YYYYMM>.md`
  aggregates daily diffs, includes treasury/steward totals, referral payouts,
  outstanding refunds, and signature block (treasury + steward).
- **Governance packet:** `docs/source/sns/reports/settlement_<YYYYMM>_governance.md`
  attaches dashboards, CLI logs, and approvals for the Council vote.
- **Portal mirror:** Each statement is mirrored under
  `docs/portal/docs/sns/reports/` so partners see the same numbers.

Store all artefacts in Git plus the governance object store
(`s3://sora-governance/sns/settlements/<period>/`).

## 7. Rollout Phases

| Phase | Deliverables | Exit Criteria |
|-------|-------------|---------------|
| **S1 — Prototype** | CLI quote + ledger helper, JSON schema, manual reconciliation guide. | One suffix processed end-to-end in staging with bundle + ledger evidence. |
| **S2 — Automation** | `sns_settlementd`, queue, ledger writer, dashboards, Alertmanager rules. | CI proof of deterministic bundle execution + alert tests via `promtool`. |
| **S3 — Governance & Refunds** | Refund helper, statement templates, council sign-off, portal publication workflow. | Monthly statement stored in both `docs/source/sns/reports/` and portal mirror with signed approvals. |
| **S4 — Partner Run** | Integrate with billing + treasury hedging, rehearse rollback of incorrect bundles, finalize referral policy. | Governance approves SN-5 completion; roadmap status flips to 🈺/🈴. |

## 8. Open Questions

- Should referral rebates land immediately or at statement close? (Default:
  pay at close, but CLI allows immediate payout for pilots.)
- How do we freeze steward payouts when disputes fire? (Answer: set
  `holdback` > 0 and require governance release before ledger writer runs.)
- Do we need fiat settlement exports? (Yes—`reconcile` command emits CSV for
  accounting; template to land in `docs/examples/sns/settlement_export_template.csv`.)

All answers and future amendments must be reflected in this document and linked
from `status.md` whenever SN-5 updates land.
