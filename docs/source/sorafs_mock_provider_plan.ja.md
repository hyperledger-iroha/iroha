---
lang: ja
direction: ltr
source: docs/source/sorafs_mock_provider_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1194424dee1f8713a7bff272174d3afa76a72fdbb61ab2f6a76614cddcd614e
source_last_modified: "2026-01-03T18:07:57.195739+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Mock Provider Harness Plan (Draft)
summary: Outline for SF-6c multi-provider test harness.
---

# SoraFS Mock Provider Harness Plan (Draft)

## Objectives

- Provide deterministic mock providers to exercise orchestrator and gateway flows.
- Support success/failure injection, latency control, and token enforcement simulations.
- Integrate with integration tests to validate multi-provider chunk ordering and proof checks.

## Components

1. **Mock provider server**
   - Implements chunk-range endpoints (`GET /chunk`, `GET /car`) with configurable profiles.
   - Reads fixtures (CAR + manifest) from `fixtures/sorafs_mock_providers/`.
   - Configurable behavior: latency distribution, failure rates, proof tampering.
2. **Harness controller** (Rust)
   - Spins up N providers with config matrix.
   - Exposes gRPC/REST control plane for tests to toggle failure modes.
3. **Integration utilities**
   - Helper to issue stream tokens, admission envelopes.
   - Assertion helpers for chunk ordering and digest parity.

## Test Scenarios

- Happy path: 3 providers, orchestrator splits chunks, proofs verified.
- Provider failure: one provider returns 422/timeout, orchestrator retries others.
- Token exhaustion: provider enforces stream token limits, orchestrator falls back.
- Corrupted proof: provider tampered response, orchestrator rejects chunk.

## Observability

- Expose metrics for requests served, induced failures.
- Log proof failures / token denials for assertions.

## Fixture Format

- **Directory layout.** Reuse the SF-5a fixture catalogue so operators maintain
  a single source of truth. The harness consumes:
  - `fixtures/sorafs_gateway/<profile>/<scenario>/manifest.to` (Norito-encoded
    `SignedGatewayReport` manifest).
  - `fixtures/sorafs_gateway/<profile>/<scenario>/car/*.car` (CAR payloads).
  - Optional `negative/` subdirectories housing intentionally corrupted proofs.
- **Metadata index.** Add a `fixtures/sorafs_mock_providers/index.norito.json`
  file that maps scenario IDs to fixture paths, expected provider behaviours,
  and latency templates. The schema mirrors the SF-5a `fixture_index` but adds
  provider-specific knobs (`failure_rate`, `proof_mutator`, `token_budget`).
- **Versioning.** Every run records the fixture commit SHA in the harness
  report; CI validates the SHA against the `fixtures/VERSION` file to ensure
  deterministic playback.

## CI Alignment

- **Shared archive job.** Extend the existing SF-5a Buildkite pipeline to upload
  the `fixtures/sorafs_gateway` bundle as an artifact; the mock provider job
  fetches the same artifact to avoid drift.
- **Matrix coverage.** Nightly CI executes the harness with two profiles:
  `mock-smoke` (minimal happy-path) and `mock-chaos` (failure + latency). Each
  profile publishes results to the telemetry dashboard under
  `ci/sorafs-mock-provider:*`.
- **Gatekeeping.** Merge CI introduces a `sorafs-mock-provider` step that must
  pass for PRs touching:
  - `crates/sorafs_mock_provider_*`
  - `docs/source/sorafs_mock_provider_plan*`
  - `fixtures/sorafs_gateway/**`
  The step spins up the providers inside a container, runs the orchestrator
  integration tests, and surfaces Norito-signed reports identical to SF-5a.

## Control Interface

- **Transport.** Expose a JSON over HTTP control plane on `127.0.0.1:5901`
  (configurable). The harness also provides a gRPC facade generated from the
  same protobuf definition for SDK tests needing typed clients.
- **Endpoints.**
  - `POST /providers` — create or update mock providers; payload lists provider
    IDs, fixture scenarios, latency distributions, failure injection rules, and
    token limits. Responses echo resolved configuration with assigned ports.
  - `POST /providers/{id}/faults` — toggle runtime fault profiles (e.g.,
    `{"mode":"timeout","rate":0.1}`).
  - `POST /providers/{id}/reset` — clear counters and restore baseline config.
  - `GET /providers/{id}/metrics` — return request counts, active tokens, proof
    failures, and rolling latency stats for assertions.
  - `DELETE /providers/{id}` — shut down a mock provider.
  All endpoints accept/emit Norito JSON (`norito::json::Value`) ensuring
  deterministic field ordering.
- **Authentication.** Control API binds to loopback and requires a shared
  bearer token (`mock_provider.auth_token`) supplied via config. Test harnesses
  set the `Authorization: Bearer` header; unauthenticated calls return `401`.
- **Event stream.** The controller optionally exposes a `GET /events` SSE feed
  streaming provider state transitions (startup, fault injected, token
  exhaustion) so tests can synchronise without polling.
