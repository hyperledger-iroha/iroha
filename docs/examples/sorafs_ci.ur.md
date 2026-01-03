---
lang: ur
direction: rtl
source: docs/examples/sorafs_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 61536f7f6dbecb2658cb5e07a15ed99888957bf87b473e0089e38f8431156c18
source_last_modified: "2025-11-08T09:36:10.010765+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/sorafs_ci.md کا اردو ترجمہ -->

---
title: SoraFS CI کوک بک
summary: GitHub Actions workflow جو سائن اور ویری فائی مراحل کو ایک job میں اکٹھا کرتا ہے، مع نظرثانی نوٹس۔
---

# SoraFS CI کوک بک

یہ snippet `docs/source/sorafs_ci_templates.md` کی رہنمائی کی عکاسی کرتا ہے اور دکھاتا ہے کہ
ایک ہی GitHub Actions job میں signing، verification اور proof checks کیسے شامل کریں۔

```yaml
name: sorafs-cli-release

on:
  push:
    branches: [main]

permissions:
  contents: read
  id-token: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-rust@v1
        with:
          rust-version: 1.92

      - name: Package payload
        run: |
          mkdir -p artifacts
          sorafs_cli car pack             --input payload.bin             --car-out artifacts/payload.car             --plan-out artifacts/chunk_plan.json             --summary-out artifacts/car_summary.json
          sorafs_cli manifest build             --summary artifacts/car_summary.json             --manifest-out artifacts/manifest.to

      - name: Sign manifest bundle
        run: |
          sorafs_cli manifest sign             --manifest artifacts/manifest.to             --chunk-plan artifacts/chunk_plan.json             --bundle-out artifacts/manifest.bundle.json             --signature-out artifacts/manifest.sig             --identity-token-provider=github-actions             --identity-token-audience=sorafs | tee artifacts/manifest.sign.summary.json

      - name: Verify manifest bundle
        run: |
          sorafs_cli manifest verify-signature             --manifest artifacts/manifest.to             --bundle artifacts/manifest.bundle.json             --summary artifacts/car_summary.json

      - name: Proof verification
        run: |
          sorafs_cli proof verify             --manifest artifacts/manifest.to             --car artifacts/payload.car             --summary-out artifacts/proof.json

      - uses: sigstore/cosign-installer@v3
      - name: Verify bundle with cosign
        run: cosign verify-blob --bundle artifacts/manifest.bundle.json artifacts/manifest.to
```

## نوٹس

- `sorafs_cli` کو runner پر دستیاب ہونا چاہیے (مثال کے طور پر، `cargo install --path crates/sorafs_car --features cli` ان مراحل سے پہلے)۔
- workflow کو واضح OIDC audience فراہم کرنا ہوگا (یہاں `sorafs`); `--identity-token-audience` کو اپنی Fulcio پالیسی کے مطابق ایڈجسٹ کریں۔
- release pipeline کو `artifacts/manifest.bundle.json`, `artifacts/manifest.sig` اور `artifacts/proof.json` کو governance review کے لئے archive کرنا چاہیے۔
- deterministic مثال artefacts `fixtures/sorafs_manifest/ci_sample` میں ہیں؛ جب golden manifests, chunk plans یا bundle JSON درکار ہوں تو انہیں tests میں کاپی کریں بغیر pipeline دوبارہ چلائے۔

## Fixtures کی تصدیق

اس workflow کے deterministic artefacts یہاں موجود ہیں:
`fixtures/sorafs_manifest/ci_sample`۔ Pipelines اوپر والے مراحل دوبارہ چلا کر outputs کو
canonical فائلوں کے مقابلے میں diff کر سکتے ہیں، مثال کے طور پر:

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

خالی diffs یہ ثابت کرتے ہیں کہ build نے byte-identical manifests، plans اور signature bundles بنائے۔
مزید تفصیل اور captured summaries سے release notes کے templates کے لئے
`fixtures/sorafs_manifest/ci_sample/README.md` دیکھیں۔

</div>
