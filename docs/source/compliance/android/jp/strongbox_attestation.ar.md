---
lang: ar
direction: rtl
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2026-01-03T18:07:59.238062+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# دليل شهادة StrongBox - عمليات النشر في اليابان

| المجال | القيمة |
|-------|-------|
| نافذة التقييم | 2026-02-10 – 2026-02-12 |
| موقع القطعة الأثرية | `artifacts/android/attestation/<device-tag>/<date>/` (تنسيق الحزمة لكل `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`) |
| أدوات الالتقاط | `scripts/android_keystore_attestation.sh`، `scripts/android_strongbox_attestation_ci.sh`، `scripts/android_strongbox_attestation_report.py` |
| المراجعين | قائد مختبر الأجهزة والامتثال والشؤون القانونية (JP) |

## 1. إجراء الالتقاط

1. على كل جهاز مدرج في مصفوفة StrongBox، أنشئ تحديًا واحصل على حزمة الشهادة:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. إرسال بيانات تعريف الحزمة (`result.json`، `chain.pem`، `challenge.hex`، `alias.txt`) إلى شجرة الأدلة.
3. قم بتشغيل مساعد CI لإعادة التحقق من جميع الحزم دون الاتصال بالإنترنت:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. ملخص الجهاز (12-02-2026)

| علامة الجهاز | الموديل / سترونج بوكس ​​| مسار الحزمة | النتيجة | ملاحظات |
|------------|-------------------|-------------|--------|-------|
| `pixel6-strongbox-a` | بكسل 6 / موتر G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ نجح (مدعوم بالأجهزة) | التحدي المقيد، تصحيح نظام التشغيل 2025-03-05. |
| `pixel7-strongbox-a` | بكسل 7 / تينسور جي 2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ نجح | مرشح حارة CI الأساسي؛ درجة الحرارة ضمن المواصفات. |
| `pixel8pro-strongbox-a` | بكسل 8 برو / تينسور جي3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ نجح (إعادة الاختبار) | تم استبدال محور USB-C؛ استحوذت Buildkite `android-strongbox-attestation#221` على حزمة التمرير. |
| `s23-strongbox-a` | جالاكسي S23 / سنابدراجون 8 الجيل الثاني | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ نجح | Knox attestation profile imported 2026-02-09. |
| `s24-strongbox-a` | جالاكسي S24 / سنابدراجون 8 الجيل الثالث | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ نجح | تم استيراد ملف تعريف شهادة Knox؛ حارة CI الآن باللون الأخضر. |

يتم تعيين علامات الجهاز إلى `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`.

## 3. قائمة مراجعة المراجعين

- [x] يظهر التحقق من `result.json` `strongbox_attestation: true` وسلسلة الشهادات إلى الجذر الموثوق به.
- [x] تأكد من تطابق وحدات بايت التحدي التي يقوم Buildkite بتشغيل `android-strongbox-attestation#219` (المسح الأولي) و`#221` (إعادة اختبار هاتف Pixel 8 Pro + التقاط S24).
- [x] إعادة تشغيل التقاط Pixel 8 Pro بعد إصلاح الأجهزة (المالك: Hardware Lab Lead، تم الانتهاء منه في 2026-02-13).
- [x] أكمل التقاط هاتف Galaxy S24 بمجرد وصول الموافقة على ملف تعريف Knox (المالك: Device Lab Ops، تم الانتهاء منه في 13-02-2026).

## 4. التوزيع

- قم بإرفاق هذا الملخص بالإضافة إلى أحدث ملف نصي للتقرير بحزم امتثال الشريك (قائمة مراجعة FISC §مقر البيانات).
- مسارات الحزمة المرجعية عند الاستجابة لعمليات تدقيق الجهات التنظيمية؛ لا تنقل الشهادات الأولية خارج القنوات المشفرة.

## 5. سجل التغيير

| التاريخ | التغيير | المؤلف |
|------|--------|--------|
| 2026-02-12 | التقاط حزمة JP الأولية + تقرير. | عمليات معمل الأجهزة |