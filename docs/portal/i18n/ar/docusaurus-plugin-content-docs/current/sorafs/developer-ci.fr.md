---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-ci.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور-ci
العنوان: Recettes CI SoraFS
Sidebar_label: سجلات CI
الوصف: قم بتنفيذ CLI SoraFS داخل خطوط الأنابيب GitHub وGitLab بتوقيع بدون مفتاح.
---

:::ملاحظة المصدر الكنسي
:::

# يعيد CI

خطوط الأنابيب SoraFS مفيدة في تحديد القطع وتوقيع البيان وال
لا التحقق من البراهين. سطح الأوامر `sorafs_cli` يحمي هذه الأشرطة المحمولة
بين مُورِّدي CI. هذه الصفحة موجودة في المقدمة للقراءات الكنسية والنقطة مقابلها
نماذج مقترحة للعمل.

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

نقاط النقاط :

- Aucune clé de Signature statique n'est Stockée ; يتم الحصول على jetons OIDC حسب الطلب.
- يتم تحميل الأعمال الفنية (السيارة، البيان، الحزمة، السيرة الذاتية للإثباتات) للمراجعة.
- يتم استخدام نفس المخططات Norito التي يتم استخدامها أثناء عمليات النشر في الإنتاج.

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
    - sorafs_cli manifest submit --manifest artifacts/site.manifest.to --chunk-plan artifacts/site.plan.json --torii-url "$TORII_URL" --authority <katakana-i105-account-id> --private-key "$IROHA_PRIVATE_KEY" --summary-out artifacts/site.submit.json
    - sorafs_cli proof verify --manifest artifacts/site.manifest.to --car artifacts/site.car --summary-out artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- قم بتزويد `SIGSTORE_ID_TOKEN` عبر اتحاد معرف عبء العمل GitLab أو أحد
  تقنية سرية قبل تنفيذ شريط النشر.
- يقوم كل شريط CLI بإيقاف خط الأنابيب، مع الحفاظ على العناصر المتماسكة.

## الموارد الإضافية- قوالب شاملة (بما في ذلك مساعدات Bash، وتكوين الهوية الفيدرالية
  وأشرطة التنظيف): `docs/examples/sorafs_ci.md`
- خيار Référence CLI couvrant chaque : `docs/source/sorafs_cli.md`
- متطلبات الحكم/الاسم المستعار avant soumission :
  `docs/source/sorafs/provider_admission_policy.md`