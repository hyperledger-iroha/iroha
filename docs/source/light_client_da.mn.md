---
lang: mn
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2025-12-29T18:16:35.975661+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Үйлчлүүлэгчийн мэдээллийн хүртээмжийн түүвэрлэлт

Light Client Sampling API нь баталгаажуулсан операторуудад мэдээлэл авах боломжийг олгодог
Нислэгийн блокод зориулсан Merkle-ийн баталгаажуулсан RBC бөөмийн дээж. Хөнгөн үйлчлүүлэгчид
санамсаргүй түүврийн хүсэлт гаргаж, буцаасан нотолгоог шалгаж болно
сурталчилсан chunk root, мөн өгөгдөлгүйгээр ашиглах боломжтой гэдэгт итгэх итгэлийг бий болгох
ачааг бүхэлд нь татаж байна.

## Төгсгөлийн цэг

```
POST /v2/sumeragi/rbc/sample
```

Төгсгөлийн цэг нь тохируулагдсан зүйлсийн аль нэгэнд тохирох `X-API-Token` толгойг шаарддаг.
Torii API токенууд. Хүсэлт нь нэмэлт хэмжээгээр хязгаарлагдмал бөгөөд өдөр бүр хамаарна
дуудлага хийгчийн байт төсөв; аль алиныг нь хэтрүүлбэл HTTP 429-ийг буцаана.

### Хүсэлтийн байгууллага

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – зургаан өнцөгт дэх зорилтот блок хэш.
* `height`, `view` – RBC сессийн таних тэмдэг.
* `count` – хүссэн дээжийн тоо (өгөгдмөл нь 1, тохиргоогоор хязгаарлагдсан).
* `seed` – давтагдах түүвэрлэлтэд зориулсан нэмэлт тодорхойлогч RNG үр.

### Хариу өгөх байгууллага

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

Түүврийн оруулга бүр нь бөөмийн индекс, ачааллын байт (hex), SHA-256 хуудас агуулсан
digest болон Merkle inclusion нотолгоог (заавал биш ах дүүсийг hex гэж кодлосон
мөр). Үйлчлүүлэгчид `chunk_root` талбарыг ашиглан нотолгоог шалгах боломжтой.

## Хязгаарлалт ба төсөв

* **Хүсэлт бүрт хамгийн их дээж** - `torii.rbc_sampling.max_samples_per_request`-ээр тохируулж болно.
* **Хүсэлт бүрт хамгийн их байт** – `torii.rbc_sampling.max_bytes_per_request` ашиглан хэрэгжүүлсэн.
* **Өдөр тутмын байт төсөв** – `torii.rbc_sampling.daily_byte_budget`-ээр дамжуулан залгасан хүн бүрийг хянадаг.
* **Үнийн хязгаарлалт** – зориулалтын токен хувин (`torii.rbc_sampling.rate_per_minute`) ашиглан хэрэгжүүлсэн.

Аливаа хязгаараас хэтэрсэн хүсэлт нь HTTP 429 (CapacityLimit) буцаана. Хэзээ хэсэг
дэлгүүр боломжгүй эсвэл сессийн төгсгөлийн цэгийн ачааллын байт дутуу байна
HTTP 404-г буцаана.

## SDK интеграци

### JavaScript

`@iroha/iroha-js` нь `ToriiClient.sampleRbcChunks` туслагчийг харуулах тул өгөгдөл
бэлэн байдлын баталгаажуулагч нь өөрийн дуудлагыг эргүүлэхгүйгээр эцсийн цэг рүү залгаж болно
логик. Туслагч нь зургаан өнцөгт ачааллыг баталгаажуулж, бүхэл тоог хэвийн болгож, буцаана
Дээрх хариултын схемийг тусгасан бичсэн объектууд:

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

Сервер алдаатай өгөгдлийг буцаах үед туслагч шидэж, JS-04-ийн паритад тусалдаг
туршилтууд нь Rust болон Python SDK-ийн зэрэгцээ регрессийг илрүүлдэг. Зэв
(`iroha_client::ToriiClient::sample_rbc_chunks`) болон Python
(`IrohaToriiClient.sample_rbc_chunks`) хөлөг онгоцны эквивалент туслахууд; алийг нь ч хэрэглэнэ
таны дээж авах хэрэгсэлтэй таарч байна.