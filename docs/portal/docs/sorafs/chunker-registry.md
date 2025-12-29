---
id: chunker-registry
title: SoraFS Chunker Profile Registry
sidebar_label: Chunker Registry
description: Profile IDs, parameters, and negotiation plan for the SoraFS chunker registry.
---

:::note Canonical Source
Mirrors `docs/source/sorafs/chunker_registry.md`. Keep both copies in sync until the legacy Sphinx documentation set is retired.
:::

## SoraFS Chunker Profile Registry (SF-2a)

The SoraFS stack negotiates chunking behaviour via a small, namespaced registry.
Each profile assigns deterministic CDC parameters, semver metadata, and the
expected digest/multicodec used in manifests and CAR archives.

Profile authors should consult
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
for the required metadata, validation checklist, and proposal template before
submitting new entries. Once governance has approved a change, follow the
[registry rollout checklist](./chunker-registry-rollout-checklist.md) and the
[staging manifest playbook](./staging-manifest-playbook) to promote
the fixtures through staging and production.

### Profiles

| Namespace | Name | SemVer | Profile ID | Min (bytes) | Target (bytes) | Max (bytes) | Break mask | Multihash | Aliases | Notes |
|-----------|------|--------|------------|-------------|----------------|-------------|------------|-----------|---------|-------|
| `sorafs`  | `sf1` | `1.0.0` | `1` | 65 536 | 262 144 | 524 288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs-sf1"]` | Canonical profile used in SF-1 fixtures |

The registry lives in code as `sorafs_manifest::chunker_registry` (governed by [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Each entry
is expressed as a `ChunkerProfileDescriptor` with:

* `namespace` – logical grouping of related profiles (e.g., `sorafs`).
* `name` – human-readable profile label (`sf1`, `sf1-fast`, …).
* `semver` – semantic version string for the parameter set.
* `profile` – the actual `ChunkProfile` (min/target/max/mask).
* `multihash_code` – the multihash used when producing chunk digests (`0x1f`
  for the SoraFS default).

The manifest serializes profiles via `ChunkingProfileV1`. The structure records
the registry metadata (namespace, name, semver) alongside the raw CDC
parameters and the alias list shown above. Consumers should first attempt a
registry lookup by `profile_id` and fall back to the inline parameters when
unknown IDs appear; the alias list ensures HTTP clients can continue sending
legacy handles in `Accept-Chunker` without guessing. Registry charter rules
require the canonical handle (`namespace.name@semver`) to be the first entry in
`profile_aliases`, followed by any legacy aliases.

To inspect the registry from tooling, run the helper CLI:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]

All of the CLI flags that write JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) accept `-` as the path, which streams the payload to stdout instead of
creating a file. This makes it easy to pipe the data into tooling while still keeping the
default behaviour of printing the main report.

### Compatibility Matrix & Rollout Plan

> Machine-readable matrix: `docs/source/sorafs/compatibility_matrix.json`  
> Regenerate via `cargo xtask sorafs-compat` (use `--out -` for stdout).

The table below captures the current support status for `sorafs.sf1@1.0.0` across
core components. "Bridge" refers to the CARv1 + SHA-256 compatibility lane that
requires explicit client negotiation (`Accept-Chunker` + `Accept-Digest`).

| Component | Status | Notes |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Supported | Validates canonical handle + aliases, streams reports via `--json-out=-`, and enforces registry charter via `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ Legacy | Legacy manifest builder; use `iroha sorafs toolkit pack` for CAR/manifest packaging and keep `--plan=-` for deterministic revalidation. |
| `sorafs_provider_advert_stub` | ⚠️ Legacy | Offline validation helper only; provider adverts should be produced by the publishing pipeline and validated via `/v1/sorafs/providers`. |
| `sorafs_fetch` (developer orchestrator) | ✅ Supported | Reads `chunk_fetch_specs`, understands `range` capability payloads, and assembles CARv2 output. |
| SDK fixtures (Rust/Go/TS) | ✅ Supported | Regenerated via `export_vectors`; canonical handle appears first in every alias list and is signed by council envelopes. |
| Torii gateway profile negotiation | ✅ Supported | Implements full `Accept-Chunker` grammar, includes `Content-Chunker` headers, and exposes CARv1 bridge only on explicit downgrade requests. |
| CARv1 bridge (`sha2-256`) | ⚠️ Transitional | Available for legacy clients when the request advertises both the canonical profile and `Accept-Digest: sha2-256`; responses include `Content-Chunker: ...;legacy=true`. |

Telemetry roll-out:

- **Chunk fetch telemetry** – Iroha CLI `sorafs toolkit pack` emits chunk digests, CAR metadata, and PoR roots for ingestion into dashboards.
- **Provider adverts** – Advert payloads include capability and alias metadata; validate coverage via `/v1/sorafs/providers` (e.g., presence of `range` capability).
- **Gateway monitoring** – operators should report `Content-Chunker`/`Content-Digest` pairings to detect unexpected downgrades; bridge usage is expected to trend to zero before deprecation.

Deprecation policy: once a successor profile is ratified, schedule a dual-publish
window (documented in the proposal) before marking `sorafs.sf1@1.0.0` as
deprecated in the registry and removing the CARv1 bridge from production gateways.

To inspect a specific PoR witness, provide chunk/segment/leaf indices and
optionally persist the proof to disk:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

You can select a profile by numeric id (`--profile-id=1`) or by registry handle
(`--profile=sorafs.sf1@1.0.0`); the handle form is convenient for scripts that
thread namespace/name/semver directly from governance metadata.

Use `--promote-profile=<handle>` to emit a JSON metadata block (including all
registered aliases) that can be pasted into `chunker_registry_data.rs` when
promoting a new default profile:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

The main report (and optional proof file) include the root digest, the sampled
leaf bytes (hex-encoded), and the segment/chunk sibling digests so verifiers can
rehash the 64 KiB/4 KiB layers against the `por_root_hex` value.

To validate an existing proof against a payload, pass the path via
`--por-proof-verify` (the CLI adds `"por_proof_verified": true` when the witness
matches the computed root):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

For batch sampling, use `--por-sample=<count>` and optionally provide a seed/
output path. The CLI guarantees deterministic ordering (`splitmix64` seeded)
and will transparently truncate when the request exceeds the available leaves:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

The manifest stub mirrors the same data, which is convenient when scripting `--chunker-profile-id` selection in pipelines. Both chunk store CLIs also accept the canonical handle form (`--profile=sorafs.sf1@1.0.0`) so build scripts can avoid hard-coding numeric IDs:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

The `handle` field (`namespace.name@semver`) matches what the CLIs accept via
`--profile=…`, making it safe to copy directly into automation.

### Negotiating Chunkers

Gateways and clients advertise supported profiles via provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

Multi-source chunk scheduling is announced via the `range` capability. The
CLI accepts it with `--capability=range[:streams]`, where the optional numeric
suffix encodes the provider's preferred range-fetch concurrency (for example,
`--capability=range:64` advertises a 64-stream budget). When omitted, consumers
fall back to the general `max_streams` hint published elsewhere in the advert.

When requesting CAR data, clients should send an `Accept-Chunker` header listing
supported `(namespace, name, semver)` tuples in preference order:

```
Accept-Chunker: sorafs.sf1;version=1.0.0, legacy.fastcdc;version=0.9.0
```

Gateways select a mutually supported profile (defaulting to `sorafs.sf1@1.0.0`)
and reflect the decision via the `Content-Chunker` response header. Manifests
embed the chosen profile so downstream nodes can validate the chunk layout
without relying on HTTP negotiation.

### CAR Compatibility

The canonical manifest envelope uses CIDv1 roots with `dag-cbor` (`0x71`). For
legacy compatibility we retain a CARv1+SHA-2 export path:

* **Primary path** – CARv2, BLAKE3 payload digest (`0x1f` multihash),
  `MultihashIndexSorted`, chunk profile recorded as above.
* **Legacy bridge** – CARv1, SHA-256 payload digest (`0x12` multihash). Servers
  MAY surface this variant when the client omits `Accept-Chunker` or requests
  `Accept-Digest: sha2-256`.

Manifests always advertise the CARv2/BLAKE3 commitment. Legacy lanes provide
additional headers for compatibility but must not replace the canonical digest.

### Conformance

* The `sorafs-sf1` profile maps to the public fixtures in
  `fixtures/sorafs_chunker` and the corpora registered under
  `fuzz/sorafs_chunker`. End-to-end parity is exercised in Rust, Go, and Node
  via the provided tests.
* `chunker_registry::lookup_by_profile` asserts that the descriptor parameters
  match `ChunkProfile::DEFAULT` to guard accidental divergence.
* Manifests produced by `iroha sorafs toolkit pack` and `sorafs_manifest_stub` include the registry metadata.
