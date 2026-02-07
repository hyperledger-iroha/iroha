---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-05T09:28:11.908615+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Quickstart

This hands-on guide walks through the deterministic SF-1 chunker profile,
manifest signing, and multi-provider fetch flow that underpin the SoraFS
storage pipeline. Pair it with the [manifest pipeline deep dive](manifest-pipeline.md)
for design notes and CLI flag reference material.

## Prerequisites

- Rust toolchain (`rustup update`), workspace cloned locally.
- Optional: [OpenSSL-generated Ed25519 keypair](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  for signing manifests.
- Optional: Node.js ≥ 18 if you plan to preview the Docusaurus portal.

Set `export RUST_LOG=info` while experimenting to surface helpful CLI messages.

## 1. Refresh the deterministic fixtures

Regenerate the canonical SF-1 chunking vectors. The command also emits signed
manifest envelopes when `--signing-key` is supplied; use `--allow-unsigned`
during local development only.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Outputs:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (if signed)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Chunk a payload and inspect the plan

Use `sorafs_chunker` to chunk an arbitrary file or archive:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Key fields:

- `profile` / `break_mask` – confirms the `sorafs.sf1@1.0.0` parameters.
- `chunks[]` – ordered offsets, lengths, and chunk BLAKE3 digests.

For larger fixtures, run the proptest-backed regression to ensure streaming and
batch chunking stay in sync:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Build and sign a manifest

Wrap the chunk plan, aliases, and governance signatures into a manifest using
`sorafs-manifest-stub`. The command below showcases a single-file payload; pass
a directory path to package a tree (the CLI walks it lexicographically).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Review `/tmp/docs.report.json` for:

- `chunking.chunk_digest_sha3_256` – SHA3 digest of offsets/lengths, matches the
  chunker fixtures.
- `manifest.manifest_blake3` – BLAKE3 digest signed in the manifest envelope.
- `chunk_fetch_specs[]` – ordered fetch instructions for orchestrators.

When ready to supply real signatures, add `--signing-key` and `--signer`
arguments. The command verifies every Ed25519 signature before writing the
envelope.

## 4. Simulate multi-provider retrieval

Use the developer fetch CLI to replay the chunk plan against one or more
providers. This is ideal for CI smoke tests and orchestrator prototyping.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Assertions:

- `payload_digest_hex` must match the manifest report.
- `provider_reports[]` surfaces success/failure counts per provider.
- Non-zero `chunk_retry_total` highlights back-pressure adjustments.
- Pass `--max-peers=<n>` to limit the number of providers scheduled for a run
  and keep CI simulations focused on the primary candidates.
- `--retry-budget=<n>` overrides the default per-chunk retry count (3) so you
  can surface orchestrator regressions faster when injecting failures.

Add `--expect-payload-digest=<hex>` and `--expect-payload-len=<bytes>` to fail
fast when the reconstructed payload deviates from the manifest.

## 5. Next steps

- **Governance integration** – pipe the manifest digest and
  `manifest_signatures.json` into the council workflow so the Pin Registry can
  advertise availability.
- **Registry negotiation** – consult [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  before registering new profiles. Automation should prefer canonical handles
  (`namespace.name@semver`) over numeric IDs.
- **CI automation** – add the commands above to release pipelines so docs,
  fixtures, and artifacts publish deterministic manifests alongside signed
  metadata.
