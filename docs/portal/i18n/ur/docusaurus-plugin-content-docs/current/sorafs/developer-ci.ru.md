---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-ci.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-سی آئی
عنوان: SoraFS کے لئے CI ترکیبیں
سائڈبار_لیبل: سی آئی ترکیبیں
تفصیل: گٹ ہب میں CLI SoraFS اور کیلیس سائننگ کے ساتھ گٹ لیب پائپ لائنوں کو چلائیں۔
---

::: نوٹ کینونیکل ماخذ
:::

# CI ترکیبیں

SoraFS پائپ لائنز کا تعی .ن ، منشور پر دستخط کرنے ، اور ثبوتوں کی توثیق سے فائدہ ہوتا ہے۔
`sorafs_cli` کمانڈ کی سطح سی آئی فراہم کرنے والوں کے مابین ان اقدامات کو پورٹیبل بناتی ہے۔ یہ صفحہ
کیننیکل ترکیبوں اور استعمال کے لئے تیار ٹیمپلیٹس کی طرف اشارہ کرتا ہے۔

## گٹ ہب ایکشنز (کیلیس)

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

کلیدی نکات:

- جامد دستخط کرنے والی چابیاں ذخیرہ نہیں ہوتی ہیں۔ درخواست پر OIDC ٹوکن کی درخواست کی گئی ہے۔
- نمونے (کار ، منشور ، بنڈل ، ثبوت کے خلاصے) جائزہ لینے کے لئے بھری ہوئی ہیں۔
- ملازمت اسی Norito اسکیموں کو دوبارہ استعمال کرتی ہے جیسا کہ پروڈکشن رول آؤٹ میں ہے۔

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
    - sorafs_cli manifest submit --manifest artifacts/site.manifest.to --chunk-plan artifacts/site.plan.json --torii-url "$TORII_URL" --authority <katakana-i105-account-id> --private-key "$IROHA_PRIVATE_KEY" --summary-out artifacts/site.submit.json
    - sorafs_cli proof verify --manifest artifacts/site.manifest.to --car artifacts/site.car --summary-out artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- اشاعت کے مرحلے کو لانچ کرنے سے پہلے کام کے بوجھ شناختی فیڈریشن گٹ لیب کے ذریعے `SIGSTORE_ID_TOKEN` تیار کریں یا سیل شدہ راز۔
- کسی بھی سی ایل آئی قدم کی ناکامی پائپ لائن کو روکتی ہے ، مستقل نمونے کو محفوظ رکھتی ہے۔

## اضافی وسائل

-اختتام سے آخر میں ٹیمپلیٹس (جس میں باش مددگار ، فیڈریٹڈ شناخت کی تشکیل ، اور صفائی کے اقدامات شامل ہیں): `docs/examples/sorafs_ci.md`
- CLI حوالہ تمام اختیارات کا احاطہ کرتا ہے: `docs/source/sorafs_cli.md`
- بھیجنے سے پہلے گورننس/عرف کی ضروریات:
  `docs/source/sorafs/provider_admission_policy.md`