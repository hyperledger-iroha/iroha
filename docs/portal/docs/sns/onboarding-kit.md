---
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
---

# SNS Metrics & Onboarding Kit

Roadmap item **SN-8** bundles two promises:

1. Publish dashboards that expose registrations, renewals, ARPU, disputes, and
   freeze windows for explicitly selected live SNS policies.
2. Ship an onboarding kit so registrars and stewards can wire DNS, pricing, and
   APIs consistently before any suffix goes live.

This page mirrors the source version
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
so external reviewers can follow the same procedure.

## 1. Metric bundle

### Grafana dashboard & portal embed

- The checked-in `dashboards/grafana/sns_suffix_analytics.json` still contains
  pre-clean-break bulk-settlement series. Do not import it or treat it as
  financial evidence until those queries are regenerated from canonical alias
  plans and committed transaction receipts.
- A read-only adapter may export plan and ledger records to Grafana. It must
  never construct payment evidence or participate in consensus.

- The **Alias Provisioning Operational Evidence** portal page defines the safe
  data contract. Do not embed the legacy dashboard until its queries satisfy
  that contract. After regeneration, run `npm run build` inside `docs/portal`
  and inspect the preview before publishing.

### Panels & evidence

| Panel | Canonical source | Governance evidence |
|-------|---------|---------------------|
| Setup dispositions | `AliasTransactionPlanV1` plus committed transaction results | No-op, repair, create, and conflict counts without per-resource submissions. |
| Native charges | Exact planner quotes matched to committed ledger debits | Totals by payment asset, policy version, and resource. |
| Onboarding readiness | Sorted `AliasSetupReportV1` snapshots | Ready/Pending/Blocked history and stable diagnostic codes. |
| Disputes & freezes | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | Shows active freezes, arbitration cadence, and guardian workload. |
| Lifecycle operations | Verified renewal/auto-renew plans and transaction receipts | CAS failures, suspensions, retries, and successful renewals. |

Export a PDF/CSV from Grafana (or the embedded iframe) during the monthly KPI
review and attach it to the relevant annex entry under
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Stewards also capture the SHA-256
of the exported bundle under `docs/source/sns/reports/` (for example,
`steward_scorecard_2026q1.md`) so audits can replay the evidence path.

### Annex automation

After the dashboard has passed the safe-source review above, generate annex
files from its export so reviewers get a consistent digest:

```bash
cargo xtask sns-annex \
  --suffix .example \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.example/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.example/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- The helper hashes the export, captures the UID/tags/panel count, and writes a
  Markdown annex under `docs/source/sns/reports/<suffix>/<cycle>.md`.
- `--dashboard-artifact` copies the export into
  `artifacts/sns/regulatory/<suffix>/<cycle>/` so the annex references the
  canonical evidence path; use `--dashboard-label` only when you need to point
  at an out-of-band archive.
- `--regulatory-entry` points at the governing memo. The helper inserts (or
  replaces) a `KPI Dashboard Annex` block that records the annex path, dashboard
  artefact, digest, and timestamp so evidence stays in sync after re-runs.
- `--portal-entry` keeps the Docusaurus copy (`docs/portal/docs/sns/regulatory/*.md`)
  aligned so reviewers do not have to diff separate annex summaries manually.
- If you skip `--regulatory-entry`/`--portal-entry`, attach the generated file to
  the memos manually and still upload the PDF/CSV snapshots captured from Grafana.
- For recurring exports, list the suffix/cycle pairs in
  `docs/source/sns/regulatory/annex_jobs.json` and run
  `python3 scripts/run_sns_annex_jobs.py --verbose`. The helper walks every entry,
  copies the dashboard export (defaulting to `dashboards/grafana/sns_suffix_analytics.json`
  when unspecified), and refreshes the annex block inside each regulatory (and,
  when available, portal) memo in one pass.
- Run `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (or `make check-sns-annex`) to prove the job list stays sorted/deduped, each memo carries the matching `sns-annex` marker, and the annex stub exists. The helper writes `artifacts/sns/annex_schedule_summary.json` beside the locale/hash summaries used in governance packets.
This removes manual copy/paste steps and keeps SN-8 annex evidence consistent while
guarding schedule, marker, and localization drift in CI.

## 2. Onboarding kit components

### Suffix wiring

- Registry schema + selector rules:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  and [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- DNS skeleton helper:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  with the rehearsal flow captured in the
  [gateway/DNS runbook](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- For every registrar launch, file a short note under
  `docs/source/sns/reports/` summarising selector samples, GAR proofs, and DNS hashes.

### Pricing and lease-policy preflight

Do not copy fee tables, suffix coefficients, term discounts, or lifecycle
windows into client collateral. Inspect the active read-only SNS policy and
request an `AliasTransactionPlanV1` immediately before approval. Record the
expected policy version, payment asset, maximum amount, deadline, exact quote,
and any blockers from that live plan in the registrar ticket.

### Premium auctions vs renewals

1. **Premium pool** — sealed-bid commit/reveal (SN-3). Track bids with
   `sns_premium_commit_total`, and publish the manifest under
   `docs/source/sns/reports/`.
2. **Dutch reopen** — after grace + redemption expire, start the governed Dutch
   sale and bind its audit trail to the canonical auction record plus the
   resulting alias plan hash.
3. **Renewals** — archive the verified renewal plan and ordinary transaction
   receipt. For auto-renew, also capture the configured revision and any native
   retry or suspension status.

### Developer APIs & automation

- API contracts: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Typed setup helper:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- Example command:

```bash
python3 scripts/sns_bulk_onboard.py setup.json \
  --config client.toml \
  --plan-file artifacts/sns/releases/2026q2/setup.plan.json
```

The command above is read-only. After approval, append `--apply` to have the
ordinary client locally sign and submit the complete plan as one transaction.
Archive the verified plan hash, live-state anchor, exact asset totals, and
single-transaction result. The helper neither accepts tokens/keys nor calls a
direct SNS mutation endpoint.

### Evidence bundle

1. Registrar ticket with contacts, suffix scope, expected payment asset,
   policy version, and approved caps.
2. DNS/resolver evidence (zonefile skeletons + GAR proofs).
3. Live policy snapshot plus the planner quote and quote-guard values.
4. API/CLI smoke-test artefacts (signed plan, verified hash, and one atomic
   transaction result).
5. Safe read-only report export attached to the monthly annex; do not attach
   output from the legacy bulk-settlement dashboard.

## 3. Launch checklist

| Step | Owner | Artefact |
|------|-------|----------|
| Reporting source validated | Product Analytics | Query review plus a sample joining exact plan quotes to committed transaction debits; the legacy dashboard is not imported. |
| Portal embed validated | Docs/DevRel | `npm run build` logs + preview screenshot |
| DNS rehearsal complete | Networking/Ops | `sns_zonefile_skeleton.py` outputs + runbook log |
| Registrar automation dry run | Registrar Eng | `sns_bulk_onboard.py` verified plan |
| Governance evidence filed | Governance Council | Annex link + SHA-256 of exported dashboard |

Complete the checklist before activating a registrar or suffix. The signed
bundle clears the SN-8 roadmap gate and gives auditors a single reference when
reviewing marketplace launches.
