---
lang: zh-hant
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2025-12-29T18:16:35.975661+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 輕客戶端數據可用性採樣

輕客戶端採樣 API 允許經過身份驗證的操作員檢索
正在運行的塊的經 Merkle 驗證的 RBC 塊樣本。輕客戶端
可以發出隨機抽樣請求，根據返回的證據驗證
廣告塊根，並建立數據可用的信心
獲取整個有效負載。

## 端點

```
POST /v1/sumeragi/rbc/sample
```

端點需要與配置的其中之一匹配的 `X-API-Token` 標頭
Torii API 令牌。請求還受到速率限制，並且每天都會受到限制
每個調用者的字節預算；超過任一值都會返回 HTTP 429。

### 請求正文

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – 十六進制目標塊哈希。
* `height`、`view` – 識別 RBC 會話的元組。
* `count` – 所需的樣本數量（默認為 1，受配置限制）。
* `seed` – 可選的確定性 RNG 種子，用於可重複採樣。

### 響應正文

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

每個示例條目包含塊索引、有效負載字節（十六進制）、SHA-256 葉
摘要和 Merkle 包含證明（可選的兄弟姐妹編碼為十六進制
字符串）。客戶端可以使用 `chunk_root` 字段驗證證明。

## 限制和預算

* **每個請求的最大樣本** – 通過 `torii.rbc_sampling.max_samples_per_request` 配置。
* **每個請求的最大字節數** – 使用 `torii.rbc_sampling.max_bytes_per_request` 強制執行。
* **每日字節預算** – 通過 `torii.rbc_sampling.daily_byte_budget` 跟踪每個調用者。
* **速率限制** – 使用專用令牌桶 (`torii.rbc_sampling.rate_per_minute`) 強制執行。

超過任何限制的請求將返回 HTTP 429 (CapacityLimit)。當塊
存儲不可用或會話缺少端點的有效負載字節
返回 HTTP 404。

## SDK集成

### JavaScript

`@iroha/iroha-js` 公開 `ToriiClient.sampleRbcChunks` 幫助程序所以數據
可用性驗證者可以調用端點，而無需滾動自己的獲取
邏輯。幫助器驗證十六進制有效負載，標準化整數並返回
反映上面響應模式的類型化對象：

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

當服務器返回格式錯誤的數據時，助手會拋出異常，幫助 JS-04 奇偶校驗
測試與 Rust 和 Python SDK 一起檢測回歸。鐵鏽
(`iroha_client::ToriiClient::sample_rbc_chunks`) 和 Python
(`IrohaToriiClient.sample_rbc_chunks`) 船舶等效助手；使用任意一個
與您的採樣線束相匹配。