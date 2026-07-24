---
title: "SoraFS First-Release Rollout Roadmap"
---

# SoraFS First-Release Rollout Roadmap (SF-1)

This file keeps its historical name so existing documentation links remain
stable, but it is a deployment roadmap, not a backward-compatibility plan.
SoraFS has not shipped a public legacy protocol. Every admitted deployment uses
the canonical V1 manifest, registry, provider-advert, alias-proof, and retrieval
contracts from its first accepted request. Pre-release caller summaries,
envelope-only admission, and grandfathered manifests are rejected.

The architecture contract lives in `docs/source/sorafs_architecture_rfc.md`.
Milestone history lives in `docs/source/sorafs/migration_ledger.md`. Repository
checks establish local conformance; only reviewed, deployment-bound evidence can
establish production readiness.
The implementation, test, documentation, and evidence mapping for every active
first-release item lives in `docs/source/sorafs/v1_closure_ledger.md`.

## Milestone Overview

| Milestone | Goal | Required output | Status source |
|-----------|------|-----------------|---------------|
| **L0 — Local contract** | Keep deterministic fixtures, canonical payloads, SDKs, and adversarial guards aligned. | Green fixture, release, SDK, static-contract, and focused crate suites. | CI logs and repository tests. |
| **L1 — Deployment qualification** | Exercise every required SoraFS lane against one reviewed deployment. | One schema-valid, `ready` summary per aggregate-gate lane with matching deployment context. | External evidence archive plus per-lane checkers. |
| **L2 — Production promotion** | Prove the final production deployment satisfies every lane simultaneously. | `sorafs.production_readiness.aggregate_gate.v1` with `status=ready`, final environment, and no errors. | `scripts/check_sorafs_production_readiness.py`. |

No milestone can be waived by changing a repository status document. A missing
artifact, deployment ID, production environment, signer approval, or required
lane keeps the aggregate gate blocked.

## L0 — Local Contract

### Deterministic publication

- Run `ci/check_sorafs_fixtures.sh`. It regenerates and compares the canonical
  chunker, provider-admission, and Pin Registry fixtures and verifies required
  signatures.
- The scheduled and manually dispatchable
  `.github/workflows/sorafs-fixtures-nightly.yml` job runs the same script and
  archives its log. Drift is a hard failure; there is no unsigned development
  override in the release path.
- Build manifests with the exact canonical `ManifestV1` payload. Pin admission
  derives the manifest digest, chunk-plan commitment, root CID, CAR commitment,
  profile, policy, aliases, and fees from that payload. Retired duplicate
  summaries are not accepted by Torii, direct ISI, CLI, or SDK builders.

### Release and SDK guards

- Run `ci/check_sorafs_cli_release.sh` for formatting, strict Clippy, focused
  crate suites, FFI-header parity, and adversarial release-helper coverage.
- Run `.github/workflows/pr_sorafs_pin_register_sdk.yml` or the corresponding
  `ci/check_sorafs_pin_register_*_sdk.sh` scripts for Swift, JVM/Java,
  JavaScript, C#, and Python parity.
- Run `ci/sdk_sorafs_orchestrator.sh` for multi-provider SDK fixture parity.
  Generated files under `artifacts/` or `dist/` are evidence outputs and remain
  untracked.
- Run `python3 -m pytest -q scripts/tests/*sorafs*test.py` for static contracts,
  readiness-checker schemas, path/symlink defenses, and their negative controls.

### Canonical publishing example

```bash
cargo run -p sorafs_car --bin sorafs_manifest_builder -- docs/book \
  --manifest-out artifacts/docs/book/manifest.to \
  --manifest-signatures-out artifacts/docs/book/manifest_signatures.json \
  --car-out artifacts/docs/book/content.car \
  --chunk-fetch-plan-out artifacts/docs/book/fetch_plan.json \
  --car-digest=<expected-lowercase-hex> \
  --car-size=<expected-bytes> \
  --root-cid=<expected-cid> \
  --dag-codec=0x71
```

Release automation must source the expectations from the reviewed release
bundle, not copy placeholder values from documentation. The manifest and
signature-envelope commitment must then be submitted to the authoritative Pin
Registry; the envelope is an audit artifact, not an admission fallback.

## L1 — Deployment Qualification

Use one canonical deployment ID across every required lane. Each lane checker
must receive evidence from the same reviewed environment and emit a
schema-closed, payload-free summary. Required evidence includes, at minimum:

- deterministic pin registration, alias proof, provider-advert replay, and
  multi-provider retrieval;
- gateway compliance, denylist, load, TLS/DNS, and cache-revocation behavior;
- PDP, PoR, PoTR, PoP, repair, reputation, reserve/rent, orderbook, settlement,
  billing/hedging, moderation, governance-DAG, transparency, AI prescreen, and
  appeal-finance lanes selected by the aggregate checker;
- four-or-more-validator consensus/finality evidence where a network exercise
  is required;
- signed approvals, key/HSM provenance, dashboards, alert tests, load/chaos
  results, and public package canaries where required by the lane contract.

The evidence archive is operational data and must not contain runtime signing
secrets. Checkers record safe relative labels and SHA-256 fingerprints, not
tokens, private keys, authorization headers, or arbitrary payload fields.

## L2 — Production Promotion

Prepare an owner-private response file containing exactly one reviewed summary
for every required lane, the signed foundational envelope and its trusted
signer/continuity values, an explicit `--now-unix`, and the final deployment
context. Use the runner's real response-file interface:

```bash
python3 scripts/run_sorafs_production_readiness.py \
  @artifacts/sorafs/production-readiness/reviewed-collection.args \
  --dry-run

python3 scripts/run_sorafs_production_readiness.py \
  @artifacts/sorafs/production-readiness/reviewed-collection.args
```

`scripts/examples/sorafs_production_readiness_collection.args.example` lists the
17 lane-specific summary flags and the complete foundational trust/continuity
surface. Its public values are shape-only examples and must be replaced from the
reviewed release record. Review the schema-closed dry-run plan before executing
the second command. Promotion is allowed only when the resulting aggregate
reports `status=ready`, `summary_file_count=17`,
`recognized_summary_count=17`, every required row is present, every lane has
the same deployment ID and final environment, all artifact fingerprints and
counts reconcile, and both aggregate and lane error lists are empty.

## Ownership and Change Control

| Area | Owner responsibility | Review evidence |
|------|----------------------|-----------------|
| Storage/tooling | Deterministic fixtures, CAR/manifest parity, node proofs, repair, and release binaries. | CI logs, fixture hashes, package attestations. |
| Governance | Registry roots, provider admission, alias authority, moderation/appeal decisions, and signed approvals. | Ledger/finality receipts and approval fingerprints. |
| Networking/SRE | Gateway, discovery, routing, DNS/TLS, observability, load, and chaos qualification. | Deployment-bound probes, dashboards, alerts, and incident-free burn-in. |
| SDK/release | Cross-language request/response parity and public package canaries. | Guard logs, package versions, fixture hashes, canary summaries. |

Any contract change must update this file, the architecture RFC, the migration
ledger, relevant public API documentation, fixtures, and negative tests in the
same change. First-release schema cleanup is permitted, but all implementations
and generated artifacts must move atomically; compatibility shims are not added.

## Exit Criteria

- L0 commands pass on the final tree, including negative/adversarial controls.
- No production source exposes retired pin summaries, envelope-only admission,
  manual PoR mutation, or single-source fallback as an unreviewed default.
- Every required L1 lane produces a fresh, reviewed, deployment-bound `ready`
  summary from the same final production deployment.
- The L2 aggregate gate reports `ready` with zero errors and a complete
  recognized-summary inventory.
- `status.md` records the validation commands and result; `roadmap.md` retains
  only genuinely outstanding implementation or external rollout work.
