---
id: developer-ci
lang: kk
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

:::ескерту Канондық дереккөз
:::

# CI рецепттері

SoraFS конвейерлері детерминирленген түйіндеу, манифестке қол қою және
дәлелді тексеру. `sorafs_cli` пәрмен беті сол қадамдарды портативті сақтайды
CI провайдерлері бойынша. Бұл бетте канондық рецепттер бөлектеліп, оларға назар аударылады
пайдалануға дайын шаблондар.

## GitHub әрекеттері (кілтсіз)

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
          TORII_URL: https://gateway.example/v2
          IROHA_PRIVATE_KEY: ${{ secrets.IROHA_PRIVATE_KEY }}
        run: |
          sorafs_cli manifest submit \
            --manifest artifacts/site.manifest.to \
            --chunk-plan artifacts/site.plan.json \
            --torii-url "$TORII_URL" \
            --authority <katakana-i105-account-id> \
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

Негізгі нүктелер:

- Статикалық қол қою кілттері сақталмайды; OIDC таңбалауыштары сұраныс бойынша алынады.
- Артефактілер (CAR, манифест, бума, дәлелдемелердің қорытындылары) шолу үшін жүктеледі.
- Тапсырма өндірісті шығаруда пайдаланылатын бірдей Norito схемаларын қайта пайдаланады.

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
    - sorafs_cli manifest submit --manifest artifacts/site.manifest.to --chunk-plan artifacts/site.plan.json --torii-url "$TORII_URL" --authority <katakana-i105-account-id> --private-key "$IROHA_PRIVATE_KEY" --summary-out artifacts/site.submit.json
    - sorafs_cli proof verify --manifest artifacts/site.manifest.to --car artifacts/site.car --summary-out artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- GitLab жұмыс жүктемесінің сәйкестендіру федерациясы арқылы `SIGSTORE_ID_TOKEN` қамтамасыз ету немесе
  жариялау кезеңін орындау алдында мөрленген құпия.
- Кез келген CLI қадамының орындалмауы құбырдың тұрақтылығын сақтай отырып тоқтауына әкеледі
  артефактілер.

## Қосымша ресурстар

- Үздік үлгілер (Bash көмекшілерін, федеративті идентификациялық конфигурацияны,
  және тазалау қадамдары): `docs/examples/sorafs_ci.md`
- Әрбір опцияны қамтитын CLI анықтамасы: `docs/source/sorafs_cli.md`
- Тапсырыс беру алдында басқаруға/бүркеншік атқа қойылатын талаптар:
  `docs/source/sorafs/provider_admission_policy.md`