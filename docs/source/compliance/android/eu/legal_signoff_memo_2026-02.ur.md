---
lang: ur
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

# اور 6 EU قانونی سائن آف میمو-2026.1 GA (Android SDK)

## خلاصہ

- ** ریلیز / ٹرین: ** 2026.1 GA (Android SDK)
-** جائزہ تاریخ: ** 2026-04-15
- ** کونسل / جائزہ لینے والا: ** صوفیہ مارٹنز - تعمیل اور قانونی
۔
-** وابستہ ٹکٹ: ** `_android-device-lab` / and6-dr-202602 ، اور 6 گورننس ٹریکر (`GOV-AND6-2026Q1`)

## آرٹ فیکٹ چیک لسٹ

| نوادرات | SHA-256 | مقام / لنک | نوٹ |
| ---------- | --------- | ----------------- | ------- |
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | 2026.1 GA کی رہائی کے شناخت کنندگان اور دھمکی دینے والے ماڈل ڈیلٹا (Torii NRPC اضافے) سے میچ کرتا ہے۔ |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | حوالہ جات اور 7 ٹیلی میٹری پالیسی (`docs/source/sdk/android/telemetry_redaction.md`)۔ |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore بنڈل (`android-sdk-release#4821`)۔ | cyclonedx + پروویژن کا جائزہ لیا گیا ؛ بلڈکائٹ جاب `android-sdk-release#4821` سے میچ کرتا ہے۔ |
| ثبوت لاگ | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (قطار `android-device-lab-failover-20260220`) | اس بات کی تصدیق کرتا ہے کہ لاگ پر قبضہ شدہ بنڈل ہیش + صلاحیت اسنیپ شاٹ + میمو انٹری۔ |
| ڈیوائس لیب ہنگامی بنڈل | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | `bundle-manifest.json` سے لیا گیا ہیش ؛ ٹکٹ اور 6-DR-202602 نے قانونی/تعمیل کے لئے ہاتھ سے دور ریکارڈ کیا۔ |

## نتائج اور مستثنیات

- بلاک کرنے والے مسائل کی نشاندہی نہیں کی گئی۔ نوادرات ETSI/GDPR کی ضروریات کے ساتھ سیدھ میں ہیں۔ اور 7 ٹیلی میٹری کی برابری DPIA سمری میں نوٹ کی گئی ہے اور اس میں اضافی تخفیف کی ضرورت نہیں ہے۔
-سفارش: شیڈول DR-2026-05-Q2 ڈرل (ٹکٹ اور 6-DR-202605) کی نگرانی کریں اور اگلی گورننس چوکی سے پہلے ثبوت لاگ میں نتیجے میں بنڈل شامل کریں۔

## منظوری

- ** فیصلہ: ** منظور شدہ
-** دستخط / ٹائم اسٹیمپ: ** _سوفیا مارٹنز (گورننس پورٹل کے ذریعے ڈیجیٹل طور پر دستخط شدہ ، 2026-04-15 14:32 UTC) _
-** فالو اپ مالکان: ** ڈیوائس لیب آپس (DR-2026-05-Q2 ثبوت بنڈل کو 2026-05-31 سے پہلے فراہم کریں)