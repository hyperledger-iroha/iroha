---
lang: ka
direction: ltr
source: docs/source/sorafs_gateway_load_tests.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fbdb35ca448a6f30e15d2a7d258ce9e40614672089bc6b578021579f7ecc1b9f
source_last_modified: "2025-12-29T18:16:36.146382+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway Load Testing Plan
summary: Deterministic load harness and follow-up tasks for the SF-5a trustless delivery profile.
---

# SoraFS Gateway Load Testing Plan

The deterministic load harness ships with the gateway conformance replay suite.
It does not open a live HTTP/3 gateway or sleep through a wall-clock soak test;
instead, it replays canonical fixture-backed adapters through
`HarnessContext::new()`. The default `LoadProfile` schedules 1,000 streams over a
60-second profile window and records total requests, elapsed time, and P50/P95/P99
latency per scenario.

The harness lives in `integration_tests/src/sorafs_gateway_conformance.rs`
(`run_deterministic_load_test`) and is gated by the
`sorafs_gateway_deterministic_load_harness` regression. CI invokes it through
`ci/check_sorafs_gateway_conformance.sh`, which runs:

```bash
cargo test -p integration_tests --test nexus_and_streaming sorafs_gateway_conformance -- --nocapture
```

Operators can reuse the JSON emitted by `SuiteReport::to_json_value()` and sign it
with the gateway attestation helpers when collecting governance evidence.

## Objectives

1. Keep the local conformance load profile deterministic and fixture-backed.
2. Validate success, byte-range, corruption, admission, rate-limit, and denylist
   outcomes without relying on live network timing.
3. Emit JSON reports and signed attestation envelopes that operators can archive
   for governance review.
4. Use live staging rigs only for hardware SLO evidence, not for the local
   regression contract.

## Current Coverage

| Area | Status | Notes |
|------|--------|-------|
| Deterministic load scheduler | Implemented | `LoadProfile` and `DeterministicLoadGenerator` produce reproducible waves from the configured stream count and duration. |
| Fixture replay | Implemented | The suite covers full CAR replay, aligned and multi-range byte replay, unsupported chunkers, missing headers, corrupted proofs, corrupted CAR payloads, provider admission failures, rate limits, GAR denylist refusals, and capability-refusal fixtures. |
| Metrics report | Implemented | `LoadTestReport` records total requests, elapsed time, per-scenario success/refusal/error counts, and P50/P95/P99 latency. |
| Signed evidence | Implemented | `generate_attestation`, `verify_attestation_envelope`, and `cargo xtask sorafs-gateway-attest --verify` cover signed report validation. |
| Live staging load evidence | Rollout evidence | Capture against deployed gateways once the operator selects hardware, cache state, and duration. |
| HTTP/3 gateway load coverage | Transport follow-up | No committed SoraFS HTTP/3 gateway endpoint is present in this checkout; add HTTP/3 scenarios only after the gateway exposes that transport. |

## Scenario Matrix

| ID | Description | Load inclusion | Expected outcome |
|----|-------------|----------------|------------------|
| A1 | Full CAR replay | Suite replay | 200 success |
| A2 | Aligned byte-range replay | Default load cycle | 206 success |
| A3 | Misaligned byte-range refusal | Default load cycle | 416 refusal |
| A4 | Multi-range byte replay | Default load cycle | 206 success |
| B1 | Unsupported chunker handle | Suite replay | 406 refusal |
| B2 | Missing required SoraFS headers | Suite replay | 428 refusal |
| B3 | Corrupted PoR proof | Suite replay | 422 refusal |
| B4 | Corrupted CAR payload | Default load cycle | 422 refusal |
| B5 | Provider not admitted | Default load cycle | 412 refusal |
| B6 | Gateway rate limit | Default load cycle | 429 refusal |
| C* | Capability-refusal fixtures | Suite replay | Fixture-declared refusal |
| D1 | GAR denylist refusal | Suite replay | 451 refusal |

## Metrics & Telemetry

The local harness JSON contains:

- `load_profile.concurrent_streams` and `load_profile.max_duration_seconds`.
- `load_report.total_requests` and `load_report.elapsed_seconds`.
- Per-scenario `total`, `success`, `refusal`, `error`, `p50_ms`, `p95_ms`, and
  `p99_ms` fields.
- Replay scenario status, outcome, and canonical refusal payloads.

Live gateways should additionally expose Prometheus metrics or JSON logs for:

- `sorafs_gateway_latency_ms_bucket{scenario}` per-scenario latency histograms.
- `sorafs_gateway_refusals_total{reason}` refusal counts by reason.
- `sorafs_gateway_bytes_total` total bytes served per run.
- `sorafs_gateway_concurrency_active` active request gauge.

## Failure Injection Coverage

- **Proof corruption:** B3 mutates PoR proof material and expects a 422 refusal.
- **Payload corruption:** B4 mutates CAR payload bytes and expects a 422 refusal.
- **Header downgrade:** B2 omits required trustless headers and expects 428.
- **Admission mismatch:** B5 exercises provider admission refusal and expects 412.
- **Rate limiting:** B6 exceeds configured gateway limits and expects 429.
- **GAR denylist:** D1 verifies policy denial and expects 451.
- **Capability refusal:** C* fixtures pin additional deterministic refusal payloads.

## Remaining Rollout Work

1. Archive signed local conformance reports from `ci/check_sorafs_gateway_conformance.sh`
   for release candidates.
2. Run a live staging load rig with the same fixture bundle and record hardware,
   cache state, duration, and gateway version alongside the signed report.
3. Add a live-target adapter if operators need the integration test to exercise a
   deployed gateway instead of the fixture-backed adapter.
4. Add HTTP/3 scenarios only after the SoraFS gateway exposes a committed HTTP/3
   endpoint and configuration surface.
5. Record cold-cache SLO baselines after the staging hardware profile is chosen.
