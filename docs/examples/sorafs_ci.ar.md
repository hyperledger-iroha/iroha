---
lang: ar
direction: rtl
source: docs/examples/sorafs_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 61536f7f6dbecb2658cb5e07a15ed99888957bf87b473e0089e38f8431156c18
source_last_modified: "2025-11-08T09:36:10.010765+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/sorafs_ci.md -->

---
title: دليل CI لـ SoraFS
summary: سير عمل GitHub Actions يجمع خطوات التوقيع والتحقق مع ملاحظات المراجعة.
---

# دليل CI لـ SoraFS

يعكس هذا المقتطف الارشاد في `docs/source/sorafs_ci_templates.md` ويعرض كيفية دمج التوقيع
والتحقق وفحوصات proof ضمن مهمة واحدة في GitHub Actions.

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

## ملاحظات

- يجب ان يكون `sorafs_cli` متاحا على runner (مثل `cargo install --path crates/sorafs_car --features cli` قبل هذه الخطوات).
- يجب على workflow توفير audience OIDC صريح (هنا `sorafs`); عدّل `--identity-token-audience` ليتوافق مع سياسة Fulcio الخاصة بك.
- يجب على pipeline الخاص بالاصدار ارشفة `artifacts/manifest.bundle.json` و `artifacts/manifest.sig` و `artifacts/proof.json` لمراجعة الحوكمة.
- توجد artefacts حتمية تجريبية في `fixtures/sorafs_manifest/ci_sample`; انسخها الى الاختبارات عندما تحتاج manifests او chunk plans او bundle JSON golden دون اعادة تشغيل pipeline.

## التحقق من fixtures

توجد artefacts حتمية لهذا workflow تحت
`fixtures/sorafs_manifest/ci_sample`. يمكن للـ pipelines اعادة تنفيذ الخطوات اعلاه ومقارنة
المخرجات مع الملفات القياسية، على سبيل المثال:

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

الـ diffs الفارغة تؤكد ان البناء انتج manifests وplans وsignature bundles متطابقة البايت.
راجع `fixtures/sorafs_manifest/ci_sample/README.md` لقائمة كاملة ونصائح حول توليد
release notes من summaries الملتقطة.

</div>
