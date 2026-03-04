---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-ci.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-سی آئی
عنوان: SoraFS کے لئے CI ترکیبیں
سائڈبار_لیبل: سی آئی ترکیبیں
تفصیل: SoraFS CLI کو گٹ ہب اور گٹ لیب پائپ لائنوں کے اندر کیلی لیس سائننگ کے ساتھ چلائیں۔
---

::: منظور شدہ ماخذ کو نوٹ کریں
یہ صفحہ `docs/source/sorafs/developer/ci.md` کی عکاسی کرتا ہے۔ اس بات کو یقینی بنائیں کہ پرانی دستاویزات ریٹائر ہونے تک دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

#CI ترکیبیں

SoraFS پائپ لائنز ڈٹرمینسٹک ہیشنگ ، مینی فیسٹ سائننگ ، اور پروف چیکنگ کا فائدہ اٹھائیں۔ سطح کے احکامات کو برقرار رکھتا ہے
`sorafs_cli` CI فراہم کنندگان کے مابین ان اقدامات کی پورٹیبلٹی کو یقینی بناتا ہے۔ اس صفحے نے منظور شدہ ترکیبوں پر روشنی ڈالی ہے اور اس کی نشاندہی کی ہے
استعمال کے لئے تیار ٹیمپلیٹس میں۔

## گٹ ہب ایکشن (کوئی چابیاں نہیں)

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

کلیدی نکات:

- جامد دستخط کرنے والی چابیاں ذخیرہ نہیں ہوتی ہیں۔ کوڈ OIDC درخواست پر دستیاب ہیں۔
- نمونے (کار ، منشور ، پیکیج ، ثبوت کے خلاصے) جائزے کے لئے پیش کیے گئے ہیں۔
- مشن اسی Norito اسکیمات کو دوبارہ تیار کرتا ہے جو پروڈکشن لانچوں میں استعمال ہوتا ہے۔

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
    - sorafs_cli manifest submit --manifest artifacts/site.manifest.to --chunk-plan artifacts/site.plan.json --torii-url "$TORII_URL" --authority ih58... --private-key "$IROHA_PRIVATE_KEY" --summary-out artifacts/site.submit.json
    - sorafs_cli proof verify --manifest artifacts/site.manifest.to --car artifacts/site.car --summary-out artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- تعیناتی کے مرحلے کو انجام دینے سے پہلے گٹ لیب ورک بوجھ شناختی فیڈریشن یا مہر بند راز کے ذریعے `SIGSTORE_ID_TOKEN` فراہم کریں۔
- سی ایل آئی میں کسی بھی قدم کو ناکام بنانا پائپ لائن کو روکتا ہے ، نمونے کو مستقل رکھتے ہوئے۔

## اضافی وسائل

-اختتام سے آخر میں ٹیمپلیٹس (بش مددگار ، فیڈریٹڈ شناخت کی ترتیب ، اور صفائی کے اقدامات شامل ہیں): `docs/examples/sorafs_ci.md`
- CLI حوالہ تمام اختیارات کا احاطہ کرتا ہے: `docs/source/sorafs_cli.md`
- جمع کرانے سے پہلے گورننس کی ضروریات/عرفیت:
  `docs/source/sorafs/provider_admission_policy.md`