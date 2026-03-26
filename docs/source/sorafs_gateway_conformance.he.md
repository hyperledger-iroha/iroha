---
lang: he
direction: rtl
source: docs/source/sorafs_gateway_conformance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cbf63ac52586868a49b6105d333ea01a9bc31e9388b6fb4e6c8d39cb56e17cae
source_last_modified: "2026-01-22T07:34:23.474301+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Gateway Conformance Harness
summary: Replay and load-testing plan for the SF-5 trustless delivery profile.
---

# SoraFS Gateway Conformance Harness

This note outlines the proposed architecture for the SF-5a conformance harness.
It is an implementation plan, not the final deliverable. The intent is to give
QA and Tooling a shared blueprint before integration work begins.

## Objectives

1. **Replay validation:** Feed canonical CAR fixtures through a gateway and
   assert BLAKE3 + PoR proofs match the manifests published in SoraFS fixtures.
2. **Negative coverage:** Ensure gateways correctly refuse unsupported chunker
   handles, malformed proofs, admission mismatches, and downgrade attempts.
3. **Load testing:** Sustain ≥1,000 concurrent range streams against a seeded
   payload set and verify deterministic latency, throughput, and refusal behaviour.
4. **Attestation:** Produce structured run reports that operators can sign when
   self-certifying gateways.

## CI Integration

The deterministic replay suite now runs as part of the workspace CI helpers via
`ci/check_sorafs_gateway_conformance.sh`. The script executes
`cargo test --locked -p integration_tests sorafs_gateway_conformance -- --nocapture`
to replay the canonical fixture matrix and fails fast when regressions surface.
Nightly pipelines should invoke the script alongside `ci/check_sorafs_fixtures.sh`
so conformance stays gated on the same proofs, manifests, and refusal scenarios
used for operator attestations.

The same test binary now drives the deterministic load harness
(`run_deterministic_load_test`) which spawns ≥1,000 concurrent requests across
the success/refusal mix defined by the SF-5a plan. Results, including latency
percentiles and refusal/error counters, flow into `SuiteReport::to_json_value`
so governance attestation bundles include both replay and load-test evidence.

## Architecture Sketch

```
┌────────────────────────────┐
│  Fixture Registry (Norito) │
│   - Manifests              │
│   - Proof bundles          │
│   - Negative cases         │
└────────────┬───────────────┘
             │
┌────────────▼─────────────┐      ┌──────────────────────────┐
│   Replay Controller      │      │   Load Generator          │
│ (Rust, Tokio orchestrator│◀────▶│ (Rust + configurable     │
│  + Norito validators)    │      │  workers, configurable)   │
└────────────┬─────────────┘      └──────────────────────────┘
             │
┌────────────▼─────────────┐
│ Gateway Adapter Layer    │
│  - HTTP client shim      │
│  - Header injection      │
│  - Proof extraction      │
└────────────┬─────────────┘
             │
┌────────────▼─────────────┐
│ Verification Pipeline    │
│  - Manifest digest check │
│  - Chunk plan validation │
│  - PoR proof verification│
└────────────┬─────────────┘
             │
┌────────────▼─────────────┐
│ Report & Attestation     │
│  - JSON summary          │
│  - Failure artifacts     │
│  - Signing hook (Norito  │
│    attestation envelope) │
└──────────────────────────┘
```

## Attestation Signing Hook

The conformance harness emits machine-readable results _and_ a Norito-signed
attestation so operators can present evidence to governance. The signing flow
is deterministic and cross-platform; it does not rely on bespoke tooling.

1. **Canonicalise the report payload.** Serialize the run report structure with
   `norito::json::to_vec(&report)` to obtain the canonical byte sequence (store
   it as `payload_bytes`). Norito's writer already enforces deterministic key ordering. The report structure must match the schema described in
   [Reporting Format](#reporting-format) and should already contain the
   `fixtures_commit`, `run_id`, and `scenarios` array.
2. **Hash for auditability.** Compute `let digest = blake3::hash(&payload_bytes);`
   and store the 32-byte output in both hexadecimal and multibase encodings.
   This digest becomes the `attestation.payload_hash` field.
3. **Sign with configured key material.** Load the operator's key pair from the
   harness configuration (e.g., `gateway_attestor.key_path`). Keys must use the
   same scheme enforced by governance manifests—`Ed25519` today. The harness
   calls `iroha_crypto::Signature::new(key_pair.private_key(), &payload_bytes)`
   and records both the signature bytes and the signing algorithm identifier.
4. **Wrap in a Norito envelope.** Construct a `SignedGatewayReport` structure:

   Example harness snippet:

   ```rust
   use blake3;
   use hex;
   use multibase::{encode, Base};
   use iroha_crypto::{Algorithm, KeyPair, Signature};
   use norito::json::{json, Value};

   let key_pair: KeyPair = load_key_pair_from_disk(&config.gateway_attestor)?;
   let payload_bytes = norito::json::to_vec(&report)?;
   let report_json_value: Value = norito::json::from_slice(&payload_bytes)?;
   let digest = blake3::hash(&payload_bytes);
   let signature = Signature::new(key_pair.private_key(), &payload_bytes);
   let (pk_alg, pk_bytes) = key_pair.public_key().to_bytes();
   let envelope = json!({
       "attestation": {
           "payload_hash": {
               "blake3_hex": format!("{digest:x}"),
               "blake3_multibase": format!("z{}", encode(Base::Z, digest.as_bytes())),
           },
           "signer": {
               "account_id": config.operator_account.as_ref(),
               "public_key_hex": hex::encode(pk_bytes),
               "algorithm": pk_alg.as_static_str(),
           },
           "signature_hex": hex::encode(signature.payload()),
           "signed_at_unix": std::time::SystemTime::now()
               .duration_since(std::time::UNIX_EPOCH)
               .expect("system clock before UNIX_EPOCH")
               .as_secs(),
       },
       "report": report_json_value, // identical to reporting payload
   });
   ```

   Encode the envelope via `norito::json::to_vec(&envelope)` for archival.
   The helper `load_key_pair_from_disk` is a thin wrapper around
   `iroha_crypto::KeyPair::from_private_key` that enforces filesystem ACL checks
   and supports hardware-backed adapters.
5. **Persist artifacts.** Write three files per run:
   - `report.json` — the unsigned JSON report (canonical formatting).
   - `report.attestation.to` — Norito-encoded attestation envelope.
   - `report.attestation.txt` — human-readable summary (hash + signer).

6. **Verification CLI.** Ship `sorafs-gateway-cert verify` (part of the planned
   `sorafs-gateway-cert` crate) that performs the inverse operation:
   - Parse the Norito attestation.
   - Recompute BLAKE3 over the embedded report.
   - Verify the signature with `iroha_crypto::Signature::verify`.
   - Emit success/failure with a non-zero exit code on mismatch.

You can generate the report/attestation bundle locally via `cargo xtask sorafs-gateway-attest`.
The command accepts `--signing-key <path>` (hex-encoded private key),
`--signer-account <soraカタカナ...>` (domainless encoded AccountId; `@domain` suffix rejected), and optional `--gateway <url>` plus `--out <dir>`.
Artifacts default to `artifacts/sorafs_gateway_attest/`.

By keeping the signing hook inside the harness binary, nightly CI runs can
auto-publish signed artifacts, and operators only need to provide a keypair
path or hardware-backed signer implementation via the same trait.

## Test Matrix

| ID | Scenario | Expected Result |
|----|----------|-----------------|
| A1 | Full CAR replay (sf1 profile) | 200 OK, chunk/PoR verification pass |
| A2 | Byte range (aligned to chunk boundaries) | 206 Partial Content, proofs limited to requested range |
| A3 | Byte range (misaligned request) | 416 Range Not Satisfiable |
| A4 | Multi-range byte replay (partial + full chunk) | 206 Partial Content with ordered multipart segments |
| B1 | Unsupported chunker handle | 406 with `unsupported_chunker` body |
| B2 | Missing manifest envelope | 428 admission failure |
| B3 | Corrupted PoR proof | 422 proof failure |
| B4 | Corrupted CAR payload (digest mismatch) | 422 refusal (payload digest mismatch) |
| B5 | Provider not admitted | 412 precondition failure with `provider_not_admitted` |
| B6 | Client exceeds rate limit window | 429 `rate_limited` with `Retry-After` header |
| C1 | 1k concurrent range streaming (warm cache) | P95 latency < target, no proof failures |
| C2 | 1k concurrent streaming with injected 1% corruption | All corrupted responses rejected, gateway returns 422 |
| D1 | Load with GAR denylist trigger | 451 Unavailable For Legal Reasons |

The Rust harness already exercises scenarios A1, A2, A3, A4, B1, B2, B3, B4, B5, and B6 against deterministic fixtures, asserting canonical digests, byte-range alignment, refusal semantics, and policy enforcement.

## Sample Denylist

To exercise policy enforcement without wiring governance feeds, point the node configuration
`torii.sorafs_gateway.denylist.path` at `docs/source/sorafs_gateway_denylist_sample.json`. The
fixture contains:
- `provider` and `manifest_digest` entries that demonstrate fixed-width hex identifiers with optional jurisdiction windows.
- a `cid` entry encoded in base64 alongside an expiry window.
- a `url` entry for URL-level blocking.
- an `account_id` entry using the canonical AccountAddress hex encoding to mirror governance suspensions.
- an `account_alias` entry that blocks an on-chain account alias (`name@dataspace` or `name@domain.dataspace`).
- a `perceptual_family` entry that pairs a family/variant UUID with perceptual hash metadata (`perceptual_hash_hex`, `perceptual_hamming_radius`) so gateways can block near-duplicate content clusters.

Each record follows the same Norito JSON layout used by the loader, including optional `issued_at`
and `expires_at` fields. Operators can copy the file as a starting point for their own governance
feeds or integration tests.

## Reporting Format

The harness should emit a signed JSON document:

```json
{
  "profile_version": "sf1",
  "gateway_target": "https://gateway.example.com",
  "fixtures_commit": "abc123",
  "scenarios": [
    { "id": "A1", "status": "pass", "duration_ms": 420 },
    { "id": "B3", "status": "pass" },
    { "id": "C1", "status": "pass", "p95_latency_ms": 84 }
  ],
  "failures": [],
  "timestamp": "2025-02-14T12:34:56Z",
  "run_id": "uuid"
}
```

Operators can append their signature (Norito-signed envelope) for self-certification.

## Manual Gateway Smoke Test (`sorafs-fetch`)

For lightweight validation outside the full harness, operators can point the multi-source CLI directly at a Torii gateway using the new `--gateway-provider` flag:

```
sorafs-fetch \
  --plan=fixtures/chunk_fetch_plan.json \
  --gateway-provider=name=gw-a,provider-id=<hex>,base-url=https://gw-a.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --gateway-client-id=ops-orchestrator \
  --json-out=reports/gateway_smoke.json
```

The resulting report mirrors the conformance output (including `provider_reports[].metadata`) and can be attached to change tickets during blue/green rollouts.

## Open Questions / Next Steps

- Determine fixture repository layout (likely `fixtures/sorafs_gateway/`).
- Decide on PoR sampling defaults for load tests (tie-in with SF-13).
- Select concurrency driver (Tokio vs external load generator).
- Integrate with CI (GitHub Actions & internal Jenkins).

Downgrade/missing-header refusal and HEAD probe coverage now live in the
integration harness (`integration_tests/tests/sorafs_gateway_conformance.rs:295`
and `integration_tests/tests/sorafs_gateway_conformance.rs:442`).

Feedback is welcome—track comments under SF-5a in the roadmap.
