---
lang: ur
direction: rtl
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2026-01-03T18:07:59.200257+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# اور 6 EU قانونی سائن آف میمو ٹیمپلیٹ

اس میمو میں روڈ میپ آئٹم ** اور 6 ** کے ذریعہ مطلوبہ قانونی جائزہ ریکارڈ کیا گیا ہے
EU (ETSI/GDPR) آرٹ فیکٹ پیکٹ ریگولیٹرز کو پیش کیا جاتا ہے۔ وکیل کو کلون کرنا چاہئے
یہ ٹیمپلیٹ فی ریلیز ، نیچے والے فیلڈز کو آباد کریں ، اور دستخط شدہ کاپی اسٹور کریں
میمو میں حوالہ کردہ غیر منقولہ نوادرات کے ساتھ ساتھ۔

## خلاصہ

- ** ریلیز / ٹرین: ** `<e.g., 2026.1 GA>`
- ** جائزہ کی تاریخ: ** `<YYYY-MM-DD>`
- ** کونسل / جائزہ لینے والا: ** `<name + organisation>`
- ** دائرہ کار: ** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- ** وابستہ ٹکٹ: ** `<governance or legal issue IDs>`

## آرٹ فیکٹ چیک لسٹ

| نوادرات | SHA-256 | مقام / لنک | نوٹ |
| ---------- | --------- | ----------------- | ------- |
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + گورننس آرکائیو | رہائی کے شناخت کنندگان اور دھمکی کے ماڈل ایڈجسٹمنٹ کی تصدیق کریں۔ |
| `gdpr_dpia_summary.md` | `<hash>` | ایک ہی ڈائرکٹری / لوکلائزیشن آئینے | یقینی بنائیں کہ Redaction پالیسی کے حوالہ جات `sdk/android/telemetry_redaction.md` سے مماثل ہیں۔ |
| `sbom_attestation.md` | `<hash>` | ثبوت بالٹی میں ایک ہی ڈائرکٹری + کوسائن بنڈل | cyclonedx + پروویژن دستخطوں کی تصدیق کریں۔ |
| ثبوت لاگ قطار | `<hash>` | `docs/source/compliance/android/evidence_log.csv` | قطار نمبر `<n>` |
| ڈیوائس لیب ہنگامی بنڈل | `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` | اس ریلیز سے منسلک فیل اوور ریہرسل کی تصدیق کرتا ہے۔ |

> اضافی قطاریں منسلک کریں اگر پیکٹ میں مزید فائلیں ہوں (مثال کے طور پر ، رازداری
> ضمیمہ یا ڈی پی آئی اے کے ترجمے)۔ ہر نوادرات کو اس کے ناقابل تسخیر کا حوالہ دینا چاہئے
> ہدف اور بلڈکائٹ جاب اپ لوڈ کریں جس نے اسے تیار کیا۔

## نتائج اور مستثنیات

- `None.` *(گولیوں کی فہرست کے ساتھ بدل دیں ، بقایا خطرات کو پورا کرتے ہوئے ، معاوضہ
  کنٹرول ، یا مطلوبہ فالو اپ اقدامات۔)*

## منظوری

- ** فیصلہ: ** `<Approved / Approved with conditions / Blocked>`
- ** دستخط / ٹائم اسٹیمپ: ** `<digital signature or email reference>`
- ** فالو اپ مالکان: ** `<team + due date for any conditions>`

حتمی میمو کو گورننس شواہد کی بالٹی میں اپ لوڈ کریں ، SHA-256 میں کاپی کریں
`docs/source/compliance/android/evidence_log.csv` ، اور اپ لوڈ کے راستے کو لنک کریں
`status.md`۔ اگر فیصلہ "مسدود" ہے تو ، اور 6 اسٹیئرنگ میں اضافہ کریں
کمیٹی اور دستاویز کے تدارک کے اقدامات دونوں روڈ میپ ہاٹ لسٹ اور دونوں میں
ڈیوائس لیب ہنگامی لاگ