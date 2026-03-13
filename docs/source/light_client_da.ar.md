---
lang: ar
direction: rtl
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2026-01-03T18:07:57.770085+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# أخذ عينات توفر بيانات العميل الخفيف

تسمح واجهة برمجة تطبيقات Light Client Sampling للمشغلين المعتمدين بالاسترداد
عينات من قطع كرات الدم الحمراء المصادق عليها من Merkle لكتلة على متن الطائرة. عملاء خفيفين
يمكن إصدار طلبات أخذ عينات عشوائية، والتحقق من الأدلة التي تم إرجاعها مقابل
جذر القطعة المعلن عنه، وبناء الثقة بأن البيانات متاحة بدونها
جلب الحمولة بأكملها.

## نقطة النهاية

```
POST /v2/sumeragi/rbc/sample
```

تتطلب نقطة النهاية رأس `X-API-Token` المطابق لواحد من العناصر التي تم تكوينها
رموز Torii API. بالإضافة إلى ذلك، تكون الطلبات محدودة السعر وتخضع يوميًا
ميزانية البايت لكل متصل؛ يتجاوز إما إرجاع HTTP 429.

### نص الطلب

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` - تجزئة الكتلة المستهدفة بالنظام الست عشري.
* `height`، `view` - تحديد الصف لجلسة RBC.
* `count` – العدد المطلوب من العينات (الإعدادات الافتراضية هي 1، مع تحديد الحد حسب التكوين).
* `seed` – بذور RNG الحتمية الاختيارية لأخذ العينات القابلة للتكرار.

### جسم الاستجابة

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

يحتوي كل إدخال عينة على فهرس القطعة، بايتات الحمولة (ست عشرية)، ورقة SHA-256
ملخص، وإثبات إدراج Merkle (مع الأشقاء الاختياريين المشفرين كـ hex
سلاسل). يمكن للعملاء التحقق من الأدلة باستخدام الحقل `chunk_root`.

## الحدود والميزانيات

* **الحد الأقصى للعينات لكل طلب** - يمكن تكوينه عبر `torii.rbc_sampling.max_samples_per_request`.
* **الحد الأقصى للبايت لكل طلب** - يتم فرضه باستخدام `torii.rbc_sampling.max_bytes_per_request`.
* **ميزانية البايت اليومية** - يتم تتبعها لكل متصل من خلال `torii.rbc_sampling.daily_byte_budget`.
* **تحديد السعر** - يتم فرضه باستخدام مجموعة رمزية مخصصة (`torii.rbc_sampling.rate_per_minute`).

الطلبات التي تتجاوز أي حد تُرجع HTTP 429 (CapacityLimit). عندما القطعة
المتجر غير متاح أو أن الجلسة تفتقد الحمولة النافعة لنقطة النهاية
إرجاع HTTP 404.

## تكامل SDK

###جافا سكريبت

يعرض `@iroha/iroha-js` المساعد `ToriiClient.sampleRbcChunks` لذلك البيانات
يمكن لوحدات التحقق من التوفر الاتصال بنقطة النهاية دون إجراء عملية الجلب الخاصة بها
المنطق. يقوم المساعد بالتحقق من صحة الحمولات السداسية وتطبيع الأعداد الصحيحة وإرجاعها
الكائنات المكتوبة التي تعكس مخطط الاستجابة أعلاه:

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

يقوم المساعد بالرمي عندما يقوم الخادم بإرجاع بيانات مشوهة، مما يساعد على تكافؤ JS-04
تكتشف الاختبارات الانحدارات جنبًا إلى جنب مع Rust وPython SDK. الصدأ
(`iroha_client::ToriiClient::sample_rbc_chunks`) وبايثون
(`IrohaToriiClient.sample_rbc_chunks`) مساعدات السفينة المكافئة؛ استخدام أيهما
يطابق تسخير العينات الخاصة بك.