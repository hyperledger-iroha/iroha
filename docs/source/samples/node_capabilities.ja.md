<!-- Japanese translation of docs/source/samples/node_capabilities.md -->

---
lang: ja
direction: ltr
source: docs/source/samples/node_capabilities.md
status: complete
translator: manual
---

# ノード機能 — ABI サポート（Torii）

エンドポイント
- `GET /v1/node/capabilities`

レスポンス（初回リリース・単一 ABI ポリシー V1）
```json
{
  "abi_version": 1,
  "data_model_version": 1,
  "crypto": {
    "sm": {
      "enabled": false,
      "default_hash": "sha2_256",
      "allowed_signing": ["ed25519"],
      "sm2_distid_default": "",
      "openssl_preview": false,
      "acceleration": {
        "scalar": true,
        "neon_sm3": false,
        "neon_sm4": false,
        "policy": "scalar-only"
      }
    },
    "curves": {
      "registry_version": 1,
      "allowed_curve_ids": [1]
    }
  }
}
```

備考
- `abi_version` には、ノードが入場時に受け付ける ABI バージョンの一覧が含まれます。
- `abi_version` はノードが受け付ける唯一の ABI バージョンであり、Kotodama コンパイラもこの値を使います。
- `data_model_version` はデータモデルの互換バージョンであり、SDK は組み込み値と異なる場合に送信を拒否する必要があります。
- `crypto.curves.allowed_curve_ids` は `iroha_config.crypto.curves.allowed_curve_ids` で設定された [`address_curve_registry`](../references/address_curve_registry.md) 上のカーブ ID を表します。ML‑DSA や GOST、SM コントローラを投入する前に、このリストに必要な ID が含まれているか確認してください。

あわせて参照
- 簡潔なランタイムメトリクスの JSON を取得するには `GET /v1/runtime/metrics` を利用してください（ABI 数およびアップグレードライフサイクルのカウンターを含みます）。
