---
lang: ar
direction: rtl
source: docs/source/sorafs/postmortem_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7ed57867c09440d713694d5603ada2b20202d823025ba5cbe8e606b82575dcc5
source_last_modified: "2026-01-03T18:07:58.409585+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Incident Postmortem Template
summary: Structured template for analysing SoraFS incidents and chaos drills.
---

# SoraFS Incident Postmortem Template

> Duplicate this file and replace the placeholders. Attach dashboards,
> telemetry snippets, and governance decisions as appendices.

## Summary

- **Incident / Drill ID:** `YYYY-MM-DD-<short-code>`
- **Date / Timeframe (UTC):** `<start> – <end>`
- **Classification:** `<P1|P2|Drill>`
- **Primary Impact:** `<gateway availability | proof integrity | replication backlog | other>`
- **Detected by:** `<alert name / manual report>`
- **Resolved by:** `<team / individual>`

## Impact Assessment

- **Customer-facing symptoms:** `<API errors, latency, backlog>`
- **Duration above SLO:** `<minutes>`
- **Affected providers / manifests:** `<list>`
- **Downstream consequences:** `<governance actions, replay required?>`

## Timeline

| Timestamp (UTC) | Actor | Event |
|-----------------|-------|-------|
| `00:00` | `<system/team>` | Detection |
| `00:05` | | Initial mitigation |
| `...` | | |

## Root Cause Analysis

- **Primary cause:** `<component + failure mode>`
- **Contributing factors:** `<config drift, telemetry gap, coordination>`
- **What worked:** `<items that reduced impact>`
- **What failed:** `<runbook gaps, tooling issues>`

## Telemetry & Evidence

- Attach relevant metrics (e.g., `sorafs_fetch.error`, `sorafs_gateway_latency_ms_bucket`).
- Link Grafana panel snapshots or logs with timestamps.

## Corrective Actions

| Action | Owner | Priority | Target Date |
|--------|-------|----------|-------------|
| `<Fix replication backlog alert thresholds>` | `<SRE>` | High | `<YYYY-MM-DD>` |

## Follow-Up Verification

- **Chaos drill scheduled:** `<yes/no> – planned date>`
- **Runbook updates required:** `<docs updated or pending>`
- **Governance / stakeholder notification:** `<completed / outstanding>`

## Lessons Learned

- `<Key observation>`
- `<Process improvement>`

## Appendix

- Links to related issues, commits, and dashboards.
- Attach artefacts (manifest snapshots, proof bundles) if applicable.
