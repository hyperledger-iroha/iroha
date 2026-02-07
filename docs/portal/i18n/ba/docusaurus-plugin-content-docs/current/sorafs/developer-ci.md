---
id: developer-ci
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/developer-ci.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS CI Recipes
sidebar_label: CI Recipes
description: Run the SoraFS CLI inside GitHub and GitLab pipelines with keyless signing.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

# CI Рецептар

I18NT000000002X торбалары детерминистик өлөштәр, асыҡ ҡул ҡуйыу һәм
иҫбатлаусы раҫлау. I18NI000000007X командаһы өҫтө шул аҙымдарҙы портатив тота
CI провайдерҙары буйынса. Был биттә канон рецепттары айырып күрһәтә һәм
әҙер шаблондар.

## GitHub ғәмәлдәре (классһыҙ)

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

Төп балл:

- Бер ниндәй ҙә статик ҡул ҡуйыу асҡыстары һаҡланмай; OIDC токендары ихтыяж буйынса алынған.
- Артфакттар (CAR, асыҡ, өйөм, иҫбатлау резюмеһы) тикшерелгән өсөн тейәлгән.
- Эш етештереүҙе таратыуҙа ҡулланылған шул уҡ I18NT000000001X схемаларын ҡабаттан файҙалана.

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

- I18NI000000008X аша GitLab’s эш йөкләмәһе федерацияһы йәки а
  нәшриәтте баҫтырып сығарыр алдынан герметизацияланған сер.
- теләһә ниндәй CLI аҙымы етешмәүе торба үткәргес туҡтатыуға килтерә, эҙмә-эҙлекле һаҡлау
  артефакттары.

## Өҫтәмә ресурстар

- осона тиклем шаблондар (Баш ярҙамсыларын үҙ эсенә ала, федерацияланған шәхес конфигурацияһы,
  һәм таҙартыу аҙымдары): `docs/examples/sorafs_ci.md` X
- CLI һылтанмаһы һәр вариантты ҡаплай: `docs/source/sorafs_cli.md`
- Идара итеү/псевдоним талаптары тапшырыу алдынан:
  `docs/source/sorafs/provider_admission_policy.md`