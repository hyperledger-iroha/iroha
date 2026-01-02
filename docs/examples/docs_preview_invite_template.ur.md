---
lang: ur
direction: rtl
source: docs/examples/docs_preview_invite_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6c819c8d2a9517f1235a66a4661efd061a166ea89c953fd599e102b3cfd9157b
source_last_modified: "2025-11-10T18:08:48.050596+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/docs_preview_invite_template.md کا اردو ترجمہ -->

# ڈاکس پورٹل پری ویو دعوت (ٹیمپلیٹ)

ریویورز کو پری ویو ایکسس ہدایات بھیجتے وقت یہ ٹیمپلیٹ استعمال کریں۔ placeholders
(`<...>`) کو متعلقہ اقدار سے بدلیں، پیغام میں درج descriptor + archive artefacts
منسلک کریں، اور حتمی متن کو متعلقہ intake ٹکٹ میں محفوظ کریں۔

```text
موضوع: [DOCS-SORA] ڈاکس پورٹل پری ویو <preview_tag> کیلئے دعوت <reviewer/org>

السلام علیکم <name>,

GA سے پہلے ڈاکس پورٹل ریویو کیلئے رضاکارانہ مدد کا شکریہ۔ آپ کو wave <wave_id> کیلئے
منظور کر لیا گیا ہے۔ براہ کرم پری ویو دیکھنے سے پہلے درج ذیل مراحل پر عمل کریں:

1. CI یا SoraFS سے تصدیق شدہ artefacts ڈاؤن لوڈ کریں:
   - Descriptor: <descriptor_url> (`sha256:<descriptor_sha256>`)
   - Archive: <archive_url> (`sha256:<archive_sha256>`)
2. checksum gate چلائیں:

   ./docs/portal/scripts/preview_verify.sh      --descriptor <path-to-descriptor>      --archive <path-to-archive>      --build-dir <path-to-extracted-build>

3. checksum enforcement فعال رکھتے ہوئے پری ویو چلائیں:

   DOCS_RELEASE_TAG=<preview_tag> npm run --prefix docs/portal serve

4. acceptable-use، security، اور observability نوٹس پڑھیں:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. <request_ticket> کے ذریعے feedback جمع کریں اور ہر تلاش کو `<preview_tag>` سے ٹیگ کریں۔

سپورٹ <contact_channel> پر دستیاب ہے۔ انسیڈنٹس یا سکیورٹی مسائل فوراً
<incident_channel> کے ذریعے رپورٹ کریں۔ اگر Torii API ٹوکنز درکار ہوں تو
ٹکٹ کے ذریعے درخواست دیں؛ پروڈکشن credentials کبھی دوبارہ استعمال نہ کریں۔

پری ویو رسائی <end_date> کو ختم ہو جاتی ہے جب تک تحریری توسیع نہ ہو۔ ہم governance
کیلئے checksums اور دعوت کی میٹا ڈیٹا لاگ کرتے ہیں؛ جب آپ فارغ ہوں تو بتائیں تاکہ
ہم صاف طریقے سے آف بورڈ کر سکیں۔

پورٹل کو مستحکم کرنے میں مدد کیلئے دوبارہ شکریہ!

- DOCS-SORA ٹیم
```

</div>
