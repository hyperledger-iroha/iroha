---
lang: my
direction: ltr
source: docs/source/ops/postmortem_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6c34855ca21b3c59e38a0f78b9c41ef5808917b18475d9f04d4ee1da6ca9a24d
source_last_modified: "2025-12-29T18:16:36.004827+00:00"
translation_last_reviewed: 2026-02-07
title: Unified Incident Postmortem Template
summary: Cross-program template for analysing SoraNet transport, SNS registrar, DA ingestion, or CDN incidents and drills.
---

# Unified Incident Postmortem Template

Duplicate this template whenever a roadmap-controlled subsystem (SoraNet
transport, SNS registrar, DA ingestion, SoraNet CDN, etc.) experiences a
production incident or scheduled chaos drill. Replace the placeholder sections
with concrete evidence (dashboards, Norito artefacts, governance minutes). When
completed, attach the document to the relevant release folder and reference it
from `status.md` and the roadmap milestone log.

## 1. Summary

- **Incident / Drill ID:** `YYYY-MM-DD-<short-code>`
- **System(s) impacted:** `<SoraNet transport | SNS registrar | DA ingestion | CDN>`
- **Severity / Classification:** `<P1|P2|P3|Drill>`
- **Date & time (UTC):** `<start → end>`
- **Detected by:** `<alert / audit / drill>`
- **Mitigation owners:** `<Primary on-call + backup>`
- **Resolution timestamp:** `<UTC>`

## 2. Impact Statement

- **Customer-facing symptoms:** `<API errors, latency, governance delays>`
- **SLOs breached:** `<SLA + duration>`
- **Affected workloads:** `<providers, registrars, data spaces>`
- **Downstream consequences:** `<rollbacks, manifest freezes, governance votes>`

## 3. Timeline

| Timestamp (UTC) | Actor | Event |
|-----------------|-------|-------|
| `00:00` | `<Alert/Grafana>` | Detection (include dashboard link) |
| `00:05` | `<On-call>` | Initial response |
| ... | | |

Include references to PagerDuty/On-call ticket IDs and attach chat summaries when
available.

## 4. Root Cause Analysis

- **Primary cause:** `<component + failure mode>`
- **Contributing factors:** `<config drift, dependency outage, telemetry gap>`
- **What worked:** `<runbooks, automation>`
- **What failed:** `<missing alarms, docs, tooling>`
- **Operator friction:** `<manual steps that should be automated>`

## 5. Evidence & Telemetry

- Grafana / dashboard snapshots (link + timestamp).
- Norito artefacts / manifests (`.json`, `.to`, proof bundles) with SHA-256
  hashes.
- Logs or queue exports (attach `PendingQueueInspector` or DA prover output
  when relevant).
- Governance tickets or consent records referencing the incident.

## 6. Remediation Plan

| Action | Owner | Priority | Target Date |
|--------|-------|----------|-------------|
| `<example: tighten SNNet-5 policy guardrail>` | `<Name>` | High | `<YYYY-MM-DD>` |

Document static analysis / CI updates, dashboard additions, or incident command
training that must happen to prevent recurrence.

## 7. Verification & Follow-Up

- **Chaos drill scheduled?** `<Yes/No – include planned date>`
- **Runbooks updated?** `<link + PR>`
- **Telemetry / alert changes deployed?** `<Yes/No – include evidence>`
- **Governance / stakeholder notification:** `<complete/pending>`

## 8. Lessons Learned

Summarise key observations for engineering, ops, and governance. Call out policy
or tooling changes that require roadmap updates.

## 9. Appendices

- Links to commits, manifests, tickets, and dashboards.
- Attach supporting files (compressed queue dumps, replay traces, etc.).
- Include a checklist confirming all artefacts are stored in the incident
  archive (`docs/source/ops/archive/<incident-id>/`).
