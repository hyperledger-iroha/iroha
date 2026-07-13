# SoraFS Migration Ledger

This ledger mirrors the migration change log captured in the SoraFS
Architecture RFC. Entries are grouped by milestone and list the effective window,
impacted teams, and required actions. Updates to the migration plan MUST modify
both this file and the RFC (`docs/source/sorafs_architecture_rfc.md`) to keep
downstream consumers aligned.

| Milestone | Effective Window | Change Summary | Impacted Teams | Action Items | Status |
|-----------|------------------|----------------|----------------|--------------|--------|
| L0 | First-release candidate | CI enforces deterministic fixtures, canonical manifest admission, explicit expectation flags, cross-SDK parity, and adversarial guards. | Docs, Storage, Governance, SDKs | Keep all local gates green and reject retired pre-release request shapes. | Implemented locally; final-tree validation in progress. |
| L1 | Deployment qualification | Every required SoraFS lane is exercised against one reviewed deployment and emits a schema-valid `ready` summary. | Storage, Governance, Networking, SRE, SDKs | Collect signed, deployment-bound evidence without persisting runtime secrets in the repository. | Blocked: the current aggregate artifact contains no recognized lane summaries. |
| L2 | Production promotion | The aggregate gate reconciles every required lane under one final production deployment context. | Release, Governance, SRE | Run the aggregate checker with explicit deployment ID and `prod`/`production` environment; promote only on `status=ready`. | Blocked pending L1 evidence and external approval. |

Governance minutes and deployment approvals are external release artifacts;
none are represented as present in this repository. Teams should add dated,
non-secret evidence fingerprints when notable events occur so the external
archive and aggregate summaries remain traceable without copying credentials or
payloads into source control.

## Recent Updates

- 2025-11-02 — Pin Registry register ISI now enforces shared chunker/policy validation via `sorafs_manifest` helpers, keeping on-chain paths aligned with Torii checks.
- 2026-02-13 — Added provider advert rollout phases (R0–R3) to the ledger and published the associated dashboards and operator guidance (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).
- 2026-06-22 — Refreshed the M1 status to separate implemented local fixture/expectation-flag controls from external staging alias and governance evidence required for live rollout sign-off.
- 2026-07-12 — Replaced the pre-release migration/fallback narrative with a
  strict V1 rollout contract. Envelope-only admission, grandfathered manifests,
  and undocumented council-minute claims are not first-release paths; current
  production readiness remains blocked until all required deployment summaries
  are present and ready.
