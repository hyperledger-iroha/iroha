---
lang: az
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2025-12-29T18:16:35.975661+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Yüngül Müştəri Məlumatının Əlçatımlılığı Nümunəsi

The Light Client Sampling API allows authenticated operators to retrieve
Merkle-authenticated RBC chunk samples for an in-flight block. Yüngül müştərilər
can issue random sampling requests, verify the returned proofs against the
advertised chunk root, and build confidence that data is available without
bütün faydalı yükün alınması.

## Son nöqtə

```
POST /v1/sumeragi/rbc/sample
```

The endpoint requires an `X-API-Token` header matching one of the configured
Torii API tokenləri. Requests are additionally rate-limited and subject to a daily
zəng edənə görə bayt büdcəsi; hər ikisini aşan HTTP 429-u qaytarır.

### Sorğu orqanı

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – hex-də hədəf blok hash.
* `height`, `view` – RBC sessiyası üçün identifikasiya dəsti.
* `count` – istədiyiniz nümunə sayı (defolt olaraq 1, konfiqurasiya ilə məhdudlaşdırılır).
* `seed` – təkrarlana bilən seçmə üçün isteğe bağlı deterministik RNG toxumu.

### Cavab orqanı

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

Each sample entry contains the chunk index, payload bytes (hex), SHA-256 leaf
həzm və Merkle daxil edilmə sübutu (hex kimi kodlanmış isteğe bağlı bacı-qardaşlarla
simlər). Müştərilər `chunk_root` sahəsindən istifadə edərək sübutları yoxlaya bilərlər.

## Limitlər və Büdcələr

* **Max samples per request** – configurable via `torii.rbc_sampling.max_samples_per_request`.
* **Sorğu üzrə maksimum bayt** – `torii.rbc_sampling.max_bytes_per_request` istifadə edərək tətbiq edilir.
* **Gündəlik bayt büdcə** – `torii.rbc_sampling.daily_byte_budget` vasitəsilə hər zəng edənə görə izlənilir.
* **Məhdud dərəcəsi** – xüsusi nişan qutusu (`torii.rbc_sampling.rate_per_minute`) istifadə edərək tətbiq edilir.

İstənilən limiti aşan sorğular HTTP 429 (CapacityLimit) qaytarır. Parça olanda
mağaza əlçatan deyil və ya sessiyada son nöqtədə faydalı yük baytları yoxdur
HTTP 404 qaytarır.

## SDK inteqrasiyası

### JavaScript

`@iroha/iroha-js`, `ToriiClient.sampleRbcChunks` köməkçisini ifşa edir, beləliklə data
əlçatanlığı yoxlayanlar son nöqtəyə öz əldə etmələrini göndərmədən zəng edə bilərlər
məntiq. Köməkçi hex faydalı yükləri təsdiqləyir, tam ədədləri normallaşdırır və qaytarır
yuxarıdakı cavab sxemini əks etdirən tipli obyektlər:

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

Köməkçi server səhv məlumatı qaytardıqda JS-04 paritetinə kömək edir
testlər Rust və Python SDK-ları ilə yanaşı reqressiyaları aşkar edir. Pas
(`iroha_client::ToriiClient::sample_rbc_chunks`) və Python
(`IrohaToriiClient.sample_rbc_chunks`) gəmi ekvivalent köməkçiləri; hansısa istifadə edin
seçmə kəmərinizə uyğun gəlir.