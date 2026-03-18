<!-- Japanese translation of docs/source/samples/find_active_abi_versions.md -->

---
lang: ja
direction: ltr
source: docs/source/samples/find_active_abi_versions.md
status: complete
translator: manual
---

# FindAbiVersion — Norito クエリスキーマ（サンプル）

型名: `iroha_data_model::query::runtime::AbiVersion`

フィールド
- `abi_version: u16` — the fixed ABI version currently accepted on this node.

Norito JSON レスポンス例（初回リリース・単一 ABI）
```json
{
  "abi_version": 1
}
```

クエリ（署名付き） — `FindAbiVersion`
- 経路: `/query`（Norito でエンコードされた `SignedQuery`）
- 単一クエリボックスのバリアント: `FindAbiVersion`
- 出力バリアント: `AbiVersion`

備考
- 初回リリースでは ABI バージョン 1 が常にアクティブです。ガバナンスで有効化されたバージョンは集合に恒久的に追加されます。上記の例は単一 ABI の状態を表します。
