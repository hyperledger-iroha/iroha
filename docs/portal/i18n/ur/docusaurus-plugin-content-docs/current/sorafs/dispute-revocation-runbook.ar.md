---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: تنازعہ کی بحالی کی کتاب
عنوان: SoraFS تنازعات اور منسوخی آپریشن دستی
سائڈبار_لیبل: تنازعات اور منسوخی کے لئے آپریشن گائیڈ
تفصیل: SoraFS صلاحیت کے تنازعات کو جمع کرنے اور منسوخ کرنے اور اعداد و شمار کے انخلاء کو مربوط کرنے کے لئے گورننس ورک فلو۔
---

::: منظور شدہ ماخذ کو نوٹ کریں
یہ صفحہ `docs/source/sorafs/dispute_revocation_runbook.md` کی عکاسی کرتا ہے۔ اس بات کو یقینی بنائیں کہ اس وقت تک دو ورژن کو مطابقت پذیر رکھیں جب تک کہ پرانی اسفنکس دستاویزات کو کھینچ نہ لیا جائے۔
:::

## ہدف

یہ گائیڈ گورننس آپریٹرز کو SoraFS صلاحیت کے تنازعات کو پیش کرنے ، منسوخ کرنے کو مربوط کرنے ، اور اعداد و شمار سے خارج ہونے والی تکمیل کو یقینی بنانے کے ذریعے گورننس آپریٹرز کی رہنمائی کرتا ہے۔

## 1. واقعہ کی تشخیص

- ** ریلیز کی شرائط: ** ایس ایل اے کی خلاف ورزی (دستیابی/پور کی ناکامی) ، فالتو پن کی کمی ، یا بلنگ اختلاف کے لئے مانیٹر کریں۔
۔
- ** اسٹیک ہولڈرز کو مطلع کرنا: ** اسٹوریج ٹیم (فراہم کنندہ آپریشنز) ، گورننس کونسل (فیصلہ ادارہ) ، مشاہدہ (ڈیش بورڈ اپ ڈیٹ)۔

## 2. ثبوت پیکیج تیار کریں

1. خام نمونے جمع کریں (ٹیلی میٹری JSON ، CLI لاگز ، آڈیٹر نوٹ)۔
2. اسے ایک عزم آرکائو (جیسے ٹربال کی طرح) میں متحد کریں۔ اور ریکارڈ:
   -ڈیجسٹ بلیک 3-256 (`evidence_digest`)
   - میڈیا کی قسم (`application/zip` ، `application/jsonl` ، وغیرہ)
   - ہوسٹنگ یو آر آئی (آبجیکٹ اسٹوریج ، SoraFS پن ، یا اختتامی نقطہ Torii کے ذریعے دستیاب ہے)
3. پیکیج کو گورننس شواہد کلیکشن کنٹینر میں ایک وقتی تحریر تک رسائی کے ساتھ اسٹور کریں۔

## 3. تنازعہ جمع کروائیں

1. `sorafs_manifest_stub capacity dispute` کے لئے JSON تفصیلات بنائیں:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. سی ایل آئی چلائیں:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=ih58... \
     --private-key=ed25519:<key>
   ```

3. `dispute_summary.json` دیکھیں (چیک کی قسم ، ڈائجسٹ ثبوت ، اور ٹائم اسٹیمپ)۔
4. گورننس ٹرانزیکشن قطار کے ذریعے Torii `/v1/sorafs/capacity/dispute` پر JSON کی درخواست بھیجیں۔ ردعمل کی قیمت `dispute_id_hex` پر قبضہ ؛ یہ اس کے بعد منسوخی کے طریقہ کار اور آڈٹ رپورٹس کی تصدیق کرتا ہے۔

## 4. رہائی اور منسوخی

1. ** اجازت ونڈو: ** آنے والی منسوخی کے فراہم کنندہ کو مطلع کریں ؛ جب پالیسی کی اجازت ہو تو انسٹال ڈیٹا کو خالی کرنے کی اجازت دیں۔
2. ** `ProviderAdmissionRevocationV1` بنائیں: **
   - منظور شدہ وجہ کے ساتھ `sorafs_manifest_stub provider-admission revoke` استعمال کریں۔
   - دستخطوں کی تصدیق کریں اور ہضم منسوخ کریں۔
3. ** پوسٹ منسوخی: **
   - اپنی منسوخی کی درخواست کو Torii پر بھیجیں۔
   - اس بات کو یقینی بنائیں کہ آپ کے فراہم کنندہ کے اشتہارات کو مسدود کردیا گیا ہے (اعلی `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` کی توقع کریں)۔
4. ** اپنے ڈیش بورڈز کو اپ ڈیٹ کریں: ** فراہم کنندہ کو منسوخ کے طور پر نشان زد کریں ، تنازعہ کی شناخت کی نشاندہی کریں ، اور ثبوت کے پیکیج کو لنک کریں۔

## 5. پوسٹ حادثہ اور فالو اپ

- گورننس واقعہ ٹریکر میں ٹائم لائن ، بنیادی وجہ اور تدارک کی کارروائیوں کو ریکارڈ کریں۔
- معاوضے کا تعین کریں (شرطوں کے لئے کم کرنا ، فیسوں کے لئے کلاب بیکس ، اور کسٹمر معاوضہ)۔
- دستاویز کے اسباق سیکھے گئے۔ اگر ضروری ہو تو ایس ایل اے کی دہلیز یا نگرانی کے انتباہات کو اپ ڈیٹ کریں۔

## 6. Reference materials

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (تنازعات کا سیکشن)
- `docs/source/sorafs/provider_admission_policy.md` (منسوخی کا ورک فلو)
- مانیٹر بورڈ: `SoraFS / Capacity Providers`

## چیک لسٹ

- [] ثبوت کے پیکیج پر قبضہ کرلیا گیا ہے اور ہیش کا حساب کتاب کیا گیا ہے۔
- [] تنازعہ کے پے لوڈ کی مقامی طور پر تصدیق کی گئی ہے۔
- [] تنازعہ کا لین دین Torii پر قبول کیا گیا تھا۔
- [] منسوخی کو نافذ کیا گیا ہے (اگر منظور شدہ)۔
- [] ڈیش بورڈز/آپریشنل دستورالعمل کو اپ ڈیٹ کیا گیا ہے۔
- [] واقعے کے بعد گورننس بورڈ کو پیش کیا گیا۔