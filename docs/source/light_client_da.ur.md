---
lang: ur
direction: rtl
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2026-01-03T18:07:57.770085+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# لائٹ کلائنٹ ڈیٹا کی دستیابی کا نمونہ

لائٹ کلائنٹ کے نمونے لینے والا API مستند آپریٹرز کو بازیافت کرنے کی اجازت دیتا ہے
ایک پرواز کے بلاک کے لئے مرکل سے تصدیق شدہ آر بی سی کے نمونے۔ ہلکے کلائنٹ
نمونے لینے کی بے ترتیب درخواستیں جاری کرسکتے ہیں ، اس کے خلاف واپس ہونے والے ثبوتوں کی تصدیق کریں
اشتہاری حصہ جڑ ، اور اعتماد پیدا کریں کہ ڈیٹا بغیر دستیاب ہے
پورا پے لوڈ لانا۔

## اختتامی نقطہ

```
POST /v1/sumeragi/rbc/sample
```

اختتامی نقطہ کے لئے ایک `X-API-Token` ہیڈر کی ضرورت ہوتی ہے جس میں سے ایک تشکیل شدہ ایک سے ملاپ ہوتا ہے
Torii API ٹوکن۔ درخواستیں اضافی طور پر شرح محدود اور روزانہ کے تابع ہوتی ہیں
فی کالر بائٹ بجٹ ؛ یا تو سے زیادہ واپسی HTTP 429۔

### درخواست باڈی

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` - ہیکس میں ہدف بلاک ہیش۔
* `height` ، `view` - RBC سیشن کے لئے ٹوپل کی شناخت کرنا۔
* `count` - نمونے کی مطلوبہ تعداد (1 سے پہلے سے طے شدہ ، ترتیب کے ذریعہ تیار کردہ)۔
* `seed` - تولیدی نمونے لینے کے لئے اختیاری عزم RNG بیج۔

### رسپانس باڈی

```json
{
  "block_hash": "…",
  "height": 42,
  "view": 0,
  "total_chunks": 128,
  "chunk_root": "…",
  "payload_hash": "…",
  "samples": [
    {
      "index": 7,
      "chunk_hex": "…",
      "digest_hex": "…",
      "proof": {
        "leaf_index": 7,
        "depth": 8,
        "audit_path": ["…", null, "…"]
      }
    }
  ]
}
```

ہر نمونے میں اندراج میں CHUNK انڈیکس ، پے لوڈ بائٹس (ہیکس) ، SHA-256 لیف ہوتا ہے
ڈائجسٹ ، اور ایک مرکل شامل کرنے کا ثبوت (اختیاری بہن بھائیوں کے ساتھ ہیکس کے طور پر انکوڈ کیا جاتا ہے
تار)۔ کلائنٹ `chunk_root` فیلڈ کا استعمال کرتے ہوئے ثبوتوں کی تصدیق کرسکتے ہیں۔

## حدود اور بجٹ

*** زیادہ سے زیادہ نمونے فی درخواست ** - `torii.rbc_sampling.max_samples_per_request` کے ذریعے تشکیل کے قابل۔
*** زیادہ سے زیادہ بائٹس فی درخواست ** - `torii.rbc_sampling.max_bytes_per_request` کا استعمال کرتے ہوئے نافذ کیا گیا۔
*** ڈیلی بائٹ بجٹ ** - `torii.rbc_sampling.daily_byte_budget` کے ذریعے فی کالر کا سراغ لگایا گیا۔
*** شرح کو محدود کرنا ** - ایک سرشار ٹوکن بالٹی (`torii.rbc_sampling.rate_per_minute`) کا استعمال کرتے ہوئے نافذ کیا گیا۔

کسی بھی حد سے زیادہ واپسی HTTP 429 (گنجائش لیمٹ) سے زیادہ درخواستیں۔ جب حصہ
اسٹور دستیاب نہیں ہے یا سیشن میں پے لوڈ بائٹس کا اختتامی نقطہ غائب ہے
HTTP 404 لوٹاتا ہے۔

## SDK انضمام

### جاوا اسکرپٹ

`@iroha/iroha-js` `ToriiClient.sampleRbcChunks` مددگار کو بے نقاب کرتا ہے لہذا ڈیٹا
دستیابی کی تصدیق کرنے والے اپنے بازیافت کو رول کیے بغیر اختتامی نقطہ پر کال کرسکتے ہیں
منطق مددگار ہیکس پے لوڈز کی توثیق کرتا ہے ، عددیوں کو معمول بناتا ہے ، اور واپسی
ٹائپ شدہ آبجیکٹ جو مذکورہ بالا رسپانس اسکیما کو آئینہ دار کرتے ہیں:

```js
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL, {
  apiToken: process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks({
  blockHash: "3d...ff",
  height: 42,
  view: 0,
  count: 3,
  seed: Date.now(),
});

if (!sample) {
  throw new Error("RBC session is not available yet");
}

for (const { digestHex, proof } of sample.samples) {
  verifyMerklizedChunk(sample.chunkRoot, digestHex, proof);
}
```

جب سرور خراب ڈیٹا لوٹاتا ہے تو ، JS-04 برابری میں مدد کرتا ہے جب سرور پھینک دیتا ہے
ٹیسٹ زنگ اور ازگر ایس ڈی کے کے ساتھ ساتھ رجعت پسندوں کا پتہ لگاتے ہیں۔ زنگ
(`iroha_client::ToriiClient::sample_rbc_chunks`) اور ازگر
(`IrohaToriiClient.sample_rbc_chunks`) جہاز کے مساوی مددگار ؛ جو بھی استعمال کریں
آپ کے نمونے لینے کے استعمال سے میل کھاتا ہے۔