---
lang: dz
direction: ltr
source: docs/portal/docs/sorafs/developer-ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a4a42040dfcc033d9e4476eb5a93a7806b8812db94a3fe47e91ec38f18e1ba7
source_last_modified: "2026-01-22T16:26:46.521619+00:00"
translation_last_reviewed: 2026-02-07
id: developer-ci
title: SoraFS CI Recipes
sidebar_label: CI Recipes
description: Run the SoraFS CLI inside GitHub and GitLab pipelines with keyless signing.
translator: machine-google-reviewed
---

:::དྲན་ཐོའི་འབྱུང་ཁུངས།
:::

# CI བླངས།

SoraFS ཆུ་མཛོད་འདི་ གཏན་འབེབས་ཀྱི་ཆ་ཤས་དང་ གསལ་སྟོན་མིང་རྟགས་བཀོད་ནི་ལས་ ཁེ་ཕན་ཡོདཔ་ཨིན།
བདེན་ཁུངས་བདེན་དཔྱད། I18NI000000007X བརྡ་བཀོད་ཁ་ཐོག་འདི་གིས་ གོམ་པ་དེ་ཚུ་ འབག་བཏུབ་སྦེ་བཞགཔ་ཨིན།
CI བྱིན་མི་ཚུ་ལུ། ཤོག་ལེབ་འདི་གིས་ ཀེ་ནོ་ནིཀ་བཟོ་ཐངས་ཚུ་དང་ ས་ཚིགས་ཚུ་ ལུ་ གསལ་སྟོན་འབདཝ་ཨིན།
གྲ་སྒྲིག་-ལག་ལེན་ཊེམ་པེལེཊི་ཚུ།

## གིཏ་ཧབ་བྱ་བ་ (ལྡེ་མིག་མེད་པ།)

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

གཙོ་བོའི་དོན་ཚན་ཚུ།

- རྟག་བརྟན་མིང་རྟགས་བཀོད་ནིའི་ལྡེ་མིག་ཚུ་ གསོག་འཇོག་འབད་མི་བཏུབ། OIDC ཊོ་ཀེན་ཚུ་ དགོ་འདོད་ཐོག་ལུ་ ལེན་ཡོདཔ་ཨིན།
- བསྐྱར་ཞིབ་འབད་ནིའི་དོན་ལུ་ ཅ་རྙིང་ (CAR, གསལ་སྟོན་, བང་རིམ་, སྒྲུབ་བྱེད་བཅུད་བསྡུས་) ཚུ་ སྐྱེལ་བཙུགས་འབདཝ་ཨིན།
- ལཱ་གཡོག་འདི་གིས་ ཐོན་སྐྱེད་བསྐོར་བའི་ནང་ལུ་ལག་ལེན་འཐབ་མི་ Norito གི་ལས་རིམ་ཚུ་ ལོག་ལག་ལེན་འཐབ་ཨིན།

## གིཏ་ལབ་སིའི།

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

- GitLab གི་ལཱ་གི་འབོར་ཚད་ངོ་རྟགས་ཚོགས་སྡེ་བརྒྱུད་དེ་ དགོངས་དོན་ `SIGSTORE_ID_TOKEN`
  དཔེ་སྐྲུན་གྱི་གནས་རིམ་འདི་ ལག་ལེན་འཐབ་པའི་ཧེ་མ་ གསང་བ་འབད་ཡོདཔ།
- སི་ཨེལ་ཨའི་ གོ་རིམ་གང་རུང་ཅིག་གིས་ འཐུས་ཤོར་བྱུང་མི་འདི་གིས་ རིམ་མཐུན་ཉམས་སྲུང་འབདཝ་ཨིན།
  ཅ་རྙིང་།

## ཐོན་ཁུངས་ཁ་སྐོང་།

- མཇུག་ལས་མཇུག་ཚུན་ཚོད་ ཊེམ་པེལེཊི་ཚུ་ (བཱཤ་གྲོགས་རམ་པ་ སྤྱི་མཐུན་ངོ་རྟགས་རིམ་སྒྲིག་ཚུ་ཚུདཔ་ཨིན།
  དང་ གཙང་མ་གི་གོམ་པ་): I18NI0000009X
- གདམ་ཁ་ག་ར་ཁྱབ་པའི་ CLI གཞི་བསྟུན་འབད་མི་ CLI: `docs/source/sorafs_cli.md`
- ཕུལ་མ་ཚར་བའི་ཧེ་མ་ གཞུང་སྐྱོང་/མིང་གཞན་དགོས་མཁོ་ཚུ་།
  `docs/source/sorafs/provider_admission_policy.md`