---
lang: uz
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2025-12-29T18:16:35.975661+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Engil mijoz maʼlumotlari mavjudligi namunasi

Light Client Sampling API autentifikatsiya qilingan operatorlarga olish imkonini beradi
Parvoz bloki uchun Merkle tomonidan tasdiqlangan RBC parcha namunalari. Engil mijozlar
tasodifiy tanlab olish so'rovlarini berishi, qaytarilgan dalillarni tekshirishi mumkin
e'lon qilingan parcha ildizi va ma'lumotlarsiz mavjud bo'lishiga ishonch hosil qiling
butun yukni olish.

## Oxirgi nuqta

```
POST /v1/sumeragi/rbc/sample
```

Yakuniy nuqta sozlanganlardan biriga mos keladigan `X-API-Token` sarlavhasini talab qiladi.
Torii API tokenlari. So'rovlar qo'shimcha stavka bilan cheklangan va kunlik
har bir qo'ng'iroq qiluvchi bayt byudjeti; har biridan oshib ketish HTTP 429 ni qaytaradi.

### So'rov organi

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – olti burchakli maqsadli blok xeshi.
* `height`, `view` - RBC seansi uchun identifikatsiya qiluvchi kortej.
* `count` – kerakli namunalar soni (standart 1 ga, konfiguratsiya bilan chegaralangan).
* `seed` - takrorlanadigan namuna olish uchun ixtiyoriy deterministik RNG urug'i.

### Javob organi

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

Har bir namuna yozuvida bo'lak indeksi, foydali yuk baytlari (oltilik), SHA-256 varaqlari mavjud
digest va Merkle inklyuziya isboti (ixtiyoriy birodarlar bilan hex sifatida kodlangan
satrlar). Mijozlar dalillarni `chunk_root` maydonidan foydalanib tekshirishlari mumkin.

## Limitlar va byudjetlar

* **So‘rov bo‘yicha maksimal namunalar** – `torii.rbc_sampling.max_samples_per_request` orqali sozlanishi.
* **Har bir soʻrov uchun maksimal bayt** – `torii.rbc_sampling.max_bytes_per_request` yordamida amalga oshiriladi.
* **Kundalik bayt byudjeti** - `torii.rbc_sampling.daily_byte_budget` orqali har bir qo'ng'iroq qiluvchiga kuzatiladi.
* **Tarifni cheklash** – maxsus token paqir yordamida amalga oshiriladi (`torii.rbc_sampling.rate_per_minute`).

Har qanday chegaradan oshib ketgan soʻrovlar HTTP 429 (CapacityLimit) ni qaytaradi. Bo'lak bo'lganda
do'kon mavjud emas yoki sessiya yakuniy nuqtada foydali yuk baytlari etishmayapti
HTTP 404 ni qaytaradi.

## SDK integratsiyasi

### JavaScript

`@iroha/iroha-js` ma'lumotlar uchun `ToriiClient.sampleRbcChunks` yordamchisini ochib beradi
mavjudligini tekshiruvchilar so'nggi nuqtaga o'z yuklarini o'tkazmasdan qo'ng'iroq qilishlari mumkin
mantiq. Yordamchi olti burchakli yuklarni tasdiqlaydi, butun sonlarni normallashtiradi va qaytaradi
Yuqoridagi javob sxemasini aks ettiruvchi terilgan ob'ektlar:

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

Server noto'g'ri shakllangan ma'lumotlarni qaytarganda, yordamchi JS-04 paritetiga yordam beradi
testlar Rust va Python SDK-lar bilan bir qatorda regressiyalarni aniqlaydi. Zang
(`iroha_client::ToriiClient::sample_rbc_chunks`) va Python
(`IrohaToriiClient.sample_rbc_chunks`) kema ekvivalent yordamchilari; qaysi birini ishlating
namuna olish jabduqlaringizga mos keladi.