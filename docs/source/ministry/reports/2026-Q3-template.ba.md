---
lang: ba
direction: ltr
source: docs/source/ministry/reports/2026-Q3-template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f313d8010f2a7174c90f51dea512bcab6eb4a207df9199f28a7352944cb43c8b
source_last_modified: "2025-12-29T18:16:35.981653+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Transparency Report — 2026 Q3 (Template)
summary: Scaffold for the MINFO-8 quarterly transparency packet; replace all tokens before publication.
quarter: 2026-Q3
---

<!--
  Usage:
    1. Copy this file when drafting a new quarter (e.g., 2026-Q3 → 2026-Q3.md).
    2. Replace every {{TOKEN}} marker and remove instructional callouts.
    3. Attach supporting artefacts (data appendix, CSVs, manifest, Grafana export) under artifacts/ministry/transparency/<YYYY-Q>/.
-->

# Executive Summary

> Provide a one-paragraph summary of moderation accuracy, appeal outcomes, denylist churn, and treasury highlights. Mention whether release met the T+14 deadline.

## Quarter in Review

### Highlights
- {{HIGHLIGHT_1}}
- {{HIGHLIGHT_2}}
- {{HIGHLIGHT_3}}

### Risks & Mitigations

| Risk | Impact | Mitigation | Owner | Status |
|------|--------|------------|-------|--------|
| {{RISK_1}} | {{Impact}} | {{Mitigation}} | {{Owner}} | {{Status}} |
| {{RISK_2}} | {{Impact}} | {{Mitigation}} | {{Owner}} | {{Status}} |

## Metrics Overview

All metrics originate from `ministry_transparency_builder` (Norito bundle) after the DP sanitizer runs. Attach corresponding CSV slices referenced below.

### AI Moderation Accuracy

| Model Profile | Region | FP Rate (Target) | FN Rate (Target) | Drift vs Calibration | Sample Size | Notes |
|---------------|--------|------------------|------------------|----------------------|-------------|-------|
| {{profile}} | {{region}} | {{fp_rate}} ({{fp_target}}) | {{fn_rate}} ({{fn_target}}) | {{drift}} | {{samples}} | {{notes}} |

### Appeals & Panel Activity

| Metric | Value | SLA Target | Trend vs Q-1 | Notes |
|--------|-------|------------|--------------|-------|
| Appeals received | {{appeals_received}} | {{sla}} | {{delta}} | {{notes}} |
| Median resolution time | {{median_resolution}} | {{sla}} | {{delta}} | {{notes}} |
| Reversal rate | {{reversal_rate}} | {{target}} | {{delta}} | {{notes}} |
| Panel utilization | {{panel_utilization}} | {{target}} | {{delta}} | {{notes}} |

### Denylist & Emergency Canon

| Metric | Count | DP Noise (ε) | Emergency Flags | TTL Compliance | Notes |
|--------|-------|--------------|-----------------|----------------|-------|
| Hash additions | {{additions}} | {{epsilon_counts}} | {{flags}} | {{ttl_status}} | {{notes}} |
| Hash removals | {{removals}} | {{epsilon_counts}} | {{flags}} | {{ttl_status}} | {{notes}} |
| Canon invocations | {{canon_invocations}} | n/a | {{flags}} | {{ttl_status}} | {{notes}} |

### Treasury Movements

| Flow | Amount (MINFO) | Source Reference | Notes |
|------|----------------|------------------|-------|
| Appeal deposits | {{amount}} | {{tx_ref}} | {{notes}} |
| Panel rewards | {{amount}} | {{tx_ref}} | {{notes}} |
| Operational spend | {{amount}} | {{tx_ref}} | {{notes}} |

### Volunteer & Outreach Signals

| Metric | Value | Target | Notes |
|--------|-------|--------|-------|
| Volunteer briefs published | {{value}} | {{target}} | {{notes}} |
| Languages covered | {{value}} | {{target}} | {{notes}} |
| Governance workshops hosted | {{value}} | {{target}} | {{notes}} |

## Differential Privacy & Sanitization

Summarise the sanitizer run and include the RNG commitment.

- Sanitizer job: `{{CI_JOB_URL}}`
- DP parameters: ε = {{epsilon_total}}, δ = {{delta_total}}
- RNG commitment: `{{blake3_seed_commitment}}`
- Buckets suppressed: {{suppressed_buckets}}
- QA reviewer: {{reviewer}}

Attach `artifacts/ministry/transparency/{{Quarter}}/dp_report.json` and note any manual interventions.

## Data Attachments

| Artefact | Path | SHA-256 | Uploaded to SoraFS? | Notes |
|----------|------|---------|---------------------|-------|
| Summary PDF | `artifacts/ministry/transparency/{{Quarter}}/summary.pdf` | {{hash}} | {{Yes/No}} | {{notes}} |
| Norito data appendix | `artifacts/ministry/transparency/{{Quarter}}/data/appendix.norito` | {{hash}} | {{Yes/No}} | {{notes}} |
| Metrics CSV bundle | `artifacts/ministry/transparency/{{Quarter}}/data/csv/` | {{hash}} | {{Yes/No}} | {{notes}} |
| Grafana export | `dashboards/grafana/ministry_transparency_overview.json` | {{hash}} | {{Yes/No}} | {{notes}} |
| Alert rules | `dashboards/alerts/ministry_transparency_rules.yml` | {{hash}} | {{Yes/No}} | {{notes}} |
| Provenance manifest | `artifacts/ministry/transparency/{{Quarter}}/manifest.json` | {{hash}} | {{Yes/No}} | {{notes}} |
| Manifest signature | `artifacts/ministry/transparency/{{Quarter}}/manifest.json.sig` | {{hash}} | {{Yes/No}} | {{notes}} |

## Publication Metadata

| Field | Value |
|-------|-------|
| Release quarter | {{Quarter}} |
| Release timestamp (UTC) | {{timestamp}} |
| SoraFS CID | `{{cid}}` |
| Governance vote ID | {{vote_id}} |
| Manifest digest (`blake2b`) | `{{manifest_digest}}` |
| Git commit / tag | `{{git_rev}}` |
| Release owner | {{owner}} |

## Approvals

| Role | Name | Decision | Timestamp | Notes |
|------|------|----------|-----------|-------|
| Ministry Observability TL | {{name}} | ✅/⚠️ | {{timestamp}} | {{notes}} |
| Governance Council Liaison | {{name}} | ✅/⚠️ | {{timestamp}} | {{notes}} |
| Docs/Comms Lead | {{name}} | ✅/⚠️ | {{timestamp}} | {{notes}} |

## Changelog & Follow-Ups

- {{CHANGELOG_ITEM_1}}
- {{CHANGELOG_ITEM_2}}

### Open Action Items

| Item | Owner | Due | Status | Notes |
|------|-------|-----|--------|-------|
| {{Action}} | {{Owner}} | {{Due}} | {{Status}} | {{Notes}} |

### Contact

- Primary contact: {{contact_name}} (`{{chat_handle}}`)
- Escalation path: {{escalation_details}}
- Distribution list: {{mailing_list}}

_Template version: 2026-03-25. Update the revision date when making structural changes._
