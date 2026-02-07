---
lang: am
direction: ltr
source: docs/source/ministry/reports/2026-08-mod-red-team-operation-seaglass.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 64cd1112df2f1fc95571ee4ed269e64bde6bf73bd94b19bbf0eaa80a5b43c219
source_last_modified: "2025-12-29T18:16:35.981105+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill — Operation SeaGlass
summary: Evidence and remediation log for the Operation SeaGlass moderation drill (gateway smuggling, governance replay, alert brownout).
---

# Red-Team Drill — Operation SeaGlass

- **Drill ID:** `20260818-operation-seaglass`
- **Date & window:** `2026-08-18 09:00Z – 11:00Z`
- **Scenario class:** `smuggling`
- **Operators:** `Miyu Sato, Liam O'Connor`
- **Dashboards frozen from commit:** `364f9573b`
- **Evidence bundle:** `artifacts/ministry/red-team/2026-08/operation-seaglass/`
- **SoraFS CID (optional):** `not pinned (local bundle only)`
- **Related roadmap items:** `MINFO-9`, plus linked follow-ups `MINFO-RT-17` / `MINFO-RT-18`.

## 1. Objectives & Entry Conditions

- **Primary objectives**
  - Validate denylist TTL enforcement and gateway quarantine during a smuggling attempt while load-shedding alerts.
  - Confirm governance replay detection and alert brownout handling in the moderation runbook.
- **Prerequisites confirmed**
  - `emergency_canon_policy.md` version `v2026-08-seaglass`.
  - `dashboards/grafana/ministry_moderation_overview.json` digest `sha256:ef5210b5b08d219242119ec4ceb61cb68ee4e42ce2eea8a67991fbff95501cc8`.
  - Override authority on-call: `Kenji Ito (GovOps pager)`.

## 2. Execution Timeline

| Timestamp (UTC) | Actor | Action / Command | Result / Notes |
|-----------------|-------|------------------|----------------|
| 09:00:12 | Miyu Sato | Froze dashboards/alerts at `364f9573b` via `scripts/ministry/export_red_team_evidence.py --freeze-only` | Baseline captured and stored under `dashboards/` |
| 09:07:44 | Liam O'Connor | Published denylist snapshot + GAR override to staging with `sorafs_cli ... gateway update-denylist --policy-tier emergency` | Snapshot accepted; override window recorded in Alertmanager |
| 09:17:03 | Miyu Sato | Injected smuggling payload + governance replay using `moderation_payload_tool.py --scenario seaglass` | Alert fired after 3m12s; governance replay flagged |
| 09:31:47 | Liam O'Connor | Ran evidence export and sealed manifest `seaglass_evidence_manifest.json` | Evidence bundle plus hashes stored under `manifests/` |

## 3. Observations & Metrics

| Metric | Target | Observed | Pass/Fail | Notes |
|--------|--------|----------|-----------|-------|
| Alert response latency | <= 5 min | 3.2 min | ✅ | Alert runbook executed without paging churn |
| Moderation detection rate | >= 0.98 | 0.992 | ✅ | Detected both smuggling and replay payloads |
| Gateway anomaly detection | Alert fired | Alert fired + automatic quarantine | ✅ | Quarantine applied before retry budget exhausted |

- `Grafana export:` `artifacts/ministry/red-team/2026-08/operation-seaglass/dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/ministry/red-team/2026-08/operation-seaglass/alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `artifacts/ministry/red-team/2026-08/operation-seaglass/manifests/seaglass_evidence_manifest.json`

## 4. Findings & Remediation

| Severity | Finding | Owner | Target Date | Status / Link |
|----------|---------|-------|-------------|---------------|
| High | Governance replay alert fired, but SoraFS seal was delayed by 2m when the waitlist failover triggered | Governance Ops (Liam O'Connor) | 2026-09-05 | `MINFO-RT-17` open — add replay seal automation to the failover path |
| Medium | Dashboard freeze not pinned to SoraFS; operators relied on local bundle | Observability (Miyu Sato) | 2026-08-25 | `MINFO-RT-18` open — pin `dashboards/*` to SoraFS with signed CID before next drill |
| Low | CLI logbook omitted Norito manifest hash in first pass | Ministry Ops (Kenji Ito) | 2026-08-22 | Fixed during drill; template updated in logbook |

Document how calibration manifests, denylist policies, or SDK/tooling must change. Link to GitHub/Jira issues and note blocked/unblocked states.

## 5. Governance & Approvals

- **Incident commander sign-off:** `Miyu Sato @ 2026-08-18T11:22Z`
- **Governance council review date:** `GovOps-2026-08-22`
- **Follow-up checklist:** `[x] status.md updated`, `[x] roadmap row updated`, `[x] transparency packet annotated`

## 6. Attachments

- `[x] CLI logbook (logs/operation_seaglass.log)`
- `[x] Dashboard JSON export`
- `[x] Alertmanager history`
- `[x] SoraFS manifest / CAR`
- `[ ] Override audit log`

Mark each attachment with `[x]` once uploaded to the evidence bundle and SoraFS snapshot.

---

_Last updated: 2026-08-18_
