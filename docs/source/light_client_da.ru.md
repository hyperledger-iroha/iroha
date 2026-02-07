---
lang: ru
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2026-01-03T18:07:57.770085+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Выборка доступности данных легкого клиента

API выборки легкого клиента позволяет аутентифицированным операторам получать
Образцы фрагментов RBC, аутентифицированные Merkle, для текущего блока. Легкие клиенты
может выдавать запросы на случайную выборку, проверять возвращенные доказательства на соответствие
объявленный корень чанка и создать уверенность в том, что данные доступны без
получение всей полезной нагрузки.

## Конечная точка

```
POST /v1/sumeragi/rbc/sample
```

Конечная точка требует заголовок `X-API-Token`, соответствующий одному из настроенных
Torii API-токены. Запросы дополнительно ограничены по тарифам и подлежат ежедневной оплате.
бюджет в байтах на вызывающего абонента; превышение любого из них возвращает HTTP 429.

### Тело запроса

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – хеш целевого блока в шестнадцатеричном формате.
* `height`, `view` – идентифицирующий кортеж для сеанса RBC.
* `count` – желаемое количество выборок (по умолчанию 1, ограничено конфигурацией).
* `seed` – опциональное детерминированное начальное значение ГСЧ для воспроизводимой выборки.

### Тело ответа

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

Каждая выборочная запись содержит индекс фрагмента, байты полезной нагрузки (шестнадцатеричные), лист SHA-256.
дайджест и доказательство включения Меркла (с дополнительными братьями и сестрами, закодированными в шестнадцатеричном формате).
струны). Клиенты могут проверить доказательства, используя поле `chunk_root`.

## Лимиты и бюджеты

* **Максимальное количество образцов на запрос** — настраивается через `torii.rbc_sampling.max_samples_per_request`.
* **Максимальное количество байтов на запрос** – применяется с помощью `torii.rbc_sampling.max_bytes_per_request`.
* **Дневной бюджет в байтах** — отслеживается для каждого вызывающего абонента через `torii.rbc_sampling.daily_byte_budget`.
* **Ограничение скорости** — обеспечивается с помощью выделенного сегмента токенов (`torii.rbc_sampling.rate_per_minute`).

Запросы, превышающие любой лимит, возвращают HTTP 429 (CapacityLimit). Когда кусок
хранилище недоступно или в сеансе отсутствуют байты полезной нагрузки конечной точки
возвращает HTTP 404.

## Интеграция SDK

### JavaScript

`@iroha/iroha-js` предоставляет помощник `ToriiClient.sampleRbcChunks`, поэтому данные
проверяющие доступность могут вызывать конечную точку без выполнения собственной выборки
логика. Помощник проверяет шестнадцатеричные полезные данные, нормализует целые числа и возвращает
типизированные объекты, которые отражают приведенную выше схему ответа:

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

Помощник выдает ошибку, когда сервер возвращает неверные данные, помогая выполнить четность JS-04.
тесты обнаруживают регрессии вместе с SDK Rust и Python. Ржавчина
(`iroha_client::ToriiClient::sample_rbc_chunks`) и Python
(`IrohaToriiClient.sample_rbc_chunks`) отправляйте эквивалентные помощники; используйте любой
соответствует вашей системе отбора проб.