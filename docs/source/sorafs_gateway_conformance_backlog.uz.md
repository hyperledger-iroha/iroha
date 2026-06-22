---
lang: uz
direction: ltr
source: docs/source/sorafs_gateway_conformance_backlog.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d01d4577391cff6d0b4c547866a0f795699fc0c0e86112a91685a98f6d22387
source_last_modified: "2025-12-29T18:16:36.140877+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway Conformance Status
summary: Implemented SF-5a conformance workstreams plus live rollout evidence hand-offs.
---

# SoraFS Gateway Conformance Status

This living document records the shipped SF-5a conformance harness and the
external rollout evidence still owned by operators or hosted CI. Replay
verification, deterministic load testing, local CI gating, signed attestation
generation, and dashboard assets are implemented in this checkout.

## Implementation And Rollout Breakdown

### 1. Replay Harness Core (Owner: Conformance WG, Issue: SF-5a-REPLAY)
- **Scope**
  - Implement HTTP adapter shim (Tokio + reqwest) with deterministic header injection.
  - Wire Norito manifest ingestion and proof verification pipeline (BLAKE3 digest, PoR validation).
  - Emit Norito-signed attestation reports using the signing hook defined in `sorafs_gateway_conformance.md`.
- **Dependencies**
  - Fixture index (`fixtures/sorafs_gateway/index.norito.json`).
  - Token schema helpers from `sorafs_token_schema`.
- **Deliverables**
  - Harness module `integration_tests::sorafs_gateway_conformance`.
  - Golden fixture and regression tests (`cargo test -p integration_tests --test nexus_and_streaming sorafs_gateway_conformance`).
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
  - Deterministic load helper `run_deterministic_load_test` in the conformance harness.
  - Telemetry dashboards added to `dashboards/grafana/sorafs_gateway_conformance.json`.
  - Documentation covering load profiles within this backlog.

### 3. CLI Packaging (`sorafs-gateway-attest`) (Owner: Tooling WG, Issue: SF-5a-CLI)
- **Scope**
  - Expose attestation generation and verification through `cargo xtask
    sorafs-gateway-attest`.
  - Keep replay and load execution in the deterministic harness used by the
    generator; avoid a second config surface until a standalone operator binary
    is needed.
- **Deliverables**
  - Implemented `cargo xtask sorafs-gateway-attest --signing-key ...` report
    generation and `--verify <attestation.to>` envelope verification.
  - Parser and verifier regression tests covering command selection, digest
    drift, and signature verification.
  - Future standalone packaging can wrap the same verified helper without
    changing the attestation format.

### 4. Fixture Publication Automation (Owner: Build Infra, Issue: SF-5a-FIXTURES)
- **Scope**
  - Keep local fixture drift gated by `scripts/verify_sorafs_fixtures.sh`.
  - Package, sign, upload, and retain hosted fixture-release artifacts once
    Buildkite credentials and release storage are provisioned.
  - Verify signatures during hosted pipeline execution and fail on drift.
- **Deliverables**
  - Local validation script `scripts/verify_sorafs_fixtures.sh` wraps
    `cargo xtask sorafs-gateway-fixtures --verify` and is ready for pre-merge
    or nightly invocation.
  - Hosted fixture publication and retention remain deployment evidence, not a
    local harness implementation blocker.

### 5. CI / Nightly Integration (Owner: CI WG, Issue: SF-5a-CI)
- **Scope**
  - Keep `ci/check_sorafs_gateway_conformance.sh` as the local merge gate for gateway, orchestrator, fixture, and conformance harness changes.
  - Let hosted nightly jobs wrap the same script when Buildkite and PagerDuty rollout secrets are available.
- **Deliverables**
  - Scripted conformance gate invoking `cargo test -p integration_tests --test nexus_and_streaming sorafs_gateway_conformance -- --nocapture`.
  - CI documentation points at the local script; hosted notification wiring remains deployment work, not harness implementation.

### 6. Governance Dashboard Integration (Owner: GovOps WG, Issue: SF-5a-DASHBOARD)
- **Scope**
  - Provide local Grafana panels for fixture metadata, refusal totals, latency, throughput, and active concurrency.
  - Keep live governance portal embedding and attestation datastore ingestion as rollout evidence once GovOps supplies the deployment surface.
- **Deliverables**
  - Dashboard panels in `dashboards/grafana/sorafs_gateway_conformance.json`.
  - Runbook references interpret `torii_sorafs_gateway_refusals_total` and fixture metadata alongside signed attestations.

## Future Enhancements

1. **HTTP/3 (QUIC) profile** — Extend the harness to exercise QUIC endpoints once the gateway supports them (SF-5a-QUIC).
2. **Corruption/failure adapters** — Implement modular fault injectors (e.g., header drop, delayed proofs) for bespoke operator tests (SF-5a-FAULTS).
3. **Synthetic latency lab** — Provide controlled latency injection harness for observability dry runs (SF-5a-LATENCY).
4. **TLS telemetry alignment** — Feed TLS handshake metrics from SF-5b into the conformance reports to ensure consistent instrumentation (SF-5b-TLS-BRIDGE).

## Status Tracking

| Workstream | Issue ID | Status | Next Checkpoint | Notes |
|------------|----------|--------|-----------------|-------|
| Replay harness core | SF-5a-REPLAY | Implemented | 2026-06-22 | Deterministic fixture replay is covered by `integration_tests::sorafs_gateway_conformance`. |
| Load runner | SF-5a-LOAD | Implemented | 2026-06-22 | `run_deterministic_load_test` covers the seeded ≥1,000 request profile in the harness. |
| CLI packaging | SF-5a-CLI | Implemented | 2026-06-22 | `cargo xtask sorafs-gateway-attest` now generates and verifies signed envelopes; standalone packaging is optional wrapper work. |
| Fixture publication | SF-5a-FIXTURES | Local verification implemented | Deployment rollout | `scripts/verify_sorafs_fixtures.sh` verifies fixture digests locally; hosted Buildkite publication waits on credentials and release storage. |
| CI integration | SF-5a-CI | Implemented | 2026-06-22 | `ci/check_sorafs_gateway_conformance.sh` runs the `nexus_and_streaming` conformance harness; hosted nightly wrapping remains rollout work. |
| Governance dashboard | SF-5a-DASHBOARD | Implemented | 2026-06-22 | Grafana panels live at `dashboards/grafana/sorafs_gateway_conformance.json`; GovOps embedding remains deployment evidence. |

Update this table when live rollout evidence, hosted fixture publication, or
new transport scenarios land.
