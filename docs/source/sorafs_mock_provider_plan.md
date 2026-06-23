---
title: SoraFS Mock Provider Harness Status
summary: Implemented SF-6c deterministic multi-provider fixture harness and remaining service follow-ups.
---

# SoraFS Mock Provider Harness Status

## Objectives

- Provide deterministic mock providers to exercise orchestrator and gateway flows.
- Support success/failure injection, latency control, and token enforcement simulations.
- Integrate with integration tests to validate multi-provider chunk ordering and proof checks.

> **Status (Jun 2026):** SF-6c local coverage is implemented through
> `sorafs_car::fixtures::MultiPeerFixture`, the shared
> `sorafs_car::local_fetch` harness, `fixtures/sorafs_orchestrator/`, and the
> Rust/SDK parity tests. The standalone loopback HTTP/gRPC mock-provider server
> remains optional future tooling; local CI does not depend on a daemonized mock
> provider control plane.

## Components

1. **Deterministic provider fixtures**
   - `MultiPeerFixture::with_providers(N)` derives provider metadata,
     per-provider payload replicas, range capability, stream budgets, and
     telemetry from the canonical SF1 chunker vectors.
   - `fixtures/sorafs_orchestrator/multi_peer_parity_v1/` stores the JSON
     fixture bundle shared by Rust and SDK parity harnesses.
2. **Local fetch harness**
   - `sorafs_car::local_fetch::execute_local_fetch` converts plan/provider JSON
     into the same multi-fetch scheduler inputs used by the orchestrator.
   - Scoreboard filters, denied providers, boosted providers, retry budgets,
     max peers, digest verification, and telemetry snapshots are configurable.
3. **Integration utilities**
   - `crates/sorafs_orchestrator/tests/multi_peer_fetch.rs` injects transient
     provider failures and corrupted chunks while proving the orchestrator
     retries, verifies digests, and assembles the canonical payload.
   - `integration_tests/tests/sorafs_orchestrator_parity.rs` replays the shared
     JSON fixture across Rust and SDK bindings.

## Test Scenarios

- Happy path: 3 providers, orchestrator splits chunks, proofs verified.
- Provider failure: one provider returns a simulated transport error,
  orchestrator retries others.
- Corrupted proof/payload: one provider returns a tampered chunk once,
  orchestrator rejects it and retries.
- Scoreboard filtering: stale or ineligible provider metadata is excluded before
  scheduling.

## Observability

- Fetch outcomes expose provider reports, successes, failures, receipts, and
  policy status for assertions.
- Local scoreboard summaries can be returned by the harness for SDK parity and
  governance evidence.

## Fixture Format

- **Directory layout.** The shared fixture bundle lives under
  `fixtures/sorafs_orchestrator/multi_peer_parity_v1/`:
  - `metadata.json` records the payload location and fixture summary.
  - `plan.json` records chunk index, offset, length, and BLAKE3 digest.
  - `providers.json` records provider metadata, range capability, stream
    budgets, and transport hints.
  - `telemetry.json` records QoS, latency, failure-rate, and staking inputs.
  - `options.json` records orchestrator and scoreboard options.
- **Regeneration.** `python3 scripts/build_sorafs_orchestrator_fixture.py`
  rebuilds the JSON bundle from the canonical SF1 vectors.
- **Versioning.** The checked-in fixture files are deterministic; CI compares
  regenerated output and parity summaries instead of depending on a mutable
  daemon state directory.

## CI Alignment

- `ci/sdk_sorafs_orchestrator.sh` and
  `crates/sorafs_orchestrator/tests/*` cover the deterministic multi-provider
  fixture and scheduler behavior.
- `scripts/build_sorafs_orchestrator_fixture.py` is the regeneration path for
  changes to the chunker profile or provider template.
- Future daemonized mock-provider work should be additive and must continue to
  replay the same `fixtures/sorafs_orchestrator/` bundle.

## Control Interface

- **Current control surface.** Tests inject behavior through Rust closures,
  `LocalFetchOptions`, and deterministic fixture files. This keeps local
  scheduling tests hermetic and avoids binding test correctness to port
  allocation or network timing.
- **Future daemon surface.** A loopback JSON/gRPC controller can be added for
  SDKs that need real socket behavior. It should bind only to loopback, require
  explicit test configuration, emit Norito JSON, and reuse the checked-in
  fixture bundle rather than inventing a second scenario format.
