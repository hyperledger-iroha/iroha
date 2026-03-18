---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: تنازعہ کی بحالی کی کتاب
عنوان: SoraFS تنازعات اور رن بک کو منسوخ کرتا ہے
سائڈبار_لیبل: رن بک کے تنازعات اور منسوخ کردیئے گئے
تفصیل: SoraFS صلاحیت کے تنازعات ، کوآرڈینیٹ منسوخ کرنے ، اور اعداد و شمار کو خالی کرنے کے لئے گورننس کا بہاؤ۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/dispute_revocation_runbook.md` کی عکاسی کرتا ہے۔ جب تک میراثی اسفنکس دستاویزات ریٹائر نہیں ہوجاتی اس وقت تک دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

## مقصد

یہ رن بک گورننس آپریٹرز کو SoraFS صلاحیت کے تنازعات کو فائل کرنے ، منسوخ کرنے کو مربوط کرنے ، اور اس بات کو یقینی بناتا ہے کہ اعداد و شمار کی انخلاء کو طے شدہ طور پر مکمل کیا جائے۔

## 1. واقعے کا اندازہ کریں

- ** چالو کرنے کے حالات: ** ایس ایل اے کی خلاف ورزی (اپ ٹائم/پور کی ناکامی) ، نقل کی کمی یا بلنگ اختلاف کا پتہ لگانا۔
۔
- ** دلچسپی رکھنے والی جماعتوں کو مطلع کریں: ** اسٹوریج ٹیم (سپلائر آپریشنز) ، گورننس کونسل (فیصلہ سازی باڈی) ، مشاہدہ (ڈیش بورڈ اپ ڈیٹ)۔

## 2. ثبوت پیکیج تیار کریں

1. خام نمونے جمع کریں (JSON ٹیلی میٹری ، سی ایل آئی لاگ ، آڈٹ نوٹ)۔
2. کسی جینیاتی فائل کو معمول بنائیں (مثال کے طور پر ، ٹربال) ؛ رجسٹر:
   - ڈائجسٹ بلیک 3-256 (`evidence_digest`)
   - میڈیا کی قسم (`application/zip` ، `application/jsonl` ، وغیرہ)
   - ہوسٹنگ URI (آبجیکٹ اسٹوریج ، SoraFS کا پن یا اختتامی نقطہ Torii کے ذریعہ قابل رسائی)
3. ایک بار رسائی کے ساتھ گورننس شواہد کلیکشن بالٹی میں پیکیج کو محفوظ کریں۔

## 3. تنازعہ جمع کروائیں

1. `sorafs_manifest_stub capacity dispute` کے لئے JSON مخصوص بنائیں:

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
     --authority=i105... \
     --private-key=ed25519:<key>
   ```

3. `dispute_summary.json` چیک کریں (تصدیق کریں قسم ، ثبوت ڈائجسٹ اور ٹائم اسٹیمپ)۔
4. درخواست JSON کو گورننس ٹرانزیکشن قطار کے ذریعے Torii `/v1/sorafs/capacity/dispute` پر بھیجیں۔ ردعمل کی قیمت `dispute_id_hex` پر قبضہ کرتا ہے۔ اس کے بعد منسوخ کرنے کے اقدامات اور آڈٹ رپورٹس کو لنگر انداز کرتے ہیں۔

## 4۔ انخلا اور منسوخی

1. ** گریس ونڈو: ** آنے والی منسوخی کے فراہم کنندہ کو مطلع کرتا ہے۔ جب پالیسی کی اجازت دیتا ہے تو پن والے ڈیٹا کو انخلا کی اجازت دیتا ہے۔
2. ** `ProviderAdmissionRevocationV1` تیار کرتا ہے: **
   - منظور شدہ وجہ کے ساتھ `sorafs_manifest_stub provider-admission revoke` استعمال کریں۔
   - دستخطوں اور منسوخی ڈائجسٹ کی تصدیق کریں۔
3. ** منسوخی کو شائع کریں: **
   - منسوخ کرنے کی درخواست Torii پر بھیجیں۔
   - اس بات کو یقینی بناتا ہے کہ وینڈر اشتہارات کو مسدود کردیا گیا ہے (`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` میں اضافے کی توقع ہے)۔
4. ** ڈیش بورڈز کو اپ ڈیٹ کریں: ** سپلائر کو کالعدم قرار دیتے ہوئے نشان زد کریں ، تنازعہ کی شناخت کا حوالہ دیں اور ثبوت کے پیکیج کو لنک کریں۔

## 5. پوسٹ مارٹم اور فالو اپ- گورننس واقعہ ٹریکر میں ٹائم لائن ، بنیادی وجہ اور تدارک کے اقدامات کو ریکارڈ کریں۔
- بحالی کا تعین کرتا ہے (داؤ پر لگنے والا ، کمیشن کلاک بیکس ، کسٹمر کی واپسی)۔
- دستاویز سیکھنا ؛ اگر ضروری ہو تو ایس ایل اے کی دہلیز یا نگرانی کے انتباہات کو اپ ڈیٹ کرتا ہے۔

## 6. حوالہ مواد

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (تنازعات کا سیکشن)
- `docs/source/sorafs/provider_admission_policy.md` (منسوخی کا بہاؤ)
- مشاہدہ ڈیش بورڈ: `SoraFS / Capacity Providers`

## چیک لسٹ

- [] ثبوت پیکیج پر قبضہ اور ہیشڈ۔
- [] مقامی طور پر توثیق شدہ تنازعہ پے لوڈ۔
- [] Torii پر تنازعہ کا لین دین قبول کیا گیا۔
- [] منسوخی کو پھانسی دے دی گئی (اگر منظور شدہ)۔
- [] ڈیش بورڈز/رن بوکس اپ ڈیٹ۔
- [] پوسٹ مارٹم گورننس کونسل کو پیش کیا گیا۔