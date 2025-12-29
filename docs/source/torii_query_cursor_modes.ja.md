<!-- Japanese translation of docs/source/torii_query_cursor_modes.md -->

---
lang: ja
direction: ltr
source: docs/source/torii_query_cursor_modes.md
status: complete
translator: manual
---

## Torii クエリカーソルモード（スナップショットレーン）

本ページは、サーバー側クエリがスナップショット上でカーソル挙動をどのように選択するか、およびクライアントがリクエスト単位でどのように上書きできるかを説明します。

### 概要

Iroha は決定性を確保するため、読み取り専用クエリを捕捉済みの `StateView` スナップショット上で実行します。イテラブルクエリは次の 2 モードで動作します。

- `ephemeral`: 最初のバッチのみ返却し、サーバー側にカーソルを保持しません。クライアントは Start リクエストを再送し、ページネーションのパラメータを更新して読み進めます。
- `stored`: 最初のバッチとサーバー側カーソルを返却します。クライアントは同じスナップショットをカーソルで継続できます。

モードは設定で切り替えられ、リクエストごとに上書き可能です。

### 設定

- `pipeline.query_default_cursor_mode`: サーバーがデフォルトで使用するカーソルモード。値は `ephemeral`（既定）または `stored`。
- `pipeline.query_stored_min_gas_units`: `stored` モードに必要な最小ガス単位（0 で要件無効化）。0 より大きい場合、クライアントはリクエスト内で十分な `gas_units` を示す必要があります。

### リクエスト単位の上書き

`/query` エンドポイントはモード制御用に予約されたクエリパラメータを受け付けます。

- `cursor_mode`: `ephemeral` | `stored`
- `gas_units`: 整数。`pipeline.query_stored_min_gas_units > 0` かつ `cursor_mode=stored` の場合に必須。充足しない場合、バリデーションエラーとなります。
- `Continue` リクエストの Norito ペイロードには `ForwardCursor.gas_budget` が埋め込まれ、サーバーが再検証できるようになっています。

備考:
- `cursor_mode` を省略すると、サーバーは `pipeline.query_default_cursor_mode` の設定値を用います。
- `ephemeral` モードでは `cursor=null` が常に返り、`Continue` リクエストは拒否されます。

### テレメトリ

テレメトリが有効（`telemetry_enabled=true`）な場合:

- `torii_query_snapshot_requests_total{mode}`: スナップショットレーンのクエリ件数をモード別に集計。
- `torii_query_snapshot_first_batch_ms{mode}`: 最初のバッチのレイテンシ（ミリ秒）のヒストグラム。
- `torii_query_snapshot_gas_consumed_units_total{mode}`: `stored` モードでクライアントが報告したガス単位の累積値（最小値が設定されている場合）。
- コアテレメトリでは `query_snapshot_lane_first_batch_ms{mode}` や `query_snapshot_lane_remaining_items{mode}` といった補助メトリクスも公開され、パイプラインを直接監視する際に役立ちます。

### 決定性とスナップショットの意味

- すべてのクエリは捕捉済み `StateView` に対して実行されます。スナップショット取得後に行われたライブ状態の変更は、進行中の stored カーソルに影響しません。
- `ephemeral` モードは最初のバッチを生成して返却します。クライアントは Start リクエストを再送してページネーションを進める必要があります。

### 例（概念）

1. デフォルトが `ephemeral` の場合（上書きなし）:

POST /v1/query  
Body: Norito エンコード済み Start リクエスト  
Response: 最初のバッチ、`cursor=null`

2. `stored` に上書きしガス単位を指定:

POST /v1/query?cursor_mode=stored&gas_units=100  
Body: Norito エンコード済み Start リクエスト  
Response: 最初のバッチと `cursor` を返却

`pipeline.query_stored_min_gas_units=200` の場合、上記リクエストは NotPermitted で拒否されます。

---

Torii の公式エンドポイント一覧はリファレンスセクションを参照してください。本ページはスナップショットレーンのカーソル挙動に特化した説明です。
