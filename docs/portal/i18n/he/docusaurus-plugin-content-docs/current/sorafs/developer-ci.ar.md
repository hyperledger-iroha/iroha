---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-ci.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: developer-ci
title: وصفات CI لـ SoraFS
sidebar_label: وصفات CI
description: شغّل واجهة SoraFS CLI داخل خطوط أنابيب GitHub وGitLab مع توقيع دون مفاتيح.
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/developer/ci.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف الوثائق القديمة.
:::

# ותק CI

تستفيد خطوط أنابيب SoraFS من التجزئة الحتمية، وتوقيع المانيفست، والتحقق من الأدلة. يحافظ سطح أوامر
`sorafs_cli` على قابلية نقل تلك الخطوات بين مزودي CI. تسلط هذه الصفحة الضوء على الوصفات المعتمدة وتشير
إلى قوالب جاهزة للاستخدام.

## GitHub Actions (بدون مفاتيح)

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

نقاط أساسية:

- لا تُخزَّن مفاتيح توقيع ثابتة؛ تُجلب رموز OIDC عند الطلب.
- تُرفع الآرتيفاكتات (CAR، المانيفست، الحزمة، ملخصات الأدلة) للمراجعة.
- تعيد المهمة استخدام مخططات Norito نفسها المستخدمة في عمليات الإطلاق الإنتاجية.

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

- زوّد `SIGSTORE_ID_TOKEN` عبر اتحاد هوية workload في GitLab أو سر مختوم قبل تنفيذ مرحلة النشر.
- يؤدي فشل أي خطوة في CLI إلى إيقاف خط الأنابيب، مع الحفاظ على آرتيفاكتات متسقة.

## موارد إضافية

- قوالب end-to-end (تتضمن مساعدات Bash، وإعداد هوية اتحادية، وخطوات تنظيف): `docs/examples/sorafs_ci.md`
- مرجع CLI الذي يغطي كل الخيارات: `docs/source/sorafs_cli.md`
- מידע כללי:
  `docs/source/sorafs/provider_admission_policy.md`