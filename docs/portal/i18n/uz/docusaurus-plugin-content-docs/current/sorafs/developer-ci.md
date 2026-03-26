---
id: developer-ci
lang: uz
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

::: Eslatma Kanonik manba
:::

# CI retseptlari

SoraFS quvurlari deterministik qismlarga ajratish, manifest imzolash va
dalillarni tekshirish. `sorafs_cli` buyruq yuzasi bu qadamlarni ko'chma ushlab turadi
CI provayderlari bo'ylab. Ushbu sahifa kanonik retseptlarni ta'kidlaydi va ularga ishora qiladi
foydalanishga tayyor shablonlar.

## GitHub amallari (kalitsiz)

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
            --authority soraカタカナ... \
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

Asosiy fikrlar:

- Statik imzo kalitlari saqlanmaydi; OIDC tokenlari talab boʻyicha olinadi.
- Artefaktlar (CAR, manifest, to'plam, isbot xulosalari) ko'rib chiqish uchun yuklanadi.
- Ish ishlab chiqarishni yo'lga qo'yishda ishlatiladigan Norito sxemalarini qayta ishlatadi.

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
    - sorafs_cli manifest submit --manifest artifacts/site.manifest.to --chunk-plan artifacts/site.plan.json --torii-url "$TORII_URL" --authority soraカタカナ... --private-key "$IROHA_PRIVATE_KEY" --summary-out artifacts/site.submit.json
    - sorafs_cli proof verify --manifest artifacts/site.manifest.to --car artifacts/site.car --summary-out artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- `SIGSTORE_ID_TOKEN`-ni GitLabning ish yukini aniqlash federatsiyasi yoki
  nashr qilish bosqichini amalga oshirishdan oldin muhrlangan sir.
- Har qanday CLI qadamining bajarilmasligi quvur liniyasining to'xtab qolishiga olib keladi va barqarorlikni saqlaydi
  artefaktlar.

## Qo'shimcha manbalar

- End-to-end shablonlari (o'z ichiga Bash yordamchilari, federatsiyalangan identifikatsiya konfiguratsiyasi,
  va tozalash bosqichlari): `docs/examples/sorafs_ci.md`
- Har bir variantni qamrab oluvchi CLI ma'lumotnomasi: `docs/source/sorafs_cli.md`
- Taqdim etishdan oldin boshqaruv/taxallus talablari:
  `docs/source/sorafs/provider_admission_policy.md`