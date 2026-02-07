---
lang: am
direction: ltr
source: docs/source/sorafs_gateway_refusal_guidance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2995bb7a8cb21272e40aa80a85c24a3f4749197d076b03a3094345ca25b54ce4
source_last_modified: "2026-01-05T09:28:12.085413+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway Capability Refusal Guidance
summary: Operator and developer playbook for handling capability refusal scenarios (SF-5c).
---

# SoraFS Gateway Capability Refusal Guidance

This document promotes the SF-5c roadmap item deliverable “Publish operator/dev
guidance” by documenting how SoraFS gateways, SDKs, and downstream tooling must
detect, surface, and remediate refusal paths that protect trustless delivery.
The guidance complements the negative test matrix in
`docs/source/sorafs_gateway_capability_tests.md` and the self-certification kit
draft captured in `docs/source/sorafs_gateway_self_cert.md`.

## Audience & Scope

- **Operators** running gateways or orchestrators who must keep refusal behaviour
  deterministic, observable, and auditable.
- **Developers** maintaining SDKs/CLIs or integrating SoraFS endpoints into
  downstream applications.
- **QA/DevRel** teams wiring conformance suites and onboarding workflows that
  exercise refusal paths before deployment.

The catalogue below enumerates the refusal reasons that MUST remain stable in
the API surface. New refusal reasons require a roadmap update and matching
fixtures before production rollout.

## Refusal Catalogue

The gateway MUST encode refusals as JSON bodies matching the schema described in
`docs/source/sorafs_gateway_capability_tests.md` and reuse the `error` field as
the telemetry label. Table rows double as the minimum compliance expectation.

| ID | HTTP | `error` code | Telemetry label(s) | Primary remediation | Cross-reference |
|----|------|--------------|--------------------|---------------------|-----------------|
| C1 | 406 | `unsupported_chunker` | `unsupported_chunker` | Align chunker handle with SF-1 profile list; refresh manifest alias. | Chunker registry docs, SF-1 deliverables. |
| C2 | 412 | `manifest_variant_missing` | `manifest_variant_missing`, `alias_stale` | Publish successor manifest or pin alias to requested profile; verify registry snapshot. | Pin registry plan, alias policy. |
| C3 | 412 | `admission_envelope_mismatch` | `governance_violation`, `alias_stale` | Regenerate admission envelope via Torii; confirm governance signatures. | `docs/source/sorafs/pin_registry_validation_plan.md`. |
| C4 | 428 | `capability_grease_unsupported` | `capability_refusal` | Reject unknown TLVs unless explicitly allowed; ensure client negotiated supported capabilities. | Capability GREASE RFC draft. |
| C5 | 428 | `missing_dag_scope` | `capability_refusal` | Add `Sora-Dag-Scope` header or upgrade client; configure orchestrator defaults. | Chunk-range spec (SF-5d). |
| C6 | 406 | `unsupported_encoding` | `unsupported_encoding` | Drop `Accept-Encoding` requests or enable compression in manifest policy. | Manifest policy docs. |
| C7 | 422 | `proof_validation_failed` | `proof_validation_failed` | Verify PoR/PoTR proofs; inspect harness replay output for corrupted chunks. | Replay harness guide. |

Gateways MAY extend the table with additional telemetry labels (e.g.,
`profile=sf1`, `provider_id=aa12`) but MUST retain the listed base labels so
dashboards and alerts remain portable across deployments.

## Troubleshooting Playbooks

### Unsupported Chunker (`C1`)

1. Inspect the request log entry for the `chunker_handle` hint emitted by Torii.
2. Compare against the manifest profile list served by the Pin Registry
   (`/v1/sorafs/pin/<cid>`). The manifest MUST include a chunker matching the
   request.
3. If the manifest is missing, publish a successor manifest via
   `sorafs-manifest submit` and wait for governance approval.
4. Confirm that the gateway cache refreshed within
   `alias_refresh_window` (see `docs/source/sorafs_alias_policy.md`). If the
   cache exceeded `alias_hard_expiry`, evict locally and re-request.

### Manifest Variant Missing (`C2`)

1. Use `sorafs-manifest inspect` to confirm the requested profile exists.
2. If the alias already points at a manifest lacking the profile, submit a
   successor manifest and trigger replication orders.
3. Verify that registries broadcast `alias-proof-updated` SSE events and that
   the gateway subscribed successfully. Missing events often indicate TLS or
   auth drift.

### Admission Envelope Mismatch (`C3`)

1. Run `sorafs-manifest attest --cid <cid>` to regenerate an admission envelope.
2. Check the Norito payload digest matches the manifest CID; mismatches indicate
   stale governance logs.
3. Confirm each council signature with `sorafs-manifest verify`, ensuring public
   keys align with the latest rotation.
4. If the envelope belongs to a revoked governance epoch, operators must fetch a
   newer snapshot before the gateway accepts the manifest.

### Capability TLV GREASE / Missing Dag Scope (`C4`, `C5`)

1. Capture the HTTP request to ensure the client advertises a capability list.
2. If GREASE TLVs appear, confirm the client enabled the preview flag listed in
   the GREASE RFC; if not, instruct the client to align with the published
   manifest capability table.
3. For missing `Sora-Dag-Scope`, update the orchestrator or SDK configuration to
   inject the header automatically; failure to do so indicates SDK regression.

### Unsupported Encoding (`C6`)

1. Inspect the manifest `StreamEncodingPolicy` in the Pin Registry snapshot.
2. If compression is disallowed, consider enabling the `compression` capability
   and publishing a successor manifest. Otherwise, strip `Accept-Encoding` from
   client requests or negotiate deflate/gzip explicitly.

### Proof Validation Failed (`C7`)

1. Run the replay harness (`cargo test -p integration_tests sorafs_gateway_conformance`)
   against the gateway; ensure the negative fixture reproduces the failure.
2. Inspect the PoR transcript in
   `fixtures/sorafs_gateway/conformance/proofs/*.norito` for tampering.
3. If corruption originated upstream, quarantine the provider and rotate the
   manifest alias to a healthy replica. Record incident IDs in governance logs.

## Developer Responsibilities

- SDKs MUST surface refusal responses verbatim, including `error`, `reason`, and
  `details`, and avoid coercing failures into transport errors.
- Provide typed error enums mirroring the catalogue above and ensure retries
  treat `4xx` refusals as deterministic (no exponential backoff).
- When SDKs expose automatic fallback, only retry against a different provider
  if the refusal reason is `unsupported_chunker` and a matching profile exists.
- Client libraries MUST log refusal events with the same telemetry labels so
  operators can correlate gateway and client trends.
- Integration tests SHOULD import the canonical fixtures referenced in the
  self-certification kit to avoid drift between SDK and gateway behaviour.

## Compliance & Monitoring

- Gateways MUST expose `sorafs_gateway_refusals_total` with at least the `reason`
  label and SHOULD include `profile`, `provider_id`, and `scope`.
- Alerts:
  - Trigger `critical` alert if `unsupported_chunker` rates spike above
    `threshold=25/minute` for five minutes.
  - Trigger `warning` alert if total refusals exceed the moving one-hour mean by
    more than two standard deviations.
- Logs MUST redact bearer tokens and session secrets. Retain refusal logs for at
  least 30 days to satisfy governance audits.
- Operators MUST document refusals in incident runbooks when a refusal type
  persists beyond 15 minutes or impacts more than 5% of traffic.
- Ensure CI pipelines run the replay harness refusal suite before deployment;
  failures block promotion.

## Integration Checklist

Before onboarding a gateway or SDK release, confirm:

1. Replay harness negative scenarios pass with deterministic status codes.
2. `self-cert` report includes refusal counts with matching telemetry labels.
3. Dashboards plot the refusal counters with baselines and alerts configured.
4. Operator runbooks reference this document for remediation steps.
5. Developers updated SDK release notes summarising any new refusal handling.

## Change Management

- Any modification to refusal codes or HTTP statuses requires:
  1. Update to this guidance document and the test matrix.
  2. Regenerated fixtures and replay harness coverage.
  3. Governance council approval with signed manifest.
- Keep roadmap item SF-5c updated when new refusal paths move from draft to
  production to avoid drift between the docs, tests, and telemetry.
