---
lang: ja
direction: ltr
source: docs/source/sorafs/chunker_registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 48ab7fd78ac9dfe11fd8dfeaaeaa830df2410a9a5e71234f05688298404a8e37
source_last_modified: "2026-01-22T15:38:30.691198+00:00"
translation_last_reviewed: 2026-01-30
---

## SoraFS Chunker Profile Registry (SF-2a)

The SoraFS stack negotiates chunking behavior through the small registry in
`crates/sorafs_manifest/src/chunker_registry.rs`. Each entry binds a numeric
`ProfileId`, namespace/name/semver metadata, deterministic CDC parameters,
multihash settings, and the accepted handle aliases used by manifests, provider
adverts, CLIs, gateways, and SDKs.

Profile authors should consult
[`docs/source/sorafs/chunker_profile_authoring.md`](chunker_profile_authoring.md)
for the required metadata, validation checklist, and proposal template before
submitting new entries. After governance approval, follow the
[registry rollout checklist](chunker_registry_rollout_checklist.md) and the
[staging manifest playbook](runbooks/staging_manifest_playbook.md) to promote the
fixtures through staging and production.

### Profiles

| Namespace | Name | SemVer | Profile ID | Min | Target | Max | Break mask | Multihash | Aliases | Notes |
|-----------|------|--------|------------|-----|--------|-----|------------|-----------|---------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65,536 | 262,144 | 524,288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `sorafs.sf1@1.0.0`, `sorafs-sf1` | Default SF-1 profile and fixture baseline. |
| `sorafs` | `sf2` | `1.0.0` | `2` | 32,768 | 131,072 | 393,216 | `0x00007fff` | `0x1f` (BLAKE3-256) | `sorafs.sf2@1.0.0`, `sorafs-sf2` | High-density SF-2 profile with smaller target chunks. |

`ChunkingProfileV1` serializes both the numeric `profile_id` and the inline CDC
parameters. Consumers must first resolve the `profile_id` through
`sorafs_manifest::chunker_registry`; current manifest validation rejects unknown
registered IDs instead of guessing layout from inline parameters. Registry
charter rules require the canonical handle (`namespace.name@semver`) to be the
first alias.

### CLI Inspection

List all registered descriptors:

```bash
cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- --list-profiles
```

Select profiles by numeric id or canonical handle:

```bash
cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- ./docs.tar \
  --profile-id=1 --json-out=-

cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- ./docs.tar \
  --profile=sorafs.sf2@1.0.0 --json-out=-
```

The manifest stub exposes the same registry data for pipeline scripts:

```bash
cargo run -p sorafs_car --bin sorafs_manifest_stub -- --list-chunker-profiles
```

Use `--promote-profile=<handle>` to emit the metadata block that reviewers paste
into `crates/sorafs_manifest/src/chunker_registry.rs` when promoting a new
profile:

```bash
cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf2@1.0.0 --json-out=-
```

All JSON-writing flags (`--json-out`, `--por-json-out`, `--por-proof-out`, and
`--por-sample-out`) accept `-` to stream to stdout. This keeps governance review
logs reproducible without creating temporary files.

### PoR Witness Checks

Inspect a specific PoR witness by chunk, segment, and leaf indices:

```bash
cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- ./docs.tar \
  --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Validate an existing proof against a payload:

```bash
cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- ./docs.tar \
  --por-proof-verify=leaf.proof.json --json-out=-
```

For batch sampling, use `--por-sample=<count>` with an optional deterministic
seed and output path:

```bash
cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- ./docs.tar \
  --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```

The main report and optional proof files include the root digest, sampled leaf
bytes, and segment/chunk sibling digests so verifiers can rehash the layers
against `por_root_hex`.

### Negotiating Chunkers

Provider adverts publish the canonical handle in `profile_id` and include the
same aliases that appear in the registry. Gateway/client negotiation should use
canonical handles such as `sorafs.sf1@1.0.0` or `sorafs.sf2@1.0.0`; numeric IDs
remain the compact manifest representation.

When requesting CAR data, clients can list supported profiles in preference
order, and gateways should reflect the selected profile in response metadata.
Manifests embed the selected registry descriptor so downstream nodes validate the
chunk layout without relying only on HTTP negotiation.

### Conformance

- `chunker_registry::ensure_charter_compliance()` validates positive,
  monotonically increasing IDs, first-position canonical aliases, duplicate
  canonical handles, duplicate aliases, alias/canonical collisions, and trimmed
  non-empty aliases.
- `chunker_registry::lookup_by_profile` maps CDC parameters plus multihash code
  back to a descriptor and guards accidental divergence.
- `sorafs_manifest_stub` and `sorafs_manifest_chunk_store` include the registry
  metadata in their JSON output so release and governance scripts can compare
  descriptors without hard-coded numeric IDs.
- Public fixture parity for the registered profiles is maintained under
  `fixtures/sorafs_chunker` and exercised by Rust plus companion SDK fixtures.
