<!-- Japanese translation of docs/source/samples/runtime_abi_hash.md -->

---
lang: ja
direction: ltr
source: docs/source/samples/runtime_abi_hash.md
status: complete
translator: manual
---

# ランタイム ABI — 正規ハッシュ（Torii）

エンドポイント
- `GET /v1/runtime/abi/hash`

レスポンス（初回リリース・単一ポリシー V1）
```json
{
  "policy": "V1",
  "abi_hash_hex": "ae5bb302d5946c938ab40bca4a234daff2d8975ce261d352b1437b51f90467cb"
}
```

備考
- このハッシュは、当該ポリシーで許可されているシステムコール面の正規ダイジェスト値です。
- コントラクトはこの値をマニフェストの `abi_hash` に埋め込み、デプロイ先ノードの ABI と結び付けます。
