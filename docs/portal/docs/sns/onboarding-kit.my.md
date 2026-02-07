---
lang: my
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c90050703841af7b2f468ead9e23445ba68344cb9c4db5d7271a8af33a8cb91
source_last_modified: "2025-12-29T18:16:35.174056+00:00"
translation_last_reviewed: 2026-02-07
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
---

# SNS Metrics & Onboarding Kit

Roadmap item **SN-8** bundles two promises:

1. Publish dashboards that expose registrations, renewals, ARPU, disputes, and
   freeze windows for `.sora`, `.nexus`, and `.dao`.
2. Ship an onboarding kit so registrars and stewards can wire DNS, pricing, and
   APIs consistently before any suffix goes live.

This page mirrors the source version
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
so external reviewers can follow the same procedure.

## 1. Metric bundle

### Grafana dashboard & portal embed

- Import `dashboards/grafana/sns_suffix_analytics.json` into Grafana (or another
  analytics host) via the standard API:

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- The same JSON powers this portal page’s iframe (see **SNS KPI Dashboard**).
  Whenever you bump the dashboard, run
  `npm run build && npm run serve-verified-preview` inside `docs/portal` to
  confirm both Grafana and the embed stay in sync.

### Panels & evidence

| Panel | Metrics | Governance evidence |
|-------|---------|---------------------|
| Registrations & renewals | `sns_registrar_status_total` (success + renewal resolver labels) | Per-suffix throughput + SLA tracking. |
| ARPU / net units | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Finance can match registrar manifests to revenue. |
| Disputes & freezes | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | Shows active freezes, arbitration cadence, and guardian workload. |
| SLA/error rates | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | Highlights API regressions before they impact customers. |
| Bulk manifest tracker | `sns_bulk_release_manifest_total`, payment metrics with `manifest_id` labels | Connects CSV drops to settlement tickets. |

Export a PDF/CSV from Grafana (or the embedded iframe) during the monthly KPI
review and attach it to the relevant annex entry under
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Stewards also capture the SHA-256
of the exported bundle under `docs/source/sns/reports/` (for example,
`steward_scorecard_2026q1.md`) so audits can replay the evidence path.

### Annex automation

Generate annex files directly from the dashboard export so reviewers get a
consistent digest:

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

- The helper hashes the export, captures the UID/tags/panel count, and writes a
  Markdown annex under `docs/source/sns/reports/.<suffix>/<cycle>.md` (see the
  `.sora/2026-03` sample committed alongside this doc).
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

### Pricing cheatsheet

| Label length | Base fee (USD equiv) |
|--------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6–9 | $12 |
| 10+ | $8 |

Suffix coefficients: `.sora` = 1.0×, `.nexus` = 0.8×, `.dao` = 1.3×.  
Term multipliers: 2‑year −5 %, 5‑year −12 %; grace window = 30 days, redemption
= 60 days (20 % fee, min $5, max $200). Record negotiated deviations in the
registrar ticket.

### Premium auctions vs renewals

1. **Premium pool** — sealed-bid commit/reveal (SN-3). Track bids with
   `sns_premium_commit_total`, and publish the manifest under
   `docs/source/sns/reports/`.
2. **Dutch reopen** — after grace + redemption expire, start a 7‑day Dutch sale
   at 10× that decays 15 % per day. Label manifests with `manifest_id` so the
   dashboard can surface progress.
3. **Renewals** — monitor `sns_registrar_status_total{resolver="renewal"}` and
   capture the autorenew checklist (notifications, SLA, fallback payment rails)
   inside the registrar ticket.

### Developer APIs & automation

- API contracts: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Bulk helper & CSV schema:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- Example command:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
```

Include the manifest ID (`--submission-log` output) in the KPI dashboard filter
so finance can reconcile revenue panels per release.

### Evidence bundle

1. Registrar ticket with contacts, suffix scope, and payment rails.
2. DNS/resolver evidence (zonefile skeletons + GAR proofs).
3. Pricing worksheet + any overrides approved by governance.
4. API/CLI smoke-test artefacts (`curl` samples, CLI transcripts).
5. KPI dashboard screenshot + CSV export, attached to the monthly annex.

## 3. Launch checklist

| Step | Owner | Artefact |
|------|-------|----------|
| Dashboard imported | Product Analytics | Grafana API response + dashboard UID |
| Portal embed validated | Docs/DevRel | `npm run build` logs + preview screenshot |
| DNS rehearsal complete | Networking/Ops | `sns_zonefile_skeleton.py` outputs + runbook log |
| Registrar automation dry run | Registrar Eng | `sns_bulk_onboard.py` submissions log |
| Governance evidence filed | Governance Council | Annex link + SHA-256 of exported dashboard |

Complete the checklist before activating a registrar or suffix. The signed
bundle clears the SN-8 roadmap gate and gives auditors a single reference when
reviewing marketplace launches.
