---
lang: az
direction: ltr
source: docs/source/ministry/moderation_red_team_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7ecf637fa28f68a997367118163224caecc6c71c604d5e7ece409941d5374f44
source_last_modified: "2025-12-29T18:16:35.978869+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team & Chaos Drill Plan
summary: Execution plan for roadmap item MINFO-9 covering recurring adversarial campaigns, telemetry hooks, and reporting requirements.
---

# Moderation Red-Team & Chaos Drills (MINFO-9)

Roadmap reference: **MINFO-9 — Moderation red-team & chaos drills** (targets Q3 2026)

The Ministry of Information must run reproducible adversarial campaigns that stress AI moderation pipelines, bounty programs, gateways, and governance controls. This plan closes the “🈳 Not Started” gap noted in the roadmap by defining scope, cadence, drill templates, and evidence requirements that tie directly into the existing moderation dashboards (`dashboards/grafana/ministry_moderation_overview.json`) and emergency canon workflows.

## Goals & Deliverables

- Run quarterly red-team exercises that cover multi-part smuggling attempts, bribery/appeal tampering, and gateway side-channel probes before widening coverage to additional threat classes.
- Capture deterministic artefacts (CLI logs, Norito manifests, Grafana exports, SoraFS CIDs) for every drill and file them via `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md`.
- Feed drill output back into calibration manifests (`docs/examples/ai_moderation_calibration_*.json`) and the denylist policy so remediation tasks become traceable roadmap tickets.
- Wire alert/runbook integration so failures surface alongside the MINFO-1 dashboards and Alertmanager packs (`dashboards/alerts/ministry_moderation_rules.yml`).

## Scope & Dependencies

- **Systems under test:** SoraFS ingest/orchestrator paths, AI moderation runner defined in `docs/source/sorafs_ai_moderation_plan.md`, denylist/Merkle enforcement, appeal treasury tooling, and gateway rate-limits.
- **Prerequisites:** Emergency canon & TTL policy (`docs/source/ministry/emergency_canon_policy.md`), moderation calibration fixtures, and Torii mock harness parity so reproducible payloads can be replayed during chaos runs.
- **Out-of-scope:** SoraDNS policies, Kaigi conferencing, or non-Ministry communications channels (tracked separately under SNNet and DOCS-SORA programs).

## Roles & Responsibilities

| Role | Responsibilities | Primary Owner | Backup |
|------|------------------|---------------|--------|
| Drill Director | Approves scenario list, assigns red-teamers, signs off on runbooks | Ministry Security Lead | Deputy Moderator |
| Adversarial Cell | Crafts payloads, runs attacks, records evidence | Security Engineering guild | Volunteer operators |
| Observability Lead | Monitors dashboards/alerts, captures Grafana exports, files incident timelines | SRE / Observability TL | On-call SRE |
| Moderator on Duty | Drives escalation flow, validates override requests, updates emergency canon records | Incident commander | Reserve commander |
| Reporting Scribe | Populates the template under `docs/source/ministry/reports/moderation_red_team_template.md`, links artefacts, opens follow-up issues | Docs/DevRel | Product liaison |

## Cadence & Timeline

| Phase | Target Window | Key Activities | Artefacts |
|-------|---------------|----------------|-----------|
| **Plan** | T−4 weeks | Select scenarios, refresh payload fixtures, scaffold artefacts via `scripts/ministry/scaffold_red_team_drill.py`, and dry-run the telemetry helpers (`scripts/telemetry/check_redaction_status.py`, `ci/run_android_telemetry_chaos_prep.sh`) that the drills mirror | Scenario briefs, ticket tracker |
| **Ready** | T−1 week | Lock participants, stage SoraFS/Torii sandboxes, freeze dashboards/alert hashes | Ready checklist, dashboard digests |
| **Execute** | Drill day (4 h) | Launch adversarial flows, collect Alertmanager notifications, capture Torii/CLI traces, enforce override approvals | Live logbook, Grafana snapshots |
| **Recover** | T+1 day | Revert overrides, scrub datasets, archive artefacts to `artifacts/ministry/red-team/<YYYY-MM>/` and SoraFS | Evidence bundle, manifest |
| **Report** | T+1 week | Publish Markdown report from template, log remediation tickets, update roadmap/status.md | Report file, Jira/GitHub links |

Quarterly drills (Mar/Jun/Sep/Dec) run at minimum; high-risk findings trigger ad-hoc runs that follow the same evidence workflow.

## Scenario Library (Initial)

| Scenario | Description | Success Signals | Evidence Inputs |
|----------|-------------|-----------------|-----------------|
| Multi-part smuggling | Chain of chunks spread across SoraFS providers with polymorphic payloads that attempt to bypass AI filters over time. Exercises orchestrator range fetches, moderation TTLs, and denylist propagation. | Smuggling detected before user delivery; denylist delta emitted; `ministry_moderation_overview` alerts fire within SLA. | CLI replay logs, chunk manifests, denylist diff, Trace IDs from `sorafs.fetch.*` dashboards. |
| Bribery & appeal tampering | Pairs of malicious moderators attempt to approve bribe-induced overrides; tests treasury flows, override approvals, and audit logging. | Override logged with mandatory evidence, treasury transfers flagged, governance vote recorded. | Norito override records, `docs/source/ministry/volunteer_brief_template.md` updates, treasury ledger entries. |
| Gateway side-channel probing | Simulates rogue publishers measuring cache timing and TTLs to infer moderated content. Exercises CDN/gateway hardening before SNNet-15. | Rate-limit & anomaly dashboards highlight probes; admin CLI shows policy enforcement; no content leak. | Gateway access logs, Grafana scrape of `ministry_gateway_observability` panels, capture packet traces (pcap) for offline review. |

Future iterations will add `honey-payload beacons`, `AI adversarial prompt floods`, and `SoraFS metadata poisoning` once the initial three scenarios graduate from the learning cadence.

## Execution Checklist

1. **Pre-Drill**
   - Confirm runbook + scenario doc is published and approved.
   - Snapshot dashboards (`dashboards/grafana/ministry_moderation_overview.json`) and alert rules to `artifacts/ministry/red-team/<YYYY-MM>/dashboards/`.
   - Use `scripts/ministry/scaffold_red_team_drill.py` to create the report + artefact directories, then record SHA256 digests for any fixture bundles staged for the drill.
   - Verify denylist Merkle roots and emergency canon notes before injecting adversarial payloads.
2. **During Drill**
   - Log every action (timestamp, operator, command) into the live logbook (shared doc or `docs/source/ministry/reports/tmp/<timestamp>.md`).
   - Capture Torii responses and AI moderation verdicts, including request IDs, model names, and risk scores.
   - Exercise the escalation workflow (override request → commander approval → `emergency_canon_policy` update).
   - Trigger at least one alert-clearing exercise and document response latency.
3. **Post-Drill**
   - Clear overrides, roll denylist entries back to production values, and verify alerts quiet.
   - Export Grafana/Alertmanager history, CLI logs, Norito manifests, and attach them to the evidence bundle.
   - File remediation issues (high/medium/low) with owners and due dates; link to the final report.

## Telemetry, Metrics & Evidence

- **Dashboards:** `dashboards/grafana/ministry_moderation_overview.json` + future `ministry_red_team_heatmap.json` (placeholder) capture live signals. Export JSON snapshots per drill.
- **Alerting:** `dashboards/alerts/ministry_moderation_rules.yml` plus upcoming `ministry_red_team_rules.yml` must include annotations referencing the drill ID and scenario to simplify audits.
- **Norito artefacts:** encode every drill run as `RedTeamDrillV1` events (spec forthcoming) so Torii/CLI exports are deterministic and can be shared with governance.
- **Report template:** copy `docs/source/ministry/reports/moderation_red_team_template.md` and fill in the scenario, metrics, evidence digests, remediation status, and governance sign-off.
- **Archive:** Store artefacts in `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/` (logs, CLI output, Norito bundles, dashboard exports) and publish a matching SoraFS CAR manifest for public review when governance approves.

## Automation & Next Steps

1. Implement helper scripts under `scripts/ministry/` to seed payload fixtures, toggle denylist entries, and collect CLI/Grafana exports. (`scaffold_red_team_drill.py`, `moderation_payload_tool.py`, and `check_red_team_reports.py` now cover scaffolding, payload bundling, denylist patching, and placeholder enforcement; `export_red_team_evidence.py` adds the missing dashboard/log export with Grafana API support so evidence manifests stay deterministic.)
2. Extend CI with `ci/check_ministry_red_team.sh` to verify template completeness and evidence digests before merging reports. ✅ (`scripts/ministry/check_red_team_reports.py` enforces placeholder removal across all committed drill reports.)
3. Add `ministry_red_team_status` section to `status.md` to surface upcoming drills, open remediation items, and last-run metrics.
4. Integrate drill metadata into the transparency pipeline so quarterly reports can reference the most recent chaos results.
5. Feed drill reports directly into `cargo xtask ministry-transparency ingest --red-team-report <path>...` so sanitized quarterly metrics and governance manifests carry the drill IDs, evidence bundles, and dashboard SHAs alongside the existing ledger/appeal/denylist feeds.

Once these steps land, MINFO-9 transitions from 🈳 Not Started to 🈺 In Progress with traceable artefacts and measurable success criteria.
