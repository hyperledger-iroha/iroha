<!-- Japanese translation of docs/source/snapshot_queries.md -->

---
lang: ja
direction: ltr
source: docs/source/snapshot_queries.md
status: complete
translator: manual
---

# スナップショットクエリ運用ガイド

スナップショットクエリは一貫した `StateView` に対して実行され、コンセンサス状態に影響を与えることなく読み取りワークロードをさばけるよう設計されています。本ノートではスナップショットレーンとともに提供される運用ガードレールをまとめます。

## リソース予算とクォータ

- `pipeline.query_stored_min_gas_units` はサーバー側カーソル保存を制御します。値が 0 より大きい場合:
  - `stored` モードを選択した `Start` リクエストは Torii のクエリパラメータでこの値以上の `gas_units` を指定する必要があります。
  - `Continue` リクエストは Norito ペイロードの `ForwardCursor.gas_budget` で予算を反映します。設定値未満の予算は `NotPermitted` バリデーションエラーで拒否されます。
- `live_query_store.capacity` は保存されたカーソル総数の上限です。
- `live_query_store.capacity_per_user` は権限主体ごとのクォータを強制します。超過時は `AuthorityQuotaExceeded` エラーが返ります。

## カーソルの TTL とガーベジコレクション

保存されたカーソルは `live_query_store.idle_time` を継承します。バックグラウンドの剪定タスクが `last_access_time` が TTL を超過したカーソルを削除します。削除後の `Continue` リクエストは `QueryExecutionFail::Expired` を返すため、クライアントはロスレスに回復できます。

## テレメトリ

テレメトリが有効な場合、2 系統のメトリクスが公開されます。

- Torii フロントエンド:
  - `torii_query_snapshot_requests_total{mode}` — リクエストカウンタ。
  - `torii_query_snapshot_first_batch_ms{mode}` — 最初のバッチのレイテンシヒストグラム。
  - `torii_query_snapshot_gas_consumed_units_total{mode}` — クライアントが提供したガス予算の累計（クエリパラメータおよびカーソルペイロード由来）。
- コアスナップショットレーン:
  - `query_snapshot_lane_first_batch_ms{mode}` — レーン内部での実行時間。
  - `query_snapshot_lane_first_batch_items{mode}` — バッチ当たり項目数。
  - `query_snapshot_lane_remaining_items{mode}` — 残アイテム数ゲージ。
  - `query_snapshot_lane_cursors_total{mode}` — 保存カーソルの発行数。

すべてのカウンタはカーソルモード（`ephemeral` / `stored`）でラベル付けされ、採用状況と予算順守を可視化できます。

## オペレーター向けチェックリスト

1. `pipeline.query_default_cursor_mode` を調整し、デフォルトで `ephemeral` と `stored` のどちらを採用するか決める。
2. 保存カーソルを有効化する場合は `pipeline.query_stored_min_gas_units` を設定し、`torii_query_snapshot_gas_consumed_units_total` を監視してクライアントの予算遵守を確認。
3. `live_query_store.capacity` と `capacity_per_user` を想定ワークロードに合わせて設定し、`query_snapshot_lane_cursors_total` で発行件数を把握。
4. `query_snapshot_lane_remaining_items` を確認して長寿命カーソルの存在を検知し、`idle_time` の剪定設定が厳しすぎないか／緩すぎないかを調整。

これらのガードレールにより、スナップショットクエリの予測可能性を維持しつつ、オペレーター向けのリソース使用状況に可視性を提供します。
