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
  "abi_hash_hex": "2ecabe125a8a9181915f9a6b905ef0e26c73b7e4b71e44e50dbcc757e1a19f91"
}
```

備考
- このハッシュは、当該ポリシーで許可されているシステムコール面の正規ダイジェスト値です。
- コントラクトはこの値をマニフェストの `abi_hash` に埋め込み、デプロイ先ノードの ABI と結び付けます。
