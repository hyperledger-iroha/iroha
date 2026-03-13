---
lang: kk
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2025-12-29T18:16:35.975661+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Жеңіл Клиент деректерінің қолжетімділігі үлгісі

Light Client Sampling API аутентификацияланған операторларға шығарып алуға мүмкіндік береді
Ұшу кезіндегі блокқа арналған Merkle аутентификацияланған қызыл қан клеткаларының үлгілері. Жеңіл клиенттер
кездейсоқ іріктеу сұрауларын бере алады, қайтарылған дәлелдемелерді тексере алады
жарнамаланған кесінді түбірі және деректер онсыз қолжетімді болатынына сенімділік
бүкіл пайдалы жүктемені алу.

## Соңғы нүкте

```
POST /v2/sumeragi/rbc/sample
```

Соңғы нүкте конфигурацияланғандардың біріне сәйкес келетін `X-API-Token` тақырыбын қажет етеді.
Torii API токендері. Сұраныстар қосымша мөлшерлемемен шектеледі және күнделікті болады
қоңырау шалушы байт бюджеті; біреуінен асып кету HTTP 429 қайтарады.

### Сұраныс органы

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – он алтылықтағы мақсатты блок хэші.
* `height`, `view` – RBC сеансы үшін сәйкестендіргіш кортеж.
* `count` – үлгілердің қажетті саны (әдепкі бойынша 1, конфигурациямен шектелген).
* `seed` – қайталанатын сынама алу үшін қосымша детерминирленген RNG тұқымы.

### Жауап беру органы

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

Әрбір үлгі жазбасы бөлік индексін, пайдалы жүк байттарын (он алтылық), SHA-256 парағын қамтиды
дайджест және Merkle қосу дәлелі (он алтылық ретінде кодталған қосымша бауырлары бар
жолдар). Клиенттер дәлелдемелерді `chunk_root` өрісін пайдаланып тексере алады.

## Лимиттер мен бюджеттер

* **Сұраныс бойынша максималды үлгілер** – `torii.rbc_sampling.max_samples_per_request` арқылы конфигурациялауға болады.
* **Сұраныс үшін максималды байт** – `torii.rbc_sampling.max_bytes_per_request` арқылы күшіне енеді.
* **Күнделікті байт бюджет** – `torii.rbc_sampling.daily_byte_budget` арқылы бір қоңырау шалушы бақыланады.
* **Тарифті шектеу** – арнайы белгі шелегі (`torii.rbc_sampling.rate_per_minute`) арқылы күшіне енеді.

Кез келген шектен асатын сұраулар HTTP 429 (CapacityLimit) қайтарады. Кесек болғанда
дүкен қолжетімсіз немесе сеанста соңғы нүкте пайдалы жүк байттары жоқ
HTTP 404 қайтарады.

## SDK интеграциясы

### JavaScript

`@iroha/iroha-js` деректер үшін `ToriiClient.sampleRbcChunks` көмекшісін көрсетеді
қол жетімділікті тексерушілер соңғы нүктеге өздерінің алуды жылжытпай қоңырау шала алады
логика. Көмекші он алтылық пайдалы жүктемелерді тексереді, бүтін сандарды қалыпқа келтіреді және қайтарады
жоғарыдағы жауап схемасын көрсететін терілген нысандар:

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

Көмекші сервер JS-04 паритетіне көмектесе отырып, қате пішінделген деректерді қайтарғанда шығарады
сынақтар Rust және Python SDK-мен қатар регрессияларды анықтайды. Тот
(`iroha_client::ToriiClient::sample_rbc_chunks`) және Python
(`IrohaToriiClient.sample_rbc_chunks`) кеменің эквивалентті көмекшілері; қайсысы болса да пайдаланыңыз
сынама алу жиегіңізге сәйкес келеді.