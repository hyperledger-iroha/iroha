---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-ci.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور-ci
العنوان: Recetas de CI de SoraFS
Sidebar_label: Recetas de CI
الوصف: قم بتشغيل CLI من SoraFS عبر خطوط الأنابيب من GitHub وGitLab بفواصل ثابتة.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/developer/ci.md`. حافظ على الإصدارات المتزامنة حتى يتم سحب المستندات المتوارثة.
:::

# وصفات CI

تستفيد خطوط الأنابيب SoraFS من تحديد القطع وشركة البيانات والبيانات
التحقق من الأدلة. يحافظ سطح أوامر `sorafs_cli` على هذه الخطوة
المنقولات بين مزودي CI. تعرض هذه الصفحة الوصفات الأساسية والأساسية
قوائم النباتات للاستخدام.

## إجراءات جيثب (عصا الخطيئة)

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

بونتوس العصا:

- لا يوجد مخزون رئيسي من الإحصائيات؛ يتم الحصول على الرموز المميزة OIDC عند الطلب.
- تخضع المصنوعات اليدوية (CAR، والبيان، والحزمة، وخلاصات البراهين) للمراجعة.
- تقوم الوظيفة بإعادة استخدام نفس الاسم Norito المستخدم في عمليات الإنتاج الأولية.

## جيتلاب سي آي

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

- توفير `SIGSTORE_ID_TOKEN` من خلال توحيد معرف عبء عمل GitLab أو واحد
  تم إغلاقه بشكل سري قبل تنفيذ مرحلة النشر.
- سقوط أي خطوة من CLI حتى يتم إغلاق خط الأنابيب والحفاظ عليه
  القطع الأثرية تتسق.

## الموارد الإضافية- نباتات شاملة (تتضمن مساعدين Bash وتكوين الهوية الفيدرالية و
  باسوس دي ليمبيزا): `docs/examples/sorafs_ci.md`
- مرجع سطر الأوامر مع جميع الخيارات: `docs/source/sorafs_cli.md`
- متطلبات الإدارة/الاسم المستعار قبل الشحن:
  `docs/source/sorafs/provider_admission_policy.md`