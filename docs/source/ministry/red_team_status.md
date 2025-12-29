---
title: Ministry Red-Team Status (MINFO-9)
summary: Snapshot of the chaos drill program covering upcoming runs, last completed scenario, and remediation items.
---

# Ministry Red-Team Status

This page complements the [Moderation Red-Team Plan](moderation_red_team_plan.md)
by tracking the near-term drill calendar, evidence bundles, and remediation
status. Update it after every run alongside the artefacts captured under
`artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`.

## Upcoming Drills

| Date (UTC) | Scenario | Owner(s) | Evidence Prep | Notes |
|------------|---------|----------|---------------|-------|
| 2026-11-12 | **Operation Blindfold** — Taikai mixed-mode smuggling rehearsal with gateway downgrade attempts | Security Engineering (Miyu Sato), Ministry Ops (Liam O’Connor) | `scripts/ministry/scaffold_red_team_drill.py` bundle `docs/source/ministry/reports/red_team/2026-11-operation-blindfold.md` + staging directory `artifacts/ministry/red-team/2026-11/operation-blindfold/` | Exercises GAR/Taikai overlap plus DNS failover; requires denylist Merkle snapshot before start and `export_red_team_evidence.py` run after dashboards are captured. |

## Last Drill Snapshot

| Date (UTC) | Scenario | Evidence Bundle | Remediation & Follow-Ups |
|------------|---------|-----------------|--------------------------|
| 2026-08-18 | **Operation SeaGlass** — Gateway smuggling, governance replay, and alert brownout rehearsal | `artifacts/ministry/red-team/2026-08/operation-seaglass/` (Grafana exports, Alertmanager logs, `seaglass_evidence_manifest.json`) | **Open:** replay seal automation (`MINFO-RT-17`, owner: Governance Ops, due 2026-09-05); pin dashboard freeze to SoraFS (`MINFO-RT-18`, Observability, due 2026-08-25). **Closed:** logbook template updated to carry Norito manifest hashes. |

## Tracking & Tooling

- Use `scripts/ministry/moderation_payload_tool.py` to package injectible
  payloads and denylist patches per scenario.
- Record dashboard/log captures via `scripts/ministry/export_red_team_evidence.py`
  immediately after each drill so the evidence manifest contains signed hashes.
- CI guard `ci/check_ministry_red_team.sh` enforces that committed drill reports
  do not contain placeholder text and that referenced artefacts exist before
  merging.

See `status.md` (§ *Ministry red-team status*) for the live summary referenced
in weekly coordination calls.
