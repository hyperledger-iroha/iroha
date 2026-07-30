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
  "abi_hash_hex": "17c61cb3a6ee164213afe410169161def1d7025b84f0b9e385a93619a862513b"
}
```

備考
- このハッシュは、当該ポリシーで許可されているシステムコール面の正規ダイジェスト値です。
- コントラクトはこの値をマニフェストの `abi_hash` に埋め込み、デプロイ先ノードの ABI と結び付けます。
