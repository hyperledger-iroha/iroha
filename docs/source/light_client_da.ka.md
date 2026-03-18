---
lang: ka
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2025-12-29T18:16:35.975661+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Light Client მონაცემთა ხელმისაწვდომობის ნიმუში

Light Client Sampling API საშუალებას აძლევს ავთენტიფიცირებულ ოპერატორებს აღადგინონ
Merkle-ის მიერ დამოწმებული RBC ბლოკის ნიმუშები ფრენის დროს ბლოკისთვის. მსუბუქი კლიენტები
შეუძლია გასცეს შემთხვევითი შერჩევის მოთხოვნები, გადაამოწმოს დაბრუნებული მტკიცებულებები მის წინააღმდეგ
რეკლამირებულია chunk root და შეიქმენით ნდობა, რომ მონაცემები ხელმისაწვდომია მის გარეშე
მთელი დატვირთვის მოტანა.

## დასასრული

```
POST /v1/sumeragi/rbc/sample
```

საბოლოო წერტილი მოითხოვს `X-API-Token` სათაურს, რომელიც შეესაბამება ერთ-ერთ კონფიგურაციას
Torii API ტოკენები. მოთხოვნები დამატებით ტარიფებით შეზღუდულია და ექვემდებარება ყოველდღიურად
თითო აბონენტის ბაიტის ბიუჯეტი; რომელიმეს გადაჭარბება აბრუნებს HTTP 429-ს.

### მოთხოვნის ორგანო

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – სამიზნე ბლოკის ჰეში თექვსმეტობით.
* `height`, `view` - იდენტიფიცირება ტუპი RBC სესიისთვის.
* `count` – ნიმუშების სასურველი რაოდენობა (ნაგულისხმევი 1-მდე, კონფიგურაციის მიხედვით დახურული).
* `seed` - არჩევითი დეტერმინისტული RNG თესლი განმეორებადი სინჯის აღებისთვის.

### საპასუხო ორგანო

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

თითოეული ნიმუშის ჩანაწერი შეიცავს ბლოკის ინდექსს, დატვირთვის ბაიტებს (თექვსმეტობით), SHA-256 ფოთოლს
დაიჯესტი და Merkle-ის ჩართვის მტკიცებულება (სურვილისამებრ და-ძმებით დაშიფრული თექვსმეტობით
სიმები). კლიენტებს შეუძლიათ დაადასტურონ მტკიცებულებები `chunk_root` ველის გამოყენებით.

## ლიმიტები და ბიუჯეტი

* **მაქსიმალური ნიმუშები თითო მოთხოვნაზე** – კონფიგურირებადია `torii.rbc_sampling.max_samples_per_request`-ის საშუალებით.
* **მაქსიმალური ბაიტი თითო მოთხოვნაზე** – განხორციელებულია `torii.rbc_sampling.max_bytes_per_request` გამოყენებით.
* **დღიური ბაიტის ბიუჯეტი** – თვალყურის დევნება თითო აბონენტზე `torii.rbc_sampling.daily_byte_budget`-ის მეშვეობით.
* **განაკვეთის შეზღუდვა** – განხორციელებულია სპეციალური ჟეტონების თაიგულის გამოყენებით (`torii.rbc_sampling.rate_per_minute`).

მოთხოვნები, რომლებიც აღემატება ნებისმიერ ლიმიტს, აბრუნებს HTTP 429 (CapacityLimit). როცა ნაჭერი
მაღაზია მიუწვდომელია ან სესიას აკლია დასასრული დატვირთვის ბაიტი
აბრუნებს HTTP 404.

## SDK ინტეგრაცია

### JavaScript

`@iroha/iroha-js` ავლენს `ToriiClient.sampleRbcChunks` დამხმარეს
ხელმისაწვდომობის შემმოწმებლებს შეუძლიათ გამოიძახონ საბოლოო წერტილი საკუთარი მოტანის გარეშე
ლოგიკა. დამხმარე ამოწმებს თექვსმეტობით დატვირთვას, ახდენს მთელი რიცხვების ნორმალიზებას და აბრუნებს
აკრეფილი ობიექტები, რომლებიც ასახავს ზემოთ მოცემული პასუხის სქემას:

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

დამხმარე ისვრის, როდესაც სერვერი აბრუნებს არასწორ ფორმატულ მონაცემებს, რაც ეხმარება JS-04 პარიტეტს
ტესტები აღმოაჩენს რეგრესიებს Rust და Python SDK-ებთან ერთად. ჟანგი
(`iroha_client::ToriiClient::sample_rbc_chunks`) და პითონი
(`IrohaToriiClient.sample_rbc_chunks`) გემის ეკვივალენტური დამხმარეები; გამოიყენეთ რომელი
შეესაბამება თქვენს სინჯის აღკაზმულობას.