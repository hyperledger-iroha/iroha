---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-ci.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور-ci
العنوان: إيصالات CI لـ SoraFS
Sidebar_label: إيصالات CI
الوصف: قم بتثبيت CLI SoraFS في GitHub وGitLab العاديين في قائمة خاصة.
---

:::note Канонический источник
:::

# الإيصالات CI

يتم التحقق من صفحتي SoraFS من تحديد القطع وتقديم البيان والتحقق من البراهين.
يقوم الأمر العلوي `sorafs_cli` بتنفيذ هذه التغييرات من خلال اختبار CI. هذه هي المنطقة
يدعم الوصايا الكنسية ويصرح بأهمية استخدام الدروس.

## إجراءات GitHub (بدون مفتاح)

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

لحظات رئيسية:

- لا تتأرجح المفاتيح الإحصائية؛ يتم إرسال الرموز المميزة OIDC إلى الطلب.
- القطع الأثرية (السيارة، البيان، الحزمة، البراهين المائية) محمية من أجل المراجعة.
- تستخدم الوظيفة مرة أخرى المخططات Norito، والتي يتم عرضها.

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

- أدخل `SIGSTORE_ID_TOKEN` من خلال اتحاد هوية عبء العمل GitLab أو سر مختوم من أجل النشر.
- كل ما تحبه هو CLI الذي يدعم خط الأنابيب، والقطع الأثرية المصاحبة.

## الموارد الإضافية

- عناصر شاملة (تتضمن مساعدات Bash وتكوينات الهوية الفيدرالية والتفاصيل): `docs/examples/sorafs_ci.md`
- واجهة سطر الأوامر (CLI) الصحيحة، التي تدعم جميع الخيارات: `docs/source/sorafs_cli.md`
- إدارة العمل/الاسم المستعار قبل التنفيذ:
  `docs/source/sorafs/provider_admission_policy.md`