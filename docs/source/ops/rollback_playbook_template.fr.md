---
lang: fr
direction: ltr
source: docs/source/ops/rollback_playbook_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 45e4f55b0baa9837f3db70bbb07af136e8607c1d31a7598a807d1cfbe2559867
source_last_modified: "2026-01-20T06:05:23.061428+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Rollback Playbook Template
summary: Checklist for reversing roadmap launches (SoraNet transport, SNS registrar, DA ingestion, CDN, etc.) and capturing auditable evidence.
---

# Rollback Playbook Template

Use this template to prepare rollback plans for roadmap-driven launches: SoraNet
transport policy flips, SNS registrar upgrades, DA ingestion changes, CDN roll
outs, and similar high-risk activities. Keep the completed playbook in the
release evidence folder and link it from the corresponding roadmap item.

## 1. Metadata

- **Feature / rollout name:** `<SF-6c multi-source default | SNNet-5 guard pinning | DA ingest pipeline | CDN PoP rollout>`
- **Owner / DRI:** `<team lead>`
- **Change window:** `<YYYY-MM-DD hh:mm UTC>`
- **Rollback window:** `<YYYY-MM-DD hh:mm UTC>`
- **Critical dependencies:** `<Torii version, orchestrator release, governance approval>`
- **Related tickets / RFCs:** `<links>`

## 2. Trigger Conditions

List the metrics, alerts, or governance decisions that require a rollback.
Examples:

- `sorafs_orchestrator_adoption_check` failure
- `sns_registry.publish_error_total` > threshold
- DA proof mismatch in `integration_tests/tests/taikai_da.rs`
- CDN PoP failover SLO miss > 5 minutes

For each trigger provide the detection method, escalation path, and who can
authorize a rollback (e.g., Storage TL, Governance duty officer).

## 3. Prerequisites & Safeguards

- Backups / snapshots captured? (list paths, hashes)
- Telemetry dashboards bookmarked? (link dashboards/alerts)
- Communication plan drafted? (email template, status page copy)
  `soranet-strict`, `da.ingestion.enabled`, etc.)

## 4. Step-by-Step Rollback

| Step | Command / Action | Owner | Notes |
|------|------------------|-------|-------|
| 1 | `<Disable feature flag / toggle config>` | `<Name>` | Reference config path (`iroha_config`, Kubernetes secret, etc.). |
| 2 | `<Redeploy previous container/image>` | `<Name>` | Include build hash / provenance. |
| 3 | `<Rebuild index / flush cache>` | `<Name>` | Example: `scripts/sorafs_cache_reset.sh`. |
| 4 | `<Notify governance channel>` | `<Name>` | Provide template snippet. |

Include automation references (e.g., `cargo xtask sorafs-adoption-check`,
`scripts/nexus_lane_registry_bundle.sh`, `ci/check_sorafs_gateway_probe.sh`) so
operators know which tools provide evidence.

## 5. Verification Checklist

- Metrics back within SLOs (list exact counters / dashboards).
- Norito artefacts restored (hash comparisons attached?).
- End-to-end tests re-run (`cargo test -p <crate> <suite>` or `swift test`, etc.).
- Governance approval recorded (ticket or council decision reference).

## 6. Communications

| Audience | Channel | Owner | Status Message |
|----------|---------|-------|----------------|
| Operators / partners | `#sorafs-ops`, email distro | `<Name>` | `<template snippet>` |
| Governance council | Agenda / pre-read | `<Name>` | `Rollback executed at hh:mm UTC; evidence attached.` |
| Incident log | `docs/source/ops/archive/<id>/` | `<Name>` | Link to postmortem template once filled. |

## 7. Post-Rollback Actions

- Root-cause analysis owner assigned? `<Name>`
- Follow-up issues filed? `<links>`
- Retry / relaunch criteria defined? `<conditions>`
- Lessons learned scheduled for next governance sync? `<date>`

## 8. Attachments

List any supporting documents: diff summaries, dashboards, log exports, config
snapshots, or release manifests.
