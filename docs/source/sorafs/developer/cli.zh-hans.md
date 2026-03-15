---
lang: zh-hans
direction: ltr
source: docs/source/sorafs/developer/cli.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eeb3aa604125d4940873e675a60b0169a95d5491bb422a7ffad165695ac44068
source_last_modified: "2026-01-22T16:26:46.591123+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS CLI Cookbook
summary: Task-oriented cookbook for the `sorafs_cli` command surface.
---

# CLI Cookbook

The consolidated `sorafs_cli` surface (crate `sorafs_car`, feature `cli`)
exposes every step required to prepare SoraFS artefacts. This page highlights
common workflows and links to deeper references.

## Package payloads

Use `car pack` to produce deterministic CAR archives and chunk plans. The
command automatically selects the SF-1 chunker unless a handle is provided.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Default chunker handle: `sorafs.sf1@1.0.0`.
- When dealing with directories, the CLI walks entries in lexicographic order so
  checksums remain stable across systems.
- The JSON summary includes payload digests, per-chunk metadata, and the root
  CID recognised by the registry and orchestrator.

## Construct manifests

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` options map directly to `PinPolicy` fields in
  `sorafs_manifest::ManifestBuilder`.
- Provide `--chunk-plan` when you want the CLI to recompute the SHA3 chunk
  digest before submission; otherwise it reuses the digest embedded in the
  summary.
- The JSON output mirrors the Norito payload for easier diffing in code reviews.

## Sign manifests without long-lived keys

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Accepts inline tokens, environment variables, or file-based sources.
- Adds provenance metadata (`token_source`, `token_hash_hex`, chunk digest)
  without persisting the raw JWT unless `--include-token=true`.
- Perfect for CI: combine with GitHub Actions OIDC by setting
  `--identity-token-provider=github-actions`.

## Submit manifests to Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority i105... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Performs Norito decoding for alias proofs and verifies they match the
  manifest digest before POSTing to Torii.
- Recomputes the chunk SHA3 digest from the plan to prevent mismatch attacks.
- Response summaries capture HTTP status, headers, and registry payloads for
  later auditing.

## Verify CAR contents and proofs

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Rebuilds the PoR tree and compares payload digests with the manifest summary.
- Captures counts and identifiers required when submitting replication proofs to
  governance.

## Stream proof telemetry

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json
```

- Emits NDJSON items for each streamed proof (replay disabled with
  `--emit-events=false`).
- Aggregates success/failure counts, latency histogram, and sampled failures in
  the summary JSON so dashboards can plot outcomes without scraping logs.
- Exits non-zero when the gateway reports any failures or when local PoR
  verification (triggered by `--por-root-hex`) rejects a proof. Override the
  budgets with `--max-failures` / `--max-verification-failures` when rehearsing
  flakier environments.
- Supports PoR today; PDP and PoTR reuse the same envelope once SF-13/SF-14 land.

## Additional references

- `docs/source/sorafs_cli.md` — exhaustive flag documentation.
- `docs/source/sorafs_proof_streaming.md` — proof telemetry schema and Grafana
  dashboard template.
- `docs/source/sorafs/manifest_pipeline.md` — deep dive on chunking, manifest
  composition, and CAR handling.
