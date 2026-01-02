---
lang: ur
direction: rtl
source: docs/examples/docs_preview_invite_email.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e3856058310e40649d5394996b2bcbfde99effb9e706be87f284e1812d5bdbd
source_last_modified: "2025-11-15T04:49:30.881970+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/docs_preview_invite_email.md کا اردو ترجمہ -->

# ڈاکس پورٹل پری ویو دعوت (نمونہ ای میل)

آؤٹ باؤنڈ پیغام تیار کرتے وقت اس نمونے کو استعمال کریں۔ یہ W2 کمیونٹی ریویورز
(`preview-2025-06-15`) کو بھیجی گئی اصل کاپی کو محفوظ کرتا ہے تاکہ آئندہ waves
پرانی ٹکٹس کھنگالے بغیر ٹون، ویریفکیشن گائیڈنس، اور ایویڈنس ٹریل کو میچ کر سکیں۔
نیا دعوت نامہ بھیجنے سے پہلے artefacts لنکس، hashes، request IDs، اور تاریخیں
اپڈیٹ کریں۔

```text
موضوع: [DOCS-SORA] ڈاکس پورٹل پری ویو preview-2025-06-15 کیلئے دعوت Horizon Wallet

سلام Sam,

W2 کمیونٹی پری ویو کیلئے Horizon Wallet کو دوبارہ پیش کرنے کا شکریہ۔ Wave W2 اب
منظور ہو چکا ہے، اس لئے آپ نیچے دیے گئے مراحل مکمل کرتے ہی ریویو شروع کر سکتے ہیں۔
براہ کرم artefacts اور access tokens کو نجی رکھیں: ہر دعوت DOCS-SORA-Preview-W2 میں
ٹریس ہوتی ہے اور آڈیٹرز acknowledgements کی پڑتال کریں گے۔

1. تصدیق شدہ artefacts ڈاؤن لوڈ کریں (وہی bits جو ہم نے SoraFS اور CI کو دیے):
   - Descriptor: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/descriptor.json (`sha256:a1f41cfb02a5f34f2a0e6535f0b079dbb645c1b5dcdbcb36f953ef5c418260ad`)
   - Archive: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/docs-portal-preview.tar.zst (`sha256:5bc30261fa3c0db032ac2b3c4b56651bebcd309d69a2634ebc9a6f0da3435399`)
2. extract کرنے سے پہلے bundle verify کریں:

   ./docs/portal/scripts/preview_verify.sh      --descriptor ~/Downloads/descriptor.json      --archive ~/Downloads/docs-portal-preview.tar.zst      --build-dir ~/sora-docs/preview-2025-06-15

3. checksum enforcement کے ساتھ پری ویو serve کریں:

   DOCS_RELEASE_TAG=preview-2025-06-15 npm run --prefix docs/portal serve

4. ٹیسٹنگ سے پہلے hardened runbooks پڑھیں:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. DOCS-SORA-Preview-REQ-C04 کے ذریعے feedback فائل کریں اور ہر finding کو
   `docs-preview/w2` سے ٹیگ کریں۔ اگر آپ structured intake چاہتے ہیں تو
   docs/examples/docs_preview_feedback_form.md استعمال کریں۔

سپورٹ Matrix (`#docs-preview:matrix.org`) پر دستیاب ہے اور office hours
2025-06-18 15:00 UTC پر ہوں گے۔ سکیورٹی یا انسیڈنٹ escalations کیلئے فوراً
ops@sora.org یا +1-555-0109 کے ذریعے docs on-call کو page کریں؛ office hours کا انتظار نہ کریں۔

Horizon Wallet کیلئے پری ویو رسائی 2025-06-15 -> 2025-06-29 تک رہے گی۔ جیسے ہی آپ
مکمل کریں اطلاع دیں تاکہ ہم عارضی access keys واپس لے سکیں اور tracker میں
close-out ریکارڈ کر سکیں۔

پورٹل کو GA تک لے جانے میں مدد کیلئے شکریہ!

- DOCS-SORA ٹیم
```

</div>
