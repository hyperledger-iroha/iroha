---
lang: es
direction: ltr
source: docs/source/ministry/reports/moderation_red_team_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bfdf5696bf3a58e7899e7d7b2ba77e404a05fa81304f12d6c78eeb1e8035e5
source_last_modified: "2026-01-03T18:07:57.925651+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Red-Team Drill Report Template
summary: Copy this file for every MINFO-9 drill to capture metadata, evidence, and remediation actions.
---

> **How to use:** duplicate this template to `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` immediately after each drill. Keep filenames lowercase, hyphenated, and aligned with the drill ID logged in Alertmanager.

# Red-Team Drill Report — `<SCENARIO NAME>`

- **Drill ID:** `<YYYYMMDD>-<scenario>`
- **Date & window:** `<YYYY-MM-DD HH:MMZ – HH:MMZ>`
- **Scenario class:** `smuggling | bribery | gateway | ...`
- **Operators:** `<names / handles>`
- **Dashboards frozen from commit:** `<git SHA>`
- **Evidence bundle:** `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`
- **SoraFS CID (optional):** `<cid>`  
- **Related roadmap items:** `MINFO-9`, plus any linked tickets.

## 1. Objectives & Entry Conditions

- **Primary objectives**
  - `<e.g. Verify denylist TTL enforcement under smuggling attack>`
- **Prerequisites confirmed**
  - `emergency_canon_policy.md` version `<tag>`
  - `dashboards/grafana/ministry_moderation_overview.json` digest `<sha256>`
  - Override authority on-call: `<name>`

## 2. Execution Timeline

| Timestamp (UTC) | Actor | Action / Command | Result / Notes |
|-----------------|-------|------------------|----------------|
|  |  |  |  |

> Include Torii request IDs, chunk hashes, override approvals, and Alertmanager links.

## 3. Observations & Metrics

| Metric | Target | Observed | Pass/Fail | Notes |
|--------|--------|----------|-----------|-------|
| Alert response latency | `<X> min` | `<Y> min` | ✅/⚠️ |  |
| Moderation detection rate | `>= <value>` |  |  |  |
| Gateway anomaly detection | `Alert fired` |  |  |  |

- `Grafana export:` `artifacts/.../dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/.../alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `<path>`

## 4. Findings & Remediation

| Severity | Finding | Owner | Target Date | Status / Link |
|----------|---------|-------|-------------|---------------|
| High |  |  |  |  |

Document how calibration manifests, denylist policies, or SDK/tooling must change. Link to GitHub/Jira issues and note blocked/unblocked states.

## 5. Governance & Approvals

- **Incident commander sign-off:** `<name / timestamp>`
- **Governance council review date:** `<meeting id>`
- **Follow-up checklist:** `[ ] status.md updated`, `[ ] roadmap row updated`, `[ ] transparency packet annotated`

## 6. Attachments

- `[ ] CLI logbook (`logs/<file>.md`)`
- `[ ] Dashboard JSON export`
- `[ ] Alertmanager history`
- `[ ] SoraFS manifest / CAR`
- `[ ] Override audit log`

Mark each attachment with `[x]` once uploaded to the evidence bundle and SoraFS snapshot.

---

_Last updated: {{ date | default("2026-02-20") }}_
