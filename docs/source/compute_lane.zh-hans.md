---
lang: zh-hans
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
---

# Compute Lane (SSC-1)

The compute lane accepts deterministic HTTP-style calls, maps them onto Kotodama
entrypoints, and records metering/receipts for billing and governance review.
This RFC freezes the manifest schema, call/receipt envelopes, sandbox guardrails,
and configuration defaults for the first release.

## Manifest

- Schema: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` is pinned to `1`; manifests with a different version are rejected
  during validation.
- Each route declares:
  - `id` (`service`, `method`)
  - `entrypoint` (Kotodama entrypoint name)
  - codec allowlist (`codecs`)
  - TTL/gas/request/response caps (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - determinism/execution class (`determinism`, `execution_class`)
  - SoraFS ingress/model descriptors (`input_limits`, optional `model`)
  - pricing family (`price_family`) + resource profile (`resource_profile`)
  - authentication policy (`auth`)
- Sandbox guardrails live in the manifest `sandbox` block and are shared by all
  routes (mode/randomness/storage and non-deterministic syscall rejection).

Example: `fixtures/compute/manifest_compute_payments.json`.

## Calls, requests, and receipts

- Schema: `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome` in
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` produces the canonical request hash (headers are kept
  in a deterministic `BTreeMap` and the payload is carried as `payload_hash`).
- `ComputeCall` captures the namespace/route, codec, TTL/gas/response cap,
  resource profile + price family, auth (`Public` or UAID-bound
  `ComputeAuthn`), determinism (`Strict` vs `BestEffort`), execution class
  hints (CPU/GPU/TEE), declared SoraFS input bytes/chunks, optional sponsor
  budget, and the canonical request envelope. The request hash is used for
  replay protection and routing.
- Routes may embed optional SoraFS model references and input limits
  (inline/chunk caps); manifest sandbox rules gate GPU/TEE hints.
- `ComputePriceWeights::charge_units` converts metering data into billed compute
  units via ceil-division on cycles and egress bytes.
- `ComputeOutcome` reports `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted`, or `InternalError` and optionally includes response hashes/
  sizes/codec for audit.

Examples:
- Call: `fixtures/compute/call_compute_payments.json`
- Receipt: `fixtures/compute/receipt_compute_payments.json`

## Sandbox and resource profiles

- `ComputeSandboxRules` locks the execution mode to `IvmOnly` by default,
  seeds deterministic randomness from the request hash, allows read-only SoraFS
  access, and rejects non-deterministic syscalls. GPU/TEE hints are gated by
  `allow_gpu_hints`/`allow_tee_hints` to keep execution deterministic.
- `ComputeResourceBudget` sets per-profile caps on cycles, linear memory, stack
  size, IO budget, and egress, plus toggles for GPU hints and WASI-lite helpers.
- Defaults ship two profiles (`cpu-small`, `cpu-balanced`) under
  `defaults::compute::resource_profiles` with deterministic fallbacks.

## Pricing and billing units

- Price families (`ComputePriceWeights`) map cycles and egress bytes into compute
  units; defaults charge `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` with
  `unit_label = "cu"`. Families are keyed by `price_family` in manifests and
  enforced at admission.
- Metering records carry `charged_units` plus raw cycle/ingress/egress/duration
  totals for reconciliation. Charges are amplified by execution-class and
  determinism multipliers (`ComputePriceAmplifiers`) and capped by
  `compute.economics.max_cu_per_call`; egress is clamped by
  `compute.economics.max_amplification_ratio` to bound response amplification.
- Sponsor budgets (`ComputeCall::sponsor_budget_cu`) are enforced against
  per-call/daily caps; billed units must not exceed the declared sponsor budget.
- Governance price updates use the risk-class bounds in
  `compute.economics.price_bounds` and the baseline families recorded in
  `compute.economics.price_family_baseline`; use
  `ComputeEconomics::apply_price_update` to validate deltas before updating
  the active family map. Torii config updates use
  `ConfigUpdate::ComputePricing`, and kiso applies it with the same bounds to
  keep governance edits deterministic.

## Configuration

New compute configuration lives in `crates/iroha_config/src/parameters`:

- User view: `Compute` (`user.rs`) with env overrides:
  - `COMPUTE_ENABLED` (default `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- Pricing/economics: `compute.economics` captures
  `max_cu_per_call`/`max_amplification_ratio`, fee split, sponsor caps
  (per-call and daily CU), price family baselines + risk classes/bounds for
  governance updates, and execution-class multipliers (GPU/TEE/best-effort).
- Actual/defaults: `actual.rs` / `defaults.rs::compute` expose parsed
  `Compute` settings (namespaces, profiles, price families, sandbox).
- Invalid configs (empty namespaces, default profile/family missing, TTL cap
  inversions) are surfaced as `InvalidComputeConfig` during parsing.

## Tests and fixtures

- Deterministic helpers (`request_hash`, pricing) and fixture roundtrips live in
  `crates/iroha_data_model/src/compute/mod.rs` (see `fixtures_round_trip`,
  `request_hash_is_stable`, `pricing_rounds_up_units`).
- JSON fixtures live in `fixtures/compute/` and are exercised by the data-model
  tests for regression coverage.

## SLO harness and budgets

- `compute.slo.*` configuration exposes the gateway SLO knobs (in-flight queue
  depth, RPS cap, and latency targets) in
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. Defaults: 32
  in-flight, 512 queued per route, 200 RPS, p50 25 ms, p95 75 ms, p99 120 ms.
- Run the lightweight bench harness to capture SLO summaries and a request/egress
  snapshot: `cargo run -p xtask --bin compute_gateway -- bench [manifest_path]
  [iterations] [concurrency] [out_dir]` (defaults: `fixtures/compute/manifest_compute_payments.json`,
  128 iterations, concurrency 16, outputs under
  `artifacts/compute_gateway/bench_summary.{json,md}`). The bench uses
  deterministic payloads (`fixtures/compute/payload_compute_payments.json`) and
  per-request headers to avoid replay collisions while exercising
  `echo`/`uppercase`/`sha3` entrypoints.

## SDK/CLI parity fixtures

- Canonical fixtures live under `fixtures/compute/`: manifest, call, payload, and
  the gateway-style response/receipt layout. Payload hashes must match the call
  `request.payload_hash`; the helper payload lives in
  `fixtures/compute/payload_compute_payments.json`.
- The CLI ships `iroha compute simulate` and `iroha compute invoke`:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` live in
  `javascript/iroha_js/src/compute.js` with regression tests under
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` loads the same fixtures, validates payload hashes,
  and simulates the entrypoints with tests in
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- The CLI/JS/Swift helpers all share the same Norito fixtures so SDKs can
  validate request construction and hash handling offline without hitting a
  running gateway.
