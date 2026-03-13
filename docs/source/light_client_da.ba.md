---
lang: ba
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2025-12-29T18:16:35.975661+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Яҡтылыҡ клиент мәғлүмәттәре доступность үлсәү

Яҡтылыҡ клиент өлгөһө API аутентификацияланған операторҙар алыу мөмкинлеге бирә .
Меркл-аутентированный РБК өлөшө өлгөләре өсөн осоу блок. Еңел клиенттар
осраҡлы үлсәү үтенестәрен сығара ала, ҡайтарылған дәлилдәрҙе раҫлау ҡаршы
рекламаланған өлөшө тамыр, һәм ышаныс төҙөү, тип мәғлүмәттәр бар, тип
бөтә файҙалы йөктө алып ташлау.

##

```
POST /v2/sumeragi/rbc/sample
```

Аҙаҡҡы нөктә `X-API-Token` башлыҡ өсөн тап килгән бер конфигурацияланған .
Torii API токендары. Запростар өҫтәмә ставка-сикләнгән һәм көн һайын буй
пер-кадловка байт бюджеты; йәки ҡайтарыу HTTP 429.

### Һорау тән

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – гекста маҡсатлы блок хеш.
* `height`, `view` – РБК сессияһы өсөн кортежды асыҡлау.
* `count` – теләк һаны өлгөләре (1-гә тиклем ғәҙәттәгесә, конфигурация ярҙамында ҡаплана).
* `seed` – опциональ детерминистик РНГ орлоҡтары өсөн ҡабатланған үлсәү.

### Яуап тән

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

Һәр өлгө инеүе индексы өлөшө, файҙалы йөк байт (гекс), SHA-256 япраҡ
үҙләштереү, һәм Меркл инклюзия иҫбатлау (факльмаль бер туғандар менән кодланған hex .
ҡылдар). Клиенттар `chunk_root` яланын ҡулланып дәлилдәрҙе раҫлай ала.

## Сиктәре һәм бюджеттары

* **Макс өлгөләре өсөн үтенес** – конфигурациялау аша `torii.rbc_sampling.max_samples_per_request`.
* **Макс байт өсөн үтенес** – `torii.rbc_sampling.max_bytes_per_request` ҡулланып мәжбүр ителә.
* **Көндәлек байт бюджеты** – `torii.rbc_sampling.daily_byte_budget` аша шылтыратыусыға бер тапҡыр күҙәтелә.
* **Рейонды сикләү ** – бағышланған токен биҙрә ярҙамында мәжбүр ителгән (`torii.rbc_sampling.rate_per_minute`).

Һорауҙарҙан ашыу теләһә ниндәй сик ҡайтарыу HTTP 429 (CapacityLimit). Ҡасан өлөшө
магазин доступный йәки сессия юҡ файҙалы йөк байт ос нөктәһе
ҡайтара HTTP 404.

## SDK интеграцияһы

### JavaScript

`@iroha/iroha-js` `ToriiClient.sampleRbcChunks` ярҙамсыһы шулай мәғлүмәттәр фашлай
доступность тикшерергә мөмкин, тип атау өсөн ос нөктәһе үҙ фетч ролл
логикаһы. Ярҙамсы шестигранный файҙалы йөктәрҙе раҫлай, бөтөн һандарҙы нормаға һала, һәм ҡайтарыу .
тип аталған объекттар, улар өҫтәге яуап схемаһын көҙгөләй:

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

Ярҙам ташлай, ҡасан сервер ҡайтара дөрөҫ формалаштырылған мәғлүмәттәрҙе, ярҙам JS-04 паритет .
анализдар регрессияларҙы рәт һәм Python SDK-лар менән бер рәттән асыҡлай. Руст
(`iroha_client::ToriiClient::sample_rbc_chunks`) һәм Питон
(`IrohaToriiClient.sample_rbc_chunks`) судно эквивалентлы ярҙамсылары; ниндәй генә булмаһын
һеҙҙең өлгө йүгән тура килә.