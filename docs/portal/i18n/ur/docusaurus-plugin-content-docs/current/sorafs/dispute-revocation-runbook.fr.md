---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: تنازعہ کی بحالی کی کتاب
عنوان: تنازعات اور منسوخیاں رن بک SoraFS
سائڈبار_لیبل: تنازعات اور منسوخیاں رن بک
تفصیل: گورننس کا بہاؤ SoraFS صلاحیت کے تنازعات کو فائل کرنے ، اصلاحات کو مربوط کرنے ، اور اعداد و شمار کو طے شدہ طور پر خالی کرنے کے لئے۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/dispute_revocation_runbook.md` کی عکاسی کرتا ہے۔ جب تک میراثی اسفنکس دستاویزات ریٹائر نہیں ہوجاتی اس وقت تک دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

## مقصد

یہ رن بک SoraFS صلاحیت کے تنازعات کو تشکیل دینے ، منسوخ کرنے کو مربوط کرنے ، اور اعداد و شمار کے انخلا کو یقینی بنانے میں گورننس آپریٹرز کی رہنمائی کرتا ہے۔

## 1. واقعے کا اندازہ کریں

- ** ٹرگر شرائط: ** ایس ایل اے کی خلاف ورزی (دستیابی/پور کی ناکامی) ، نقل کی خسارہ یا بلنگ اختلاف کا پتہ لگانا۔
- ** ٹیلی میٹری کی تصدیق کریں: ** فراہم کنندہ کے لئے اسنیپ شاٹس `/v1/sorafs/capacity/state` اور `/v1/sorafs/capacity/telemetry` پر قبضہ کریں۔
- ** اسٹیک ہولڈرز کو مطلع کریں: ** اسٹوریج ٹیم (سپلائر آپریشنز) ، گورننس کونسل (فیصلہ سازی باڈی) ، مشاہدہ (ڈیش بورڈ اپ ڈیٹ)۔

## 2. ثبوت بنڈل تیار کریں

1. خام نمونے جمع کریں (ٹیلی میٹری JSON ، CLI لاگز ، آڈٹ نوٹ)۔
2. ایک عزم آرکائو میں معمول بنائیں (مثال کے طور پر ، ایک ٹربال) ؛ ریکارڈ:
   - ڈائجسٹ بلیک 3-256 (`evidence_digest`)
   - میڈیا کی قسم (`application/zip` ، `application/jsonl` ، وغیرہ)
   - ہوسٹنگ یو آر آئی (آبجیکٹ اسٹوریج ، پن SoraFS یا اختتامی نقطہ Torii کے ذریعے قابل رسائی)
3. گورننس شواہد کلیکشن بالٹی میں بنڈل کو لکھنے کے لئے ایک بار رسائی کے ساتھ اسٹور کریں۔

## 3. تنازعہ فائل کریں

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

2. سی ایل آئی لانچ کریں:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. `dispute_summary.json` چیک کریں (تصدیق کی قسم ، شواہد ڈائجسٹ ، ٹائم اسٹیمپس)۔
4. گورننس ٹرانزیکشن قطار کے ذریعے JSON JSON کو Torii `/v1/sorafs/capacity/dispute` پر جمع کروائیں۔ ردعمل کی قیمت `dispute_id_hex` پر قبضہ کریں ؛ یہ بعد میں منسوخی کے اقدامات اور آڈٹ رپورٹس کو لنگر انداز کرتا ہے۔

## 4۔ انخلا اور منسوخی

1. ** گریس ونڈو: ** آنے والی منسوخی کے فراہم کنندہ کو مطلع کریں ؛ جب پالیسی کی اجازت ہو تو پن والے ڈیٹا کو انخلا کی اجازت دیں۔
2. ** `ProviderAdmissionRevocationV1` پیدا کریں: **
   - منظور شدہ وجہ کے ساتھ `sorafs_manifest_stub provider-admission revoke` استعمال کریں۔
   - دستخطوں اور منسوخی ڈائجسٹ کو چیک کریں۔
3. ** منسوخی کو شائع کریں: **
   - منسوخی کی درخواست Torii پر جمع کروائیں۔
   - اس بات کو یقینی بنائیں کہ فراہم کنندہ کے اشتہارات کو مسدود کردیا گیا ہے (`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` کے اضافے کا انتظار کریں)۔
4. ** ڈیش بورڈز کو اپ ڈیٹ کریں: ** سپلائر کو منسوخ کے طور پر نشان زد کریں ، تنازعہ کی شناخت کا حوالہ دیں اور ثبوت کے بنڈل کو جوڑیں۔

## 5. پوسٹ مارٹم اور فالو اپ- گورننس واقعہ ٹریکر میں ٹائم لائن ، جڑ کی وجہ اور تدارک کی کارروائیوں کو ریکارڈ کریں۔
- بحالی کا تعین کریں (داؤ پر لگا ہوا ، فیس کلاو بیکس ، کسٹمر معاوضے)۔
- اسباق کی دستاویز کریں ؛ اگر ضروری ہو تو ایس ایل اے کی دہلیز یا نگرانی کے انتباہات کو اپ ڈیٹ کریں۔

## 6. حوالہ دستاویزات

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (تنازعات کا سیکشن)
- `docs/source/sorafs/provider_admission_policy.md` (منسوخی کا ورک فلو)
- مشاہدہ ڈیش بورڈ: `SoraFS / Capacity Providers`

## چیک لسٹ

- [] ثبوت کے بنڈل پکڑے گئے اور کٹی ہوئی۔
- [] تنازعہ پے لوڈ مقامی طور پر توثیق شدہ۔
- [] تنازعات کا تصفیہ Torii قبول کیا گیا۔
- [] منسوخی کو پھانسی دے دی گئی (اگر منظور شدہ)۔
- [] ڈیش بورڈز/رن بوکس اپ ڈیٹ۔
- [] پوسٹ مارٹم گورننس بورڈ میں دائر کیا گیا۔