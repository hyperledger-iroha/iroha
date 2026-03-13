---
lang: zh-hans
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2025-12-29T18:16:35.975661+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 轻客户端数据可用性采样

轻客户端采样 API 允许经过身份验证的操作员检索
正在运行的块的经 Merkle 验证的 RBC 块样本。轻客户端
可以发出随机抽样请求，根据返回的证据验证
广告块根，并建立数据可用的信心
获取整个有效负载。

## 端点

```
POST /v2/sumeragi/rbc/sample
```

端点需要与配置的其中之一匹配的 `X-API-Token` 标头
Torii API 令牌。请求还受到速率限制，并且每天都会受到限制
每个调用者的字节预算；超过任一值都会返回 HTTP 429。

### 请求正文

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – 十六进制目标块哈希。
* `height`、`view` – 识别 RBC 会话的元组。
* `count` – 所需的样本数量（默认为 1，受配置限制）。
* `seed` – 可选的确定性 RNG 种子，用于可重复采样。

### 响应正文

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

每个示例条目包含块索引、有效负载字节（十六进制）、SHA-256 叶
摘要和 Merkle 包含证明（可选的兄弟姐妹编码为十六进制
字符串）。客户端可以使用 `chunk_root` 字段验证证明。

## 限制和预算

* **每个请求的最大样本** – 通过 `torii.rbc_sampling.max_samples_per_request` 配置。
* **每个请求的最大字节数** – 使用 `torii.rbc_sampling.max_bytes_per_request` 强制执行。
* **每日字节预算** – 通过 `torii.rbc_sampling.daily_byte_budget` 跟踪每个调用者。
* **速率限制** – 使用专用令牌桶 (`torii.rbc_sampling.rate_per_minute`) 强制执行。

超过任何限制的请求将返回 HTTP 429 (CapacityLimit)。当块
存储不可用或会话缺少端点的有效负载字节
返回 HTTP 404。

## SDK集成

### JavaScript

`@iroha/iroha-js` 公开 `ToriiClient.sampleRbcChunks` 帮助程序所以数据
可用性验证者可以调用端点，而无需滚动自己的获取
逻辑。帮助器验证十六进制有效负载，标准化整数并返回
反映上面响应模式的类型化对象：

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

当服务器返回格式错误的数据时，助手会抛出异常，帮助 JS-04 奇偶校验
测试与 Rust 和 Python SDK 一起检测回归。铁锈
(`iroha_client::ToriiClient::sample_rbc_chunks`) 和 Python
(`IrohaToriiClient.sample_rbc_chunks`) 船舶等效助手；使用任意一个
与您的采样线束相匹配。