# SoraFS Manifest Library

Utilities for building and serialising SoraFS manifests using the Norito
codec. Pair this crate with `sorafs_car` (for chunk planning) and
`sorafs_chunker` (raw chunking utilities) to derive chunk metadata, wrap CAR
commitments, embed the ordered chunk-plan SHA3-256 digest, and emit
governance-ready manifest blobs.

## Usage

```rust
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    ManifestBuilder, DagCodecId, PinPolicy, StorageClass, BLAKE3_256_MULTIHASH_CODE,
};

let manifest = ManifestBuilder::new()
    .root_cid(sorafs_manifest::canonical_manifest_root_cid([0xAA; 32]))
    .dag_codec(DagCodecId(0x71)) // dag-cbor
    .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
    .chunk_digest_sha3_256([0x41; 32]) // SHA3-256 of ordered chunk metadata
    .content_length(1_048_576)
    .car_digest([0x42; 32])
    .car_size(1_111_111)
    .pin_policy(PinPolicy {
        min_replicas: 3,
        storage_class: StorageClass::Hot,
        retention_epoch: 42,
    })
    .build()
    .expect("missing required fields");

let bytes = manifest.encode().expect("serialize manifest");
let digest = manifest.digest().expect("hash manifest");
```

### CLI helper

The `sorafs_manifest_builder` binary emits chunk metadata and a manifest for
the provided input. It accepts alias claims, governance signatures, metadata,
and can optionally write the encoded Norito payload to disk. The tool now emits
spec-compliant CARv2 archives and will compute both the CAR **payload** digest,
the full archive digest/size, and the raw CID for you (it verifies any values
you pass via `--car-digest`/`--car-size`/`--car-cid`).

```bash
cargo run -p sorafs_car --bin sorafs_manifest_builder \
  ./docs.tar \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --min-replicas=3 \
  --storage-class=hot \
  --retention-epoch=42 \
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
- `--retention-epoch=EPOCH` is mandatory for production manifests and must be
  positive; zero is not an unlimited-retention shortcut.
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
  Only the canonical first-release Norito layout is accepted; there is no legacy
  encoding mode.
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
- `sorafs_provider_advert --prepare` accepts only a raw 32-byte public-key file
  plus its reviewed SHA-256 fingerprint and writes the exact canonical payload
  for an external Ed25519/HSM signer.
- `sorafs_provider_advert --emit` requires that reviewed signing-payload file
  plus the exact raw public key and raw 64-byte external signature. The
  production tool has no private-key or inline key/signature option.

The tool prints a JSON report (chunk digests, manifest snapshot with alias/metadata
details, CAR root/codec, payload + archive digests, raw CAR CID) and
writes the Norito bytes to `docs.manifest` when `--manifest-out` is provided.
When `--car-out` is supplied, a CARv2 file (with MultihashIndexSorted index) is
written and its size and digest appear in the report. The same report now carries
`chunk_fetch_specs`: an ordered list of chunk indices/offsets/lengths/digests that
multi-source downloaders can feed directly into the SoraFS fetch orchestrator.

To inspect the registered chunker profiles (and their IDs), run:

```
cargo run -p sorafs_car --bin sorafs_manifest_builder -- --list-chunker-profiles
```

### Provider advert production builder

`sorafs_provider_advert` assembles a `ProviderAdvertV1` payload for storage
nodes. It validates TTLs, QoS parameters, path-diversity policies, the exact
reviewed signer key/fingerprint, and an external Ed25519 signature before
writing Norito bytes. The two-phase handoff keeps all private keys outside the
process:

```bash
advert_args=(
  --chunker-profile=sorafs.sf1@1.0.0
  --provider-id=001122...
  --stake-pool-id=ffeedd...
  --stake-amount=5000000
  --availability=hot
  --max-latency-ms=1500
  --max-streams=32
  --capability=torii
  --capability=quic
  --capability=range:64
  --endpoint=torii:storage.example.com
  --endpoint-meta=region:global
  --topic=sorafs.sf1.primary:global
  --issued-at=1700000000
)

cargo run -p sorafs_car --bin sorafs_provider_advert -- \
  --prepare "${advert_args[@]}" \
  --public-key-file=provider.pub \
  --public-key-fingerprint-sha256="$REVIEWED_PROVIDER_KEY_SHA256" \
  --signing-payload-out=provider-advert.signing-payload

# Sign provider-advert.signing-payload with the governed external HSM.

cargo run -p sorafs_car --bin sorafs_provider_advert -- \
  --emit "${advert_args[@]}" \
  --public-key-file=provider.pub \
  --public-key-fingerprint-sha256="$REVIEWED_PROVIDER_KEY_SHA256" \
  --signing-payload-file=provider-advert.signing-payload \
  --signature-file=provider.sig \
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
`--verify --advert=<path> --public-key-file=<path>
--public-key-fingerprint-sha256=<reviewed-hex>` to validate an existing advert.
The command rejects symlinks, hard links, unsafe permissions, path replacement,
malformed raw material, and signer/fingerprint mismatches before printing the
same JSON payload.
