---
lang: ka
direction: ltr
source: docs/source/sorafs_gateway_capability_tests.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa3811e4524172204563b055e09c87c16846d8e354489f8e4690af0b3a9afaaa
source_last_modified: "2025-12-29T18:16:36.138965+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway Capability Refusal Tests
summary: Canonical refusal matrix and harness coverage for SF-5c.
---

# SoraFS Gateway Capability Refusal Tests

The gateway conformance suite (`integration_tests/tests/sorafs_gateway_conformance.rs`) consumes the canonical fixtures published under `fixtures/sorafs_gateway/capability_refusal/`. Each case maps to a stable `error` code, HTTP status, and telemetry reason label (`torii_sorafs_gateway_refusals_total{reason,profile,provider_id,scope}`) so SDKs and operators see consistent behaviour.

## Test Matrix (Harness module `capability_refusal`)

| ID | Scenario | Status | Error code | Notes |
|----|----------|--------|------------|-------|
| C1 | Unsupported chunker handle | 406 | `unsupported_chunker` | Gateway refuses manifests whose `chunk_profile_handle` is not enabled. |
| C2 | Missing manifest variant (alias absent) | 412 | `manifest_variant_missing` | Range requests carrying `Sora-Name` aliases that are not bound to the manifest envelope are rejected; `details.alias` echoes the requested alias. |
| C3 | Admission envelope mismatch | 412 | `admission_mismatch` | Manifest digest not covered by the governance envelope. |
| C4 | Unknown capability TLV | 428 | `unsupported_capability` | GREASE capability rejected when `allow_unknown_capabilities=false`. |
| C5 | Range request missing `dag-scope` header | 428 | `missing_header` | Enforces the trustless `Accept: …; dag-scope=block` contract; `details.header` is `Sora-Dag-Scope`. |
| C6 | Gzip request when profile disallows compression | 406 | `unsupported_encoding` | Harness sends `Accept-Encoding: gzip`; `details.encoding` records the unsupported encoding. |
| C7 | Proof tampering (chunk digest mismatch) | 422 | `proof_mismatch` | Gateway rejects PoR proofs whose chunk digest is replaced with the `0xff` tamper sentinel; `details.section` identifies the offending sample. |

Fixtures live under `fixtures/sorafs_gateway/capability_refusal/` and are referenced by the integration tests. Each scenario exposes `request.json`, `response.json`, and `gateway.json` alongside the shared `scenarios.json` index so operators and SDK authors can reproduce the refusal end-to-end using the canonical fixture bundle in `fixtures/sorafs_gateway/1.0.0/`.

> **Header contracts:** `Sora-Name` carries the requested alias, `Accept` must include `dag-scope=block` when requesting trustless CAR ranges, and `Accept-Encoding` must omit `gzip` for profiles that do not advertise compression support.

## Response Contract

Gateways emit JSON bodies shaped as follows:

```json
{
  "error": "unsupported_chunker",
  "reason": "profile sorafs.sf1@1.0.0 is not enabled on this gateway",
  "details": {
    "request_id": "uuid",
    "manifest_cid": "bafkrei…",
    "hint": "check alias binding"
  }
}
```

- `error` (string, required) — machine code, matches the telemetry `reason`.
- `reason` (string, required) — operator-facing explanation.
- `details` (object, optional) — additional context (`request_id`, `manifest_cid`, `provider_id`, `hint`, etc.). Keys are optional and omitted when not applicable.
- The conformance harness persists the same payload under each scenario's `refusal` object inside the attestation JSON (`suite.scenarios[].refusal`), including the HTTP status code for parity checks.

**Gateway requirements**

- HTTP status must match the table above.
- Response must include `Content-Type: application/json`.
- `X-SoraFS-Request-Id` header mirrors `details.request_id` when present.

## Telemetry & Logging

- **Counter:** `torii_sorafs_gateway_refusals_total{reason,profile,provider_id,scope}` increments once per refused request (provider id is the empty string when the caller did not supply one).
- **Structured log:** Emit entries containing `error`, `reason`, `status_code`, `remote_addr`, `request_duration_ms`, and any `details.*` fields. Do not log stream tokens or other secrets.
- **Dashboards/Alerts:** Plot refusal counts per `reason`; alert on spikes (e.g., `unsupported_chunker` during rollout) using 5‑minute windows or anomaly detection.

## SDK Expectations

- Client libraries (Rust, TypeScript, Go, Swift, etc.) should parse the JSON and surface `error` + `reason` to callers.
- Unit/integration tests in each SDK should simulate the matrix above using the shared loader (`integration_tests/src/sorafs_gateway_capability_refusal.rs`) to keep fixture parity with Torii.

## Relationship to Self-Certification

The self-cert handbook (`docs/source/sorafs_gateway_self_cert.md`) and deployment guide reference the refusal IDs when guiding operators through troubleshooting. The `sorafs_fetch --gateway-provider …` workflow can be used to reproduce individual scenarios for documentation or training.
