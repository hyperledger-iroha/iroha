---
lang: am
direction: ltr
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3649db9b00f9be968cfeb98bc34bbc797aaf22d7ac3936698b4f562094911073
source_last_modified: "2025-12-29T18:16:35.173090+00:00"
translation_last_reviewed: 2026-02-07
title: SNS KPI dashboard
description: Live Grafana panels that aggregate registrar, freeze, and revenue metrics for SN-8a.
---

# Sora Name Service KPI Dashboard

The KPI dashboard gives stewards, guardians, and regulators a single place to
review adoption, error, and revenue signals before the monthly annex cadence
(SN-8a). The Grafana definition ships in the repository at
`dashboards/grafana/sns_suffix_analytics.json` and the portal mirrors the same
panels via an embedded iframe so the experience matches the internal Grafana
instance.

## Filters & Data Sources

- **Suffix filter** – drives the `sns_registrar_status_total{suffix}` queries so
  `.sora`, `.nexus`, and `.dao` can be inspected independently.
- **Bulk release filter** – scopes the `sns_bulk_release_payment_*` metrics so
  finance can reconcile a specific registrar manifest.
- **Metrics** – pulls from Torii (`sns_registrar_status_total`,
  `torii_request_duration_seconds`), guardian CLI (`guardian_freeze_active`),
  `sns_governance_activation_total`, and the bulk-onboarding helper metrics.

## Panels

1. **Registrations (last 24h)** – number of successful registrar events for the
   selected suffix.
2. **Governance activations (30d)** – charter/addendum motions recorded by the
   CLI.
3. **Registrar throughput** – per-suffix rate of successful registrar actions.
4. **Registrar error modes** – 5 minute rate of error-labelled
   `sns_registrar_status_total` counters.
5. **Guardian freeze windows** – live selectors where `guardian_freeze_active`
   reports an open freeze ticket.
6. **Net payment units by asset** – totals reported by
   `sns_bulk_release_payment_net_units` per asset.
7. **Bulk requests per suffix** – manifest volumes per suffix id.
8. **Net units per request** – ARPU-style calculation derived from the release
   metrics.

## Monthly KPI Review Checklist

The finance lead drives a recurring review on the first Tuesday of every month:

1. Open the portal’s **Analytics → SNS KPI** page (or Grafana dashboard `sns-kpis`).
2. Capture a PDF/CSV export of the registrar throughput and revenue tables.
3. Compare suffixes for SLA breaches (error rate spikes, frozen selectors >72 h,
   ARPU deltas >10 %).
4. Log summaries + action items in the relevant annex entry under
   `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
5. Attach the exported dashboard artefacts to the annex commit and link them in
   the council agenda.

If the review uncovers SLA breaches, file a PagerDuty incident for the affected
owner (registrar duty manager, guardian on-call, or steward program lead) and
track the remediation in the annex log.
