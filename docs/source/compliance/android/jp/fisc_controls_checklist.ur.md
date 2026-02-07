---
lang: ur
direction: rtl
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2026-01-03T18:07:59.237724+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC سیکیورٹی کنٹرولز چیک لسٹ - Android SDK

| فیلڈ | قیمت |
| ------- | ------- |
| ورژن | 0.1 (2026-02-12) |
| دائرہ کار | Android SDK + آپریٹر ٹولنگ جاپانی مالی تعیناتیوں میں استعمال ہوتا ہے |
| مالکان | تعمیل اور قانونی (ڈینیئل پارک) ، اینڈروئیڈ پروگرام لیڈ |

## کنٹرول میٹرکس

| FISC Control | عمل درآمد کی تفصیل | ثبوت / حوالہ جات | حیثیت |
| -------------- |
| ** سسٹم کنفیگریشن سالمیت ** | `ClientConfig` مینیفیسٹ ہیشنگ ، اسکیما کی توثیق ، ​​اور صرف پڑھنے کے لئے رن ٹائم تک رسائی کو نافذ کرتا ہے۔ کنفیگریشن دوبارہ لوڈ ناکامیوں کا اخراج `android.telemetry.config.reload` رن بک میں دستاویزی دستاویزات۔ | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` ؛ `docs/source/android_runbook.md` §1–2. | ✅ نافذ |
| ** رسائی کنٹرول اور توثیق ** | ایس ڈی کے آنرز Torii TLS پالیسیاں اور `/v1/pipeline` نے درخواستوں پر دستخط کیے۔ آپریٹر ورک فلوز ریفرنس سپورٹ پلے بوک §4–5 کے لئے بڑھتی ہوئی Norito نمونے کے ذریعے بڑھتی ہوئی گیٹنگ کے لئے اوور رائڈ گیٹنگ۔ | `docs/source/android_support_playbook.md` ؛ `docs/source/sdk/android/telemetry_redaction.md` (ورک فلو کو اوور رائڈ)۔ | ✅ نافذ |
| ** کریپٹوگرافک کلیدی انتظام ** | مضبوط باکس-ترجیحی فراہم کنندگان ، تصدیق کی توثیق ، ​​اور ڈیوائس میٹرکس کوریج KMS تعمیل کو یقینی بناتی ہے۔ `artifacts/android/attestation/` کے تحت محفوظ شدہ تنظیموں کی تصدیق کے نتائج اور تیاری میٹرکس میں ٹریک کیا گیا ہے۔ | `docs/source/sdk/android/key_management.md` ؛ `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` ؛ `scripts/android_strongbox_attestation_ci.sh`۔ | ✅ نافذ |
| ** لاگنگ ، نگرانی ، اور برقرار رکھنا ** | ٹیلی میٹری ریڈیکشن پالیسی حساس ڈیٹا ، بکٹائزز ڈیوائس کی خصوصیات ، اور برقرار رکھنے (7/30/90/365 دن کی ونڈوز) کو نافذ کرتی ہے۔ سپورٹ پلے بوک §8 میں ڈیش بورڈ کی حد کی وضاحت کی گئی ہے۔ `telemetry_override_log.md` میں ریکارڈ شدہ اوور رائڈس۔ | `docs/source/sdk/android/telemetry_redaction.md` ؛ `docs/source/android_support_playbook.md` ؛ `docs/source/sdk/android/telemetry_override_log.md`۔ | ✅ نافذ |
| ** آپریشنز اور چینج مینجمنٹ ** | GA کٹ اوور کا طریقہ کار (سپورٹ پلے بوک §7.2) پلس `status.md` تازہ ترین ٹریک کی رہائی کی تیاری۔ `docs/source/compliance/android/eu/sbom_attestation.md` کے ذریعے منسلک ثبوت (SBOM ، Sigstore بنڈل) جاری کریں۔ | `docs/source/android_support_playbook.md` ؛ `status.md` ؛ `docs/source/compliance/android/eu/sbom_attestation.md`۔ | ✅ نافذ |
| ** واقعہ کا جواب اور رپورٹنگ ** | پلے بوک نے شدت میٹرکس ، ایس ایل اے رسپانس ونڈوز ، اور تعمیل اطلاعاتی اقدامات کی وضاحت کی ہے۔ ٹیلی میٹری اوور رائڈس + افراتفری کی مشقیں پائلٹوں سے پہلے تولیدی صلاحیت کو یقینی بناتی ہیں۔ | `docs/source/android_support_playbook.md` §§4–9 ؛ `docs/source/sdk/android/telemetry_chaos_checklist.md`۔ | ✅ نافذ |
| ** ڈیٹا رہائش / لوکلائزیشن ** | جے پی کی تعیناتیوں کے لئے ٹیلی میٹری جمع کرنے والے منظور شدہ ٹوکیو خطے میں چلتے ہیں۔ ریجن میں ذخیرہ شدہ اور پارٹنر ٹکٹوں سے حوالہ دیتے ہیں۔ لوکلائزیشن کا منصوبہ بیٹا (اور 5) سے پہلے جاپانیوں میں دستیاب دستاویزات کو یقینی بناتا ہے۔ | `docs/source/android_support_playbook.md` §9 ؛ `docs/source/sdk/android/developer_experience_plan.md` §5 ؛ `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`۔ | 🈺 پیشرفت میں (لوکلائزیشن جاری) |

## جائزہ لینے والے نوٹ

- ریگولیٹ پارٹنر آن بورڈنگ سے پہلے گلیکسی S23/S24 کے لئے ڈیوائس میٹرکس اندراجات کی تصدیق کریں (ملاحظہ کریں ڈاکٹر کی قطاریں `s23-strongbox-a` ، `s24-strongbox-a`)۔
- جے پی کی تعیناتیوں میں ٹیلی میٹری جمع کرنے والے کو یقینی بنائیں کہ ڈی پی آئی اے (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`) میں بیان کردہ ایک ہی برقراری/اوور رائڈ منطق کو نافذ کریں۔
- ایک بار بینکاری شراکت دار اس چیک لسٹ کا جائزہ لینے کے بعد بیرونی آڈیٹرز سے تصدیق پر قبضہ کریں۔