---
lang: am
direction: ltr
source: docs/source/sorafs_gateway_load_tests.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4888b8a63ddf992f4a52d72ea253efc61c836f2651d58cfd0b84da74b3d353c4
source_last_modified: "2026-07-04T05:54:23.764215+00:00"
translation_last_reviewed: 2026-07-03
title: SoraFS Gateway Load Testing Plan
summary: Deterministic load harness and follow-up tasks for the SF-5a trustless delivery profile.
source_mtime: 2026-07-04T05:54:23.764215+00:00
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
The SF-5a rollout evidence gate also requires staging-load artifacts to carry a
`policy_digest_hex`, and governance approval artifacts must match that staged
policy digest before promotion. Suite, staging, and policy digest mismatches
mark the offending artifact invalid in the emitted summary before the gate can
report ready. Local conformance artifacts must also keep `scenario_count` equal
to the unique canonical `scenarios` inventory, and duplicate scenario entries
fail the artifact before promotion can report ready.
Their `cargo_command` evidence must be one of the reviewed gateway conformance
commands, so substring-only or shell-expanded command strings cannot stand in
for the actual replay harness.
Staging-load artifacts must also keep `stream_count` and `provider_count` equal
to the unique canonical `streams[].name` and `providers[].name` inventories, and
duplicate stream or provider entries fail the artifact before promotion can
report ready. Stream labels must use generated `gateway-load-stream-0000`-style
names, provider names must use reviewed `gateway-load-provider-*` slugs without
placeholder or test markers, `hardware_profile.name` must use a reviewed
`gateway-load-hardware-*` label, and `cache_state.mode` is closed to
`cold-cache`, `warm-cache`, or `mixed-cache`. Their `gateway_version` evidence
must use a concrete
`iroha-gateway X.Y.Z` release label or `iroha-gateway X.Y.Z-rc.N` release
candidate label, so placeholder or unscoped version strings cannot enter
promotion packets. Telemetry/SLO artifacts also bind `metric_count` to the unique
canonical `metrics` inventory and reject duplicate metric labels before
promotion can report ready. Staging-load SLO values for `error_rate_bps`,
`p95_latency_ms`, and `p99_latency_ms` must be non-negative integers before
they can satisfy the rollout ceilings, and `success_rate_bps` must be a
positive integer in the inclusive basis-point range up to `10000`; operator
success/error bps thresholds and hand-written `error_rate_bps` evidence are also
capped at `10000` so impossible basis-point rates cannot satisfy promotion.

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
| Payload-free rollout canary builder | Implemented | `scripts/build_sorafs_gateway_load_canary.py` builds checked-in local conformance, staging load, telemetry/SLO, transport-scope, and governance approval evidence artifacts from reviewed rollout facts. |
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

`scripts/check_sorafs_gateway_load_rollout_evidence.py` validates payload-free
local conformance, live staging load, telemetry/SLO, transport-scope, and
governance approval evidence before SF-5a load promotion. It binds the live
staging report back to the signed local conformance digest, requires live
telemetry and governance artifacts to reference the staging report digest, and
fails closed if raw reports, response bodies, fixture payloads, or runtime
secrets appear in evidence. `scripts/run_sorafs_gateway_load_rollout_evidence.py`
emits the matching collection plan and dry-run evidence contract so operators
can review the required fields before submitting promotion packets, and the
runner validates the schema-closed collection plan, required kinds, thresholds,
external evidence map, evidence contract, and command steps before dry-run
output or verifier execution. The shared runner plan guard also rejects
non-canonical nested required-kind, threshold, external-evidence,
evidence-contract, and command-step shapes before any live gateway-load contact.
Gateway-load payload-safety artifacts must explicitly set
`raw_report_included`, `private_keys_included`, `response_bodies_included`,
`raw_payloads_included`, and `critical_alerts_firing` to `false`; transport-scope
artifacts must also explicitly set the non-applicable HTTP/3 booleans to
`false` before promotion can report ready.
`scripts/build_sorafs_gateway_load_canary.py` builds individual payload-free
evidence artifacts for local conformance, staging load, telemetry/SLO,
transport-scope, and governance approval runs. The builder requires reviewed
deployment context, complete deterministic scenario and metric coverage where
applicable, reviewed staging provider names using
`gateway-load-provider-*` labels whose unique inventory matches
`--provider-count`, reviewed `gateway-load-hardware-*` hardware-profile labels,
reviewed cache-state modes,
generated `gateway-load-stream-*` per-stream inventory labels matching
`--stream-count`, suite/staging
digest bindings, SLO threshold facts, and
validates every generated artifact through
`scripts/check_sorafs_gateway_load_rollout_evidence.py` before writing. Checked
in response-file examples cover the local conformance and staging-load roots.
Local-conformance artifacts also bind `scenario_count` to the unique canonical
`scenarios` inventory, require the reviewed gateway-load scenarios, and reject
duplicate or unknown scenario labels before promotion can report ready.
They also reject unreviewed `cargo_command` values before promotion can report
ready.
Staging-load artifacts reject placeholder or malformed `gateway_version` labels
before promotion can report ready, reject unknown cache-state modes, require
`providers[].name` entries to use reviewed `gateway-load-provider-*` labels,
require `hardware_profile.name` to use reviewed `gateway-load-hardware-*`
labels, and reject placeholder or test markers in provider and hardware-profile
labels before promotion can report ready. The staging-load checker also rejects
fractional or out-of-range `success_rate_bps` values plus fractional or
out-of-range `error_rate_bps` values plus fractional or negative
`p95_latency_ms` and `p99_latency_ms` values before those integer-unit SLO fields
can satisfy promotion thresholds.
Telemetry/SLO artifacts also bind `metric_count` to the unique canonical
`metrics` inventory, require the reviewed gateway-load metrics, and reject
duplicate or unknown metric labels before promotion can report ready. The
summary exports the sorted reviewed `metrics` inventory plus
`metric_count_values`, and the aggregate production-readiness gate requires
those fields to match the telemetry/SLO artifact fingerprint before final
promotion can report ready.

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

The checked-in canary builder and examples do not replace live staging load
execution, signed local conformance report archival, or future HTTP/3 transport
coverage once a committed gateway endpoint exists.
