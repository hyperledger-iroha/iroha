---
lang: kk
direction: ltr
source: docs/source/sorafs/migration_ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0e479cae4018bbbc689fba16e2e59f93af50f1fad35509a65bce80e09e62186
source_last_modified: "2026-01-05T09:28:12.077152+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Migration Ledger

This ledger mirrors the migration change log captured in the SoraFS
Architecture RFC. Entries are grouped by milestone and list the effective window,
impacted teams, and required actions. Updates to the migration plan MUST modify
both this file and the RFC (`docs/source/sorafs_architecture_rfc.md`) to keep
downstream consumers aligned.

| Milestone | Effective Window | Change Summary | Impacted Teams | Action Items | Status |
|-----------|------------------|----------------|----------------|--------------|--------|
| M1 | Weeks 7–12 | CI enforces deterministic fixtures; alias proofs available in staging; tooling exposes explicit expectation flags. | Docs, Storage, Governance | Ensure fixtures stay signed, register aliases in staging registry, update release checklists with `--car-digest/--root-cid` enforcement. | ⏳ Pending |

Governance control plane minutes referencing these milestones are stored under
`docs/source/sorafs/`. Teams should add dated bullet points beneath each row
when notable events occur (e.g., new alias registrations, registry incident
retrospectives) to provide an auditable paper trail.

## Recent Updates

- 2025-11-01 — Circulated `migration_roadmap.md` to governance council and operator lists for review; awaiting sign-off at next council session (ref: `docs/source/sorafs/council_minutes_2025-10-29.md` follow-up).
- 2025-11-02 — Pin Registry register ISI now enforces shared chunker/policy validation via `sorafs_manifest` helpers, keeping on-chain paths aligned with Torii checks.
- 2026-02-13 — Added provider advert rollout phases (R0–R3) to the ledger and published the associated dashboards and operator guidance (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).
