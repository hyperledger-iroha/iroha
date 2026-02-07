---
lang: uz
direction: ltr
source: docs/source/taikai_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0858e93d54041f4c57759e797888e7d6219be88d76d0bc04f4a18a26d70242f3
source_last_modified: "2026-01-22T14:35:37.633133+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Policy & Crypto Plan

_Status: Completed_ — Owners: Governance Council / Security Engineering / Media Platform WG  
Roadmap item: **SN13-E — Policy/crypto (GAR v2, CEK rotation, RPT)** — GAR v2
payloads (licensing/moderation/metrics + RPT digests) are live in
`iroha_data_model`/`sorafs_manifest`, `iroha app taikai cek-rotate` +
`rpt-attest` emit deterministic receipts/attestations (covered by
`crates/iroha_cli/tests/taikai_policy.rs`), and `cargo xtask taikai-rpt-verify`
verifies bundles for rollout gates.

Taikai’s broadcast stack already ships deterministic ingest and caching
primitives (`TaikaiSegmentEnvelopeV1`, `TaikaiCache`) plus the governance
surface that ties manifests, encryption, and attestation artefacts together.
This note remains the canonical reference for the shipped policy/crypto
behaviour.

## Objectives

1. **Gateway Authorization Record (GAR) v2** — Extend the JWS payload parsed by
   `crates/sorafs_manifest/src/gateway.rs` with licensing, moderation, and
   metrics directives so governance can express per-event constraints without
   bespoke configuration.
2. **Content Encryption Key (CEK) rotation backed by KMS** — Tie the Norito
   streaming hierarchy (GCK → CEK) to managed keys, rotation policies, and
   observability so relays can prove that encryption state tracks governance
   policy.
3. **Replication Proof Token (RPT) attestation workflow** — Produce a signed
   artefact for every rollout that binds GAR, CEK lineage, and distribution
   evidence together. Provide a CLI + CI hook so operators cannot skip it.

Deliverables must be deterministic (Norito wire formats, CLI output paths) and
land alongside fixtures/tests so CI can gate future changes.

## GAR v2 (Licensing, Moderation, Telemetry)

GAR remains a compact JWS with Ed25519 signatures; v2 adds schema fields and
validation rules rather than inventing a new envelope. The payload extensions
until all issuers migrate.

### Payload extensions

| Field | Type | Purpose |
|-------|------|---------|
| `license_sets` | array<`LicenseSetV1`> | Declares the legal basis (jurisdiction, holder, expiry) for distributing the referenced manifest. Required for public events; optional for private pilots. |
| `moderation_directives` | array<`ModerationDirectiveV1`> | Enumerates policy hints (allowlist, denylist, sensitivity classes) that gateways must enforce before serving a segment. |
| `metrics_policy` | `MetricsPolicyV1` | Specifies which audience metrics may be collected, sampling ceilings, and retention periods. |
| `telemetry_labels` | array<string> | Canonical labels emitted with `torii_sorafs_gar_violations_total` so alerts remain searchable. |
| `rpt_digest` | `[u8; 32]` | Optional BLAKE3 digest of the latest RPT attestation bundle, enabling gateways to assert that the operational evidence matches governance intent. |

All new structs live in `iroha_data_model::sorafs::gar` and use Norito codecs.
Issuers MUST include at least one license set whenever the manifest references
public audiences. Moderation directives support escalation levels (e.g.,
`Block`, `Warn`, `Quarantine`); gateways that lack a corresponding toggle must
refuse the request and emit `SorafsGarPolicyDetail::DenylistedContent`.

### Validation & enforcement

- **Signature surface:** JWS header `kid` keys remain Ed25519. Governance signs
  GAR v2 with the same key catalog used today; rotation procedure adds
  `preferred_kid` so operators know which key should verify the record.
- **Evaluation:** `GatewayAuthorizationRecord::ensure_applicable_at` keeps the
  validity window logic. New helper `GatewayAuthorizationRecord::policy_payload`
  exposes the structured additions so gateways do not have to re-parse JSON.
- **Telemetry:** `iroha_telemetry::privacy::SoranetGarAbuse*` gains label/value
  pairs for `license_slug`, `moderation_directive`, and `metrics_policy_id`.
  Alert pack updates mirror these additions so incidents stay actionable.
- **Docs & fixtures:** `docs/source/taikai_segment_envelope.md` references GAR
  v2 when describing SoraNS anchoring, and fixtures under
  `fixtures/sorafs_manifest/gateway_v2/` cover good/bad payloads plus
  downgrade paths.

### Migration plan

1. **Pilot:** Accept optional v2 fields; keep them opaque in telemetry.
2. **Validation gate:** Upgrade `ci/check_sorafs_gateway_conformance.sh` to
   require GAR v2 payloads for public manifests.
3. **Default:** Remove v1-only fixtures; reject records lacking licensing data.
   Target date: Q1 2027 per SN13-E.

## CEK Rotation & Managed KMS

Norito streaming (§12 of `norito_streaming.md`) defines the cryptographic
hierarchy: viewers receive Group Content Keys (GCKs) via `ContentKeyUpdate`
frames, then derive per-segment CEKs via HKDF. SN13-E mandates operational
controls around that flow.

### Key hierarchy

| Layer | Material | Storage | Rotation trigger |
|-------|----------|---------|------------------|
| Root KMS key | `taikai/<event>/root` (Ed25519 + HKDF salt) | CloudHSM / NitroKMS / SoftHSM (test) | Annual, or on compromise. |
| GCK wrap key | `taikai/<event>/<stream>/gck_wrap` | KMS symmetric key (AES-256-GCM) | Every 15 minutes of media or membership change. |
| CEK | Derived via `HKDF-Expand(GCK, rend_id || segment_seq)` | Memory only | Every segment. |

The ingest pipeline records the active key IDs inside
`TaikaiSegmentEnvelopeV1::metadata` so data availability proofs can assert the
exact KMS lineage without revealing secrets.

### Automation & CLI

- New command: `iroha app taikai cek-rotate --event <event> --stream <stream> \
  --effective-segment <seq> --kms-profile <profile>` which:
  1. Requests a fresh GCK wrap key from the configured KMS profile.
  2. Emits a Norito `CekRotationReceiptV1` containing the old/new key labels,
     HKDF salt, and operator signature.
  3. Stores the receipt under
     `artifacts/taikai/cek_rotations/<event>/<stream>/<timestamp>.to`.
- CI hook: `ci/check_taikai_keys.sh` (future) asserts receipts exist for every
  published manifest window. Until then, release engineering runs the CLI
  manually and attaches receipts to rollout tickets.
- Telemetry: `taikai.cek_rotation_total{event,stream}` and
  `taikai.cek_rotation_failure_total{reason}` exported via `iroha_telemetry`.

### Observability & alerts

- Grafana board `taikai_encryption_health.json` plots rotation cadence, KMS
  latency, and CEK derivation errors.
- Alert rule: if no rotation occurs for >20 minutes during an active broadcast
  raise `taikai_cek_rotation_stalled`.

## Replication Proof Tokens (RPT)

RPTs are signed Norito envelopes that prove a Taikai rollout tied GAR v2,
encryption state, and SoraFS distribution evidence together. They are designed
to be generic so SoraDNS/SoraNet programs can re-use them when proving cache
consistency or zone publication.

### Schema

`ReplicationProofTokenV1` (draft) fields:

| Field | Description |
|-------|-------------|
| `event_id`, `stream_id`, `rendition_id` | Scope of the attestation. |
| `gar_digest` | BLAKE3-256 digest of the GAR v2 JWS payload. |
| `cek_receipt_digest` | Digest of the latest `CekRotationReceiptV1`. |
| `distribution_bundle_digest` | Digest of the artefact directory under `artifacts/fastpq_rollouts/` (or Taikai equivalent) proving CAR replication. |
| `policy_labels` | Copy of GAR `telemetry_labels` at attestation time. |
| `valid_from`, `valid_until` | Unix timestamps describing the coverage window. |
| `signer` | Governance key identifier. |

### Workflow

1. Operators run `iroha app taikai rpt-attest --event <event> --stream <stream> \
   --manifest <gar.json> --cek-receipt <path> --bundle <dir> --out <path>`.
2. CLI validates each input (GAR signature, CEK receipt signature, bundle hash)
   and writes:
   - `taikai_rpt_<timestamp>.to` — Norito-encoded RPT.
   - `taikai_rpt_<timestamp>.json` — human summary.
3. `cargo xtask taikai-rpt-verify --envelope <path>` verifies signatures and
   prints the digests. CI wires this into the rollout gate alongside the GAR
   and CEK checks.
4. Torii stores the latest valid RPT digest under `/v1/config/taikai.rpt.digest`
   so relays and gateways can assert that the operational surface matches the
   attested policy.

### CLI integration

- `iroha app taikai status` gains `--show-rpt` to fetch and display the live digest
  plus attestation window.
- `sorafs_orchestrator` exposes `taikai.rpt_digest()` for health checks.

## Acceptance Criteria & Timeline

| Milestone | Scope | Target |
|-----------|-------|--------|
| GAR v2 MVP | Structs, parser updates, fixtures, docs | 2026‑12 |
| CEK rotation beta | CLI, receipts, telemetry hooks | 2027‑01 |
| RPT GA | CLI + xtask verifiers, Torii plumbing, docs | 2027‑02 |

Exit checklist:

1. GAR v2 fields documented here are implemented, covered by unit/serde tests,
   and referenced in `docs/source/sorafs_gateway_dns_design_pre_read.md`.
2. CEK rotation CLI emits receipts referenced by SoraNS anchors (`TaikaiSegmentEnvelopeV1::metadata`).
3. `ci/check_taikai_policy.sh` (follow-up) ensures every rollout bundle
   contains GAR v2, CEK receipts, and RPT envelopes before merges land.

This document stays the canonical reference; update it whenever policy or
crypto behaviour changes, and mirror high-level status in `status.md`.
