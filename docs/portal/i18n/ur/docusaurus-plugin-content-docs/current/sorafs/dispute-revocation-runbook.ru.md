---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: تنازعہ کی بحالی کی کتاب
عنوان: تنازعات اور جائزوں کی رن بک SoraFS
سائڈبار_لیبل: تنازعات اور جائزوں کی رن بک
تفصیل: SoraFS صلاحیت کے تنازعات ، جائزوں کو مربوط کرنے ، اور اعداد و شمار کے اعداد و شمار کو انخلا کرنے کے لئے گورننس کا عمل۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/dispute_revocation_runbook.md` کی عکاسی کرتا ہے۔ فرسودہ اسفنکس دستاویزات ریٹائر ہونے تک دونوں کاپیاں ہم آہنگی کو برقرار رکھیں۔
:::

## مقصد

یہ رن بک صلاحیت کے تنازعات SoraFS فائل کرنے ، یادوں کو مربوط کرنے ، اور اعداد و شمار کے انخلا کو یقینی بنانے کے ذریعے گورننس آپریٹرز کی رہنمائی کرتا ہے۔

## 1۔ واقعے کا اندازہ لگائیں

- ** ٹرگر شرائط: ** ایس ایل اے کی خلاف ورزی (اپ ٹائم/پور کی ناکامی) ، نقل کی کمی یا بلنگ اختلاف کا پتہ لگانا۔
- ** ٹیلی میٹری کی تصدیق کریں: ** سنیپ شاٹس `/v2/sorafs/capacity/state` اور `/v2/sorafs/capacity/telemetry` کو فراہم کنندہ سے وابستہ کریں۔
- ** اسٹیک ہولڈرز کو مطلع کریں: ** اسٹوریج ٹیم (فراہم کنندہ آپریشنز) ، گورننس کونسل (فیصلہ سازی ادارہ) ، مشاہدہ (ڈیش بورڈ اپ ڈیٹ)۔

## 2. ثبوت کا ایک پیکیج تیار کریں

1. خام نمونے جمع کریں (ٹیلی میٹری JSON ، CLI لاگز ، آڈیٹر نوٹ)۔
2. ایک جینیاتی محفوظ شدہ دستاویزات کو معمول بنائیں (مثال کے طور پر ، ٹربال) ؛ ارتکاب:
   - ڈائجسٹ بلیک 3-256 (`evidence_digest`)
   - میڈیا کی قسم (`application/zip` ، `application/jsonl` ، وغیرہ)
   - اسٹوریج URI (آبجیکٹ اسٹوریج ، SoraFS پن یا اختتامی نقطہ ، Torii کے ذریعے قابل رسائی)
3. گورننس ثبوت کی بالٹی میں پیکیج کو تحریری طور پر ایک بار رسائی کے ساتھ محفوظ کریں۔

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
     --authority=i105... \
     --private-key=ed25519:<key>
   ```

3. `dispute_summary.json` چیک کریں (تصدیق کریں قسم ، ثبوت ڈائجسٹ اور ٹائم اسٹیمپ)۔
4. گورننس ٹرانزیکشن قطار کے ذریعے Torii `/v2/sorafs/capacity/dispute` پر JSON کی درخواست بھیجیں۔ ردعمل کی قیمت `dispute_id_hex` پر قبضہ کریں ؛ یہ اس کے بعد کے اعمال اور آڈٹ رپورٹس کو لنگر انداز کرتا ہے۔

## 4۔ انخلا اور یاد کرنا

1. ** بینیفٹ ونڈو: ** آئندہ یاد آنے والے فراہم کنندہ کو مطلع کریں ؛ جب پالیسی کی اجازت ہو تو پن والے ڈیٹا کو انخلا کی اجازت دیں۔
2. ** `ProviderAdmissionRevocationV1` پیدا کریں: **
   - کسی منظور شدہ وجہ کے ساتھ `sorafs_manifest_stub provider-admission revoke` استعمال کریں۔
   - جائزہ کے دستخط اور ڈائجسٹ چیک کریں۔
3. ** ایک جائزہ شائع کریں: **
   - Torii پر جائزہ لینے کی درخواست جمع کروائیں۔
   - اس بات کو یقینی بنائیں کہ فراہم کنندہ کے اشتہارات کو مسدود کردیا گیا ہے (`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` میں اضافے کی توقع کریں)۔
4. ** اپنے ڈیش بورڈز کو اپ ڈیٹ کریں: ** فراہم کنندہ کو واپس لے کر نشان زد کریں ، تنازعہ کی شناخت کی نشاندہی کریں اور ثبوت کے پیکیج سے لنک منسلک کریں۔

## 5. پوسٹ مارٹم اور اس کے بعد کے اقدامات

- گورننس واقعہ ٹریکر میں ٹائم لائن ، بنیادی وجہ اور تدارک کے اقدامات کو ریکارڈ کریں۔
- بحالی کی وضاحت کریں (داؤ کو کم کرنا ، کمیشنوں کے پنجوں کی بیکس ، مؤکلوں کو رقم کی واپسی)۔
- دستاویز کے نتائج ؛ ضرورت کے مطابق ایس ایل اے کی دہلیز یا نگرانی کے انتباہات کو اپ ڈیٹ کریں۔

## 6. حوالہ مواد

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (تنازعات کا سیکشن)
- `docs/source/sorafs/provider_admission_policy.md` (ورک فلو جائزہ)
- مشاہدہ ڈیش بورڈ: `SoraFS / Capacity Providers`

## چیک لسٹ- [] ثبوت کا پیکیج جمع اور ہیش کیا جاتا ہے۔
- [] تنازعہ کا پے لوڈ مقامی طور پر توثیق کیا جاتا ہے۔
- [] Torii-dispute ٹرانزیکشن قبول کیا گیا۔
- [] جائزہ مکمل (اگر منظور شدہ ہے)۔
- [] ڈیش بورڈز/رن بوکس اپ ڈیٹ۔
- [] پوسٹ مارٹم گورننس کونسل میں رجسٹرڈ ہے۔