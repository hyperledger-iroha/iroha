---
lang: ar
direction: rtl
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-04T11:42:43.493867+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM وشهادة المصدر — Android SDK

| المجال | القيمة |
|-------|-------|
| النطاق | Android SDK (`java/iroha_android`) + نماذج التطبيقات (`examples/android/*`) |
| مالك سير العمل | هندسة الإصدار (أليكسي موروزوف) |
| آخر التحقق | 11-02-2026 (Buildkite `android-sdk-release#4821`) |

## 1. سير عمل الإنشاء

قم بتشغيل البرنامج النصي المساعد (تمت إضافته لأتمتة AND6):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

يقوم البرنامج النصي بما يلي:

1. ينفذ `ci/run_android_tests.sh` و`scripts/check_android_samples.sh`.
2. استدعاء برنامج تضمين Gradle ضمن `examples/android/` لإنشاء CycloneDX SBOMs لـ
   `:android-sdk`، و`:operator-console`، و`:retail-wallet` مع المرفق
   `-PversionName`.
3. انسخ كل SBOM إلى `artifacts/android/sbom/<sdk-version>/` بأسماء متعارف عليها
   (`iroha-android.cyclonedx.json`، وما إلى ذلك).

## 2. المصدر والتوقيع

نفس البرنامج النصي يوقع كل SBOM بـ `cosign sign-blob --bundle <file>.sigstore --yes`
ويصدر `checksums.txt` (SHA-256) في دليل الوجهة. قم بتعيين `COSIGN`
متغير البيئة إذا كان الثنائي يعيش خارج `$PATH`. وبعد انتهاء السيناريو،
قم بتسجيل مسارات الحزمة/المجموع الاختباري بالإضافة إلى معرف تشغيل Buildkite
`docs/source/compliance/android/evidence_log.csv`.

## 3. Verification

للتحقق من SBOM المنشورة:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

قارن مخرجات SHA بالقيمة المدرجة في `checksums.txt`. يقوم المراجعون أيضًا بمقارنة SBOM بالإصدار السابق للتأكد من أن دلتا التبعية مقصودة.

## 4. لقطة الأدلة (2026-02-11)

| مكون | سبوم | شا-256 | حزمة Sigstore |
|-----------|------|--------|-----------------|
| أندرويد SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | حزمة `.sigstore` مخزنة بجانب SBOM |
| نموذج وحدة تحكم المشغل | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| عينة من محفظة التجزئة | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(تعمل التجزئات التي تم التقاطها من Buildkite على تشغيل `android-sdk-release#4821`؛ ويتم إعادة إنتاجها عبر أمر التحقق أعلاه.)*

## 5. العمل المتميز

- أتمتة خطوات SBOM + cosign داخل مسار الإصدار قبل GA.
- قم بنسخ SBOMs إلى مجموعة المصنوعات اليدوية العامة بمجرد وضع علامة AND6 على اكتمال قائمة التحقق.
- التنسيق مع المستندات لربط مواقع تنزيل SBOM من ملاحظات الإصدار التي تواجه الشريك.