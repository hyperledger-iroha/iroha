---
lang: pt
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2026-01-03T18:07:57.770085+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Amostragem leve de disponibilidade de dados do cliente

A API Light Client Sampling permite que operadores autenticados recuperem
Amostras de pedaços RBC autenticados por Merkle para um bloco em andamento. Clientes leves
pode emitir solicitações de amostragem aleatória, verificar as provas retornadas em relação ao
raiz do pedaço anunciada e criar confiança de que os dados estão disponíveis sem
buscando toda a carga útil.

## Ponto final

```
POST /v1/sumeragi/rbc/sample
```

O endpoint requer um cabeçalho `X-API-Token` correspondente a um dos configurados
Tokens de API Torii. As solicitações também têm taxa limitada e estão sujeitas a uma taxa diária
orçamento de bytes por chamador; exceder qualquer um deles retorna HTTP 429.

### Corpo da solicitação

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – hash do bloco de destino em hexadecimal.
* `height`, `view` – identificando tupla para a sessão RBC.
* `count` – número desejado de amostras (o padrão é 1, limitado pela configuração).
* `seed` – semente RNG determinística opcional para amostragem reproduzível.

### Corpo de resposta

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

Cada entrada de amostra contém o índice do bloco, bytes de carga útil (hex), folha SHA-256
resumo e uma prova de inclusão Merkle (com irmãos opcionais codificados como hexadecimal
cordas). Os clientes podem verificar as provas usando o campo `chunk_root`.

## Limites e Orçamentos

* **Máximo de amostras por solicitação** – configurável via `torii.rbc_sampling.max_samples_per_request`.
* **Máximo de bytes por solicitação** – aplicado usando `torii.rbc_sampling.max_bytes_per_request`.
* **Orçamento diário de bytes** – rastreado por chamador por meio de `torii.rbc_sampling.daily_byte_budget`.
* **Limitação de taxa** – aplicada usando um token bucket dedicado (`torii.rbc_sampling.rate_per_minute`).

Solicitações que excedem qualquer limite retornam HTTP 429 (CapacityLimit). Quando o pedaço
store está indisponível ou a sessão está faltando bytes de carga no endpoint
retorna HTTP 404.

## Integração SDK

### JavaScript

`@iroha/iroha-js` expõe o auxiliar `ToriiClient.sampleRbcChunks` para que os dados
verificadores de disponibilidade podem chamar o endpoint sem rolar sua própria busca
lógica. O auxiliar valida as cargas hexadecimais, normaliza números inteiros e retorna
objetos digitados que espelham o esquema de resposta acima:

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

O auxiliar é lançado quando o servidor retorna dados malformados, ajudando na paridade JS-04
os testes detectam regressões junto com os SDKs Rust e Python. Ferrugem
(`iroha_client::ToriiClient::sample_rbc_chunks`) e Python
(`IrohaToriiClient.sample_rbc_chunks`) enviar ajudantes equivalentes; use qualquer
corresponde ao seu chicote de amostragem.