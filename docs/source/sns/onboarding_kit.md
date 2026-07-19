---
title: SNS Metrics & Onboarding Kit (SN-8)
summary: Dashboards, pricing cheatsheets, and registrar tooling required by roadmap item SN-8.
---

# Sora Name Service Metrics & Onboarding Kit

**Roadmap reference:** SN-8 “Metrics & Onboarding” (see `roadmap.md:432`).  
**Related assets:** `dashboards/grafana/sns_suffix_analytics.json`, `docs/portal/docs/sns/kpi-dashboard.md`.

This guide bundles the analytics package and registrar onboarding collateral
needed to exercise `.sora`, `.nexus`, and `.dao` launches. It ties together the
Grafana dashboard, pricing references, DNS helpers, and developer automation so
governance has deterministic evidence when approving registrars or suffixes.

## 1. Metric bundle

### 1.1 Grafana dashboard & portal mirror

The checked-in `dashboards/grafana/sns_suffix_analytics.json` still contains
pre-clean-break bulk-settlement series. Do not import it or treat it as
financial evidence until those queries are regenerated from canonical alias
plans and committed transaction receipts. A read-only reporting adapter may
export those records to Grafana; it must never create payment evidence or
participate in consensus.

- The portal page at `docs/portal/docs/sns/kpi-dashboard.md` defines the safe
  data contract. Do not embed the legacy dashboard there until its queries
  satisfy that contract.
- `ci/check_docs_portal.sh` already exercises the embed; run it after updating
  the dashboard JSON to catch stale hashes before publishing.

### 1.2 Panels & signals

| Panel | Canonical source | Evidence produced |
|-------|--------------------|-------------------|
| Setup dispositions | `AliasTransactionPlanV1` plus the committed transaction result | No-op, repair, create, and conflict counts without inventing per-resource submissions. |
| Native charges | Exact planner quotes matched to committed ledger debits | Totals by payment asset, policy version, and resource. |
| Onboarding readiness | Sorted `AliasSetupReportV1` snapshots | Ready/Pending/Blocked history and stable diagnostic codes. |
| Disputes & freezes | `guardian_freeze_active`, `sns_governance_activation_total`, `sns_dispute_outcome_total` | Shows open freezes, arbitration cadence, dispute backlog. |
| Lifecycle operations | Verified renewal/auto-renew plans and transaction receipts | CAS failures, suspensions, retries, and successful renewals. |

Export PDF or CSV snapshots from Grafana (or the portal embed) during the
monthly review and attach them to the annex entry under
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. The `docs/source/sns/reports/`
directory captures quarterly summaries (for example,
`docs/source/sns/reports/steward_scorecard_2026q1.md`); include dashboard SHA-256
hashes and Grafana URLs there for long-term audits.

### 1.3 Regulatory annex automation

Automate the KPI annex generation so governance reviewers receive a consistent
snapshot only after the dashboard has passed the safe-source review above:

```bash
cargo xtask sns-annex \
  --suffix .sora \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.sora/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- The command hashes the exported JSON, records the dashboard metadata, and
  writes a Markdown annex under `docs/source/sns/reports/.<suffix>/<cycle>.md`
  (mirroring the `.sora` example added in this change).
- Pass `--dashboard-artifact` to copy the export into
  `artifacts/sns/regulatory/<suffix>/<cycle>/` automatically so the annex
  references the canonical evidence path instead of your workstation file.
- Use `--dashboard-label` only when the archive lives outside of the repo (for
  example, a secure bucket); otherwise the helper records the relative artefact
  path for you.
- Pass `--regulatory-entry` to point at the governing memo for that cycle. The
  helper inserts or updates a `KPI Dashboard Annex` block (bounded by
  `<!-- sns-annex:... -->` markers) with the annex path, dashboard artefact
  location, digest, and timestamp so auditors can diff memos without manual
  edits.
- Pass `--portal-entry` with the matching `docs/portal/docs/sns/regulatory/*.md`
  page so the public portal mirrors receive the same annex block.
- If you omit `--regulatory-entry`/`--portal-entry`, you can still attach the
  generated file to the memos manually, but the automated blocks keep evidence in
  sync across repeated re-generations. Upload the JSON bundle to artefact
  storage for auditors either way.
- For recurring reviews, add each suffix/cycle pair to
  `docs/source/sns/regulatory/annex_jobs.json` and run
  `python3 scripts/run_sns_annex_jobs.py --verbose` (optionally with
  `--dry-run`) so the helper iterates over every entry, copies the dashboard
  export, and updates the corresponding regulatory (and portal, when present)
  memos with the annex block in one pass. The script defaults to
  `dashboards/grafana/sns_suffix_analytics.json` when a job omits `dashboard`.
- Run `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (or `make check-sns-annex`) to prove the job list stays sorted/deduped, every memo carries a matching `sns-annex` marker, and each annex stub exists. The helper emits `artifacts/sns/annex_schedule_summary.json` alongside the locale and hash summaries so governance packets can cite a single evidence bundle.
This automation replaces ad-hoc copy/paste steps and ensures every annex entry
captures the dashboard UID, tags, panel count, export digest, and schedule/locale
coverage required by SN-8.

#### 1.3.1 Annex job runbook & cadence

- Keep the annex job list in sync with the governance calendar. The schedule
  check (`scripts/check_sns_annex_schedule.py`) fails if a memo marker lacks a
  job entry or a stub is missing, forcing upcoming cycles to be booked before
  exports land.
- Follow {doc}`sns/regulatory/annex_job_runbook` when adding new cycles: create
  the regulatory memo skeleton (`docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`),
  append the job entry, and run `scripts/run_sns_annex_jobs.py --verbose` to
  refresh the annex blocks + artefacts.
- Attach the resulting artefacts (`artifacts/sns/regulatory/<suffix>/<cycle>/`)
  to the governance ticket and record the run in `status.md`.

## 2. Onboarding kit contents

### 2.1 Suffix wiring & DNS helpers

- Registry schema and selector encoding live in
  `docs/source/sns/registry_schema.md` and `docs/source/sns/local_to_global_toolkit.md`.
- DNS/gateway teams convert registrar manifests into resolver skeletons via
  `scripts/sns_zonefile_skeleton.py`; the workflow is documented in
  `docs/source/sorafs_gateway_dns_owner_runbook.md`.
- Zonefile metadata now records `{name, version, ttl, effective_at, cid, gar_digest, proof}`
  at the top level (`zonefile.*`). When invoking the helper, provide a `--zonefile-version`
  label, `--effective-at` timestamp aligned with the rollout window, and the GAR digest
  (`--gar-digest`) produced by the signing automation so resolvers and auditors can tie
  each skeleton back to the GAR evidence bundle.
- When onboarding a new registrar or suffix, create a
  `docs/source/sns/reports/<name>_launch.md` note that records the selector
  samples, zonefile hashes, and GAR evidence.

### 2.2 Pricing & coefficient cheatsheet

Base fees (USD equivalent, before suffix coefficient):

| Label length | Base fee |
|--------------|----------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6–9 | $12 |
| 10+ | $8 |

Suffix coefficients (`Final fee = base × coefficient`):

| Suffix | Coefficient |
|--------|-------------|
| `.sora` | 1.0× |
| `.nexus` | 0.8× |
| `.dao` | 1.3× |

Term modifiers: 2‑year renewals receive −5 %; 5‑year renewals receive −12 %.
Grace = 30 days, redemption = 60 days (20 % fee, min $5, max $200). Document
any governance-approved deviations in the registrar’s onboarding ticket.

### 2.3 Premium auctions vs renewals

1. **Premium commit/reveal** (SN-3) — an auction may determine acquisition
   entitlement, but it does not restore a direct registration DTO or mutation
   route. The winning resource is resolved and planned through the same signed
   `AliasIntentV1`/`EnsureAlias` flow, with any required endorsement validated
   before the atomic transaction executes. Publish the auction manifest under
   `docs/source/sns/reports/` for future audits.
2. **Dutch reopen** — when grace + redemption elapse, the registrar publishes a
   Dutch reopen manifest detailing the initial multiplier (10×) and daily decay
   (15 %). Track the decay status in a read-only report keyed by the canonical
   auction record and resulting alias plan hash.
3. **Renewals** — archive the verified renewal plan and ordinary transaction
   receipt. For auto-renew, also capture the configured revision and any native
   retry or suspension status.

### 2.4 Developer APIs & automation

- The clean-break plan/local-sign/apply contract lives in
  `docs/source/sns/registrar_api.md`.
- `scripts/sns_bulk_onboard.py` (documented in
  `docs/source/sns/bulk_onboarding_toolkit.md`) consumes typed, secret-free
  setup intents and delegates signed planning plus atomic apply to `iroha app
  alias setup`. It does not call a direct SNS mutation endpoint.
- Persist the canonical `AliasTransactionPlanV1` beside the operator change
  record. Its hash, live-state anchor, dispositions, framed instructions, and
  exact asset totals replace ad-hoc payment-proof and per-resource submission
  logs.
- Use `irohad --check-config` and the authenticated onboarding-readiness report
  as the preflight evidence for generated or manually operated peers.

### 2.5 Evidence checklist

1. Ticket in the registrar tracker including suffix, contact, expected payment
   asset, policy version, and approved caps.
2. DNS/resolver evidence bundle (`scripts/sns_zonefile_skeleton.py` output).
3. Pricing worksheet referencing the tables above plus any negotiated overrides.
4. API smoke-test artefacts (signed plan, verified plan hash, and the single
   ordinary transaction result).
5. Safe read-only report export stored under
   `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`; do not attach output from
   the legacy bulk-settlement dashboard.

## 3. Readiness checklist

| Step | Owner | Command / Artefact |
|------|-------|--------------------|
| Validate reporting source | Product Analytics | Query review plus a sample joining exact plan quotes to committed transaction debits; the legacy dashboard is not imported. |
| Publish portal page | Docs/DevRel | `npm run build` inside `docs/portal`, ensure `sns/kpi-dashboard` renders. |
| Generate onboarding kit | DevRel | Copy this file, fill registrar-specific deltas, attach to ticket. |
| DNS rehearsal | Networking/Ops | `scripts/sns_zonefile_skeleton.py` + `docs/source/sorafs_gateway_dns_owner_runbook.md` steps. |
| Registrar automation dry run | Registrar eng | `python3 scripts/sns_bulk_onboard.py setup.json --plan-file setup.plan.json --plan-only` |
| Governance evidence | Governance Council | File annex updates + attach dashboard exports and artefacts. |

Run this checklist before activating every registrar or suffix. Attach the
resulting artefacts to the governance proposal so SN-8’s metrics/onboarding
gate is auditable.
