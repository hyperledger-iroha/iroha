---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-ci.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-سی آئی
عنوان: SoraFS CI ترکیبیں
سائڈبار_لیبل: سی آئی ترکیبیں
تفصیل: گٹھب اور گٹ لیب پائپ لائنوں پر SoraFS CLI کو کیلی لیس سائننگ کے ساتھ چلائیں۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/developer/ci.md` کا آئینہ دار ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

# CI ترکیبیں

SoraFS پائپ لائنز کا تعی .ن چنکنگ ، مینی فیسٹ سائننگ اور پروف چیکنگ سے فائدہ ہوتا ہے۔ کمانڈ کی سطح
`sorafs_cli` ان اقدامات کو CI فراہم کرنے والوں میں پورٹیبل رکھتا ہے۔ اس صفحے میں کیننیکل ترکیبیں پر روشنی ڈالی گئی ہے اور استعمال کے لئے تیار ٹیمپلیٹس کی نشاندہی کی گئی ہے۔

## گٹ ہب ایکشن (چابیاں کے بغیر)

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

کلیدی نکات:

- کوئی مستحکم ، ذخیرہ شدہ دستخطی چابیاں نہیں۔ OIDC ٹوکن طلب کے مطابق حاصل کیے جاتے ہیں۔
- نمونے (کار ، منشور ، بنڈل ، پروف خلاصے) کو جائزہ لینے کے لئے بھیجا گیا ہے۔
- نوکری اسی Norito اسکیموں کو دوبارہ تیار کرتی ہے جو پروڈکشن رول آؤٹ میں استعمال ہوتی ہیں۔

## گٹ لیب سی آئی

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

- اشاعت `SIGSTORE_ID_TOKEN` گٹ لیب ورک بوجھ شناختی فیڈریشن کے ذریعے یا شائع مرحلہ چلانے سے پہلے ایک مہر بند راز۔
- کسی بھی سی ایل آئی قدم کی ناکامی پائپ لائن کو رکنے کا سبب بنتی ہے ، جس سے مستقل نمونے کا تحفظ ہوتا ہے۔

## اضافی خصوصیات

-اختتام سے آخر میں ٹیمپلیٹس (بش مددگار ، فیڈریٹڈ شناخت کی تشکیل اور صفائی کے اقدامات شامل ہیں): `docs/examples/sorafs_ci.md`
- CLI حوالہ تمام اختیارات کا احاطہ کرتا ہے: `docs/source/sorafs_cli.md`
- جمع کرانے سے پہلے گورننس/الیاسنگ کی ضروریات:
  `docs/source/sorafs/provider_admission_policy.md`