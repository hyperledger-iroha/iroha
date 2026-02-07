---
lang: hy
direction: ltr
source: docs/source/sorafs/developer/ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 430dcd36bcdb49f1e6f432ee58a2527346a64be5ce5254c15e1133d577103c68
source_last_modified: "2026-01-22T16:26:46.590975+00:00"
translation_last_reviewed: 2026-02-07
title: CI Recipes
summary: Use the SoraFS CLI inside GitHub and GitLab pipelines with keyless signing.
---

> **Portal:** Mirrored in `docs/portal/docs/sorafs/developer-ci.md`.
> Update both copies to keep reviewers aligned.

# CI Recipes

SoraFS pipelines benefit from deterministic chunking, manifest signing, and
proof verification. The `sorafs_cli` command surface keeps those steps portable
across CI providers. This page highlights the canonical recipes and points to
ready-to-use templates.

## GitHub Actions (keyless)

```yaml
name: sorafs-artifacts

on:
  push:
    branches: [ main ]

jobs:
  build-and-publish:
    runs-on: ubuntu-latest
    permissions:
      id-token: write
      contents: read
    env:
      RUSTFLAGS: "-C target-cpu=native"
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          profile: minimal
          toolchain: stable
      - name: Build CLI
        run: cargo install --path crates/sorafs_car --features cli --bin sorafs_cli --debug
      - name: Pack payload and manifest
        run: |
          sorafs_cli car pack \
            --input fixtures/site.tar.gz \
            --car-out artifacts/site.car \
            --plan-out artifacts/site.plan.json \
            --summary-out artifacts/site.car.json
          sorafs_cli manifest build \
            --summary artifacts/site.car.json \
            --chunk-plan artifacts/site.plan.json \
            --manifest-out artifacts/site.manifest.to
      - name: Sign manifest (Sigstore OIDC)
        run: |
          sorafs_cli manifest sign \
            --manifest artifacts/site.manifest.to \
            --bundle-out artifacts/site.manifest.bundle.json \
            --signature-out artifacts/site.manifest.sig \
            --identity-token-provider=github-actions
      - name: Submit manifest
        env:
          TORII_URL: https://gateway.example/v1
          IROHA_PRIVATE_KEY: ${{ secrets.IROHA_PRIVATE_KEY }}
        run: |
          sorafs_cli manifest submit \
            --manifest artifacts/site.manifest.to \
            --chunk-plan artifacts/site.plan.json \
            --torii-url "$TORII_URL" \
            --authority ih58... \
            --private-key "$IROHA_PRIVATE_KEY" \
            --summary-out artifacts/site.submit.json
      - name: Stream PoR proofs
        env:
          GATEWAY_URL: https://gateway.example/v1/sorafs/proof/stream
          STREAM_TOKEN: ${{ secrets.SORAFS_STREAM_TOKEN }}
        run: |
          sorafs_cli proof stream \
            --manifest artifacts/site.manifest.to \
            --gateway-url "$GATEWAY_URL" \
            --provider-id provider::alpha \
            --samples 64 \
            --stream-token "$STREAM_TOKEN" \
            --summary-out artifacts/site.proof_stream.json
      - uses: actions/upload-artifact@v4
        with:
          name: sorafs-artifacts
          path: artifacts/
```

Key points:

- No static signing keys are stored; OIDC tokens are fetched on-demand.
- Artefacts (CAR, manifest, bundle, proof summaries) are uploaded for review.
- The job reuses the same Norito schemas used in production rollouts.

## GitLab CI

```yaml
stages:
  - build
  - publish

variables:
  RUSTFLAGS: "-C target-cpu=native"

sorafs:build:
  stage: build
  image: rust:1.81
  script:
    - cargo install --path crates/sorafs_car --features cli --bin sorafs_cli --debug
    - sorafs_cli car pack --input fixtures/site.tar.gz --car-out artifacts/site.car --plan-out artifacts/site.plan.json --summary-out artifacts/site.car.json
    - sorafs_cli manifest build --summary artifacts/site.car.json --chunk-plan artifacts/site.plan.json --manifest-out artifacts/site.manifest.to
  artifacts:
    paths:
      - artifacts/

sorafs:publish:
  stage: publish
  needs: ["sorafs:build"]
  image: rust:1.81
  script:
    - sorafs_cli manifest sign --manifest artifacts/site.manifest.to --bundle-out artifacts/site.manifest.bundle.json --signature-out artifacts/site.manifest.sig --identity-token-env SIGSTORE_ID_TOKEN
    - sorafs_cli manifest submit --manifest artifacts/site.manifest.to --chunk-plan artifacts/site.plan.json --torii-url "$TORII_URL" --authority ih58... --private-key "$IROHA_PRIVATE_KEY" --summary-out artifacts/site.submit.json
    - sorafs_cli proof verify --manifest artifacts/site.manifest.to --car artifacts/site.car --summary-out artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- Provision `SIGSTORE_ID_TOKEN` via GitLab’s workload identity federation or a
  sealed secret before executing the publish stage.
- Failure of any CLI step causes the pipeline to halt, preserving consistent
  artefacts.

## Additional resources

- End-to-end templates (includes Bash helpers, federated identity configuration,
  and clean-up steps): `docs/examples/sorafs_ci.md`
- CLI reference covering every option: `docs/source/sorafs_cli.md`
- Governance/alias requirements prior to submission:
  `docs/source/sorafs/provider_admission_policy.md`
