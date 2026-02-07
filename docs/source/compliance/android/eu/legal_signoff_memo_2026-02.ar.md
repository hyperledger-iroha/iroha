---
lang: ar
direction: rtl
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2026-01-03T18:07:59.201100+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 مذكرة تسجيل الخروج القانونية للاتحاد الأوروبي - 2026.1 GA (Android SDK)

## ملخص

- **الإصدار/التدريب:** 2026.1 GA (Android SDK)
- **تاريخ المراجعة:** 15-04-2026
- **المستشار / المراجع:** صوفيا مارتينز - الامتثال والشؤون القانونية
- **النطاق:** الهدف الأمني ETSI EN 319 401، وملخص القانون العام لحماية البيانات DPIA، وشهادة SBOM، وأدلة الطوارئ الخاصة بمختبر الأجهزة AND6
- **التذاكر المرتبطة:** `_android-device-lab` / AND6-DR-202602، AND6 متتبع الحوكمة (`GOV-AND6-2026Q1`)

## قائمة التحقق من القطع الأثرية

| قطعة أثرية | شا-256 | الموقع / الرابط | ملاحظات |
|----------|--------|-----------------|-------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | يطابق معرفات إصدار GA لعام 2026.1 ودلتا نموذج التهديد (إضافات Torii NRPC). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | مراجع سياسة القياس عن بعد AND7 (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | حزمة `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore (`android-sdk-release#4821`). | تمت مراجعة مصدر CycloneDX +؛ يطابق مهمة Buildkite `android-sdk-release#4821`. |
| سجل الأدلة | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (الصف `android-device-lab-failover-20260220`) | يؤكد سجل تجزئات الحزمة التي تم التقاطها + لقطة السعة + إدخال المذكرة. |
| حزمة طوارئ الأجهزة والمختبرات | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | التجزئة مأخوذة من `bundle-manifest.json`؛ تم تسجيل التذكرة AND6-DR-202602 بالتسليم إلى الشؤون القانونية/الامتثال. |

## النتائج والاستثناءات

- لم يتم تحديد مشكلات الحظر. تتوافق المصنوعات اليدوية مع متطلبات ETSI/GDPR؛ تمت الإشارة إلى تكافؤ القياس عن بعد AND7 في ملخص DPIA ولا يلزم إجراء أي عمليات تخفيف إضافية.
- التوصية: مراقبة التدريبات المجدولة DR-2026-05-Q2 (التذكرة AND6-DR-202605) وإلحاق الحزمة الناتجة بسجل الأدلة قبل نقطة تفتيش الحوكمة التالية.

## موافقة

- **القرار:** تمت الموافقة عليه
- **التوقيع / الطابع الزمني:** _صوفيا مارتينز (موقعة رقميًا عبر بوابة الإدارة، 15/04/2026 الساعة 14:32 بالتوقيت العالمي)_
- **أصحاب المتابعة:** Device Lab Ops (تسليم حزمة الأدلة DR-2026-05-Q2 قبل 2026-05-31)