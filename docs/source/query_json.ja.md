<!-- Japanese translation of docs/source/query_json.md -->

---
lang: ja
direction: ltr
source: docs/source/query_json.md
status: complete
translator: manual
---

# クエリ JSON エンベロープ

Iroha は署名付きフレームを受け付ける Norito ベースの `/query` エンドポイントを提供しています。CLI やスクリプトなど対話的なツールから扱いやすいよう、リクエストを JSON で記述し、ツール側で署名済み `SignedQuery` へ変換できます。`iroha_data_model::query::json` モジュールが `iroha_cli query stdin` などのユーティリティで使用する正規エンベロープを定義しています。

## エンベロープ形状

トップレベルは `singular` または `iterable` セクションのどちらか一方を含むオブジェクトです。

```json
{"singular": { /* singular query */ }}
{"iterable": { /* iterable query */ }}
```

両方のセクションを同時に含む、あるいはどちらも含まないリクエストは拒否されます。

## 単一クエリ（Singular）

単一クエリではクエリ名を `type` に指定し、必要に応じてペイロードを付与します。

```json
{
  "singular": {
    "type": "FindContractManifestByCodeHash",
    "payload": {
      "code_hash": "0x00112233…"
    }
  }
}
```

サポートされている単一クエリ:

- `FindActiveAbiVersions`
- `FindExecutorDataModel`
- `FindParameters`
- `FindContractManifestByCodeHash`（32 バイトの `code_hash` を 16 進文字列で指定）

## イテラブルクエリ

イテラブルクエリではクエリ名に加え、実行時の修飾子や述語をオプションで指定できます。

```json
{
  "iterable": {
    "type": "FindDomains",
    "params": {
      "limit": 25,
      "offset": 10,
      "fetch_size": 50,
      "sort_by_metadata_key": "ui.order",
      "order": "Desc",
      "ids_projection": false,
      "lane_id": null,   // 予約フィールド（将来のカーソルレーン）
      "dsid": null       // 予約フィールド（将来のデータシャード識別子）
    },
    "predicate": {
      "equals": [
        {"field": "authority", "value": "ih58..."}
      ],
      "in": [
        {"field": "metadata.tier", "values": [1, 2, 3]}
      ],
      "exists": ["metadata.display_name"]
    }
  }
}
```

### パラメータ

任意の `params` オブジェクトでページングやソートを制御します。

- `limit` (`u64`, 任意) — 取得する総件数の上限。
- `offset` (`u64`, 既定 `0`) — スキップする件数。
- `fetch_size` (`u64`, 任意) — 1 バッチあたりの取得件数。
- `sort_by_metadata_key` (`string`, 任意) — 安定ソートに用いるメタデータキー。
- `order` (`"Asc"` | `"Desc"`, 任意) — ソート順。メタデータキーが指定された場合のみ受理。
- `ids_projection` (`bool`, 任意) — ノードが実験的な `ids_projection` 機能でビルドされている場合に ID のみを要求。
- `lane_id`, `dsid` (任意) — 将来のカーソルレーン／データシャード向け予約フィールド。パーサは受理しますが現在は無視されます。

数値パラメータを指定する場合は 0 以外である必要があります。ソートキーは正規の [`Name`](../../crates/iroha_data_model/src/name.rs) ルールで検証されます。

### 述語ミニ DSL

述語は 3 つの任意配列を持つオブジェクトで表現します。

- `equals`: `{ "field": <path>, "value": <json value> }` の配列。
- `in`: `{ "field": <path>, "values": [<json value>, …] }` の配列（`values` は非空）。
- `exists`: 存在を確認するフィールドパスの配列。

フィールドパスはドット区切り表記（例: `metadata.display_name`, `authority`）。エンコーダはフィールド名順でセクションをソートし、署名の決定性を保証します。

## CLI 利用例

CLI は標準入力からエンベロープを読み込み、設定済みアカウントで署名し、`/query` に送信します。

```shell
$ cargo run -p iroha_cli -- query stdin <<'JSON'
{
  "iterable": {
    "type": "FindDomains",
    "params": {"limit": 5, "sort_by_metadata_key": "ui.order", "order": "Asc"},
    "predicate": {"exists": ["metadata.display_name"]}
  }
}
JSON
```

レスポンスは設定した出力形式で表示されます。同じエンベロープは `QueryEnvelopeJson::into_signed_request` を用いてプログラムから署名済みフレームへ変換可能です。Rust API の詳細は [`iroha_data_model::query::json`](../../crates/iroha_data_model/src/query/json) を参照してください。
