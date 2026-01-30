---
lang: fr
direction: ltr
source: docs/source/examples/sorafs_manifest/cli_end_to_end.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8209e602132efb6c29962bf09aea8cd74f972fa956ea8a7a1dbac08a7f6f00f
source_last_modified: "2026-01-04T10:50:53.612671+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: "SoraFS Manifest CLI End-to-End Example"
---

# SoraFS Manifest CLI End-to-End Example

This example walks through publishing a documentation build to SoraFS using the
`sorafs_manifest_stub` CLI together with the deterministic chunking fixtures
described in the SoraFS Architecture RFC. The flow covers manifest generation,
expectation checks, fetch-plan validation, and proof-of-retrieval rehearsal so
teams can embed the same steps in CI.

## Prerequisites

- Workspace cloned and toolchain ready (`cargo`, `rustc`).
- Fixtures from `fixtures/sorafs_chunker` available so expectation values can be
  derived (for production runs, pull the values from the migration ledger entry
  associated with the artifact).
- Sample payload directory to publish (this example uses `docs/book`).

## Step 1 — Generate manifest, CAR, signatures, and fetch plan

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-out target/sorafs/docs.manifest_signatures.json \
  --car-out target/sorafs/docs.car \
  --chunk-fetch-plan-out target/sorafs/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101d0cfa9be459f4a4ba4da51990b75aef262ef546270db0e42d37728755d \
  --dag-codec=0x71 \
  --chunker-profile=sorafs.sf1@1.0.0
```

The command:

- Streams the payload through `ChunkProfile::DEFAULT`.
- Emits a CARv2 archive plus chunk-fetch plan.
- Builds a `ManifestV1` record, verifies manifest signatures (if provided), and
  writes the envelope.
- Enforces expectation flags so the run fails if bytes drift.

## Step 2 — Verify outputs with chunk store + PoR rehearsal

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

This replays the CAR through the deterministic chunk store, derives the
Proof-of-Retrievability sampling tree, and emits a manifest report suitable for
governance review.

## Step 3 — Simulate multi-provider retrieval

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

For CI environments, provide separate payload paths per provider (e.g., mounted
fixtures) to exercise range scheduling and failure handling.

## Step 4 — Record ledger entry

Log the publication in `docs/source/sorafs/migration_ledger.md`, capturing:

- Manifest CID, CAR digest, and council signature hash.
- Status (`Draft`, `Staging`, `Pinned`).
- Links to CI runs or governance tickets.

## Step 5 — Pin via governance tooling (when registry is live)

Once the Pin Registry is deployed (Milestone M2 in the migration roadmap),
submit the manifest through the CLI:

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --plan=target/sorafs/docs.fetch_plan.json \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-in target/sorafs/docs.manifest_signatures.json \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --council-signature-file <signer_hex>:path/to/signature.bin

cargo run -p sorafs_cli --bin sorafs_pin -- propose \
  --manifest target/sorafs/docs.manifest \
  --manifest-signatures target/sorafs/docs.manifest_signatures.json
```

The proposal identifier and subsequent approval transaction hashes should be
captured in the migration ledger entry for auditability.

## Cleanup

Artifacts under `target/sorafs/` can be archived or uploaded to staging nodes.
Keep the manifest, signatures, CAR, and fetch plan together so downstream
operators and SDK teams can validate the deployment deterministically.
