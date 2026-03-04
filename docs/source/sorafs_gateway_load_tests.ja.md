---
lang: ja
direction: ltr
source: docs/source/sorafs_gateway_load_tests.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fbdb35ca448a6f30e15d2a7d258ce9e40614672089bc6b578021579f7ecc1b9f
source_last_modified: "2026-01-03T18:07:58.686488+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Gateway Load Testing Plan
summary: Deterministic load harness and follow-up tasks for the SF-5a trustless delivery profile.
---

# SoraFS Gateway Load Testing Plan

The deterministic load harness now ships alongside the replay suite. It exercises
the same fixture-backed gateway adapters under ≥1,000 concurrent requests and
records latency percentiles plus refusal/error breakdowns. The harness lives in
`integration_tests/src/sorafs_gateway_conformance.rs` (`run_deterministic_load_test`)
and is gated via the `sorafs_gateway_deterministic_load_harness` regression. CI
invokes it through `cargo test -p integration_tests sorafs_gateway_conformance`.

Operators can reuse the JSON emitted by `SuiteReport::to_json_value()` (see
`ci/check_sorafs_gateway_conformance.sh`) to collect signed load-test reports for
governance.

The sections below capture remaining improvements and the scenario matrix used by
the harness.

## Objectives

1. Sustain ≥1,000 concurrent range streams while validating proofs and refusal
   behaviour.
2. Measure tail latency (P95, P99) for both hot-cache and cold-cache scenarios.
3. Inject controlled corruption (chunk tampering, proof mismatch) and assert
   deterministic refusal and logging.
4. Produce signed load-test reports consumable by operators and governance.

## Work Breakdown (Draft)

| Task | Owner(s) | Notes |
|------|----------|-------|
| Implement deterministic worker pool | QA Guild / Tooling WG | Reuse Tokio-based executor; support seeded RNG for reproducible request order. |
| Integrate replay fixtures | QA Guild | Stream canonical CAR fixtures, PoR proofs, and negative cases. |
| Collect metrics | Observability | Capture latency histograms, refusal counts, throughput, CPU/memory. |
| Failure injection | QA Guild | Flip bits in payload, alter proof nodes, drop required headers. |
| Reporting | Tooling WG | Emit JSON/CSV run reports with summary statistics and failure artefacts. |

## Scenario Matrix

| ID | Description | Load | Expected Outcome |
|----|-------------|------|------------------|
| L1 | Hot-cache range streaming | 1,000 concurrent, 10-minute run | P95 < 120 ms, P99 < 250 ms, zero proof failures |
| L2 | Cold-cache full CAR bootstrap | 250 concurrent | P95 < 500 ms, proof validation success |
| L3 | Corruption injection | 1% of requests tampered | All corrupted responses refused (422) |
| L4 | Admission mismatch | 5% requests without manifest envelope | 428 refusal with telemetry logged |
| L5 | GAR rate-limit drill | Burst beyond configured limit | 429 with `rate_limited` reason |
| L6 | Header downgrade attempt | 2% requests missing required trustless headers | Gateway returns 428 `required_headers_missing`, telemetry tagged |

## Metrics & Telemetry

Gateways should expose Prometheus metrics (via `/metrics`) or provide JSON logs
for:

- `sorafs_gateway_latency_ms_bucket{scenario}` — Per-scenario latency histograms.
- `sorafs_gateway_refusals_total{reason}` — Refusal counts by reason (unsupported_chunker, proof_verification_failed, etc.).
- `sorafs_gateway_bytes_total` — Total bytes served per test.
- `sorafs_gateway_concurrency_active` — Concurrent request gauge.

Load harness should emit:

- Per-second throughput.
- Latency percentiles (P50, P95, P99, max).
- Failure summaries (reason, count, first occurrence timestamp).
- Proof verification counters (successful vs rejected).
- Admission enforcement counts (missing envelope, expired envelope).

## Failure Injection Coverage

- **Proof corruption:** Bit-flip samples within PoR proofs, ensure gateways return 422 and log the
  failure.
- **Header downgrade:** Omit required trustless headers (e.g., `X-SoraFS-Nonce`), expect 428 responses.
- **Admission mismatch:** Drop manifest envelope or send expired envelopes, expect 428.
- **GAR limit:** Exceed configured rate/egress caps to trigger 429 with telemetry tagging.

## Pending Decisions

- Final concurrency target for cold-cache scenario (needs hardware sizing input).
- Whether to require HTTP/3 coverage in the first iteration.
- Integration with internal CI runners (GitHub Actions vs dedicated load rigs).
- Automated health checks post-run (e.g., verifying no lingering admission expiry warnings).

## Next Steps

1. Finalise fixture list and chunk ranges for hot vs cold cache runs.
2. Extend worker pool to cover HTTP/3 gateways once transport lands.
3. Validate on internal staging gateway, record baseline metrics.
4. Iterate on failure injection strategy before opening to operators.
