---
lang: ba
direction: ltr
source: docs/source/sorafs/chunker_registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 48ab7fd78ac9dfe11fd8dfeaaeaa830df2410a9a5e71234f05688298404a8e37
source_last_modified: "2026-01-22T14:35:37.713916+00:00"
translation_last_reviewed: 2026-02-07
---

## SoraFS Chunker Profile Registry (SF-2a)

The SoraFS stack negotiates chunking behaviour via a small, namespaced registry.
Each profile assigns deterministic CDC parameters, semver metadata, and the
expected digest/multicodec used in manifests and CAR archives.

Profile authors should consult
[`docs/source/sorafs/chunker_profile_authoring.md`](chunker_profile_authoring.md)
for the required metadata, validation checklist, and proposal template before
submitting new entries. Once governance has approved a change, follow the
[registry rollout checklist](chunker_registry_rollout_checklist.md) and the
[staging manifest playbook](runbooks/staging_manifest_playbook.md) to promote
the fixtures through staging and production.

### Profiles

| Namespace | Name | SemVer | Profile ID | Min (bytes) | Target (bytes) | Max (bytes) | Break mask | Multihash | Aliases | Notes |
|-----------|------|--------|------------|-------------|----------------|-------------|------------|-----------|---------|-------|
| `sorafs`  | `sf1` | `1.0.0` | `1` | 65 536 | 262 144 | 524 288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | Canonical profile used in SF-1 fixtures |

The registry lives in code as `sorafs_manifest::chunker_registry` (governed by [`chunker_registry_charter.md`](chunker_registry_charter.md)). Each entry
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
unknown IDs appear. Registry charter rules require the canonical handle
(`namespace.name@semver`) to be the first entry in `profile_aliases`.

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
Accept-Chunker: sorafs.sf1;version=1.0.0
```

Gateways select a mutually supported profile (defaulting to `sorafs.sf1@1.0.0`)
and reflect the decision via the `Content-Chunker` response header. Manifests
embed the chosen profile so downstream nodes can validate the chunk layout
without relying on HTTP negotiation.

### Conformance

* The `sorafs.sf1@1.0.0` profile maps to the public fixtures in
  `fixtures/sorafs_chunker` and the corpora registered under
  `fuzz/sorafs_chunker`. End-to-end parity is exercised in Rust, Go, and Node
  via the provided tests.
* `chunker_registry::lookup_by_profile` asserts that the descriptor parameters
  match `ChunkProfile::DEFAULT` to guard accidental divergence.
* Manifests produced by `iroha app sorafs toolkit pack` and `sorafs_manifest_stub` include the registry metadata.
