<!-- Japanese translation of docs/source/light_client_da.md -->

---
lang: ja
direction: ltr
source: docs/source/light_client_da.md
status: complete
translator: manual
---

# ライトクライアント・データ可用性サンプリング

Light Client Sampling API を利用すると、認証済みのオペレータが進行中ブロックに対する RBC チャンクを Merkle 証明付きでサンプリングできます。ライトクライアントはランダムなサンプリングリクエストを発行し、返却された証明を公開済みのチャンクルートと照合することで、全ペイロードを取得せずともデータ可用性に自信を持つことができます。

## エンドポイント

```
POST /v1/sumeragi/rbc/sample
```

このエンドポイントは設定済み Torii API トークンのいずれかと一致する `X-API-Token` ヘッダを必須とします。さらにレート制限と呼び出し元ごとの日次バイト予算が適用され、いずれかを超過すると HTTP 429 が返されます。

### リクエストボディ

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` — 対象ブロックハッシュ（hex）。
* `height`, `view` — RBC セッションを識別するタプル。
* `count` — 要求するサンプル数（既定値は 1。設定で上限あり）。
* `seed` — 任意の決定的 RNG シード。再現可能なサンプリングに使用。

### レスポンスボディ

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

各サンプルにはチャンクインデックス、ペイロードバイト列（hex）、SHA-256 の葉ダイジェスト、Merkle 包含証明（兄弟ノードは hex 文字列で表現）が含まれます。クライアントは `chunk_root` フィールドを使って証明を検証できます。

## 制限と予算

* **リクエストごとの最大サンプル数** — `torii.rbc_sampling.max_samples_per_request` で設定。
* **リクエストごとの最大バイト数** — `torii.rbc_sampling.max_bytes_per_request` で強制。
* **日次バイト予算** — 呼び出し元ごとに `torii.rbc_sampling.daily_byte_budget` で追跡。
* **レート制限** — 専用トークンバケット（`torii.rbc_sampling.rate_per_minute`）で適用。

これらの制限を超過すると HTTP 429（CapacityLimit）が返されます。チャンクストアが利用できない場合、またはセッションに必要なペイロードバイトが存在しない場合は HTTP 404 が返されます。

## SDK 統合

### JavaScript

`@iroha/iroha-js` には `ToriiClient.sampleRbcChunks` ヘルパーが用意されており、独自の HTTP 実装を記述しなくても認証付きサンプリングを実行できます。ヘルパーは受け取った 16 進データを検証し、上記レスポンスと同じ形の型付きオブジェクトを返します。

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
  throw new Error("RBC セッションがまだ利用できません");
}

for (const { digestHex, proof } of sample.samples) {
  verifyMerklizedChunk(sample.chunkRoot, digestHex, proof);
}
```

サーバーが不正なデータを返した場合、ヘルパーは例外を送出するため、Rust/Python と同じように JS-04 のパリティテストで回帰を検出できます。Rust（`iroha_client::ToriiClient::sample_rbc_chunks`）と Python（`IrohaToriiClient.sample_rbc_chunks`）にも同等のヘルパーがあり、用途に応じて選択してください。
