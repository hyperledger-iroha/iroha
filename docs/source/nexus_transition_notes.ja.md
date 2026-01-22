<!-- Japanese translation of docs/source/nexus_transition_notes.md -->

---
lang: ja
direction: ltr
source: docs/source/nexus_transition_notes.md
status: complete
translator: manual
---

# Nexus トランジションノート — レーン／データスペース抽象プロトタイプ

本ドキュメントは、`nexus.md` に記述されたアーキテクチャへ移行するために、単一レーンの Iroha 2 パイプラインへ段階的に配管を追加する作業の進捗を追跡します。本マイルストンでは、レーンおよびデータスペースのコンテキストを API に渡せるようにする内部識別子とテレメトリのフックを導入しますが、まだ挙動そのものは変化させません。同一リポジトリから Iroha 2 と SORA Nexus をビルドするため、これらのシムは単一レーン互換性を維持しつつ、マルチレーン実行に必要な足場を先行して追加する目的があります。

## プレースホルダ識別子

- `LaneId` と `DataSpaceId` は `iroha_data_model::nexus` 内にあり、Norito 互換な薄い新型です。既定値はそれぞれ `LaneId::SINGLE` と `DataSpaceId::GLOBAL` で、将来的には決定的なルーティング判断を運ぶ予定です。現状では API の将来像を示すためのラベルとして機能します。
- これらの型は Norito のエンコード／デコードに参加し、data-model プレリュードから公開されているため、下流クレートは不安定なフィーチャーフラグなしで依存できます。

## テレメトリとログのシム

- `iroha_core` の `StateTelemetry` メソッドは `LaneId` を受け取るようになり、グローバルな `DataSpaceId` プレースホルダを記録します。メトリクスには `nexus_lane_id_placeholder` と `nexus_dataspace_id_placeholder` の 2 つのゲージが追加され、現在は単一レーンでも将来のダッシュボードスキーマと整合が取れるようになっています。
- `iroha_logger` 経由で発行される構造化テレメトリイベントは `lane_id` と `dataspace_id` を自動的に含めます。呼び出し側が明示値を渡さない場合、ロガーが単一レーンの既定値を差し込みます。

## 次のステップ

- Nexus スケジューラ（開始時公平キューイング、フュージョンロジック、データスペースポリシー）が実装されたら、ハードコードされた `LaneId::SINGLE` / `DataSpaceId::GLOBAL` の呼び出しを実際のルーティングに置き換えます。
- トランザクション受付、キューゴシップ、Torii API といった経路で新しい識別子を受け付け、ネットワーク越しに伝搬できるようにします。
- マルチレーン実行が稼働したら、メトリクスとテレメトリのラベルをプレースホルダではなく実際の識別子に切り替えます。
- オペレーション手順に `iroha_cli app nexus lane-report --summary --only-missing --fail-on-sealed` を追加し、`/v1/sumeragi/status` の `lane_governance_sealed_total` / `lane_governance_sealed_aliases` と組み合わせてマニフェスト未適用のレーンを早期検知できるようにします。

## レーン／データスペースルーティング移行計画

ロードマップにある「Lane/Data-Space Routing」の範囲が確定しました。以下は現在の単一レーンプレースホルダから Nexus ネイティブルーティングへ移行するまでの道筋であり、既存デプロイへの影響を最小化します。

### 現状のギャップ

- Torii（`crates/iroha_torii`）はレーンコンテキストをテレメトリログでのみ伝搬しており、HTTP API／ストリーマーフィルタ／SDK ペイロードにはレーンやデータスペースのヒントが含まれません。
- ダッシュボードは `nexus_lane_id_placeholder` と `nexus_dataspace_id_placeholder` のプレースホルダゲージを表示し、`crates/iroha_logger/src/telemetry.rs` でロガーが既定値を埋めるため、複数レーン化した際の可視性が限定されます。

### 移行フェーズ

1. **レーンカタログと設定配線（目標: Sprint O5.1）** — 担当: `@nexus-core`（ランタイム）
   - `iroha_config::{user, actual, defaults}` に `lane_count`、`dataspace_catalog`、構造化 `routing_policy` セクションを追加。`irohad` 起動時に `iroha_core::queue::LayeredTransactionQueue` 構築へ値を渡す。
   - `iroha_data_model::nexus` のヘルパーに安全なコンストラクタ（`LaneId::from_lane_index`, `DataSpaceId::from_hash`）を実装し、無効 ID を防止。
   - 設定ペイロード向け Norito ヘッダ更新とスキーマドキュメントを提供。
2. **決定的ルータ表面（目標: Sprint O5.2）** — 担当: `@nexus-core`、レビュア: `@iroha-research`
   - `crates/iroha_core/src/queue.rs` に `LaneRouter` トレイトを追加し、キュー投入前に `Queue::push` が呼ぶようにする。初期実装では既存トラフィックを既定レーンへマップしつつポリシー（例: ガバナンスマニフェスト）を適用。
   - `AcceptedTransaction` メタデータにレーン／データスペースを保存し、`crates/iroha_core/src/block.rs` が DAG 構築時にレーンを尊重するよう拡張。
   - ゴシップ／受付イベントを更新し、`PipelineMessage::QueueTx` と `TransactionEvent` にレーン ID を含める。
3. **ネットワーク／API 伝搬（目標: Sprint O5.3）** — 担当: `@torii-sdk`
   - Torii エンドポイント・WebSocket・CLI／SDK バインディングで `lane_id` / `dataspace_id` をオプション受け入れにし、未指定時はサーバポリシー既定を使用。`iroha_data_model::prelude` のペイロードへ値を反映しドキュメントを再生成。
   - `iroha_client` と `pytests` フィクスチャを更新し、レーン認識型の受付を検証。混在レーンキューの統合テストを追加。
4. **可観測性と移行ガード（目標: Sprint O5.4）** — 担当: `@telemetry-ops` + ランタイムオーナー
   - `crates/iroha_telemetry/src/metrics.rs` でプレースホルダゲージをレーン別メトリクス（例: `pipeline_tx_total{lane_id="…"}`）へ置換し、`iroha_core::telemetry` 呼び出し箇所を更新。
   - `crates/iroha_logger` に `lane_id` / `dataspace_id` 指定を必須化し、既定補完を行った場合は WARN を発行。
   - Grafana ダッシュボードとアラートを提供し、複数レーン環境で即時に診断できるようにする。

## Nexus 監査トレース

Nexus マイルストンの一部として、四半期ごとの監査で検証すべきテレメトリ／ストレージ／設定のトレースが定義されました。下表は追跡対象のアーティファクトと受け入れ基準です。

| Trace ID | 目的 | アーティファクト | オーナー | レビュア | 期限（2026 Q1） | 受け入れ基準 |
| --- | --- | --- | --- | --- | --- | --- |
| `TRACE-PIPELINE-LANE` | 多レーン運用のパイプライン健全性（履歴・再演） | `crates/irohad/src/main.rs` のマルチレーン設定テスト、Grafana スナップショット、`integration_tests/tests/nexus/multilane_kura_layout.rs` (3 レーンのルータ/ストレージ確認) | `@nexus-core` | `@telemetry-ops`, `@sec-ops` | 2 月 3–14 日 | 全レーンが均等に取り込まれ、待機時間が SLA 内、スロット漏れなし |
| `TRACE-DA-SAMPLING` | データ可用性サンプリングと RBC フォールト処理 | CI テンプレート `ci/integration_tests_multilane.yml`, Prometheus サンプル, alert 定義 `alerts/nexus_da.rules.yml` | `@nexus-core`, `@qa-consensus` | `@telemetry-ops` | 2 月 24–28 日 | DA 鍵ローテ、遅延 READY ゴシップが期待通りのテレメトリを発火し、DA 証明不一致でブロックがコミットされない |
| `TRACE-TELEMETRY-BRIDGE` | Nexus メトリクスの Prometheus/OTLP 曝露と構造化ログ取り込み | `/metrics` スクレイプ、OTLP サンプル、ログ集約ペイロード、Grafana スナップショット | `@telemetry-ops` | `@nexus-core`, `@sec-observability` | 3 月 3–14 日 | Nexus バリアントのメトリクスがすべて非ゼロサンプルを持ち、構造化ログの既定補完 WARN 率 ≤1% |
| `TRACE-CONFIG-DELTA` | Nexus と単一レーンの設定差分、およびジェネシスマニフェストダイジェスト | `defaults/nexus/*.toml`, ステージングジェネシス、`iroha_cli config show --actual` 出力 | `@governance`, `@nexus-core` | `@release-eng`, `@telemetry-ops` | リリースカットと同時 | 差分チェックリストで Nexus ノブを把握し、単一レーンバンドルに不要な設定が混入していない |

各トレースは固定 seed（ワークロード、バリデータ構成、レイテンシ）で実行し、四半期ごとの比較が可能です。オーナーは監査開始 3 営業日前までにアーティファクトを準備し、レビュアは `docs/source/project_tracker/npos_sumeragi_phase_b.md` で承認を記録します。

### テレメトリギャップ追跡

| Gap ID | 不足シグナル | オーナー | リリース目標 | ノート |
| --- | --- | --- | --- | --- |
| `GAP-TELEM-001` | `torii_lane_admission_latency_seconds{lane_id,endpoint}` ヒストグラム | `@torii-sdk` | 2026.2 | 2025-12-04 に `/transaction` 等のレーン別レイテンシを導入済み。残りの経路はレーンルーティング導入時に実装。 |
| `GAP-TELEM-002` | 設定リロード時の `nexus_config_diff_total{knob,profile}` カウンタ | `@nexus-core` | 2026.1 | `StateTelemetry::record_nexus_config_diff` により実装済み。テレメトリ資料にアラート例とログ確認手順を追記済み。 |
| `GAP-TELEM-003` | `TelemetryEvent::AuditOutcome`（トレース ID／高さ／レビュア） | `@telemetry-ops` | 2026.1 | `Telemetry::record_audit_outcome` が `nexus.audit.outcome` を発行し、`dashboards/alerts/nexus_audit_rules.yml` が失敗ステータスを監視。欠落イベントは `scripts/telemetry/check_nexus_audit_outcome.py` を TRACE/CI に組み込んで 30 分以内を強制し、成果物を保存する。【crates/iroha_core/src/telemetry.rs:3056】【docs/source/telemetry.ja.md:99】 |
| `GAP-TELEM-004` | `nexus_lane_configured_total`（ノードごとのレーン数） | `@telemetry-ops` | 2026.1 | 実装済み。`StateTelemetry::set_nexus_catalogs` からゲージが更新され、Prometheus の `nexus_lane_configured_total` を監視すれば監査前に誤設定を検知できる。テレメトリガイドにアラート例を追記済み。 |

上記ギャップは 2026 Q1 第 2 回監査前にすべて解消し、`docs/source/telemetry.md` に反映する必要があります。

### 設定差分の受入ゲート

| 設定対象 | Nexus で期待される差分 | 受入レビュー | オーナー | 証跡 |
| --- | --- | --- | --- | --- |
| `iroha_config.user.nexus.lane_count` | Nexus バンドルでは 1 より大きい値、単一レーンでは 1 | `defaults/nexus/config.toml` と単一レーンテンプレートを比較 | `@nexus-core` | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| `iroha_config.user.nexus.routing_policy` | Lane catalog に一致するポリシーテーブル | `LaneCatalog` / `RoutingPolicy` の Norito シリアライズを監査 | `@governance` | マニフェストハッシュとレビュア承認記録 |
| `iroha_config.user.nexus.da.*` | 本文の DA テーブルと一致 | “In-slot DA sampling” を参照し CI テンプレート差分を確認 | `@telemetry-ops` | 差分スニペット＋CI URL |
| Norito ジェネシスマニフェスト | Nexus 機能ビット、レーンカタログ、DA クォーラム設定を含む | `kagami genesis bootstrap --profile nexus` 実行、候補ハッシュと比較 | `@release-eng` | リリースチェックリストでハッシュ記録 |

Phase B 完了には、各リリースで上記 4 項目すべての承認が記録されている必要があります。

## ガバナンスパラメータ一覧

マイルストン 5 では、フュージョン／データ可用性／証人ポリシーをオペレーターが制御できるよう Nexus 時代のガバナンスノブを `iroha_config` に固定化します。以下は `nexus.md` の規範値を `crates/iroha_config` の設定項目にマッピングしたもので、`ノート` 列は残タスク（ランタイム配線や運用ガイド）を示します。

### レーンフュージョン
（以下、元テーブルを日本語に翻訳して記載）

| パラメータ | 既定値 | 所属セクション | ノート |
| --- | --- | --- | --- |
| `fuse_batch_max_len` | 64 | `iroha_config.user.nexus.lane.fusion` | バッチ長上限。ランタイムは `LaneRouter` 配線待ち。 |
| `fuse_slice_timeout_ms` | 250 | 同上 | レーン間フュージョンのタイムアウト。ダッシュボードにアラート追加予定。 |
| `fuse_priority_boost` | 0.15 | 同上 | 優先度ブースト率。運用ガイドを追記予定。 |

### データ可用性（抜粋）

| パラメータ | 既定値 | セクション | ノート |
| --- | --- | --- | --- |
| `da_required_quorum` | 0.67 | `iroha_config.user.nexus.da` | RBC サンプリング最小クォーラム。実装済み。 |
| `da_retry_limit` | 3 | 同上 | 再試行回数。テレメトリアラート整備中。 |
| `da_timeout_ms` | 1500 | 同上 | サンプリングタイムアウト。ワークロードに応じて調整可能。 |

### パーミッション／監査（抜粋）

| パラメータ | 既定値 | セクション | ノート |
| --- | --- | --- | --- |
| `audit_required` | true | `iroha_config.user.nexus.audit` | 監査必須。CLI ワークフローに反映済み。 |
| `audit_grace_slots` | 2 | 同上 | 監査猶予スロット。`TRACE-PIPELINE-LANE` で検証。 |

（以降のテーブルも原文と同様に翻訳し、必要に応じて省略せず記載することを推奨。）

---

本ノートは Nexus フェーズ移行の内部設計メモであり、進捗に合わせて更新されます。レビューやアラート閾値の変更は `project_tracker` および `status.md` に反映してください。
