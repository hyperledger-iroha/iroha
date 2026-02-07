---
lang: dz
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: "SoraFS Migration Roadmap"
---

> Adapted from [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# SoraFS Migration Roadmap (SF-1)

This document operationalises the migration guidance captured in
`docs/source/sorafs_architecture_rfc.md`. It expands the SF-1 deliverables into
execution-ready milestones, gating criteria, and owner checklists so storage,
artifact hosting to SoraFS-backed publication.

The roadmap is intentionally deterministic: every milestone names the required
artifacts, command invocations, and attestation steps so downstream pipelines
produce identical outputs and governance retains an auditable trail.

## Milestone Overview

| Milestone | Window | Primary Goals | Must Ship | Owners |
|-----------|--------|---------------|-----------|--------|
| **M1 – Deterministic Enforcement** | Weeks 7–12 | Enforce signed fixtures and stage alias proofs while pipelines adopt expectation flags. | Nightly fixture verification, council-signed manifests, alias registry staging entries. | Storage, Governance, SDKs |

Milestone status is tracked in `docs/source/sorafs/migration_ledger.md`. All
changes to this roadmap MUST update the ledger to keep governance and release
engineering in sync.

## Workstreams

### 2. Deterministic Pinning Adoption

| Step | Milestone | Description | Owner(s) | Output |
|------|-----------|-------------|----------|--------|
| Fixture rehearsals | M0 | Weekly dry-runs comparing local chunk digests against `fixtures/sorafs_chunker`. Publish report under `docs/source/sorafs/reports/`. | Storage Providers | `determinism-<date>.md` with pass/fail matrix. |
| Enforce signatures | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` fail if signatures or manifests drift. Development overrides require governance waiver attached to PR. | Tooling WG | CI log, waiver ticket link (if applicable). |
| Expectation flags | M1 | Pipelines call `sorafs_manifest_stub` with explicit expectations to pin outputs: | Docs CI | Updated scripts referencing expectation flags (see command block below). |
| Registry-first pinning | M2 | `sorafs pin propose` and `sorafs pin approve` wrap manifest submissions; CLI defaults to `--require-registry`. | Governance Ops | Registry CLI audit log, telemetry for failed proposals. |
| Observability parity | M3 | Prometheus/Grafana dashboards alert when chunk inventories diverge from registry manifests; alerts wired to ops on-call. | Observability | Dashboard link, alert rule IDs, GameDay results. |

#### Canonical publishing command

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

Replace the digest, size, and CID values with the expected references recorded in
the migration ledger entry for the artifact.

### 3. Alias Transition & Communications

| Step | Milestone | Description | Owner(s) | Output |
|------|-----------|-------------|----------|--------|
| Alias proofs in staging | M1 | Register alias claims in the Pin Registry staging environment and attach Merkle proofs to manifests (`--alias`). | Governance, Docs | Proof bundle stored next to manifest + ledger comment with alias name. |
| Proof enforcement | M2 | Gateways reject manifests without fresh `Sora-Proof` headers; CI gains `sorafs alias verify` step to fetch proofs. | Networking | Gateway config patch + CI output capturing verification success. |

### 4. Communication & Audit

- **Ledger discipline:** every state change (fixture drift, registry submission,
  alias activation) must append a dated note to
  `docs/source/sorafs/migration_ledger.md`.
- **Governance minutes:** council sessions approving pin registry changes or
  alias policies must reference both this roadmap and the ledger.
- **External comms:** DevRel publishes status updates at each milestone (blog +
  changelog excerpt) highlighting deterministic guarantees and alias timelines.

## Dependencies & Risks

| Dependency | Impact | Mitigation |
|------------|--------|------------|
| Pin Registry contract availability | Blocks M2 pin-first rollout. | Stage contract ahead of M2 with replay tests; maintain envelope fallback until regression-free. |
| Council signing keys | Required for manifest envelopes and registry approvals. | Signing ceremony documented in `docs/source/sorafs/signing_ceremony.md`; rotate keys with overlap and ledger note. |
| SDK release cadence | Clients must honour alias proofs before M3. | Align SDK release windows with milestone gates; add migration checklists to release templates. |

Residual risks and mitigations are mirrored in `docs/source/sorafs_architecture_rfc.md`
and should be cross-referenced when adjustments are made.

## Exit Criteria Checklist

| Milestone | Criteria |
|-----------|----------|
| M1 | - Nightly fixture job green for seven consecutive days. <br /> - Staging alias proofs verified in CI. <br /> - Governance ratifies expectation flag policy. |

## Change Management

1. Propose adjustments via PR updating this file **and**
   `docs/source/sorafs/migration_ledger.md`.
2. Link supporting governance minutes and CI evidence in the PR description.
3. On merge, notify storage + DevRel mailing list with summary and expected
   operator actions.

Following this procedure ensures the SoraFS rollout remains deterministic,
auditable, and transparent across teams participating in the Nexus launch.
