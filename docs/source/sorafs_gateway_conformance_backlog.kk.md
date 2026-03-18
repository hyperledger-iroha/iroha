---
lang: kk
direction: ltr
source: docs/source/sorafs_gateway_conformance_backlog.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d01d4577391cff6d0b4c547866a0f795699fc0c0e86112a91685a98f6d22387
source_last_modified: "2025-12-29T18:16:36.140877+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway Conformance Backlog
summary: Execution plan for remaining SF-5a conformance tasks, including owners, dependencies, and status.
---

# SoraFS Gateway Conformance Backlog

This living document tracks the work required to ship the SF-5a conformance harness,
covering replay verification, load testing, release automation, and governance reporting.

## Milestone Breakdown

### 1. Replay Harness Core (Owner: Conformance WG, Issue: SF-5a-REPLAY)
- **Scope**
  - Implement HTTP adapter shim (Tokio + reqwest) with deterministic header injection.
  - Wire Norito manifest ingestion and proof verification pipeline (BLAKE3 digest, PoR validation).
  - Emit Norito-signed attestation reports using the signing hook defined in `sorafs_gateway_conformance.md`.
- **Dependencies**
  - Fixture index (`fixtures/sorafs_gateway/index.norito.json`).
  - Token schema helpers from `sorafs_token_schema`.
- **Deliverables**
  - Rust crate `sorafs_gateway_cert::replay`.
  - Golden fixtures + regression tests (`cargo test -p sorafs_gateway_cert -- replay_*`).
  - Attestation samples archived under `artifacts/sorafs_gateway/replay/`.

### 2. Concurrent Load Runner (Owner: Reliability WG, Issue: SF-5a-LOAD)
- **Scope**
  - Build seeded workload generator sustaining ≥1 000 concurrent range streams.
  - Capture per-request telemetry (latency histograms, proof results) and export via Prometheus.
  - Support failure injection (timeouts, PoR corruption) toggled via CLI flags.
- **Dependencies**
  - Replay harness libraries (reuse HTTP adapter + proof pipeline).
  - Metrics export pipeline (`sorafs_telemetry`).
- **Deliverables**
  - Module `sorafs_gateway_cert::load` with CLI integration (`--profile mock-chaos`).
  - Telemetry dashboards added to `dashboards/sorafs_gateway_conformance.json`.
  - Documentation covering load profiles within this backlog.

### 3. CLI Packaging (`sorafs-gateway-cert`) (Owner: Tooling WG, Issue: SF-5a-CLI)
- **Scope**
  - Expose replay and load flows via a single CLI with subcommands:
    - `replay run --scenario sf1/full`
    - `load run --profile mock-chaos`
    - `report sign --input report.json`
  - Implement configuration discovery (`.sorafs-gateway-cert/config.toml` + env overrides).
- **Deliverables**
  - Binary crate `sorafs-gateway-cert`.
  - Installation guide in `docs/source/sorafs_gateway_cert_cli.md`.
  - Smoke tests executed in CI (`cargo run --bin sorafs-gateway-cert -- replay --help`).

### 4. Fixture Publication Automation (Owner: Build Infra, Issue: SF-5a-FIXTURES)
- **Scope**
  - Create CI job (`.buildkite/sorafs-fixtures.yml`) that packages fixture bundles, signs manifests, and uploads artifacts.
  - Publish Norito-signed manifest list to `fixtures/releases/latest.manifest`.
  - Verify signatures during pipeline execution and fail on drift.
- **Deliverables**
  - Buildkite step integrated into nightly and merge pipelines.
  - Artifact retention policy documented in `docs/source/fixtures_retention.md`.
  - Validation script `scripts/verify_sorafs_fixtures.sh` (wraps `cargo xtask sorafs-gateway-fixtures --verify`) invoked pre-merge so fixture digests, helper files, and scenario matrices drift only with intentional commits.

### 5. CI / Nightly Integration (Owner: CI WG, Issue: SF-5a-CI)
- **Scope**
  - Add pipeline `ci/sorafs-gateway-cert` that runs replay + load profiles against nightly builds.
  - Introduce merge gate for PRs touching gateway, orchestrator, or fixture code.
- **Deliverables**
  - Buildkite configuration with notifications to PagerDuty service `svc_sorafs_cert`.
  - Metrics exported to `ci/sorafs-gateway-cert:{profile}`.
  - CI documentation updates in `docs/source/ci_matrix.md`.

### 6. Governance Dashboard Integration (Owner: GovOps WG, Issue: SF-5a-DASHBOARD)
- **Scope**
  - Surface conformance status, last attestation hash, and outstanding failures on the governance dashboard.
  - Provide drill-down views for recent runs with links to artifacts.
- **Deliverables**
  - Dashboard panels (`dashboards/governance.json` sections `sorafs_conformance_*`).
  - Telemetry ingestion job that reads Norito attestation envelopes and updates the dashboard datastore.
  - Operations runbook entry for interpreting dashboard alerts.

## Future Enhancements

1. **HTTP/3 (QUIC) profile** — Extend the harness to exercise QUIC endpoints once the gateway supports them (SF-5a-QUIC).
2. **Corruption/failure adapters** — Implement modular fault injectors (e.g., header drop, delayed proofs) for bespoke operator tests (SF-5a-FAULTS).
3. **Synthetic latency lab** — Provide controlled latency injection harness for observability dry runs (SF-5a-LATENCY).
4. **TLS telemetry alignment** — Feed TLS handshake metrics from SF-5b into the conformance reports to ensure consistent instrumentation (SF-5b-TLS-BRIDGE).

## Status Tracking

| Workstream | Issue ID | Status | Next Checkpoint | Notes |
|------------|----------|--------|-----------------|-------|
| Replay harness core | SF-5a-REPLAY | In design | 2026-03-05 | Waiting on fixture index finalization. |
| Load runner | SF-5a-LOAD | Planned | 2026-03-08 | Seed distribution agreed with Reliability WG. |
| CLI packaging | SF-5a-CLI | In progress | 2026-03-04 | Parsing layer scaffolding in review. |
| Fixture publication | SF-5a-FIXTURES | Not started | 2026-03-06 | Requires Buildkite secrets approval. |
| CI integration | SF-5a-CI | Planned | 2026-03-10 | Dependent on CLI availability. |
| Governance dashboard | SF-5a-DASHBOARD | Planned | 2026-03-12 | Design mock-ups awaiting GovOps sign-off. |

Review and update this table at the end of each sprint to keep stakeholders aligned with the roadmap.
