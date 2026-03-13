---
lang: mn
direction: ltr
source: docs/source/soradns/resolver_attestation_directory.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fce286e2e094f8f26ebb9a0aa1f4818388fd599e4cfad6f23a72938a0aa1db06
source_last_modified: "2025-12-29T18:16:36.104567+00:00"
translation_last_reviewed: 2026-02-07
---

# Resolver Attestation Directory (DG-4)

Status: Drafted 2026-04-02  
Owners: Networking TL, Governance Council, Platform SRE  
Roadmap reference: DG-4 — Resolver Attestation Directory

## 1. Overview

SoraDNS gateways must only trust recursive resolvers that can prove:

1. the code and configuration they run were approved by governance;
2. their TLS material matches the approved manifest; and
3. their discovery metadata (hostnames, GAR bindings, transport policies)
   have not been tampered with in transit.

The Resolver Attestation Directory (RAD directory) provides that trust anchor.
Each resolver publishes a Norito-encoded **Resolver Attestation Document
(RAD)** signed by the operator and co-signed by the governance council.
Gateways, SDKs, and tooling subscribe to the directory and refuse to speak
to resolvers whose attestation chain they cannot verify.

This document captures the RAD schema, the distribution model, rotation and
monitoring policy, and the notification channel that keep gateways in sync.

## 2. Artefacts

### 2.1 Resolver Attestation Document (RAD)

RADs are versioned Norito payloads. The initial schema uses `version = 1`
and is defined as:

```norito
#![norito::schema(version = 1)]
struct ResolverAttestationDocumentV1 {
    resolver_id: [u8; 32],          // blake3(public_key || fqdn)
    fqdn: String,                   // punycode-normalised name
    canonical_hosts: GatewayHostSet,
    transport: ResolverTransportBundle,
    tls: ResolverTlsBundle,
    resolver_manifest_hash: [u8; 32],
    gar_manifest_hash: [u8; 32],
    issued_at_unix: u64,
    valid_from_unix: u64,
    valid_until_unix: u64,
    operator_account: AccountId,
    operator_signature: Signature,
    governance_signature: Signature,
    rotation_policy: RotationPolicyV1,
    telemetry_endpoint: Option<String>,
}
```

* `GatewayHostSet` mirrors the deterministic host derivation described in
  `docs/source/soradns/deterministic_hosts.md`.
* `ResolverTransportBundle` enumerates the enabled transports (DoH, DoT,
  DoQ/ODoH preview) along with port ranges, cipher requirements, and the
  Norito flags that spell out padding/QNAME-minimisation behaviour.
* `ResolverTlsBundle` encodes the ACME profile (DNS-01 or TLS-ALPN-01),
  certificate fingerprints, and the approved wildcard coverage.
* `RotationPolicyV1` contains the maximum key lifetime and the expected
  overlap window (default 7 days) so clients know when to expect staged RADs.

RADs must include both the operator and governance signatures. Operators
sign first, governance verifies the payload and co-signs, and only the
double-signed document is distributed.

### 2.2 On-chain directory record

The on-chain directory stores a Merkle tree root over the active RAD set.
Each leaf is the `blake3` hash of the full RAD (excluding signatures),
tagged with its version. The on-chain record is embedded in
`ResolverDirectoryRecordV1`:

```norito
struct ResolverDirectoryRecordV1 {
    root_hash: [u8; 32],
    record_version: u16,               // maps to RAD schema version
    published_at_block: u64,
    published_at_unix: u64,
    proof_manifest_cid: Cid,
}
```

The record is stored under the governance lane and exposed via Torii
(`GET /v2/soradns/directory/latest`). The endpoint returns a Norito JSON
object with a convenience `directory_id_hex` plus the embedded
`ResolverDirectoryRecordV1` (under `record`), allowing clients to verify the
Merkle root against the RAD they intend to trust and reject any RAD that is
not anchored in the ledger.

### 2.3 Distribution bundle

Every directory update produces a SoraFS CAR bundle:

```
car/
 ├─ directory.json          # Norito-encoded ResolverDirectoryRecordV1
 ├─ rad/<resolver_id>.norito
 └─ proofs/<resolver_id>.norito
```

`rad/*.norito` files contain the signed RADs. `proofs/*.norito` files contain
the Merkle branch linking each RAD to `root_hash`. The bundle is pinned via
`sorafs_pin_registry` and mirrored to the gateway publish set. Hashes from
the CAR manifest feed into the on-chain record for tamper detection.

### 2.4 Torii endpoints

- `GET /v2/soradns/directory/latest` — returns a Norito JSON document with
  `directory_id_hex` and the canonical `ResolverDirectoryRecordV1`, enabling
  gateways and tooling to bootstrap without decoding raw Norito bytes.
- `GET /v2/soradns/directory/events` — Server-Sent Events stream that emits
  the `ResolverDirectoryEventV1` variants as JSON objects with
  `event`/`payload` keys (hex digests, builder keys, revocation metadata, and
  rotation policy updates) so subscribers can keep caches in sync.

## 3. Client bootstrap

1. Fetch Torii `ResolverDirectoryRecordV1`.
2. Download the referenced SoraFS CAR (preferably via SoraNet transports).
3. Verify the CAR manifest against the directory record.
4. For each RAD:
   - validate operator + governance signatures;
   - confirm the RAD hash appears in the Merkle branch; and
   - ensure `valid_from_unix ≤ now < valid_until_unix`.
5. Cache the RAD in memory alongside the deterministic gateway hosts.
6. Use `resolve_gateway_hosts()` (Rust helper) to cross-check the
   `canonical_hosts` set before establishing HTTPS sessions.

Bootstrap helpers live in `tools/soradns-resolver` and will also be exposed
via the Rust/JS SDKs so CLI workflows can download + verify the directory
without re-implementing Norito handling.

## 4. Rotation & monitoring

* **Staging:** Operators upload the next RAD at least 7 days before
  `valid_until_unix`. Governance signs and publishes it alongside the active
  document so clients can prefetch.
* **Revocation:** Emergency revocations set `valid_until_unix = now` and
  push the revoked resolver ID onto the `resolver.revocations` Norito topic.
  Gateways must drop the resolver immediately.
* **Monitoring:** `dashboards/grafana/soradns_resolver_health.json` (new)
  will chart RAD freshness, signature verification failures, and rotation
  lead times. An accompanying Alertmanager file will page when:
  - a RAD expires without an overlapping replacement,
  - Merkle mismatch is detected, or
  - signature verification fails more than twice within an hour.

## 5. Pub/Sub schema

Resolver→gateway notifications ride over a shared SSE channel:

```norito
enum ResolverDirectoryEventV1 {
    DraftSubmitted { directory_id: [u8; 32], car_cid: Cid, builder_public_key: PublicKey },
    Published {
        directory_id: [u8; 32],
        previous_directory_id: Option<[u8; 32]>,
        directory_json_sha256: [u8; 32],
        car_cid: Cid,
        block_height: u64,
    },
    Revoked { resolver_id: [u8; 32], reason: RadRevokeReason, block_height: u64 },
    Unrevoked { resolver_id: [u8; 32], block_height: u64 },
    ReleaseSignerAdded { public_key: PublicKey },
    ReleaseSignerRemoved { public_key: PublicKey },
    PolicyUpdated { policy: DirectoryRotationPolicyV1 },
}
```

Torii streams these events over `/v2/soradns/directory/events` using SSE.
Gateways persist the last block height and resume streams on failure so
they never miss a publication, revocation, or policy change. Each event
includes the Merkle proof (when applicable) so clients can validate
updates before mutating local caches.

## 6. Implementation checkpoints

1. **Schema & tooling**
   - Add the Norito schema to `crates/iroha_data_model::soradns`.
   - Extend `tools/soradns-resolver` with `rad verify` and `directory fetch`
     subcommands plus integration tests.
2. **On-chain directory**
   - New `ResolverDirectory` smart contract (governance lane) with methods
     `publish_record`, `revoke_resolver`, and `set_pubsub_endpoint`.
   - Admission ensures signatures are valid and records are monotonic.
3. **Distribution pipeline**
   - Update `scripts/soradns_publish_directory.sh` to build the CAR bundle,
     pin it via `sorafs_cli`, and emit manifests for governance review.
4. **Observability**
   - Add OTEL metrics `soradns.rad.validity_window`, `soradns.rad.revocations`,
     and hook them into the Grafana dashboard + alert rules.
5. **Documentation & SDKs**
   - Reference this document from `docs/source/soradns/soradns_registry_rfc.md`.
   - Publish code samples in Rust/TS demonstrating the bootstrap helper
     once bindings land.

### CLI helpers

The resolver CLI now exposes tooling for RAD/directory verification:

- `soradns_resolver rad verify --rad <path> [--rad ...]` decodes Norito/JSON
  resolver attestations, enforces the structural constraints defined above,
  and prints the canonical SHA-256 digest (`rad-v1 || canonical_json`). The
  command accepts multiple files, making it easy to lint an entire RAD bundle
  before submitting it to governance.
- `soradns_resolver directory fetch --record-url <torii-endpoint> --directory-url <http(s)://...> --output <dir>`
  downloads the latest `ResolverDirectoryRecordV1` and matching `directory.json`,
  canonicalises the listing, verifies the recorded digest/count, and writes
  both artefacts to the supplied directory. Operators can point this at the
  Torii `/v2/soradns/directory/latest` endpoint plus the public SoraFS gateway to
  bootstrap a verified cache before enabling resolver clients.
- Both commands accept offline-friendly flags: `--record-file` and
  `--directory-file` let you validate artifacts that were fetched via other
  channels (for example, the signed release bundle checked into Git), which is
  especially useful in restricted environments or during incident response.

## 7. Acceptance criteria

* Every resolver reachable from production gateways has an active, signed RAD
  anchored in the directory Merkle root.
* Gateways refuse to use resolvers that lack a verifiable RAD.
* RAD rotation drills (quarterly) prove that overlapping publication works
  and that revocations propagate within the 5-minute SLO.
* Observability dashboards stay green: no expired RADs, no unsigned
  documents, no Merkle mismatches.
* Tooling (`tools/soradns-resolver`) can fetch + verify the directory in CI,
  providing reproducible evidence for audits.
