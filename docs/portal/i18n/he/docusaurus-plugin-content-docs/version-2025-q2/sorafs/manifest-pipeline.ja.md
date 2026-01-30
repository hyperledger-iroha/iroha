---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/version-2025-q2/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff1b3b8632f684db3bdb4072f9b138413ae09f79ac5e19d884887a3039e6b7ca
source_last_modified: "2026-01-30T17:50:55+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS Chunking → Manifest Pipeline

This companion to the quickstart traces the end-to-end pipeline that turns raw
bytes into Norito manifests suitable for the SoraFS Pin Registry. The content is
adapted from [`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
consult that document for the canonical specification and changelog.

## 1. Chunk deterministically

SoraFS uses the SF-1 (`sorafs.sf1@1.0.0`) profile: a FastCDC-inspired rolling
hash with a 64 KiB minimum chunk size, 256 KiB target, 512 KiB maximum, and a
`0x0000ffff` break mask. The profile is registered in
`sorafs_manifest::chunker_registry`.

### Rust helpers

- `sorafs_car::CarBuildPlan::single_file` – Emits chunk offsets, lengths, and
  BLAKE3 digests while preparing CAR metadata.
- `sorafs_car::ChunkStore` – Streams payloads, persists chunk metadata, and
  derives the 64 KiB / 4 KiB Proof-of-Retrievability (PoR) sampling tree.
- `sorafs_chunker::chunk_bytes_with_digests` – Library helper behind both CLIs.

### CLI tooling

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

The JSON contains the ordered offsets, lengths, and chunk digests. Persist the
plan when constructing manifests or orchestrator fetch specifications.

### PoR witnesses

`ChunkStore` exposes `--por-proof=<chunk>:<segment>:<leaf>` and
`--por-sample=<count>` so auditors can request deterministic witness sets. Pair
those flags with `--por-proof-out` or `--por-sample-out` to record the JSON.

## 2. Wrap a manifest

`ManifestBuilder` combines chunk metadata with governance attachments:

- Root CID (dag-cbor) and CAR commitments.
- Alias proofs and provider capability claims.
- Council signatures and optional metadata (e.g., build IDs).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Important outputs:

- `payload.manifest` – Norito-encoded manifest bytes.
- `payload.report.json` – Human/automation readable summary, including
  `chunk_fetch_specs`, `payload_digest_hex`, CAR digests, and alias metadata.
- `payload.manifest_signatures.json` – Envelope containing manifest BLAKE3
  digest, chunk-plan SHA3 digest, and sorted Ed25519 signatures.

Use `--manifest-signatures-in` to verify envelopes supplied by external
signatories before writing them back out, and `--chunker-profile-id` or
`--chunker-profile=<handle>` to lock the registry selection.

## 3. Publish and pin

1. **Governance submission** – Provide the manifest digest and signature
   envelope to the council so the pin can be admitted. External auditors should
   store the chunk-plan SHA3 digest alongside the manifest digest.
2. **Pin payloads** – Upload the CAR archive (and optional CAR index) referenced
   in the manifest to the Pin Registry. Ensure the manifest and CAR share the
   same root CID.
3. **Record telemetry** – Persist the JSON report, PoR witnesses, and any fetch
   metrics in release artifacts. These records feed operator dashboards and
   help reproduce issues without downloading large payloads.

## 4. Multi-provider fetch simulation

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` increases per-provider parallelism (`#4` above).
- `@<weight>` tunes scheduling bias; defaults to 1.
- `--max-peers=<n>` caps the number of providers scheduled for a run when
  discovery yields more candidates than desired.
- `--expect-payload-digest` and `--expect-payload-len` guard against silent
  corruption.
- `--provider-advert=name=advert.to` verifies provider capabilities before
  using them in the simulation.
- `--retry-budget=<n>` overrides the per-chunk retry count (default: 3) so CI
  can surface regressions faster when testing failure scenarios.

`fetch_report.json` surfaces aggregated metrics (`chunk_retry_total`,
`provider_failure_rate`, etc.) suitable for CI assertions and observability.

## 5. Registry updates & governance

When proposing new chunker profiles:

1. Author the descriptor in `sorafs_manifest::chunker_registry_data`.
2. Update `docs/source/sorafs/chunker_registry.md` and related charters.
3. Regenerate fixtures (`export_vectors`) and capture signed manifests.
4. Submit the charter compliance report with governance signatures.

Automation should prefer canonical handles (`namespace.name@semver`) and fall
