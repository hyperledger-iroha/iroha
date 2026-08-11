---
title: CI Recipes
summary: Use the SoraFS CLI in CI and hand release artifacts to governed Ed25519 signing.
---

> Public and in-depth documentation is maintained in the sibling
> `iroha-docs` repository and published at <https://docs.iroha.tech/>.

# CI Recipes

SoraFS pipelines benefit from deterministic chunking, manifest construction,
and proof verification. The `sorafs_cli` command surface keeps those steps
portable across CI providers. Release authenticity is a separate aggregate
manifest step backed by `signing_provider=authenticated_external_signer` with
exact `signing_backend=software`; verified output is
`signer_qualification=software-key-qualified`.

## GitHub Actions

```yaml
name: sorafs-artifacts

on:
  push:
    branches: [ main ]

jobs:
  build-and-publish:
    runs-on: ubuntu-latest
    permissions:
      contents: read
    env:
      RUSTFLAGS: "-C target-cpu=native"
    steps:
      - uses: actions/checkout@<reviewed-full-sha>
      - uses: actions-rs/toolchain@<reviewed-full-sha>
        with:
          profile: minimal
          toolchain: stable
      - name: Build CLI
        run: cargo install --path crates/sorafs_orchestrator --bin sorafs_cli --debug
      - name: Pack payload and manifest
        run: |
          sorafs_cli car pack \
            --input=fixtures/site.tar.gz \
            --car-out=artifacts/site.car \
            --plan-out=artifacts/site.plan.json \
            --summary-out=artifacts/site.car.json
          sorafs_cli manifest build \
            --summary=artifacts/site.car.json \
            --manifest-out=artifacts/site.manifest.to
      - name: Submit manifest
        env:
          TORII_URL: https://gateway.example/v2
          IROHA_NETWORK_ID: ${{ vars.IROHA_NETWORK_ID }}
          IROHA_PRIVATE_KEY: ${{ secrets.IROHA_PRIVATE_KEY }}
        run: |
          sorafs_cli manifest submit \
            --manifest=artifacts/site.manifest.to \
            --chunk-plan=artifacts/site.plan.json \
            --torii-url="$TORII_URL" \
            --network-id="$IROHA_NETWORK_ID" \
            --authority=<i105-account-id> \
            --private-key="$IROHA_PRIVATE_KEY" \
            --summary-out=artifacts/site.submit.json
      - name: Stream PoR proofs
        env:
          GATEWAY_URL: https://gateway.example/v1/sorafs/proof/stream
          STREAM_TOKEN: ${{ secrets.SORAFS_STREAM_TOKEN }}
          PROVIDER_ID_HEX: ${{ vars.SORAFS_PROVIDER_ID_HEX }}
        run: |
          sorafs_cli proof stream \
            --manifest=artifacts/site.manifest.to \
            --gateway-url="$GATEWAY_URL" \
            --provider-id-hex="$PROVIDER_ID_HEX" \
            --samples=64 \
            --stream-token="$STREAM_TOKEN" \
            --summary-out=artifacts/site.proof_stream.json
      - uses: actions/upload-artifact@<reviewed-full-sha>
        with:
          name: sorafs-artifacts
          path: artifacts/
```

Key points:

- Release signing keys are not exposed to this build-and-submit job.
- Artefacts (CAR, content manifest, and proof summaries) are uploaded for review.
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
    - cargo install --path crates/sorafs_orchestrator --bin sorafs_cli --debug
    - sorafs_cli car pack --input=fixtures/site.tar.gz --car-out=artifacts/site.car --plan-out=artifacts/site.plan.json --summary-out=artifacts/site.car.json
    - sorafs_cli manifest build --summary=artifacts/site.car.json --manifest-out=artifacts/site.manifest.to
  artifacts:
    paths:
      - artifacts/

sorafs:publish:
  stage: publish
  needs: ["sorafs:build"]
  image: rust:1.81
  script:
    - sorafs_cli manifest submit --manifest=artifacts/site.manifest.to --chunk-plan=artifacts/site.plan.json --torii-url="$TORII_URL" --network-id="$IROHA_NETWORK_ID" --authority=<i105-account-id> --private-key="$IROHA_PRIVATE_KEY" --summary-out=artifacts/site.submit.json
    - sorafs_cli proof verify --manifest=artifacts/site.manifest.to --car=artifacts/site.car --chunk-plan=artifacts/site.plan.json --summary-out=artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- Failure of any CLI step causes the pipeline to halt, preserving consistent
  artefacts.
- `IROHA_NETWORK_ID` must be the exact identity derived from the deployment's
  expected genesis-header hash. The submit command signs one native
  `RegisterPinManifest` transaction for that network; lifecycle event epochs
  are derived by consensus and have no CI override flag.

## Release authenticity job

After the release pipeline generates canonical `release_manifest.json`, run the
governed signing adapter on a protected runner:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/release/release_manifest.json \
  --external-signer /run/sorafs-release/ed25519-sign \
  --signing-public-key /run/sorafs-release/release.ed25519.pub \
  --trusted-signing-fingerprint "$REVIEWED_SIGNER_SHA256" \
  --release-manifest-verifier /opt/iroha/bin/sorafs-validate \
  --trusted-release-manifest-verifier-sha256 "$REVIEWED_VERIFIER_SHA256"
```

The external signer is an authenticated adapter to the isolated software
signing service. The wrapper rejects missing inputs and verifies the exact raw
signature and key with the pinned native validator. OIDC/cosign attestations can
be added for provenance, but are not a substitute for this authentication step.
A future HSM adapter requires new HSM-backed evidence.

## Additional resources

- End-to-end templates (includes Bash helpers, federated identity configuration,
  and clean-up steps): `fixtures/documentation/sorafs_ci.md`
- CLI reference covering every option: `specs/sorafs_cli.md`
- Governance/alias requirements prior to submission:
  `specs/sorafs/provider_admission_policy.md`
