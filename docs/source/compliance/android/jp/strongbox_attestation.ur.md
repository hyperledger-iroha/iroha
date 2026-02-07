---
lang: ur
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

# مضبوط باکس کی تصدیق کے ثبوت - جاپان کی تعیناتی

| فیلڈ | قیمت |
| ------- | ------- |
| تشخیص ونڈو | 2026-02-10-2026-02-12 |
| نوادرات کا مقام | `artifacts/android/attestation/<device-tag>/<date>/` (بنڈل فارمیٹ فی `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`) |
| کیپچر ٹولنگ | `scripts/android_keystore_attestation.sh` ، `scripts/android_strongbox_attestation_ci.sh` ، `scripts/android_strongbox_attestation_report.py` |
| جائزہ لینے والے | ہارڈ ویئر لیب لیڈ ، تعمیل اور قانونی (جے پی) |

## 1. گرفتاری کا طریقہ کار

1. مضبوط باکس میٹرکس میں درج ہر آلے پر ، ایک چیلنج پیدا کریں اور تصدیق کے بنڈل کو پکڑیں:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. بنڈل میٹا ڈیٹا (`result.json` ، `chain.pem` ، `challenge.hex` ، `alias.txt`) شواہد کے درخت سے کمٹ کریں۔
3. تمام بنڈل آف لائن کی دوبارہ توثیق کرنے کے لئے CI مددگار چلائیں:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2۔ آلہ کا خلاصہ (2026-02-12)

| ڈیوائس ٹیگ | ماڈل / مضبوط باکس | بنڈل راستہ | نتیجہ | نوٹ |
| ------------ | ------------------- | ------------- | -------- | ------- |
| `pixel6-strongbox-a` | پکسل 6 / ٹینسر جی 1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ پاس ہوا (ہارڈ ویئر کی حمایت یافتہ) | چیلنج پابند ، OS پیچ 2025-03-05۔ |
| `pixel7-strongbox-a` | پکسل 7 / ٹینسر جی 2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ پاس ہوا | پرائمری سی آئی لین امیدوار ؛ درجہ حرارت مخصوص کے اندر۔ |
| `pixel8pro-strongbox-a` | پکسل 8 پرو / ٹینسر جی 3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ پاس ہوا (دوبارہ کوشش کریں) | USB-C حب کی جگہ ؛ بلڈکائٹ `android-strongbox-attestation#221` نے گزرتے ہوئے بنڈل پر قبضہ کرلیا۔ |
| `s23-strongbox-a` | گلیکسی ایس 23 / اسنیپ ڈریگن 8 جنرل 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ پاس ہوا | ناکس کی تصدیق کا پروفائل 2026-02-09 درآمد کیا گیا۔ |
| `s24-strongbox-a` | گلیکسی ایس 24 / اسنیپ ڈریگن 8 جنرل 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ پاس ہوا | ناکس کی تصدیق کا پروفائل درآمد ؛ CI لین اب سبز ہے۔ |

ڈیوائس ٹیگز کا نقشہ `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` پر۔

## 3. جائزہ لینے والا چیک لسٹ

- [x] `result.json` کی تصدیق `strongbox_attestation: true` اور سرٹیفکیٹ چین کو قابل اعتماد جڑ سے ظاہر کرتا ہے۔
۔
-[x] ہارڈ ویئر فکس کے بعد دوبارہ پکسل 8 پرو کیپچر (مالک: ہارڈ ویئر لیب لیڈ ، 2026-02-13 مکمل ہوا)۔
-[x] مکمل گلیکسی ایس 24 کیپچر ایک بار ناکس پروفائل کی منظوری آجائے (مالک: ڈیوائس لیب آپس ، 2026-02-13 مکمل ہوا)۔

## 4. تقسیم

- اس سمری کے علاوہ تازہ ترین رپورٹ ٹیکسٹ فائل کو پارٹنر تعمیل پیکٹ (ایف آئی ایس سی لسٹ § ڈیٹا ریذیڈنسی) سے منسلک کریں۔
- ریگولیٹر آڈٹ کا جواب دیتے وقت حوالہ بنڈل راستے۔ خفیہ کردہ چینلز کے باہر خام سرٹیفکیٹ منتقل نہ کریں۔

## 5. لاگ ان لاگ

| تاریخ | تبدیلی | مصنف |
| ------ | -------- | -------- |
| 2026-02-12 | ابتدائی جے پی بنڈل کیپچر + رپورٹ۔ | ڈیوائس لیب آپس |