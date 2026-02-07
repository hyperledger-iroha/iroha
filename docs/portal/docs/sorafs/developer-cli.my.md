---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c9acf8a8d9c298ad2fe95e5480942441aa790346f90c5b5e1a8c1ff638c5e73
source_last_modified: "2026-01-22T16:26:46.522695+00:00"
translation_last_reviewed: 2026-02-07
id: developer-cli
title: SoraFS CLI Cookbook
sidebar_label: CLI Cookbook
description: Task-focused walkthrough of the consolidated `sorafs_cli` surface.
---

:::note Canonical Source
:::

The consolidated `sorafs_cli` surface (provided by the `sorafs_car` crate with
the `cli` feature enabled) exposes every step required to prepare SoraFS
artifacts. Use this cookbook to jump directly to common workflows; pair it with
the manifest pipeline and orchestrator runbooks for operational context.

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
- Directory inputs are walked in lexicographic order so checksums stay stable
  across platforms.
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
- The JSON output mirrors the Norito payload for straightforward diffs during
  reviews.

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
- Works well in CI: combine with GitHub Actions OIDC by setting
  `--identity-token-provider=github-actions`.

## Submit manifests to Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority ih58... \
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
- Captures counts and identifiers required when submitting replication proofs
  to governance.

## Stream proof telemetry

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Emits NDJSON items for each streamed proof (disable replay with
  `--emit-events=false`).
- Aggregates success/failure counts, latency histograms, and sampled failures in
  the summary JSON so dashboards can plot outcomes without scraping logs.
- Exits non-zero when the gateway reports failures or local PoR verification
  (via `--por-root-hex`) rejects proofs. Adjust the thresholds with
  `--max-failures` and `--max-verification-failures` for rehearsal runs.
- Supports PoR today; PDP and PoTR reuse the same envelope once SF-13/SF-14
  land.
- `--governance-evidence-dir` writes the rendered summary, metadata (timestamp,
  CLI version, gateway URL, manifest digest), and a copy of the manifest into
  the supplied directory so governance packets can archive the proof-stream
  evidence without replaying the run.

## Additional references

- `docs/source/sorafs_cli.md` — exhaustive flag documentation.
- `docs/source/sorafs_proof_streaming.md` — proof telemetry schema and Grafana
  dashboard template.
- `docs/source/sorafs/manifest_pipeline.md` — deep dive on chunking, manifest
  composition, and CAR handling.
