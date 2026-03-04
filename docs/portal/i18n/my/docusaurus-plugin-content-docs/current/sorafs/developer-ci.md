---
id: developer-ci
lang: my
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

::: Canonical Source ကို သတိပြုပါ။
:::

# CI ချက်ပြုတ်နည်းများ

SoraFS ပိုက်လိုင်းများသည် အဆုံးအဖြတ်ဖြတ်တောက်ခြင်း၊ ထင်ရှားစွာ လက်မှတ်ရေးထိုးခြင်း၊ နှင့်
အထောက်အထားစိစစ်ခြင်း။ `sorafs_cli` command မျက်နှာပြင်သည် ထိုခြေလှမ်းများကို သယ်ဆောင်ရလွယ်ကူစေသည်။
CI ဝန်ဆောင်မှုပေးသူများ ဤစာမျက်နှာသည် Canonical ချက်ပြုတ်နည်းများကို မီးမောင်းထိုးပြပြီး ညွှန်ပြထားသည်။
အဆင်သင့်သုံးနိုင်သော ပုံစံများ။

## GitHub လုပ်ဆောင်ချက်များ (သော့မရှိ)

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

အဓိကအချက်များ-

- တည်ငြိမ်သောလက်မှတ်ထိုးသော့များကိုသိမ်းဆည်းထားခြင်းမရှိပါ။ OIDC တိုကင်များကို လိုအပ်သလောက် ရယူထားပါသည်။
- Artefacts (CAR၊ manifest၊ bundle၊ proof summaries) ကို ပြန်လည်သုံးသပ်ရန်အတွက် အပ်လုဒ်လုပ်ထားပါသည်။
- အလုပ်သည် ထုတ်လုပ်မှုစတင်ခြင်းများတွင်အသုံးပြုသည့်တူညီသော Norito အစီအစဉ်များကို ပြန်လည်အသုံးပြုသည်။

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

- GitLab ၏ workload အထောက်အထား အဖွဲ့ချုပ် သို့မဟုတ် တစ်ခုမှတစ်ဆင့် ပံ့ပိုးမှု `SIGSTORE_ID_TOKEN`
  ထုတ်ဝေခြင်းအဆင့်ကို မလုပ်ဆောင်မီ လျှို့ဝှက်ပိတ်သိမ်းထားသည်။
- မည်သည့် CLI အဆင့်မှ ပျက်ကွက်ပါက ပိုက်လိုင်းကို ရပ်တန့်စေပြီး တသမတ်တည်း ထိန်းသိမ်းထားသည်။
  ရှေးဟောင်းပစ္စည်း။

## နောက်ထပ်အရင်းအမြစ်များ

- အဆုံးမှအဆုံး နမူနာပုံစံများ (Bash ကူညီပေးသူများ၊ ဖက်ဒရယ်အထောက်အထားဖွဲ့စည်းပုံ၊
  နှင့် ရှင်းလင်းရေး အဆင့်များ): `docs/examples/sorafs_ci.md`
- ရွေးချယ်မှုတိုင်းအတွက် CLI ရည်ညွှန်းချက်- `docs/source/sorafs_cli.md`
- တင်ပြခြင်းမပြုမီ အုပ်ချုပ်မှု/အမည်နာမ လိုအပ်ချက်များ-
  `docs/source/sorafs/provider_admission_policy.md`