# SoraFS Manifest Library

Utilities for building and serialising SoraFS manifests using the Norito
codec. Pair this crate with `sorafs_car` (for chunk planning) and
`sorafs_chunker` (raw chunking utilities) to derive chunk metadata, wrap CAR
commitments, and emit governance-ready manifest blobs.

## Usage

```rust
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    ManifestBuilder, DagCodecId, PinPolicy, StorageClass, BLAKE3_256_MULTIHASH_CODE,
};

let manifest = ManifestBuilder::new()
    .root_cid(vec![0x01, 0x55, 0xaa])
    .dag_codec(DagCodecId(0x71)) // dag-cbor
    .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
    .content_length(1_048_576)
    .car_digest([0x42; 32])
    .car_size(1_111_111)
    .pin_policy(PinPolicy {
        min_replicas: 3,
        storage_class: StorageClass::Hot,
        retention_epoch: 0,
    })
    .build()
    .expect("missing required fields");

let bytes = manifest.encode().expect("serialize manifest");
let digest = manifest.digest().expect("hash manifest");
```

### CLI helper

The `sorafs-manifest-stub` binary emits chunk metadata and a manifest stub for
the provided input. It accepts alias claims, governance signatures, metadata,
and can optionally write the encoded Norito payload to disk. The tool now emits
spec-compliant CARv2 archives and will compute both the CAR **payload** digest,
the full archive digest/size, and the raw CID for you (it verifies any values
you pass via `--car-digest`/`--car-size`/`--car-cid`).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub \
  ./docs.tar \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --min-replicas=3 \
  --storage-class=hot \
  --car-cid=0155deadbeef... \
  --alias-file=docs:sora:alias_proof.bin \
  --council-signature-file=0123...cafe:council.sig \
  --metadata=build:ci-123 \
  --manifest-out=docs.manifest \
  --car-out=docs.car
```

#### Useful flags

- `--alias=name:namespace:proofhex` or `--alias-file=name:namespace:path`  
  Embed alias claims with their Merkle proofs.
- `--council-signature=signerhex:signaturehex` or `--council-signature-file=signerhex:path`  
  Attach governance approvals for the manifest digest.
- `--metadata=key:value` to add arbitrary annotations (repeatable).
- `--chunker-profile-id=ID` or `--chunker-profile=namespace.name@semver` to select a
  registered chunker profile (mirrors `sorafs_manifest_chunk_store --list-profiles`).
- `--root-cid=hex` / `--dag-codec=0xNN` verify the computed CAR root and
  codec; omit them during normal operation.
- `--car-cid=hex` verifies the computed raw CID (CIDv1 + `raw` codec +
  BLAKE3 multihash) of the emitted CAR file.
- The JSON report includes both the payload digest (`car_payload_digest_hex`)
  and the full archive digest (`car_archive_digest_hex`); pass
  `--car-digest` to enforce the payload hash during CI.
- `--manifest-out=path` to persist the Norito payload; omit to only print the report.
- `--manifest-signatures-out=path` writes a `manifest_signatures.json` envelope
  that records the manifest BLAKE3 digest, the aggregated chunk plan digest
  (offsets, lengths, chunk BLAKE3 digests), and the supplied council signatures
  (Ed25519). The envelope emits the chunker handle in canonical
  `namespace.name@semver` form and includes the `namespace-name`
  alias under `profile_aliases`. Requires `--manifest-out` and
  at least one `--council-signature*` flag.
- `--manifest-signatures-in=path` verifies an existing signatures envelope against
  the freshly computed manifest digest and chunk plan digest, checking Ed25519 signatures
  when the signer key is valid. Combine with `--manifest-out` so the CLI can confirm
  the manifest filename advertised in the envelope.
- `--car-out=path` writes the CARv2 archive using `CarWriter`. The CLI always
  computes and prints the CAR size/digest; pass `--car-digest`/`--car-size` only
  if you need the tool to enforce pre-computed values.
- `--json-out=path` writes the JSON report to disk (exactly the same payload that
  is printed to stdout). Pass `--json-out=-` to stream the report directly to stdout
  without creating a temporary file.
- `--chunk-fetch-plan-out=path` emits the ordered chunk fetch specifications
  (index, offset, length, BLAKE3 digest) derived from the manifest’s CAR plan so
  orchestrators can schedule multi-source downloads without re-reading the payload.
- `--plan=chunk_fetch_specs.json|-` imports a previously generated chunk fetch plan
  (for example from CI or a dry-run) and verifies that every chunk’s index, offset,
  length, and digest still match the bytes on disk before proceeding.
- `--por-json-out=path` emits the Proof-of-Retrievability tree (chunks → segments →
  leaves) as JSON for sampling auditors.
- `--por-proof=chunk:segment:leaf` materialises a PoR proof for the requested leaf;
  combine with `--por-proof-out=path` to persist it and `--por-proof-verify=path`
  to validate an existing proof against the freshly generated tree.
- `--por-sample=count` (with optional `--por-sample-seed=value`) draws unique PoR
  leaf samples; `--por-sample-out=path` writes the JSON array of sampled proofs,
  and the report marks `por_samples_truncated` if the request exceeded the tree size.
- `--signing-key=hex` (provider helper) accepts a hex-encoded 32- or 64-byte
  Ed25519 key inlined on the command line.
- `--public-key-out=path` / `--signature-out=path` export the computed key and
  signature alongside the Norito advert.
- `--signing-key-file=path` (provider helper) accepts a 32-byte Ed25519 seed or
  64-byte expanded key, derives the public key, and signs the advert body for
  you. When using this flag, omit `--public-key`/`--signature`.

The tool prints a JSON report (chunk digests, manifest snapshot with alias/metadata
details, CAR root/codec, payload + archive digests, raw CAR CID) and
writes the Norito bytes to `docs.manifest` when `--manifest-out` is provided.
When `--car-out` is supplied, a CARv2 file (with MultihashIndexSorted index) is
written and its size and digest appear in the report. The same report now carries
`chunk_fetch_specs`: an ordered list of chunk indices/offsets/lengths/digests that
multi-source downloaders can feed directly into the SoraFS fetch orchestrator.

To inspect the registered chunker profiles (and their IDs), run:

```
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- --list-chunker-profiles
```

### Provider advert helper

`sorafs-provider-advert-stub` assembles a `ProviderAdvertV1` payload for
storage nodes. It validates TTLs, QoS parameters, path-diversity policies, and
signatures before writing the Norito bytes and a JSON summary. You can either
provide an existing signature/public key or let the CLI sign with an Ed25519
secret key via `--signing-key-file` (seed/expanded key) or `--signing-key=hex`.
Example:

```bash
cargo run -p sorafs_manifest --bin sorafs-provider-advert-stub -- \
  --emit \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --provider-id=001122... \
  --stake-pool-id=ffeedd... \
  --stake-amount=5000000 \
  --availability=hot \
  --max-latency-ms=1500 \
  --max-streams=32 \
  --capability=torii \
  --capability=quic \
  --capability=range:64 \
  --endpoint=torii:storage.example.com \
  --endpoint-meta=region:global \
  --topic=sorafs.sf1.primary:global \
  --signing-key=deadbeef... \
  --public-key-out=provider.pub \
  --signature-out=provider.sig \
  --advert-out=provider.advert \
  --json-out=provider.report.json

# You can still pass `--profile-id=<alias>`, but prefer the canonical handle
# (`namespace.name@semver`) so automation stays aligned with the shared registry.
```

The `range` capability advertises support for ranged chunk requests. Supplying
an optional numeric suffix (for example, `--capability=range:64`) encodes the
provider's preferred concurrent range-fetch budget in little-endian form so
downstream fetchers can tune multi-source scheduling.

The JSON report mirrors the validated fields (stake, capabilities, endpoints,
rendezvous topics, signature metadata), stores the Norito bytes as hex, and
sets `signature_verified=true` when the signature check succeeds. Pass
`--verify --advert=<path>` to validate an existing advert; the command checks
expiry, path/QoS guard rails, and ed25519 signatures before printing the same
JSON payload. When signing, you may persist the derived public key and signature
via `--public-key-out` / `--signature-out`. Use this tool in CI to refresh or
audit provider adverts alongside manifest updates.
