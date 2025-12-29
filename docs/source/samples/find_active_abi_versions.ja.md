<!-- Japanese translation of docs/source/samples/find_active_abi_versions.md -->

---
lang: ja
direction: ltr
source: docs/source/samples/find_active_abi_versions.md
status: complete
translator: manual
---

# FindActiveAbiVersions — Norito クエリスキーマ（サンプル）

型名: `iroha_data_model::query::runtime::ActiveAbiVersions`

フィールド
- `active_versions: Vec<u16>` — このノードで現在有効な ABI バージョンの昇順リスト。
- `default_compile_target: u16` — アクティブなバージョンのうち最大値。コンパイラが既定でターゲットにするべき値。

Norito JSON レスポンス例（初回リリース・単一 ABI）
```json
{
  "active_versions": [1],
  "default_compile_target": 1
}
```

クエリ（署名付き） — `FindActiveAbiVersions`
- 経路: `/query`（Norito でエンコードされた `SignedQuery`）
- 単一クエリボックスのバリアント: `FindActiveAbiVersions`
- 出力バリアント: `ActiveAbiVersions`

備考
- 初回リリースでは ABI バージョン 1 が常にアクティブです。ガバナンスで有効化されたバージョンは集合に恒久的に追加されます。上記の例は単一 ABI の状態を表します。
