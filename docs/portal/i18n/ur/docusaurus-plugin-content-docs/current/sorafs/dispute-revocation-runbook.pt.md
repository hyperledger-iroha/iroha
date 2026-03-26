---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: تنازعہ کی بحالی کی کتاب
عنوان: SoraFS تنازعہ اور منسوخ کرنے والی رن بک
سائڈبار_لیبل: تنازعات اور منسوخیاں رن بک
تفصیل: گورننس کا بہاؤ SoraFS صلاحیت کے تنازعات کو ریکارڈ کرنے کے لئے بہاؤ ، بحالی کو مربوط کرنے اور اعداد و شمار کو خالی کرنے کے لئے اعداد و شمار کو خالی کرنا۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/dispute_revocation_runbook.md` کی عکاسی کرتا ہے۔ جب تک میراثی اسفنکس دستاویزات ریٹائر نہیں ہوجاتی ہیں ، دونوں کاپیاں ہم وقت سازی رکھیں۔
:::

## مقصد

یہ رن بوک گورننس آپریٹرز کو SoraFS صلاحیت کے تنازعات کو کھولنے ، منسوخ کرنے کو مربوط کرنے ، اور اس بات کو یقینی بنانے میں رہنمائی کرتا ہے کہ اعداد و شمار کو انخلاء سے طے شدہ طور پر مکمل کیا گیا ہے۔

## 1۔ واقعے کا اندازہ لگائیں

- ** ٹرگر شرائط: ** ایس ایل اے کی خلاف ورزی (اپ ٹائم/پور کی ناکامی) ، نقل کی خسارہ یا بلنگ موڑ کا پتہ لگانا۔
- ** ٹیلی میٹری کی تصدیق کریں: ** فراہم کنندہ سے `/v1/sorafs/capacity/state` اور `/v1/sorafs/capacity/telemetry` کے اسنیپ شاٹس پر قبضہ کریں۔
- ** دلچسپی رکھنے والی جماعتوں کو مطلع کریں: ** اسٹوریج ٹیم (فراہم کنندہ آپریشنز) ، گورننس کونسل (فیصلہ سازی باڈی) ، مشاہدہ (ڈیش بورڈ اپ ڈیٹ)۔

## 2. ثبوت پیکیج تیار کریں

1. خام نمونے جمع کریں (JSON ٹیلی میٹری ، سی ایل آئی لاگ ، آڈٹ نوٹ)۔
2. ایک جینیاتی فائل میں معمول بنائیں (مثال کے طور پر ، ٹربال) ؛ رجسٹر:
   - ڈائجسٹ بلیک 3-256 (`evidence_digest`)
   - میڈیا کی قسم (`application/zip` ، `application/jsonl` ، وغیرہ)
   - ہوسٹنگ یو آر آئی (آبجیکٹ اسٹوریج ، SoraFS پن یا اختتامی نقطہ Torii کے ذریعے قابل رسائی)
3. گورننس شواہد کی بالٹی میں پیکیج کو تحریری طور پر ایک بار رسائی کے ساتھ اسٹور کریں۔

## 3. تنازعہ کو رجسٹر کریں

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
     --authority=soraカタカナ... \
     --private-key=ed25519:<key>
   ```

3. جائزہ `dispute_summary.json` (تصدیق کی قسم ، ثبوت اور ٹائم اسٹیمپ کی ڈائجسٹ)۔
4. گورننس ٹرانزیکشن قطار کے ذریعے JSON JSON کو Torii `/v1/sorafs/capacity/dispute` پر بھیجیں۔ ردعمل کی قیمت `dispute_id_hex` پر قبضہ کریں ؛ یہ بعد میں منسوخی کے اقدامات اور آڈٹ رپورٹس کو لنگر انداز کرتا ہے۔

## 4۔ انخلا اور منسوخی

1. ** مفت ونڈو: ** آسنن منسوخ کرنے والے فراہم کنندہ کو مطلع کریں۔ جب پالیسی کی اجازت ہو تو پن والے ڈیٹا کو انخلا کی اجازت دیں۔
2. ** `ProviderAdmissionRevocationV1` پیدا کریں: **
   - منظور شدہ وجہ کے ساتھ `sorafs_manifest_stub provider-admission revoke` استعمال کریں۔
   - دستخطوں اور منسوخی ڈائجسٹ کو چیک کریں۔
3. ** منسوخی کو شائع کریں: **
   - منسوخ کرنے کی درخواست Torii پر بھیجیں۔
   - یقینی بنائیں کہ فراہم کنندہ کے اشتہارات کو مسدود کردیا گیا ہے (`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` میں اضافے کی توقع ہے)۔
4. ** ڈیش بورڈز کو اپ ڈیٹ کریں: ** فراہم کنندہ کو کالعدم قرار دیں ، تنازعہ کی شناخت کا حوالہ دیں اور ثبوت کے پیکیج کو لنک کریں۔

## 5. پوسٹ مارٹم اور فالو اپ

- گورننس واقعہ ٹریکر میں ٹائم لائن ، بنیادی وجہ اور تدارک کے اقدامات کو ریکارڈ کریں۔
- رقم کی واپسی کا تعین کریں (اسٹیک سلیشنگ ، فیس کلاب بیکس ، صارفین کو رقم کی واپسی)۔
- دستاویز سیکھنا ؛ اگر ضروری ہو تو ایس ایل اے کی حدود یا نگرانی کے انتباہات کو اپ ڈیٹ کریں۔

## 6. حوالہ مواد- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (تنازعات کا سیکشن)
- `docs/source/sorafs/provider_admission_policy.md` (منسوخی کا بہاؤ)
- مشاہدہ ڈیش بورڈ: `SoraFS / Capacity Providers`

## چیک لسٹ

- [] ثبوت پیکیج پر قبضہ اور ہیشڈ۔
- [] تنازعہ پے لوڈ مقامی طور پر توثیق شدہ۔
- [] Torii پر تنازعہ کا لین دین قبول کیا گیا۔
- [] منسوخی کو پھانسی دے دی گئی (اگر منظور شدہ)۔
- [] تازہ کاری شدہ ڈیش بورڈز/رن بکس۔
- [] پوسٹ مارٹم گورننس کونسل میں دائر کیا گیا۔