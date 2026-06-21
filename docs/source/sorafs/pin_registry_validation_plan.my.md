---
lang: my
direction: ltr
source: docs/source/sorafs/pin_registry_validation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 62c8c31ff44fb65b8f1cab411aeb5146062d8f0812324a98add868df5cd561eb
source_last_modified: "2025-12-29T18:16:36.120274+00:00"
translation_last_reviewed: 2026-02-07
title: "Pin Registry Manifest Validation Plan"
---

# Pin Registry Manifest Validation Plan (SF-4 Prep)

This plan outlines the steps required to thread `sorafs_manifest::ManifestV1`
validation into the forthcoming Pin Registry contract so that SF-4 work can
build on the existing tooling without duplicating encode/decode logic.

## Goals

1. Host-side submission paths verify manifest structure, chunking profile, and
   governance envelopes before accepting proposals.
2. Torii and gateway services reuse the same validation routines to ensure
   deterministic behaviour across hosts.
3. Integration tests cover positive/negative cases for manifest acceptance,
   policy enforcement, and error telemetry.

## Architecture

```mermaid
flowchart LR
    cli["sorafs_pin CLI"] --> torii["Torii Manifest Service"]
    torii --> validator["ManifestValidator (new)"]
    validator --> manifest["sorafs_manifest::ManifestV1"]
    validator --> registry["Pin Registry Contract"]
    validator --> policy["Governance Policy Checks"]
    registry --> torii
```

### Components

- `ManifestValidator` (new module in `sorafs_manifest` or `sorafs_pin` crate)
  encapsulates structural checks and policy gates.
- Torii exposes a gRPC endpoint `SubmitManifest` that calls into
  `ManifestValidator` before forwarding to the contract.
- Gateway fetch path optionally consumes the same validator when caching new
  manifests from the registry.

## Task Breakdown

| Task | Description | Owner | Status |
|------|-------------|-------|--------|
| V1 API skeleton | Add `validate_manifest(manifest: &ManifestV1, policy: &PinPolicyInputs) -> Result<(), ValidationError>` to `sorafs_manifest`. Include BLAKE3 digest verification and chunker registry lookup. | Core Infra | ✅ Done | Shared helpers (`validate_chunker_handle`, `validate_pin_policy`, `validate_manifest`) now live in `sorafs_manifest::validation`. |
| Policy wiring | Map registry policy config (`min_replicas`, replica ceilings, retention ceilings, storage-class allowlists, and council-signature requirements) into validation inputs. | Governance / Core Infra | ✅ Done | `manifest_pin_policy_constraints_from_config` maps governance config into `sorafs_manifest::PinPolicyConstraints`; `RegisterPinManifest` enforces the registry DTO subset before state or fee side effects, and Torii validates the full `ManifestV1` governance envelope when `manifest_b64` is supplied. |
| Torii integration | Call validator inside Torii manifest submission path; return structured Norito errors on failure. | Torii Team | ✅ Done | `/v1/sorafs/pin/register` validates chunker and pin-policy fields through the shared validator, accepts optional `manifest_b64` for full Norito `ManifestV1` validation, checks digest/chunker/content-length/policy consistency, and requires `manifest_b64` when governance requires council signatures. |
| Host contract stub | Ensure contract entrypoint rejects manifests that fail validation hash; expose metrics counters. | Smart Contract Team | ✅ Done | `RegisterPinManifest` now invokes the shared validator (`ensure_chunker_handle`/`ensure_pin_policy`) before mutating state and unit tests cover the failure cases. |
| Tests | Add unit tests for validator error cases and integration tests in `crates/iroha_core/tests/pin_registry.rs`. | QA Guild | ✅ Done | Validator tests cover chunker/profile checks, council-signature policy, replica floors/ceilings, retention ceilings, and storage-class allowlists. The integration suite covers on-chain registration acceptance and governance-policy rejections without fee or state side effects. |
| Docs | Update `docs/source/sorafs_architecture_rfc.md` and `migration_roadmap.md` once validator lands; document CLI usage in `docs/source/sorafs/manifest_pipeline.md`. | Docs Team | ✅ Done | Architecture, migration, manifest-pipeline, CLI, OpenAPI, status, and roadmap docs now describe the completed shared validation path and the remaining work has moved to endpoint error-label hardening rather than validator wiring. |

## Dependencies

- Pin Registry Norito schema finalisation (ref: SF-4 item in roadmap).
- Council-signed chunker registry envelopes (ensures validator mapping is
  deterministic).
- Torii authentication decisions for manifest submission.

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Divergent policy interpretation between Torii and contract | Non-deterministic acceptance. | Share validation crate + add integration tests that compare host vs on-chain decisions. |
| Performance regression for large manifests | Slower submission | Benchmark via cargo criterion; consider caching manifest digest results. |
| Error messaging drift | Operator confusion | Define Norito error codes; document them in `manifest_pipeline.md`. |

## Timeline Targets

- Week 1: Land `ManifestValidator` skeleton + unit tests.
- Week 2: Wire Torii submission path and update CLI to surfacing validation errors.
- Week 3: Implement contract hooks, add integration tests, update docs.
- Week 4: Run end-to-end rehearsal with migration ledger entry, capture council sign-off.

This plan will be referenced in the roadmap once the validator work begins.
