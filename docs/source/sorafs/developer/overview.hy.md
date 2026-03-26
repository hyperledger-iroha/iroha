---
lang: hy
direction: ltr
source: docs/source/sorafs/developer/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3120369292f8fc80e5f212b653bbe01904915702e2786b0d02e1003fdb36a7ab
source_last_modified: "2026-01-22T16:26:46.591306+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Quickstart
summary: Walk through the minimal steps to package, submit, and validate SoraFS content.
---

# Quickstart

Developers can exercise the full SoraFS flow using the consolidated `sorafs_cli`
binary. The outline below mirrors the production pipeline while remaining
friendly to local experiments and CI smoke tests.

## 1. Install tooling

```bash
cargo install --path crates/sorafs_car --features cli --bin sorafs_cli
```

Optional helpers:

- `cargo run -p sorafs_car --bin sorafs_fetch -- --help` — deterministic
  orchestrator simulator for multi-provider fetches.
- `cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- --help` — manifest
  fixture generator used in examples and tests.

## 2. Compile Kotodama and pack a CAR archive

```bash
# Compile Kotodama bytecode and capture a reproducible summary
sorafs_cli norito build \
  --source contracts/register_domain.ko \
  --bytecode-out artifacts/register_domain.to \
  --summary-out artifacts/register_domain.summary.json

# Produce a CAR archive plus chunk plan for deterministic pinning
sorafs_cli car pack \
  --input fixtures/site.tar.gz \
  --chunker-handle sorafs.sf1@1.0.0 \
  --car-out artifacts/site.car \
  --plan-out artifacts/site.chunk_plan.json \
  --summary-out artifacts/site.car.json
```

The CLI writes pretty JSON summaries by default and mirrors the same payload
to `--summary-out` when supplied. Downstream CI can diff those summaries or feed
them into the manifest builder without parsing CAR bytes directly.

## 3. Build, sign, and submit a manifest

```bash
# Build a manifest from the CAR summary and chunk plan
sorafs_cli manifest build \
  --summary artifacts/site.car.json \
  --chunk-plan artifacts/site.chunk_plan.json \
  --pin-min-replicas 3 \
  --pin-storage-class warm \
  --manifest-out artifacts/site.manifest.to \
  --manifest-json-out artifacts/site.manifest.json

# Mint a Sigstore-backed signature bundle (OIDC token read from SIGSTORE_ID_TOKEN)
sorafs_cli manifest sign \
  --manifest artifacts/site.manifest.to \
  --bundle-out artifacts/site.manifest.bundle.json \
  --signature-out artifacts/site.manifest.sig

# Submit the manifest to a Torii gateway
sorafs_cli manifest submit \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --torii-url https://gateway.local:8080 \
  --authority <katakana-i105-account-id> \
  --private-key ed25519:0123...cafe \
  --summary-out artifacts/site.submit.summary.json
```

See `docs/source/sorafs_cli.md` for a detailed breakdown of flags, chunker
handles, and signature handling.

## 4. Stream and verify proofs

```bash
# Fetch PoR samples from a gateway and emit aggregated metrics
sorafs_cli proof stream \
  --manifest artifacts/site.manifest.to \
  --gateway-url https://gateway.local/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 64 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/site.proof_stream.json

# Rebuild the PoR store locally to ensure CAR contents match the manifest
sorafs_cli proof verify \
  --manifest artifacts/site.manifest.to \
  --car artifacts/site.car \
  --summary-out artifacts/site.proof_verify.json
```

The streaming summary exposes success/failure counts, latency quantiles, and a
sample of failing items. Use those values to power dashboards or CI thresholds.
Refer to `docs/source/sorafs_proof_streaming.md` for the schema and dashboard
templates.

## 5. Adopt the pipeline in CI/CD

- GitHub/GitLab recipes: `docs/examples/sorafs_ci.md`
- CI-focused CLI notes: `docs/source/sorafs_ci_templates.md`

Both guides demonstrate keyless signing, manifest submission, proof verification,
and artefact archiving. Extend the example workflows with environment-specific
steps (stream token provisioning, alias proofs, governance approvals) as needed.

## 6. Production considerations

After the basics work end-to-end:

- Apply the deployment checklist in `docs/source/sorafs/developer/deployment.md`.
- Enforce capability and policy checks using the operator guides under
  `docs/source/sorafs/runbooks/`.
- Enable telemetry exporters (`torii_sorafs_*` metrics) so observability matches
  SF-7 expectations.

Following this checklist yields reproducible manifests, signed artefacts, and
proof telemetry backed by the same Norito schemas used in production rollouts.
