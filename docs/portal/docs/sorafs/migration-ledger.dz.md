---
lang: dz
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ae38d9ff7f10a14e63f6d47490dbbe56c9d3b207a30a5899e63414cb726a88f7
source_last_modified: "2026-01-05T09:28:11.880522+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Migration Ledger"
description: "Canonical change log tracking every migration milestone, owners, and required follow-ups."
---

> Adapted from [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# SoraFS Migration Ledger

This ledger mirrors the migration change log captured in the SoraFS
Architecture RFC. Entries are grouped by milestone and list the effective
window, impacted teams, and required actions. Updates to the migration plan
MUST modify both this page and the RFC (`docs/source/sorafs_architecture_rfc.md`)
to keep downstream consumers aligned.

| Milestone | Effective Window | Change Summary | Impacted Teams | Action Items | Status |
|-----------|------------------|----------------|----------------|--------------|--------|
| M1 | Weeks&nbsp;7–12 | CI enforces deterministic fixtures; alias proofs available in staging; tooling exposes explicit expectation flags. | Docs, Storage, Governance | Ensure fixtures stay signed, register aliases in staging registry, update release checklists with `--car-digest/--root-cid` enforcement. | ⏳ Pending |

Governance control plane minutes referencing these milestones live under
`docs/source/sorafs/`. Teams should add dated bullet points beneath each row
when notable events occur (e.g., new alias registrations, registry incident
retrospectives) to provide an auditable paper trail.

## Recent Updates

- 2025-11-01 — Circulated `migration_roadmap.md` to governance council and
  operator lists for review; awaiting sign-off at the next council session
  (ref: `docs/source/sorafs/council_minutes_2025-10-29.md` follow-up).
- 2025-11-02 — Pin Registry register ISI now enforces shared chunker/policy
  validation via `sorafs_manifest` helpers, keeping on-chain paths aligned
  with Torii checks.
- 2026-02-13 — Added provider advert rollout phases (R0–R3) to the ledger and
  published the associated dashboards and operator guidance
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).

