---
lang: ur
direction: rtl
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2026-01-03T18:07:59.259775+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android ڈیوائس لیب انسٹرومینٹیشن ہکس (and6)

یہ حوالہ روڈ میپ ایکشن کو بند کرتا ہے "باقی ڈیوائس لیب / اسٹیج
اور 6 کک آف سے پہلے ہی آلہ سازی ہکس "۔ یہ بتاتا ہے کہ ہر محفوظ کیسے ہے
ڈیوائس لیب سلاٹ میں ٹیلی میٹری ، قطار اور تصدیق کے نوادرات پر قبضہ کرنا ہوگا
اور 6 تعمیل چیک لسٹ ، شواہد لاگ ، اور گورننس پیکٹ ایک ہی شریک ہیں
ڈٹرمینسٹک ورک فلو۔ اس نوٹ کو بکنگ کے طریقہ کار کے ساتھ جوڑیں
(`device_lab_reservation.md`) اور جب ریہرسل کی منصوبہ بندی کرتے ہیں تو فیل اوور رن بک۔

## اہداف اور دائرہ کار

- ** عزم ثبوت ** - تمام آلات کے آؤٹ پٹ کے تحت زندہ رہتے ہیں
  `artifacts/android/device_lab/<slot-id>/` SHA-256 کے ساتھ ظاہر ہوتا ہے تو آڈیٹرز
  تحقیقات کو دوبارہ جاری کیے بغیر بنڈل کو مختلف کر سکتے ہیں۔
- ** اسکرپٹ فرسٹ ورک فلو **- موجودہ مددگاروں کو دوبارہ استعمال کریں
  (`ci/run_android_telemetry_chaos_prep.sh` ،
  `scripts/android_keystore_attestation.sh` ، `scripts/android_override_tool.sh`)
  اس کے بجائے بیسپوک ADB کے احکامات۔
- ** چیک لسٹس مطابقت پذیری میں رہیں ** - ہر رن کا اس دستاویز کو اس دستاویز کا حوالہ دیا جاتا ہے
  اور 6 تعمیل چیک لسٹ اور نوادرات کو شامل کرتا ہے
  `docs/source/compliance/android/evidence_log.csv`۔

## آرٹیکٹیکٹ لے آؤٹ

1. ایک انوکھا سلاٹ شناخت کنندہ منتخب کریں جو ریزرویشن ٹکٹ سے مماثل ہو ، جیسے۔
   `2026-05-12-slot-a`۔
2. معیاری ڈائریکٹریز بیج:

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. مماثل فولڈر کے اندر ہر کمانڈ لاگ کو بچائیں (جیسے۔
   `telemetry/status.ndjson` ، `attestation/pixel8pro.log`)۔
4. ایک بار سلاٹ بند ہونے کے بعد SHA-256 پر قبضہ کریں:

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## اوزار میٹرکس

| بہاؤ | کمانڈ (زبانیں) | آؤٹ پٹ مقام | نوٹ |
| ------ | -------------- | ----------------- | ------- |
| ٹیلی میٹری ریڈیکشن + اسٹیٹس بنڈل | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson` ، `telemetry/status.log` | سلاٹ کے آغاز اور اختتام پر چلائیں۔ `status.log` سے Cli stdout منسلک کریں۔ |
| زیر التواء قطار + افراتفری پریپ | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin` ، `queue/*.json` ، `queue/*.sha256` | `readiness/labs/telemetry_lab_01.md` سے آئینہ دار منظر نامہ ؛ سلاٹ میں ہر آلے کے لئے env var کو بڑھاؤ۔ |
| اوور رائڈ لیجر ڈائجسٹ | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | یہاں تک کہ جب کوئی اوور رائڈز فعال نہ ہوں تو بھی ضروری ہے۔ صفر ریاست کو ثابت کریں۔ |
| مضبوط باکس / ٹی کی تصدیق | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | ہر محفوظ آلہ کے لئے دہرائیں (`android_strongbox_device_matrix.md` میں میچ کے نام)۔ |
| CI کنٹرول کی تصدیق رجعت | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | وہی ثبوت حاصل کرتا ہے جو CI اپ لوڈ کرتا ہے۔ توازن کے لئے دستی رنز میں شامل کریں۔ |
| لنٹ / انحصار بیس لائن | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt` ، `logs/lint.log` | فی منجمد ونڈو میں ایک بار چلائیں ؛ تعمیل پیکٹوں میں خلاصہ پیش کریں۔ |

## معیاری سلاٹ کا طریقہ کار1. ** پری فلائٹ (T-24H) **-ریزرویشن ٹکٹ کے حوالہ جات کی تصدیق کریں
   دستاویز ، ڈیوائس میٹرکس اندراج کو اپ ڈیٹ کریں ، اور نمونے کی جڑ کو بیج دیں۔
2. ** سلاٹ کے دوران **
   - پہلے ٹیلی میٹری بنڈل + قطار ایکسپورٹ کمانڈ چلائیں۔ پاس
     `--note <ticket>` to `ci/run_android_telemetry_chaos_prep.sh` تو لاگ ان کریں
     واقعہ کی شناخت کا حوالہ دیتا ہے۔
   - فی آلہ کی تصدیق کے اسکرپٹ کو متحرک کریں۔ جب کنٹرول پیدا کرتا ہے a
     `device_lab_reservation.md` ، اسے نوادرات کی جڑ میں کاپی کریں اور پرنٹ شدہ گٹ شا کو ریکارڈ کریں
     اسکرپٹ کا اختتام۔
   - `make android-lint` کو اوورراڈڈ سمری راہ کے ساتھ انجام دیں یہاں تک کہ اگر CI
     پہلے ہی بھاگ گیا ؛ آڈیٹر فی سلاٹ لاگ کی توقع کرتے ہیں۔
3. ** رنز کے بعد **
   - سلاٹ کے اندر `sha256sum.txt` اور `README.md` (فری فارم نوٹ) بنائیں
     فولڈر نے پھانسی والے کمانڈز کا خلاصہ کیا۔
   - `docs/source/compliance/android/evidence_log.csv` میں ایک قطار کو شامل کریں
     سلاٹ ID ، ہیش مینی فیسٹ راہ ، بلڈکائٹ حوالہ جات (اگر کوئی ہے) ، اور تازہ ترین
     ریزرویشن کیلنڈر برآمد سے ڈیوائس لیب کی گنجائش فیصد۔
   - `_android-device-lab` ٹکٹ ، اور 6 میں سلاٹ فولڈر کو لنک کریں
     چیک لسٹ ، اور `docs/source/android_support_playbook.md` ریلیز رپورٹ۔

## ناکامی سے نمٹنے اور بڑھتے ہوئے

- اگر کوئی کمانڈ ناکام ہوجاتا ہے تو ، `logs/` کے تحت stderr آؤٹ پٹ پر قبضہ کریں اور اس کی پیروی کریں
  `device_lab_reservation.md` §6 میں اسکیلیشن سیڑھی۔
- قطار یا ٹیلی میٹری کی کمی کو فوری طور پر اوور رائڈ کی حیثیت کو نوٹ کرنا چاہئے
  `docs/source/sdk/android/telemetry_override_log.md` اور سلاٹ ID کا حوالہ دیں
  لہذا گورننس ڈرل کا سراغ لگاسکتی ہے۔
- تصدیق کے رجعتوں کو ریکارڈ کرنا ضروری ہے
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  ناکام ڈیوائس سیریلز اور مذکورہ بالا بنڈل راستوں کے ساتھ۔

## چیک لسٹ کی اطلاع دہندگی

سلاٹ کو مکمل نشان لگانے سے پہلے ، تصدیق کریں کہ مندرجہ ذیل حوالوں کو اپ ڈیٹ کیا گیا ہے:

- `docs/source/compliance/android/and6_compliance_checklist.md` - نشان زد کریں
  آلہ سازی کی قطار مکمل اور سلاٹ ID کو نوٹ کریں۔
- `docs/source/compliance/android/evidence_log.csv` - اندراج کو شامل/اپ ڈیٹ کریں
  سلاٹ ہیش اور صلاحیت پڑھنا۔
- `_android-device-lab` ٹکٹ - آرٹ فیکٹ لنکس اور بلڈکائٹ جاب IDs منسلک کریں۔
- `status.md` - اگلے Android تیاری میں ایک مختصر نوٹ شامل کریں
  روڈ میپ کے قارئین جانتے ہیں کہ کس سلاٹ نے تازہ ترین ثبوت پیش کیے۔

اس عمل کے بعد اور 6 کے "ڈیوائس لیب + انسٹرومینٹیشن ہکس" رہتا ہے
سنگ میل کے قابل آڈٹ اور بکنگ ، عملدرآمد ، کے مابین دستی موڑ کو روکتا ہے
اور رپورٹنگ